//! 桌面應用程式：視窗裡是 React 寫的搜尋畫面，這裡提供它呼叫的指令。
//!
//! 檔案位置（都可以用環境變數指定，沒指定就用執行檔旁邊的預設位置）：
//! - `RAG_ROOT`：找 llama-server 和模型的根目錄，預設是執行檔所在的資料夾。
//! - `RAG_DATA`：索引資料夾，預設是 `<RAG_ROOT>/data`。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine as _;
use rag_engine::throttle::{self, Conditions, Decision, PauseReason, ThrottleSettings};
use rag_engine::{system, AnswerEvent, AnswerOptions, AnswerOutcome, Engine, EngineConfig, FileStatus, IndexSummary, OcrConfig, ResultItem, SearchOptions, SourceEntry};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{Manager, State};

/// 視窗現在是不是縮到最小（只在縮小、還原的那一下處理）。
static MINIMIZED: AtomicBool = AtomicBool::new(false);

/// 視窗縮到最小時，把畫面（WebView2）的記憶體目標調成 LOW，還原時調回 NORMAL（實驗 90、90b）。
/// LOW 讓 WebView2 把記憶體交還系統：工作管理員看到的畫面記憶體從約 90 MB 降到 2–3 MB。
/// 不做隱藏和暫停（TrySuspend）：記憶體沒有再少，CPU 也沒有差，還多了恢復時出錯的風險。
#[cfg(windows)]
fn webview_idle(webview: &tauri::WebviewWindow, minimized: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;
    let level = if minimized { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
    let _ = webview.with_webview(move |platform| unsafe {
        if let Ok(core) = platform.controller().CoreWebView2().and_then(|core| core.cast::<ICoreWebView2_19>()) {
            let _ = core.SetMemoryUsageTargetLevel(level);
        }
    });
}

/// 關掉 WebView2 的自動填入和密碼儲存（審查第 2 輪）：搜尋框打的病歷號、客戶編號
/// 不能被瀏覽器記到使用者資料夾裡。
#[cfg(windows)]
fn webview_private(webview: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings4;
    use windows_core::Interface;
    let _ = webview.with_webview(|platform| unsafe {
        if let Ok(settings) = platform.controller().CoreWebView2().and_then(|core| core.Settings()).and_then(|s| s.cast::<ICoreWebView2Settings4>()) {
            let _ = settings.SetIsGeneralAutofillEnabled(false);
            let _ = settings.SetIsPasswordAutosaveEnabled(false);
        }
    });
}

#[derive(Debug, Clone, Default, Serialize)]
struct IndexProgress {
    running: bool,
    /// 這次更新是怎麼開始的（手動、啟動時、每天定時、加入新來源）。
    reason: Option<String>,
    done: usize,
    total: usize,
    current: Option<String>,
    indexed: usize,
    duplicates: usize,
    unchanged: usize,
    failed: usize,
    moved: usize,
    removed: usize,
    offline_sources: usize,
    reembedded: usize,
    unsupported_files: Vec<String>,
    /// 開始處理前的估計。
    estimate_files: usize,
    estimate_chunks: u64,
    estimate_seconds: f64,
    finishing: bool,
    finished_at: Option<String>,
    /// 暫停中時的原因（使用者暫停、記憶體不足、電腦正忙）。
    paused: Option<String>,
    /// 使用者按了暫停（立刻反映，不等目前的檔案做完）。
    user_paused: bool,
    /// 目前的執行方式（白天背景、夜間全速）。
    mode: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct Status {
    files: usize,
    indexed: usize,
    failed: usize,
    text_chunks: u64,
    images: u64,
    can_index_images: bool,
    model_loaded: bool,
    /// 不能產生回答時的原因；可以時是 `None`。
    answer_unavailable: Option<String>,
    data_dir: String,
    progress: IndexProgress,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    results: Vec<ResultItem>,
    milliseconds: u64,
    /// 一次最多回幾筆；結果剛好這麼多時，畫面要說明只列出最相關的這幾筆
    limit: usize,
}

struct App {
    engine: Arc<Engine>,
    progress: Arc<Mutex<IndexProgress>>,
    stop: Arc<AtomicBool>,
    /// 最近一次的搜尋結果：產生回答時直接用它，不必再搜一次。
    last_search: Arc<Mutex<Option<(String, Vec<ResultItem>)>>>,
    /// 使用者按了「停止回答」。
    stop_answer: Arc<AtomicBool>,
}

fn text<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[tauri::command]
async fn status(app: State<'_, App>) -> Result<Status, String> {
    let engine = app.engine.clone();
    let mut progress = lock(&app.progress).clone();
    progress.user_paused = PAUSE.load(Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let store = engine.store();
        let files = store.list_files().map_err(text)?;
        let (text_chunks, images) = store.chunk_counts().map_err(text)?;
        drop(store);
        Ok(Status {
            files: files.len(),
            indexed: files.iter().filter(|f| f.status == FileStatus::Indexed).count(),
            failed: files.iter().filter(|f| f.status == FileStatus::Failed).count(),
            text_chunks,
            images,
            can_index_images: engine.config().mmproj.is_some(),
            model_loaded: engine.query_model_loaded(),
            answer_unavailable: engine.answer_availability().err(),
            data_dir: engine.config().data_dir.to_string_lossy().to_string(),
            progress,
        })
    })
    .await
    .map_err(text)?
}

/// 搜尋。`keyword_only` 時只用關鍵字（不需要模型，馬上有結果）：模型還沒載入時畫面先用它顯示，
/// 使用者也可以自己切換，用來精確查代號。
#[tauri::command]
async fn search(app: State<'_, App>, query: String, keyword_only: bool) -> Result<SearchResponse, String> {
    let engine = app.engine.clone();
    let last = app.last_search.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let options = SearchOptions::default();
        let results = if keyword_only { engine.search_keywords(&query, &options) } else { engine.search(&query, &options) }.map_err(text)?;
        *lock(&last) = Some((query, results.clone()));
        Ok(SearchResponse { results, milliseconds: started.elapsed().as_millis() as u64, limit: options.limit })
    })
    .await
    .map_err(text)?
}

/// 用搜尋結果產生回答。過程中把交給模型的資料和一段一段的回答經由 `events` 送給畫面。
#[tauri::command]
async fn answer(app: State<'_, App>, query: String, events: Channel<AnswerEvent>) -> Result<AnswerOutcome, String> {
    let engine = app.engine.clone();
    let last = lock(&app.last_search).clone();
    let stop = app.stop_answer.clone();
    stop.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let mut send = |event: AnswerEvent| events.send(event).is_ok() && !stop.load(Ordering::SeqCst);
        let options = AnswerOptions::default();
        match last {
            Some((q, results)) if q == query => engine.answer_from_results(&query, &results, &options, &mut send),
            _ => engine.answer(&query, &SearchOptions::default(), &options, &mut send),
        }
        .map_err(text)
    })
    .await
    .map_err(text)?
}

/// 停止產生回答。已經產生的部分保留在畫面上。
#[tauri::command]
fn stop_answer(app: State<'_, App>) {
    app.stop_answer.store(true, Ordering::SeqCst);
}

/// 預先載入模型，把載入時間藏在使用者打字、看結果的時候（實驗 96：查詢模型約 1.7 秒、回答模型約 4.4 秒）。
/// `answer` 為 false 時載入查詢用的模型（點到輸入框、開始打字時），true 時載入回答用的模型（搜尋結果出來時）。
/// 在背景執行，馬上回傳；已經載入就不做。沒用到的模型照常在閒置計時到了之後卸載。
#[tauri::command]
fn preload(app: State<'_, App>, answer: bool) {
    let engine = app.engine.clone();
    std::thread::spawn(move || {
        if answer {
            // 回答模型約 1.7 GB：可用記憶體 3 GB 以上才先載，不和使用者的其他程式搶
            let _ = engine.warm_up_generator(3 << 30);
        } else if !engine.query_model_loaded() {
            let _ = engine.warm_up();
        }
    });
}

/// 使用者加入的資料夾和檔案。
#[tauri::command]
async fn sources(app: State<'_, App>) -> Result<Vec<SourceEntry>, String> {
    let engine = app.engine.clone();
    tauri::async_runtime::spawn_blocking(move || engine.sources().map_err(text)).await.map_err(text)?
}

/// 加入一個資料夾或檔案，接著在背景更新索引。
#[tauri::command]
async fn add_source(app: State<'_, App>, path: String) -> Result<(), String> {
    let engine = app.engine.clone();
    tauri::async_runtime::spawn_blocking(move || engine.add_source(Path::new(&path)).map_err(text)).await.map_err(text)??;
    start_update(&app, "加入新來源").or_else(|e| if e == BUSY { Ok(()) } else { Err(e) })
}

/// 取消一個來源，它的檔案從索引拿掉（檔案本身不動）。建索引時不能取消。
#[tauri::command]
async fn remove_source(app: State<'_, App>, path: String) -> Result<usize, String> {
    if lock(&app.progress).running {
        return Err("正在更新索引，請先停止或等它做完".into());
    }
    let engine = app.engine.clone();
    tauri::async_runtime::spawn_blocking(move || engine.remove_source(&path).map_err(text)).await.map_err(text)?
}

const BUSY: &str = "已經在更新索引了";

/// 手動更新：全部來源重新比對一次。
#[tauri::command]
fn update_now(app: State<'_, App>) -> Result<(), String> {
    // 沒有來源時更新什麼都不會做；直接說明，免得按了沒反應（例如用命令列建的索引，搜尋範圍是空的）
    if app.engine.sources().map_err(text)?.is_empty() {
        return Err("搜尋範圍還是空的，請先到「搜尋範圍」加入資料夾或檔案".into());
    }
    start_update(&app, "手動更新")
}

fn start_update(app: &App, reason: &str) -> Result<(), String> {
    spawn_update(&app.engine, &app.progress, &app.stop, reason)
}

/// 在背景執行緒更新索引。進度寫在共用的狀態裡，畫面定時來讀。
fn spawn_update(engine: &Arc<Engine>, progress: &Arc<Mutex<IndexProgress>>, stop: &Arc<AtomicBool>, reason: &str) -> Result<(), String> {
    {
        let mut p = lock(progress);
        if p.running {
            return Err(BUSY.into());
        }
        *p = IndexProgress { running: true, reason: Some(reason.to_string()), ..Default::default() };
    }
    stop.store(false, Ordering::SeqCst);
    PAUSE.store(false, Ordering::SeqCst);
    let (engine, progress, stop) = (engine.clone(), progress.clone(), stop.clone());
    std::thread::spawn(move || {
        let result = run_update(&engine, &progress, &stop);
        let mut p = lock(&progress);
        p.running = false;
        p.finishing = false;
        p.current = None;
        p.finished_at = Some(chrono::Local::now().format("%Y/%m/%d %H:%M").to_string());
        if let Err(e) = result {
            p.error = Some(text(e));
        }
    });
    Ok(())
}

fn run_update(engine: &Engine, progress: &Mutex<IndexProgress>, stop: &AtomicBool) -> anyhow::Result<()> {
    let mut summary = IndexSummary::default();
    let files = engine.begin_update(None, &mut summary)?;
    {
        let mut p = lock(progress);
        p.total = files.len();
        p.moved = summary.moved;
        p.removed = summary.removed;
        p.offline_sources = summary.offline_sources;
        p.unsupported_files = std::mem::take(&mut summary.unsupported_files);
        p.estimate_files = summary.estimate_files;
        p.estimate_chunks = summary.estimate_chunks;
        p.estimate_seconds = summary.estimate_seconds;
    }
    let settings = load_schedule(engine).throttle;
    let cores = system::physical_cores().map(|(_, performance)| performance).unwrap_or(2);
    let mut monitor = system::CpuMonitor::new();
    monitor.others_busy();
    let mut paused: Option<PauseReason> = None;
    let mut awake: Option<system::KeepAwake> = None;
    let mut background = None;
    for path in &files {
        // 每個檔案開始前決定：暫停，或用幾個執行緒、要不要背景執行
        let decision = loop {
            if stop.load(Ordering::SeqCst) {
                break None;
            }
            let now = Conditions {
                hour: chrono::Timelike::hour(&chrono::Local::now()) as u8,
                user_paused: PAUSE.load(Ordering::SeqCst),
                available_memory: system::memory().map(|(_, available)| available),
                others_busy: monitor.others_busy(),
                performance_cores: cores,
            };
            match throttle::decide(&settings, &now, paused) {
                Decision::Pause(reason) => {
                    if paused.is_none() || reason == PauseReason::Memory {
                        // 暫停期間把建索引的模型卸載，記憶體還給使用者
                        engine.unload_index_model();
                    }
                    paused = Some(reason);
                    awake = None;
                    lock(progress).paused = Some(reason.describe().to_string());
                    std::thread::sleep(Duration::from_secs(5));
                }
                Decision::Run { threads, background: bg, night } => break Some((threads, bg, night)),
            }
        };
        let Some((threads, bg, night)) = decision else { break };
        if paused.take().is_some() {
            lock(progress).paused = None;
        }
        engine.set_index_profile(threads, bg);
        if background != Some(bg) {
            system::set_current_thread_background(bg);
            background = Some(bg);
        }
        if night && awake.is_none() {
            awake = Some(system::KeepAwake::new());
        } else if !night {
            awake = None;
        }
        {
            let mut p = lock(progress);
            p.current = Some(path.to_string_lossy().to_string());
            p.mode = Some(if night { format!("夜間全速（{threads} 個執行緒）") } else { format!("白天背景執行（{threads} 個執行緒）") });
        }
        engine.index_one(path, &mut summary)?;
        let mut p = lock(progress);
        p.done += 1;
        p.indexed = summary.indexed;
        p.duplicates = summary.duplicates;
        p.unchanged = summary.unchanged;
        p.failed = summary.failed + summary.still_failed;
        p.reembedded = summary.reembedded;
    }
    drop(awake);
    if background == Some(true) {
        system::set_current_thread_background(false);
    }
    lock(progress).finishing = true;
    engine.finish_index(&mut summary)?;
    // 2026-10-16 以前建的索引沒有段落結尾頁（實驗 94）：更新完順便補上，不重算向量；
    // 按了停止或暫停就停下，下次更新接著補。都補過之後這一步只是一個查詢
    engine.fill_page_end(&mut |path| {
        lock(progress).current = Some(format!("補段落結尾頁：{path}"));
        !stop.load(Ordering::SeqCst) && !PAUSE.load(Ordering::SeqCst)
    })?;
    Ok(())
}

/// 使用者按了「暫停」。只有一個視窗程式，用全域的旗標就夠了。
static PAUSE: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn pause_index(paused: bool) {
    PAUSE.store(paused, Ordering::SeqCst);
}

/// 自動更新的時機：程式啟動時、每天固定的時刻（例如晚上，不打擾使用者）；以及白天、夜間的分界。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Schedule {
    on_startup: bool,
    /// 每天幾點（0–23）更新；`None` 是不每天更新。
    daily_hour: Option<u8>,
    #[serde(default)]
    throttle: ThrottleSettings,
}

impl Default for Schedule {
    fn default() -> Self {
        Self { on_startup: true, daily_hour: Some(20), throttle: ThrottleSettings::default() }
    }
}

fn load_schedule(engine: &Engine) -> Schedule {
    engine.store().setting("schedule").ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
fn get_schedule(app: State<'_, App>) -> Schedule {
    load_schedule(&app.engine)
}

#[tauri::command]
fn set_schedule(app: State<'_, App>, schedule: Schedule) -> Result<(), String> {
    let t = &schedule.throttle;
    if schedule.daily_hour.is_some_and(|h| h > 23) || t.night_start > 23 || t.night_end > 23 {
        return Err("時刻要在 0 到 23 之間".into());
    }
    if t.day_threads == 0 {
        return Err("白天的執行緒數至少是 1".into());
    }
    let json = serde_json::to_string(&schedule).map_err(text)?;
    app.engine.store().set_setting("schedule", &json).map_err(text)
}

/// 單一檔案的大小上限（MB），超過的不處理。
#[tauri::command]
fn get_max_file_mb(app: State<'_, App>) -> u64 {
    app.engine.max_file_bytes() >> 20
}

#[tauri::command]
fn set_max_file_mb(app: State<'_, App>, mb: u64) -> Result<(), String> {
    app.engine.set_max_file_mb(mb).map_err(text)
}

/// 每分鐘檢查一次：到了每天更新的時刻、而且今天還沒自動更新過，就開始更新。
fn schedule_loop(engine: Arc<Engine>, progress: Arc<Mutex<IndexProgress>>, stop: Arc<AtomicBool>) {
    use chrono::Timelike;
    if load_schedule(&engine).on_startup && !engine.sources().unwrap_or_default().is_empty() {
        let _ = spawn_update(&engine, &progress, &stop, "啟動時更新");
    }
    loop {
        std::thread::sleep(Duration::from_secs(60));
        let Some(hour) = load_schedule(&engine).daily_hour else { continue };
        let now = chrono::Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        let last = engine.store().setting("last_daily_update").ok().flatten();
        if now.hour() as u8 == hour && last.as_deref() != Some(today.as_str()) && spawn_update(&engine, &progress, &stop, "每天定時更新").is_ok() {
            let _ = engine.store().set_setting("last_daily_update", &today);
        }
    }
}

/// 請背景的建索引工作在處理完目前這個檔案後停下來。已完成的部分會保留，下次接著做。
#[tauri::command]
fn stop_index(app: State<'_, App>) {
    app.stop.store(true, Ordering::SeqCst);
}

/// 頁面影像讀成 data URL。只允許讀索引資料夾裡的頁面圖片，或已經在索引清單裡的圖片檔。
#[tauri::command]
async fn page_image(app: State<'_, App>, path: String) -> Result<String, String> {
    let engine = app.engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let target = Path::new(&path);
        let pages_dir = engine.config().data_dir.join("pages");
        let allowed = target.starts_with(&pages_dir) || engine.store().file_by_path(&path).map_err(text)?.is_some();
        if !allowed || target.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err("不允許讀取這個檔案".to_string());
        }
        let bytes = std::fs::read(target).map_err(text)?;
        let mime = if path.to_ascii_lowercase().ends_with(".png") { "image/png" } else { "image/jpeg" };
        Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    })
    .await
    .map_err(text)?
}

/// 用系統預設的程式開啟檔案。只允許開啟索引清單裡的檔案。
#[tauri::command]
fn open_file(app: State<'_, App>, path: String) -> Result<(), String> {
    if app.engine.store().file_by_path(&path).map_err(text)?.is_none() {
        return Err("這個檔案不在索引裡".into());
    }
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(text)
}

/// 在檔案總管中顯示檔案（選取它）。和開啟檔案一樣只允許索引裡的檔案。
#[tauri::command]
fn reveal_file(app: State<'_, App>, path: String) -> Result<(), String> {
    if app.engine.store().file_by_path(&path).map_err(text)?.is_none() {
        return Err("這個檔案不在索引裡".into());
    }
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(text)
}

/// 用檔案總管打開程式的資料資料夾（設定頁「關於」）
#[tauri::command]
fn open_data_dir(app: State<'_, App>) -> Result<(), String> {
    tauri_plugin_opener::open_path(&app.engine.config().data_dir, None::<&str>).map_err(text)
}

fn engine_config() -> anyhow::Result<EngineConfig> {
    let exe_dir = std::env::current_exe()?.parent().map(Path::to_path_buf).unwrap_or_default();
    // 沒設 RAG_ROOT 時：正式安裝的模型就在執行檔旁邊；開發時執行檔在 bin\app-win，往上找有 models 的資料夾，
    // 這樣直接點兩下執行檔也開得起來
    let root = match std::env::var_os("RAG_ROOT") {
        Some(root) => PathBuf::from(root),
        None => exe_dir.ancestors().take(4).find(|dir| dir.join("models").is_dir()).map(Path::to_path_buf).unwrap_or(exe_dir),
    };
    let data = std::env::var_os("RAG_DATA").map(PathBuf::from).unwrap_or_else(|| root.join("data"));
    // 開發時的目錄配置和正式安裝後的配置都找一遍
    let first = |candidates: &[&str]| candidates.iter().map(|c| root.join(c)).find(|p| p.exists());
    // 修改五的版本（win-p5）才能讓 MTP 和「讀提示時跳過上層」同時開（實驗 88）
    let server = first(&["llama-server.exe", "bin/win-p5-noomp/llama-server.exe", "bin/win-p4-noomp/llama-server.exe", "bin/linux-p4/llama-server"])
        .ok_or_else(|| anyhow::anyhow!("找不到 llama-server（根目錄：{}）", root.display()))?;
    let model = first(&["models/embeddinggemma-2-BF16.gguf", "models/embeddinggemma-2/embeddinggemma-2-BF16.gguf"])
        .ok_or_else(|| anyhow::anyhow!("找不到向量模型（根目錄：{}）", root.display()))?;
    let mmproj = first(&["models/mmproj-embeddinggemma-2-vision-BF16.gguf", "models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf"]);
    let generator = first(&["models/e2b-q4_0-exact-head-zhtw-en.gguf", "models/derived/e2b-q4_0-exact-head-zhtw-en.gguf"]);
    let mut config = EngineConfig::new(data, server, model);
    config.mmproj = mmproj;
    config.generator_model = generator;
    // MTP 草稿模型預設開（使用者 10-16 決定）：回答和不開時相同，產生速度 19.9 → 27.1 token/秒，記憶體多約 194 MB；
    // 只有 8 GB 以上的電腦會產生回答（min_memory_for_answers），所以不另外設門檻
    config.generator_draft = first(&["models/mtp-gemma-4-E2B-it-Q4_0.gguf", "models/gemma-4-e2b/mtp-gemma-4-E2B-it-Q4_0.gguf"]);
    // 掃描頁和圖片檔建索引時自動辨識文字（使用者 10-20 決定，實驗 99）；模型或 ONNX Runtime 不在就照舊只留頁面圖片
    if let (Some(models), Some(runtime)) = (first(&["models/ppocr", "models/ocr/ppocr"]), first(&["onnxruntime.dll", "bin/app-win/onnxruntime.dll"])) {
        config.convert.ocr = Some(OcrConfig::in_dir(&models, runtime));
    }
    // 升級後索引的設定不同時，逐檔重建（能只重算向量的就不重新解析）
    config.rebuild_on_version_change = true;
    Ok(config)
}

/// 啟動失敗時視窗還沒建立，用系統的訊息方塊告訴使用者原因（視窗程式沒有主控台可以印）。
fn fatal(message: &str) -> ! {
    eprintln!("{message}");
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(window: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
        }
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (text, caption) = (wide(message), wide("文件搜尋"));
        // 0x10：錯誤圖示
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
        }
    }
    std::process::exit(1)
}

fn main() {
    let engine = match engine_config().and_then(Engine::open) {
        Ok(e) => Arc::new(e),
        Err(e) => fatal(&format!("無法啟動：{e:#}")),
    };

    // 每 30 秒檢查一次，閒置太久的查詢模型就卸載，把記憶體還給系統
    let idle = engine.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        idle.unload_idle();
        // 可用記憶體少於 1 GB：沒在用的模型馬上卸載，不等閒置計時
        idle.unload_if_low_memory(1 << 30);
    });

    let progress = Arc::new(Mutex::new(IndexProgress::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let (e, p, s) = (engine.clone(), progress.clone(), stop.clone());
    std::thread::spawn(move || schedule_loop(e, p, s));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(windows)]
            if let Some(webview) = app.get_webview_window("main") {
                webview_private(&webview);
            }
            Ok(())
        })
        .manage(App { engine, progress, stop, last_search: Arc::new(Mutex::new(None)), stop_answer: Arc::new(AtomicBool::new(false)) })
        .on_window_event(|window, event| {
            if !matches!(event, tauri::WindowEvent::Resized(_)) {
                return;
            }
            let minimized = window.is_minimized().unwrap_or(false);
            // 只在「縮小」和「還原」的那一下動作，一般的調整大小不管
            if MINIMIZED.swap(minimized, Ordering::SeqCst) == minimized {
                return;
            }
            if minimized {
                // 視窗縮到最小：使用者暫時不用，模型馬上卸載，不等 5 分鐘的閒置計時（計畫「閒置卸載的判斷」）
                let engine = window.state::<App>().engine.clone();
                std::thread::spawn(move || engine.unload_unused());
            }
            #[cfg(windows)]
            if let Some(webview) = window.app_handle().get_webview_window("main") {
                webview_idle(&webview, minimized);
            }
        })
        .invoke_handler(tauri::generate_handler![
            status,
            search,
            answer,
            stop_answer,
            preload,
            sources,
            add_source,
            remove_source,
            update_now,
            stop_index,
            pause_index,
            get_schedule,
            set_schedule,
            get_max_file_mb,
            set_max_file_mb,
            page_image,
            open_file,
            reveal_file,
            open_data_dir
        ])
        .run(tauri::generate_context!())
        .expect("視窗程式執行失敗");
}
