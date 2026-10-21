//! 切分：把中間格式的文字區塊切成 chunk。
//!
//! 做法是遞迴切分：先依段落，再依句子，句子還是太長才硬切。標題出現時可以強制開新的 chunk，
//! 並把標題放在 chunk 的開頭（向量主要代表開頭的內容，見實驗 44）。

use crate::text::estimate_tokens;
use crate::types::{Block, BlockKind};

#[derive(Debug, Clone)]
pub struct ChunkOptions {
    /// 一個 chunk 最多幾個 token（用 [`estimate_tokens`] 粗估）。
    pub max_tokens: usize,
    /// 相鄰 chunk 重疊的 token 數上限。重疊以句子為單位，不會把句子切開。
    pub overlap_tokens: usize,
    /// 遇到標題就開新的 chunk。
    pub split_at_headings: bool,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self { max_tokens: 256, overlap_tokens: 32, split_at_headings: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    /// 這份文件裡的第幾個 chunk（0 起算）。
    pub index: u32,
    /// chunk 開頭所在的頁碼。
    pub page: u32,
    /// chunk 結尾所在的頁碼（跨頁的段落比 `page` 大）。
    pub page_end: u32,
    /// chunk 所屬的最近一個標題。
    pub heading: Option<String>,
    pub text: String,
}

/// 內文少於這個 token 數就不單獨成一個 chunk，併到相鄰的 chunk。
/// PDF 抽出來的標題常被拆成好幾行，或是標題前後只剩一兩個字，單獨成塊只會產生沒有內容的搜尋結果。
const MIN_BODY_TOKENS: usize = 16;

struct Piece {
    text: String,
    tokens: usize,
    page: u32,
    heading: bool,
}

/// 把跨行的標題接回一行：中文直接相接，其他的中間補一個空白。
fn join_heading(a: &str, b: &str) -> String {
    let wide = |c: Option<char>| c.is_some_and(|c| !c.is_ascii());
    if wide(a.chars().next_back()) && wide(b.chars().next()) {
        format!("{a}{b}")
    } else {
        format!("{a} {b}")
    }
}

fn is_sentence_end(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '；' | '!' | '?' | ';')
}

/// 把一段文字切成不超過 `max_tokens` 的小段：一段一段、一句一句，太長的句子硬切。
fn split_pieces(text: &str, page: u32, heading: bool, max_tokens: usize, out: &mut Vec<Piece>) {
    for para in text.split('\n') {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        let first = out.len();
        let chars: Vec<char> = para.chars().collect();
        let mut cur = String::new();
        for (i, &c) in chars.iter().enumerate() {
            cur.push(c);
            // 英文句點後面接空白才算句子結束，避免把小數和縮寫切開
            let end = is_sentence_end(c) || (c == '.' && chars.get(i + 1).is_some_and(|n| n.is_whitespace()));
            if end || estimate_tokens(&cur) >= max_tokens {
                out.push(Piece { tokens: estimate_tokens(&cur), text: std::mem::take(&mut cur), page, heading });
            }
        }
        if !cur.trim().is_empty() {
            out.push(Piece { tokens: estimate_tokens(&cur), text: cur, page, heading });
        }
        if out.len() > first {
            out.last_mut().unwrap().text.push('\n');
        }
    }
}

pub fn chunk_blocks(blocks: &[Block], opt: &ChunkOptions) -> Vec<Chunk> {
    let max_tokens = opt.max_tokens.max(8);
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut buf: Vec<Piece> = Vec::new();
    let mut heading: Option<String> = None;
    // buf 開頭有幾段是從上一個 chunk 帶過來的重疊；buf 只剩重疊時不輸出
    let mut carried = 0usize;

    fn flush(buf: &mut Vec<Piece>, carried: &mut usize, heading: &Option<String>, chunks: &mut Vec<Chunk>, overlap: usize) {
        if buf.len() > *carried {
            let text: String = buf.iter().map(|p| p.text.as_str()).collect();
            let page_end = buf.iter().map(|p| p.page).max().unwrap_or(buf[0].page);
            chunks.push(Chunk { index: chunks.len() as u32, page: buf[0].page, page_end, heading: heading.clone(), text: text.trim().to_string() });
        }
        // 留下尾端不超過 overlap 的幾段，當作下一個 chunk 的開頭
        let mut keep = 0usize;
        let mut tokens = 0usize;
        while keep < buf.len().saturating_sub(1) && tokens + buf[buf.len() - 1 - keep].tokens <= overlap {
            tokens += buf[buf.len() - 1 - keep].tokens;
            keep += 1;
        }
        let tail = buf.split_off(buf.len() - keep);
        *buf = tail;
        *carried = buf.len();
    }

    /// 把 buf 裡還沒輸出的部分接到上一個 chunk 的後面。
    fn append_to_last(buf: &mut Vec<Piece>, carried: &mut usize, chunks: &mut [Chunk]) {
        let text: String = buf[*carried..].iter().map(|p| p.text.as_str()).collect();
        let last = chunks.last_mut().expect("append_to_last needs a previous chunk");
        last.text.push('\n');
        last.text.push_str(text.trim());
        buf.clear();
        *carried = 0;
    }

    // 上一個 chunk 是因為長度滿了才輸出的：buf 裡的內容是同一段的後續
    let mut continues = false;
    // 上一個文字區塊也是標題：兩個標題其實是同一個標題被拆成兩行
    let mut heading_run = false;

    for block in blocks.iter().filter(|b| b.kind == BlockKind::Text) {
        let is_heading = block.heading_level.is_some();
        if is_heading {
            let title = block.text.trim();
            if opt.split_at_headings {
                let body: usize = buf[carried..].iter().filter(|p| !p.heading).map(|p| p.tokens).sum();
                if body >= MIN_BODY_TOKENS {
                    flush(&mut buf, &mut carried, &heading, &mut chunks, 0);
                } else if continues && buf.len() > carried {
                    // 長段落剩下的一小段尾巴：接回上一個 chunk，不要擋在新標題前面
                    append_to_last(&mut buf, &mut carried, &mut chunks);
                }
                // 其他情況（只有標題，或標題加一兩個字）留在 buf 裡，和這個標題合成同一個 chunk 的開頭
                continues = false;
            }
            heading = Some(match (&heading, heading_run) {
                (Some(prev), true) => join_heading(prev, title),
                _ => title.to_string(),
            });
        }
        heading_run = is_heading;
        let mut pieces = Vec::new();
        split_pieces(&block.text, block.page, is_heading, max_tokens, &mut pieces);
        for piece in pieces {
            let used: usize = buf.iter().map(|p| p.tokens).sum();
            if buf.len() > carried && used + piece.tokens > max_tokens {
                flush(&mut buf, &mut carried, &heading, &mut chunks, opt.overlap_tokens);
                continues = true;
            }
            buf.push(piece);
        }
    }
    let rest: usize = buf[carried..].iter().map(|p| p.tokens).sum();
    if rest > 0 && rest < MIN_BODY_TOKENS && !chunks.is_empty() {
        append_to_last(&mut buf, &mut carried, &mut chunks);
    } else {
        flush(&mut buf, &mut carried, &heading, &mut chunks, 0);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(max: usize, overlap: usize) -> ChunkOptions {
        ChunkOptions { max_tokens: max, overlap_tokens: overlap, split_at_headings: true }
    }

    #[test]
    fn short_text_is_one_chunk() {
        let blocks = vec![Block::text("第一條　本要點適用於全行。\n第二條　超過五百萬元須經兩位人員審查。", 1)];
        let c = chunk_blocks(&blocks, &opts(256, 32));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].page, 1);
        assert!(c[0].text.starts_with("第一條"));
        assert!(c[0].text.ends_with("審查。"));
    }

    #[test]
    fn heading_starts_a_new_chunk_and_leads_it() {
        let blocks = vec![
            Block::heading("房屋貸款審查作業要點", 1, 1),
            Block::text("第一條　本要點適用於房屋貸款案件，由各營業單位依本要點辦理審查。", 1),
            Block::heading("汽車貸款審查作業要點", 1, 2),
            Block::text("第一條　本要點適用於汽車貸款案件，由各營業單位依本要點辦理審查。", 2),
        ];
        let c = chunk_blocks(&blocks, &opts(256, 32));
        assert_eq!(c.len(), 2);
        assert!(c[0].text.starts_with("房屋貸款審查作業要點"));
        assert!(c[1].text.starts_with("汽車貸款審查作業要點"));
        assert_eq!(c[1].page, 2);
        assert_eq!(c[1].heading.as_deref(), Some("汽車貸款審查作業要點"));
    }

    #[test]
    fn chunk_across_pages_records_last_page() {
        let blocks = vec![
            Block::text("（八）配合其他成分時，該列之配合係數不可大於1。", 66),
            Block::text("（九）配合整腸活菌成分時，每日劑量所含之菌數不得小於1×106個。", 67),
        ];
        let c = chunk_blocks(&blocks, &opts(256, 32));
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].page, c[0].page_end), (66, 67));
        let single = chunk_blocks(&[Block::text("第一條　本要點適用於全行。", 5)], &opts(256, 32));
        assert_eq!((single[0].page, single[0].page_end), (5, 5));
    }

    #[test]
    fn long_text_splits_at_sentences_with_overlap() {
        let sentence = "這是一句大約二十個字的句子用來測試切分的行為。";
        let text = sentence.repeat(10);
        let c = chunk_blocks(&[Block::text(text, 3)], &opts(60, 25));
        assert!(c.len() >= 4);
        for ch in &c {
            assert!(estimate_tokens(&ch.text) <= 60 + 25, "chunk too long: {}", estimate_tokens(&ch.text));
            assert!(ch.text.ends_with('。'));
            assert!(ch.text.starts_with("這是"));
        }
        // 重疊：每個 chunk（除了第一個）開頭的句子是上一個 chunk 的最後一句
        assert_eq!(c.iter().map(|ch| ch.text.matches(sentence).count()).sum::<usize>(), 10 + c.len() - 1);
    }

    #[test]
    fn one_very_long_sentence_is_hard_split() {
        let text = "字".repeat(500);
        let c = chunk_blocks(&[Block::text(text, 1)], &opts(100, 0));
        assert_eq!(c.len(), 5);
        assert!(c.iter().all(|ch| ch.text.chars().count() == 100));
    }

    #[test]
    fn title_split_over_two_lines_stays_together() {
        let blocks = vec![
            Block::heading("信用合作社資本適足性及資本等級管理辦法第五條、", 1, 1),
            Block::heading("第十一條修正總說明", 1, 1),
            Block::text("為配合國際規範，爰修正本辦法部分條文，其修正要點如下。", 1),
        ];
        let c = chunk_blocks(&blocks, &opts(256, 32));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].heading.as_deref(), Some("信用合作社資本適足性及資本等級管理辦法第五條、第十一條修正總說明"));
        assert!(c[0].text.starts_with("信用合作社") && c[0].text.ends_with("如下。"));
        assert_eq!(join_heading("Annex", "A"), "Annex A");
    }

    #[test]
    fn tiny_sections_do_not_become_chunks() {
        // 標題後面只有幾個字就接下一個標題：併進下一段
        let body = "本表所列各項比率，應依主管機關規定之計算方法及表格填報。";
        let blocks = vec![
            Block::heading("附表三", 1, 1),
            Block::text("單位：千元", 1),
            Block::heading("信用風險加權風險性資產計算表", 2, 1),
            Block::text(body, 1),
            Block::text("以下空白", 2),
        ];
        let c = chunk_blocks(&blocks, &opts(256, 32));
        assert_eq!(c.len(), 1);
        assert!(c[0].text.starts_with("附表三\n單位：千元\n信用風險"));
        assert!(c[0].text.ends_with("以下空白"));

        // 長段落的一小段尾巴：接回上一個 chunk，新標題仍然在自己的 chunk 開頭
        let sentence = "這是一句大約二十個字的句子用來測試切分的行為。";
        let blocks = vec![
            Block::heading("第一章", 1, 1),
            Block::text(format!("{}結尾。", sentence.repeat(3)), 1),
            Block::heading("第二章", 1, 2),
            Block::text(sentence.repeat(2), 2),
        ];
        let c = chunk_blocks(&blocks, &opts(73, 0));
        assert_eq!(c.len(), 2);
        assert!(c[0].text.ends_with("結尾。"));
        assert!(c[1].text.starts_with("第二章"));
    }

    #[test]
    fn images_are_ignored() {
        assert!(chunk_blocks(&[Block::image("p1.png", 1)], &ChunkOptions::default()).is_empty());
    }
}
