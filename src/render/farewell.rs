//! Leaving the game: "Farewell!" (`art/farewell.png`, see [`super::picture`]). The heroine
//! raises her sword to the full moon; it blazes, stars twinkle, the castle's lights and
//! the lamps along the road sparkle, its flag waves.
//!
//! The flag, the twinkling windows and lamps and the drifting valley fog are effects on the
//! painting's own pixels, attached in [`Picture::farewell`]. This module draws overlays on
//! top: stars, soft glints on the road lamps, the blazing sword tip and two gleams.
//!
//! Coordinates are picture pixels on the 320x200 painting (add [`Picture::origin`] for
//! screen pixels), each anchored to something painted there. When the painting is replaced,
//! re-check every anchor: run `task cover`, then `task snapshot`, and compare
//! `snapshots/farewell.png`.
//! Times are seconds since the screen appeared.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, gleam, hash, soft_cross, sparkle, twinkle_stars};

/// Natively the game closes this long after the farewell appears (or on any key), in
/// seconds. `app.rs` ignores keys for the first 0.6 s so a held key does not skip it.
pub const LINGER: f32 = 8.0;

/// Stars in the open sky (the painted ones are lost in 256 colours; clouds stay starless).
///
/// Picture pixels, placed by hand in the gaps between the painted clouds; drawn with
/// [`twinkle_stars`].
const STARS: [(i32, i32); 7] = [
    (88, 6),
    (132, 5),
    (147, 7),
    (220, 8),
    (239, 18),
    (261, 12),
    (290, 51),
];

/// Draw the farewell screen at time `t`.
///
/// The road lamps are found at runtime with [`Picture::spots`] over the road rectangle
/// (240..284, 100..142), at least 4 px apart, using the same `warm` test as the twinkle
/// effect in [`Picture::farewell`]; keep the two in sync. Each lamp runs its own cycle of
/// about 15.6 s (14 units at 0.9 units per second) with a hashed offset, and during the
/// first ~0.17 s of it shows a [`soft_cross`] that fades from 0.35 to nothing, so now and
/// then a single lamp glints.
///
/// Other anchors, in picture pixels: the raised sword's tip at (168, 12) gets `blaze`;
/// gleams on her belt buckle at (135, 98) every 4.3 s and on her dagger hilt at (120, 110)
/// every 5.7 s. The blinking "PRESS ANY KEY" prompt sits in the bottom-right corner.
pub fn draw(s: &mut Screen, pic: &Picture, t: f32) {
    pic.draw(s, t);
    let (ox, oy) = pic.origin();

    twinkle_stars(s, (ox, oy), &STARS, t);

    // The lamps along the road up to the castle: now and then one glints softly.
    let warm = |c: pal::Rgb| c.0 > 200 && c.1 > 140 && c.2 < 140;
    let lamps = pic.spots((240, 100, 284, 142), 4, warm);
    for (i, &(x, y)) in lamps.iter().enumerate() {
        let c = (t * 0.9 + hash(i, 3.0) * 14.0).rem_euclid(14.0);
        if c < 0.15 {
            soft_cross(s, ox + x, oy + y, 0.35 * (1.0 - c / 0.15));
        }
    }

    // The raised sword's tip blazes; her buckle and dagger hilt catch the light.
    blaze(s, (ox + 168, oy + 12), t);
    gleam(s, (ox + 135, oy + 98), t, 4.3, 1.1);
    gleam(s, (ox + 120, oy + 110), t, 5.7, 3.0);

    if blink(t) {
        let msg = "PRESS ANY KEY";
        let w = Font::Tiny.measure(msg, 1);
        let (x, y) = (W - w - 6, H - 9);
        s.blend_rect(x - 3, y - 2, w + 6, 9, pal::INK, 0.7);
        font::draw(s, Font::Tiny, x, y, msg, pal::YELLOW_LT);
    }
}

/// The sword tip, raised to the moon: a soft halo, long breathing rays, and a diagonal
/// cross that shoots out at the peak. Much bigger than an ordinary glint.
///
/// `(x, y)` is in screen pixels. `pulse = (0.5 + 0.5 sin(1.8 t))^1.5` (a cycle about every
/// 3.5 s) drives everything:
/// - a yellow halo of radius 11, blended by `(0.3 + 0.4 pulse) * (1 - d^2 / r^2)`;
/// - a [`sparkle`] whose arms reach 5 to 20 px;
/// - a second set of arms offset one pixel to each side along half that length, so the rays
///   are three pixels thick near the centre and read at 320x200;
/// - above `pulse` 0.45, a diagonal cross of up to 11 px;
/// - a solid white 3x3 core.
fn blaze(s: &mut Screen, (x, y): (i32, i32), t: f32) {
    let pulse = ((t * 1.8).sin() * 0.5 + 0.5).powf(1.5);
    let halo = 11;
    for dy in -halo..=halo {
        for dx in -halo..=halo {
            let d2 = (dx * dx + dy * dy) as f32;
            let r2 = (halo * halo) as f32;
            if d2 <= r2 {
                let a = (0.3 + 0.4 * pulse) * (1.0 - d2 / r2);
                s.blend(x + dx, y + dy, pal::YELLOW_LT, a);
            }
        }
    }
    let r = (5.0 + 15.0 * pulse).round() as i32;
    sparkle(s, x, y, r);
    // A second, thicker pass so the rays read at 320x200.
    for d in 1..=r / 2 {
        let a = 0.7 * (1.0 - d as f32 / (r / 2 + 1) as f32);
        for (dx, dy) in [
            (d, 1),
            (-d, 1),
            (d, -1),
            (-d, -1),
            (1, d),
            (-1, d),
            (1, -d),
            (-1, -d),
        ] {
            s.blend(x + dx, y + dy, pal::WHITE, a);
        }
    }
    if pulse > 0.45 {
        let n = ((pulse - 0.45) * 20.0).round() as i32;
        for k in 1..=n {
            let a = 0.85 * (1.0 - k as f32 / (n + 1) as f32);
            for (dx, dy) in [(k, k), (-k, k), (k, -k), (-k, -k)] {
                s.blend(x + dx, y + dy, pal::WHITE, a);
            }
        }
    }
    s.fill_rect(x - 1, y - 1, 3, 3, pal::WHITE);
}
