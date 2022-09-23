#!/bin/bash

WD="$(
    cd "$(dirname "$0")"
    pwd -P
)"

# Generate certificates

sudo rm -rf ${WD}/data
mkdir -p ${WD}/data/certs
openssl genrsa -out ${WD}/data/certs/tls.key 2048
openssl req -new -key ${WD}/data/certs/tls.key -out ${WD}/data/certs/tls.csr -subj "/C=US/ST=YourState/L=YourCity/O=OnlyOffice-Certificates/CN=onlyoffice.local"
openssl x509 -req -days 365 -in ${WD}/data/certs/tls.csr -signkey ${WD}/data/certs/tls.key -out ${WD}/data/certs/tls.crt

$WD/down.sh
docker run -d --name onlyoffice \
    --restart unless-stopped \
    -v /etc/localtime:/etc/localtime:ro \
    -v /etc/timezone:/etc/timezone:ro \
    -v $WD/data:/var/www/onlyoffice/Data \
    -p 2443:443 \
    -e "DOCKER_HOST=$(ip -4 addr show docker0 | grep -Po 'inet \K[\d.]+')" \
    onlyoffice/documentserver

docker exec -it onlyoffice /var/www/onlyoffice/documentserver/npm/json -f /etc/onlyoffice/documentserver/default.json -I -e 'this.services.CoAuthoring.requestDefaults.rejectUnauthorized=false'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files1.atrium.127.0.0.1.nip.io" >>/etc/hosts'
docker exec -it onlyoffice /bin/bash -c 'echo "$DOCKER_HOST files2.atrium.127.0.0.1.nip.io" >>/etc/hosts'
