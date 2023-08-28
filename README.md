## Build the Docker image
docker build -t alpine-latest .

# Run a container from the built image
docker run -d --name alpine-latest -it alpine-latest
