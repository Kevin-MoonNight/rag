//! 回答裡的 LaTeX 數學式轉成一般文字（實驗 93）。
//!
//! 生成模型遇到化學式、次方、統計式會寫成 `$\text{CO}_2$`、`$1 \times 10^6$`，介面不渲染數學式，使用者會看到原始符號。
//! 在提示裡禁止 LaTeX 會讓模型改寫很多題的回答，還會把文件裡被壓平的次方（PDF 文字是「1×106」）照抄成錯的數字；
//! 所以提示不改，回答出來後把簡單的數學式換成一般文字和上下標字元：`1 × 10⁶`、`CO₂`。
//!
//! 只處理 `$…$` 和 `$$…$$`，而且要像數學式（裡面有 `\`、`^`、`_`、`{`、`}`、`<`、`>`、`=`）才轉，
//! 「US$100 和 US$200」這種金額不動。超過 [`MAX_SPAN`] 個字或跨行的不算數學式。

/// 一個數學式最多幾個字。
const MAX_SPAN: usize = 300;

/// 整段文字裡的數學式轉成一般文字。
pub fn plain_math(text: &str) -> String {
    convert(text, true).0
}

/// 串流用：一段一段收回答，轉換後馬上可以顯示的部分先送出，還沒結束的數學式先留著。
/// 所有 [`push`](Self::push) 和 [`finish`](Self::finish) 的輸出接起來，等於整段做 [`plain_math`]。
#[derive(Debug, Default)]
pub struct MathStream {
    raw: String,
    done: usize,
}

impl MathStream {
    pub fn push(&mut self, text: &str) -> String {
        self.raw.push_str(text);
        let (out, used) = convert(&self.raw[self.done..], false);
        self.done += used;
        out
    }

    pub fn finish(&mut self) -> String {
        let (out, used) = convert(&self.raw[self.done..], true);
        self.done += used;
        out
    }
}

/// 從頭轉換到第一個「還不知道是不是數學式」的 `$`（`finished` 時轉到最後）；回傳轉好的文字和用掉的位元組數。
fn convert(text: &str, finished: bool) -> (String, usize) {
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if !rest.starts_with('$') {
            let c = rest.chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        let delimiter = if rest.starts_with("$$") { "$$" } else { "$" };
        let body = &rest[delimiter.len()..];
        // 找結尾：MAX_SPAN 個字以內、不跨行
        let mut end = None;
        let mut undecided = true;
        for (n, (at, c)) in body.char_indices().enumerate() {
            if n >= MAX_SPAN || c == '\n' {
                undecided = false;
                break;
            }
            if body[at..].starts_with(delimiter) {
                end = Some(at);
                break;
            }
        }
        match end {
            Some(at) if looks_like_math(&body[..at]) => {
                out.push_str(&span(&body[..at]));
                i += delimiter.len() + at + delimiter.len();
            }
            None if undecided && !finished => return (out, i),
            _ => {
                out.push('$');
                i += 1;
            }
        }
    }
    (out, i)
}

fn looks_like_math(s: &str) -> bool {
    !s.trim().is_empty() && s.contains(['\\', '^', '_', '{', '}', '<', '>', '='])
}

/// 一個數學式的內容轉成一般文字。
fn span(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let text = render(&chars, &mut i, false);
    let mut out = String::new();
    for c in text.trim().chars() {
        if !(c == ' ' && out.ends_with(' ')) {
            out.push(c);
        }
    }
    out
}

/// 從 `i` 開始轉換；`group` 時遇到對應的 `}` 停下。
fn render(chars: &[char], i: &mut usize, group: bool) -> String {
    let mut out = String::new();
    while *i < chars.len() {
        let c = chars[*i];
        *i += 1;
        match c {
            '}' if group => return out,
            '{' => out.push_str(&render(chars, i, true)),
            '}' => {}
            '\\' => out.push_str(&command(chars, i)),
            '^' => {
                let arg = argument(chars, i);
                out.push_str(&scripted(&arg, true));
            }
            '_' => {
                let arg = argument(chars, i);
                out.push_str(&scripted(&arg, false));
            }
            '~' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// `^`、`_`、`\frac` 等後面的一個參數：`{…}`、一個指令、或一個字。
fn argument(chars: &[char], i: &mut usize) -> String {
    while *i < chars.len() && chars[*i] == ' ' {
        *i += 1;
    }
    let Some(&c) = chars.get(*i) else { return String::new() };
    *i += 1;
    match c {
        '{' => render(chars, i, true),
        '\\' => command(chars, i),
        _ => c.to_string(),
    }
}

/// `\` 後面的指令（`i` 指在 `\` 的下一個字）。
fn command(chars: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < chars.len() && chars[*i].is_ascii_alphabetic() {
        *i += 1;
    }
    if *i == start {
        // 單一符號：\%、\{、\,、\ 等
        let Some(&c) = chars.get(*i) else { return String::new() };
        *i += 1;
        return match c {
            ',' | ';' | ':' | ' ' | '\\' => " ".into(),
            '!' => String::new(),
            _ => c.to_string(),
        };
    }
    let name: String = chars[start..*i].iter().collect();
    // 指令名稱後面的一個空白是分隔用的，不是內容
    if chars.get(*i) == Some(&' ') {
        *i += 1;
    }
    let symbol = match name.as_str() {
        "text" | "mathrm" | "textrm" | "mathbf" | "textbf" | "mathit" | "textit" | "operatorname" | "mbox" | "mathsf" | "boldsymbol" => {
            return argument(chars, i);
        }
        "frac" | "dfrac" | "tfrac" => {
            let a = argument(chars, i);
            let b = argument(chars, i);
            let wrap = |s: String| if s.chars().count() > 1 && s.contains([' ', '+', '-', '−']) { format!("({s})") } else { s };
            return format!("{}/{}", wrap(a), wrap(b));
        }
        "sqrt" => return format!("√{}", argument(chars, i)),
        "left" | "right" | "big" | "Big" | "bigl" | "bigr" | "displaystyle" => "",
        "times" => " × ",
        "cdot" => "·",
        "div" => " ÷ ",
        "pm" => "±",
        "le" | "leq" | "leqslant" => " ≤ ",
        "ge" | "geq" | "geqslant" => " ≥ ",
        "ne" | "neq" => " ≠ ",
        "approx" => " ≈ ",
        "sim" => "~",
        "to" | "rightarrow" => " → ",
        "leftarrow" => " ← ",
        "infty" => "∞",
        "circ" => "∘",
        "degree" => "°",
        "percent" => "%",
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "Delta" => "Δ",
        "epsilon" | "varepsilon" => "ε",
        "lambda" => "λ",
        "mu" => "μ",
        "pi" => "π",
        "sigma" => "σ",
        "Sigma" => "Σ",
        "omega" => "ω",
        "Omega" => "Ω",
        "quad" | "qquad" => " ",
        _ => return name,
    };
    symbol.to_string()
}

/// 上標（`up`）或下標：全部字都有對應的上下標字元就換，否則寫成 ^(…) / _(…)。
fn scripted(s: &str, up: bool) -> String {
    let s = s.trim();
    if up && s == "∘" {
        return "°".into();
    }
    let mapped: Option<String> = s.chars().map(|c| if up { superscript(c) } else { subscript(c) }).collect();
    match mapped {
        Some(m) if !m.is_empty() => m,
        _ if s.is_empty() => String::new(),
        _ => {
            let mark = if up { '^' } else { '_' };
            if s.chars().count() == 1 { format!("{mark}{s}") } else { format!("{mark}({s})") }
        }
    }
}

fn superscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' | '−' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        _ => return None,
    })
}

fn subscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '−' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_seen_in_experiment_91() {
        assert_eq!(plain_math("菌數不得小於 $1 \\times 10^6$ 個 [4]。"), "菌數不得小於 1 × 10⁶ 個 [4]。");
        assert_eq!(plain_math("本品所含 $\\text{CO}_2$ 應在 $99.5\\% (\\text{v/v})$ 以上"), "本品所含 CO₂ 應在 99.5% (v/v) 以上");
        assert_eq!(
            plain_math("$\\text{C}_8\\text{H}_{11}\\text{O}_3\\text{N}\\cdot\\text{HCl}$ 按乾品計算"),
            "C₈H₁₁O₃N·HCl 按乾品計算"
        );
        assert_eq!(plain_math("每 $\\text{mL}$ 的 $0.1\\text{ N}$ 過氯酸液相當於 $20.56\\text{ mg}$"), "每 mL 的 0.1 N 過氯酸液相當於 20.56 mg");
        assert_eq!(plain_math("上限為 $\\le 3.0 \\text{ mGy}$ [2]"), "上限為 ≤ 3.0 mGy [2]");
        assert_eq!(plain_math("（$\\Delta AUC$）之下降具統計顯著（$p < 0.05$）"), "（ΔAUC）之下降具統計顯著（p < 0.05）");
    }

    #[test]
    fn other_forms() {
        assert_eq!(plain_math("$$x^{2} + y_{i}$$"), "x² + y_i");
        assert_eq!(plain_math("$10^{-3}$、$25^\\circ\\text{C}$、$\\frac{1}{2}$、$\\frac{a+b}{2}$"), "10⁻³、25°C、1/2、(a+b)/2");
        assert_eq!(plain_math("$\\mu\\text{g}/\\text{kg}$"), "μg/kg");
    }

    #[test]
    fn dollar_amounts_and_unclosed_signs_stay() {
        assert_eq!(plain_math("US$100 和 US$200"), "US$100 和 US$200");
        assert_eq!(plain_math("費用 $5 元，另有 $x^2$"), "費用 $5 元，另有 x²");
        assert_eq!(plain_math("只有一個 $\\times"), "只有一個 $\\times");
        assert_eq!(plain_math("跨行 $a\n= b$"), "跨行 $a\n= b$");
        assert_eq!(plain_math("沒有數學式"), "沒有數學式");
    }

    #[test]
    fn stream_matches_whole_text() {
        let text = "菌數 $1 \\times 10^6$ 個，US$100，$\\text{CO}_2$ 和 $$x^2$$，最後 $ 沒關";
        let whole = plain_math(text);
        for size in 1..8 {
            let mut stream = MathStream::default();
            let mut out = String::new();
            let chars: Vec<char> = text.chars().collect();
            for piece in chars.chunks(size) {
                out.push_str(&stream.push(&piece.iter().collect::<String>()));
            }
            out.push_str(&stream.finish());
            assert_eq!(out, whole, "piece size {size}");
        }
        // 數學式還沒結束時先不送出，結束後一次送出
        let mut stream = MathStream::default();
        assert_eq!(stream.push("約 $1 \\times"), "約 ");
        assert_eq!(stream.push(" 10^6$ 個"), "1 × 10⁶ 個");
    }
}
