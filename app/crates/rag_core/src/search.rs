//! 查詢：問題 → 向量 → 三份名單 → 合併。

use crate::merge::{take_turns, vectors_then_keywords, Merged};
use crate::text::query_sparse;
use crate::types::{Embedder, Hit, Store};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// 每一份名單各取幾筆候選。
    pub candidates: usize,
    /// 合併後保留幾筆。
    pub limit: usize,
    /// 文字和圖片比相似度時，加在圖片相似度上的補償值。
    /// 0.11 是實驗 76 在真實文件上定的：0.10 以下掃描頁排不進前 5 名，0.13 以上圖片開始搶走文字題的第 1 名。
    pub image_offset: f32,
    /// 用舊的「三份名單輪流」合併（實驗 36），不用預設的「向量合一再和關鍵字輪流」（實驗 76）。
    /// 舊做法配的補償值是 0.08。
    pub take_turns: bool,
    pub use_images: bool,
    pub use_keywords: bool,
    /// 文字向量和關鍵字兩份名單的前幾名都出現的段落優先排在最前面；0 表示不做。
    /// 預設不做：真實文件的評測裡只多對 1 題（128 題），深度 10 以上會把掃描頁往後擠（實驗 59）。
    pub agree_depth: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self { candidates: 50, limit: 20, image_offset: 0.11, take_turns: false, use_images: true, use_keywords: true, agree_depth: 0 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// 合併後的名單。
    pub merged: Vec<Merged>,
    /// 合併前的三份名單，評測時用來分開看各自的表現。
    pub text: Vec<Hit>,
    pub image: Vec<Hit>,
    pub keyword: Vec<Hit>,
}

pub fn search(embedder: &mut dyn Embedder, store: &dyn Store, query: &str, opt: &SearchOptions) -> Result<SearchResult> {
    let vector = embedder.embed_query(query)?;
    let text = store.search_text(&vector, opt.candidates)?;
    let image = if opt.use_images { store.search_images(&vector, opt.candidates)? } else { Vec::new() };
    let keyword = if opt.use_keywords { store.search_keywords(&query_sparse(query), opt.candidates)? } else { Vec::new() };
    let merged = merge(&text, &image, &keyword, opt);
    Ok(SearchResult { merged, text, image, keyword })
}

/// 只用關鍵字搜尋：不需要向量模型，模型還沒載入、或使用者要精確查代號時用。
pub fn search_keywords(store: &dyn Store, query: &str, opt: &SearchOptions) -> Result<SearchResult> {
    let keyword = store.search_keywords(&query_sparse(query), opt.candidates)?;
    let merged = merge(&[], &[], &keyword, opt);
    Ok(SearchResult { merged, keyword, ..Default::default() })
}

fn merge(text: &[Hit], image: &[Hit], keyword: &[Hit], opt: &SearchOptions) -> Vec<Merged> {
    // 「兩邊都找到的先排」只有輪流的做法有
    if opt.take_turns || opt.agree_depth > 0 {
        take_turns(text, image, keyword, opt.image_offset, opt.agree_depth, opt.limit)
    } else {
        vectors_then_keywords(text, image, keyword, opt.image_offset, opt.limit)
    }
}
