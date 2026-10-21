#!/bin/sh
# Build the desktop app's web frontend. Run inside the node container:
#   docker run --rm -v "<rag dir>:/rag" -w /rag/app/desktop node:22-bookworm-slim sh /rag/experiments/build_frontend.sh
# The npm cache stays inside the project directory.
export npm_config_cache=/rag/work/npm-cache
[ -d node_modules ] || npm install --no-audit --no-fund
npm run build
