#!/bin/bash
# Stop any running index, run the tests, rebuild both binaries, then index the real PDFs from scratch.
# Usage: bash 55_rebuild_index.sh <tag> [extra rag index args]
tag=$1; shift
bash /rag/experiments/stop_rag.sh
bash /rag/experiments/build.sh test || exit 1
bash /rag/experiments/build.sh linux || exit 1
bash /rag/experiments/build.sh windows || exit 1
rm -rf "/rag/work/index-$tag"
bash /rag/experiments/54_index_real.sh "$tag" "$@"
