# Use a base Linux distribution image
FROM alpine:latest

# Update package repositories and install necessary packages
RUN apk update && apk upgrade && apk add bash

# Set the default command to run when the container starts
CMD ["bash"]

