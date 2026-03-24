use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::{Json, Router};
use musixmatch_inofficial::models::{SortOrder, SubtitleFormat, Track, TrackId};
use musixmatch_inofficial::{Error as MxmError, Musixmatch};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const DEFAULT_PORT: u16 = 8092;
const PROVIDER_NAME: &str = "musicxmatch";

#[derive(Clone)]
struct AppState {
    mxm: Musixmatch,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    logger: Logger,
}

#[derive(Clone)]
struct CacheEntry {
    expires_at: Instant,
    payload: LyricsPayload,
}

#[derive(Debug, Deserialize)]
struct LyricsQuery {
    title: Option<String>,
    artist: Option<String>,
    #[serde(alias = "spotifyId")]
    spotify_id: Option<String>,
    #[serde(alias = "durationMs")]
    duration_ms: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
struct HealthPayload {
    status: &'static str,
    version: &'static str,
    provider: &'static str,
    backend: &'static str,
}

#[derive(Debug, Serialize, Clone)]
struct LyricsPayload {
    provider: &'static str,
    #[serde(rename = "trackId")]
    track_id: Option<u64>,
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    lrc: Option<String>,
    text: Option<String>,
    cached: bool,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    detail: String,
}

#[derive(Clone)]
struct Logger {
    file: Arc<std::sync::Mutex<Option<std::fs::File>>>,
}

#[tokio::main]
async fn main() {
    let logger = Logger::new(log_file_path());
    logger.log("server boot");

    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let state = AppState {
        mxm: build_client(),
        cache: Arc::new(Mutex::new(HashMap::new())),
        logger: logger.clone(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/lyrics", get(get_lyrics))
        .route("/cache", delete(clear_cache))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    logger.log(&format!("binding to {addr}"));
    println!("ivLyrics MusicXMatch Server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");
    logger.log("listener bound successfully");
    axum::serve(listener, app)
        .await
        .expect("server exited unexpectedly");
}

fn build_client() -> Musixmatch {
    let storage_file = session_file_path();
    if let Some(parent) = storage_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    Musixmatch::builder()
        .storage_file(storage_file)
        .build()
        .expect("failed to construct Musixmatch client")
}

fn session_file_path() -> PathBuf {
    if let Ok(value) = std::env::var("MXM_SESSION_FILE") {
        return PathBuf::from(value);
    }

    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".ivlyrics-musicxmatch");
    path.push("musixmatch_session.json");
    path
}

fn log_file_path() -> PathBuf {
    if let Ok(value) = std::env::var("IVLYRICS_MXM_LOG") {
        return PathBuf::from(value);
    }

    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".ivlyrics-musicxmatch");
    path.push("server.log");
    path
}

impl Logger {
    fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }

        let file = OpenOptions::new().create(true).append(true).open(path).ok();
        Self {
            file: Arc::new(std::sync::Mutex::new(file)),
        }
    }

    fn log(&self, message: &str) {
        let line = format!("[{}] {message}\n", timestamp_string());
        print!("{line}");
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }
}

fn timestamp_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}

async fn health(State(state): State<AppState>) -> Json<HealthPayload> {
    state.logger.log("GET /health");
    Json(HealthPayload {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        provider: PROVIDER_NAME,
        backend: "musixmatch-inofficial",
    })
}

async fn clear_cache(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.logger.log("DELETE /cache");
    let mut cache = state.cache.lock().await;
    let deleted = cache.len();
    cache.clear();
    Json(serde_json::json!({ "deleted": deleted }))
}

async fn get_lyrics(
    State(state): State<AppState>,
    Query(query): Query<LyricsQuery>,
) -> impl IntoResponse {
    let title = query.title.unwrap_or_default().trim().to_string();
    let artist = query.artist.unwrap_or_default().trim().to_string();
    let spotify_id = query.spotify_id.unwrap_or_default().trim().to_string();
    let duration_secs = query.duration_ms.map(|value| value as f32 / 1000.0);
    state.logger.log(&format!(
        "GET /lyrics title={title:?} artist={artist:?} spotify_id={spotify_id:?}"
    ));

    if spotify_id.is_empty() && (title.is_empty() || artist.is_empty()) {
        state
            .logger
            .log("rejecting /lyrics request because title/artist are missing");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorPayload {
                detail: "title and artist are required when spotifyId is missing".to_string(),
            }),
        )
            .into_response();
    }

    let cache_key = build_cache_key(&title, &artist, &spotify_id);
    if let Some(cached) = cached_payload(&state, &cache_key).await {
        state.logger.log(&format!("cache hit for key={cache_key}"));
        return (StatusCode::OK, Json(cached)).into_response();
    }

    match fetch_payload(&state.mxm, &title, &artist, &spotify_id, duration_secs).await {
        Ok(mut payload) => {
            state.logger.log(&format!(
                "lyrics resolved track_id={:?} track_name={:?} has_lrc={} has_text={}",
                payload.track_id,
                payload.track_name,
                payload.lrc.is_some(),
                payload.text.is_some()
            ));
            payload.cached = false;
            store_cache(&state, cache_key, payload.clone()).await;
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(error) => {
            let (status, detail) = map_error(error);
            state.logger.log(&format!(
                "lyrics request failed status={} detail={detail}",
                status.as_u16()
            ));
            (status, Json(ErrorPayload { detail })).into_response()
        }
    }
}

async fn cached_payload(state: &AppState, key: &str) -> Option<LyricsPayload> {
    let mut cache = state.cache.lock().await;
    if let Some(entry) = cache.get(key) {
        if Instant::now() < entry.expires_at {
            let mut payload = entry.payload.clone();
            payload.cached = true;
            return Some(payload);
        }
    }
    cache.remove(key);
    None
}

async fn store_cache(state: &AppState, key: String, payload: LyricsPayload) {
    let mut cache = state.cache.lock().await;
    cache.insert(
        key,
        CacheEntry {
            expires_at: Instant::now() + CACHE_TTL,
            payload,
        },
    );
}

fn build_cache_key(title: &str, artist: &str, spotify_id: &str) -> String {
    if !spotify_id.is_empty() {
        return format!("spotify:{spotify_id}");
    }
    format!("{}::{}", normalize(title), normalize(artist))
}

async fn fetch_payload(
    mxm: &Musixmatch,
    title: &str,
    artist: &str,
    spotify_id: &str,
    duration_secs: Option<f32>,
) -> Result<LyricsPayload, MxmError> {
    if !spotify_id.is_empty() {
        if let Ok(payload) = fetch_by_id(
            mxm,
            TrackId::Spotify(spotify_id.to_owned().into()),
            None,
            duration_secs,
        )
        .await
        {
            return Ok(payload);
        }
    }

    let track = resolve_track(mxm, title, artist).await?;
    fetch_by_id(
        mxm,
        TrackId::TrackId(track.track_id),
        Some(track),
        duration_secs,
    )
    .await
}

async fn fetch_by_id(
    mxm: &Musixmatch,
    id: TrackId<'static>,
    known_track: Option<Track>,
    duration_secs: Option<f32>,
) -> Result<LyricsPayload, MxmError> {
    let track = match known_track {
        Some(track) => track,
        None => mxm.track(id.clone(), false, false, false).await?,
    };

    let subtitle = mxm
        .track_subtitle(
            id.clone(),
            SubtitleFormat::Lrc,
            duration_secs,
            duration_secs.map(|_| 1.0),
        )
        .await;

    if let Ok(subtitle) = subtitle {
        if !subtitle.subtitle_body.trim().is_empty() {
            return Ok(LyricsPayload {
                provider: PROVIDER_NAME,
                track_id: Some(track.track_id),
                track_name: Some(track.track_name),
                artist_name: Some(track.artist_name),
                lrc: Some(subtitle.subtitle_body),
                text: None,
                cached: false,
            });
        }
    }

    let lyrics = mxm.track_lyrics(id).await?;
    Ok(LyricsPayload {
        provider: PROVIDER_NAME,
        track_id: Some(track.track_id),
        track_name: Some(track.track_name),
        artist_name: Some(track.artist_name),
        lrc: None,
        text: Some(strip_lyrics_footer(&lyrics.lyrics_body)),
        cached: false,
    })
}

async fn resolve_track(mxm: &Musixmatch, title: &str, artist: &str) -> Result<Track, MxmError> {
    let mut tracks_by_id = HashMap::new();
    let title_variants = title_variants(title);
    let artist_variants = artist_variants(artist);

    for title_variant in &title_variants {
        for artist_variant in &artist_variants {
            let tracks = search_tracks(mxm, Some(title_variant), Some(artist_variant)).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        for title_variant in &title_variants {
            let tracks = search_tracks(mxm, Some(title_variant), None).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        for artist_variant in &artist_variants {
            let tracks = search_tracks(mxm, None, Some(artist_variant)).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        for title_variant in &title_variants {
            for artist_variant in &artist_variants {
                if let Ok(matched) = mxm
                    .matcher_track(title_variant, artist_variant, "", false, false, false)
                    .await
                {
                    tracks_by_id.entry(matched.track_id).or_insert(matched);
                }
            }
        }
    }

    if tracks_by_id.is_empty() {
        let matched = mxm.matcher_track(title, artist, "", false, false, false).await?;
        tracks_by_id.insert(matched.track_id, matched);
    }

    tracks_by_id
        .into_values()
        .max_by(|left, right| {
            score_track(left, title, artist)
                .partial_cmp(&score_track(right, title, artist))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .ok_or(MxmError::NotFound)
}

async fn search_tracks(
    mxm: &Musixmatch,
    title: Option<&str>,
    artist: Option<&str>,
) -> Result<Vec<Track>, MxmError> {
    let mut query = mxm.track_search();

    if let Some(title) = title {
        query = query.q_track(title);
    }
    if let Some(artist) = artist {
        query = query.q_artist(artist);
    }

    query
        .f_has_lyrics()
        .s_track_rating(SortOrder::Desc)
        .send(10, 1)
        .await
}

fn title_variants(title: &str) -> Vec<String> {
    let mut values = Vec::new();
    let base = title.trim();
    push_variant(&mut values, base);
    push_variant(&mut values, &strip_brackets(base));
    push_variant(&mut values, &strip_featured(base));
    push_variant(&mut values, &collapse_to_words(base));
    push_variant(&mut values, &collapse_alnum(base));
    push_variant(&mut values, &normalize_connectors(base));
    values
}

fn artist_variants(artist: &str) -> Vec<String> {
    let mut values = Vec::new();
    let base = artist.trim();
    push_variant(&mut values, base);
    push_variant(&mut values, &first_artist(base));
    push_variant(&mut values, &strip_featured(base));
    push_variant(&mut values, &collapse_to_words(base));
    push_variant(&mut values, &normalize_connectors(base));
    values
}

fn push_variant(values: &mut Vec<String>, candidate: &str) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if !values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(trimmed))
    {
        values.push(trimmed.to_string());
    }
}

fn strip_brackets(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut depth_round = 0usize;
    let mut depth_square = 0usize;

    for ch in value.chars() {
        match ch {
            '(' => depth_round += 1,
            ')' => depth_round = depth_round.saturating_sub(1),
            '[' => depth_square += 1,
            ']' => depth_square = depth_square.saturating_sub(1),
            _ if depth_round == 0 && depth_square == 0 => result.push(ch),
            _ => {}
        }
    }

    collapse_to_words(&result)
}

fn strip_featured(value: &str) -> String {
    let lower = value.to_lowercase();
    for marker in [" feat. ", " feat ", " featuring ", " ft. ", " ft "] {
        if let Some(index) = lower.find(marker) {
            return value[..index].trim().to_string();
        }
    }
    value.trim().to_string()
}

fn first_artist(value: &str) -> String {
    for marker in [",", "&", " x ", ";"] {
        if let Some(index) = value.find(marker) {
            return value[..index].trim().to_string();
        }
    }
    value.trim().to_string()
}

fn collapse_to_words(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn collapse_alnum(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>()
}

fn normalize_connectors(value: &str) -> String {
    value
        .replace('&', " and ")
        .replace('×', " x ")
        .replace('/', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize(value: &str) -> String {
    collapse_to_words(value).to_lowercase()
}

fn simplify(value: &str) -> String {
    let no_brackets = strip_brackets(value);
    let base = no_brackets
        .split(" - ")
        .next()
        .unwrap_or(no_brackets.as_str())
        .trim()
        .to_string();
    normalize(&base)
}

fn similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let len = a.len().max(b.len()) as f32;
    let matches = a
        .chars()
        .zip(b.chars())
        .filter(|(left, right)| left == right)
        .count() as f32;
    matches / len
}

fn score_track(track: &Track, title: &str, artist: &str) -> f32 {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(&track.track_name);
    let track_artist = normalize(&track.artist_name);

    let mut score = similarity(&want_title, &track_title) * 70.0
        + similarity(&want_artist, &track_artist) * 30.0;

    if want_title == track_title {
        score += 15.0;
    } else if track_title.contains(&want_title) {
        score += 8.0;
    }

    if want_artist == track_artist || track_artist.contains(&want_artist) {
        score += 10.0;
    }

    if track.has_subtitles {
        score += 8.0;
    }
    if track.has_richsync {
        score += 4.0;
    }
    if track.has_lyrics {
        score += 2.0;
    }

    let noise = format!("{track_title} {track_artist}");
    for word in [
        "acoustic",
        "cover",
        "instrumental",
        "karaoke",
        "live",
        "remix",
        "tribute",
    ] {
        if noise.contains(word) {
            score -= 18.0;
        }
    }

    score
}

fn strip_lyrics_footer(value: &str) -> String {
    value
        .split("\n\n*******")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn map_error(error: MxmError) -> (StatusCode, String) {
    match error {
        MxmError::NotFound => (StatusCode::NOT_FOUND, "No tracks found".to_string()),
        MxmError::NotAvailable => (
            StatusCode::NOT_FOUND,
            "No lyrics are available for this track".to_string(),
        ),
        MxmError::Ratelimit => (
            StatusCode::TOO_MANY_REQUESTS,
            "Musixmatch rate limit reached. Wait a minute and try again.".to_string(),
        ),
        MxmError::TokenExpired => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Musixmatch session expired. Retry in a moment.".to_string(),
        ),
        MxmError::MissingCredentials => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Musixmatch credentials are required for this request.".to_string(),
        ),
        MxmError::WrongCredentials => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Configured Musixmatch credentials are invalid.".to_string(),
        ),
        MxmError::MusixmatchError { status_code, msg } => (
            StatusCode::BAD_GATEWAY,
            format!("Musixmatch error {status_code}: {msg}"),
        ),
        other => (StatusCode::BAD_GATEWAY, other.to_string()),
    }
}
