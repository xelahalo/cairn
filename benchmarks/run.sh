#!/bin/sh

if [ -z "${1}" ]; then
	echo "benchmark stage is required" 1>&2
	usage
fi

STAGE=$1
COMMIT_HASH=$(git log -1 --pretty=%H)
BENCHMARK_NAME=benchmark_$(date +%Y-%m-%d_%H:%M:%S)_$COMMIT_HASH

# for each directory in stage_1
for d in benchmark/$STAGE/*; do
		EXECUTABLE=$(basename $d)
		cp $d/$EXECUTABLE mnt/workdir

		# for each dir for the executable
		for f in $d/*; do
			# if it is a directory (benchmark test)
				if [ -d $f ]; then

					# copy the benchmark test to the workdir
					cp $f/* mnt/workdir

					# benchmark the command that is found in run.txt
					bench --before 'cd mnt/workdir' --after 'cd ../..' '$(cat run.txt)' --json $BENCHMARK_NAME.json

					# move the csv to the benchmark directory
					mv mnt/workdir/$BENCHMARK_NAME.csv benchmark/$STAGE/$EXECUTABLE/$(basename $f)

					# clean the workdir
					rm -rf mnt/workdir/!(.gitkeep)
				fi
		done
done
