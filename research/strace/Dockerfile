FROM ubuntu:latest
WORKDIR /usr/strace 

ENV PATH="$/usr/strace:$PATH"

RUN apt update && \
    apt install -y sudo strace 

CMD ["/bin/sh"]
