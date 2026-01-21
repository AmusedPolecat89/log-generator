//! Scenario executor - orchestrates timeline and spikes.

use super::spikes::{SpikeEffect, SpikeScheduler};
use super::timeline::TimelineState;
use crate::config::scenario::Scenario;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Shared state accessible by all workers (lock-free).
pub struct SharedScenarioState {
    /// Target logs per second (workers read this)
    pub target_logs_per_sec: AtomicU64,
    /// Current error rate as fixed-point (rate * 10000)
    pub error_rate_fp: AtomicU32,
    /// Whether a spike is active
    pub spike_active: std::sync::atomic::AtomicBool,
}

impl SharedScenarioState {
    pub fn new() -> Self {
        Self {
            target_logs_per_sec: AtomicU64::new(0),
            error_rate_fp: AtomicU32::new(100), // 1% default
            spike_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Get the current error rate (0.0-1.0).
    #[inline(always)]
    pub fn get_error_rate(&self) -> f32 {
        self.error_rate_fp.load(Ordering::Relaxed) as f32 / 10000.0
    }

    /// Get the target logs per second.
    #[inline(always)]
    pub fn get_target_logs_per_sec(&self) -> u64 {
        self.target_logs_per_sec.load(Ordering::Relaxed)
    }

    /// Update from timeline state.
    fn update(&self, timeline: &TimelineState, spike_effect: &SpikeEffect) {
        self.target_logs_per_sec
            .store(timeline.target_logs_per_sec, Ordering::Relaxed);

        // Combine base error rate with spike effect
        let effective_error_rate = if spike_effect.is_active {
            spike_effect.error_rate.max(timeline.error_rate)
        } else {
            timeline.error_rate
        };

        self.error_rate_fp
            .store((effective_error_rate * 10000.0) as u32, Ordering::Relaxed);
        self.spike_active
            .store(spike_effect.is_active, Ordering::Relaxed);
    }
}

impl Default for SharedScenarioState {
    fn default() -> Self {
        Self::new()
    }
}

/// Scenario executor manages the scenario timeline.
pub struct ScenarioExecutor {
    /// The scenario configuration
    scenario: Scenario,
    /// Scenario start time
    start_time: Instant,
    /// Total duration
    total_duration: Duration,
    /// Timeline state
    timeline: TimelineState,
    /// Spike scheduler
    spike_scheduler: SpikeScheduler,
    /// Index of next timeline event to process
    next_event_idx: usize,
    /// Shared state for workers
    shared_state: Arc<SharedScenarioState>,
}

impl ScenarioExecutor {
    /// Create a new scenario executor.
    pub fn new(scenario: Scenario) -> Self {
        let start_time = Instant::now();
        let total_duration = scenario.scenario.total_duration;
        let timeline = TimelineState::new(&scenario.rates);
        let spike_scheduler = SpikeScheduler::new(&scenario.spikes, start_time);
        let shared_state = Arc::new(SharedScenarioState::new());

        let mut executor = Self {
            scenario,
            start_time,
            total_duration,
            timeline,
            spike_scheduler,
            next_event_idx: 0,
            shared_state,
        };

        // Process any events at time 0
        executor.tick();

        executor
    }

    /// Get shared state reference for workers.
    pub fn shared_state(&self) -> Arc<SharedScenarioState> {
        Arc::clone(&self.shared_state)
    }

    /// Update scenario state. Call this regularly (e.g., every 10ms).
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start_time);

        // Check if scenario is complete
        if elapsed >= self.total_duration {
            return false;
        }

        // Process timeline events
        while self.next_event_idx < self.scenario.timeline.len() {
            let event = &self.scenario.timeline[self.next_event_idx];
            if elapsed >= event.at {
                self.timeline
                    .apply_event(event, &self.scenario.rates, now);
                self.next_event_idx += 1;
            } else {
                break;
            }
        }

        // Update ramp progress
        self.timeline.update_ramp(now);

        // Update spike scheduler
        let spike_effect = self.spike_scheduler.update(now);

        // Update shared state
        self.shared_state.update(&self.timeline, &spike_effect);

        true
    }

    /// Get elapsed time.
    pub fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self.start_time)
    }

    /// Get remaining time.
    pub fn remaining(&self) -> Duration {
        let elapsed = self.elapsed();
        if elapsed >= self.total_duration {
            Duration::ZERO
        } else {
            self.total_duration - elapsed
        }
    }

    /// Get progress as percentage (0-100).
    pub fn progress_percent(&self) -> f32 {
        let elapsed = self.elapsed().as_secs_f32();
        let total = self.total_duration.as_secs_f32();
        (elapsed / total * 100.0).min(100.0)
    }

    /// Get current rate description.
    pub fn rate_description(&self) -> String {
        self.timeline.rate_description()
    }

    /// Get current spike description.
    pub fn spike_description(&self) -> String {
        self.spike_scheduler.active_description()
    }

    /// Check if a spike is currently active.
    pub fn is_spike_active(&self) -> bool {
        self.spike_scheduler.is_spike_active()
    }

    /// Get total duration.
    pub fn total_duration(&self) -> Duration {
        self.total_duration
    }
}
