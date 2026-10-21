//! 建索引：使用者勾選的資料夾和個別檔案（來源）逐一掃描，跳過沒變的、轉換、切分、取向量、寫入，
//! 中斷後可以接著做。
//!
//! 每次更新時的處理：
//! - 檔案被搬移或改名（內容相同）：只改路徑，不重算。
//! - 檔案已不存在、或來源被取消：從索引拿掉。
//! - 來源所在的磁碟不在（外接磁碟拔掉）：標為離線，不拿掉；離線超過 30 天才清除。
//! - 新增和修改的檔案排在前面；同一組裡 PDF 在圖片檔前面。
//! - 隱藏檔、系統檔、Office 的暫存檔（`~$` 開頭）、索引資料夾本身不處理；超過大小上限的檔案標為失敗。
//! - 失敗的檔案沒有變動就不重試，只有「被其他程式使用中」的會在下次更新重試。

use crate::shared::lock;
use crate::{Engine, SharedStore};
use anyhow::{bail, Context, Result};
use rag_core::ids::content_hash;
use rag_core::{index_document, reembed_document, BlockKind, ImagePoint, Store, TextPoint};
use rag_store::{ChunkKind, FileRow, FileStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 來源所在的磁碟離線多久之後，才把它的檔案從索引清掉。
const OFFLINE_DAYS: i64 = 30;
/// 檔案被其他程式鎖住時的錯誤訊息開頭。這種失敗下次更新會重試。
const LOCKED: &str = "檔案被其他程式使用中";
/// 檔案太大時的錯誤訊息開頭。使用者調高上限後會重試。
const TOO_LARGE: &str = "檔案超過大小上限";
/// 摘要裡列出檔名的上限。
pub const LIST_LIMIT: usize = 200;
/// 建索引途中，每新增這麼多個文字 chunk 就整理一次索引。還沒整理的新資料（尤其是關鍵字）留在記憶體裡，
/// 一次建很多檔案時會一直長大（約每個 chunk 11 KB，10 萬個就超過 1 GB）。
const OPTIMIZE_EVERY: u64 = 5000;

/// 一類檔案（PDF 或圖片檔）建索引的累計數字，用來估計還要多久。存在設定裡，每次更新後累加。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct History {
    files: u64,
    text_chunks: u64,
    images: u64,
    seconds: f64,
}

impl History {
    /// 每個檔案平均的文字 chunk 數和秒數。還沒有紀錄時用計畫書的估計：
    /// PDF 約 30 個 chunk、每個 chunk 0.5 秒；圖片檔一張約 8 秒（文書機）。
    fn per_file(&self, is_image: bool) -> (f64, f64) {
        if self.files >= 5 {
            (self.text_chunks as f64 / self.files as f64, self.seconds / self.files as f64)
        } else if is_image {
            (0.0, 8.0)
        } else {
            (30.0, 15.0)
        }
    }
}

fn is_image_file(path: &Path) -> bool {
    !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// 兩組設定版本的解析和切分部分是否相同（相同的話，換模型只要重算向量）。
/// 版本字串的格式見 `EngineConfig::versions`：解析器;模型;chunk=…;headings=…;kw=…
fn same_parsing(a: &str, b: &str) -> bool {
    let parts = |s: &str| -> Vec<String> { s.split(';').map(str::to_string).collect() };
    let (a, b) = (parts(a), parts(b));
    a.len() == 5 && b.len() == 5 && a[0] == b[0] && a[2] == b[2] && a[3] == b[3]
}

/// 同一組設定在開文字辨識以前的版本字串（實驗 99）；目前的設定沒有開辨識時是 `None`。
/// （沒有影像模型時圖片區塊不會存進索引，這種索引的掃描頁也就認不出來，照這個規則不會補辨識；目前一律附影像模型。）
fn before_ocr(current: &str) -> Option<String> {
    let tag = format!("+{}", rag_convert::OCR_VERSION);
    current.contains(&tag).then(|| current.replacen(&tag, "", 1))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum FileOutcome {
    Indexed { pages: u32, text_chunks: usize, images: usize },
    /// 內容和另一個已建索引的檔案相同。
    Duplicate,
    /// 上次建完之後沒有變動。
    Unchanged,
    /// 上次失敗，檔案也沒有變動，這次不重試。
    StillFailed,
    Failed(String),
}

/// 每處理完一個檔案回報一次。
#[derive(Debug, Clone)]
pub struct Progress<'a> {
    pub done: usize,
    pub total: usize,
    pub path: &'a str,
    pub outcome: &'a FileOutcome,
    pub seconds: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexSummary {
    pub files: usize,
    pub indexed: usize,
    pub duplicates: usize,
    pub unchanged: usize,
    pub failed: usize,
    /// 上次失敗、這次沒有重試的檔案。
    pub still_failed: usize,
    /// 檔案已不存在（或不再屬於任何來源）、從索引拿掉的數量。
    pub removed: usize,
    /// 搬移或改名、只更新了路徑的檔案。
    pub moved: usize,
    /// 離線的來源（磁碟不在）。
    pub offline_sources: usize,
    /// 掃描時略過的隱藏檔和系統檔、不支援的格式。
    pub skipped_hidden: usize,
    pub unsupported: usize,
    /// 不支援的檔案（最多列 [`LIST_LIMIT`] 個），給介面列出。
    pub unsupported_files: Vec<String>,
    /// 換了向量模型或關鍵字切法、用存下的原文重算向量的檔案。
    pub reembedded: usize,
    /// 開始處理前的估計：要處理（新增、修改、要重建）的檔案數、約略的文字 chunk 數和秒數。
    pub estimate_files: usize,
    pub estimate_chunks: u64,
    pub estimate_seconds: f64,
    /// 這次完整建好的檔案的紀錄，結束時累加到設定裡。
    #[serde(skip)]
    history: [History; 2],
    /// 上次整理索引之後新增的文字 chunk 數。
    #[serde(skip)]
    unoptimized: u64,
    pub pages: u64,
    pub text_chunks: u64,
    pub images: u64,
    pub convert_seconds: f64,
    pub embed_seconds: f64,
    pub optimize_seconds: f64,
    pub total_seconds: f64,
}

/// 使用者加入的來源：一個資料夾（含子資料夾）或一個檔案。只記錄位置，不複製檔案。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceEntry {
    pub path: String,
    pub is_folder: bool,
    /// 上次發現它不在的時間（Unix 秒）；在的時候是 `None`。
    #[serde(default)]
    pub offline_since: Option<i64>,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn modified_seconds(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// `path` 是不是在這個來源底下（以路徑的每一段比，`C:\a\b` 不算在 `C:\a\bc` 底下）。
fn is_under(path: &str, source: &SourceEntry) -> bool {
    if source.is_folder {
        Path::new(path).starts_with(&source.path)
    } else {
        path == source.path
    }
}

/// 隱藏檔、系統檔、Office 的暫存檔。
fn is_hidden(path: &Path, meta: &std::fs::Metadata) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    if name.starts_with("~$") {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        let _ = &name;
        meta.file_attributes() & (HIDDEN | SYSTEM) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        name.starts_with('.')
    }
}

/// 掃描時的計數。
#[derive(Debug, Default)]
pub struct ScanCounts {
    pub hidden: usize,
    pub unsupported: usize,
    pub unsupported_files: Vec<String>,
}

/// 資料夾底下第一版會處理的檔案，依路徑排序。`exclude` 底下的不處理（索引資料夾本身）。
pub fn scan(folder: &Path, exclude: &[PathBuf], counts: &mut ScanCounts) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            // 沒有權限讀的子資料夾跳過，不讓整次掃描失敗
            Err(_) if dir != folder => continue,
            Err(e) => return Err(e).with_context(|| format!("無法讀取資料夾 {}", dir.display())),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if is_hidden(&path, &meta) {
                counts.hidden += 1;
                continue;
            }
            if exclude.iter().any(|x| path.starts_with(x)) {
                continue;
            }
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                if rag_convert::is_supported(&path) {
                    out.push(path);
                } else {
                    counts.unsupported += 1;
                    if counts.unsupported_files.len() < LIST_LIMIT {
                        counts.unsupported_files.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

impl Engine {
    /// 使用者加入的來源。
    pub fn sources(&self) -> Result<Vec<SourceEntry>> {
        match self.store().setting("sources")? {
            Some(json) => serde_json::from_str(&json).context("來源清單的格式不對"),
            None => Ok(Vec::new()),
        }
    }

    fn save_sources(&self, sources: &[SourceEntry]) -> Result<()> {
        self.store().set_setting("sources", &serde_json::to_string(sources)?)
    }

    /// 加入一個資料夾或檔案。已經加過就不重複加。
    pub fn add_source(&self, path: &Path) -> Result<SourceEntry> {
        let path = std::path::absolute(path)?;
        let meta = std::fs::metadata(&path).with_context(|| format!("找不到 {}", path.display()))?;
        if meta.is_file() && !rag_convert::is_supported(&path) {
            bail!("不支援這種檔案：{}（只處理 PDF 和圖片檔）", path.display());
        }
        let entry = SourceEntry { path: path.to_string_lossy().to_string(), is_folder: meta.is_dir(), offline_since: None };
        let mut sources = self.sources()?;
        if !sources.iter().any(|s| s.path == entry.path) {
            sources.push(entry.clone());
            self.save_sources(&sources)?;
        }
        Ok(entry)
    }

    /// 取消一個來源，並把只屬於它的檔案從索引拿掉。回傳拿掉的檔案數。
    pub fn remove_source(&self, path: &str) -> Result<usize> {
        let mut sources = self.sources()?;
        let Some(i) = sources.iter().position(|s| s.path == path) else { bail!("沒有這個來源：{path}") };
        let removed = sources.remove(i);
        self.save_sources(&sources)?;
        let mut store = self.store();
        let mut count = 0;
        for row in store.list_files()? {
            if is_under(&row.path, &removed) && !sources.iter().any(|s| is_under(&row.path, s)) {
                store.remove_file(row.id)?;
                count += 1;
            }
        }
        if count > 0 {
            store.optimize()?;
            store.flush()?;
            drop(store);
            self.remove_unused_page_images()?;
        }
        Ok(count)
    }

    /// 把一個資料夾（含子資料夾）加入來源並建成索引。已建過而且沒變動的檔案會跳過，所以中斷後再跑一次就是接著做。
    pub fn index_folder(&self, folder: &Path, progress: &mut dyn FnMut(Progress)) -> Result<IndexSummary> {
        let source = self.add_source(folder)?;
        self.run_update(Some(&source.path), progress)
    }

    /// 全部的來源重新比對一次，處理新增、修改、搬移、刪除的檔案。
    pub fn update(&self, progress: &mut dyn FnMut(Progress)) -> Result<IndexSummary> {
        self.run_update(None, progress)
    }

    /// 下面三步的組合。需要在檔案之間做別的事（回報進度、檢查使用者有沒有按停止）時，直接呼叫那三步。
    fn run_update(&self, only: Option<&str>, progress: &mut dyn FnMut(Progress)) -> Result<IndexSummary> {
        let started = Instant::now();
        let mut summary = IndexSummary::default();
        let files = self.begin_update(only, &mut summary)?;
        for (n, path) in files.iter().enumerate() {
            let t = Instant::now();
            let outcome = self.index_one(path, &mut summary)?;
            let key = path.to_string_lossy();
            progress(Progress { done: n + 1, total: files.len(), path: &key, outcome: &outcome, seconds: t.elapsed().as_secs_f64() });
        }
        self.finish_index(&mut summary)?;
        summary.total_seconds = started.elapsed().as_secs_f64();
        Ok(summary)
    }

    /// 第一步：掃描來源（`only` 給的話只掃那一個），處理搬移和刪除，回傳要逐一處理的檔案（已排好順序）。
    pub fn begin_update(&self, only: Option<&str>, summary: &mut IndexSummary) -> Result<Vec<PathBuf>> {
        self.check_versions()?;
        let mut sources = self.sources()?;
        let exclude = [std::path::absolute(&self.config.data_dir)?];
        let mut counts = ScanCounts::default();
        let mut found: Vec<PathBuf> = Vec::new();
        let mut live: Vec<SourceEntry> = Vec::new();
        let mut expired: Vec<SourceEntry> = Vec::new();
        for source in sources.iter_mut().filter(|s| only.is_none_or(|o| s.path == o)) {
            let path = Path::new(&source.path);
            if !path.exists() {
                let since = *source.offline_since.get_or_insert(now());
                summary.offline_sources += 1;
                if now() - since > OFFLINE_DAYS * 86_400 {
                    expired.push(source.clone());
                }
                continue;
            }
            source.offline_since = None;
            if source.is_folder {
                found.extend(scan(path, &exclude, &mut counts)?);
            } else {
                found.push(path.to_path_buf());
            }
            live.push(source.clone());
        }
        self.save_sources(&sources)?;
        found.sort();
        found.dedup();
        summary.files = found.len();
        summary.skipped_hidden = counts.hidden;
        summary.unsupported = counts.unsupported;
        summary.unsupported_files = std::mem::take(&mut counts.unsupported_files);

        let mut store = self.store();
        let rows = store.list_files()?;
        let registered: HashMap<&str, &FileRow> = rows.iter().map(|r| (r.path.as_str(), r)).collect();
        let found_set: HashSet<String> = found.iter().map(|p| p.to_string_lossy().to_string()).collect();
        // 清單裡屬於這次掃描的來源、但這次沒有掃到的檔案：已刪除、搬走、或變成不處理的檔案
        let mut missing: Vec<&FileRow> = rows.iter().filter(|r| live.iter().any(|s| is_under(&r.path, s)) && !found_set.contains(&r.path)).collect();
        // 離線太久的來源：它的檔案也要清掉
        missing.extend(rows.iter().filter(|r| expired.iter().any(|s| is_under(&r.path, s))));

        // 搬移或改名：新出現的檔案和消失的檔案大小相同時，比對內容
        let mut by_size: HashMap<u64, Vec<&FileRow>> = HashMap::new();
        for row in missing.iter().filter(|r| r.status == FileStatus::Indexed && r.content_hash.is_some() && !Path::new(&r.path).exists()) {
            by_size.entry(row.size).or_default().push(row);
        }
        let mut moved: HashSet<i64> = HashSet::new();
        if !by_size.is_empty() {
            for path in found.iter().filter(|p| !registered.contains_key(p.to_string_lossy().as_ref())) {
                let Ok(meta) = std::fs::metadata(path) else { continue };
                let Some(candidates) = by_size.get(&meta.len()) else { continue };
                let Ok(bytes) = std::fs::read(path) else { continue };
                let hash = content_hash(&bytes);
                if let Some(row) = candidates.iter().find(|r| !moved.contains(&r.id) && r.content_hash.as_deref() == Some(hash.as_str())) {
                    store.move_file(row.id, &path.to_string_lossy(), meta.len(), modified_seconds(&meta))?;
                    moved.insert(row.id);
                    summary.moved += 1;
                }
            }
        }
        for row in missing.iter().filter(|r| !moved.contains(&r.id)) {
            store.remove_file(row.id)?;
            summary.removed += 1;
        }
        drop(store);

        // 順序：新增的、修改過的、要重建的、其餘（大多沒變，很快跳過）；同一組裡 PDF 在圖片檔前面
        let current = self.config.versions();
        let group = |path: &PathBuf| -> (u8, bool) {
            let key = path.to_string_lossy();
            let order = match registered.get(key.as_ref()) {
                None => 0,
                Some(row) => match std::fs::metadata(path) {
                    Ok(meta) if meta.len() != row.size || modified_seconds(&meta) != row.mtime => 1,
                    _ if row.status == FileStatus::Pending => 1,
                    _ if row.status == FileStatus::Indexed && row.versions.as_deref() != Some(current.as_str()) => 2,
                    _ => 3,
                },
            };
            (order, is_image_file(path))
        };
        let mut keyed: Vec<((u8, bool), PathBuf)> = found.into_iter().map(|p| (group(&p), p)).collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        // 估計：要處理的檔案數、約略的文字 chunk 數和時間（依這台電腦過去的紀錄，沒有紀錄時用預設值）
        let history = self.history();
        for ((order, is_image), _) in keyed.iter().filter(|(k, _)| k.0 < 3) {
            let (chunks, seconds) = history[*is_image as usize].per_file(*is_image);
            // 只重算向量的檔案不必解析，時間大約是全部重做的八成（解析佔的比例很小）
            let share = if *order == 2 { 0.8 } else { 1.0 };
            summary.estimate_files += 1;
            summary.estimate_chunks += chunks as u64;
            summary.estimate_seconds += seconds * share;
        }
        Ok(keyed.into_iter().map(|(_, p)| p).collect())
    }

    /// PDF 和圖片檔各自累計的建索引紀錄。
    fn history(&self) -> [History; 2] {
        self.store().setting("history").ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// 單一檔案的大小上限（位元組）。使用者在設定裡改過就用設定的值。
    pub fn max_file_bytes(&self) -> u64 {
        let saved = self.store().setting("max_file_mb").ok().flatten().and_then(|s| s.parse::<u64>().ok());
        saved.map(|mb| mb << 20).unwrap_or(self.config.max_file_bytes)
    }

    pub fn set_max_file_mb(&self, mb: u64) -> Result<()> {
        if mb == 0 {
            bail!("大小上限要大於 0");
        }
        self.store().set_setting("max_file_mb", &mb.to_string())
    }

    /// 第二步：處理一個檔案。單一檔案失敗不算錯誤，會記在清單裡並回傳 [`FileOutcome::Failed`]。
    pub fn index_one(&self, path: &Path, summary: &mut IndexSummary) -> Result<FileOutcome> {
        let key = path.to_string_lossy().to_string();
        let started = Instant::now();
        let reembedded = summary.reembedded;
        let outcome = match self.index_file(path, &key, summary) {
            Ok(o) => o,
            Err(e) => {
                let message = format!("{e:#}");
                let store = self.store();
                if let Some(row) = store.file_by_path(&key)? {
                    store.mark_failed(row.id, &message)?;
                }
                FileOutcome::Failed(message)
            }
        };
        match &outcome {
            FileOutcome::Indexed { pages, text_chunks, images } => {
                summary.indexed += 1;
                summary.pages += *pages as u64;
                summary.text_chunks += *text_chunks as u64;
                summary.images += *images as u64;
                summary.unoptimized += *text_chunks as u64;
                // 只重算向量的不算進紀錄，紀錄是完整建一個檔案要多久
                if summary.reembedded == reembedded {
                    let h = &mut summary.history[is_image_file(path) as usize];
                    h.files += 1;
                    h.text_chunks += *text_chunks as u64;
                    h.images += *images as u64;
                    h.seconds += started.elapsed().as_secs_f64();
                }
            }
            FileOutcome::Duplicate => summary.duplicates += 1,
            FileOutcome::Unchanged => summary.unchanged += 1,
            FileOutcome::StillFailed => summary.still_failed += 1,
            FileOutcome::Failed(_) => summary.failed += 1,
        }
        if summary.unoptimized >= OPTIMIZE_EVERY {
            let t = Instant::now();
            let mut store = self.store();
            store.optimize()?;
            store.flush()?;
            summary.optimize_seconds += t.elapsed().as_secs_f64();
            summary.unoptimized = 0;
        }
        Ok(outcome)
    }

    /// 第三步：卸載建索引用的模型（含影像編碼器和文字辨識），整理向量索引。
    pub fn finish_index(&self, summary: &mut IndexSummary) -> Result<()> {
        self.unload_index_model();
        if summary.history.iter().any(|h| h.files > 0) {
            let mut total = self.history();
            for (t, h) in total.iter_mut().zip(summary.history) {
                t.files += h.files;
                t.text_chunks += h.text_chunks;
                t.images += h.images;
                t.seconds += h.seconds;
            }
            self.store().set_setting("history", &serde_json::to_string(&total)?)?;
        }
        if summary.indexed > 0 || summary.removed > 0 {
            let t = Instant::now();
            self.remove_unused_page_images()?;
            let mut store = self.store();
            store.optimize()?;
            store.flush()?;
            summary.optimize_seconds += t.elapsed().as_secs_f64();
        }
        Ok(())
    }

    /// 刪掉已經沒有檔案用到的頁面影像（檔案被刪除、取消來源、或內容改過）。
    /// 頁面影像的檔名以內容雜湊的前 16 碼開頭。
    fn remove_unused_page_images(&self) -> Result<()> {
        let pages_dir = self.config.data_dir.join("pages");
        let Ok(entries) = std::fs::read_dir(&pages_dir) else { return Ok(()) };
        let used: HashSet<String> = self.store().list_files()?.into_iter().filter_map(|r| r.content_hash).map(|h| h[..16.min(h.len())].to_string()).collect();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let prefix = name.split('_').next().unwrap_or("");
            if !used.contains(prefix) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    /// 索引是用哪一組設定建的，記在索引裡。設定不同時，依 `rebuild_on_version_change` 逐檔重建或拒絕使用。
    fn check_versions(&self) -> Result<()> {
        let current = self.config.versions();
        let store = self.store();
        match store.setting("versions")? {
            Some(saved) if saved != current && !self.config.rebuild_on_version_change => {
                bail!("這份索引是用不同的設定建的，不能混用。\n  索引：{saved}\n  現在：{current}\n請換一個索引資料夾，或加上 --rebuild 逐檔重建。")
            }
            Some(saved) if saved == current => Ok(()),
            // 新索引，或要重建：記下新的版本，各檔案依自己記錄的版本決定要不要重做
            _ => store.set_setting("versions", &current),
        }
    }

    /// 補段落的結尾頁（實驗 94）：2026-10-16 以前建的索引只記段落開頭的頁碼，跨頁段落的答案常在下一頁。
    /// 重新解析、切分每個檔案，只把結尾頁寫回 SQLite，不重算向量（切法沒變時，chunk 編號由內容、頁碼、順序決定，
    /// 對得上的就是同一段）。檔案不見了或解析失敗就跳過，那些段落當作不跨頁。
    /// `progress` 在每個檔案開始前收到它的路徑，回傳 false 就停下（已補的保留，下次接著補）。
    /// 回傳（處理了幾個檔案、幾個段落是跨頁的）。
    pub fn fill_page_end(&self, progress: &mut dyn FnMut(&str) -> bool) -> Result<(usize, usize)> {
        let files = self.store().files_without_page_end()?;
        let pages_dir = self.config.data_dir.join("pages");
        let options = self.index_options();
        let (mut done, mut spanning) = (0, 0);
        for row in files {
            if !progress(&row.path) {
                break;
            }
            let mut ends = Vec::new();
            if let Some(hash) = row.content_hash.as_deref() {
                let converted = lock(&self.indexer).converter.convert_without_ocr(Path::new(&row.path), &pages_dir, &hash[..16.min(hash.len())]);
                if let Ok(mut converted) = converted {
                    // 和建索引時一樣：沒有影像模型時不留圖片區塊
                    if self.config.mmproj.is_none() {
                        converted.blocks.retain(|b| b.kind != BlockKind::Image);
                    }
                    for c in rag_core::chunk_blocks(&converted.blocks, &options.chunk) {
                        if c.page_end > c.page {
                            ends.push((rag_core::ids::point_id(hash, rag_core::ids::PointKind::Text, c.page, c.index), c.page_end));
                        }
                    }
                }
            }
            spanning += self.store().set_page_end(row.id, &ends)?;
            done += 1;
        }
        Ok((done, spanning))
    }

    /// 這個檔案在索引裡有沒有圖片區塊（掃描頁或圖片檔）。
    fn has_image_chunks(&self, file_id: i64) -> Result<bool> {
        Ok(self.store().chunks_of_file(file_id)?.iter().any(|c| c.kind == ChunkKind::Image))
    }

    /// 解析和切分設定沒變、只換了向量模型或關鍵字切法：用 SQLite 裡的原文重算向量。
    fn reembed_file(&self, row: &FileRow, path: &Path) -> Result<FileOutcome> {
        let chunks = self.store().chunks_of_file(row.id)?;
        let mut text = Vec::new();
        let mut images = Vec::new();
        for c in chunks {
            match (c.kind, c.text, c.image_path) {
                (ChunkKind::Text, Some(t), _) => text.push(TextPoint {
                    id: c.id,
                    file_id: row.id,
                    page: c.page,
                    page_end: c.page_end,
                    chunk_index: c.chunk_index,
                    heading: c.heading,
                    text: t,
                    dense: Vec::new(),
                    sparse: Default::default(),
                }),
                (ChunkKind::Image, _, Some(p)) if self.config.mmproj.is_some() => {
                    images.push(ImagePoint { id: c.id, file_id: row.id, page: c.page, image_path: p, dense: Vec::new() })
                }
                _ => {}
            }
        }
        let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let batch = self.index_options().batch_size;
        let mut indexer = lock(&self.indexer);
        let mut store = SharedStore(&self.store);
        let report = reembed_document(&mut indexer.embedder, &mut store, title, text, images, batch)?;
        self.store().set_file_versions(row.id, &self.config.versions())?;
        Ok(FileOutcome::Indexed { pages: row.pages.unwrap_or(0), text_chunks: report.text_chunks, images: report.images })
    }

    fn index_file(&self, path: &Path, key: &str, summary: &mut IndexSummary) -> Result<FileOutcome> {
        let meta = std::fs::metadata(path).with_context(|| format!("讀不到 {key}"))?;
        let (size, mtime) = (meta.len(), modified_seconds(&meta));

        // 先用大小和修改時間判斷有沒有變，沒變就不必讀整個檔案
        let current = self.config.versions();
        // 先取出來再判斷：直接寫在 if let 裡，儲存層的鎖會一直佔到區塊結束，裡面重算向量時再鎖就卡死
        let existing = self.store().file_by_path(key)?;
        if let Some(row) = existing {
            let same = row.size == size && row.mtime == mtime;
            if same && row.status == FileStatus::Duplicate {
                return Ok(FileOutcome::Unchanged);
            }
            if same && row.status == FileStatus::Indexed {
                match row.versions.as_deref() {
                    Some(v) if v == current => return Ok(FileOutcome::Unchanged),
                    // 開了文字辨識以前建的、又沒有掃描頁（圖片區塊）的檔案：辨識不會改變內容，只更新版本
                    Some(v) if before_ocr(&current).as_deref() == Some(v) && !self.has_image_chunks(row.id)? => {
                        self.store().set_file_versions(row.id, &current)?;
                        return Ok(FileOutcome::Unchanged);
                    }
                    Some(v) if same_parsing(v, &current) => {
                        let outcome = self.reembed_file(&row, path)?;
                        summary.reembedded += 1;
                        return Ok(outcome);
                    }
                    // 解析或切分設定不同：往下整份重做
                    _ => {}
                }
            }
            // 失敗的檔案沒變就不重試；例外是被鎖住的，和太大但上限已經調高的
            let error = row.error.as_deref().unwrap_or("");
            let retry = error.starts_with(LOCKED) || (error.starts_with(TOO_LARGE) && size <= self.max_file_bytes());
            if same && row.status == FileStatus::Failed && !retry {
                return Ok(FileOutcome::StillFailed);
            }
        }
        let limit = self.max_file_bytes();
        if size > limit {
            let message = format!("{TOO_LARGE}（{} MB）", limit >> 20);
            let store = self.store();
            let file_id = store.register_file(key, size, mtime)?;
            store.mark_failed(file_id, &message)?;
            return Ok(FileOutcome::Failed(message));
        }
        let bytes = std::fs::read(path).map_err(|e| {
            // Windows：32 是共用違規、33 是鎖定違規，都是別的程式正開著這個檔案
            if matches!(e.raw_os_error(), Some(32) | Some(33)) {
                anyhow::anyhow!("{LOCKED}，下次更新時重試")
            } else {
                anyhow::Error::new(e).context(format!("讀不到 {key}"))
            }
        });
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => {
                // 先登記，失敗原因才記得下來
                self.store().register_file(key, size, mtime)?;
                return Err(e);
            }
        };
        let hash = content_hash(&bytes);
        drop(bytes);

        // 清單的讀寫集中在這一段，做完就放掉儲存層的鎖
        let file_id = {
            let mut store = self.store();
            let file_id = store.register_file(key, size, mtime)?;
            // 先清掉這個檔案舊的資料（內容改過，或上次做到一半）
            store.delete_file(file_id)?;
            if let Some(original) = store.indexed_file_with_hash(&hash, file_id)? {
                store.mark_duplicate(file_id, &hash, original)?;
                return Ok(FileOutcome::Duplicate);
            }
            file_id
        };

        let mut indexer = lock(&self.indexer);
        let t = Instant::now();
        let pages_dir = self.config.data_dir.join("pages");
        let mut converted = indexer.converter.convert(path, &pages_dir, &hash[..16])?;
        summary.convert_seconds += t.elapsed().as_secs_f64();
        if self.config.mmproj.is_none() {
            converted.blocks.retain(|b| b.kind != BlockKind::Image);
        }

        let t = Instant::now();
        let title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let options = self.index_options();
        let mut store = SharedStore(&self.store);
        let report = index_document(&mut indexer.embedder, &mut store, file_id, &hash, title, &converted.blocks, &options)?;
        summary.embed_seconds += t.elapsed().as_secs_f64();

        self.store().mark_indexed(file_id, &hash, converted.pages, &self.config.versions())?;
        Ok(FileOutcome::Indexed { pages: converted.pages, text_chunks: report.text_chunks, images: report.images })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_ocr_drops_only_the_ocr_tag() {
        let current = format!("liteparse-2.16.0+han1+{};model.gguf;chunk=256/32/true;headings=true;kw=2", rag_convert::OCR_VERSION);
        assert_eq!(before_ocr(&current).as_deref(), Some("liteparse-2.16.0+han1;model.gguf;chunk=256/32/true;headings=true;kw=2"));
        assert_eq!(before_ocr("liteparse-2.16.0+han1;model.gguf;chunk=256/32/true;headings=true;kw=2"), None);
    }

    #[test]
    fn under_compares_whole_components() {
        let folder = SourceEntry { path: "/data/docs".into(), is_folder: true, offline_since: None };
        assert!(is_under("/data/docs/a.pdf", &folder));
        assert!(is_under("/data/docs/sub/b.pdf", &folder));
        assert!(!is_under("/data/docs2/a.pdf", &folder));
        let file = SourceEntry { path: "/data/x.pdf".into(), is_folder: false, offline_since: None };
        assert!(is_under("/data/x.pdf", &file));
        assert!(!is_under("/data/x.pdf.bak", &file));
    }

    #[test]
    fn scan_skips_hidden_temp_unsupported_and_excluded() {
        let dir = std::env::temp_dir().join(format!("rag-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for f in ["a.pdf", "b.PNG", "~$c.pdf", ".d.pdf", "e.docx", "data/f.pdf", "sub/g.jpg"] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        let mut counts = ScanCounts::default();
        let found = scan(&dir, &[dir.join("data")], &mut counts).unwrap();
        let names: Vec<String> = found.iter().map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(names, vec!["a.pdf", "b.PNG", "sub/g.jpg"]);
        assert_eq!(counts.unsupported, 1);
        assert_eq!(counts.hidden, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
