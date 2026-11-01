//! The soundtrack, in tracker notation (see [`super::tracker`]).
//!
//! The title promises an epic: distorted power chords, pounding drums, a wailing lead. The
//! game then delivers a dainty music-box minuet. That's the joke. Finish the episode and the
//! epic is back, in major: the extro.

use super::synth::Wave;
use super::tracker::{Channel, Instrument, Song};

/// Rhythm guitar: chugging, distorted power chords.
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
pub fn extro() -> Song {
    let ring = Instrument {
        sustain: 0.7,
        decay: 2.5,
        release: 12.0,
        ..guitar()
    };
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

fn music_box_low() -> Instrument {
    Instrument {
        decay: 7.0,
        ..Instrument::new(Wave::Triangle, 0.22)
    }
}

/// In-game: a dainty minuet in F, three quarter-note rows per bar. No drums. Not one.
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
