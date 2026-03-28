#!/bin/bash

set -euo pipefail

INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_PATH="$HOME/.cargo/bin/ivlyrics-musicxmatch-server"
EXTENSIONS_DIR="$HOME/.config/spicetify/Extensions"
ADDON_NAMES=("Addon_Lyrics_MusicXMatch.js" "Addon_Lyrics_Deezer.js" "Addon_Lyrics_Bugs.js" "Addon_Lyrics_Genie.js")

close_spotify_if_running() {
    if pgrep -x "Spotify" >/dev/null 2>&1 || pgrep -x "spotify" >/dev/null 2>&1; then
        echo "  [INFO] Spotify is running. Closing it before spicetify apply..."
        pkill -x "Spotify" >/dev/null 2>&1 || true
        pkill -x "spotify" >/dev/null 2>&1 || true
        sleep 2
    fi
}

remove_addon_from_spicetify() {
    local addon_name="$1"
    spicetify config "extensions-$addon_name" >/dev/null 2>&1 || true
}

echo ""
echo "Removing MusicXMatch Provider..."

close_spotify_if_running

if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/$SERVICE_LABEL.plist"
    if [ -f "$PLIST" ]; then
        launchctl bootout "gui/$(id -u)/$SERVICE_LABEL" >/dev/null 2>&1 || true
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

if command -v spicetify >/dev/null 2>&1; then
    for addon_name in "${ADDON_NAMES[@]}"; do
        remove_addon_from_spicetify "$addon_name"
        if [ -f "$EXTENSIONS_DIR/$addon_name" ]; then
            rm -f "$EXTENSIONS_DIR/$addon_name"
            echo "  [OK] Removed addon file: $addon_name"
        fi
    done

    if spicetify apply >/dev/null 2>&1; then
        echo "  [OK] Spicetify apply completed"
    else
        echo "  [WARN] Spicetify apply failed. Run 'spicetify apply' manually if needed."
    fi
else
    echo "  [WARN] spicetify not found. Addon files/config may need manual cleanup."
fi

echo ""
echo "Uninstall complete."
echo ""
