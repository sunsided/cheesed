//! Chip-style building blocks for the sound effects: pulse / triangle / saw oscillators,
//! noise, and helpers to render a closure into a buffer.
//!
//! Everything here is stateful and advances by exactly one sample per `sample()` call, at
//! [`SR`] samples per second. The effects in [`super::sfx`] are rendered with it once at
//! startup. (The music doesn't use it: it plays on synthie, see [`super::songs`].)
//!
//! The oscillators are deliberately naive: they compute the ideal waveform at each sample
//! instant with no band-limiting, so the hard edges of pulse and saw waves alias. That is the
//! sound of real 8-bit hardware too, and it is cheap.

use super::SAMPLE_RATE;

/// [`SAMPLE_RATE`] as `f32`, for the per-sample arithmetic (phase increments, `1.0 / SR`
/// time steps, seconds-to-samples conversions).
pub const SR: f32 = SAMPLE_RATE as f32;

/// MIDI note number to frequency in Hz, in twelve-tone equal temperament tuned to A4 = 440 Hz.
///
/// Each semitone multiplies the frequency by 2^(1/12); note 69 is A4, 60 is middle C
/// (about 261.6 Hz), and every 12 notes is an octave (factor 2).
pub fn midi_hz(note: u8) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0)
}

/// Oscillator waveform. All shapes are bipolar, spanning -1..=1, and are evaluated from a
/// phase `p` in 0..1 (one cycle) by [`Osc::sample`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    /// Pulse with the given duty cycle (0.5 = square).
    ///
    /// The output is +1 for the first `duty` fraction of each cycle and -1 for the rest. The
    /// duty sets the timbre: 0.5 is a hollow square (odd harmonics only), while the narrower
    /// NES settings 0.25 and 0.125 sound progressively thinner and more nasal. Duties other than
    /// 0.5 carry a DC offset of `2 * duty - 1`, which is harmless here.
    Pulse(f32),
    /// A triangle quantized to steps of 1/8 (17 levels from -1 to +1), mimicking the 4-bit
    /// staircase of the NES triangle channel. The steps add a faint buzzy edge to an
    /// otherwise soft, flute-like tone.
    Triangle,
    /// Sawtooth, for the sad trombone.
    ///
    /// Ramps linearly from -1 to +1 over each cycle and drops back, giving every harmonic at
    /// 1/n amplitude: the brightest, buzziest of the three. The rhythm guitar uses it.
    Saw,
}

/// Naive oscillator; the aliasing is part of the charm.
///
/// It is just a phase accumulator: each sample adds `hz / SR` (the fraction of a cycle that
/// passes in one sample) to `phase` and wraps it into 0..1, then maps that phase through the
/// waveform. Because the frequency is applied as an increment rather than baked into a
/// `sin(2 pi f t)` formula, the frequency can change every sample (sweeps, vibrato,
/// arpeggios) without clicks or phase jumps.
#[derive(Clone, Debug, Default)]
pub struct Osc {
    /// Position within the current cycle, in 0..1. Starts at 0.
    pub phase: f32,
}

impl Osc {
    /// Advance by one sample at frequency `hz` and return the new output value in -1..=1.
    ///
    /// The phase is advanced before it is read, so the very first sample is already one
    /// increment into the cycle. `hz` should be non-negative and below the Nyquist
    /// frequency (`SR / 2`); higher values wrap around and alias to a different pitch.
    pub fn sample(&mut self, wave: Wave, hz: f32) -> f32 {
        self.phase = (self.phase + hz / SR).fract();
        let p = self.phase;
        match wave {
            Wave::Pulse(duty) => {
                if p < duty {
                    1.0
                } else {
                    -1.0
                }
            }
            // Quantized to steps of 1/8 (16 steps from -1 to +1) like the NES triangle channel.
            Wave::Triangle => {
                let t = if p < 0.5 {
                    4.0 * p - 1.0
                } else {
                    3.0 - 4.0 * p
                };
                (t * 8.0).round() / 8.0
            }
            Wave::Saw => 2.0 * p - 1.0,
        }
    }
}

/// A 15-bit LFSR noise generator (NES/C64 flavor), clocked at `hz`.
///
/// A linear-feedback shift register produces a pseudo-random bit stream. On every clock the
/// two lowest bits are XORed, the register shifts right by one, and the XOR result is fed in
/// at bit 14. These taps (x^15 + x^14 + 1, the NES "long" noise mode) give a maximal-length
/// sequence that repeats only every 32767 clocks. The output is bit 0, mapped to +1 or -1,
/// and it is held between clocks.
///
/// The clock rate is the timbre control, not a pitch: the output can only change at clock
/// ticks, so its energy is spread roughly evenly up to about half the clock rate and falls off
/// above that. A high clock (16-20 kHz) gives bright hiss for hi-hats and cymbals; a few kHz
/// gives a duller "shh" for a snare or a scrape; a few hundred Hz gives a grainy rumble. The
/// clock is accumulated as a fraction of samples, so rates that do not divide [`SR`] evenly
/// produce slightly uneven hold lengths, which only adds grit.
#[derive(Clone, Debug)]
pub struct Noise {
    /// The 15-bit shift register. Must never be zero, or it would stay zero forever.
    reg: u16,
    /// Fractional clock ticks owed, in units of whole ticks (adds `hz / SR` per sample).
    acc: f32,
    /// The current held output, +1 or -1.
    out: f32,
}

impl Default for Noise {
    /// Seeds the register with 1, so every new generator plays the identical sequence. That
    /// makes all effects deterministic and every drum hit of a kind sound exactly the same.
    fn default() -> Self {
        Self {
            reg: 1,
            acc: 0.0,
            out: 1.0,
        }
    }
}

impl Noise {
    /// Advance by one sample with the register clocked at `hz` and return +1 or -1.
    ///
    /// If `hz` exceeds [`SR`], the register is clocked several times within one sample and
    /// only the last result is heard.
    pub fn sample(&mut self, hz: f32) -> f32 {
        self.acc += hz / SR;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let bit = (self.reg ^ (self.reg >> 1)) & 1;
            self.reg = (self.reg >> 1) | (bit << 14);
            self.out = if self.reg & 1 == 1 { 1.0 } else { -1.0 };
        }
        self.out
    }
}

/// Render `secs` of a closure `f(t)` into a buffer.
///
/// Produces `secs * SR` samples (truncated), calling `f` once per sample, in order, with the
/// sample's time in seconds (`i / SR`). The closure is `FnMut` so it can own and advance
/// oscillators; because the calls are sequential, those oscillators stay phase-continuous.
pub fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * SR) as usize;
    (0..n).map(|i| f(i as f32 / SR)).collect()
}

/// Normalize a buffer to `peak`.
///
/// Scales every sample so the largest absolute value becomes exactly `peak` (0..=1). This
/// lets each effect recipe ignore its raw loudness and choose its final level in one place.
/// A buffer that is effectively silent (max below 1e-5) is left untouched rather than
/// amplified into noise or divided by zero.
pub fn normalize(buf: &mut [f32], peak: f32) {
    let m = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if m > 1e-5 {
        for s in buf {
            *s *= peak / m;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440_hz() {
        assert!((midi_hz(69) - 440.0).abs() < 1e-3);
        assert!((midi_hz(81) - 880.0).abs() < 1e-2);
    }

    #[test]
    fn noise_is_bipolar() {
        let mut n = Noise::default();
        let v: Vec<f32> = (0..1000).map(|_| n.sample(8000.0)).collect();
        assert!(v.contains(&1.0) && v.contains(&-1.0));
    }
}
