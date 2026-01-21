//! Hyper-optimized log generator capable of 1GB+/s throughput.
//!
//! This library provides a high-performance log generation engine with:
//! - Multi-threaded generation (one worker per CPU core)
//! - Pre-allocated field pools for zero-allocation hot paths
//! - Lock-free configuration updates via arc-swap
//! - Expressive scenario scripting for rate control and anomaly injection

pub mod anomaly;
pub mod config;
pub mod fields;
pub mod generator;
pub mod output;
pub mod scenario;
pub mod templates;

pub use config::scenario::{RatePreset, Scenario, Spike, SpikeType, TimelineEvent};
pub use generator::engine::Engine;
pub use templates::LogFormat;
