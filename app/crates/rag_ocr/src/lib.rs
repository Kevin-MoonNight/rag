//! 掃描頁的文字辨識（OCR），用 PP-OCRv6 small（實驗 99）：先找出每一行，再逐行辨識，所以不會像視覺語言模型那樣整段漏掉。
//! 每頁約 0.8 秒（開發機 CPU、6 執行緒），記憶體約 0.6 GB，模型 30 MB。
//!
//! 辨識完再做兩件事：
//! - 直式頁重排：偵測框大多是瘦長的，就一欄一欄由右往左排、欄內由上往下（函式庫的順序是由上往下、由左往右，直式公文的句子會接錯）。
//! - 簡體字換回繁體：模型的字典混有簡體字，會把「額」「頁」讀成「额」「页」。只換「一個簡體字只對應一個繁體字」的字
//!   （OpenCC STCharacters，Apache-2.0，3,736 個字；另外把「别」固定成「別」）；一對多的字（发、台……）不碰。

use anyhow::{anyhow, Context, Result};
use oar_ocr::core::config::OrtSessionConfig;
use oar_ocr::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 辨識結果的版本字串，記在索引的解析器版本裡；換模型或後處理時改它，掃描頁才會重做。
pub const OCR_VERSION: &str = "ppocrv6s1";

#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// 偵測模型、辨識模型、字典。
    pub det: PathBuf,
    pub rec: PathBuf,
    pub dict: PathBuf,
    /// ONNX Runtime 的動態函式庫（onnxruntime.dll／libonnxruntime.so…）。
    pub runtime: PathBuf,
    pub threads: usize,
}

impl OcrConfig {
    /// `models` 底下的 PP-OCRv6 small 三個檔案。
    pub fn in_dir(models: &Path, runtime: impl Into<PathBuf>) -> Self {
        Self {
            det: models.join("pp-ocrv6_small_det.onnx"),
            rec: models.join("pp-ocrv6_small_rec.onnx"),
            dict: models.join("ppocrv6_dict.txt"),
            runtime: runtime.into(),
            threads: 6,
        }
    }

    /// 需要的檔案是否都在。
    pub fn available(&self) -> bool {
        [&self.det, &self.rec, &self.dict, &self.runtime].iter().all(|p| p.is_file())
    }
}

/// ONNX Runtime 一個程序只能載入一次；第一次的結果（成功或錯誤訊息）留下來給之後的呼叫。
static RUNTIME: OnceLock<std::result::Result<(), String>> = OnceLock::new();

fn load_runtime(path: &Path) -> Result<()> {
    RUNTIME
        .get_or_init(|| match ort::init_from(path) {
            Ok(builder) => {
                builder.commit();
                Ok(())
            }
            Err(e) => Err(format!("無法載入 ONNX Runtime（{}）：{e}", path.display())),
        })
        .clone()
        .map_err(|e| anyhow!(e))
}

pub struct Ocr {
    pipeline: OAROCR,
}

impl Ocr {
    pub fn load(config: &OcrConfig) -> Result<Self> {
        load_runtime(&config.runtime)?;
        let pipeline = OAROCRBuilder::new(config.det.as_path(), config.rec.as_path(), config.dict.as_path())
            .ort_session(OrtSessionConfig::default().with_intra_threads(config.threads.max(1)))
            .build()
            .map_err(|e| anyhow!("無法載入文字辨識模型：{e}"))?;
        Ok(Self { pipeline })
    }

    /// 辨識一張圖片（PNG 或 JPEG 的檔案內容），回傳一行一列的文字。
    pub fn recognize(&self, image_bytes: &[u8]) -> Result<String> {
        let image = image::load_from_memory(image_bytes).context("讀不懂圖片")?.to_rgb8();
        let results = self.pipeline.predict(vec![image]).map_err(|e| anyhow!("文字辨識失敗：{e}"))?;
        let Some(result) = results.first() else { return Ok(String::new()) };
        let lines: Vec<Line> = result
            .recognized_text_regions()
            .map(|r| Line {
                x0: r.bounding_box.x_min(),
                x1: r.bounding_box.x_max(),
                y0: r.bounding_box.y_min(),
                y1: r.bounding_box.y_max(),
                text: r.text.as_deref().unwrap_or("").to_string(),
            })
            .collect();
        let text = vertical_order(lines).iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
        Ok(to_traditional(&text))
    }
}

struct Line {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    text: String,
}

/// 直式頁（字大多在高度是寬度 2 倍以上的框裡）重排成由右往左一欄一欄、欄內由上往下；橫式頁照原本的順序。
/// 一個框和某一欄重疊的寬度超過兩者中較窄者的一半，就算同一欄。
fn vertical_order(mut lines: Vec<Line>) -> Vec<Line> {
    let chars = |l: &Line| l.text.chars().count();
    let total: usize = lines.iter().map(chars).sum();
    let tall: usize = lines.iter().filter(|l| l.y1 - l.y0 >= 2.0 * (l.x1 - l.x0)).map(chars).sum();
    if 2 * tall <= total {
        return lines;
    }
    lines.sort_by(|a, b| b.x1.total_cmp(&a.x1));
    let mut columns: Vec<(f32, f32, Vec<Line>)> = Vec::new();
    for line in lines {
        let found = columns.iter_mut().find(|(x0, x1, _)| {
            let overlap = x1.min(line.x1) - x0.max(line.x0);
            overlap >= 0.5 * (*x1 - *x0).min(line.x1 - line.x0)
        });
        match found {
            Some((x0, x1, members)) => {
                *x0 = x0.min(line.x0);
                *x1 = x1.max(line.x1);
                members.push(line);
            }
            None => columns.push((line.x0, line.x1, vec![line])),
        }
    }
    columns.sort_by(|a, b| b.1.total_cmp(&a.1));
    columns
        .into_iter()
        .flat_map(|(_, _, mut members)| {
            members.sort_by(|a, b| a.y0.total_cmp(&b.y0));
            members
        })
        .collect()
}

/// 簡體字換回繁體（只換一對一的字）。
pub fn to_traditional(text: &str) -> String {
    static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let chars: Vec<char> = include_str!("s2t.txt").chars().collect();
        chars.chunks_exact(2).map(|p| (p[0], p[1])).collect()
    });
    text.chars().map(|c| *table.get(&c).unwrap_or(&c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f32, y0: f32, x1: f32, y1: f32, text: &str) -> Line {
        Line { x0, x1, y0, y1, text: text.to_string() }
    }

    #[test]
    fn simplified_only_characters_become_traditional() {
        assert_eq!(to_traditional("金额第一页速别"), "金額第一頁速別");
        // 一對多的字不碰，本來就是繁體的字不變
        assert_eq!(to_traditional("发台後"), "发台後");
    }

    #[test]
    fn vertical_pages_read_right_to_left() {
        // 兩欄直書，右欄分成上下兩個框；左邊一個頁碼
        let lines = vec![
            line(10.0, 900.0, 60.0, 920.0, "第一頁"),
            line(100.0, 0.0, 130.0, 400.0, "左欄"),
            line(200.0, 500.0, 230.0, 800.0, "右欄下"),
            line(200.0, 0.0, 230.0, 400.0, "右欄上"),
        ];
        let order: Vec<String> = vertical_order(lines).into_iter().map(|l| l.text).collect();
        assert_eq!(order, ["右欄上", "右欄下", "左欄", "第一頁"]);
    }

    #[test]
    fn horizontal_pages_keep_their_order() {
        let lines = vec![line(0.0, 0.0, 400.0, 30.0, "第一行"), line(0.0, 40.0, 400.0, 70.0, "第二行")];
        let order: Vec<String> = vertical_order(lines).into_iter().map(|l| l.text).collect();
        assert_eq!(order, ["第一行", "第二行"]);
    }
}
