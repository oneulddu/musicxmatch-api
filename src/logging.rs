use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use crate::BackendMode;

#[derive(Clone)]
pub struct Logger {
    file: Arc<std::sync::Mutex<Option<std::fs::File>>>,
}

impl Logger {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }

        let file = OpenOptions::new().create(true).append(true).open(path).ok();
        Self {
            file: Arc::new(std::sync::Mutex::new(file)),
        }
    }

    pub fn log_tagged(&self, tag: &str, message: &str) {
        self.write_line(&format!("[{tag}] {message}"));
    }

    fn write_line(&self, message: &str) {
        let line = format!("[{}] {message}\n", timestamp_string());
        print!("{line}");
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }
}

fn timestamp_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}

pub fn backend_log_tag(backend: BackendMode) -> &'static str {
    match backend {
        BackendMode::Auto => "Auto",
        BackendMode::Musicxmatch => "MusicXMatch",
        BackendMode::Deezer => "Deezer",
        BackendMode::Bugs => "Bugs",
    }
}

pub fn provider_log_tag(provider: &str) -> &'static str {
    match provider {
        "musicxmatch" => "MusicXMatch",
        "deezer" => "Deezer",
        "bugs" => "Bugs",
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
