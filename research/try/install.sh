docker build -t "try-env:test" .

docker run --rm --cap-add SYS_ADMIN --privileged --name "try-env" -it "try-env:test"
