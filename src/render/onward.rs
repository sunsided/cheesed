//! After episode one's ending: "Onward!" (`art/onward-episode-1.png`, see
//! [`super::picture`]). The heroine points at episode two's villains, which you can't play,
//! because this is the shareware version.
//!
//! The painting's own pixels move through the effects attached in
//! [`Picture::onward_episode_1`] (the waterfall, the torch flame's core, the castle's
//! flags); this module draws the torchlight on the wall and the glints on top.
//!
//! Coordinates are picture pixels on the 320x200 painting (add [`Picture::origin`] for
//! screen pixels), each anchored to something painted there. When the painting is replaced,
//! re-check every anchor: run `task cover`, then `task snapshot`, and compare
//! `snapshots/onward.png`.
//! Times are seconds since the screen appeared.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, flare, gleam, glints, sparkle};

/// Draw the "Onward!" screen at time `t`.
///
/// Overlay anchors, in picture pixels:
/// - Torchlight: an orange glow of radius 17 centred on the wall torch at (68, 13), the same
///   torch whose flame flickers in the painting. Its strength breathes between 0.065 and
///   0.115 (a sine at 1.3 rad/s, about 4.8 s per breath) with a squared falloff
///   `(1 - d^2 / r^2)^2`, so the edge is very soft.
/// - Sparkles (radius 1..4) on the moon in the barred window at (115, 13) and on the setting
///   sun at (215, 87).
/// - A [`flare`] on the point of her sword at (105, 160).
/// - A [`gleam`] every 3.7 s (shifted by 1.9 s) at (282, 124), beside the hooded villain on
///   the right.
/// - Glints on the cheese on the floor at (186, 180) and (198, 183).
///
/// The blinking "PRESS SPACE" prompt sits in the bottom-right corner.
pub fn draw(s: &mut Screen, pic: &Picture, t: f32) {
    pic.draw(s, t);
    let (ox, oy) = pic.origin();
    let twinkle = |speed: f32| (((t * speed).sin() * 0.5 + 0.5) * 3.0).round() as i32 + 1;

    // Torchlight on the stones: a soft, mostly steady glow that breathes slowly. (A fast
    // flicker here makes the wall itself look animated.)
    let breath = 0.09 + 0.025 * (t * 1.3).sin();
    let (fx, fy, r) = (ox + 68, oy + 13, 17i32);
    for dy in -r..=r {
        for dx in -r..=r {
            let d2 = (dx * dx + dy * dy) as f32;
            let r2 = (r * r) as f32;
            if d2 <= r2 {
                let falloff = 1.0 - d2 / r2;
                s.blend(fx + dx, fy + dy, pal::ORANGE, breath * falloff * falloff);
            }
        }
    }
    sparkle(s, ox + 115, oy + 13, twinkle(0.8));
    sparkle(s, ox + 215, oy + 87, twinkle(1.3));
    flare(s, (ox + 105, oy + 160), t);
    gleam(s, (ox + 282, oy + 124), t, 3.7, 1.9);
    glints(s, pic, &[(186, 180), (198, 183)], t, 1.2);

    if blink(t) {
        let msg = "PRESS SPACE";
        let w = Font::Tiny.measure(msg, 1);
        let (x, y) = (W - w - 6, H - 9);
        s.blend_rect(x - 3, y - 2, w + 6, 9, pal::INK, 0.7);
        font::draw(s, Font::Tiny, x, y, msg, pal::YELLOW_LT);
    }
}
