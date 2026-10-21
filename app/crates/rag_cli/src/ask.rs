//! 產生回答：`rag ask <問題>` 回答一題；`rag ask-eval <題目檔>` 用評測題目跑一遍，統計出處和耗時。
//!
//! 題目檔和 `rag eval` 相同（一行一題的 JSONL）。`file` 是空陣列的題目表示文件裡沒有答案，
//! 模型應該回答找不到。

use crate::args::Args;
use crate::{engine_config, hold, search_options};
use anyhow::{Context, Result};
use rag_engine::{AnswerEvent, AnswerOptions, Engine};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::time::Instant;

fn answer_options(args: &mut Args) -> Result<AnswerOptions> {
    let mut o = AnswerOptions::default();
    if let Some(v) = args.number("--passages")? {
        o.max_passages = v;
    }
    if let Some(v) = args.number("--passage-tokens")? {
        o.passage_tokens = v;
    }
    if let Some(v) = args.number("--max-answer")? {
        o.max_answer_tokens = v;
    }
    Ok(o)
}

pub fn ask(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    let search = search_options(args)?;
    let options = answer_options(args)?;
    let question = args.positional(1).context("請指定問題")?;
    let engine = Engine::open(config)?;

    let started = Instant::now();
    let outcome = engine.answer(&question, &search, &options, &mut |event| {
        match event {
            AnswerEvent::Sources { sources } => {
                println!("交給模型的資料：");
                for s in &sources {
                    println!("  [{}] {} 第 {} 頁", s.number, s.file_path, crate::pages(s.page, s.page_end));
                }
                println!("\n回答：");
            }
            AnswerEvent::Text { text } => {
                print!("{text}");
                let _ = std::io::stdout().flush();
            }
        }
        true
    })?;
    let g = &outcome.generation;
    println!("\n");
    if outcome.skipped_images > 0 {
        println!("（搜尋結果前幾名裡有 {} 筆掃描頁或圖片，沒有交給模型）", outcome.skipped_images);
    }
    println!("引用 {:?}，超出範圍 {:?}，找不到={}，結束原因 {:?}", outcome.cited, outcome.invalid_citations, outcome.not_found, g.finish_reason);
    println!(
        "搜尋 {:.0} ms，載入模型 {:.1} 秒，第一個字 {:.1} 秒，回答完 {:.1} 秒，全部 {:.1} 秒",
        outcome.search_ms,
        g.load_ms / 1000.0,
        g.first_text_ms / 1000.0,
        g.total_ms / 1000.0,
        started.elapsed().as_secs_f64()
    );
    if let (Some(pn), Some(pms), Some(an), Some(ams)) = (g.prompt_tokens, g.prompt_ms, g.answer_tokens, g.answer_ms) {
        println!("提示 {pn} 個 token（每秒 {:.0}），回答 {an} 個 token（每秒 {:.1}）", pn as f64 / pms * 1000.0, an as f64 / ams * 1000.0);
    }
    hold(args, &engine)?;
    args.finish()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Question {
    q: String,
    file: OneOrMany,
    #[serde(default)]
    tag: String,
}

#[derive(Default)]
struct Tally {
    n: usize,
    /// 有答案的題目：正確的檔案在交給模型的資料裡、回答引用了正確的檔案。
    answerable: usize,
    gold_in_sources: usize,
    gold_cited: usize,
    /// 回答「找不到」的題數（有答案的題目裡、沒有答案的題目裡）。
    not_found_answerable: usize,
    unanswerable: usize,
    not_found_unanswerable: usize,
    /// 沒有回答找不到、卻一個出處都沒標的題數。
    no_citation: usize,
    invalid_citation: usize,
    truncated: usize,
}

impl Tally {
    fn line(&self) -> String {
        let pct = |a: usize, b: usize| if b == 0 { "-".to_string() } else { format!("{:.1}%（{a}/{b}）", 100.0 * a as f64 / b as f64) };
        format!(
            "題數 {}｜有答案的題目：正確檔案在資料裡 {}，回答引用正確檔案 {}，回答找不到 {}｜沒有答案的題目：回答找不到 {}｜沒標出處 {}，出處編號超出範圍 {}，被長度上限截斷 {}",
            self.n,
            pct(self.gold_in_sources, self.answerable),
            pct(self.gold_cited, self.answerable),
            pct(self.not_found_answerable, self.answerable),
            pct(self.not_found_unanswerable, self.unanswerable),
            self.no_citation,
            self.invalid_citation,
            self.truncated
        )
    }
}

pub fn eval(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    let search = search_options(args)?;
    let options = answer_options(args)?;
    let out = args.value("--out").context("請用 --out 指定逐題結果的檔案")?;
    let limit = args.number("--limit")?;
    let path = args.positional(1).context("請指定題目檔")?;
    let text = std::fs::read_to_string(&path).with_context(|| format!("讀不到 {path}"))?;
    let mut questions: Vec<Question> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| serde_json::from_str(l).with_context(|| format!("題目檔第 {} 行格式不對", i + 1)))
        .collect::<Result<_>>()?;
    if let Some(n) = limit {
        questions.truncate(n);
    }

    let engine = Engine::open(config)?;
    let t = Instant::now();
    engine.warm_up()?;
    let embed_load = t.elapsed().as_secs_f64();
    let t = Instant::now();
    engine.load_generator()?;
    println!("載入向量模型 {embed_load:.2} 秒，生成模型 {:.2} 秒，題目 {} 題", t.elapsed().as_secs_f64(), questions.len());

    let mut file = std::fs::File::create(&out).with_context(|| format!("無法建立 {out}"))?;
    let mut overall = Tally::default();
    let mut by_tag: BTreeMap<String, Tally> = BTreeMap::new();
    let (mut first, mut total, mut prompt_rate, mut answer_rate) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());

    for (n, question) in questions.iter().enumerate() {
        let golds: Vec<&str> = match &question.file {
            OneOrMany::One(s) => vec![s.as_str()],
            OneOrMany::Many(v) => v.iter().map(String::as_str).collect(),
        };
        let outcome = engine.answer(&question.q, &search, &options, &mut |_| true)?;
        let g = &outcome.generation;
        let is_gold = |path: &str| {
            let path = path.replace('\\', "/");
            golds.iter().any(|gold| path.contains(gold))
        };
        let in_sources = outcome.sources.iter().any(|s| is_gold(&s.file_path));
        let cited_gold = outcome.cited.iter().any(|&c| outcome.sources.get(c - 1).is_some_and(|s| is_gold(&s.file_path)));

        for tally in [&mut overall, by_tag.entry(question.tag.clone()).or_default()] {
            tally.n += 1;
            if golds.is_empty() {
                tally.unanswerable += 1;
                tally.not_found_unanswerable += outcome.not_found as usize;
            } else {
                tally.answerable += 1;
                tally.gold_in_sources += in_sources as usize;
                tally.gold_cited += cited_gold as usize;
                tally.not_found_answerable += outcome.not_found as usize;
            }
            tally.no_citation += (!outcome.not_found && outcome.cited.is_empty()) as usize;
            tally.invalid_citation += (!outcome.invalid_citations.is_empty()) as usize;
            tally.truncated += (g.finish_reason.as_deref() == Some("length")) as usize;
        }
        if !outcome.sources.is_empty() {
            first.push(g.first_text_ms / 1000.0);
            total.push(g.total_ms / 1000.0);
        }
        if let (Some(pn), Some(pms)) = (g.prompt_tokens, g.prompt_ms) {
            prompt_rate.push(pn as f64 / pms * 1000.0);
        }
        if let (Some(an), Some(ams)) = (g.answer_tokens, g.answer_ms) {
            if an > 0 {
                answer_rate.push(an as f64 / ams * 1000.0);
            }
        }
        let record = serde_json::json!({
            "q": question.q, "tag": question.tag, "gold": golds,
            "gold_in_sources": in_sources, "gold_cited": cited_gold,
            "sources": outcome.sources.iter().map(|s| serde_json::json!({"n": s.number, "file": s.file_path, "page": s.page})).collect::<Vec<_>>(),
            "skipped_images": outcome.skipped_images,
            "answer": g.text, "cited": outcome.cited, "invalid": outcome.invalid_citations, "not_found": outcome.not_found,
            "finish": g.finish_reason, "search_ms": outcome.search_ms, "first_s": g.first_text_ms / 1000.0, "total_s": g.total_ms / 1000.0,
            "prompt_tokens": g.prompt_tokens, "cached_tokens": g.cached_tokens, "answer_tokens": g.answer_tokens,
        });
        writeln!(file, "{record}")?;
        file.flush()?;
        println!("[{}/{}] {:.1}s 資料裡有正確檔案={} 引用正確檔案={} 找不到={}  {}", n + 1, questions.len(), g.total_ms / 1000.0, in_sources, cited_gold, outcome.not_found, question.q);
    }

    println!("\n== 全部\n{}", overall.line());
    for (tag, tally) in &by_tag {
        println!("\n== 類別：{}\n{}", if tag.is_empty() { "（未分類）" } else { tag }, tally.line());
    }
    let summary = |name: &str, v: &mut Vec<f64>, unit: &str| {
        if v.is_empty() {
            return;
        }
        v.sort_by(|a, b| a.total_cmp(b));
        let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        println!("{name}：中位數 {:.1}{unit}、90% {:.1}{unit}、最小 {:.1}{unit}、最大 {:.1}{unit}", at(0.5), at(0.9), at(0.0), at(1.0));
    };
    println!();
    summary("出現第一個字（秒）", &mut first, "");
    summary("回答完（秒）", &mut total, "");
    summary("讀入提示（token/s）", &mut prompt_rate, "");
    summary("產生回答（token/s）", &mut answer_rate, "");
    hold(args, &engine)?;
    args.finish()
}
