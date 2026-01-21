//! Timeline event processing.
//!
//! Handles rate changes and transitions over time.

use crate::config::scenario::{RatePreset, RatePresets, RateSetting, TimelineEvent};
use std::time::{Duration, Instant};

/// Current state of the timeline.
#[derive(Debug, Clone)]
pub struct TimelineState {
    /// Current target logs per second
    pub target_logs_per_sec: u64,
    /// Current target throughput in bytes per second
    pub target_bytes_per_sec: u64,
    /// Current base error rate (0.0-1.0)
    pub error_rate: f32,
    /// Whether we're currently ramping
    pub is_ramping: bool,
    /// Ramp start values (if ramping)
    ramp_start_logs_per_sec: u64,
    ramp_start_bytes_per_sec: u64,
    /// Ramp end values (if ramping)
    ramp_end_logs_per_sec: u64,
    ramp_end_bytes_per_sec: u64,
    /// Ramp timing
    ramp_start_time: Instant,
    ramp_duration: Duration,
}

impl TimelineState {
    /// Create initial timeline state from presets.
    pub fn new(presets: &RatePresets) -> Self {
        Self {
            target_logs_per_sec: presets.low.logs_per_sec,
            target_bytes_per_sec: (presets.low.throughput_mb as u64) * 1024 * 1024,
            error_rate: 0.01, // 1% default
            is_ramping: false,
            ramp_start_logs_per_sec: 0,
            ramp_start_bytes_per_sec: 0,
            ramp_end_logs_per_sec: 0,
            ramp_end_bytes_per_sec: 0,
            ramp_start_time: Instant::now(),
            ramp_duration: Duration::ZERO,
        }
    }

    /// Apply a timeline event.
    pub fn apply_event(&mut self, event: &TimelineEvent, presets: &RatePresets, now: Instant) {
        // Set error rate if specified
        if let Some(rate) = event.error_rate {
            self.error_rate = rate / 100.0; // Convert from percentage
        }

        // Handle rate setting
        match &event.rate {
            RateSetting::Preset(name) => {
                if name.starts_with("ramp_to_") {
                    // Start a ramp transition
                    let target_name = &name[8..]; // Skip "ramp_to_"
                    let target = get_preset(target_name, presets);
                    let duration = event.duration.unwrap_or(Duration::from_secs(30));

                    self.start_ramp(target, duration, now);
                } else {
                    // Immediate switch
                    let preset = get_preset(name, presets);
                    self.set_rate(preset);
                    self.is_ramping = false;
                }
            }
            RateSetting::Explicit {
                throughput_mb,
                logs_per_sec,
            } => {
                self.target_logs_per_sec = *logs_per_sec;
                self.target_bytes_per_sec = (*throughput_mb as u64) * 1024 * 1024;
                self.is_ramping = false;
            }
        }
    }

    /// Start a ramp transition.
    fn start_ramp(&mut self, target: &RatePreset, duration: Duration, now: Instant) {
        self.is_ramping = true;
        self.ramp_start_logs_per_sec = self.target_logs_per_sec;
        self.ramp_start_bytes_per_sec = self.target_bytes_per_sec;
        self.ramp_end_logs_per_sec = target.logs_per_sec;
        self.ramp_end_bytes_per_sec = (target.throughput_mb as u64) * 1024 * 1024;
        self.ramp_start_time = now;
        self.ramp_duration = duration;
    }

    /// Set rate immediately from preset.
    fn set_rate(&mut self, preset: &RatePreset) {
        self.target_logs_per_sec = preset.logs_per_sec;
        self.target_bytes_per_sec = (preset.throughput_mb as u64) * 1024 * 1024;
    }

    /// Update ramp progress (call every tick).
    pub fn update_ramp(&mut self, now: Instant) {
        if !self.is_ramping {
            return;
        }

        let elapsed = now.duration_since(self.ramp_start_time);
        if elapsed >= self.ramp_duration {
            // Ramp complete
            self.target_logs_per_sec = self.ramp_end_logs_per_sec;
            self.target_bytes_per_sec = self.ramp_end_bytes_per_sec;
            self.is_ramping = false;
        } else {
            // Linear interpolation
            let progress = elapsed.as_secs_f64() / self.ramp_duration.as_secs_f64();

            self.target_logs_per_sec = lerp_u64(
                self.ramp_start_logs_per_sec,
                self.ramp_end_logs_per_sec,
                progress,
            );
            self.target_bytes_per_sec = lerp_u64(
                self.ramp_start_bytes_per_sec,
                self.ramp_end_bytes_per_sec,
                progress,
            );
        }
    }

    /// Get description of current rate for display.
    pub fn rate_description(&self) -> String {
        let mb_per_sec = self.target_bytes_per_sec / (1024 * 1024);
        if self.is_ramping {
            format!("ramping to {} MB/s", mb_per_sec)
        } else {
            format!("{} MB/s", mb_per_sec)
        }
    }
}

/// Get a rate preset by name.
fn get_preset<'a>(name: &str, presets: &'a RatePresets) -> &'a RatePreset {
    match name {
        "low" => &presets.low,
        "medium" => &presets.medium,
        "full" => &presets.full,
        _ => &presets.medium, // Default to medium for unknown
    }
}

/// Linear interpolation for u64.
#[inline]
fn lerp_u64(start: u64, end: u64, t: f64) -> u64 {
    if end >= start {
        start + ((end - start) as f64 * t) as u64
    } else {
        start - ((start - end) as f64 * t) as u64
    }
}
