# Experiment 99: order-insensitive comparison. Per page, a character bigram (after 99_compare.norm, so punctuation and
# spaces are dropped) is "consensus" when at least 3 of the 5 systems contain it. Missing = consensus bigrams a system lacks;
# unique = a system's bigrams that no other system has (mostly misreadings, sometimes text only it read).
# Prints totals and the pages where each system misses the most.
import collections, os, sys
sys.path.insert(0, "/rag/experiments")
from importlib import import_module
c = import_module("99_compare")

def grams(t):
    return collections.Counter(t[i:i + 2] for i in range(len(t) - 1))

# Optional: python3 99_bigram.py <min votes> <system…> (default 3 of all five)
need = int(sys.argv[1]) if len(sys.argv) > 1 else 3
names = sys.argv[2:] or list(c.SYSTEMS)
miss = collections.Counter(); uniq = collections.Counter(); own = collections.Counter(); cons_total = 0
worst = {s: [] for s in names}
for n in range(36):
    g = {s: grams(c.load(s, n)) for s in names}
    votes = collections.Counter(k for s in names for k in g[s])
    cons = {k for k, v in votes.items() if v >= need}
    cons_total += len(cons)
    for s in names:
        m = len(cons - set(g[s]))
        miss[s] += m
        worst[s].append((m, n))
        uniq[s] += sum(1 for k in g[s] if votes[k] == 1)
        own[s] += len(g[s])
print(f"consensus bigrams: {cons_total}")
for s in names:
    top = sorted(worst[s], reverse=True)[:4]
    print(f"{s:9} missing {miss[s]:5} ({100 * miss[s] / cons_total:5.2f}%)  unique {uniq[s]:5} ({100 * uniq[s] / own[s]:5.2f}%)  worst pages "
          + " ".join(f"{n:03d}:{m}" for m, n in top))
