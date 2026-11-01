//! The title screen: the cover painting (`art/cover.png`, see [`super::picture`]), with a
//! little life drawn over it: a twinkling star, glinting cheese, lightning.
//!
//! Two layers of animation meet here. The effects baked into [`Picture::cover`] move the
//! painting's own pixels (glowing cheese, lit windows, waving flags, drifting fog, bolts that
//! blaze). This module draws overlays on top: twinkling stars, a flare on the logo's star,
//! glints on the cheese and the cat's eyes, a whole-screen lightning tint, and the
//! "PRESS ANY KEY" prompt.
//!
//! It also owns the lightning *schedule* ([`lightning`], [`flash_strength`], [`struck`]),
//! which is shared: the cover's bolts and episode 1's storm cloud (`picture.rs`) read
//! [`flash_strength`], and `app.rs` calls [`struck`] to play the thunder sound on the title
//! and on episode 1's intro card. All times are seconds since the screen appeared.
//!
//! Every coordinate below is a picture pixel on the 320x200 cover, anchored to something
//! painted there (noted at each use). When `art/cover.png` is replaced, re-check all of
//! them: run `task cover`, then `task snapshot`, and compare `snapshots/title.png`.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, flare, glints, sparkle, twinkle_stars};

/// Seconds between lightning strikes, and when in that cycle one strikes.
pub const LIGHTNING_EVERY: f32 = 7.0;
/// Seconds into each [`LIGHTNING_EVERY`] cycle at which the strike begins, so the first one
/// comes 1.2 s after the screen appears (then at 8.2 s, 15.2 s, ...).
pub const LIGHTNING_AT: f32 = 1.2;

/// Whether the screen flashes at `t` (a double flash, like the real thing).
///
/// Measured from the start of a strike: lit for 0.00..0.08 s, dark for 0.08 s, lit again
/// for 0.16..0.22 s.
pub fn lightning(t: f32) -> bool {
    let c = t.rem_euclid(LIGHTNING_EVERY) - LIGHTNING_AT;
    (0.0..0.08).contains(&c) || (0.16..0.22).contains(&c)
}

/// How hard the lightning lights things up at `t`: 1 during a flash, then a short
/// afterglow, 0 between strikes.
///
/// The afterglow starts at 0.45 when the strike begins and falls linearly to 0 over 0.6 s;
/// during the two flashes it is overridden by 1. So the gap between the flashes is not
/// fully dark (about 0.33 to 0.39), and after the second flash the bolts fade out over
/// roughly 0.4 s.
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
///
/// `eyes` are picture pixels, `(ox, oy)` the picture origin. Every 4.3 s both eyes show a
/// [`sparkle`] for 0.06 s: radius 2 for the first 0.03 s, then radius 1. Callers shift `t`
/// to move the glint within the cycle (episode 1's intro passes `t + 2.0`). `t` is expected
/// to be non-negative.
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
///
/// Counts strike cycles with `floor((t - LIGHTNING_AT) / LIGHTNING_EVERY)` and reports a
/// change of cycle between the previous frame's time `t0` and this frame's `t1`. Called once
/// per frame, it is true on exactly one frame per strike, however short the frames are.
pub fn struck(t0: f32, t1: f32) -> bool {
    let cycle = |t: f32| ((t - LIGHTNING_AT) / LIGHTNING_EVERY).floor();
    t1 >= LIGHTNING_AT && cycle(t0) != cycle(t1)
}

/// The painted stars: compact bright points, found once on the cover (bolts, being lines,
/// don't qualify).
///
/// Picture pixels on `art/cover.png`, sorted by row. They were located once on this exact
/// painting and are not searched at runtime, so a new cover needs a new list. They are
/// drawn by [`twinkle_stars`] rather than animated in the painting: dimming bright sky
/// pixels made the neighbouring bolts pulse too.
const STARS: [(i32, i32); 16] = [
    (284, 3),
    (75, 4),
    (261, 4),
    (17, 6),
    (8, 8),
    (43, 11),
    (23, 13),
    (262, 13),
    (5, 17),
    (52, 17),
    (269, 17),
    (12, 23),
    (305, 24),
    (300, 32),
    (293, 34),
    (293, 53),
];

/// Draw the title screen at time `t` (seconds since it appeared).
///
/// Overlay anchors on the cover, in picture pixels: the logo's painted star at its top-right
/// corner (246, 10) gets a [`flare`]; the raised cheese's glints hop between (182, 54),
/// (214, 62) and (194, 70); the giant cat's eyes are at (84, 56) and (113, 70). While
/// [`lightning`] is on, the whole screen is tinted 16% towards a pale violet (`#d8c8ff`).
/// The blinking prompt sits in the bottom-right corner on a dark plate: "PRESS ANY KEY",
/// or "CLICK FOR SOUND" while `need_sound` (a browser that hasn't unlocked audio yet; the
/// first press then only switches the sound on).
pub fn draw(s: &mut Screen, cover: &Picture, t: f32, need_sound: bool) {
    cover.draw(s, t);
    let (ox, oy) = cover.origin();

    // The painted stars twinkle (on top: the bolts beside them must not flicker along).
    twinkle_stars(s, (ox, oy), &STARS, t);
    // The painted star on the logo's corner flares.
    flare(s, (ox + 246, oy + 10), t);
    // The cheese glints, and now and then the cat's eyes do too.
    glints(s, cover, &[(182, 54), (214, 62), (194, 70)], t, 1.3);
    eye_glint(s, (ox, oy), &[(84, 56), (113, 70)], t);

    // Lightning, now and then.
    if lightning(t) {
        s.tint(pal::Rgb::hex(0xd8c8ff), 0.16);
    }

    if blink(t) {
        let msg = if need_sound {
            "CLICK FOR SOUND"
        } else {
            "PRESS ANY KEY"
        };
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
