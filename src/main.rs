mod bugs;
mod deezer;
mod genie;
mod logging;
mod matching;
mod musixmatch;

use std::collections::HashMap;
use std::fs::create_dir_all;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
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
use bugs::{BugsClient, BugsError, BugsTrack};
use deezer::{DeezerClient, DeezerError, DeezerTrack};
use genie::{GenieClient, GenieError, GenieTrack};
use logging::{
    backend_log_tag, bool_text, display_opt_text, display_opt_u64, display_str, matched_by_text,
    provider_log_tag, translate_log_detail, Logger,
};
use matching::{
    artist_variants, can_use_title_only_fallback, exact_title_artist_match,
    is_acceptable_bugs_match,
    is_acceptable_deezer_match, is_acceptable_genie_match, is_acceptable_match, normalize,
    score_bugs_track, score_deezer_track, score_genie_track, score_track, strip_lyrics_footer,
    title_variants,
};
use musixmatch::{Error as MxmError, Musixmatch, SortOrder, SubtitleFormat, Track, TrackId};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const CACHE_CLEANUP_INTERVAL: Duration = Duration::from_secs(10 * 60);
const DEFAULT_PROVIDER_TIMEOUT_SECS: u64 = 10;
const DEFAULT_UPDATE_TIMEOUT_SECS: u64 = 5;
const DEFAULT_PORT: u16 = 8092;
const PROVIDER_NAME: &str = "musicxmatch";
const DEEZER_PROVIDER_NAME: &str = "deezer";
const BUGS_PROVIDER_NAME: &str = "bugs";
const GENIE_PROVIDER_NAME: &str = "genie";
const VERSION_INFO_URL: &str =
    "https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/version.json";

#[derive(Clone)]
struct AppState {
    mxm: Musixmatch,
    deezer: DeezerClient,
    bugs: BugsClient,
    genie: GenieClient,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    config: Arc<Mutex<AppConfig>>,
    config_path: PathBuf,
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
    backend: Option<String>,
    #[serde(default, deserialize_with = "deserialize_boolish_opt")]
    debug: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
struct HealthPayload {
    status: &'static str,
    version: &'static str,
    provider: &'static str,
    backend: &'static str,
    cors: bool,
    #[serde(rename = "deezerConfigured")]
    deezer_configured: bool,
    #[serde(rename = "cacheEntries")]
    cache_entries: usize,
    #[serde(rename = "sessionFile")]
    session_file: String,
    #[serde(rename = "logFile")]
    log_file: String,
    #[serde(rename = "updateAvailable")]
    update_available: bool,
    #[serde(rename = "providerStatuses")]
    provider_statuses: ProviderStatusesPayload,
}

#[derive(Debug, Serialize, Clone)]
struct ProviderStatusesPayload {
    musicxmatch: &'static str,
    deezer: &'static str,
    bugs: &'static str,
    genie: &'static str,
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
    #[serde(skip_serializing)]
    matched_by: Option<&'static str>,
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

#[derive(Debug, Serialize, Deserialize)]
struct ErrorPayload {
    detail: String,
}

#[derive(Debug, Deserialize)]
struct VersionInfo {
    server: String,
    addon: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct AppConfig {
    #[serde(rename = "deezerArl", default)]
    deezer_arl: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigPayload {
    #[serde(rename = "deezerArlConfigured")]
    deezer_arl_configured: bool,
    #[serde(rename = "deezerArlPreview")]
    deezer_arl_preview: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigUpdatePayload {
    #[serde(rename = "deezerArl")]
    deezer_arl: Option<String>,
}

#[derive(Debug)]
enum LyricsError {
    Musixmatch(MxmError),
    Deezer(DeezerError),
    Bugs(BugsError),
    Genie(GenieError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendMode {
    Auto,
    Musicxmatch,
    Deezer,
    Bugs,
    Genie,
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
    #[serde(rename = "serverCommand")]
    server_command: Vec<String>,
    #[serde(rename = "allCommand")]
    all_command: Vec<String>,
}

#[derive(Debug, Serialize)]
struct UpdateApplyPayload {
    status: &'static str,
    platform: &'static str,
    command: Vec<String>,
}

#[tokio::main]
async fn main() {
    let logger = Logger::new(log_file_path());
    logger.log_tagged("Server", "서버 부팅 시작");

    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let config_path = config_file_path();

    let state = AppState {
        mxm: build_client(),
        deezer: DeezerClient::new(provider_timeout("DEEZER", DEFAULT_PROVIDER_TIMEOUT_SECS)),
        bugs: BugsClient::new(provider_timeout("BUGS", DEFAULT_PROVIDER_TIMEOUT_SECS)),
        genie: GenieClient::new(provider_timeout("GENIE", DEFAULT_PROVIDER_TIMEOUT_SECS)),
        cache: Arc::new(Mutex::new(HashMap::new())),
        config: Arc::new(Mutex::new(load_config(&config_path))),
        config_path,
        logger: logger.clone(),
    };

    spawn_cache_cleanup_task(state.cache.clone(), logger.clone());

    let app = Router::new()
        .route("/health", get(health))
        .route("/lyrics", get(get_lyrics))
        .route("/cache", delete(clear_cache))
        .route("/config", get(get_config).post(save_config))
        .route("/update/check", get(update_check))
        .route("/update/apply", post(update_apply))
        .route("/update/apply-all", post(update_apply_all))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    logger.log_tagged("Server", &format!("리스너 바인딩 준비: {addr}"));
    println!("ivLyrics MusicXMatch Server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");
    logger.log_tagged("Server", "리스너 바인딩 완료");
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
        .timeout(provider_timeout(
            "MUSIXMATCH",
            DEFAULT_PROVIDER_TIMEOUT_SECS,
        ))
        .build()
        .expect("failed to construct Musixmatch client")
}

fn provider_timeout(provider: &str, default_secs: u64) -> Duration {
    let provider_key = format!("IVLYRICS_{}_TIMEOUT_SECS", provider);
    read_timeout_secs(&provider_key)
        .or_else(|| read_timeout_secs("IVLYRICS_HTTP_TIMEOUT_SECS"))
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(default_secs))
}

fn read_timeout_secs(key: &str) -> Option<u64> {
    let raw = std::env::var(key).ok()?;
    let value = raw.trim().parse::<u64>().ok()?;
    (value > 0).then_some(value)
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

fn update_log_file_path() -> PathBuf {
    if let Ok(value) = std::env::var("IVLYRICS_MXM_UPDATE_LOG") {
        return PathBuf::from(value);
    }

    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".ivlyrics-musicxmatch");
    path.push("update.log");
    path
}

fn config_file_path() -> PathBuf {
    if let Ok(value) = std::env::var("IVLYRICS_MXM_CONFIG") {
        return PathBuf::from(value);
    }

    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".ivlyrics-musicxmatch");
    path.push("config.json");
    path
}

fn load_config(path: &PathBuf) -> AppConfig {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

fn save_config_file(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

fn deserialize_boolish_opt<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => Ok(None),
        Some("1") | Some("true") | Some("TRUE") | Some("True") => Ok(Some(true)),
        Some("0") | Some("false") | Some("FALSE") | Some("False") => Ok(Some(false)),
        Some(other) => Err(serde::de::Error::custom(format!(
            "provided string was not `true` or `false`: {other}"
        ))),
    }
}

async fn health(State(state): State<AppState>) -> Response {
    state.logger.log_tagged("Server", "GET /health 요청");
    let cache_entries = state.cache.lock().await.len();
    let deezer_configured = current_deezer_arl(&state).await.is_some();
    let update_available = latest_version_info()
        .await
        .map(|info| compare_versions(&info.server, env!("CARGO_PKG_VERSION")) > 0)
        .unwrap_or(false);
    json_response(
        StatusCode::OK,
        HealthPayload {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
            provider: PROVIDER_NAME,
            backend: "musixmatch + deezer(optional) + bugs + genie",
            cors: true,
            deezer_configured,
            cache_entries,
            session_file: session_file_path().display().to_string(),
            log_file: log_file_path().display().to_string(),
            update_available,
            provider_statuses: ProviderStatusesPayload {
                musicxmatch: "ready",
                deezer: if deezer_configured {
                    "configured"
                } else {
                    "not-configured"
                },
                bugs: "ready",
                genie: "ready",
            },
        },
    )
}

async fn clear_cache(State(state): State<AppState>) -> Response {
    state.logger.log_tagged("Server", "DELETE /cache 요청");
    let mut cache = state.cache.lock().await;
    let deleted = cache.len();
    cache.clear();
    json_response(StatusCode::OK, serde_json::json!({ "deleted": deleted }))
}

async fn get_config(State(state): State<AppState>) -> Response {
    state.logger.log_tagged("Server", "GET /config 요청");
    json_response(StatusCode::OK, runtime_config_payload(&state).await)
}

async fn save_config(
    State(state): State<AppState>,
    Json(payload): Json<ConfigUpdatePayload>,
) -> Response {
    state.logger.log_tagged("Server", "POST /config 요청");

    let next_arl = payload
        .deezer_arl
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let next = AppConfig {
        deezer_arl: next_arl,
    };

    if let Some(arl) = next.deezer_arl.as_deref() {
        match state.deezer.validate_arl(arl).await {
            Ok(()) => state
                .logger
                .log_tagged("Deezer", "설정 검증 성공"),
            Err(error) => {
                state.logger.log_tagged(
                    "Deezer",
                    &format!("설정 검증 실패 detail={}", error),
                );
                return json_response(
                    StatusCode::BAD_REQUEST,
                    ErrorPayload {
                        detail: format!("Invalid Deezer ARL: {error}"),
                    },
                );
            }
        }
    }

    if let Err(error) = save_config_file(&state.config_path, &next) {
        return json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorPayload { detail: error },
        );
    }
    if let Err(error) = set_private_file_permissions(&state.config_path) {
        state.logger.log_tagged(
            "Server",
            &format!("설정 파일 권한 조정 실패 path={} detail={error}", state.config_path.display()),
        );
    }

    *state.config.lock().await = next.clone();
    if next.deezer_arl.is_none() {
        state.deezer.clear_token().await;
    }
    json_response(StatusCode::OK, config_payload(&next))
}

async fn get_lyrics(
    State(state): State<AppState>,
    Query(query): Query<LyricsQuery>,
) -> impl IntoResponse {
    let title = query.title.unwrap_or_default().trim().to_string();
    let artist = query.artist.unwrap_or_default().trim().to_string();
    let spotify_id = query.spotify_id.unwrap_or_default().trim().to_string();
    let duration_secs = query.duration_ms.map(|value| value as f32 / 1000.0);
    let backend = parse_backend_mode(query.backend.as_deref());
    let include_debug = query.debug.unwrap_or(false);
    let request_tag = backend_log_tag(backend);
    state.logger.log_tagged(
        request_tag,
        &format!(
            "가사 조회 시작 title={:?} artist={:?} spotify_id={:?}",
            display_str(&title),
            display_str(&artist),
            display_str(&spotify_id)
        ),
    );

    if spotify_id.is_empty() && (title.is_empty() || artist.is_empty()) {
        state.logger.log_tagged(
            request_tag,
            "가사 조회 요청 거부: spotify_id 없이 title 또는 artist가 비어 있음",
        );
        return (
            StatusCode::BAD_REQUEST,
            json_response(
                StatusCode::BAD_REQUEST,
                ErrorPayload {
                    detail: "title and artist are required when spotifyId is missing".to_string(),
                },
            ),
        )
            .into_response();
    }

    let cache_key = build_cache_key(&title, &artist, &spotify_id, backend);
    if let Some(cached) = cached_payload(&state, &cache_key).await {
        state.logger.log_tagged(
            provider_log_tag(cached.provider),
            &format!(
                "캐시 사용 title={:?} artist={:?} matched_by={} track_id={} synced={} plain={} key={cache_key}",
                display_str(&title),
                display_str(&artist),
                matched_by_text(cached.matched_by),
                display_opt_u64(cached.track_id),
                bool_text(cached.lrc.is_some()),
                bool_text(cached.text.is_some())
            ),
        );
        return json_response(StatusCode::OK, cached);
    }

    match fetch_payload(
        &state,
        &title,
        &artist,
        &spotify_id,
        duration_secs,
        backend,
        include_debug,
    )
    .await
    {
        Ok(mut payload) => {
            state.logger.log_tagged(
                provider_log_tag(payload.provider),
                &format!(
                    "가사 조회 성공 title={:?} artist={:?} matched_by={} track_id={} synced={} plain={}",
                    display_opt_text(payload.track_name.as_deref()),
                    display_opt_text(payload.artist_name.as_deref()),
                    matched_by_text(payload.matched_by),
                    display_opt_u64(payload.track_id),
                    bool_text(payload.lrc.is_some()),
                    bool_text(payload.text.is_some())
                ),
            );
            payload.cached = false;
            store_cache(&state, cache_key, payload.clone()).await;
            json_response(StatusCode::OK, payload)
        }
        Err(error) => {
            let (status, detail) = map_error(error);
            state.logger.log_tagged(
                request_tag,
                &format!(
                    "가사 조회 실패 title={:?} artist={:?} spotify_id={:?} status={} detail={} raw_detail={detail}",
                    display_str(&title),
                    display_str(&artist),
                    display_str(&spotify_id),
                    status.as_u16(),
                    translate_log_detail(&detail),
                ),
            );
            json_response(status, ErrorPayload { detail })
        }
    }
}

async fn update_check(State(state): State<AppState>) -> Response {
    state.logger.log_tagged("Server", "GET /update/check 요청");
    match latest_version_info().await {
        Ok(info) => json_response(
            StatusCode::OK,
            UpdateCheckPayload {
                current_version: env!("CARGO_PKG_VERSION"),
                latest_version: info.server.clone(),
                latest_addon_version: info.addon,
                update_available: compare_versions(&info.server, env!("CARGO_PKG_VERSION")) > 0,
                platform: current_platform(),
                server_command: update_server_command_lines(),
                all_command: update_all_command_lines(),
            },
        ),
        Err(error) => json_response(StatusCode::BAD_GATEWAY, ErrorPayload { detail: error }),
    }
}

async fn update_apply(State(state): State<AppState>) -> Response {
    state.logger.log_tagged("Server", "POST /update/apply 요청");
    match spawn_update_process(false) {
        Ok(()) => json_response(
            StatusCode::ACCEPTED,
            UpdateApplyPayload {
                status: "scheduled",
                platform: current_platform(),
                command: update_server_command_lines(),
            },
        ),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorPayload { detail: error },
        ),
    }
}

async fn update_apply_all(State(state): State<AppState>) -> Response {
    state
        .logger
        .log_tagged("Server", "POST /update/apply-all 요청");
    match spawn_update_process(true) {
        Ok(()) => json_response(
            StatusCode::ACCEPTED,
            UpdateApplyPayload {
                status: "scheduled",
                platform: current_platform(),
                command: update_all_command_lines(),
            },
        ),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorPayload { detail: error },
        ),
    }
}

fn json_response<T: Serialize>(status: StatusCode, payload: T) -> Response {
    let mut response = (status, Json(payload)).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        "application/json; charset=utf-8"
            .parse()
            .expect("valid content-type header"),
    );
    response
}

async fn latest_version_info() -> Result<VersionInfo, String> {
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(
            read_timeout_secs("IVLYRICS_UPDATE_TIMEOUT_SECS")
                .unwrap_or(DEFAULT_UPDATE_TIMEOUT_SECS),
        ))
        .build()
        .map_err(|error| error.to_string())?
        .get(VERSION_INFO_URL)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!(
            "Latest version lookup failed ({})",
            response.status()
        ));
    }

    response
        .json::<VersionInfo>()
        .await
        .map_err(|error| error.to_string())
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

fn parse_backend_mode(value: Option<&str>) -> BackendMode {
    match value.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "musicxmatch" | "musixmatch" | "mxm" => BackendMode::Musicxmatch,
        "deezer" => BackendMode::Deezer,
        "bugs" => BackendMode::Bugs,
        "genie" => BackendMode::Genie,
        _ => BackendMode::Auto,
    }
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

fn runtime_path() -> String {
    let mut parts = vec![
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
        "/opt/homebrew/sbin".to_string(),
    ];

    if let Some(home) = dirs::home_dir() {
        parts.push(home.join(".cargo/bin").display().to_string());
        parts.push(home.join(".spicetify").display().to_string());
    }

    if let Ok(existing) = std::env::var("PATH") {
        parts.push(existing);
    }

    let mut seen = std::collections::HashSet::new();
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .filter(|part| seen.insert(part.clone()))
        .collect::<Vec<_>>()
        .join(":")
}

async fn current_deezer_arl(state: &AppState) -> Option<String> {
    if let Ok(value) = std::env::var("DEEZER_ARL") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    state
        .config
        .lock()
        .await
        .deezer_arl
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn config_payload(config: &AppConfig) -> ConfigPayload {
    ConfigPayload {
        deezer_arl_configured: config
            .deezer_arl
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        deezer_arl_preview: config
            .deezer_arl
            .as_deref()
            .map(mask_secret)
            .filter(|value| !value.is_empty()),
    }
}

async fn runtime_config_payload(state: &AppState) -> ConfigPayload {
    if let Ok(value) = std::env::var("DEEZER_ARL") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return ConfigPayload {
                deezer_arl_configured: true,
                deezer_arl_preview: Some(mask_secret(trimmed)),
            };
        }
    }

    let config = state.config.lock().await.clone();
    config_payload(&config)
}

fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return "••••".to_string();
    }

    let prefix = chars.iter().take(4).collect::<String>();
    let suffix = chars.iter().rev().take(4).collect::<Vec<_>>();
    let suffix = suffix.into_iter().rev().collect::<String>();
    format!("{prefix}…{suffix}")
}

fn update_runner_script_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".ivlyrics-musicxmatch");
    path.push("run-update.sh");
    path
}

fn update_server_command_lines() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            "$env:IVLYRICS_SKIP_ADDONS='1'; iwr -useb \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1\" | iex".to_string(),
        ]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![
            "export IVLYRICS_SKIP_ADDONS=1; curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash".to_string(),
        ]
    }
}

fn update_all_command_lines() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec!["iwr -useb \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1\" | iex".to_string()]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec!["curl -fsSL https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh | bash".to_string()]
    }
}

fn spawn_update_process(include_addon: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let install_command = if include_addon {
            "iwr -useb \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1\" | iex".to_string()
        } else {
            "$env:IVLYRICS_SKIP_ADDONS='1'; iwr -useb \"https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.ps1\" | iex".to_string()
        };
        let command = format!(
            "Start-Process powershell.exe -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command','Start-Sleep -Seconds 1; {}'",
            install_command
        );
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
            ])
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let path = runtime_path();
        let update_log = update_log_file_path();
        let runner_script = update_runner_script_path();
        if let Some(parent) = update_log.parent() {
            let _ = create_dir_all(parent);
        }
        if let Some(parent) = runner_script.parent() {
            create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let mut script_lines = vec![
            "#!/bin/sh".to_string(),
            "set -eu".to_string(),
            format!("export HOME='{}'", home_dir.display()),
            format!("export PATH='{}'", path),
            format!("echo \"[update] 시작 include_addon={include_addon}\""),
            "echo \"[update] HOME=$HOME\"".to_string(),
            "echo \"[update] PATH=$PATH\"".to_string(),
            "sleep 1".to_string(),
        ];

        if include_addon {
            script_lines.push(
                "curl -fsSL 'https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh' | bash"
                    .to_string(),
            );
        } else {
            script_lines.push("export IVLYRICS_SKIP_ADDONS=1".to_string());
            script_lines.push(
                "curl -fsSL 'https://raw.githubusercontent.com/oneulddu/musicxmatch-api/main/install.sh' | bash"
                    .to_string(),
            );
        }

        script_lines.push("echo \"[update] 완료\"".to_string());
        std::fs::write(&runner_script, format!("{}\n", script_lines.join("\n")))
            .map_err(|error| error.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&runner_script, permissions)
                .map_err(|error| error.to_string())?;
        }

        Command::new("sh")
            .env("PATH", path)
            .args([
                "-c",
                &format!(
                    "nohup sh '{}' >> '{}' 2>&1 &",
                    runner_script.display(),
                    update_log.display()
                ),
            ])
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

fn spawn_cache_cleanup_task(
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    logger: Logger,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CACHE_CLEANUP_INTERVAL);
        loop {
            interval.tick().await;
            let now = Instant::now();
            let mut cache = cache.lock().await;
            let before = cache.len();
            cache.retain(|_, entry| entry.expires_at > now);
            let removed = before.saturating_sub(cache.len());
            if removed > 0 {
                logger.log_tagged(
                    "Server",
                    &format!("캐시 정리 완료 removed={} remaining={}", removed, cache.len()),
                );
            }
        }
    });
}

fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, permissions).map_err(|error| error.to_string())
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn build_cache_key(title: &str, artist: &str, spotify_id: &str, backend: BackendMode) -> String {
    let prefix = match backend {
        BackendMode::Auto => "auto",
        BackendMode::Musicxmatch => "musicxmatch",
        BackendMode::Deezer => "deezer",
        BackendMode::Bugs => "bugs",
        BackendMode::Genie => "genie",
    };
    if !spotify_id.is_empty() {
        return format!("{prefix}:spotify:{spotify_id}");
    }
    format!("{prefix}:{}::{}", normalize(title), normalize(artist))
}

async fn fetch_payload(
    state: &AppState,
    title: &str,
    artist: &str,
    spotify_id: &str,
    duration_secs: Option<f32>,
    backend: BackendMode,
    include_debug: bool,
) -> Result<LyricsPayload, LyricsError> {
    match backend {
        BackendMode::Musicxmatch => fetch_musixmatch_payload(
            &state.mxm,
            title,
            artist,
            spotify_id,
            duration_secs,
            include_debug,
        )
        .await
        .map_err(LyricsError::Musixmatch),
        BackendMode::Deezer => {
            let arl = current_deezer_arl(state)
                .await
                .ok_or(LyricsError::Deezer(DeezerError::ConfigMissing))?;
            fetch_deezer_payload(
                &state.deezer,
                &arl,
                title,
                artist,
                duration_secs,
                include_debug,
            )
            .await
            .map_err(LyricsError::Deezer)
        }
        BackendMode::Bugs => {
            fetch_bugs_payload(&state.bugs, title, artist, duration_secs, include_debug)
                .await
                .map_err(LyricsError::Bugs)
        }
        BackendMode::Genie => fetch_genie_payload(
            &state.genie,
            title,
            artist,
            duration_secs,
            include_debug,
        )
        .await
        .map_err(LyricsError::Genie),
        BackendMode::Auto => match fetch_musixmatch_payload(
            &state.mxm,
            title,
            artist,
            spotify_id,
            duration_secs,
            include_debug,
        )
        .await
        {
            Ok(payload) => Ok(payload),
            Err(error) => {
                state.logger.log_tagged(
                    "MusicXMatch",
                    &format!("조회 실패, fallback provider 시도: {error}"),
                );
                let mut deezer_error = None;
                if let Some(arl) = current_deezer_arl(state).await {
                    match fetch_deezer_payload(
                        &state.deezer,
                        &arl,
                        title,
                        artist,
                        duration_secs,
                        include_debug,
                    )
                    .await
                    {
                        Ok(payload) => return Ok(payload),
                        Err(next_error) => {
                            state.logger.log_tagged(
                                "Deezer",
                                &format!("fallback 조회 실패, Bugs provider 시도: {next_error}"),
                            );
                            deezer_error = Some(next_error);
                        }
                    }
                }

                let bugs_result =
                    fetch_bugs_payload(&state.bugs, title, artist, duration_secs, include_debug)
                        .await;
                if let Ok(payload) = bugs_result {
                    return Ok(payload);
                }

                let bugs_error = bugs_result.err();
                state.logger.log_tagged(
                    "Bugs",
                    "fallback 조회 실패, Genie provider 시도",
                );

                match fetch_genie_payload(&state.genie, title, artist, duration_secs, include_debug)
                    .await
                {
                    Ok(payload) => Ok(payload),
                    Err(next_error) => {
                        if let Some(deezer_error) = deezer_error {
                            Err(LyricsError::Deezer(deezer_error))
                        } else if let Some(bugs_error) = bugs_error {
                            if matches!(error, MxmError::NotFound | MxmError::NotAvailable) {
                                Err(LyricsError::Bugs(bugs_error))
                            } else {
                                Err(LyricsError::Musixmatch(error))
                            }
                        } else if matches!(error, MxmError::NotFound | MxmError::NotAvailable) {
                            Err(LyricsError::Genie(next_error))
                        } else {
                            Err(LyricsError::Musixmatch(error))
                        }
                    }
                }
            }
        },
    }
}

async fn fetch_musixmatch_payload(
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
            "track_id",
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
        resolution.matched_by,
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
    matched_by: &'static str,
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
                matched_by: Some(matched_by),
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
        matched_by: Some(matched_by),
        debug,
    })
}

async fn fetch_deezer_payload(
    deezer: &DeezerClient,
    arl: &str,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
    include_debug: bool,
) -> Result<LyricsPayload, DeezerError> {
    let resolution = resolve_deezer_tracks(deezer, title, artist, duration_secs).await?;

    for track in resolution.tracks {
        match deezer.fetch_lyrics_for_track(arl, &track).await {
            Ok(payload) => {
                return Ok(LyricsPayload {
                    provider: DEEZER_PROVIDER_NAME,
                    track_id: Some(payload.track_id),
                    track_name: Some(payload.track_name),
                    artist_name: Some(payload.artist_name),
                    lrc: payload.lrc,
                    text: payload.text,
                    cached: false,
                    matched_by: Some(resolution.matched_by),
                    debug: include_debug.then(|| DebugPayload {
                        source: "deezer_search",
                        matched_by: resolution.matched_by,
                        duration_ms: duration_secs.map(|value| (value * 1000.0).round() as u64),
                        selected_track_id: Some(payload.track_id),
                        selected_track_duration_ms: payload.duration_ms,
                        search_variants: resolution.search_variants.clone(),
                    }),
                });
            }
            Err(DeezerError::NotFound | DeezerError::NotAvailable) => continue,
            Err(error) => return Err(error),
        }
    }

    Err(DeezerError::NotAvailable)
}

async fn fetch_bugs_payload(
    bugs: &BugsClient,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
    include_debug: bool,
) -> Result<LyricsPayload, BugsError> {
    let resolution = resolve_bugs_tracks(bugs, title, artist, duration_secs).await?;

    for track in resolution.tracks {
        match bugs.fetch_lyrics_for_track(&track).await {
            Ok(payload) => {
                return Ok(LyricsPayload {
                    provider: BUGS_PROVIDER_NAME,
                    track_id: Some(payload.track_id),
                    track_name: Some(payload.track_name),
                    artist_name: Some(payload.artist_name),
                    lrc: payload.lrc,
                    text: payload.text,
                    cached: false,
                    matched_by: Some(resolution.matched_by),
                    debug: include_debug.then(|| DebugPayload {
                        source: "bugs_search",
                        matched_by: resolution.matched_by,
                        duration_ms: duration_secs.map(|value| (value * 1000.0).round() as u64),
                        selected_track_id: Some(payload.track_id),
                        selected_track_duration_ms: payload.duration_ms,
                        search_variants: resolution.search_variants.clone(),
                    }),
                });
            }
            Err(BugsError::NotFound | BugsError::NotAvailable) => continue,
            Err(error) => return Err(error),
        }
    }

    Err(BugsError::NotAvailable)
}

async fn fetch_genie_payload(
    genie: &GenieClient,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
    include_debug: bool,
) -> Result<LyricsPayload, GenieError> {
    let resolution = resolve_genie_tracks(genie, title, artist, duration_secs).await?;

    for track in resolution.tracks {
        match genie.fetch_lyrics_for_track(&track).await {
            Ok(payload) => {
                return Ok(LyricsPayload {
                    provider: GENIE_PROVIDER_NAME,
                    track_id: Some(payload.track_id),
                    track_name: Some(payload.track_name),
                    artist_name: Some(payload.artist_name),
                    lrc: payload.lrc,
                    text: payload.text,
                    cached: false,
                    matched_by: Some(resolution.matched_by),
                    debug: include_debug.then(|| DebugPayload {
                        source: "genie_search",
                        matched_by: resolution.matched_by,
                        duration_ms: duration_secs.map(|value| (value * 1000.0).round() as u64),
                        selected_track_id: Some(payload.track_id),
                        selected_track_duration_ms: payload.duration_ms,
                        search_variants: resolution.search_variants.clone(),
                    }),
                });
            }
            Err(GenieError::NotFound | GenieError::NotAvailable) => continue,
            Err(error) => return Err(error),
        }
    }

    Err(GenieError::NotAvailable)
}

struct TrackResolution {
    track: Track,
    matched_by: &'static str,
    search_variants: Vec<String>,
}

struct DeezerTrackResolution {
    tracks: Vec<DeezerTrack>,
    matched_by: &'static str,
    search_variants: Vec<String>,
}

struct BugsTrackResolution {
    tracks: Vec<BugsTrack>,
    matched_by: &'static str,
    search_variants: Vec<String>,
}

struct GenieTrackResolution {
    tracks: Vec<GenieTrack>,
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

    'title_artist_search: for title_variant in &title_variants {
        for artist_variant in &artist_variants {
            attempted_variants.push(format!("title={title_variant} | artist={artist_variant}"));
            let tracks = search_tracks(mxm, Some(title_variant), Some(artist_variant)).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
            if has_exact_musixmatch_candidate(&tracks_by_id, title, artist) {
                break 'title_artist_search;
            }
        }
    }

    if tracks_by_id.is_empty() && can_use_title_only_fallback(title) {
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
                attempted_variants.push(format!(
                    "matcher title={title_variant} | artist={artist_variant}"
                ));
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
        let matched = mxm
            .matcher_track(title, artist, "", false, false, false)
            .await?;
        tracks_by_id.insert(matched.track_id, matched);
    }

    tracks_by_id
        .into_values()
        .max_by(|left, right| {
            score_track(left, title, artist, duration_secs)
                .partial_cmp(&score_track(right, title, artist, duration_secs))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|track| is_acceptable_match(track, title, artist, matched_by))
        .map(|track| TrackResolution {
            track,
            matched_by,
            search_variants: attempted_variants,
        })
        .ok_or(MxmError::NotFound)
}

async fn resolve_deezer_tracks(
    deezer: &DeezerClient,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> Result<DeezerTrackResolution, DeezerError> {
    let mut tracks_by_id = HashMap::new();
    let title_variants = title_variants(title);
    let artist_variants = artist_variants(artist);
    let mut attempted_variants = Vec::new();
    let mut matched_by = "search:title+artist";

    'title_artist_search: for title_variant in &title_variants {
        for artist_variant in &artist_variants {
            attempted_variants.push(format!("title={title_variant} | artist={artist_variant}"));
            let tracks = deezer
                .search_tracks(Some(title_variant), Some(artist_variant))
                .await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
            if has_exact_deezer_candidate(&tracks_by_id, title, artist) {
                break 'title_artist_search;
            }
        }
    }

    if tracks_by_id.is_empty() && can_use_title_only_fallback(title) {
        matched_by = "search:title";
        for title_variant in &title_variants {
            attempted_variants.push(format!("title={title_variant} | artist=<none>"));
            let tracks = deezer.search_tracks(Some(title_variant), None).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        return Err(DeezerError::NotFound);
    }

    let mut candidates = tracks_by_id.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        score_deezer_track(right, title, artist, duration_secs)
            .partial_cmp(&score_deezer_track(left, title, artist, duration_secs))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.retain(|track| is_acceptable_deezer_match(track, title, artist, matched_by));

    if candidates.is_empty() {
        return Err(DeezerError::NotFound);
    }

    Ok(DeezerTrackResolution {
        tracks: candidates,
        matched_by,
        search_variants: attempted_variants,
    })
}

async fn resolve_bugs_tracks(
    bugs: &BugsClient,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> Result<BugsTrackResolution, BugsError> {
    let mut tracks_by_id = HashMap::new();
    let title_variants = title_variants(title);
    let artist_variants = artist_variants(artist);
    let mut attempted_variants = Vec::new();
    let mut matched_by = "search:title+artist";

    'title_artist_search: for title_variant in &title_variants {
        for artist_variant in &artist_variants {
            attempted_variants.push(format!("title={title_variant} | artist={artist_variant}"));
            let tracks = bugs
                .search_tracks(Some(title_variant), Some(artist_variant))
                .await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
            if has_exact_bugs_candidate(&tracks_by_id, title, artist) {
                break 'title_artist_search;
            }
        }
    }

    if tracks_by_id.is_empty() && can_use_title_only_fallback(title) {
        matched_by = "search:title";
        for title_variant in &title_variants {
            attempted_variants.push(format!("title={title_variant} | artist=<none>"));
            let tracks = bugs.search_tracks(Some(title_variant), None).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        return Err(BugsError::NotFound);
    }

    let mut candidates = tracks_by_id.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        score_bugs_track(right, title, artist, duration_secs)
            .partial_cmp(&score_bugs_track(left, title, artist, duration_secs))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.retain(|track| is_acceptable_bugs_match(track, title, artist, matched_by));

    if candidates.is_empty() {
        return Err(BugsError::NotFound);
    }

    Ok(BugsTrackResolution {
        tracks: candidates,
        matched_by,
        search_variants: attempted_variants,
    })
}

async fn resolve_genie_tracks(
    genie: &GenieClient,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> Result<GenieTrackResolution, GenieError> {
    let mut tracks_by_id = HashMap::new();
    let title_variants = title_variants(title);
    let artist_variants = artist_variants(artist);
    let mut attempted_variants = Vec::new();
    let mut matched_by = "search:title+artist";

    'title_artist_search: for title_variant in &title_variants {
        for artist_variant in &artist_variants {
            attempted_variants.push(format!("title={title_variant} | artist={artist_variant}"));
            let tracks = genie
                .search_tracks(Some(title_variant), Some(artist_variant))
                .await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
            if has_exact_genie_candidate(&tracks_by_id, title, artist) {
                break 'title_artist_search;
            }
        }
    }

    if tracks_by_id.is_empty() && can_use_title_only_fallback(title) {
        matched_by = "search:title";
        for title_variant in &title_variants {
            attempted_variants.push(format!("title={title_variant} | artist=<none>"));
            let tracks = genie.search_tracks(Some(title_variant), None).await?;
            for track in tracks {
                tracks_by_id.entry(track.track_id).or_insert(track);
            }
        }
    }

    if tracks_by_id.is_empty() {
        return Err(GenieError::NotFound);
    }

    let mut candidates = tracks_by_id.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        score_genie_track(right, title, artist, duration_secs)
            .partial_cmp(&score_genie_track(left, title, artist, duration_secs))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.retain(|track| is_acceptable_genie_match(track, title, artist, matched_by));

    if candidates.is_empty() {
        return Err(GenieError::NotFound);
    }

    Ok(GenieTrackResolution {
        tracks: candidates,
        matched_by,
        search_variants: attempted_variants,
    })
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

fn has_exact_musixmatch_candidate(
    tracks_by_id: &HashMap<u64, Track>,
    title: &str,
    artist: &str,
) -> bool {
    tracks_by_id.values().any(|track| {
        exact_title_artist_match(&track.track_name, &track.artist_name, title, artist)
    })
}

fn has_exact_deezer_candidate(
    tracks_by_id: &HashMap<u64, DeezerTrack>,
    title: &str,
    artist: &str,
) -> bool {
    tracks_by_id.values().any(|track| {
        exact_title_artist_match(&track.track_name, &track.artist_name, title, artist)
    })
}

fn has_exact_bugs_candidate(
    tracks_by_id: &HashMap<u64, BugsTrack>,
    title: &str,
    artist: &str,
) -> bool {
    tracks_by_id.values().any(|track| {
        exact_title_artist_match(&track.track_name, &track.artist_name, title, artist)
    })
}

fn has_exact_genie_candidate(
    tracks_by_id: &HashMap<u64, GenieTrack>,
    title: &str,
    artist: &str,
) -> bool {
    tracks_by_id.values().any(|track| {
        exact_title_artist_match(&track.track_name, &track.artist_name, title, artist)
    })
}

fn map_error(error: LyricsError) -> (StatusCode, String) {
    match error {
        LyricsError::Bugs(BugsError::NotFound) => {
            (StatusCode::NOT_FOUND, "No tracks found".to_string())
        }
        LyricsError::Bugs(BugsError::NotAvailable) => (
            StatusCode::NOT_FOUND,
            "No lyrics are available for this track".to_string(),
        ),
        LyricsError::Bugs(BugsError::Provider(detail)) => {
            (StatusCode::BAD_GATEWAY, format!("Bugs error: {detail}"))
        }
        LyricsError::Genie(GenieError::NotFound) => {
            (StatusCode::NOT_FOUND, "No tracks found".to_string())
        }
        LyricsError::Genie(GenieError::NotAvailable) => (
            StatusCode::NOT_FOUND,
            "No lyrics are available for this track".to_string(),
        ),
        LyricsError::Genie(GenieError::Provider(detail)) => {
            (StatusCode::BAD_GATEWAY, format!("Genie error: {detail}"))
        }
        LyricsError::Deezer(DeezerError::ConfigMissing) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Deezer ARL cookie is not configured.".to_string(),
        ),
        LyricsError::Deezer(DeezerError::NotFound) => {
            (StatusCode::NOT_FOUND, "No tracks found".to_string())
        }
        LyricsError::Deezer(DeezerError::NotAvailable) => (
            StatusCode::NOT_FOUND,
            "No lyrics are available for this track".to_string(),
        ),
        LyricsError::Deezer(DeezerError::Auth(detail)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Deezer authentication failed: {detail}"),
        ),
        LyricsError::Deezer(DeezerError::Provider(detail)) => {
            (StatusCode::BAD_GATEWAY, format!("Deezer error: {detail}"))
        }
        LyricsError::Musixmatch(error) => match error {
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
            MxmError::Provider { status_code, msg } => (
                StatusCode::BAD_GATEWAY,
                format!("Musixmatch error {status_code}: {msg}"),
            ),
            other => (StatusCode::BAD_GATEWAY, other.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use crate::matching::{
        collapse_to_words, duration_score, normalize_connectors, similarity, simplify,
    };
    use std::fs;

    #[test]
    fn collapse_to_words_preserves_unicode_letters() {
        assert_eq!(
            collapse_to_words("에픽하이 feat. 융진"),
            "에픽하이 feat 융진"
        );
        assert_eq!(collapse_to_words("끊었어? (demo)"), "끊었어 demo");
    }

    #[test]
    fn similarity_counts_unicode_characters_not_utf8_bytes() {
        assert_eq!(similarity("끊었어", "끊었어"), 1.0);
        assert!(similarity("끊었어", "끊었어 demo") > 0.3);
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
        assert!(variants
            .iter()
            .any(|value| value == "Epik High feat. Yoong Jin of Casker"));
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

    #[test]
    fn short_single_word_titles_do_not_use_title_only_fallback() {
        assert!(!can_use_title_only_fallback("KO"));
        assert!(!can_use_title_only_fallback("VVS"));
        assert!(can_use_title_only_fallback("Love Love Love"));
        assert!(can_use_title_only_fallback("끊었어?"));
    }

    #[test]
    fn compare_versions_handles_semver_like_strings() {
        assert!(compare_versions("0.3.2", "0.3.1") > 0);
        assert!(compare_versions("0.3.1", "0.3.2") < 0);
        assert_eq!(compare_versions("0.3.1", "0.3.1"), 0);
    }

    #[test]
    fn parse_backend_mode_handles_expected_values() {
        assert_eq!(parse_backend_mode(None), BackendMode::Auto);
        assert_eq!(parse_backend_mode(Some("auto")), BackendMode::Auto);
        assert_eq!(
            parse_backend_mode(Some("musicxmatch")),
            BackendMode::Musicxmatch
        );
        assert_eq!(
            parse_backend_mode(Some("musixmatch")),
            BackendMode::Musicxmatch
        );
        assert_eq!(parse_backend_mode(Some("deezer")), BackendMode::Deezer);
        assert_eq!(parse_backend_mode(Some("bugs")), BackendMode::Bugs);
        assert_eq!(parse_backend_mode(Some("genie")), BackendMode::Genie);
    }

    #[test]
    fn cache_keys_include_backend_mode() {
        let auto = build_cache_key("Tell Me", "CAMO", "abc", BackendMode::Auto);
        let mxm = build_cache_key("Tell Me", "CAMO", "abc", BackendMode::Musicxmatch);
        let deezer = build_cache_key("Tell Me", "CAMO", "abc", BackendMode::Deezer);
        let bugs = build_cache_key("Tell Me", "CAMO", "abc", BackendMode::Bugs);
        let genie = build_cache_key("Tell Me", "CAMO", "abc", BackendMode::Genie);
        assert_ne!(auto, mxm);
        assert_ne!(mxm, deezer);
        assert_ne!(auto, deezer);
        assert_ne!(deezer, bugs);
        assert_ne!(mxm, bugs);
        assert_ne!(bugs, genie);
        assert_ne!(deezer, genie);
    }

    #[test]
    fn mask_secret_preserves_only_edges() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("abcd"), "••••");
        assert_eq!(mask_secret("abcdefghijkl"), "abcd…ijkl");
    }

    #[test]
    fn deezer_short_titles_follow_same_title_only_rule() {
        assert!(!can_use_title_only_fallback("KO"));
        assert!(can_use_title_only_fallback("신호는 잘 지켜"));
    }

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ivlyrics-musicxmatch-{name}-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        ))
    }

    fn test_state(config: AppConfig, config_path: PathBuf) -> AppState {
        let session_path = test_path("session");
        AppState {
            mxm: Musixmatch::builder()
                .storage_file(session_path)
                .timeout(Duration::from_secs(1))
                .build()
                .expect("test musixmatch client should build"),
            deezer: DeezerClient::new(Duration::from_secs(1)),
            bugs: BugsClient::new(Duration::from_secs(1)),
            genie: GenieClient::new(Duration::from_secs(1)),
            cache: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(Mutex::new(config)),
            config_path,
            logger: Logger::new(test_path("log")),
        }
    }

    async fn parse_response_json<T: for<'de> Deserialize<'de>>(response: Response) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&bytes).expect("response body should be valid json")
    }

    #[tokio::test]
    async fn get_config_returns_masked_deezer_arl_preview() {
        let config_path = test_path("config");
        let state = test_state(
            AppConfig {
                deezer_arl: Some("abcdefghijklmnop".to_string()),
            },
            config_path,
        );

        let response = get_config(State(state)).await;
        let payload: ConfigPayload = parse_response_json(response).await;

        assert!(payload.deezer_arl_configured);
        assert_eq!(payload.deezer_arl_preview.as_deref(), Some("abcd…mnop"));
    }

    #[tokio::test]
    async fn save_config_clears_deezer_arl_and_persists_file() {
        let config_path = test_path("config");
        let state = test_state(
            AppConfig {
                deezer_arl: Some("abcdefghijklmnop".to_string()),
            },
            config_path.clone(),
        );

        let response = save_config(
            State(state.clone()),
            Json(ConfigUpdatePayload { deezer_arl: None }),
        )
        .await;
        let payload: ConfigPayload = parse_response_json(response).await;

        assert!(!payload.deezer_arl_configured);
        assert_eq!(payload.deezer_arl_preview, None);

        let saved = fs::read_to_string(&config_path).expect("config should be written");
        let parsed: AppConfig = serde_json::from_str(&saved).expect("config json should parse");
        assert_eq!(parsed.deezer_arl, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn save_config_writes_private_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let config_path = test_path("config");
        let state = test_state(AppConfig::default(), config_path.clone());

        let response = save_config(
            State(state),
            Json(ConfigUpdatePayload { deezer_arl: None }),
        )
        .await;
        let _: ConfigPayload = parse_response_json(response).await;

        let mode = fs::metadata(&config_path)
            .expect("config should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
