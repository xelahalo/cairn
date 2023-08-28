#!/usr/bin/env bash

source .env

error() {
	echo "Error:" "$@" 1>&2
}

tag=$(date +%s)

# If Docker socket is not mounted
if [[ ! -S /var/run/docker.sock ]]; then
	error "Please bind mount in the docker socket to /var/run/docker.sock"
	error "docker run -v /var/run/docker.sock:/var/run/docker.sock"
	error "...or make sure you have access to the docker socket at /var/run/docker.sock"
	exit 1
fi

if ! [ -x "$(command -v pip3)" ]; then
	error "pip3 is not installed"
	exit 1
fi

if ! [ -x "$(command -v outrun)" ]; then
	pip3 install outrun
fi

docker build -t "$IMAGE_NAME:$tag" .

if ! docker ps --format '{{.Names}}' | grep -w "$CONTAINER_NAME" &>/dev/null; then
	docker run -d -p 8080:22 --name "$CONTAINER_NAME" -it "$IMAGE_NAME:$tag"
fi
