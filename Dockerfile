FROM python:3.11.5-bookworm
WORKDIR /usr/src/app

RUN  mkdir /usr/src/fusemount /usr/src/dockermount

ENV VIRTUAL_ENV=/opt/venv
RUN python3 -m venv $VIRTUAL_ENV
ENV PATH="$VIRTUAL_ENV/bin:$PATH"

RUN apt update && \
    apt install -y fuse kmod git rsync && \
    # 'mknod': creates a special file
    # 'Name': /dev/fuse name of the driver 
    # '{ b | c }': c, which correspons to character-oriented device
    # 'Major': 10, which corresponds to the "miscellaneous devices" category
    # 'Minor': 299, which corresponds to the "fuse" driver
    mknod /dev/fuse c 10 299

# --------------------------------- ONLY FOR BENCHMARKING -------------------------------------------
RUN useradd -m -s /bin/bash linuxbrew && \
    usermod -aG sudo linuxbrew &&  \
    mkdir -p /home/linuxbrew/.linuxbrew && \
    chown -R linuxbrew: /home/linuxbrew/.linuxbrew /usr/src/ /opt/venv/lib/python3.11/site-packages/
USER linuxbrew
RUN /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install.sh)"
USER root
RUN chown -R $CONTAINER_USER: /home/linuxbrew/.linuxbrew
ENV PATH="/home/linuxbrew/.linuxbrew/bin:$PATH"
RUN git config --global --add safe.directory /home/linuxbrew/.linuxbrew/Homebrew
USER linuxbrew
RUN brew install bench
USER root
# -------------------------------------------------------------------------------------------------

COPY tracer.py requirements.txt startup.sh .
RUN pip install -r requirements.txt
RUN chmod +x /usr/src/app/startup.sh

CMD ["/usr/src/app/startup.sh"]
