//! Field generators for log entries.
//!
//! This module provides pre-generated pools of field values to avoid
//! runtime allocations in the hot path.

pub mod ip;
pub mod path;
pub mod pool;
pub mod status;
pub mod timestamp;
pub mod user_agent;

pub use pool::FieldPool;
