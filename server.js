/**
 * ivLyrics MusicXMatch Server
 * Node.js server for fetching lyrics from MusicXMatch
 */

'use strict';

require('dotenv').config();

const express = require('express');
const cors = require('cors');
const MusicXMatchAPI = require('./musicxmatch');

const PORT = parseInt(process.env.PORT || '8092', 10);
const CACHE_TTL = parseInt(process.env.CACHE_TTL || '1800', 10) * 1000;
const SERVER_VERSION = '1.0.0';

const log = {
  time: () => new Date().toTimeString().slice(0, 8),
  info: (...a) => console.log(`${log.time()} [INFO]`, ...a),
  warn: (...a) => console.warn(`${log.time()} [WARN]`, ...a),
  error: (...a) => console.error(`${log.time()} [ERROR]`, ...a),
};

// Cache
const cache = new Map();

function cacheGet(key) {
  const item = cache.get(key);
  if (!item) return null;
  if (Date.now() - item.ts > CACHE_TTL) {
    cache.delete(key);
    return null;
  }
  return item.val;
}

function cacheSet(key, val) {
  cache.set(key, { ts: Date.now(), val });
}

// Scoring helpers
const PENALTY_WORDS = ['acoustic', 'cover', 'instrumental', 'karaoke', 'live', 'remix', 'tribute'];

function normalize(text) {
  return (text || '').toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim().replace(/\s+/g, ' ');
}

function simplify(text) {
  return normalize((text || '').replace(/\([^)]*\)|\[[^\]]*\]/g, ' ').replace(/\s+-\s+.*$/, ''));
}

function similarity(a, b) {
  if (!a || !b) return 0;
  const len = Math.max(a.length, b.length);
  let matches = 0;
  for (let i = 0; i < Math.min(a.length, b.length); i++) {
    if (a[i] === b[i]) matches++;
  }
  return matches / len;
}

function scoreTrack(track, title, artist) {
  const wantTitle = simplify(title);
  const wantArtist = normalize(artist);
  const trackTitle = simplify(track.track_name || '');
  const trackArtist = normalize(track.artist_name || '');

  let score = similarity(wantTitle, trackTitle) * 70 + similarity(wantArtist, trackArtist) * 30;

  if (wantTitle === trackTitle) score += 15;
  else if (trackTitle.includes(wantTitle)) score += 8;

  if (wantArtist === trackArtist || trackArtist.includes(wantArtist)) score += 10;

  if (track.has_richsync) score += 6;
  if (track.has_lyrics) score += 4;

  const noise = `${trackTitle} ${trackArtist}`;
  for (const word of PENALTY_WORDS) {
    if (noise.includes(word)) score -= 18;
  }

  return score;
}

// Richsync parser
function richsyncToLrc(richsyncBody) {
  if (!richsyncBody) return null;
  try {
    const entries = JSON.parse(richsyncBody);
    const lines = [];
    for (const entry of entries) {
      const ms = Math.floor((entry.ts || 0) * 1000);
      const text = (entry.x || entry.l?.map(c => c.c).join('') || '').trim();
      if (text) {
        const m = Math.floor(ms / 60000);
        const s = ((ms % 60000) / 1000).toFixed(2).padStart(5, '0');
        lines.push(`[${String(m).padStart(2, '0')}:${s}]${text}`);
      }
    }
    return lines.length ? lines.join('\n') : null;
  } catch {
    return null;
  }
}

// Lyrics provider
async function getLyrics(api, title, artist) {
  const cacheKey = `${normalize(title)}::${normalize(artist)}`;
  const cached = cacheGet(cacheKey);
  if (cached) return { ...cached, cached: true };

  log.info(`[요청] "${title}" by "${artist}"`);

  const queries = [`${title} ${artist}`, title];
  const tracksById = {};

  for (const q of queries) {
    const res = await api.searchTracks(q);
    const tracks = res?.message?.body?.track_list || [];
    for (const item of tracks) {
      const track = item.track;
      if (track?.track_id) tracksById[track.track_id] = track;
    }
  }

  const candidates = Object.values(tracksById);
  if (!candidates.length) throw new Error('No tracks found');

  const best = candidates.reduce((a, b) => scoreTrack(a, title, artist) > scoreTrack(b, title, artist) ? a : b);
  log.info(`[매칭] ${best.track_name} - ${best.artist_name} (score: ${scoreTrack(best, title, artist).toFixed(1)})`);

  const payload = { provider: 'musicxmatch', trackId: best.track_id, trackName: best.track_name, artistName: best.artist_name, lrc: null, text: null };

  if (best.has_richsync) {
    const res = await api.getTrackRichsync(best.track_id);
    const body = res?.message?.body?.richsync?.richsync_body;
    payload.lrc = richsyncToLrc(body);
    if (payload.lrc) log.info('[가사] richsync 가져옴');
  }

  if (!payload.lrc && best.has_lyrics) {
    const res = await api.getTrackLyrics(best.track_id);
    const text = res?.message?.body?.lyrics?.lyrics_body || '';
    payload.text = text.replace(/\n+\*{3,}.*$/s, '').trim();
    if (payload.text) log.info('[가사] 일반 가사 가져옴');
  }

  if (!payload.lrc && !payload.text) throw new Error('No lyrics available');

  cacheSet(cacheKey, payload);
  return { ...payload, cached: false };
}

// Express app
const app = express();
app.use(cors());
app.use(express.json());

const api = new MusicXMatchAPI();

app.get('/health', (req, res) => {
  res.json({ status: 'ok', version: SERVER_VERSION, provider: 'musicxmatch' });
});

app.get('/lyrics', async (req, res) => {
  const { title, artist } = req.query;
  if (!title || !artist) {
    return res.status(400).json({ detail: 'title and artist are required' });
  }

  try {
    const result = await getLyrics(api, title, artist);
    res.json(result);
  } catch (err) {
    log.error(`[오류] ${err.message}`);
    res.status(404).json({ detail: err.message });
  }
});

app.delete('/cache', (req, res) => {
  const count = cache.size;
  cache.clear();
  res.json({ deleted: count });
});

async function start() {
  log.info('[시작] MusicXMatch API 초기화 중...');
  await api.init();
  log.info('[시작] 초기화 완료');

  app.listen(PORT, '0.0.0.0', () => {
    log.info(`[시작] ivLyrics MusicXMatch Server v${SERVER_VERSION}`);
    log.info(`[시작] http://0.0.0.0:${PORT}`);
  });
}

start();
