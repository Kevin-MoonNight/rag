#!/bin/bash
# Stop any running rag program and llama-server inside the container.
pkill -f "app-linux/rag " 2>/dev/null
sleep 1
pkill -f "linux-p4/llama-server" 2>/dev/null
sleep 1
ps -eo etime,args | grep -E "app-linux/rag |llama-server" | grep -v grep | cut -c1-100
echo "stopped"
