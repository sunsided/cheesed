//! The soundtrack, as synthie songs played live by game-toolkit's `synth` feature
//! (`Audio::play_synth_sound` in `app.rs`).
//!
//! The title promises an epic: distorted power chords, pounding drums, a wailing lead. The
//! game then delivers a dainty music-box minuet. That's the joke. Finish the episode and the
//! epic is back, in major: the extro.
//!
//! Notation (synthie's `pattern` module): one cell per row, `C4` / `F#3` / `Bb2` a note, `.`
//! hold, `-` release, `k s h c` kick, snare, hat, crash, `|` a bar line for readability, and
//! `@n` on a lane's first note to select the song's patch `n`. Each lane is monophonic, so the
//! power chords are three lanes on one channel: the riff, a fifth up and an octave up.

use game_toolkit::prelude::synth::{ChannelNo, Song, SynthParams, Tempo};
use game_toolkit::prelude::synthie::params::{
    EnvParams, FilterParams, GlobalParams, LfoShape, LfoTarget, OscParams, Waveform,
};
use game_toolkit::prelude::synthie::pattern::parse_note;

/// One synthie unit of pitch modulation is a tenth of an octave (`freq * 2^(pitch / 10)`),
/// i.e. 1.2 semitones; vibrato depths below are given in semitones and converted with this.
const SEMITONES_PER_PITCH_UNIT: f32 = 1.2;

/// A plain chip patch: `wave` through a wide-open low-pass (bright, no resonance), with an
/// amplitude envelope of `attack`, `decay` and `release` in seconds and `sustain` as a level,
/// at `volume`. No vibrato, no drive.
fn patch(
    wave: Waveform,
    volume: f32,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
) -> SynthParams {
    // synthie's sub-parameter structs have no `Default` of their own; each takes the rest of
    // its fields from the full default patch (a low-pass filter, no LFO depth, ...).
    let d = SynthParams::default();
    SynthParams {
        osc: OscParams {
            waveform: wave,
            ..d.osc
        },
        env: EnvParams {
            attack,
            decay,
            sustain,
            release,
            ..d.env
        },
        filter: FilterParams {
            cutoff: 12_000.0,
            resonance: 0.0,
            ..d.filter
        },
        global: GlobalParams { volume, ..d.global },
        ..d
    }
}

/// `p` with a sine vibrato of `semitones` depth at `rate` Hz on the pitch LFO.
fn vibrato(mut p: SynthParams, semitones: f32, rate: f32) -> SynthParams {
    p.lfo.lfo_target = LfoTarget::Pitch;
    p.lfo.lfo_shape = LfoShape::Sine;
    p.lfo.lfo_rate = rate;
    p.lfo.lfo_depth = semitones / SEMITONES_PER_PITCH_UNIT;
    p
}

/// `p` with `drive` (0..1) of the filter's pre-saturation: the guitar crunch.
fn drive(mut p: SynthParams, drive: f32) -> SynthParams {
    p.filter.drive = drive;
    p
}

/// Rhythm guitar: chugging, overdriven sawtooth power chords.
fn guitar() -> SynthParams {
    drive(
        patch(Waveform::Sawtooth, 0.42, 0.004, 0.11, 0.45, 0.025),
        0.8,
    )
}

/// The extro's rhythm guitar: slower decay to a higher sustain and a longer release, so the
/// chords ring out instead of chugging.
fn ring() -> SynthParams {
    drive(patch(Waveform::Sawtooth, 0.42, 0.004, 0.4, 0.7, 0.083), 0.8)
}

/// Lead guitar: a singing, overdriven 30 % pulse with a wide vibrato.
fn shred() -> SynthParams {
    let mut p = patch(Waveform::Pulse, 0.26, 0.01, 0.5, 0.75, 0.1);
    p.osc.pulse_width = 0.3;
    drive(vibrato(p, 0.45, 6.5), 0.71)
}

/// The extro's doubling lead: quieter, with a shallower and slightly slower vibrato, so the
/// two leads don't wobble in lockstep.
fn double() -> SynthParams {
    let mut p = shred();
    p.global.volume = 0.16;
    vibrato(p, 0.3, 6.0)
}

/// Rock bass: a lightly driven triangle.
fn rock_bass() -> SynthParams {
    drive(patch(Waveform::Triangle, 0.6, 0.004, 0.17, 0.7, 0.03), 0.5)
}

/// A music box: a plucked, bell-like triangle that dies away, with a faint vibrato.
fn music_box() -> SynthParams {
    vibrato(
        patch(Waveform::Triangle, 0.34, 0.002, 0.18, 0.0, 0.17),
        0.05,
        5.0,
    )
}

/// The music box's accompaniment: a softer triangle that holds while a note lasts.
fn music_box_low() -> SynthParams {
    patch(Waveform::Triangle, 0.22, 0.004, 0.14, 0.6, 0.06)
}

/// `pattern` with every note moved by `semitones` (holds, releases and bar lines kept), for
/// the upper voices of the power chords. A lane-selecting `@n` is dropped: the root lane on
/// the same channel already selects the patch.
fn transpose(pattern: &str, semitones: i8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    pattern
        .split_whitespace()
        .map(|cell| {
            let note = cell.split('@').next().unwrap_or(cell);
            match parse_note(note) {
                Ok(n) => {
                    let m = (i16::from(n.0) + i16::from(semitones)).clamp(0, 127) as usize;
                    format!("{}{}", NAMES[m % 12], m as i32 / 12 - 1)
                }
                Err(_) => cell.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Add the guitar part on `channel` as a power chord: the riff, a fifth up and an octave up.
fn power_chords(song: Song, channel: u8, riff: &str) -> Song {
    let ch = ChannelNo(channel);
    song.lane(ch, riff)
        .and_then(|s| s.lane(ch, &transpose(riff, 7)))
        .and_then(|s| s.lane(ch, &transpose(riff, 12)))
        .expect("guitar lanes parse")
}

/// Title: heroic chiptune metal in E minor, eighth-note rows at 152 bpm.
pub fn title() -> Song {
    let song = Song::new(Tempo::bpm(152.0, 2))
        .patch(guitar())
        .patch(shred())
        .patch(rock_bass());
    power_chords(
        song,
        0,
        "E2@0 E2 E2 E2 E2 E2 G2 A2 | E2 E2 E2 E2 E2 E2 D2 C2 |
                 E2 E2 E2 E2 E2 E2 G2 A2 | Bb2 . A2 . G2 . D2 . |
                 C2 . C2 C2 C2 . C2 C2 | D2 . D2 D2 D2 . D2 D2 |
                 E2 E2 E2 E2 E2 E2 E2 E2 | C2 . D2 . E2 . - . |
                 E2 E2 E2 E2 E2 E2 G2 A2 | E2 E2 E2 E2 E2 E2 D2 C2 |
                 E2 E2 E2 E2 E2 E2 G2 A2 | Bb2 . A2 . G2 . D2 . |
                 C2 . C2 C2 C2 . C2 C2 | D2 . D2 D2 D2 . D2 D2 |
                 B1 . B1 B1 B1 . B1 B1 | E2 . . . . . - . |",
    )
    .lane(
        ChannelNo(1),
        "@1 . . . . . . . | - . . . . . . . |
                 - . . . . . . . | B4 . . . . . . . |
                 G4 . E4 . G4 . A4 . | B4 . . . A4 . G4 . |
                 A4 . . . G4 . F#4 . | E4 . . . . . - . |
                 E5 . . . D5 . B4 . | D5 . . . E5 . G5 . |
                 E5 . . . D5 . B4 . | A4 . B4 . D5 . . . |
                 E5 . G5 . E5 . D5 . | F#5 . . . E5 . D5 . |
                 D#5 . . . F#5 . . . | E5 . . . . . - . |",
    )
    .and_then(|s| {
        s.lane(
            ChannelNo(2),
            "E1@2 E1 E1 E1 E1 E1 G1 A1 | E1 E1 E1 E1 E1 E1 D1 C1 |
                 E1 E1 E1 E1 E1 E1 G1 A1 | Bb1 . A1 . G1 . D1 . |
                 C1 C2 C1 C2 C1 C2 C1 C2 | D1 D2 D1 D2 D1 D2 D1 D2 |
                 E1 E2 E1 E2 E1 E2 E1 E2 | C1 . D1 . E1 . - . |
                 E1 E1 E1 E1 E1 E1 G1 A1 | E1 E1 E1 E1 E1 E1 D1 C1 |
                 E1 E1 E1 E1 E1 E1 G1 A1 | Bb1 . A1 . G1 . D1 . |
                 C1 C2 C1 C2 C1 C2 C1 C2 | D1 D2 D1 D2 D1 D2 D1 D2 |
                 B0 B1 B0 B1 B0 B1 B0 B1 | E1 . . . . . - . |",
        )
    })
    .and_then(|s| {
        s.lane(
            ChannelNo(0),
            "k h s h k k s h | k h s h k k s h | k h s h k k s h | s s k s s k s s |
                 k h s h k k s h | k h s h k k s h | k h s h k k s h | k . s . k s s s |
                 k h s h k k s h | k h s h k k s h | k h s h k k s h | s s k s s k s s |
                 k h s h k k s h | k h s h k k s h | k h s h k k s h | k . s . k . k . |",
        )
    })
    .and_then(|s| {
        s.lane(
            ChannelNo(0),
            "c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . c . . . |
                 c . . . . . . . | - . . . . . . . | c . . . . . . . | - . . . c . . . |
                 c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . c . . . |
                 c . . . . . . . | c . . . . . . . | c . . . . . . . | c . . . . . . . |",
        )
    })
    .expect("title parses")
}

/// Episode ending: the title's riff turned triumphant. E major, eighth-note rows at 120 bpm,
/// the title melody quoted in major, then a bVI-bVII-I climb (C-D-E) with a doubled lead.
pub fn extro() -> Song {
    let song = Song::new(Tempo::bpm(120.0, 2))
        .patch(ring())
        .patch(shred())
        .patch(double())
        .patch(rock_bass());
    power_chords(
        song,
        0,
        "E2@0 . . . E2 . E2 E2 | C#2 . . . C#2 . C#2 C#2 |
                 A1 . . . A1 . A1 A1 | B1 . . . B1 B1 B1 B1 |
                 E2 . . . E2 . E2 E2 | G#1 . . . G#1 . G#1 G#1 |
                 A1 . . . A1 . A1 A1 | B1 . B1 B1 B1 B1 B1 B1 |
                 C2 . . . C2 . C2 C2 | D2 . . . D2 . D2 D2 |
                 E2 . . . E2 . E2 E2 | E2 . . . . . - . |
                 C2 . . . C2 . C2 C2 | D2 . . . D2 . D2 D2 |
                 E2 . . . E2 . E2 E2 | E2 . . . . . - . |",
    )
    .lane(
        ChannelNo(1),
        "E4@1 . . . G#4 . B4 . | C#5 . . . B4 . G#4 . |
                 A4 . B4 . C#5 . E5 . | F#5 . . . . . D#5 . |
                 E5 . . . B4 . G#4 . | B4 . . . D#5 . B4 . |
                 C#5 . B4 . A4 . C#5 . | B4 . . . . . - . |
                 C5 . . . E5 . G5 . | F#5 . . . A5 . F#5 . |
                 G#5 . . . . . . . | B5 . . . . . - . |
                 G5 . E5 . C5 . E5 . | F#5 . A5 . D6 . A5 . |
                 B5 . . . G#5 . . . | E6 . . . . . - . |",
    )
    .and_then(|s| {
        s.lane(
            ChannelNo(2),
            "@2 . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . . . . . |
                 - . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . . . . . |
                 C4 . . . E4 . G4 . | F#4 . . . A4 . F#4 . |
                 G#4 . . . . . . . | B4 . . . . . - . |
                 G4 . E4 . C4 . E4 . | F#4 . A4 . D5 . A4 . |
                 B4 . . . G#4 . . . | E5 . . . . . - . |",
        )
    })
    .and_then(|s| {
        s.lane(
            ChannelNo(3),
            "E1@3 E1 E1 E1 E1 E1 E1 E1 | C#1 C#1 C#1 C#1 C#1 C#1 C#1 C#1 |
                 A0 A0 A0 A0 A0 A0 A0 A0 | B0 B0 B0 B0 B0 B0 B0 B0 |
                 E1 E1 E1 E1 E1 E1 E1 E1 | G#0 G#0 G#0 G#0 G#0 G#0 G#0 G#0 |
                 A0 A0 A0 A0 A0 A0 A0 A0 | B0 B0 B0 B0 B0 B0 B0 B0 |
                 C1 C1 C1 C1 C1 C1 C1 C1 | D1 D1 D1 D1 D1 D1 D1 D1 |
                 E1 E1 E1 E1 E1 E1 E1 E1 | E1 . . . . . - . |
                 C1 C1 C1 C1 C1 C1 C1 C1 | D1 D1 D1 D1 D1 D1 D1 D1 |
                 E1 E1 E1 E1 E1 E1 E1 E1 | E1 . . . . . - . |",
        )
    })
    .and_then(|s| {
        s.lane(
            ChannelNo(0),
            "k . h . s . h . | k . h k s . h . | k . h . s . h . | k . h k s . s s |
                 k . h . s . h . | k . h k s . h . | k . h . s . h . | s s s s k k s s |
                 k . h . s . h . | k . h k s . h . | k . h . s . h . | s s k s s k s s |
                 k . h . s . h . | k . h k s . h . | k . h . s . h . | k . s . k . k . |",
        )
    })
    .and_then(|s| {
        s.lane(
            ChannelNo(0),
            "c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . . . . . |
                 c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . c . . . |
                 c . . . . . . . | c . . . . . . . | c . . . . . . . | - . . . c . . . |
                 c . . . . . . . | c . . . . . . . | c . . . . . . . | c . . . . . . . |",
        )
    })
    .expect("extro parses")
}

/// In-game: a dainty minuet in F, three quarter-note rows per bar. No drums. Not one.
pub fn game() -> Song {
    Song::new(Tempo::bpm(132.0, 1))
        .patch(music_box())
        .patch(music_box_low())
        .lane(
            ChannelNo(0),
            "C5@0 F5 A5 | G5 . F5 | E5 G5 C6 | Bb5 A5 G5 |
                 A5 F5 C5 | D5 E5 F5 | G5 . C5 | F5 . . |
                 A5 . C6 | Bb5 A5 G5 | F5 . A5 | G5 F5 E5 |
                 F5 D5 Bb4 | C5 E5 G5 | F5 . E5 | F5 . - |",
        )
        .and_then(|s| {
            s.lane(
                ChannelNo(1),
                "F3@1 A3 C4 | C3 E3 G3 | C3 E3 G3 | C3 E3 Bb3 |
                 F3 A3 C4 | Bb2 D3 F3 | C3 E3 G3 | F3 A3 C4 |
                 F3 A3 C4 | C3 E3 G3 | F3 A3 C4 | C3 E3 G3 |
                 Bb2 D3 F3 | C3 E3 G3 | C3 E3 Bb3 | F3 C3 F2 |",
            )
        })
        .expect("minuet parses")
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_toolkit::prelude::Synth;

    #[test]
    fn songs_have_whole_bars() {
        assert_eq!(title().len(), 128);
        assert_eq!(extro().len(), 128);
        assert_eq!(title().len() % 8, 0, "title is in 4/4 bars");
        assert_eq!(game().len() % 3, 0, "the minuet is in 3/4");
    }

    #[test]
    fn transpose_moves_notes_only() {
        assert_eq!(transpose("E2@0 . - | Bb1", 7), "B2 . - | F2");
        assert_eq!(transpose("E2 E2", 12), "E3 E3");
    }

    #[test]
    fn songs_are_loud_where_they_should_be() {
        let rms = |pcm: &[f32]| (pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len() as f32).sqrt();
        let mut synth = Synth::new(32_000).with_target_peak(0.0);
        let rock = synth.render_song(title().one_shot(), 0.0);
        let minuet = synth.render_song(game().one_shot(), 0.0);
        assert!(rock.iter().chain(&minuet).all(|s| s.is_finite()));
        assert!(
            rms(&rock) > 2.0 * rms(&minuet),
            "the title should rock harder than the game"
        );
    }
}
