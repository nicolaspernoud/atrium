#!/bin/bash

WD="$(
    cd "$(dirname "$0")"
    pwd -P
)"

if docker pull "nicolaspernoud/atrium:latest" | grep -q "Image is up to date"; then
    echo "Image is up to date, no need to restart..."
else
    echo "New image available, restarting service..."
    ${WD}/up.sh
fi
