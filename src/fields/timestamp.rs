//! Fast timestamp generation and formatting.
//!
//! Implements cached timestamp formatting to avoid repeated formatting calls.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Cached timestamp for Apache/Nginx log format.
/// Format: [10/Oct/2023:13:55:36 +0000]
pub struct CachedApacheTimestamp {
    /// Pre-formatted timestamp string
    formatted: String,
    /// When this cache expires
    expires_at: Instant,
    /// Cache duration
    cache_duration: Duration,
}

impl CachedApacheTimestamp {
    /// Create a new cached timestamp with given cache duration.
    pub fn new(cache_duration: Duration) -> Self {
        let mut ts = Self {
            formatted: String::with_capacity(32),
            expires_at: Instant::now(),
            cache_duration,
        };
        ts.refresh();
        ts
    }

    /// Update the cached timestamp if expired.
    #[inline]
    pub fn maybe_refresh(&mut self) {
        let now = Instant::now();
        if now >= self.expires_at {
            self.refresh();
            self.expires_at = now + self.cache_duration;
        }
    }

    /// Force refresh the timestamp.
    fn refresh(&mut self) {
        self.formatted.clear();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        format_apache_timestamp(now, &mut self.formatted);
    }

    /// Get the formatted timestamp string.
    #[inline(always)]
    pub fn get(&self) -> &str {
        &self.formatted
    }
}

/// Cached timestamp for ISO 8601 format (JSON logs).
/// Format: 2023-10-10T13:55:36.123Z
pub struct CachedIsoTimestamp {
    /// Pre-formatted timestamp string
    formatted: String,
    /// When this cache expires
    expires_at: Instant,
    /// Cache duration
    cache_duration: Duration,
    /// Last unix timestamp (for millisecond updates)
    last_secs: u64,
}

impl CachedIsoTimestamp {
    pub fn new(cache_duration: Duration) -> Self {
        let mut ts = Self {
            formatted: String::with_capacity(32),
            expires_at: Instant::now(),
            cache_duration,
            last_secs: 0,
        };
        ts.refresh();
        ts
    }

    #[inline]
    pub fn maybe_refresh(&mut self) {
        let now = Instant::now();
        if now >= self.expires_at {
            self.refresh();
            self.expires_at = now + self.cache_duration;
        }
    }

    fn refresh(&mut self) {
        self.formatted.clear();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let secs = now.as_secs();
        let millis = now.subsec_millis();

        format_iso_timestamp(secs, millis, &mut self.formatted);
        self.last_secs = secs;
    }

    #[inline(always)]
    pub fn get(&self) -> &str {
        &self.formatted
    }
}

/// Cached timestamp for Syslog RFC5424 format.
/// Format: 2023-10-10T13:55:36.123456+00:00
pub struct CachedSyslogTimestamp {
    formatted: String,
    expires_at: Instant,
    cache_duration: Duration,
}

impl CachedSyslogTimestamp {
    pub fn new(cache_duration: Duration) -> Self {
        let mut ts = Self {
            formatted: String::with_capacity(32),
            expires_at: Instant::now(),
            cache_duration,
        };
        ts.refresh();
        ts
    }

    #[inline]
    pub fn maybe_refresh(&mut self) {
        let now = Instant::now();
        if now >= self.expires_at {
            self.refresh();
            self.expires_at = now + self.cache_duration;
        }
    }

    fn refresh(&mut self) {
        self.formatted.clear();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let secs = now.as_secs();
        let micros = now.subsec_micros();

        format_syslog_timestamp(secs, micros, &mut self.formatted);
    }

    #[inline(always)]
    pub fn get(&self) -> &str {
        &self.formatted
    }
}

/// Format a unix timestamp as Apache log format: [10/Oct/2023:13:55:36 +0000]
fn format_apache_timestamp(unix_secs: u64, out: &mut String) {
    // Calculate date/time components
    let (year, month, day, hour, min, sec) = unix_to_datetime(unix_secs);

    let month_str = MONTHS[month as usize - 1];

    use std::fmt::Write;
    write!(
        out,
        "[{:02}/{}/{:04}:{:02}:{:02}:{:02} +0000]",
        day, month_str, year, hour, min, sec
    )
    .unwrap();
}

/// Format a unix timestamp as ISO 8601: 2023-10-10T13:55:36.123Z
fn format_iso_timestamp(unix_secs: u64, millis: u32, out: &mut String) {
    let (year, month, day, hour, min, sec) = unix_to_datetime(unix_secs);

    use std::fmt::Write;
    write!(
        out,
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hour, min, sec, millis
    )
    .unwrap();
}

/// Format a unix timestamp as Syslog RFC5424: 2023-10-10T13:55:36.123456+00:00
fn format_syslog_timestamp(unix_secs: u64, micros: u32, out: &mut String) {
    let (year, month, day, hour, min, sec) = unix_to_datetime(unix_secs);

    use std::fmt::Write;
    write!(
        out,
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
        year, month, day, hour, min, sec, micros
    )
    .unwrap();
}

/// Convert unix timestamp to (year, month, day, hour, min, sec).
/// All in UTC.
fn unix_to_datetime(unix_secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    // Days since Unix epoch
    let days = (unix_secs / 86400) as u32;
    let time_of_day = (unix_secs % 86400) as u32;

    let hour = time_of_day / 3600;
    let min = (time_of_day % 3600) / 60;
    let sec = time_of_day % 60;

    // Calculate year, month, day from days since epoch
    // Using a simplified algorithm
    let (year, month, day) = days_to_ymd(days);

    (year, month, day, hour, min, sec)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u32) -> (u32, u32, u32) {
    // Days from 1970-01-01
    // This is a simplified algorithm that works for reasonable date ranges

    let mut remaining = days;
    let mut year = 1970u32;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let mut month = 1u32;

    for m in 1..=12 {
        let days_in_month = days_in_month(m, leap);
        if remaining < days_in_month {
            month = m;
            break;
        }
        remaining -= days_in_month;
    }

    let day = remaining + 1;

    (year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(month: u32, leap: bool) -> u32 {
    match month {
        1 => 31,
        2 => if leap { 29 } else { 28 },
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30,
    }
}

const MONTHS: &[&str] = &[
    "Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apache_timestamp() {
        let mut out = String::new();
        // 2023-10-10 13:55:36 UTC
        format_apache_timestamp(1696945736, &mut out);
        assert!(out.contains("Oct"));
        assert!(out.contains("2023"));
    }

    #[test]
    fn test_iso_timestamp() {
        let mut out = String::new();
        format_iso_timestamp(1696945736, 123, &mut out);
        assert!(out.contains("2023"));
        assert!(out.contains("123Z"));
    }
}
