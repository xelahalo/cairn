#!/bin/bash

usage() {
  echo "Usage: $0 [-h] <chroot_dir> <workdir> <command>" 1>&2
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
	echo "Directory to chroot into is required" 1>&2
	usage
elif [ -z "${2}" ]; then
  echo "Workdir is required" 1>&2
  usage
elif [ -z "${3}" ]; then
  echo "Command is required" 1>&2
  usage
fi

chroot_dir=$1
workdir=$2
command=$3

if [ ! -d "$chroot_dir" ]; then
  echo "Directory $chroot_dir does not exist" 1>&2
  exit 1
fi

(chroot "${chroot_dir}" /bin/bash -c "cd ${workdir} && ${command}" ) &

pid=$!

wait "$pid"
echo "$pid"