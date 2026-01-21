//! Helios event format generator.
//!
//! Produces events compatible with the Helios ingestion API.
//! Format: {"events":[{"timestamp":1768961500,"tags":{"service":"api","host":"prod-1","level":"info"},"metrics":{"response_time_ms":45,"request_count":1}}]}

use super::LogFormatter;
use crate::fields::FieldPool;
use std::time::{SystemTime, UNIX_EPOCH};

/// Helios event formatter.
///
/// This formatter generates events in the Helios API format.
/// Unlike other formatters that write one log per line, this generates
/// individual event objects that should be batched into an events array.
pub struct HeliosFormatter {
    /// Cached base timestamp (Unix epoch seconds)
    base_timestamp: u64,
}

impl HeliosFormatter {
    pub fn new() -> Self {
        let base_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self { base_timestamp }
    }

    /// Update the base timestamp (called periodically)
    pub fn refresh_timestamp(&mut self) {
        self.base_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

impl Default for HeliosFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl LogFormatter for HeliosFormatter {
    #[inline]
    fn write_log(
        &self,
        buf: &mut Vec<u8>,
        pool: &FieldPool,
        rng: &mut fastrand::Rng,
        error_rate: f32,
        _timestamp: &str, // We use Unix timestamps instead
    ) -> usize {
        let start_len = buf.len();

        // Get random field indices
        let service_idx = rng.u8(..);
        let host_idx = rng.u8(..);
        let method_idx = rng.u8(..);
        let path_idx = rng.u16(..);

        // Get status based on error rate
        let status = pool.status_codes.get_status(rng, error_rate);

        // Determine level based on status
        let level = if status >= 500 {
            "error"
        } else if status >= 400 {
            "warn"
        } else {
            "info"
        };

        // Generate metrics
        let response_time_ms = if status >= 500 {
            100 + rng.u32(0..5000)
        } else if status >= 400 {
            10 + rng.u32(0..500)
        } else {
            1 + rng.u32(0..200)
        };

        let request_size = 100 + rng.u32(0..10000);
        let response_size = pool.response_sizes.get_size(rng, status);

        // Get field values
        let service = pool.get_service(service_idx);
        let host = pool.get_host(host_idx);
        let method = pool.get_method(method_idx);
        let path = pool.get_path(path_idx);

        // Add small jitter to timestamp (within last second)
        let timestamp = self.base_timestamp.saturating_sub(rng.u64(0..2));

        let mut itoa_buf = itoa::Buffer::new();

        // Build event JSON
        // {"timestamp":1234567890,"tags":{"service":"api","host":"prod-1","level":"info","method":"GET","path":"/api/v1","status":"200"},"metrics":{"response_time_ms":45,"request_count":1,"request_size":1024,"response_size":2048}}
        buf.extend_from_slice(b"{\"timestamp\":");
        buf.extend_from_slice(itoa_buf.format(timestamp).as_bytes());

        buf.extend_from_slice(b",\"tags\":{\"service\":\"");
        buf.extend_from_slice(service.as_bytes());
        buf.extend_from_slice(b"\",\"host\":\"");
        buf.extend_from_slice(host.as_bytes());
        buf.extend_from_slice(b"\",\"level\":\"");
        buf.extend_from_slice(level.as_bytes());
        buf.extend_from_slice(b"\",\"method\":\"");
        buf.extend_from_slice(method.as_bytes());
        buf.extend_from_slice(b"\",\"path\":\"");
        write_json_escaped(buf, path);
        buf.extend_from_slice(b"\",\"status\":\"");
        buf.extend_from_slice(itoa_buf.format(status).as_bytes());
        buf.extend_from_slice(b"\"}");

        buf.extend_from_slice(b",\"metrics\":{\"response_time_ms\":");
        buf.extend_from_slice(itoa_buf.format(response_time_ms).as_bytes());
        buf.extend_from_slice(b",\"request_count\":1,\"request_size\":");
        buf.extend_from_slice(itoa_buf.format(request_size).as_bytes());
        buf.extend_from_slice(b",\"response_size\":");
        buf.extend_from_slice(itoa_buf.format(response_size).as_bytes());
        buf.extend_from_slice(b"}}");

        // Add comma for batching (will be removed for last item)
        buf.push(b',');

        buf.len() - start_len
    }

    fn estimated_size(&self) -> usize {
        // Helios events are moderate size
        250
    }
}

/// Write a string with basic JSON escaping.
#[inline]
fn write_json_escaped(buf: &mut Vec<u8>, s: &str) {
    for &b in s.as_bytes() {
        match b {
            b'"' => buf.extend_from_slice(b"\\\""),
            b'\\' => buf.extend_from_slice(b"\\\\"),
            b'\n' => buf.extend_from_slice(b"\\n"),
            b'\r' => buf.extend_from_slice(b"\\r"),
            b'\t' => buf.extend_from_slice(b"\\t"),
            _ => buf.push(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helios_format() {
        let formatter = HeliosFormatter::new();
        let pool = FieldPool::new();
        let mut rng = fastrand::Rng::new();

        let mut buf = Vec::with_capacity(512);
        let len = formatter.write_log(&mut buf, &pool, &mut rng, 0.01, "");

        // Remove trailing comma for single event
        if buf.last() == Some(&b',') {
            buf.pop();
        }

        let event = String::from_utf8_lossy(&buf);
        println!("Helios event: {}", event);

        assert!(len > 150);
        assert!(event.starts_with('{'));
        assert!(event.contains("\"timestamp\":"));
        assert!(event.contains("\"tags\":{"));
        assert!(event.contains("\"metrics\":{"));
        assert!(event.contains("\"service\":"));
        assert!(event.contains("\"response_time_ms\":"));
    }
}
