#!/usr/bin/env bash
# 實驗用：編 CUDA 版的 llama-server（只為了加快實驗的建索引；產品一律用 CPU）。
# 在 nvidia/cuda:12.6.3-devel-ubuntu24.04 容器裡執行：
#   docker run --rm --gpus all -v "<rag 資料夾>:/rag" nvidia/cuda:12.6.3-devel-ubuntu24.04 bash /rag/experiments/build_linux_cuda.sh
# 原始碼 /rag/work/llama.cpp（含全部修改），只編 RTX 3060 的架構（sm_86），輸出到 /rag/bin/linux-p5-cuda。
# 另外產生 llama-server-gpu：在參數最後加上 -ngl 99（程式固定帶 -ngl 0，後面的設定會蓋掉前面的）。
set -euo pipefail
apt-get update -qq && apt-get install -y -qq cmake ninja-build g++ > /dev/null
BUILD=/build/cuda
OUT=/rag/bin/linux-p5-cuda
cmake -S /rag/work/llama.cpp -B $BUILD -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
    -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=86 -DGGML_NATIVE=OFF -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON \
    -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_TESTS=OFF > /rag/work/build-cuda.log 2>&1
cmake --build $BUILD --target llama-server -j "$(nproc)" >> /rag/work/build-cuda.log 2>&1
mkdir -p $OUT
cp $BUILD/bin/llama-server $OUT/
cat > $OUT/llama-server-gpu <<'EOF'
#!/bin/sh
exec "$(dirname "$0")/llama-server" "$@" -ngl 99
EOF
chmod +x $OUT/llama-server-gpu
ls -la $OUT
