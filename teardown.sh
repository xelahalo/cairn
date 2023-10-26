#!/bin/bash

docker container kill build-env

cd mnt
./clean.sh
cd ..

