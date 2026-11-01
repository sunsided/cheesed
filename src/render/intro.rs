//! Episode intro cards (see [`super::picture`]), shown before an episode's first level.
//!
//! - Episode 1 (`art/intro-episode-1.png`): the heroine squares up to the cat over a
//!   glowing cheese.
//! - Episode 2 (`art/intro-episode-2.png`): a western stand-off over a gouda.
//! - Episode 3 (`art/intro-episode-3.png`): fondue duel with a chef cat under the full moon.
//!
//! Episodes 2 and 3 are baked in for the registered version that doesn't exist;
//! `--cheese` mode flips through all cards with `I`.
//!
//! As on the title, the painting's own pixels are animated by the effects attached in
//! `picture.rs` ([`Picture::intro_episode_1`] and friends: glowing sun and cheese, a storm
//! cloud that flashes with the title's lightning schedule, waterfalls, flags), and this
//! module draws overlays on top: glints, sparkles, flickering firelight, glaring eyes.
//!
//! All coordinates are picture pixels on the 320x200 painting, converted to screen pixels by
//! adding [`Picture::origin`]; each is anchored to something painted on that card (noted at
//! the use). When a card's painting is replaced, re-check every anchor for it: run
//! `task cover`, then `task snapshot`, and compare `snapshots/intro*.png`. Times are seconds
//! since the card appeared.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, gleam, glints, sparkle};

/// Episodes that have an intro card.
pub const EPISODES: usize = 3;

/// A firelight flicker strength for `t`; `seed` desynchronizes several lights.
///
/// `0.18 + 0.12 * |sin(13 t + seed) * cos(7.3 t + 2 seed)|`, so 0.18..0.30: a steady base
/// glow with a fast, irregular shimmer from two unrelated frequencies. Meant as the
/// `strength` of [`glow`].
fn flicker(t: f32, seed: f32) -> f32 {
    0.18 + 0.12 * ((t * 13.0 + seed).sin() * (t * 7.3 + seed * 2.0).cos()).abs()
}

/// A soft round glow of `color`, strongest in the middle.
///
/// Every screen pixel within radius `r` of `(x, y)` (screen pixels) is blended towards
/// `color` by `strength * (1 - d^2 / r^2)`: full strength at the centre, zero at the rim.
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

/// Now and then light runs along the top edge of her revolver's barrel, cylinder to
/// muzzle, and flashes at the end.
///
/// Episode 2. The barrel runs from (125, 122) at the cylinder to (143, 114) at the muzzle
/// (picture pixels). Once every 7 s (shifted by 0.8 s) a radius-1 sparkle travels along it
/// in 0.3 s, then a sparkle at the muzzle swells to radius 4 and shrinks again over 0.2 s.
fn barrel_glint(s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
    let (from, to) = ((125.0, 122.0), (143.0, 114.0));
    let c = (t - 0.8).rem_euclid(7.0);
    if c < 0.3 {
        let k = c / 0.3;
        let x = from.0 + (to.0 - from.0) * k;
        let y = from.1 + (to.1 - from.1) * k;
        sparkle(s, ox + x.round() as i32, oy + y.round() as i32, 1);
    } else if c < 0.5 {
        let r = (3.5 * (1.0 - ((c - 0.3) / 0.1 - 1.0).abs())).round() as i32;
        if r > 0 {
            sparkle(s, ox + to.0 as i32, oy + to.1 as i32, r);
        }
    }
}

/// The caped guardian on the bridge keeps watch: two glowing eyes under his hat brim, cool
/// and heroic against the moon. They pulse, blink now and then, and shoot a short streak of
/// glare at their brightest.
///
/// Episode 3. The eyes are at (202, 41) and (204, 41) (picture pixels), on the small caped
/// figure standing on the bridge in front of the moon. `heat = 0.55 + 0.45 sin^2(1.5 t)`
/// (a cycle about every 2.1 s) sets how white each eye pixel is and how strongly its four
/// neighbours are tinted cyan. Above `heat` 0.93 a horizontal cyan streak of up to 4 px
/// grows out on both sides of each eye. Every 5.3 s (shifted by 1.7 s) the eyes vanish for
/// 0.14 s, a blink.
fn glaring_eyes(s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
    // A blink: eyes shut for a moment every few seconds.
    if (t + 1.7).rem_euclid(5.3) < 0.14 {
        return;
    }
    let heat = 0.55 + 0.45 * (t * 1.5).sin().powi(2);
    for (x, y) in [(202, 41), (204, 41)] {
        let (x, y) = (ox + x, oy + y);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            s.blend(x + dx, y + dy, pal::CYAN, 0.45 * heat);
        }
        s.blend(x, y, pal::WHITE, heat);
        if heat > 0.93 {
            let len = ((heat - 0.93) * 60.0).round() as i32;
            for k in 1..=len {
                let a = 0.7 * (1.0 - k as f32 / (len + 1) as f32);
                s.blend(x - k - 1, y, pal::CYAN_LT, a);
                s.blend(x + k + 1, y, pal::CYAN_LT, a);
            }
        }
    }
}

/// Draw the intro card for `episode` (1-based; anything above 2 draws episode 3) at time `t`.
///
/// Overlay anchors, in picture pixels:
/// - Episode 1: glints on the cheese on its stump at (186, 145), (206, 149), (196, 155); a
///   sparkle on the setting sun at (175, 104); the cat's eyes at (236, 64) and (251, 57),
///   with [`super::title::eye_glint`] shifted by 2 s; a gleam at the heroine's sword point
///   (119, 177) every 3.1 s; and a gleam running along the cat's four claws, (281, 84) to
///   (297, 78), every 4.7 s, each claw 0.07 s after the previous.
/// - Episode 2: glints on the gouda at (180, 142), (214, 139), (200, 151); a sparkle on the
///   sun behind the castle at (166, 68); a flickering glow of radius 9 on the hanging
///   lantern at (275, 27); and `barrel_glint` on her revolver.
/// - Episode 3: glints on the cheese cube on her fondue fork at (192, 88), (203, 93),
///   (196, 99); a sparkle on the full moon at (201, 28); `glaring_eyes`; a large orange
///   glow (radius 22, 1.2 times the flicker) on the fire under the pot at (202, 184) and a
///   yellow one (radius 14) on the lantern at the left edge at (13, 160); and bubbles on
///   the fondue: five ring outlines spread across x = 176..228 and rows 136..140, each growing
///   from radius 1 to 2 over a 1.1 s cycle (0.9 cycles per second, staggered by 0.37) and
///   hidden for the last 15% of it, as if it popped.
///
/// The sparkles use a local `twinkle(speed)` radius of 1..4, following a sine at `speed`
/// rad/s. The blinking "PRESS SPACE" prompt sits in the bottom-right corner.
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
            barrel_glint(s, (ox, oy), t);
        }
        _ => {
            glints(s, pic, &[(192, 88), (203, 93), (196, 99)], t, 1.7);
            sparkle(s, ox + 201, oy + 28, twinkle(0.9));
            glaring_eyes(s, (ox, oy), t);
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
                let by = oy + 136 + (i * 3) % 5;
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
