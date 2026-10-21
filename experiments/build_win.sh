#!/usr/bin/env bash
# 在容器內用 MinGW 交叉編譯 Windows 版 llama.cpp（x64，靜態連結，不需要額外 DLL）。
# 用法：bash build_win.sh <原始碼目錄> <輸出名稱>
#   例：bash build_win.sh /rag/work/llama.cpp p2   ->  /rag/bin/win-p2/*.exe
set -euo pipefail
SRC=$1
NAME=$2
BUILD=/build/win-$NAME
OUT=/rag/bin/win-$NAME
mkdir -p /build
cat > /build/mingw-toolchain.cmake <<'EOF'
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR x86_64)
set(CMAKE_C_COMPILER x86_64-w64-mingw32-gcc-posix)
set(CMAKE_CXX_COMPILER x86_64-w64-mingw32-g++-posix)
set(CMAKE_RC_COMPILER x86_64-w64-mingw32-windres)
set(CMAKE_FIND_ROOT_PATH /usr/x86_64-w64-mingw32)
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
EOF
# Ubuntu 24.04 內附的 MinGW 標頭檔比較舊，缺少 llama.cpp 用到的「不要降速這個執行緒」相關定義。
# 這裡用強制引入的標頭檔補上（數值取自 Windows SDK），不修改 llama.cpp 原始碼。
cat > /build/mingw-compat.h <<'EOF'
#include <windows.h>
#ifndef THREAD_POWER_THROTTLING_CURRENT_VERSION
typedef struct _THREAD_POWER_THROTTLING_STATE {
    ULONG Version;
    ULONG ControlMask;
    ULONG StateMask;
} THREAD_POWER_THROTTLING_STATE;
#define THREAD_POWER_THROTTLING_CURRENT_VERSION 1
#define THREAD_POWER_THROTTLING_EXECUTION_SPEED 0x1
#endif
EOF
cmake -S "$SRC" -B "$BUILD" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE=/build/mingw-toolchain.cmake \
    -DCMAKE_C_FLAGS="-include /build/mingw-compat.h" \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=OFF \
    -DCMAKE_EXE_LINKER_FLAGS="-static" \
    -DGGML_NATIVE=OFF -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON -DGGML_BMI2=ON \
    -DGGML_OPENMP=${OPENMP:-OFF} \
    -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=ON \
    -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
    > "$BUILD.cmake.log" 2>&1 || { tail -40 "$BUILD.cmake.log"; exit 1; }
TARGETS=${TARGETS:-"llama-bench llama-server"}
cmake --build "$BUILD" --target $TARGETS -j "$(nproc)" > "$BUILD.build.log" 2>&1 || { grep -E 'error|Error|undefined' "$BUILD.build.log" | head -30; tail -15 "$BUILD.build.log"; exit 1; }
mkdir -p "$OUT"
for t in $TARGETS; do cp "$BUILD/bin/$t.exe" "$OUT/"; done
# 預設不用 OpenMP：MinGW 附的 libgomp 在 Windows 上讓逐 token 產生的速度掉到一半（實測 24 -> 13 token/s），
# llama.cpp 內建的執行緒池沒有這個問題。OPENMP=ON 時要把執行期 DLL 放到執行檔旁邊。
if [ "${OPENMP:-OFF}" = ON ]; then
    cp /usr/lib/gcc/x86_64-w64-mingw32/13-posix/libgomp-1.dll /usr/lib/gcc/x86_64-w64-mingw32/13-posix/libgcc_s_seh-1.dll /usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll "$OUT/"
fi
ls -la "$OUT"
x86_64-w64-mingw32-objdump -p "$OUT/llama-server.exe" | grep 'DLL Name' | sort -u
