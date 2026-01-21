//! Anomaly injection module.
//!
//! Provides lock-free anomaly state for workers.

pub mod controller;

pub use controller::AnomalyController;
