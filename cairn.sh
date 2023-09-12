#!/bin/bash

usage() {
	echo "Usage: $0 [-h] <command>" 1>&2
	echo "  -h: Display this help message" 1>&2
	exit 1
}

while getopts ":h" opt; do
	case ${opt} in
	h)
		usage
		;;
	\?)
		echo "Invalid option: -$OPTARG" 1>&2
		usage
		;;
	:)
		echo "Invalid option: -$OPTARG requires an argument" 1>&2
		usage
		;;
	esac
done

shift $((OPTIND - 1))

if [ -z "${1}" ]; then
	echo "Command is required" 1>&2
	usage
fi

mountpoint=$1

if ! docker info >/dev/null 2>&1; then
	echo "Docker is not running. Quitting."
	exit 1
fi

docker build -t "build-env:test" .
docker run \
	--rm \
	--privileged \
	--mount type=bind,source="$(pwd)$mountpoint",target=/usr/src/dockermount,bind-propagation=rshared \
	--cap-add SYS_ADMIN \
	--name "build-env" \
	-it "build-env:test"
