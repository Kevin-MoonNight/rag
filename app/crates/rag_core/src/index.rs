//! 建索引：一份文件的中間格式 → chunk → 向量 → 寫入儲存層。

use crate::chunk::{chunk_blocks, ChunkOptions};
use crate::ids::{point_id, PointKind};
use crate::text::document_sparse;
use crate::types::{Block, BlockKind, Embedder, ImagePoint, Store, TextPoint};
use anyhow::{ensure, Context, Result};

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub chunk: ChunkOptions,
    /// 一次送幾個 chunk 給向量模型，也是一次寫入儲存層的筆數。
    pub batch_size: usize,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self { chunk: ChunkOptions::default(), batch_size: 8 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub text_chunks: usize,
    pub images: usize,
}

/// 把一份文件寫進索引。`title` 會放進文件用提示前綴的標題欄位（通常是檔名）。
///
/// 每一批寫完會呼叫 [`Store::flush`]；整份文件成功後由呼叫端在自己的紀錄裡標記完成。
/// 同一份內容重做一次會得到同樣的編號，只會覆蓋，不會重複。
pub fn index_document(
    embedder: &mut dyn Embedder,
    store: &mut dyn Store,
    file_id: i64,
    content_hash: &str,
    title: &str,
    blocks: &[Block],
    opt: &IndexOptions,
) -> Result<IndexReport> {
    let mut report = IndexReport::default();
    let batch = opt.batch_size.max(1);

    let chunks = chunk_blocks(blocks, &opt.chunk);
    for group in chunks.chunks(batch) {
        let titles: Vec<String> = group.iter().map(|_| title.to_string()).collect();
        let texts: Vec<String> = group.iter().map(|c| c.text.clone()).collect();
        let vectors = embedder.embed_documents(&titles, &texts)?;
        ensure!(vectors.len() == group.len(), "向量模型回傳 {} 筆，預期 {} 筆", vectors.len(), group.len());
        let points: Vec<TextPoint> = group
            .iter()
            .zip(vectors)
            .map(|(c, dense)| TextPoint {
                id: point_id(content_hash, PointKind::Text, c.page, c.index),
                file_id,
                page: c.page,
                page_end: c.page_end,
                chunk_index: c.index,
                heading: c.heading.clone(),
                sparse: document_sparse(&c.text),
                text: c.text.clone(),
                dense,
            })
            .collect();
        store.upsert_text(&points)?;
        store.flush()?;
        report.text_chunks += points.len();
    }

    let mut image_index = 0u32;
    for block in blocks.iter().filter(|b| b.kind == BlockKind::Image) {
        let path = block.image_path.as_deref().context("圖片區塊沒有影像檔路徑")?;
        let bytes = std::fs::read(path).with_context(|| format!("讀不到影像檔 {path}"))?;
        let dense = embedder.embed_image(&bytes)?;
        store.upsert_images(&[ImagePoint {
            id: point_id(content_hash, PointKind::Image, block.page, image_index),
            file_id,
            page: block.page,
            image_path: path.to_string(),
            dense,
        }])?;
        store.flush()?;
        image_index += 1;
        report.images += 1;
    }
    Ok(report)
}

/// 用已存的原文重算向量（換了向量模型或關鍵字的切法時），不必重新解析檔案。
///
/// `text` 和 `images` 的編號、位置、原文照舊，向量和關鍵字欄位會被重算後覆蓋寫入。
pub fn reembed_document(
    embedder: &mut dyn Embedder,
    store: &mut dyn Store,
    title: &str,
    text: Vec<TextPoint>,
    images: Vec<ImagePoint>,
    batch_size: usize,
) -> Result<IndexReport> {
    let mut report = IndexReport::default();
    for group in text.chunks(batch_size.max(1)) {
        let titles: Vec<String> = group.iter().map(|_| title.to_string()).collect();
        let texts: Vec<String> = group.iter().map(|p| p.text.clone()).collect();
        let vectors = embedder.embed_documents(&titles, &texts)?;
        ensure!(vectors.len() == group.len(), "向量模型回傳 {} 筆，預期 {} 筆", vectors.len(), group.len());
        let points: Vec<TextPoint> = group
            .iter()
            .zip(vectors)
            .map(|(p, dense)| TextPoint { sparse: document_sparse(&p.text), dense, ..p.clone() })
            .collect();
        store.upsert_text(&points)?;
        store.flush()?;
        report.text_chunks += points.len();
    }
    for point in images {
        let bytes = std::fs::read(&point.image_path).with_context(|| format!("讀不到影像檔 {}", point.image_path))?;
        let dense = embedder.embed_image(&bytes)?;
        store.upsert_images(&[ImagePoint { dense, ..point }])?;
        store.flush()?;
        report.images += 1;
    }
    Ok(report)
}
