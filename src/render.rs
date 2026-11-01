//! Draws every screen into the 320x200 framebuffer. No toolkit types: the snapshot example
//! renders the same way headlessly.

pub mod anim;
pub mod art;
pub mod defeat;
pub mod ending;
pub mod fx;
pub mod intro;
pub mod picture;
pub mod title;

use std::sync::Arc;

use crate::gfx::font::{self, Font};
use crate::gfx::palette::{self as pal, Rgb};
use crate::gfx::{H, Screen, W};
use crate::logic::{CheeseKind, Dir, Level, Pos, Status, World, stars};
use anim::{Anim, dist, lerp};
use art::Art;
use fx::Fx;

pub const TILE: i32 = 16;
/// Height of the status bar at the top.
pub const HUD_H: i32 = 12;
/// Height of the key-help bar at the bottom.
pub const FOOT_H: i32 = 12;
/// The board area: 20x11 tiles.
pub const MAX_COLS: i32 = W / TILE;
pub const MAX_ROWS: i32 = (H - HUD_H - FOOT_H) / TILE;

/// Seconds after solving before the "Delicious!" panel shows.
pub const SOLVED_DELAY: f32 = 0.45;
/// How far a cheese sinks into its mouse hole, in pixels.
const SINK: i32 = 12;

pub struct PlayView<'a> {
    pub world: &'a World,
    pub anim: Option<&'a Anim>,
    pub fx: &'a Fx,
    /// 1-based level number.
    pub number: usize,
    pub best: Option<u32>,
    /// Seconds since launch, for blinking.
    pub t: f32,
    /// Seconds since the level was solved or the mouse caught.
    pub end_t: f32,
    pub paused: bool,
    pub trails: &'a [Trail],
    /// Seconds the mouse has stood still (she twitches her ears when idle).
    pub idle: f32,
    /// Seconds since her last idle squeak, while it's on screen.
    pub squeak_age: Option<f32>,
}

/// A faint streak where a brie slid. Purely cosmetic.
#[derive(Clone, Copy, Debug)]
pub struct Trail {
    pub from: Pos,
    pub to: Pos,
    /// The move count after the slide: undoing past it wipes the trail.
    pub turn: u32,
    /// When it was made (seconds since launch), so it grows with the slide.
    pub born: f32,
}

pub struct SelectView<'a> {
    pub levels: &'a [Arc<Level>],
    pub best: &'a [Option<u32>],
    pub cursor: usize,
    /// Levels `0..unlocked` can be played.
    pub unlocked: usize,
    /// Showing the (pretend) order form for the full version.
    pub order: bool,
    /// Testing mode (`--cheese`): advertise its shortcuts.
    pub cheese: bool,
    pub t: f32,
}

/// Top-left pixel of the board, centered in the play area.
pub fn board_origin(level: &Level) -> (i32, i32) {
    (
        (W - level.w * TILE) / 2,
        HUD_H + (MAX_ROWS * TILE - level.h * TILE) / 2,
    )
}

/// The tile under a framebuffer pixel, for taps.
pub fn tile_at(level: &Level, x: i32, y: i32) -> Pos {
    let (ox, oy) = board_origin(level);
    Pos::new((x - ox).div_euclid(TILE), (y - oy).div_euclid(TILE))
}

const SELECT_COLS: usize = 4;
const BOX_W: i32 = 60;
const BOX_H: i32 = 28;
const GAP_X: i32 = 8;
const GAP_Y: i32 = 4;
const SELECT_Y: i32 = 26;
const EP_Y: i32 = 124;
const EP_H: i32 = 14;

/// The episodes "only in the registered version": (title, level count).
pub const LOCKED_EPISODES: [(&str, u32); 2] =
    [("THE GOUDA, THE BAD & THE UGLY", 18), ("FONDUE OR DIE", 22)];

fn grid_x0() -> i32 {
    (W - (SELECT_COLS as i32 * (BOX_W + GAP_X) - GAP_X)) / 2
}

fn select_box(i: usize) -> (i32, i32) {
    let (col, row) = ((i % SELECT_COLS) as i32, (i / SELECT_COLS) as i32);
    (
        grid_x0() + col * (BOX_W + GAP_X),
        SELECT_Y + row * (BOX_H + GAP_Y),
    )
}

/// The banner of locked episode `k` (0 = episode two).
fn episode_box(k: usize) -> (i32, i32, i32, i32) {
    let w = SELECT_COLS as i32 * (BOX_W + GAP_X) - GAP_X;
    (grid_x0(), EP_Y + k as i32 * (EP_H + 3), w, EP_H)
}

/// Which slot a framebuffer pixel is on: levels are `0..count`, then the locked episodes.
pub fn select_hit(count: usize, x: i32, y: i32) -> Option<usize> {
    let level = (0..count).find(|&i| {
        let (bx, by) = select_box(i);
        (bx..bx + BOX_W).contains(&x) && (by..by + BOX_H).contains(&y)
    });
    level.or_else(|| {
        (0..LOCKED_EPISODES.len())
            .find(|&k| {
                let (bx, by, bw, bh) = episode_box(k);
                (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y)
            })
            .map(|k| count + k)
    })
}

/// Move the select cursor: around the level grid, and down into the locked episodes.
pub fn select_move(count: usize, cursor: usize, dir: Dir) -> usize {
    let cols = SELECT_COLS;
    let last_row = (count - 1) / cols;
    if cursor >= count {
        let k = cursor - count;
        return match dir {
            Dir::Up if k == 0 => (last_row * cols).min(count - 1),
            Dir::Up => cursor - 1,
            Dir::Down if k + 1 < LOCKED_EPISODES.len() => cursor + 1,
            _ => cursor,
        };
    }
    match dir {
        Dir::Left if !cursor.is_multiple_of(cols) => cursor - 1,
        Dir::Right if cursor % cols + 1 < cols && cursor + 1 < count => cursor + 1,
        Dir::Up if cursor >= cols => cursor - cols,
        Dir::Down if cursor + cols < count => cursor + cols,
        Dir::Down if cursor / cols == last_row => count,
        // Below a gap in the last row: drop to the last level.
        Dir::Down if cursor / cols + 1 == last_row => count - 1,
        _ => cursor,
    }
}

pub struct Renderer {
    art: Art,
    cover: picture::Picture,
    /// Episode intro cards, episode 1 first.
    intros: [picture::Picture; intro::EPISODES],
    victory: picture::Picture,
    defeat: picture::Picture,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            art: Art::new(),
            cover: picture::Picture::cover(),
            intros: [
                picture::Picture::intro_episode_1(),
                picture::Picture::intro_episode_2(),
                picture::Picture::intro_episode_3(),
            ],
            victory: picture::Picture::victory_episode_1(),
            defeat: picture::Picture::defeat(),
        }
    }

    fn backdrop(&self, s: &mut Screen) {
        s.clear(pal::NAVY);
        for y in 0..H {
            for x in 0..W {
                if (x + y) % 4 == 0 && (x / 2 + y) % 8 < 2 {
                    s.put(x, y, pal::NIGHT);
                }
            }
        }
    }

    // ---- Play ------------------------------------------------------------------------

    pub fn draw_play(&self, s: &mut Screen, v: &PlayView) {
        let w = v.world;
        let level = &*w.level;
        let state = &w.state;
        let (ox, oy) = board_origin(level);
        let px = |x: f32, y: f32| {
            (
                ox + (x * TILE as f32).round() as i32,
                oy + (y * TILE as f32).round() as i32,
            )
        };

        self.backdrop(s);

        // Floor, walls, holes.
        for y in 0..level.h {
            for x in 0..level.w {
                let p = Pos::new(x, y);
                let (sx, sy) = (ox + x * TILE, oy + y * TILE);
                if level.inside(p) {
                    s.blit(&self.art.floor[((x * 7 + y * 3) % 2) as usize], sx, sy);
                    // Walls cast a short shadow onto the floor below and to the right.
                    if level.wall(Pos::new(x, y - 1)) {
                        s.blend_rect(sx, sy, TILE, 3, pal::INK, 0.45);
                    }
                    if level.wall(Pos::new(x - 1, y)) {
                        s.blend_rect(sx, sy, 2, TILE, pal::INK, 0.35);
                    }
                    if level.is_hole(p) {
                        let sp = match state.cheese_at(p).map(|i| state.cheeses[i].kind) {
                            Some(CheeseKind::Wedge) => &self.art.hole_full[0],
                            Some(CheeseKind::Wheel) => &self.art.hole_full[1],
                            Some(CheeseKind::Brie) => &self.art.hole_full[2],
                            None => &self.art.hole,
                        };
                        s.blit(sp, sx, sy);
                    }
                } else if level.wall(p) && touches_inside(level, p) {
                    s.blit(&self.art.wall, sx, sy);
                }
            }
        }

        for tr in v.trails {
            let tiles = dist(tr.from, tr.to);
            let grow = ((v.t - tr.born) / anim::duration(tiles as f32)).clamp(0.0, 1.0);
            let ease = 1.0 - (1.0 - grow) * (1.0 - grow);
            let (ax, ay) = (
                ox + tr.from.x * TILE + TILE / 2,
                oy + tr.from.y * TILE + TILE / 2 + 3,
            );
            let (ex, ey) = lerp(tr.from, tr.to, ease);
            let (bx, by) = (
                ox + (ex * TILE as f32) as i32 + TILE / 2,
                oy + (ey * TILE as f32) as i32 + TILE / 2 + 3,
            );
            let horizontal = tr.from.y == tr.to.y;
            for k in -4i32..=4 {
                let a = if k.abs() == 4 {
                    0.10
                } else if k.abs() >= 2 {
                    0.16
                } else {
                    0.08
                };
                let (dx, dy) = if horizontal { (0, k) } else { (k, 0) };
                let (x0, x1) = (ax.min(bx) + dx, ax.max(bx) + dx);
                let (y0, y1) = (ay.min(by) + dy, ay.max(by) + dy);
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if (x + y * 3) % 7 != 0 {
                            s.blend(x, y, pal::CREAM, a);
                        }
                    }
                }
            }
        }

        let (from, walk) = match v.anim {
            Some(a) => (&a.from, Some(a)),
            None => (state, None),
        };

        // Where each cat will go next: faint paw prints.
        if state.status == Status::Playing && walk.is_none_or(Anim::done) {
            for (i, next) in w.cat_intents().into_iter().enumerate() {
                if let Some(n) = next {
                    let (x, y) = (ox + n.x * TILE, oy + n.y * TILE);
                    let flip = state.cats[i].dir == Dir::Left;
                    let (a, b) = if flip { (8, 3) } else { (3, 8) };
                    s.blit_ex(&self.art.paw, x + a, y + 3, false, Some(pal::BROWN));
                    s.blit_ex(&self.art.paw, x + b, y + 8, false, Some(pal::BROWN));
                }
            }
        }

        // Cheese. One that ends its push on a hole sinks into it as it arrives (and one
        // pushed out rises again), so it reads as going *in*.
        for (i, c) in state.cheeses.iter().enumerate() {
            let a = from.cheeses.get(i).map_or(c.pos, |f| f.pos);
            let u = walk.map_or(1.0, |an| an.progress(dist(a, c.pos)));
            let (fx, fy) = lerp(a, c.pos, u);
            let (x, y) = px(fx, fy);
            let sprite = match c.kind {
                CheeseKind::Wedge => &self.art.wedge,
                CheeseKind::Wheel => &self.art.wheel,
                CheeseKind::Brie => &self.art.brie,
            };
            let (in_from, in_to) = (level.is_hole(a), level.is_hole(c.pos));
            let depth = match (in_from, in_to) {
                (true, true) => 1.0,
                (false, true) => ((u - 0.5) * 2.0).clamp(0.0, 1.0),
                (true, false) => (1.0 - u * 2.0).clamp(0.0, 1.0),
                (false, false) => 0.0,
            };
            if depth > 0.0 {
                // Only clip against the hole the cheese is over.
                let hole = if in_to && u > 0.5 { c.pos } else { a };
                let clip = oy + hole.y * TILE + 13;
                let sink = (depth * SINK as f32).round() as i32;
                s.blit_clipped(sprite, x, y + sink, clip);
            } else {
                s.blit(sprite, x, y);
            }
            if depth >= 1.0 {
                // Snug in its hole: an occasional glint and a floating heart.
                let phase = (v.t * 0.8 + i as f32 * 0.37).fract();
                if phase < 0.15 {
                    let gx = x + 4 + (i as i32 * 5) % 8;
                    let gy = y + 6;
                    s.put(gx, gy, pal::WHITE);
                    s.put(gx - 1, gy + 1, pal::YELLOW_LT);
                    s.put(gx + 1, gy + 1, pal::YELLOW_LT);
                    s.put(gx, gy + 2, pal::WHITE);
                } else if (0.5..0.8).contains(&phase) {
                    let rise = ((phase - 0.5) * 20.0) as i32;
                    font::draw(s, Font::Tiny, x + 6, y - rise + 2, "^", pal::PINK);
                }
            }
        }

        // Cats.
        let caught = state.status == Status::Caught;
        for (i, cat) in state.cats.iter().enumerate() {
            let a = from.cats.get(i).map_or(cat.pos, |f| f.pos);
            let u = walk.map_or(1.0, Anim::walk);
            let (fx, fy) = lerp(a, cat.pos, u);
            let (x, y) = px(fx, fy);
            let left = facing_left(cat.dir, from.cats.get(i).map(|c| c.dir));
            if caught && cat.pos == state.mouse && walk.is_none_or(Anim::done) {
                continue; // Drawn pouncing below.
            }
            let frame = usize::from(u > 0.3 && u < 1.0);
            let sprite = &self.art.cat_right[frame];
            s.blit_ex(sprite, x, y, left, None);
        }

        // The mouse.
        let u = walk.map_or(1.0, Anim::walk);
        let (fx, fy) = lerp(from.mouse, state.mouse, u);
        let (x, y) = px(fx, fy);
        if caught && walk.is_none_or(Anim::done) {
            s.blit(&self.art.mouse_caught, x, y);
            if let Some(cat) = state.cats.iter().find(|c| c.pos == state.mouse) {
                s.blit_ex(&self.art.cat_pounce, x, y - 4, cat.dir == Dir::Left, None);
            } else if let Some(c) = state.cats.iter().find(|c| dist(c.pos, state.mouse) <= 1) {
                // Walked into a cat that then stepped off: it still gets the pounce.
                let (cx, cy) = (ox + c.pos.x * TILE, oy + c.pos.y * TILE);
                s.blit_ex(&self.art.cat_pounce, cx, cy, c.pos.x > state.mouse.x, None);
            }
        } else {
            let moving = u > 0.0 && u < 1.0;
            let frame = usize::from(moving && u > 0.4);
            let bob = i32::from(moving && u < 0.6);
            // Ear twitches: now and then while idle, and rapidly while squeaking.
            let squeaking = v
                .squeak_age
                .is_some_and(|a| a < 0.45 && (a * 14.0) as i32 % 2 == 0);
            let idle_flick = v.idle > 0.6 && (v.t % 2.9) < 0.13;
            let twitch = !moving && (squeaking || idle_flick);
            let art = &self.art;
            let (sprite, flip) = match state.facing {
                Dir::Right | Dir::Left if twitch => {
                    (&art.mouse_twitch[0], state.facing == Dir::Left)
                }
                Dir::Down if twitch => (&art.mouse_twitch[1], false),
                Dir::Up if twitch => (&art.mouse_twitch[2], false),
                Dir::Right => (&art.mouse_right[frame], false),
                Dir::Left => (&art.mouse_right[frame], true),
                Dir::Down => (&art.mouse_down[frame], false),
                Dir::Up => (&art.mouse_up[frame], false),
            };
            s.blit_ex(sprite, x, y - bob, flip, None);
            if let Some(a) = v.squeak_age.filter(|&a| a < 0.9) {
                let msg = "SQUEAK!";
                let rise = (a * 10.0) as i32;
                let tx = x + TILE / 2 - Font::Tiny.measure(msg, 1) / 2;
                font::draw_outlined(
                    s,
                    Font::Tiny,
                    tx,
                    y - 7 - rise,
                    msg,
                    1,
                    &|_| pal::WHITE,
                    pal::INK,
                );
            }
        }

        v.fx.draw(s);
        self.hud(s, v);

        if let Some(hint) = &level.hint
            && state.moves == 0
            && state.status == Status::Playing
        {
            hint_bubble(s, hint);
        }

        match state.status {
            Status::Solved if v.end_t >= SOLVED_DELAY => self.solved_panel(s, v),
            Status::Caught => defeat::draw(s, &self.defeat, v.end_t, v.t),
            _ => {}
        }
        if v.paused {
            s.blend_rect(0, 0, W, H, pal::NAVY, 0.6);
            panel(s, 90, 76, 140, 44);
            font::draw_centered_shadow(s, Font::Normal, W / 2, 84, "PAUSED", pal::YELLOW, pal::INK);
            font::draw_centered(s, Font::Tiny, W / 2, 98, "P TO RESUME", pal::PALE);
            font::draw_centered(
                s,
                Font::Tiny,
                W / 2,
                106,
                "ESC FOR THE LEVEL MENU",
                pal::PALE,
            );
        }
    }

    fn hud(&self, s: &mut Screen, v: &PlayView) {
        let st = &v.world.state;
        let level = &v.world.level;
        s.bevel(0, 0, W, HUD_H, pal::STEEL, pal::SLATE, pal::NIGHT);
        let label = format!("{:02} {}", v.number, level.title);
        font::draw_shadow(s, Font::Normal, 4, 3, &label, pal::YELLOW, pal::INK);
        let filled = level
            .holes
            .iter()
            .filter(|&&h| st.cheese_at(h).is_some())
            .count();
        let right = format!(
            "HOLES {filled}/{}  CRUMBS {}  PAR {}",
            level.holes.len(),
            st.moves,
            level.par
        );
        let rw = Font::Normal.measure(&right, 1);
        let over = st.moves > level.par;
        font::draw_shadow(
            s,
            Font::Normal,
            W - 4 - rw,
            3,
            &right,
            if over { pal::PINK } else { pal::PALE },
            pal::INK,
        );

        let fy = H - FOOT_H;
        s.bevel(0, fy, W, FOOT_H, pal::SLATE, pal::NIGHT, pal::NAVY);
        let help = "ARROWS MOVE  Z UNDO  R RESTART  P PAUSE  ESC LEVELS  M MUTE";
        font::draw_centered(s, Font::Tiny, W / 2, fy + 4, help, pal::STEEL_LT);
        if let Some(b) = v.best {
            let txt = format!("BEST {b}");
            font::draw(
                s,
                Font::Tiny,
                W - 4 - Font::Tiny.measure(&txt, 1),
                fy - 7,
                &txt,
                pal::STEEL_LT,
            );
        }
    }

    fn solved_panel(&self, s: &mut Screen, v: &PlayView) {
        let st = &v.world.state;
        let level = &v.world.level;
        let (x, y, w, h) = (70, 54, 180, 88);
        panel(s, x, y, w, h);
        let pulse = ((v.t * 6.0).sin() * 0.5 + 0.5) * 0.4;
        font::draw_outlined(
            s,
            Font::Normal,
            W / 2 - Font::Normal.measure("DELICIOUS!", 2) / 2,
            y + 8,
            "DELICIOUS!",
            2,
            &|row| pal::YELLOW_LT.lerp(pal::CHEESE_DK, row as f32 / 6.0 - pulse * 0.2),
            pal::INK,
        );
        let n = stars(st.moves, level.par);
        let shown = (((v.end_t - SOLVED_DELAY) / 0.25) as i32).clamp(0, 3);
        for i in 0..3 {
            let sx = W / 2 - 22 + i * 15;
            let filled = i < n as i32 && i < shown;
            let sp = if filled {
                &self.art.star
            } else {
                &self.art.star_empty
            };
            s.blit(sp, sx, y + 30);
        }
        let line = format!("{} CRUMBS  PAR {}", st.moves, level.par);
        font::draw_centered(s, Font::Normal, W / 2, y + 46, &line, pal::PALE);
        let verdict = match n {
            3 => "NOT A CRUMB WASTED!",
            2 => "TASTY. A FEW CRUMBS LEFT.",
            _ => "MESSY, BUT IT'S IN.",
        };
        font::draw_centered(s, Font::Tiny, W / 2, y + 58, verdict, pal::STEEL_LT);
        if blink(v.t) {
            font::draw_centered(
                s,
                Font::Tiny,
                W / 2,
                y + 72,
                "SPACE: NEXT   Z: UNDO",
                pal::YELLOW,
            );
        }
    }

    // ---- Level select --------------------------------------------------------------

    pub fn draw_select(&self, s: &mut Screen, v: &SelectView) {
        self.backdrop(s);
        let n = v.levels.len();
        s.bevel(0, 0, W, 21, pal::STEEL, pal::SLATE, pal::NIGHT);
        font::draw_shadow(s, Font::Normal, 6, 3, "EPISODE ONE:", pal::PALE, pal::INK);
        font::draw_shadow(
            s,
            Font::Normal,
            6,
            12,
            "A HOLE NEW WORLD",
            pal::YELLOW,
            pal::INK,
        );
        // The shareware stamp.
        let (sx, sy) = (W - 84, 3);
        s.rect_outline(sx, sy, 78, 15, pal::RED);
        s.rect_outline(sx + 1, sy + 1, 76, 13, pal::RED);
        font::draw(s, Font::Normal, sx + 6, sy + 4, "SHAREWARE", pal::PINK);

        for (i, level) in v.levels.iter().enumerate() {
            let (x, y) = select_box(i);
            let locked = i >= v.unlocked;
            let sel = i == v.cursor;
            let (light, fill, dark) = if sel {
                (pal::YELLOW_LT, pal::CHEESE_DK, pal::BROWN)
            } else if locked {
                (pal::SLATE, pal::NIGHT, pal::NAVY)
            } else {
                (pal::STEEL_LT, pal::STEEL, pal::SLATE)
            };
            s.bevel(x, y, BOX_W, BOX_H, light, fill, dark);
            if sel {
                s.rect_outline(x - 1, y - 1, BOX_W + 2, BOX_H + 2, pal::INK);
            }
            if locked {
                s.blit(&self.art.lock, x + BOX_W / 2 - 4, y + 4);
            } else {
                let num = format!("{}", i + 1);
                font::draw_outlined(
                    s,
                    Font::Normal,
                    x + BOX_W / 2 - Font::Normal.measure(&num, 2) / 2,
                    y + 3,
                    &num,
                    2,
                    &|_| pal::WHITE,
                    pal::INK,
                );
            }
            let earned = v.best[i].map_or(0, |b| stars(b, level.par));
            for k in 0..3 {
                let sp = if k < earned {
                    &self.art.star
                } else {
                    &self.art.star_empty
                };
                s.blit(sp, x + BOX_W / 2 - 15 + k as i32 * 10, y + 18);
            }
        }
        // Empty grid slots: a tease.
        for i in n..n.div_ceil(SELECT_COLS) * SELECT_COLS {
            let (x, y) = select_box(i);
            s.dither_rect(x, y, BOX_W, BOX_H, pal::NAVY, pal::NIGHT);
            s.rect_outline(x, y, BOX_W, BOX_H, pal::SLATE);
            font::draw_centered(
                s,
                Font::Tiny,
                x + BOX_W / 2,
                y + 8,
                "MORE IN THE",
                pal::STEEL_LT,
            );
            font::draw_centered(
                s,
                Font::Tiny,
                x + BOX_W / 2,
                y + 15,
                "FULL VERSION",
                pal::STEEL_LT,
            );
        }
        for (k, (name, count)) in LOCKED_EPISODES.iter().enumerate() {
            let (x, y, w, h) = episode_box(k);
            let sel = v.cursor == n + k;
            if sel {
                s.bevel(x, y, w, h, pal::PINK, pal::RED_DK, pal::INK);
            } else {
                s.bevel(x, y, w, h, pal::SLATE, pal::NIGHT, pal::NAVY);
            }
            s.blit(&self.art.lock, x + 3, y + 2);
            let label = format!("{}. {name}", k + 2);
            font::draw(
                s,
                Font::Normal,
                x + 16,
                y + 4,
                &label,
                if sel { pal::WHITE } else { pal::STEEL_LT },
            );
            let c = format!("{count} LEVELS");
            font::draw(
                s,
                Font::Tiny,
                x + w - 4 - Font::Tiny.measure(&c, 1),
                y + 5,
                &c,
                pal::PINK,
            );
        }

        // The cursor's level name and record, or the sales pitch.
        let y = H - 38;
        s.bevel(0, y, W, 38, pal::SLATE, pal::NIGHT, pal::NAVY);
        let (name, rec) = if v.cursor >= n {
            (
                "REGISTERED VERSION ONLY!".to_string(),
                "PRESS SPACE TO ORDER - OPERATORS ARE STANDING BY".to_string(),
            )
        } else {
            let level = &v.levels[v.cursor];
            let name = if v.cursor < v.unlocked {
                level.title.to_uppercase()
            } else {
                "LOCKED - SOLVE THE ONE BEFORE".into()
            };
            let rec = match v.best[v.cursor] {
                Some(b) => format!("BEST {b} CRUMBS  PAR {}", level.par),
                None => format!("NOT SOLVED YET  PAR {}", level.par),
            };
            (name, rec)
        };
        font::draw_centered_shadow(s, Font::Normal, W / 2, y + 6, &name, pal::YELLOW, pal::INK);
        font::draw_centered(s, Font::Tiny, W / 2, y + 18, &rec, pal::PALE);
        let (help, color) = if v.cheese {
            (
                "ARROWS PICK  SPACE PLAY  ESC TITLE  -  CHEESE MODE: I INTRO  E ENDING",
                pal::PINK,
            )
        } else {
            ("ARROWS PICK  SPACE PLAY  ESC TITLE", pal::STEEL_LT)
        };
        font::draw_centered(s, Font::Tiny, W / 2, y + 28, help, color);

        if v.order {
            order_form(s, v.t);
        }
    }

    // ---- Title and ending --------------------------------------------------------------

    pub fn draw_title(&self, s: &mut Screen, t: f32) {
        title::draw(s, &self.cover, t);
    }

    /// The card before an episode's first level (`episode` is 1-based).
    pub fn draw_intro(&self, s: &mut Screen, episode: usize, t: f32) {
        let i = episode.clamp(1, intro::EPISODES);
        intro::draw(s, &self.intros[i - 1], i, t);
    }

    /// The end of episode one.
    pub fn draw_end(&self, s: &mut Screen, earned: u32, max: u32, t: f32) {
        ending::draw(s, &self.victory, earned, max, t);
    }
}

/// A screen-centered toast near the bottom.
pub fn toast(s: &mut Screen, msg: &str) {
    let w = Font::Normal.measure(msg, 1) + 12;
    let (x, y) = (W / 2 - w / 2, H - FOOT_H - 18);
    s.round_box(x, y, w, 13, pal::NIGHT, pal::STEEL_LT);
    font::draw_centered(s, Font::Normal, W / 2, y + 3, msg, pal::YELLOW);
}

/// A hint in a speech bubble at the top of the board area, word-wrapped.
fn hint_bubble(s: &mut Screen, text: &str) {
    let max = (W - 24) / Font::Normal.advance();
    let mut lines: Vec<String> = Vec::new();
    for word in text.to_uppercase().split_whitespace() {
        match lines.last_mut() {
            Some(l) if (l.len() + 1 + word.len()) as i32 <= max => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    let w = lines
        .iter()
        .map(|l| Font::Normal.measure(l, 1))
        .max()
        .unwrap_or(0)
        + 14;
    let h = lines.len() as i32 * 10 + 7;
    let (x, y) = (W / 2 - w / 2, H - FOOT_H - h - 4);
    s.blend_rect(x + 2, y + 2, w, h, pal::INK, 0.5);
    s.round_box(x, y, w, h, pal::CREAM, pal::INK);
    for (i, l) in lines.iter().enumerate() {
        font::draw_centered(
            s,
            Font::Normal,
            W / 2,
            y + 4 + i as i32 * 10,
            l,
            pal::BROWN_DK,
        );
    }
}

/// The (pretend) order form for the registered version.
fn order_form(s: &mut Screen, t: f32) {
    s.blend_rect(0, 0, W, H, pal::NAVY, 0.7);
    let (x, y, w, h) = (14, 14, W - 28, H - 28);
    s.blend_rect(x + 4, y + 4, w, h, pal::INK, 0.6);
    s.fill_rect(x, y, w, h, pal::CREAM);
    s.rect_outline(x, y, w, h, pal::INK);
    s.rect_outline(x + 3, y + 3, w - 6, h - 6, pal::RIND_DK);
    let cx = W / 2;
    font::draw_outlined(
        s,
        Font::Normal,
        cx - Font::Normal.measure("ORDER NOW!", 2) / 2,
        y + 8,
        "ORDER NOW!",
        2,
        &|row| pal::RED.lerp(pal::RED_DK, row as f32 / 6.0),
        pal::INK,
    );
    let ink = pal::BROWN_DK;
    let lines: [(&str, Rgb); 7] = [
        ("GET THE COMPLETE CHEESED! TRILOGY:", ink),
        ("EPISODE 2: THE GOUDA, THE BAD & THE UGLY", pal::RED_DK),
        ("EPISODE 3: FONDUE OR DIE", pal::RED_DK),
        ("* 40 NEW LEVELS * THE SWISS ROLL (IT ROLLS!)", ink),
        ("* A BOSS FIGHT WITH A VERY LARGE CAT", ink),
        ("* EXCLUSIVE HINT BOOK: \"PUSH IT\"", ink),
        ("ONLY $14.95 + $3.00 SHIPPING & HANDLING", pal::RED),
    ];
    for (i, (l, c)) in lines.iter().enumerate() {
        font::draw_centered(s, Font::Normal, cx, y + 30 + i as i32 * 11, l, *c);
    }
    let fine = [
        "SEND CHECK OR MONEY ORDER TO: CHEDDARWARE,",
        "P.O. BOX 1337, BRIE-SUR-MER. ALLOW 6-8 WEEKS FOR DELIVERY.",
        "NO CATS WERE HARMED. SEVERAL WERE STARTLED.",
    ];
    for (i, l) in fine.iter().enumerate() {
        font::draw_centered(s, Font::Tiny, cx, y + 112 + i as i32 * 8, l, pal::RIND_DK);
    }
    if blink(t) {
        font::draw_centered(
            s,
            Font::Tiny,
            cx,
            y + h - 12,
            "PRESS ANY KEY TO KEEP DREAMING",
            pal::RED,
        );
    }
}

fn panel(s: &mut Screen, x: i32, y: i32, w: i32, h: i32) {
    s.blend_rect(x + 3, y + 3, w, h, pal::INK, 0.5);
    s.round_box(x, y, w, h, pal::NIGHT, pal::YELLOW);
    s.rect_outline(x + 2, y + 2, w - 4, h - 4, pal::SLATE);
}

fn blink(t: f32) -> bool {
    (t * 2.0) as i32 % 3 != 0
}

/// Whether a wall tile borders the inside (walls deep in the void aren't drawn).
fn touches_inside(level: &Level, p: Pos) -> bool {
    (-1..=1).any(|dy| (-1..=1).any(|dx| level.inside(Pos::new(p.x + dx, p.y + dy))))
}

/// Cats face the way they walk; walking up or down they keep their last sideways look.
fn facing_left(dir: Dir, prev: Option<Dir>) -> bool {
    match dir {
        Dir::Left => true,
        Dir::Right => false,
        _ => prev == Some(Dir::Left),
    }
}

/// The colors of a cheese, for crumb bursts.
pub fn cheese_colors(kind: CheeseKind) -> &'static [Rgb] {
    match kind {
        CheeseKind::Wedge => &[pal::YELLOW, pal::YELLOW_LT, pal::CHEESE_DK],
        CheeseKind::Wheel => &[pal::RED, pal::PINK, pal::YELLOW],
        CheeseKind::Brie => &[pal::CREAM, pal::RIND, pal::YELLOW_LT],
    }
}

/// Pixel center of a tile.
pub fn tile_center(level: &Level, p: Pos) -> (f32, f32) {
    let (ox, oy) = board_origin(level);
    (
        (ox + p.x * TILE + TILE / 2) as f32,
        (oy + p.y * TILE + TILE / 2) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_cursor_walks_the_grid_and_into_the_episodes() {
        let n = 11; // rows 0..3, last row has 3 levels
        assert_eq!(select_move(n, 0, Dir::Right), 1);
        assert_eq!(select_move(n, 3, Dir::Right), 3, "edge of the row");
        assert_eq!(select_move(n, 0, Dir::Down), 4);
        assert_eq!(
            select_move(n, 7, Dir::Down),
            10,
            "under the gap: last level"
        );
        assert_eq!(select_move(n, 9, Dir::Down), n, "into episode two");
        assert_eq!(select_move(n, n, Dir::Down), n + 1);
        assert_eq!(select_move(n, n + 1, Dir::Down), n + 1);
        assert_eq!(select_move(n, n + 1, Dir::Up), n);
        assert_eq!(select_move(n, n, Dir::Up), 8);
        assert_eq!(select_move(n, 10, Dir::Right), 10);
    }

    #[test]
    fn select_hit_finds_boxes_and_banners() {
        let (x, y) = select_box(5);
        assert_eq!(select_hit(11, x + 2, y + 2), Some(5));
        let (x, y, ..) = episode_box(1);
        assert_eq!(select_hit(11, x + 20, y + 3), Some(12));
        assert_eq!(select_hit(11, 0, 0), None);
    }

    #[test]
    fn levels_fit_the_board() {
        for l in crate::logic::levels() {
            assert!(
                l.w <= MAX_COLS && l.h <= MAX_ROWS,
                "{} is {}x{}",
                l.title,
                l.w,
                l.h
            );
        }
    }
}
