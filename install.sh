#!/bin/bash
set -e

echo "=== ivLyrics Lyrics Providers Installer ==="
echo ""

INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_DIR="$HOME/.cargo/bin"
BIN_PATH="$BIN_DIR/ivlyrics-musicxmatch-server"
SERVER_URL="http://127.0.0.1:8092"
RUNTIME_PATH="$HOME/.cargo/bin:$HOME/.spicetify:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
RAW_BASE_URL="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
ADDON_URLS=(
    "$RAW_BASE_URL/Addon_Lyrics_MusicXMatch.js"
    "$RAW_BASE_URL/Addon_Lyrics_Deezer.js"
    "$RAW_BASE_URL/Addon_Lyrics_Bugs.js"
    "$RAW_BASE_URL/Addon_Lyrics_Genie.js"
)

echo "[1/7] Creating installation directory..."
mkdir -p "$INSTALL_DIR"

echo "[2/7] Ensuring Rust toolchain is available..."
if ! command -v cargo >/dev/null 2>&1; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install rust
    else
        echo "cargo is required. Install Rust first: https://rustup.rs"
        exit 1
    fi
fi

echo "[3/7] Installing server binary..."
cargo install --git https://github.com/oneulddu/musicxmatch-api.git --bin ivlyrics-musicxmatch-server --force

echo "[4/7] Setting up auto-start..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/$SERVICE_LABEL.plist"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$SERVICE_LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN_PATH</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>MXM_SESSION_FILE</key>
        <string>$INSTALL_DIR/musixmatch_session.json</string>
        <key>PATH</key>
        <string>$RUNTIME_PATH</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF
else
    SERVICE_FILE="$HOME/.config/systemd/user/ivlyrics-musicxmatch.service"
    mkdir -p "$HOME/.config/systemd/user"
    cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=ivLyrics MusicXMatch Server

[Service]
Environment=MXM_SESSION_FILE=$INSTALL_DIR/musixmatch_session.json
ExecStart=$BIN_PATH
Restart=always

[Install]
WantedBy=default.target
EOF
    systemctl --user daemon-reload
    systemctl --user enable ivlyrics-musicxmatch
fi

echo "[5/6] Starting server..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    LAUNCHD_DOMAIN="gui/$(id -u)"
    LAUNCHD_SERVICE="$LAUNCHD_DOMAIN/$SERVICE_LABEL"
    if launchctl print "$LAUNCHD_SERVICE" >/dev/null 2>&1; then
        launchctl kickstart -k "$LAUNCHD_SERVICE"
    else
        launchctl bootstrap "$LAUNCHD_DOMAIN" "$PLIST"
    fi
else
    systemctl --user restart ivlyrics-musicxmatch
fi

sleep 2

echo "[6/6] Verifying health and CORS..."
HEALTH_HEADERS="$(curl -fsSI "$SERVER_URL/health" || true)"
if [[ -z "$HEALTH_HEADERS" ]]; then
    echo "Server health check failed: $SERVER_URL/health"
    exit 1
fi

echo "$HEALTH_HEADERS" | tr -d '\r' | grep -qi '^access-control-allow-origin: \*$' || {
    echo "CORS header check failed: access-control-allow-origin: * not found"
    exit 1
}

echo ""
echo "✓ Installation complete!"
echo "Server running at $SERVER_URL"
echo ""
echo "Install addons with ivLyrics addon-manager:"
for addon_url in "${ADDON_URLS[@]}"; do
    echo "  curl -fsSL https://ivlis.kr/ivLyrics/addon-manager.sh | bash -s -- \"$addon_url\""
done
echo ""
