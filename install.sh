#!/bin/bash

set -euo pipefail

REPO_ARCHIVE_URL="https://codeload.github.com/Strvm/musicxmatch-api/tar.gz/refs/heads/main"
INSTALL_DIR="$HOME/.config/ivLyrics/musicxmatch-provider"
SERVICE_NAME="ivlyrics-musicxmatch-provider"
PORT="${PORT:-8092}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}=================================================="
echo -e "  MusicXMatch Provider Install"
echo -e "==================================================${NC}"
echo ""

echo -e "${YELLOW}[1/4] Checking Python 3...${NC}"
if ! command -v python3 >/dev/null 2>&1; then
    echo -e "  ${RED}[FAIL] python3 not found${NC}"
    exit 1
fi
echo -e "  ${GREEN}[OK] $(python3 --version)${NC}"

echo -e "${YELLOW}[2/4] Downloading repository...${NC}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
ARCHIVE_PATH="$TMP_DIR/repo.tar.gz"
curl -fsSL "$REPO_ARCHIVE_URL" -o "$ARCHIVE_PATH"
tar -xzf "$ARCHIVE_PATH" -C "$TMP_DIR"
SRC_DIR="$(find "$TMP_DIR" -maxdepth 1 -type d -name 'musicxmatch-api-*' | head -n 1)"

mkdir -p "$INSTALL_DIR"
find "$INSTALL_DIR" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
cp -R "$SRC_DIR"/. "$INSTALL_DIR"/
echo -e "  ${GREEN}[OK] Installed files to $INSTALL_DIR${NC}"

echo -e "${YELLOW}[3/4] Creating virtualenv and installing package...${NC}"
python3 -m venv "$INSTALL_DIR/.venv"
"$INSTALL_DIR/.venv/bin/python" -m pip install --upgrade pip setuptools wheel >/dev/null
"$INSTALL_DIR/.venv/bin/python" -m pip install -e "$INSTALL_DIR" >/dev/null

cat > "$INSTALL_DIR/run-server.sh" << EOF
#!/bin/bash
set -euo pipefail
exec "$INSTALL_DIR/.venv/bin/musicxmatch-addon-server" --host 127.0.0.1 --port $PORT
EOF
chmod +x "$INSTALL_DIR/run-server.sh"
echo -e "  ${GREEN}[OK] Virtualenv ready${NC}"

echo -e "${YELLOW}[4/4] Registering auto-start...${NC}"
AUTOSTART_NOTE="Manual start required."
if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/$SERVICE_NAME.plist"
    mkdir -p "$(dirname "$PLIST")"
    cat > "$PLIST" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>$SERVICE_NAME</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/run-server.sh</string>
    </array>
    <key>WorkingDirectory</key><string>$INSTALL_DIR</string>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardOutPath</key><string>$INSTALL_DIR/server.log</string>
    <key>StandardErrorPath</key><string>$INSTALL_DIR/server.log</string>
</dict>
</plist>
EOF
    launchctl unload "$PLIST" >/dev/null 2>&1 || true
    launchctl load "$PLIST"
    AUTOSTART_NOTE="LaunchAgent registered."
elif command -v systemctl >/dev/null 2>&1; then
    mkdir -p "$HOME/.config/systemd/user"
    cat > "$HOME/.config/systemd/user/$SERVICE_NAME.service" << EOF
[Unit]
Description=MusicXMatch ivLyrics provider
After=network.target

[Service]
Type=simple
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/run-server.sh
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
    if systemctl --user daemon-reload >/dev/null 2>&1; then
        systemctl --user enable "$SERVICE_NAME.service" >/dev/null
        systemctl --user restart "$SERVICE_NAME.service"
        AUTOSTART_NOTE="systemd user service registered."
    else
        nohup "$INSTALL_DIR/run-server.sh" >/dev/null 2>&1 &
        AUTOSTART_NOTE="systemd unavailable, started in background once."
    fi
else
    nohup "$INSTALL_DIR/run-server.sh" >/dev/null 2>&1 &
    AUTOSTART_NOTE="No service manager found, started in background once."
fi

echo ""
echo -e "${CYAN}=================================================="
echo -e "  Installation complete!"
echo -e "==================================================${NC}"
echo ""
echo -e "  Server URL:   http://localhost:$PORT"
echo -e "  Install path: $INSTALL_DIR"
echo -e "  Addon file:   $INSTALL_DIR/Addon_Lyrics_MusicXMatch.js"
echo -e "  $AUTOSTART_NOTE"
echo ""
echo -e "  In ivLyrics Settings > MusicXMatch Provider > Server URL,"
echo -e "  enter: http://localhost:$PORT"
echo ""

sleep 2
if command -v curl >/dev/null 2>&1 && curl -sf "http://127.0.0.1:$PORT/health" >/dev/null; then
    echo -e "  ${GREEN}[OK] Server is running${NC}"
else
    echo -e "  ${YELLOW}Server may still be starting. Test the connection in ivLyrics shortly.${NC}"
fi
echo ""
