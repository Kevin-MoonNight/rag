//! 產生回答：拿搜尋結果裡的文字段落當資料，交給生成模型回答，並標出引用了哪幾段。

use crate::shared::lock;
use crate::{Engine, ResultItem};
use anyhow::{bail, Result};
use rag_core::answer::{citations, says_not_found, select_passages, system_prompt, user_prompt, NOT_FOUND};
use rag_core::math::{plain_math, MathStream};
use rag_core::{AnswerOptions, Passage, SearchOptions};
use rag_models::GenerationReport;
use serde::Serialize;
use std::path::Path;
use std::time::Instant;

/// 交給模型的一段資料。`number` 就是回答裡 [n] 的 n。
#[derive(Debug, Clone, Serialize)]
pub struct AnswerSource {
    pub number: usize,
    pub file_path: String,
    pub page: u32,
    /// 段落跨頁時結尾所在的頁碼。交給模型的提示只寫開頭的頁碼（改提示會讓回答整個改寫，見實驗 93）。
    pub page_end: u32,
    pub heading: Option<String>,
    pub text: String,
}

/// 回答過程中依序送出的事件：先是交給模型的資料，接著是一段一段的回答。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnswerEvent {
    Sources { sources: Vec<AnswerSource> },
    Text { text: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerOutcome {
    pub sources: Vec<AnswerSource>,
    /// 搜尋結果前幾名裡有幾筆是掃描頁或圖片：它們不交給模型（只顯示在搜尋結果裡）。
    pub skipped_images: usize,
    /// 搜尋花的時間（毫秒）；用現成的搜尋結果回答時是 0。
    pub search_ms: f64,
    pub generation: GenerationReport,
    /// 回答引用的資料編號（依出現順序），和超出範圍的編號。
    pub cited: Vec<usize>,
    pub invalid_citations: Vec<usize>,
    /// 回答是「找不到答案」。
    pub not_found: bool,
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string())
}

impl Engine {
    /// 這台電腦能不能產生回答；不能時回傳原因。
    pub fn answer_availability(&self) -> std::result::Result<(), String> {
        if self.generator.is_none() {
            return Err("沒有安裝產生回答用的模型".into());
        }
        if let Some((total, _)) = rag_models::system::memory() {
            if total < self.config.min_memory_for_answers {
                return Err(format!(
                    "這台電腦的記憶體是 {:.1} GB，產生回答需要 8 GB 以上的電腦",
                    total as f64 / (1u64 << 30) as f64
                ));
            }
        }
        Ok(())
    }

    /// 搜尋，再用結果回答。
    pub fn answer(
        &self,
        question: &str,
        search: &SearchOptions,
        options: &AnswerOptions,
        on_event: &mut dyn FnMut(AnswerEvent) -> bool,
    ) -> Result<AnswerOutcome> {
        let t = Instant::now();
        let results = self.search(question, search)?;
        let search_ms = t.elapsed().as_secs_f64() * 1000.0;
        let mut outcome = self.answer_from_results(question, &results, options, on_event)?;
        outcome.search_ms = search_ms;
        Ok(outcome)
    }

    /// 用已經有的搜尋結果回答（畫面上先顯示搜尋結果，使用者再要求回答時用這個，不必再搜一次）。
    pub fn answer_from_results(
        &self,
        question: &str,
        results: &[ResultItem],
        options: &AnswerOptions,
        on_event: &mut dyn FnMut(AnswerEvent) -> bool,
    ) -> Result<AnswerOutcome> {
        if let Err(reason) = self.answer_availability() {
            bail!("{reason}");
        }
        let candidates: Vec<(Passage, &ResultItem)> = results
            .iter()
            .filter_map(|r| {
                let text = r.text.as_ref().filter(|_| !r.is_image)?;
                Some((Passage { file_name: file_name(&r.file_path), page: r.page, heading: r.heading.clone(), text: text.clone() }, r))
            })
            .collect();
        let chosen = select_passages(candidates.iter().map(|(p, _)| p.clone()).collect(), options);
        let sources: Vec<AnswerSource> = chosen
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // 找回這一段原本的完整路徑（select_passages 只保留檔名）
                let item = candidates.iter().find(|(c, _)| c == p).map(|(_, r)| *r);
                AnswerSource {
                    number: i + 1,
                    file_path: item.map(|r| r.file_path.clone()).unwrap_or_else(|| p.file_name.clone()),
                    page: p.page,
                    page_end: item.map_or(p.page, |r| r.page_end),
                    heading: p.heading.clone(),
                    text: p.text.clone(),
                }
            })
            .collect();
        // 交給模型的段落排在第幾名之前，就算這個範圍裡的掃描頁和圖片
        let reach = sources
            .last()
            .and_then(|last| results.iter().position(|r| r.file_path == last.file_path && r.page == last.page && r.text.as_deref() == Some(&last.text)))
            .map_or(results.len().min(options.max_passages), |p| p + 1);
        let skipped_images = results[..reach].iter().filter(|r| r.is_image).count();

        on_event(AnswerEvent::Sources { sources: sources.clone() });
        let mut outcome = AnswerOutcome {
            sources,
            skipped_images,
            search_ms: 0.0,
            generation: GenerationReport::default(),
            cited: Vec::new(),
            invalid_citations: Vec::new(),
            not_found: false,
        };
        if chosen.is_empty() {
            // 沒有文字段落可以交給模型（例如只找到掃描頁）：不載入模型，直接回答找不到
            on_event(AnswerEvent::Text { text: NOT_FOUND.to_string() });
            outcome.generation.text = NOT_FOUND.to_string();
            outcome.not_found = true;
            return Ok(outcome);
        }

        let generator = self.generator.as_ref().expect("answer_availability 已確認有生成模型");
        let mut generator = lock(generator);
        // 模型寫的 LaTeX 數學式（$\text{CO}_2$）邊收邊換成一般文字（CO₂），介面不渲染數學式（實驗 93）
        let mut math = MathStream::default();
        let mut report = generator.chat(&system_prompt(), &user_prompt(question, &chosen), options.max_answer_tokens, &mut |text| {
            let text = math.push(text);
            text.is_empty() || on_event(AnswerEvent::Text { text })
        })?;
        let rest = math.finish();
        if !rest.is_empty() {
            on_event(AnswerEvent::Text { text: rest });
        }
        report.text = plain_math(&report.text);
        let (cited, invalid) = citations(&report.text, chosen.len());
        outcome.cited = cited;
        outcome.invalid_citations = invalid;
        outcome.not_found = says_not_found(&report.text);
        outcome.generation = report;
        Ok(outcome)
    }

    /// 先把生成模型載入（量載入時間用）。
    pub fn load_generator(&self) -> Result<()> {
        match &self.generator {
            Some(g) => lock(g).load(),
            None => bail!("沒有設定生成模型"),
        }
    }
}
