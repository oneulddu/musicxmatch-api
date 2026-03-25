#!/bin/bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <version>"
    exit 1
fi

VERSION="$1"
TODAY="$(date +%F)"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version must match semver: x.y.z"
    exit 1
fi

perl -0pi -e "s/version = \"[0-9]+\\.[0-9]+\\.[0-9]+\"/version = \"$VERSION\"/" \
    "$ROOT_DIR/Cargo.toml"

perl -0pi -e "s/\\*\\s+\\@version\\s+[0-9]+\\.[0-9]+\\.[0-9]+/ * \\@version     $VERSION/" \
    "$ROOT_DIR/Addon_Lyrics_MusicXMatch.js"

perl -0pi -e "s/version: '[0-9]+\\.[0-9]+\\.[0-9]+'/version: '$VERSION'/" \
    "$ROOT_DIR/Addon_Lyrics_MusicXMatch.js"

perl -0pi -e "s/\\*\\s+\\@version\\s+[0-9]+\\.[0-9]+\\.[0-9]+/ * \\@version     $VERSION/" \
    "$ROOT_DIR/Addon_Lyrics_Deezer.js"

perl -0pi -e "s/version: '[0-9]+\\.[0-9]+\\.[0-9]+'/version: '$VERSION'/" \
    "$ROOT_DIR/Addon_Lyrics_Deezer.js"

perl -0pi -e "s/\\*\\s+\\@version\\s+[0-9]+\\.[0-9]+\\.[0-9]+/ * \\@version     $VERSION/" \
    "$ROOT_DIR/Addon_Lyrics_Bugs.js"

perl -0pi -e "s/version: '[0-9]+\\.[0-9]+\\.[0-9]+'/version: '$VERSION'/" \
    "$ROOT_DIR/Addon_Lyrics_Bugs.js"

perl -0pi -e "s/\"version\": \"[0-9]+\\.[0-9]+\\.[0-9]+\"/\"version\": \"$VERSION\"/" \
    "$ROOT_DIR/manifest.json"

perl -0pi -e "s/\"updated\": \"[0-9]{4}-[0-9]{2}-[0-9]{2}\"/\"updated\": \"$TODAY\"/" \
    "$ROOT_DIR/manifest.json"

perl -0pi -e "s/\"server\": \"[0-9]+\\.[0-9]+\\.[0-9]+\"/\"server\": \"$VERSION\"/" \
    "$ROOT_DIR/version.json"

perl -0pi -e "s/\"addon\": \"[0-9]+\\.[0-9]+\\.[0-9]+\"/\"addon\": \"$VERSION\"/" \
    "$ROOT_DIR/version.json"

echo "Updated project version to $VERSION"
