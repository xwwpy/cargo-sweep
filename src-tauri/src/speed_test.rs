use serde::Serialize;
use std::io::Read;
use std::time::{Duration, Instant};

const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DURATION: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpeedResult {
    pub url: String,
    pub ok: bool,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub duration_ms: u64,
    pub bytes: u64,
    pub speed_bps: u64,
    pub error: Option<String>,
}

fn failed(url: &str, latency_ms: u64, error: String) -> SourceSpeedResult {
    SourceSpeedResult {
        url: url.to_string(),
        ok: false,
        status: None,
        latency_ms,
        duration_ms: 0,
        bytes: 0,
        speed_bps: 0,
        error: Some(error),
    }
}

pub fn test_source(url: &str, max_bytes: Option<u64>) -> SourceSpeedResult {
    let started = Instant::now();
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(error) => return failed(url, 0, format!("地址无效：{error}")),
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return failed(url, 0, "只支持 HTTP 或 HTTPS 源".into());
    }
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .user_agent("Cargo-Sweep-Speed-Test/1.0")
        .build()
    {
        Ok(client) => client,
        Err(error) => return failed(url, 0, format!("无法创建测速客户端：{error}")),
    };
    let response = match client.get(parsed).send() {
        Ok(response) => response,
        Err(error) => {
            return failed(
                url,
                started.elapsed().as_millis() as u64,
                format!("连接失败：{error}"),
            )
        }
    };
    let latency_ms = started.elapsed().as_millis() as u64;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return SourceSpeedResult {
            url: url.to_string(),
            ok: false,
            status: Some(status),
            latency_ms,
            duration_ms: 0,
            bytes: 0,
            speed_bps: 0,
            error: Some(format!("HTTP {status}")),
        };
    }

    let limit = max_bytes.unwrap_or(DEFAULT_MAX_BYTES).max(64 * 1024);
    let download_started = Instant::now();
    let mut body = response;
    let mut bytes = 0u64;
    let mut buffer = [0u8; 16 * 1024];
    while bytes < limit {
        if download_started.elapsed() >= MAX_DURATION {
            break;
        }
        match body.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => bytes += read as u64,
            Err(error) => {
                if bytes == 0 {
                    return failed(url, latency_ms, format!("读取失败：{error}"));
                }
                break;
            }
        }
    }
    let duration_ms = download_started.elapsed().as_millis() as u64;
    let speed_bps = if duration_ms > 0 {
        (bytes as f64 / (duration_ms as f64 / 1000.0)) as u64
    } else {
        0
    };
    SourceSpeedResult {
        url: url.to_string(),
        ok: true,
        status: Some(status),
        latency_ms,
        duration_ms,
        bytes,
        speed_bps,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_urls() {
        let result = test_source("file:///etc/hosts", None);
        assert!(!result.ok);
        assert!(result.error.is_some());
    }

    #[test]
    fn rejects_invalid_urls() {
        let result = test_source("not a url", None);
        assert!(!result.ok);
        assert!(result.error.is_some());
    }
}
