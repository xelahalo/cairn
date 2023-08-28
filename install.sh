#!/usr/bin/env bash

error() {
	echo "Error:" "$@" 1>&2
}

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

docker build -t alpine-latest .
docker run -d --name alpine-latest -it alpine-latest
