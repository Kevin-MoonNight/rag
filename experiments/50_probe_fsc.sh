#!/bin/bash
# Probe the FSC law system: open-data XML files and per-law download links.
set -u
mkdir -p /rag/testdata/realdocs/raw /rag/testdata/realdocs/probe
cd /rag/testdata/realdocs/raw
UA="Mozilla/5.0 (X11; Linux x86_64) rag-eval-fetch"
for name in 本會 銀行局 證券期貨局 保險局 檢查局; do
  if [ ! -s "fsc_${name}.xml" ]; then
    enc=$(python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))" "$name")
    curl -sL -m 300 -A "$UA" -o "fsc_${name}.xml" "https://law.fsc.gov.tw/opendata/${enc}.xml"
    sleep 2
  fi
  ls -la "fsc_${name}.xml"
  head -c 300 "fsc_${name}.xml"; echo
done
cd /rag/testdata/realdocs/probe
echo "== links in category page"
grep -o 'href="[^"]*"' cat.html | sort -u | head -80
