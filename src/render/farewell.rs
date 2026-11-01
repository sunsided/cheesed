//! Leaving the game: "Farewell!" (`art/farewell.png`, see [`super::picture`]). The heroine
//! raises her sword to the full moon; it blazes, stars twinkle, the castle's lights and
//! the lamps along the road sparkle, its flag waves.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, gleam, hash, soft_cross, sparkle, twinkle_stars};

/// Natively the game closes this long after the farewell appears (or on any key).
pub const LINGER: f32 = 8.0;

/// Stars in the open sky (the painted ones are lost in 256 colours; clouds stay starless).
const STARS: [(i32, i32); 7] = [
    (88, 6),
    (132, 5),
    (147, 7),
    (220, 8),
    (239, 18),
    (261, 12),
    (290, 51),
];

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
