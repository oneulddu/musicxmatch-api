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
    file: Arc<std::sync::Mutex<Option<std::fs::File>>>,
    path: PathBuf,
}

impl Logger {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }
        rotate_log_if_needed(&path);

        let file = open_log_file(&path);
        Self {
            file: Arc::new(std::sync::Mutex::new(file)),
            path,
        }
    }

    pub fn log_tagged(&self, tag: &str, message: &str) {
        self.write_line(&format!("[{tag}] {message}"));
    }

    fn write_line(&self, message: &str) {
        let line = format!("[{}] {message}\n", timestamp_string());
        print!("{line}");
        if let Ok(mut guard) = self.file.lock() {
            reopen_if_log_too_large(&self.path, &mut guard);
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
            reopen_if_log_too_large(&self.path, &mut guard);
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

fn reopen_if_log_too_large(path: &Path, file: &mut Option<File>) {
    let is_too_large = file
        .as_ref()
        .and_then(|value| value.metadata().ok())
        .map(|metadata| metadata.len() > LOG_MAX_BYTES)
        .unwrap_or_else(|| {
            metadata(path)
                .map(|metadata| metadata.len() > LOG_MAX_BYTES)
                .unwrap_or(false)
        });

    if !is_too_large {
        if file.is_none() {
            *file = open_log_file(path);
        }
        return;
    }

    *file = None;
    rotate_log_if_needed(path);
    *file = open_log_file(path);
}

fn rotate_log_if_needed(path: &Path) {
    let Ok(current) = metadata(path) else {
        return;
    };
    if current.len() <= LOG_MAX_BYTES {
        return;
    }

    let rotated = path.with_extension(match path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => format!("{extension}.1"),
        _ => "1".to_string(),
    });
    let _ = remove_file(&rotated);
    if rename(path, &rotated).is_err() {
        let _ = OpenOptions::new().write(true).truncate(true).open(path);
    }
}
