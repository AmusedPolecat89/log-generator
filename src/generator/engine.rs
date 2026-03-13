//! Core generation engine.
//!
//! Orchestrates workers, scenario execution, and output.

use crate::config::scenario::Scenario;
use crate::fields::FieldPool;
use crate::generator::worker::{spawn_worker, LogBatch, WorkerConfig};
use crate::output::metrics::{MetricsCounters, MetricsDisplay};
use crate::output::{create_writer, OutputConfig};
use crate::scenario::executor::ScenarioExecutor;
use crate::templates::LogFormat;
use crossbeam::channel::{bounded, TryRecvError};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// Default batch size (logs per batch).
const DEFAULT_BATCH_SIZE: usize = 10_000;

/// Channel buffer size (number of batches).
const CHANNEL_BUFFER_SIZE: usize = 64;

/// Log generation engine.
pub struct Engine {
    scenario: Scenario,
    output_config: OutputConfig,
    num_workers: usize,
    show_metrics: bool,
}

impl Engine {
    /// Create a new engine.
    pub fn new(
        scenario: Scenario,
        output_config: OutputConfig,
        num_workers: usize,
        show_metrics: bool,
    ) -> Self {
        Self {
            scenario,
            output_config,
            num_workers,
            show_metrics,
        }
    }

    /// Run the generator.
    pub fn run(&mut self, running: Arc<AtomicBool>) -> io::Result<()> {
        // Create shared resources
        let field_pool = Arc::new(FieldPool::new());
        let metrics = Arc::new(MetricsCounters::new());

        // Create scenario executor
        let mut executor = ScenarioExecutor::new(self.scenario.clone());
        let scenario_state = executor.shared_state();

        // Create output channel
        let (tx, rx) = bounded::<LogBatch>(CHANNEL_BUFFER_SIZE);

        // Create output writer
        let mut writer = create_writer(&self.output_config)?;

        // Determine log format
        let format = self.scenario.format.unwrap_or(LogFormat::Apache);

        // Spawn worker threads
        let workers: Vec<JoinHandle<()>> = (0..self.num_workers)
            .map(|id| {
                let config = WorkerConfig {
                    id,
                    batch_size: DEFAULT_BATCH_SIZE,
                    format,
                };
                spawn_worker(
                    config,
                    Arc::clone(&field_pool),
                    Arc::clone(&running),
                    Arc::clone(&scenario_state),
                    tx.clone(),
                    Arc::clone(&metrics),
                )
            })
            .collect();

        // Drop our sender so channel closes when workers finish
        drop(tx);

        // Create metrics display
        let mut display = if self.show_metrics {
            Some(MetricsDisplay::new(Arc::clone(&metrics)))
        } else {
            None
        };

        // Main loop: process output and update scenario
        let tick_interval = Duration::from_millis(10);
        let mut last_tick = std::time::Instant::now();

        loop {
            // Update scenario state
            let now = std::time::Instant::now();
            if now.duration_since(last_tick) >= tick_interval {
                if !executor.tick() {
                    // Scenario complete
                    running.store(false, Ordering::SeqCst);
                }
                last_tick = now;

                // Update metrics display
                if let Some(ref mut display) = display {
                    display.maybe_display(
                        executor.progress_percent(),
                        &executor.rate_description(),
                        &executor.spike_description(),
                    );
                }
            }

            // Drain all available batches from the channel
            let mut drained = false;
            let mut disconnected = false;
            loop {
                match rx.try_recv() {
                    Ok(batch) => {
                        writer.write_batch(&batch.data)?;
                        drained = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                break;
            }

            // Only block-wait if nothing was drained (avoids busy-spin)
            if !drained {
                match rx.recv_timeout(Duration::from_millis(1)) {
                    Ok(batch) => {
                        writer.write_batch(&batch.data)?;
                    }
                    Err(crossbeam::channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            // Check if we should stop
            if !running.load(Ordering::Relaxed) && rx.is_empty() {
                break;
            }
        }

        // Wait for workers to finish
        for handle in workers {
            let _ = handle.join();
        }

        // Flush output
        writer.flush()?;

        // Show final summary
        if let Some(display) = display {
            display.display_summary();
        }

        Ok(())
    }
}
