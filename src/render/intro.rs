//! Episode intro cards (see [`super::picture`]), shown before an episode's first level.
//!
//! - Episode 1 (`art/intro-episode-1.png`): the heroine squares up to the cat over a
//!   glowing cheese.
//! - Episode 2 (`art/intro-episode-2.png`): a western stand-off over a gouda.
//! - Episode 3 (`art/intro-episode-3.png`): fondue duel with a chef cat under the full moon.
//!
//! Episodes 2 and 3 are baked in for the registered version that doesn't exist;
//! `--cheese` mode flips through all cards with `I`.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, glints, sparkle};

/// Episodes that have an intro card.
pub const EPISODES: usize = 3;

/// A firelight flicker strength for `t`; `seed` desynchronizes several lights.
fn flicker(t: f32, seed: f32) -> f32 {
    0.18 + 0.12 * ((t * 13.0 + seed).sin() * (t * 7.3 + seed * 2.0).cos()).abs()
}

/// A soft round glow of `color`, strongest in the middle.
fn glow(s: &mut Screen, x: i32, y: i32, r: i32, color: pal::Rgb, strength: f32) {
    let r2 = (r * r) as f32;
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 <= r2 {
                s.blend(x + dx, y + dy, color, strength * (1.0 - d2 / r2));
            }
        }
    }
}

/// A quick gleam at a point once every `period` seconds (shifted by `offset`): a sparkle
/// that swells and shrinks within a quarter second.
fn gleam(s: &mut Screen, (x, y): (i32, i32), t: f32, period: f32, offset: f32) {
    let c = (t - offset).rem_euclid(period);
    if c < 0.25 {
        let r = (3.5 * (1.0 - (c / 0.125 - 1.0).abs())).round() as i32;
        if r > 0 {
            sparkle(s, x, y, r);
        }
    }
}

pub fn draw(s: &mut Screen, pic: &Picture, episode: usize, t: f32) {
    pic.draw(s, t);
    let (ox, oy) = pic.origin();
    let twinkle = |speed: f32| (((t * speed).sin() * 0.5 + 0.5) * 3.0).round() as i32 + 1;
    match episode {
        1 => {
            glints(s, pic, &[(186, 145), (206, 149), (196, 155)], t, 1.8);
            sparkle(s, ox + 175, oy + 104, twinkle(1.4));
            super::title::eye_glint(s, (ox, oy), &[(236, 64), (251, 57)], t + 2.0);
            // The sword's point gleams; a glint runs along the cat's claws.
            gleam(s, (ox + 119, oy + 177), t, 3.1, 0.0);
            for (i, &(x, y)) in [(281, 84), (287, 85), (292, 82), (297, 78)]
                .iter()
                .enumerate()
            {
                gleam(s, (ox + x, oy + y), t, 4.7, 1.4 + i as f32 * 0.07);
            }
        }
        2 => {
            glints(s, pic, &[(180, 142), (214, 139), (200, 151)], t, 1.5);
            sparkle(s, ox + 166, oy + 68, twinkle(1.1));
            glow(s, ox + 275, oy + 27, 9, pal::YELLOW_LT, flicker(t, 0.0));
        }
        _ => {
            glints(s, pic, &[(192, 88), (203, 93), (196, 99)], t, 1.7);
            sparkle(s, ox + 201, oy + 28, twinkle(0.9));
            // The fire under the pot and the lantern flicker; the fondue bubbles.
            glow(
                s,
                ox + 202,
                oy + 184,
                22,
                pal::ORANGE,
                flicker(t, 1.0) * 1.2,
            );
            glow(s, ox + 13, oy + 160, 14, pal::YELLOW_LT, flicker(t, 2.0));
            for i in 0..5 {
                let phase = (t * 0.9 + i as f32 * 0.37).fract();
                let bx = ox + 176 + (i * 13) % 56;
                let by = oy + 136 + (i * 5) % 5;
                let r = (phase * 3.0) as i32;
                if phase < 0.85 {
                    s.circle_outline(bx, by, r.max(1), pal::YELLOW_LT);
                }
            }
        }
    }
    if blink(t) {
        let msg = "PRESS SPACE";
        let w = Font::Tiny.measure(msg, 1);
        let (x, y) = (W - w - 6, H - 9);
        s.blend_rect(x - 3, y - 2, w + 6, 9, pal::INK, 0.7);
        font::draw(s, Font::Tiny, x, y, msg, pal::YELLOW_LT);
    }
}
