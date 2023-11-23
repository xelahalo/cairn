#!/bin/bash

# start the tracer
cairn-fuse /usr/src/dockermount /usr/src/fusemount & disown

# mount relevant dirs
for f in proc sys dev bin etc lib lib32 lib64 libx32 usr/lib usr/lib32 usr/lib64 usr/libx32 usr/include; do
    mkdir -p /usr/src/fusemount/$f
    mount --bind /$f /usr/src/fusemount/$f
done

# create the symlinks for the ones that are in /usr
# for f in lib lib32 lib64 libx32; do
#   ln -s /usr/src/fusemount/usr/$f /usr/src/fusemount/$f
# done

/bin/bash
