# Experiment 96: build one fixed answer prompt (five consecutive text chunks of one real PDF and a question)
# from an index, so every load scenario sends the same request.
# Usage: python3 experiments/96_prompt.py work/index-win-1bit/index.sqlite work/win-args/96_prompt.json
import json
import sqlite3
import sys

db = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
row = db.execute(
    "select f.id, f.path from files f join chunks c on c.file_id = f.id where c.kind = 0 "
    "group by f.id having count(*) >= 8 order by f.id limit 1"
).fetchone()
texts = [t for (t,) in db.execute("select c.text from chunks c where c.file_id = ? and c.kind = 0 order by c.page, c.chunk_index limit 5", (row[0],))]
passages = "\n\n".join(f"[{i + 1}] {t}" for i, t in enumerate(texts))
system = "你是文件搜尋助理。只根據提供的資料回答，用繁體中文，簡短回答，並標出引用的段落編號。"
user = f"資料：\n{passages}\n\n問題：這份文件的重點是什麼？"
json.dump({"system": system, "user": user}, open(sys.argv[2], "w", encoding="utf-8"), ensure_ascii=False)
print(row[1], len(user), "chars")
