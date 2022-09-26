#!/bin/bash

WD="$(
    cd "$(dirname "$0")"
    pwd -P
)"

$WD/down.sh
docker run -d --name onlyoffice \
    --restart unless-stopped \
    -v /etc/localtime:/etc/localtime:ro \
    -v /etc/timezone:/etc/timezone:ro \
    -p 8081:80 \
    -e "DOCKER_HOST=$(ip -4 addr show docker0 | grep -Po 'inet \K[\d.]+')" \
    onlyoffice/documentserver

docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files1.atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files2.atrium.127.0.0.1.nip.io" >>/etc/hosts'
