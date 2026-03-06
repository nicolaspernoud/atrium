#!/usr/bin/env bash
set -euo pipefail

WD="$(
  cd "$(dirname "$0")" || exit
  pwd -P
)"

APP="atrium"
OWNER="nicolaspernoud"
REPO="atrium"

INSTALL_DIR="$WD"
STATE_DIR="$WD"
VERSION_FILE="${STATE_DIR}/release_tag"
BIN_PATH="${INSTALL_DIR}/${APP}"
SERVICE_PATH="/etc/systemd/system/${APP}.service"

mkdir -p "$STATE_DIR"

detect_arch() {
    case "$(uname -m)" in
        x86_64)  echo "amd64" ;;
        aarch64) echo "arm64" ;;
        armv6l)  echo "armv6" ;;
        armv7l)  echo "armv7" ;;
        *)
            echo "Unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac
}

ARCH="$(detect_arch)"
ASSET_NAME="${APP}-${ARCH}"

LATEST_URL="https://github.com/${OWNER}/${REPO}/releases/latest"

echo "[+] Resolving latest release…"

FINAL_URL="$(curl -fsSL -o /dev/null -w '%{url_effective}' "$LATEST_URL")"

if [[ -z "$FINAL_URL" ]]; then
    echo "Failed to resolve latest release" >&2
    exit 1
fi

LATEST_TAG="${FINAL_URL##*/}"

DOWNLOAD_URL="https://github.com/${OWNER}/${REPO}/releases/download/${LATEST_TAG}/${ASSET_NAME}"

INSTALLED_TAG="$(cat "$VERSION_FILE" 2>/dev/null || true)"

if [[ ! -f "$BIN_PATH" ]]; then
    echo "[+] Installing ${APP} (${LATEST_TAG})…"

    curl -fL "$DOWNLOAD_URL" -o "${BIN_PATH}.tmp"
    chmod +x "${BIN_PATH}.tmp"
    sudo mv "${BIN_PATH}.tmp" "$BIN_PATH"

    echo "$LATEST_TAG" > "$VERSION_FILE"
else
    if [[ "$LATEST_TAG" != "$INSTALLED_TAG" ]]; then
        echo "[+] Updating ${APP} (${INSTALLED_TAG} → ${LATEST_TAG})…"

        curl -fL "$DOWNLOAD_URL" -o "${BIN_PATH}.tmp"
        chmod +x "${BIN_PATH}.tmp"
        sudo mv "${BIN_PATH}.tmp" "$BIN_PATH"

        echo "$LATEST_TAG" > "$VERSION_FILE"
        sudo systemctl restart "${APP}"
    else
        echo "[+] Binary already up to date (${INSTALLED_TAG})"
    fi
fi

if [[ ! -f "$SERVICE_PATH" ]]; then
    echo "[+] Creating systemd service…"

    cat <<EOF | sudo tee "$SERVICE_PATH" >/dev/null
[Unit]
Description=atrium service
After=network.target

[Service]
ExecStart=${BIN_PATH}
WorkingDirectory=${INSTALL_DIR}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable "${APP}"
    sudo systemctl start "${APP}"
else
    echo "[+] Systemd service already exists"
fi


echo "[+] Service running"