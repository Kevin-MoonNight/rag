//! 文件轉換：把檔案轉成中間格式（文字區塊和圖片區塊）。第一版只處理 PDF 和圖片檔。
//!
//! PDF 用 LiteParse（底層是 PDFium）。有文字層的頁面取文字；沒有可用文字層的頁面（掃描檔）
//! 轉成一張頁面圖片；有設定文字辨識（[`ConvertOptions::ocr`]）時，再加上辨識出來的文字。

mod clean;

pub use clean::clean_text;
pub use rag_ocr::{OcrConfig, OCR_VERSION};

use anyhow::{anyhow, Context, Result};
use liteparse::{LiteParse, LiteParseConfig, OutputFormat};
use rag_core::Block;
use std::path::Path;
use std::sync::Mutex;

/// 解析器和文字整理規則的版本字串，記在索引裡；日後換版本時用來判斷要不要重建。
/// `+han1`：相容漢字換成一般漢字（2026-10-14）。
pub const PARSER_VERSION: &str = "liteparse-2.16.0+han1";

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// 一頁的文字少於這麼多字（不算空白），就當成沒有文字層，改用頁面圖片。
    pub min_text_chars: usize,
    /// 頁面圖片的解析度。
    pub image_dpi: f32,
    /// 用 LiteParse 的版面分析找標題。關掉時每頁是一個沒有標題的文字區塊。
    pub detect_headings: bool,
    /// 最多處理幾頁。
    pub max_pages: usize,
    /// 實驗用（實驗 83）：另一個轉換工具（docling.rs）先轉好的 Markdown 所在的資料夾，檔案是 `<資料夾>/<PDF 所在資料夾名>/<PDF 檔名>.md`，
    /// 頁和頁之間用 `<!-- page break -->` 分開。有文字層的頁改用這裡的內容；找不到檔案或那一頁是空的就照舊。
    /// 掃描頁（實驗 87，OCR 的結果）：頁面圖片照舊，另外加上這裡的文字。
    pub text_override: Option<std::path::PathBuf>,
    /// 掃描頁和圖片檔的文字辨識（實驗 99）。`None` 不辨識，只留頁面圖片。
    pub ocr: Option<rag_ocr::OcrConfig>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self { min_text_chars: 20, image_dpi: 110.0, detect_headings: true, max_pages: 2000, text_override: None, ocr: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Converted {
    pub blocks: Vec<Block>,
    pub pages: u32,
    /// 有文字層的頁數。
    pub text_pages: u32,
    /// 轉成頁面圖片的頁數。
    pub image_pages: u32,
    /// 沒有文字、畫面也幾乎全白而略過的頁數。
    pub blank_pages: u32,
    /// 用文字辨識補上文字的頁數。
    pub ocr_pages: u32,
}

/// 頁面圖片每個像素不到這麼多位元組，就當成幾乎全白。
/// 實測（110 dpi、A4）：空白頁 0.021、只有一行表頭的頁 0.029；真正的掃描頁最少 0.108。
const MIN_IMAGE_BYTES_PER_PIXEL: f64 = 0.045;

/// PNG 檔的大小除以像素數。寬高從檔頭的 IHDR 讀出來；不是 PNG 就回傳 `None`。
fn png_bytes_per_pixel(png: &[u8]) -> Option<f64> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(png[16..20].try_into().ok()?) as f64;
    let height = u32::from_be_bytes(png[20..24].try_into().ok()?) as f64;
    (width > 0.0 && height > 0.0).then(|| png.len() as f64 / (width * height))
}

/// 表格的分隔線（`|-----|-----|`）縮成每格三個 `-`：docling.rs 會補到和最寬的格一樣長，標點每個都算一個 token，會吃掉 chunk 的長度。
fn short_table_rules(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    for line in markdown.lines() {
        let t = line.trim();
        if t.starts_with('|') && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
            let cells = t.trim_matches('|').split('|').count();
            out.push('|');
            out.push_str(&"---|".repeat(cells));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

pub fn is_pdf(path: &Path) -> bool {
    has_extension(path, &["pdf"])
}

pub fn is_image(path: &Path) -> bool {
    has_extension(path, &["png", "jpg", "jpeg"])
}

/// 這個檔案第一版會不會處理。
pub fn is_supported(path: &Path) -> bool {
    is_pdf(path) || is_image(path)
}

fn has_extension(path: &Path, list: &[&str]) -> bool {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).is_some_and(|e| list.contains(&e.as_str()))
}

pub struct Converter {
    options: ConvertOptions,
    text_parser: LiteParse,
    image_parser: LiteParse,
    runtime: tokio::runtime::Runtime,
    /// 文字辨識的模型：第一次遇到掃描頁才載入（約 0.6 GB），建完索引用 [`Converter::unload_ocr`] 卸下。
    ocr: Mutex<Option<rag_ocr::Ocr>>,
    /// 模型載入失敗的原因。失敗後這個轉換器不再嘗試，掃描頁只留圖片。
    ocr_error: Mutex<Option<String>>,
    /// 文字辨識用幾個執行緒（白天和夜間不同，見 [`Converter::set_ocr_threads`]）；0 表示照設定。
    ocr_threads: std::sync::atomic::AtomicUsize,
}

impl Converter {
    pub fn new(options: ConvertOptions) -> Result<Self> {
        let base = LiteParseConfig { ocr_enabled: false, quiet: true, max_pages: options.max_pages, continue_on_page_error: true, ..Default::default() };
        let text_config = LiteParseConfig {
            output_format: if options.detect_headings { OutputFormat::Markdown } else { OutputFormat::Text },
            extract_links: false,
            ..base.clone()
        };
        let image_config = LiteParseConfig { dpi: options.image_dpi, ..base };
        let runtime = tokio::runtime::Builder::new_current_thread().build().context("無法建立執行環境")?;
        Ok(Self {
            options,
            text_parser: LiteParse::new(text_config),
            image_parser: LiteParse::new(image_config),
            runtime,
            ocr: Mutex::new(None),
            ocr_error: Mutex::new(None),
            ocr_threads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// 轉換一個檔案。頁面圖片寫到 `image_dir`，檔名以 `image_prefix` 開頭。
    pub fn convert(&self, path: &Path, image_dir: &Path, image_prefix: &str) -> Result<Converted> {
        self.convert_with(path, image_dir, image_prefix, true)
    }

    /// 和 [`Converter::convert`] 相同，但不做文字辨識（補舊索引的資料時用：那些檔案當初沒有辨識，段落編號才對得上）。
    pub fn convert_without_ocr(&self, path: &Path, image_dir: &Path, image_prefix: &str) -> Result<Converted> {
        self.convert_with(path, image_dir, image_prefix, false)
    }

    fn convert_with(&self, path: &Path, image_dir: &Path, image_prefix: &str, ocr: bool) -> Result<Converted> {
        if is_image(path) {
            let p = path.to_str().context("檔案路徑不是合法的文字")?;
            let mut out = Converted { blocks: vec![Block::image(p, 1)], pages: 1, image_pages: 1, ..Default::default() };
            if ocr {
                let bytes = std::fs::read(path).with_context(|| format!("讀不到 {}", path.display()))?;
                if let Some(text) = self.recognize(&bytes) {
                    out.blocks.extend(clean::markdown_blocks(&text, 1));
                    out.ocr_pages += 1;
                }
            }
            return Ok(out);
        }
        if !is_pdf(path) {
            return Err(anyhow!("不支援的檔案類型：{}", path.display()));
        }
        self.convert_pdf(path, image_dir, image_prefix, ocr)
    }

    /// 這個轉換器會不會做文字辨識（有設定，而且模型沒有載入失敗）。
    pub fn ocr_enabled(&self) -> bool {
        self.options.ocr.is_some() && self.ocr_error().is_none()
    }

    /// 文字辨識模型載入失敗的原因。
    pub fn ocr_error(&self) -> Option<String> {
        self.ocr_error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 文字辨識改用幾個執行緒。和目前載入的不同時先卸下，下次遇到掃描頁用新的數目載入。
    pub fn set_ocr_threads(&self, threads: usize) {
        if self.ocr_threads.swap(threads, std::sync::atomic::Ordering::SeqCst) != threads {
            self.unload_ocr();
        }
    }

    /// 卸下文字辨識模型（下次遇到掃描頁再載入）。
    pub fn unload_ocr(&self) {
        *self.ocr.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// 辨識一張頁面圖片。沒有設定、模型載入失敗、辨識失敗或字數太少都回傳 `None`（照舊只留圖片）。
    fn recognize(&self, image: &[u8]) -> Option<String> {
        let config = self.options.ocr.as_ref()?;
        if self.ocr_error().is_some() {
            return None;
        }
        let mut ocr = self.ocr.lock().unwrap_or_else(|e| e.into_inner());
        if ocr.is_none() {
            let mut config = config.clone();
            match self.ocr_threads.load(std::sync::atomic::Ordering::SeqCst) {
                0 => {}
                n => config.threads = n,
            }
            match rag_ocr::Ocr::load(&config) {
                Ok(loaded) => *ocr = Some(loaded),
                Err(e) => {
                    eprintln!("文字辨識停用：{e:#}");
                    *self.ocr_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(format!("{e:#}"));
                    return None;
                }
            }
        }
        let text = ocr.as_ref()?.recognize(image).ok()?;
        (text.chars().filter(|c| !c.is_whitespace()).count() >= self.options.min_text_chars).then_some(text)
    }

    /// 實驗用：讀另一個轉換工具先轉好的 Markdown，依頁拆開（見 [`ConvertOptions::text_override`]）。
    fn override_pages(&self, path: &Path) -> Vec<String> {
        let Some(dir) = &self.options.text_override else { return Vec::new() };
        let (Some(folder), Some(stem)) = (path.parent().and_then(|p| p.file_name()), path.file_stem()) else { return Vec::new() };
        // 檔名裡常有「（107.11.08）」這種點，不能用 with_extension
        let file = dir.join(folder).join(format!("{}.md", stem.to_string_lossy()));
        match std::fs::read_to_string(&file) {
            Ok(text) => text.split("<!-- page break -->").map(short_table_rules).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn convert_pdf(&self, path: &Path, image_dir: &Path, image_prefix: &str, use_ocr: bool) -> Result<Converted> {
        let input = path.to_str().context("檔案路徑不是合法的文字")?;
        let parsed = self.runtime.block_on(self.text_parser.parse(input)).map_err(|e| anyhow!("解析 PDF 失敗：{e}"))?;

        let mut out = Converted { pages: parsed.total_pages, ..Default::default() };
        let mut image_pages: Vec<u32> = Vec::new();
        // 文字很少的頁先記下僅有的幾個字：頁面圖片幾乎全白時，這幾個字就是整頁的內容
        let mut few_words: Vec<(u32, String)> = Vec::new();
        // 先照頁碼順序排好每頁的區塊，圖片頁之後再補上
        let mut per_page: Vec<(u32, Vec<Block>)> = Vec::new();
        let override_pages = self.override_pages(path);
        for page in &parsed.pages {
            let number = page.page_number as u32;
            let visible = page.text.chars().filter(|c| !c.is_whitespace()).count();
            if visible < self.options.min_text_chars {
                image_pages.push(number);
                few_words.push((number, clean_text(&page.text)));
                per_page.push((number, Vec::new()));
                continue;
            }
            if clean::is_garbled(&page.text) {
                // 文字層是亂碼（字型的對照表是錯的）：文字不能用，當成掃描頁
                image_pages.push(number);
                per_page.push((number, Vec::new()));
                continue;
            }
            out.text_pages += 1;
            let replaced = (number as usize).checked_sub(1).and_then(|i| override_pages.get(i)).filter(|m| !m.trim().is_empty());
            let blocks = if let Some(markdown) = replaced {
                clean::markdown_blocks(markdown, number)
            } else if self.options.detect_headings && !page.markdown.trim().is_empty() {
                clean::markdown_blocks(&page.markdown, number)
            } else {
                vec![Block::text(clean_text(&page.text), number)]
            };
            per_page.push((number, blocks));
        }

        if !image_pages.is_empty() {
            std::fs::create_dir_all(image_dir).with_context(|| format!("無法建立資料夾 {}", image_dir.display()))?;
            let shots = self
                .runtime
                .block_on(self.image_parser.screenshot(input, Some(image_pages.clone())))
                .map_err(|e| anyhow!("頁面轉圖片失敗：{e}"))?;
            for shot in shots {
                let Some(entry) = per_page.iter_mut().find(|(n, _)| *n == shot.page_num) else { continue };
                if png_bytes_per_pixel(&shot.image_bytes).is_some_and(|d| d < MIN_IMAGE_BYTES_PER_PIXEL) {
                    // 幾乎全白：不是掃描頁，是空白頁或只有幾個字的頁
                    match few_words.iter().find(|(n, text)| *n == shot.page_num && !text.is_empty()) {
                        Some((_, text)) => {
                            entry.1.push(Block::text(text.clone(), shot.page_num));
                            out.text_pages += 1;
                        }
                        None => out.blank_pages += 1,
                    }
                    continue;
                }
                let file = image_dir.join(format!("{image_prefix}_p{:04}.png", shot.page_num));
                std::fs::write(&file, &shot.image_bytes).with_context(|| format!("無法寫入 {}", file.display()))?;
                let file = file.to_str().context("圖片路徑不是合法的文字")?.to_string();
                entry.1.push(Block::image(file, shot.page_num));
                out.image_pages += 1;
                // 掃描頁的文字：實驗用的外部辨識結果（實驗 87）優先，沒有就自己辨識（實驗 99）；頁面圖片之外再加上文字段落
                let given = (shot.page_num as usize).checked_sub(1).and_then(|i| override_pages.get(i));
                if let Some(markdown) = given.filter(|m| m.chars().filter(|c| !c.is_whitespace()).count() >= self.options.min_text_chars) {
                    entry.1.extend(clean::markdown_blocks(markdown, shot.page_num));
                } else if let Some(text) = use_ocr.then(|| self.recognize(&shot.image_bytes)).flatten() {
                    entry.1.extend(clean::markdown_blocks(&text, shot.page_num));
                    out.ocr_pages += 1;
                }
            }
        }

        out.blocks = per_page.into_iter().flat_map(|(_, b)| b).collect();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_density_reads_the_header() {
        // 909 x 1286 的檔頭，後面補到 25,000 位元組：相當於一張空白的 A4 頁面圖片
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
        png.extend_from_slice(&909u32.to_be_bytes());
        png.extend_from_slice(&1286u32.to_be_bytes());
        png.resize(25_000, 0);
        let density = png_bytes_per_pixel(&png).unwrap();
        assert!((density - 0.0214).abs() < 0.001 && density < MIN_IMAGE_BYTES_PER_PIXEL);
        png.resize(126_000, 0);
        assert!(png_bytes_per_pixel(&png).unwrap() > MIN_IMAGE_BYTES_PER_PIXEL);
        assert_eq!(png_bytes_per_pixel(b"not a png"), None);
    }

    #[test]
    fn override_markdown_is_split_by_page() {
        let dir = std::env::temp_dir().join(format!("rag-override-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("FL1")).unwrap();
        std::fs::write(dir.join("FL1").join("辦法 對照表（107.11.08）.md"), "第一頁\n|-------|:------|\n<!-- page break -->\n\n<!-- page break -->第三頁").unwrap();
        let converter = Converter::new(ConvertOptions { text_override: Some(dir.clone()), ..Default::default() }).unwrap();
        let pages = converter.override_pages(Path::new("/docs/FL1/辦法 對照表（107.11.08）.pdf"));
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].trim(), "第一頁\n|---|---|");
        assert!(pages[1].trim().is_empty());
        assert_eq!(pages[2].trim(), "第三頁");
        assert!(converter.override_pages(Path::new("/docs/FL2/a.pdf")).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
