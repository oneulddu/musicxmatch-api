#!/bin/bash

set -euo pipefail

INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_PATH="$HOME/.cargo/bin/ivlyrics-musicxmatch-server"

echo ""
echo "Removing MusicXMatch Provider..."

if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/$SERVICE_LABEL.plist"
    if [ -f "$PLIST" ]; then
        launchctl unload "$PLIST" >/dev/null 2>&1 || true
        rm -f "$PLIST"
        echo "  [OK] LaunchAgent removed"
    fi
elif command -v systemctl >/dev/null 2>&1; then
    systemctl --user stop "ivlyrics-musicxmatch.service" >/dev/null 2>&1 || true
    systemctl --user disable "ivlyrics-musicxmatch.service" >/dev/null 2>&1 || true
    rm -f "$HOME/.config/systemd/user/ivlyrics-musicxmatch.service"
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    echo "  [OK] systemd user service removed"
fi

pkill -f "ivlyrics-musicxmatch-server" >/dev/null 2>&1 || true

if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
    echo "  [OK] Install directory removed"
fi

if [ -f "$BIN_PATH" ]; then
    rm -f "$BIN_PATH"
    echo "  [OK] Binary removed"
fi

echo ""
echo "Uninstall complete."
echo ""
