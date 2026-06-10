use std::time::Duration;

use tokio::time::sleep;

const RETRY_ATTEMPTS: usize = 2;
const RETRY_DELAY_MS: u64 = 250;

#[derive(Clone, Copy)]
pub enum TimestampRounding {
    Floor,
    Nearest,
}

pub async fn send_with_retry<F, E, M>(
    build: F,
    map_provider_error: M,
) -> Result<reqwest::Response, E>
where
    F: Fn() -> reqwest::RequestBuilder,
    M: Fn(String) -> E,
{
    for attempt in 0..RETRY_ATTEMPTS {
        match build().send().await {
            Ok(response) => {
                if attempt + 1 < RETRY_ATTEMPTS && is_retryable_status(response.status()) {
                    sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                return Ok(response);
            }
            Err(error) => {
                if attempt + 1 < RETRY_ATTEMPTS && is_retryable_error(&error) {
                    sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                return Err(map_provider_error(error.to_string()));
            }
        }
    }

    Err(map_provider_error(
        "request retry loop exited unexpectedly".to_string(),
    ))
}

pub fn parse_duration_ms(value: &str) -> Option<u64> {
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

pub fn format_lrc_timestamp_ms(milliseconds: u64, rounding: TimestampRounding) -> String {
    let total_centis = match rounding {
        TimestampRounding::Floor => milliseconds / 10,
        TimestampRounding::Nearest => (milliseconds + 5) / 10,
    };
    let minutes = total_centis / 6000;
    let secs = (total_centis / 100) % 60;
    let centis = total_centis % 100;
    format!("[{minutes:02}:{secs:02}.{centis:02}]")
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn is_retryable_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_ms_supports_minutes_and_hours() {
        assert_eq!(parse_duration_ms("02:53"), Some(173000));
        assert_eq!(parse_duration_ms("1:02:03"), Some(3_723_000));
        assert_eq!(parse_duration_ms(""), None);
    }

    #[test]
    fn format_lrc_timestamp_ms_preserves_rounding_modes() {
        assert_eq!(
            format_lrc_timestamp_ms(1039, TimestampRounding::Floor),
            "[00:01.03]"
        );
        assert_eq!(
            format_lrc_timestamp_ms(1039, TimestampRounding::Nearest),
            "[00:01.04]"
        );
    }
}
