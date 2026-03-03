//! Output module for writing generated logs.
//!
//! Supports file, stdout, null (benchmark), and HTTP endpoint outputs.

pub mod file;
pub mod http;
pub mod http_concurrent;
pub mod metrics;
pub mod null;

use std::io;
use std::path::PathBuf;

pub use http::{HttpBatchFormat, HttpConfig};

/// Output configuration.
#[derive(Debug, Clone)]
pub enum OutputConfig {
    /// Write to a file
    File(PathBuf),
    /// Write to stdout
    Stdout,
    /// Discard output (for benchmarking)
    Null,
    /// Send to HTTP endpoint
    Http(HttpConfig),
}

/// Trait for output writers.
pub trait OutputWriter: Send {
    /// Write a batch of log data.
    fn write_batch(&mut self, data: &[u8]) -> io::Result<usize>;

    /// Flush any buffered data.
    fn flush(&mut self) -> io::Result<()>;

    /// Get total bytes written.
    fn bytes_written(&self) -> u64;
}

/// Create an output writer for the given configuration.
pub fn create_writer(config: &OutputConfig) -> io::Result<Box<dyn OutputWriter>> {
    match config {
        OutputConfig::File(path) => {
            let writer = file::FileWriter::new(path)?;
            Ok(Box::new(writer))
        }
        OutputConfig::Stdout => {
            let writer = file::StdoutWriter::new();
            Ok(Box::new(writer))
        }
        OutputConfig::Null => {
            let writer = null::NullWriter::new();
            Ok(Box::new(writer))
        }
        OutputConfig::Http(http_config) => {
            let writer = http_concurrent::ConcurrentHttpWriter::new(http_config.clone())?;
            Ok(Box::new(writer))
        }
    }
}
