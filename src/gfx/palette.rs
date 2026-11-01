//! The game's palette. Sprites are ASCII art; each character maps to one entry here, so the
//! whole game shares one ~45-color palette the way a VGA game would.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Self((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(m(self.0, o.0), m(self.1, o.1), m(self.2, o.2))
    }

    pub fn scale(self, f: f32) -> Rgb {
        let m = |a: u8| (a as f32 * f).clamp(0.0, 255.0) as u8;
        Rgb(m(self.0), m(self.1), m(self.2))
    }
}

// Blues and greys: the night outside, the cellar's stone walls, the mouse's fur.
pub const BLACK: Rgb = Rgb::hex(0x000000);
pub const NAVY: Rgb = Rgb::hex(0x0b0d1f);
pub const NIGHT: Rgb = Rgb::hex(0x1a1f3d);
pub const SLATE: Rgb = Rgb::hex(0x2c3463);
pub const STEEL: Rgb = Rgb::hex(0x4a5591);
pub const STEEL_LT: Rgb = Rgb::hex(0x7d8cc4);
pub const PALE: Rgb = Rgb::hex(0xc2cbe8);
pub const WHITE: Rgb = Rgb::hex(0xffffff);
pub const GREY_DK: Rgb = Rgb::hex(0x3a3a46);
pub const GREY: Rgb = Rgb::hex(0x6a6a7a);
pub const GREY_LT: Rgb = Rgb::hex(0xa8a8b8);

// Floorboards.
pub const BROWN_DK: Rgb = Rgb::hex(0x3b2314);
pub const BROWN: Rgb = Rgb::hex(0x6b3d1f);
pub const WOOD: Rgb = Rgb::hex(0xa0622e);
pub const WOOD_LT: Rgb = Rgb::hex(0xc8864a);
pub const WOOD_HI: Rgb = Rgb::hex(0xe8b47a);
/// Outlines.
pub const INK: Rgb = Rgb::hex(0x1a0e08);

// Cheese.
pub const CHEESE_DK: Rgb = Rgb::hex(0xc88a14);
pub const CREAM: Rgb = Rgb::hex(0xfaf4e0);
pub const RIND: Rgb = Rgb::hex(0xd8cba4);
pub const RIND_DK: Rgb = Rgb::hex(0xa8996e);
/// The cat's darker stripes.
pub const GINGER_DK: Rgb = Rgb::hex(0xa84e10);

// Accents.
pub const RED_DK: Rgb = Rgb::hex(0x6e1424);
pub const RED: Rgb = Rgb::hex(0xc42a3c);
pub const PINK: Rgb = Rgb::hex(0xf06a6a);
pub const ORANGE: Rgb = Rgb::hex(0xf08a24);
pub const YELLOW: Rgb = Rgb::hex(0xf7d23e);
pub const YELLOW_LT: Rgb = Rgb::hex(0xfff3a0);
pub const GREEN_DK: Rgb = Rgb::hex(0x134a2e);
pub const GREEN: Rgb = Rgb::hex(0x2f9a4a);
pub const GREEN_LT: Rgb = Rgb::hex(0x8ee06a);
pub const TEAL: Rgb = Rgb::hex(0x0f4a5a);
pub const CYAN: Rgb = Rgb::hex(0x2ab6c8);
pub const CYAN_LT: Rgb = Rgb::hex(0x9af0f0);
pub const PURPLE_DK: Rgb = Rgb::hex(0x3d1a5a);
pub const PURPLE: Rgb = Rgb::hex(0x7a3cb0);
pub const MAGENTA: Rgb = Rgb::hex(0xd65ad0);
pub const BLUE: Rgb = Rgb::hex(0x2e5cc8);
pub const BLUE_DK: Rgb = Rgb::hex(0x1c3a8a);
pub const BLUE_LT: Rgb = Rgb::hex(0x6a9af0);

// Skin.
pub const SKIN_DK: Rgb = Rgb::hex(0xb06a4a);
pub const SKIN: Rgb = Rgb::hex(0xe8a87c);
pub const SKIN_LT: Rgb = Rgb::hex(0xffd2a8);

/// ASCII-art key. `.` and ` ` are transparent.
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
