#!/bin/bash
set -Eeuo pipefail

HOME="${HOME:?HOME is required}"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_DIR="$CARGO_HOME/bin"
BIN_PATH="$BIN_DIR/ivlyrics-musicxmatch-server"
SERVER_URL="http://127.0.0.1:8092"
RUNTIME_PATH="$BIN_DIR:$HOME/.spicetify:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"
RAW_BASE_URL="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main"
ADDON_URLS=(
    "$RAW_BASE_URL/Addon_Lyrics_MusicXMatch.js"
    "$RAW_BASE_URL/Addon_Lyrics_Deezer.js"
    "$RAW_BASE_URL/Addon_Lyrics_Bugs.js"
    "$RAW_BASE_URL/Addon_Lyrics_Genie.js"
)
SKIP_ADDONS="${IVLYRICS_SKIP_ADDONS:-0}"
SERVICE_WAS_LOADED=0
SERVER_WAS_RUNNING=0
SERVER_STOPPED=0
INSTALL_COMPLETED=0
PREVIOUS_BIN_BACKUP="$INSTALL_DIR/previous-server.bin"

export PATH="$RUNTIME_PATH"

xml_escape() {
    printf "%s" "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' -e 's/"/\&quot;/g' -e "s/'/\&apos;/g"
}

systemd_escape_arg() {
    printf '"%s"' "$(printf "%s" "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')"
}
is_server_running() {
    pgrep -f "ivlyrics-musicxmatch-server" >/dev/null 2>&1
}

wait_for_server_stop() {
    for _ in $(seq 1 30); do
        if ! is_server_running; then
            return 0
        fi
        sleep 0.2
    done
    return 1
}

backup_previous_binary() {
    rm -f "$PREVIOUS_BIN_BACKUP"
    if [[ -f "$BIN_PATH" ]]; then
        mkdir -p "$INSTALL_DIR"
        cp -p "$BIN_PATH" "$PREVIOUS_BIN_BACKUP"
    fi
}

restore_previous_binary() {
    if [[ -f "$PREVIOUS_BIN_BACKUP" ]]; then
        mkdir -p "$(dirname "$BIN_PATH")"
        cp -p "$PREVIOUS_BIN_BACKUP" "$BIN_PATH"
        chmod +x "$BIN_PATH" 2>/dev/null || true
    fi
}

stop_existing_server() {
    if is_server_running; then
        SERVER_WAS_RUNNING=1
    fi

    if [[ "$OSTYPE" == "darwin"* ]]; then
        local domain="gui/$(id -u)"
        local service="$domain/$SERVICE_LABEL"
        if launchctl print "$service" >/dev/null 2>&1; then
            SERVICE_WAS_LOADED=1
            SERVER_WAS_RUNNING=1
            launchctl bootout "$service" >/dev/null 2>&1 || true
        fi
    elif command -v systemctl >/dev/null 2>&1; then
        if systemctl --user is-active --quiet ivlyrics-musicxmatch.service 2>/dev/null; then
            SERVICE_WAS_LOADED=1
            SERVER_WAS_RUNNING=1
        fi
        systemctl --user stop ivlyrics-musicxmatch.service >/dev/null 2>&1 || true
    fi

    if [ "$SERVER_WAS_RUNNING" -eq 1 ]; then
        SERVER_STOPPED=1
    fi
    pkill -f "ivlyrics-musicxmatch-server" >/dev/null 2>&1 || true
    if ! wait_for_server_stop; then
        echo "Existing server did not stop cleanly. Please close ivlyrics-musicxmatch-server and retry." >&2
        return 1
    fi
}

restart_previous_server_after_failure() {
    echo "Installation failed; trying to restore and restart the previously installed server if available." >&2
    restore_previous_binary
    if [[ "$OSTYPE" == "darwin"* ]]; then
        local plist="$HOME/Library/LaunchAgents/$SERVICE_LABEL.plist"
        if [[ -f "$plist" ]]; then
            launchctl bootstrap "gui/$(id -u)" "$plist" >/dev/null 2>&1 || true
            launchctl kickstart -k "gui/$(id -u)/$SERVICE_LABEL" >/dev/null 2>&1 || true
            return
        fi
    elif command -v systemctl >/dev/null 2>&1; then
        if systemctl --user start ivlyrics-musicxmatch.service >/dev/null 2>&1; then
            return
        fi
    fi

    if [[ -x "$BIN_PATH" ]]; then
        MXM_SESSION_FILE="$INSTALL_DIR/musixmatch_session.json" \
        IVLYRICS_MXM_LOG="$INSTALL_DIR/server.log" \
        nohup "$BIN_PATH" >/dev/null 2>&1 &
    fi
}

restart_previous_server_on_error() {
    local exit_code=$?
    if [ "$INSTALL_COMPLETED" -ne 1 ] && [ "$SERVER_STOPPED" -eq 1 ] && [ "$SERVER_WAS_RUNNING" -eq 1 ]; then
        restart_previous_server_after_failure
    fi
    exit "$exit_code"
}

trap restart_previous_server_on_error ERR

verify_server() {
    local last_error=""
    for attempt in $(seq 1 30); do
        if headers="$(curl -fsS -D - -o /dev/null --max-time 2 "$SERVER_URL/ready" 2>&1)"; then
            if printf "%s" "$headers" | tr -d '\r' | grep -qi '^access-control-allow-origin: \*$'; then
                return 0
            fi
            last_error="CORS header missing"
        else
            last_error="$headers"
        fi
        sleep 1
    done

    echo "Server health check failed: $SERVER_URL/ready" >&2
    if [[ -n "$last_error" ]]; then
        echo "$last_error" >&2
    fi
    return 1
}

write_launch_agent() {
    local plist="$HOME/Library/LaunchAgents/$SERVICE_LABEL.plist"
    local escaped_bin_path escaped_session_file escaped_log_file escaped_runtime_path
    escaped_bin_path="$(xml_escape "$BIN_PATH")"
    escaped_session_file="$(xml_escape "$INSTALL_DIR/musixmatch_session.json")"
    escaped_log_file="$(xml_escape "$INSTALL_DIR/server.log")"
    escaped_runtime_path="$(xml_escape "$RUNTIME_PATH")"

    mkdir -p "$(dirname "$plist")"
    cat > "$plist" <<EOF_PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$SERVICE_LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>$escaped_bin_path</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>MXM_SESSION_FILE</key>
        <string>$escaped_session_file</string>
        <key>IVLYRICS_MXM_LOG</key>
        <string>$escaped_log_file</string>
        <key>PATH</key>
        <string>$escaped_runtime_path</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF_PLIST

    launchctl bootout "gui/$(id -u)/$SERVICE_LABEL" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$plist"
}

write_systemd_user_service() {
    local service_file="$HOME/.config/systemd/user/ivlyrics-musicxmatch.service"
    local escaped_bin_path
    escaped_bin_path="$(systemd_escape_arg "$BIN_PATH")"
    mkdir -p "$HOME/.config/systemd/user"
    cat > "$service_file" <<EOF_SERVICE
[Unit]
Description=ivLyrics MusicXMatch Server

[Service]
Environment="MXM_SESSION_FILE=$INSTALL_DIR/musixmatch_session.json"
Environment="IVLYRICS_MXM_LOG=$INSTALL_DIR/server.log"
Environment="PATH=$RUNTIME_PATH"
ExecStart=$escaped_bin_path
Restart=always

[Install]
WantedBy=default.target
EOF_SERVICE
    systemctl --user daemon-reload
    systemctl --user enable ivlyrics-musicxmatch.service
}

echo "=== ivLyrics Lyrics Providers Installer ==="
echo ""

echo "[1/8] Creating installation directory..."
mkdir -p "$INSTALL_DIR"

echo "[2/8] Ensuring Rust toolchain is available..."
if ! command -v cargo >/dev/null 2>&1; then
    if [[ "$OSTYPE" == "darwin"* ]] && command -v brew >/dev/null 2>&1; then
        brew install rust
    else
        echo "cargo is required. Install Rust first: https://rustup.rs" >&2
        exit 1
    fi
fi

echo "[3/8] Stopping existing server if running..."
stop_existing_server

echo "[4/8] Installing server binary..."
backup_previous_binary
cargo install --git https://github.com/oneulddu/musicxmatch-api.git --bin ivlyrics-musicxmatch-server --force

echo "[5/8] Setting up login auto-start..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    write_launch_agent
else
    write_systemd_user_service
fi

echo "[6/8] Registering addons..."
if [[ "$SKIP_ADDONS" == "1" ]]; then
    echo "Addon registration skipped by IVLYRICS_SKIP_ADDONS=1."
elif command -v spicetify >/dev/null 2>&1; then
    compat_script="$(mktemp "${TMPDIR:-/tmp}/ivlyrics-addon-manager.XXXXXX")"
    if curl -fsSL "$RAW_BASE_URL/addon-manager-compat.sh?ts=$(date +%s)" -o "$compat_script" && sh "$compat_script" "${ADDON_URLS[@]}"; then
        echo "Addons registered successfully."
    else
        echo "Addon registration failed. Server install succeeded, but addon registration needs manual retry."
        echo "Manual command:"
        echo "  curl -fsSL \"$RAW_BASE_URL/addon-manager-compat.sh\" | sh -s -- \"${ADDON_URLS[0]}\" \"${ADDON_URLS[1]}\" \"${ADDON_URLS[2]}\" \"${ADDON_URLS[3]}\""
    fi
    rm -f "$compat_script"
else
    echo "spicetify was not found, so addon registration was skipped."
    echo "Run this after installing/configuring spicetify:"
    echo "  curl -fsSL \"$RAW_BASE_URL/addon-manager-compat.sh\" | sh -s -- \"${ADDON_URLS[0]}\" \"${ADDON_URLS[1]}\" \"${ADDON_URLS[2]}\" \"${ADDON_URLS[3]}\""
fi

echo "[7/8] Starting server..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    launchctl kickstart -k "gui/$(id -u)/$SERVICE_LABEL"
else
    systemctl --user restart ivlyrics-musicxmatch.service
fi

echo "[8/8] Verifying health and CORS..."
verify_server
INSTALL_COMPLETED=1
rm -f "$PREVIOUS_BIN_BACKUP"

echo ""
echo "✓ Installation complete!"
echo "Server running at $SERVER_URL"
echo ""
