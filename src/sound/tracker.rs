//! A tiny tracker. Songs are channels of whitespace-separated rows:
//!
//! - `C4`, `F#3`, `Bb2`: note on
//! - `C4^047`: note on as a fast chip arpeggio over the hex semitone offsets (here C-E-G)
//! - `.`: keep holding
//! - `-`: note off
//! - `k` / `s` / `h` / `c`: kick / snare / hi-hat / crash (drum channel)
//! - `|`: ignored, for readability (bar lines)

use super::synth::{Drum, DrumVoice, Osc, SR, Wave, midi_hz, parse_note};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instrument {
    pub wave: Wave,
    pub volume: f32,
    /// Seconds.
    pub attack: f32,
    /// Decay rate towards `sustain` (1/s).
    pub decay: f32,
    pub sustain: f32,
    /// Release rate (1/s).
    pub release: f32,
    /// Vibrato depth in semitones, rate in Hz, onset delay in seconds.
    pub vibrato: (f32, f32, f32),
    /// Overdrive: 0 is clean; higher pushes the voice into a soft clipper (guitar crunch).
    pub drive: f32,
    /// Play every note as a power chord: root, fifth and octave together.
    pub power: bool,
    /// Detune of the stacked voices in semitones, for a thicker sound.
    pub detune: f32,
}

impl Instrument {
    pub const fn new(wave: Wave, volume: f32) -> Self {
        Self {
            wave,
            volume,
            attack: 0.004,
            decay: 6.0,
            sustain: 0.6,
            release: 18.0,
            vibrato: (0.0, 0.0, 0.0),
            drive: 0.0,
            power: false,
            detune: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell {
    /// A note, optionally arpeggiated over three semitone offsets (cycled at 50 Hz).
    Note(u8, Option<[u8; 3]>),
    Hold,
    Off,
    Hit(Drum),
}

#[derive(Clone, Debug)]
pub struct Channel {
    /// `None` for the drum channel.
    pub inst: Option<Instrument>,
    pub rows: Vec<Cell>,
}

impl Channel {
    pub fn new(inst: Option<Instrument>, pattern: &str) -> Self {
        let rows = pattern
            .split_whitespace()
            .filter(|t| *t != "|")
            .map(|t| match t {
                "." => Cell::Hold,
                "-" => Cell::Off,
                "k" => Cell::Hit(Drum::Kick),
                "s" => Cell::Hit(Drum::Snare),
                "h" => Cell::Hit(Drum::Hat),
                "c" => Cell::Hit(Drum::Crash),
                n => {
                    let (name, arp) = match n.split_once('^') {
                        Some((name, digits)) => {
                            let d: Vec<u8> = digits
                                .chars()
                                .map(|c| c.to_digit(16).expect("arp digit") as u8)
                                .collect();
                            assert_eq!(d.len(), 3, "arp needs 3 offsets: {n:?}");
                            (name, Some([d[0], d[1], d[2]]))
                        }
                        None => (n, None),
                    };
                    Cell::Note(
                        parse_note(name).unwrap_or_else(|| panic!("bad note {n:?}")),
                        arp,
                    )
                }
            })
            .collect();
        Self { inst, rows }
    }
}

#[derive(Clone, Debug)]
pub struct Song {
    /// Rows per minute.
    pub rpm: f32,
    pub looped: bool,
    pub channels: Vec<Channel>,
    pub gain: f32,
}

impl Song {
    pub fn len(&self) -> usize {
        self.channels
            .iter()
            .map(|c| c.rows.len())
            .max()
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn duration(&self) -> f32 {
        self.len() as f32 * 60.0 / self.rpm
    }
}

#[derive(Clone, Debug, Default)]
struct Voice {
    osc: Osc,
    /// The fifth and octave of a power chord.
    stack: [Osc; 2],
    note: Option<u8>,
    arp: Option<[u8; 3]>,
    gate: bool,
    env: f32,
    on_t: f32,
    drums: Vec<DrumVoice>,
}

/// Plays a [`Song`] sample by sample.
pub struct Player {
    song: Song,
    row: usize,
    samples_into_row: f32,
    voices: Vec<Voice>,
    /// Samples played after the last row of an unlooped song.
    tail: usize,
    started: bool,
}

impl Player {
    pub fn new(song: Song) -> Self {
        let voices = vec![Voice::default(); song.channels.len()];
        Self {
            song,
            row: 0,
            samples_into_row: 0.0,
            voices,
            tail: 0,
            started: false,
        }
    }

    pub fn finished(&self) -> bool {
        !self.song.looped && self.row >= self.song.len() && self.tail > SR as usize
    }

    fn trigger_row(&mut self) {
        for (ch, v) in self.song.channels.iter().zip(&mut self.voices) {
            match ch.rows.get(self.row).copied() {
                Some(Cell::Note(n, arp)) => {
                    v.note = Some(n);
                    v.arp = arp;
                    v.gate = true;
                    v.on_t = 0.0;
                    v.env = 0.0;
                }
                Some(Cell::Off) => v.gate = false,
                Some(Cell::Hit(d)) => v.drums.push(DrumVoice::new(d)),
                Some(Cell::Hold) | None => {}
            }
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let row_len = SR * 60.0 / self.song.rpm;
        if !self.started {
            self.started = true;
            self.trigger_row();
        }
        if self.samples_into_row >= row_len {
            self.samples_into_row -= row_len;
            self.row += 1;
            if self.row >= self.song.len() && self.song.looped {
                self.row = 0;
            }
            if self.row < self.song.len() {
                self.trigger_row();
            } else {
                for v in &mut self.voices {
                    v.gate = false;
                }
            }
        }
        if self.row >= self.song.len() {
            self.tail += 1;
        }
        self.samples_into_row += 1.0;

        let mut mix = 0.0;
        for (ch, v) in self.song.channels.iter().zip(&mut self.voices) {
            for d in &mut v.drums {
                mix += d.sample() * 0.55;
            }
            v.drums.retain(|d| !d.done());
            let (Some(inst), Some(note)) = (ch.inst, v.note) else {
                continue;
            };
            let dt = 1.0 / SR;
            v.on_t += dt;
            if v.gate {
                if v.on_t < inst.attack {
                    v.env = v.on_t / inst.attack;
                } else {
                    v.env += (inst.sustain - v.env) * (inst.decay * dt).min(1.0);
                }
            } else {
                v.env -= v.env * (inst.release * dt).min(1.0);
            }
            if v.env < 1e-4 && !v.gate {
                continue;
            }
            let mut semis = 0.0;
            let (depth, rate, delay) = inst.vibrato;
            if depth > 0.0 && v.on_t > delay {
                semis += depth * (std::f32::consts::TAU * rate * (v.on_t - delay)).sin();
            }
            if let Some(arp) = v.arp {
                semis += arp[((v.on_t * 50.0) as usize) % 3] as f32;
            }
            let hz = midi_hz(note) * 2f32.powf(semis / 12.0);
            let mut x = v.osc.sample(inst.wave, hz);
            if inst.power {
                let up = |s: f32| 2f32.powf(s / 12.0);
                x += 0.8 * v.stack[0].sample(inst.wave, hz * up(7.0 + inst.detune));
                x += 0.6 * v.stack[1].sample(inst.wave, hz * up(12.0 - inst.detune));
                x /= 2.4;
            }
            if inst.drive > 0.0 {
                x = (x * (1.0 + inst.drive)).tanh() / (1.0 + inst.drive).tanh();
            }
            mix += x * v.env * inst.volume;
        }
        // Soft clip to keep stacked channels from crackling.
        (mix * self.song.gain).tanh()
    }

    pub fn fill(&mut self, out: &mut [f32]) {
        for s in out {
            *s = self.next_sample();
        }
    }
}

/// Render an unlooped song (a jingle) to PCM, including a short release tail.
pub fn render_song(song: Song) -> Vec<f32> {
    let n = ((song.duration() + 0.6) * SR) as usize;
    let mut p = Player::new(Song {
        looped: false,
        ..song
    });
    (0..n).map(|_| p.next_sample()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_parse() {
        let c = Channel::new(None, "k . s | h - C4");
        assert_eq!(c.rows.len(), 6);
        assert_eq!(c.rows[0], Cell::Hit(Drum::Kick));
        assert_eq!(c.rows[5], Cell::Note(60, None));
        let c = Channel::new(None, "A3^037");
        assert_eq!(c.rows[0], Cell::Note(57, Some([0, 3, 7])));
    }

    #[test]
    fn player_produces_bounded_sound() {
        let song = Song {
            rpm: 600.0,
            looped: false,
            gain: 1.0,
            channels: vec![
                Channel::new(Some(Instrument::new(Wave::Pulse(0.5), 0.5)), "C4 . E4 -"),
                Channel::new(None, "k h s h"),
            ],
        };
        let pcm = render_song(song);
        assert!(pcm.iter().all(|s| s.abs() <= 1.0));
        assert!(pcm.iter().any(|s| s.abs() > 0.1));
    }
}
