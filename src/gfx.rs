//! Pixel graphics: a fixed-size RGB framebuffer and the primitives the renderer draws with.
//!
//! The whole game is drawn on the CPU into one [`Screen`]: a 320x200 array of [`Rgb`]
//! pixels, the resolution of the old VGA mode 13h. Once per frame the game converts it to
//! RGBA bytes ([`Screen::to_rgba`]) and uploads it as a single texture, which the toolkit
//! draws with integer scaling so every framebuffer pixel stays a crisp square. Nothing in
//! here depends on the toolkit, which is why the headless snapshot path (see [`png`]) can
//! render exactly the same frames.
//!
//! Coordinates are framebuffer pixels: `(0, 0)` is the top-left corner, x grows to the
//! right and y grows downwards. Every primitive clips against the screen edges, so callers
//! may draw partly (or wholly) off screen without checks. Pixels are written opaquely
//! unless a method says it blends; blending is a straight linear mix in sRGB space via
//! [`Rgb::lerp`], with no alpha channel stored in the framebuffer itself.
//!
//! Submodules: [`palette`] (the fixed color set and the ASCII-art color key), [`sprite`]
//! (sprites parsed from ASCII art), [`font`] (two bitmap fonts) and [`png`] (a tiny PNG
//! encoder for snapshots).

pub mod font;
pub mod palette;
pub mod png;
pub mod sprite;

pub use palette::Rgb;
pub use sprite::Sprite;

/// Framebuffer width in pixels (the classic 320x200 VGA mode 13h resolution).
pub const W: i32 = 320;
/// Framebuffer height in pixels. Together with [`W`] this gives the 16:10 picture that the
/// window scales up by a whole number.
pub const H: i32 = 200;

/// A 320x200 RGB framebuffer, the single canvas every screen of the game is drawn into.
///
/// Pixels are stored row-major: the pixel at `(x, y)` lives at index `y * W + x`. There is no
/// alpha channel; transparency only exists while drawing (sprites skip transparent pixels,
/// `blend*` methods mix colors in).
#[derive(Clone)]
pub struct Screen {
    /// The `W * H` pixels in row-major order, top row first.
    pub px: Vec<Rgb>,
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    /// A black screen.
    pub fn new() -> Self {
        Self {
            px: vec![Rgb(0, 0, 0); (W * H) as usize],
        }
    }

    /// Fill the whole screen with `c`.
    pub fn clear(&mut self, c: Rgb) {
        self.px.fill(c);
    }

    /// Overwrite this screen with the pixels of `other`. The defeat card draws itself into a
    /// separate screen and copies it over the play screen this way once it is fully faded in
    /// (and uses [`Screen::mix_from`] while it is still fading).
    pub fn copy_from(&mut self, other: &Screen) {
        self.px.copy_from_slice(&other.px);
    }

    /// Mix `other` over this screen with opacity `a` (clamped to 0..=1), for cross-fades.
    ///
    /// Each pixel becomes `self + (other - self) * a`, per channel, so `a = 0` keeps this screen
    /// and `a = 1` copies `other`.
    pub fn mix_from(&mut self, other: &Screen, a: f32) {
        for (p, o) in self.px.iter_mut().zip(&other.px) {
            *p = p.lerp(*o, a);
        }
    }

    /// Set the pixel at `x, y` to `c`. Coordinates outside the screen are silently ignored,
    /// which is what makes every other primitive clip for free.
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.px[(y * W + x) as usize] = c;
        }
    }

    /// Read the pixel at `x, y`. Outside the screen this returns black rather than
    /// panicking.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.px[(y * W + x) as usize]
        } else {
            Rgb(0, 0, 0)
        }
    }

    /// Mix `c` over the pixel at `x, y` with opacity `a` (clamped to 0..=1). Off-screen
    /// coordinates are ignored, like [`Screen::put`].
    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            let i = (y * W + x) as usize;
            self.px[i] = self.px[i].lerp(c, a);
        }
    }

    /// Fill the `w` x `h` rectangle whose top-left corner is `x, y` with `c`.
    ///
    /// The rectangle is intersected with the screen up front, so the inner loop writes straight
    /// into the pixel buffer without per-pixel bounds checks. A zero or negative size draws
    /// nothing.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                self.px[(yy * W + xx) as usize] = c;
            }
        }
    }

    /// Mix `c` over the rectangle with opacity `a` (0..=1). Used for drop shadows, wall
    /// shadows on the floor and the dimming overlays behind dialogs.
    pub fn blend_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb, a: f32) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                let i = (yy * W + xx) as usize;
                self.px[i] = self.px[i].lerp(c, a);
            }
        }
    }

    /// Checkerboard dither of two colors, the 16-color-era way to fake a third shade.
    ///
    /// The choice is made on absolute screen coordinates (`(x + y)` even gives `a`, odd gives
    /// `b`), so neighbouring dithered rectangles line up seamlessly.
    pub fn dither_rect(&mut self, x: i32, y: i32, w: i32, h: i32, a: Rgb, b: Rgb) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                let c = if (xx + yy) & 1 == 0 { a } else { b };
                self.px[(yy * W + xx) as usize] = c;
            }
        }
    }

    /// Vertical gradient from `top` to `bottom`, quantised to `steps` flat bands (at least 2)
    /// and ordered-dithered between them, so it reads like a limited palette rather than a
    /// smooth modern ramp.
    ///
    /// How it works: for each row, `t` is the row's position in the gradient scaled to
    /// `0..steps-1`, so its integer part is the band and its fraction is how far the row is
    /// towards the next band. Each pixel adds a threshold from a 4x4 Bayer matrix (the values
    /// 0..15 divided by 16, picked by the pixel's absolute `x & 3, y & 3`) before flooring. A row
    /// that is, say, 25% of the way to the next band therefore shows the next band's color on
    /// the 4 of 16 matrix cells whose threshold is at least 0.75, in the evenly spread Bayer
    /// pattern; at 50% it is a checkerboard. Band `k` is then the color `top.lerp(bottom, k /
    /// (steps - 1))`, so the first and last band are exactly `top` and `bottom`.
    pub fn gradient_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        top: Rgb,
        bottom: Rgb,
        steps: u32,
    ) {
        /// The classic 4x4 Bayer (ordered dither) matrix: each value 0..=15 appears once, laid
        /// out so that any threshold level picks pixels spread as evenly as possible.
        const BAYER: [[f32; 4]; 4] = [
            [0.0, 8.0, 2.0, 10.0],
            [12.0, 4.0, 14.0, 6.0],
            [3.0, 11.0, 1.0, 9.0],
            [15.0, 7.0, 13.0, 5.0],
        ];
        let steps = steps.max(2) as f32;
        for yy in y.max(0)..(y + h).min(H) {
            let t = (yy - y) as f32 / (h - 1).max(1) as f32 * (steps - 1.0);
            for xx in x.max(0)..(x + w).min(W) {
                let th = BAYER[(yy & 3) as usize][(xx & 3) as usize] / 16.0;
                let band = (t + th).floor().min(steps - 1.0) / (steps - 1.0);
                self.px[(yy * W + xx) as usize] = top.lerp(bottom, band);
            }
        }
    }

    /// Horizontal line from `x0` to `x1` inclusive (in either order) on row `y`.
    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb) {
        for x in x0.min(x1)..=x0.max(x1) {
            self.put(x, y, c);
        }
    }

    /// Vertical line from `y0` to `y1` inclusive (in either order) in column `x`.
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb) {
        for y in y0.min(y1)..=y0.max(y1) {
            self.put(x, y, c);
        }
    }

    /// A 1px rectangle outline whose outer edge covers exactly the `w` x `h` area at `x, y`.
    pub fn rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        self.hline(x, x + w - 1, y, c);
        self.hline(x, x + w - 1, y + h - 1, c);
        self.vline(x, y, y + h - 1, c);
        self.vline(x + w - 1, y, y + h - 1, c);
    }

    /// A 3D bevelled panel in the style of early-90s GUIs: `fill` inside, a 1px `light` edge
    /// along the top and left, and a 1px `dark` edge along the bottom and right, which makes the
    /// panel look raised under a light from the top-left. Swapping `light` and `dark` gives a
    /// sunken look. The dark edges are drawn last, so they win the top-right and bottom-left
    /// corner pixels.
    pub fn bevel(&mut self, x: i32, y: i32, w: i32, h: i32, light: Rgb, fill: Rgb, dark: Rgb) {
        self.fill_rect(x, y, w, h, fill);
        self.hline(x, x + w - 1, y, light);
        self.vline(x, y, y + h - 1, light);
        self.hline(x, x + w - 1, y + h - 1, dark);
        self.vline(x + w - 1, y, y + h - 1, dark);
    }

    /// A 1px line from `x0, y0` to `x1, y1`, both ends included (Bresenham's algorithm).
    ///
    /// This is the all-octant integer form: `dx` is the positive x distance and `dy` the
    /// *negative* y distance, and `err = dx + dy` tracks how far the ideal line is from the
    /// current pixel. Each step looks at `e2 = 2 * err`: if `e2 >= dy` the line has moved far
    /// enough along x to step x, and if `e2 <= dx` it has moved far enough along y to step y
    /// (both happen on diagonal steps). Only additions and comparisons are needed, and the
    /// result has no gaps.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Filled circle of radius `r` around `cx, cy`.
    ///
    /// Every pixel in the bounding square is tested with `dx^2 + dy^2 <= r^2 + r/2`. The extra
    /// `r/2` (rather than a plain `r^2`) rounds the edge outwards a little, which avoids the
    /// single-pixel nubs at the four extreme points that a strict test leaves on small circles.
    /// `r = 0` draws one pixel.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r + r / 2 {
                    self.put(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// 1px circle outline of radius `r` around `cx, cy`: the pixels that pass the
    /// [`Screen::fill_circle`] test for radius `r` but not for radius `r - 1`, so it matches the
    /// filled circle's edge exactly.
    pub fn circle_outline(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        let outer = r * r + r / 2;
        let inner = (r - 1) * (r - 1) + (r - 1) / 2;
        for dy in -r..=r {
            for dx in -r..=r {
                let d = dx * dx + dy * dy;
                if d <= outer && d > inner {
                    self.put(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// Rounded box with a 1px outline, used for speech bubbles and dialogs.
    ///
    /// The straight outline edges stop two pixels short of each corner and a single diagonal
    /// pixel at `(1, 1)` in from each corner joins them, which gives a corner with a radius of
    /// about two pixels. The three pixels outside that diagonal at each corner are left
    /// untouched, so the background shows through there.
    pub fn round_box(&mut self, x: i32, y: i32, w: i32, h: i32, fill: Rgb, outline: Rgb) {
        self.fill_rect(x + 1, y + 1, w - 2, h - 2, fill);
        self.hline(x + 2, x + w - 3, y, outline);
        self.hline(x + 2, x + w - 3, y + h - 1, outline);
        self.vline(x, y + 2, y + h - 3, outline);
        self.vline(x + w - 1, y + 2, y + h - 3, outline);
        self.put(x + 1, y + 1, outline);
        self.put(x + w - 2, y + 1, outline);
        self.put(x + 1, y + h - 2, outline);
        self.put(x + w - 2, y + h - 2, outline);
    }

    /// Draw sprite `s` with its top-left corner at `x, y`. Transparent sprite pixels leave the
    /// screen untouched.
    pub fn blit(&mut self, s: &Sprite, x: i32, y: i32) {
        self.blit_ex(s, x, y, false, None);
    }

    /// Draw sprite `s` mirrored left-to-right with its top-left corner at `x, y`.
    pub fn blit_flipped(&mut self, s: &Sprite, x: i32, y: i32) {
        self.blit_ex(s, x, y, true, None);
    }

    /// Draw `s` at `x, y`, optionally mirrored, optionally as a flat-color silhouette.
    ///
    /// With `flip`, destination column `sx` reads source column `w - 1 - sx`, so the sprite is
    /// mirrored within its own box and stays at the same position. With `solid`, every opaque
    /// pixel is drawn in that one color instead of its own; this is how
    /// [`Screen::blit_outline`] builds an outline.
    pub fn blit_ex(&mut self, s: &Sprite, x: i32, y: i32, flip: bool, solid: Option<Rgb>) {
        for sy in 0..s.h {
            for sx in 0..s.w {
                let src_x = if flip { s.w - 1 - sx } else { sx };
                if let Some(c) = s.px[(sy * s.w + src_x) as usize] {
                    self.put(x + sx, y + sy, solid.unwrap_or(c));
                }
            }
        }
    }

    /// Draw `s` at `x, y`, cutting off every row at or below screen row `clip_y` (a cheese
    /// sinking into a hole).
    ///
    /// Only the first `clip_y - y` sprite rows are drawn, so as the caller moves the sprite down
    /// while keeping `clip_y` fixed, the sprite disappears from the bottom up, as if it were
    /// sliding behind the edge of the hole. If `clip_y <= y` nothing is drawn. There is no flip
    /// option.
    pub fn blit_clipped(&mut self, s: &Sprite, x: i32, y: i32, clip_y: i32) {
        for sy in 0..s.h.min(clip_y - y) {
            for sx in 0..s.w {
                if let Some(c) = s.px[(sy * s.w + sx) as usize] {
                    self.put(x + sx, y + sy, c);
                }
            }
        }
    }

    /// Draw `s` blown up by an integer `scale` (chunky pixels), optionally mirrored. Each sprite
    /// pixel becomes a `scale` x `scale` block, with the sprite's top-left at `x, y`.
    pub fn blit_scaled(&mut self, s: &Sprite, x: i32, y: i32, scale: i32, flip: bool) {
        for sy in 0..s.h {
            for sx in 0..s.w {
                let src_x = if flip { s.w - 1 - sx } else { sx };
                if let Some(c) = s.px[(sy * s.w + src_x) as usize] {
                    self.fill_rect(x + sx * scale, y + sy * scale, scale, scale, c);
                }
            }
        }
    }

    /// Filled polygon (even-odd scanline fill). Kept as a general primitive; none of the
    /// current screens call it.
    ///
    /// `pts` are the vertices in order; the last connects back to the first. For each screen row
    /// between the lowest and highest vertex, the row is sampled through its pixel centres
    /// (`y + 0.5`), and every edge that crosses that height contributes one x intersection. The
    /// half-pixel offset means an edge touching the sample line exactly at a vertex is counted
    /// once, not twice, and horizontal edges never count. The intersections are sorted and
    /// filled pairwise (first to second, third to fourth, ...), which is the even-odd rule:
    /// self-intersecting or concave outlines work, and overlapping parts of a self-intersecting
    /// polygon come out as holes. Spans are rounded to whole pixels and the right end is
    /// exclusive.
    pub fn fill_polygon(&mut self, pts: &[(i32, i32)], c: Rgb) {
        let Some(y0) = pts.iter().map(|p| p.1).min() else {
            return;
        };
        let y1 = pts.iter().map(|p| p.1).max().unwrap_or(y0);
        let mut xs = Vec::new();
        for y in y0.max(0)..=y1.min(H - 1) {
            let fy = y as f32 + 0.5;
            xs.clear();
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                let (ay, by) = (a.1 as f32, b.1 as f32);
                if (ay <= fy) != (by <= fy) {
                    xs.push(a.0 as f32 + (fy - ay) / (by - ay) * (b.0 - a.0) as f32);
                }
            }
            xs.sort_by(f32::total_cmp);
            for pair in xs.as_chunks::<2>().0 {
                for x in pair[0].round() as i32..pair[1].round() as i32 {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// A thick line with round caps: a filled circle of radius `r` (so `2r + 1` pixels wide)
    /// stamped at every integer step along the line from `x0, y0` to `x1, y1`.
    ///
    /// The step count is the longer axis distance, and the points are spaced by integer
    /// division, so consecutive circles always overlap and leave no gaps. Simple rather than
    /// fast, which is fine for short strokes.
    pub fn thick_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, r: i32, c: Rgb) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=n {
            let x = x0 + (x1 - x0) * i / n;
            let y = y0 + (y1 - y0) * i / n;
            self.fill_circle(x, y, r, c);
        }
    }

    /// Filled ellipse centered at `cx, cy` with radii `rx, ry`.
    ///
    /// Each pixel in the bounding box is normalised by the radii and kept when
    /// `(dx/rx)^2 + (dy/ry)^2 <= 1.05`; the 5% slack plays the same role as the `r/2` in
    /// [`Screen::fill_circle`] and keeps the outline from looking pinched.
    pub fn fill_ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: Rgb) {
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let (fx, fy) = (dx as f32 / rx.max(1) as f32, dy as f32 / ry.max(1) as f32);
                if fx * fx + fy * fy <= 1.05 {
                    self.put(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// Draw a 1px outline around a sprite's silhouette. Kept as a general primitive; none of
    /// the current screens call it.
    ///
    /// It stamps the sprite as a flat `c` silhouette four times, shifted one pixel left, right,
    /// up and down. Draw the sprite itself on top afterwards; only the ring around it remains
    /// visible. Diagonal neighbours are not covered, so outer corners stay notched.
    pub fn blit_outline(&mut self, s: &Sprite, x: i32, y: i32, c: Rgb) {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            self.blit_ex(s, x + dx, y + dy, false, Some(c));
        }
    }

    /// Mix every pixel towards `c` by `a` (0..=1), in place. Used for flashes and fades; see
    /// also [`Screen::to_rgba`], which can apply the same tint on the way out without touching
    /// the framebuffer.
    pub fn tint(&mut self, c: Rgb, a: f32) {
        for p in &mut self.px {
            *p = p.lerp(c, a);
        }
    }

    /// Convert to RGBA8 for upload, applying an optional full-screen tint flash.
    ///
    /// `out` must hold at least `W * H * 4` bytes; pixels are written in the same row-major
    /// order, with alpha always 255. `flash` is `(color, amount)` and mixes every output pixel
    /// towards that color like [`Screen::tint`], but only in the output, so the framebuffer
    /// itself (which may be reused next frame) is left as drawn.
    pub fn to_rgba(&self, out: &mut [u8], flash: Option<(Rgb, f32)>) {
        for (o, p) in out.as_chunks_mut::<4>().0.iter_mut().zip(&self.px) {
            let c = match flash {
                Some((f, a)) => p.lerp(f, a),
                None => *p,
            };
            o[0] = c.0;
            o[1] = c.1;
            o[2] = c.2;
            o[3] = 255;
        }
    }
}
