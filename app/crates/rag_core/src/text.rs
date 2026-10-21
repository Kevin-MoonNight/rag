//! 文字正規化與關鍵字切詞。
//!
//! 關鍵字索引的做法：中文的連續字串切成單字和相鄰的雙字，英文和數字保留整個詞。
//! 不需要字典，編號和姓名可以完全比對。文件和查詢兩邊都先經過 [`normalize`]。

use crate::types::SparseVec;

/// 關鍵字的正規化和切詞方式的版本。改了這裡的規則就要加一，舊索引的關鍵字才不會和新的查詢對不上。
/// 2：繁體字和簡體字折成同一種字形（已拿掉）。
/// 3：拿掉繁簡統一，切法和 1 相同。
pub const KEYWORD_VERSION: u32 = 3;

/// 是不是中文字（中日韓表意文字）。
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

/// 相容漢字（U+F900–FAFF、U+2F800–2FA1F）和康熙部首（U+2E80–2FDF）換成一般的漢字。
/// PDF 的字型常用這些編碼，外觀和一般的字一樣，但模型抄不出來（「參數」會寫成「引數」之類），
/// 關鍵字也比對不到。相容區裡本身就是一般漢字的那幾個（例如「﨎」）不會變。
pub fn canonical_han(c: char) -> char {
    use unicode_normalization::UnicodeNormalization;
    if !matches!(c as u32, 0x2E80..=0x2FDF | 0xF900..=0xFAFF | 0x2F800..=0x2FA1F) {
        return c;
    }
    let mut it = std::iter::once(c).nfkc();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// 全形英數和符號轉半形、英文轉小寫、相容漢字換成一般漢字，並去掉中文字之間的空白。
/// 繁體和簡體不統一：使用者用簡體查繁體文件的情況太少，不值得處理（2026-10-14 決定）。
pub fn normalize(s: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(s.len());
    for c in s.chars() {
        let c = match c as u32 {
            0x2E80..=0x2FDF | 0xF900..=0xFAFF | 0x2F800..=0x2FA1F => canonical_han(c),
            0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            0x3000 => ' ',
            _ => c,
        };
        for l in c.to_lowercase() {
            out.push(l);
        }
    }
    // 去掉夾在兩個中文字之間的空白（PDF 抽出的文字常在字和字之間插入空白）
    let mut res = String::with_capacity(out.len() * 3);
    let mut i = 0;
    while i < out.len() {
        if out[i] == ' ' {
            let mut j = i;
            while j < out.len() && out[j] == ' ' {
                j += 1;
            }
            let prev_cjk = i > 0 && is_cjk(out[i - 1]);
            let next_cjk = j < out.len() && is_cjk(out[j]);
            if !(prev_cjk && next_cjk) {
                res.push(' ');
            }
            i = j;
        } else {
            res.push(out[i]);
            i += 1;
        }
    }
    res
}

/// 切成關鍵字索引用的詞。輸入要先經過 [`normalize`]。
pub fn keyword_tokens(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut run: Vec<char> = Vec::new();
    let flush_word = |word: &mut String, tokens: &mut Vec<String>| {
        if !word.is_empty() {
            tokens.push(std::mem::take(word));
        }
    };
    let flush_run = |run: &mut Vec<char>, tokens: &mut Vec<String>| {
        for (i, c) in run.iter().enumerate() {
            tokens.push(c.to_string());
            if i + 1 < run.len() {
                tokens.push([*c, run[i + 1]].iter().collect());
            }
        }
        run.clear();
    };
    for c in s.chars() {
        if is_cjk(c) {
            flush_word(&mut word, &mut tokens);
            run.push(c);
        } else if c.is_alphanumeric() {
            flush_run(&mut run, &mut tokens);
            word.push(c);
        } else {
            flush_word(&mut word, &mut tokens);
            flush_run(&mut run, &mut tokens);
        }
    }
    flush_word(&mut word, &mut tokens);
    flush_run(&mut run, &mut tokens);
    tokens
}

/// 詞換成固定的 32 位元編號（FNV-1a）。不同的詞撞到同一個編號的機率很低，撞到只會讓排名略有偏差。
pub fn term_id(term: &str) -> u32 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in term.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h ^ (h >> 32)) as u32
}

/// BM25 的參數。詞的稀有程度（IDF）由資料庫在查詢時計算，這裡只算文件這一側的詞頻部分。
const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;
/// 假設的平均文件長度（詞數）。建索引時還不知道真正的平均值，用固定值讓分數在不同批次之間一致。
const BM25_AVG_LEN: f32 = 300.0;

fn to_sparse(mut pairs: Vec<(u32, f32)>) -> SparseVec {
    pairs.sort_by_key(|p| p.0);
    let mut out = SparseVec::default();
    for (i, v) in pairs {
        if out.indices.last() == Some(&i) {
            *out.values.last_mut().unwrap() += v; // 編號相撞的詞合併
        } else {
            out.indices.push(i);
            out.values.push(v);
        }
    }
    out
}

/// 文件 chunk 的稀疏向量。
pub fn document_sparse(text: &str) -> SparseVec {
    let tokens = keyword_tokens(&normalize(text));
    let len = tokens.len() as f32;
    let mut ids: Vec<u32> = tokens.iter().map(|t| term_id(t)).collect();
    ids.sort_unstable();
    let mut pairs = Vec::new();
    let mut i = 0;
    while i < ids.len() {
        let mut j = i;
        while j < ids.len() && ids[j] == ids[i] {
            j += 1;
        }
        let tf = (j - i) as f32;
        let w = tf * (BM25_K1 + 1.0) / (tf + BM25_K1 * (1.0 - BM25_B + BM25_B * len / BM25_AVG_LEN));
        pairs.push((ids[i], w));
        i = j;
    }
    to_sparse(pairs)
}

/// 查詢的稀疏向量：每個出現的詞權重 1。
pub fn query_sparse(text: &str) -> SparseVec {
    let tokens = keyword_tokens(&normalize(text));
    let mut ids: Vec<u32> = tokens.iter().map(|t| term_id(t)).collect();
    ids.sort_unstable();
    ids.dedup();
    to_sparse(ids.into_iter().map(|i| (i, 1.0)).collect())
}

/// 粗估一段文字有幾個 token：中文字、數字、標點和其他符號各算 1 個，英文字母 4 個算 1 個。
/// 只用來決定 chunk 切在哪裡，不需要和模型的分詞器完全一致，但不能低估太多：模型一次能處理的 token 有上限。
///
/// 和 EmbeddingGemma 2 的分詞器比對（4,469 個真實 chunk，實驗 62）：實際值是這個估計的 0.80 倍（中位數），
/// 99% 的 chunk 不超過 1.05 倍，最多 1.56 倍。原本把標點和數字也當成 4 個算 1 個，表格類的內容最多低估到 6 倍。
pub fn estimate_tokens(s: &str) -> usize {
    let mut whole = 0usize;
    let mut letters = 0usize;
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            letters += 1;
        } else if !c.is_whitespace() {
            whole += 1;
        }
    }
    whole + letters.div_ceil(4)
}

/// 一個字元折成比對用的形式：全形轉半形、英文轉小寫。和 [`normalize`] 一樣，但一個字對一個字，位置不變。
fn fold(c: char) -> char {
    let c = match c as u32 {
        0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        0x3000 => ' ',
        _ => c,
    };
    c.to_lowercase().next().unwrap_or(c)
}

/// 結果段落裡要標示的位置：查詢裡的英數詞，和中文的連續兩個字（只有一個中文字時就是那個字）。
/// 回傳 `(開始, 結束)`，單位是 UTF-16 的位置（畫面用 JavaScript 處理字串），已排序、不重疊。
pub fn highlight(text: &str, query: &str) -> Vec<(u32, u32)> {
    let query = normalize(query);
    let mut terms: Vec<Vec<char>> = Vec::new();
    let mut run: Vec<char> = Vec::new();
    let mut word: Vec<char> = Vec::new();
    let flush = |run: &mut Vec<char>, word: &mut Vec<char>, terms: &mut Vec<Vec<char>>| {
        if run.len() == 1 {
            terms.push(run.clone());
        }
        terms.extend(run.windows(2).map(|w| w.to_vec()));
        if !word.is_empty() {
            terms.push(word.clone());
        }
        run.clear();
        word.clear();
    };
    for c in query.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                terms.push(std::mem::take(&mut word));
            }
            run.push(c);
        } else if c.is_alphanumeric() {
            if !run.is_empty() {
                flush(&mut run, &mut Vec::new(), &mut terms);
            }
            word.push(c);
        } else {
            flush(&mut run, &mut word, &mut terms);
        }
    }
    flush(&mut run, &mut word, &mut terms);
    terms.sort();
    terms.dedup();

    let chars: Vec<char> = text.chars().collect();
    let folded: Vec<char> = chars.iter().map(|&c| fold(c)).collect();
    let mut marked = vec![false; chars.len()];
    for term in &terms {
        let ascii_word = term.iter().all(|c| !is_cjk(*c));
        for start in 0..folded.len().saturating_sub(term.len() - 1) {
            if folded[start..start + term.len()] != term[..] {
                continue;
            }
            // 英數詞要整個詞相符：「ab」不標示「abc」裡的 ab
            let end = start + term.len();
            if ascii_word && (start > 0 && folded[start - 1].is_ascii_alphanumeric() || end < folded.len() && folded[end].is_ascii_alphanumeric()) {
                continue;
            }
            marked[start..end].iter_mut().for_each(|m| *m = true);
        }
    }

    let mut out = Vec::new();
    let mut offset = 0u32;
    let mut open: Option<u32> = None;
    for (c, &m) in chars.iter().zip(&marked) {
        match (m, open) {
            (true, None) => open = Some(offset),
            (false, Some(s)) => {
                out.push((s, offset));
                open = None;
            }
            _ => {}
        }
        offset += c.len_utf16() as u32;
    }
    if let Some(s) = open {
        out.push((s, offset));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn highlight_marks_bigrams_and_whole_words() {
        let text = "授信審查：ＡＢＣ公司 abc123 與 ab 的授信";
        let spans = super::highlight(text, "授信 ab");
        let pieces: Vec<String> = spans.iter().map(|&(s, e)| text.chars().skip(s as usize).take((e - s) as usize).collect()).collect();
        assert_eq!(pieces, vec!["授信", "ab", "授信"]);
        // 只有一個中文字時標那個字；全形英文也比對得到
        let spans = super::highlight("ＡＢＣ公司ab", "abc 司 ab");
        assert_eq!(spans, vec![(0, 3), (4, 7)]);
    }

    use super::*;

    #[test]
    fn normalize_widths_and_spaces() {
        assert_eq!(normalize("ＡＢＣ１２３"), "abc123");
        assert_eq!(normalize("授 信  審 查 ABC def"), "授信審查 abc def");
        assert_eq!(normalize("編號　A-01"), "編號 a-01");
        // 相容漢字（參 U+F96B、數 U+F969）和康熙部首（⾦ U+2FA6）換成一般的字；本身是一般漢字的不變
        assert_eq!(normalize("\u{F96B}\u{F969}\u{2FA6}"), "參數金");
        assert_eq!(canonical_han('\u{FA0E}'), '\u{FA0E}');
    }

    #[test]
    fn tokens_mix() {
        let t = keyword_tokens(&normalize("房屋貸款 A-123 審查"));
        assert_eq!(t, vec!["房", "房屋", "屋", "屋貸", "貸", "貸款", "款", "a", "123", "審", "審查", "查"]);
    }

    #[test]
    fn sparse_sorted_unique() {
        let v = document_sparse("貸款貸款 loan loan loan");
        assert!(v.indices.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(v.indices.len(), v.values.len());
        let q = query_sparse("貸款");
        assert_eq!(q.indices.len(), 3); // 貸、貸款、款
        assert!(q.indices.iter().all(|i| v.indices.contains(i)));
    }

    #[test]
    fn token_estimate() {
        assert_eq!(estimate_tokens("授信審查"), 4);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }
}
