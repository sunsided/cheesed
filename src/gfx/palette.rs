//! The game's palette. Sprites are ASCII art; each character maps to one entry here, so the
//! whole game shares one fixed palette of a little over 40 colors, the way a VGA game would.
//!
//! The constants are grouped by what they were picked for (night and stone, floorboards,
//! cheese, accents, skin), but any screen may use any of them. Code that draws primitives
//! uses the named constants directly; ASCII-art sprites refer to them through the
//! single-character key in [`from_char`]. The framebuffer itself is true color, so blends,
//! fades and the painted 256-color pictures can produce colors outside this list; the
//! palette is a style constraint for hand-drawn art, not a hardware limit.

/// An 8-bit-per-channel sRGB color: `Rgb(red, green, blue)`.
///
/// There is no alpha; translucency is done by mixing colors with [`Rgb::lerp`] at draw time.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Build a color from a `0xRRGGBB` literal, the way colors are written in image editors.
    /// The top byte of `v` is ignored. `const`, so the palette below can be constants.
    pub const fn hex(v: u32) -> Self {
        Self((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// Linear mix from `self` (at `t = 0`) to `o` (at `t = 1`), per channel, rounded to the
    /// nearest integer. `t` is clamped to 0..=1. The mix is done directly on the sRGB values (no
    /// gamma correction), which is cheap and matches how old palette fades looked.
    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2))
    }

    /// Multiply every channel by `f` and clamp to 0..=255 (truncating), to darken (`f < 1`)
    /// or brighten (`f > 1`) a color.
    pub fn scale(self, f: f32) -> Rgb {
        let m = |a: u8| (a as f32 * f).clamp(0.0, 255.0) as u8;
        Rgb(m(self.0), m(self.1), m(self.2))
    }
}

// Blues and greys: the night outside, the cellar's stone walls, the mouse's fur.
/// Pure black. Key `k`; also the mouse's outline.
pub const BLACK: Rgb = Rgb::hex(0x000000);
/// The darkest blue, the backdrop behind every screen. Key `n`.
pub const NAVY: Rgb = Rgb::hex(0x0b0d1f);
/// Dark night blue, for panel fills and the backdrop's stipple pattern. Key `b`.
pub const NIGHT: Rgb = Rgb::hex(0x1a1f3d);
/// Muted blue, for bar fills, bevel edges and the cellar walls. Key `s`.
pub const SLATE: Rgb = Rgb::hex(0x2c3463);
/// Mid blue-grey, for raised buttons, bevel highlights and the cellar walls. Key `S`.
pub const STEEL: Rgb = Rgb::hex(0x4a5591);
/// Light blue-grey, for secondary text and highlights. Key `l`.
pub const STEEL_LT: Rgb = Rgb::hex(0x7d8cc4);
/// Very light blue, for body text. Key `p`.
pub const PALE: Rgb = Rgb::hex(0xc2cbe8);
/// Pure white. Key `w`.
pub const WHITE: Rgb = Rgb::hex(0xffffff);
/// Dark grey, the mouse's darkest fur. Key `z`.
pub const GREY_DK: Rgb = Rgb::hex(0x3a3a46);
/// Mid grey, the mouse's back and shaded fur. Key `a`.
pub const GREY: Rgb = Rgb::hex(0x6a6a7a);
/// Light grey, the mouse's lit fur and the lock's shackle. Key `A`.
pub const GREY_LT: Rgb = Rgb::hex(0xa8a8b8);

// Floorboards.
/// Darkest brown, the rim of a mouse hole and the hint bubble's text. Key `d`.
pub const BROWN_DK: Rgb = Rgb::hex(0x3b2314);
/// Brown, for the floorboards' seams, the lip of a hole and the cats' paw prints. Key `u`.
pub const BROWN: Rgb = Rgb::hex(0x6b3d1f);
/// The floorboards' main color. Key `c`.
pub const WOOD: Rgb = Rgb::hex(0xa0622e);
/// Lighter wood, the grain flecks in the floorboards. Key `C`.
pub const WOOD_LT: Rgb = Rgb::hex(0xc8864a);
/// Wood highlight. Key `h`.
pub const WOOD_HI: Rgb = Rgb::hex(0xe8b47a);
/// A warm near-black for outlines, text shadows and drop shadows. Key `x`.
pub const INK: Rgb = Rgb::hex(0x1a0e08);

// Cheese.
/// The shaded side and rim of the yellow cheese. Key `O`.
pub const CHEESE_DK: Rgb = Rgb::hex(0xc88a14);
/// Off-white, the inside of the brie, brie trails and the hint bubble. Key `E`.
pub const CREAM: Rgb = Rgb::hex(0xfaf4e0);
/// The brie's rind. Key `r`.
pub const RIND: Rgb = Rgb::hex(0xd8cba4);
/// The brie rind's shaded side; also the order form's fine print. Key `D`.
pub const RIND_DK: Rgb = Rgb::hex(0xa8996e);
/// The cat's darker stripes. Key `H`.
pub const GINGER_DK: Rgb = Rgb::hex(0xa84e10);

// Accents.
/// Dark red, the wheel cheese's shaded wax. Key `m`.
pub const RED_DK: Rgb = Rgb::hex(0x6e1424);
/// Red, for the wheel cheese's wax and warnings. Key `R`.
pub const RED: Rgb = Rgb::hex(0xc42a3c);
/// Pink, for the mouse's ears and nose, hearts and the move counter when over par. Key `P`.
pub const PINK: Rgb = Rgb::hex(0xf06a6a);
/// Orange, the cat's main fur. Key `o`.
pub const ORANGE: Rgb = Rgb::hex(0xf08a24);
/// Cheese yellow, also the main highlight color for titles. Key `y`.
pub const YELLOW: Rgb = Rgb::hex(0xf7d23e);
/// Pale yellow, the lit side of the cheese. Key `Y`.
pub const YELLOW_LT: Rgb = Rgb::hex(0xfff3a0);
/// Dark green. Key `g`.
pub const GREEN_DK: Rgb = Rgb::hex(0x134a2e);
/// Green. Key `G`.
pub const GREEN: Rgb = Rgb::hex(0x2f9a4a);
/// Light green. Key `L`.
pub const GREEN_LT: Rgb = Rgb::hex(0x8ee06a);
/// Dark teal. Key `t`.
pub const TEAL: Rgb = Rgb::hex(0x0f4a5a);
/// Cyan. Key `T`.
pub const CYAN: Rgb = Rgb::hex(0x2ab6c8);
/// Light cyan. Key `i`.
pub const CYAN_LT: Rgb = Rgb::hex(0x9af0f0);
/// Dark purple. Key `v`.
pub const PURPLE_DK: Rgb = Rgb::hex(0x3d1a5a);
/// Purple. Key `V`.
pub const PURPLE: Rgb = Rgb::hex(0x7a3cb0);
/// Magenta. Key `M`.
pub const MAGENTA: Rgb = Rgb::hex(0xd65ad0);
/// Saturated blue. Key `B`.
pub const BLUE: Rgb = Rgb::hex(0x2e5cc8);
/// Dark saturated blue. Key `j`.
pub const BLUE_DK: Rgb = Rgb::hex(0x1c3a8a);
/// Light saturated blue. Key `q`.
pub const BLUE_LT: Rgb = Rgb::hex(0x6a9af0);

// Skin.
/// Shaded skin tone. Key `e`.
pub const SKIN_DK: Rgb = Rgb::hex(0xb06a4a);
/// Skin tone. Key `f`.
pub const SKIN: Rgb = Rgb::hex(0xe8a87c);
/// Lit skin tone. Key `F`.
pub const SKIN_LT: Rgb = Rgb::hex(0xffd2a8);

/// The ASCII-art color key: the palette entry a sprite character stands for, or `None` for
/// a transparent pixel.
///
/// Letters were picked as loose mnemonics where possible (`k` blacK, `w` white, `y`/`Y`
/// yellow and its light shade, `P` pink), and an uppercase/lowercase pair is often a light
/// and dark shade of one hue. `.` and ` ` are the intended transparent characters, but
/// *any* character not listed here also returns `None`, so a typo in sprite art shows up as
/// a hole rather than an error.
pub fn from_char(c: char) -> Option<Rgb> {
    Some(match c {
        'k' => BLACK,
        'n' => NAVY,
        'b' => NIGHT,
        's' => SLATE,
        'S' => STEEL,
        'l' => STEEL_LT,
        'p' => PALE,
        'w' => WHITE,
        'z' => GREY_DK,
        'a' => GREY,
        'A' => GREY_LT,
        'd' => BROWN_DK,
        'u' => BROWN,
        'c' => WOOD,
        'C' => WOOD_LT,
        'h' => WOOD_HI,
        'x' => INK,
        'O' => CHEESE_DK,
        'E' => CREAM,
        'r' => RIND,
        'D' => RIND_DK,
        'H' => GINGER_DK,
        'm' => RED_DK,
        'R' => RED,
        'P' => PINK,
        'o' => ORANGE,
        'y' => YELLOW,
        'Y' => YELLOW_LT,
        'g' => GREEN_DK,
        'G' => GREEN,
        'L' => GREEN_LT,
        't' => TEAL,
        'T' => CYAN,
        'i' => CYAN_LT,
        'v' => PURPLE_DK,
        'V' => PURPLE,
        'M' => MAGENTA,
        'B' => BLUE,
        'j' => BLUE_DK,
        'q' => BLUE_LT,
        'e' => SKIN_DK,
        'f' => SKIN,
        'F' => SKIN_LT,
        _ => return None,
    })
}
