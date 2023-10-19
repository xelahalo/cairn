#!/bin/sh

git checkout results
git add benchmarks/*
git commit -m "Add results for benchmark $(git log -1 --pretty=%h)"
git push origin
