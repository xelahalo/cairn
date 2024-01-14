FROM debian:bookworm
WORKDIR /usr/src/app

RUN mkdir /usr/src/dockermount /usr/src/app/mnt /usr/src/app/mnt_ll

RUN apt update && \
    apt install -y fuse3 libfuse3-dev=3.14.0-4 kmod rsync hyperfine build-essential pkg-config gcc && \
    mknod /dev/fuse c 10 299

# Copy the source code into the container
COPY . .
RUN gcc -Wall ./libfuse_examples/passthrough_ll.c `pkg-config fuse3 --cflags --libs` -o passthrough_ll
RUN gcc -Wall ./libfuse_examples/passthrough.c `pkg-config fuse3 --cflags --libs` -o passthrough
RUN chmod +x /usr/src/app/bench-container-start.sh

# Set the entry point for the container
CMD ["/usr/src/app/bench-container-start.sh"]
