#!/usr/bin/env python3
"""向 llama-server 取嵌入向量的共用函式，以及簡單的冒煙測試。

用法：python3 -I embed_client.py <port>
"""
import base64
import json
import sys
import time
import urllib.request

import numpy as np

QUERY_PREFIX = "task: search result | query: "
DOC_PREFIX = "title: none | text: "


def post(port: int, path: str, payload: dict, timeout: float = 600.0):
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def _vec(item) -> np.ndarray:
    emb = item["embedding"]
    if emb and isinstance(emb[0], list):
        emb = emb[0]
    return np.asarray(emb, dtype=np.float32)


def embed_texts(port: int, texts: list[str]) -> np.ndarray:
    """/embedding 端點（非 OpenAI 相容版），一次送一批文字。"""
    res = post(port, "/embedding", {"content": texts})
    return np.stack([_vec(r) for r in sorted(res, key=lambda r: r["index"])])


def embed_image(port: int, image_bytes: bytes, text: str = "<__media__>") -> np.ndarray:
    """送一張圖片（可夾文字），回傳一個向量。<__media__> 是 llama-server 的圖片位置標記。"""
    b64 = base64.b64encode(image_bytes).decode("ascii")
    res = post(port, "/embedding", {"content": {"prompt_string": text, "multimodal_data": [b64]}})
    if isinstance(res, list):
        res = res[0]
    return _vec(res)


def cos(a: np.ndarray, b: np.ndarray) -> float:
    return float(np.dot(a, b) / (np.linalg.norm(a) * np.linalg.norm(b)))


def main() -> None:
    port = int(sys.argv[1])
    queries = ["貸款超過五百萬元需要幾位授信人員審查？", "How long are transaction records kept for corporate accounts?"]
    docs = [
        "本行內部稽核規定，超過五百萬元的貸款申請須經兩位授信人員分別審查後方可核准。",
        "Transaction records are retained for seven years for corporate accounts and five years for personal accounts.",
        "病人入住心臟內科病房後，須於二十四小時內完成用藥核對，如有差異應通報主治醫師。",
        "今天台北的天氣晴朗，氣溫攝氏二十八度，適合到陽明山健行。",
    ]
    t0 = time.time()
    q = embed_texts(port, [QUERY_PREFIX + s for s in queries])
    d = embed_texts(port, [DOC_PREFIX + s for s in docs])
    dt = time.time() - t0
    print(f"dim={q.shape[1]} norm={np.linalg.norm(q[0]):.4f} time={dt*1000:.0f} ms for {len(queries)+len(docs)} texts")
    for i, qs in enumerate(queries):
        sims = [cos(q[i], d[j]) for j in range(len(docs))]
        print(f"Q{i}: " + "  ".join(f"D{j}={s:.3f}" for j, s in enumerate(sims)) + f"  -> best D{int(np.argmax(sims))}")


if __name__ == "__main__":
    main()
