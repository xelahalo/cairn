docker build -t "strace:test" .

docker run --rm --cap-add SYS_ADMIN --privileged --name "strace" -it "strace:test"
