//! 量儲存層在大量資料下的表現：灌入隨機產生的資料，再跑查詢。不需要模型。
//!
//! ```text
//! rag bench-store --data <資料夾> --points 100000     灌資料、整理索引、查詢
//! rag bench-store --data <資料夾> --points 0          只開啟已有的資料夾並查詢（量查詢時的記憶體）
//! rag bench-store ... --optimize-every 5000            寫入途中每 5000 筆整理一次（量記憶體高峰）
//! rag replicate --data <原索引> --out <新資料夾> --times 20   把真實索引放大成 20 倍
//! ```

use crate::args::Args;
use anyhow::{Context, Result};
use rag_core::{SparseVec, Store, TextPoint};
use rag_store::{LocalStore, VECTOR_SIZE};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// 簡單的亂數產生器（xorshift），結果可重現。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn vector(&mut self) -> Vec<f32> {
        let mut v: Vec<f32> = (0..VECTOR_SIZE).map(|_| self.unit() - 0.5).collect();
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= norm);
        v
    }

    /// 模擬一個 chunk 的關鍵字：約 `terms` 個不同的詞，常見詞出現的機率高。
    fn sparse(&mut self, terms: usize, vocabulary: u32) -> SparseVec {
        let mut ids: Vec<u32> = (0..terms)
            .map(|_| {
                let u = self.unit().max(1e-6);
                ((u * u * u) * vocabulary as f32) as u32 // 偏向小編號，近似常見詞多、罕見詞少
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let values = ids.iter().map(|_| 0.5 + self.unit()).collect();
        SparseVec { indices: ids, values }
    }
}

pub fn run(args: &mut Args) -> Result<()> {
    let data = PathBuf::from(args.value("--data").context("請用 --data 指定資料夾")?);
    let points = args.number("--points")?.unwrap_or(0);
    let queries = args.number("--queries")?.unwrap_or(200);
    let hold = args.number("--hold")?;
    // 寫入途中每這麼多筆整理一次索引（0 = 只在最後整理）。沒整理的新資料（尤其是關鍵字）會留在記憶體裡。
    let optimize_every = args.number("--optimize-every")?.unwrap_or(0);
    // 量化向量粗篩取幾倍的候選（預設和搜尋時一樣是 2）
    let oversampling: Option<f64> = args.value("--oversampling").map(|v| v.parse()).transpose().context("--oversampling 要是數字")?;
    // 新建資料時用的量化方式（預設和新索引一樣）
    let quant = args.value("--quant").map(|q| rag_store::Quantization::parse(&q).context("--quant 只能是 1bit、1bit-q8 或 int8")).transpose()?.unwrap_or_default();
    args.finish()?;

    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let t = Instant::now();
    let mut store = LocalStore::open_with(&data, quant)?;
    if let Some(v) = oversampling {
        store.set_oversampling(v);
    }
    println!("開啟 {:.2} 秒", t.elapsed().as_secs_f64());

    if points > 0 {
        let file_id = store.register_file("/bench/synthetic.pdf", 0, 0)?;
        let text: String = "這是用來量測的假資料，內容長度接近一個真實的文字段落。".repeat(12);
        let t = Instant::now();
        let mut done = 0usize;
        while done < points {
            let batch: Vec<TextPoint> = (done..(done + 1000).min(points))
                .map(|i| TextPoint {
                    id: i as u64 + 1,
                    file_id,
                    page: (i / 3) as u32 + 1,
                    page_end: (i / 3) as u32 + 1,
                    chunk_index: i as u32,
                    heading: None,
                    text: text.clone(),
                    dense: rng.vector(),
                    sparse: rng.sparse(250, 200_000),
                })
                .collect();
            store.upsert_text(&batch)?;
            store.flush()?;
            done += batch.len();
            if optimize_every > 0 && done % optimize_every == 0 && done < points {
                store.optimize()?;
                store.flush()?;
            }
            if done % 20_000 == 0 {
                println!("  已寫入 {done} 筆，{:.0} 秒，記憶體高峰 {}", t.elapsed().as_secs_f64(), peak());
            }
        }
        println!("寫入 {points} 筆 {:.1} 秒", t.elapsed().as_secs_f64());
        let t = Instant::now();
        store.optimize()?;
        store.flush()?;
        println!("整理索引 {:.1} 秒", t.elapsed().as_secs_f64());
        println!("寫入和整理期間的記憶體高峰 {}", peak());
    }

    let (text_chunks, images) = store.chunk_counts()?;
    println!("資料量：文字 {text_chunks}、圖片 {images}");
    let mut dense_ms = Vec::new();
    let mut sparse_ms = Vec::new();
    for _ in 0..queries {
        let q = rng.vector();
        let t = Instant::now();
        let hits = store.search_text(&q, 50)?;
        dense_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        let q = rng.sparse(12, 200_000);
        let t = Instant::now();
        let keyword_hits = store.search_keywords(&q, 50)?;
        sparse_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        // 順便讀回前幾筆的原文，和實際查詢一樣
        for h in hits.iter().chain(keyword_hits.iter()).take(8) {
            store.chunk(h.id)?;
        }
    }
    let report = |name: &str, v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.total_cmp(b));
        if !v.is_empty() {
            println!("{name}（毫秒）：中位數 {:.2}、90% {:.2}、最慢 {:.1}", v[v.len() / 2], v[v.len() * 9 / 10], v[v.len() - 1]);
        }
    };
    report("向量查詢", &mut dense_ms);
    report("關鍵字查詢", &mut sparse_ms);
    println!("全程記憶體高峰 {}", peak());

    if let Some(seconds) = hold {
        println!("HOLD pid={} seconds={seconds}", std::process::id());
        std::thread::sleep(Duration::from_secs(seconds as u64));
    }
    Ok(())
}

/// 把一份索引複製成另一種量化方式（不重算向量），用來比較量化方式對準確度和記憶體的影響。
///
/// ```text
/// rag requantize --data <原索引> --out <新索引> --quant 1bit|1bit-q8|int8
/// ```
pub fn requantize(args: &mut Args) -> Result<()> {
    let from = PathBuf::from(args.value("--data").context("請用 --data 指定原索引")?);
    let to = PathBuf::from(args.value("--out").context("請用 --out 指定新索引的資料夾")?);
    let quant = args.value("--quant").context("請用 --quant 指定 1bit、1bit-q8 或 int8")?;
    let quantization = rag_store::Quantization::parse(&quant).context("--quant 只能是 1bit、1bit-q8 或 int8")?;
    args.finish()?;
    if to.exists() {
        anyhow::bail!("{} 已經存在，請先刪掉", to.display());
    }
    let t = Instant::now();
    // 檔案清單、原文、頁面影像照抄；SQLite 的寫入紀錄檔（-wal）也要一起抄
    std::fs::create_dir_all(&to)?;
    let source = LocalStore::open(&from)?;
    for name in ["index.sqlite", "index.sqlite-wal", "index.sqlite-shm"] {
        if from.join(name).exists() {
            std::fs::copy(from.join(name), to.join(name))?;
        }
    }
    if from.join("pages").exists() {
        std::fs::create_dir_all(to.join("pages"))?;
        for entry in std::fs::read_dir(from.join("pages"))?.flatten() {
            std::fs::copy(entry.path(), to.join("pages").join(entry.file_name()))?;
        }
    }
    let mut target = LocalStore::open_with(&to, quantization)?;
    let (text, image) = target.copy_vectors_from(&source)?;
    target.optimize()?;
    target.flush()?;
    println!("複製 {text} 個文字點、{image} 個圖片點，量化方式 {quant}，{:.1} 秒", t.elapsed().as_secs_f64());
    Ok(())
}

/// 把真實的索引放大成 N 倍（實驗用：量 1 萬到 50 萬個 chunk 時的記憶體和速度，資料仍然是真實文件的內容）。
///
/// ```text
/// rag replicate --data <原索引> --out <新資料夾> --times 20 [--noise 0.1]
/// ```
///
/// 先照抄原索引（同 `requantize`，量化方式沿用原索引的），再把每個檔案、chunk、向量多複製 N−1 份。
/// 複本的路徑是「/副本N」加上原路徑，搜尋結果裡會看到；稠密向量加了一點擾動，不會完全重複。
pub fn replicate(args: &mut Args) -> Result<()> {
    let from = PathBuf::from(args.value("--data").context("請用 --data 指定原索引")?);
    let to = PathBuf::from(args.value("--out").context("請用 --out 指定新索引的資料夾")?);
    let times = args.number("--times")?.context("請用 --times 指定放大幾倍")?.max(1);
    let noise: f32 = args.value("--noise").map(|v| v.parse()).transpose().context("--noise 要是數字")?.unwrap_or(0.1);
    args.finish()?;
    if to.exists() {
        anyhow::bail!("{} 已經存在，請先刪掉", to.display());
    }
    let t = Instant::now();
    std::fs::create_dir_all(&to)?;
    let source = LocalStore::open(&from)?;
    for name in ["index.sqlite", "index.sqlite-wal", "index.sqlite-shm"] {
        if from.join(name).exists() {
            std::fs::copy(from.join(name), to.join(name))?;
        }
    }
    if from.join("pages").exists() {
        std::fs::create_dir_all(to.join("pages"))?;
        for entry in std::fs::read_dir(from.join("pages"))?.flatten() {
            std::fs::copy(entry.path(), to.join("pages").join(entry.file_name()))?;
        }
    }
    // 放大的索引沿用原索引的量化方式
    let mut target = LocalStore::open_with(&to, source.quantization())?;
    let (text, image) = target.copy_vectors_from(&source)?;
    drop(source);
    println!("照抄 {text} 個文字點、{image} 個圖片點，{:.1} 秒", t.elapsed().as_secs_f64());
    let (more_text, more_image) = target.replicate(times - 1, noise)?;
    println!("複製 {} 份：多 {more_text} 個文字點、{more_image} 個圖片點，{:.1} 秒，記憶體高峰 {}", times - 1, t.elapsed().as_secs_f64(), peak());
    target.optimize()?;
    target.flush()?;
    let (chunks_text, chunks_image) = target.chunk_counts()?;
    println!("整理完：文字 chunk {chunks_text} 個、圖片 {chunks_image} 個，全部 {:.1} 秒，記憶體高峰 {}", t.elapsed().as_secs_f64(), peak());
    Ok(())
}

/// 這個程序到目前為止的記憶體高峰（MB），量不到時是「?」。
fn peak() -> String {
    rag_models::system::peak_memory().map_or("?".into(), |b| format!("{} MB", b >> 20))
}
