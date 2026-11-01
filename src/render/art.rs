//! Every sprite in the game, as ASCII art (color key: [`crate::gfx::palette::from_char`]).
//! `.` is transparent. Tiles are 16x16.

use crate::gfx::Sprite;

pub struct Art {
    pub floor: [Sprite; 2],
    pub wall: Sprite,
    pub hole: Sprite,
    /// A hole with a cheese snug inside, per kind: wedge, wheel, brie.
    pub hole_full: [Sprite; 3],
    pub mouse_right: [Sprite; 2],
    pub mouse_down: [Sprite; 2],
    pub mouse_up: [Sprite; 2],
    /// Squashed flat, under a paw.
    pub mouse_caught: Sprite,
    /// Sitting upright, facing right (the ending).
    pub mouse_sit: Sprite,
    pub wedge: Sprite,
    pub wheel: Sprite,
    pub brie: Sprite,
    pub cat_right: [Sprite; 2],
    pub cat_pounce: Sprite,
    pub paw: Sprite,
    pub star: Sprite,
    pub star_empty: Sprite,
    pub lock: Sprite,
}

impl Art {
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
            mouse_down: [
                Sprite::parse(MOUSE_DOWN),
                feet_swapped(Sprite::parse(MOUSE_DOWN)),
            ],
            mouse_up: [
                Sprite::parse(MOUSE_UP),
                feet_swapped(Sprite::parse(MOUSE_UP)),
            ],
            mouse_caught: Sprite::parse(MOUSE_CAUGHT),
            mouse_sit: Sprite::parse(MOUSE_SIT),
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

/// A cheap second walk frame for the front and back views: the feet row mirrored.
fn feet_swapped(mut s: Sprite) -> Sprite {
    let y = s.h - 2;
    let row = (y * s.w) as usize..((y + 1) * s.w) as usize;
    s.px[row].reverse();
    s
}

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

const MOUSE_RIGHT: &str = "
    ....kkkk........
    ...kPPPPk.......
    ..kPPPPPPk......
    ..kPPPPPkkkkk...
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

const MOUSE_RIGHT_2: &str = "
    ....kkkk........
    ...kPPPPk.......
    ..kPPPPPPk......
    ..kPPPPPkkkkk...
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

const MOUSE_SIT: &str = "
    ....kkkk........
    ...kPPPPk.......
    ..kPPPPPPk......
    ..kPPPPPkkkkk...
    ...kPPkkAAAAAk..
    ....kkAAAAAAAAk.
    ...kAAAAAAkkAAk.
    ...kAAAAAAkwAAk.
    ...kaAAAPPAAAAPk
    ...kaaaAAAAAAkk.
    ...kaaaaaaAk....
    ...kaaaaaAPk....
    ...kzaaaaaak....
    ..kPkzzzzzk.....
    ..kP.kPkkPk.....
    ...kPk..........
";

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

const CAT_RIGHT: &str = "
    ....k.....k.....
    ...kok...kok....
    ...koHkkkHok....
    ..koooooooook...
    ..kowkooowkok...
    ..koGGoooGGok...
    ..kPooooooPok...
    ...koooxoook..k.
    ....kkwwwkk..kok
    ...koowwwook.kok
    ..kooHowoHook.ko
    ..kooHoooHooookk
    ..kooooooooook..
    ..kwwkkkkkwwk...
    ...kk.....kk....
    ................
";

const CAT_RIGHT_2: &str = "
    ....k.....k.....
    ...kok...kok....
    ...koHkkkHok....
    ..koooooooook...
    ..kowkooowkok...
    ..koGGoooGGok...
    ..kPooooooPok...
    ...koooxoook....
    ....kkwwwkk.....
    ...koowwwook....
    ..kooHowoHook..k
    ..kooHoooHookkok
    ..kooooooooookk.
    ..kwwkkkkkwwk...
    ...kk.....kk....
    ................
";

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

const PAW: &str = "
    .....
    .k.k.
    .....
    .kkk.
    .kkk.
";

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
    fn all_art_parses_and_tiles_are_16x16() {
        let a = Art::new();
        for s in a
            .floor
            .iter()
            .chain([&a.wall, &a.hole, &a.wedge, &a.wheel, &a.brie])
            .chain(&a.mouse_right)
            .chain(&a.mouse_down)
            .chain(&a.mouse_up)
            .chain(&a.cat_right)
            .chain([&a.cat_pounce, &a.mouse_caught, &a.mouse_sit])
        {
            assert_eq!((s.w, s.h), (16, 16));
        }
    }
}
