docker build -t "libbigbro:test" .

docker run --rm --cap-add SYS_ADMIN --privileged --name "libbigbro" -it "libbigbro:test"
