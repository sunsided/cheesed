//! Sprites parsed from ASCII art (see [`super::palette::from_char`] for the color key).
//!
//! Every piece of hand-drawn art in the game is a string constant where each character is
//! one pixel: a palette letter for an opaque pixel, `.` for a transparent one. Rows are
//! lines; the surrounding blank lines and the indentation that comes from writing the art
//! inside Rust source are stripped, so a sprite can be laid out like this:
//!
//! ```text
//! const PAW: &str = "
//!     .....
//!     .k.k.
//!     .....
//!     .kkk.
//!     .kkk.
//! ";
//! ```
//!
//! Parsing happens once at startup (see `render::art::Art::new`); afterwards a sprite is
//! just a small array of optional colors that the `Screen::blit*` methods copy into the
//! framebuffer.

use super::palette::{Rgb, from_char};

/// A small image with per-pixel transparency, usually parsed from ASCII art.
#[derive(Clone, Debug)]
pub struct Sprite {
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
    /// The `w * h` pixels, row-major from the top-left. `None` is transparent and is skipped
    /// when blitting.
    pub px: Vec<Option<Rgb>>,
}

impl Sprite {
    /// Parse ASCII art: one row per line, leading/trailing blank lines and indentation are
    /// ignored. All rows must be equally wide.
    ///
    /// Each line is trimmed on both sides and empty lines are dropped (anywhere, not only at the
    /// ends), so a row cannot start or end with a space; use `.` for transparent pixels at the
    /// edges. The width is taken from the first row, and a row of any other width panics with
    /// its index and contents, which catches hand-editing mistakes the first time the art is
    /// loaded (the art tests construct every sprite).
    pub fn parse(art: &str) -> Sprite {
        Self::parse_with(art, &[])
    }

    /// Like [`Sprite::parse`], but `swap` remaps characters first, which is how one drawing
    /// becomes several color variants (the grey empty star from the gold one, the wheel and
    /// brie in a hole from the yellow wedge in a hole).
    ///
    /// Each character is looked up in `swap` once, using the first matching `(from, to)` pair,
    /// and the result is not remapped again. Swaps therefore apply simultaneously:
    /// `[('Y', 'P'), ('P', 'Y')]` exchanges light yellow and pink rather than turning both
    /// into one color.
    pub fn parse_with(art: &str, swap: &[(char, char)]) -> Sprite {
        let rows: Vec<&str> = art
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let h = rows.len() as i32;
        let w = rows.first().map_or(0, |r| r.chars().count()) as i32;
        let mut px = Vec::with_capacity((w * h) as usize);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(
                row.chars().count() as i32,
                w,
                "sprite row {i} is {} wide, expected {w}: {row:?}",
                row.chars().count()
            );
            for ch in row.chars() {
                let ch = swap
                    .iter()
                    .find(|(from, _)| *from == ch)
                    .map_or(ch, |(_, to)| *to);
                px.push(from_char(ch));
            }
        }
        Sprite { w, h, px }
    }

    /// The sprite turned 90 degrees clockwise. Width and height trade places: source pixel
    /// `(x, y)` lands at `(h - 1 - y, x)`, so the source's left column becomes the top row
    /// (read bottom to top) and its bottom row becomes the left column.
    pub fn rotated_cw(&self) -> Sprite {
        let mut px = vec![None; self.px.len()];
        for y in 0..self.h {
            for x in 0..self.w {
                let nx = self.h - 1 - y;
                let ny = x;
                px[(ny * self.h + nx) as usize] = self.px[(y * self.w + x) as usize];
            }
        }
        Sprite {
            w: self.h,
            h: self.w,
            px,
        }
    }
}
