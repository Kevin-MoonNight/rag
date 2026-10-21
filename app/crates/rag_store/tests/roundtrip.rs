//! 儲存層的整合測試：寫入、三種搜尋、刪除、關掉再開。

use rag_core::text::{document_sparse, query_sparse};
use rag_core::{ImagePoint, Store, TextPoint};
use rag_store::{ChunkKind, FileStatus, LocalStore, VECTOR_SIZE};

/// 只有一個維度是 1 的單位向量，再混一點別的維度，讓相似度有高有低。
fn vector(main: usize, other: usize, mix: f32) -> Vec<f32> {
    let mut v = vec![0.0; VECTOR_SIZE];
    v[main] = 1.0;
    v[other] += mix;
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= norm);
    v
}

fn text_point(id: u64, file_id: i64, text: &str, dense: Vec<f32>) -> TextPoint {
    TextPoint { id, file_id, page: 1, page_end: 1, chunk_index: id as u32, heading: None, text: text.to_string(), dense, sparse: document_sparse(text) }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rag_store_test_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn write_search_delete_reopen() {
    let dir = temp_dir("roundtrip");
    {
        let mut store = LocalStore::open(&dir).unwrap();
        let a = store.register_file("/docs/a.pdf", 100, 1).unwrap();
        let b = store.register_file("/docs/b.pdf", 200, 2).unwrap();
        assert_ne!(a, b);

        store
            .upsert_text(&[
                text_point(1, a, "房屋貸款超過五百萬元須經兩位授信人員審查", vector(0, 1, 0.1)),
                text_point(2, a, "汽車貸款利率依央行公告機動調整", vector(1, 0, 0.3)),
                text_point(3, b, "交易紀錄保存七年 retention policy A-123", vector(2, 0, 0.2)),
            ])
            .unwrap();
        store.upsert_images(&[ImagePoint { id: 10, file_id: b, page: 4, image_path: "/pages/b_p0004.png".into(), dense: vector(0, 2, 0.5) }]).unwrap();
        store.flush().unwrap();
        store.mark_indexed(a, "hash-a", 2, "v1").unwrap();
        store.mark_indexed(b, "hash-b", 4, "v1").unwrap();

        // 文字向量：只在文字 chunk 裡找，最像的排第一
        let hits = store.search_text(&vector(0, 1, 0.0), 10).unwrap();
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(hits[0].score > 0.9 && hits[0].score > hits[1].score);

        // 圖片向量：只在圖片裡找
        let hits = store.search_images(&vector(0, 1, 0.0), 10).unwrap();
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![10]);

        // 關鍵字：編號要能完全比對到；沒出現的詞找不到東西
        let hits = store.search_keywords(&query_sparse("A-123"), 10).unwrap();
        assert_eq!(hits.first().map(|h| h.id), Some(3));
        let hits = store.search_keywords(&query_sparse("授信人員"), 10).unwrap();
        assert_eq!(hits.first().map(|h| h.id), Some(1));
        assert!(store.search_keywords(&query_sparse("zzzz"), 10).unwrap().is_empty());

        let chunk = store.chunk(3).unwrap().unwrap();
        assert_eq!((chunk.file_path.as_str(), chunk.kind, chunk.page), ("/docs/b.pdf", ChunkKind::Text, 1));
        assert!(chunk.text.unwrap().contains("retention"));
        let image = store.chunk(10).unwrap().unwrap();
        assert_eq!((image.kind, image.page, image.image_path.as_deref()), (ChunkKind::Image, 4, Some("/pages/b_p0004.png")));
        assert_eq!(store.chunk_counts().unwrap(), (3, 1));
        assert!(store.optimize().is_ok());
    }

    // 關掉再開：資料都在，刪掉一個檔案後它的資料查不到
    let mut store = LocalStore::open(&dir).unwrap();
    assert_eq!(store.search_text(&vector(0, 1, 0.0), 10).unwrap().len(), 3);
    let b = store.file_by_path("/docs/b.pdf").unwrap().unwrap();
    assert_eq!(b.status, FileStatus::Indexed);
    store.remove_file(b.id).unwrap();
    store.flush().unwrap();
    assert_eq!(store.search_text(&vector(0, 1, 0.0), 10).unwrap().iter().map(|h| h.id).collect::<Vec<_>>(), vec![1, 2]);
    assert!(store.search_images(&vector(0, 1, 0.0), 10).unwrap().is_empty());
    assert!(store.search_keywords(&query_sparse("A-123"), 10).unwrap().is_empty());
    assert!(store.chunk(3).unwrap().is_none());
    assert_eq!(store.chunk_counts().unwrap(), (2, 0));
    assert!(store.file_by_path("/docs/b.pdf").unwrap().is_none());

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicates_are_promoted_when_the_original_is_removed() {
    let dir = temp_dir("dups");
    let mut store = LocalStore::open(&dir).unwrap();
    let a = store.register_file("/docs/a.pdf", 100, 1).unwrap();
    let copy = store.register_file("/docs/copy.pdf", 100, 5).unwrap();
    store.mark_indexed(a, "same", 1, "v1").unwrap();
    assert_eq!(store.indexed_file_with_hash("same", copy).unwrap(), Some(a));
    store.mark_duplicate(copy, "same", a).unwrap();
    assert_eq!(store.duplicates_of(a).unwrap(), vec!["/docs/copy.pdf".to_string()]);

    store.remove_file(a).unwrap();
    let row = store.file_by_path("/docs/copy.pdf").unwrap().unwrap();
    assert_eq!((row.status, row.duplicate_of), (FileStatus::Pending, None));
    assert_eq!(store.setting("versions").unwrap(), None);
    store.set_setting("versions", "x").unwrap();
    store.set_setting("versions", "y").unwrap();
    assert_eq!(store.setting("versions").unwrap().as_deref(), Some("y"));

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replicate_makes_independent_copies() {
    let dir = temp_dir("replicate");
    let mut store = LocalStore::open(&dir).unwrap();
    let a = store.register_file("/docs/a.pdf", 100, 1).unwrap();
    store.upsert_text(&[text_point(1, a, "交易紀錄保存七年 A-123", vector(0, 1, 0.1)), text_point(2, a, "汽車貸款利率", vector(1, 0, 0.3))]).unwrap();
    store.upsert_images(&[ImagePoint { id: 10, file_id: a, page: 1, image_path: "/pages/a.png".into(), dense: vector(2, 0, 0.5) }]).unwrap();
    store.flush().unwrap();
    store.mark_indexed(a, "hash-a", 1, "v1").unwrap();

    let (text, image) = store.replicate(3, 0.1).unwrap();
    assert_eq!((text, image), (6, 3));
    assert_eq!(store.chunk_counts().unwrap(), (8, 4));
    let files = store.list_files().unwrap();
    assert_eq!(files.len(), 4);
    assert!(files.iter().any(|f| f.path == "/副本2/docs/a.pdf" && f.content_hash.as_deref() == Some("hash-a#2")));

    // 關鍵字照抄：四份都找得到；稠密向量有擾動，原本的仍然最像
    let hits = store.search_keywords(&query_sparse("A-123"), 10).unwrap();
    assert_eq!(hits.len(), 4);
    let hits = store.search_text(&vector(0, 1, 0.1), 10).unwrap();
    assert_eq!(hits[0].id, 1);
    assert!(hits[1].score > 0.98 && hits[1].score < hits[0].score);
    // 複本的 chunk 指向自己的檔案
    let copy = store.chunk(hits[1].id).unwrap().unwrap();
    assert!(copy.file_path.starts_with("/副本"));
    assert_eq!(copy.text.as_deref(), Some("交易紀錄保存七年 A-123"));
    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_index_is_one_bit_and_old_int8_index_stays_int8() {
    use rag_store::Quantization;
    let new = temp_dir("quant_new");
    let store = LocalStore::open(&new).unwrap();
    assert_eq!(store.quantization(), Quantization::OneBitQuery8);
    drop(store);
    // 重新開啟時讀回建立時的量化方式
    assert_eq!(LocalStore::open_with(&new, Quantization::Int8).unwrap().quantization(), Quantization::OneBitQuery8);

    let old = temp_dir("quant_old");
    let mut store = LocalStore::open_with(&old, Quantization::Int8).unwrap();
    let a = store.register_file("/docs/a.pdf", 100, 1).unwrap();
    store.upsert_text(&[text_point(1, a, "交易紀錄保存七年", vector(0, 1, 0.1))]).unwrap();
    store.flush().unwrap();
    drop(store);
    let store = LocalStore::open(&old).unwrap();
    assert_eq!(store.quantization(), Quantization::Int8);
    assert_eq!(store.search_text(&vector(0, 1, 0.1), 5).unwrap()[0].id, 1);
    drop(store);
    let _ = std::fs::remove_dir_all(&new);
    let _ = std::fs::remove_dir_all(&old);
}

fn dir_size(path: &std::path::Path) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

/// 實驗 89：flush 後清掉寫入紀錄（WAL）。清掉以後寫入的編號要接續下去，否則重開後的更新和刪除會被當成舊操作略過。
#[test]
fn wal_is_trimmed_after_flush_and_later_updates_survive_reopen() {
    let dir = temp_dir("wal");
    let wal = dir.join("vectors-text").join("wal");
    {
        let mut store = LocalStore::open(&dir).unwrap();
        let f = store.register_file("/docs/big.pdf", 1, 1).unwrap();
        // 每個 chunk 在紀錄檔裡約 6 KB，1,200 個約 7 MB（每個紀錄檔 1 MB）
        for batch in 0..24u64 {
            let points: Vec<TextPoint> = (0..50u64)
                .map(|i| {
                    let id = batch * 50 + i;
                    text_point(id, f, "全民健康保險醫療服務給付項目及支付標準", vector(10 + (id as usize % 700), 9, 0.1))
                })
                .collect();
            store.upsert_text(&points).unwrap();
        }
        store.flush().unwrap();
        let size = dir_size(&wal);
        assert!(size < 4 * 1024 * 1024, "flush 後寫入紀錄還有 {size} bytes");
    }
    {
        // 重開後：更新一個舊的 chunk、加一個新檔案
        let mut store = LocalStore::open(&dir).unwrap();
        let f = store.file_by_path("/docs/big.pdf").unwrap().unwrap().id;
        let g = store.register_file("/docs/small.pdf", 2, 2).unwrap();
        store.upsert_text(&[text_point(0, f, "更新後的內容", vector(7, 8, 0.1))]).unwrap();
        store.upsert_text(&[text_point(100_000, g, "長期照顧服務人員訓練", vector(5, 6, 0.1))]).unwrap();
        store.flush().unwrap();
    }
    {
        // 再重開：更新有效；刪掉新檔案
        let mut store = LocalStore::open(&dir).unwrap();
        assert_eq!(store.search_text(&vector(7, 8, 0.0), 1).unwrap()[0].id, 0);
        assert_eq!(store.search_text(&vector(5, 6, 0.0), 1).unwrap()[0].id, 100_000);
        let g = store.file_by_path("/docs/small.pdf").unwrap().unwrap().id;
        store.remove_file(g).unwrap();
        store.flush().unwrap();
    }
    // 最後再重開：刪掉的不會回來，總數正確
    let store = LocalStore::open(&dir).unwrap();
    assert_ne!(store.search_text(&vector(5, 6, 0.0), 1).unwrap()[0].id, 100_000);
    assert_eq!(store.chunk_counts().unwrap(), (1200, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn page_end_roundtrip_and_old_index_without_the_column() {
    // 2026-10-16 以前建的索引：chunks 表沒有 page_end 欄
    let dir = temp_dir("old_schema");
    std::fs::create_dir_all(&dir).unwrap();
    {
        let db = rusqlite::Connection::open(dir.join("index.sqlite")).unwrap();
        db.execute_batch(
            "CREATE TABLE files (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, size INTEGER NOT NULL, mtime INTEGER NOT NULL,
                content_hash TEXT, status TEXT NOT NULL, error TEXT, pages INTEGER, duplicate_of INTEGER, versions TEXT, indexed_at INTEGER);
             CREATE TABLE chunks (id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL, kind INTEGER NOT NULL, page INTEGER NOT NULL,
                chunk_index INTEGER NOT NULL, heading TEXT, text TEXT, image_path TEXT);
             INSERT INTO files (id, path, size, mtime, status) VALUES (1, '/docs/old.pdf', 1, 1, 'indexed');
             INSERT INTO chunks (id, file_id, kind, page, chunk_index, text) VALUES (5, 1, 0, 7, 0, '舊索引的段落');",
        )
        .unwrap();
    }
    let mut store = LocalStore::open(&dir).unwrap();
    let old = store.chunk(5).unwrap().unwrap();
    assert_eq!((old.page, old.page_end), (7, 7));

    // 新寫入的跨頁段落記得結尾頁
    let file = store.register_file("/docs/new.pdf", 1, 1).unwrap();
    let mut p = text_point(6, file, "跨頁的段落", vector(0, 1, 0.1));
    p.page = 66;
    p.page_end = 67;
    store.upsert_text(&[p]).unwrap();
    store.flush().unwrap();
    let new = store.chunk(6).unwrap().unwrap();
    assert_eq!((new.page, new.page_end), (66, 67));
    assert_eq!(store.chunks_of_file(file).unwrap()[0].page_end, 67);
}
