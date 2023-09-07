FROM python:3.11.5-bookworm
WORKDIR /usr/src/app

RUN  mkdir /usr/src/fusemount /usr/src/dockermount

ENV VIRTUAL_ENV=/opt/venv
RUN python3 -m venv $VIRTUAL_ENV
ENV PATH="$VIRTUAL_ENV/bin:$PATH"

RUN apt-get update && \
    apt-get install -y fuse kmod git gcc

COPY . .
RUN pip install -r requirements.txt

CMD ["python", "tracer.py", "/usr/src/dockermount", "/usr/src/fusemount"]

# CMD ["/bin/sh"]
