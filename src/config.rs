//! Persistent configuration stored as JSON in
//! `%APPDATA%\MechKeyboard\config.json`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Whether the click sound is currently active.
    pub enabled: bool,
    /// Volume as an integer percentage, 0..=100.
    pub volume_pct: u32,
    /// Whether the app should launch automatically at Windows login.
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            volume_pct: 75,
            autostart: false,
        }
    }
}

impl Config {
    /// Directory that holds the config file: `%APPDATA%\MechKeyboard`.
    fn dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("MechKeyboard"))
    }

    /// Full path to `config.json`.
    fn path() -> Option<PathBuf> {
        Self::dir().map(|d| d.join("config.json"))
    }

    /// Load config from disk. Returns defaults if the file is missing or
    /// cannot be parsed, so a corrupt file never blocks startup.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Config::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| {
                // Corrupt or incompatible file: fall back to defaults.
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    /// Persist config to disk, creating the directory if needed.
    pub fn save(&self) -> Result<()> {
        let dir = Self::dir().context("could not resolve config directory")?;
        std::fs::create_dir_all(&dir).context("could not create config directory")?;
        let path = dir.join("config.json");
        let json = serde_json::to_string_pretty(self).context("could not serialize config")?;
        std::fs::write(&path, json).context("could not write config file")?;
        Ok(())
    }

    /// Clamp volume into a valid step and range on load.
    pub fn sanitized(mut self) -> Self {
        self.volume_pct = self.volume_pct.min(100);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let c = Config::default();
        assert!(c.enabled);
        assert_eq!(c.volume_pct, 75);
        assert!(!c.autostart);
    }

    #[test]
    fn round_trip_serialization() {
        let c = Config {
            enabled: false,
            volume_pct: 50,
            autostart: true,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.enabled, c.enabled);
        assert_eq!(back.volume_pct, c.volume_pct);
        assert_eq!(back.autostart, c.autostart);
    }

    #[test]
    fn corrupt_json_yields_defaults_via_from_str() {
        // Simulate the load() fallback path for unparseable content.
        let parsed: Config = serde_json::from_str("{ not valid json")
            .unwrap_or_else(|_| Config::default());
        assert_eq!(parsed.volume_pct, Config::default().volume_pct);
    }

    #[test]
    fn sanitize_clamps_volume() {
        let c = Config {
            enabled: true,
            volume_pct: 250,
            autostart: false,
        }
        .sanitized();
        assert_eq!(c.volume_pct, 100);
    }
}
