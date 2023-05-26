#!/bin/bash
# Working directory
WD="$(
    cd "$(dirname "$0")"
    pwd -P
)"

# STOP KEYCLOAK
$WD/down.sh

# START KEYCLOAK
docker run -d --name keycloak \
    --restart unless-stopped \
    -v /etc/localtime:/etc/localtime:ro \
    -v /etc/timezone:/etc/timezone:ro \
    -p 8888:8080 \
    -e KEYCLOAK_ADMIN=admin \
    -e KEYCLOAK_ADMIN_PASSWORD=admin \
    -v $WD:/opt/keycloak/data/import \
    quay.io/keycloak/keycloak:21.1.1 start-dev --import-realm
