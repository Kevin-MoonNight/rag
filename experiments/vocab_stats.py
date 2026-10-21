#!/usr/bin/env python3
"""統計 GGUF 詞彙表裡各種文字的 token 數，估算「輸出層只保留中英文」能縮多少。

用法：python3 -I vocab_stats.py <model.gguf> [--dump <輸出 keep-list 檔>]

分類規則（每個 token 取「最需要排除」的那一類）：
  special   控制 / 特殊 token（依 GGUF 的 token_type，或 <...> 形式）
  byte      位元組備援 token
  other     含有中英文以外的文字（日文假名、韓文、西里爾、阿拉伯、天城文、泰文……）
  latin_ext 含有帶重音符號等非 ASCII 的拉丁字母
  han_sc    含有 Big5 編不出來的漢字（簡體字、日本新字體、罕用字）
  han_tc    含有漢字，且全部能用 Big5 編碼（繁體或繁簡共用）
  ascii     只有 ASCII 字母、數字、標點、空白
  symbol    其餘（全形標點、數學符號、表情符號等）
"""
import sys
import unicodedata
from collections import Counter

sys.path.insert(0, "/rag/vendor/llama.cpp-b11567/gguf-py")
from gguf import GGUFReader  # noqa: E402

LATIN_OK = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")


def is_han(ch: str) -> bool:
    o = ord(ch)
    return (0x4E00 <= o <= 0x9FFF or 0x3400 <= o <= 0x4DBF or 0x20000 <= o <= 0x2FA1F or 0xF900 <= o <= 0xFAFF)


def big5_ok(ch: str) -> bool:
    try:
        ch.encode("big5hkscs")
        return True
    except UnicodeEncodeError:
        return False


def classify(tok: str, ttype: int) -> str:
    # token_type: 1 normal, 2 unknown, 3 control, 4 user defined, 5 unused, 6 byte
    if ttype == 6:
        return "byte"
    if ttype in (2, 3, 5):
        return "special"
    text = tok.replace("▁", " ")
    if ttype == 4 and text.startswith("<") and text.endswith(">"):
        return "special"
    cats = set()
    for ch in text:
        if ch.isascii():
            cats.add("ascii")
            continue
        if is_han(ch):
            cats.add("han_tc" if big5_ok(ch) else "han_sc")
            continue
        cat = unicodedata.category(ch)
        if cat[0] == "L" or cat[0] == "M":
            name = unicodedata.name(ch, "")
            if name.startswith("LATIN") or name.startswith("FULLWIDTH LATIN"):
                cats.add("latin_ext")
            else:
                cats.add("other")
        elif cat[0] == "N":
            name = unicodedata.name(ch, "")
            if name.startswith(("FULLWIDTH DIGIT", "CIRCLED", "PARENTHESIZED", "SUPERSCRIPT", "SUBSCRIPT", "ROMAN", "VULGAR", "DIGIT")):
                cats.add("symbol")
            else:
                cats.add("other")
        else:
            cats.add("symbol")
    for c in ("other", "latin_ext", "han_sc", "han_tc"):
        if c in cats:
            return c
    if cats == {"ascii"} or not cats:
        return "ascii"
    return "symbol"


def main() -> None:
    path = sys.argv[1]
    dump = sys.argv[sys.argv.index("--dump") + 1] if "--dump" in sys.argv else None
    r = GGUFReader(path)
    tokens = r.fields["tokenizer.ggml.tokens"].contents()
    types = r.fields["tokenizer.ggml.token_type"].contents()
    n = len(tokens)

    counts = Counter()
    cls = []
    for tok, tt in zip(tokens, types):
        c = classify(tok, int(tt))
        counts[c] += 1
        cls.append(c)

    print(f"vocab size = {n}")
    order = ["special", "byte", "ascii", "symbol", "han_tc", "han_sc", "latin_ext", "other"]
    for c in order:
        print(f"  {c:10s} {counts[c]:7d}  {counts[c]/n*100:5.1f}%")

    def keep_size(keep: set) -> int:
        return sum(counts[c] for c in keep)

    base = {"special", "byte", "ascii", "symbol", "han_tc"}
    for label, keep in [
        ("繁體中文 + 英文（含符號、特殊、位元組）", base),
        ("再加上簡體等其他漢字", base | {"han_sc"}),
        ("再加上帶重音的拉丁字母", base | {"han_sc", "latin_ext"}),
    ]:
        k = keep_size(keep)
        print(f"  keep {k:7d} = {k/n*100:5.1f}%   {label}")

    # 每一類印幾個例子，方便人工檢查分類是否合理
    for c in order:
        ex = [repr(tokens[i]) for i in range(n) if cls[i] == c][1000:1012] or [repr(tokens[i]) for i in range(n) if cls[i] == c][:12]
        print(f"  e.g. {c:10s} {' '.join(ex)}")

    if dump:
        with open(dump, "w", encoding="utf-8") as f:
            for i in range(n):
                f.write(f"{i}\t{cls[i]}\n")


if __name__ == "__main__":
    main()
