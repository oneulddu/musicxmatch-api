use std::fs::{create_dir_all, metadata, remove_file, rename, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::BackendMode;
use time::format_description::FormatItem;
use time::macros::format_description;

const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

const LOG_TIMESTAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

#[derive(Clone)]
pub struct Logger {
    state: Arc<std::sync::Mutex<LoggerState>>,
    path: PathBuf,
}

struct LoggerState {
    file: Option<File>,
    bytes_written: u64,
}

impl Logger {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }
        rotate_log_if_needed(&path);

        let state = open_log_state(&path);
        Self {
            state: Arc::new(std::sync::Mutex::new(state)),
            path,
        }
    }

    pub fn log_tagged(&self, tag: &str, message: &str) {
        self.write_line(&format!("[{tag}] {message}"));
    }

    fn write_line(&self, message: &str) {
        let line = format!("[{}] {message}\n", timestamp_string());
        print!("{line}");
        if let Ok(mut state) = self.state.lock() {
            if state.file.is_none() {
                *state = open_log_state(&self.path);
            }
            if let Some(file) = state.file.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
                state.bytes_written = state
                    .bytes_written
                    .saturating_add(line.len().try_into().unwrap_or(u64::MAX));
            }
            rotate_log_if_counter_too_large(&self.path, &mut state);
        }
    }
}

fn timestamp_string() -> String {
    let now = time::OffsetDateTime::now_utc()
        .to_offset(time::UtcOffset::from_hms(9, 0, 0).unwrap_or(time::UtcOffset::UTC));
    now.format(LOG_TIMESTAMP_FORMAT)
        .unwrap_or_else(|_| "0000-00-00 00:00:00".to_string())
}

pub fn backend_log_tag(backend: BackendMode) -> &'static str {
    match backend {
        BackendMode::Auto => "Auto",
        BackendMode::Musicxmatch => "MusicXMatch",
        BackendMode::Deezer => "Deezer",
        BackendMode::Bugs => "Bugs",
        BackendMode::Genie => "Genie",
    }
}

pub fn provider_log_tag(provider: &str) -> &'static str {
    match provider {
        "musicxmatch" => "MusicXMatch",
        "deezer" => "Deezer",
        "bugs" => "Bugs",
        "genie" => "Genie",
        _ => "Server",
    }
}

pub fn display_str(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "-"
    } else {
        trimmed
    }
}

pub fn display_opt_text(value: Option<&str>) -> &str {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value,
        None => "-",
    }
}

pub fn display_opt_u64(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub fn bool_text(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

pub fn matched_by_text(value: Option<&'static str>) -> &'static str {
    match value.unwrap_or("-") {
        "track_id" => "트랙 ID 직접 조회",
        "search:title+artist" => "제목+아티스트 검색",
        "search:title" => "제목 검색",
        "search:artist" => "아티스트 검색",
        "matcher:variants" => "변형 매처",
        "matcher:original" => "원본 매처",
        _ => "-",
    }
}

pub fn translate_log_detail(detail: &str) -> String {
    match detail.trim() {
        "No tracks found" => "트랙을 찾지 못함".to_string(),
        "No lyrics are available for this track" => "가사를 찾지 못함".to_string(),
        "Musixmatch session expired. Retry in a moment." => "Musixmatch 세션이 만료됨".to_string(),
        "Configured Deezer ARL cookie is invalid or expired." => {
            "Deezer ARL 설정이 잘못되었거나 만료됨".to_string()
        }
        other if other.starts_with("Invalid Deezer ARL:") => {
            "Deezer ARL 설정 검증 실패".to_string()
        }
        other => other.to_string(),
    }
}

fn open_log_file(path: &Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(path).ok()
}

fn open_log_state(path: &Path) -> LoggerState {
    let file = open_log_file(path);
    let bytes_written = metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
    LoggerState {
        file,
        bytes_written,
    }
}

fn rotate_log_if_counter_too_large(path: &Path, state: &mut LoggerState) {
    if state.bytes_written <= LOG_MAX_BYTES {
        return;
    }

    state.file = None;
    rotate_log_if_needed(path);
    *state = open_log_state(path);
}

fn rotate_log_if_needed(path: &Path) {
    let Ok(current) = metadata(path) else {
        return;
    };
    if current.len() <= LOG_MAX_BYTES {
        return;
    }

    let rotated = rotated_log_path(path);
    let _ = remove_file(&rotated);
    if rename(path, &rotated).is_err() {
        let _ = OpenOptions::new().write(true).truncate(true).open(path);
    }
}

fn rotated_log_path(path: &Path) -> PathBuf {
    path.with_extension(match path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => format!("{extension}.1"),
        _ => "1".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_log_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ivlyrics-musicxmatch-{name}-{}.log",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[test]
    fn logger_rotates_after_byte_counter_exceeds_limit() {
        let path = test_log_path("rotate");
        let rotated = rotated_log_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&rotated);

        let logger = Logger::new(path.clone());
        logger.log_tagged("Test", &"x".repeat(LOG_MAX_BYTES as usize + 1));
        logger.log_tagged("Test", "after rotate");

        let rotated_metadata = fs::metadata(&rotated).expect("rotated log should exist");
        assert!(rotated_metadata.len() > LOG_MAX_BYTES);
        let current = fs::read_to_string(&path).expect("current log should be readable");
        assert!(current.contains("after rotate"));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&rotated);
    }
}
