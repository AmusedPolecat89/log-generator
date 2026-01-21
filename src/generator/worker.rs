//! Worker thread for log generation.
//!
//! Each worker generates logs independently with thread-local state.

use crate::fields::timestamp::{CachedApacheTimestamp, CachedIsoTimestamp, CachedSyslogTimestamp};
use crate::fields::FieldPool;
use crate::output::metrics::MetricsCounters;
use crate::scenario::executor::SharedScenarioState;
use crate::templates::{LogFormat, LogFormatter};
use crossbeam::channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A batch of generated logs.
pub struct LogBatch {
    /// Buffer containing log data
    pub data: Vec<u8>,
    /// Number of log entries
    pub log_count: u64,
    /// Number of error logs
    pub error_count: u64,
}

/// Worker configuration.
pub struct WorkerConfig {
    /// Worker ID (for debugging)
    pub id: usize,
    /// Batch size (logs per batch)
    pub batch_size: usize,
    /// Log format
    pub format: LogFormat,
}

/// Worker thread state.
pub struct Worker {
    config: WorkerConfig,
    /// Thread-local RNG
    rng: fastrand::Rng,
    /// Pre-allocated output buffer
    buffer: Vec<u8>,
    /// Shared field pools
    field_pool: Arc<FieldPool>,
    /// Log formatter
    formatter: Box<dyn LogFormatter>,
    /// Cached timestamps
    apache_ts: CachedApacheTimestamp,
    iso_ts: CachedIsoTimestamp,
    syslog_ts: CachedSyslogTimestamp,
}

impl Worker {
    /// Create a new worker.
    pub fn new(config: WorkerConfig, field_pool: Arc<FieldPool>) -> Self {
        let formatter = crate::templates::create_formatter(config.format);
        let estimated_batch_size = config.batch_size * formatter.estimated_size();

        Self {
            config,
            rng: fastrand::Rng::new(),
            buffer: Vec::with_capacity(estimated_batch_size),
            field_pool,
            formatter,
            apache_ts: CachedApacheTimestamp::new(Duration::from_millis(100)),
            iso_ts: CachedIsoTimestamp::new(Duration::from_millis(50)),
            syslog_ts: CachedSyslogTimestamp::new(Duration::from_millis(50)),
        }
    }

    /// Run the worker loop.
    pub fn run(
        mut self,
        running: Arc<AtomicBool>,
        scenario_state: Arc<SharedScenarioState>,
        output_tx: Sender<LogBatch>,
        metrics: Arc<MetricsCounters>,
    ) {
        while running.load(Ordering::Relaxed) {
            // Check target rate and potentially throttle
            let target_rate = scenario_state.get_target_logs_per_sec();
            if target_rate == 0 {
                // Scenario hasn't started yet or is paused
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }

            // Generate a batch
            let batch = self.generate_batch(&scenario_state);

            // Update metrics
            metrics.add_logs(batch.log_count);
            metrics.add_bytes(batch.data.len() as u64);
            metrics.add_errors(batch.error_count);

            // Send to output
            if output_tx.send(batch).is_err() {
                // Channel closed, exit
                break;
            }
        }
    }

    /// Generate a batch of logs.
    fn generate_batch(&mut self, scenario_state: &SharedScenarioState) -> LogBatch {
        self.buffer.clear();

        let error_rate = scenario_state.get_error_rate();
        let mut error_count = 0u64;

        // Update timestamps
        self.apache_ts.maybe_refresh();
        self.iso_ts.maybe_refresh();
        self.syslog_ts.maybe_refresh();

        // Get appropriate timestamp for format
        // Helios uses Unix timestamps internally, so we pass empty string
        let timestamp = match self.config.format {
            LogFormat::Apache | LogFormat::Nginx => self.apache_ts.get(),
            LogFormat::Json => self.iso_ts.get(),
            LogFormat::Syslog => self.syslog_ts.get(),
            LogFormat::Helios => "", // Helios generates Unix timestamps internally
        };

        // Generate logs
        for _ in 0..self.config.batch_size {
            // Track if this will be an error
            if self.rng.f32() < error_rate {
                error_count += 1;
            }

            self.formatter.write_log(
                &mut self.buffer,
                &self.field_pool,
                &mut self.rng,
                error_rate,
                timestamp,
            );
        }

        LogBatch {
            data: std::mem::take(&mut self.buffer),
            log_count: self.config.batch_size as u64,
            error_count,
        }
    }
}

/// Spawn a worker thread.
pub fn spawn_worker(
    config: WorkerConfig,
    field_pool: Arc<FieldPool>,
    running: Arc<AtomicBool>,
    scenario_state: Arc<SharedScenarioState>,
    output_tx: Sender<LogBatch>,
    metrics: Arc<MetricsCounters>,
) -> std::thread::JoinHandle<()> {
    let worker_id = config.id;

    std::thread::Builder::new()
        .name(format!("log-worker-{}", worker_id))
        .spawn(move || {
            // Pin to CPU core on Linux
            #[cfg(target_os = "linux")]
            {
                let core_ids = core_affinity::get_core_ids().unwrap_or_default();
                if !core_ids.is_empty() {
                    let core_id = core_ids[worker_id % core_ids.len()];
                    core_affinity::set_for_current(core_id);
                }
            }

            let worker = Worker::new(config, field_pool);
            worker.run(running, scenario_state, output_tx, metrics);
        })
        .expect("Failed to spawn worker thread")
}
