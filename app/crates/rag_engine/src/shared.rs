//! 讓儲存層可以被多個執行緒共用：每一次讀寫各自上鎖，不會在取向量（很慢）的期間佔著鎖。

use anyhow::Result;
use rag_core::{Hit, ImagePoint, SparseVec, Store, TextPoint};
use rag_store::LocalStore;
use std::sync::{Mutex, MutexGuard};

/// 上鎖。別的執行緒在持有鎖時當掉，鎖會被標記為中毒；資料本身都已寫進磁碟，照常繼續用。
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct SharedStore<'a>(pub &'a Mutex<LocalStore>);

impl Store for SharedStore<'_> {
    fn upsert_text(&mut self, points: &[TextPoint]) -> Result<()> {
        lock(self.0).upsert_text(points)
    }

    fn upsert_images(&mut self, points: &[ImagePoint]) -> Result<()> {
        lock(self.0).upsert_images(points)
    }

    fn flush(&mut self) -> Result<()> {
        lock(self.0).flush()
    }

    fn delete_file(&mut self, file_id: i64) -> Result<()> {
        lock(self.0).delete_file(file_id)
    }

    fn search_text(&self, query: &[f32], limit: usize) -> Result<Vec<Hit>> {
        lock(self.0).search_text(query, limit)
    }

    fn search_images(&self, query: &[f32], limit: usize) -> Result<Vec<Hit>> {
        lock(self.0).search_images(query, limit)
    }

    fn search_keywords(&self, query: &SparseVec, limit: usize) -> Result<Vec<Hit>> {
        lock(self.0).search_keywords(query, limit)
    }
}
