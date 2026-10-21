//! 命令列工具：開發和評測用。
//!
//! ```text
//! rag convert <檔案>                 看一個檔案轉出來的中間格式
//! rag index [資料夾]                 把資料夾加入來源並建成索引；不給資料夾就是全部來源更新一次
//! rag remove-source <路徑>           取消一個來源，它的檔案從索引拿掉
//! rag search <問題>                  搜尋
//! rag eval <題目檔.jsonl>            跑評測題目，算命中率（--pages 頁標註檔：算答案頁的命中率和 nDCG）
//! rag ask <問題>                     搜尋並產生回答
//! rag ask-eval <題目檔.jsonl>        用評測題目跑產生回答，統計出處和耗時
//! rag stats                          索引裡有什麼
//! ```
//!
//! 共用參數：`--root`（專案根目錄，用來找預設的執行檔和模型）、`--data`（索引資料夾）、
//! `--server`、`--model`、`--mmproj`、`--gen-model`、`--gen-draft`（MTP 草稿模型）、`--threads`、`--rebuild`（索引設定不同時逐檔重建）、
//! `--ocr`（掃描頁和圖片檔辨識文字；模型 `--ocr-models`、ONNX Runtime `--ocr-runtime`）。

mod args;
mod ask;
mod bench;
mod bench_http;
mod eval;

use anyhow::{bail, Context, Result};
use args::Args;
use rag_engine::{ChunkOptions, ConvertOptions, Engine, EngineConfig, FileOutcome, OcrConfig, SearchOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn main() {
    if let Err(e) = run() {
        eprintln!("錯誤：{e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = Args::parse(std::env::args().skip(1).collect());
    let command = args.positional(0).context("請指定指令：convert、index、search、eval、ask、ask-eval、stats")?;
    match command.as_str() {
        "convert" => convert(&mut args),
        "index" => index(&mut args),
        "remove-source" => remove_source(&mut args),
        "fill-page-end" => fill_page_end(&mut args),
        "search" => search(&mut args),
        "eval" => eval::run(&mut args),
        "ask" => ask::ask(&mut args),
        "ask-eval" => ask::eval(&mut args),
        "stats" => stats(&mut args),
        "bench-store" => bench::run(&mut args),
        "requantize" => bench::requantize(&mut args),
        "replicate" => bench::replicate(&mut args),
        "bench-http" => {
            let config = engine_config(&mut args)?;
            bench_http::run(&mut args, config.server_exe, config.model, config.threads)
        }
        other => bail!("不認得的指令：{other}"),
    }
}

/// 頁碼；段落跨頁時寫成「66–67」。
fn pages(page: u32, page_end: u32) -> String {
    if page_end > page { format!("{page}–{page_end}") } else { page.to_string() }
}

fn convert_options(args: &mut Args) -> Result<ConvertOptions> {
    let mut o = ConvertOptions::default();
    if args.flag("--no-headings") {
        o.detect_headings = false;
    }
    if let Some(v) = args.number("--min-text-chars")? {
        o.min_text_chars = v;
    }
    // 實驗 83：有文字層的頁改用 docling.rs 先轉好的 Markdown
    o.text_override = args.value("--text-override").map(PathBuf::from);
    Ok(o)
}

fn chunk_options(args: &mut Args) -> Result<ChunkOptions> {
    let mut o = ChunkOptions::default();
    if let Some(v) = args.number("--max-tokens")? {
        o.max_tokens = v;
    }
    if let Some(v) = args.number("--overlap")? {
        o.overlap_tokens = v;
    }
    if args.flag("--no-heading-split") {
        o.split_at_headings = false;
    }
    Ok(o)
}

/// 從共用參數組出引擎設定。沒指定的路徑用專案根目錄底下的預設位置。
pub(crate) fn engine_config(args: &mut Args) -> Result<EngineConfig> {
    let root = PathBuf::from(args.value("--root").or_else(|| std::env::var("RAG_ROOT").ok()).unwrap_or_else(|| ".".into()));
    let default_server = if cfg!(windows) { "bin/win-p4-noomp/llama-server.exe" } else { "bin/linux-p4/llama-server" };
    let path = |args: &mut Args, name: &str, default: &str| args.value(name).map(PathBuf::from).unwrap_or_else(|| root.join(default));

    let data = path(args, "--data", "work/index");
    let server = path(args, "--server", default_server);
    let model = path(args, "--model", "models/embeddinggemma-2/embeddinggemma-2-BF16.gguf");
    let mmproj = path(args, "--mmproj", "models/derived/mmproj-embeddinggemma-2-vision-BF16.gguf");
    let gen_model = path(args, "--gen-model", "models/derived/e2b-q4_0-exact-head-zhtw-en.gguf");

    let mut config = EngineConfig::new(data, server, model);
    config.mmproj = if args.flag("--no-images") { None } else { Some(mmproj) };
    // 生成模型只在產生回答時才啟動，檔案在就設定上去
    config.generator_model = gen_model.exists().then_some(gen_model);
    config.generator_draft = args.value("--gen-draft").map(|p| root.join(p));
    if let Some(v) = args.number("--threads")? {
        config.threads = v;
    }
    if let Some(v) = args.number("--batch")? {
        config.batch_size = v;
    }
    config.chunk = chunk_options(args)?;
    config.convert = convert_options(args)?;
    // 掃描頁的文字辨識（實驗 99）：命令列加 --ocr 才開，實驗才能和以前的結果比（桌面程式預設開）
    if args.flag("--ocr") {
        let runtime = if cfg!(windows) { "bin/app-win/onnxruntime.dll" } else { "bin/app-linux/libonnxruntime.so.1.28.0" };
        let runtime = path(args, "--ocr-runtime", runtime);
        let models = path(args, "--ocr-models", "models/ocr/ppocr");
        let ocr = OcrConfig::in_dir(&models, runtime);
        if !ocr.available() {
            bail!("找不到文字辨識的模型或 ONNX Runtime：{}、{}", models.display(), ocr.runtime.display());
        }
        config.convert.ocr = Some(ocr);
    }
    // 索引是用不同設定建的時候逐檔重建（預設拒絕，避免實驗時不小心改掉舊索引）
    config.rebuild_on_version_change = args.flag("--rebuild");
    if let Some(v) = args.value("--oversampling") {
        config.oversampling = Some(v.parse().context("--oversampling 要是數字")?);
    }
    Ok(config)
}

pub(crate) fn search_options(args: &mut Args) -> Result<SearchOptions> {
    let mut o = SearchOptions::default();
    if let Some(v) = args.number("--candidates")? {
        o.candidates = v;
    }
    if let Some(v) = args.number("-k")? {
        o.limit = v;
    }
    if args.flag("--take-turns") {
        o.take_turns = true;
        // 舊做法配的補償值
        o.image_offset = 0.08;
    }
    if let Some(v) = args.value("--image-offset") {
        o.image_offset = v.parse().context("--image-offset 要是數字")?;
    }
    if args.flag("--no-keywords") {
        o.use_keywords = false;
    }
    if let Some(v) = args.number("--agree")? {
        o.agree_depth = v;
    }
    if args.flag("--no-image-search") {
        o.use_images = false;
    }
    Ok(o)
}

/// 等幾秒再結束，讓外部工具有時間量記憶體。
pub(crate) fn hold(args: &mut Args, engine: &Engine) -> Result<()> {
    if let Some(seconds) = args.number("--hold")? {
        let (query, index) = engine.model_pids();
        println!(
            "HOLD pid={} query_model_pid={:?} index_model_pid={:?} answer_model_pid={:?} seconds={seconds}",
            std::process::id(),
            query,
            index,
            engine.generator_pid()
        );
        std::thread::sleep(Duration::from_secs(seconds as u64));
    }
    Ok(())
}

fn convert(args: &mut Args) -> Result<()> {
    let options = convert_options(args)?;
    let out_dir = PathBuf::from(args.value("--image-dir").unwrap_or_else(|| "work/convert-pages".into()));
    let json = args.flag("--json");
    let file = args.positional(1).context("請指定檔案")?;
    args.finish()?;

    let converter = rag_convert::Converter::new(options)?;
    let t = Instant::now();
    let converted = converter.convert(Path::new(&file), &out_dir, "convert")?;
    let seconds = t.elapsed().as_secs_f64();
    if json {
        println!("{}", serde_json::to_string_pretty(&converted.blocks)?);
        return Ok(());
    }
    println!("頁數 {}（有文字 {}、轉成圖片 {}），區塊 {}，耗時 {:.2} 秒", converted.pages, converted.text_pages, converted.image_pages, converted.blocks.len(), seconds);
    for b in &converted.blocks {
        match (&b.image_path, b.heading_level) {
            (Some(p), _) => println!("--- p{} [圖片] {p}", b.page),
            (None, Some(level)) => println!("--- p{} [標題 {level}] {}", b.page, b.text),
            (None, None) => println!("--- p{} [文字 {} 字]\n{}", b.page, b.text.chars().count(), b.text),
        }
    }
    Ok(())
}

fn index(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    let quiet = args.flag("--quiet");
    let folder = args.positional(1);
    let engine = Engine::open(config)?;
    println!("設定：{}", engine.config().versions());
    let mut report = |p: rag_engine::Progress| {
        let what = match p.outcome {
            FileOutcome::Indexed { pages, text_chunks, images } => format!("完成：{pages} 頁、{text_chunks} 個文字 chunk、{images} 張圖"),
            FileOutcome::Duplicate => "內容重複，略過".to_string(),
            FileOutcome::Unchanged => "沒有變動，略過".to_string(),
            FileOutcome::StillFailed => "上次失敗、檔案沒有變動，不重試".to_string(),
            FileOutcome::Failed(e) => format!("失敗：{e}"),
        };
        if !quiet || matches!(p.outcome, FileOutcome::Failed(_)) {
            println!("[{}/{}] {:.1}s {} — {}", p.done, p.total, p.seconds, p.path, what);
        }
    };
    let summary = match folder {
        Some(folder) => engine.index_folder(Path::new(&folder), &mut report)?,
        None => engine.update(&mut report)?,
    };
    println!("{}", serde_json::to_string_pretty(&summary)?);
    hold(args, &engine)?;
    args.finish()
}

/// 舊索引補段落的結尾頁，不重算向量（實驗 94）。
fn fill_page_end(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    args.finish()?;
    let engine = Engine::open(config)?;
    let t = Instant::now();
    let (files, spanning) = engine.fill_page_end(&mut |path| {
        println!("{path}");
        true
    })?;
    println!("補了 {files} 個檔案，跨頁的段落 {spanning} 個，耗時 {:.1} 秒", t.elapsed().as_secs_f64());
    Ok(())
}

fn remove_source(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    let path = args.positional(1).context("請指定來源的路徑")?;
    let engine = Engine::open(config)?;
    let removed = engine.remove_source(&path)?;
    println!("已取消來源 {path}，從索引拿掉 {removed} 個檔案");
    args.finish()
}

fn search(args: &mut Args) -> Result<()> {
    let mut config = engine_config(args)?;
    let options = search_options(args)?;
    let show = args.number("--show")?.unwrap_or(8);
    // 測試用：搜尋完閒置這麼多秒，確認模型會卸載，再搜尋一次量重新載入的時間
    let idle_test = args.number("--idle-test")?;
    if let Some(seconds) = idle_test {
        config.idle_unload = Duration::from_secs(seconds as u64);
    }
    // 只用關鍵字：不載入模型
    let keyword_only = args.flag("--keyword-only");
    let query = args.positional(1).context("請指定要搜尋的問題")?;
    let engine = Engine::open(config)?;

    let t = Instant::now();
    if !keyword_only {
        engine.warm_up()?;
    }
    let load = t.elapsed();
    let t = Instant::now();
    let results = if keyword_only { engine.search_keywords(&query, &options)? } else { engine.search(&query, &options)? };
    println!("載入模型 {:.2} 秒，搜尋 {:.0} 毫秒，共 {} 筆", load.as_secs_f64(), t.elapsed().as_secs_f64() * 1000.0, results.len());
    for r in results.iter().take(show) {
        println!("\n#{} [{} {:.3}] {} 第 {} 頁", r.rank, r.source, r.score, r.file_path, pages(r.page, r.page_end));
        if let Some(text) = &r.text {
            let preview: String = text.chars().take(160).collect();
            println!("   {}", preview.replace('\n', " / "));
        }
        if let Some(image) = &r.image_path {
            println!("   [頁面圖片] {image}");
        }
        for other in &r.same_content {
            println!("   （內容相同：{other}）");
        }
    }
    if let Some(seconds) = idle_test {
        let (before, _) = engine.model_pids();
        std::thread::sleep(Duration::from_secs(seconds as u64 + 1));
        let unloaded = engine.unload_idle();
        println!("閒置 {} 秒：卸載={unloaded}，模型載入中={}，原本的子行程 {before:?}", seconds + 1, engine.query_model_loaded());
        // 留幾秒讓外部工具量到卸載後的記憶體
        std::thread::sleep(Duration::from_secs(4));
        let t = Instant::now();
        let again = engine.search(&query, &options)?;
        println!("卸載後再搜尋（含重新載入模型）{:.2} 秒，共 {} 筆", t.elapsed().as_secs_f64(), again.len());
    }
    hold(args, &engine)?;
    args.finish()
}

fn stats(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    args.finish()?;
    let engine = Engine::open(config)?;
    let files = engine.store().list_files()?;
    let (text, images) = engine.store().chunk_counts()?;
    let count = |s: rag_engine::FileStatus| files.iter().filter(|f| f.status == s).count();
    println!("索引設定：{}", engine.store().setting("versions")?.unwrap_or_else(|| "（還沒建）".into()));
    println!("向量量化：{}", engine.store().quantization().name());
    println!(
        "檔案 {}（已建 {}、重複 {}、待處理 {}、失敗 {}）",
        files.len(),
        count(rag_engine::FileStatus::Indexed),
        count(rag_engine::FileStatus::Duplicate),
        count(rag_engine::FileStatus::Pending),
        count(rag_engine::FileStatus::Failed)
    );
    println!("文字 chunk {text}，圖片 {images}，頁數 {}", files.iter().filter_map(|f| f.pages).map(u64::from).sum::<u64>());
    for f in files.iter().filter(|f| f.status == rag_engine::FileStatus::Failed) {
        println!("失敗：{} — {}", f.path, f.error.as_deref().unwrap_or(""));
    }
    Ok(())
}
