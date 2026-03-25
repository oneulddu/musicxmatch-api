use crate::bugs::BugsTrack;
use crate::deezer::DeezerTrack;
use musixmatch_inofficial::models::Track;

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

pub fn score_deezer_track(
    track: &DeezerTrack,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> f32 {
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

    if let (Some(want_duration), Some(actual_ms)) = (duration_secs, track.duration_ms) {
        let actual_duration = actual_ms as f32 / 1000.0;
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
            score -= 18.0;
        }
    }

    score
}

pub fn score_bugs_track(
    track: &BugsTrack,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> f32 {
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

    if let (Some(want_duration), Some(actual_ms)) = (duration_secs, track.duration_ms) {
        let actual_duration = actual_ms as f32 / 1000.0;
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
            score -= 18.0;
        }
    }

    score
}

pub fn duration_score(delta_secs: f32) -> f32 {
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
