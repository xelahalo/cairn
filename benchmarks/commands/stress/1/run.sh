#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <n>"
    exit 1
fi

n="$1"

for ((i = 1; i <= n; i++)); do
    gcc donut.c
done

echo "Done running command: gcc donut.c (iterations: $n)"