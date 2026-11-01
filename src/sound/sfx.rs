//! One-shot sound effects, rendered to PCM once at startup.

use super::synth::{Noise, Osc, SR, Wave, midi_hz, normalize, render};

/// Every effect in the game.
pub struct Bank {
    pub step: Vec<f32>,
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
    pub unplop: Vec<f32>,
    pub meow: Vec<f32>,
    pub undo: Vec<f32>,
    pub restart: Vec<f32>,
    pub solved: Vec<f32>,
    pub menu: Vec<f32>,
    pub confirm: Vec<f32>,
    pub nope: Vec<f32>,
    pub fanfare: Vec<f32>,
}

impl Bank {
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
        }
    }
}

impl Default for Bank {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick notes in a row, each decaying, with a tail.
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
        ] {
            assert!(!pcm.is_empty(), "{name}");
            assert!(
                pcm.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{name}"
            );
        }
    }
}
