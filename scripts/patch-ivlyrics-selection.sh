#!/bin/sh
set -eu

DRY_RUN=0
APPLY=1
TARGET_PATH=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run|-n)
            DRY_RUN=1
            APPLY=0
            shift
            ;;
        --no-apply)
            APPLY=0
            shift
            ;;
        --help|-h)
            cat <<'EOF_HELP'
Usage: patch-ivlyrics-selection.sh [--dry-run|-n] [--no-apply] [LyricsAddonManager.js]

Patch ivLyrics provider selection so the best lyrics type wins:
karaoke > synced > unsynced.

Options:
  --dry-run, -n   Show whether a patch is needed without writing or applying.
  --no-apply      Patch the file but skip spicetify apply / Spotify restart.
EOF_HELP
            exit 0
            ;;
        --*)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
        *)
            if [ -n "$TARGET_PATH" ]; then
                echo "Unexpected extra argument: $1" >&2
                exit 1
            fi
            TARGET_PATH="$1"
            shift
            ;;
    esac
done

TARGET_PATH="${TARGET_PATH:-${HOME}/.config/spicetify/CustomApps/ivLyrics/LyricsAddonManager.js}"
ROLLBACK_COPY=""
if [ ! -f "$TARGET_PATH" ]; then
    echo "LyricsAddonManager.js not found: $TARGET_PATH" >&2
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required to patch LyricsAddonManager.js." >&2
    exit 1
fi

if [ "$DRY_RUN" -eq 0 ] && [ "$APPLY" -eq 1 ]; then
    ROLLBACK_COPY="$(mktemp "${TMPDIR:-/tmp}/ivlyrics-lyrics-manager.XXXXXX")"
    cp "$TARGET_PATH" "$ROLLBACK_COPY"
fi

python3 - "$TARGET_PATH" "$DRY_RUN" <<'PY'
import sys
from datetime import datetime, timezone
from pathlib import Path

path = Path(sys.argv[1]).expanduser().resolve()
dry_run = sys.argv[2] == "1"
source = path.read_text(encoding="utf-8")


def replace_once(text: str, needle: str, replacement: str, label: str) -> str:
    if needle not in text:
        raise SystemExit(f"Patch target block not found: {label}")
    return text.replace(needle, replacement, 1)


def ensure_helpers(text: str) -> str:
    if "scoreLyricsResult(result)" in text:
        return text

    score_helper = """
    function scoreLyricsResult(result) {
        const hasKaraoke = hasLyricsContent(result?.karaoke);
        const hasSynced = hasLyricsContent(result?.synced);
        const hasUnsynced = hasLyricsContent(result?.unsynced);

        if (hasKaraoke) return 3;
        if (hasSynced) return 2;
        if (hasUnsynced) return 1;
        return 0;
    }
"""

    if "function hasLyricsContent(lines)" in text:
        anchor = """    function hasLyricsContent(lines) {
        return Array.isArray(lines) && lines.length > 0;
    }
"""
        return replace_once(text, anchor, anchor + score_helper, "score helper insertion")

    anchor = """    const LYRICS_TYPES = {
        KARAOKE: 'karaoke',     // 노래방 가사 (단어별 타이밍)
        SYNCED: 'synced',       // 싱크 가사 (줄별 타이밍)
        UNSYNCED: 'unsynced'    // 일반 가사 (타이밍 없음)
    };
"""
    helpers = anchor + """
    function hasLyricsContent(lines) {
        return Array.isArray(lines) && lines.length > 0;
    }
""" + score_helper
    return replace_once(text, anchor, helpers, "LYRICS_TYPES helper insertion")


def patch_get_lyrics_state(text: str) -> str:
    if "let bestResult = null;" in text:
        return text

    old = """            const trackId = info.uri?.split(':')[2];

            // 디버그 로깅"""
    new = """            const trackId = info.uri?.split(':')[2];
            let bestResult = null;
            let bestScore = 0;
            let bestMeta = null;

            // 디버그 로깅"""
    return replace_once(text, old, new, "best result state insertion")


def patch_cache_source(text: str) -> str:
    if "let resultSource = 'provider';" in text:
        return text

    text = replace_once(
        text,
        """                    // 0. IndexedDB 캐시 확인
                    let result = null;
""",
        """                    // 0. IndexedDB 캐시 확인
                    let result = null;
                    let resultSource = 'provider';
""",
        "resultSource declaration",
    )
    text = replace_once(
        text,
        """                                result = cached;
                                window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Cache hit for ${provider.id}`);""",
        """                                result = cached;
                                resultSource = 'cache';
                                window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Cache hit for ${provider.id}`);""",
        "cache source assignment",
    )
    text = replace_once(
        text,
        """                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Got lyrics from: ${provider.id}`, {
                        hasKaraoke: !!result.karaoke,""",
        """                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Got lyrics from: ${provider.id}`, {
                        source: resultSource,
                        hasKaraoke: !!result.karaoke,""",
        "source debug field",
    )
    return text


def patch_content_checks(text: str) -> str:
    text = text.replace(
        """                    const needsKaraoke = allowKaraoke && !result.karaoke;
                    const hasBaseLyrics = result.synced || result.unsynced;""",
        """                    const needsKaraoke = allowKaraoke && !hasLyricsContent(result.karaoke);
                    const hasBaseLyrics = hasLyricsContent(result.synced) || hasLyricsContent(result.unsynced);""",
        1,
    )

    if "const hasKaraoke = hasLyricsContent(finalResult.karaoke);" in text:
        return text

    old = """                    if (!allowKaraoke) finalResult.karaoke = null;
                    if (!allowSynced) finalResult.synced = null;
                    if (!allowUnsynced) finalResult.unsynced = null;

                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] After filtering for ${provider.id}:`, {
                        hasKaraoke: !!finalResult.karaoke,
                        hasSynced: !!finalResult.synced,
                        hasUnsynced: !!finalResult.unsynced
                    });

                    // 5. 허용된 가사가 있으면 반환
                    if (finalResult.karaoke || finalResult.synced || finalResult.unsynced) {"""
    new = """                    if (!allowKaraoke) finalResult.karaoke = null;
                    if (!allowSynced) finalResult.synced = null;
                    if (!allowUnsynced) finalResult.unsynced = null;

                    const hasKaraoke = hasLyricsContent(finalResult.karaoke);
                    const hasSynced = hasLyricsContent(finalResult.synced);
                    const hasUnsynced = hasLyricsContent(finalResult.unsynced);

                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] After filtering for ${provider.id}:`, {
                        hasKaraoke,
                        hasSynced,
                        hasUnsynced
                    });

                    // 5. 허용된 가사가 있으면 후보로 저장
                    if (hasKaraoke || hasSynced || hasUnsynced) {"""
    return replace_once(text, old, new, "final content checks")


def patch_immediate_return(text: str) -> str:
    if "Final best result selected" in text:
        return text

    old = """                        // 디버그 타이머 종료
                        if (window.AddonDebug?.isEnabled()) {
                            window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                            window.AddonDebug.log('lyrics', 'getLyrics success', {
                                provider: finalResult.provider,
                                hasKaraoke: !!finalResult.karaoke,
                                hasSynced: !!finalResult.synced,
                                hasUnsynced: !!finalResult.unsynced,
                                syncDataApplied: finalResult.syncDataApplied || false
                            });
                        }

                        // 이벤트 발생
                        this.emit('lyrics:fetch:success', {
                            uri: info.uri,
                            provider: finalResult.provider,
                            hasKaraoke: !!finalResult.karaoke,
                            hasSynced: !!finalResult.synced,
                            hasUnsynced: !!finalResult.unsynced,
                            syncDataApplied: finalResult.syncDataApplied || false
                        });

                        // IndexedDB에 캐시 저장
                        if (trackId && window.LyricsService?.cacheLyrics && !finalResult.skipCache) {
                            const cachePayload = { ...finalResult };
                            delete cachePayload.skipCache;
                            window.LyricsService.cacheLyrics(trackId, provider.id, cachePayload);
                        }

                        return finalResult;"""
    old_with_content_flags = """                        // 디버그 타이머 종료
                        if (window.AddonDebug?.isEnabled()) {
                            window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                            window.AddonDebug.log('lyrics', 'getLyrics success', {
                                provider: finalResult.provider,
                                hasKaraoke,
                                hasSynced,
                                hasUnsynced,
                                syncDataApplied: finalResult.syncDataApplied || false
                            });
                        }

                        // 이벤트 발생
                        this.emit('lyrics:fetch:success', {
                            uri: info.uri,
                            provider: finalResult.provider,
                            hasKaraoke,
                            hasSynced,
                            hasUnsynced,
                            syncDataApplied: finalResult.syncDataApplied || false
                        });

                        // IndexedDB에 캐시 저장
                        if (trackId && window.LyricsService?.cacheLyrics && !finalResult.skipCache) {
                            const cachePayload = { ...finalResult };
                            delete cachePayload.skipCache;
                            window.LyricsService.cacheLyrics(trackId, provider.id, cachePayload);
                        }

                        return finalResult;"""
    new = """                        // IndexedDB에 캐시 저장
                        if (trackId && window.LyricsService?.cacheLyrics && !finalResult.skipCache) {
                            const cachePayload = { ...finalResult };
                            delete cachePayload.skipCache;
                            window.LyricsService.cacheLyrics(trackId, provider.id, cachePayload);
                        }

                        const score = scoreLyricsResult(finalResult);
                        window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Candidate from ${provider.id} scored ${score}`, {
                            source: resultSource,
                            hasKaraoke,
                            hasSynced,
                            hasUnsynced,
                            currentBestScore: bestScore,
                            currentBestProvider: bestMeta?.provider || null
                        });

                        if (score > bestScore) {
                            bestScore = score;
                            bestResult = finalResult;
                            bestMeta = {
                                provider: finalResult.provider,
                                hasKaraoke,
                                hasSynced,
                                hasUnsynced,
                                syncDataApplied: finalResult.syncDataApplied || false
                            };

                            window.__ivLyricsDebugLog?.(`[LyricsAddonManager] ${provider.id} is now the best candidate`, {
                                provider: finalResult.provider,
                                score: bestScore,
                                hasKaraoke,
                                hasSynced,
                                hasUnsynced,
                                source: resultSource
                            });
                        } else {
                            window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Keeping current best over ${provider.id}`, {
                                candidateScore: score,
                                bestScore,
                                candidateProvider: finalResult.provider,
                                bestProvider: bestMeta?.provider || null,
                                source: resultSource
                            });
                        }

                        const stopScore = allowKaraoke ? 3 : 2;
                        if (bestScore >= stopScore) {
                            window.__ivLyricsDebugLog?.(
                                allowKaraoke
                                    ? '[LyricsAddonManager] Karaoke result found, stopping provider search early'
                                    : '[LyricsAddonManager] Synced result found with karaoke disabled, stopping provider search early'
                            );
                            break;
                        }

                        continue;"""

    for candidate in (old, old_with_content_flags):
        if candidate in text:
            text = text.replace(candidate, new, 1)
            break
    else:
        raise SystemExit("Patch target block not found: replace immediate return with best candidate selection")

    insertion = """
            if (bestResult) {
                window.__ivLyricsDebugLog?.('[LyricsAddonManager] Final best result selected', {
                    bestScore,
                    ...bestMeta
                });

                if (window.AddonDebug?.isEnabled()) {
                    window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                    window.AddonDebug.log('lyrics', 'getLyrics success', bestMeta);
                }

                this.emit('lyrics:fetch:success', {
                    uri: info.uri,
                    ...bestMeta
                });

                return bestResult;
            }
"""
    anchor = """
            // 디버그 타이머 종료
            if (window.AddonDebug?.isEnabled()) {"""
    return replace_once(text, anchor, insertion + anchor, "final best result return")


patched = source
patched = ensure_helpers(patched)
patched = patch_get_lyrics_state(patched)
patched = patch_cache_source(patched)
patched = patch_content_checks(patched)
patched = patch_immediate_return(patched)

if patched == source:
    if dry_run:
        print(f"Dry run: already patched: {path}")
    else:
        print(f"Already patched: {path}")
elif dry_run:
    print(f"Dry run: patch needed: {path}")
else:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")
    backup = path.with_name(path.name + f".bak-{stamp}")
    backup.write_text(source, encoding="utf-8")
    path.write_text(patched, encoding="utf-8")
    print(f"Patched: {path}")
    print(f"Backup:  {backup}")
PY

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

restore_from_rollback_copy() {
    if [ -n "$ROLLBACK_COPY" ] && [ -f "$ROLLBACK_COPY" ]; then
        cp "$ROLLBACK_COPY" "$TARGET_PATH" || true
        echo "Restored LyricsAddonManager.js because spicetify apply failed." >&2
    fi
}

cleanup() {
    restart_spotify_if_needed
    if [ -n "$ROLLBACK_COPY" ] && [ -f "$ROLLBACK_COPY" ]; then
        rm -f "$ROLLBACK_COPY"
    fi
}

trap cleanup EXIT INT TERM

if [ "$DRY_RUN" -eq 1 ]; then
    echo "Dry run: skipped spicetify apply."
elif [ "$APPLY" -eq 0 ]; then
    echo "Skipped spicetify apply by request."
elif command -v spicetify >/dev/null 2>&1; then
    stop_spotify_if_running
    if ! spicetify apply; then
        restore_from_rollback_copy
        exit 1
    fi
else
    echo "spicetify not found; patch was written but apply was skipped." >&2
fi
