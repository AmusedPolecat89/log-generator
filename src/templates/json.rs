//! JSON structured log format generator.
//!
//! Produces structured JSON logs suitable for log aggregation systems.
//! Example: {"timestamp":"2023-10-10T13:55:36.123Z","level":"INFO","service":"api-gateway","trace_id":"abc123","message":"Request processed","duration_ms":45,"status":200}

use super::LogFormatter;
use crate::fields::FieldPool;

/// JSON structured log formatter.
pub struct JsonFormatter;

impl JsonFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl LogFormatter for JsonFormatter {
    #[inline]
    fn write_log(
        &self,
        buf: &mut Vec<u8>,
        pool: &FieldPool,
        rng: &mut fastrand::Rng,
        error_rate: f32,
        timestamp: &str,
    ) -> usize {
        let start_len = buf.len();

        // Get random field indices
        let service_idx = rng.u8(..);
        let level_idx = rng.u8(..);
        let path_idx = rng.u16(..);
        let method_idx = rng.u8(..);
        let ip_idx = rng.u16(..);

        // Get status based on error rate
        let status = pool.status_codes.get_status(rng, error_rate);

        // Adjust level based on status
        let level = if status >= 500 {
            "ERROR"
        } else if status >= 400 {
            "WARN"
        } else {
            pool.get_log_level(level_idx)
        };

        // Generate trace_id and span_id
        let trace_id = rng.u64(..);
        let span_id = rng.u32(..);

        // Generate duration (faster for success, slower for errors)
        let duration_ms = if status >= 500 {
            100 + rng.u32(0..5000)
        } else if status >= 400 {
            10 + rng.u32(0..500)
        } else {
            1 + rng.u32(0..200)
        };

        // Get field values
        let service = pool.get_service(service_idx);
        let path = pool.get_path(path_idx);
        let method = pool.get_method(method_idx);
        let client_ip = pool.get_ip(ip_idx);

        // Generate message based on status
        let message = match status {
            200 | 201 => "Request processed successfully",
            204 => "No content returned",
            301 | 302 | 307 => "Redirect issued",
            304 => "Not modified",
            400 => "Bad request",
            401 => "Unauthorized access attempt",
            403 => "Forbidden resource access",
            404 => "Resource not found",
            429 => "Rate limit exceeded",
            500 => "Internal server error",
            502 => "Bad gateway",
            503 => "Service unavailable",
            504 => "Gateway timeout",
            _ => "Request handled",
        };

        // Build JSON manually for speed (no serde overhead)
        buf.extend_from_slice(b"{\"timestamp\":\"");
        buf.extend_from_slice(timestamp.as_bytes());
        buf.extend_from_slice(b"\",\"level\":\"");
        buf.extend_from_slice(level.as_bytes());
        buf.extend_from_slice(b"\",\"service\":\"");
        buf.extend_from_slice(service.as_bytes());
        buf.extend_from_slice(b"\",\"trace_id\":\"");

        // Format trace_id as hex
        write_hex_u64(buf, trace_id);

        buf.extend_from_slice(b"\",\"span_id\":\"");
        write_hex_u32(buf, span_id);

        buf.extend_from_slice(b"\",\"message\":\"");
        buf.extend_from_slice(message.as_bytes());
        buf.extend_from_slice(b"\",\"method\":\"");
        buf.extend_from_slice(method.as_bytes());
        buf.extend_from_slice(b"\",\"path\":\"");

        // Escape path for JSON (basic escaping)
        write_json_escaped(buf, path);

        buf.extend_from_slice(b"\",\"status\":");

        let mut itoa_buf = itoa::Buffer::new();
        buf.extend_from_slice(itoa_buf.format(status).as_bytes());

        buf.extend_from_slice(b",\"duration_ms\":");
        buf.extend_from_slice(itoa_buf.format(duration_ms).as_bytes());

        buf.extend_from_slice(b",\"client_ip\":\"");
        buf.extend_from_slice(client_ip.as_bytes());

        buf.extend_from_slice(b"\"}\n");

        buf.len() - start_len
    }

    fn estimated_size(&self) -> usize {
        // JSON logs are typically larger
        350
    }
}

/// Write a u64 as hexadecimal.
#[inline]
fn write_hex_u64(buf: &mut Vec<u8>, value: u64) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut tmp = [0u8; 16];
    for i in 0..16 {
        let nibble = ((value >> (60 - i * 4)) & 0xF) as usize;
        tmp[i] = HEX_CHARS[nibble];
    }
    buf.extend_from_slice(&tmp);
}

/// Write a u32 as hexadecimal.
#[inline]
fn write_hex_u32(buf: &mut Vec<u8>, value: u32) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut tmp = [0u8; 8];
    for i in 0..8 {
        let nibble = ((value >> (28 - i * 4)) & 0xF) as usize;
        tmp[i] = HEX_CHARS[nibble];
    }
    buf.extend_from_slice(&tmp);
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
    use crate::fields::timestamp::CachedIsoTimestamp;
    use std::time::Duration;

    #[test]
    fn test_json_format() {
        let formatter = JsonFormatter::new();
        let pool = FieldPool::new();
        let mut rng = fastrand::Rng::new();
        let mut ts = CachedIsoTimestamp::new(Duration::from_millis(100));

        let mut buf = Vec::with_capacity(512);
        let len = formatter.write_log(&mut buf, &pool, &mut rng, 0.01, ts.get());

        let line = String::from_utf8_lossy(&buf);
        println!("JSON log: {}", line);

        assert!(len > 200);
        assert!(line.starts_with('{'));
        assert!(line.contains("\"timestamp\":"));
        assert!(line.contains("\"level\":"));
        assert!(line.contains("\"service\":"));
    }
}
