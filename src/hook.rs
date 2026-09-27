//! Global keyboard hook. Runs `rdev::listen` on a dedicated thread and plays a
//! click on each *physical* key press, filtering out OS auto-repeat events.
//!
//! Privacy: this module never records, stores, or transmits which keys are
//! pressed. It only tracks the set of currently-held keys in memory so it can
//! distinguish a fresh press from an auto-repeat, and it discards that
//! information as soon as the key is released.

use crate::audio::AudioHandle;
use crate::state::AppState;
use rdev::{listen, Event, EventType, Key};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Tracks which keys are currently held so auto-repeat can be ignored.
#[derive(Default)]
pub struct RepeatFilter {
    held: HashSet<Key>,
}

impl RepeatFilter {
    pub fn new() -> Self {
        Self {
            held: HashSet::new(),
        }
    }

    /// Handle a key-down. Returns `true` if this is a fresh press that should
    /// produce a sound, or `false` if it is an auto-repeat of a held key.
    pub fn on_press(&mut self, key: Key) -> bool {
        // `insert` returns true if the key was NOT already present.
        self.held.insert(key)
    }

    /// Handle a key-up, clearing the held state for that key. Returns `true` if
    /// the key was actually held (so its release sound should play).
    pub fn on_release(&mut self, key: Key) -> bool {
        self.held.remove(&key)
    }
}

/// Spawn the background listener thread. Never returns a handle we join on;
/// the thread lives for the duration of the process.
pub fn spawn(state: Arc<AppState>, audio: AudioHandle) {
    std::thread::spawn(move || {
        let filter = Arc::new(Mutex::new(RepeatFilter::new()));

        let callback = move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    let is_fresh = {
                        let mut f = filter.lock().unwrap();
                        f.on_press(key)
                    };
                    if is_fresh && state.is_enabled() {
                        // Map the key to its Mechvibes code; unmapped keys fall
                        // back to a default slice inside the audio engine.
                        // Play only the press (attack) portion on key-down.
                        let code = crate::keymap::code_for(key);
                        audio.play_press(code, state.volume_gain());
                    }
                }
                EventType::KeyRelease(key) => {
                    // Clear the held state so the next press of this key counts
                    // as fresh. No sound plays on release: the whole click
                    // already fired on key-down for minimal latency.
                    let mut f = filter.lock().unwrap();
                    f.on_release(key);
                }
                _ => {}
            }
        };

        // `listen` blocks; if it errors (e.g. hook could not be installed) the
        // thread simply ends and the app keeps running without sounds.
        let _ = listen(callback);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_press_plays_repeat_does_not() {
        let mut f = RepeatFilter::new();
        // First press of a key is fresh.
        assert!(f.on_press(Key::KeyA));
        // Held-key auto-repeat should be filtered.
        assert!(!f.on_press(Key::KeyA));
        assert!(!f.on_press(Key::KeyA));
    }

    #[test]
    fn release_allows_next_press_to_play_again() {
        let mut f = RepeatFilter::new();
        assert!(f.on_press(Key::KeyB));
        // Releasing a held key reports that it was held (release sound plays).
        assert!(f.on_release(Key::KeyB));
        // After release, the next press is fresh again.
        assert!(f.on_press(Key::KeyB));
    }

    #[test]
    fn stray_release_reports_not_held() {
        let mut f = RepeatFilter::new();
        // Releasing a key that was never pressed should report false so no
        // release sound fires.
        assert!(!f.on_release(Key::KeyZ));
    }

    #[test]
    fn independent_keys_each_play() {
        let mut f = RepeatFilter::new();
        assert!(f.on_press(Key::KeyA));
        assert!(f.on_press(Key::KeyS));
        assert!(f.on_press(Key::KeyD));
        // Repeats of each are filtered independently.
        assert!(!f.on_press(Key::KeyA));
        assert!(!f.on_press(Key::KeyS));
    }
}
