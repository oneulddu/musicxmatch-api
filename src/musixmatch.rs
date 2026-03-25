use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::header::{self, HeaderMap};
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use sha1::Sha1;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use tokio::sync::Mutex;

const APP_ID: &str = "android-player-v1.0";
const API_URL: &str = "https://apic.musixmatch.com/ws/1.1/";
const SIGNATURE_SECRET: &[u8; 29] = b"mNdca@6W7TeEcFn6*3.s97sJ*yPMd";
const DEFAULT_UA: &str = "Dalvik/2.1.0 (Linux; U; Android 13; Pixel 6 Build/T3B2.230316.003)";
const DEFAULT_BRAND: &str = "Google";
const DEFAULT_DEVICE: &str = "Pixel 6";

const YMD_FORMAT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]");

static RANDOM_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct Musixmatch {
    inner: Arc<MusixmatchRef>,
}

struct MusixmatchRef {
    http: Client,
    session_path: Option<PathBuf>,
    brand: String,
    device: String,
    usertoken: Mutex<Option<String>>,
}

#[derive(Default)]
pub struct MusixmatchBuilder {
    storage_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackId<'a> {
    TrackId(u64),
    Spotify(Cow<'a, str>),
}

impl TrackId<'_> {
    fn to_param(&self) -> (&'static str, String) {
        match self {
            Self::TrackId(id) => ("track_id", id.to_string()),
            Self::Spotify(id) => ("track_spotify_id", id.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleFormat {
    Lrc,
}

impl SubtitleFormat {
    fn to_param(self) -> &'static str {
        match self {
            Self::Lrc => "lrc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Desc,
}

impl SortOrder {
    fn as_str(self) -> &'static str {
        match self {
            Self::Desc => "desc",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Track {
    pub track_id: u64,
    pub track_name: String,
    pub track_length: u32,
    pub artist_name: String,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub has_lyrics: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub has_subtitles: bool,
    #[serde(default, deserialize_with = "bool_from_int")]
    pub has_richsync: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Lyrics {
    pub lyrics_id: u64,
    pub lyrics_body: String,
    #[serde(default, deserialize_with = "null_if_empty")]
    pub lyrics_copyright: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Subtitle {
    pub subtitle_id: u64,
    pub subtitle_body: String,
}

#[derive(Debug)]
pub enum Error {
    Ratelimit,
    TokenExpired,
    NotFound,
    NotAvailable,
    InvalidData(String),
    Http(reqwest::Error),
    Provider { status_code: u16, msg: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ratelimit => write!(f, "You are sending requests too fast. Wait a minute and try again."),
            Self::TokenExpired => write!(f, "The Musixmatch user token expired. Request a new one."),
            Self::NotFound => write!(f, "The requested content could not be found"),
            Self::NotAvailable => write!(f, "Unfortunately we're not authorized to show these lyrics"),
            Self::InvalidData(detail) => write!(f, "JSON parsing error: {detail}"),
            Self::Http(error) => write!(f, "http error: {error}"),
            Self::Provider { status_code, msg } => {
                write!(f, "Error {status_code} returned by the Musixmatch API. Message: '{msg}'")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value.without_url())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidData(value.to_string())
    }
}

impl Musixmatch {
    pub fn builder() -> MusixmatchBuilder {
        MusixmatchBuilder::default()
    }

    pub async fn matcher_track(
        &self,
        q_track: &str,
        q_artist: &str,
        q_album: &str,
        _translation_status: bool,
        _lang_3c: bool,
        _performer_tagging: bool,
    ) -> Result<Track, Error> {
        let mut url = self.new_url("matcher.track.get");
        {
            let mut query = url.query_pairs_mut();
            if !q_track.is_empty() {
                query.append_pair("q_track", q_track);
            }
            if !q_artist.is_empty() {
                query.append_pair("q_artist", q_artist);
            }
            if !q_album.is_empty() {
                query.append_pair("q_album", q_album);
            }
            query.finish();
        }

        let body = self.execute_get_request::<TrackBody>(&url).await?;
        Ok(body.track)
    }

    pub async fn track(
        &self,
        id: TrackId<'_>,
        _translation_status: bool,
        _lang_3c: bool,
        _performer_tagging: bool,
    ) -> Result<Track, Error> {
        let mut url = self.new_url("track.get");
        {
            let mut query = url.query_pairs_mut();
            let param = id.to_param();
            query.append_pair(param.0, &param.1);
            query.finish();
        }
        let body = self.execute_get_request::<TrackBody>(&url).await?;
        Ok(body.track)
    }

    pub fn track_search(&self) -> TrackSearchQuery<'_> {
        TrackSearchQuery {
            mxm: self.clone(),
            q_track: None,
            q_artist: None,
            f_has_lyrics: false,
            s_track_rating: None,
        }
    }

    pub async fn track_subtitle(
        &self,
        id: TrackId<'_>,
        subtitle_format: SubtitleFormat,
        f_subtitle_length: Option<f32>,
        f_subtitle_length_max_deviation: Option<f32>,
    ) -> Result<Subtitle, Error> {
        let mut url = self.new_url("track.subtitle.get");
        {
            let mut query = url.query_pairs_mut();
            let param = id.to_param();
            query.append_pair(param.0, &param.1);
            query.append_pair("subtitle_format", subtitle_format.to_param());
            if let Some(value) = f_subtitle_length {
                query.append_pair("f_subtitle_length", &value.to_string());
            }
            if let Some(value) = f_subtitle_length_max_deviation {
                query.append_pair("f_subtitle_length_max_deviation", &value.to_string());
            }
            query.finish();
        }
        let body = self.execute_get_request::<SubtitleBody>(&url).await?;
        Ok(body.subtitle)
    }

    pub async fn track_lyrics(&self, id: TrackId<'_>) -> Result<Lyrics, Error> {
        let mut url = self.new_url("track.lyrics.get");
        {
            let mut query = url.query_pairs_mut();
            let param = id.to_param();
            query.append_pair(param.0, &param.1);
            query.finish();
        }
        let body = self.execute_get_request::<LyricsBody>(&url).await?;
        body.validate()?;
        Ok(body.lyrics)
    }

    async fn get_usertoken(&self, force_new: bool) -> Result<String, Error> {
        let mut stored = self.inner.usertoken.lock().await;
        if !force_new {
            if let Some(token) = stored.as_ref() {
                return Ok(token.clone());
            }
        }

        let now = OffsetDateTime::now_utc();
        let guid = random_guid();
        let adv_id = random_uuid();

        let mut url = Url::parse_with_params(
            &format!("{API_URL}token.get"),
            &[
                ("adv_id", adv_id.as_str()),
                ("root", "0"),
                ("sideloaded", "0"),
                ("app_id", APP_ID),
                ("build_number", "2022090901"),
                ("guid", guid.as_str()),
                ("lang", "en_US"),
                ("model", self.model_string().as_str()),
                ("timestamp", now.format(&Rfc3339).unwrap_or_default().as_str()),
                ("format", "json"),
            ],
        )
        .expect("valid Musixmatch token URL");
        sign_url_with_date(&mut url, now);

        let response = self
            .inner
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?;
        let text = response.text().await?;
        let token = parse_body::<GetToken>(&text)?.user_token;

        *stored = Some(token.clone());
        self.store_session(&token);
        Ok(token)
    }

    fn new_url(&self, endpoint: &str) -> Url {
        Url::parse_with_params(
            &format!("{API_URL}{endpoint}"),
            &[("app_id", APP_ID), ("format", "json")],
        )
        .expect("valid Musixmatch URL")
    }

    async fn finish_url(&self, url: &mut Url, force_new_session: bool) -> Result<(), Error> {
        let usertoken = self.get_usertoken(force_new_session).await?;
        url.query_pairs_mut()
            .append_pair("usertoken", &usertoken)
            .finish();
        sign_url_with_date(url, OffsetDateTime::now_utc());
        Ok(())
    }

    async fn execute_get_request<T>(&self, url: &Url) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let mut request_url = url.clone();
        self.finish_url(&mut request_url, false).await?;
        let response = self
            .inner
            .http
            .get(request_url)
            .send()
            .await?
            .error_for_status()?;
        let body = response.text().await?;

        match parse_body::<T>(&body) {
            Ok(parsed) => Ok(parsed),
            Err(Error::TokenExpired) => {
                let mut retry_url = url.clone();
                self.finish_url(&mut retry_url, true).await?;
                let response = self
                    .inner
                    .http
                    .get(retry_url)
                    .send()
                    .await?
                    .error_for_status()?;
                let body = response.text().await?;
                parse_body::<T>(&body)
            }
            Err(error) => Err(error),
        }
    }

    fn model_string(&self) -> String {
        format!(
            "manufacturer/{0} brand/{0} model/{1}",
            self.inner.brand, self.inner.device
        )
    }

    fn store_session(&self, usertoken: &str) {
        let Some(path) = self.inner.session_path.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let stored = StoredSession {
            usertoken: usertoken.to_string(),
        };
        if let Ok(json) = serde_json::to_vec_pretty(&stored) {
            let _ = std::fs::write(path, json);
        }
    }
}

impl MusixmatchBuilder {
    pub fn storage_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.storage_file = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn build(self) -> Result<Musixmatch, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "AWSELBCORS=0; AWSELB=0".parse().unwrap());

        let http = Client::builder()
            .user_agent(DEFAULT_UA)
            .default_headers(headers)
            .build()?;

        let session_path = self.storage_file;
        let stored_session = retrieve_session(session_path.as_deref());

        Ok(Musixmatch {
            inner: Arc::new(MusixmatchRef {
                http,
                session_path,
                brand: DEFAULT_BRAND.to_string(),
                device: DEFAULT_DEVICE.to_string(),
                usertoken: Mutex::new(stored_session.map(|session| session.usertoken)),
            }),
        })
    }
}

pub struct TrackSearchQuery<'a> {
    mxm: Musixmatch,
    q_track: Option<&'a str>,
    q_artist: Option<&'a str>,
    f_has_lyrics: bool,
    s_track_rating: Option<SortOrder>,
}

impl<'a> TrackSearchQuery<'a> {
    pub fn q_track(mut self, q_track: &'a str) -> Self {
        self.q_track = Some(q_track);
        self
    }

    pub fn q_artist(mut self, q_artist: &'a str) -> Self {
        self.q_artist = Some(q_artist);
        self
    }

    pub fn f_has_lyrics(mut self) -> Self {
        self.f_has_lyrics = true;
        self
    }

    pub fn s_track_rating(mut self, value: SortOrder) -> Self {
        self.s_track_rating = Some(value);
        self
    }

    pub async fn send(&self, page_size: u8, page: u32) -> Result<Vec<Track>, Error> {
        let mut url = self.mxm.new_url("track.search");
        {
            let mut query = url.query_pairs_mut();
            if let Some(q_track) = self.q_track {
                query.append_pair("q_track", q_track);
            }
            if let Some(q_artist) = self.q_artist {
                query.append_pair("q_artist", q_artist);
            }
            if self.f_has_lyrics {
                query.append_pair("f_has_lyrics", "1");
            }
            if let Some(sort) = self.s_track_rating {
                query.append_pair("s_track_rating", sort.as_str());
            }
            query.append_pair("page_size", &page_size.to_string());
            query.append_pair("page", &page.to_string());
            query.finish();
        }

        let body = self.mxm.execute_get_request::<TrackListBody>(&url).await?;
        Ok(body.track_list.into_iter().map(|item| item.track).collect())
    }
}

#[derive(Debug, Deserialize)]
struct Resp<T> {
    message: T,
}

#[derive(Debug, Deserialize)]
struct HeaderMsg {
    header: Header,
}

#[derive(Debug, Deserialize)]
struct BodyMsg<T> {
    body: T,
}

#[derive(Debug, Deserialize)]
struct Header {
    status_code: u16,
    #[serde(default)]
    hint: String,
}

fn parse_body<T: DeserializeOwned>(response: &str) -> Result<T, Error> {
    let header = serde_json::from_str::<Resp<HeaderMsg>>(response)?.message.header;
    if header.status_code < 400 {
        let body = serde_json::from_str::<Resp<BodyMsg<T>>>(response)?;
        Ok(body.message.body)
    } else if header.status_code == 404 {
        Err(Error::NotFound)
    } else if header.status_code == 401 && header.hint == "renew" {
        Err(Error::TokenExpired)
    } else if header.status_code == 401 && header.hint == "captcha" {
        Err(Error::Ratelimit)
    } else {
        Err(Error::Provider {
            status_code: header.status_code,
            msg: header.hint,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GetToken {
    user_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredSession {
    usertoken: String,
}

fn retrieve_session(path: Option<&Path>) -> Option<StoredSession> {
    let path = path?;
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<StoredSession>(&json).ok()
}

fn random_guid() -> String {
    let nanos = current_nanos();
    let counter = RANDOM_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:016x}", nanos ^ counter)
}

fn random_uuid() -> String {
    let a = current_nanos();
    let b = RANDOM_COUNTER.fetch_add(1, Ordering::Relaxed);
    let c = a.rotate_left(13) ^ b.rotate_left(7);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (a & 0xffff_ffff) as u32,
        ((a >> 32) & 0xffff) as u16,
        (b & 0xffff) as u16,
        ((b >> 16) & 0xffff) as u16,
        c & 0xffff_ffff_ffff
    )
}

fn current_nanos() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
}

fn sign_url_with_date(url: &mut Url, date: OffsetDateTime) {
    let mut mac = Hmac::<Sha1>::new_from_slice(SIGNATURE_SECRET).unwrap();
    mac.update(url.as_str().as_bytes());
    mac.update(date.format(YMD_FORMAT).unwrap_or_default().as_bytes());
    let sig = mac.finalize().into_bytes();
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig) + "\n";
    url.query_pairs_mut()
        .append_pair("signature", &sig_b64)
        .append_pair("signature_protocol", "sha1")
        .finish();
}

fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Boolish {
        Bool(bool),
        Number(u64),
    }

    match Boolish::deserialize(deserializer)? {
        Boolish::Bool(value) => Ok(value),
        Boolish::Number(value) => Ok(value != 0),
    }
}

fn null_if_empty<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()))
}

#[derive(Debug, Deserialize)]
struct TrackBody {
    track: Track,
}

#[derive(Debug, Deserialize)]
struct TrackListItem {
    track: Track,
}

#[derive(Debug, Deserialize)]
struct TrackListBody {
    track_list: Vec<TrackListItem>,
}

#[derive(Debug, Deserialize)]
struct LyricsBody {
    lyrics: Lyrics,
}

impl LyricsBody {
    fn validate(&self) -> Result<(), Error> {
        if self.lyrics.lyrics_body.is_empty()
            && self
                .lyrics
                .lyrics_copyright
                .as_deref()
                .map(|text| text.contains("not authorized"))
                .unwrap_or(false)
        {
            Err(Error::NotAvailable)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Deserialize)]
struct SubtitleBody {
    subtitle: Subtitle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_file_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("mxm-session-test-{}.json", current_nanos()));
        let client = Musixmatch::builder()
            .storage_file(&path)
            .build()
            .expect("client should build");
        client.store_session("abc123");
        let session = retrieve_session(Some(&path)).expect("session should exist");
        assert_eq!(session.usertoken, "abc123");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_body_maps_token_expired() {
        let body = r#"{"message":{"header":{"status_code":401,"hint":"renew"}}}"#;
        let error = parse_body::<GetToken>(body).expect_err("should be token expired");
        assert!(matches!(error, Error::TokenExpired));
    }

    #[test]
    fn parse_body_maps_rate_limit() {
        let body = r#"{"message":{"header":{"status_code":401,"hint":"captcha"}}}"#;
        let error = parse_body::<GetToken>(body).expect_err("should be ratelimited");
        assert!(matches!(error, Error::Ratelimit));
    }

    #[test]
    fn lyrics_validate_rejects_not_authorized_payload() {
        let body = LyricsBody {
            lyrics: Lyrics {
                lyrics_id: 1,
                lyrics_body: String::new(),
                lyrics_copyright: Some("Unfortunately we're not authorized to show these lyrics".to_string()),
            },
        };
        let error = body.validate().expect_err("lyrics should be unavailable");
        assert!(matches!(error, Error::NotAvailable));
    }

    #[test]
    fn subtitle_payload_preserves_lrc_text() {
        let payload = r#"{"message":{"header":{"status_code":200,"hint":""},"body":{"subtitle":{"subtitle_id":1,"subtitle_body":"[00:01.23]hello"}}}}"#;
        let parsed = parse_body::<SubtitleBody>(payload).expect("subtitle body should parse");
        assert_eq!(parsed.subtitle.subtitle_body, "[00:01.23]hello");
    }
}
