#!/bin/bash
# Build the Linux release binary first (needed for experiments), then run the unit tests.
rm -f /rag/work/build-linux.log /rag/work/build-test.log
bash /rag/experiments/build.sh linux
grep -B2 -A12 "^error" /rag/work/build-linux.log | head -60
ls -la /rag/bin/app-linux/
bash /rag/experiments/build.sh test
grep -E "^test result|FAILED|panicked|^test .* FAILED|^error" -A3 /rag/work/build-test.log | head -60
