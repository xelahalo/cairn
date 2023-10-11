# Use a base image with a Linux-based OS
FROM debian:latest

# Install QEMU for user emulation
RUN dpkg --add-architecture amd64 && \
    apt-get update && \
    apt-get install -y qemu-user-static

# Set the QEMU interpreter as the entry point
ENTRYPOINT ["/usr/bin/qemu-x86_64-static", "-E", "LD_LIBRARY_PATH=/lib64:/lib/x86_64-linux-gnu"]

# Copy the non-Linux binary into the container
COPY gcc /gcc

# Specify the non-Linux binary as the command
CMD ["/gcc"]
