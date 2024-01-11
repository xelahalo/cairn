#!/bin/sh

cd rattle
cabal update
cabal install --overwrite-policy=always

script_dir="$(pwd)/../"

"$HOME"/.local/bin/rattle-benchmark fsatrace redis micro --script-dir "$script_dir"

