"""Print sampled chunks from an index (SQLite) so evaluation questions can be written from real content.

Usage: python3 -I dump_chunks.py <data_dir> <n_files> [seed] [chars]
       python3 -I dump_chunks.py <data_dir> file <path substring> [chars]
       python3 -I dump_chunks.py <data_dir> summary
"""
import random
import sqlite3
import sys

data = sys.argv[1]
db = sqlite3.connect(f"{data}/index.sqlite")


def show(file_id, path, pages, chars, max_chunks):
    rows = db.execute("SELECT kind, page, chunk_index, heading, text, image_path FROM chunks WHERE file_id=? ORDER BY kind, chunk_index", (file_id,)).fetchall()
    rel = path.split("/realdocs/")[-1]
    print(f"\n##### {rel}  pages={pages} chunks={len(rows)}")
    step = max(1, len(rows) // max_chunks)
    for kind, page, idx, heading, text, image in rows[::step][:max_chunks]:
        if kind == 1:
            print(f"  [p{page} IMAGE] {image}")
        else:
            body = (text or "").replace("\n", " / ")
            print(f"  [p{page} #{idx} h={heading!r} len={len(text or '')}] {body[:chars]}")


if sys.argv[2] == "summary":
    print("files by status:", db.execute("SELECT status, COUNT(*) FROM files GROUP BY status").fetchall())
    print("chunks by kind:", db.execute("SELECT kind, COUNT(*) FROM chunks GROUP BY kind").fetchall())
    lens = sorted(r[0] for r in db.execute("SELECT LENGTH(text) FROM chunks WHERE kind=0"))
    if lens:
        print("text chunk chars: median", lens[len(lens) // 2], "p10", lens[len(lens) // 10], "p90", lens[len(lens) * 9 // 10], "max", lens[-1])
    print("files with image pages:", db.execute("SELECT COUNT(DISTINCT file_id) FROM chunks WHERE kind=1").fetchone())
    print("files with zero chunks:", db.execute("SELECT COUNT(*) FROM files f WHERE status='indexed' AND NOT EXISTS (SELECT 1 FROM chunks c WHERE c.file_id=f.id)").fetchone())
    for row in db.execute("SELECT path, error FROM files WHERE status='failed' LIMIT 10"):
        print("failed:", row)
elif sys.argv[2] == "short":
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    rows = db.execute("SELECT LENGTH(c.text), c.page, c.chunk_index, c.text, f.path FROM chunks c JOIN files f ON f.id=c.file_id WHERE c.kind=0 AND LENGTH(c.text) < ? ORDER BY 1", (limit,)).fetchall()
    print(len(rows), "text chunks shorter than", limit, "chars")
    for n, page, idx, text, path in rows[:60]:
        print(f"  {n:3d} p{page} #{idx} {text.replace(chr(10), ' / ')!r}  <- {path.split('/')[-1][:50]}")
elif sys.argv[2] == "file":
    chars = int(sys.argv[4]) if len(sys.argv) > 4 else 2000
    for file_id, path, pages in db.execute("SELECT id, path, pages FROM files WHERE path LIKE ?", (f"%{sys.argv[3]}%",)):
        show(file_id, path, pages, chars, 200)
else:
    n = int(sys.argv[2])
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    chars = int(sys.argv[4]) if len(sys.argv) > 4 else 500
    files = db.execute("SELECT id, path, pages FROM files WHERE status='indexed' ORDER BY path").fetchall()
    random.Random(seed).shuffle(files)
    for file_id, path, pages in files[:n]:
        show(file_id, path, pages, chars, 3)
