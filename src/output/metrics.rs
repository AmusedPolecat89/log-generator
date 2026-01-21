//! Real-time metrics display.

use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Shared metrics counters.
pub struct MetricsCounters {
    /// Total logs generated
    pub logs_generated: AtomicU64,
    /// Total bytes generated
    pub bytes_generated: AtomicU64,
    /// Total errors generated
    pub errors_generated: AtomicU64,
}

impl MetricsCounters {
    pub fn new() -> Self {
        Self {
            logs_generated: AtomicU64::new(0),
            bytes_generated: AtomicU64::new(0),
            errors_generated: AtomicU64::new(0),
        }
    }

    /// Add to log count.
    #[inline(always)]
    pub fn add_logs(&self, count: u64) {
        self.logs_generated.fetch_add(count, Ordering::Relaxed);
    }

    /// Add to byte count.
    #[inline(always)]
    pub fn add_bytes(&self, count: u64) {
        self.bytes_generated.fetch_add(count, Ordering::Relaxed);
    }

    /// Add to error count.
    #[inline(always)]
    pub fn add_errors(&self, count: u64) {
        self.errors_generated.fetch_add(count, Ordering::Relaxed);
    }

    /// Get current counts.
    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.logs_generated.load(Ordering::Relaxed),
            self.bytes_generated.load(Ordering::Relaxed),
            self.errors_generated.load(Ordering::Relaxed),
        )
    }
}

impl Default for MetricsCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics display state.
pub struct MetricsDisplay {
    counters: Arc<MetricsCounters>,
    start_time: Instant,
    last_update: Instant,
    last_logs: u64,
    last_bytes: u64,
    update_interval: Duration,
}

impl MetricsDisplay {
    pub fn new(counters: Arc<MetricsCounters>) -> Self {
        let now = Instant::now();
        Self {
            counters,
            start_time: now,
            last_update: now,
            last_logs: 0,
            last_bytes: 0,
            update_interval: Duration::from_millis(500),
        }
    }

    /// Update and display metrics if interval has passed.
    pub fn maybe_display(
        &mut self,
        progress_percent: f32,
        rate_desc: &str,
        spike_desc: &str,
    ) {
        let now = Instant::now();
        if now.duration_since(self.last_update) < self.update_interval {
            return;
        }

        let (logs, bytes, _errors) = self.counters.snapshot();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();

        // Calculate rates
        let logs_per_sec = (logs - self.last_logs) as f64 / elapsed;
        let bytes_per_sec = (bytes - self.last_bytes) as f64 / elapsed;
        let mb_per_sec = bytes_per_sec / (1024.0 * 1024.0);

        // Total stats
        let total_elapsed = now.duration_since(self.start_time).as_secs_f64();
        let total_mb = bytes as f64 / (1024.0 * 1024.0);
        let avg_mb_per_sec = total_mb / total_elapsed;

        // Print metrics line (overwrite previous)
        eprint!(
            "\r\x1b[K[{:5.1}%] {:>8.1} MB/s | {:>8.1}M logs/s | Total: {:>8.1} MB | Avg: {:>7.1} MB/s | Rate: {} | Spikes: {}",
            progress_percent,
            mb_per_sec,
            logs_per_sec / 1_000_000.0,
            total_mb,
            avg_mb_per_sec,
            rate_desc,
            spike_desc
        );
        let _ = io::stderr().flush();

        self.last_update = now;
        self.last_logs = logs;
        self.last_bytes = bytes;
    }

    /// Display final summary.
    pub fn display_summary(&self) {
        let (logs, bytes, errors) = self.counters.snapshot();
        let elapsed = Instant::now().duration_since(self.start_time).as_secs_f64();

        let total_mb = bytes as f64 / (1024.0 * 1024.0);
        let total_gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let avg_mb_per_sec = total_mb / elapsed;
        let avg_logs_per_sec = logs as f64 / elapsed;
        let error_rate = if logs > 0 {
            errors as f64 / logs as f64 * 100.0
        } else {
            0.0
        };

        eprintln!("\n\n=== Generation Complete ===");
        eprintln!("Duration:        {:.2} seconds", elapsed);
        eprintln!("Total logs:      {} ({:.2}M)", logs, logs as f64 / 1_000_000.0);
        eprintln!("Total size:      {:.2} GB ({:.2} MB)", total_gb, total_mb);
        eprintln!("Average rate:    {:.2} MB/s", avg_mb_per_sec);
        eprintln!("Average logs/s:  {:.2}M", avg_logs_per_sec / 1_000_000.0);
        eprintln!("Error logs:      {} ({:.2}%)", errors, error_rate);

        if avg_mb_per_sec >= 1000.0 {
            eprintln!("\n✓ Target achieved: {:.2} GB/s", avg_mb_per_sec / 1024.0);
        } else {
            eprintln!(
                "\n○ Current rate: {:.2} MB/s (target: 1000 MB/s)",
                avg_mb_per_sec
            );
        }
    }
}
