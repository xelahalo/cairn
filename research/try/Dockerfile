FROM ubuntu:latest
WORKDIR /usr/try 

ENV PATH="$/usr/try:$PATH"

RUN apt update && \
    apt install -y curl unzip util-linux && \
    curl -LO https://github.com/binpash/try/archive/refs/tags/v0.2.0.zip && \
    unzip v0.2.0.zip && mv try-0.2.0/* .

CMD ["/bin/sh"]
