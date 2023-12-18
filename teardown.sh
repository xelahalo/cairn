#!/bin/bash

docker container kill build-env
docker container kill build-env-bench

cd host_mnt
./clean.sh
cd ..
