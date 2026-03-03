//! HTTP endpoint output writer.
//!
//! Sends log batches to an HTTP endpoint via POST requests.

use super::OutputWriter;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::io::{self, Error, ErrorKind};
use std::time::Duration;

/// Wrapper format for HTTP batches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpBatchFormat {
    /// Raw data, newline-delimited (default)
    Raw,
    /// Helios format: {"events":[...]}
    Helios,
}

impl Default for HttpBatchFormat {
    fn default() -> Self {
        Self::Raw
    }
}

/// Configuration for HTTP output.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Target URL endpoint
    pub url: String,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum retries on failure
    pub max_retries: u32,
    /// Batch size before sending (bytes)
    pub batch_size: usize,
    /// Content-Type header
    pub content_type: String,
    /// Optional authorization header
    pub auth_header: Option<String>,
    /// Enable gzip compression
    pub gzip: bool,
    /// Batch format wrapper
    pub batch_format: HttpBatchFormat,
    /// Number of concurrent sender threads for HTTP output
    pub num_senders: usize,
    /// Extra custom headers (name, value)
    pub custom_headers: Vec<(String, String)>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            batch_size: 1024 * 1024, // 1MB batches
            content_type: "application/x-ndjson".to_string(),
            auth_header: None,
            gzip: true,
            batch_format: HttpBatchFormat::Raw,
            num_senders: 4,
            custom_headers: Vec::new(),
        }
    }
}

impl HttpConfig {
    /// Create a new HTTP config with the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the batch size threshold.
    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Set the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = content_type.into();
        self
    }

    /// Set authorization header (e.g., "Bearer token123").
    pub fn with_auth(mut self, auth: impl Into<String>) -> Self {
        self.auth_header = Some(auth.into());
        self
    }

    /// Enable or disable gzip compression.
    pub fn with_gzip(mut self, enabled: bool) -> Self {
        self.gzip = enabled;
        self
    }

    /// Set the batch format (Raw or Helios).
    pub fn with_batch_format(mut self, format: HttpBatchFormat) -> Self {
        self.batch_format = format;
        self
    }

    /// Add a custom header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((name.into(), value.into()));
        self
    }

    /// Set the number of concurrent sender threads.
    pub fn with_num_senders(mut self, n: usize) -> Self {
        self.num_senders = n;
        self
    }

    /// Configure for Helios API endpoint.
    pub fn for_helios(mut self) -> Self {
        self.batch_format = HttpBatchFormat::Helios;
        self.content_type = "application/json".to_string();
        self
    }
}

/// HTTP output writer that sends log batches to an endpoint.
pub struct HttpWriter {
    client: Client,
    config: HttpConfig,
    buffer: Vec<u8>,
    bytes_written: u64,
    requests_sent: u64,
    requests_failed: u64,
}

impl HttpWriter {
    /// Create a new HTTP writer with the given configuration.
    pub fn new(config: HttpConfig) -> io::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(&config.content_type)
                .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
        );

        if let Some(auth) = &config.auth_header {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(auth)
                    .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
            );
        }

        for (name, value) in &config.custom_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
                HeaderValue::from_str(value)
                    .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?,
            );
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .default_headers(headers)
            .gzip(config.gzip)
            .pool_max_idle_per_host(4)
            .build()
            .map_err(|e| Error::new(ErrorKind::Other, e))?;

        Ok(Self {
            client,
            config,
            buffer: Vec::with_capacity(1024 * 1024), // 1MB initial capacity
            bytes_written: 0,
            requests_sent: 0,
            requests_failed: 0,
        })
    }

    /// Create a new HTTP writer with just a URL (using defaults).
    pub fn from_url(url: impl Into<String>) -> io::Result<Self> {
        Self::new(HttpConfig::new(url))
    }

    /// Send the buffered data to the endpoint.
    fn send_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Prepare the payload based on batch format
        let payload = match self.config.batch_format {
            HttpBatchFormat::Raw => {
                std::mem::take(&mut self.buffer)
            }
            HttpBatchFormat::Helios => {
                // Wrap events in {"events":[...]} format
                // The buffer contains comma-separated events like: {event1},{event2},
                let mut events_data = std::mem::take(&mut self.buffer);

                // Remove trailing comma if present
                if events_data.last() == Some(&b',') {
                    events_data.pop();
                }

                // Build the wrapper
                let mut payload = Vec::with_capacity(events_data.len() + 15);
                payload.extend_from_slice(b"{\"events\":[");
                payload.extend_from_slice(&events_data);
                payload.extend_from_slice(b"]}");
                payload
            }
        };

        let data_len = payload.len();

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            match self.client.post(&self.config.url).body(payload.clone()).send() {
                Ok(response) => {
                    self.requests_sent += 1;
                    if response.status().is_success() {
                        self.bytes_written += data_len as u64;
                        self.buffer = Vec::with_capacity(self.config.batch_size);
                        return Ok(());
                    } else {
                        last_error = Some(format!(
                            "HTTP {}: {}",
                            response.status(),
                            response.text().unwrap_or_default()
                        ));
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            if attempt < self.config.max_retries {
                // Exponential backoff: 100ms, 200ms, 400ms
                std::thread::sleep(Duration::from_millis(100 << attempt));
            }
        }

        self.requests_failed += 1;
        // Restore buffer for retry later (unwrapped)
        self.buffer = match self.config.batch_format {
            HttpBatchFormat::Raw => payload,
            HttpBatchFormat::Helios => {
                // Extract events from payload for retry
                if payload.len() > 13 {
                    let mut restored = payload[11..payload.len() - 2].to_vec();
                    restored.push(b','); // Re-add trailing comma
                    restored
                } else {
                    Vec::new()
                }
            }
        };
        Err(Error::new(
            ErrorKind::Other,
            format!(
                "Failed to send after {} retries: {}",
                self.config.max_retries,
                last_error.unwrap_or_default()
            ),
        ))
    }

    /// Get the number of successful HTTP requests sent.
    pub fn requests_sent(&self) -> u64 {
        self.requests_sent
    }

    /// Get the number of failed HTTP requests.
    pub fn requests_failed(&self) -> u64 {
        self.requests_failed
    }
}

impl OutputWriter for HttpWriter {
    fn write_batch(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(data);

        // Send when buffer exceeds threshold
        if self.buffer.len() >= self.config.batch_size {
            self.send_buffer()?;
        }

        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send_buffer()
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_config_builder() {
        let config = HttpConfig::new("http://localhost:8080")
            .with_timeout(Duration::from_secs(60))
            .with_batch_size(512 * 1024)
            .with_content_type("application/json")
            .with_auth("Bearer token123")
            .with_gzip(false);

        assert_eq!(config.url, "http://localhost:8080");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.batch_size, 512 * 1024);
        assert_eq!(config.content_type, "application/json");
        assert_eq!(config.auth_header, Some("Bearer token123".to_string()));
        assert!(!config.gzip);
    }
}
