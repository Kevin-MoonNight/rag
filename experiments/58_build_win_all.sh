#!/bin/bash
# Run the tests, then build both Windows binaries: the CLI and the desktop app.
# (Does not touch the Linux binary, so a running Linux index job is not disturbed.)
bash /rag/experiments/build.sh test || exit 1
bash /rag/experiments/build.sh windows || exit 1
bash /rag/experiments/build.sh desktop
