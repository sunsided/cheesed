//! The soundtrack, in tracker notation (see [`super::tracker`]).
//!
//! The title promises an epic: distorted power chords, pounding drums, a wailing lead. The
//! game then delivers a dainty music-box minuet. That's the joke. Finish the episode and the
//! epic is back, in major: the extro.
//!
//! All three songs loop. The `|` bar lines are only for the reader; every channel of a song
//! must have the same number of rows, and the tests check that and that the length is a
//! whole number of bars. Drum channels have no instrument; the crash cymbal gets a
//! channel of its own because a channel holds only one cell per row, so a crash could not
//! otherwise land on the same row as a kick or snare.
//!
//! Levels are balanced by ear: instrument volumes are linear and summed, drums add at a fixed
//! 0.55 each, and the song's `gain` feeds the final `tanh` soft clipper in
//! [`super::tracker::Player::next_sample`].

use super::synth::Wave;
use super::tracker::{Channel, Instrument, Song};

/// Rhythm guitar: chugging, distorted power chords.
///
/// A sawtooth (the brightest wave) stacked into root, fifth and octave (`power`), with the
/// stacked voices detuned by 0.08 semitones so they beat against each other, then driven hard
/// (`drive` 4) into the `tanh` clipper. Clipping the summed chord is what produces the dense,
/// growling intermodulation of a distorted guitar. The envelope falls quickly (decay 9/s) to
/// a 0.45 sustain, and the very fast release (40/s, about 25 ms) cuts notes off almost at
/// once on `-`, like palm muting. Repeated notes re-trigger from silence, which gives the
/// eighth-note chug its attack.
fn guitar() -> Instrument {
    Instrument {
        sustain: 0.45,
        decay: 9.0,
        release: 40.0,
        drive: 4.0,
        power: true,
        detune: 0.08,
        ..Instrument::new(Wave::Saw, 0.42)
    }
}

/// Lead guitar: singing, overdriven, with a wide late vibrato.
///
/// A 30 % pulse with a slightly softened 10 ms attack and a slow decay (2/s) to a high 0.75
/// sustain, so long notes keep singing. The vibrato is wide (0.45 semitones) at 6.5 Hz and
/// only starts 150 ms into each note, the way a guitarist bends into a held note. The
/// release (10/s, about 0.1 s) lets phrases end smoothly.
///
/// Note: the `drive` 2.5 has no audible effect here. Drive is applied to the raw oscillator
/// output, and a single pulse wave is already exactly +-1, which the normalized clipper
/// leaves unchanged.
fn shred() -> Instrument {
    Instrument {
        attack: 0.01,
        sustain: 0.75,
        decay: 2.0,
        release: 10.0,
        vibrato: (0.45, 6.5, 0.15),
        drive: 2.5,
        ..Instrument::new(Wave::Pulse(0.3), 0.26)
    }
}

/// Bass: a triangle an octave below the guitar.
///
/// A fairly high sustain (0.7) keeps the low end steady, and a fast release (35/s) stops it
/// cleanly at a `-`. Repeated eighth notes are separated by the re-trigger itself, which
/// restarts the envelope from zero. The light `drive` of 1 rounds the triangle towards a
/// square (`tanh(2x) / tanh(2)`), adding odd harmonics so the bass still reads on small
/// speakers that cannot reproduce its fundamental.
fn rock_bass() -> Instrument {
    Instrument {
        sustain: 0.7,
        decay: 6.0,
        release: 35.0,
        drive: 1.0,
        ..Instrument::new(Wave::Triangle, 0.6)
    }
}

/// Title: heroic chiptune metal in E minor, eighth-note rows at 152 bpm.
///
/// 16 bars of 8 rows (128 rows, about 25 s per loop), in two halves of the same shape:
///
/// - Bars 1-4, the riff: E power chords chugging on eighths, answered by G-A and
///   D-C pickups, then a descending Bb-A-G-D turnaround (the Bb is the flat-fifth "blue"
///   note of E minor).
/// - Bars 5-8, the climb: C, D, then E chords (bVI-bVII-i), ending on a stop.
/// - Bars 9-16 repeat this, but the last two bars go to B (the dominant) and resolve to a
///   held E.
///
/// The lead rests for three bars, enters on a long B4 in bar 4, plays a phrase in the
/// middle octave over bars 5-8, then plays a new, higher eight-bar phrase that ends with
/// D#5 (the raised leading tone of harmonic minor) resolving to E5. The bass doubles the
/// riff one octave below the guitar and switches to octave-jumping eighths in the climb.
/// Drums play a straight rock beat with a fill in every fourth bar; crashes mark the section
/// starts and the pickups into them, and every bar of the final four.
pub fn title() -> Song {
    Song {
        rpm: 304.0,
        looped: true,
        gain: 0.95,
        channels: vec![
            Channel::new(
                Some(guitar()),
                "E2 E2 E2 E2 E2 E2 G2 A2 | E2 E2 E2 E2 E2 E2 D2 C2 |
                 E2 E2 E2 E2 E2 E2 G2 A2 | Bb2 . A2 . G2 . D2 . |
                 C2 . C2 C2 C2 . C2 C2 | D2 . D2 D2 D2 . D2 D2 |
                 E2 E2 E2 E2 E2 E2 E2 E2 | C2 . D2 . E2 . - . |
                 E2 E2 E2 E2 E2 E2 G2 A2 | E2 E2 E2 E2 E2 E2 D2 C2 |
                 E2 E2 E2 E2 E2 E2 G2 A2 | Bb2 . A2 . G2 . D2 . |
                 C2 . C2 C2 C2 . C2 C2 | D2 . D2 D2 D2 . D2 D2 |
                 B1 . B1 B1 B1 . B1 B1 | E2 . . . . . - . |",
            ),
            Channel::new(
                Some(shred()),
                "- . . . . . . . | - . . . . . . . |
                 - . . . . . . . | B4 . . . . . . . |
                 G4 . E4 . G4 . A4 . | B4 . . . A4 . G4 . |
                 A4 . . . G4 . F#4 . | E4 . . . . . - . |
                 E5 . . . D5 . B4 . | D5 . . . E5 . G5 . |
                 E5 . . . D5 . B4 . | A4 . B4 . D5 . . . |
                 E5 . G5 . E5 . D5 . | F#5 . . . E5 . D5 . |
                 D#5 . . . F#5 . . . | E5 . . . . . - . |",
            ),
            Channel::new(
                Some(rock_bass()),
                "E1 E1 E1 E1 E1 E1 G1 A1 | E1 E1 E1 E1 E1 E1 D1 C1 |
                 E1 E1 E1 E1 E1 E1 G1 A1 | Bb1 . A1 . G1 . D1 . |
                 C1 C2 C1 C2 C1 C2 C1 C2 | D1 D2 D1 D2 D1 D2 D1 D2 |
                 E1 E2 E1 E2 E1 E2 E1 E2 | C1 . D1 . E1 . - . |
                 E1 E1 E1 E1 E1 E1 G1 A1 | E1 E1 E1 E1 E1 E1 D1 C1 |
                 E1 E1 E1 E1 E1 E1 G1 A1 | Bb1 . A1 . G1 . D1 . |
                 C1 C2 C1 C2 C1 C2 C1 C2 | D1 D2 D1 D2 D1 D2 D1 D2 |
                 B0 B1 B0 B1 B0 B1 B0 B1 | E1 . . . . . - . |",
            ),
            Channel::new(
                None,
                "k h s h k k s h | k h s h k k s h | k h s h k k s h | s s k s s k s s |
                 k h s h k k s h | k h s h k k s h | k h s h k k s h | k . s . k s s s |
                 k h s h k k s h | k h s h k k s h | k h s h k k s h | s s k s s k s s |
                 k h s h k k s h | k h s h k k s h | k h s h k k s h | k . s . k . k . |",
            ),
            Channel::new(
                None,
                "c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . c . . . |
                 c . . . . . . . | - . . . . . . . | c . . . . . . . | - . . . c . . . |
                 c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . c . . . |
                 c . . . . . . . | c . . . . . . . | c . . . . . . . | c . . . . . . . |",
            ),
        ],
    }
}

/// Episode ending: the title's riff turned triumphant. E major, eighth-note rows at 120 bpm,
/// the title melody quoted in major, then a bVI-bVII-I climb (C-D-E) with a doubled lead.
///
/// 16 bars of 8 rows (128 rows, 32 s per loop). Chords: E-C#-A-B (I-vi-IV-V), then
/// E-G#-A-B, then C-D-E twice (bars 9-16), each ending on a held E. The lead plays
/// over all 16 bars and climbs to E6 at the end; in bars 9-16 a second, quieter lead doubles
/// it an octave lower. The bass plays steady root eighths, reaching down to A0 (27.5 Hz) and
/// G#0 (26 Hz), below what most speakers reproduce, so what is heard there is mainly the
/// triangle's harmonics. The drums switch to a half-time feel (kick on beat 1, snare on beat
/// 3) with fills every fourth bar, and crashes come thicker towards the end.
pub fn extro() -> Song {
    // The rhythm guitar with a slower decay to a higher sustain and a longer release, so the
    // chords ring out instead of chugging.
    let ring = Instrument {
        sustain: 0.7,
        decay: 2.5,
        release: 12.0,
        ..guitar()
    };
    // The doubling lead: quieter than the main lead, with a shallower, slightly slower and
    // later vibrato, so the two do not wobble in lockstep.
    let double = Instrument {
        volume: 0.16,
        vibrato: (0.3, 6.0, 0.2),
        ..shred()
    };
    Song {
        rpm: 240.0,
        looped: true,
        gain: 0.95,
        channels: vec![
            Channel::new(
                Some(ring),
                "E2 . . . E2 . E2 E2 | C#2 . . . C#2 . C#2 C#2 |
                 A1 . . . A1 . A1 A1 | B1 . . . B1 B1 B1 B1 |
                 E2 . . . E2 . E2 E2 | G#1 . . . G#1 . G#1 G#1 |
                 A1 . . . A1 . A1 A1 | B1 . B1 B1 B1 B1 B1 B1 |
                 C2 . . . C2 . C2 C2 | D2 . . . D2 . D2 D2 |
                 E2 . . . E2 . E2 E2 | E2 . . . . . - . |
                 C2 . . . C2 . C2 C2 | D2 . . . D2 . D2 D2 |
                 E2 . . . E2 . E2 E2 | E2 . . . . . - . |",
            ),
            Channel::new(
                Some(shred()),
                "E4 . . . G#4 . B4 . | C#5 . . . B4 . G#4 . |
                 A4 . B4 . C#5 . E5 . | F#5 . . . . . D#5 . |
                 E5 . . . B4 . G#4 . | B4 . . . D#5 . B4 . |
                 C#5 . B4 . A4 . C#5 . | B4 . . . . . - . |
                 C5 . . . E5 . G5 . | F#5 . . . A5 . F#5 . |
                 G#5 . . . . . . . | B5 . . . . . - . |
                 G5 . E5 . C5 . E5 . | F#5 . A5 . D6 . A5 . |
                 B5 . . . G#5 . . . | E6 . . . . . - . |",
            ),
            Channel::new(
                Some(double),
                "- . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . . . . . |
                 - . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . . . . . |
                 C4 . . . E4 . G4 . | F#4 . . . A4 . F#4 . |
                 G#4 . . . . . . . | B4 . . . . . - . |
                 G4 . E4 . C4 . E4 . | F#4 . A4 . D5 . A4 . |
                 B4 . . . G#4 . . . | E5 . . . . . - . |",
            ),
            Channel::new(
                Some(rock_bass()),
                "E1 E1 E1 E1 E1 E1 E1 E1 | C#1 C#1 C#1 C#1 C#1 C#1 C#1 C#1 |
                 A0 A0 A0 A0 A0 A0 A0 A0 | B0 B0 B0 B0 B0 B0 B0 B0 |
                 E1 E1 E1 E1 E1 E1 E1 E1 | G#0 G#0 G#0 G#0 G#0 G#0 G#0 G#0 |
                 A0 A0 A0 A0 A0 A0 A0 A0 | B0 B0 B0 B0 B0 B0 B0 B0 |
                 C1 C1 C1 C1 C1 C1 C1 C1 | D1 D1 D1 D1 D1 D1 D1 D1 |
                 E1 E1 E1 E1 E1 E1 E1 E1 | E1 . . . . . - . |
                 C1 C1 C1 C1 C1 C1 C1 C1 | D1 D1 D1 D1 D1 D1 D1 D1 |
                 E1 E1 E1 E1 E1 E1 E1 E1 | E1 . . . . . - . |",
            ),
            Channel::new(
                None,
                "k . h . s . h . | k . h k s . h . | k . h . s . h . | k . h k s . s s |
                 k . h . s . h . | k . h k s . h . | k . h . s . h . | s s s s k k s s |
                 k . h . s . h . | k . h k s . h . | k . h . s . h . | s s k s s k s s |
                 k . h . s . h . | k . h k s . h . | k . h . s . h . | k . s . k . k . |",
            ),
            Channel::new(
                None,
                "c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . . . . . |
                 c . . . . . . . | - . . . . . . . | - . . . . . . . | - . . . c . . . |
                 c . . . . . . . | c . . . . . . . | c . . . . . . . | - . . . c . . . |
                 c . . . . . . . | c . . . . . . . | c . . . . . . . | c . . . . . . . |",
            ),
        ],
    }
}

/// A music box: a plucked, bell-like triangle that dies away.
///
/// A near-instant 2 ms attack, then an exponential decay at 5.5/s towards a sustain of 0,
/// so every note fades out on its own (time constant about 0.18 s; at the minuet's tempo it
/// is below 10 % by the next beat) whether or not it is held. A tiny always-on vibrato
/// (0.05 semitones at 5 Hz) adds a faint mechanical waver.
fn music_box() -> Instrument {
    Instrument {
        attack: 0.002,
        sustain: 0.0,
        decay: 5.5,
        release: 6.0,
        vibrato: (0.05, 5.0, 0.0),
        ..Instrument::new(Wave::Triangle, 0.34)
    }
}

/// The minuet's accompaniment: a quieter triangle for the broken chords.
///
/// Apart from a slightly faster decay (7/s) it keeps the default envelope, including its 0.6
/// sustain, so unlike the melody these notes do not die away: each holds at 60 % until the
/// next note re-triggers the channel, giving a soft, continuous bed under the plucked tune.
fn music_box_low() -> Instrument {
    Instrument {
        decay: 7.0,
        ..Instrument::new(Wave::Triangle, 0.22)
    }
}

/// In-game: a dainty minuet in F, three quarter-note rows per bar. No drums. Not one.
///
/// 132 rpm is 132 bpm in 3/4; 16 bars (48 rows) make a loop of about 22 s, in two 8-bar
/// phrases. The melody runs on `music_box`; the low channel plays the harmony as broken
/// chords, one chord tone per beat: F, C, C, C7, F, Bb, C, F, then F, C, F, C, Bb, C, C7, F.
/// The overall `gain` is lower than the rock songs', and with no drums the minuet stays much
/// quieter, which the tests below check.
pub fn game() -> Song {
    Song {
        rpm: 132.0,
        looped: true,
        gain: 0.8,
        channels: vec![
            Channel::new(
                Some(music_box()),
                "C5 F5 A5 | G5 . F5 | E5 G5 C6 | Bb5 A5 G5 |
                 A5 F5 C5 | D5 E5 F5 | G5 . C5 | F5 . . |
                 A5 . C6 | Bb5 A5 G5 | F5 . A5 | G5 F5 E5 |
                 F5 D5 Bb4 | C5 E5 G5 | F5 . E5 | F5 . - |",
            ),
            Channel::new(
                Some(music_box_low()),
                "F3 A3 C4 | C3 E3 G3 | C3 E3 G3 | C3 E3 Bb3 |
                 F3 A3 C4 | Bb2 D3 F3 | C3 E3 G3 | F3 A3 C4 |
                 F3 A3 C4 | C3 E3 G3 | F3 A3 C4 | C3 E3 G3 |
                 Bb2 D3 F3 | C3 E3 G3 | C3 E3 Bb3 | F3 C3 F2 |",
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::tracker::Player;

    fn render(song: Song, secs: f32) -> Vec<f32> {
        let mut p = Player::new(song);
        (0..(secs * crate::sound::synth::SR) as usize)
            .map(|_| p.next_sample())
            .collect()
    }

    #[test]
    fn channels_line_up() {
        for (name, song) in [("title", title()), ("game", game()), ("extro", extro())] {
            let len = song.channels[0].rows.len();
            for (i, c) in song.channels.iter().enumerate() {
                assert_eq!(c.rows.len(), len, "{name} channel {i}");
            }
        }
        assert_eq!(title().len() % 8, 0, "title is in 4/4 bars");
        assert_eq!(extro().len() % 8, 0, "extro is in 4/4 bars");
        assert_eq!(game().len() % 3, 0, "the minuet is in 3/4");
    }

    #[test]
    fn songs_are_loud_where_they_should_be() {
        let rms = |pcm: &[f32]| (pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len() as f32).sqrt();
        let rock = render(title(), 6.0);
        let minuet = render(game(), 6.0);
        assert!(
            rock.iter()
                .chain(&minuet)
                .all(|s| s.is_finite() && s.abs() <= 1.0)
        );
        assert!(
            rms(&rock) > 2.0 * rms(&minuet),
            "the title should rock harder than the game"
        );
    }
}
