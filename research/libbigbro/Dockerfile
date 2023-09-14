FROM ubuntu:latest
WORKDIR /usr/libbigbro 

ENV PATH="$/usr/libbigbro:$PATH"

RUN apt update && \
    apt install -y git python3 gcc sudo && \
    git clone https://github.com/droundy/bigbro && \
    mv bigbro/* .

CMD ["/bin/sh"]
