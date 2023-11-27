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

	# for each dir for the executable
	for f in $d/*; do
		# if it is a directory 
		if [ -d "$f" ] ; then
			# copy the benchmark test to the workdir
			rsync -av $f/ host_mnt/workdir/
			cp $d/$EXECUTABLE host_mnt/workdir/
			cd host_mnt/workdir/

			# STEP 1: Benchmark it locally
			chmod +x run.sh && hyperfine --warmup 3 './run.sh' --export-json local_$BENCHMARK_NAME.json

			# STEP 2: Benchmark it in the container
			docker exec build-env mkdir -p /usr/src/benchmark/ 
			docker exec build-env rsync -av /usr/src/dockermount/workdir/ /usr/src/benchmark/
			docker exec build-env /bin/bash -c "cd /usr/src/benchmark && chmod +x run.sh && hyperfine --warmup 3 './run.sh' --export-json docker_$BENCHMARK_NAME.json"
			docker exec build-env cp /usr/src/benchmark/docker_$BENCHMARK_NAME.json /usr/src/dockermount/workdir/
			docker exec build-env find /usr/src/benchmark -delete

			# STEP 3: Benchmark it using Cairn
			hyperfine --warmup 3 'cairn "./run.sh" --container build-env' --export-json cairn_$BENCHMARK_NAME.json

			cd ../..

			# move the json to the benchmark directory
			mkdir -p benchmarks/results/$EXECUTABLE/$(basename $f)
			# copy over all the files that were used to make the benchmarks
			rsync -av --exclude ".gitkeep" host_mnt/workdir/ benchmarks/results/$EXECUTABLE/$(basename $f)/

			# clean up workdir
			find host_mnt/workdir \( -type d -o -type f \) ! -name ".gitkeep" -mindepth 1 -exec rm -r {} \;
		fi
	done
done

# # zip the results
cd benchmarks/results
zip -r $BENCHMARK_NAME.zip *
cd ../..
mv benchmarks/results/$BENCHMARK_NAME.zip benchmarks/
rm -rf benchmarks/results/
