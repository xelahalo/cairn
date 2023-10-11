FROM python:3.11.5-bookworm
WORKDIR /usr/src/app

RUN  mkdir /usr/src/fusemount /usr/src/dockermount

ENV VIRTUAL_ENV=/opt/venv
RUN python3 -m venv $VIRTUAL_ENV
ENV PATH="$VIRTUAL_ENV/bin:$PATH"

RUN apt update && \
    apt install -y fuse kmod && \
    # 'mknod': creates a special file
    # 'Name': /dev/fuse name of the driver 
    # '{ b | c }': c, which correspons to character-oriented device
    # 'Major': 10, which corresponds to the "miscellaneous devices" category
    # 'Minor': 299, which corresponds to the "fuse" driver
    mknod /dev/fuse c 10 299

COPY tracer.py requirements.txt startup.sh .

RUN pip install -r requirements.txt
RUN chmod +x /usr/src/app/startup.sh

CMD ["/usr/src/app/startup.sh"]
