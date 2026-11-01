//! All audio is generated in code: no sample files.
//!
//! - [`synth`]: chip-style oscillators, envelopes and noise.
//! - [`sfx`]: one-shot effects rendered to PCM at startup.

pub mod sfx;
pub mod synth;

/// Sample rate for everything we synthesize; kira resamples to the device.
pub const SAMPLE_RATE: u32 = 32_000;
