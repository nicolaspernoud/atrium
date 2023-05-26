#!/bin/bash
docker exec -it keycloak /opt/keycloak/bin/kc.sh export --file /opt/keycloak/data/import/mdl.json --realm mdl
