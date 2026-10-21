#!/bin/bash
# Build the Rust workspace inside the rag-rust container.
# Usage: bash build.sh check|test|linux|windows [extra cargo args]
# Output goes to /rag/work/build-<mode>.log; binaries are copied to /rag/bin/app-<platform>/, with the PDFium and
# ONNX Runtime libraries (Microsoft 1.28.0 release in /rag/work/onnxruntime, loaded at run time for OCR).
set -u
mode=$1; shift
cd /rag/app
# keep downloaded build inputs (PDFium) inside the project directory
export XDG_CACHE_HOME=/rag/work/cache
export LOCALAPPDATA=/rag/work/cache
log=/rag/work/build-$mode.log
case "$mode" in
check)
    cargo check --workspace --exclude rag_desktop "$@" > "$log" 2>&1 ;;
test)
    cargo test --workspace --exclude rag_desktop --no-fail-fast "$@" > "$log" 2>&1 ;;
linux)
    cargo build --release -p rag_cli "$@" > "$log" 2>&1 \
      && mkdir -p /rag/bin/app-linux \
      && cp /build/target/release/rag /rag/bin/app-linux/ \
      && find /rag/work/cache -name "libpdfium.so" -path "*linux-x64*" -exec cp {} /rag/bin/app-linux/ \; \
      && cp /rag/work/onnxruntime/linux/libonnxruntime.so.1.28.0 /rag/bin/app-linux/ ;;
windows)
    cargo build --release -p rag_cli --target x86_64-pc-windows-gnu "$@" > "$log" 2>&1 \
      && mkdir -p /rag/bin/app-win \
      && cp /build/target/x86_64-pc-windows-gnu/release/rag.exe /rag/bin/app-win/ \
      && find /rag/work/cache -name "pdfium.dll" -path "*win-x64*" -exec cp {} /rag/bin/app-win/ \; \
      && cp /rag/work/onnxruntime/win/onnxruntime.dll /rag/bin/app-win/ ;;
desktop-check)
    cargo check -p rag_desktop --target x86_64-pc-windows-gnu --features custom-protocol "$@" > "$log" 2>&1 ;;
desktop)
    # the frontend must already be built into /rag/app/desktop/dist
    cargo build --release -p rag_desktop --target x86_64-pc-windows-gnu --features custom-protocol "$@" > "$log" 2>&1 \
      && mkdir -p /rag/bin/app-win \
      && cp /build/target/x86_64-pc-windows-gnu/release/rag-desktop.exe /rag/bin/app-win/ \
      && find /build/target/x86_64-pc-windows-gnu/release -maxdepth 1 -name "WebView2Loader.dll" -exec cp {} /rag/bin/app-win/ \; \
      && cp /rag/work/onnxruntime/win/onnxruntime.dll /rag/bin/app-win/ ;;
esac
code=$?
echo "exit=$code" >> "$log"
grep -E "^(warning: unused|error)" "$log" | sort | uniq -c | head -5
tail -4 "$log"
exit $code
