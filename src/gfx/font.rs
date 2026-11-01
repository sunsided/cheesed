//! Two hand-drawn bitmap fonts: a 5x7 one for text and a 3x5 one for tiny labels.
//! Lowercase letters render as uppercase.
//!
//! Glyphs are stored as tiny ASCII-art bitmaps, one string per row, top row first: `#` is
//! an inked pixel and `.` is empty. Glyphs carry no spacing of their own; text is laid out
//! on a fixed advance (glyph width plus one pixel), so every string is monospaced and its
//! width is known without drawing it ([`Font::measure`]).
//!
//! Characters missing from a font are not an error: they draw nothing but still advance,
//! so they behave like a space. The two fonts do not have the same character set; in
//! particular the inline symbols `~` (a cheese wedge) and `^` (a heart) exist only in
//! [`Font::Normal`].
//!
//! Text is drawn by filling one `scale` x `scale` square per inked pixel, so larger sizes
//! are integer multiples with the same chunky look. Positions are framebuffer pixels and
//! refer to the top-left of the first glyph.

use super::{Rgb, Screen};

/// Which of the two bitmap fonts to draw with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    /// 5x7 glyphs, 6px advance.
    Normal,
    /// 3x5 glyphs, 4px advance.
    Tiny,
}

impl Font {
    /// Horizontal distance in pixels from one glyph's left edge to the next at scale 1: the
    /// glyph width plus a 1px gap.
    pub fn advance(self) -> i32 {
        match self {
            Font::Normal => 6,
            Font::Tiny => 4,
        }
    }

    /// Glyph height in pixels at scale 1. There is no extra line spacing built in; callers
    /// choose their own line pitch.
    pub fn height(self) -> i32 {
        match self {
            Font::Normal => 7,
            Font::Tiny => 5,
        }
    }

    /// The bitmap rows for `c` (uppercased first), or `None` if this font has no glyph for it.
    /// A linear search through the table, which is fast enough for the few dozen characters
    /// drawn per frame.
    fn glyph(self, c: char) -> Option<&'static [&'static str]> {
        let c = c.to_ascii_uppercase();
        match self {
            Font::Normal => NORMAL.iter().find(|(g, _)| *g == c).map(|(_, rows)| *rows),
            Font::Tiny => TINY.iter().find(|(g, _)| *g == c).map(|(_, rows)| *rows),
        }
    }

    /// Width of `s` in pixels at `scale`, from the left edge of the first glyph to the right
    /// edge of the last: `n` advances minus the trailing 1px gap, all times `scale`. An empty
    /// string is 0 wide. Every character counts, including ones the font cannot draw.
    pub fn measure(self, s: &str, scale: i32) -> i32 {
        let n = s.chars().count() as i32;
        if n == 0 {
            0
        } else {
            (n * self.advance() - 1) * scale
        }
    }
}

/// Draw `s` with its top-left at `x, y`. `color` gets the glyph row (0 = top) so callers can
/// draw gradients. Returns the x after the last glyph.
///
/// The row passed to `color` is the unscaled bitmap row (0..height), so a gradient spans one
/// line of text regardless of `scale`. The returned x is one full advance past the last
/// glyph (including the 1px gap), which is where a following string would start.
pub fn draw_with(
    screen: &mut Screen,
    font: Font,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: &dyn Fn(i32) -> Rgb,
) -> i32 {
    let mut cx = x;
    for ch in s.chars() {
        if let Some(rows) = font.glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for (rx, bit) in row.chars().enumerate() {
                    if bit == '#' {
                        let c = color(ry as i32);
                        screen.fill_rect(
                            cx + rx as i32 * scale,
                            y + ry as i32 * scale,
                            scale,
                            scale,
                            c,
                        );
                    }
                }
            }
        }
        cx += font.advance() * scale;
    }
    cx
}

/// Draw `s` at scale 1 in a single color with its top-left at `x, y`. Returns the x after
/// the last glyph, like [`draw_with`].
pub fn draw(screen: &mut Screen, font: Font, x: i32, y: i32, s: &str, c: Rgb) -> i32 {
    draw_with(screen, font, x, y, s, 1, &|_| c)
}

/// Text with a 1px drop shadow down-right: the string is drawn once in `shadow` offset by
/// `(1, 1)` and then in `c` on top.
pub fn draw_shadow(screen: &mut Screen, font: Font, x: i32, y: i32, s: &str, c: Rgb, shadow: Rgb) {
    draw(screen, font, x + 1, y + 1, s, shadow);
    draw(screen, font, x, y, s, c);
}

/// Text with a full 1px outline, at any `scale`.
///
/// The text is drawn eight times in `outline`, shifted by one framebuffer pixel in every
/// direction (including diagonals), and then once with `color` on top. The shift does not
/// grow with `scale`, so scaled-up text still gets a thin 1px outline rather than a chunky
/// one.
pub fn draw_outlined(
    screen: &mut Screen,
    font: Font,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: &dyn Fn(i32) -> Rgb,
    outline: Rgb,
) {
    for (dx, dy) in [
        (-1, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (-1, -1),
        (1, -1),
        (-1, 1),
        (1, 1),
    ] {
        draw_with(screen, font, x + dx, y + dy, s, scale, &|_| outline);
    }
    draw_with(screen, font, x, y, s, scale, color);
}

/// Draw `s` at scale 1, horizontally centered around `cx` (rounded towards the left when
/// the width is odd), with its top at `y`.
pub fn draw_centered(screen: &mut Screen, font: Font, cx: i32, y: i32, s: &str, c: Rgb) {
    let w = font.measure(s, 1);
    draw(screen, font, cx - w / 2, y, s, c);
}

/// Like [`draw_centered`], with a 1px drop shadow in `sh` as in [`draw_shadow`].
pub fn draw_centered_shadow(
    screen: &mut Screen,
    font: Font,
    cx: i32,
    y: i32,
    s: &str,
    c: Rgb,
    sh: Rgb,
) {
    let w = font.measure(s, 1);
    draw_shadow(screen, font, cx - w / 2, y, s, c, sh);
}

/// A font table entry: the (uppercase) character and its bitmap rows, top row first.
type Glyph = (char, &'static [&'static str]);

/// The 5x7 font: A-Z, 0-9, punctuation, and two picture glyphs (`~` cheese, `^` heart).
#[rustfmt::skip]
const NORMAL: &[Glyph] = &[
    ('A', &[".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', &["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', &[".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', &["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', &["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', &["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', &[".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', &["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', &[".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('J', &["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."]),
    ('K', &["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', &["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', &["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', &["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"]),
    ('O', &[".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', &["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', &[".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', &["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', &[".###.", "#...#", "#....", ".###.", "....#", "#...#", ".###."]),
    ('T', &["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', &["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', &["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', &["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."]),
    ('X', &["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', &["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', &["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('0', &[".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', &["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', &[".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', &["#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###."]),
    ('4', &["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', &["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', &["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', &["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', &[".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', &[".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."]),
    ('!', &["..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#.."]),
    ('\'', &["..#..", "..#..", ".#...", ".....", ".....", ".....", "....."]),
    ('"', &[".#.#.", ".#.#.", ".....", ".....", ".....", ".....", "....."]),
    ('.', &[".....", ".....", ".....", ".....", ".....", ".##..", ".##.."]),
    (',', &[".....", ".....", ".....", ".....", ".##..", "..#..", ".#..."]),
    (':', &[".....", ".##..", ".##..", ".....", ".##..", ".##..", "....."]),
    (';', &[".....", ".##..", ".##..", ".....", ".##..", "..#..", ".#..."]),
    ('-', &[".....", ".....", ".....", "#####", ".....", ".....", "....."]),
    ('_', &[".....", ".....", ".....", ".....", ".....", ".....", "#####"]),
    ('=', &[".....", ".....", "#####", ".....", "#####", ".....", "....."]),
    ('+', &[".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."]),
    ('*', &[".....", "#.#.#", ".###.", "#####", ".###.", "#.#.#", "....."]),
    ('?', &[".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."]),
    ('/', &["....#", "...#.", "...#.", "..#..", ".#...", ".#...", "#...."]),
    ('(', &["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."]),
    (')', &[".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."]),
    ('[', &[".###.", ".#...", ".#...", ".#...", ".#...", ".#...", ".###."]),
    (']', &[".###.", "...#.", "...#.", "...#.", "...#.", "...#.", ".###."]),
    ('<', &["...#.", "..#..", ".#...", "#....", ".#...", "..#..", "...#."]),
    ('>', &[".#...", "..#..", "...#.", "....#", "...#.", "..#..", ".#..."]),
    ('%', &["##..#", "##..#", "...#.", "..#..", ".#...", "#..##", "#..##"]),
    ('&', &[".##..", "#..#.", "#.#..", ".#...", "#.#.#", "#..#.", ".##.#"]),
    ('@', &[".###.", "#...#", "#.###", "#.#.#", "#.###", "#....", ".###."]),
    ('$', &["..#..", ".####", "#.#..", ".###.", "..#.#", "####.", "..#.."]),
    ('#', &[".#.#.", ".#.#.", "#####", ".#.#.", "#####", ".#.#.", ".#.#."]),
    // A tiny cheese wedge, usable inline as `~`.
    ('~', &[".....", "...##", ".##.#", "###.#", "#####", ".....", "....."]),
    // A heart, as `^`.
    ('^', &[".#.#.", "#####", "#####", ".###.", "..#..", ".....", "....."]),
];

/// The 3x5 font: A-Z, 0-9 and the punctuation used in small labels. At 3 pixels wide some
/// letters (M, N, W) are approximations; it has no picture glyphs.
#[rustfmt::skip]
const TINY: &[Glyph] = &[
    ('A', &[".#.", "#.#", "###", "#.#", "#.#"]),
    ('B', &["##.", "#.#", "##.", "#.#", "##."]),
    ('C', &[".##", "#..", "#..", "#..", ".##"]),
    ('D', &["##.", "#.#", "#.#", "#.#", "##."]),
    ('E', &["###", "#..", "##.", "#..", "###"]),
    ('F', &["###", "#..", "##.", "#..", "#.."]),
    ('G', &[".##", "#..", "#.#", "#.#", ".##"]),
    ('H', &["#.#", "#.#", "###", "#.#", "#.#"]),
    ('I', &["###", ".#.", ".#.", ".#.", "###"]),
    ('J', &["..#", "..#", "..#", "#.#", ".#."]),
    ('K', &["#.#", "#.#", "##.", "#.#", "#.#"]),
    ('L', &["#..", "#..", "#..", "#..", "###"]),
    ('M', &["#.#", "###", "###", "#.#", "#.#"]),
    ('N', &["#.#", "###", "###", "###", "#.#"]),
    ('O', &[".#.", "#.#", "#.#", "#.#", ".#."]),
    ('P', &["##.", "#.#", "##.", "#..", "#.."]),
    ('Q', &[".#.", "#.#", "#.#", "##.", ".##"]),
    ('R', &["##.", "#.#", "##.", "#.#", "#.#"]),
    ('S', &[".##", "#..", ".#.", "..#", "##."]),
    ('T', &["###", ".#.", ".#.", ".#.", ".#."]),
    ('U', &["#.#", "#.#", "#.#", "#.#", "###"]),
    ('V', &["#.#", "#.#", "#.#", "#.#", ".#."]),
    ('W', &["#.#", "#.#", "###", "###", "#.#"]),
    ('X', &["#.#", "#.#", ".#.", "#.#", "#.#"]),
    ('Y', &["#.#", "#.#", ".#.", ".#.", ".#."]),
    ('Z', &["###", "..#", ".#.", "#..", "###"]),
    ('0', &["###", "#.#", "#.#", "#.#", "###"]),
    ('1', &[".#.", "##.", ".#.", ".#.", "###"]),
    ('2', &["##.", "..#", ".#.", "#..", "###"]),
    ('3', &["##.", "..#", ".#.", "..#", "##."]),
    ('4', &["#.#", "#.#", "###", "..#", "..#"]),
    ('5', &["###", "#..", "##.", "..#", "##."]),
    ('6', &[".##", "#..", "###", "#.#", "###"]),
    ('7', &["###", "..#", ".#.", ".#.", ".#."]),
    ('8', &["###", "#.#", "###", "#.#", "###"]),
    ('9', &["###", "#.#", "###", "..#", "##."]),
    ('.', &["...", "...", "...", "...", ".#."]),
    ('!', &[".#.", ".#.", ".#.", "...", ".#."]),
    ('-', &["...", "...", "###", "...", "..."]),
    ('/', &["..#", "..#", ".#.", "#..", "#.."]),
    ('\'', &[".#.", ".#.", "...", "...", "..."]),
    ('+', &["...", ".#.", "###", ".#.", "..."]),
    (':', &["...", ".#.", "...", ".#.", "..."]),
    ('?', &["##.", "..#", ".#.", "...", ".#."]),
    ('%', &["#.#", "..#", ".#.", "#..", "#.#"]),
    ('(', &[".#.", "#..", "#..", "#..", ".#."]),
    (')', &[".#.", "..#", "..#", "..#", ".#."]),
    ('<', &["..#", ".#.", "#..", ".#.", "..#"]),
    ('*', &["...", "#.#", ".#.", "#.#", "..."]),
    (',', &["...", "...", "...", ".#.", "#.."]),
    ('=', &["...", "###", "...", "###", "..."]),
    ('>', &["#..", ".#.", "..#", ".#.", "#.."]),
    ('&', &[".#.", "#.#", ".#.", "#.#", ".##"]),
    ('$', &[".##", "##.", ".#.", ".##", "##."]),
    ('"', &["#.#", "#.#", "...", "...", "..."]),
    // A heart, as `^` (floats over cheese snug in a hole).
    ('^', &["#.#", "###", "###", ".#.", "..."]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_have_consistent_dimensions() {
        for (c, rows) in NORMAL {
            assert_eq!(rows.len(), 7, "normal {c:?}");
            assert!(rows.iter().all(|r| r.len() == 5), "normal {c:?}");
        }
        for (c, rows) in TINY {
            assert_eq!(rows.len(), 5, "tiny {c:?}");
            assert!(rows.iter().all(|r| r.len() == 3), "tiny {c:?}");
        }
    }

    #[test]
    fn measure_matches_advance() {
        assert_eq!(Font::Normal.measure("AB", 1), 11);
        assert_eq!(Font::Normal.measure("AB", 2), 22);
        assert_eq!(Font::Tiny.measure("", 1), 0);
    }
}
