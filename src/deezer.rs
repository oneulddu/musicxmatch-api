use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

const AUTH_URL: &str = "https://auth.deezer.com/login/arl";
const SEARCH_URL: &str = "https://api.deezer.com/search";
const GRAPHQL_URL: &str = "https://pipe.deezer.com/api";

const LYRICS_QUERY: &str = r#"query GetLyrics($trackId: String!) {
  track(trackId: $trackId) {
    id
    lyrics {
      id
      text
      synchronizedWordByWordLines {
        start
        end
        words {
          start
          end
          word
        }
      }
      synchronizedLines {
        lrcTimestamp
        line
        lineTranslated
        milliseconds
        duration
      }
      licence
      copyright
      writers
    }
  }
}"#;

#[derive(Clone)]
pub struct DeezerClient {
    http: reqwest::Client,
    jwt: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Debug)]
pub struct DeezerTrack {
    pub track_id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DeezerLyricsResult {
    pub track_id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub duration_ms: Option<u64>,
    pub lrc: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug)]
pub enum DeezerError {
    ConfigMissing,
    NotFound,
    NotAvailable,
    Auth(String),
    Provider(String),
}

impl fmt::Display for DeezerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigMissing => write!(f, "Deezer ARL cookie is not configured."),
            Self::NotFound => write!(f, "No matching Deezer tracks found."),
            Self::NotAvailable => write!(f, "No Deezer lyrics are available for this track."),
            Self::Auth(detail) => write!(f, "Deezer authentication failed: {detail}"),
            Self::Provider(detail) => write!(f, "Deezer request failed: {detail}"),
        }
    }
}

impl DeezerClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to construct Deezer HTTP client");

        Self {
            http,
            jwt: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn clear_token(&self) {
        *self.jwt.lock().await = None;
    }

    pub async fn search_tracks(
        &self,
        title: Option<&str>,
        artist: Option<&str>,
    ) -> Result<Vec<DeezerTrack>, DeezerError> {
        let title = title.unwrap_or("").trim();
        let artist = artist.unwrap_or("").trim();
        let query = build_search_query(title, artist);
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .http
            .get(SEARCH_URL)
            .query(&[("q", query.as_str())])
            .send()
            .await
            .map_err(|error| DeezerError::Provider(error.to_string()))?;

        if !response.status().is_success() {
            return Err(DeezerError::Provider(format!(
                "search returned {}",
                response.status()
            )));
        }

        let payload = response
            .json::<DeezerSearchResponse>()
            .await
            .map_err(|error| DeezerError::Provider(error.to_string()))?;

        Ok(payload
            .data
            .into_iter()
            .map(|track| DeezerTrack {
                track_id: track.id,
                track_name: track.title,
                artist_name: track.artist.name,
                duration_ms: Some(track.duration.saturating_mul(1000)),
            })
            .collect())
    }

    pub async fn fetch_lyrics_for_track(
        &self,
        arl: &str,
        track: &DeezerTrack,
    ) -> Result<DeezerLyricsResult, DeezerError> {
        if arl.trim().is_empty() {
            return Err(DeezerError::ConfigMissing);
        }

        let payload = json!({
            "operationName": "GetLyrics",
            "variables": { "trackId": track.track_id.to_string() },
            "query": LYRICS_QUERY,
        });

        let first_token = self.get_token(arl).await?;
        match self.query_lyrics(&first_token, &payload).await {
            Ok(lyrics) => parse_lyrics(track, lyrics),
            Err(DeezerError::Auth(_)) => {
                let refreshed = self.refresh_token(arl).await?;
                let lyrics = self.query_lyrics(&refreshed, &payload).await?;
                parse_lyrics(track, lyrics)
            }
            Err(error) => Err(error),
        }
    }

    async fn get_token(&self, arl: &str) -> Result<String, DeezerError> {
        if let Some(token) = self.jwt.lock().await.clone() {
            return Ok(token);
        }
        self.refresh_token(arl).await
    }

    async fn refresh_token(&self, arl: &str) -> Result<String, DeezerError> {
        let response = self
            .http
            .post(AUTH_URL)
            .query(&[("jo", "p"), ("rto", "c"), ("i", "c")])
            .header(COOKIE, format!("arl={arl}"))
            .header(CONTENT_LENGTH, "0")
            .body("")
            .send()
            .await
            .map_err(|error| DeezerError::Auth(error.to_string()))?;

        if !response.status().is_success() {
            return Err(DeezerError::Auth(format!(
                "login returned {}",
                response.status()
            )));
        }

        let payload = response
            .json::<DeezerAuthResponse>()
            .await
            .map_err(|error| DeezerError::Auth(error.to_string()))?;

        let jwt = payload
            .jwt
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| DeezerError::Auth("missing JWT token".to_string()))?;

        *self.jwt.lock().await = Some(jwt.clone());
        Ok(jwt)
    }

    async fn query_lyrics(
        &self,
        jwt: &str,
        payload: &serde_json::Value,
    ) -> Result<DeezerLyricsPayload, DeezerError> {
        let response = self
            .http
            .post(GRAPHQL_URL)
            .header(AUTHORIZATION, format!("Bearer {jwt}"))
            .header(CONTENT_TYPE, "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|error| DeezerError::Provider(error.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DeezerError::Auth("token expired".to_string()));
        }

        if !response.status().is_success() {
            return Err(DeezerError::Provider(format!(
                "lyrics query returned {}",
                response.status()
            )));
        }

        let body = response
            .json::<GraphQlResponse>()
            .await
            .map_err(|error| DeezerError::Provider(error.to_string()))?;

        body.data
            .and_then(|data| data.track)
            .and_then(|track| track.lyrics)
            .ok_or(DeezerError::NotFound)
    }
}

fn build_search_query(title: &str, artist: &str) -> String {
    match (title.is_empty(), artist.is_empty()) {
        (false, false) => format!(
            "track:\"{}\" artist:\"{}\"",
            escape_query(title),
            escape_query(artist)
        ),
        (false, true) => format!("track:\"{}\"", escape_query(title)),
        (true, false) => format!("artist:\"{}\"", escape_query(artist)),
        (true, true) => String::new(),
    }
}

fn escape_query(value: &str) -> String {
    value.replace('"', "\\\"")
}

fn parse_lyrics(track: &DeezerTrack, lyrics: DeezerLyricsPayload) -> Result<DeezerLyricsResult, DeezerError> {
    if let Some(lines) = lyrics.synchronized_lines {
        let lrc = build_lrc_from_sync_lines(&lines);
        if !lrc.trim().is_empty() {
            return Ok(DeezerLyricsResult {
                track_id: track.track_id,
                track_name: track.track_name.clone(),
                artist_name: track.artist_name.clone(),
                duration_ms: track.duration_ms,
                lrc: Some(lrc),
                text: None,
            });
        }
    }

    if let Some(lines) = lyrics.synchronized_word_by_word_lines {
        let lrc = build_lrc_from_word_lines(&lines);
        if !lrc.trim().is_empty() {
            return Ok(DeezerLyricsResult {
                track_id: track.track_id,
                track_name: track.track_name.clone(),
                artist_name: track.artist_name.clone(),
                duration_ms: track.duration_ms,
                lrc: Some(lrc),
                text: None,
            });
        }
    }

    let text = lyrics.text.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(DeezerError::NotAvailable);
    }

    Ok(DeezerLyricsResult {
        track_id: track.track_id,
        track_name: track.track_name.clone(),
        artist_name: track.artist_name.clone(),
        duration_ms: track.duration_ms,
        lrc: None,
        text: Some(text),
    })
}

fn build_lrc_from_sync_lines(lines: &[SyncLine]) -> String {
    lines
        .iter()
        .filter_map(|line| {
            let text = line.line.trim();
            if text.is_empty() {
                return None;
            }
            Some(format!("{} {}", format_lrc_timestamp(line.milliseconds), text))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_lrc_from_word_lines(lines: &[WordLine]) -> String {
    lines
        .iter()
        .filter_map(|line| {
            let text = line
                .words
                .iter()
                .map(|word| word.word.trim())
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if text.is_empty() {
                return None;
            }
            Some(format!("{} {}", format_lrc_timestamp(line.start), text))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_lrc_timestamp(milliseconds: u64) -> String {
    let total_seconds = milliseconds / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let hundredths = (milliseconds % 1000) / 10;
    format!("[{minutes:02}:{seconds:02}.{hundredths:02}]")
}

#[derive(Debug, Deserialize)]
struct DeezerAuthResponse {
    jwt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerSearchResponse {
    data: Vec<DeezerSearchTrack>,
}

#[derive(Debug, Deserialize)]
struct DeezerSearchTrack {
    id: u64,
    title: String,
    duration: u64,
    artist: DeezerArtist,
}

#[derive(Debug, Deserialize)]
struct DeezerArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    data: Option<GraphQlData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlData {
    track: Option<GraphQlTrack>,
}

#[derive(Debug, Deserialize)]
struct GraphQlTrack {
    lyrics: Option<DeezerLyricsPayload>,
}

#[derive(Debug, Deserialize)]
struct DeezerLyricsPayload {
    text: Option<String>,
    #[serde(rename = "synchronizedLines")]
    synchronized_lines: Option<Vec<SyncLine>>,
    #[serde(rename = "synchronizedWordByWordLines")]
    synchronized_word_by_word_lines: Option<Vec<WordLine>>,
}

#[derive(Debug, Deserialize)]
struct SyncLine {
    line: String,
    milliseconds: u64,
}

#[derive(Debug, Deserialize)]
struct WordLine {
    start: u64,
    words: Vec<Word>,
}

#[derive(Debug, Deserialize)]
struct Word {
    word: String,
}
