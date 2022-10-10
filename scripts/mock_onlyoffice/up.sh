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
    -p 8083:80 \
    -e "DOCKER_HOST=$(ip -4 addr show docker0 | grep -Po 'inet \K[\d.]+')" \
    -e "JWT_SECRET=CHANGE_ME_IN_PRODUCTION" \
    onlyoffice/documentserver

docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files1.atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files2.atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST atrium.10.0.2.2.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files1.atrium.10.0.2.2.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files2.atrium.10.0.2.2.nip.io" >>/etc/hosts'
