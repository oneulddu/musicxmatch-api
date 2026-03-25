use std::fmt;
use std::time::Duration;

use serde::Deserialize;

const SEARCH_URL: &str = "https://m.bugs.co.kr/api/getSearchList";
const SYNCED_LYRICS_URL: &str = "https://music.bugs.co.kr/player/lyrics/T";
const PLAIN_LYRICS_URL: &str = "https://music.bugs.co.kr/player/lyrics/N";

#[derive(Clone)]
pub struct BugsClient {
    http: reqwest::Client,
}

#[derive(Clone, Debug)]
pub struct BugsTrack {
    pub track_id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct BugsLyricsResult {
    pub track_id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub duration_ms: Option<u64>,
    pub lrc: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug)]
pub enum BugsError {
    NotFound,
    NotAvailable,
    Provider(String),
}

impl fmt::Display for BugsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "No matching Bugs tracks found."),
            Self::NotAvailable => write!(f, "No Bugs lyrics are available for this track."),
            Self::Provider(detail) => write!(f, "Bugs request failed: {detail}"),
        }
    }
}

impl BugsClient {
    pub fn new(timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("Mozilla/5.0 (compatible; ivLyrics-MusicXMatch/1.0)")
            .build()
            .expect("failed to construct Bugs HTTP client");

        Self { http }
    }

    pub async fn search_tracks(
        &self,
        title: Option<&str>,
        artist: Option<&str>,
    ) -> Result<Vec<BugsTrack>, BugsError> {
        let query = build_search_query(title.unwrap_or(""), artist.unwrap_or(""));
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .http
            .get(SEARCH_URL)
            .query(&[
                ("type", "track"),
                ("query", query.as_str()),
                ("page", "1"),
                ("size", "30"),
            ])
            .send()
            .await
            .map_err(|error| BugsError::Provider(error.to_string()))?;

        if !response.status().is_success() {
            return Err(BugsError::Provider(format!(
                "search returned {}",
                response.status()
            )));
        }

        let payload = response
            .json::<BugsSearchResponse>()
            .await
            .map_err(|error| BugsError::Provider(error.to_string()))?;

        Ok(payload
            .list
            .into_iter()
            .map(|track| BugsTrack {
                track_id: track.track_id,
                track_name: track.track_title,
                artist_name: join_artist_names(&track.artists),
                duration_ms: parse_duration_ms(track.len.as_deref()),
            })
            .collect())
    }

    pub async fn fetch_lyrics_for_track(
        &self,
        track: &BugsTrack,
    ) -> Result<BugsLyricsResult, BugsError> {
        let lrc = self
            .fetch_lyrics_body(track.track_id, true)
            .await?
            .as_deref()
            .and_then(parse_synced_lyrics);

        let text = self
            .fetch_lyrics_body(track.track_id, false)
            .await?
            .as_deref()
            .map(normalize_plain_lyrics)
            .filter(|value| !value.is_empty());

        if lrc.is_none() && text.is_none() {
            return Err(BugsError::NotAvailable);
        }

        Ok(BugsLyricsResult {
            track_id: track.track_id,
            track_name: track.track_name.clone(),
            artist_name: track.artist_name.clone(),
            duration_ms: track.duration_ms,
            lrc,
            text,
        })
    }

    async fn fetch_lyrics_body(
        &self,
        track_id: u64,
        synced: bool,
    ) -> Result<Option<String>, BugsError> {
        let mode_url = if synced {
            SYNCED_LYRICS_URL
        } else {
            PLAIN_LYRICS_URL
        };
        let response = self
            .http
            .get(format!("{mode_url}/{track_id}"))
            .send()
            .await
            .map_err(|error| BugsError::Provider(error.to_string()))?;

        if !response.status().is_success() {
            return Err(BugsError::Provider(format!(
                "lyrics lookup returned {}",
                response.status()
            )));
        }

        let payload = response
            .json::<BugsLyricsResponse>()
            .await
            .map_err(|error| BugsError::Provider(error.to_string()))?;

        Ok(payload
            .lyrics
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()))
    }
}

fn build_search_query(title: &str, artist: &str) -> String {
    [title.trim(), artist.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn join_artist_names(artists: &[BugsArtist]) -> String {
    artists
        .iter()
        .map(|artist| artist.artist_nm.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_duration_ms(value: Option<&str>) -> Option<u64> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let parts = value
        .split(':')
        .map(|part| part.trim().parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;

    let total_seconds = match parts.as_slice() {
        [minutes, seconds] => minutes.saturating_mul(60).saturating_add(*seconds),
        [hours, minutes, seconds] => hours
            .saturating_mul(3600)
            .saturating_add(minutes.saturating_mul(60))
            .saturating_add(*seconds),
        _ => return None,
    };

    Some(total_seconds.saturating_mul(1000))
}

fn parse_synced_lyrics(value: &str) -> Option<String> {
    let mut lines = Vec::new();

    for entry in value.split('＃') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((seconds, text)) = trimmed.split_once('|') else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        let seconds = match seconds.trim().parse::<f64>() {
            Ok(value) if value >= 0.0 => value,
            _ => continue,
        };

        lines.push(format!("{}{}", format_lrc_timestamp(seconds), text));
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn format_lrc_timestamp(seconds: f64) -> String {
    let total_centis = (seconds.max(0.0) * 100.0).round() as u64;
    let minutes = total_centis / 6000;
    let secs = (total_centis / 100) % 60;
    let centis = total_centis % 100;
    format!("[{minutes:02}:{secs:02}.{centis:02}]")
}

fn normalize_plain_lyrics(value: &str) -> String {
    value
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[derive(Deserialize)]
struct BugsSearchResponse {
    #[serde(default)]
    list: Vec<BugsSearchTrack>,
}

#[derive(Deserialize)]
struct BugsSearchTrack {
    track_id: u64,
    track_title: String,
    #[serde(default)]
    artists: Vec<BugsArtist>,
    len: Option<String>,
}

#[derive(Deserialize)]
struct BugsArtist {
    artist_nm: String,
}

#[derive(Deserialize)]
struct BugsLyricsResponse {
    lyrics: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::from_str;

    #[test]
    fn parse_duration_supports_mm_ss() {
        assert_eq!(parse_duration_ms(Some("02:53")), Some(173000));
        assert_eq!(parse_duration_ms(Some("1:02:03")), Some(3_723_000));
        assert_eq!(parse_duration_ms(Some("")), None);
    }

    #[test]
    fn parse_synced_bugs_lyrics_into_lrc() {
        let parsed = parse_synced_lyrics("7.3|Uh uh＃8.3|나는 가 Straight").unwrap();
        assert_eq!(parsed, "[00:07.30]Uh uh\n[00:08.30]나는 가 Straight");
    }

    #[test]
    fn normalize_plain_lyrics_collapses_crlf() {
        let parsed = normalize_plain_lyrics("A\r\nB\r\n");
        assert_eq!(parsed, "A\nB");
    }

    #[test]
    fn parse_search_response_fixture_extracts_tracks() {
        let body = include_str!("../tests/fixtures/bugs/search_response.json");
        let payload: BugsSearchResponse = from_str(body).expect("fixture json should parse");

        assert_eq!(payload.list.len(), 2);
        assert_eq!(payload.list[0].track_id, 6196642);
        assert_eq!(payload.list[0].track_title, "How We Came (Feat. pH-1)");
        assert_eq!(join_artist_names(&payload.list[0].artists), "Lil Moshpit, Fleeky Bang");
        assert_eq!(parse_duration_ms(payload.list[0].len.as_deref()), Some(177000));
    }

    #[test]
    fn parse_synced_lyrics_fixture_into_lrc() {
        let payload = include_str!("../tests/fixtures/bugs/synced_lyrics.txt");
        let parsed = parse_synced_lyrics(payload).expect("fixture lyrics should parse");

        assert!(parsed.contains("[00:06.90]존나 멋 존나 멋 그건 누구겠어 뱃"));
        assert!(parsed.contains("[00:11.10]마이크로폰 첵 원투 난 일차원으로 해"));
        assert!(parsed.contains("[00:14.72]나는 꽤 멋쟁이야"));
    }
}
