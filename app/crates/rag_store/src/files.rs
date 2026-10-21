//! SQLite：檔案清單、每個 chunk 的原文和位置、設定。

use crate::LocalStore;
use anyhow::{Context, Result};
use rag_core::{ImagePoint, TextPoint};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY,
    path          TEXT NOT NULL UNIQUE,
    size          INTEGER NOT NULL,
    mtime         INTEGER NOT NULL,
    content_hash  TEXT,
    status        TEXT NOT NULL,
    error         TEXT,
    pages         INTEGER,
    duplicate_of  INTEGER,
    versions      TEXT,
    indexed_at    INTEGER
);
CREATE INDEX IF NOT EXISTS files_hash ON files(content_hash);
-- 每個搜尋結果都要查「內容相同的其他檔案」，沒有索引時每查一次就掃過整張檔案表
CREATE INDEX IF NOT EXISTS files_duplicate ON files(duplicate_of) WHERE duplicate_of IS NOT NULL;
CREATE TABLE IF NOT EXISTS chunks (
    id           INTEGER PRIMARY KEY,
    file_id      INTEGER NOT NULL,
    kind         INTEGER NOT NULL,
    page         INTEGER NOT NULL,
    chunk_index  INTEGER NOT NULL,
    heading      TEXT,
    text         TEXT,
    image_path   TEXT,
    -- 段落跨頁時結尾所在的頁碼；舊索引沒有這一欄（開啟時補上），值是 NULL，當作和 page 相同
    page_end     INTEGER
);
CREATE INDEX IF NOT EXISTS chunks_file ON chunks(file_id);
CREATE TABLE IF NOT EXISTS settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// 已登記，還沒建完索引（包含做到一半中斷的）。
    Pending,
    Indexed,
    Failed,
    /// 內容和另一個已建索引的檔案完全相同，不重複建。
    Duplicate,
}

impl FileStatus {
    fn parse(s: &str) -> Self {
        match s {
            "indexed" => Self::Indexed,
            "failed" => Self::Failed,
            "duplicate" => Self::Duplicate,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileRow {
    pub id: i64,
    pub path: String,
    pub size: u64,
    /// 修改時間（Unix 秒）。
    pub mtime: i64,
    pub content_hash: Option<String>,
    pub status: FileStatus,
    pub error: Option<String>,
    pub pages: Option<u32>,
    pub duplicate_of: Option<i64>,
    /// 建索引時的設定版本（見 `EngineConfig::versions`）。
    pub versions: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    Text,
    Image,
}

#[derive(Debug, Clone)]
pub struct ChunkRow {
    pub id: u64,
    pub file_id: i64,
    pub file_path: String,
    pub kind: ChunkKind,
    pub page: u32,
    /// 段落跨頁時結尾所在的頁碼；不跨頁、圖片、舊索引都和 `page` 相同。
    pub page_end: u32,
    pub chunk_index: u32,
    pub heading: Option<String>,
    /// 文字 chunk 的原文；圖片是 `None`。
    pub text: Option<String>,
    /// 頁面圖片或圖片檔的路徑；文字 chunk 是 `None`。
    pub image_path: Option<String>,
}

pub(crate) fn open_db(path: &Path) -> Result<Connection> {
    let db = Connection::open(path).with_context(|| format!("無法開啟 {}", path.display()))?;
    db.pragma_update(None, "journal_mode", "WAL")?;
    db.pragma_update(None, "synchronous", "NORMAL")?;
    db.execute_batch(SCHEMA)?;
    // 2026-10-16 以前建的索引沒有 page_end 欄
    let has_page_end: bool = db.query_row("SELECT COUNT(*) FROM pragma_table_info('chunks') WHERE name = 'page_end'", [], |r| r.get::<_, i64>(0))? > 0;
    if !has_page_end {
        db.execute_batch("ALTER TABLE chunks ADD COLUMN page_end INTEGER")?;
    }
    Ok(db)
}

pub(crate) fn insert_text_chunks(db: &Connection, points: &[TextPoint]) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO chunks (id, file_id, kind, page, chunk_index, heading, text, image_path, page_end) VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, NULL, ?7)",
        )?;
        for p in points {
            stmt.execute(params![p.id as i64, p.file_id, p.page, p.chunk_index, p.heading, p.text, p.page_end])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn insert_image_chunks(db: &Connection, points: &[ImagePoint]) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO chunks (id, file_id, kind, page, chunk_index, heading, text, image_path) VALUES (?1, ?2, 1, ?3, 0, NULL, NULL, ?4)",
        )?;
        for p in points {
            stmt.execute(params![p.id as i64, p.file_id, p.page, p.image_path])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// 一個檔案的 chunk 編號，分成文字的和圖片的（兩種放在不同的向量庫）。
pub(crate) fn chunk_ids_of_file(db: &Connection, file_id: i64) -> Result<(Vec<u64>, Vec<u64>)> {
    let mut stmt = db.prepare_cached("SELECT id, kind FROM chunks WHERE file_id = ?1")?;
    let rows = stmt.query_map([file_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let ids = |kind: i64| rows.iter().filter(|(_, k)| *k == kind).map(|(id, _)| *id as u64).collect();
    Ok((ids(0), ids(1)))
}

pub(crate) fn delete_chunks_of_file(db: &Connection, file_id: i64) -> Result<()> {
    db.execute("DELETE FROM chunks WHERE file_id = ?1", [file_id])?;
    Ok(())
}

const FILE_COLUMNS: &str = "id, path, size, mtime, content_hash, status, error, pages, duplicate_of, versions";

fn file_row(r: &Row) -> rusqlite::Result<FileRow> {
    Ok(FileRow {
        id: r.get(0)?,
        path: r.get(1)?,
        size: r.get::<_, i64>(2)? as u64,
        mtime: r.get(3)?,
        content_hash: r.get(4)?,
        status: FileStatus::parse(&r.get::<_, String>(5)?),
        error: r.get(6)?,
        pages: r.get(7)?,
        duplicate_of: r.get(8)?,
        versions: r.get(9)?,
    })
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl LocalStore {
    pub fn file_by_path(&self, path: &str) -> Result<Option<FileRow>> {
        let sql = format!("SELECT {FILE_COLUMNS} FROM files WHERE path = ?1");
        Ok(self.db.query_row(&sql, [path], file_row).optional()?)
    }

    pub fn file_by_id(&self, id: i64) -> Result<Option<FileRow>> {
        let sql = format!("SELECT {FILE_COLUMNS} FROM files WHERE id = ?1");
        Ok(self.db.query_row(&sql, [id], file_row).optional()?)
    }

    pub fn list_files(&self) -> Result<Vec<FileRow>> {
        let sql = format!("SELECT {FILE_COLUMNS} FROM files ORDER BY path");
        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt.query_map([], file_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 登記一個檔案（或更新它的大小和修改時間），狀態設為待處理。回傳檔案編號。
    pub fn register_file(&self, path: &str, size: u64, mtime: i64) -> Result<i64> {
        self.db.execute(
            "INSERT INTO files (path, size, mtime, status) VALUES (?1, ?2, ?3, 'pending')
             ON CONFLICT(path) DO UPDATE SET size = ?2, mtime = ?3, status = 'pending', error = NULL, duplicate_of = NULL",
            params![path, size as i64, mtime],
        )?;
        Ok(self.db.query_row("SELECT id FROM files WHERE path = ?1", [path], |r| r.get(0))?)
    }

    /// 找一個內容雜湊值相同、而且已經建好索引的檔案。
    pub fn indexed_file_with_hash(&self, content_hash: &str, except: i64) -> Result<Option<i64>> {
        Ok(self
            .db
            .query_row(
                "SELECT id FROM files WHERE content_hash = ?1 AND status = 'indexed' AND id != ?2 LIMIT 1",
                params![content_hash, except],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// `versions` 記錄建這份索引時用的解析器和模型版本，日後版本不同時用來判斷要不要重建。
    pub fn mark_indexed(&self, file_id: i64, content_hash: &str, pages: u32, versions: &str) -> Result<()> {
        self.db.execute(
            "UPDATE files SET status = 'indexed', content_hash = ?2, pages = ?3, versions = ?4, error = NULL, duplicate_of = NULL, indexed_at = ?5 WHERE id = ?1",
            params![file_id, content_hash, pages, versions, now()],
        )?;
        Ok(())
    }

    pub fn mark_duplicate(&self, file_id: i64, content_hash: &str, duplicate_of: i64) -> Result<()> {
        self.db.execute(
            "UPDATE files SET status = 'duplicate', content_hash = ?2, duplicate_of = ?3, error = NULL, indexed_at = ?4 WHERE id = ?1",
            params![file_id, content_hash, duplicate_of, now()],
        )?;
        Ok(())
    }

    pub fn mark_failed(&self, file_id: i64, error: &str) -> Result<()> {
        self.db.execute("UPDATE files SET status = 'failed', error = ?2 WHERE id = ?1", params![file_id, error])?;
        Ok(())
    }

    /// 把檔案從清單和索引裡都拿掉（檔案已被刪除或搬走時）。
    ///
    /// 如果有別的檔案因為內容相同而沒有建索引，把它們改回待處理，下次掃描時其中一個會補建。
    pub fn remove_file(&mut self, file_id: i64) -> Result<()> {
        rag_core::Store::delete_file(self, file_id)?;
        self.db.execute("UPDATE files SET status = 'pending', duplicate_of = NULL WHERE duplicate_of = ?1", [file_id])?;
        self.db.execute("DELETE FROM files WHERE id = ?1", [file_id])?;
        Ok(())
    }

    /// 檔案被搬移或改名：只改清單裡的路徑，向量和原文都沿用（chunk 編號由內容算出，和路徑無關）。
    pub fn move_file(&self, file_id: i64, new_path: &str, size: u64, mtime: i64) -> Result<()> {
        self.db.execute("UPDATE files SET path = ?2, size = ?3, mtime = ?4 WHERE id = ?1", params![file_id, new_path, size as i64, mtime])?;
        Ok(())
    }

    /// 清單裡路徑以 `prefix` 開頭的檔案。
    pub fn files_under(&self, prefix: &str) -> Result<Vec<FileRow>> {
        Ok(self.list_files()?.into_iter().filter(|f| f.path.starts_with(prefix)).collect())
    }

    /// 內容和 `file_id` 相同的其他檔案的路徑。
    pub fn duplicates_of(&self, file_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.db.prepare_cached("SELECT path FROM files WHERE duplicate_of = ?1 ORDER BY path")?;
        let rows = stmt.query_map([file_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(rows)
    }

    /// 用檢索命中的編號取回 chunk 的原文和位置。
    pub fn chunk(&self, id: u64) -> Result<Option<ChunkRow>> {
        Ok(self
            .db
            .query_row(
                "SELECT c.id, c.file_id, f.path, c.kind, c.page, c.chunk_index, c.heading, c.text, c.image_path, COALESCE(c.page_end, c.page)
                 FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.id = ?1",
                [id as i64],
                |r| {
                    Ok(ChunkRow {
                        id: r.get::<_, i64>(0)? as u64,
                        file_id: r.get(1)?,
                        file_path: r.get(2)?,
                        kind: if r.get::<_, i64>(3)? == 1 { ChunkKind::Image } else { ChunkKind::Text },
                        page: r.get(4)?,
                        chunk_index: r.get(5)?,
                        heading: r.get(6)?,
                        text: r.get(7)?,
                        image_path: r.get(8)?,
                        page_end: r.get(9)?,
                    })
                },
            )
            .optional()?)
    }

    /// 一個檔案的全部 chunk，依頁碼和順序排好。換模型時用它重算向量，不必重新解析檔案。
    pub fn chunks_of_file(&self, file_id: i64) -> Result<Vec<ChunkRow>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT c.id, c.file_id, f.path, c.kind, c.page, c.chunk_index, c.heading, c.text, c.image_path, COALESCE(c.page_end, c.page)
             FROM chunks c JOIN files f ON f.id = c.file_id WHERE c.file_id = ?1 ORDER BY c.kind, c.page, c.chunk_index, c.id",
        )?;
        let rows = stmt.query_map([file_id], |r| {
            Ok(ChunkRow {
                id: r.get::<_, i64>(0)? as u64,
                file_id: r.get(1)?,
                file_path: r.get(2)?,
                kind: if r.get::<_, i64>(3)? == 1 { ChunkKind::Image } else { ChunkKind::Text },
                page: r.get(4)?,
                chunk_index: r.get(5)?,
                heading: r.get(6)?,
                text: r.get(7)?,
                image_path: r.get(8)?,
                page_end: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// 重算向量之後，記下新的設定版本。
    pub fn set_file_versions(&self, file_id: i64, versions: &str) -> Result<()> {
        self.db.execute("UPDATE files SET versions = ?2, indexed_at = ?3 WHERE id = ?1", params![file_id, versions, now()])?;
        Ok(())
    }

    /// 已建索引、但文字 chunk 還沒有記結尾頁（`page_end` 是 NULL）的檔案：2026-10-16 以前建的。
    pub fn files_without_page_end(&self) -> Result<Vec<FileRow>> {
        let sql = format!(
            "SELECT {FILE_COLUMNS} FROM files WHERE status = 'indexed'
               AND EXISTS (SELECT 1 FROM chunks c WHERE c.file_id = files.id AND c.kind = 0 AND c.page_end IS NULL)
             ORDER BY path"
        );
        let mut stmt = self.db.prepare(&sql)?;
        let rows = stmt.query_map([], file_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 寫入一個檔案的段落結尾頁（chunk 編號, 結尾頁）；這個檔案其餘還沒填的文字 chunk 當作不跨頁。
    /// 回傳實際更新了幾個。
    pub fn set_page_end(&self, file_id: i64, pages: &[(u64, u32)]) -> Result<usize> {
        let tx = self.db.unchecked_transaction()?;
        let mut updated = 0;
        {
            let mut stmt = tx.prepare_cached("UPDATE chunks SET page_end = ?3 WHERE id = ?1 AND file_id = ?2 AND kind = 0")?;
            for (id, page_end) in pages {
                updated += stmt.execute(params![*id as i64, file_id, page_end])?;
            }
        }
        tx.execute("UPDATE chunks SET page_end = page WHERE file_id = ?1 AND kind = 0 AND page_end IS NULL", [file_id])?;
        tx.commit()?;
        Ok(updated)
    }

    /// 文字 chunk 數和圖片數。
    pub fn chunk_counts(&self) -> Result<(u64, u64)> {
        let count = |kind: i64| -> Result<u64> {
            Ok(self.db.query_row("SELECT COUNT(*) FROM chunks WHERE kind = ?1", [kind], |r| r.get::<_, i64>(0))? as u64)
        };
        Ok((count(0)?, count(1)?))
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.db.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2", params![key, value])?;
        Ok(())
    }
}

/// 放大索引（實驗用）：每個已建好索引的檔案多登記 `copies` 份，路徑是「/副本N」加上原路徑（不在任何來源底下，
/// 更新時不會被當成已刪除），內容雜湊值加上「#N」。回傳（原檔案編號, 第幾份）→ 新檔案編號。
pub(crate) fn duplicate_files(db: &Connection, copies: usize) -> Result<std::collections::HashMap<(i64, usize), i64>> {
    let mut map = std::collections::HashMap::new();
    let tx = db.unchecked_transaction()?;
    {
        let originals: Vec<(i64, String, Option<String>)> = tx
            .prepare("SELECT id, path, content_hash FROM files WHERE status = 'indexed'")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut insert = tx.prepare(
            "INSERT INTO files (path, size, mtime, content_hash, status, error, pages, duplicate_of, versions, indexed_at)
             SELECT ?2, size, mtime, ?3, status, error, pages, duplicate_of, versions, indexed_at FROM files WHERE id = ?1",
        )?;
        for c in 1..=copies {
            for (id, path, hash) in &originals {
                let path = format!("/副本{c}{}{path}", if path.starts_with('/') { "" } else { "/" });
                insert.execute(params![id, path, hash.as_ref().map(|h| format!("{h}#{c}"))])?;
                map.insert((*id, c), tx.last_insert_rowid());
            }
        }
    }
    tx.commit()?;
    Ok(map)
}

/// 放大索引：複製這些檔案的 chunk，編號用 `new_id`（原編號, 第幾份）算。回傳新增的 chunk 數。
pub(crate) fn duplicate_chunks(
    db: &Connection,
    files: &std::collections::HashMap<(i64, usize), i64>,
    new_id: &dyn Fn(u64, usize) -> u64,
) -> Result<usize> {
    let tx = db.unchecked_transaction()?;
    let mut count = 0;
    {
        let mut by_file: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
        for row in tx.prepare("SELECT id, file_id FROM chunks")?.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (id, file) = row?;
            by_file.entry(file).or_default().push(id);
        }
        let mut insert = tx.prepare(
            "INSERT OR REPLACE INTO chunks (id, file_id, kind, page, chunk_index, heading, text, image_path, page_end)
             SELECT ?2, ?3, kind, page, chunk_index, heading, text, image_path, page_end FROM chunks WHERE id = ?1",
        )?;
        for ((old_file, c), new_file) in files {
            for id in by_file.get(old_file).into_iter().flatten() {
                insert.execute(params![id, new_id(*id as u64, *c) as i64, new_file])?;
                count += 1;
            }
        }
    }
    tx.commit()?;
    Ok(count)
}
