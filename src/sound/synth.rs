//! Chip-style building blocks: pulse / triangle / noise oscillators and drum voices.
//!
//! Everything here is stateful and advances by exactly one sample per `sample()` call, at
//! [`SR`] samples per second. Nothing allocates while sampling, so the same code serves both
//! the offline effect renderer ([`super::sfx`]) and the real-time music player
//! ([`super::tracker`]), which runs on the audio thread.
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

/// Parse "C4", "F#3", "Bb2" into a MIDI note.
///
/// The format is a letter `A`..`G` (either case), an optional accidental (`#` sharp or a
/// lowercase `b` flat; only one), and a signed octave number in scientific pitch notation, so
/// "C4" is middle C (60) and "A4" is 69. The octave changes at C, so "B3" is directly below
/// "C4", and "Cb4" is the same as "B3". German "H" is not accepted. Returns `None` for
/// anything malformed or for results below MIDI 0 or above 255.
pub fn parse_note(s: &str) -> Option<u8> {
    let mut chars = s.chars();
    let base = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (acc, oct) = if let Some(o) = rest.strip_prefix('#') {
        (1, o)
    } else if let Some(o) = rest.strip_prefix('b') {
        (-1, o)
    } else {
        (0, rest.as_str())
    };
    let oct: i32 = oct.parse().ok()?;
    u8::try_from((oct + 1) * 12 + base + acc).ok()
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
    /// 0.5 carry a DC offset of `2 * duty - 1`, which is harmless here. Because the output is
    /// only ever exactly +1 or -1, the tracker's `drive` has no effect on a lone pulse.
    Pulse(f32),
    /// A triangle quantized to steps of 1/8 (17 levels from -1 to +1), mimicking the 4-bit
    /// staircase of the NES triangle channel. The steps add a faint buzzy edge to an
    /// otherwise soft, flute-like tone; it is used for bass, drums and the music box.
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

/// Drum hits for the noise channel.
///
/// Each kind is a fixed recipe in [`DrumVoice::sample`]; in tracker patterns they are written
/// as `k`, `s`, `h` and `c`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drum {
    /// Bass drum: a triangle whose pitch drops quickly from 190 Hz to 50 Hz.
    Kick,
    /// Snare: mid-bright noise plus a short 190 Hz triangle "body".
    Snare,
    /// Closed hi-hat: a very short burst of bright noise.
    Hat,
    /// A long, washy cymbal crash.
    Crash,
}

/// A playing drum hit.
///
/// A voice is created at the moment of the hit and owns its own oscillator and noise
/// generator, so overlapping hits do not disturb each other. The tracker samples it until
/// [`DrumVoice::done`] and then drops it.
#[derive(Clone, Debug)]
pub struct DrumVoice {
    /// Which recipe this voice plays.
    pub drum: Drum,
    /// Seconds since the hit; advanced by `1 / SR` per sample.
    pub t: f32,
    /// Tonal part (kick and snare body).
    osc: Osc,
    /// Noise part (snare, hat, crash).
    noise: Noise,
}

impl DrumVoice {
    /// Start a hit of `drum` at time 0.
    pub fn new(drum: Drum) -> Self {
        Self {
            drum,
            t: 0.0,
            osc: Osc::default(),
            noise: Noise::default(),
        }
    }

    /// Advance by one sample and return the hit's output (roughly -1.2..=1.2 at most).
    ///
    /// All envelopes are exponential decays `exp(-t * k)` with `k` in 1/s:
    ///
    /// - Kick: a triangle whose frequency is `50 + 140 * exp(-35 t)` Hz, so it starts at
    ///   190 Hz and falls to a 50 Hz thump with a time constant of about 29 ms. That fast
    ///   downward pitch sweep is what reads as a "punch". Amplitude decays as `exp(-18 t)`
    ///   (time constant about 55 ms), scaled by 1.2 so the kick sits on top of the mix.
    /// - Snare: noise clocked at 9 kHz (weight 0.7) plus a 190 Hz triangle (weight 0.5) for
    ///   the drum's body. The whole hit decays as `exp(-22 t)`, and the tone additionally as
    ///   `exp(-40 t)`, so the body is gone after a few tens of milliseconds and the rattle
    ///   of the noise lingers slightly longer.
    /// - Hat: noise clocked at 20 kHz (the brightest hiss) decaying as `exp(-90 t)`, about
    ///   11 ms, at half level: a short "tss".
    /// - Crash: noise clocked around 17 kHz, with the clock rate wobbled by +-30 % through
    ///   `sin(37 t)` (37 rad/s, about 6 Hz) so the wash shimmers instead of being static.
    ///   It decays slowly as `exp(-2.6 t)` (time constant about 0.38 s) at 0.45 level.
    pub fn sample(&mut self) -> f32 {
        let t = self.t;
        self.t += 1.0 / SR;
        match self.drum {
            Drum::Kick => {
                let hz = 50.0 + 140.0 * (-t * 35.0).exp();
                let env = (-t * 18.0).exp();
                self.osc.sample(Wave::Triangle, hz) * env * 1.2
            }
            Drum::Snare => {
                let env = (-t * 22.0).exp();
                let tone = self.osc.sample(Wave::Triangle, 190.0) * (-t * 40.0).exp();
                (self.noise.sample(9000.0) * 0.7 + tone * 0.5) * env
            }
            Drum::Hat => self.noise.sample(20000.0) * (-t * 90.0).exp() * 0.5,
            Drum::Crash => {
                let shimmer = 1.0 + 0.3 * (t * 37.0).sin();
                self.noise.sample(17000.0 * shimmer) * (-t * 2.6).exp() * 0.45
            }
        }
    }

    /// Whether the hit has faded enough to be dropped: after 1.8 s for a crash (down to
    /// about 1 % amplitude) and 0.4 s for everything else (well below 0.1 %).
    pub fn done(&self) -> bool {
        self.t > if self.drum == Drum::Crash { 1.8 } else { 0.4 }
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
    fn notes_parse() {
        assert_eq!(parse_note("A4"), Some(69));
        assert_eq!(parse_note("C4"), Some(60));
        assert_eq!(parse_note("C#4"), Some(61));
        assert_eq!(parse_note("Bb3"), Some(58));
        assert_eq!(parse_note("H2"), None);
        assert!((midi_hz(69) - 440.0).abs() < 1e-3);
    }

    #[test]
    fn noise_is_bipolar() {
        let mut n = Noise::default();
        let v: Vec<f32> = (0..1000).map(|_| n.sample(8000.0)).collect();
        assert!(v.contains(&1.0) && v.contains(&-1.0));
    }
}
