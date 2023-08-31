FROM ubuntu:latest

RUN apt-get update && \
    apt-get install -y openssh-server python3-pip fuse3 kmod && \
    pip3 install outrun && \
    rm -rf /var/lib/apt/lists/* && \
    mkdir /var/run/sshd

# Set a password for the root user
RUN echo 'root:password' | chpasswd

# Allow SSH root login and password authentication
RUN sed -i 's/#PermitRootLogin prohibit-password/PermitRootLogin yes/' /etc/ssh/sshd_config
RUN sed -i 's/PasswordAuthentication no/PasswordAuthentication yes/' /etc/ssh/sshd_config

EXPOSE 22

CMD ["/usr/sbin/sshd", "-D"]

