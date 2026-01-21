//! Log generation engine.
//!
//! Multi-threaded log generation with worker threads.

pub mod engine;
pub mod worker;

pub use engine::Engine;
