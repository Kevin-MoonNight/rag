//! 三份檢索名單的合併。
//!
//! 文字向量、圖片向量、關鍵字各自產生一份名單。關鍵字的分數和向量相似度是不同的尺度，不能直接比，
//! 所以向量和關鍵字兩邊輪流各取一筆（實驗 36：RRF 會把只在一份名單裡的掃描頁、跨語言文件壓到後面）。
//!
//! 文字和圖片的相似度是同一個模型算的，加上固定的補償值就能比：預設先把兩份向量名單依分數排成一份，
//! 再和關鍵字輪流（[`vectors_then_keywords`]）。舊的做法是三份名單輪流（[`take_turns`]），
//! 每 3 個名次就有 1 個給圖片，文字問題的前 10 名平均有 3.8 筆不相關的圖片（實驗 76）。

use crate::types::Hit;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    TextVector,
    ImageVector,
    Keyword,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Merged {
    pub id: u64,
    /// 這一筆最先是從哪一份名單取到的。
    pub source: Source,
    /// 在那份名單裡的原始分數。
    pub score: f32,
}

/// 三份名單輪流各取一筆，重複的編號只保留第一次出現的位置。每份名單要先由高分排到低分。
///
/// `agree_depth` 大於 0 時，文字向量和關鍵字兩份名單的前 `agree_depth` 名裡都出現的段落先排在最前面
/// （兩種方法都找到同一段，比只有一種方法找到的可靠）；依兩邊名次的總和排序。0 表示不做這一步。
pub fn take_turns(text: &[Hit], image: &[Hit], keyword: &[Hit], image_offset: f32, agree_depth: usize, limit: usize) -> Vec<Merged> {
    let mut out: Vec<Merged> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    let rounds = text.len().max(image.len()).max(keyword.len());
    let mut push = |hit: &Hit, source: Source, out: &mut Vec<Merged>| {
        if seen.insert(hit.id) {
            out.push(Merged { id: hit.id, source, score: hit.score });
        }
    };

    if agree_depth > 0 {
        // 圖片的第一名夠接近文字的第一名時，仍然排在最前面（和下面每一輪裡的規則相同）
        if let (Some(t), Some(i)) = (text.first(), image.first()) {
            if i.score + image_offset > t.score {
                push(i, Source::ImageVector, &mut out);
            }
        }
        let mut agreed: Vec<(usize, &Hit)> = text
            .iter()
            .take(agree_depth)
            .enumerate()
            .filter_map(|(t_rank, t)| keyword.iter().take(agree_depth).position(|k| k.id == t.id).map(|k_rank| (t_rank + k_rank, t)))
            .collect();
        agreed.sort_by_key(|(rank_sum, _)| *rank_sum);
        for (_, hit) in agreed {
            push(hit, Source::TextVector, &mut out);
        }
    }

    for r in 0..rounds {
        match (text.get(r), image.get(r)) {
            (Some(t), Some(i)) if i.score + image_offset > t.score => {
                push(i, Source::ImageVector, &mut out);
                push(t, Source::TextVector, &mut out);
            }
            (t, i) => {
                if let Some(t) = t {
                    push(t, Source::TextVector, &mut out);
                }
                if let Some(i) = i {
                    push(i, Source::ImageVector, &mut out);
                }
            }
        }
        if let Some(k) = keyword.get(r) {
            push(k, Source::Keyword, &mut out);
        }
        if out.len() >= limit {
            break;
        }
    }
    out.truncate(limit);
    out
}

/// 圖片名單的第 1 名至少排在第幾名（第一頁）。文字相似度普遍很高的問題，掃描頁加了補償值仍可能排到第 11 名以後（實驗 76）。
pub const IMAGE_FLOOR: usize = 10;

/// 文字和圖片兩份向量名單依分數排成一份（圖片的相似度加上 `image_offset` 再比），再和關鍵字名單輪流各取一筆，
/// 向量先。重複的編號只保留第一次出現的位置。每份名單要先由高分排到低分。
/// 圖片名單的第 1 名如果排在第 [`IMAGE_FLOOR`] 名以後，移到第 [`IMAGE_FLOOR`] 名。
pub fn vectors_then_keywords(text: &[Hit], image: &[Hit], keyword: &[Hit], image_offset: f32, limit: usize) -> Vec<Merged> {
    let mut vectors: Vec<(f32, &Hit, Source)> = text
        .iter()
        .map(|h| (h.score, h, Source::TextVector))
        .chain(image.iter().map(|h| (h.score + image_offset, h, Source::ImageVector)))
        .collect();
    // 穩定排序：同分時文字在前
    vectors.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out: Vec<Merged> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for r in 0..vectors.len().max(keyword.len()) {
        let v = vectors.get(r).map(|&(_, h, s)| (h, s));
        let k = keyword.get(r).map(|h| (h, Source::Keyword));
        for (hit, source) in [v, k].into_iter().flatten() {
            if seen.insert(hit.id) {
                out.push(Merged { id: hit.id, source, score: hit.score });
            }
        }
        if out.len() >= limit.max(IMAGE_FLOOR) {
            break;
        }
    }
    if let Some(top) = image.first() {
        let at = out.iter().position(|m| m.id == top.id);
        if at.is_none_or(|at| at >= IMAGE_FLOOR) {
            if let Some(at) = at {
                out.remove(at);
            }
            out.insert((IMAGE_FLOOR - 1).min(out.len()), Merged { id: top.id, source: Source::ImageVector, score: top.score });
        }
    }
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_sorted_by_score_then_alternate_with_keywords() {
        let text = hits(&[(1, 0.60), (2, 0.55), (3, 0.50)]);
        let image = hits(&[(10, 0.45), (11, 0.30)]);
        let keyword = hits(&[(2, 9.0), (4, 7.0), (1, 3.0)]);
        // 向量合一：1 (0.60)、2 (0.55)、10 (0.45 + 0.08)、3 (0.50)、11 (0.38)
        let m = vectors_then_keywords(&text, &image, &keyword, 0.08, 10);
        // 第 1 輪 1、2（關鍵字）；第 2 輪 2 已出現、4；第 3 輪 10、1 已出現；之後 3、11
        assert_eq!(ids(&m), vec![1, 2, 4, 10, 3, 11]);
        assert_eq!(m[1].source, Source::Keyword);
        assert_eq!(m[3].source, Source::ImageVector);
        assert_eq!(m[3].score, 0.45); // 回傳原始分數，不含補償值
        // 圖片夠接近時排到第一
        let m = vectors_then_keywords(&text, &hits(&[(10, 0.55)]), &[], 0.08, 10);
        assert_eq!(ids(&m), vec![10, 1, 2, 3]);
        // 不相關的圖片不會每輪都佔一個名次
        let m = vectors_then_keywords(&text, &hits(&[(10, 0.10), (11, 0.09)]), &keyword, 0.08, 4);
        assert_eq!(ids(&m), vec![1, 2, 4, 3]);
        assert!(vectors_then_keywords(&[], &[], &[], 0.08, 5).is_empty());
    }

    #[test]
    fn top_image_stays_on_first_page() {
        let text: Vec<Hit> = (1..=30).map(|n| Hit { id: n, score: 0.9 - n as f32 * 0.001 }).collect();
        let image = hits(&[(100, 0.5), (101, 0.49)]);
        let m = vectors_then_keywords(&text, &image, &[], 0.11, 20);
        assert_eq!(m.len(), 20);
        assert_eq!(m.iter().position(|x| x.id == 100), Some(IMAGE_FLOOR - 1));
        assert!(!m.iter().any(|x| x.id == 101));
        // 已經在前面的不動
        let m = vectors_then_keywords(&text[..2], &image, &[], 0.11, 20);
        assert_eq!(ids(&m), vec![1, 2, 100, 101]);
    }

    fn hits(v: &[(u64, f32)]) -> Vec<Hit> {
        v.iter().map(|&(id, score)| Hit { id, score }).collect()
    }

    fn ids(m: &[Merged]) -> Vec<u64> {
        m.iter().map(|x| x.id).collect()
    }

    #[test]
    fn rounds_interleave_and_dedupe() {
        let text = hits(&[(1, 0.60), (2, 0.55), (3, 0.50)]);
        let image = hits(&[(10, 0.40), (11, 0.35)]);
        let keyword = hits(&[(2, 9.0), (1, 7.0), (4, 3.0)]);
        let m = take_turns(&text, &image, &keyword, 0.08, 0, 10);
        // 第 1 輪：文字 1（0.60 > 0.40 + 0.08）、圖片 10、關鍵字 2；第 2 輪：2 已出現，圖片 11，關鍵字 1 已出現；第 3 輪：3、4
        assert_eq!(ids(&m), vec![1, 10, 2, 11, 3, 4]);
        assert_eq!(m[2].source, Source::Keyword);
    }

    #[test]
    fn chunks_found_by_both_text_and_keywords_go_first() {
        let text = hits(&[(1, 0.60), (2, 0.55), (3, 0.50)]);
        let image = hits(&[(10, 0.40)]);
        let keyword = hits(&[(3, 9.0), (4, 7.0), (2, 3.0)]);
        // 2（名次 1 + 2）和 3（名次 2 + 0）兩邊都有，3 的名次總和較小；之後才是原本的輪流
        let m = take_turns(&text, &image, &keyword, 0.08, 10, 10);
        assert_eq!(ids(&m), vec![3, 2, 1, 10, 4]);
        // 只看前 2 名：關鍵字的前 2 名是 3、4，文字的前 2 名是 1、2，沒有交集
        let m = take_turns(&text, &image, &keyword, 0.08, 2, 10);
        assert_eq!(ids(&m), vec![1, 10, 3, 2, 4]);
        // 圖片第一名夠接近文字第一名時，仍然排最前面
        let image = hits(&[(10, 0.58)]);
        let m = take_turns(&text, &image, &keyword, 0.08, 10, 10);
        assert_eq!(ids(&m), vec![10, 3, 2, 1, 4]);
    }

    #[test]
    fn image_goes_first_when_close_enough() {
        let text = hits(&[(1, 0.45)]);
        let image = hits(&[(10, 0.40)]);
        let m = take_turns(&text, &image, &[], 0.08, 0, 10);
        assert_eq!(ids(&m), vec![10, 1]);
        let m = take_turns(&text, &image, &[], 0.0, 0, 10);
        assert_eq!(ids(&m), vec![1, 10]);
    }

    #[test]
    fn only_one_list_and_limit() {
        let image = hits(&[(10, 0.4), (11, 0.3), (12, 0.2)]);
        let m = take_turns(&[], &image, &[], 0.08, 0, 2);
        assert_eq!(ids(&m), vec![10, 11]);
        assert!(take_turns(&[], &[], &[], 0.08, 0, 5).is_empty());
    }
}
