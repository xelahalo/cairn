#!/bin/sh

cd rattle || exit
cabal update
cabal install --overwrite-policy=always

script_dir="$(pwd)/../"

"$HOME"/.cabal/bin/rattle-benchmark fsatrace redis micro --script-dir "$script_dir"

