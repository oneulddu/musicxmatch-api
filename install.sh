#!/bin/bash
set -e

echo "=== ivLyrics MusicXMatch Provider Installer ==="
echo ""

INSTALL_DIR="$HOME/.ivlyrics-musicxmatch"
SERVICE_LABEL="com.ivlyrics.musicxmatch"
BIN_DIR="$HOME/.cargo/bin"
BIN_PATH="$BIN_DIR/ivlyrics-musicxmatch-server"

echo "[1/5] Creating installation directory..."
mkdir -p "$INSTALL_DIR"

echo "[2/5] Ensuring Rust toolchain is available..."
if ! command -v cargo >/dev/null 2>&1; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install rust
    else
        echo "cargo is required. Install Rust first: https://rustup.rs"
        exit 1
    fi
fi

echo "[3/5] Installing server binary..."
cargo install --git https://github.com/oneulddu/musicxmatch-api.git --bin ivlyrics-musicxmatch-server --force

echo "[4/5] Setting up auto-start..."
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

echo "[5/5] Starting server..."
sleep 2

echo ""
echo "✓ Installation complete!"
echo "Server running at http://127.0.0.1:8092"
echo ""
