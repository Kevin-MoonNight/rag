//! 儲存：向量和關鍵字索引放 Qdrant Edge，檔案清單、chunk 原文和設定放 SQLite。
//!
//! 兩者都是嵌在應用程式行程裡的函式庫，不需要另外啟動服務。資料全部在使用者指定的資料夾底下：
//! `<資料夾>/vectors-text/`、`<資料夾>/vectors-image/`（向量）和 `<資料夾>/index.sqlite`。

mod files;
mod vectors;

pub use files::{ChunkKind, ChunkRow, FileRow, FileStatus};
pub use vectors::Quantization;

use anyhow::{bail, Context, Result};
use rag_core::{Hit, ImagePoint, SparseVec, Store, TextPoint};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// 向量的維度（EmbeddingGemma）。
pub const VECTOR_SIZE: usize = 768;

pub struct LocalStore {
    dir: PathBuf,
    /// 文字向量和關鍵字。
    text: qdrant_edge::EdgeShard,
    /// 圖片向量。
    image: qdrant_edge::EdgeShard,
    db: Connection,
    /// 這份索引實際用的量化方式（建立時決定）。
    quantization: Quantization,
    /// 向量搜尋時，量化向量粗篩取幾倍的候選，再用完整向量重新計分。
    oversampling: f64,
}

impl LocalStore {
    /// 開啟資料夾裡的索引；沒有就建立新的。
    pub fn open(dir: &Path) -> Result<Self> {
        Self::open_with(dir, Quantization::default())
    }

    /// 開啟索引；沒有就用指定的量化方式建立新的（已經存在的索引沿用它建立時的量化方式）。
    pub fn open_with(dir: &Path, quantization: Quantization) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("無法建立資料夾 {}", dir.display()))?;
        if dir.join("qdrant").join("edge_config.json").exists() {
            bail!("{} 裡的索引是舊格式（文字和圖片向量放在同一個資料庫），請刪除這個資料夾後重新建立索引", dir.display());
        }
        // 已經建好的索引沿用它自己的量化方式（例如 2026-10-15 以前建的 int8 索引）
        let quantization = Quantization::of_shard(&dir.join("vectors-text")).unwrap_or(quantization);
        let text = vectors::open_shard(&dir.join("vectors-text"), vectors::ShardKind::Text, quantization)?;
        let image = vectors::open_shard(&dir.join("vectors-image"), vectors::ShardKind::Image, quantization)?;
        let db = files::open_db(&dir.join("index.sqlite"))?;
        Ok(Self { dir: dir.to_path_buf(), text, image, db, quantization, oversampling: quantization.default_oversampling() })
    }

    /// 這份索引記憶體裡放的量化向量是哪一種。
    pub fn quantization(&self) -> Quantization {
        self.quantization
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 改變粗篩的倍數（預設依量化方式：int8 2 倍、1 位元 5 倍，見 [`Quantization::default_oversampling`]）。
    pub fn set_oversampling(&mut self, oversampling: f64) {
        self.oversampling = oversampling;
    }

    /// 把另一份索引的向量原樣複製過來（量化方式用這份索引自己的）。實驗用：換量化方式不必重算向量。
    /// 回傳（文字點數、圖片點數）。
    pub fn copy_vectors_from(&mut self, other: &LocalStore) -> Result<(usize, usize)> {
        let text = vectors::copy_points(&other.text, &self.text)?;
        let image = vectors::copy_points(&other.image, &self.image)?;
        Ok((text, image))
    }

    /// 放大索引（實驗用，量大規模時的記憶體和速度）：每個檔案和它的 chunk、向量多複製 `copies` 份。
    /// 複本的路徑是「/副本N」加上原路徑；稠密向量加上 `noise` 大小的擾動（見 `vectors::replicate_points`）。
    /// 回傳（新增的文字點數, 新增的圖片點數）。
    pub fn replicate(&mut self, copies: usize, noise: f32) -> Result<(usize, usize)> {
        let files = files::duplicate_files(&self.db, copies)?;
        let new_id = |id: u64, c: usize| {
            // splitmix64：不同的（原編號, 第幾份）幾乎不會撞號
            let mut z = id ^ (c as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        files::duplicate_chunks(&self.db, &files, &new_id)?;
        let map = |id: u64, file: i64, c: usize| files.get(&(file, c)).map(|&f| (new_id(id, c), f));
        let text = vectors::replicate_points(&self.text, copies, noise, &map)?;
        let image = vectors::replicate_points(&self.image, copies, noise, &map)?;
        Ok((text, image))
    }

    /// 整理向量索引：合併小區段、建立圖索引和量化後的向量。建完一批索引後呼叫，期間會佔用 CPU。
    /// 回傳這次有沒有東西被整理。
    pub fn optimize(&self) -> Result<bool> {
        let text = vectors::optimize(&self.text)?;
        let image = vectors::optimize(&self.image)?;
        Ok(text || image)
    }
}

impl Store for LocalStore {
    fn upsert_text(&mut self, points: &[TextPoint]) -> Result<()> {
        vectors::upsert_text(&self.text, points)?;
        files::insert_text_chunks(&self.db, points)
    }

    fn upsert_images(&mut self, points: &[ImagePoint]) -> Result<()> {
        vectors::upsert_images(&self.image, points)?;
        files::insert_image_chunks(&self.db, points)
    }

    fn flush(&mut self) -> Result<()> {
        vectors::flush(&self.text)?;
        vectors::flush(&self.image)
    }

    fn delete_file(&mut self, file_id: i64) -> Result<()> {
        let (text_ids, image_ids) = files::chunk_ids_of_file(&self.db, file_id)?;
        vectors::delete_points(&self.text, &text_ids)?;
        vectors::delete_points(&self.image, &image_ids)?;
        files::delete_chunks_of_file(&self.db, file_id)
    }

    fn search_text(&self, query: &[f32], limit: usize) -> Result<Vec<Hit>> {
        vectors::search_dense(&self.text, vectors::TEXT, query, limit, self.oversampling)
    }

    fn search_images(&self, query: &[f32], limit: usize) -> Result<Vec<Hit>> {
        vectors::search_dense(&self.image, vectors::IMAGE, query, limit, self.oversampling)
    }

    fn search_keywords(&self, query: &SparseVec, limit: usize) -> Result<Vec<Hit>> {
        vectors::search_sparse(&self.text, query, limit)
    }
}
