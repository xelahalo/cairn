docker build -t "build-env:test" .
docker run --rm --privileged --mount type=bind,source="$(pwd)"/mount-point,target=/usr/src/dockermount,bind-propagation=rshared --cap-add SYS_ADMIN --name "build-env" -it "build-env:test"
