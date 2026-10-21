#!/usr/bin/env python3
"""Print digest entries for chosen law folders.

Usage: python3 digest_pick.py <max_files_per_law> <mid_chars> <law_id> [<law_id> ...]
"""
import sys

limit, chars, wanted = int(sys.argv[1]), int(sys.argv[2]), set(sys.argv[3:])
show, files = False, 0
for line in open("/rag/work/text/digest.txt", encoding="utf-8"):
    line = line.rstrip("\n")
    if line.startswith("==="):
        show = line.split()[1] in wanted
        files = 0
        if show:
            print(line[:150])
    elif show and line.startswith("  - "):
        files += 1
        if files <= limit:
            print(line[:120])
    elif show and files <= limit and line.lstrip().startswith("MID:"):
        print("     ", line.strip()[5:5 + chars])
