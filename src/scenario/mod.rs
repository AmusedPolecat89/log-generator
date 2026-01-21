//! Scenario execution module.
//!
//! Manages timeline events, rate transitions, and spike scheduling.

pub mod executor;
pub mod spikes;
pub mod timeline;

pub use executor::ScenarioExecutor;
