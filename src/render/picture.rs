//! Full-screen paintings (the title cover, the episode ending), reduced to 256 colors like
//! a VGA game's pictures. `tools/cover.py` converts a PNG from `art/` into the tiny
//! palette-indexed format parsed here, which is embedded with `include_bytes!`.

use crate::gfx::palette::{self as pal, Rgb};
use crate::gfx::{H, Screen, W};

pub struct Picture {
    pub w: i32,
    pub h: i32,
    palette: Vec<Rgb>,
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
pub struct Fog {
    px: Vec<Px>,
}

impl Fog {
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        use std::f32::consts::TAU;
        for &(x, y, c) in &self.px {
            let sway = (y as f32 * 0.3 + t * 0.6).sin() * 1.1;
            let wisp = 0.5 + 0.5 * (TAU * (x as f32 / 34.0 - t * 0.17) + sway).sin();
            let lit = c
                .lerp(pal::PALE, 0.45 * wisp * wisp)
                .scale(0.82 + 0.18 * wisp);
            s.put(ox + x, oy + y, lit);
        }
    }
}

/// Flags in the wind: light and shade ripple along the cloth.
pub struct Flutter {
    px: Vec<Px>,
}

impl Flutter {
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        for &(x, y, c) in &self.px {
            let wave = (t * 9.0 - x as f32 * 1.3 + y as f32 * 0.5).sin();
            s.put(ox + x, oy + y, c.scale(0.8 + 0.35 * wave));
        }
    }
}

/// A picture pixel selected for an effect: position and its painted color.
type Px = (i32, i32, Rgb);

/// Warm, bright pixels pulse brighter in rings travelling outwards from a center, so rays
/// seem to radiate.
pub struct Glow {
    /// Pixels and their distance from the center.
    px: Vec<(i32, i32, Rgb, f32)>,
    amount: f32,
}

impl Glow {
    fn draw(&self, s: &mut Screen, (ox, oy): (i32, i32), t: f32) {
        for &(x, y, c, d) in &self.px {
            let pulse = 0.5 + 0.5 * (std::f32::consts::TAU * (t * 0.8 - d / 22.0)).sin();
            s.put(ox + x, oy + y, c.lerp(pal::YELLOW_LT, self.amount * pulse));
        }
    }
}

/// Points that flicker independently; `cluster` groups neighbors (a whole window).
pub struct Twinkle {
    px: Vec<(i32, i32, Rgb, f32)>,
    /// How dark a flicker gets (0 = not at all, 1 = black).
    depth: f32,
    rate: f32,
}

impl Twinkle {
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
pub struct Flash {
    px: Vec<Px>,
    intensity: f32,
}

impl Flash {
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

/// Luma, 0..255.
fn luma(c: Rgb) -> u32 {
    (c.0 as u32 * 3 + c.1 as u32 * 6 + c.2 as u32) / 10
}

/// Cheap stable hash of a position into 0..1.
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
pub struct Water {
    /// Picture pixels: x, y and their index into `ramp`.
    px: Vec<(i32, i32, usize)>,
    /// The region's water colors, darkest first.
    ramp: Vec<Rgb>,
    /// Bands per pixel along the flow, and bands per second.
    wavelength: f32,
    speed: f32,
}

impl Water {
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
    pub fn with_fog(mut self, areas: &[(i32, i32, i32, i32)]) -> Self {
        let mist = |_, _, c: Rgb| {
            let spread = c.0.max(c.1).max(c.2) - c.0.min(c.1).min(c.2);
            c.2 > c.0 && c.2 > c.1 && (70..220).contains(&luma(c)) && spread < 110
        };
        let px = areas.iter().flat_map(|&r| self.select(r, mist)).collect();
        self.fog.push(Fog { px });
        self
    }

    /// Flag pixels that `pick` accepts flutter, inside tight boxes around each flag (a
    /// sunset sky is full of reds that must stay put).
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

    /// The title: the cheese's sunburst radiates, stars and castle windows twinkle, and the
    /// lightning bolts blaze on every strike.
    pub fn cover() -> Picture {
        let bright = |c: Rgb| luma(c) > 175 && c.2 as u32 + 10 >= c.0 as u32;
        let lit_window = |c: Rgb| c.0 > 170 && c.1 > 120 && c.2 < 110;
        let bolt = |x: i32, y: i32, c: Rgb| {
            let (dx, dy) = (x - 276, y - 61);
            c.2 as u32 > c.0 as u32 + 10 && luma(c) > 140 && dx * dx + dy * dy > 18 * 18
        };
        Picture::parse(include_bytes!("cover.bin"))
            .expect("embedded cover parses")
            .with_glow((166, 42, 250, 100), (200, 57), 0.35)
            .with_twinkle((0, 0, 64, 18), 1, 0.85, 2.3, bright)
            .with_twinkle((290, 0, 320, 48), 1, 0.85, 2.0, bright)
            .with_twinkle((268, 70, 300, 102), 2, 0.45, 5.0, lit_window)
            .with_flash((232, 18, 288, 106), 1.0, bolt)
            .with_flash((266, 0, 320, 26), 1.0, bolt)
            .with_flash((44, 0, 92, 12), 0.8, bolt)
    }

    /// Episode 1: the sun's rays and the cheese radiate; the storm cloud flashes.
    pub fn intro_episode_1() -> Picture {
        let cloud = |_, _, c: Rgb| luma(c) > 110;
        Picture::parse(include_bytes!("intro1.bin"))
            .expect("embedded intro parses")
            .with_glow((146, 62, 232, 136), (177, 105), 0.3)
            .with_glow((166, 120, 232, 168), (196, 150), 0.35)
            .with_flash((150, 0, 240, 40), 0.7, cloud)
    }

    /// The little waterfall under the castle flows and the castle's flags wave.
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
    pub fn intro_episode_3() -> Picture {
        Picture::parse(include_bytes!("intro3.bin"))
            .expect("embedded intro parses")
            .with_water((143, 58, 163, 108), 95)
    }

    /// Leaving: the castle's flag waves; its windows and the torchlit path up the mountain
    /// twinkle; mist drifts through the valley. (The painting's faint stars don't survive 256 colours.)
    pub fn farewell() -> Picture {
        let warm = |c: Rgb| c.0 > 200 && c.1 > 140 && c.2 < 140;
        Picture::parse(include_bytes!("farewell.bin"))
            .expect("embedded farewell parses")
            .with_flutter(&[(258, 72, 266, 77)], |c| {
                c.0 > 120 && c.0 as u32 > 2 * c.1 as u32 && c.0 as u32 * 2 > 3 * c.2 as u32
            })
            .with_twinkle((244, 78, 272, 102), 2, 0.45, 5.0, warm)
            .with_twinkle((240, 100, 284, 142), 2, 0.5, 4.0, warm)
            .with_fog(&[(165, 125, 320, 195), (0, 110, 60, 170)])
    }

    /// Caught: the cheese, just out of reach, glows.
    pub fn defeat() -> Picture {
        Picture::parse(include_bytes!("defeat.bin"))
            .expect("embedded defeat parses")
            .with_glow((246, 104, 316, 160), (279, 137), 0.4)
    }

    /// After the ending, "Onward!": the torch flickers, the waterfall flows and the castle's
    /// flags wave.
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
    pub fn victory_episode_1() -> Picture {
        Picture::parse(include_bytes!("victory.bin"))
            .expect("embedded victory parses")
            .with_water((202, 72, 234, 108), 120)
    }

    fn get(&self, x: i32, y: i32) -> Rgb {
        let (x, y) = (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1));
        self.palette[self.px[(y * self.w + x) as usize] as usize]
    }

    /// Distinct light points in `rect` (picture pixels) that `pick` accepts, at least
    /// `spacing` apart: one per lamp or window rather than every pixel of it.
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
    pub fn origin(&self) -> (i32, i32) {
        ((W - self.w) / 2, (H - self.h) / 2)
    }

    /// Fill the screen with the picture (water flowing at time `t`). One narrower than the
    /// screen gets its edges smeared outwards, fading to black, so the margins feel like
    /// more of the same.
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

    /// How many pixels are animated as water (for tests).
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

/// Glints hopping between `spots` (picture pixels): one at a time, then a pause.
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
