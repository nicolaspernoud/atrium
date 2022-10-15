#!/bin/bash

WD="$(
    cd "$(dirname "$0")"
    pwd -P
)"

"${WD}"/down.sh

mkdir -p "${WD}"/data "${WD}"/letsencrypt_cache
chown -Rf 1000:1000 "${WD}"/data "${WD}"/letsencrypt_cache

docker run -d --name atrium \
    --net=host \
    -v "${WD}"/atrium.yaml:/app/atrium.yaml \
    -v "${WD}"/GeoLite2-City.mmdb:/app/GeoLite2-City.mmdb \
    -v "${WD}"/letsencrypt_cache:/app/letsencrypt_cache \
    -v "${WD}"/data:/app/data \
    nicolaspernoud/atrium:latest
