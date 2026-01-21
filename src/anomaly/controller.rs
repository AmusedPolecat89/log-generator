//! Lock-free anomaly controller.
//!
//! Provides atomic access to anomaly state for worker threads.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Lock-free anomaly controller.
///
/// All operations are atomic and suitable for concurrent access.
pub struct AnomalyController {
    /// Current error rate multiplier (fixed-point: value / 10000)
    error_rate: AtomicU32,
    /// Whether any anomaly is currently active
    active: AtomicBool,
    /// Latency multiplier (fixed-point: value / 100)
    latency_multiplier: AtomicU32,
}

impl AnomalyController {
    /// Create a new anomaly controller with default (inactive) state.
    pub fn new() -> Self {
        Self {
            error_rate: AtomicU32::new(100), // 1% default (100/10000)
            active: AtomicBool::new(false),
            latency_multiplier: AtomicU32::new(100), // 1.0x (100/100)
        }
    }

    /// Set the error rate (0.0-1.0).
    #[inline]
    pub fn set_error_rate(&self, rate: f32) {
        let fixed = (rate.clamp(0.0, 1.0) * 10000.0) as u32;
        self.error_rate.store(fixed, Ordering::Release);
    }

    /// Get the current error rate (0.0-1.0).
    #[inline(always)]
    pub fn get_error_rate(&self) -> f32 {
        self.error_rate.load(Ordering::Acquire) as f32 / 10000.0
    }

    /// Set the latency multiplier (1.0 = normal).
    #[inline]
    pub fn set_latency_multiplier(&self, multiplier: f32) {
        let fixed = (multiplier.max(0.0) * 100.0) as u32;
        self.latency_multiplier.store(fixed, Ordering::Release);
    }

    /// Get the current latency multiplier.
    #[inline(always)]
    pub fn get_latency_multiplier(&self) -> f32 {
        self.latency_multiplier.load(Ordering::Acquire) as f32 / 100.0
    }

    /// Set whether anomaly is active.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Release);
    }

    /// Check if anomaly is active.
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Update all state at once.
    #[inline]
    pub fn update(&self, error_rate: f32, latency_multiplier: f32, active: bool) {
        self.set_error_rate(error_rate);
        self.set_latency_multiplier(latency_multiplier);
        self.set_active(active);
    }

    /// Reset to default state.
    #[inline]
    pub fn reset(&self) {
        self.error_rate.store(100, Ordering::Release);
        self.latency_multiplier.store(100, Ordering::Release);
        self.active.store(false, Ordering::Release);
    }
}

impl Default for AnomalyController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_rate() {
        let controller = AnomalyController::new();

        controller.set_error_rate(0.5);
        assert!((controller.get_error_rate() - 0.5).abs() < 0.001);

        controller.set_error_rate(0.0);
        assert!((controller.get_error_rate() - 0.0).abs() < 0.001);

        controller.set_error_rate(1.0);
        assert!((controller.get_error_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_latency_multiplier() {
        let controller = AnomalyController::new();

        controller.set_latency_multiplier(2.5);
        assert!((controller.get_latency_multiplier() - 2.5).abs() < 0.01);
    }
}
