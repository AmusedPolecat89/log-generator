//! Nginx access log format generator.
//!
//! Format: $remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" $request_time
//! Example: 192.168.1.1 - john [10/Oct/2023:13:55:36 +0000] "GET /api/users HTTP/1.1" 200 1234 "https://example.com" "Mozilla/5.0..." 0.045

use super::LogFormatter;
use crate::fields::FieldPool;

/// Nginx access log formatter.
pub struct NginxFormatter;

impl NginxFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NginxFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl LogFormatter for NginxFormatter {
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
        let ip_idx = rng.u16(..);
        let path_idx = rng.u16(..);
        let ua_idx = rng.u8(..);
        let method_idx = rng.u8(..);
        let protocol_idx = rng.u8(..);
        let referrer_idx = rng.u8(..);
        let username_idx = rng.u8(..);

        // Get status and response size
        let status = pool.status_codes.get_status(rng, error_rate);
        let body_bytes = pool.response_sizes.get_size(rng, status);

        // Generate request time (0.001 - 5.0 seconds, weighted towards lower)
        let request_time = if status >= 500 {
            // Slower for server errors
            0.5 + rng.f32() * 4.5
        } else if status >= 400 {
            // Medium for client errors
            0.01 + rng.f32() * 0.5
        } else {
            // Fast for success
            0.001 + rng.f32() * 0.2
        };

        // Get field values
        let ip = pool.get_ip(ip_idx);
        let path = pool.get_path(path_idx);
        let ua = pool.get_user_agent(ua_idx);
        let method = pool.get_method(method_idx);
        let protocol = pool.get_protocol(protocol_idx);
        let referrer = pool.get_referrer(referrer_idx);
        let username = pool.get_username(username_idx);

        // Write: ip " - " username " "
        buf.extend_from_slice(ip.as_bytes());
        buf.extend_from_slice(b" - ");
        buf.extend_from_slice(username.as_bytes());
        buf.push(b' ');

        // Write: timestamp " "
        buf.extend_from_slice(timestamp.as_bytes());
        buf.push(b' ');

        // Write: "\"" method " " path " " protocol "\" "
        buf.push(b'"');
        buf.extend_from_slice(method.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(path.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(protocol.as_bytes());
        buf.extend_from_slice(b"\" ");

        // Write: status " " body_bytes " "
        let mut itoa_buf = itoa::Buffer::new();
        buf.extend_from_slice(itoa_buf.format(status).as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(itoa_buf.format(body_bytes).as_bytes());
        buf.push(b' ');

        // Write: "\"" referrer "\" "
        buf.push(b'"');
        buf.extend_from_slice(referrer.as_bytes());
        buf.extend_from_slice(b"\" ");

        // Write: "\"" user_agent "\" "
        buf.push(b'"');
        buf.extend_from_slice(ua.as_bytes());
        buf.extend_from_slice(b"\" ");

        // Write: request_time (use ryu for fast float formatting)
        let mut ryu_buf = ryu::Buffer::new();
        buf.extend_from_slice(ryu_buf.format(request_time).as_bytes());

        // Write newline
        buf.push(b'\n');

        buf.len() - start_len
    }

    fn estimated_size(&self) -> usize {
        // Nginx logs slightly larger than Apache due to request_time
        260
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::timestamp::CachedApacheTimestamp;
    use std::time::Duration;

    #[test]
    fn test_nginx_format() {
        let formatter = NginxFormatter::new();
        let pool = FieldPool::new();
        let mut rng = fastrand::Rng::new();
        let mut ts = CachedApacheTimestamp::new(Duration::from_millis(100));

        let mut buf = Vec::with_capacity(512);
        let len = formatter.write_log(&mut buf, &pool, &mut rng, 0.01, ts.get());

        let line = String::from_utf8_lossy(&buf);
        println!("Nginx log: {}", line);

        assert!(len > 100);
        assert!(line.contains("HTTP/"));
        // Should have a decimal number at the end (request_time)
        assert!(line.contains('.'));
    }
}
