//! File output writer with buffering.

use super::OutputWriter;
use std::fs::File;
use std::io::{self, BufWriter, Stdout, Write};
use std::path::Path;

/// Buffered file writer.
pub struct FileWriter {
    writer: BufWriter<File>,
    bytes_written: u64,
}

impl FileWriter {
    /// Create a new file writer.
    pub fn new(path: &Path) -> io::Result<Self> {
        let file = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        // Use 64KB buffer for efficient writes
        let writer = BufWriter::with_capacity(64 * 1024, file);

        Ok(Self {
            writer,
            bytes_written: 0,
        })
    }
}

impl OutputWriter for FileWriter {
    fn write_batch(&mut self, data: &[u8]) -> io::Result<usize> {
        self.writer.write_all(data)?;
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

/// Stdout writer with buffering.
pub struct StdoutWriter {
    writer: BufWriter<Stdout>,
    bytes_written: u64,
}

impl StdoutWriter {
    pub fn new() -> Self {
        Self {
            writer: BufWriter::with_capacity(64 * 1024, io::stdout()),
            bytes_written: 0,
        }
    }
}

impl Default for StdoutWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputWriter for StdoutWriter {
    fn write_batch(&mut self, data: &[u8]) -> io::Result<usize> {
        self.writer.write_all(data)?;
        self.bytes_written += data.len() as u64;
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}
