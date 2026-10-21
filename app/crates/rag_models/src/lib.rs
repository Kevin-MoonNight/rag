//! 模型執行：把 llama-server 當子行程管理，透過本機 HTTP 取向量。
//!
//! 模型不載入到應用程式自己的行程裡。要用時啟動子行程，閒置一段時間後整個行程結束，記憶體完全還給系統。

mod generate;
#[cfg(windows)]
mod job;
mod server;
pub mod system;

pub use generate::{GenerationReport, LlamaGenerator};
pub use server::{own_pids, LlamaServer, ServerConfig, ServerMode};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use rag_core::Embedder;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// 用到時才啟動的子行程，閒置太久就結束，記憶體完全還給系統。
pub(crate) struct OnDemand {
    config: ServerConfig,
    server: Option<LlamaServer>,
    last_used: Instant,
}

impl OnDemand {
    pub(crate) fn new(config: ServerConfig) -> Self {
        Self { config, server: None, last_used: Instant::now() }
    }

    pub(crate) fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// 改執行緒數和背景模式。和目前不同時先卸載，下次使用時用新的設定啟動。
    pub(crate) fn set_profile(&mut self, threads: usize, background: bool) {
        if self.config.threads != threads || self.config.background != background {
            self.unload();
            self.config.threads = threads;
            self.config.background = background;
        }
    }

    pub(crate) fn is_loaded(&self) -> bool {
        self.server.is_some()
    }

    pub(crate) fn pid(&self) -> Option<u32> {
        self.server.as_ref().map(|s| s.pid())
    }

    /// 還沒啟動就啟動，回傳子行程。
    pub(crate) fn get(&mut self) -> Result<&LlamaServer> {
        if self.server.is_none() {
            self.server = Some(LlamaServer::start(&self.config)?);
        }
        self.last_used = Instant::now();
        Ok(self.server.as_ref().unwrap())
    }

    pub(crate) fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    pub(crate) fn unload(&mut self) {
        self.server = None;
    }

    pub(crate) fn unload_if_idle(&mut self, idle: Duration) -> bool {
        if self.server.is_some() && self.last_used.elapsed() >= idle {
            self.unload();
            return true;
        }
        false
    }

    /// 請求失敗之後呼叫：子行程如果已經不在了，下次使用時重新啟動。
    pub(crate) fn forget_if_dead(&mut self) {
        if let Some(server) = self.server.as_mut() {
            if !server.is_running() {
                self.server = None;
            }
        }
    }
}

/// EmbeddingGemma 的提示前綴：問題用。
pub const QUERY_PREFIX: &str = "task: search result | query: ";
/// llama-server 的圖片位置標記。預設每次啟動隨機產生，啟動子行程時用環境變數固定成這個值。
pub(crate) const MEDIA_MARKER: &str = "<__media__>";

/// EmbeddingGemma 的提示前綴：文件用。標題欄位沒有內容時填 none。
pub fn document_prefix(title: &str) -> String {
    let title: String = title.chars().map(|c| if c == '|' || c.is_control() { ' ' } else { c }).collect();
    let title = title.trim();
    format!("title: {} | text: ", if title.is_empty() { "none" } else { title })
}

/// 透過 llama-server 子行程取向量。第一次使用時才啟動子行程。
pub struct LlamaEmbedder {
    process: OnDemand,
}

impl LlamaEmbedder {
    pub fn new(config: ServerConfig) -> Self {
        Self { process: OnDemand::new(config) }
    }

    pub fn is_loaded(&self) -> bool {
        self.process.is_loaded()
    }

    /// 子行程的行程編號（量記憶體用）。
    pub fn pid(&self) -> Option<u32> {
        self.process.pid()
    }

    /// 先把模型載入，不等到第一次查詢。
    pub fn load(&mut self) -> Result<()> {
        self.process.get().map(|_| ())
    }

    /// 結束子行程，釋放模型佔用的記憶體。
    pub fn unload(&mut self) {
        self.process.unload();
    }

    /// 改執行緒數和背景模式（白天、夜間切換）。設定不同時會卸載，下次使用時重新啟動。
    pub fn set_profile(&mut self, threads: usize, background: bool) {
        self.process.set_profile(threads, background);
    }

    /// 超過 `idle` 沒有使用就卸載。回傳這次有沒有卸載。
    pub fn unload_if_idle(&mut self, idle: Duration) -> bool {
        self.process.unload_if_idle(idle)
    }

    fn embedding(&mut self, content: Value) -> Result<Vec<Vec<f32>>> {
        let result = self.process.get()?.post_json("/embedding", &json!({ "content": content }));
        self.process.touch();
        let value = result.inspect_err(|_| self.process.forget_if_dead())?;
        parse_embeddings(&value)
    }

    /// 一段文字轉成向量。超過模型一次能處理的長度時，從後面截短再試：向量只代表前面的部分，
    /// 但不會因為一段太長就整個失敗。chunk 的切分用的是估計的 token 數，極少數內容（整頁的符號或亂碼）會超過。
    fn embed_one(&mut self, prompt: &str) -> Result<Vec<f32>> {
        let mut chars = prompt.chars().count();
        for _ in 0..8 {
            let text: String = prompt.chars().take(chars).collect();
            match self.embedding(json!([text])) {
                Ok(mut v) => return v.pop().context("向量模型沒有回傳結果"),
                Err(e) if is_too_large(&e) && chars > 16 => chars = chars * 2 / 3,
                Err(e) => return Err(e),
            }
        }
        bail!("這段文字截短之後還是超過模型一次能處理的長度")
    }
}

/// llama-server 回報輸入超過一次能處理的 token 數。
fn is_too_large(e: &anyhow::Error) -> bool {
    format!("{e:#}").contains("too large to process")
}

/// 回應是 `[{"index": 0, "embedding": [...]}, ...]`，embedding 也可能多包一層。
fn parse_embeddings(value: &Value) -> Result<Vec<Vec<f32>>> {
    let single;
    let items: &[Value] = match value {
        Value::Array(a) => a,
        Value::Object(_) => {
            single = [value.clone()];
            &single
        }
        _ => bail!("向量模型的回應格式不對"),
    };
    let mut out: Vec<(u64, Vec<f32>)> = Vec::with_capacity(items.len());
    for (n, item) in items.iter().enumerate() {
        let index = item.get("index").and_then(Value::as_u64).unwrap_or(n as u64);
        let mut emb = item.get("embedding").ok_or_else(|| anyhow!("回應裡沒有 embedding 欄位：{}", brief(item)))?;
        if let Some(inner) = emb.get(0).filter(|v| v.is_array()) {
            emb = inner;
        }
        let list = emb.as_array().context("embedding 不是陣列")?;
        let mut vector: Vec<f32> = list.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
        normalize(&mut vector);
        out.push((index, vector));
    }
    out.sort_by_key(|p| p.0);
    Ok(out.into_iter().map(|p| p.1).collect())
}

fn brief(v: &Value) -> String {
    v.to_string().chars().take(200).collect()
}

fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt() as f32;
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
}

impl Embedder for LlamaEmbedder {
    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        self.embed_one(&format!("{QUERY_PREFIX}{text}"))
    }

    fn embed_documents(&mut self, titles: &[String], texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let prompts: Vec<String> = texts
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{}{}", document_prefix(titles.get(i).map(String::as_str).unwrap_or("")), t))
            .collect();
        let vectors = match self.embedding(json!(prompts)) {
            Ok(v) => v,
            // 這一批裡有一段太長：改成一段一段送，太長的那段截短
            Err(e) if is_too_large(&e) => prompts.iter().map(|p| self.embed_one(p)).collect::<Result<Vec<_>>>()?,
            Err(e) => return Err(e),
        };
        if vectors.len() != texts.len() {
            bail!("送出 {} 段文字，只收到 {} 個向量", texts.len(), vectors.len());
        }
        Ok(vectors)
    }

    fn embed_image(&mut self, bytes: &[u8]) -> Result<Vec<f32>> {
        if self.process.config().mmproj.is_none() {
            bail!("這個模型設定沒有載入影像編碼器，不能處理圖片");
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let mut v = self.embedding(json!({ "prompt_string": MEDIA_MARKER, "multimodal_data": [b64] }))?;
        v.pop().context("向量模型沒有回傳結果")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes() {
        assert_eq!(document_prefix(""), "title: none | text: ");
        assert_eq!(document_prefix(" 授信|要點\n"), "title: 授信 要點 | text: ");
    }

    #[test]
    fn parse_both_shapes() {
        let v: Value = serde_json::from_str(r#"[{"index":1,"embedding":[[0.0,2.0]]},{"index":0,"embedding":[3.0,4.0]}]"#).unwrap();
        let e = parse_embeddings(&v).unwrap();
        assert_eq!(e, vec![vec![0.6, 0.8], vec![0.0, 1.0]]);
        let v: Value = serde_json::from_str(r#"{"index":0,"embedding":[[1.0,0.0]]}"#).unwrap();
        assert_eq!(parse_embeddings(&v).unwrap(), vec![vec![1.0, 0.0]]);
        assert!(parse_embeddings(&serde_json::json!({"error": "x"})).is_err());
    }
}
