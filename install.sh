#!/bin/bash
set -e

echo "=== ivLyrics MusicXMatch Provider Installer ==="
echo ""

INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_NAME="ivlyrics-musicxmatch"

echo "[1/5] Creating installation directory..."
mkdir -p "$INSTALL_DIR"

echo "[2/5] Downloading files..."
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/server.js -o "$INSTALL_DIR/server.js"
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/musicxmatch.js -o "$INSTALL_DIR/musicxmatch.js"
curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/package.json -o "$INSTALL_DIR/package.json"

echo "[3/5] Installing dependencies..."
cd "$INSTALL_DIR"
npm install --production

echo "[4/5] Setting up auto-start..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/com.ivlyrics.musicxmatch.plist"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.ivlyrics.musicxmatch</string>
    <key>ProgramArguments</key>
    <array>
        <string>$(which node)</string>
        <string>$INSTALL_DIR/server.js</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF
    launchctl load "$PLIST"
else
    SERVICE_FILE="$HOME/.config/systemd/user/$SERVICE_NAME.service"
    mkdir -p "$HOME/.config/systemd/user"
    cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=ivLyrics MusicXMatch Server

[Service]
ExecStart=$(which node) $INSTALL_DIR/server.js
Restart=always

[Install]
WantedBy=default.target
EOF
    systemctl --user daemon-reload
    systemctl --user enable $SERVICE_NAME
    systemctl --user start $SERVICE_NAME
fi

echo "[5/5] Starting server..."
sleep 2

echo ""
echo "✓ Installation complete!"
echo "Server running at http://localhost:8092"
echo ""


