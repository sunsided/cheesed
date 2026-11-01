//! Caught by the cat: after the pounce, the scene fades into the defeat painting
//! (`art/defeat.png`, see [`super::picture`]), with the glowing cheese just out of reach.
//!
//! Unlike the other screens this one is drawn over the level: the card is rendered into its
//! own [`Screen`] and cross-faded onto whatever is already on screen. The cheese's glow is
//! an effect attached in [`Picture::defeat`]; this module adds the drifting dust, the
//! glints and the text.
//!
//! Coordinates are picture pixels on the 320x200 painting (add [`Picture::origin`] for
//! screen pixels), each anchored to something painted there. When the painting is replaced,
//! re-check every anchor: run `task cover`, then `task snapshot`, and compare
//! `snapshots/defeat.png`.
//! Times are seconds since the screen appeared.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen};

use super::blink;
use super::picture::{Picture, glints};

/// Seconds after the catch before the painting starts fading in, and how long that takes.
///
/// The delay leaves the pounce on the level visible first. `app.rs` also uses it to play
/// the defeat sound and to stop the music at the moment the card begins to appear.
pub const DELAY: f32 = 0.9;
/// Seconds the cross-fade from the level to the defeat card takes, after [`DELAY`].
pub const FADE: f32 = 0.5;

/// Draw the defeat card over whatever is on screen; `since` counts from the catch.
///
/// `t` drives the animations (both are in seconds). Until `DELAY` nothing is drawn; then
/// the card is mixed in with opacity rising linearly over `FADE`, and copied outright once
/// fully opaque.
///
/// Anchors, in picture pixels:
/// - Dust: 16 motes in the light falling from the barred window, roughly x = 212..294 and
///   y = 24..108. Each mote's start and speed come from a sine hash of its index. It sinks
///   at 4..8 px/s, wrapping every 84 px, while sliding left 3 px/s and wrapping every
///   24 px, and its opacity breathes between 0.35 and 0.7 towards `PALE`.
/// - Glints on the cheese at (268, 131), (284, 126) and (291, 138).
/// - The text sits in the dark of the dungeon floor, from 34 px above the bottom, at the
///   left edge (screen pixels; the painting is full-width).
pub fn draw(s: &mut Screen, pic: &Picture, since: f32, t: f32) {
    let alpha = ((since - DELAY) / FADE).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let mut card = Screen::new();
    pic.draw(&mut card, t);
    let (ox, oy) = pic.origin();

    // Dust drifting down through the light from the barred window.
    for i in 0..16 {
        let seed = i as f32 * 12.9898;
        let fx = (seed.sin() * 43_758.547).fract().abs();
        let fy = ((seed * 1.7).sin() * 24_634.635).fract().abs();
        let x = 236.0 + fx * 58.0 - (t * 3.0 + fy * 20.0) % 24.0;
        let y = 24.0 + (fy * 80.0 + t * (4.0 + fx * 4.0)) % 84.0;
        let a = 0.35 + 0.35 * ((t * 2.0 + seed).sin() * 0.5 + 0.5);
        card.blend(ox + x as i32, oy + y as i32, pal::PALE, a);
    }
    glints(
        &mut card,
        pic,
        &[(268, 131), (284, 126), (291, 138)],
        t,
        1.4,
    );

    // Words in the dark of the dungeon floor.
    let y = H - 34;
    font::draw_outlined(
        &mut card,
        Font::Normal,
        8,
        y,
        "CAUGHT!",
        2,
        &|row| pal::PINK.lerp(pal::RED, row as f32 / 6.0),
        pal::INK,
    );
    font::draw_shadow(
        &mut card,
        Font::Normal,
        98,
        y + 4,
        "SO CLOSE...",
        pal::PALE,
        pal::INK,
    );
    if blink(t) {
        let help = "Z: UNDO   R: RESTART";
        font::draw_outlined(
            &mut card,
            Font::Tiny,
            9,
            y + 20,
            help,
            1,
            &|_| pal::YELLOW_LT,
            pal::INK,
        );
    }

    if alpha >= 1.0 {
        s.copy_from(&card);
    } else {
        s.mix_from(&card, alpha);
    }
}
