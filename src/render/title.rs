//! The title screen: the cover painting (`art/cover.png`, see [`super::picture`]), with a
//! little life drawn over it: a twinkling star, glinting cheese, lightning.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, glints, sparkle};

/// Seconds between lightning strikes, and when in that cycle one strikes.
pub const LIGHTNING_EVERY: f32 = 7.0;
pub const LIGHTNING_AT: f32 = 1.2;

/// Whether the screen flashes at `t` (a double flash, like the real thing).
pub fn lightning(t: f32) -> bool {
    let c = t.rem_euclid(LIGHTNING_EVERY) - LIGHTNING_AT;
    (0.0..0.08).contains(&c) || (0.16..0.22).contains(&c)
}

/// How hard the lightning lights things up at `t`: 1 during a flash, then a short
/// afterglow, 0 between strikes.
pub fn flash_strength(t: f32) -> f32 {
    if lightning(t) {
        return 1.0;
    }
    let c = t.rem_euclid(LIGHTNING_EVERY) - LIGHTNING_AT;
    if (0.0..0.6).contains(&c) {
        0.45 * (1.0 - c / 0.6)
    } else {
        0.0
    }
}

/// A menacing glint in a cat's eyes, every few seconds.
pub fn eye_glint(s: &mut Screen, (ox, oy): (i32, i32), eyes: &[(i32, i32)], t: f32) {
    let c = (t / 4.3).fract();
    if c < 0.06 {
        let r = if c < 0.03 { 2 } else { 1 };
        for &(x, y) in eyes {
            sparkle(s, ox + x, oy + y, r);
        }
    }
}

/// Whether a strike began between `t0` and `t1` (to cue the thunder).
pub fn struck(t0: f32, t1: f32) -> bool {
    let cycle = |t: f32| ((t - LIGHTNING_AT) / LIGHTNING_EVERY).floor();
    t1 >= LIGHTNING_AT && cycle(t0) != cycle(t1)
}

pub fn draw(s: &mut Screen, cover: &Picture, t: f32) {
    cover.draw(s, t);
    let (ox, oy) = cover.origin();

    // The star above the title twinkles.
    let r = (((t * 1.7).sin() * 0.5 + 0.5) * 4.0).round() as i32 + 1;
    sparkle(s, ox + 250, oy + 10, r);
    // The cheese glints, and now and then the cat's eyes do too.
    glints(s, cover, &[(182, 54), (214, 62), (196, 78)], t, 1.3);
    eye_glint(s, (ox, oy), &[(84, 56), (113, 70)], t);

    // Lightning, now and then.
    if lightning(t) {
        s.tint(pal::Rgb::hex(0xd8c8ff), 0.16);
    }

    if blink(t) {
        let msg = "PRESS ANY KEY";
        let w = Font::Tiny.measure(msg, 1);
        let (x, y) = (W - w - 6, H - 9);
        s.blend_rect(x - 3, y - 2, w + 6, 9, pal::INK, 0.7);
        font::draw(s, Font::Tiny, x, y, msg, pal::YELLOW_LT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thunder_cues_once_per_strike() {
        let strikes = (0..2000)
            .filter(|&i| struck(i as f32 * 0.01, (i + 1) as f32 * 0.01))
            .count();
        assert_eq!(strikes, 3, "strikes at 1.2, 8.2 and 15.2 s within 20 s");
        assert!(lightning(LIGHTNING_AT + 0.01));
        assert!(!lightning(LIGHTNING_AT - 0.5));
    }
}
