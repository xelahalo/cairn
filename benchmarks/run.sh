#!/bin/sh

if [ -z "${1}" ]; then
	echo "benchmark stage is required" 1>&2
	exit 1
fi

STAGE=$1
COMMIT_HASH=$(git log -1 --pretty=%h)
BENCHMARK_NAME=$(date +%Y-%m-%d_%H-%M-%S)_$COMMIT_HASH

# for each directory in stage_1
for d in benchmarks/$STAGE/*; do
		EXECUTABLE=$(basename $d)
		cp $d/$EXECUTABLE mnt/workdir/

		# for each dir for the executable
		for f in $d/*; do
				# if it is a directory other then results
				if [ -d "$f" ] && [ $(basename $f) != "results" ]; then

					# copy the benchmark test to the workdir
					rsync -av $f/ mnt/workdir/
					cd mnt/workdir/

					# STEP 1: Benchmark it locally
					chmod +x run.sh && bench './run.sh' --json local_$BENCHMARK_NAME.json

					# STEP 2: Benchmark it in the container
					docker exec -it build-env mkdir -p /usr/src/benchmark/ 
					docker exec -it build-env rsync -av /usr/src/dockermount/workdir/ /usr/src/benchmark/
					docker exec -it build-env /bin/bash -c "cd /usr/src/benchmark && chmod +x run.sh && bench './run.sh' --json docker_$BENCHMARK_NAME.json"
					docker exec -it build-env cp /usr/src/benchmark/docker_$BENCHMARK_NAME.json /usr/src/dockermount/workdir/
					docker exec -it build-env find /usr/src/benchmark -delete

					# STEP 3: Benchmark it using Cairn
					bench 'cairn "./run.sh" --container build-env' --json cairn_$BENCHMARK_NAME.json

					cd ../..

					# move the json to the benchmark directory
					mkdir -p benchmarks/$STAGE/results/$EXECUTABLE/$(basename $f)
					mv mnt/workdir/*$BENCHMARK_NAME.json benchmarks/$STAGE/$EXECUTABLE/$(basename $f)/

					# clean up workdir
					find mnt/workdir/ -type f ! -name ".gitkeep" -exec rm {} \;
				fi
		done
done

# zip the results
cd benchmarks/$STAGE/results
zip -r $BENCHMARK_NAME.zip *
cd ../../..
