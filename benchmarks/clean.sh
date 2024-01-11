#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <folder>"
    exit 1
fi

folder="$1"

find "$folder" -mindepth 1 -maxdepth 1 \
  ! -name bin ! -name dev ! -name etc ! -name 'lib*' ! -name proc ! -name sys ! -name usr ! -name tracer.log \
  -exec rm -r {} +
