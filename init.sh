#!/bin/bash

usage() {
	echo "Usage: $0 [-hb]" 1>&2
	echo "  -h: Display this help message" 1>&2
  echo "  -b: Also build and run the benchmarking container" 1>&2
	exit 1
}

while getopts ":hb" opt; do
	case ${opt} in
	h)
		usage
		;;
  b)
		BENCHMARK=true
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

source ".env"
if [ -z "$MNT_DIR" ]; then
    echo "MNT_DIR is not set in the env file."
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
	echo "Docker is not running. Quitting."
	exit 1
fi

if [ "$BENCHMARK" = true ]; then 
  echo "Building and running benchmarking container"
  docker build -t "build-env:bench" -f benchmarks/Dockerfile .
  docker run \
    --rm \
    --detach \
    --privileged \
    --mount type=bind,source="$(pwd)/$MNT_DIR",target=/usr/src/dockermount,bind-propagation=slave \
    --cap-add SYS_ADMIN \
    --name "build-env-bench" \
    -it "build-env:bench" 
fi

docker build -t "build-env:test" .
docker run \
	--rm \
  --detach \
	--privileged \
	-v "$(pwd)/$MNT_DIR":/usr/src/dockermount \
	--cap-add SYS_ADMIN \
	--name "build-env" \
	-it "build-env:test" 

#	--mount type=bind,source="$(pwd)/$MNT_DIR",target=/usr/src/dockermount,bind-propagation=slave \
