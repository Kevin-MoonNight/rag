//! 產生回答：從搜尋結果挑段落、組成給生成模型的提示、從回答裡找出引用了哪幾段。
//!
//! 模型本身不在這裡（個人電腦是 llama-server 子行程），這裡只有和模型無關的規則。
//! 只有文字段落交給模型；掃描頁和圖片不交（E2B 讀繁體中文頁面圖片會讀錯，2026-10-13 決定）。

use crate::text::estimate_tokens;

#[derive(Debug, Clone)]
pub struct AnswerOptions {
    /// 最多交給模型幾段。
    pub max_passages: usize,
    /// 交給模型的段落合計最多幾個 token（估計值）。
    pub passage_tokens: usize,
    /// 回答最多幾個 token。
    pub max_answer_tokens: usize,
}

impl Default for AnswerOptions {
    fn default() -> Self {
        Self { max_passages: 5, passage_tokens: 2000, max_answer_tokens: 512 }
    }
}

/// 一段候選內容：搜尋結果裡的文字段落，依搜尋名次排好。
#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    pub file_name: String,
    pub page: u32,
    pub heading: Option<String>,
    pub text: String,
}

/// 依名次挑段落，直到段數或 token 數用完。完全相同的段落（同一份內容出現在兩個檔案）只留一段。
/// 第一段一定會留，即使它本身超過 token 上限。
pub fn select_passages(candidates: Vec<Passage>, options: &AnswerOptions) -> Vec<Passage> {
    let mut out: Vec<Passage> = Vec::new();
    let mut tokens = 0usize;
    for passage in candidates {
        if out.len() >= options.max_passages {
            break;
        }
        if passage.text.trim().is_empty() || out.iter().any(|p| p.text == passage.text) {
            continue;
        }
        let cost = estimate_tokens(&passage.text);
        if !out.is_empty() && tokens + cost > options.passage_tokens {
            break;
        }
        tokens += cost;
        out.push(passage);
    }
    out
}

/// 資料裡沒有答案時，模型要回答的固定句子。
pub const NOT_FOUND: &str = "提供的資料裡找不到這個問題的答案。";

/// 系統提示。
pub fn system_prompt() -> String {
    format!(
        "你是文件查詢助理，根據使用者電腦裡的文件回答問題。\n\
         規則：\n\
         1. 只能使用「資料」裡的內容回答，不可以加入資料以外的知識或推測。\n\
         2. 每一句話的句尾用方括號標出根據的資料編號，例如 [1] 或 [2][3]。\n\
         3. 資料裡沒有答案時，只回答「{NOT_FOUND}」，不要猜。\n\
         4. 用和問題相同的語言回答；中文一律用繁體中文和臺灣用語。條號、金額、名稱照資料原文寫。\n\
         5. 先直接回答，再補充必要的條件或例外。回答要精簡：只寫和問題有關的重點，不要整段照抄，同樣的內容不要重複。"
    )
}

/// 使用者訊息：編號的資料，後面接問題。
pub fn user_prompt(question: &str, passages: &[Passage]) -> String {
    let mut s = String::from("資料：\n");
    for (i, p) in passages.iter().enumerate() {
        s.push_str(&format!("\n[{}] 檔名：{}，第 {} 頁\n", i + 1, p.file_name, p.page));
        if let Some(h) = p.heading.as_deref().filter(|h| !h.trim().is_empty() && !p.text.trim_start().starts_with(h.trim())) {
            s.push_str(&format!("章節：{}\n", h.trim()));
        }
        s.push_str(p.text.trim());
        s.push('\n');
    }
    s.push_str(&format!("\n問題：{}", question.trim()));
    s
}

/// 回答裡引用的資料編號，依第一次出現的順序，不重複。
/// 認得 [1]、[1][2]、[1, 2]、[1、2]，以及全形的［1］、【1】。超出範圍的編號放在第二個回傳值。
pub fn citations(answer: &str, passages: usize) -> (Vec<usize>, Vec<usize>) {
    let chars: Vec<char> = answer.chars().collect();
    let (mut valid, mut invalid) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < chars.len() {
        if !matches!(chars[i], '[' | '［' | '【') {
            i += 1;
            continue;
        }
        let close = |c: char| matches!(c, ']' | '］' | '】');
        let Some(end) = chars[i + 1..].iter().position(|&c| close(c) || c == '\n').map(|p| i + 1 + p) else { break };
        let inside: String = chars[i + 1..end].iter().collect();
        let parts: Vec<&str> = inside.split([',', '，', '、', ' ']).filter(|p| !p.is_empty()).collect();
        let numbers: Option<Vec<usize>> = parts.iter().map(|p| p.trim().parse().ok()).collect();
        if let (true, Some(numbers)) = (close(chars[end]) && !parts.is_empty(), numbers) {
            for n in numbers {
                let list = if (1..=passages).contains(&n) { &mut valid } else { &mut invalid };
                if !list.contains(&n) {
                    list.push(n);
                }
            }
        }
        i = end + 1;
    }
    (valid, invalid)
}

/// 回答是不是「找不到答案」：固定句子，或第一句就說找不到（模型有時會改寫成「資料裡找不到 X 的規定」）。
/// 第一句之後才說找不到的，是「部分有答案」，不算。
pub fn says_not_found(answer: &str) -> bool {
    if answer.contains("找不到這個問題的答案") {
        return true;
    }
    let first = answer.trim_start().split(['。', '\n']).next().unwrap_or("").to_lowercase();
    ["找不到", "沒有提到", "未提及", "沒有相關", "does not contain", "do not contain", "not mentioned", "no information", "cannot find", "can't find"]
        .iter()
        .any(|p| first.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(file: &str, text: &str) -> Passage {
        Passage { file_name: file.into(), page: 1, heading: None, text: text.into() }
    }

    #[test]
    fn selection_stops_at_count_and_budget() {
        let long = "字".repeat(900);
        let candidates = vec![passage("a", &long), passage("b", "第二段"), passage("c", &"文".repeat(900)), passage("d", "第四段")];
        let options = AnswerOptions { max_passages: 5, passage_tokens: 1000, max_answer_tokens: 100 };
        let chosen = select_passages(candidates, &options);
        // 第三段會超過 1000 個 token，到那裡就停（不跳過去拿後面較短的，維持名次順序）
        assert_eq!(chosen.iter().map(|p| p.file_name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);

        let options = AnswerOptions { max_passages: 1, ..options };
        assert_eq!(select_passages(vec![passage("a", "x"), passage("b", "y")], &options).len(), 1);
    }

    #[test]
    fn selection_skips_identical_text() {
        let chosen = select_passages(vec![passage("a", "同一段"), passage("b", "同一段"), passage("c", "另一段")], &AnswerOptions::default());
        assert_eq!(chosen.iter().map(|p| p.file_name.as_str()).collect::<Vec<_>>(), vec!["a", "c"]);
    }

    #[test]
    fn prompt_has_numbers_and_question() {
        let mut p = passage("授信要點.pdf", "第五條 授信額度……");
        p.heading = Some("第二章 授信".into());
        let s = user_prompt("額度上限是多少？", &[p, passage("b.pdf", "其他內容")]);
        assert!(s.contains("[1] 檔名：授信要點.pdf，第 1 頁\n章節：第二章 授信\n第五條"));
        assert!(s.contains("[2] 檔名：b.pdf"));
        assert!(s.ends_with("問題：額度上限是多少？"));
        // 段落本身就從標題開始時，不再重複標題
        let mut q = passage("c.pdf", "第二章 授信\n內文");
        q.heading = Some("第二章 授信".into());
        assert!(!user_prompt("?", &[q]).contains("章節："));
    }

    #[test]
    fn citation_forms() {
        assert_eq!(citations("甲 [1]。乙 [2][3]。丙 [1, 4]。", 4), (vec![1, 2, 3, 4], vec![]));
        assert_eq!(citations("全形［2］和【1】、[2、3]", 3), (vec![2, 1, 3], vec![]));
        assert_eq!(citations("超出 [7]，不是編號 [附件] [ ]", 3), (vec![], vec![7]));
        assert_eq!(citations("沒有括號", 3), (vec![], vec![]));
        assert_eq!(citations("沒關起來 [1", 3), (vec![], vec![]));
    }

    #[test]
    fn not_found_sentence() {
        assert!(says_not_found(NOT_FOUND));
        assert!(!says_not_found("額度是 100 萬元 [1]。"));
        assert!(says_not_found("提供的資料裡找不到公務人員退休金的計算方式 [1][2]。"));
        assert!(says_not_found("The provided documents do not contain this information."));
        assert!(!says_not_found("額度是 100 萬元 [1]。資料裡找不到例外的規定。"));
    }
}
