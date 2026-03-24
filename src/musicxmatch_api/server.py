import argparse
import json
import logging
import os
import re
import threading
import time
from dataclasses import dataclass
from difflib import SequenceMatcher
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Dict, List, Optional, Set, Tuple
from urllib.parse import parse_qs, urlparse

from . import __version__
from .main import MusixMatchAPI

LOG = logging.getLogger("musicxmatch_api.server")
CACHE_TTL_SECONDS = 60 * 30
PENALTY_WORDS = {
    "acoustic",
    "cover",
    "instrumental",
    "karaoke",
    "live",
    "remix",
    "tribute",
}


def normalize_text(value: str) -> str:
    return " ".join(
        "".join(char.lower() if char.isalnum() else " " for char in (value or "")).split()
    )


def simplify_title(value: str) -> str:
    value = re.sub(r"\([^)]*\)|\[[^\]]*\]", " ", value or "")
    value = re.sub(r"\s+-\s+.*$", "", value)
    return normalize_text(value)


def similarity(left: str, right: str) -> float:
    if not left or not right:
        return 0.0
    return SequenceMatcher(None, left, right).ratio()


def format_lrc_timestamp(start_ms: int) -> str:
    total_centiseconds = max(0, round(start_ms / 10))
    minutes = total_centiseconds // 6000
    seconds = (total_centiseconds % 6000) / 100
    return f"{minutes:02d}:{seconds:05.2f}"


def clean_lyrics_text(text: str) -> str:
    text = (text or "").strip()
    text = re.sub(r"\n+\*{3,}.*$", "", text, flags=re.DOTALL)
    return text.strip()


def richsync_to_lines(richsync_body: str) -> List[Dict[str, Any]]:
    if not richsync_body:
        return []

    try:
        entries = json.loads(richsync_body)
    except json.JSONDecodeError:
        return []

    lines: List[Dict[str, Any]] = []
    seen: Set[Tuple[int, str]] = set()
    for entry in entries:
        start_ms = int(float(entry.get("ts", 0)) * 1000)
        text = (entry.get("x") or "").strip()
        if not text:
            text = "".join(chunk.get("c", "") for chunk in entry.get("l", [])).strip()
        if not text:
            continue

        key = (start_ms, text)
        if key in seen:
            continue
        seen.add(key)
        lines.append({"startTime": start_ms, "text": text})

    return lines


def lines_to_lrc(lines: List[Dict[str, Any]]) -> str:
    return "\n".join(
        f"[{format_lrc_timestamp(int(line['startTime']))}]{line['text']}" for line in lines
    )


def score_track(track: dict[str, Any], title: str, artist: str) -> float:
    wanted_title = simplify_title(title)
    wanted_artist = normalize_text(artist)
    track_title = simplify_title(track.get("track_name", ""))
    track_artist = normalize_text(track.get("artist_name", ""))
    title_ratio = similarity(wanted_title, track_title)
    artist_ratio = similarity(wanted_artist, track_artist)

    score = title_ratio * 70 + artist_ratio * 30

    if wanted_title and wanted_title == track_title:
        score += 15
    elif wanted_title and wanted_title in track_title:
        score += 8

    if wanted_artist and (wanted_artist == track_artist or wanted_artist in track_artist):
        score += 10

    if track.get("has_richsync"):
        score += 6
    if track.get("has_lyrics"):
        score += 4

    noise_text = f"{track_title} {track_artist}"
    for word in PENALTY_WORDS:
        if word in noise_text:
            score -= 18

    return score


@dataclass
class CacheEntry:
    expires_at: float
    value: Dict[str, Any]


class LyricsProvider:
    def __init__(self, cache_ttl: int = CACHE_TTL_SECONDS):
        self.cache_ttl = cache_ttl
        self._api = None  # type: Optional[MusixMatchAPI]
        self._api_lock = threading.Lock()
        self._cache = {}  # type: Dict[str, CacheEntry]
        self._cache_lock = threading.Lock()

    @property
    def api(self) -> MusixMatchAPI:
        if self._api is None:
            with self._api_lock:
                if self._api is None:
                    self._api = MusixMatchAPI()
        return self._api

    def health(self) -> Dict[str, Any]:
        return {"status": "ok", "version": __version__, "provider": "musicxmatch"}

    def get_lyrics(self, title: str, artist: str) -> Dict[str, Any]:
        cache_key = f"{normalize_text(title)}::{normalize_text(artist)}"
        cached = self._cache_get(cache_key)
        if cached is not None:
            return {**cached, "cached": True}

        candidates = self._search_candidates(title=title, artist=artist)
        if not candidates:
            raise LookupError(f"No tracks found for '{artist} - {title}'.")

        best_track = max(candidates, key=lambda track: score_track(track, title, artist))
        payload = self._build_track_payload(best_track, title=title, artist=artist)
        if not payload.get("lrc") and not payload.get("text"):
            raise LookupError(f"No lyrics available for '{artist} - {title}'.")

        self._cache_set(cache_key, payload)
        return {**payload, "cached": False}

    def _cache_get(self, key: str) -> Optional[Dict[str, Any]]:
        with self._cache_lock:
            item = self._cache.get(key)
            if item is None:
                return None
            if time.time() > item.expires_at:
                self._cache.pop(key, None)
                return None
            return item.value

    def _cache_set(self, key: str, value: Dict[str, Any]) -> None:
        with self._cache_lock:
            self._cache[key] = CacheEntry(
                expires_at=time.time() + self.cache_ttl,
                value=value,
            )

    def _search_candidates(self, title: str, artist: str) -> List[Dict[str, Any]]:
        queries = [f"{title} {artist}".strip(), title.strip()]
        tracks_by_id = {}  # type: Dict[int, Dict[str, Any]]

        for query in queries:
            if not query:
                continue
            response = self.api.search_tracks(query)
            track_list = (
                response.get("message", {})
                .get("body", {})
                .get("track_list", [])
            )
            for item in track_list:
                track = item.get("track", {})
                track_id = track.get("track_id")
                if track_id:
                    tracks_by_id[track_id] = track

        return list(tracks_by_id.values())

    def _build_track_payload(
        self,
        track: Dict[str, Any],
        title: str,
        artist: str,
    ) -> Dict[str, Any]:
        track_id = track["track_id"]
        payload = {
            "provider": "musicxmatch",
            "trackId": track_id,
            "trackName": track.get("track_name"),
            "artistName": track.get("artist_name"),
            "albumName": track.get("album_name"),
            "matchScore": round(score_track(track, title, artist), 2),
            "source": None,
            "lrc": None,
            "text": None,
            "language": None,
        }

        if track.get("has_richsync"):
            richsync_response = self.api.get_track_richsync(track_id=track_id)
            richsync_body = (
                richsync_response.get("message", {})
                .get("body", {})
                .get("richsync", {})
                .get("richsync_body")
            )
            richsync_lines = richsync_to_lines(richsync_body)
            if richsync_lines:
                payload["source"] = "richsync"
                payload["lrc"] = lines_to_lrc(richsync_lines)
                payload["language"] = track.get("lyrics_language")

        if not payload["lrc"] and track.get("has_lyrics"):
            lyrics_response = self.api.get_track_lyrics(track_id=track_id)
            lyrics = (
                lyrics_response.get("message", {})
                .get("body", {})
                .get("lyrics", {})
            )
            lyrics_text = clean_lyrics_text(lyrics.get("lyrics_body", ""))
            if lyrics_text:
                payload["source"] = "lyrics"
                payload["text"] = lyrics_text
                payload["language"] = lyrics.get("lyrics_language")

        return payload


class LyricsRequestHandler(BaseHTTPRequestHandler):
    provider = LyricsProvider()
    protocol_version = "HTTP/1.1"

    def do_OPTIONS(self) -> None:  # noqa: N802
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)

        if parsed.path == "/health":
            self._send_json(200, self.provider.health())
            return

        if parsed.path == "/lyrics":
            title = self._get_query_value(query, "title")
            artist = self._get_query_value(query, "artist")
            if not title or not artist:
                self._send_json(400, {"detail": "title and artist are required"})
                return

            try:
                payload = self.provider.get_lyrics(title=title, artist=artist)
            except LookupError as error:
                self._send_json(404, {"detail": str(error)})
            except Exception as error:  # pragma: no cover
                LOG.exception("lyrics lookup failed")
                self._send_json(502, {"detail": str(error)})
            else:
                self._send_json(200, payload)
            return

        self._send_json(404, {"detail": "Not found"})

    def log_message(self, fmt: str, *args: Any) -> None:
        LOG.info("%s - %s", self.address_string(), fmt % args)

    def _get_query_value(self, query: Dict[str, List[str]], key: str) -> str:
        values = query.get(key)
        return values[0].strip() if values and values[0] else ""

    def _send_json(self, status_code: int, payload: Dict[str, Any]) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status_code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the MusicXMatch ivLyrics addon server.")
    parser.add_argument(
        "--host",
        default=os.getenv("MUSICXMATCH_HOST", "127.0.0.1"),
        help="Host to bind to.",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=int(os.getenv("MUSICXMATCH_PORT", os.getenv("PORT", "8092"))),
        help="Port to bind to.",
    )
    return parser


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    args = build_arg_parser().parse_args()
    server = ThreadingHTTPServer((args.host, args.port), LyricsRequestHandler)
    LOG.info("MusicXMatch addon server listening on http://%s:%s", args.host, args.port)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        LOG.info("MusicXMatch addon server shutting down")
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
