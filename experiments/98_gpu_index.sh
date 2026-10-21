#!/bin/bash
# 實驗 98：干擾文件改用 GPU 建索引（只為了省時間；建索引的速度前面已經量過，這裡不量）。
# 在 CUDA 容器裡執行：
#   docker run --rm --gpus all -v "<rag 資料夾>:/rag" nvidia/cuda:12.6.3-devel-ubuntu24.04 bash /rag/experiments/98_gpu_index.sh
# 向量模型和影像編碼器跑在 GPU 上（bin/linux-p5-cuda/llama-server-gpu）；之後的評估（查詢）仍用 CPU 版，和基準相同。
# 做完留下 work/index-98/.done-index-scale，98_scale.sh 就會跳過建索引、直接評估。
cd /rag
data=work/index-98
out=results/98/index-scale.txt
t0=$(date +%s)
./bin/app-linux/rag index work/scale/pdf --root /rag --data $data --server bin/linux-p5-cuda/llama-server-gpu --quiet > $out 2>&1
code=$?
echo "exit=$code seconds=$(( $(date +%s) - t0 )) (GPU: RTX 3060)" >> $out
[ $code = 0 ] && touch $data/.done-index-scale
tail -3 $out
