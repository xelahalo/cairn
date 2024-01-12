#!/bin/bash

usage() {
	echo "Usage: $0 [-hb] <mnt-dir>" 1>&2
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

MNT_DIR=$1
if [ -z "$MNT_DIR" ] || [[ "$MNT_DIR" != /* ]] ; then
    echo "Mount directory is required, needs to be a full path" 1>&2
    usage
fi

if ! docker info >/dev/null 2>&1; then
	echo "Docker is not running. Quitting."
	exit 1
fi

SCRIPT_PATH=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$SCRIPT_PATH" || exit

if [ "$BENCHMARK" = true ]; then 
  echo "Building and running benchmarking container"
  docker build -t "build-env:bench" -f benchmarks/Dockerfile .
  docker run \
    --rm \
    --detach \
    --mount type=bind,source="${MNT_DIR}",target=/usr/src/dockermount,bind-propagation=rshared \
    --cap-add SYS_ADMIN \
    --name "build-env-bench" \
    -it "build-env:bench" 
fi

docker build -t "build-env:test" -f "${SCRIPT_PATH}/Dockerfile" .
docker run \
	--rm \
  --detach \
  --mount type=bind,source="${MNT_DIR}",target=/usr/src/dockermount,bind-propagation=rshared \
	--cap-add SYS_ADMIN \
	--name "build-env" \
	-it "build-env:test" 

#-u "$(id -u):$(id -g)" \
#-v "${MNT_DIR}":/usr/src/dockermount \
#	--mount type=bind,source="$(pwd)/$MNT_DIR",target=/usr/src/dockermount,bind-propagation=slave \

cd - || exit
