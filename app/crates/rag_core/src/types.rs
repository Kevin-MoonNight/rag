//! 中間格式與對外介面。

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 中間格式的區塊類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    /// 文字，可帶標題層級。
    Text,
    /// 圖片或頁面影像的參照（沒有可用文字層的頁面、單獨的圖片檔）。
    Image,
}

/// 文件轉換套件輸出的一個區塊。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub kind: BlockKind,
    /// 文字區塊的內容；圖片區塊是空字串。
    pub text: String,
    /// 標題層級（1 最大）；不是標題時為 `None`。
    pub heading_level: Option<u8>,
    /// 圖片區塊的影像檔路徑（PNG 或 JPEG）；文字區塊為 `None`。
    pub image_path: Option<String>,
    /// 頁碼，從 1 起算。
    pub page: u32,
}

impl Block {
    pub fn text(text: impl Into<String>, page: u32) -> Self {
        Self { kind: BlockKind::Text, text: text.into(), heading_level: None, image_path: None, page }
    }

    pub fn heading(text: impl Into<String>, level: u8, page: u32) -> Self {
        Self { kind: BlockKind::Text, text: text.into(), heading_level: Some(level), image_path: None, page }
    }

    pub fn image(path: impl Into<String>, page: u32) -> Self {
        Self { kind: BlockKind::Image, text: String::new(), heading_level: None, image_path: Some(path.into()), page }
    }
}

/// 關鍵字檢索用的稀疏向量：詞的編號與權重，編號遞增且不重複。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SparseVec {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

/// 一筆文字 chunk，準備寫入儲存層。
#[derive(Debug, Clone)]
pub struct TextPoint {
    pub id: u64,
    pub file_id: i64,
    pub page: u32,
    /// 段落跨頁時，結尾所在的頁碼；不跨頁時和 `page` 相同。
    pub page_end: u32,
    /// 這個檔案裡的第幾個 chunk（0 起算）。
    pub chunk_index: u32,
    pub heading: Option<String>,
    pub text: String,
    pub dense: Vec<f32>,
    pub sparse: SparseVec,
}

/// 一筆頁面圖片或圖片檔，準備寫入儲存層。
#[derive(Debug, Clone)]
pub struct ImagePoint {
    pub id: u64,
    pub file_id: i64,
    pub page: u32,
    pub image_path: String,
    pub dense: Vec<f32>,
}

/// 一筆檢索命中：編號與分數（分數越高越相關）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub id: u64,
    pub score: f32,
}

/// 向量模型。實作在模型執行套件（個人電腦是 llama-server 子行程）。
pub trait Embedder {
    /// 問題轉向量（實作要自己加上「問題用」的提示前綴）。
    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>>;
    /// 文件 chunk 轉向量（實作要自己加上「文件用」的提示前綴；`titles` 和 `texts` 等長）。
    fn embed_documents(&mut self, titles: &[String], texts: &[String]) -> Result<Vec<Vec<f32>>>;
    /// 一張圖片轉向量，`bytes` 是 PNG 或 JPEG 檔案內容。
    fn embed_image(&mut self, bytes: &[u8]) -> Result<Vec<f32>>;
}

/// 向量與關鍵字索引的讀寫。實作在儲存套件（個人電腦是 Qdrant Edge）。
pub trait Store {
    fn upsert_text(&mut self, points: &[TextPoint]) -> Result<()>;
    fn upsert_images(&mut self, points: &[ImagePoint]) -> Result<()>;
    /// 把寫入的資料確實存到磁碟。每批寫完後呼叫，之後才能把這一批標記為完成。
    fn flush(&mut self) -> Result<()>;
    /// 刪掉某個檔案的全部資料。
    fn delete_file(&mut self, file_id: i64) -> Result<()>;
    fn search_text(&self, query: &[f32], limit: usize) -> Result<Vec<Hit>>;
    fn search_images(&self, query: &[f32], limit: usize) -> Result<Vec<Hit>>;
    fn search_keywords(&self, query: &SparseVec, limit: usize) -> Result<Vec<Hit>>;
}
