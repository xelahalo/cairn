docker build -t "strace:test" .

docker run --rm --cap-add SYS_PTRACE --name "strace" -it "strace:test"
