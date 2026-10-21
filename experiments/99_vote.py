# Experiment 99: two-against-one adjudication of small disagreements (spans up to 3 characters on each side) between
# system A and B: the version that also appears in system C (with 3 characters of context on both sides) wins.
# Usage: python3 99_vote.py A B C   -> counts of spans where A is outvoted, B is outvoted, or C agrees with neither
import difflib, sys
sys.path.insert(0, "/rag/experiments")
from importlib import import_module
c = import_module("99_compare")
a, b, ref = sys.argv[1:4]
wins = {a: [], b: []}; none = 0
for n in range(36):
    x, y, z = c.load(a, n), c.load(b, n), c.load(ref, n)
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, x, y, autojunk=False).get_opcodes():
        if tag == "equal" or i2 - i1 > 3 or j2 - j1 > 3:
            continue
        left, right = x[max(0, i1 - 3):i1], x[i2:i2 + 3]
        va, vb = left + x[i1:i2] + right, left + y[j1:j2] + right
        if va in z and vb not in z:
            wins[a].append(f"{n:03d}:{x[i1:i2]}>{y[j1:j2]}")   # A right, B wrong
        elif vb in z and va not in z:
            wins[b].append(f"{n:03d}:{y[j1:j2]}>{x[i1:i2]}")   # B right, A wrong
        else:
            none += 1
print(f"{b} wrong {len(wins[a])}: {' '.join(wins[a])}")
print(f"{a} wrong {len(wins[b])}: {' '.join(wins[b])}")
print(f"undecided {none}")
