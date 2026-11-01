//! A tiny tracker. Songs are channels of whitespace-separated rows:
//!
//! - `C4`, `F#3`, `Bb2`: note on (see [`parse_note`] for the exact format; C4 is middle C)
//! - `C4^047`: note on as a fast chip arpeggio over the hex semitone offsets (here C-E-G)
//! - `.`: keep holding
//! - `-`: note off
//! - `k` / `s` / `h` / `c`: kick / snare / hi-hat / crash (drum channel)
//! - `|`: ignored, for readability (bar lines)
//!
//! # How it plays
//!
//! A [`Song`] is a list of [`Channel`]s that all advance one row at a time, at
//! [`Song::rpm`] rows per minute. A row has no fixed musical value: the songs use one row per
//! eighth note (so `rpm` is twice the bpm) or one per quarter note (`rpm` equals the bpm).
//! Each channel is monophonic, like a channel on a sound chip: a new note cuts the previous
//! one off. Chords therefore need several channels, or the [`Instrument::power`] stacking.
//!
//! A [`Player`] renders the song one sample at a time. On each row boundary it reads the
//! cell of every channel: notes (re)start that channel's voice, `-` releases it, `.` leaves
//! it alone, and drum letters start a new [`DrumVoice`] that rings out independently. Then,
//! every sample, each voice runs its envelope, applies vibrato and arpeggio to the pitch,
//! samples its oscillator(s), applies overdrive, and everything is summed and soft-clipped.
//!
//! The player is moved into the audio stream callback (see `Music::play` in `app.rs`) and
//! pulled on the audio thread in buffer-sized chunks through [`Player::fill`]. That keeps the
//! music in sync with the audio clock, lets it loop seamlessly forever, and costs no memory
//! for a pre-rendered track. The flip side is that sampling must stay cheap and should not
//! allocate; the only allocation is the occasional growth of a channel's drum voice list.

use super::synth::{Drum, DrumVoice, Osc, SR, Wave, midi_hz, parse_note};

/// How a melodic channel sounds: waveform, level, envelope and effects.
///
/// Start from [`Instrument::new`] and override fields with struct update syntax, as
/// `songs.rs` does.
///
/// The envelope is an attack-decay-sustain-release (ADSR) shape computed per sample in
/// [`Player::next_sample`]: a linear ramp from 0 to 1 over `attack` seconds, then an
/// exponential approach towards `sustain` at rate `decay`, held there while the note is held,
/// and an exponential fall towards 0 at rate `release` after a note off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instrument {
    /// Oscillator waveform for the note (and for the stacked power chord voices).
    pub wave: Wave,
    /// Linear output level of the channel before mixing; around 0.2-0.6 in practice.
    pub volume: f32,
    /// Seconds.
    ///
    /// Length of the linear ramp from silence to full level at the start of each note.
    /// A few milliseconds avoids a click; longer values swell in.
    pub attack: f32,
    /// Decay rate towards `sustain` (1/s).
    ///
    /// After the attack the envelope moves `decay * dt` of its remaining distance to
    /// `sustain` each sample, i.e. an exponential with time constant `1 / decay` seconds.
    /// Higher values give a sharper, more plucked onset.
    pub decay: f32,
    /// Level (0..=1) the envelope settles at while the note is held. 0 makes every note die
    /// away on its own, like a plucked or struck instrument.
    pub sustain: f32,
    /// Release rate (1/s).
    ///
    /// After a note off (`-`, or the end of an unlooped song) the envelope falls
    /// exponentially with time constant `1 / release` seconds. High values cut notes off
    /// short (palm-muted chugs); low values let them ring.
    pub release: f32,
    /// Vibrato depth in semitones, rate in Hz, onset delay in seconds.
    ///
    /// Once a note has sounded for `delay` seconds, its pitch is modulated by
    /// `depth * sin(2 pi rate (t - delay))` semitones. The sine starts at zero, so the
    /// vibrato fades in without a pitch jump. A depth of 0 disables it.
    pub vibrato: (f32, f32, f32),
    /// Overdrive: 0 is clean; higher pushes the voice into a soft clipper (guitar crunch).
    ///
    /// The raw oscillator signal `x` (before the envelope) becomes
    /// `tanh(x * (1 + drive)) / tanh(1 + drive)`. The gain in front of `tanh` squashes the
    /// waveform towards a square, adding odd harmonics; dividing by `tanh(1 + drive)` maps
    /// an input of +-1 back to exactly +-1, so the peak level stays the same and only the
    /// shape changes. Gotcha: a lone [`Wave::Pulse`] is already exactly +-1, so drive does
    /// nothing to it; it only matters for triangle, saw, or stacked power chords.
    pub drive: f32,
    /// Play every note as a power chord: root, fifth and octave together.
    ///
    /// Two extra oscillators play 7 and 12 semitones above the root, mixed at 0.8 and 0.6
    /// of the root's level, and the sum is divided by 2.4 (the sum of the weights) so the
    /// peak stays within +-1. Combined with `drive`, the intermodulation of the three gives
    /// the distorted-guitar sound.
    pub power: bool,
    /// Detune of the stacked voices in semitones, for a thicker sound.
    ///
    /// The fifth is raised and the octave lowered by this amount, so they beat slowly
    /// against the root's harmonics instead of locking in phase. Only used with `power`.
    /// 0.08 semitones is 8 cents.
    pub detune: f32,
}

impl Instrument {
    /// A plain instrument with a generic short-attack envelope: 4 ms attack, decay at 6/s
    /// to a 0.6 sustain, release at 18/s (about 55 ms), and no vibrato, drive or stacking.
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

/// One parsed row of a channel pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell {
    /// A note, optionally arpeggiated over three semitone offsets (cycled at 50 Hz).
    ///
    /// The `u8` is the MIDI note. With an arpeggio, the pitch steps through
    /// `note + offset[0]`, `note + offset[1]`, `note + offset[2]`, 20 ms each (a full cycle
    /// every 60 ms), for as long as the note sounds. This is the classic chip trick for
    /// suggesting a chord on a single monophonic channel. Offsets are single hex digits, so
    /// they range 0..=15 semitones.
    Note(u8, Option<[u8; 3]>),
    /// `.`: nothing new; a sounding note keeps sounding (and a released one keeps releasing).
    Hold,
    /// `-`: note off; the channel's voice enters its release phase.
    Off,
    /// `k`, `s`, `h` or `c`: start a drum hit. Hits on the same channel overlap freely.
    Hit(Drum),
}

/// One track of a song: an instrument and its rows.
#[derive(Clone, Debug)]
pub struct Channel {
    /// `None` for the drum channel.
    ///
    /// A channel without an instrument still plays drum hits but ignores notes. A channel
    /// with one plays notes and would also play any drum hits written into it.
    pub inst: Option<Instrument>,
    /// The parsed pattern, one cell per row.
    pub rows: Vec<Cell>,
}

impl Channel {
    /// Parse a pattern string (see the module docs for the notation) into a channel.
    ///
    /// Tokens are separated by any whitespace, including newlines, so long patterns can be
    /// laid out a few bars per line. Bar lines must be standalone `|` tokens.
    ///
    /// # Panics
    ///
    /// On a malformed note or arpeggio (anything that is not a note, a drum letter, `.`,
    /// `-` or `|`). Patterns are string constants in `songs.rs`, so this is a programming
    /// error; the tests in `songs.rs` build every song to catch it before it can happen in
    /// the game.
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

/// A complete piece of music: channels played in lockstep at a fixed row rate.
#[derive(Clone, Debug)]
pub struct Song {
    /// Rows per minute.
    ///
    /// Each row lasts `60 / rpm` seconds. With one row per eighth note this is twice the
    /// tempo in bpm; with one row per quarter note it equals it.
    pub rpm: f32,
    /// Whether to jump back to row 0 after the last row (music) or stop and let the notes
    /// release (jingles).
    pub looped: bool,
    /// The tracks. They should all have the same number of rows; a shorter one simply holds
    /// its last state until the song wraps.
    pub channels: Vec<Channel>,
    /// Linear gain applied to the summed mix before the final `tanh` soft clipper. Above
    /// about 1 it starts to compress the loud parts audibly.
    pub gain: f32,
}

impl Song {
    /// Length in rows: that of the longest channel.
    pub fn len(&self) -> usize {
        self.channels
            .iter()
            .map(|c| c.rows.len())
            .max()
            .unwrap_or(0)
    }

    /// Whether the song has no rows at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Length of one pass in seconds, without any release tail.
    pub fn duration(&self) -> f32 {
        self.len() as f32 * 60.0 / self.rpm
    }
}

/// Playback state of one channel.
#[derive(Clone, Debug, Default)]
struct Voice {
    /// The main oscillator, playing the note's (vibrato- and arpeggio-shifted) pitch.
    osc: Osc,
    /// The fifth and octave of a power chord.
    stack: [Osc; 2],
    /// The current or most recent MIDI note; stays set during the release so it can fade.
    /// `None` until the channel plays its first note.
    note: Option<u8>,
    /// Arpeggio offsets of the current note, if any.
    arp: Option<[u8; 3]>,
    /// True while the note is held (between its note-on and a note off).
    gate: bool,
    /// Current envelope level, 0..=1.
    env: f32,
    /// Seconds since the current note started; drives the attack, vibrato and arpeggio.
    on_t: f32,
    /// Drum hits still ringing on this channel.
    drums: Vec<DrumVoice>,
}

/// Plays a [`Song`] sample by sample.
///
/// Created on the main thread and then moved into the audio callback, which calls
/// [`Player::fill`]. All timing is counted in samples at [`SR`], so playback is
/// sample-accurate and independent of the frame rate.
pub struct Player {
    /// The song being played.
    song: Song,
    /// Index of the current row; equals `song.len()` once an unlooped song has ended.
    row: usize,
    /// Samples played since the current row started. Kept fractional because a row is
    /// rarely a whole number of samples long; the leftover carries into the next row so the
    /// tempo does not drift.
    samples_into_row: f32,
    /// One voice per channel, in the same order as `song.channels`.
    voices: Vec<Voice>,
    /// Samples played after the last row of an unlooped song.
    tail: usize,
    /// Whether row 0 has been triggered yet (done lazily on the first sample).
    started: bool,
}

impl Player {
    /// Prepare to play `song` from the top. No sound is made until samples are pulled.
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

    /// Whether an unlooped song has ended and had one second (`SR` samples) to release.
    /// Always false for looped songs.
    pub fn finished(&self) -> bool {
        !self.song.looped && self.row >= self.song.len() && self.tail > SR as usize
    }

    /// Apply the cells of the current row to every channel's voice.
    ///
    /// A note resets the voice: new pitch and arpeggio, gate on, note time and envelope back
    /// to 0. The envelope restarts from silence even if the previous note was still loud,
    /// which gives repeated notes a hard re-articulation (the guitar chug). The oscillator
    /// phase is not reset. A row past the end of a shorter channel counts as a hold.
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

    /// Produce the next mono sample, in -1..1.
    ///
    /// Per sample, in order:
    ///
    /// 1. Sequencing: a row lasts `SR * 60 / rpm` samples. When the current row is used up,
    ///    advance (wrapping to 0 for looped songs) and trigger the new row. Past the end of
    ///    an unlooped song, all gates close so every note releases, and the tail counter
    ///    runs.
    /// 2. Drums: every ringing drum voice is sampled and mixed at 0.55; finished ones are
    ///    dropped.
    /// 3. Envelope, with `dt = 1 / SR`: during the attack, `env = on_t / attack` (linear);
    ///    afterwards `env += (sustain - env) * decay * dt`, a one-pole exponential approach.
    ///    After a note off, `env -= env * release * dt`. Both factors are capped at 1 so a
    ///    huge rate cannot overshoot. A released voice below 1e-4 is skipped entirely.
    /// 4. Pitch: vibrato and arpeggio offsets are summed in semitones and applied as a
    ///    factor `2^(semis / 12)` to the note's frequency.
    /// 5. Oscillators: the main one, plus the power chord stack if enabled, then drive.
    /// 6. Mix: each voice adds `x * env * volume`. The sum is multiplied by the song's
    ///    `gain` and passed through `tanh`, a soft clipper: nearly linear for quiet signals,
    ///    smoothly saturating towards +-1 when several loud channels coincide. That keeps the
    ///    output in range without the harsh crackle of hard clipping.
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

    /// Fill `out` with consecutive samples; this is the audio stream callback's body.
    pub fn fill(&mut self, out: &mut [f32]) {
        for s in out {
            *s = self.next_sample();
        }
    }
}

/// Render an unlooped song (a jingle) to PCM, including a short release tail.
///
/// Forces `looped` off, then renders one pass plus 0.6 s so the last notes and drum hits
/// can fade out instead of being cut. Notes with a very slow release may still be cut at the
/// end of the tail.
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
