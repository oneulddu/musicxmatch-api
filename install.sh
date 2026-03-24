#!/bin/bash
set -e

echo "=== ivLyrics Lyrics Providers Installer ==="
echo ""

INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
EXTENSIONS_DIR="$HOME/.config/spicetify/Extensions"
ADDON_NAMES=("Addon_Lyrics_MusicXMatch.js" "Addon_Lyrics_Deezer.js")
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_DIR="$HOME/.cargo/bin"
BIN_PATH="$BIN_DIR/ivlyrics-musicxmatch-server"
SERVER_URL="http://127.0.0.1:8092"
RUNTIME_PATH="$HOME/.cargo/bin:$HOME/.spicetify:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

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
    launchctl unload "$PLIST" >/dev/null 2>&1 || true
    launchctl load "$PLIST"
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
    systemctl --user restart ivlyrics-musicxmatch
fi

echo "[5/7] Starting server..."
sleep 2

echo "[6/7] Verifying health and CORS..."
HEALTH_HEADERS="$(curl -fsSI "$SERVER_URL/health" || true)"
if [[ -z "$HEALTH_HEADERS" ]]; then
    echo "Server health check failed: $SERVER_URL/health"
    exit 1
fi

echo "$HEALTH_HEADERS" | tr -d '\r' | grep -qi '^access-control-allow-origin: \*$' || {
    echo "CORS header check failed: access-control-allow-origin: * not found"
    exit 1
}

echo "[7/7] Installing ivLyrics addons..."
if ! command -v spicetify >/dev/null 2>&1; then
    echo "spicetify is not installed or not in PATH. Skipping addon registration."
else
    mkdir -p "$EXTENSIONS_DIR"
    CURRENT_EXTENSIONS="$(spicetify config extensions 2>/dev/null || true)"
    for ADDON_NAME in "${ADDON_NAMES[@]}"; do
        ADDON_PATH="$EXTENSIONS_DIR/$ADDON_NAME"
        ADDON_URL="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/$ADDON_NAME"
        curl -fsSL "$ADDON_URL" -o "$ADDON_PATH"

        if ! printf '%s\n' "$CURRENT_EXTENSIONS" | tr '|' '\n' | sed 's/^ *//;s/ *$//' | grep -Fxq "$ADDON_NAME"; then
            spicetify config extensions "$ADDON_NAME"
            CURRENT_EXTENSIONS="${CURRENT_EXTENSIONS}${CURRENT_EXTENSIONS:+ | }$ADDON_NAME"
        fi
    done

    spicetify apply
fi

echo ""
echo "✓ Installation complete!"
echo "Server running at $SERVER_URL"
echo "Addon paths: $EXTENSIONS_DIR/${ADDON_NAMES[0]}, $EXTENSIONS_DIR/${ADDON_NAMES[1]}"
echo ""
