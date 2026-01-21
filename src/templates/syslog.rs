//! Syslog RFC5424 format generator.
//!
//! Format: <priority>version timestamp hostname app-name procid msgid [structured-data] msg
//! Example: <134>1 2023-10-10T13:55:36.123456+00:00 web-01 api-gateway 12345 REQ001 [meta key="value"] Request processed successfully

use super::LogFormatter;
use crate::fields::FieldPool;

/// Syslog RFC5424 formatter.
pub struct SyslogFormatter;

impl SyslogFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SyslogFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Syslog facility codes.
const FACILITY_LOCAL0: u8 = 16;

/// Syslog severity levels.
#[derive(Clone, Copy)]
#[repr(u8)]
enum Severity {
    Emergency = 0,
    Alert = 1,
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
    Debug = 7,
}

impl Severity {
    fn from_level(level: &str) -> Self {
        match level {
            "DEBUG" => Severity::Debug,
            "INFO" => Severity::Info,
            "WARN" => Severity::Warning,
            "ERROR" => Severity::Error,
            "FATAL" => Severity::Critical,
            _ => Severity::Info,
        }
    }
}

impl LogFormatter for SyslogFormatter {
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
        let hostname_idx = rng.u8(..);
        let service_idx = rng.u8(..);
        let level_idx = rng.u8(..);

        // Adjust level based on error rate
        let level = if rng.f32() < error_rate {
            if rng.bool() {
                "ERROR"
            } else {
                "WARN"
            }
        } else {
            pool.get_log_level(level_idx)
        };

        let severity = Severity::from_level(level);
        let priority = (FACILITY_LOCAL0 as u16 * 8) + (severity as u16);

        // Get field values
        let hostname = pool.get_hostname(hostname_idx);
        let app_name = pool.get_service(service_idx);

        // Generate PID and message ID
        let pid = 1000 + rng.u32(0..50000);
        let msg_id = rng.u32(..);

        // Generate structured data
        let request_id = rng.u64(..);
        let duration_ms = rng.u32(1..1000);

        // Generate message
        let message = match level {
            "DEBUG" => "Debug checkpoint reached",
            "INFO" => "Operation completed successfully",
            "WARN" => "Potential issue detected",
            "ERROR" => "Error occurred during processing",
            "FATAL" => "Critical failure, service impacted",
            _ => "Log event recorded",
        };

        // Write: "<" priority ">" version " "
        buf.push(b'<');
        let mut itoa_buf = itoa::Buffer::new();
        buf.extend_from_slice(itoa_buf.format(priority).as_bytes());
        buf.extend_from_slice(b">1 ");

        // Write: timestamp " "
        buf.extend_from_slice(timestamp.as_bytes());
        buf.push(b' ');

        // Write: hostname " " app-name " " procid " " msgid " "
        buf.extend_from_slice(hostname.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(app_name.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(itoa_buf.format(pid).as_bytes());
        buf.push(b' ');

        // Message ID (formatted as MSG followed by hex)
        buf.extend_from_slice(b"MSG");
        write_hex_u32_short(buf, msg_id);
        buf.push(b' ');

        // Structured data: [meta request_id="..." duration_ms="..."]
        buf.extend_from_slice(b"[meta request_id=\"");
        write_hex_u64(buf, request_id);
        buf.extend_from_slice(b"\" duration_ms=\"");
        buf.extend_from_slice(itoa_buf.format(duration_ms).as_bytes());
        buf.extend_from_slice(b"\" level=\"");
        buf.extend_from_slice(level.as_bytes());
        buf.extend_from_slice(b"\"] ");

        // Write message
        buf.extend_from_slice(message.as_bytes());

        // Write newline
        buf.push(b'\n');

        buf.len() - start_len
    }

    fn estimated_size(&self) -> usize {
        // Syslog messages are medium-sized
        220
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

/// Write a u32 as short hex (8 chars).
#[inline]
fn write_hex_u32_short(buf: &mut Vec<u8>, value: u32) {
    const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
    let mut tmp = [0u8; 8];
    for i in 0..8 {
        let nibble = ((value >> (28 - i * 4)) & 0xF) as usize;
        tmp[i] = HEX_CHARS[nibble];
    }
    buf.extend_from_slice(&tmp);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::timestamp::CachedSyslogTimestamp;
    use std::time::Duration;

    #[test]
    fn test_syslog_format() {
        let formatter = SyslogFormatter::new();
        let pool = FieldPool::new();
        let mut rng = fastrand::Rng::new();
        let mut ts = CachedSyslogTimestamp::new(Duration::from_millis(100));

        let mut buf = Vec::with_capacity(512);
        let len = formatter.write_log(&mut buf, &pool, &mut rng, 0.01, ts.get());

        let line = String::from_utf8_lossy(&buf);
        println!("Syslog: {}", line);

        assert!(len > 100);
        assert!(line.starts_with('<'));
        assert!(line.contains("[meta"));
        assert!(line.contains("request_id="));
    }
}
