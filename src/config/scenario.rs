//! Scenario script configuration for the log generator.
//!
//! Scenarios define a complete run timeline with rate changes, error spikes,
//! and anomaly events using an expressive TOML-based DSL.

use crate::templates::LogFormat;
use serde::{Deserialize, Deserializer, Serialize};
use std::time::Duration;

/// Root scenario configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Scenario metadata
    pub scenario: ScenarioMeta,

    /// Timeline events for rate/error changes
    #[serde(default)]
    pub timeline: Vec<TimelineEvent>,

    /// Error/latency spikes
    #[serde(default)]
    pub spikes: Vec<Spike>,

    /// Rate presets (low, medium, full)
    #[serde(default)]
    pub rates: RatePresets,

    /// Override format (optional, can be set via CLI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<LogFormat>,
}

/// Scenario metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMeta {
    /// Scenario name
    pub name: String,

    /// Total duration of the scenario
    #[serde(deserialize_with = "deserialize_duration")]
    pub total_duration: Duration,
}

/// A timeline event that changes rate or error configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// When this event triggers (from scenario start)
    #[serde(deserialize_with = "deserialize_duration")]
    pub at: Duration,

    /// Rate setting: "low", "medium", "full", "ramp_to_full", "ramp_to_low", etc.
    pub rate: RateSetting,

    /// Duration for ramp transitions (optional)
    #[serde(default, deserialize_with = "deserialize_optional_duration")]
    pub duration: Option<Duration>,

    /// Base error rate as percentage (0.0-100.0)
    #[serde(default)]
    pub error_rate: Option<f32>,
}

/// Rate setting - either a preset name or explicit values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RateSetting {
    /// Preset name: "low", "medium", "full", "ramp_to_full", etc.
    Preset(String),
    /// Explicit rate configuration
    Explicit {
        throughput_mb: u32,
        logs_per_sec: u64,
    },
}

impl RateSetting {
    /// Check if this is a ramp transition.
    pub fn is_ramp(&self) -> bool {
        match self {
            RateSetting::Preset(s) => s.starts_with("ramp_to_"),
            RateSetting::Explicit { .. } => false,
        }
    }

    /// Get the target preset for a ramp transition.
    pub fn ramp_target(&self) -> Option<&str> {
        match self {
            RateSetting::Preset(s) if s.starts_with("ramp_to_") => Some(&s[8..]),
            _ => None,
        }
    }
}

/// An error or latency spike.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spike {
    /// When this spike triggers
    #[serde(deserialize_with = "deserialize_duration")]
    pub at: Duration,

    /// Type of spike
    #[serde(rename = "type")]
    pub spike_type: SpikeType,

    /// Duration of the spike
    #[serde(deserialize_with = "deserialize_duration")]
    pub duration: Duration,

    /// Error rate during spike (for error_burst type)
    #[serde(default)]
    pub error_rate: Option<ErrorRateConfig>,

    /// Latency multiplier (for latency_spike type)
    #[serde(default)]
    pub latency_multiplier: Option<f32>,

    /// Repeat configuration for recurring spikes
    #[serde(default)]
    pub repeat: Option<RepeatConfig>,
}

/// Spike type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpikeType {
    /// Burst of error responses (4xx, 5xx)
    ErrorBurst,
    /// Increased response times
    LatencySpike,
    /// Unusual request patterns
    UnusualPatterns,
    /// Mixed anomalies
    Mixed,
}

/// Error rate configuration - can be a fixed value or a range.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ErrorRateConfig {
    /// Fixed error rate
    Fixed(f32),
    /// Random within range
    Range { min: f32, max: f32 },
}

impl ErrorRateConfig {
    /// Get the error rate (random if range).
    pub fn get(&self, rng: &mut fastrand::Rng) -> f32 {
        match self {
            ErrorRateConfig::Fixed(rate) => *rate,
            ErrorRateConfig::Range { min, max } => min + rng.f32() * (max - min),
        }
    }
}

/// Repeat configuration for recurring spikes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatConfig {
    /// Number of repetitions
    pub count: u32,
    /// Interval between repetitions
    #[serde(deserialize_with = "deserialize_duration")]
    pub interval: Duration,
    /// Random jitter to add/subtract from interval
    #[serde(default, deserialize_with = "deserialize_optional_duration")]
    pub jitter: Option<Duration>,
}

/// Rate presets configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatePresets {
    /// Trickle rate preset
    #[serde(default = "RatePreset::default_trickle")]
    pub trickle: RatePreset,
    /// Low rate preset
    #[serde(default = "RatePreset::default_low")]
    pub low: RatePreset,
    /// Medium rate preset
    #[serde(default = "RatePreset::default_medium")]
    pub medium: RatePreset,
    /// High rate preset
    #[serde(default = "RatePreset::default_high")]
    pub high: RatePreset,
    /// Full rate preset
    #[serde(default = "RatePreset::default_full")]
    pub full: RatePreset,
    /// Max rate preset
    #[serde(default = "RatePreset::default_max")]
    pub max: RatePreset,
}

impl Default for RatePresets {
    fn default() -> Self {
        Self {
            trickle: RatePreset::default_trickle(),
            low: RatePreset::default_low(),
            medium: RatePreset::default_medium(),
            high: RatePreset::default_high(),
            full: RatePreset::default_full(),
            max: RatePreset::default_max(),
        }
    }
}

impl RatePresets {
    /// Look up a preset by name.
    pub fn get(&self, name: &str) -> Option<&RatePreset> {
        match name {
            "trickle" => Some(&self.trickle),
            "low" => Some(&self.low),
            "medium" => Some(&self.medium),
            "high" => Some(&self.high),
            "full" => Some(&self.full),
            "max" => Some(&self.max),
            _ => None,
        }
    }
}

/// A rate preset with throughput targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatePreset {
    /// Target throughput in MB/s
    pub throughput_mb: u32,
    /// Target logs per second
    pub logs_per_sec: u64,
}

impl RatePreset {
    pub fn default_trickle() -> Self {
        Self {
            throughput_mb: 10,
            logs_per_sec: 50_000,
        }
    }

    pub fn default_low() -> Self {
        Self {
            throughput_mb: 100,
            logs_per_sec: 500_000,
        }
    }

    pub fn default_medium() -> Self {
        Self {
            throughput_mb: 500,
            logs_per_sec: 2_500_000,
        }
    }

    pub fn default_high() -> Self {
        Self {
            throughput_mb: 750,
            logs_per_sec: 3_750_000,
        }
    }

    pub fn default_full() -> Self {
        Self {
            throughput_mb: 1000,
            logs_per_sec: 5_000_000,
        }
    }

    pub fn default_max() -> Self {
        Self {
            throughput_mb: 2000,
            logs_per_sec: 10_000_000,
        }
    }

    /// Create a preset from a custom logs-per-second value.
    /// Throughput is estimated assuming ~200 bytes per log line.
    pub fn from_logs_per_sec(lps: u64) -> Self {
        let throughput_mb = ((lps as f64 * 200.0) / (1024.0 * 1024.0)).ceil() as u32;
        Self {
            throughput_mb,
            logs_per_sec: lps,
        }
    }
}

/// Deserialize a duration from a string like "1m30s", "10s", "2h".
fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    parse_duration(&s).map_err(serde::de::Error::custom)
}

/// Deserialize an optional duration.
fn deserialize_optional_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => parse_duration(&s).map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Parse a duration string like "1m30s", "10s", "2h", "500ms".
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty duration string".to_string());
    }

    let mut total_ms: u64 = 0;
    let mut current_num = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else if c.is_alphabetic() {
            if current_num.is_empty() {
                return Err(format!("Invalid duration format: {}", s));
            }
            let num: u64 = current_num
                .parse()
                .map_err(|_| format!("Invalid number in duration: {}", current_num))?;
            current_num.clear();

            // Peek ahead for "ms"
            let unit = c.to_ascii_lowercase();
            let multiplier = match unit {
                'h' => 3_600_000,
                'm' => {
                    // Could be 'm' for minutes or start of 'ms'
                    60_000
                }
                's' => 1_000,
                _ => return Err(format!("Unknown time unit: {}", c)),
            };
            total_ms += num * multiplier;
        }
    }

    // Handle trailing number (assumed seconds)
    if !current_num.is_empty() {
        let num: u64 = current_num
            .parse()
            .map_err(|_| format!("Invalid number in duration: {}", current_num))?;
        total_ms += num * 1000;
    }

    Ok(Duration::from_millis(total_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("1m").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_duration("1m30s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert_eq!(
            parse_duration("1h30m").unwrap(),
            Duration::from_secs(5400)
        );
    }

    #[test]
    fn test_scenario_parse() {
        let toml = r#"
[scenario]
name = "test"
total_duration = "10s"

[[timeline]]
at = "0s"
rate = "low"
error_rate = 1.0

[[spikes]]
at = "5s"
type = "error_burst"
duration = "2s"
error_rate = 50.0
"#;
        let scenario: Scenario = toml::from_str(toml).unwrap();
        assert_eq!(scenario.scenario.name, "test");
        assert_eq!(scenario.timeline.len(), 1);
        assert_eq!(scenario.spikes.len(), 1);
    }
}
