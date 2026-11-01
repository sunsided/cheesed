//! Chip-style building blocks: pulse / triangle / noise oscillators and drum voices.

use super::SAMPLE_RATE;

pub const SR: f32 = SAMPLE_RATE as f32;

/// MIDI note number to frequency.
pub fn midi_hz(note: u8) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0)
}

/// Parse "C4", "F#3", "Bb2" into a MIDI note.
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    /// Pulse with the given duty cycle (0.5 = square).
    Pulse(f32),
    Triangle,
    /// Sawtooth, for the sad trombone.
    Saw,
}

/// Naive oscillator; the aliasing is part of the charm.
#[derive(Clone, Debug, Default)]
pub struct Osc {
    pub phase: f32,
}

impl Osc {
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
            // Quantized to 16 steps like the NES triangle channel.
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
#[derive(Clone, Debug)]
pub struct Noise {
    reg: u16,
    acc: f32,
    out: f32,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            reg: 1,
            acc: 0.0,
            out: 1.0,
        }
    }
}

impl Noise {
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drum {
    Kick,
    Snare,
    Hat,
}

/// A playing drum hit.
#[derive(Clone, Debug)]
pub struct DrumVoice {
    pub drum: Drum,
    pub t: f32,
    osc: Osc,
    noise: Noise,
}

impl DrumVoice {
    pub fn new(drum: Drum) -> Self {
        Self {
            drum,
            t: 0.0,
            osc: Osc::default(),
            noise: Noise::default(),
        }
    }

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
        }
    }

    pub fn done(&self) -> bool {
        self.t > 0.4
    }
}

/// Render `secs` of a closure `f(t)` into a buffer.
pub fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * SR) as usize;
    (0..n).map(|i| f(i as f32 / SR)).collect()
}

/// Normalize a buffer to `peak`.
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
