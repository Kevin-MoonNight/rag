#!/usr/bin/env python3
"""測試 E2B 能不能讀懂以圖片形式提供的頁面（不做 OCR 的前提是否成立）。

對每一頁（乾淨頁、模擬掃描頁）做兩件事：
  1. 逐字抄錄：和原文比對，算字元層級的相似度。
  2. 問答：只給圖片和問題，檢查回答裡有沒有正確答案的關鍵字。
另外用「文字版」當對照組（同樣的問題，把頁面文字直接放進提示）。

用法：python3 -I e2b_read_pages.py <port> <輸出.json> [pattern 例如 page_??.png]
"""
import base64
import difflib
import glob
import io
import json
import os
import re
import sys
import time

sys.path.insert(0, "/rag/experiments")
from embed_client import post  # noqa: E402

# 每個查詢的正確答案關鍵字（任一組全部出現就算對）
ANSWERS = {
    0: [["兩位"], ["二位"], ["2位"], ["2 位"]],
    1: [["六個月"], ["6個月"], ["6 個月"]],
    2: [["七年"], ["7年"], ["7 年"]],
    3: [["十年"], ["10年"], ["10 年"]],
    4: [["二十四小時"], ["24小時"], ["24 小時"]],
    5: [["暫停"]],
    6: [["30 minutes"], ["thirty minutes"], ["30 分鐘"], ["30分鐘"]],
    7: [["24"], ["二十四"]],
    8: [["全額"], ["可以"]],
    9: [["二十八"], ["28"]],
    10: [["15"], ["fifteen"], ["十五"]],
    11: [["department head"], ["部門主管"], ["主管"]],
}


def chat(port: int, content, max_tokens: int = 400) -> tuple[str, dict]:
    res = post(
        port,
        "/v1/chat/completions",
        {
            "messages": [{"role": "user", "content": content}],
            "temperature": 0,
            "max_tokens": max_tokens,
            "cache_prompt": False,
        },
    )
    return res["choices"][0]["message"]["content"], res.get("timings", {})


TILES = int(os.environ.get("TILES", "1"))  # 把一頁由上到下切成幾條橫條，各自當一張圖片送
TILE_OVERLAP = 56  # 相鄰橫條重疊的像素（略大於一行字高），避免一行字被切成兩半而兩邊都讀不到


def image_parts(path: str) -> list[dict]:
    if TILES <= 1:
        mime = "image/png" if path.endswith(".png") else "image/jpeg"
        b64 = base64.b64encode(open(path, "rb").read()).decode("ascii")
        return [{"type": "image_url", "image_url": {"url": f"data:{mime};base64,{b64}"}}]
    from PIL import Image

    img = Image.open(path).convert("RGB")
    w, h = img.size
    step = h / TILES
    parts = []
    for i in range(TILES):
        top = max(0, int(i * step) - (TILE_OVERLAP // 2 if i else 0))
        bottom = min(h, int((i + 1) * step) + (TILE_OVERLAP // 2 if i < TILES - 1 else 0))
        buf = io.BytesIO()
        img.crop((0, top, w, bottom)).save(buf, format="PNG")
        b64 = base64.b64encode(buf.getvalue()).decode("ascii")
        parts.append({"type": "image_url", "image_url": {"url": f"data:image/png;base64,{b64}"}})
    return parts


PAGE_NOTE = "" if TILES <= 1 else f"這 {TILES} 張圖片是同一頁文件由上到下切成的橫條，相鄰兩張有少許重疊。"
# 專用的文件辨識模型各有固定的指令，用環境變數換掉
TRANSCRIBE_PROMPT = os.environ.get("TRANSCRIBE_PROMPT") or (PAGE_NOTE + "請逐字抄錄圖片中的所有文字，不要翻譯、不要摘要、不要加任何說明。")


def norm(s: str) -> str:
    return re.sub(r"[\s　*#>\-`|]+", "", s)


def correct(qi: int, answer: str) -> bool:
    a = answer.lower()
    return any(all(k.lower() in a for k in group) for group in ANSWERS[qi])


def main() -> None:
    port = int(sys.argv[1])
    out_path = sys.argv[2]
    pattern = sys.argv[3] if len(sys.argv) > 3 else "page_??.png"

    testdir = os.environ.get("TESTDIR", "/rag/testdata")
    data = json.load(open(f"{testdir}/texts.json", encoding="utf-8"))
    docs, queries = data["docs"], data["queries"]
    results = {"transcribe": [], "qa_image": [], "qa_text": []}

    paths = sorted(glob.glob(f"{testdir}/pages/{pattern}"))
    for path in paths:
        di = int(re.search(r"page_(\d+)", path).group(1))
        truth = docs[di]["text"]

        t0 = time.time()
        text, tm = chat(port, [*image_parts(path), {"type": "text", "text": TRANSCRIBE_PROMPT}], int(os.environ.get("MAX_TOKENS", "700")))
        ratio = difflib.SequenceMatcher(None, norm(truth), norm(text), autojunk=False).ratio()
        results["transcribe"].append({"page": path, "ratio": ratio, "text": text, "timings": tm, "sec": time.time() - t0})
        print(f"[transcribe] {path.split('/')[-1]}: similarity {ratio:.3f}  prompt {tm.get('prompt_n')} tok {tm.get('prompt_ms', 0)/1000:.1f}s, gen {tm.get('predicted_n')} tok {tm.get('predicted_per_second', 0):.1f} t/s")
        print("    " + text.replace("\n", " ")[:160])

        for qi, q in enumerate(queries):
            if q["doc"] != di or os.environ.get("SKIP_IMAGE_QA") == "1":
                continue
            ans, tm = chat(port, [*image_parts(path), {"type": "text", "text": f"{PAGE_NOTE}請只根據圖片的內容，用繁體中文簡短回答：{q['text']}"}], 200)
            ok = correct(qi, ans)
            results["qa_image"].append({"page": path, "q": q["text"], "answer": ans, "ok": ok, "timings": tm})
            print(f"[qa image] {'OK  ' if ok else 'MISS'} Q{qi} {q['text']} -> {ans.replace(chr(10), ' ')[:110]}")

    # 對照組：同樣的問題，給文字版
    for qi, q in enumerate(queries):
        if os.environ.get("SKIP_TEXT_QA") == "1":
            break
        ans, tm = chat(port, f"以下是文件內容：\n\n{docs[q['doc']]['text']}\n\n請只根據上面的內容，用繁體中文簡短回答：{q['text']}", 200)
        ok = correct(qi, ans)
        results["qa_text"].append({"q": q["text"], "answer": ans, "ok": ok, "timings": tm})
        print(f"[qa text ] {'OK  ' if ok else 'MISS'} Q{qi} {q['text']} -> {ans.replace(chr(10), ' ')[:110]}")

    n_img = len(results["qa_image"])
    print(f"\nSUMMARY {pattern}: transcription similarity avg {sum(r['ratio'] for r in results['transcribe'])/len(paths):.3f} "
          f"(min {min(r['ratio'] for r in results['transcribe']):.3f}); "
          f"QA from image {sum(r['ok'] for r in results['qa_image'])}/{n_img}; QA from text {sum(r['ok'] for r in results['qa_text'])}/{len(results['qa_text'])}")
    json.dump(results, open(out_path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
