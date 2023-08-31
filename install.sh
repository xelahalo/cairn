#!/usr/bin/env bash

source .env

error() {
	echo "Error:" "$@" 1>&2
}

tag=$(date +%s)

if ! [ -x "$(command -v pip3)" ]; then
	error "pip3 is not installed"
	exit 1
fi

if ! [ -x "$(command -v outrun)" ]; then
	pip3 install outrun
fi

#docker build -t "$IMAGE_NAME:$tag" .
#
#if ! docker ps --format '{{.Names}}' | grep -w "$CONTAINER_NAME" &>/dev/null; then
#	docker run -d -p 8080:22 --name "$CONTAINER_NAME" -it "$IMAGE_NAME:$tag"
#fi

docker compose down
docker compose up -d
