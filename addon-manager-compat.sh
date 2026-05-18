#!/bin/sh
set -eu

if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required." >&2
    exit 1
fi

ADDON_DIR="${HOME}/.config/spicetify/CustomApps/ivLyrics"
SOURCES_DIR="${HOME}/.config/spicetify/ivLyrics"
MANIFEST_PATH="${ADDON_DIR}/manifest.json"
SOURCES_PATH="${SOURCES_DIR}/addon_sources.json"
REPO_RAW_MAIN_PREFIX="https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/"
KNOWN_ADDONS="Addon_Lyrics_MusicXMatch.js Addon_Lyrics_Deezer.js Addon_Lyrics_Bugs.js Addon_Lyrics_Genie.js"
RESTORE_FROM_SOURCES=0
resolved_ref=""

if [ "$#" -gt 0 ] && { [ "$1" = "--restore" ] || [ "$1" = "--restore-existing" ]; }; then
    RESTORE_FROM_SOURCES=1
    shift
fi

if [ "$#" -eq 0 ] && [ "$RESTORE_FROM_SOURCES" -eq 0 ]; then
    RESTORE_FROM_SOURCES=1
fi

if [ ! -f "$MANIFEST_PATH" ]; then
    echo "ivLyrics manifest not found at $MANIFEST_PATH" >&2
    exit 1
fi

mkdir -p "$ADDON_DIR" "$SOURCES_DIR"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ivlyrics-addon-compat.XXXXXX")"

spotify_was_running=0

stop_spotify_if_running() {
    if pgrep -x "Spotify" >/dev/null 2>&1; then
        spotify_was_running=1
        if command -v osascript >/dev/null 2>&1; then
            osascript -e 'tell application "Spotify" to quit' >/dev/null 2>&1 || true
        fi
        pkill -x "Spotify" >/dev/null 2>&1 || true
        sleep 2
    fi
}

restart_spotify_if_needed() {
    if [ "$spotify_was_running" -eq 1 ]; then
        open -a Spotify >/dev/null 2>&1 || true
    fi
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

if [ "$RESTORE_FROM_SOURCES" -eq 1 ]; then
    RESTORE_URLS_PATH="$TMP_DIR/restore_urls.txt"
    python3 - "$SOURCES_PATH" "$RESTORE_URLS_PATH" $KNOWN_ADDONS <<'PY'
import json
import sys
from pathlib import Path

sources_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])
known = sys.argv[3:]
if not sources_path.exists():
    sys.exit(0)

try:
    sources = json.loads(sources_path.read_text())
except (json.JSONDecodeError, OSError):
    sys.exit(0)

if not isinstance(sources, dict):
    sys.exit(0)

with output_path.open("w", encoding="utf-8") as output:
    for name in known:
        url = sources.get(name)
        if isinstance(url, str) and (url.startswith("http") or url.startswith("local:")):
            output.write(url + "\n")
PY
    if [ -f "$RESTORE_URLS_PATH" ]; then
        while IFS= read -r restored_url || [ -n "$restored_url" ]; do
            [ -n "$restored_url" ] || continue
            set -- "$@" "$restored_url"
        done < "$RESTORE_URLS_PATH"
    fi
fi

if [ "$#" -eq 0 ]; then
    echo "No addon URLs were provided and no restorable provider sources were found." >&2
    echo "Usage: addon-manager-compat.sh [--restore] <addon-url> [<addon-url> ...]" >&2
    exit 1
fi

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

    case "$clean_url" in
        local:*)
            local_path="${clean_url#local:}"
            cp "$local_path" "$TMP_DIR/$filename"
            ;;
        *)
            curl -fsSL "$download_url" -o "$TMP_DIR/$filename"
            ;;
    esac
done

python3 - "$ADDON_DIR" "$SOURCES_PATH" "$MANIFEST_PATH" "$TMP_DIR" "$@" <<'PY'
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

registered = []
for url in urls:
    clean_url = url.split("?")[0]
    filename = clean_url.rsplit("/", 1)[-1]
    source_file = tmp_dir / filename
    target_file = addon_dir / filename
    target_file.write_text(source_file.read_text())
    sources[filename] = clean_url
    if filename not in subfiles:
        subfiles.append(filename)
    registered.append(filename)

manifest["subfiles_extension"] = subfiles
sources_path.write_text(json.dumps(sources, indent=4, ensure_ascii=False) + "\n")
manifest_path.write_text(json.dumps(manifest, indent="\t", ensure_ascii=False) + "\n")

print("Registered addons:")
for filename in registered:
    print(f" - {filename}")
PY

if command -v spicetify >/dev/null 2>&1; then
    stop_spotify_if_running
    spicetify apply
else
    echo "spicetify not found; addon files were registered but apply was skipped." >&2
fi
