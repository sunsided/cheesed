//! Leaving the game: "Farewell!" (`art/farewell.png`, see [`super::picture`]). The heroine
//! raises her sword to the full moon; it blazes, stars twinkle, the castle's lights and
//! the lamps along the road sparkle, its flag waves.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, gleam, sparkle};

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

/// Stable pseudo-random 0..1 per index.
fn hash(i: usize, salt: f32) -> f32 {
    ((i as f32 * 12.9898 + salt * 78.233).sin() * 43_758.547)
        .fract()
        .abs()
}

pub fn draw(s: &mut Screen, pic: &Picture, t: f32) {
    pic.draw(s, t);
    let (ox, oy) = pic.origin();

    // Stars: each on its own rhythm, a faint dot, a tiny cross at its brightest.
    for (i, &(x, y)) in STARS.iter().enumerate() {
        let (rate, phase) = (0.8 + hash(i, 1.0) * 1.6, hash(i, 2.0) * 6.3);
        let b = (t * rate + phase).sin().max(0.0);
        let (x, y) = (ox + x, oy + y);
        s.blend(x, y, pal::PALE, 0.15 + 0.4 * b);
        if b > 0.97 {
            soft_cross(s, x, y, 0.4);
        }
    }

    // The lamps along the road up to the castle: now and then one glints softly.
    let warm = |c: pal::Rgb| c.0 > 200 && c.1 > 140 && c.2 < 140;
    let lamps = pic.spots((240, 100, 284, 142), 4, warm);
    for (i, &(x, y)) in lamps.iter().enumerate() {
        let c = (t * 0.9 + hash(i, 3.0) * 7.0).rem_euclid(7.0);
        if c < 0.15 {
            soft_cross(s, ox + x, oy + y, 0.55 * (1.0 - c / 0.15));
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

/// A small, soft glint: a bright pixel with a faint one-pixel cross, at `strength` 0..1.
fn soft_cross(s: &mut Screen, x: i32, y: i32, strength: f32) {
    s.blend(x, y, pal::WHITE, strength);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        s.blend(x + dx, y + dy, pal::WHITE, strength * 0.45);
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
