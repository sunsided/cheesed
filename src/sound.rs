//! All audio is generated in code: no sample files.
//!
//! The sound effects are modelled loosely on 8-bit home consoles (NES, C64): a handful of
//! naive oscillators (pulse, quantized triangle, sawtooth) and an LFSR noise generator,
//! shaped by simple envelopes, on mono `f32` samples in the range -1..=1 at [`SAMPLE_RATE`].
//! The music plays on synthie instead.
//!
//! - [`synth`]: the building blocks of the effects: note/frequency helpers, oscillators,
//!   noise, and small helpers to render a closure into a buffer.
//! - [`songs`]: the tunes, as synthie songs (game-toolkit's `synth` feature) in tracker
//!   notation: heavy rock for the title, a music box for the game, and a major-key rock
//!   reprise for the end of an episode. synthie plays them live on the audio thread.
//! - [`sfx`]: one-shot effects. Each is a small formula rendered to a PCM buffer once at
//!   startup (see [`sfx::Bank`]) and handed to the audio backend whenever it is played.
//!
//! Units used throughout: frequencies in Hz, times in seconds, pitches as MIDI note numbers
//! (60 = middle C = C4, 69 = A4 = 440 Hz) and offsets in semitones. Rates written as
//! `exp(-t * k)` use `k` in 1/s, so the time constant (fall to about 37 %) is `1/k` seconds.

pub mod sfx;
pub mod songs;
pub mod synth;

/// Sample rate in Hz for everything we synthesize; kira resamples to the device.
///
/// 32 kHz puts the Nyquist frequency at 16 kHz. The oscillators are not band-limited, so
/// harmonics above 16 kHz fold back (alias) into the audible range; at this rate that adds to
/// the gritty chip character rather than being a problem. Pitches and durations are all
/// computed from this constant, so changing it keeps them intact; only the aliasing changes.
pub const SAMPLE_RATE: u32 = 32_000;
