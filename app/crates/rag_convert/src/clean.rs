//! 整理 PDF 抽出來的文字。

use rag_core::text::{canonical_han, is_cjk};
use rag_core::Block;

fn is_cjk_punct(c: char) -> bool {
    matches!(c, '，' | '。' | '、' | '；' | '：' | '？' | '！' | '（' | '）' | '「' | '」' | '『' | '』' | '《' | '》' | '〈' | '〉' | '．')
}

fn is_wide(c: char) -> bool {
    is_cjk(c) || is_cjk_punct(c)
}

/// 一行文字：連續的空白縮成一個，中文字（和全形標點）之間的空白拿掉。
/// PDF 為了對齊版面，常在字和字之間塞空白。相容漢字順便換成一般的漢字（見 [`canonical_han`]）。
fn clean_line(line: &str) -> String {
    let chars: Vec<char> = line.trim().chars().map(canonical_han).collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let prev_wide = out.chars().next_back().is_some_and(is_wide);
            let next_wide = chars.get(j).copied().is_some_and(is_wide);
            if !(prev_wide && next_wide) {
                out.push(' ');
            }
            i = j;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn ends_sentence(line: &str) -> bool {
    line.chars().next_back().is_some_and(|c| matches!(c, '。' | '！' | '？' | '；' | '：' | '.' | '!' | '?' | ';' | ':'))
}

/// 看起來像條列或條文的開頭（「第 3 條」「一、」「(一)」「1.」等），這種行不接在上一行後面。
fn starts_item(line: &str) -> bool {
    let mut chars = line.chars();
    let Some(first) = chars.next() else { return false };
    if first == '第' {
        return line.chars().take(8).any(|c| matches!(c, '條' | '章' | '節' | '點' | '款' | '項'));
    }
    if matches!(first, '(' | '（' | '•' | '-' | '＊' | '*' | '※') {
        return true;
    }
    let numeral = |c: char| c.is_ascii_digit() || "一二三四五六七八九十百壹貳參肆伍陸柒捌玖拾".contains(c);
    if numeral(first) {
        let rest: String = line.chars().skip_while(|c| numeral(*c)).take(1).collect();
        return matches!(rest.as_str(), "、" | "." | "．" | ")" | "）" | " ");
    }
    false
}

/// 只有頁碼的行，例如「- 1 -」「第 3 頁」「3/12」。
fn is_page_number(line: &str) -> bool {
    let core: String = line.chars().filter(|c| !c.is_whitespace() && !matches!(c, '-' | '－' | '—' | '–' | '~' | '～')).collect();
    if core.is_empty() || core.chars().count() > 12 {
        return false;
    }
    let digits = core.chars().filter(|c| c.is_ascii_digit()).count();
    // 單獨一個數字：三位數以內才當成頁碼，免得把表格裡的年份、金額丟掉
    if digits == core.chars().count() && digits == line.trim().chars().count() {
        return digits <= 3;
    }
    digits > 0 && core.chars().all(|c| c.is_ascii_digit() || matches!(c, '第' | '頁' | '共' | '/' | '，' | ','))
}

/// 轉成 Markdown 時留下的記號，不是文件內容：程式碼區塊的 ``` 和整行的分隔線。
fn is_markup_noise(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("```") || (line.chars().count() >= 3 && line.chars().all(|c| matches!(c, '-' | '_' | '*' | '=')))
}

/// 文字裡有這個比例以上的字不是中文、英文該有的字，就當成亂碼。
/// 實測（400 份法規附件，實驗 63）：正常的 chunk 最高不到 10%（只有符號字型的項目符號），亂碼的那一頁是 60% 和 86%。
const GARBLED_SHARE: f64 = 0.3;

/// 中文和英文文件裡會出現的文字：拉丁字母、希臘字母（算式）、中日韓的字和全形字。
fn is_expected_letter(c: char) -> bool {
    matches!(c as u32,
        0..=0x24F | 0x370..=0x3FF | 0x2E80..=0x9FFF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

/// 這段文字是不是亂碼：PDF 的字型把字對應到錯的編碼時，抽出來的會是泰文、孟加拉文這類毫不相干的文字，
/// 或是私人使用區的編碼。只算字母類的字，數字和標點不算。
pub fn is_garbled(text: &str) -> bool {
    let (mut letters, mut bad) = (0usize, 0usize);
    for c in text.chars() {
        let private = matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0x10FFFF) || c == '\u{FFFD}';
        if private || c.is_alphabetic() {
            letters += 1;
            if private || !is_expected_letter(c) {
                bad += 1;
            }
        }
    }
    letters >= 20 && bad as f64 >= letters as f64 * GARBLED_SHARE
}

/// 標題超過這個字數就當成內文。LiteParse 有時把字體較大的整段附註判成標題。
const MAX_HEADING_CHARS: usize = 60;

/// 最寬的行至少要這麼寬（10 個中文字），才把沒寫完的行接回上一行。
const MIN_WRAP_WIDTH: usize = 20;

/// 一行佔多寬：中文字和全形標點算 2，其他算 1。
fn width(line: &str) -> usize {
    line.chars().map(|c| if is_wide(c) { 2 } else { 1 }).sum()
}

/// 整理一頁（或一段）文字：每行清掉多餘空白，被版面寬度切斷的句子接回同一行。
///
/// 判斷「被版面切斷」的依據：上一行寫到接近最寬的那一行那麼寬、結尾不是句號類的標點，
/// 而且下一行不像條列的開頭。比較短的行視為作者自己換的行。
pub fn clean_text(text: &str) -> String {
    let lines: Vec<String> = text.lines().filter(|l| !is_markup_noise(l)).map(clean_line).collect();
    let widest = lines.iter().map(|l| width(l)).max().unwrap_or(0);
    // 整段都是很短的行（表頭、欄位名稱）時不接行：那不是被版面切斷的句子
    let full = if widest < MIN_WRAP_WIDTH { usize::MAX } else { widest * 8 / 10 };
    let mut out = String::with_capacity(text.len());
    let mut prev = String::new();
    for line in lines {
        if line.is_empty() {
            if !prev.is_empty() {
                out.push('\n');
                prev.clear();
            }
            continue;
        }
        if is_page_number(&line) {
            continue;
        }
        // 表格的每一列各自成行
        let table = prev.starts_with('|') || line.starts_with('|');
        if prev.is_empty() {
            // 段落的第一行
        } else if table || width(&prev) < full || ends_sentence(&prev) || starts_item(&line) {
            out.push('\n');
        } else {
            // 上一行沒有結束：中文直接相接，英文中間補一個空白
            let a = prev.chars().next_back().is_some_and(is_wide);
            let b = line.chars().next().is_some_and(is_wide);
            if !(a || b) {
                out.push(' ');
            }
        }
        out.push_str(&line);
        prev = line;
    }
    out.trim().to_string()
}

/// 把 LiteParse 輸出的 Markdown 拆成區塊：`#` 開頭的行是標題，其他的行併成文字區塊。
pub fn markdown_blocks(markdown: &str, page: u32) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut body = String::new();
    let flush = |body: &mut String, blocks: &mut Vec<Block>| {
        let text = clean_text(body);
        if !text.is_empty() {
            blocks.push(Block::text(text, page));
        }
        body.clear();
    };
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
            let title = clean_line(&trimmed[level..]);
            if title.chars().count() > MAX_HEADING_CHARS || title.ends_with('。') {
                // 不像標題：當成一般的內文
                body.push_str(&title);
                body.push('\n');
            } else if !title.is_empty() {
                flush(&mut body, &mut blocks);
                blocks.push(Block::heading(title, level as u8, page));
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    flush(&mut body, &mut blocks);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_between_cjk_are_removed() {
        assert_eq!(clean_line("  授 信  審 查   作業 ， 要點 "), "授信審查作業，要點");
        assert_eq!(clean_line("Loan   review  2024 年"), "Loan review 2024 年");
        assert_eq!(clean_line("投入\u{F96B}\u{F969}"), "投入參數");
    }

    #[test]
    fn wrapped_lines_are_joined() {
        let text = "第 1 條\n本要點適用於全行之授信案件，並由各\n營業單位辦理。\n第 2 條\n超過五百萬元須經兩位人員審查。";
        assert_eq!(clean_text(text), "第 1 條\n本要點適用於全行之授信案件，並由各營業單位辦理。\n第 2 條\n超過五百萬元須經兩位人員審查。");
        assert_eq!(
            clean_text("all loan records are kept\nfor seven years.\n\nNext paragraph"),
            "all loan records are kept for seven years.\nNext paragraph"
        );
        // 整段都是短行：各自成行
        assert_eq!(clean_text("修正條文\n現行條文\n說明"), "修正條文\n現行條文\n說明");
    }

    #[test]
    fn list_items_stay_on_their_own_lines() {
        let text = "前條委員之組成如下\n一、金管會代表三人\n二、保險業代表四人\n(一) 產險\n1. 其他";
        assert_eq!(clean_text(text), text);
    }

    #[test]
    fn table_rows_and_page_numbers() {
        let text = "附表一\n| 機構 | 等級 |\n|---|---|\n| Fitch Ratings Ltd. | BBB- |\n| 中華信評 | twBBB- |\n- 1 -\n第 2 頁，共 5 頁\n說明如下";
        assert_eq!(clean_text(text), "附表一\n| 機構 | 等級 |\n|---|---|\n| Fitch Ratings Ltd. | BBB- |\n| 中華信評 | twBBB- |\n說明如下");
        assert!(is_page_number("3/12") && is_page_number("－ 12 －"));
        assert!(!is_page_number("第 3 條") && !is_page_number("2024") && is_page_number("7"));
    }

    #[test]
    fn markdown_headings() {
        let b = markdown_blocks("# 授信 審查 要點\n第一條 本要點適用於全行各營業\n單位之授信案件。\n## 附則\n本要點自發布日施行。", 3);
        assert_eq!(b.len(), 4);
        assert_eq!(b[0].heading_level, Some(1));
        assert_eq!(b[0].text, "授信審查要點");
        assert_eq!(b[1].text, "第一條本要點適用於全行各營業單位之授信案件。");
        assert_eq!(b[2].heading_level, Some(2));
        assert_eq!(b[3].page, 3);
    }

    #[test]
    fn markup_noise_and_fake_headings() {
        assert_eq!(clean_text("修正條文\n```\n現行條文\n```\n---\n說明"), "修正條文\n現行條文\n說明");
        // 分隔列是表格的一部分，要留著
        assert_eq!(clean_text("| 甲 | 乙 |\n|---|---|\n| 1 | 2 |"), "| 甲 | 乙 |\n|---|---|\n| 1 | 2 |");
        let note = "註：本表所稱信用評等，指經主管機關認可之信用評等機構所為之評等。";
        let b = markdown_blocks(&format!("# 附表\n第一列\n## {note}\n第二列"), 1);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].heading_level, Some(1));
        assert_eq!(b[1].heading_level, None);
        assert!(b[1].text.contains("第一列") && b[1].text.contains(note) && b[1].text.contains("第二列"));
    }

    #[test]
    fn garbled_text_layer() {
        // 真實文件裡抽出來的亂碼（外國銀行在臺分行發行新臺幣金融債券辦法 條文對照表，第 4 頁）
        assert!(is_garbled("ಃΒచคߕҹ)অ҅ࡕ!*\nಃΒచߕҹ)অ҅߻!*\nѦ୯ሌՉӧᆵϩՉวՉཥᆵჾߎᑼ໸چҙፎਜ!!!!!!!!!!!"));
        assert!(is_garbled(&"\u{E123}\u{E456}\u{F0E5}".repeat(10)));
        assert!(!is_garbled("第一條　本辦法依銀行法第七十二條之一規定訂定之。Basel III 資本適足率 α = 8%"));
        // 符號字型的項目符號夾在正常文字裡，比例很低
        assert!(!is_garbled("\u{F0E5} 信用合作社授信審議委員會應有全體委員三分之二以上出席\n\u{F06C} 出席委員過半數同意"));
        // 字太少不判斷
        assert!(!is_garbled("ಃΒచ"));
    }
}
