use crate::bugs::BugsTrack;
use crate::deezer::DeezerTrack;
use crate::genie::GenieTrack;
use crate::musixmatch::Track;

const TITLE_SIMILARITY_WEIGHT: f32 = 70.0;
const ARTIST_SIMILARITY_WEIGHT: f32 = 30.0;
const EXACT_TITLE_BONUS: f32 = 15.0;
const PARTIAL_TITLE_BONUS: f32 = 8.0;
const ARTIST_MATCH_BONUS: f32 = 10.0;
const SUBTITLE_BONUS: f32 = 8.0;
const RICHSYNC_BONUS: f32 = 4.0;
const LYRICS_BONUS: f32 = 2.0;
const NOISE_PENALTY: f32 = 18.0;
const VERY_CLOSE_DURATION_BONUS: f32 = 18.0;
const CLOSE_DURATION_BONUS: f32 = 10.0;
const SMALL_DURATION_BONUS: f32 = 4.0;
const MEDIUM_DURATION_PENALTY: f32 = -8.0;
const LARGE_DURATION_PENALTY: f32 = -20.0;
const EXACT_DURATION_MATCH_SECS: f32 = 2.5;

pub fn title_variants(title: &str) -> Vec<String> {
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

pub fn artist_variants(artist: &str) -> Vec<String> {
    let mut values = Vec::new();
    let base = artist.trim();
    push_variant(&mut values, base);
    push_variant(&mut values, &first_artist(base));
    push_variant(&mut values, &strip_featured(base));
    push_variant(&mut values, &collapse_to_words(base));
    push_variant(&mut values, &normalize_connectors(base));
    values
}

pub fn normalize(value: &str) -> String {
    collapse_to_words(value).to_lowercase()
}

pub fn exact_title_artist_match(
    track_title: &str,
    track_artist: &str,
    title: &str,
    artist: &str,
) -> bool {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let actual_title = simplify(track_title);
    let actual_artist = normalize(track_artist);

    want_title == actual_title
        && (want_artist == actual_artist
            || actual_artist.contains(&want_artist)
            || want_artist.contains(&actual_artist))
}

pub fn simplify(value: &str) -> String {
    let no_brackets = strip_brackets(value);
    let base = no_brackets
        .split(" - ")
        .next()
        .unwrap_or(no_brackets.as_str())
        .trim()
        .to_string();
    normalize(&base)
}

pub fn similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let len = a.chars().count().max(b.chars().count()) as f32;
    let matches = a
        .chars()
        .zip(b.chars())
        .filter(|(left, right)| left == right)
        .count() as f32;
    matches / len
}

pub fn score_track(track: &Track, title: &str, artist: &str, duration_secs: Option<f32>) -> f32 {
    let mut score = score_basic_match(
        title,
        artist,
        &track.track_name,
        &track.artist_name,
        duration_secs,
        Some(track.track_length as f32 / 1000.0),
    );

    if track.has_subtitles {
        score += SUBTITLE_BONUS;
    }
    if track.has_richsync {
        score += RICHSYNC_BONUS;
    }
    if track.has_lyrics {
        score += LYRICS_BONUS;
    }

    score
}

pub fn score_deezer_track(
    track: &DeezerTrack,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> f32 {
    score_basic_match(
        title,
        artist,
        &track.track_name,
        &track.artist_name,
        duration_secs,
        track.duration_ms.map(|actual_ms| actual_ms as f32 / 1000.0),
    )
}

pub fn score_bugs_track(
    track: &BugsTrack,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> f32 {
    score_basic_match(
        title,
        artist,
        &track.track_name,
        &track.artist_name,
        duration_secs,
        track.duration_ms.map(|actual_ms| actual_ms as f32 / 1000.0),
    )
}

pub fn score_genie_track(
    track: &GenieTrack,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> f32 {
    score_basic_match(
        title,
        artist,
        &track.track_name,
        &track.artist_name,
        duration_secs,
        track.duration_ms.map(|actual_ms| actual_ms as f32 / 1000.0),
    )
}

fn score_basic_match(
    title: &str,
    artist: &str,
    track_title: &str,
    track_artist: &str,
    duration_secs: Option<f32>,
    actual_duration_secs: Option<f32>,
) -> f32 {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(track_title);
    let track_artist = normalize(track_artist);

    let mut score = similarity(&want_title, &track_title) * TITLE_SIMILARITY_WEIGHT
        + similarity(&want_artist, &track_artist) * ARTIST_SIMILARITY_WEIGHT;

    if want_title == track_title {
        score += EXACT_TITLE_BONUS;
    } else if track_title.contains(&want_title) {
        score += PARTIAL_TITLE_BONUS;
    }

    if want_artist == track_artist || track_artist.contains(&want_artist) {
        score += ARTIST_MATCH_BONUS;
    }

    if let (Some(want_duration), Some(actual_duration)) = (duration_secs, actual_duration_secs) {
        score += duration_score((actual_duration - want_duration).abs());
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
            score -= NOISE_PENALTY;
        }
    }

    score
}

pub fn duration_score(delta_secs: f32) -> f32 {
    if delta_secs <= 1.5 {
        VERY_CLOSE_DURATION_BONUS
    } else if delta_secs <= 3.0 {
        CLOSE_DURATION_BONUS
    } else if delta_secs <= 6.0 {
        SMALL_DURATION_BONUS
    } else if delta_secs >= 20.0 {
        LARGE_DURATION_PENALTY
    } else if delta_secs >= 10.0 {
        MEDIUM_DURATION_PENALTY
    } else {
        0.0
    }
}

pub fn exact_duration_match(duration_secs: Option<f32>, actual_duration_secs: Option<f32>) -> bool {
    matches!(
        (duration_secs, actual_duration_secs),
        (Some(want), Some(actual)) if (actual - want).abs() <= EXACT_DURATION_MATCH_SECS
    )
}

pub fn can_use_title_only_fallback(title: &str) -> bool {
    let simplified = simplify(title);
    let compact_len = simplified.chars().filter(|ch| !ch.is_whitespace()).count();
    let has_non_ascii = !simplified.is_ascii();
    let word_count = simplified.split_whitespace().count();
    compact_len >= 4 || word_count >= 2 || (has_non_ascii && compact_len >= 3)
}

pub fn is_acceptable_match(track: &Track, title: &str, artist: &str, matched_by: &str) -> bool {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(&track.track_name);
    let track_artist = normalize(&track.artist_name);

    let title_similarity = similarity(&want_title, &track_title);
    let artist_similarity = if want_artist.is_empty() {
        1.0
    } else {
        similarity(&want_artist, &track_artist)
    };
    let artist_contains = !want_artist.is_empty()
        && (track_artist.contains(&want_artist) || want_artist.contains(&track_artist));

    match matched_by {
        "search:title" => {
            title_similarity >= 0.8
                && (want_artist.is_empty() || artist_similarity >= 0.35 || artist_contains)
        }
        "search:artist" => artist_similarity >= 0.5 && title_similarity >= 0.3,
        "matcher:original" | "matcher:variants" => {
            title_similarity >= 0.45 || artist_similarity >= 0.45
        }
        _ => title_similarity >= 0.45 || artist_similarity >= 0.45,
    }
}

pub fn is_acceptable_deezer_match(
    track: &DeezerTrack,
    title: &str,
    artist: &str,
    matched_by: &str,
) -> bool {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(&track.track_name);
    let track_artist = normalize(&track.artist_name);

    let title_similarity = similarity(&want_title, &track_title);
    let artist_similarity = if want_artist.is_empty() {
        1.0
    } else {
        similarity(&want_artist, &track_artist)
    };
    let artist_contains = !want_artist.is_empty()
        && (track_artist.contains(&want_artist) || want_artist.contains(&track_artist));

    match matched_by {
        "search:title" => {
            title_similarity >= 0.8
                && (want_artist.is_empty() || artist_similarity >= 0.35 || artist_contains)
        }
        _ => title_similarity >= 0.45 || artist_similarity >= 0.45,
    }
}

pub fn is_acceptable_bugs_match(
    track: &BugsTrack,
    title: &str,
    artist: &str,
    matched_by: &str,
    duration_secs: Option<f32>,
) -> bool {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(&track.track_name);
    let track_artist = normalize(&track.artist_name);

    let title_similarity = similarity(&want_title, &track_title);
    let artist_similarity = if want_artist.is_empty() {
        1.0
    } else {
        similarity(&want_artist, &track_artist)
    };
    let artist_contains = !want_artist.is_empty()
        && (track_artist.contains(&want_artist) || want_artist.contains(&track_artist));
    let exact_duration = exact_duration_match(
        duration_secs,
        track.duration_ms.map(|actual_ms| actual_ms as f32 / 1000.0),
    );

    match matched_by {
        "search:title" => {
            title_similarity >= 0.8
                && (
                    want_artist.is_empty()
                        || artist_similarity >= 0.35
                        || artist_contains
                        || exact_duration
                )
        }
        _ => title_similarity >= 0.45 || artist_similarity >= 0.45 || (title_similarity >= 0.8 && exact_duration),
    }
}

pub fn is_acceptable_genie_match(
    track: &GenieTrack,
    title: &str,
    artist: &str,
    matched_by: &str,
    duration_secs: Option<f32>,
) -> bool {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(&track.track_name);
    let track_artist = normalize(&track.artist_name);

    let title_similarity = similarity(&want_title, &track_title);
    let artist_similarity = if want_artist.is_empty() {
        1.0
    } else {
        similarity(&want_artist, &track_artist)
    };
    let artist_contains = !want_artist.is_empty()
        && (track_artist.contains(&want_artist) || want_artist.contains(&track_artist));
    let exact_duration = exact_duration_match(
        duration_secs,
        track.duration_ms.map(|actual_ms| actual_ms as f32 / 1000.0),
    );

    match matched_by {
        "search:title" => {
            title_similarity >= 0.8
                && (
                    want_artist.is_empty()
                        || artist_similarity >= 0.35
                        || artist_contains
                        || exact_duration
                )
        }
        _ => title_similarity >= 0.45 || artist_similarity >= 0.45 || (title_similarity >= 0.8 && exact_duration),
    }
}

pub fn strip_lyrics_footer(value: &str) -> String {
    value
        .split("\n\n*******")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
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

pub fn collapse_to_words(value: &str) -> String {
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

pub fn normalize_connectors(value: &str) -> String {
    value
        .replace('&', " and ")
        .replace('×', " x ")
        .replace('/', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
