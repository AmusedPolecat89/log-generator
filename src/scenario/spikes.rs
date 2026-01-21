//! Spike scheduling and execution.
//!
//! Handles error bursts, latency spikes, and other anomalies.

use crate::config::scenario::{RepeatConfig, Spike, SpikeType};
use std::time::{Duration, Instant};

/// Scheduled spike instance (potentially repeated).
#[derive(Debug, Clone)]
pub struct ScheduledSpike {
    /// When this spike triggers (absolute)
    pub trigger_at: Instant,
    /// Duration of the spike
    pub duration: Duration,
    /// Type of spike
    pub spike_type: SpikeType,
    /// Error rate during spike
    pub error_rate: f32,
    /// Latency multiplier
    pub latency_multiplier: f32,
}

/// Active spike state.
#[derive(Debug, Clone)]
pub struct ActiveSpike {
    /// When this spike ends
    pub ends_at: Instant,
    /// Type of spike
    pub spike_type: SpikeType,
    /// Error rate override
    pub error_rate: f32,
    /// Latency multiplier
    pub latency_multiplier: f32,
}

/// Spike scheduler manages upcoming and active spikes.
pub struct SpikeScheduler {
    /// Scheduled spikes (sorted by trigger time)
    scheduled: Vec<ScheduledSpike>,
    /// Currently active spikes
    active: Vec<ActiveSpike>,
    /// Index of next spike to check
    next_spike_idx: usize,
    /// RNG for randomized spikes
    rng: fastrand::Rng,
}

impl SpikeScheduler {
    /// Create a new spike scheduler from scenario spikes.
    pub fn new(spikes: &[Spike], scenario_start: Instant) -> Self {
        let mut rng = fastrand::Rng::new();
        let mut scheduled = Vec::new();

        for spike in spikes {
            let base_time = scenario_start + spike.at;

            // Generate spike instances (handling repeats)
            if let Some(repeat) = &spike.repeat {
                Self::schedule_repeated_spikes(&mut scheduled, spike, base_time, repeat, &mut rng);
            } else {
                // Single spike
                scheduled.push(ScheduledSpike {
                    trigger_at: base_time,
                    duration: spike.duration,
                    spike_type: spike.spike_type,
                    error_rate: spike
                        .error_rate
                        .as_ref()
                        .map(|e| e.get(&mut rng) / 100.0)
                        .unwrap_or(0.5),
                    latency_multiplier: spike.latency_multiplier.unwrap_or(1.0),
                });
            }
        }

        // Sort by trigger time
        scheduled.sort_by_key(|s| s.trigger_at);

        Self {
            scheduled,
            active: Vec::new(),
            next_spike_idx: 0,
            rng,
        }
    }

    /// Schedule repeated spikes.
    fn schedule_repeated_spikes(
        scheduled: &mut Vec<ScheduledSpike>,
        spike: &Spike,
        base_time: Instant,
        repeat: &RepeatConfig,
        rng: &mut fastrand::Rng,
    ) {
        let mut time = base_time;

        for _ in 0..repeat.count {
            // Add jitter if configured
            let jittered_time = if let Some(jitter) = repeat.jitter {
                let jitter_ms = jitter.as_millis() as i64;
                let offset = rng.i64(-jitter_ms..=jitter_ms);
                if offset >= 0 {
                    time + Duration::from_millis(offset as u64)
                } else {
                    time.checked_sub(Duration::from_millis((-offset) as u64))
                        .unwrap_or(time)
                }
            } else {
                time
            };

            scheduled.push(ScheduledSpike {
                trigger_at: jittered_time,
                duration: spike.duration,
                spike_type: spike.spike_type,
                error_rate: spike
                    .error_rate
                    .as_ref()
                    .map(|e| e.get(rng) / 100.0)
                    .unwrap_or(0.5),
                latency_multiplier: spike.latency_multiplier.unwrap_or(1.0),
            });

            time += repeat.interval;
        }
    }

    /// Update scheduler state. Call this every tick.
    /// Returns the current error rate modifier and latency multiplier.
    pub fn update(&mut self, now: Instant) -> SpikeEffect {
        // Check for new spikes to activate
        while self.next_spike_idx < self.scheduled.len() {
            let spike = &self.scheduled[self.next_spike_idx];
            if now >= spike.trigger_at {
                self.active.push(ActiveSpike {
                    ends_at: spike.trigger_at + spike.duration,
                    spike_type: spike.spike_type,
                    error_rate: spike.error_rate,
                    latency_multiplier: spike.latency_multiplier,
                });
                self.next_spike_idx += 1;
            } else {
                break;
            }
        }

        // Remove expired spikes
        self.active.retain(|s| now < s.ends_at);

        // Calculate combined effect
        if self.active.is_empty() {
            SpikeEffect::default()
        } else {
            let mut effect = SpikeEffect::default();

            for spike in &self.active {
                match spike.spike_type {
                    SpikeType::ErrorBurst | SpikeType::Mixed => {
                        // Use maximum error rate from active spikes
                        effect.error_rate = effect.error_rate.max(spike.error_rate);
                    }
                    SpikeType::LatencySpike => {
                        effect.latency_multiplier =
                            effect.latency_multiplier.max(spike.latency_multiplier);
                    }
                    SpikeType::UnusualPatterns => {
                        effect.unusual_patterns = true;
                    }
                }

                if spike.spike_type == SpikeType::Mixed {
                    effect.latency_multiplier =
                        effect.latency_multiplier.max(spike.latency_multiplier);
                }
            }

            effect.is_active = true;
            effect
        }
    }

    /// Check if any spike is currently active.
    pub fn is_spike_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// Get description of active spikes.
    pub fn active_description(&self) -> String {
        if self.active.is_empty() {
            "none".to_string()
        } else {
            self.active
                .iter()
                .map(|s| format!("{:?}", s.spike_type))
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

/// Combined effect of all active spikes.
#[derive(Debug, Clone, Default)]
pub struct SpikeEffect {
    /// Whether any spike is active
    pub is_active: bool,
    /// Error rate override (0.0-1.0)
    pub error_rate: f32,
    /// Latency multiplier
    pub latency_multiplier: f32,
    /// Whether unusual patterns mode is active
    pub unusual_patterns: bool,
}
