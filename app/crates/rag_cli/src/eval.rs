//! 評測：一行一題的 JSONL，每題有問題和正確答案所在的檔案（可選頁碼）。
//!
//! ```json
//! {"q": "問題", "file": "檔名的一部分", "page": 3, "tag": "同義改寫"}
//! ```
//!
//! `file` 可以是字串或字串陣列（多個檔案都算對）。分別統計文字向量、圖片向量、關鍵字三份名單
//! 和合併後的名單，看正確的檔案（以及正確的頁）排在第幾名。
//!
//! `--pages <頁標註.jsonl>`：另外標出每題答案在哪幾頁（一題可以有好幾頁），算頁的命中率和 nDCG@10。
//! 一行一題，`n` 是題目檔裡的第幾題（從 0 算），每頁是［資料夾或檔名裡的代號, 檔名的一部分, 頁碼］：
//!
//! ```json
//! {"n": 0, "pages": [["FL005713", "條文對照表", 1], ["FL005713", "總說明", 1]]}
//! ```
//!
//! 有頁標註的題目用標註算頁，沒有的才看題目檔的 `page`。

use crate::args::Args;
use crate::{engine_config, hold, search_options};
use anyhow::{Context, Result};
use rag_core::Hit;
use rag_engine::Engine;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Instant;

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
    page: Option<u32>,
    #[serde(default)]
    tag: String,
}

/// 一份名單在一組題目上的統計。
#[derive(Default, Clone)]
struct Tally {
    n: usize,
    file_at: [usize; 3], // 前 1、5、10 名內有正確的檔案
    page_n: usize,
    page_at: [usize; 3],
    reciprocal: f64,
    page_reciprocal: f64,
    /// 有頁標註的題目數，和它們的 nDCG@10 合計。
    labeled_n: usize,
    ndcg: f64,
}

/// 一個標註的答案頁。
#[derive(Deserialize)]
struct PageLabel(String, String, u32);

#[derive(Deserialize)]
struct PageLabels {
    n: usize,
    pages: Vec<PageLabel>,
}

impl PageLabel {
    /// 段落跨頁時（`page`..=`page_end`），標註的頁落在範圍裡就算對：答案就在這段文字裡。
    fn matches(&self, file_path: &str, page: u32, page_end: u32) -> bool {
        let name = file_path.rsplit('/').next().unwrap_or(file_path);
        (page..=page_end.max(page)).contains(&self.2) && file_path.contains(&self.0) && name.contains(&self.1)
    }
}

/// 名單的 nDCG@10：每個標註頁只算第一次出現（同一頁的其他段落不重複給分）。
fn ndcg_at_10(matched: &[Option<usize>], labels: usize) -> f64 {
    let mut seen = Vec::new();
    let mut dcg = 0.0;
    for (n, m) in matched.iter().take(10).enumerate() {
        if let Some(i) = m {
            if !seen.contains(i) {
                seen.push(*i);
                dcg += 1.0 / ((n + 2) as f64).log2();
            }
        }
    }
    let ideal: f64 = (0..labels.min(10)).map(|n| 1.0 / ((n + 2) as f64).log2()).sum();
    if ideal == 0.0 { 0.0 } else { dcg / ideal }
}

const CUTS: [usize; 3] = [1, 5, 10];

impl Tally {
    fn add(&mut self, file_rank: Option<usize>, page_rank: Option<Option<usize>>) {
        self.n += 1;
        if let Some(r) = file_rank {
            self.reciprocal += 1.0 / r as f64;
            for (i, cut) in CUTS.iter().enumerate() {
                if r <= *cut {
                    self.file_at[i] += 1;
                }
            }
        }
        if let Some(page_rank) = page_rank {
            self.page_n += 1;
            if let Some(r) = page_rank {
                self.page_reciprocal += 1.0 / r as f64;
                for (i, cut) in CUTS.iter().enumerate() {
                    if r <= *cut {
                        self.page_at[i] += 1;
                    }
                }
            }
        }
    }

    fn line(&self) -> String {
        let pct = |a: usize, b: usize| if b == 0 { "   -  ".to_string() } else { format!("{:5.1}%", 100.0 * a as f64 / b as f64) };
        let avg = |sum: f64, n: usize| if n == 0 { 0.0 } else { sum / n as f64 };
        let mut s = format!(
            "檔案 前1 {} 前5 {} 前10 {}  MRR {:.3} | 頁 前1 {} 前5 {} 前10 {}  MRR {:.3}",
            pct(self.file_at[0], self.n),
            pct(self.file_at[1], self.n),
            pct(self.file_at[2], self.n),
            avg(self.reciprocal, self.n),
            pct(self.page_at[0], self.page_n),
            pct(self.page_at[1], self.page_n),
            pct(self.page_at[2], self.page_n),
            avg(self.page_reciprocal, self.page_n),
        );
        if self.labeled_n > 0 {
            s.push_str(&format!("  nDCG@10 {:.3}", avg(self.ndcg, self.labeled_n)));
        }
        s + &format!("  (n={}, 有頁碼 {})", self.n, self.page_n)
    }
}

pub fn run(args: &mut Args) -> Result<()> {
    let config = engine_config(args)?;
    let options = search_options(args)?;
    let details = args.value("--out");
    // 細節檔裡另外附上三份原始名單（離線比較合併規則用）
    let dump_lists = args.flag("--lists");
    let mut page_labels: BTreeMap<usize, Vec<PageLabel>> = BTreeMap::new();
    if let Some(labels) = args.value("--pages") {
        let text = std::fs::read_to_string(&labels).with_context(|| format!("讀不到 {labels}"))?;
        for (i, l) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let entry: PageLabels = serde_json::from_str(l).with_context(|| format!("頁標註第 {} 行格式不對", i + 1))?;
            page_labels.insert(entry.n, entry.pages);
        }
    }
    let path = args.positional(1).context("請指定題目檔")?;
    let text = std::fs::read_to_string(&path).with_context(|| format!("讀不到 {path}"))?;
    let questions: Vec<Question> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| serde_json::from_str(l).with_context(|| format!("題目檔第 {} 行格式不對", i + 1)))
        .collect::<Result<_>>()?;

    let engine = Engine::open(config)?;
    let t = Instant::now();
    engine.warm_up()?;
    println!("載入模型 {:.2} 秒，題目 {} 題", t.elapsed().as_secs_f64(), questions.len());

    const LISTS: [&str; 4] = ["文字向量", "圖片向量", "關鍵字", "合併"];
    let mut overall = vec![Tally::default(); 4];
    let mut by_tag: BTreeMap<String, Vec<Tally>> = BTreeMap::new();
    let mut times: Vec<f64> = Vec::new();
    let mut detail_lines: Vec<String> = Vec::new();

    for (qi, question) in questions.iter().enumerate() {
        let labels = page_labels.get(&qi).filter(|l| !l.is_empty());
        let golds: Vec<&str> = match &question.file {
            OneOrMany::One(s) => vec![s.as_str()],
            OneOrMany::Many(v) => v.iter().map(String::as_str).collect(),
        };
        let t = Instant::now();
        let raw = engine.search_raw(&question.q, &options)?;
        times.push(t.elapsed().as_secs_f64() * 1000.0);

        let merged: Vec<Hit> = raw.merged.iter().map(|m| Hit { id: m.id, score: m.score }).collect();
        let lists = [&raw.text, &raw.image, &raw.keyword, &merged];
        let mut ranks = Vec::new();
        let mut ndcgs = Vec::new();
        let mut dumped: Vec<Vec<serde_json::Value>> = Vec::new();
        for (i, list) in lists.iter().enumerate() {
            let mut file_rank = None;
            let mut page_rank = None;
            // 每個名次對到第幾個標註頁（沒有頁標註就都是 None）
            let mut matched: Vec<Option<usize>> = Vec::new();
            let mut dump = Vec::new();
            for (n, hit) in list.iter().enumerate() {
                let Some(chunk) = engine.store().chunk(hit.id)? else {
                    matched.push(None);
                    dump.push(serde_json::json!([hit.id, hit.score, -1, 0, 0]));
                    continue;
                };
                // 路徑分隔符號統一成 /，題目檔在兩種系統都能用
                let file_path = chunk.file_path.replace('\\', "/");
                let label = labels.and_then(|l| l.iter().position(|p| p.matches(&file_path, chunk.page, chunk.page_end)));
                matched.push(label);
                if golds.iter().any(|g| file_path.contains(g)) {
                    file_rank.get_or_insert(n + 1);
                }
                let page_hit = match labels {
                    Some(_) => label.is_some(),
                    None => question.page.is_some_and(|p| (chunk.page..=chunk.page_end.max(chunk.page)).contains(&p)) && golds.iter().any(|g| file_path.contains(g)),
                };
                if page_hit {
                    page_rank.get_or_insert(n + 1);
                }
                let file_hit = golds.iter().any(|g| file_path.contains(g));
                dump.push(serde_json::json!([hit.id, hit.score, label.map_or(-1, |l| l as i64), page_hit as u8, file_hit as u8]));
            }
            if i < 3 {
                dumped.push(dump);
            }
            let page_stat = (labels.is_some() || question.page.is_some()).then_some(page_rank);
            let ndcg = labels.map(|l| ndcg_at_10(&matched, l.len()));
            let tag_tally = &mut by_tag.entry(question.tag.clone()).or_insert_with(|| vec![Tally::default(); 4])[i];
            for tally in [&mut overall[i], tag_tally] {
                tally.add(file_rank, page_stat);
                if let Some(v) = ndcg {
                    tally.labeled_n += 1;
                    tally.ndcg += v;
                }
            }
            ranks.push((file_rank, page_rank));
            ndcgs.push(ndcg);
        }
        let mut detail = serde_json::json!({
            "q": question.q, "tag": question.tag, "gold": golds, "page": question.page,
            "text": ranks[0], "image": ranks[1], "keyword": ranks[2], "merged": ranks[3],
            "ndcg": ndcgs,
        });
        if dump_lists {
            // 每筆是［段落編號, 分數, 對到第幾個標註頁（-1 表示沒有）, 是不是答案頁, 是不是正確的檔案］
            detail["lists"] = serde_json::json!({"text": dumped[0], "image": dumped[1], "keyword": dumped[2]});
        }
        detail_lines.push(detail.to_string());
    }

    println!("\n== 全部 {} 題", questions.len());
    for (i, name) in LISTS.iter().enumerate() {
        println!("{name:　<4} {}", overall[i].line());
    }
    if by_tag.len() > 1 {
        for (tag, tallies) in &by_tag {
            println!("\n== 類別：{}", if tag.is_empty() { "（未分類）" } else { tag });
            for (i, name) in LISTS.iter().enumerate() {
                println!("{name:　<4} {}", tallies[i].line());
            }
        }
    }
    times.sort_by(|a, b| a.total_cmp(b));
    if !times.is_empty() {
        let at = |p: f64| times[((times.len() - 1) as f64 * p).round() as usize];
        println!("\n每題搜尋時間（毫秒）：中位數 {:.0}、90% {:.0}、最慢 {:.0}", at(0.5), at(0.9), at(1.0));
    }
    if let Some(out) = details {
        std::fs::write(&out, detail_lines.join("\n") + "\n").with_context(|| format!("無法寫入 {out}"))?;
    }
    hold(args, &engine)?;
    args.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndcg_counts_each_label_once() {
        // 兩個標註頁都在第 1、2 名：滿分
        assert!((ndcg_at_10(&[Some(0), Some(1), None], 2) - 1.0).abs() < 1e-9);
        // 同一頁出現兩次，第二次不給分
        let twice = ndcg_at_10(&[Some(0), Some(0), Some(1)], 2);
        let ideal = 1.0 + 1.0 / 3f64.log2();
        assert!((twice - (1.0 + 0.5) / ideal).abs() < 1e-9);
        assert_eq!(ndcg_at_10(&[None, None], 3), 0.0);
    }

    #[test]
    fn label_matches_folder_name_and_page() {
        let label = PageLabel("FL005713".into(), "條文對照表".into(), 1);
        assert!(label.matches("/rag/pdf/FL005713/準則 條文對照表（107.11.08）.pdf", 1, 1));
        assert!(!label.matches("/rag/pdf/FL005713/準則 總說明（107.11.08）.pdf", 1, 1));
        assert!(!label.matches("/rag/pdf/FL005713/準則 條文對照表（107.11.08）.pdf", 2, 2));
        assert!(!label.matches("/rag/pdf/FL000001/條文對照表.pdf", 1, 1));
        // 跨頁的段落（第 2 到 3 頁）也算對到第 3 頁的標註；第 1 頁的標註不算
        assert!(PageLabel("FL005713".into(), "條文對照表".into(), 3).matches("/rag/pdf/FL005713/準則 條文對照表（107.11.08）.pdf", 2, 3));
        assert!(!label.matches("/rag/pdf/FL005713/準則 條文對照表（107.11.08）.pdf", 2, 3));
    }
}
