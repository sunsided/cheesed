//! The end of episode one: the heroine, her cheese, a treasure chest and a very dizzy cat
//! (`art/victory-episode-1.png`, see [`super::picture`]). Then a shareware nag.

use crate::gfx::font::{self, Font};
use crate::gfx::palette as pal;
use crate::gfx::{H, Screen, W};

use super::blink;
use super::picture::{Picture, glints, sparkle};

pub fn draw(s: &mut Screen, pic: &Picture, earned: u32, max: u32, t: f32) {
    pic.draw(s, t);
    let (ox, oy) = pic.origin();

    // The treasure and the cheese in her paws glint; the sun over the castle twinkles.
    glints(
        s,
        pic,
        &[(236, 118), (262, 114), (250, 124), (158, 40)],
        t,
        1.6,
    );
    let r = (((t * 1.2).sin() * 0.5 + 0.5) * 3.0).round() as i32 + 1;
    sparkle(s, ox + 219, oy + 42, r);

    // Words go in the dark rock along the bottom.
    let y = H - 34;
    s.blend_rect(0, y, W, 34, pal::INK, 0.45);
    font::draw_outlined(
        s,
        Font::Normal,
        W / 2 - Font::Normal.measure("THE CHEESE IS SAFE!", 1) / 2,
        y + 3,
        "THE CHEESE IS SAFE!",
        1,
        &|row| pal::YELLOW_LT.lerp(pal::CHEESE_DK, row as f32 / 6.0),
        pal::INK,
    );
    let stars = format!("STARS {earned} / {max}");
    font::draw_centered_shadow(s, Font::Normal, W / 2, y + 13, &stars, pal::PALE, pal::INK);
    let nag = if earned < max {
        "PAR IS ALWAYS POSSIBLE - GO BACK FOR MORE STARS!"
    } else {
        "EPISODES 2 & 3: ORDER THE FULL VERSION TODAY!"
    };
    let line = if blink(t) { nag } else { "PRESS SPACE" };
    font::draw_centered(s, Font::Tiny, W / 2, y + 25, line, pal::YELLOW_LT);
}
