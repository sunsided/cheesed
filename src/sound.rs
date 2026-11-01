//! All audio is generated in code: no sample files.
//!
//! - [`synth`]: chip-style oscillators, envelopes, noise and drums.
//! - [`tracker`]: a tiny pattern player for the music, pulled on the audio thread.
//! - [`songs`]: the tunes: heavy rock for the title, a music box for the game.
//! - [`sfx`]: one-shot effects rendered to PCM at startup.

pub mod sfx;
pub mod songs;
pub mod synth;
pub mod tracker;

/// Sample rate for everything we synthesize; kira resamples to the device.
pub const SAMPLE_RATE: u32 = 32_000;
