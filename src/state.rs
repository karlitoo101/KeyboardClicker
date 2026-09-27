//! Shared application state accessible from both the tray (main thread) and the
//! keyboard-hook thread. Uses atomics so reads on the hot path (every keypress)
//! are lock-free and cheap.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Runtime state shared across threads.
///
/// `volume` is stored as an integer percentage (0..=100) in an `AtomicU32`
/// so it can be updated and read without locking.
pub struct AppState {
    enabled: AtomicBool,
    volume_pct: AtomicU32,
}

impl AppState {
    pub fn new(enabled: bool, volume_pct: u32) -> Arc<Self> {
        Arc::new(Self {
            enabled: AtomicBool::new(enabled),
            volume_pct: AtomicU32::new(volume_pct.min(100)),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::Relaxed);
    }

    /// Volume as an integer percentage, 0..=100.
    pub fn volume_pct(&self) -> u32 {
        self.volume_pct.load(Ordering::Relaxed)
    }

    /// Volume as a linear gain factor in the range 0.0..=1.0 for rodio.
    pub fn volume_gain(&self) -> f32 {
        self.volume_pct() as f32 / 100.0
    }

    pub fn set_volume_pct(&self, value: u32) {
        self.volume_pct.store(value.min(100), Ordering::Relaxed);
    }
}
