//! Log format templates for high-speed generation.
//!
//! Supports Apache, Nginx, JSON, Syslog, and Helios formats.

pub mod apache;
pub mod helios;
pub mod json;
pub mod nginx;
pub mod syslog;

use crate::fields::FieldPool;
use serde::{Deserialize, Serialize};

/// Supported log formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Apache Combined Log Format
    #[default]
    Apache,
    /// Nginx access log format
    Nginx,
    /// JSON structured logs
    Json,
    /// Syslog RFC5424 format
    Syslog,
    /// Helios event format
    Helios,
}

/// Trait for log formatters.
pub trait LogFormatter: Send + Sync {
    /// Write a single log entry to the buffer.
    /// Returns the number of bytes written.
    fn write_log(
        &self,
        buf: &mut Vec<u8>,
        pool: &FieldPool,
        rng: &mut fastrand::Rng,
        error_rate: f32,
        timestamp: &str,
    ) -> usize;

    /// Estimated average log line size in bytes.
    fn estimated_size(&self) -> usize;
}

/// Create a formatter for the given log format.
pub fn create_formatter(format: LogFormat) -> Box<dyn LogFormatter> {
    match format {
        LogFormat::Apache => Box::new(apache::ApacheFormatter::new()),
        LogFormat::Nginx => Box::new(nginx::NginxFormatter::new()),
        LogFormat::Json => Box::new(json::JsonFormatter::new()),
        LogFormat::Syslog => Box::new(syslog::SyslogFormatter::new()),
        LogFormat::Helios => Box::new(helios::HeliosFormatter::new()),
    }
}
