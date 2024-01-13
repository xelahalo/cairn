#!/bin/bash

cd rattle || exit
cabal update
cabal install --overwrite-policy=always --installdir="$HOME"/.cabal/bin

script_dir="$(pwd)/../"

"$HOME"/.cabal/bin/rattle-benchmark fsatrace --script-dir "$script_dir"
