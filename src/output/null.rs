//! Null output writer for benchmarking.
//!
//! Discards all output but counts bytes.

use super::OutputWriter;
use std::io;

/// Null writer that discards output.
pub struct NullWriter {
    bytes_written: u64,
}

impl NullWriter {
    pub fn new() -> Self {
        Self { bytes_written: 0 }
    }
}

impl Default for NullWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputWriter for NullWriter {
    #[inline(always)]
    fn write_batch(&mut self, data: &[u8]) -> io::Result<usize> {
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }

    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}
