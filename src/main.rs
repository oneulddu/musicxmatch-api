use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use musixmatch_inofficial::models::{SortOrder, SubtitleFormat, Track, TrackId};
use musixmatch_inofficial::{Error as MxmError, Musixmatch};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const DEFAULT_PORT: u16 = 8092;
const PROVIDER_NAME: &str = "musicxmatch";
const VERSION_INFO_URL: &str = "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/version.json";

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
    debug: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
struct HealthPayload {
    status: &'static str,
    version: &'static str,
    provider: &'static str,
    backend: &'static str,
    cors: bool,
    #[serde(rename = "cacheEntries")]
    cache_entries: usize,
    #[serde(rename = "sessionFile")]
    session_file: String,
    #[serde(rename = "logFile")]
    log_file: String,
    #[serde(rename = "updateAvailable")]
    update_available: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugPayload>,
}

#[derive(Debug, Serialize, Clone)]
struct DebugPayload {
    source: &'static str,
    matched_by: &'static str,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
    #[serde(rename = "selectedTrackId")]
    selected_track_id: Option<u64>,
    #[serde(rename = "selectedTrackDurationMs")]
    selected_track_duration_ms: Option<u64>,
    #[serde(rename = "searchVariants")]
    search_variants: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    detail: String,
}

#[derive(Debug, Deserialize)]
struct VersionInfo {
    server: String,
    addon: String,
}

#[derive(Debug, Serialize)]
struct UpdateCheckPayload {
    #[serde(rename = "currentVersion")]
    current_version: &'static str,
    #[serde(rename = "latestVersion")]
    latest_version: String,
    #[serde(rename = "latestAddonVersion")]
    latest_addon_version: String,
    #[serde(rename = "updateAvailable")]
    update_available: bool,
    platform: &'static str,
    command: Vec<String>,
}

#[derive(Debug, Serialize)]
struct UpdateApplyPayload {
    status: &'static str,
    platform: &'static str,
    command: Vec<String>,
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
        .route("/update/check", get(update_check))
        .route("/update/apply", post(update_apply))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
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

async fn health(State(state): State<AppState>) -> Response {
    state.logger.log("GET /health");
    let cache_entries = state.cache.lock().await.len();
    let update_available = latest_version_info()
        .await
        .map(|info| compare_versions(&info.server, env!("CARGO_PKG_VERSION")) > 0)
        .unwrap_or(false);
    json_response(StatusCode::OK, HealthPayload {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        provider: PROVIDER_NAME,
        backend: "musixmatch-inofficial",
        cors: true,
        cache_entries,
        session_file: session_file_path().display().to_string(),
        log_file: log_file_path().display().to_string(),
        update_available,
    })
}

async fn clear_cache(State(state): State<AppState>) -> Response {
    state.logger.log("DELETE /cache");
    let mut cache = state.cache.lock().await;
    let deleted = cache.len();
    cache.clear();
    json_response(StatusCode::OK, serde_json::json!({ "deleted": deleted }))
}

async fn get_lyrics(
    State(state): State<AppState>,
    Query(query): Query<LyricsQuery>,
) -> impl IntoResponse {
    let title = query.title.unwrap_or_default().trim().to_string();
    let artist = query.artist.unwrap_or_default().trim().to_string();
    let spotify_id = query.spotify_id.unwrap_or_default().trim().to_string();
    let duration_secs = query.duration_ms.map(|value| value as f32 / 1000.0);
    let include_debug = query.debug.unwrap_or(false);
    state.logger.log(&format!(
        "GET /lyrics title={title:?} artist={artist:?} spotify_id={spotify_id:?}"
    ));

    if spotify_id.is_empty() && (title.is_empty() || artist.is_empty()) {
        state
            .logger
            .log("rejecting /lyrics request because title/artist are missing");
        return (
            StatusCode::BAD_REQUEST,
            json_response(StatusCode::BAD_REQUEST, ErrorPayload {
                detail: "title and artist are required when spotifyId is missing".to_string(),
            }),
        )
            .into_response();
    }

    let cache_key = build_cache_key(&title, &artist, &spotify_id);
    if let Some(cached) = cached_payload(&state, &cache_key).await {
        state.logger.log(&format!("cache hit for key={cache_key}"));
        return json_response(StatusCode::OK, cached);
    }

    match fetch_payload(
        &state.mxm,
        &title,
        &artist,
        &spotify_id,
        duration_secs,
        include_debug,
    )
    .await
    {
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
            json_response(StatusCode::OK, payload)
        }
        Err(error) => {
            let (status, detail) = map_error(error);
            state.logger.log(&format!(
                "lyrics request failed status={} detail={detail}",
                status.as_u16()
            ));
            json_response(status, ErrorPayload { detail })
        }
    }
}

async fn update_check(State(state): State<AppState>) -> Response {
    state.logger.log("GET /update/check");
    match latest_version_info().await {
        Ok(info) => json_response(
            StatusCode::OK,
            UpdateCheckPayload {
                current_version: env!("CARGO_PKG_VERSION"),
                latest_version: info.server.clone(),
                latest_addon_version: info.addon,
                update_available: compare_versions(&info.server, env!("CARGO_PKG_VERSION")) > 0,
                platform: current_platform(),
                command: update_command_lines(),
            },
        ),
        Err(error) => json_response(
            StatusCode::BAD_GATEWAY,
            ErrorPayload {
                detail: error,
            },
        ),
    }
}

async fn update_apply(State(state): State<AppState>) -> Response {
    state.logger.log("POST /update/apply");
    match spawn_update_process() {
        Ok(()) => json_response(
            StatusCode::ACCEPTED,
            UpdateApplyPayload {
                status: "scheduled",
                platform: current_platform(),
                command: update_command_lines(),
            },
        ),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorPayload {
                detail: error,
            },
        ),
    }
}

fn json_response<T: Serialize>(status: StatusCode, payload: T) -> Response {
    let mut response = (status, Json(payload)).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        "application/json; charset=utf-8".parse().expect("valid content-type header"),
    );
    response
}

async fn latest_version_info() -> Result<VersionInfo, String> {
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())?
        .get(VERSION_INFO_URL)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Latest version lookup failed ({})", response.status()));
    }

    response.json::<VersionInfo>().await.map_err(|error| error.to_string())
}

fn compare_versions(left: &str, right: &str) -> i32 {
    let a = parse_version(left);
    let b = parse_version(right);
    let length = a.len().max(b.len());
    for index in 0..length {
        let delta = (a.get(index).copied().unwrap_or(0) as i32)
            - (b.get(index).copied().unwrap_or(0) as i32);
        if delta != 0 {
            return delta;
        }
    }
    0
}

fn parse_version(value: &str) -> Vec<u32> {
    value
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

fn current_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        "linux"
    }
}

fn update_command_lines() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            "iwr -useb \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1\" | iex".to_string(),
            "Invoke-WebRequest -Uri \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_MusicXMatch.js\" -OutFile \"$env:APPDATA\\spicetify\\Extensions\\Addon_Lyrics_MusicXMatch.js\"".to_string(),
            "spicetify apply".to_string(),
        ]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![
            "curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash".to_string(),
            "curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/Addon_Lyrics_MusicXMatch.js -o ~/.config/spicetify/Extensions/Addon_Lyrics_MusicXMatch.js".to_string(),
            "spicetify apply".to_string(),
        ]
    }
}

fn spawn_update_process() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let command = "Start-Process powershell.exe -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','Start-Sleep -Seconds 1; iwr -useb \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1\" | iex'";
        Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command])
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let command = "sleep 1; curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash";
        Command::new("sh")
            .args(["-c", &format!("nohup sh -c '{}' >/dev/null 2>&1 &", command)])
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
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
    include_debug: bool,
) -> Result<LyricsPayload, MxmError> {
    if !spotify_id.is_empty() {
        if let Ok(payload) = fetch_by_id(
            mxm,
            TrackId::Spotify(spotify_id.to_owned().into()),
            None,
            duration_secs,
            include_debug.then(|| DebugPayload {
                source: "spotify_id",
                matched_by: "track_id",
                duration_ms: duration_secs.map(|value| (value * 1000.0).round() as u64),
                selected_track_id: None,
                selected_track_duration_ms: None,
                search_variants: Vec::new(),
            }),
        )
        .await
        {
            return Ok(payload);
        }
    }

    let resolution = resolve_track(mxm, title, artist, duration_secs).await?;
    fetch_by_id(
        mxm,
        TrackId::TrackId(resolution.track.track_id),
        Some(resolution.track),
        duration_secs,
        include_debug.then(|| DebugPayload {
            source: "search",
            matched_by: resolution.matched_by,
            duration_ms: duration_secs.map(|value| (value * 1000.0).round() as u64),
            selected_track_id: None,
            selected_track_duration_ms: None,
            search_variants: resolution.search_variants,
        }),
    )
    .await
}

async fn fetch_by_id(
    mxm: &Musixmatch,
    id: TrackId<'static>,
    known_track: Option<Track>,
    duration_secs: Option<f32>,
    debug: Option<DebugPayload>,
) -> Result<LyricsPayload, MxmError> {
    let track = match known_track {
        Some(track) => track,
        None => mxm.track(id.clone(), false, false, false).await?,
    };
    let mut debug = debug;
    if let Some(payload) = debug.as_mut() {
        payload.selected_track_id = Some(track.track_id);
        payload.selected_track_duration_ms = Some(track.track_length.into());
    }

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
                debug,
            });
        }
    }

    let lyrics = mxm.track_lyrics(id).await?;
    let text = strip_lyrics_footer(&lyrics.lyrics_body);
    if text.is_empty() {
        return Err(MxmError::NotAvailable);
    }

    Ok(LyricsPayload {
        provider: PROVIDER_NAME,
        track_id: Some(track.track_id),
        track_name: Some(track.track_name),
        artist_name: Some(track.artist_name),
        lrc: None,
        text: Some(text),
        cached: false,
        debug,
    })
}

struct TrackResolution {
    track: Track,
    matched_by: &'static str,
    search_variants: Vec<String>,
}

async fn resolve_track(
    mxm: &Musixmatch,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> Result<TrackResolution, MxmError> {
    let mut tracks_by_id = HashMap::new();
    let title_variants = title_variants(title);
    let artist_variants = artist_variants(artist);
    let mut attempted_variants = Vec::new();
    let mut matched_by = "search:title+artist";

    for title_variant in &title_variants {
        for artist_variant in &artist_variants {
            attempted_variants.push(format!("title={title_variant} | artist={artist_variant}"));
            let tracks = search_tracks(mxm, Some(title_variant), Some(artist_variant)).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        matched_by = "search:title";
        for title_variant in &title_variants {
            attempted_variants.push(format!("title={title_variant} | artist=<none>"));
            let tracks = search_tracks(mxm, Some(title_variant), None).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        matched_by = "search:artist";
        for artist_variant in &artist_variants {
            attempted_variants.push(format!("title=<none> | artist={artist_variant}"));
            let tracks = search_tracks(mxm, None, Some(artist_variant)).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        matched_by = "matcher:variants";
        for title_variant in &title_variants {
            for artist_variant in &artist_variants {
                attempted_variants.push(format!("matcher title={title_variant} | artist={artist_variant}"));
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
        matched_by = "matcher:original";
        attempted_variants.push(format!("matcher title={title} | artist={artist}"));
        let matched = mxm.matcher_track(title, artist, "", false, false, false).await?;
        tracks_by_id.insert(matched.track_id, matched);
    }

    tracks_by_id
        .into_values()
        .max_by(|left, right| {
            score_track(left, title, artist, duration_secs)
                .partial_cmp(&score_track(right, title, artist, duration_secs))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|track| TrackResolution {
            track,
            matched_by,
            search_variants: attempted_variants,
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

fn score_track(track: &Track, title: &str, artist: &str, duration_secs: Option<f32>) -> f32 {
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

    if let Some(want_duration) = duration_secs {
        let actual_duration = track.track_length as f32 / 1000.0;
        score += duration_score((actual_duration - want_duration).abs());
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

fn duration_score(delta_secs: f32) -> f32 {
    if delta_secs <= 1.5 {
        18.0
    } else if delta_secs <= 3.0 {
        10.0
    } else if delta_secs <= 6.0 {
        4.0
    } else if delta_secs >= 20.0 {
        -20.0
    } else if delta_secs >= 10.0 {
        -8.0
    } else {
        0.0
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_to_words_preserves_unicode_letters() {
        assert_eq!(collapse_to_words("에픽하이 feat. 융진"), "에픽하이 feat 융진");
        assert_eq!(collapse_to_words("끊었어? (demo)"), "끊었어 demo");
    }

    #[test]
    fn title_variants_include_search_fallback_forms() {
        let variants = title_variants("끊었어? (demo)");
        assert!(variants.iter().any(|value| value == "끊었어? (demo)"));
        assert!(variants.iter().any(|value| value == "끊었어"));
        assert!(variants.iter().any(|value| value == "끊었어 demo"));
    }

    #[test]
    fn artist_variants_strip_featured_and_split_collaborators() {
        let variants = artist_variants("Epik High feat. Yoong Jin of Casker");
        assert!(variants.iter().any(|value| value == "Epik High feat. Yoong Jin of Casker"));
        assert!(variants.iter().any(|value| value == "Epik High"));
    }

    #[test]
    fn normalize_connectors_expands_symbols() {
        assert_eq!(normalize_connectors("A&B"), "A and B");
        assert_eq!(normalize_connectors("A/B"), "A B");
        assert_eq!(normalize_connectors("A×B"), "A x B");
    }

    #[test]
    fn simplify_drops_brackets_and_preserves_korean() {
        assert_eq!(simplify("Love Love Love (feat. 융진)"), "love love love");
        assert_eq!(simplify("끊었어? (demo)"), "끊었어");
    }

    #[test]
    fn duration_score_rewards_close_matches_and_penalizes_far_ones() {
        assert_eq!(duration_score(1.0), 18.0);
        assert_eq!(duration_score(2.5), 10.0);
        assert_eq!(duration_score(5.0), 4.0);
        assert_eq!(duration_score(12.0), -8.0);
        assert_eq!(duration_score(24.0), -20.0);
        assert_eq!(duration_score(8.0), 0.0);
    }
}
