#!/bin/bash

set -euo pipefail

HOME="${HOME:?HOME is required}"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_PATH="$CARGO_HOME/bin/ivlyrics-musicxmatch-server"
UPDATE_RESIDUALS=(
    "$INSTALL_DIR/update.lock"
    "$INSTALL_DIR/run-update.sh"
    "$INSTALL_DIR/update.log"
)

is_server_running() {
    pgrep -f "ivlyrics-musicxmatch-server" >/dev/null 2>&1
}

wait_for_server_stop() {
    for _ in $(seq 1 20); do
        if ! is_server_running; then
            return 0
        fi
        sleep 0.2
    done
    return 1
}

echo ""
echo "Removing local lyrics server..."

if [[ "$OSTYPE" == "darwin"* ]]; then
    PLIST="$HOME/Library/LaunchAgents/$SERVICE_LABEL.plist"
    launchctl bootout "gui/$(id -u)/$SERVICE_LABEL" >/dev/null 2>&1 || true
    if [ -f "$PLIST" ]; then
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
if ! wait_for_server_stop; then
    echo "  [WARN] Server process may still be running. Please close it manually if uninstall cannot remove the binary." >&2
fi

for path in "${UPDATE_RESIDUALS[@]}"; do
    if [ -e "$path" ]; then
        rm -f "$path"
        echo "  [OK] Update residual removed: $path"
    fi
done

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
echo "Addon removal is managed separately by ivLyrics addon-manager."
echo ""
