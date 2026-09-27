//! Audio engine for Mechvibes-style sound packs.
//!
//! The whole sound file (e.g. `sound.ogg`) is decoded once into an in-memory
//! PCM buffer. Each keypress plays a *slice* of that buffer identified by a
//! start offset and duration in milliseconds (from the pack's `config.json`),
//! so there is no per-key disk I/O or decoding.
//!
//! Why a dedicated thread: rodio's `OutputStream` (and the underlying
//! `cpal::Stream`) is neither `Send` nor `Sync`, so it cannot be moved into or
//! shared with the keyboard-hook thread. The stream stays pinned to one thread,
//! and other threads talk to it through a `Send`-safe channel handle.

use crate::soundpack::{Slice, SoundPack};
use anyhow::{anyhow, Context, Result};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Source};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

/// Embedded default sound pack so the app is self-contained.
static EMBEDDED_SOUND: &[u8] = include_bytes!("../assets/sound.ogg");
static EMBEDDED_CONFIG: &str = include_str!("../assets/soundpack.json");

/// A ready-to-play click: interleaved f32 PCM already sliced out of the sound
/// file at load time. Wrapped in `Arc` so the hot path only clones a pointer.
type Clip = Arc<[f32]>;

/// Only the first portion of each key's slice is used: the sharp attack/click
/// (the "press half" of the old version). The tail is dropped so every key is
/// a short, punchy click with no lingering release sound.
const PRESS_MS: u32 = 60;

/// A playback request: the pre-sliced clip to play and its linear gain.
struct PlayRequest {
    clip: Clip,
    gain: f32,
}

/// A cheap, `Send + Clone` handle used by other threads to request playback.
///
/// All the per-key slicing and format conversion happens once up front, so the
/// keypress path is just a map lookup, an `Arc` clone, and a channel send — no
/// allocation, decoding, or sample copying between keydown and sound.
#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<PlayRequest>,
    clips: Arc<ClipCache>,
}

/// Pre-computed clips keyed by Mechvibes key code, plus a default fallback.
struct ClipCache {
    by_code: HashMap<u32, Clip>,
    default: Option<Clip>,
}

impl ClipCache {
    fn clip_for(&self, key_code: Option<u32>) -> Option<&Clip> {
        key_code
            .and_then(|c| self.by_code.get(&c))
            .or(self.default.as_ref())
    }
}

impl AudioHandle {
    /// Play a key's click on key-down. The *entire* clip fires immediately so
    /// the click is a single, crisp sound with no artificial split or delay.
    /// `gain` is linear volume 0.0..=1.0.
    pub fn play_press(&self, key_code: Option<u32>, gain: f32) {
        if gain <= 0.0 {
            return;
        }
        if let Some(clip) = self.clips.clip_for(key_code) {
            if !clip.is_empty() {
                let _ = self.tx.send(PlayRequest {
                    clip: clip.clone(),
                    gain,
                });
            }
        }
    }
}

/// Fully decoded PCM plus format info.
struct DecodedAudio {
    channels: u16,
    sample_rate: u32,
    /// Interleaved f32 samples for the entire sound file.
    data: Vec<f32>,
}

impl DecodedAudio {
    /// Extract the interleaved samples for a time slice as a ready-to-play clip.
    ///
    /// Offsets are converted from milliseconds to interleaved-sample indices
    /// using `sample_rate * channels`, then clamped to the buffer bounds.
    fn slice_clip(&self, slice: Slice) -> Clip {
        let frames_per_ms = self.sample_rate as f64 / 1000.0;
        let start_frame = (slice.start_ms as f64 * frames_per_ms) as usize;
        let len_frames = (slice.duration_ms as f64 * frames_per_ms) as usize;

        let ch = self.channels.max(1) as usize;
        let start_idx = (start_frame * ch).min(self.data.len());
        let end_idx = (start_idx + len_frames * ch).min(self.data.len());
        Arc::from(&self.data[start_idx..end_idx])
    }
}

/// Start the audio engine on a dedicated thread and return a playback handle.
///
/// Loads the sound pack (custom files next to the exe if present, otherwise the
/// embedded default), decodes it, and pre-slices every key's clip so playback
/// is allocation-free.
pub fn start() -> Result<AudioHandle> {
    let (pack, sound_bytes) = load_pack()?;
    let decoded = decode(sound_bytes)?;
    let clips = Arc::new(build_clips(&pack, &decoded));
    let channels = decoded.channels;
    let sample_rate = decoded.sample_rate;

    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let (play_tx, play_rx) = mpsc::channel::<PlayRequest>();

    std::thread::Builder::new()
        .name("audio".into())
        .spawn(move || audio_thread(channels, sample_rate, play_rx, ready_tx))
        .context("failed to spawn audio thread")?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(AudioHandle {
            tx: play_tx,
            clips,
        }),
        Ok(Err(e)) => Err(anyhow!(e)),
        Err(_) => Err(anyhow!("audio thread exited before initialization")),
    }
}

/// Trim a slice to its first `PRESS_MS` (the attack/press half), clamped so a
/// slice already shorter than that is used whole.
fn press_slice(slice: Slice) -> Slice {
    Slice {
        start_ms: slice.start_ms,
        duration_ms: slice.duration_ms.min(PRESS_MS),
    }
}

/// Samples below this absolute amplitude (on the -1.0..=1.0 f32 scale) are
/// treated as silence when trimming a clip's lead-in.
const SILENCE_THRESHOLD: f32 = 0.01;

/// Drop leading near-silent frames so the audible click starts immediately.
///
/// Mechvibes slices often begin a few milliseconds before the actual key
/// transient; those quiet samples are pure latency you hear on every press.
/// We scan for the first frame whose amplitude crosses `SILENCE_THRESHOLD` and
/// start the clip there. `channels` keeps the cut on a frame boundary so the
/// left/right interleaving stays aligned.
fn trim_leading_silence(samples: &[f32], channels: u16) -> Clip {
    let ch = channels.max(1) as usize;
    let first_audible = samples
        .chunks_exact(ch)
        .position(|frame| frame.iter().any(|s| s.abs() >= SILENCE_THRESHOLD))
        .map(|frame_idx| frame_idx * ch)
        .unwrap_or(0);
    Arc::from(&samples[first_audible..])
}

/// Pre-slice every key code's clip once so the keypress path never allocates.
/// Each clip is trimmed to its press (attack) portion, then has any leading
/// silence removed so the sound fires the instant a key goes down.
fn build_clips(pack: &SoundPack, decoded: &DecodedAudio) -> ClipCache {
    let make = |slice: Slice| {
        let clip = decoded.slice_clip(press_slice(slice));
        trim_leading_silence(&clip, decoded.channels)
    };
    let by_code = pack.slices().map(|(code, slice)| (code, make(slice))).collect();
    let default = pack.default_slice().map(make);
    ClipCache { by_code, default }
}

/// Body of the audio thread: owns the non-`Send` stream and services requests.
fn audio_thread(
    channels: u16,
    sample_rate: u32,
    rx: Receiver<PlayRequest>,
    ready: Sender<Result<(), String>>,
) {
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            let _ = ready.send(Err(format!("no default audio output device: {e}")));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }

    while let Ok(req) = rx.recv() {
        // The clip is already decoded, sliced, and in the output sample format.
        // Mix it directly into the stream via `play_raw` (no per-play `Sink`
        // allocation) so rapid or overlapping keys layer without queuing.
        let source =
            SamplesBuffer::new(channels, sample_rate, req.clip.to_vec()).amplify(req.gain);
        let _ = handle.play_raw(source);
    }
}

/// Load the sound pack config + audio bytes, preferring user files next to the
/// exe (`config.json` + its referenced sound file), else the embedded default.
fn load_pack() -> Result<(SoundPack, Vec<u8>)> {
    if let Some(dir) = exe_dir() {
        let cfg_path = dir.join("config.json");
        if let Ok(cfg_text) = std::fs::read_to_string(&cfg_path) {
            if let Ok(pack) = SoundPack::from_json(&cfg_text) {
                let sound_path = dir.join(&pack.sound_file);
                if let Ok(bytes) = std::fs::read(&sound_path) {
                    if !bytes.is_empty() {
                        return Ok((pack, bytes));
                    }
                }
            }
        }
    }
    // Fall back to the embedded pack.
    let pack = SoundPack::from_json(EMBEDDED_CONFIG)
        .context("embedded sound pack config failed to parse")?;
    Ok((pack, EMBEDDED_SOUND.to_vec()))
}

/// Directory of the running executable.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// Decode compressed audio bytes into interleaved f32 PCM plus format info.
///
/// Decoding to f32 once (the format the output stream wants) means the keypress
/// path plays samples straight through with no per-key conversion.
fn decode(bytes: Vec<u8>) -> Result<DecodedAudio> {
    let decoder = rodio::Decoder::new(Cursor::new(bytes))
        .map_err(|e| anyhow!("failed to decode sound file: {e}"))?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let data: Vec<f32> = decoder.convert_samples::<f32>().collect();
    if data.is_empty() {
        return Err(anyhow!("decoded sound file contained no audio"));
    }
    Ok(DecodedAudio {
        channels,
        sample_rate,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_pack_parses_and_decodes() {
        let pack = SoundPack::from_json(EMBEDDED_CONFIG).expect("embedded config parses");
        assert!(!pack.is_empty());
        let decoded = decode(EMBEDDED_SOUND.to_vec()).expect("embedded sound decodes");
        assert!(decoded.channels >= 1);
        assert!(decoded.sample_rate > 0);
        assert!(!decoded.data.is_empty());
    }

    #[test]
    fn slice_clip_respects_bounds_and_length() {
        let decoded = DecodedAudio {
            channels: 2,
            sample_rate: 1000, // 1 frame per ms for easy math
            data: vec![0.0f32; 2 * 1000], // 1000 frames, 1s
        };
        // 100ms starting at 200ms => 100 frames * 2 channels = 200 samples.
        let s = decoded.slice_clip(Slice {
            start_ms: 200,
            duration_ms: 100,
        });
        assert_eq!(s.len(), 200);

        // A slice past the end clamps instead of panicking.
        let s2 = decoded.slice_clip(Slice {
            start_ms: 990,
            duration_ms: 100,
        });
        assert!(s2.len() <= 20);
    }
}
