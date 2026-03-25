use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use serde_json::Value;

const SEARCH_URL: &str = "https://www.genie.co.kr/search/searchMain";
const LYRICS_URL: &str = "https://dn.genie.co.kr/app/purchase/get_msl.asp";

#[derive(Clone)]
pub struct GenieClient {
    http: reqwest::Client,
}

#[derive(Clone, Debug)]
pub struct GenieTrack {
    pub track_id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct GenieLyricsResult {
    pub track_id: u64,
    pub track_name: String,
    pub artist_name: String,
    pub duration_ms: Option<u64>,
    pub lrc: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug)]
pub enum GenieError {
    NotFound,
    NotAvailable,
    Provider(String),
}

impl fmt::Display for GenieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "No matching Genie tracks found."),
            Self::NotAvailable => write!(f, "No Genie lyrics are available for this track."),
            Self::Provider(detail) => write!(f, "Genie request failed: {detail}"),
        }
    }
}

impl GenieClient {
    pub fn new(timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("Mozilla/5.0 (compatible; ivLyrics-MusicXMatch/1.0)")
            .build()
            .expect("failed to construct Genie HTTP client");

        Self { http }
    }

    pub async fn search_tracks(
        &self,
        title: Option<&str>,
        artist: Option<&str>,
    ) -> Result<Vec<GenieTrack>, GenieError> {
        let query = build_search_query(title.unwrap_or(""), artist.unwrap_or(""));
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .http
            .get(SEARCH_URL)
            .query(&[("query", query.as_str())])
            .send()
            .await
            .map_err(|error| GenieError::Provider(error.to_string()))?;

        if !response.status().is_success() {
            return Err(GenieError::Provider(format!(
                "search returned {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|error| GenieError::Provider(error.to_string()))?;

        Ok(parse_search_tracks(&body))
    }

    pub async fn fetch_lyrics_for_track(
        &self,
        track: &GenieTrack,
    ) -> Result<GenieLyricsResult, GenieError> {
        let response = self
            .http
            .get(LYRICS_URL)
            .query(&[
                ("path", "a"),
                ("songid", &track.track_id.to_string()),
            ])
            .header(reqwest::header::REFERER, "https://www.genie.co.kr/")
            .send()
            .await
            .map_err(|error| GenieError::Provider(error.to_string()))?;

        if !response.status().is_success() {
            return Err(GenieError::Provider(format!(
                "lyrics lookup returned {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|error| GenieError::Provider(error.to_string()))?;
        let parsed = match parse_lyrics_payload(&body) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => return Err(GenieError::NotAvailable),
            Err(detail) => return Err(GenieError::Provider(detail)),
        };

        let lrc = format_genie_lrc(&parsed);
        let text = format_plain_text(&parsed);

        if lrc.is_none() && text.is_none() {
            return Err(GenieError::NotAvailable);
        }

        Ok(GenieLyricsResult {
            track_id: track.track_id,
            track_name: track.track_name.clone(),
            artist_name: track.artist_name.clone(),
            duration_ms: track.duration_ms,
            lrc,
            text,
        })
    }
}

fn build_search_query(title: &str, artist: &str) -> String {
    [title.trim(), artist.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_search_tracks(body: &str) -> Vec<GenieTrack> {
    let mut tracks = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for row in body.split("<tr class=\"list\"").skip(1) {
        let row = format!("<tr class=\"list\"{row}");
        let Some(track) = parse_search_track_row(&row) else {
            continue;
        };
        if seen.insert(track.track_id) {
            tracks.push(track);
        }
    }

    tracks
}

fn parse_search_track_row(row: &str) -> Option<GenieTrack> {
    let track_id = capture_between(row, "songid=\"", "\"")?.parse::<u64>().ok()?;
    let info_block = capture_between(row, "<td class=\"info\">", "</td>")?;

    let title = capture_anchor_attr_or_text(info_block, "title ellipsis", "title")
        .filter(|value| !value.is_empty())?;
    let artist = capture_anchor_attr_or_text(info_block, "artist ellipsis", "title")
        .filter(|value| !value.is_empty())?;

    let duration_ms = capture_between(info_block, "<span class=\"duration\">", "</span>")
        .and_then(|value| parse_duration_ms(&cleanup_text(value)));

    Some(GenieTrack {
        track_id,
        track_name: title,
        artist_name: artist,
        duration_ms,
    })
}

fn parse_lyrics_payload(body: &str) -> Result<Option<BTreeMap<u64, String>>, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("NOT FOUND LYRICS") {
        return Ok(None);
    }

    let Some(json) = extract_jsonp_payload(trimmed) else {
        return Err("Genie lyrics payload was not valid JSONP".to_string());
    };
    let value: Value =
        serde_json::from_str(json).map_err(|error| format!("Genie lyrics JSON parse failed: {error}"))?;
    let Some(object) = value.as_object() else {
        return Err("Genie lyrics payload body was not an object".to_string());
    };
    let mut entries = BTreeMap::new();

    for (key, value) in object {
        let Some(timestamp_ms) = key.parse::<u64>().ok() else {
            continue;
        };
        let Some(line) = value.as_str() else {
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        entries.insert(timestamp_ms, html_entity_decode(line));
    }

    if entries.is_empty() {
        Ok(None)
    } else {
        Ok(Some(entries))
    }
}

fn format_genie_lrc(entries: &BTreeMap<u64, String>) -> Option<String> {
    let lines = entries
        .iter()
        .map(|(ms, line)| format!("{}{}", format_lrc_timestamp_ms(*ms), line))
        .collect::<Vec<_>>();

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn format_plain_text(entries: &BTreeMap<u64, String>) -> Option<String> {
    let lines = entries.values().cloned().collect::<Vec<_>>();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn extract_jsonp_payload(body: &str) -> Option<&str> {
    let start = body.find('(')? + 1;
    let end = body.rfind(')')?;
    (start < end).then_some(&body[start..end])
}

fn capture_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_index = haystack.find(start)? + start.len();
    let remainder = &haystack[start_index..];
    let end_index = remainder.find(end)?;
    Some(&remainder[..end_index])
}

fn capture_anchor_attr_or_text(haystack: &str, class_name: &str, attr_name: &str) -> Option<String> {
    let marker = format!("class=\"{class_name}\"");
    let class_index = haystack.find(&marker)?;
    let before = &haystack[..class_index];
    let anchor_start = before.rfind("<a")?;
    let anchor = &haystack[anchor_start..];

    if let Some(value) = capture_between(anchor, &format!("{attr_name}=\""), "\"") {
        let cleaned = cleanup_display_text(value);
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
    }

    capture_between(anchor, ">", "</a>").map(cleanup_display_text)
}

fn strip_tags(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut inside_tag = false;

    for ch in value.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }

    result
}

fn cleanup_text(value: &str) -> String {
    value
        .replace("TITLE", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn cleanup_display_text(value: &str) -> String {
    html_entity_decode(&cleanup_text(&strip_tags(value)))
}

fn parse_duration_ms(value: &str) -> Option<u64> {
    let value = value.trim();
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

fn html_entity_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
        .replace("&#40;", "(")
        .replace("&#41;", ")")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&#44;", ",")
}

fn format_lrc_timestamp_ms(milliseconds: u64) -> String {
    let total_centis = (milliseconds + 5) / 10;
    let minutes = total_centis / 6000;
    let secs = (total_centis / 100) % 60;
    let centis = total_centis % 100;
    format!("[{minutes:02}:{secs:02}.{centis:02}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_genie_jsonp_payload() {
        let payload = r#"null({"1030":"그대여","7010":"그대여"})"#;
        let parsed = parse_lyrics_payload(payload)
            .expect("lyrics should parse")
            .expect("lyrics should exist");
        assert_eq!(parsed.get(&1030).map(String::as_str), Some("그대여"));
        assert_eq!(parsed.get(&7010).map(String::as_str), Some("그대여"));
    }

    #[test]
    fn format_genie_lrc_uses_millisecond_timestamps() {
        let mut entries = BTreeMap::new();
        entries.insert(1030, "그대여".to_string());
        entries.insert(7010, "오늘은".to_string());
        let lrc = format_genie_lrc(&entries).expect("lrc should exist");
        assert!(lrc.contains("[00:01.03]그대여"));
        assert!(lrc.contains("[00:07.01]오늘은"));
    }

    #[test]
    fn parse_search_tracks_extracts_unique_tracks() {
        let body = r##"
        <tr class="list" songid="101374441">
            <td class="info">
                <a href="#" class="title ellipsis"><span class="icon icon-title">TITLE</span><span class="t_point">How We Came</span> (Feat. pH-1)</a>
                <a href="#" class="artist ellipsis">Lil Moshpit &amp; Fleeky Bang</a>
            </td>
        </tr>
        <tr class="list" songid="101374441">
            <td class="info">
                <a href="#" class="title ellipsis">How We Came (Feat. pH-1)</a>
                <a href="#" class="artist ellipsis">Lil Moshpit &amp; Fleeky Bang</a>
            </td>
        </tr>
        "##;
        let tracks = parse_search_tracks(body);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].track_id, 101374441);
        assert_eq!(tracks[0].track_name, "How We Came (Feat. pH-1)");
        assert_eq!(tracks[0].artist_name, "Lil Moshpit & Fleeky Bang");
    }

    #[test]
    fn parse_search_tracks_prefers_title_attribute_and_duration() {
        let body = r##"
        <tr class="list" songid="101374441">
            <td class="info">
                <a href="#" class="title ellipsis" title="How We Came (Feat. pH-1)">
                    <span class="icon icon-title">TITLE</span><span class="t_point"></span>How We Came
                </a>
                <a href="#" class="artist ellipsis">Lil Moshpit &amp; Fleeky Bang</a>
                <span class="duration">2:57</span>
            </td>
        </tr>
        "##;
        let tracks = parse_search_tracks(body);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].track_name, "How We Came (Feat. pH-1)");
        assert_eq!(tracks[0].duration_ms, Some(177000));
    }

    #[test]
    fn parse_lyrics_payload_rejects_invalid_jsonp() {
        let error = parse_lyrics_payload("hello world").expect_err("should fail");
        assert!(error.contains("JSONP"));
    }

    #[test]
    fn parse_search_tracks_fixture_extracts_expected_rows() {
        let body = include_str!("../tests/fixtures/genie/search_result.html");
        let tracks = parse_search_tracks(body);

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].track_id, 101374441);
        assert_eq!(tracks[0].track_name, "How We Came (Feat. pH-1)");
        assert_eq!(tracks[0].artist_name, "Lil Moshpit & Fleeky Bang");
        assert_eq!(tracks[0].duration_ms, Some(177000));
        assert_eq!(tracks[1].track_name, "끊었어? (demo)");
        assert_eq!(tracks[1].artist_name, "Chan (찬), 기원 (Giwon)");
    }

    #[test]
    fn parse_lyrics_payload_fixture_extracts_timestamped_lines() {
        let payload = include_str!("../tests/fixtures/genie/lyrics_payload.jsonp");
        let parsed = parse_lyrics_payload(payload)
            .expect("fixture should parse")
            .expect("fixture should contain lyrics");

        assert_eq!(parsed.get(&1030).map(String::as_str), Some("그대여"));
        assert_eq!(parsed.get(&7010).map(String::as_str), Some("오늘은"));
        assert_eq!(parsed.get(&24210).map(String::as_str), Some("벚꽃엔딩"));
    }
}
