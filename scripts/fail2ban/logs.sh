#!/bin/bash

WD="$(
    cd "$(dirname "$0")" || exit
    pwd -P
)"

cd "${WD}" || exit 1

echo "=== Container logs ==="
docker compose logs fail2ban
echo "=== Fail2ban logs ==="
docker compose exec -it fail2ban cat /config/log/fail2ban/fail2ban.log
