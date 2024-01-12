#!/bin/sh

START=1
END=10
RANGE=2

while getopts ":s:e:r:" opt; do
  case $opt in
    s)
      START="$OPTARG"
      ;;
    e)
      END="$OPTARG"
      ;;
    r)
      RANGE="$OPTARG"
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      exit 1
      ;;
    :)
      echo "Option -$OPTARG requires an argument." >&2
      exit 1
      ;;
  esac
done

shift $((OPTIND-1))

COMMIT_HASH=$(git log -1 --pretty=%h)
BENCHMARK_NAME=$(date +%Y-%m-%d_%H-%M-%S)_$COMMIT_HASH
chmod +x stop.sh

# for each directory in stage_1
for d in benchmarks/commands/*; do
	EXECUTABLE=$(basename $d)

	# for each dir for the executable
	for f in $d/*; do
		# if it is a directory 
		if [ -d "$f" ] ; then
      echo "-----------------------------------------"
      echo "Benchmarking $f"
      echo "-----------------------------------------"

			# copy the benchmark test to the workdir
      cp -r $f/. host_mnt/

			if [ "$EXECUTABLE" = "stress" ]; then
        cp $d/gcc host_mnt/
      else
		  	cp $d/$EXECUTABLE host_mnt/
      fi

			cd host_mnt || exit
			chmod +x run.sh

      echo "-----------------------------------------"
      echo "Benchmarking locally..."
      echo "-----------------------------------------"

			# STEP 1: Benchmark it locally
			if [ "$EXECUTABLE" = "stress" ]; then
        hyperfine --warmup 3 --parameter-scan iter "$START" "$END" -D "$RANGE" './run.sh {iter}' --export-json local_$BENCHMARK_NAME.json
      else
      	hyperfine --warmup 3 './run.sh' --export-json local_$BENCHMARK_NAME.json
      fi 

      echo "-----------------------------------------"
      echo "Benchmarking in Docker..."
      echo "-----------------------------------------"

			# STEP 2: Benchmark it in docker
      docker exec build-env mkdir -p /usr/src/benchmark/
      docker exec build-env cp -r /usr/src/dockermount/. /usr/src/benchmark/
      docker exec build-env /bin/bash -c "cd /usr/src/benchmark && chmod +x run.sh && \
      if [ \"$EXECUTABLE\" = \"stress\" ]; then \
        hyperfine --warmup 3 --parameter-scan iter $START $END -D $RANGE './run.sh {iter}' --export-json docker_$BENCHMARK_NAME.json; \
      else \
        hyperfine --warmup 3 './run.sh' --export-json docker_$BENCHMARK_NAME.json; \
      fi"
      docker exec build-env cp /usr/src/benchmark/docker_$BENCHMARK_NAME.json /usr/src/dockermount/
      docker exec build-env find /usr/src/benchmark -delete

      echo "-----------------------------------------"
      echo "Benchmarking in Docker on FUSE..."
      echo "-----------------------------------------"

      # STEP 3: Benchmark it in docker on top of FUSE
      # Make it run in a different container instead of trying to make it run alongside the current fuse layer
      docker exec build-env-bench mkdir -p /workdir
      docker exec build-env-bench /bin/bash -c "cp -r /usr/src/dockermount/workdir/. /workdir/"
      docker exec build-env-bench /bin/bash -c "cd /usr/src/app/mnt/workdir && chmod +x run.sh && \
        if [ \"$EXECUTABLE\" = \"stress\" ]; then \
          hyperfine --warmup 3 --parameter-scan iter $START $END -D $RANGE './run.sh {iter}' --export-json fuse_docker_$BENCHMARK_NAME.json; \
        else \
          hyperfine --warmup 3 './run.sh' --export-json fuse_docker_$BENCHMARK_NAME.json; \
        fi"
      docker exec build-env-bench cp /workdir/fuse_docker_$BENCHMARK_NAME.json /usr/src/dockermount/workdir/ 
      docker exec build-env-bench find /workdir -delete 

      echo "-----------------------------------------"
      echo "Benchmarking with Cairn..."
      echo "-----------------------------------------"

			# STEP 4: Benchmark it using Cairn
			if [ "$EXECUTABLE" = "stress" ]; then
        hyperfine --warmup 3 --parameter-scan iter "$START" "$END" -D "$RANGE" 'fsatrace rwmdtq tracer.log -- ./run.sh {iter}' --export-json cairn_$BENCHMARK_NAME.json
      else
      	hyperfine --warmup 3 'fsatrace rwmdtq ./run.sh' --export-json cairn_$BENCHMARK_NAME.json
      fi

			cd - || exit

      echo "-----------------------------------------"
      echo "Copying results..."
      echo "-----------------------------------------"

			# move the json to the benchmark directory
			mkdir -p benchmarks/results/$EXECUTABLE/$(basename $f)
			# copy over all the files that were used to make the benchmarks
			find host_mnt -mindepth 1 -maxdepth 1 \
			! -name bin ! -name dev ! -name etc ! -name 'lib*' ! -name proc ! -name sys ! -name usr ! -name tracer.log \
        -exec cp -r {} benchmarks/results/$EXECUTABLE/$(basename $f) \;

      echo "-----------------------------------------"
      echo "Cleaning up..."
      echo "-----------------------------------------"

			# clean up dir
			find host_mnt -mindepth 1 -maxdepth 1 \
        ! -name bin ! -name dev ! -name etc ! -name 'lib*' ! -name proc ! -name sys ! -name usr ! -name tracer.log \
        -exec rm -r {} +
		fi
	done
done

# run python scripts
source venv/bin/activate
python3 benchmarks/tex.py benchmarks/results/
python3 benchmarks/plot.py benchmarks/results/stress
deactivate

# # zip the results
cd benchmarks/results
zip -r $BENCHMARK_NAME.zip *
cd ../..
mv benchmarks/results/$BENCHMARK_NAME.zip benchmarks/
rm -rf benchmarks/results/
