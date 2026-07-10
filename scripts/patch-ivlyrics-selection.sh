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
real karaoke > synced or pseudo karaoke > unsynced.

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

TARGET_PATH="$(python3 - "$TARGET_PATH" <<'PY_RESOLVE'
import sys
from pathlib import Path

print(Path(sys.argv[1]).expanduser().resolve())
PY_RESOLVE
)"

OPERATION_SUCCEEDED=0
FINALIZING=0
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

create_rollback_copy() {
    ROLLBACK_COPY="$(mktemp "${TMPDIR:-/tmp}/ivlyrics-lyrics-manager.XXXXXX")" || return 1
    if ! cp -p "$TARGET_PATH" "$ROLLBACK_COPY"; then
        echo "Failed to create rollback copy: $ROLLBACK_COPY" >&2
        rm -f "$ROLLBACK_COPY"
        ROLLBACK_COPY=""
        return 1
    fi
    if ! cmp -s "$ROLLBACK_COPY" "$TARGET_PATH"; then
        echo "Rollback copy verification failed: $ROLLBACK_COPY" >&2
        rm -f "$ROLLBACK_COPY"
        ROLLBACK_COPY=""
        return 1
    fi
}

discard_rollback_copy() {
    if [ -z "$ROLLBACK_COPY" ] || [ ! -f "$ROLLBACK_COPY" ]; then
        return
    fi
    if rm -f "$ROLLBACK_COPY"; then
        ROLLBACK_COPY=""
    else
        echo "Warning: could not remove transient rollback copy: $ROLLBACK_COPY" >&2
    fi
}

restore_from_rollback_copy() {
    if [ -z "$ROLLBACK_COPY" ] || [ ! -f "$ROLLBACK_COPY" ]; then
        echo "ERROR: rollback copy is unavailable; target could not be restored: $ROLLBACK_COPY" >&2
        return 1
    fi

    target_dir=${TARGET_PATH%/*}
    target_name=${TARGET_PATH##*/}
    restore_copy=""
    if ! restore_copy="$(mktemp "$target_dir/.${target_name}.restore.XXXXXX")"; then
        echo "ERROR: failed to create an atomic restore file." >&2
        echo "Rollback copy retained at: $ROLLBACK_COPY" >&2
        return 1
    fi

    if ! cp -p "$ROLLBACK_COPY" "$restore_copy"; then
        rm -f "$restore_copy"
        echo "ERROR: failed to copy rollback data into the restore file." >&2
        echo "Rollback copy retained at: $ROLLBACK_COPY" >&2
        return 1
    fi
    if ! mv -f "$restore_copy" "$TARGET_PATH"; then
        rm -f "$restore_copy"
        echo "ERROR: failed to replace the target with the rollback data." >&2
        echo "Rollback copy retained at: $ROLLBACK_COPY" >&2
        return 1
    fi
    if ! cmp -s "$ROLLBACK_COPY" "$TARGET_PATH"; then
        echo "ERROR: restored target verification failed." >&2
        echo "Rollback copy retained at: $ROLLBACK_COPY" >&2
        return 1
    fi

    echo "Restored LyricsAddonManager.js after an unsuccessful patch/apply operation." >&2
    discard_rollback_copy
}

finalize_and_exit() {
    final_status=$1
    trap - EXIT INT TERM HUP
    set +e

    if [ "$FINALIZING" -eq 1 ]; then
        exit "$final_status"
    fi
    FINALIZING=1

    if [ "$OPERATION_SUCCEEDED" -eq 1 ]; then
        discard_rollback_copy
    elif [ -n "$ROLLBACK_COPY" ]; then
        if ! restore_from_rollback_copy; then
            if [ "$final_status" -eq 0 ]; then
                final_status=1
            fi
        fi
    elif [ "$final_status" -eq 0 ]; then
        final_status=1
    fi

    restart_spotify_if_needed
    exit "$final_status"
}

handle_exit() {
    exit_status=$?
    finalize_and_exit "$exit_status"
}

handle_interrupt() {
    finalize_and_exit 130
}

handle_termination() {
    finalize_and_exit 143
}

handle_hangup() {
    finalize_and_exit 129
}

if [ "$DRY_RUN" -eq 0 ] && [ "$APPLY" -eq 1 ]; then
    create_rollback_copy
    trap handle_exit EXIT
    trap handle_interrupt INT
    trap handle_termination TERM
    trap handle_hangup HUP
fi

python3 - "$TARGET_PATH" "$DRY_RUN" <<'PY'
import sys
import os
import stat
import tempfile
from datetime import datetime, timezone
from pathlib import Path

path = Path(sys.argv[1]).expanduser().resolve()
dry_run = sys.argv[2] == "1"
source = path.read_text(encoding="utf-8")


def find_matching_brace(text: str, open_index: int, label: str) -> int:
    if open_index < 0 or text[open_index] != "{":
        raise SystemExit(f"Patch target block not found: {label}")

    depth = 0
    quote = None
    escaped = False
    line_comment = False
    block_comment = False
    index = open_index

    while index < len(text):
        char = text[index]
        next_char = text[index + 1] if index + 1 < len(text) else ""

        if line_comment:
            if char == "\n":
                line_comment = False
        elif block_comment:
            if char == "*" and next_char == "/":
                block_comment = False
                index += 1
        elif quote:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif char == "/" and next_char == "/":
            line_comment = True
            index += 1
        elif char == "/" and next_char == "*":
            block_comment = True
            index += 1
        elif char in ("'", '"', "`"):
            quote = char
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index

        index += 1

    raise SystemExit(f"Patch target block not found: {label}")


def get_get_lyrics_bounds(text: str):
    method_start = text.find("        async getLyrics(info")
    if method_start < 0:
        raise SystemExit("Patch target block not found: getLyrics method")
    open_brace = text.find("{", method_start)
    method_end = find_matching_brace(text, open_brace, "getLyrics method end") + 1
    return method_start, method_end


def ensure_helpers(text: str) -> str:
    score_helper = """    function scoreLyricsResult(result) {
        const hasKaraoke = hasLyricsContent(result?.karaoke);
        const hasSynced = hasLyricsContent(result?.synced);
        const hasUnsynced = hasLyricsContent(result?.unsynced);
        const pseudoSources = new Set([
            'audio-analysis-pseudo',
            'spotify-audio-analysis',
            'line-timing-pseudo'
        ]);
        const isPseudoKaraoke = hasKaraoke && (
            window.PseudoKaraokeService?.isPseudoSource?.(result?.karaokeSource) === true
            || pseudoSources.has(result?.karaokeSource)
        );

        if (hasKaraoke && !isPseudoKaraoke) return 3;
        if (hasSynced || isPseudoKaraoke) return 2;
        if (hasUnsynced) return 1;
        return 0;
    }
"""

    score_start = text.find("    function scoreLyricsResult(result)")
    if score_start >= 0:
        score_open = text.find("{", score_start)
        score_end = find_matching_brace(text, score_open, "scoreLyricsResult helper end") + 1
        if score_end < len(text) and text[score_end] == "\n":
            score_end += 1
        return text[:score_start] + score_helper + text[score_end:]

    has_content_start = text.find("    function hasLyricsContent(lines)")
    if has_content_start >= 0:
        has_content_open = text.find("{", has_content_start)
        has_content_end = find_matching_brace(text, has_content_open, "hasLyricsContent helper end") + 1
        if has_content_end < len(text) and text[has_content_end] == "\n":
            has_content_end += 1
        return text[:has_content_end] + "\n" + score_helper + text[has_content_end:]

    class_start = text.find("    class LyricsAddonManager")
    if class_start < 0:
        raise SystemExit("Patch target block not found: LyricsAddonManager class")
    class_label = text.rfind("    // LyricsAddonManager Class", 0, class_start)
    helper_insert = text.rfind("    // ============================================", 0, class_label) if class_label >= 0 else class_start
    if helper_insert < 0:
        helper_insert = class_start
    has_content_helper = """    function hasLyricsContent(lines) {
        return Array.isArray(lines) && lines.length > 0;
    }

"""
    return text[:helper_insert] + has_content_helper + score_helper + "\n" + text[helper_insert:]


def add_best_result_state(method: str) -> str:
    if "let bestResult = null;" in method:
        return method

    loop_anchor = "            for (const provider of enabledProviders) {"
    loop_start = method.find(loop_anchor)
    if loop_start < 0:
        raise SystemExit("Patch target block not found: enabled provider loop")

    state = """            let bestResult = null;
            let bestScore = 0;
            let bestMeta = null;

"""
    return method[:loop_start] + state + method[loop_start:]


def ensure_final_content_flags(method: str) -> str:
    if "const hasKaraoke = hasLyricsContent(finalResult.karaoke);" not in method:
        filter_anchor = "                    if (!allowUnsynced) finalResult.unsynced = null;"
        filter_end = method.find(filter_anchor)
        if filter_end < 0:
            raise SystemExit("Patch target block not found: final lyrics filtering")
        filter_end += len(filter_anchor)
        flags = """

                    const hasKaraoke = hasLyricsContent(finalResult.karaoke);
                    const hasSynced = hasLyricsContent(finalResult.synced);
                    const hasUnsynced = hasLyricsContent(finalResult.unsynced);"""
        method = method[:filter_end] + flags + method[filter_end:]

    debug_replacements = (
        ("hasKaraoke: !!finalResult.karaoke", "hasKaraoke"),
        ("hasSynced: !!finalResult.synced", "hasSynced"),
        ("hasUnsynced: !!finalResult.unsynced", "hasUnsynced"),
    )
    for old, new in debug_replacements:
        method = method.replace(old, new, 1)

    old_condition = "if (finalResult.karaoke || finalResult.synced || finalResult.unsynced) {"
    new_condition = "if (hasKaraoke || hasSynced || hasUnsynced) {"
    if old_condition in method:
        method = method.replace(old_condition, new_condition, 1)
    elif new_condition not in method:
        raise SystemExit("Patch target block not found: valid lyrics candidate condition")

    method = method.replace(
        "// 5. 허용된 가사가 있으면 반환",
        "// 5. 허용된 가사가 있으면 후보로 저장",
        1,
    )
    return method


def replace_immediate_return(method: str) -> str:
    if "Final best result selected" in method:
        # Upgrade files patched by the previous version. A synced result must
        # never stop the search merely because karaoke is disabled.
        return method.replace("const stopScore = allowKaraoke ? 3 : 2;", "const stopScore = 3;", 1)

    condition = "                    if (hasKaraoke || hasSynced || hasUnsynced) {"
    condition_start = method.find(condition)
    if condition_start < 0:
        raise SystemExit("Patch target block not found: candidate selection block")
    open_brace = method.find("{", condition_start)
    close_brace = find_matching_brace(method, open_brace, "candidate selection block end")
    body = method[open_brace + 1:close_brace]

    cache_marker = "// IndexedDB에 캐시 저장"
    cache_marker_at = body.find(cache_marker)
    if cache_marker_at < 0:
        raise SystemExit("Patch target block not found: candidate cache block")
    cache_start = body.rfind("\n", 0, cache_marker_at) + 1

    return_at = body.find("return finalResult;", cache_marker_at)
    if return_at < 0:
        raise SystemExit("Patch target block not found: immediate finalResult return")
    return_line_start = body.rfind("\n", 0, return_at) + 1
    cache_block = body[cache_start:return_line_start]

    selection = """                        const score = scoreLyricsResult(finalResult);
                        window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Candidate from ${provider.id} scored ${score}`, {
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
                        }

                        if (score === 3) {
                            window.__ivLyricsDebugLog?.('[LyricsAddonManager] Real karaoke result found, stopping provider search early');
                            break;
                        }

                        continue;
"""
    new_body = "\n" + cache_block + selection + (" " * 20)
    method = method[:open_brace + 1] + new_body + method[close_brace:]

    final_selection = """            if (bestResult) {
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
    no_result_anchor = """            // 디버그 타이머 종료
            if (window.AddonDebug?.isEnabled()) {
                window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                window.AddonDebug.warn('lyrics', 'No lyrics found from any provider');
            }"""
    no_result_at = method.find(no_result_anchor)
    if no_result_at < 0:
        raise SystemExit("Patch target block not found: no-result handling")
    return method[:no_result_at] + final_selection + method[no_result_at:]


def patch_get_lyrics(text: str) -> str:
    method_start, method_end = get_get_lyrics_bounds(text)
    method = text[method_start:method_end]
    method = add_best_result_state(method)
    method = ensure_final_content_flags(method)
    method = replace_immediate_return(method)
    return text[:method_start] + method + text[method_end:]


patched = source
patched = ensure_helpers(patched)
patched = patch_get_lyrics(patched)

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
    target_mode = stat.S_IMODE(path.stat().st_mode)
    temporary_path = None
    temporary_fd = None
    try:
        temporary_fd, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.tmp-",
            dir=path.parent,
        )
        temporary_path = Path(temporary_name)
        os.fchmod(temporary_fd, target_mode)
        with os.fdopen(temporary_fd, "w", encoding="utf-8", newline="") as temporary_file:
            temporary_fd = None
            temporary_file.write(patched)
            temporary_file.flush()
            os.fsync(temporary_file.fileno())
        os.replace(temporary_path, path)
        temporary_path = None
    finally:
        if temporary_fd is not None:
            os.close(temporary_fd)
        if temporary_path is not None:
            try:
                temporary_path.unlink()
            except FileNotFoundError:
                pass
    print(f"Patched: {path}")
    print(f"Backup:  {backup}")
PY

if [ "$DRY_RUN" -eq 1 ]; then
    echo "Dry run: skipped spicetify apply."
    OPERATION_SUCCEEDED=1
elif [ "$APPLY" -eq 0 ]; then
    echo "Skipped spicetify apply by request."
    OPERATION_SUCCEEDED=1
elif command -v spicetify >/dev/null 2>&1; then
    stop_spotify_if_running
    if spicetify apply; then
        OPERATION_SUCCEEDED=1
    else
        apply_status=$?
        exit "$apply_status"
    fi
else
    echo "spicetify not found; patch was written but apply was skipped." >&2
    OPERATION_SUCCEEDED=1
fi
