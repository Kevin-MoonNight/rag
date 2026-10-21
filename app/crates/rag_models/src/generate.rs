//! 產生回答：生成模型（Gemma 4 E2B）的 llama-server 子行程，用相容 OpenAI 的聊天端點串流取回答。
//!
//! 和向量模型一樣，用到時才啟動、閒置就結束。模型 1.4 GB，只在使用者要求回答時載入。

use crate::{OnDemand, ServerConfig};
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// 一次回答的結果和耗時。
#[derive(Debug, Clone, Default, Serialize)]
pub struct GenerationReport {
    pub text: String,
    /// 這次要不要先啟動子行程、花了多久（毫秒）。
    pub load_ms: f64,
    /// 從送出請求到收到第一段文字（毫秒），含讀入提示。
    pub first_text_ms: f64,
    /// 從送出請求到結束（毫秒）。
    pub total_ms: f64,
    /// llama-server 回報的數字：提示幾個 token、讀入花幾毫秒、回答幾個 token、產生花幾毫秒。
    pub prompt_tokens: Option<u64>,
    /// 提示開頭和上一次請求相同、直接沿用 KV 而沒有重算的 token 數（系統提示固定，連續問問題時可以沿用）。
    pub cached_tokens: Option<u64>,
    pub prompt_ms: Option<f64>,
    pub answer_tokens: Option<u64>,
    pub answer_ms: Option<f64>,
    /// stop（自然結束）、length（到達長度上限）；中途取消時是 None。
    pub finish_reason: Option<String>,
    pub cancelled: bool,
}

pub struct LlamaGenerator {
    process: OnDemand,
}

impl LlamaGenerator {
    pub fn new(config: ServerConfig) -> Self {
        Self { process: OnDemand::new(config) }
    }

    pub fn is_loaded(&self) -> bool {
        self.process.is_loaded()
    }

    pub fn pid(&self) -> Option<u32> {
        self.process.pid()
    }

    pub fn load(&mut self) -> Result<()> {
        self.process.get().map(|_| ())
    }

    pub fn unload(&mut self) {
        self.process.unload();
    }

    pub fn unload_if_idle(&mut self, idle: Duration) -> bool {
        self.process.unload_if_idle(idle)
    }

    /// 送出系統提示和使用者訊息，回答一段一段交給 `on_text`。`on_text` 回傳 `false` 就停止產生。
    /// 溫度設 0：同樣的問題和資料得到同樣的回答（可以重現，也方便評測）。
    pub fn chat(&mut self, system: &str, user: &str, max_tokens: usize, on_text: &mut dyn FnMut(&str) -> bool) -> Result<GenerationReport> {
        let mut report = GenerationReport::default();
        if !self.process.is_loaded() {
            let t = Instant::now();
            self.process.get()?;
            report.load_ms = t.elapsed().as_secs_f64() * 1000.0;
        }
        let payload = json!({
            "messages": [{ "role": "system", "content": system }, { "role": "user", "content": user }],
            "stream": true,
            "max_tokens": max_tokens,
            "temperature": 0.0,
        });
        let started = Instant::now();
        let mut first: Option<f64> = None;
        let mut cancelled = false;
        let result = self.process.get()?.post_stream("/v1/chat/completions", &payload, &mut |event| {
            if let Some(timings) = event.get("timings") {
                report.prompt_tokens = timings.get("prompt_n").and_then(Value::as_u64);
                report.cached_tokens = timings.get("cache_n").and_then(Value::as_u64);
                report.prompt_ms = timings.get("prompt_ms").and_then(Value::as_f64);
                report.answer_tokens = timings.get("predicted_n").and_then(Value::as_u64);
                report.answer_ms = timings.get("predicted_ms").and_then(Value::as_f64);
            }
            let Some(choice) = event.get("choices").and_then(|c| c.get(0)) else { return true };
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                report.finish_reason = Some(reason.to_string());
            }
            match choice.get("delta").and_then(|d| d.get("content")).and_then(Value::as_str) {
                Some(text) if !text.is_empty() => {
                    first.get_or_insert_with(|| started.elapsed().as_secs_f64() * 1000.0);
                    report.text.push_str(text);
                    if !on_text(text) {
                        cancelled = true;
                        return false;
                    }
                    true
                }
                _ => true,
            }
        });
        self.process.touch();
        if let Err(e) = result {
            self.process.forget_if_dead();
            return Err(e);
        }
        report.total_ms = started.elapsed().as_secs_f64() * 1000.0;
        report.first_text_ms = first.unwrap_or(report.total_ms);
        report.cancelled = cancelled;
        if !cancelled && report.finish_reason.is_none() && report.text.is_empty() {
            bail!("生成模型沒有回傳任何內容");
        }
        Ok(report)
    }
}
