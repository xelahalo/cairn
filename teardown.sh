#!/bin/bash

docker container kill build-env

cd host_mnt
./clean.sh
cd ..

