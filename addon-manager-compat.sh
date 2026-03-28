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
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

for url in "$@"; do
    filename=$(basename "${url%%\?*}")
    case "$filename" in
        *.js) ;;
        *)
            echo "Invalid addon URL: $url" >&2
            exit 1
            ;;
    esac

    curl -fsSL "$url" -o "$TMP_DIR/$filename"
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

for url in urls:
    filename = url.split("?")[0].rsplit("/", 1)[-1]
    source_file = tmp_dir / filename
    target_file = addon_dir / filename
    target_file.write_text(source_file.read_text())
    sources[filename] = url
    if filename not in subfiles:
        subfiles.append(filename)

manifest["subfiles_extension"] = subfiles
sources_path.write_text(json.dumps(sources, indent=4, ensure_ascii=False) + "\n")
manifest_path.write_text(json.dumps(manifest, indent="\t", ensure_ascii=False) + "\n")

print("Registered addons:")
for url in urls:
    print(f" - {url.split('?')[0].rsplit('/', 1)[-1]}")
PY

if command -v spicetify >/dev/null 2>&1; then
    spicetify apply
else
    echo "spicetify not found; addon files were registered but apply was skipped." >&2
fi
