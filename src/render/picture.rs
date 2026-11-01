//! Full-screen paintings (the title cover, the episode intro cards, the ending, defeat,
//! "Onward!" and farewell screens), reduced to 256 colors like a VGA game's pictures, and
//! the effects that bring them to life.
//!
//! # Format and pipeline
//!
//! The source paintings live in `art/` (kept out of git). `tools/cover.py` crops each one to
//! at most 16:10, scales it to 200 rows, quantizes it to a 256-colour palette with
//! Floyd-Steinberg dithering, and writes a tiny raw format next to this file
//! (`src/render/*.bin`, regenerated with `task cover`). The game embeds those files with
//! `include_bytes!` and [`Picture::parse`] reads them back: a palette plus one palette index
//! per pixel. A picture is normally exactly 320x200; a narrower one is centered and its
//! side margins are filled by [`Picture::draw`].
//!
//! # Coordinates and units
//!
//! Every rectangle, centre and point passed to the builders below is in *picture pixels*:
//! `(0, 0)` is the painting's top-left corner, `x` grows right and `y` grows down.
//! Rectangles are `(x0, y0, x1, y1)`, inclusive at the top-left and exclusive at the
//! bottom-right, and are clipped to the picture. [`Picture::origin`] converts to screen
//! pixels (for a 320-wide picture the two coincide). Time `t` is in seconds since the
//! screen appeared; colours are 8-bit RGB; `luma` is 0..255.
//!
//! # How the effects work
//!
//! A 256-colour picture invites palette cycling, but these palettes are produced by an
//! automatic quantizer, so no run of palette slots belongs to "the water" or "the flag".
//! Instead every effect works on the painting's *own pixels*: at construction time a builder
//! (`with_water`, `with_glow`, `with_twinkle`, `with_flash`, `with_flutter`, `with_fog`)
//! scans a hand-placed rectangle, keeps the pixels a colour predicate accepts, and stores
//! their positions and original colours. Each frame, [`Picture::draw`] first paints the
//! picture, then every effect overwrites its own pixels with a colour computed purely from
//! the stored original colour, the pixel's position and `t`. Effects never read the screen,
//! so a frame is a pure function of `t` and nothing accumulates or drifts. They are drawn in
//! a fixed order (water, glow, twinkle, flash, flutter, fog); where two effects select the
//! same pixel the later one wins.
//!
//! The rectangles, centres and colour thresholds in the per-picture constructors
//! ([`Picture::cover`], [`Picture::intro_episode_1`], ...) were tuned by eye against one
//! specific painting. Several predicates are deliberately tight: a loose "red" also picks up
//! a sunset sky, a loose "warm and bright" picks up the heroine's face, a loose "bright and
//! bluish" picks up the moon. When a painting is replaced, every rectangle and threshold for
//! it has to be re-checked: run `task cover`, then `task snapshot`, and look at the result
//! (and at the pixel-count tests at the bottom of this file, which catch a selection that
//! suddenly finds nothing or far too much).
//!
//! The free functions at the end ([`sparkle`], [`gleam`], [`flare`], [`soft_cross`],
//! [`twinkle_stars`], [`glints`], [`hash`]) are overlays: they draw *on top of* the picture
//! at fixed points instead of changing its pixels, and are called from the per-screen
//! modules (`title.rs`, `intro.rs`, ...).

use crate::gfx::palette::{self as pal, Rgb};
use crate::gfx::{H, Screen, W};

/// A palette-indexed painting plus the animated effects layered on its pixels.
///
/// Build one with [`Picture::parse`] (or a per-screen constructor such as
/// [`Picture::cover`]), chain `with_*` builders to attach effects, then call
/// [`Picture::draw`] every frame.
pub struct Picture {
    /// Width in pixels (320 for every current painting; at most the screen width).
    pub w: i32,
    /// Height in pixels (always 200, the screen height, for the embedded paintings).
    pub h: i32,
    /// The picture's own colours, at most 256; `px` indexes into this.
    palette: Vec<Rgb>,
    /// One palette index per pixel, row-major, `w * h` entries.
    px: Vec<u8>,
    /// Animated water, drawn over the picture.
    water: Vec<Water>,
    /// Radiant pulses (sunbursts, glowing cheese).
    glow: Vec<Glow>,
    /// Flickering points (stars, candle-lit windows).
    twinkle: Vec<Twinkle>,
    /// Pixels lit up by the lightning (bolts, storm clouds).
    flash: Vec<Flash>,
    /// Cloth waving in the wind (flags).
    flutter: Vec<Flutter>,
    /// Mist drifting through a valley.
    fog: Vec<Fog>,
}

/// Valley mist: soft, brighter wisps drift slowly sideways through the fog pixels, with a
/// gentle vertical sway, so the mist breathes without changing its shape.
///
/// The pixels are chosen by [`Picture::with_fog`]: pale, bluish, unsaturated pixels inside
/// hand-placed valley rectangles.
pub struct Fog {
    /// The fog pixels and their painted colours.
    px: Vec<Px>,
    /// How strongly the wisps lighten and the gaps between them darken; 1.0 is the
    /// farewell's valley. The title cover uses 1.9: its painted mist is thinner and busier,
    /// so at 1.0 the drift barely shows there. Raising one picture's strength does not
    /// affect the other; tune each against its own painting.
    strength: f32,
}

impl Fog {
    /// Repaint the fog pixels at time `t`; `(ox, oy)` is the picture origin on screen.
    ///
    /// A wisp is a travelling sine across `x`: wavelength 34 px, moving 0.17 wavelengths per
    /// second (about 5.8 px/s) towards +x. Its phase is shifted per row by
    /// `sin(0.3 * y + 0.6 * t) * 1.1` radians, so the wisp fronts are bent and slowly
    /// sway up and down instead of being vertical stripes. `wisp` is 0..1; it is squared
    /// when lightening, so only the crests brighten noticeably: each pixel is pulled towards
    /// `PALE` by `0.45 * strength * wisp^2` (capped at 0.9) and then darkened by up to
    /// `18% * strength` in the troughs. The painted shape of the mist stays; only its
    /// brightness moves.
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        use std::f32::consts::TAU;
        for &(x, y, c) in &self.px {
            let sway = (y as f32 * 0.3 + t * 0.6).sin() * 1.1;
            let wisp = 0.5 + 0.5 * (TAU * (x as f32 / 34.0 - t * 0.17) + sway).sin();
            let k = self.strength;
            let lit = c
                .lerp(pal::PALE, (0.45 * k * wisp * wisp).min(0.9))
                .scale(1.0 - 0.18 * k * (1.0 - wisp));
            s.put(ox + x, oy + y, lit);
        }
    }
}

/// Flags in the wind: light and shade ripple along the cloth.
///
/// The pixels are chosen by [`Picture::with_flutter`]: a per-picture "flag red" predicate
/// inside a tight box around each flag.
pub struct Flutter {
    /// The flag pixels and their painted colours.
    px: Vec<Px>,
}

impl Flutter {
    /// Repaint the flag pixels at time `t`; `(ox, oy)` is the picture origin on screen.
    ///
    /// Brightness follows `sin(9 t - 1.3 x + 0.5 y)`: about 1.4 ripples per second, a
    /// ripple every ~4.8 px along the cloth, travelling towards +x at ~7 px/s, slightly
    /// slanted by the `y` term. The painted colour is scaled by `0.8 + 0.35 * wave`, i.e.
    /// between 0.45 and 1.15, so the cloth is on average a little darker than painted and
    /// the crests a little brighter. The flags are only a few pixels across, so this reads
    /// as cloth snapping in the wind rather than as a wave.
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        for &(x, y, c) in &self.px {
            let wave = (t * 9.0 - x as f32 * 1.3 + y as f32 * 0.5).sin();
            s.put(ox + x, oy + y, c.scale(0.8 + 0.35 * wave));
        }
    }
}

/// A picture pixel selected for an effect: position (picture pixels) and its painted color.
type Px = (i32, i32, Rgb);

/// Warm, bright pixels pulse brighter in rings travelling outwards from a center, so rays
/// seem to radiate.
///
/// Only pixels already painted as part of the glow (warm and bright, see
/// [`Picture::with_glow`]) take part, so the rings follow the painted rays and halo instead
/// of drawing new circles.
pub struct Glow {
    /// Pixels and their distance from the center, in picture pixels.
    px: Vec<(i32, i32, Rgb, f32)>,
    /// How far a pixel is pulled towards `YELLOW_LT` at a ring's crest (0..1).
    amount: f32,
}

impl Glow {
    /// Repaint the glow pixels at time `t`; `(ox, oy)` is the picture origin on screen.
    ///
    /// `pulse = 0.5 + 0.5 * sin(TAU * (0.8 t - d / 22))`, with `d` the distance from the
    /// centre: rings 22 px apart, 0.8 rings per second leaving the centre (a ring every
    /// 1.25 s, moving outwards at ~17.6 px/s). Each pixel is lerped towards `YELLOW_LT` by
    /// `amount * pulse`, so between rings it shows its painted colour.
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        for &(x, y, c, d) in &self.px {
            let pulse = 0.5 + 0.5 * (std::f32::consts::TAU * (t * 0.8 - d / 22.0)).sin();
            s.put(ox + x, oy + y, c.lerp(pal::YELLOW_LT, self.amount * pulse));
        }
    }
}

/// Points that flicker independently; `cluster` groups neighbors (a whole window).
///
/// Used for lit windows, torch flames and road lamps. Flicker only ever *dims* a pixel
/// below its painted colour; it never brightens it.
pub struct Twinkle {
    /// Pixels, their painted colour and a stable per-cluster phase in 0..1 (from
    /// `hash01` of the cluster's cell), so pixels of one cluster flicker in step and
    /// neighbouring clusters do not.
    px: Vec<(i32, i32, Rgb, f32)>,
    /// How dark a flicker gets (0 = not at all, 1 = black).
    depth: f32,
    /// Angular speed of the flicker in radians per second (5.0 is about 0.8 Hz for the
    /// base sine; a second sine runs 1.73 times faster).
    rate: f32,
}

impl Twinkle {
    /// Repaint the twinkling pixels at time `t`; `(ox, oy)` is the picture origin on screen.
    ///
    /// Two sines at `rate` and `1.73 * rate` (an irrational-ish ratio, so the pattern does
    /// not visibly repeat), each offset by the cluster's phase, are multiplied. Their
    /// product (-1..1) is mapped to 0..1 and scaled by `depth`, and the pixel is darkened by
    /// that fraction. On average a twinkling pixel therefore sits about `depth / 2` below its
    /// painted brightness, flickering irregularly between full and `1 - depth`.
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        for &(x, y, c, phase) in &self.px {
            let a = (t * self.rate + phase * 13.0).sin();
            let b = (t * self.rate * 1.73 + phase * 7.0).sin();
            let dim = ((a * b) * 0.5 + 0.5) * self.depth;
            s.put(ox + x, oy + y, c.scale(1.0 - dim));
        }
    }
}

/// Pixels that blaze up with the title's lightning schedule.
///
/// The timing comes from [`super::title::flash_strength`], the same schedule that tints the
/// title screen and (through [`super::title::struck`]) cues the thunder sound in `app.rs`,
/// so the painted bolts, the screen flash and the thunder stay in sync. Episode 1's intro
/// card uses the same schedule for its storm cloud.
pub struct Flash {
    /// The bolt or cloud pixels and their painted colours.
    px: Vec<Px>,
    /// Multiplier on the flash strength (0..1); below 1 for distant or secondary bolts.
    intensity: f32,
}

impl Flash {
    /// Repaint the flash pixels at time `t`; `(ox, oy)` is the picture origin on screen.
    ///
    /// With `k = flash_strength(t) * intensity` (1 during the double flash, a fading
    /// afterglow of at most 0.45 for 0.6 s, 0 otherwise), each pixel is lerped towards white
    /// by `k`, and its four neighbours are blended towards a pale violet (`#e8d8ff`) by
    /// `0.35 k`. Neighbours shared by several bolt pixels are blended several times, so the
    /// halo is strongest along the bolt's length and the one-pixel line reads thicker while
    /// it burns. Between strikes nothing is drawn and the painting shows as painted.
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        let k = super::title::flash_strength(t) * self.intensity;
        if k <= 0.0 {
            return;
        }
        for &(x, y, c) in &self.px {
            s.put(ox + x, oy + y, c.lerp(pal::WHITE, k));
            // A faint halo, so a bolt reads thicker while it burns.
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                s.blend(ox + x + dx, oy + y + dy, pal::Rgb::hex(0xe8d8ff), k * 0.35);
            }
        }
    }
}

/// Perceived brightness, 0..255: an integer approximation of Rec. 601 luma
/// (0.3 R + 0.6 G + 0.1 B). Used by the colour predicates and the water ramp.
fn luma(c: Rgb) -> u32 {
    (c.0 as u32 * 3 + c.1 as u32 * 6 + c.2 as u32) / 10
}

/// Cheap stable hash of a position into 0..1 (24 bits of an integer mix).
///
/// Deterministic across runs and platforms, so a twinkle cluster always gets the same
/// phase. [`Picture::with_twinkle`] feeds it cluster cells (`x / cluster, y / cluster`), not
/// raw pixels.
fn hash01(x: i32, y: i32) -> f32 {
    let h = (x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263)) as u32;
    let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Fake color cycling for water. A VGA artist would paint water with a run of consecutive
/// palette slots and rotate them; our pictures are auto-quantized, so instead each water
/// pixel remembers its place on the region's own brightness ramp and gets nudged up and
/// down that ramp by bands that march in the flow direction. The painting's shading stays,
/// the highlights flow.
///
/// The flow direction is always +y (downwards on screen), which suits waterfalls; a river
/// region gets the same downward motion.
pub struct Water {
    /// Picture pixels: x, y and their index into `ramp`.
    px: Vec<(i32, i32, usize)>,
    /// The region's distinct water colors, darkest first (sorted by luma, ties broken by
    /// the RGB channels).
    ramp: Vec<Rgb>,
    /// Distance between bright bands along the flow, in picture pixels (7 for every
    /// region).
    wavelength: f32,
    /// How fast the bands march, in bands per second (1.6, i.e. about 11 px/s with a
    /// 7 px wavelength).
    speed: f32,
}

impl Water {
    /// Repaint the water pixels at time `t`; `(ox, oy)` is the picture origin on screen.
    ///
    /// For each pixel the band phase is `y / wavelength - speed * t` (plus a per-column
    /// jitter of up to 0.35 band, from `(x * 73 + 11) % 7`, so bands are slightly ragged
    /// instead of ruled lines), taken mod 1. A triangle wave turns it into -1..1, and the
    /// pixel is shown as the ramp colour `rank + wave * amp`, rounded and clamped to the
    /// ramp. `amp` is a quarter of the ramp length (at least one step), so each pixel moves
    /// at most a quarter of the way up or down the region's brightness range: dark water
    /// stays darkish, foam stays bright, and crests of light run down through it. A ramp of
    /// fewer than two colours cannot move and is skipped.
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        let n = self.ramp.len();
        if n < 2 {
            return;
        }
        let amp = (n as f32 / 4.0).max(1.0);
        for &(x, y, rank) in &self.px {
            // A little per-column jitter keeps the bands from looking ruled.
            let jitter = ((x * 73 + 11) % 7) as f32 / 7.0 * 0.35;
            let phase = (y as f32 / self.wavelength - t * self.speed + jitter).rem_euclid(1.0);
            // Triangle wave in -1..1: bright crest, dark trough.
            let wave = 1.0 - 4.0 * (phase - 0.5).abs();
            let i = (rank as f32 + wave * amp)
                .round()
                .clamp(0.0, (n - 1) as f32) as usize;
            s.put(ox + x, oy + y, self.ramp[i]);
        }
    }
}

impl Picture {
    /// Parse the format written by `tools/cover.py`: u16 width, u16 height (LE), u8 palette
    /// size - 1, the palette as RGB, then one index per pixel.
    ///
    /// Returns `None` if the data is truncated, has trailing bytes, or uses a palette index
    /// beyond the palette. The result has no effects attached yet.
    pub fn parse(bytes: &[u8]) -> Option<Picture> {
        let w = u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]) as i32;
        let h = u16::from_le_bytes([*bytes.get(2)?, *bytes.get(3)?]) as i32;
        let n = *bytes.get(4)? as usize + 1;
        let pal = bytes.get(5..5 + n * 3)?;
        let px = bytes.get(5 + n * 3..)?;
        if px.len() != (w * h) as usize || px.iter().any(|&i| i as usize >= n) {
            return None;
        }
        Some(Picture {
            w,
            h,
            palette: pal.chunks(3).map(|c| Rgb(c[0], c[1], c[2])).collect(),
            px: px.to_vec(),
            water: Vec::new(),
            glow: Vec::new(),
            twinkle: Vec::new(),
            flash: Vec::new(),
            flutter: Vec::new(),
            fog: Vec::new(),
        })
    }

    /// Pixels inside a rectangle (picture pixels, inclusive-exclusive) that `pick` accepts.
    ///
    /// The rectangle is clipped to the picture. `pick` gets the position as well as the
    /// colour, so a predicate can exclude an area (the cover's bolt predicate skips a disc
    /// around the moon). Results come in row-major scan order.
    fn select(
        &self,
        (x0, y0, x1, y1): (i32, i32, i32, i32),
        pick: impl Fn(i32, i32, Rgb) -> bool,
    ) -> Vec<Px> {
        let mut out = Vec::new();
        for y in y0.max(0)..y1.min(self.h) {
            for x in x0.max(0)..x1.min(self.w) {
                let c = self.get(x, y);
                if pick(x, y, c) {
                    out.push((x, y, c));
                }
            }
        }
        out
    }

    /// Warm bright pixels in `rect` radiate pulses from `center`.
    ///
    /// "Warm and bright" means red above 170, green above 110, blue below green and luma
    /// above 140: the yellows and oranges of a painted sunburst or glowing cheese. The
    /// heroine's skin and hair can pass that test too, so keep `rect` off faces (the cover
    /// splits its glow into two boxes for that reason). `center` (picture pixels) is where
    /// the rings start; it does not need to lie inside `rect`. `amount` (0..1) is how far a
    /// pixel is pulled towards `YELLOW_LT` at a crest; 0.3 to 0.4 reads as a glow without
    /// flattening the painting. Several calls may share a centre; each adds its own set.
    pub fn with_glow(
        mut self,
        rect: (i32, i32, i32, i32),
        center: (i32, i32),
        amount: f32,
    ) -> Self {
        let warm = |_, _, c: Rgb| c.0 > 170 && c.1 > 110 && c.2 < c.1 && luma(c) > 140;
        let px = self
            .select(rect, warm)
            .into_iter()
            .map(|(x, y, c)| {
                let (dx, dy) = ((x - center.0) as f32, (y - center.1) as f32);
                (x, y, c, (dx * dx + dy * dy).sqrt())
            })
            .collect();
        self.glow.push(Glow { px, amount });
        self
    }

    /// Pixels in `rect` that `pick` accepts flicker; `cluster` px squares flicker together.
    ///
    /// The picture is divided into a grid of `cluster` x `cluster` cells (aligned to the
    /// picture origin, not to `rect`); every selected pixel in one cell shares a phase, so a
    /// small window flickers as one light. `cluster` below 1 is treated as 1 (every pixel on
    /// its own). `depth` is the darkest a flicker gets (0..1, see [`Twinkle`]) and `rate` its
    /// angular speed in radians per second: 5.0 for the cover's windows, 2.5 for the
    /// farewell's calmer lamps, 9.0 for a torch flame.
    pub fn with_twinkle(
        mut self,
        rect: (i32, i32, i32, i32),
        cluster: i32,
        depth: f32,
        rate: f32,
        pick: impl Fn(Rgb) -> bool,
    ) -> Self {
        let cl = cluster.max(1);
        let px = self
            .select(rect, |_, _, c| pick(c))
            .into_iter()
            .map(|(x, y, c)| (x, y, c, hash01(x / cl, y / cl)))
            .collect();
        self.twinkle.push(Twinkle { px, depth, rate });
        self
    }

    /// Misty pixels (pale, bluish, low saturation) inside `areas` drift like fog.
    ///
    /// A pixel counts as mist when blue is its strongest channel, its luma is in 70..220
    /// (not shadow, not highlight) and its channel spread (max minus min) is under 110.
    /// Night skies and snowy peaks can pass this test as well, so `areas` should hug the
    /// valleys. `strength` scales the effect; see [`Fog`] for why it differs per picture.
    pub fn with_fog(mut self, areas: &[(i32, i32, i32, i32)], strength: f32) -> Self {
        let mist = |_, _, c: Rgb| {
            let spread = c.0.max(c.1).max(c.2) - c.0.min(c.1).min(c.2);
            c.2 > c.0 && c.2 > c.1 && (70..220).contains(&luma(c)) && spread < 110
        };
        let px = areas.iter().flat_map(|&r| self.select(r, mist)).collect();
        self.fog.push(Fog { px, strength });
        self
    }

    /// Flag pixels that `pick` accepts flutter, inside tight boxes around each flag (a
    /// sunset sky is full of reds that must stay put).
    ///
    /// Each painting's flags are a different red against a different sky, so `pick` is
    /// written per picture: the cover and farewell accept red that dominates green twice
    /// over and blue by half (with a different minimum red), episode 2's deep crimson flags
    /// need a lower red threshold, and the onward screen's flags sit in front of a red
    /// sunset and need a strict "bright red, little green or blue" test. A box should
    /// contain one flag and as little background as possible; a few pixels of sky inside a
    /// box will ripple visibly.
    pub fn with_flutter(
        mut self,
        flags: &[(i32, i32, i32, i32)],
        pick: impl Fn(Rgb) -> bool,
    ) -> Self {
        let px = flags
            .iter()
            .flat_map(|&r| self.select(r, |_, _, c| pick(c)))
            .collect();
        self.flutter.push(Flutter { px });
        self
    }

    /// Pixels in `rect` that `pick` accepts light up with the lightning.
    ///
    /// `intensity` (0..1) scales the flash for this set, so a distant bolt can burn less
    /// brightly than the main one. Timing is shared by all flash sets (see [`Flash`]).
    pub fn with_flash(
        mut self,
        rect: (i32, i32, i32, i32),
        intensity: f32,
        pick: impl Fn(i32, i32, Rgb) -> bool,
    ) -> Self {
        let px = self.select(rect, pick);
        self.flash.push(Flash { px, intensity });
        self
    }

    /// Animate the water inside a rectangle (picture pixels, inclusive-exclusive): bluish
    /// or foam-white pixels at least `min_luma` bright. It flows downwards.
    ///
    /// A pixel is water when blue is at least as strong as red and green (bluish, or a
    /// neutral white/grey foam) and its luma is at least `min_luma`. The threshold keeps out
    /// the dark rock and shadow around a fall, which would otherwise join the ramp and drag
    /// the animation into black; each region's value (95 to 120) was found by eye. The
    /// distinct colours found become the region's brightness ramp, and each pixel stores its
    /// rank on it. Wavelength (7 px) and speed (1.6 bands/s) are the same for every region.
    pub fn with_water(mut self, (x0, y0, x1, y1): (i32, i32, i32, i32), min_luma: u32) -> Self {
        let luma = |c: Rgb| (c.0 as u32 * 3 + c.1 as u32 * 6 + c.2 as u32) / 10;
        let is_water = |c: Rgb| c.2 >= c.0 && c.2 >= c.1 && luma(c) >= min_luma;
        let mut found = Vec::new();
        for y in y0.max(0)..y1.min(self.h) {
            for x in x0.max(0)..x1.min(self.w) {
                let c = self.get(x, y);
                if is_water(c) {
                    found.push((x, y, c));
                }
            }
        }
        let mut ramp: Vec<Rgb> = found.iter().map(|p| p.2).collect();
        ramp.sort_by_key(|&c| (luma(c), c.0, c.1, c.2));
        ramp.dedup();
        let px = found
            .into_iter()
            .map(|(x, y, c)| {
                (
                    x,
                    y,
                    ramp.binary_search_by_key(&(luma(c), c.0, c.1, c.2), |&r| {
                        (luma(r), r.0, r.1, r.2)
                    })
                    .unwrap_or(0),
                )
            })
            .collect();
        self.water.push(Water {
            px,
            ramp,
            wavelength: 7.0,
            speed: 1.6,
        });
        self
    }

    /// The title: the cheese's sunburst radiates, castle windows twinkle, flags wave, mist
    /// drifts between the mountains and the lightning bolts blaze on every strike. (Stars
    /// twinkle on top, in `title.rs`.)
    ///
    /// Anchors on `art/cover.png` (picture pixels):
    /// - Glow: centred on the raised cheese at (198, 60), split into a wide box above the
    ///   heroine's head and a narrow one beside it, so her face is never selected.
    /// - Twinkle: the castle's lit windows under the moon, (260..292, 66..100), in 2 px
    ///   clusters, dimming up to 45% at 5 rad/s. `lit_window` is "warm yellow, little blue".
    /// - Flash: the bolts on the right of the sky, split into three boxes that follow them,
    ///   plus a weaker (0.8) one for the bolt in the top-left. `bolt` is "bright and bluish"
    ///   (blue more than 10 above red, luma above 140); the full moon, centred at (270, 63)
    ///   with a radius of about 17 px, would pass that test and is excluded by distance.
    /// - Fog: the valleys left (0..120, rows 112..175) and right (205..320, rows 112..178)
    ///   of the rock, at strength 1.9.
    /// - Flutter: the two small red flags on the castle towers, in tight boxes.
    pub fn cover() -> Picture {
        let lit_window = |c: Rgb| c.0 > 170 && c.1 > 120 && c.2 < 110;
        // Bolts are bright and bluish; the moon (centred at 270,63) is not a bolt.
        let bolt = |x: i32, y: i32, c: Rgb| {
            let (dx, dy) = (x - 270, y - 63);
            c.2 as u32 > c.0 as u32 + 10 && luma(c) > 140 && dx * dx + dy * dy > 17 * 17
        };
        Picture::parse(include_bytes!("cover.bin"))
            .expect("embedded cover parses")
            // Two boxes: wide above her head, narrow beside it (her face stays still).
            .with_glow((148, 38, 234, 70), (198, 60), 0.35)
            .with_glow((176, 70, 234, 82), (198, 60), 0.35)
            .with_twinkle((260, 66, 292, 100), 2, 0.45, 5.0, lit_window)
            .with_flash((230, 16, 288, 106), 1.0, bolt)
            .with_flash((286, 16, 320, 72), 1.0, bolt)
            .with_flash((264, 0, 320, 18), 1.0, bolt)
            .with_flash((44, 0, 112, 18), 0.8, bolt)
            .with_fog(&[(0, 112, 120, 175), (205, 112, 320, 178)], 1.9)
            .with_flutter(&[(279, 60, 290, 67), (249, 89, 258, 96)], |c| {
                c.0 > 140 && c.0 as u32 > 2 * c.1 as u32 && c.0 as u32 * 2 > 3 * c.2 as u32
            })
    }

    /// Episode 1: the sun's rays and the cheese radiate; the storm cloud flashes.
    ///
    /// Anchors on `art/intro-episode-1.png`: one glow centred on the setting sun at
    /// (177, 105), one on the cheese on its stump at (196, 150); the flash selects every
    /// pixel brighter than luma 110 in the cloud band at the top, (150..240, 0..40), at
    /// 0.7 intensity, following the title's lightning schedule.
    pub fn intro_episode_1() -> Picture {
        let cloud = |_, _, c: Rgb| luma(c) > 110;
        Picture::parse(include_bytes!("intro1.bin"))
            .expect("embedded intro parses")
            .with_glow((146, 62, 232, 136), (177, 105), 0.3)
            .with_glow((166, 120, 232, 168), (196, 150), 0.35)
            .with_flash((150, 0, 240, 40), 0.7, cloud)
    }

    /// The little waterfall under the castle flows and the castle's flags wave.
    ///
    /// Anchors on `art/intro-episode-2.png`: the waterfall below the castle hill at
    /// (188..200, 70..92), and seven tight boxes, one per pennant on the castle's towers.
    pub fn intro_episode_2() -> Picture {
        Picture::parse(include_bytes!("intro2.bin"))
            .expect("embedded intro parses")
            .with_water((188, 70, 200, 92), 120)
            .with_flutter(
                &[
                    (223, 17, 237, 25),
                    (212, 27, 219, 33),
                    (239, 32, 253, 41),
                    (187, 52, 197, 58),
                    (249, 49, 256, 53),
                    (261, 47, 267, 51),
                    (217, 41, 222, 46),
                ],
                // These flags are a deep crimson; the boxes are tight enough for a loose red.
                |c| c.0 > 110 && c.0 as u32 > 2 * c.1 as u32 && c.0 as u32 * 2 > 3 * c.2 as u32,
            )
    }

    /// The waterfall below the castle flows.
    ///
    /// Anchor on `art/intro-episode-3.png`: the waterfall below the castle, (143..163,
    /// 58..108), with a lower luma threshold (95) because this night scene's water is darker.
    pub fn intro_episode_3() -> Picture {
        Picture::parse(include_bytes!("intro3.bin"))
            .expect("embedded intro parses")
            .with_water((143, 58, 163, 108), 95)
    }

    /// Leaving: the castle's flag waves; its windows and the torchlit path up the mountain
    /// twinkle; mist drifts through the valley. (The painting's faint stars don't survive
    /// 256 colours; `farewell.rs` draws stars on top instead.)
    ///
    /// Anchors on `art/farewell.png`: the flag on the castle's top tower in the box
    /// (258..266, 72..77); the castle's windows (244..272, 78..102) and the lamps on the road
    /// winding up the mountain (240..284, 100..142), both "warm" (bright orange-yellow) in
    /// 2 px clusters, dimming only 20% at a slow 2.5 rad/s; the fog in the valley below the
    /// castle (165..320, 125..195) and at the far left (0..60, 110..170) at strength 1.0.
    /// `farewell.rs` reuses the road rectangle and the same `warm` test to place its lamp
    /// glints, so change them together.
    pub fn farewell() -> Picture {
        let warm = |c: Rgb| c.0 > 200 && c.1 > 140 && c.2 < 140;
        Picture::parse(include_bytes!("farewell.bin"))
            .expect("embedded farewell parses")
            .with_flutter(&[(258, 72, 266, 77)], |c| {
                c.0 > 120 && c.0 as u32 > 2 * c.1 as u32 && c.0 as u32 * 2 > 3 * c.2 as u32
            })
            .with_twinkle((244, 78, 272, 102), 2, 0.2, 2.5, warm)
            .with_twinkle((240, 100, 284, 142), 2, 0.2, 2.5, warm)
            .with_fog(&[(165, 125, 320, 195), (0, 110, 60, 170)], 1.0)
    }

    /// Caught: the cheese, just out of reach, glows.
    ///
    /// Anchor on `art/defeat.png`: the cheese on the dungeon floor, glow centred at
    /// (279, 137) inside (246..316, 104..160).
    pub fn defeat() -> Picture {
        Picture::parse(include_bytes!("defeat.bin"))
            .expect("embedded defeat parses")
            .with_glow((246, 104, 316, 160), (279, 137), 0.4)
    }

    /// After the ending, "Onward!": the torch flickers, the waterfall flows and the castle's
    /// flags wave.
    ///
    /// Anchors on `art/onward-episode-1.png`: the waterfall at the lower right
    /// (264..298, 136..178); the torch flame on the dungeon wall (60..76, 2..22), only its
    /// near-white core (luma above 200), flickering lightly (15%) but fast (9 rad/s); and
    /// four tight boxes around the red pennants of the distant castle. `onward.rs` draws a
    /// breathing glow centred on the same torch.
    pub fn onward_episode_1() -> Picture {
        // Only the flame's bright core: the torch-lit wall around it must stay still.
        let flame = |c: Rgb| luma(c) > 200 && c.2 < 200;
        Picture::parse(include_bytes!("onward.bin"))
            .expect("embedded onward parses")
            .with_water((264, 136, 298, 178), 110)
            .with_twinkle((60, 2, 76, 22), 2, 0.15, 9.0, flame)
            .with_flutter(
                &[
                    (282, 49, 292, 56),
                    (272, 58, 279, 63),
                    (289, 57, 297, 63),
                    (256, 64, 265, 70),
                ],
                // Bright flag red only: the sunset behind is red too.
                |c| c.0 > 180 && c.1 < 80 && c.2 < 90,
            )
    }

    /// The river in the valley flows.
    ///
    /// Anchor on `art/victory-episode-1.png`: a stretch of the river winding through the
    /// valley, (202..234, 72..108). Like all water here it flows towards +y.
    pub fn victory_episode_1() -> Picture {
        Picture::parse(include_bytes!("victory.bin"))
            .expect("embedded victory parses")
            .with_water((202, 72, 234, 108), 120)
    }

    /// The painted colour at picture pixel `(x, y)`; coordinates outside the picture are
    /// clamped to the nearest edge pixel.
    fn get(&self, x: i32, y: i32) -> Rgb {
        let (x, y) = (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1));
        self.palette[self.px[(y * self.w + x) as usize] as usize]
    }

    /// Distinct light points in `rect` (picture pixels) that `pick` accepts, at least
    /// `spacing` apart: one per lamp or window rather than every pixel of it.
    ///
    /// A greedy pass in scan order (top row first, left to right): a pixel is kept when its
    /// Manhattan distance to every already kept spot is at least `spacing`. Each spot is
    /// therefore the top-left-most accepted pixel of its light, not its centre. It scans the
    /// picture on every call; the farewell calls it once per frame over a small rectangle.
    pub fn spots(
        &self,
        rect: (i32, i32, i32, i32),
        spacing: i32,
        pick: impl Fn(Rgb) -> bool,
    ) -> Vec<(i32, i32)> {
        let mut out: Vec<(i32, i32)> = Vec::new();
        for (x, y, _) in self.select(rect, |_, _, c| pick(c)) {
            if out
                .iter()
                .all(|&(a, b)| (a - x).abs() + (b - y).abs() >= spacing)
            {
                out.push((x, y));
            }
        }
        out
    }

    /// Where the picture's top-left lands on screen (centered).
    ///
    /// Add this to a picture-pixel coordinate to get a screen pixel. It is `(0, 0)` for a
    /// full 320x200 painting, but overlays should still go through it so a narrower painting
    /// keeps working.
    pub fn origin(&self) -> (i32, i32) {
        ((W - self.w) / 2, (H - self.h) / 2)
    }

    /// Fill the screen with the picture (water flowing at time `t`). One narrower than the
    /// screen gets its edges smeared outwards, fading to black, so the margins feel like
    /// more of the same.
    ///
    /// The margin fill: a screen pixel `d` columns outside the picture samples the picture
    /// column `d / 3` in from that edge (a mirror image stretched three times), darkened by
    /// `0.88 * sqrt(d / margin)`, so it is nearly black at the screen edge. Rows outside the
    /// picture clamp to its edge row. Then every attached effect redraws its pixels for time
    /// `t`, in the order water, glow, twinkle, flash, flutter, fog. Overlays from the
    /// per-screen modules are drawn by the caller afterwards.
    pub fn draw(&self, s: &mut Screen, t: f32) {
        let (ox, oy) = self.origin();
        for y in 0..H {
            for x in 0..W {
                let (cx, cy) = (x - ox, y - oy);
                let c = if (0..self.w).contains(&cx) {
                    self.get(cx, cy)
                } else {
                    let edge = if cx < 0 { 0 } else { self.w - 1 };
                    let d = if cx < 0 { -cx } else { cx - edge };
                    let fade = (d as f32 / ox.max(1) as f32).min(1.0);
                    let src = self.get(edge - (d / 3) * cx.signum(), cy);
                    src.scale(1.0 - 0.88 * fade.sqrt())
                };
                s.put(x, y, c);
            }
        }
        for w in &self.water {
            w.draw(s, (ox, oy), t);
        }
        for g in &self.glow {
            g.draw(s, (ox, oy), t);
        }
        for tw in &self.twinkle {
            tw.draw(s, (ox, oy), t);
        }
        for f in &self.flash {
            f.draw(s, (ox, oy), t);
        }
        for f in &self.flutter {
            f.draw(s, (ox, oy), t);
        }
        for f in &self.fog {
            f.draw(s, (ox, oy), t);
        }
    }

    /// How many pixels are animated as water (for tests: a water rectangle that drifted off
    /// its waterfall after a repaint finds almost nothing).
    pub fn water_pixels(&self) -> usize {
        self.water.iter().map(|w| w.px.len()).sum()
    }

    /// How many pixels each kind of effect animates: glow, twinkle, flash (for tests).
    pub fn effect_pixels(&self) -> (usize, usize, usize) {
        (
            self.glow.iter().map(|g| g.px.len()).sum(),
            self.twinkle.iter().map(|g| g.px.len()).sum(),
            self.flash.iter().map(|g| g.px.len()).sum(),
        )
    }
}

/// A small four-pointed sparkle of radius `r`.
///
/// `(x, y)` is in *screen* pixels (callers add the picture origin). The centre is set to
/// pure white; each arm is `r` pixels long and blended towards white with an opacity that
/// falls off linearly from `r / (r + 1)` next to the centre to `1 / (r + 1)` at the tip, so
/// the arms fade out. This is the building block of most overlays below.
pub fn sparkle(s: &mut Screen, x: i32, y: i32, r: i32) {
    for d in 1..=r {
        let a = 1.0 - d as f32 / (r + 1) as f32;
        for (dx, dy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
            s.blend(x + dx, y + dy, pal::WHITE, a);
        }
    }
    s.put(x, y, pal::WHITE);
}

/// A quick gleam at a point once every `period` seconds (shifted by `offset`): a sparkle
/// that swells and shrinks within a quarter second.
///
/// `(x, y)` is in screen pixels; `period` and `offset` are in seconds. Within the first
/// 0.25 s of each period the sparkle radius follows a triangle from 0 up to 4 (at 0.125 s)
/// and back; the rest of the period draws nothing. Giving neighbouring points the same
/// period and slightly increasing offsets makes a glint run along an edge (the cat's claws
/// on episode 1's intro).
pub fn gleam(s: &mut Screen, (x, y): (i32, i32), t: f32, period: f32, offset: f32) {
    let c = (t - offset).rem_euclid(period);
    if c < 0.25 {
        let r = (3.5 * (1.0 - (c / 0.125 - 1.0).abs())).round() as i32;
        if r > 0 {
            sparkle(s, x, y, r);
        }
    }
}

/// A painted highlight brought alive (a sword's reflection): a star whose rays breathe,
/// with a diagonal cross that flashes out at the peak.
///
/// `(x, y)` is in screen pixels. Unlike [`gleam`] it never switches off:
/// `pulse = (0.5 + 0.5 sin(2.2 t))^2` (a cycle about every 2.9 s; squaring keeps it small
/// most of the time and makes the peak short) sets the sparkle radius between 2 and 7 px.
/// Above `pulse` 0.6 a diagonal cross of up to 3 px grows out of the centre, at most 80%
/// opaque and fading towards its tips.
pub fn flare(s: &mut Screen, (x, y): (i32, i32), t: f32) {
    let pulse = ((t * 2.2).sin() * 0.5 + 0.5).powi(2);
    let r = (2.0 + 5.0 * pulse).round() as i32;
    sparkle(s, x, y, r);
    if pulse > 0.6 {
        let d = ((pulse - 0.6) * 7.0).round() as i32;
        for k in 1..=d {
            let a = 0.8 * (1.0 - k as f32 / (d + 1) as f32);
            for (dx, dy) in [(k, k), (-k, k), (k, -k), (-k, -k)] {
                s.blend(x + dx, y + dy, pal::WHITE, a);
            }
        }
    }
}

/// Stable pseudo-random 0..1 per index (for desynchronising effects).
///
/// The classic shader hash `fract(sin(i * 12.9898 + salt * 78.233) * 43758.547)`. Different
/// `salt` values give independent-looking sequences for the same index, e.g. one for a
/// star's rate and one for its phase. Good enough for visuals, not for anything that needs
/// real randomness.
pub fn hash(i: usize, salt: f32) -> f32 {
    ((i as f32 * 12.9898 + salt * 78.233).sin() * 43_758.547)
        .fract()
        .abs()
}

/// A small, soft glint: a bright pixel with a faint one-pixel cross, at `strength` 0..1.
///
/// `(x, y)` is in screen pixels. The centre is blended towards white by `strength`, its four
/// neighbours by `0.45 * strength`. Much quieter than [`sparkle`], for distant lights.
pub fn soft_cross(s: &mut Screen, x: i32, y: i32, strength: f32) {
    s.blend(x, y, pal::WHITE, strength);
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        s.blend(x + dx, y + dy, pal::WHITE, strength * 0.45);
    }
}

/// Stars at fixed points (picture pixels), each twinkling on its own rhythm: a faint dot
/// that brightens, with a tiny cross at its brightest. Drawn on top, so nothing painted
/// around them (bolts, clouds) ever flickers along.
///
/// Why fixed points: the first attempt dimmed every bright pixel in the sky with a
/// [`Twinkle`] effect, but the lightning bolts next to the stars are bright too, and they
/// pulsed along like palette cycling. A hand-picked list of star positions avoids that, and
/// also lets a painting whose stars were lost in quantization (the farewell) get stars
/// back. `stars` are picture pixels; `(ox, oy)` is the picture origin.
///
/// Each star gets its own angular rate (0.8..2.4 rad/s) and phase from [`hash`] of its
/// index. `b = max(0, sin(rate * t + phase))`, so a star rests at a faint blend of 15%
/// towards `PALE` half the time and rises to 55% on the positive half of the sine; at the
/// very top (`b > 0.97`) a [`soft_cross`] at 0.4 adds a brief cross.
pub fn twinkle_stars(s: &mut Screen, (ox, oy): (i32, i32), stars: &[(i32, i32)], t: f32) {
    for (i, &(x, y)) in stars.iter().enumerate() {
        let (rate, phase) = (0.8 + hash(i, 1.0) * 1.6, hash(i, 2.0) * 6.3);
        let b = (t * rate + phase).sin().max(0.0);
        let (x, y) = (ox + x, oy + y);
        s.blend(x, y, pal::PALE, 0.15 + 0.4 * b);
        if b > 0.97 {
            soft_cross(s, x, y, 0.4);
        }
    }
}

/// Glints hopping between `spots` (picture pixels): one at a time, then a pause.
///
/// Time is cut into steps of `1 / rate` seconds. With `n` spots a cycle is `2 n` steps: in
/// step `k < n` spot `k` shows a radius-2 [`sparkle`], in the remaining `n` steps nothing is
/// drawn. So the glint walks across the spots in list order, then rests as long as it
/// walked. Used for cheese and treasure on most screens. `spots` must not be empty (the
/// cycle length would be zero).
pub fn glints(s: &mut Screen, pic: &Picture, spots: &[(i32, i32)], t: f32, rate: f32) {
    let (ox, oy) = pic.origin();
    let k = (t * rate) as usize % (spots.len() * 2);
    if let Some(&(gx, gy)) = spots.get(k) {
        sparkle(s, ox + gx, oy + gy, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_pictures_fit_the_screen() {
        for p in [
            Picture::cover(),
            Picture::intro_episode_1(),
            Picture::intro_episode_2(),
            Picture::intro_episode_3(),
            Picture::victory_episode_1(),
            Picture::defeat(),
            Picture::onward_episode_1(),
            Picture::farewell(),
        ] {
            assert_eq!(p.h, H);
            assert!(p.w <= W);
        }
    }

    #[test]
    fn water_regions_find_water() {
        for p in [
            Picture::intro_episode_2(),
            Picture::intro_episode_3(),
            Picture::victory_episode_1(),
        ] {
            assert!(
                p.water_pixels() > 40,
                "only {} water pixels",
                p.water_pixels()
            );
        }
        assert_eq!(Picture::cover().water_pixels(), 0);
    }

    #[test]
    fn effects_find_their_pixels() {
        let (glow, twinkle, flash) = Picture::cover().effect_pixels();
        assert!(
            glow > 200 && twinkle > 10 && flash > 30,
            "{glow} {twinkle} {flash}"
        );
        let (glow, _, flash) = Picture::intro_episode_1().effect_pixels();
        assert!(glow > 200 && flash > 30, "{glow} {flash}");
        let flags: usize = Picture::onward_episode_1()
            .flutter
            .iter()
            .map(|f| f.px.len())
            .sum();
        assert!(
            (12..=40).contains(&flags),
            "{flags} flag pixels: flags only, no sky"
        );
        let flags: usize = Picture::intro_episode_2()
            .flutter
            .iter()
            .map(|f| f.px.len())
            .sum();
        assert!((60..=130).contains(&flags), "{flags} episode 2 flag pixels");
    }

    #[test]
    fn title_has_fog() {
        let fog: usize = Picture::cover().fog.iter().map(|f| f.px.len()).sum();
        assert!((800..=4000).contains(&fog), "{fog} fog pixels");
    }

    #[test]
    fn farewell_valley_has_fog() {
        let fog: usize = Picture::farewell().fog.iter().map(|f| f.px.len()).sum();
        assert!((500..=3000).contains(&fog), "{fog} fog pixels");
    }

    #[test]
    fn farewell_road_has_lamps() {
        let warm = |c: Rgb| c.0 > 200 && c.1 > 140 && c.2 < 140;
        let lamps = Picture::farewell().spots((240, 100, 284, 142), 4, warm);
        assert!((5..=40).contains(&lamps.len()), "{} lamps", lamps.len());
    }

    #[test]
    fn water_moves() {
        let p = Picture::intro_episode_3();
        let (mut a, mut b) = (Screen::new(), Screen::new());
        p.draw(&mut a, 0.0);
        p.draw(&mut b, 0.3);
        assert_ne!(a.px, b.px);
    }

    #[test]
    fn rejects_truncated_data() {
        assert!(Picture::parse(&[]).is_none());
        assert!(Picture::parse(&[2, 0, 2, 0, 0, 1, 2, 3, 0, 0, 0]).is_none());
        assert!(Picture::parse(&[2, 0, 1, 0, 0, 1, 2, 3, 0, 0]).is_some());
    }
}
