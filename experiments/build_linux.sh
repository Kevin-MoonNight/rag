#!/usr/bin/env bash
# 在容器內編譯 llama.cpp（Linux 版）。
# 用法：bash build_linux.sh <原始碼目錄> <輸出名稱>
#   例：bash build_linux.sh /rag/vendor/llama.cpp-b11567 stock
# 編譯中間檔放在容器內的 /build/<名稱>（9p 掛載太慢），完成後把執行檔複製到 /rag/bin/linux-<名稱>。
set -euo pipefail
SRC=$1
NAME=$2
BUILD=/build/$NAME
OUT=/rag/bin/linux-$NAME
cmake -S "$SRC" -B "$BUILD" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=OFF \
    -DGGML_NATIVE=OFF -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON -DGGML_BMI2=ON \
    -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=ON \
    -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
    > "$BUILD.cmake.log" 2>&1 || { tail -40 "$BUILD.cmake.log"; exit 1; }
TARGETS=${TARGETS:-"llama-bench llama-server llama-cli llama-completion llama-mtmd-cli llama-tokenize llama-quantize llama-embedding"}
cmake --build "$BUILD" --target $TARGETS -j "$(nproc)" > "$BUILD.build.log" 2>&1 || { tail -60 "$BUILD.build.log"; exit 1; }
mkdir -p "$OUT"
for t in $TARGETS; do cp "$BUILD/bin/$t" "$OUT/"; done
ls -la "$OUT"
