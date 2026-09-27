//! Parses a Mechvibes-format sound pack: a `config.json` describing a sprite
//! map plus a single audio file (`sound.ogg`/`sound.wav`) containing every
//! key's recording laid out sequentially.
//!
//! Config shape (only the fields we use):
//! ```json
//! {
//!   "sound": "sound.ogg",
//!   "key_define_type": "single",
//!   "defines": { "57": [start_ms, duration_ms], ... }
//! }
//! ```

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// A single sprite slice: where in the audio file it starts and how long it
/// plays, both in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slice {
    pub start_ms: u32,
    pub duration_ms: u32,
}

/// Parsed sound-pack metadata: the audio file name and the keycode->slice map.
#[derive(Debug, Clone)]
pub struct SoundPack {
    pub sound_file: String,
    slices: HashMap<u32, Slice>,
}

/// Raw JSON mirror used only for deserialization.
#[derive(Deserialize)]
struct RawConfig {
    sound: String,
    #[serde(default)]
    defines: HashMap<String, Option<(f64, f64)>>,
}

impl SoundPack {
    /// Parse a Mechvibes `config.json` from its text.
    pub fn from_json(text: &str) -> Result<Self> {
        let raw: RawConfig =
            serde_json::from_str(text).map_err(|e| anyhow!("invalid sound pack config: {e}"))?;

        let mut slices = HashMap::new();
        for (code_str, val) in raw.defines {
            // Entries can be null in some packs; skip those.
            let Some((start, dur)) = val else { continue };
            let Ok(code) = code_str.parse::<u32>() else {
                continue;
            };
            slices.insert(
                code,
                Slice {
                    start_ms: start.max(0.0) as u32,
                    duration_ms: dur.max(0.0) as u32,
                },
            );
        }

        if slices.is_empty() {
            return Err(anyhow!("sound pack defined no key slices"));
        }

        Ok(SoundPack {
            sound_file: raw.sound,
            slices,
        })
    }

    /// Look up the slice for a Mechvibes key code.
    pub fn slice(&self, code: u32) -> Option<Slice> {
        self.slices.get(&code).copied()
    }

    /// Iterate over every `(key_code, slice)` pair, for pre-computing clips.
    pub fn slices(&self) -> impl Iterator<Item = (u32, Slice)> + '_ {
        self.slices.iter().map(|(&code, &slice)| (code, slice))
    }

    /// A reasonable default slice for keys not present in the map: prefer the
    /// spacebar (57), else any entry.
    pub fn default_slice(&self) -> Option<Slice> {
        self.slice(57).or_else(|| self.slices.values().next().copied())
    }

    pub fn len(&self) -> usize {
        self.slices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slices.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "name": "Test",
        "sound": "sound.ogg",
        "key_define_type": "single",
        "defines": {
            "57": [1000, 200],
            "28": [2000, 180],
            "30": [3000, 190],
            "99": null
        }
    }"#;

    #[test]
    fn parses_defines_and_sound_file() {
        let pack = SoundPack::from_json(SAMPLE).unwrap();
        assert_eq!(pack.sound_file, "sound.ogg");
        assert_eq!(pack.len(), 3); // null entry skipped
        assert_eq!(
            pack.slice(57),
            Some(Slice {
                start_ms: 1000,
                duration_ms: 200
            })
        );
        assert_eq!(
            pack.slice(28),
            Some(Slice {
                start_ms: 2000,
                duration_ms: 180
            })
        );
    }

    #[test]
    fn default_slice_prefers_space() {
        let pack = SoundPack::from_json(SAMPLE).unwrap();
        assert_eq!(pack.default_slice().unwrap().start_ms, 1000);
    }

    #[test]
    fn unknown_code_is_none() {
        let pack = SoundPack::from_json(SAMPLE).unwrap();
        assert_eq!(pack.slice(12345), None);
    }

    #[test]
    fn empty_defines_errors() {
        let bad = r#"{ "sound": "s.ogg", "defines": {} }"#;
        assert!(SoundPack::from_json(bad).is_err());
    }

}
