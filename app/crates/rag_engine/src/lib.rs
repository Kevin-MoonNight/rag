//! 引擎：把文件轉換、模型執行、儲存和核心邏輯接起來，提供「把資料夾建成索引」和「搜尋」兩件事。
//! 命令列工具和桌面應用程式都用這一層。
//!
//! 引擎可以同時被多個執行緒使用：建索引在背景跑的時候，搜尋不用等它。內部有三把鎖——
//! 儲存層（只在讀寫的瞬間鎖住）、查詢用的模型、建索引用的模型和轉換器。

mod answer;
mod indexing;
mod shared;
pub mod throttle;

pub use answer::{AnswerEvent, AnswerOutcome, AnswerSource};
pub use indexing::{scan, FileOutcome, IndexSummary, Progress, ScanCounts, SourceEntry};
pub use rag_convert::{ConvertOptions, OcrConfig};
pub use rag_core::{AnswerOptions, ChunkOptions, SearchOptions, Source};
pub use rag_models::{system, GenerationReport};
pub use rag_store::{ChunkKind, FileRow, FileStatus, LocalStore};

use anyhow::{Context, Result};
use rag_convert::Converter;
use rag_core::IndexOptions;
use rag_models::{LlamaEmbedder, LlamaGenerator, ServerConfig};
use serde::Serialize;
use shared::{lock, SharedStore};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// 索引放在哪個資料夾。
    pub data_dir: PathBuf,
    /// llama-server 執行檔。
    pub server_exe: PathBuf,
    /// 向量模型（GGUF）。
    pub model: PathBuf,
    /// 影像編碼器（GGUF）。沒有就不處理掃描頁和圖片。
    pub mmproj: Option<PathBuf>,
    pub threads: usize,
    pub chunk: ChunkOptions,
    pub convert: ConvertOptions,
    /// 一次送幾個 chunk 給向量模型。
    pub batch_size: usize,
    /// 模型閒置多久後卸載。
    pub idle_unload: Duration,
    /// llama-server 的紀錄檔要不要留（放在 data_dir 底下）。
    pub keep_server_log: bool,
    /// 產生回答的模型（GGUF）。沒有就不提供回答。
    pub generator_model: Option<PathBuf>,
    /// 推測解碼用的草稿模型（MTP）。沒有就不用。
    pub generator_draft: Option<PathBuf>,
    /// 實體記憶體少於這個值（位元組）的電腦不提供回答。計畫的設定是 4 GB 的電腦不啟用、8 GB 的可以；
    /// 8 GB 的電腦系統回報的總量略少於 8 GB，所以門檻放在 6 GB。
    pub min_memory_for_answers: u64,
    /// 超過這個大小（位元組）的檔案不處理，標為失敗。
    pub max_file_bytes: u64,
    /// 索引是用不同設定建的時候：`true` 逐檔重建（只差在向量模型或關鍵字切法的，用存下的原文重算向量；
    /// 解析或切分設定不同的，重新解析）；`false` 拒絕使用（實驗時避免不小心覆蓋掉舊索引）。
    pub rebuild_on_version_change: bool,
    /// 向量搜尋時，量化向量粗篩取幾倍的候選再用完整向量重新計分。`None` 依索引的量化方式決定（int8 2 倍、1 位元 5 倍）。
    pub oversampling: Option<f64>,
}

impl EngineConfig {
    pub fn new(data_dir: impl Into<PathBuf>, server_exe: impl Into<PathBuf>, model: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            server_exe: server_exe.into(),
            model: model.into(),
            mmproj: None,
            threads: 6,
            chunk: ChunkOptions::default(),
            convert: ConvertOptions::default(),
            batch_size: 8,
            idle_unload: Duration::from_secs(300),
            keep_server_log: true,
            generator_model: None,
            generator_draft: None,
            min_memory_for_answers: 6 << 30,
            max_file_bytes: 200 << 20,
            rebuild_on_version_change: false,
            oversampling: None,
        }
    }

    /// 這份設定下建出來的索引的版本字串。任何一項不同，舊索引就不能直接沿用。
    pub fn versions(&self) -> String {
        let model = self.model.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        // 掃描頁的文字辨識（實驗 99）和實驗用的外部轉換結果（實驗 83）也算在解析器版本裡，免得和一般的索引混用
        let mut parser = rag_convert::PARSER_VERSION.to_string();
        if self.convert.ocr.is_some() {
            parser = format!("{parser}+{}", rag_convert::OCR_VERSION);
        }
        if self.convert.text_override.is_some() {
            parser.push_str("+override");
        }
        format!(
            "{};{};chunk={}/{}/{};headings={};kw={}",
            parser,
            model,
            self.chunk.max_tokens,
            self.chunk.overlap_tokens,
            self.chunk.split_at_headings,
            self.convert.detect_headings,
            rag_core::text::KEYWORD_VERSION
        )
    }
}

/// 一筆搜尋結果。
#[derive(Debug, Clone, Serialize)]
pub struct ResultItem {
    pub rank: usize,
    /// 輸出成 JSON 時轉成字串：64 位元的編號超過 JavaScript 數字能精確表示的範圍。
    #[serde(serialize_with = "id_as_string")]
    pub id: u64,
    /// 這一筆是從哪一份名單來的：text、image 或 keyword。
    pub source: &'static str,
    pub score: f32,
    pub file_path: String,
    pub page: u32,
    /// 段落跨頁時結尾所在的頁碼；不跨頁時和 `page` 相同。
    pub page_end: u32,
    pub is_image: bool,
    pub heading: Option<String>,
    /// 文字 chunk 的原文。
    pub text: Option<String>,
    /// 段落裡要標示的關鍵字位置（UTF-16 的開始和結束位置）。
    pub highlights: Vec<(u32, u32)>,
    /// 掃描頁或圖片檔的影像路徑。
    pub image_path: Option<String>,
    /// 內容完全相同的其他檔案。
    pub same_content: Vec<String>,
}

fn id_as_string<S: serde::Serializer>(id: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&id.to_string())
}

/// 建索引用到的東西：有影像編碼器的模型，和文件轉換器。處理一個檔案的期間整組鎖住。
struct Indexer {
    embedder: LlamaEmbedder,
    converter: Converter,
}

pub struct Engine {
    config: EngineConfig,
    store: Mutex<LocalStore>,
    /// 查詢用：不載入影像編碼器，佔用記憶體較少。
    query_embedder: Mutex<LlamaEmbedder>,
    indexer: Mutex<Indexer>,
    /// 產生回答用。沒有設定生成模型時是 `None`。
    generator: Option<Mutex<LlamaGenerator>>,
}

impl Engine {
    pub fn open(mut config: EngineConfig) -> Result<Self> {
        // 存進索引的頁面圖片路徑以這個資料夾為準：用相對路徑開時，換個工作目錄（例如命令列建、桌面版用）就找不到圖
        config.data_dir = std::path::absolute(&config.data_dir).with_context(|| format!("無法取得 {} 的完整路徑", config.data_dir.display()))?;
        // 文字辨識的模型或 ONNX Runtime 不在就不辨識（版本字串也不帶辨識的標記，掃描頁照舊只有圖片）
        if config.convert.ocr.as_ref().is_some_and(|o| !o.available()) {
            config.convert.ocr = None;
        }
        if let Some(ocr) = config.convert.ocr.as_mut() {
            ocr.threads = config.threads;
        }
        let mut store = LocalStore::open(&config.data_dir)?;
        if let Some(v) = config.oversampling {
            store.set_oversampling(v);
        }
        let log = |name: &str| config.keep_server_log.then(|| config.data_dir.join(name));
        let server = |name: &str, mmproj: Option<PathBuf>| {
            let mut s = ServerConfig::new(&config.server_exe, &config.model);
            s.threads = config.threads;
            s.mmproj = mmproj;
            s.log_file = log(name);
            s
        };
        let query_embedder = LlamaEmbedder::new(server("llama-query.log", None));
        let indexer = Indexer {
            embedder: LlamaEmbedder::new(server("llama-index.log", config.mmproj.clone())),
            converter: Converter::new(config.convert.clone())?,
        };
        let generator = config.generator_model.as_ref().map(|model| {
            let mut s = ServerConfig::generation(&config.server_exe, model);
            s.threads = config.threads;
            s.log_file = log("llama-answer.log");
            s.draft = config.generator_draft.clone();
            Mutex::new(LlamaGenerator::new(s))
        });
        Ok(Self { config, store: Mutex::new(store), query_embedder: Mutex::new(query_embedder), indexer: Mutex::new(indexer), generator })
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// 儲存層。拿著回傳值的期間其他執行緒不能讀寫，用完要馬上放掉。
    pub fn store(&self) -> MutexGuard<'_, LocalStore> {
        lock(&self.store)
    }

    fn index_options(&self) -> IndexOptions {
        IndexOptions { chunk: self.config.chunk.clone(), batch_size: self.config.batch_size }
    }

    /// 合併前的三份名單和合併後的名單（評測用）。
    pub fn search_raw(&self, query: &str, options: &SearchOptions) -> Result<rag_core::SearchResult> {
        let mut embedder = lock(&self.query_embedder);
        rag_core::search(&mut *embedder, &SharedStore(&self.store), query, options)
    }

    /// 搜尋。回傳合併後的名單，附上原文和位置。
    pub fn search(&self, query: &str, options: &SearchOptions) -> Result<Vec<ResultItem>> {
        let raw = self.search_raw(query, options)?;
        self.result_items(&raw, query)
    }

    /// 只用關鍵字搜尋：不需要向量模型，馬上有結果。模型還沒載入時先用它，或使用者要精確查代號時用。
    pub fn search_keywords(&self, query: &str, options: &SearchOptions) -> Result<Vec<ResultItem>> {
        let raw = rag_core::search_keywords(&SharedStore(&self.store), query, options)?;
        self.result_items(&raw, query)
    }

    /// 舊的索引若是用相對路徑的資料夾建的，頁面圖片存的是相對路徑；頁面圖片一定在 `<資料夾>/pages` 底下，用檔名補回完整路徑。
    /// 圖片檔本身（使用者的 .jpg 等）存的是原檔路徑，不在 pages 底下，不動。
    fn page_image_path(&self, path: String) -> String {
        let p = Path::new(&path);
        if p.is_absolute() {
            return path;
        }
        match p.file_name().map(|name| self.config.data_dir.join("pages").join(name)) {
            Some(full) if full.exists() => full.to_string_lossy().to_string(),
            _ => path,
        }
    }

    /// 合併後的名單附上原文、位置和要標示的關鍵字。
    fn result_items(&self, raw: &rag_core::SearchResult, query: &str) -> Result<Vec<ResultItem>> {
        let store = self.store();
        let mut items = Vec::with_capacity(raw.merged.len());
        for m in &raw.merged {
            // 索引裡有、清單裡卻找不到的資料（例如刪除做到一半）直接略過
            let Some(chunk) = store.chunk(m.id)? else { continue };
            items.push(ResultItem {
                rank: items.len() + 1,
                id: m.id,
                source: match m.source {
                    Source::TextVector => "text",
                    Source::ImageVector => "image",
                    Source::Keyword => "keyword",
                },
                score: m.score,
                same_content: store.duplicates_of(chunk.file_id)?,
                file_path: chunk.file_path,
                page: chunk.page,
                page_end: chunk.page_end,
                is_image: chunk.kind == ChunkKind::Image,
                heading: chunk.heading,
                highlights: chunk.text.as_deref().map(|t| rag_core::text::highlight(t, query)).unwrap_or_default(),
                text: chunk.text,
                image_path: chunk.image_path.map(|p| self.page_image_path(p)),
            });
        }
        Ok(items)
    }

    /// 先把查詢用的模型載入，第一次搜尋就不用等。
    pub fn warm_up(&self) -> Result<()> {
        lock(&self.query_embedder).load()
    }

    /// 先把回答用的模型載入（搜尋結果出來、使用者還在看的時候），按「產生回答」時就不用再等約 4 秒的載入
    /// （實驗 96）。可用記憶體少於 `min_available`、這台電腦不產生回答、或正在回答時不做。回傳這次有沒有載入。
    /// 沒用到的話照常在閒置計時到了之後卸載。
    pub fn warm_up_generator(&self, min_available: u64) -> Result<bool> {
        if self.answer_availability().is_err() || rag_models::system::memory().is_some_and(|(_, available)| available < min_available) {
            return Ok(false);
        }
        let Some(Ok(mut g)) = self.generator.as_ref().map(|g| g.try_lock()) else { return Ok(false) };
        if g.is_loaded() {
            return Ok(false);
        }
        g.load()?;
        Ok(true)
    }

    /// 查詢用的模型目前有沒有載入。
    pub fn query_model_loaded(&self) -> bool {
        lock(&self.query_embedder).is_loaded()
    }

    /// 卸載閒置太久的查詢模型。應用程式定時呼叫。回傳這次有沒有卸載。
    /// （建索引用的模型在每次建完時就卸載，見 [`Engine::finish_index`]。）
    pub fn unload_idle(&self) -> bool {
        let query = lock(&self.query_embedder).unload_if_idle(self.config.idle_unload);
        // 正在回答時鎖被佔著，不算閒置
        let answer = self.generator.as_ref().and_then(|g| g.try_lock().ok()).is_some_and(|mut g| g.unload_if_idle(self.config.idle_unload));
        query || answer
    }

    /// 馬上卸載查詢用的模型。
    pub fn unload_query_model(&self) {
        lock(&self.query_embedder).unload();
    }

    /// 目前載入中的模型子行程編號（量記憶體用）：查詢用、建索引用。
    /// 建索引進行中時第二個值拿不到（那把鎖被佔著），回傳 `None`。
    pub fn model_pids(&self) -> (Option<u32>, Option<u32>) {
        let query = lock(&self.query_embedder).pid();
        let index = self.indexer.try_lock().ok().and_then(|i| i.embedder.pid());
        (query, index)
    }

    /// 建索引用的模型改用幾個執行緒、要不要背景執行（白天和夜間切換時呼叫，在檔案之間呼叫）。
    pub fn set_index_profile(&self, threads: usize, background: bool) {
        let mut indexer = lock(&self.indexer);
        indexer.embedder.set_profile(threads, background);
        indexer.converter.set_ocr_threads(threads);
    }

    /// 卸載建索引用的模型和文字辨識模型（暫停建索引時釋放記憶體）。
    pub fn unload_index_model(&self) {
        let mut indexer = lock(&self.indexer);
        indexer.embedder.unload();
        indexer.converter.unload_ocr();
    }

    /// 建索引時會不會辨識掃描頁的文字（有模型，而且沒有載入失敗）。
    pub fn ocr_enabled(&self) -> bool {
        // 建索引進行中時鎖被佔著：不等，照設定回答（載入失敗的話，建完之後就看得到）
        self.config.convert.ocr.is_some() && self.indexer.try_lock().map_or(true, |i| i.converter.ocr_enabled())
    }

    /// 可用記憶體少於 `min_available` 時，馬上卸載沒在使用的模型，不等閒置計時。回傳有沒有卸載。
    pub fn unload_if_low_memory(&self, min_available: u64) -> bool {
        let Some((_, available)) = rag_models::system::memory() else { return false };
        if available >= min_available {
            return false;
        }
        self.unload_unused()
    }

    /// 馬上卸載沒在使用的模型（查詢中、回答中的不動）。視窗縮小、可用記憶體不足時用。回傳有沒有卸載。
    pub fn unload_unused(&self) -> bool {
        let mut unloaded = false;
        if let Ok(mut q) = self.query_embedder.try_lock() {
            unloaded |= q.is_loaded();
            q.unload();
        }
        if let Some(Ok(mut g)) = self.generator.as_ref().map(|g| g.try_lock()) {
            unloaded |= g.is_loaded();
            g.unload();
        }
        unloaded
    }

    /// 生成模型的子行程編號（量記憶體用）。正在回答時拿不到，回傳 `None`。
    pub fn generator_pid(&self) -> Option<u32> {
        self.generator.as_ref().and_then(|g| g.try_lock().ok()).and_then(|g| g.pid())
    }
}
