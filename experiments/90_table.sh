#!/bin/sh
# Experiment 90: turn results/win/90/summary.txt into one row per configuration (average of the runs, MB):
# total WebView2 private memory at each point, plus the GPU process and renderer at "later".
awk '
/^\xef\xbb\xbf?=====|^=====/ { label = $2; order[++n] = label; next }
/^run [0-9]+ (idle|search|later|min|back) / {
  point = $3; total[label, point] += $5; runs[label, point]++
  if (point == "later") {
    if (match($0, /wv2 gpu-process=[0-9]+/)) { gpu[label] += substr($0, RSTART + 16, RLENGTH - 16); gn[label]++ }
    if (match($0, /wv2 renderer=[0-9]+/)) { ren[label] += substr($0, RSTART + 13, RLENGTH - 13); rn[label]++ }
  }
}
END {
  printf "%-14s %6s %6s %6s %6s %6s | %6s %8s\n", "config", "idle", "search", "later", "min", "back", "gpu@l", "rend@l"
  for (i = 1; i <= n; i++) {
    l = order[i]; if (!runs[l, "idle"]) continue
    printf "%-14s", l
    split("idle search later min back", p, " ")
    for (j = 1; j <= 5; j++) printf " %6.0f", runs[l, p[j]] ? total[l, p[j]] / runs[l, p[j]] : 0
    printf " | %6.0f %8.0f\n", gn[l] ? gpu[l] / gn[l] : 0, rn[l] ? ren[l] / rn[l] : 0
  }
}' results/win/90/summary.txt
