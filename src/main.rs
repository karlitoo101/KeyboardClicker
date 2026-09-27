//! Mechanical Keyboard Sound
//!
//! A lightweight Windows tray application that plays a mechanical keyboard
//! click on every physical keypress. It runs quietly in the background and is
//! controlled entirely from its system-tray icon.
//!
//! Privacy: the global keyboard hook only detects *that* a key was pressed (and
//! tracks currently-held keys in memory to skip auto-repeat). It never records,
//! stores, or transmits which keys you type.

// Hide the console window on Windows for a clean background app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod autostart;
mod config;
mod hook;
mod keymap;
mod soundpack;
mod state;
mod tray;

use anyhow::Result;

use config::Config;
use state::AppState;

fn main() -> Result<()> {
    // Load persisted settings (falls back to defaults if missing/corrupt).
    let config = Config::load().sanitized();

    // Shared, lock-free state read by the hook on every keypress.
    let state = AppState::new(config.enabled, config.volume_pct);

    // Start the audio engine on its own thread. If no output device is
    // available we continue running (the tray still works); sounds simply
    // won't play.
    match audio::start() {
        Ok(handle) => {
            // Start the global keyboard hook, giving it a handle to request
            // playback.
            hook::spawn(state.clone(), handle);
        }
        Err(_) => {
            // No audio device: carry on with a silent tray.
        }
    }

    // Registry-backed autostart store for the "Start with Windows" toggle.
    #[cfg(windows)]
    let store = autostart::win::RegistryRunKey::new();

    // Run the tray + event loop. Blocks until the user quits.
    tray::run(state, store, config)?;

    Ok(())
}
