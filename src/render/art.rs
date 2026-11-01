//! Every sprite in the game, as ASCII art (color key: [`crate::gfx::palette::from_char`]).
//! `.` is transparent. Tiles are 16x16.
//!
//! Conventions for the art below:
//!
//! - One character is one pixel. Letters pick palette colors (`k` black outline, `x` warm
//!   ink outline, `y`/`Y`/`O` the cheese yellows, `a`/`A`/`z` the mouse's greys, `P` pink,
//!   and so on); `.` is transparent. Any character missing from the key is transparent
//!   too, so a typo shows up as a hole, not an error.
//! - Rows are written indented inside a string literal; [`Sprite::parse`] trims every line
//!   and drops blank ones, then asserts that all rows are as wide as the first. A row of
//!   the wrong length panics when [`Art::new`] runs, which the test below does.
//! - Board sprites (floor, walls, holes, the mouse, cats and cheeses) are exactly one tile,
//!   16x16, so they can be blitted at a tile's top-left pixel. Characters face right; the
//!   renderer mirrors them for left. Small decorations (paw print, stars, lock) have their
//!   own sizes and are placed by pixel offsets in the renderer.
//!
//! To keep the art small, several sprites are derived from others instead of being drawn
//! out in full: color variants via [`Sprite::parse_with`] (characters swapped before
//! parsing), animation frames that differ in a few rows via `patched`, and the second
//! walk frame of the front and back views via `feet_swapped`.

use crate::gfx::Sprite;

/// All sprites, parsed once at startup and owned by the renderer. Arrays of two are walk
/// cycles: index 0 is the standing frame and index 1 the stride frame.
pub struct Art {
    /// Two floorboard variants, laid out in a checkerboard so the boards do not repeat every
    /// tile.
    pub floor: [Sprite; 2],
    /// A stone wall block. Only walls that touch the inside of the level are drawn.
    pub wall: Sprite,
    /// An empty mouse hole: a dark pit with a brown lip on its lower edge (row 13), the row
    /// cheese is clipped against as it sinks in.
    pub hole: Sprite,
    /// A hole with a cheese snug inside, per kind: wedge, wheel, brie.
    pub hole_full: [Sprite; 3],
    /// The mouse in side view facing right (mirrored for left): standing and mid-stride.
    pub mouse_right: [Sprite; 2],
    /// The mouse seen from the front (walking down): standing, and the same with the feet row
    /// mirrored as the stride frame.
    pub mouse_down: [Sprite; 2],
    /// The mouse seen from behind (walking up): standing, and the same with the feet row
    /// mirrored as the stride frame.
    pub mouse_up: [Sprite; 2],
    /// Idle ear twitch frames: right (mirrored for left), down, up. Each is its view's standing
    /// frame with the top four rows replaced, see `SIDE_TWITCH`, `FRONT_TWITCH` and
    /// `BACK_TWITCH`.
    pub mouse_twitch: [Sprite; 3],
    /// Squashed flat, under a paw.
    pub mouse_caught: Sprite,
    /// The wedge of yellow cheese.
    pub wedge: Sprite,
    /// The red-waxed cheese wheel.
    pub wheel: Sprite,
    /// The brie, which leaves a trail when it slides.
    pub brie: Sprite,
    /// A cat walking right (mirrored for left): standing and mid-stride.
    pub cat_right: [Sprite; 2],
    /// A cat mid-pounce, drawn over the mouse when she is caught.
    pub cat_pounce: Sprite,
    /// A 5x5 paw print (one pad, two toes). The renderer stamps two of them as a flat brown
    /// silhouette on the tile each cat will step onto next.
    pub paw: Sprite,
    /// A 9x9 gold star for the par rating.
    pub star: Sprite,
    /// The same star recolored to dark blues, for stars not (yet) earned.
    pub star_empty: Sprite,
    /// A 9x11 padlock for locked levels and episodes.
    pub lock: Sprite,
}

impl Art {
    /// Parse every sprite. Panics if any art has rows of unequal width, so a broken drawing
    /// fails at startup (and in the tests) rather than rendering garbled.
    ///
    /// The three `hole_full` variants come from one drawing of a wedge in a hole, recolored:
    /// the wheel swaps the yellows for red wax (`y` to `R`, `O` to `m`) and exchanges the light
    /// yellow and pink (`Y` and `P`); the brie swaps the yellows for cream, white and rind.
    pub fn new() -> Self {
        Self {
            floor: [Sprite::parse(FLOOR_A), Sprite::parse(FLOOR_B)],
            wall: Sprite::parse(WALL),
            hole: Sprite::parse(HOLE),
            hole_full: [
                Sprite::parse(HOLE_FULL),
                Sprite::parse_with(HOLE_FULL, &[('y', 'R'), ('Y', 'P'), ('O', 'm'), ('P', 'Y')]),
                Sprite::parse_with(HOLE_FULL, &[('y', 'E'), ('Y', 'w'), ('O', 'r')]),
            ],
            mouse_right: [Sprite::parse(MOUSE_RIGHT), Sprite::parse(MOUSE_RIGHT_2)],
            mouse_twitch: [
                patched(MOUSE_RIGHT, &SIDE_TWITCH),
                patched(MOUSE_DOWN, &FRONT_TWITCH),
                patched(MOUSE_UP, &BACK_TWITCH),
            ],
            mouse_down: [
                Sprite::parse(MOUSE_DOWN),
                // The feet row is symmetric, so mirroring it changes nothing: shuffle the
                // feet inward instead.
                patched(MOUSE_DOWN, &[(14, "...kPkkkkkkPk...")]),
            ],
            mouse_up: [
                Sprite::parse(MOUSE_UP),
                feet_swapped(Sprite::parse(MOUSE_UP)),
            ],
            mouse_caught: Sprite::parse(MOUSE_CAUGHT),
            wedge: Sprite::parse(WEDGE),
            wheel: Sprite::parse(WHEEL),
            brie: Sprite::parse(BRIE),
            cat_right: [Sprite::parse(CAT_RIGHT), Sprite::parse(CAT_RIGHT_2)],
            cat_pounce: Sprite::parse(CAT_POUNCE),
            paw: Sprite::parse(PAW),
            star: Sprite::parse(STAR),
            star_empty: Sprite::parse_with(STAR, &[('y', 's'), ('Y', 'S'), ('O', 'b')]),
            lock: Sprite::parse(LOCK),
        }
    }
}

impl Default for Art {
    fn default() -> Self {
        Self::new()
    }
}

/// The side view with the near ear flicked back (the far one stays perked). Replaces rows
/// 0..=3 of [`MOUSE_RIGHT`]; each entry is `(row index, new row)`.
const SIDE_TWITCH: [(usize, &str); 4] = [
    (0, "..........k....."),
    (1, "...kkkkk..kPk..."),
    (2, "..kPPPPPkkPPPk.."),
    (3, "..kPPPPkkkkkkk.."),
];

/// Front and back views with the left ear flopped out sideways. Replaces rows 0..=3 of
/// [`MOUSE_DOWN`].
const FRONT_TWITCH: [(usize, &str); 4] = [
    (0, "...........kkk.."),
    (1, "..........kPPPk."),
    (2, ".kkkk.kkkkPPPPPk"),
    (3, "kPPPPkAAAAkkPPPk"),
];

/// The back view's flop (its head is the darker grey `a`, where the front view's face is
/// the light grey `A`). Replaces rows 0..=3 of [`MOUSE_UP`].
const BACK_TWITCH: [(usize, &str); 4] = [
    (0, "...........kkk.."),
    (1, "..........kPPPk."),
    (2, ".kkkk.kkkkPPPPPk"),
    (3, "kPPPPkaaaakkPPPk"),
];

/// Parse `art` with some rows replaced (for animation frames that differ in a few rows).
///
/// `art` is split into rows exactly the way [`Sprite::parse`] would (trimmed, blank lines
/// dropped), so the indices in `rows` count drawn rows from the top, starting at 0. The
/// replacement rows must have the same width, or the parse panics.
fn patched(art: &str, rows: &[(usize, &str)]) -> Sprite {
    let mut lines: Vec<&str> = art
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    for &(i, row) in rows {
        lines[i] = row;
    }
    Sprite::parse(&lines.join("\n"))
}

/// A cheap second walk frame for the front and back views: the feet row mirrored.
///
/// The feet are on the second-to-last row (the last row is left free for the tail in the
/// back view). Reversing that one row moves an off-center foot or tail detail to the other
/// side, which reads as a step when the two frames alternate. If the row is already
/// symmetric, the frame comes out identical to the original, which is why the front view
/// uses a hand-made feet row via `patched` instead.
fn feet_swapped(mut s: Sprite) -> Sprite {
    let y = s.h - 2;
    let row = (y * s.w) as usize..((y + 1) * s.w) as usize;
    s.px[row].reverse();
    s
}

/// Floorboards, first variant: planks with grain flecks and brown seams.
const FLOOR_A: &str = "
    ccccccccccccccCc
    cCCCcccccccccccc
    cccccccccCcccccc
    uuuuuuuuuuuuuuuu
    CccccccuccccccCc
    ccccccccucccCccc
    cccCcccculcccccc
    uuuuuuuuuuuuuuuu
    cccccCcccccccccc
    cccccccccccccCcc
    cCcccccccccccccc
    uuuuuuuuuuuuuuuu
    cccccccccccuCccc
    cccCccccccccuccc
    ccccccccccccuccC
    uuuuuuuuuuuuuuuu
";

/// Floorboards, second variant, with the seams and grain in different places.
const FLOOR_B: &str = "
    cccccccccCcccccc
    cccccccccccccCcc
    cCcccccccccccccc
    uuuuuuuuuuuuuuuu
    ccccucccccccCccc
    cccculcccCcccccc
    cCccuccccccccccc
    uuuuuuuuuuuuuuuu
    cccccccccccucccc
    ccCccccccccuccCc
    cccccccCcccucccc
    uuuuuuuuuuuuuuuu
    cccccCcccccccccc
    cccccccccccccccC
    ccCccccccccccccc
    uuuuuuuuuuuuuuuu
";

/// A wall of blue stone blocks in three shades, with dark mortar lines between them.
const WALL: &str = "
    llllllllblllllll
    lSSSSSSSblSSSSSS
    lSSSSSSSblSSSSSS
    sssssssssbssssss
    bbbbbbbbbbbbbbbb
    llllblllllllllbl
    SSSSblSSSSSSSSbl
    SSSSblSSSSSSSSbl
    ssssbsssssssssbs
    bbbbbbbbbbbbbbbb
    llllllllllblllll
    lSSSSSSSSSblSSSS
    lSSSSSSSSSblSSSS
    ssssssssssbsssss
    bbbbbbbbbbbbbbbb
    llllblllllllllbl
";

/// An empty mouse hole: a black pit (rows 3..=12) with a dark brown rim and a brown lip on
/// row 13.
const HOLE: &str = "
    ................
    ................
    ......dddd......
    ....ddkkkkdd....
    ...dkkkkkkkkd...
    ..dkkkkkkkkkkd..
    ..dkkkkkkkkkkd..
    .dkkkkkkkkkkkkd.
    .dkkkkkkkkkkkkd.
    .dkkkkkkkkkkkkd.
    .dkkkkkkkkkkkkd.
    .dkkkkkkkkkkkkd.
    .dkkkkkkkkkkkkd.
    .duuuuuuuuuuuud.
    ................
    ................
";

/// A hole filled with a wedge of cheese (yellow with holes), the same outline as [`HOLE`]
/// but in cheese colors. The source of all three `hole_full` variants.
const HOLE_FULL: &str = "
    ................
    ................
    ......OOOO......
    ....OOYYYYOO....
    ...OYYYyYYYYO...
    ..OYYyyyyyyyYO..
    ..OYyyOyyyyyyO..
    .OYyyxyyyyxyyyO.
    .OYyxyxyyxyxyyO.
    .OyPyyyyyyyyPyO.
    .OyyyyyxxyyyyyO.
    .OyOyyyyyyyyOyO.
    .OyyyyyyyOyyyyO.
    .OuuuuuuuuuuuuO.
    ................
    ................
";

/// The mouse in side view, facing right, standing.
const MOUSE_RIGHT: &str = "
    ....kkkk..k.....
    ...kPPPPk.kPk...
    ..kPPPPPPkPPPk..
    ..kPPPPPkkkkkk..
    ...kPPkkAAAAAk..
    ....kkAAAAAAAAk.
    ...kAAAAAAkkAAk.
    ...kAAAAAAkwAAk.
    ..kaAAAAPPAAAAPk
    .kaaaAAAAAAAAkk.
    .kaaaaaAAAAAk...
    kPkaaaaaaaaak...
    kP.kzaaaaazk....
    .kPkPkkkkkPk....
    ..k.............
    ................
";

/// The mouse in side view, facing right, mid-stride.
const MOUSE_RIGHT_2: &str = "
    ....kkkk..k.....
    ...kPPPPk.kPk...
    ..kPPPPPPkPPPk..
    ..kPPPPPkkkkkk..
    ...kPPkkAAAAAk..
    ....kkAAAAAAAAk.
    ...kAAAAAAkkAAk.
    ...kAAAAAAkwAAk.
    ..kaAAAAPPAAAAPk
    .kaaaAAAAAAAAkk.
    .kaaaaaAAAAAk...
    kPkaaaaaaaaak...
    kP.kzaaaaazk....
    .kPk.kPkkPk.....
    ..k.............
    ................
";

/// The mouse from the front: ears, eyes, pink nose and paws, feet on row 14.
const MOUSE_DOWN: &str = "
    ..kkk......kkk..
    .kPPPk....kPPPk.
    kPPPPPkkkkPPPPPk
    kPPPkkAAAAkkPPPk
    .kkkAAAAAAAAkkk.
    ..kAAAAAAAAAAk..
    .kAAkkAAAAkkAAk.
    .kAAkwAAAAwkAAk.
    .kPPAAAAAAAAPPk.
    ..kAAAAPPAAAAk..
    ...kAAAkkAAAk...
    ..kaaaaaaaaaak..
    .kaaaAAAAAAaaak.
    .kzaaaaaaaaaazk.
    ..kPkkkkkkkkPk..
    ................
";

/// The mouse from behind: ears and a grey back, feet on row 14 and the tail tip on row 15.
const MOUSE_UP: &str = "
    ..kkk......kkk..
    .kPPPk....kPPPk.
    kPPPPPkkkkPPPPPk
    kPPPkkaaaakkPPPk
    .kkkaaaaaaaakkk.
    ..kaaaaaaaaaak..
    .kaaaaaaaaaaaak.
    .kaaaaaaaaaaaak.
    .kaaaaaaaaaaaak.
    ..kaaaaaaaaaak..
    ...kaaaaaaaak...
    ..kaaaaaaaaaak..
    .kaaaaaaaaaaaak.
    .kzzaaaaaaaazzk.
    ..kPkkkPkkkkPk..
    .......kP.......
";

/// The mouse squashed flat, shown under the pouncing cat.
const MOUSE_CAUGHT: &str = "
    ................
    ................
    ................
    ................
    ................
    ................
    ................
    ................
    ................
    ................
    ..kk........kk..
    .kPPkkkkkkkkPPk.
    .kaaaaaaaaaaaak.
    kzaaAkkAAkkAaazk
    .kzzzzzzzzzzzzk.
    ..kkkkkkkkkkkk..
";

/// A wedge of yellow cheese.
const WEDGE: &str = "
    ................
    ................
    ................
    ................
    ...........kk...
    .........kkYYk..
    .......kkYYYYk..
    .....kkYYYYYYYk.
    ...kkYYYYYYYYYk.
    .kkyyyyyyyyyyyyk
    .kyyOyyyyyyyOyyk
    .kyyyykyyykyyyyk
    .kyOyPyyyyyPyyyk
    .kyyyyykkkyyyOyk
    .kOyyyyyyyyyyyOk
    ..kkkkkkkkkkkkk.
";

/// A round cheese in red wax, with pink highlights and darker wax on the shaded side.
const WHEEL: &str = "
    ................
    ................
    .....kkkkkk.....
    ...kkRRRRRRkk...
    ..kRRPPRRRRRRk..
    .kRRPPRRRRRRRRk.
    .kRPRRRRRRRRRRk.
    kRRPRRkRRRkRRRmk
    kRRRRRkRRRkRRRmk
    kRRRPPRRRRRPPRmk
    kRRRRRRkkkRRRmmk
    .kRRRRRRRRRRRmk.
    .kmRRRRRRRRRmmk.
    ..kmmRRRRRmmmk..
    ...kkmmmmmmkk...
    .....kkkkkk.....
";

/// A round brie: a cream top with its rind showing along the lower edge.
const BRIE: &str = "
    ................
    ................
    ................
    ................
    ................
    ....kkkkkkkk....
    ..kkEEEEEEEEkk..
    .kEEEEEEEEEEEEk.
    kEEEEEEwwEEEEEEk
    kEEEEkEEEEkEEEEk
    krEEPkEEEEkPEErk
    kDrrEEEkkEEErrDk
    kYDDrrrrrrrrDDYk
    .kYYYYYYYYYYYYk.
    ..kkkkkkkkkkkk..
    ................
";

/// A ginger cat facing right, standing: dark stripes, green eyes, white chest and paws.
const CAT_RIGHT: &str = "
    .....k.....k....
    ....kok...kok...
    ....koHkkkHok...
    ...koooooooook..
    ...kokwoookwok..
    ...koGGoooGGok..
    ...koPooooooPk..
    .k..koooxoook...
    kok..kkwwwkk....
    kok.koowwwook...
    ok.kooHowoHook..
    kkooooHoooHook..
    ..kooooooooook..
    ...kwwkkkkkwwk..
    ....kk.....kk...
    ................
";

/// A ginger cat facing right, mid-stride.
const CAT_RIGHT_2: &str = "
    .....k.....k....
    ....kok...kok...
    ....koHkkkHok...
    ...koooooooook..
    ...kokwoookwok..
    ...koGGoooGGok..
    ...koPooooooPk..
    ....koooxoook...
    .....kkwwwkk....
    ....koowwwook...
    k..kooHowoHook..
    kokkooHoooHook..
    .kkooooooooook..
    ...kwwkkkkkwwk..
    ....kk.....kk...
    ................
";

/// A ginger cat pouncing, drawn four pixels above the caught mouse.
const CAT_POUNCE: &str = "
    ..........k...k.
    .........kok.kok
    .........koooook
    k.......kookoook
    ok......kooooooP
    kok.....kkoRRRk.
    .kok..kkkooRRk..
    ..kkkoooHoook...
    ..kooHooHooook..
    ..kooHooHoooook.
    ..kooooooooooook
    kkoooooooooooook
    kwwkooooooooookk
    .kk.kkkkkkk.kwwk
    ............kkk.
    ................
";

/// A paw print: two toes over a pad. Drawn as a silhouette, so only its shape matters.
const PAW: &str = "
    .....
    .k.k.
    .....
    .kkk.
    .kkk.
";

/// A gold star with an ink outline and a shaded right side.
const STAR: &str = "
    ....x....
    ...xYx...
    ...xYx...
    xxxxYyxxx
    xYYYyyyOx
    .xyyyyOx.
    .xyyxyOx.
    xyOx.xOOx
    xxx...xxx
";

/// A padlock: grey shackle over a gold body with a keyhole.
const LOCK: &str = "
    ..kkkkk..
    .kAAAAAk.
    .kAk.kAk.
    .kAk.kAk.
    kkkkkkkkk
    kyyyyyyOk
    kyyyxyyOk
    kyyyxyyOk
    kyyyyyyOk
    kOOOOOOOk
    kkkkkkkkk
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Building [`Art`] parses every sprite (so any row-width mistake panics here), and every
    /// board sprite must be exactly one 16x16 tile.
    fn all_art_parses_and_tiles_are_16x16() {
        let a = Art::new();
        for s in a
            .floor
            .iter()
            .chain([&a.wall, &a.hole, &a.wedge, &a.wheel, &a.brie])
            .chain(&a.mouse_right)
            .chain(&a.mouse_down)
            .chain(&a.mouse_up)
            .chain(&a.mouse_twitch)
            .chain(&a.cat_right)
            .chain([&a.cat_pounce, &a.mouse_caught])
        {
            assert_eq!((s.w, s.h), (16, 16));
        }
    }
}
