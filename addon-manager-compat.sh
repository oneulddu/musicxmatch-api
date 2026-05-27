#!/bin/sh
set -eu

if [ "$#" -eq 0 ]; then
    echo "Usage: addon-manager-compat.sh <addon-url> [<addon-url> ...]" >&2
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required." >&2
    exit 1
fi

ADDON_DIR="${HOME}/.config/spicetify/CustomApps/ivLyrics"
SOURCES_DIR="${HOME}/.config/spicetify/ivLyrics"
MANIFEST_PATH="${ADDON_DIR}/manifest.json"
SOURCES_PATH="${SOURCES_DIR}/addon_sources.json"

if [ ! -f "$MANIFEST_PATH" ]; then
    echo "ivLyrics manifest not found at $MANIFEST_PATH" >&2
    exit 1
fi

mkdir -p "$ADDON_DIR" "$SOURCES_DIR"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ivlyrics-addon-compat.XXXXXX")"
BACKUP_DIR="$TMP_DIR/backup"
BACKUP_LIST="$TMP_DIR/backup-list"
BACKUP_INDEX=0
REPO_RAW_MAIN_PREFIX="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/"
PATCH_SCRIPT_PATH="scripts/patch-ivlyrics-selection.sh"
resolved_ref=""

mkdir -p "$BACKUP_DIR"
: > "$BACKUP_LIST"

spotify_was_running=0

is_spotify_running() {
    pgrep -x "Spotify" >/dev/null 2>&1 || pgrep -x "spotify" >/dev/null 2>&1
}

stop_spotify_if_running() {
    if is_spotify_running; then
        spotify_was_running=1
        if command -v osascript >/dev/null 2>&1; then
            osascript -e 'tell application "Spotify" to quit' >/dev/null 2>&1 || true
        fi
        pkill -x "Spotify" >/dev/null 2>&1 || true
        pkill -x "spotify" >/dev/null 2>&1 || true
        sleep 2
    fi
}

restart_spotify_if_needed() {
    if [ "$spotify_was_running" -ne 1 ]; then
        return
    fi
    spotify_was_running=0
    if command -v open >/dev/null 2>&1; then
        open -a Spotify >/dev/null 2>&1 && return
    fi
    if command -v spotify >/dev/null 2>&1; then
        spotify >/dev/null 2>&1 &
    fi
}

backup_path() {
    path="$1"
    BACKUP_INDEX=$((BACKUP_INDEX + 1))
    backup_file="$BACKUP_DIR/$BACKUP_INDEX"
    printf '%s
%s
' "$path" "$backup_file" >> "$BACKUP_LIST"
    if [ -e "$path" ]; then
        cp -p "$path" "$backup_file"
    else
        : > "$backup_file.missing"
    fi
}

restore_backups() {
    if [ ! -f "$BACKUP_LIST" ]; then
        return
    fi
    while IFS= read -r path && IFS= read -r backup_file; do
        if [ -f "$backup_file.missing" ]; then
            rm -f "$path"
        elif [ -f "$backup_file" ]; then
            mkdir -p "$(dirname -- "$path")"
            cp -p "$backup_file" "$path"
        fi
    done < "$BACKUP_LIST"
}

cleanup() {
    restart_spotify_if_needed
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM

resolve_repo_ref() {
    if [ -n "$resolved_ref" ]; then
        return
    fi

    resolved_ref="$(
        curl -fsSL "https://api.github.com/repos/oneulddu/musicxmatch-api/commits/main" \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["sha"])' 2>/dev/null || true
    )"
}

for url in "$@"; do
    clean_url="${url%%\?*}"
    filename=$(basename "$clean_url")
    download_url="$url"
    case "$filename" in
        *.js) ;;
        *)
            echo "Invalid addon URL: $url" >&2
            exit 1
            ;;
    esac

    case "$clean_url" in
        "$REPO_RAW_MAIN_PREFIX"*)
            resolve_repo_ref
            if [ -n "$resolved_ref" ]; then
                download_url="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/$resolved_ref/${clean_url#"$REPO_RAW_MAIN_PREFIX"}"
            else
                separator='?'
                case "$url" in
                    *\?*) separator='&' ;;
                esac
                download_url="${url}${separator}ts=$(date +%s)"
            fi
            ;;
        https://raw.githubusercontent.com/*)
            separator='?'
            case "$url" in
                *\?*) separator='&' ;;
            esac
            download_url="${url}${separator}ts=$(date +%s)"
            ;;
    esac

    curl -fsSL "$download_url" -o "$TMP_DIR/$filename"
done

backup_path "$SOURCES_PATH"
backup_path "$MANIFEST_PATH"
backup_path "$ADDON_DIR/LyricsAddonManager.js"
for url in "$@"; do
    clean_url="${url%%\?*}"
    filename=$(basename "$clean_url")
    backup_path "$ADDON_DIR/$filename"
done

if ! python3 - "$ADDON_DIR" "$SOURCES_PATH" "$MANIFEST_PATH" "$TMP_DIR" "$@" <<'PY'
import json
import sys
from pathlib import Path

addon_dir = Path(sys.argv[1])
sources_path = Path(sys.argv[2])
manifest_path = Path(sys.argv[3])
tmp_dir = Path(sys.argv[4])
urls = sys.argv[5:]

sources = {}
if sources_path.exists():
    try:
        sources = json.loads(sources_path.read_text())
    except json.JSONDecodeError:
        sources = {}

manifest = json.loads(manifest_path.read_text())
subfiles = manifest.get("subfiles_extension", [])
if not isinstance(subfiles, list):
    subfiles = []

for url in urls:
    clean_url = url.split("?")[0]
    filename = clean_url.rsplit("/", 1)[-1]
    source_file = tmp_dir / filename
    target_file = addon_dir / filename
    target_file.write_text(source_file.read_text())
    sources[filename] = clean_url
    if filename not in subfiles:
        subfiles.append(filename)

manifest["subfiles_extension"] = subfiles
sources_path.write_text(json.dumps(sources, indent=4, ensure_ascii=False) + "\n")
manifest_path.write_text(json.dumps(manifest, indent="\t", ensure_ascii=False) + "\n")

print("Registered addons:")
for url in urls:
    print(f" - {url.split('?')[0].rsplit('/', 1)[-1]}")
PY
then
    :
else
    restore_backups
    exit 1
fi

if command -v spicetify >/dev/null 2>&1; then
    patch_script="$TMP_DIR/patch-ivlyrics-selection.sh"
    patch_url="${REPO_RAW_MAIN_PREFIX}${PATCH_SCRIPT_PATH}"
    local_patch_script=""

    if [ -n "${0:-}" ] && [ "${0#-}" = "$0" ]; then
        script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || true)"
        if [ -n "$script_dir" ] && [ -f "$script_dir/$PATCH_SCRIPT_PATH" ]; then
            local_patch_script="$script_dir/$PATCH_SCRIPT_PATH"
        fi
    fi
    if [ -z "$local_patch_script" ] && [ -f "./$PATCH_SCRIPT_PATH" ]; then
        local_patch_script="./$PATCH_SCRIPT_PATH"
    fi

    if [ -n "$local_patch_script" ]; then
        if ! sh "$local_patch_script" --no-apply "$ADDON_DIR/LyricsAddonManager.js"; then
            echo "ivLyrics selection patch failed; continuing without it." >&2
        fi
    elif curl -fsSL "$patch_url?ts=$(date +%s)" -o "$patch_script"; then
        if ! sh "$patch_script" --no-apply "$ADDON_DIR/LyricsAddonManager.js"; then
            echo "ivLyrics selection patch failed; continuing without it." >&2
        fi
    else
        echo "ivLyrics selection patch download failed; continuing without it." >&2
    fi

    stop_spotify_if_running
    if ! spicetify apply; then
        restore_backups
        exit 1
    fi
else
    echo "spicetify not found; addon files were registered but apply was skipped." >&2
fi
