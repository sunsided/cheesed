//! The soundtrack on synthie (game-toolkit's `synth` feature), converted from the tracker
//! songs in [`super::songs`] so the notes have one source of truth.
//!
//! [`to_synthie`] turns a [`tracker::Song`] into a [`synth::Song`] that
//! `Audio::play_synth` streams through kira, sequenced sample-accurately on the audio thread:
//!
//! - Every melodic tracker channel becomes a synthie channel (synthie has four), with a
//!   patch derived from its [`Instrument`] by [`patch_for`]. A `power` instrument (power
//!   chords) becomes three lanes on the same channel, playing the root, the fifth and the
//!   octave, since synthie lanes are monophonic and chords take several lanes (a channel has
//!   four voices).
//! - Drum channels become drum lanes; synthie's drum machine plays kick, snare, hat and crash
//!   by itself, independently of the channels.
//! - Tempo carries over as rows per minute, and the cells map one to one onto synthie's
//!   notation (`C4`, `.` hold, `-` release, `k s h c` drums). Each melodic lane's first cell
//!   also selects its patch with `@n`.

use game_toolkit::prelude::synth::{self, ChannelNo, Tempo};
use game_toolkit::prelude::synthie::params::{
    FilterMode, LfoShape, LfoTarget, SynthParams, Waveform,
};

use super::synth::{Drum, Wave};
use super::tracker::{self, Cell, Instrument};

/// How many melodic channels synthie offers (`synthie::NUM_CHANNELS`); a song may use at
/// most this many instruments.
const CHANNELS: usize = 4;

/// One synthie unit of pitch modulation is a tenth of an octave (`freq * 2^(pitch / 10)`),
/// i.e. 1.2 semitones. Used to convert vibrato depths given in semitones.
const SEMITONES_PER_PITCH_UNIT: f32 = 1.2;

/// A synthie patch that approximates a tracker instrument.
///
/// - Waveform: pulse (with its duty cycle as pulse width), triangle or sawtooth.
/// - Envelope: the tracker's decay and release are exponential *rates* (1/s), synthie wants
///   *times* in seconds, so each becomes `1 / rate` (the time constant), attack and sustain
///   carry over.
/// - Vibrato: a sine pitch LFO at the same rate, its depth converted from semitones to
///   synthie's pitch units. (The tracker's vibrato onset delay has no counterpart.)
/// - Overdrive: the tracker's unbounded `drive` gain is squashed into synthie's 0..1 filter
///   drive as `drive / (1 + drive)`, so 0 stays clean and 4 (the rock guitar) becomes 0.8.
/// - The filter is opened up (a high low-pass cutoff, no resonance): the tracker has no
///   filter, and the chip timbre should stay bright.
/// - Volume carries over as the patch volume.
pub fn patch_for(inst: &Instrument) -> SynthParams {
    let mut p = SynthParams::default();
    match inst.wave {
        Wave::Pulse(duty) => {
            p.osc.waveform = Waveform::Pulse;
            p.osc.pulse_width = duty.clamp(0.05, 0.95);
        }
        Wave::Triangle => p.osc.waveform = Waveform::Triangle,
        Wave::Saw => p.osc.waveform = Waveform::Sawtooth,
    }
    p.env.attack = inst.attack.max(0.001);
    p.env.decay = (1.0 / inst.decay.max(0.1)).clamp(0.001, 10.0);
    p.env.sustain = inst.sustain.clamp(0.0, 1.0);
    p.env.release = (1.0 / inst.release.max(0.1)).clamp(0.001, 10.0);
    let (depth, rate, _delay) = inst.vibrato;
    if depth > 0.0 {
        p.lfo.lfo_target = LfoTarget::Pitch;
        p.lfo.lfo_shape = LfoShape::Sine;
        p.lfo.lfo_rate = rate;
        p.lfo.lfo_depth = (depth / SEMITONES_PER_PITCH_UNIT).clamp(0.0, 1.0);
    } else {
        p.lfo.lfo_depth = 0.0;
    }
    p.filter.filter_mode = FilterMode::LowPass;
    p.filter.cutoff = 12_000.0;
    p.filter.resonance = 0.0;
    p.filter.drive = (inst.drive / (1.0 + inst.drive)).clamp(0.0, 1.0);
    p.global.volume = inst.volume.clamp(0.0, 1.0);
    p
}

/// Convert a tracker song into a synthie song (see the module docs for the mapping).
///
/// # Panics
///
/// If the song has more melodic channels than synthie offers, or a lane fails to parse
/// (which would be a bug in this conversion, as the tracker already parsed the notes).
pub fn to_synthie(song: &tracker::Song) -> synth::Song {
    let mut out = synth::Song::new(Tempo::rows_per_minute(song.rpm));
    if !song.looped {
        out = out.one_shot();
    }
    let mut next_channel = 0u8;
    let mut patch_index = 0usize;
    for ch in &song.channels {
        match &ch.inst {
            Some(inst) => {
                assert!(
                    usize::from(next_channel) < CHANNELS,
                    "song has more than {CHANNELS} melodic channels"
                );
                out = out.patch(patch_for(inst));
                let channel = ChannelNo(next_channel);
                // Power chords: root, fifth and octave as three lanes on one channel.
                let voicings: &[i32] = if inst.power { &[0, 7, 12] } else { &[0] };
                for &shift in voicings {
                    let lane = lane_text(&ch.rows, shift, Some(patch_index));
                    out = out
                        .lane(channel, &lane)
                        .unwrap_or_else(|e| panic!("converted lane does not parse: {e}"));
                }
                next_channel += 1;
                patch_index += 1;
            }
            None => {
                // Drums play on the drum machine; the lane's channel doesn't matter.
                let lane = lane_text(&ch.rows, 0, None);
                out = out
                    .lane(ChannelNo(0), &lane)
                    .unwrap_or_else(|e| panic!("converted drum lane does not parse: {e}"));
            }
        }
    }
    out
}

/// One tracker channel as a synthie lane: one cell per row, notes transposed by `shift`
/// semitones, and (for melodic lanes) the first cell switching to patch `patch` with `@n`.
fn lane_text(rows: &[Cell], shift: i32, patch: Option<usize>) -> String {
    let mut cells: Vec<String> = rows
        .iter()
        .map(|cell| match *cell {
            Cell::Note(n, arp) => {
                let mut s = note_name((i32::from(n) + shift).clamp(0, 127) as u8);
                if let Some([a, b, c]) = arp {
                    s.push_str(&format!("^{a:x}{b:x}{c:x}"));
                }
                s
            }
            Cell::Hold => ".".to_string(),
            Cell::Off => "-".to_string(),
            Cell::Hit(d) => match d {
                Drum::Kick => "k",
                Drum::Snare => "s",
                Drum::Hat => "h",
                Drum::Crash => "c",
            }
            .to_string(),
        })
        .collect();
    if let (Some(p), Some(first)) = (patch, cells.first_mut()) {
        // A note gets `@n` appended; a hold or release becomes a bare patch switch.
        *first = if first.starts_with(|c: char| c.is_ascii_uppercase()) {
            format!("{first}@{p}")
        } else {
            format!("@{p}")
        };
    }
    cells.join(" ")
}

/// A MIDI note as a synthie note name with sharps (60 is `C4`).
fn note_name(midi: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = i32::from(midi) / 12 - 1;
    format!("{}{octave}", NAMES[usize::from(midi % 12)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::songs;

    #[test]
    fn note_names_round_trip_through_synthie() {
        for n in [21u8, 40, 60, 61, 69, 83, 108] {
            let parsed =
                game_toolkit::prelude::synthie::pattern::parse_note(&note_name(n)).expect("parses");
            assert_eq!(parsed.0, n, "{}", note_name(n));
        }
    }

    #[test]
    fn every_song_converts() {
        for (name, song) in [
            ("title", songs::title()),
            ("game", songs::game()),
            ("extro", songs::extro()),
        ] {
            let s = to_synthie(&song);
            assert_eq!(s.len(), song.len(), "{name} keeps its length");
        }
    }

    #[test]
    fn power_chords_become_three_lanes() {
        let rows = [Cell::Note(40, None), Cell::Hold, Cell::Off];
        assert_eq!(lane_text(&rows, 0, Some(2)), "E2@2 . -");
        assert_eq!(lane_text(&rows, 7, None), "B2 . -");
        assert_eq!(lane_text(&rows, 12, None), "E3 . -");
    }
}
