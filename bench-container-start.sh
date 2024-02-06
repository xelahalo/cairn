#!/bin/bash

/usr/src/app/passthrough /usr/src/app/mnt
/usr/src/app/passthrough_ll /usr/src/app/mnt_ll
cairn-fuse /usr/src/dockermount /usr/src/app/mnt_cairn > app.log 2>&1 &

# wait for the fs to start
while [ ! -f ./.cairn-fuse-ready ]; do
    sleep 1
done

for f in proc sys dev bin etc lib lib32 lib64 libx32 usr/lib usr/lib32 usr/lib64 usr/libx32 usr/include usr/bin usr/sbin; do
    mkdir -p /usr/src/app/mnt_cairn/$f
    chmod -R u=rwx /usr/src/app/mnt_cairn/$f
    mount --bind /$f /usr/src/app/mnt_cairn/$f
done

/bin/bash
