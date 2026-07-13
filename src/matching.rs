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
        Some(track.track_length as f32),
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

pub trait SearchableTrack {
    fn id(&self) -> u64;
    fn name(&self) -> &str;
    fn artist(&self) -> &str;
    fn duration_ms(&self) -> Option<u64>;
}

impl SearchableTrack for DeezerTrack {
    fn id(&self) -> u64 {
        self.track_id
    }

    fn name(&self) -> &str {
        &self.track_name
    }

    fn artist(&self) -> &str {
        &self.artist_name
    }

    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}

impl SearchableTrack for BugsTrack {
    fn id(&self) -> u64 {
        self.track_id
    }

    fn name(&self) -> &str {
        &self.track_name
    }

    fn artist(&self) -> &str {
        &self.artist_name
    }

    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}

impl SearchableTrack for GenieTrack {
    fn id(&self) -> u64 {
        self.track_id
    }

    fn name(&self) -> &str {
        &self.track_name
    }

    fn artist(&self) -> &str {
        &self.artist_name
    }

    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}

pub fn score_provider_track<T: SearchableTrack>(
    track: &T,
    title: &str,
    artist: &str,
    duration_secs: Option<f32>,
) -> f32 {
    score_basic_match(
        title,
        artist,
        track.name(),
        track.artist(),
        duration_secs,
        track
            .duration_ms()
            .map(|actual_ms| actual_ms as f32 / 1000.0),
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
    // Keep bracketed/suffixed words (e.g. "(Retro Remix)") visible for the
    // noise check below; `simplify` would strip them.
    let want_noise_source = format!("{} {}", normalize(title), want_artist);
    let track_noise_source = format!("{} {}", normalize(track_title), normalize(track_artist));
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

    for word in [
        "acoustic",
        "cover",
        "instrumental",
        "karaoke",
        "live",
        "remix",
        "tribute",
    ] {
        // Penalize a mismatch in either direction: a remix when the plain
        // song was requested, or the plain song when a remix was requested.
        if contains_word(&track_noise_source, word) != contains_word(&want_noise_source, word) {
            score -= NOISE_PENALTY;
        }
    }

    score
}

fn contains_word(value: &str, expected: &str) -> bool {
    value.split_whitespace().any(|word| word == expected)
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

/// Match confidence, ordered weakest first so `Ord` reflects strength.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderMatchTier {
    /// Only an exact playtime backs the candidate; weakest evidence.
    LooseDuration,
    /// The title agrees but the artist names share no script, so the
    /// mismatch is inconclusive (e.g. "BTS" vs "방탄소년단").
    LooseIncomparableArtist,
    /// Only the artist agrees (e.g. another song by the same artist).
    LooseArtist,
    /// Title and artist both agree with the request.
    Strict,
}

pub fn provider_match_tier(
    track_name: &str,
    artist_name: &str,
    title: &str,
    artist: &str,
    matched_by: &str,
    duration_pair: Option<(f32, f32)>,
) -> Option<ProviderMatchTier> {
    let want_title = simplify(title);
    let want_artist = normalize(artist);
    let track_title = simplify(track_name);
    let track_artist = normalize(artist_name);

    let title_similarity = similarity(&want_title, &track_title);
    let artist_similarity = if want_artist.is_empty() {
        1.0
    } else {
        similarity(&want_artist, &track_artist)
    };
    let artist_contains = !want_artist.is_empty()
        && (track_artist.contains(&want_artist) || want_artist.contains(&track_artist));
    let artist_agrees = want_artist.is_empty() || artist_similarity >= 0.35 || artist_contains;
    let artist_comparable = artists_comparable(&want_artist, &track_artist);
    let exact_duration = duration_pair
        .map(|(want, actual)| exact_duration_match(Some(want), Some(actual)))
        .unwrap_or(false);

    match matched_by {
        "search:title" => {
            if title_similarity < 0.8 {
                None
            } else if want_artist.is_empty() || artist_similarity >= 0.35 || artist_contains {
                Some(ProviderMatchTier::Strict)
            } else if !artist_comparable {
                Some(ProviderMatchTier::LooseIncomparableArtist)
            } else if exact_duration {
                Some(ProviderMatchTier::LooseDuration)
            } else {
                None
            }
        }
        _ => {
            if title_similarity >= 0.45 && artist_agrees {
                Some(ProviderMatchTier::Strict)
            } else if artist_similarity >= 0.45 {
                Some(ProviderMatchTier::LooseArtist)
            } else if title_similarity >= 0.45 && !artist_comparable {
                Some(ProviderMatchTier::LooseIncomparableArtist)
            } else if title_similarity >= 0.8 && exact_duration {
                Some(ProviderMatchTier::LooseDuration)
            } else {
                None
            }
        }
    }
}

/// Returns true when the two normalized artist names share at least one
/// script family, meaning a low character similarity is evidence of an
/// actual mismatch rather than a transliteration difference.
fn artists_comparable(a: &str, b: &str) -> bool {
    const LATIN: u8 = 1 << 0;
    const HANGUL: u8 = 1 << 1;
    const HAN: u8 = 1 << 2;
    const HIRAGANA: u8 = 1 << 3;
    const KATAKANA: u8 = 1 << 4;
    const OTHER: u8 = 1 << 5;

    fn script_families(value: &str) -> u8 {
        let mut mask = 0u8;
        // Digits are script-neutral: "9와 숫자들" and "9 and the Numbers"
        // share only the digit, which proves nothing about the script.
        for ch in value
            .chars()
            .filter(|ch| ch.is_alphanumeric() && !ch.is_ascii_digit())
        {
            mask |= match ch {
                ch if ch.is_ascii() => LATIN,
                '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' => {
                    HANGUL
                }
                '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}' => HAN,
                '\u{3040}'..='\u{309F}' => HIRAGANA,
                '\u{30A0}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' => KATAKANA,
                _ => OTHER,
            };
        }
        mask
    }

    let a_mask = script_families(a);
    let b_mask = script_families(b);
    // Without script evidence on one side (digits-only or empty names),
    // never excuse a similarity mismatch as a transliteration difference.
    if a_mask == 0 || b_mask == 0 {
        return true;
    }
    a_mask & b_mask != 0
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
    for marker in [" feat. ", " feat ", " featuring ", " ft. ", " ft "] {
        if let Some(index) = find_ascii_marker(value, marker) {
            return value[..index].trim().to_string();
        }
    }
    value.trim().to_string()
}

fn find_ascii_marker(value: &str, marker: &str) -> Option<usize> {
    value.char_indices().find_map(|(index, _)| {
        let end = index + marker.len();
        value
            .get(index..end)
            .filter(|candidate| candidate.eq_ignore_ascii_case(marker))
            .map(|_| index)
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_featured_handles_multibyte_case_expansion() {
        assert_eq!(strip_featured("İstanbul feat. Someone"), "İstanbul");
    }

    #[test]
    fn noise_penalty_does_not_match_substrings() {
        let track = Track {
            track_id: 1,
            track_name: "Alive".to_string(),
            track_length: 180,
            artist_name: "Artist".to_string(),
            has_lyrics: false,
            has_subtitles: false,
            has_richsync: false,
        };

        assert_eq!(score_track(&track, "Alive", "Artist", None), 125.0);
    }

    #[test]
    fn noise_penalty_applies_to_bracketed_remix_titles() {
        let original = score_basic_match("Dynamite", "BTS", "Dynamite", "방탄소년단", None, None);
        let remix = score_basic_match(
            "Dynamite",
            "BTS",
            "Dynamite (Retro Remix)",
            "방탄소년단",
            None,
            None,
        );
        assert!(original > remix, "original={original} remix={remix}");
    }

    #[test]
    fn noise_penalty_skipped_when_request_also_mentions_remix() {
        let remix_request = score_basic_match(
            "Dynamite (Retro Remix)",
            "BTS",
            "Dynamite (Retro Remix)",
            "방탄소년단",
            None,
            None,
        );
        let plain_request =
            score_basic_match("Dynamite", "BTS", "Dynamite", "방탄소년단", None, None);
        assert_eq!(remix_request, plain_request);
    }

    #[test]
    fn requested_remix_outranks_plain_version() {
        let remix_track = score_basic_match(
            "Dynamite (Retro Remix)",
            "BTS",
            "Dynamite (Retro Remix)",
            "방탄소년단",
            None,
            None,
        );
        let plain_track = score_basic_match(
            "Dynamite (Retro Remix)",
            "BTS",
            "Dynamite",
            "방탄소년단",
            None,
            None,
        );
        assert!(
            remix_track > plain_track,
            "remix={remix_track} plain={plain_track}"
        );
    }

    #[test]
    fn provider_match_tier_rejects_same_title_different_artist() {
        // Regression: Genie returned Chipmunk's lyrics for KIRBS' request
        // because a title-only match was accepted on the title+artist path.
        assert_eq!(
            provider_match_tier(
                "Follow My Lead (Feat. Eric B.)",
                "Chipmunk",
                "Follow My Lead",
                "KIRBS",
                "search:title+artist",
                None,
            ),
            None
        );
    }

    #[test]
    fn provider_match_tier_accepts_matching_title_and_artist_as_strict() {
        assert_eq!(
            provider_match_tier(
                "follow my lead",
                "Kirbs",
                "Follow My Lead",
                "KIRBS",
                "search:title+artist",
                None,
            ),
            Some(ProviderMatchTier::Strict)
        );
        assert_eq!(
            provider_match_tier(
                "밤편지",
                "아이유 (IU)",
                "밤편지",
                "아이유",
                "search:title+artist",
                None,
            ),
            Some(ProviderMatchTier::Strict)
        );
    }

    #[test]
    fn provider_match_tier_keeps_artist_only_match_as_loose() {
        assert_eq!(
            provider_match_tier(
                "Again",
                "Kirbs",
                "Follow My Lead",
                "KIRBS",
                "search:title+artist",
                None,
            ),
            Some(ProviderMatchTier::LooseArtist)
        );
    }

    #[test]
    fn provider_match_tier_allows_cross_script_artist_as_loose() {
        // Genie often lists K-pop artists only in Korean; "BTS" and
        // "방탄소년단" share no script, so the mismatch is not conclusive.
        assert_eq!(
            provider_match_tier(
                "Dynamite",
                "방탄소년단",
                "Dynamite",
                "BTS",
                "search:title+artist",
                None,
            ),
            Some(ProviderMatchTier::LooseIncomparableArtist)
        );
        // Same-script artists that do not match stay rejected.
        assert_eq!(
            provider_match_tier(
                "Dynamite",
                "Sigrid",
                "Dynamite",
                "BTS",
                "search:title+artist",
                None,
            ),
            None
        );
    }

    #[test]
    fn provider_match_tier_keeps_exact_duration_rescue_loose() {
        // A duration coincidence alone must not allow walking through
        // multiple wrong-artist candidates, so it stays Loose.
        assert_eq!(
            provider_match_tier(
                "Follow My Lead",
                "Someone Else",
                "Follow My Lead",
                "KIRBS",
                "search:title+artist",
                Some((201.0, 200.0)),
            ),
            Some(ProviderMatchTier::LooseDuration)
        );
    }

    #[test]
    fn provider_match_tier_treats_japanese_transliterations_as_loose() {
        // Same artist written in kanji+hiragana vs katakana shares no
        // script family, so the mismatch is not conclusive.
        assert_eq!(
            provider_match_tier(
                "Dearest",
                "浜崎あゆみ",
                "Dearest",
                "ハマサキアユミ",
                "search:title+artist",
                None,
            ),
            Some(ProviderMatchTier::LooseIncomparableArtist)
        );
        // Two different artists in the same script stay rejected.
        assert_eq!(
            provider_match_tier(
                "Dearest",
                "アイミョン",
                "Dearest",
                "ハマサキアユミ",
                "search:title+artist",
                None,
            ),
            None
        );
    }

    #[test]
    fn provider_match_tier_ignores_digits_when_comparing_scripts() {
        // "9와 숫자들" vs "9 and the Numbers": the shared digit must not make
        // the two names count as the same script.
        assert_eq!(
            provider_match_tier(
                "Number Nine",
                "9 and the Numbers",
                "Number Nine",
                "9와 숫자들",
                "search:title+artist",
                None,
            ),
            Some(ProviderMatchTier::LooseIncomparableArtist)
        );
        // Digits-only artist names carry no script evidence, so a mismatch
        // stays a mismatch.
        assert_eq!(
            provider_match_tier(
                "Follow My Lead",
                "81",
                "Follow My Lead",
                "KIRBS",
                "search:title+artist",
                None,
            ),
            None
        );
    }

    #[test]
    fn provider_match_tier_orders_confidence() {
        assert!(ProviderMatchTier::Strict > ProviderMatchTier::LooseArtist);
        assert!(ProviderMatchTier::LooseArtist > ProviderMatchTier::LooseIncomparableArtist);
        assert!(ProviderMatchTier::LooseIncomparableArtist > ProviderMatchTier::LooseDuration);
    }

    #[test]
    fn provider_match_tier_title_only_search_stays_strict() {
        assert_eq!(
            provider_match_tier(
                "Follow My Lead",
                "Chipmunk",
                "Follow My Lead",
                "KIRBS",
                "search:title",
                None,
            ),
            None
        );
        assert_eq!(
            provider_match_tier(
                "Follow My Lead",
                "Kirbs",
                "Follow My Lead",
                "KIRBS",
                "search:title",
                None,
            ),
            Some(ProviderMatchTier::Strict)
        );
    }
}
