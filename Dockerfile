FROM rust:1.73-bookworm
WORKDIR /usr/src/app

RUN mkdir /usr/src/fusemount /usr/src/dockermount

RUN apt update && \
    # hyperfine is only needed for benchmarking
    apt install -y strace fuse3 libfuse3-dev=3.14.0-4 kmod rsync hyperfine && \
    # 'mknod': creates a special file
    # 'Name': /dev/fuse name of the driver 
    # '{ b | c }': c, which correspons to character-oriented device
    # 'Major': 10, which corresponds to the "miscellaneous devices" category
    # 'Minor': 299, which corresponds to the "fuse" driver
    mknod /dev/fuse c 10 299

# cairn-fuse
COPY . .
RUN cargo install --path ./cairn-fuse
RUN chmod +x /usr/src/app/startup.sh

CMD ["/usr/src/app/startup.sh"]
# CMD ["/bin/bash"]

