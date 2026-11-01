//! The title screen: the official cover painting, reduced to 256 colors like a VGA game's
//! title picture (`tools/cover.py` makes `cover.bin` from `art/cover.png`), plus a little
//! life drawn over it.

use crate::gfx::font::{self, Font};
use crate::gfx::palette::{self as pal, Rgb};
use crate::gfx::{H, Screen, W};

use super::blink;

/// A palette-indexed picture.
pub struct Cover {
    pub w: i32,
    pub h: i32,
    palette: Vec<Rgb>,
    px: Vec<u8>,
}

impl Cover {
    /// Parse the format written by `tools/cover.py`.
    pub fn parse(bytes: &[u8]) -> Option<Cover> {
        let w = u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]) as i32;
        let h = u16::from_le_bytes([*bytes.get(2)?, *bytes.get(3)?]) as i32;
        let n = *bytes.get(4)? as usize + 1;
        let pal = bytes.get(5..5 + n * 3)?;
        let px = bytes.get(5 + n * 3..)?;
        if px.len() != (w * h) as usize || px.iter().any(|&i| i as usize >= n) {
            return None;
        }
        Some(Cover {
            w,
            h,
            palette: pal.chunks(3).map(|c| Rgb(c[0], c[1], c[2])).collect(),
            px: px.to_vec(),
        })
    }

    pub fn embedded() -> Cover {
        Cover::parse(include_bytes!("cover.bin")).expect("embedded cover parses")
    }

    fn get(&self, x: i32, y: i32) -> Rgb {
        let (x, y) = (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1));
        self.palette[self.px[(y * self.w + x) as usize] as usize]
    }
}

/// Where a point of the cover (in its own pixels) lands on screen.
fn origin(c: &Cover) -> (i32, i32) {
    ((W - c.w) / 2, (H - c.h) / 2)
}

pub fn draw(s: &mut Screen, cover: &Cover, t: f32) {
    let (ox, oy) = origin(cover);
    for y in 0..H {
        for x in 0..W {
            let (cx, cy) = (x - ox, y - oy);
            let inside = (0..cover.w).contains(&cx) && (0..cover.h).contains(&cy);
            let c = if inside {
                cover.get(cx, cy)
            } else {
                // A painting narrower than the screen: smear its edge outwards, fading to
                // black, so the margins feel like more of the same night.
                let edge = if cx < 0 { 0 } else { cover.w - 1 };
                let d = if cx < 0 { -cx } else { cx - edge };
                let fade = (d as f32 / ox.max(1) as f32).min(1.0);
                let src = cover.get(edge - (d / 3) * cx.signum(), cy);
                src.scale(1.0 - 0.88 * fade.sqrt())
            };
            s.put(x, y, c);
        }
    }

    // The star above the title twinkles.
    let (sx, sy) = (ox + 250, oy + 10);
    let tw = ((t * 1.7).sin() * 0.5 + 0.5) * 4.0;
    let r = tw.round() as i32 + 1;
    for d in 1..=r {
        let a = 1.0 - d as f32 / (r + 1) as f32;
        for (dx, dy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
            s.blend(sx + dx, sy + dy, pal::WHITE, a);
        }
    }
    s.put(sx, sy, pal::WHITE);

    // The cheese glints.
    let glints = [(182, 54), (214, 62), (196, 78)];
    let k = (t * 1.3) as usize % (glints.len() * 2);
    if k < glints.len() {
        let (gx, gy) = (ox + glints[k].0, oy + glints[k].1);
        for d in 1..=2 {
            for (dx, dy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
                s.blend(gx + dx, gy + dy, pal::WHITE, 0.8 / d as f32);
            }
        }
        s.put(gx, gy, pal::WHITE);
    }

    // Lightning, now and then.
    let cycle = t % 5.5;
    if (4.6..4.68).contains(&cycle) || (4.76..4.82).contains(&cycle) {
        s.tint(pal::Rgb::hex(0xd8c8ff), 0.28);
    }

    if blink(t) {
        let msg = "PRESS SPACE";
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
    fn embedded_cover_fits_the_screen() {
        let c = Cover::embedded();
        assert_eq!(c.h, H);
        assert!(c.w <= W);
    }

    #[test]
    fn rejects_truncated_data() {
        assert!(Cover::parse(&[]).is_none());
        assert!(Cover::parse(&[2, 0, 2, 0, 0, 1, 2, 3, 0, 0, 0]).is_none());
        assert!(Cover::parse(&[2, 0, 1, 0, 0, 1, 2, 3, 0, 0]).is_some());
    }
}
