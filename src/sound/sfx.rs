//! One-shot sound effects, rendered to PCM once at startup.
//!
//! Each effect is a small recipe: a closure of time `t` (seconds) that drives one or more
//! oscillators or noise generators from [`super::synth`], rendered with [`render`] and then
//! scaled with [`normalize`] to a chosen peak level. Rendering everything up front means
//! playing an effect is just handing a ready buffer to the audio backend, and the recipes
//! need not be cheap enough for the audio thread.
//!
//! Conventions in the recipes: `k = t / secs` is normalized progress 0..1 through the
//! effect; `(1.0 - k)` is a linear fade-out; `(t * a).min(1.0)` is a linear fade-in over
//! `1 / a` seconds; `exp(-t * a)` is an exponential decay with time constant `1 / a` seconds.
//! Note that the `sin(t * w)` wobbles take `w` in radians per second, not Hz, so
//! `sin(t * 38.0)` wobbles at about 6 Hz (divide by 2 pi).

use super::synth::{Noise, Osc, SR, Wave, midi_hz, normalize, render};

/// Every effect in the game.
///
/// Each field is mono PCM at [`super::SAMPLE_RATE`], with samples in -1..=1. `app.rs` picks
/// the buffer for a game event and plays it at a per-call volume.
pub struct Bank {
    /// The mouse takes a step: a 25 ms square-wave tick gliding from 1300 to 1000 Hz.
    pub step: Vec<f32>,
    /// The mouse bumps into something immovable: a 60 ms low triangle thud falling from
    /// 180 to 120 Hz.
    pub bump: Vec<f32>,
    /// A wedge scraping over the floorboards.
    pub push: Vec<f32>,
    /// A wheel rolling: a low rumble.
    pub roll: Vec<f32>,
    /// Brie sliding away: "fwoosh".
    pub slide: Vec<f32>,
    /// Straining against a wheel without a run-up.
    pub too_heavy: Vec<f32>,
    /// A cheese drops into a mouse hole.
    pub plop: Vec<f32>,
    /// A cheese is pushed back out of a hole.
    ///
    /// A plop in reverse: a 90 ms triangle rising from 300 to 700 Hz.
    pub unplop: Vec<f32>,
    /// The cat catches the mouse: "mee-OW" and a hiss.
    pub meow: Vec<f32>,
    /// One move taken back: a 50 ms thin (12.5 % pulse) blip falling 900 to 600 Hz.
    pub undo: Vec<f32>,
    /// The level restarts: a longer 180 ms zap falling two octaves, 1200 to 300 Hz.
    pub restart: Vec<f32>,
    /// Level solved: a quick rising C major arpeggio, C5 E5 G5 C6 G5 C6 E6, 60 ms per note.
    pub solved: Vec<f32>,
    /// The menu cursor moves: a 30 ms blip rising 900 to 1100 Hz.
    pub menu: Vec<f32>,
    /// A choice is accepted: a very fast G4 C5 E5 G5 arpeggio, 35 ms per note.
    pub confirm: Vec<f32>,
    /// A choice is refused (e.g. a locked level): a 180 ms buzzy square dropping an
    /// octave, 220 to 110 Hz.
    pub nope: Vec<f32>,
    /// A big C major arpeggio climbing three octaves, C4 up to C7, 80 ms per note, with a
    /// 0.6 s ring-out.
    pub fanfare: Vec<f32>,
    /// The crack and roll of thunder, for the title's lightning (3.2 s).
    pub thunder: Vec<f32>,
    /// The mouse, bored: "squeak-eek!"
    pub squeak: Vec<f32>,
    /// Caught: a slow, sad, falling line.
    pub defeat: Vec<f32>,
}

impl Bank {
    /// Render every effect. This is pure computation; do it once at startup and keep the
    /// bank.
    pub fn new() -> Self {
        Self {
            step: sweep(1300.0, 1000.0, 0.025, Wave::Pulse(0.5), 0.18),
            bump: sweep(180.0, 120.0, 0.06, Wave::Triangle, 0.5),
            push: scrape(),
            roll: roll(),
            slide: slide(),
            too_heavy: too_heavy(),
            plop: plop(),
            unplop: sweep(300.0, 700.0, 0.09, Wave::Triangle, 0.5),
            meow: meow(),
            undo: sweep(900.0, 600.0, 0.05, Wave::Pulse(0.125), 0.3),
            restart: sweep(1200.0, 300.0, 0.18, Wave::Pulse(0.25), 0.35),
            solved: arpeggio(&[72, 76, 79, 84, 79, 84, 88], 0.06, Wave::Pulse(0.25), 0.3),
            menu: sweep(900.0, 1100.0, 0.03, Wave::Pulse(0.25), 0.3),
            confirm: arpeggio(&[67, 72, 76, 79], 0.035, Wave::Pulse(0.25), 0.1),
            nope: sweep(220.0, 110.0, 0.18, Wave::Pulse(0.5), 0.5),
            fanfare: arpeggio(
                &[60, 64, 67, 72, 67, 72, 76, 79, 84, 88, 91, 96],
                0.08,
                Wave::Pulse(0.25),
                0.6,
            ),
            thunder: thunder(),
            squeak: squeak(),
            defeat: defeat(),
        }
    }
}

impl Default for Bank {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick notes in a row, each decaying, with a tail.
///
/// Plays the MIDI `notes` back to back, `step` seconds each, on one oscillator. Every note
/// starts at full level and decays as `exp(-14 t)` from its own onset (time constant about
/// 70 ms), which gives a plucked, chiming articulation. The last note keeps sounding for
/// another `tail` seconds. On top, the whole buffer is faded linearly from full at the start
/// to zero at the end, so later notes are a little quieter and it never stops with a click.
/// The result is normalized to a peak of 0.6.
fn arpeggio(notes: &[u8], step: f32, wave: Wave, tail: f32) -> Vec<f32> {
    let total = step * notes.len() as f32 + tail;
    let mut osc = Osc::default();
    let mut buf = render(total, |t| {
        let i = ((t / step) as usize).min(notes.len() - 1);
        let local = t - i as f32 * step;
        let env = (-local * 14.0).exp() * (1.0 - t / total).max(0.0);
        osc.sample(wave, midi_hz(notes[i])) * env
    });
    normalize(&mut buf, 0.6);
    buf
}

/// Exponential pitch sweep.
///
/// Glides from `from` to `to` Hz over `secs` seconds as `from * (to / from)^k`, i.e. by an
/// equal number of semitones per unit time, which the ear hears as a straight glide. The
/// amplitude fades linearly to zero, and the result is normalized to `peak`. Most of the UI
/// blips and simple thuds are a sweep with a different waveform and range.
fn sweep(from: f32, to: f32, secs: f32, wave: Wave, peak: f32) -> Vec<f32> {
    let mut osc = Osc::default();
    let mut buf = render(secs, |t| {
        let k = t / secs;
        let hz = from * (to / from).powf(k);
        osc.sample(wave, hz) * (1.0 - k)
    });
    normalize(&mut buf, peak);
    buf
}

/// A wedge scraping across the floor (90 ms).
///
/// Dull noise (clocked at 2.5 kHz, so little treble) chopped by a gate that alternates
/// between full and half level every 1/220 s. That 110 Hz amplitude flutter turns a smooth
/// hiss into a rough, juddering scrape. Linear fade-out, peak 0.35.
fn scrape() -> Vec<f32> {
    let mut n = Noise::default();
    let mut buf = render(0.09, |t| {
        let grain = if ((t * 220.0) as u32).is_multiple_of(2) {
            1.0
        } else {
            0.5
        };
        n.sample(2500.0) * grain * (1.0 - t / 0.09)
    });
    normalize(&mut buf, 0.35);
    buf
}

/// A cheese wheel rolling (160 ms).
///
/// A low thud, a triangle whose pitch drops from 125 Hz to 55 Hz (`55 + 70 exp(-30 t)`) and
/// whose level decays as `exp(-14 t)`, plus a quiet bed of dark noise (clocked at 900 Hz)
/// that fades linearly for the rumble of the wheel on the floor. Peak 0.7.
fn roll() -> Vec<f32> {
    let mut osc = Osc::default();
    let mut n = Noise::default();
    let mut buf = render(0.16, |t| {
        let thud = osc.sample(Wave::Triangle, 55.0 + 70.0 * (-t * 30.0).exp()) * (-t * 14.0).exp();
        thud + n.sample(900.0) * 0.25 * (1.0 - t / 0.16)
    });
    normalize(&mut buf, 0.7);
    buf
}

/// Brie sliding away: "fwoosh" (280 ms).
///
/// A noise whoosh whose clock sweeps from 3 kHz to 12 kHz, so it brightens as it goes, under
/// a parabolic envelope `4 k (1 - k)` that swells to full in the middle and dies away. On
/// top, a quieter triangle rising from 300 to 900 Hz and fading out adds a sense of speed.
/// Peak 0.4.
fn slide() -> Vec<f32> {
    let mut n = Noise::default();
    let mut osc = Osc::default();
    let mut buf = render(0.28, |t| {
        let k = t / 0.28;
        let whoosh = n.sample(3000.0 + 9000.0 * k) * (k * (1.0 - k) * 4.0);
        let tone = osc.sample(Wave::Triangle, 300.0 + 600.0 * k) * (1.0 - k) * 0.4;
        whoosh * 0.6 + tone
    });
    normalize(&mut buf, 0.4);
    buf
}

/// Pushing a wheel without a run-up: a strained grunt (220 ms).
///
/// A 30 % pulse around 105 Hz whose pitch wobbles by +-12 Hz at `45` rad/s (about 7 Hz),
/// like a voice under effort. It fades in over 33 ms and out linearly. Peak 0.45.
fn too_heavy() -> Vec<f32> {
    // "Hnngh": a strained low buzz with a wobble.
    let mut osc = Osc::default();
    let mut buf = render(0.22, |t| {
        let hz = 105.0 + 12.0 * (t * 45.0).sin();
        let env = (t * 30.0).min(1.0) * (1.0 - t / 0.22);
        osc.sample(Wave::Pulse(0.3), hz) * env
    });
    normalize(&mut buf, 0.45);
    buf
}

/// A cheese dropping into a mouse hole: a pop and a chime.
///
/// The pop is a 70 ms triangle falling exponentially from 900 Hz to 270 Hz
/// (`900 * 0.3^(t / 0.07)`) with a linear fade, like a cork. Directly after it comes a tiny
/// C6 E6 G6 chime ([`arpeggio`] on a thin 12.5 % pulse, 40 ms per note), rescaled to sit at
/// half the pop's level.
fn plop() -> Vec<f32> {
    // A cork-like pop dropping in pitch, then a little chime.
    let mut osc = Osc::default();
    let mut pop = render(0.07, |t| {
        let hz = 900.0 * (0.3f32).powf(t / 0.07);
        osc.sample(Wave::Triangle, hz) * (1.0 - t / 0.07)
    });
    normalize(&mut pop, 0.7);
    let mut chime = arpeggio(&[84, 88, 91], 0.04, Wave::Pulse(0.125), 0.12);
    normalize(&mut chime, 0.35);
    pop.extend(chime);
    pop
}

/// Caught: a slow, sad, falling line (about 2.75 s).
///
/// Five notes on a 40 % pulse, one shared oscillator so the phase runs on between notes.
/// Each note fades in over 25 ms and sags linearly to half level by its end. The pitch
/// carries a vibrato of +-1.2 % (about a fifth of a semitone) at `5.5` rad/s, under 1 Hz,
/// which itself fades in over a third of a second; so the short notes barely move and only
/// the long final E wavers. The whole line is then shaped by `1 - x^3` (x = progress 0..1),
/// which stays near full for the first half and fades out over the long last note. Peak 0.45.
fn defeat() -> Vec<f32> {
    // B4 G4 E4 D#4 ... E4: falling, then resolving into a long, wavering low note.
    let notes: [(u8, f32); 5] = [(71, 0.3), (67, 0.3), (64, 0.3), (63, 0.45), (64, 1.4)];
    let mut out = Vec::new();
    let mut osc = Osc::default();
    for (note, secs) in notes {
        let hz = midi_hz(note);
        out.extend(render(secs, |t| {
            let vib = 1.0 + 0.012 * (t * 5.5).sin() * (t * 3.0).min(1.0);
            let env = (t * 40.0).min(1.0) * (1.0 - t / secs * 0.5);
            osc.sample(Wave::Pulse(0.4), hz * vib) * env
        }));
    }
    let n = out.len() as f32;
    for (i, s) in out.iter_mut().enumerate() {
        *s *= 1.0 - (i as f32 / n).powi(3);
    }
    normalize(&mut out, 0.45);
    out
}

/// The idle mouse: "squeak-eek!" (155 ms).
///
/// Two chirps on a thin 25 % pulse, separated by 35 ms of silence. Each chirp's pitch
/// follows a parabolic arc `lo + (hi - lo) * (1 - (2k - 1)^2)`: it starts at `lo`, peaks at
/// `hi` exactly halfway and falls back to `lo`. A fast +-40 Hz flutter at `90` rad/s (about
/// 14 Hz) roughens it. The first chirp is 70 ms over 2100-2900 Hz, the second 50 ms and
/// higher, 2500-3400 Hz. Each has a 3 ms fade-in and a linear fade-out. Peak 0.35.
fn squeak() -> Vec<f32> {
    // Two tiny chirps: a quick rise-and-fall, then a higher, shorter one.
    let mut osc = Osc::default();
    // `lo`/`hi` in Hz; the closure takes the oscillator so both chirps can share it.
    let chirp = |osc: &mut Osc, secs: f32, lo: f32, hi: f32| {
        render(secs, |t| {
            let k = t / secs;
            let hz = lo + (hi - lo) * (1.0 - (2.0 * k - 1.0).powi(2)) + 40.0 * (t * 90.0).sin();
            let env = (t * 300.0).min(1.0) * (1.0 - k);
            osc.sample(Wave::Pulse(0.25), hz) * env
        })
    };
    let mut out = chirp(&mut osc, 0.07, 2100.0, 2900.0);
    out.extend(std::iter::repeat_n(0.0, (SR * 0.035) as usize));
    out.extend(chirp(&mut osc, 0.05, 2500.0, 3400.0));
    normalize(&mut out, 0.35);
    out
}

/// Thunder for the title's lightning: crack, rolling rumble, sub boom (3.2 s).
///
/// Three layers, summed and normalized to 0.95:
///
/// - Crack: bright noise (16 kHz clock) decaying as `exp(-22 t)`, about 45 ms. The sharp
///   transient of the strike.
/// - Rumble: two dark noise sources, one clocked from 700 Hz falling to 400 Hz over the
///   whole sound (so it darkens as it recedes) and one at only 180 Hz for a grainy low
///   growl. Their sum is multiplied by a "roll" `0.6 + 0.4 sin(7 t) cos(2.3 t)`; two slow,
///   unrelated wobbles (about 1.1 Hz and 0.37 Hz) make the level surge irregularly between
///   0.2 and 1. A "swell" envelope ramps it in over about 170 ms and, from 0.15 s on,
///   decays it as `exp(-1.1 (t - 0.15))`, a time constant of about 0.9 s.
/// - Boom: a sub-bass triangle sliding from 68 Hz down to 38 Hz (`38 + 30 exp(-6 t)`),
///   decaying as `exp(-2.5 t)`, which gives the strike its weight on speakers that can
///   reproduce it.
fn thunder() -> Vec<f32> {
    // A sharp crack, then a low rumble that rolls (amplitude wobbles) and fades.
    let secs = 3.2;
    let mut crack = Noise::default();
    let mut rumble = Noise::default();
    let mut sub = Noise::default();
    let mut boom = Osc::default();
    let mut buf = render(secs, |t| {
        let c = crack.sample(16000.0) * (-t * 22.0).exp();
        let roll = 0.6 + 0.4 * (t * 7.0).sin() * (t * 2.3).cos();
        let swell = (t * 6.0).min(1.0) * (-(t - 0.15).max(0.0) * 1.1).exp();
        let r = (rumble.sample(700.0 - 300.0 * (t / secs)) * 0.7 + sub.sample(180.0) * 0.6)
            * roll
            * swell;
        let b = boom.sample(Wave::Triangle, 38.0 + 30.0 * (-t * 6.0).exp()) * (-t * 2.5).exp();
        c * 0.9 + r + b * 0.8
    });
    normalize(&mut buf, 0.95);
    buf
}

/// The cat catches the mouse: "mee-OW", then a hiss.
///
/// The voice (0.55 s) is a narrow 20 % pulse, whose strong upper harmonics sound nasal. Its
/// pitch contour rises linearly from 450 to 950 Hz over the first 35 % ("mee") and then
/// falls linearly to 400 Hz ("OW"), with a +-3 % vibrato at `38` rad/s (about 6 Hz). The
/// level fades in over 40 ms and out as `(1 - k)^0.7`, which holds up longer than a linear
/// fade before dropping. After 40 ms of silence comes the hiss: 0.3 s of bright noise
/// (16 kHz clock) fading linearly, at a lower peak (0.3) than the voice (0.55).
fn meow() -> Vec<f32> {
    // "Mee-OW": up, then down, nasal and wobbly, then a hiss.
    let secs = 0.55;
    let mut osc = Osc::default();
    let mut voice = render(secs, |t| {
        let k = t / secs;
        let contour = if k < 0.35 {
            450.0 + 500.0 * (k / 0.35)
        } else {
            950.0 - 550.0 * ((k - 0.35) / 0.65)
        };
        let hz = contour * (1.0 + 0.03 * (t * 38.0).sin());
        let env = (t * 25.0).min(1.0) * (1.0 - k).powf(0.7);
        osc.sample(Wave::Pulse(0.2), hz) * env
    });
    normalize(&mut voice, 0.55);
    let mut n = Noise::default();
    let mut hiss = render(0.3, |t| n.sample(16000.0) * (1.0 - t / 0.3) * 0.5);
    normalize(&mut hiss, 0.3);
    voice.extend(std::iter::repeat_n(0.0, (SR * 0.04) as usize));
    voice.extend(hiss);
    voice
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_effect_renders() {
        let b = super::Bank::new();
        for (name, pcm) in [
            ("step", &b.step),
            ("push", &b.push),
            ("roll", &b.roll),
            ("slide", &b.slide),
            ("too_heavy", &b.too_heavy),
            ("plop", &b.plop),
            ("meow", &b.meow),
            ("solved", &b.solved),
            ("fanfare", &b.fanfare),
            ("thunder", &b.thunder),
            ("squeak", &b.squeak),
            ("defeat", &b.defeat),
        ] {
            assert!(!pcm.is_empty(), "{name}");
            assert!(
                pcm.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{name}"
            );
        }
    }
}
