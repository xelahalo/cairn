#!/bin/bash

# start the tracer
cairn-fuse /usr/src/dockermount /usr/src/fusemount > app.log 2>&1 &

echo "$!"

# wait for the fs to start
while [ ! -f /usr/src/dockermount/.cairn-fuse-ready ]; do
    sleep 1
done

# mount relevant dirs
for f in proc sys dev bin etc lib lib32 lib64 libx32 usr/lib usr/lib32 usr/lib64 usr/libx32 usr/include; do
    mkdir -p /usr/src/fusemount/$f
    mount --bind /$f /usr/src/fusemount/$f
done

/bin/bash

# TODO: handle SIGTERM, SIGKILL, SIGINT to gracefully unmount