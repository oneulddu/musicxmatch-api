#!/bin/bash

set -euo pipefail

INSTALL_DIR="$HOME/.config/ivLyrics/musicxmatch-provider"
SERVICE_NAME="ivlyrics-musicxmatch-provider"

echo ""
echo "Removing MusicXMatch Provider..."

if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/$SERVICE_NAME.plist"
    if [ -f "$PLIST" ]; then
        launchctl unload "$PLIST" >/dev/null 2>&1 || true
        rm -f "$PLIST"
        echo "  [OK] LaunchAgent removed"
    fi
elif command -v systemctl >/dev/null 2>&1; then
    systemctl --user stop "$SERVICE_NAME.service" >/dev/null 2>&1 || true
    systemctl --user disable "$SERVICE_NAME.service" >/dev/null 2>&1 || true
    rm -f "$HOME/.config/systemd/user/$SERVICE_NAME.service"
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    echo "  [OK] systemd user service removed"
fi

pkill -f "musicxmatch-addon-server --host 127.0.0.1" >/dev/null 2>&1 || true

if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
    echo "  [OK] Install directory removed"
fi

echo ""
echo "Uninstall complete."
echo ""
