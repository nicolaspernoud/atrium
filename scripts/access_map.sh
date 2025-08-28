#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <config.yaml>"
  exit 1
fi

CONFIG_FILE=$(realpath "$1")

TMPDIR=$(mktemp -d)
cleanup() {
  rm -rf "$TMPDIR"
}
trap cleanup EXIT

# --- Python script ---
cat > "$TMPDIR/access_map.py" <<'PYCODE'
import yaml
import sys
from collections import defaultdict

def load_config(path):
    with open(path, "r", encoding="utf-8") as f:
        return yaml.safe_load(f)

def classify_services(services):
    """Return (public_list, secure_dict_by_name)"""
    public = []
    secure = defaultdict(set)  # name -> set(roles)
    for svc in services:
        name = svc["name"]
        roles = svc.get("roles", [])
        secured = svc.get("secured", False)

        if not secured:  # Public regardless of roles
            public.append(name)
        else:
            for role in roles:
                secure[name].add(role)
    return public, secure

def resolve_access(config):
    public_apps, secure_apps = classify_services(config.get("apps", []))
    public_davs, secure_davs = classify_services(config.get("davs", []))

    # Build user access maps for secure resources
    users = {u["login"]: set(u.get("roles", [])) for u in config.get("users", [])}

    access_apps = {name: {u: ("✔" if roles & user_roles else "") 
                          for u, user_roles in users.items()}
                   for name, roles in secure_apps.items()}

    access_davs = {name: {u: ("✔" if roles & user_roles else "") 
                          for u, user_roles in users.items()}
                   for name, roles in secure_davs.items()}

    return public_apps, public_davs, access_apps, access_davs, list(users.keys())

def print_markdown_table(title, data, users):
    if not data:
        return
    print(f"### {title}")
    header = [""] + users
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join(["---"] * len(header)) + "|")
    for resource, access in data.items():
        row = [resource] + [access[u] for u in users]
        print("| " + " | ".join(row) + " |")
    print()

def main():
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <config.yaml>")
        sys.exit(1)

    config_path = sys.argv[1]
    config = load_config(config_path)
    public_apps, public_davs, access_apps, access_davs, users = resolve_access(config)

    print("## Public (non-secured) resources\n")
    print("**Apps:** " + (", ".join(public_apps) if public_apps else "(none)"))
    print("\n**Davs:** " + (", ".join(public_davs) if public_davs else "(none)"))
    print()

    print("## Secure resources access tables\n")
    print_markdown_table("Apps", access_apps, users)
    print_markdown_table("Davs", access_davs, users)

if __name__ == "__main__":
    main()
PYCODE

# --- Dockerfile ---
cat > "$TMPDIR/Dockerfile" <<'DOCKER'
FROM python:3.11-slim
RUN pip install --no-cache-dir pyyaml
WORKDIR /app
COPY access_map.py /app/access_map.py
RUN chmod +x /app/access_map.py
ENTRYPOINT ["/usr/bin/env", "python", "/app/access_map.py"]
DOCKER

# Build Docker image
docker build -q -t access-map:local "$TMPDIR" >/dev/null

# Run container mounting the config file
docker run --rm -v "$CONFIG_FILE":/config.yaml access-map:local /config.yaml
