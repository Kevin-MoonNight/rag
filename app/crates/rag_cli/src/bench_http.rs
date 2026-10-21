//! 量呼叫 llama-server 的額外開銷（HTTP 和 JSON）佔一次向量計算的多少，用來決定要不要改用 base64
//! 傳向量、或自己寫工作程序。
//!
//! ```text
//! rag bench-http [--rounds 30]
//! ```
//!
//! - 最短的輸入（1 個字）：模型幾乎不用算，時間主要是 HTTP、JSON、排程，是額外開銷的上限。
//! - 一般的查詢（約 20 字）和一批 8 個約 256 token 的段落（建索引時的一批）。
//! - 在程式裡解析 8 個 768 維向量的 JSON 回應：base64 能省掉的就是這一段（加上傳輸量）。

use crate::args::Args;
use anyhow::Result;
use rag_core::Embedder;
use rag_models::{LlamaEmbedder, ServerConfig};
use std::time::Instant;

fn stats(name: &str, mut ms: Vec<f64>) {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = ms[ms.len() / 2];
    let min = ms[0];
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    println!("{name}：中位數 {median:.2} 毫秒、平均 {mean:.2}、最快 {min:.2}（{} 次）", ms.len());
}

pub fn run(args: &mut Args, server_exe: std::path::PathBuf, model: std::path::PathBuf, threads: usize) -> Result<()> {
    let rounds = args.number("--rounds")?.unwrap_or(30);
    args.finish()?;
    let mut config = ServerConfig::new(server_exe, model);
    config.threads = threads;
    let mut embedder = LlamaEmbedder::new(config);
    let t = Instant::now();
    embedder.load()?;
    println!("載入模型 {:.2} 秒，{threads} 執行緒", t.elapsed().as_secs_f64());

    let time = |f: &mut dyn FnMut() -> Result<()>| -> Result<f64> {
        let t = Instant::now();
        f()?;
        Ok(t.elapsed().as_secs_f64() * 1000.0)
    };
    // 先暖機
    embedder.embed_query("暖機")?;

    let mut tiny = Vec::new();
    for _ in 0..rounds {
        tiny.push(time(&mut || embedder.embed_query("a").map(|_| ()))?);
    }
    stats("最短的輸入（1 個字）", tiny);

    let mut query = Vec::new();
    for _ in 0..rounds {
        query.push(time(&mut || embedder.embed_query("信用合作社授信審議委員會要多少委員出席才能開會").map(|_| ()))?);
    }
    stats("一般的查詢（約 20 字）", query);

    let paragraph = "第一條 本準則依信用合作社法第二十一條第三項規定訂定之。信用合作社辦理授信，應設授信審議委員會，負責審議授信案件。".repeat(4);
    let titles: Vec<String> = (0..8).map(|i| format!("文件{i}")).collect();
    let texts: Vec<String> = (0..8).map(|i| format!("{i} {paragraph}")).collect();
    let mut batch = Vec::new();
    for _ in 0..rounds.min(10) {
        batch.push(time(&mut || embedder.embed_documents(&titles, &texts).map(|_| ()))?);
    }
    stats("一批 8 段，每段約 250 字", batch);

    // 8 個向量的 JSON 回應，在程式裡解析的時間
    let vector: Vec<f32> = (0..768).map(|i| ((i * 7919) % 1000) as f32 / 1000.0 - 0.5).collect();
    let body = serde_json::to_string(&(0..8).map(|i| serde_json::json!({ "index": i, "embedding": [vector.clone()] })).collect::<Vec<_>>())?;
    let mut parse = Vec::new();
    for _ in 0..200 {
        parse.push(time(&mut || {
            let v: serde_json::Value = serde_json::from_str(&body)?;
            std::hint::black_box(v);
            Ok(())
        })?);
    }
    println!("8 個向量的 JSON 回應 {} KB；base64 約 {} KB", body.len() / 1024, 8 * 768 * 4 * 4 / 3 / 1024);
    stats("解析 8 個向量的 JSON", parse);
    Ok(())
}
