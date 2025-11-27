#!/bin/bash

WD="$(
    cd "$(dirname "$0")" || exit
    pwd -P
)"

cd "${WD}" || exit 1

docker compose exec -it fail2ban fail2ban-client reload atrium-denied