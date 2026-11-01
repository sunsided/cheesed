//! Sprites parsed from ASCII art (see [`super::palette::from_char`] for the color key).

use super::palette::{Rgb, from_char};

#[derive(Clone, Debug)]
pub struct Sprite {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Option<Rgb>>,
}

impl Sprite {
    /// Parse ASCII art: one row per line, leading/trailing blank lines and indentation are
    /// ignored. All rows must be equally wide.
    pub fn parse(art: &str) -> Sprite {
        Self::parse_with(art, &[])
    }

    /// Like [`Sprite::parse`], but `swap` remaps characters first, which is how one drawing
    /// becomes several color variants (a golden cookie from a plain one).
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

    /// The sprite turned 90 degrees clockwise.
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
