#!/usr/bin/env bash

source .env

docker kill $CONTAINER_NAME && docker rm $CONTAINER_NAME

KNOWN_HOSTS="$HOME/.ssh/known_hosts"
PATTERN="\[localhost\]:$PORT"
TEMP_FILE="$(mktemp)"

grep -v "$PATTERN" "$KNOWN_HOSTS" >"$TEMP_FILE"
mv "$TEMP_FILE" "$KNOWN_HOSTS"
