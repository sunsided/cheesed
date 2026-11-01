//! Draws every screen into the 320x200 framebuffer. No toolkit types: the snapshot example
//! renders the same way headlessly.
//!
//! The renderer is a pure function of what it is shown: the app builds a view struct
//! ([`PlayView`], [`SelectView`]) or passes a few numbers, and a [`Renderer`] method paints
//! a complete frame into a [`Screen`], from the backdrop up. Nothing is cached between
//! frames and nothing here mutates game state; time-based effects (blinking, bobbing,
//! sinking cheese) are computed from the timestamps in the view.
//!
//! Three coordinate systems meet here:
//!
//! - **Framebuffer pixels**: `0..320` x `0..200`, top-left origin. Everything is finally
//!   drawn in these.
//! - **Tiles**: level cells ([`Pos`]), each [`TILE`] = 16 pixels square. The board is
//!   centered in the space between the HUD bar at the top and the key-help bar at the
//!   bottom, so tile `(x, y)` has its top-left pixel at
//!   `board_origin + (x * 16, y * 16)` ([`board_origin`], [`tile_at`], [`tile_center`]).
//!   While a turn animates, positions are fractional tiles (see [`anim::lerp`]) and are
//!   rounded to pixels at the end.
//! - **Picture pixels**: the painted full-screen images in [`picture`] have their own
//!   resolution (200 rows tall, possibly wider than 320) and are mapped onto the
//!   framebuffer by the screens in the submodules.
//!
//! The play screen and the level select live in this file. The painted screens (title,
//! episode intros, defeat card, ending, "Onward!" and "Farewell!") each have a submodule
//! with a `draw` function; [`Renderer`] owns their pictures and forwards to them. Sprite
//! art is in [`art`], turn animation timing in [`anim`] and particles in [`fx`].

pub mod anim;
pub mod art;
pub mod defeat;
pub mod ending;
pub mod farewell;
pub mod fx;
pub mod intro;
pub mod onward;
pub mod picture;
pub mod title;

use std::sync::Arc;

use crate::gfx::font::{self, Font};
use crate::gfx::palette::{self as pal, Rgb};
use crate::gfx::{H, Screen, Sprite, W};
use crate::logic::{CheeseKind, Dir, Level, Pos, Status, World, stars};
use anim::{Anim, dist, lerp};
use art::Art;
use fx::Fx;

/// Edge length of a board tile in framebuffer pixels. All board sprites are this size.
pub const TILE: i32 = 16;
/// Height of the status bar at the top (level name, holes, moves, par).
pub const HUD_H: i32 = 12;
/// Height of the key-help bar at the bottom.
pub const FOOT_H: i32 = 12;
/// The widest board that fits: 320 / 16 = 20 tiles. Together with [`MAX_ROWS`] this makes
/// the board area 20x11 tiles; a test checks that every level fits.
pub const MAX_COLS: i32 = W / TILE;
/// The tallest board that fits between the two bars: (200 - 12 - 12) / 16 = 11 tiles,
/// which fills the 176 pixels between them exactly.
pub const MAX_ROWS: i32 = (H - HUD_H - FOOT_H) / TILE;

/// Seconds after solving before the "Delicious!" panel shows, so the last cheese can
/// finish sinking into its hole first. The panel's stars then pop in one per 0.25 s.
pub const SOLVED_DELAY: f32 = 0.45;
/// How far a cheese sinks into its mouse hole, in pixels. Combined with the clip line at
/// row 13 of the hole tile this leaves at most the sprite's top row visible at full depth,
/// so the hole's own "cheese inside" sprite is what remains on screen.
const SINK: i32 = 12;

/// Everything [`Renderer::draw_play`] needs for one frame of a level, borrowed from the
/// app. All times are in seconds.
pub struct PlayView<'a> {
    /// The level and its current state (after the latest move).
    pub world: &'a World,
    /// The turn being animated, if any: it holds the state *before* the move, and positions
    /// are interpolated from there towards `world.state`. `None` draws everything at rest.
    pub anim: Option<&'a Anim>,
    /// Particles (crumb bursts and the like), drawn over the board and under the HUD.
    pub fx: &'a Fx,
    /// 1-based level number.
    pub number: usize,
    /// The best move count recorded for this level, shown above the key-help bar.
    pub best: Option<u32>,
    /// Seconds since launch, for blinking.
    pub t: f32,
    /// Seconds since the level was solved or the mouse caught.
    pub end_t: f32,
    /// Whether to dim the screen and show the pause box on top.
    pub paused: bool,
    /// Brie trails to draw on the floor.
    pub trails: &'a [Trail],
    /// Seconds the mouse has stood still (she twitches her ears when idle).
    pub idle: f32,
    /// Seconds since her last idle squeak, while it's on screen.
    pub squeak_age: Option<f32>,
}

/// A faint streak where a brie slid. Purely cosmetic.
///
/// It is drawn as a 9-pixel-wide band of cream along the line between the two tile centers
/// (shifted 3 pixels down, to sit under the cheese rather than through it). The band is made
/// of nine parallel 1px lines, blended at 8% in the middle three, 16% in the next two on
/// each side and 10% on the outermost, so it looks like a groove with brighter edges. One
/// pixel in seven is skipped in a diagonal pattern to give it some texture. Starting at
/// `born`, the band grows from `from` to `to` over [`anim::duration`] for the slide's
/// length with a quadratic ease-out, like the slide itself, so it roughly keeps pace with
/// the cheese.
#[derive(Clone, Copy, Debug)]
pub struct Trail {
    /// Tile the brie started its slide from.
    pub from: Pos,
    /// Tile the brie stopped on. Trails are straight lines, so `from` and `to` share a row
    /// or a column.
    pub to: Pos,
    /// When it was made (seconds since launch), so it grows with the slide.
    pub born: f32,
}

/// Everything [`Renderer::draw_select`] needs for one frame of the level select.
pub struct SelectView<'a> {
    /// The episode's levels, in order.
    pub levels: &'a [Arc<Level>],
    /// Best move count per level (same indices as `levels`), `None` if never solved.
    pub best: &'a [Option<u32>],
    /// The selected slot: `0..levels.len()` is a level, and `levels.len() + k` is locked
    /// episode `k` (see [`select_move`]).
    pub cursor: usize,
    /// Levels `0..unlocked` can be played.
    pub unlocked: usize,
    /// Showing the (pretend) order form for the full version.
    pub order: bool,
    /// Testing mode (`--cheese`): advertise its shortcuts.
    pub cheese: bool,
    /// Seconds since launch, for the order form's blinking prompt.
    pub t: f32,
}

/// Top-left pixel of the board, centered in the play area.
///
/// Horizontally the board is centered on the full 320-pixel width; vertically it is centered
/// in the [`MAX_ROWS`] * 16 pixel area that starts right below the HUD bar. Odd leftovers
/// round towards the top-left.
pub fn board_origin(level: &Level) -> (i32, i32) {
    (
        (W - level.w * TILE) / 2,
        HUD_H + (MAX_ROWS * TILE - level.h * TILE) / 2,
    )
}

/// The tile under a framebuffer pixel, for taps. The inverse of [`board_origin`] plus tile
/// size. Uses floor division, so pixels left of or above the board map to negative tiles
/// rather than to tile 0; the result may be outside the level and the caller must check.
pub fn tile_at(level: &Level, x: i32, y: i32) -> Pos {
    let (ox, oy) = board_origin(level);
    Pos::new((x - ox).div_euclid(TILE), (y - oy).div_euclid(TILE))
}

/// Number of level boxes per row in the select grid.
const SELECT_COLS: usize = 4;
/// Width of one level box in the select grid, in pixels.
const BOX_W: i32 = 60;
/// Height of one level box (the big number on top, three stars below).
const BOX_H: i32 = 28;
/// Horizontal gap between level boxes.
const GAP_X: i32 = 8;
/// Vertical gap between rows of level boxes.
const GAP_Y: i32 = 4;
/// Top of the first row of level boxes, just below the 21-pixel title bar. Rows follow every
/// `BOX_H + GAP_Y` = 32 pixels, so rows start at 26, 58 and 90; a fourth row would reach into
/// the episode banners at [`EP_Y`], so the grid has room for 12 levels.
const SELECT_Y: i32 = 26;
/// Top of the first locked-episode banner below the level grid.
const EP_Y: i32 = 124;
/// Height of one locked-episode banner. Banners are stacked with a 3-pixel gap.
const EP_H: i32 = 14;

/// The episodes "only in the registered version": (title, level count). They appear as
/// selectable banners under the level grid; choosing one opens the (pretend) order form.
pub const LOCKED_EPISODES: [(&str, u32); 2] =
    [("THE GOUDA, THE BAD & THE UGLY", 18), ("FONDUE OR DIE", 22)];

/// Left edge of the select grid: the grid is `4 * 60 + 3 * 8` = 264 pixels wide and
/// centered on the screen, so this is 28.
fn grid_x0() -> i32 {
    (W - (SELECT_COLS as i32 * (BOX_W + GAP_X) - GAP_X)) / 2
}

/// Top-left pixel of level box `i` in the select grid, filled row by row, four per row.
fn select_box(i: usize) -> (i32, i32) {
    let (col, row) = ((i % SELECT_COLS) as i32, (i / SELECT_COLS) as i32);
    (
        grid_x0() + col * (BOX_W + GAP_X),
        SELECT_Y + row * (BOX_H + GAP_Y),
    )
}

/// The banner of locked episode `k` (0 = episode two) as `(x, y, w, h)`: as wide as the
/// level grid and aligned with it.
fn episode_box(k: usize) -> (i32, i32, i32, i32) {
    let w = SELECT_COLS as i32 * (BOX_W + GAP_X) - GAP_X;
    (grid_x0(), EP_Y + k as i32 * (EP_H + 3), w, EP_H)
}

/// Which slot a framebuffer pixel is on: levels are `0..count`, then the locked episodes.
///
/// The slots use the same numbering as [`SelectView::cursor`]. Gaps between boxes and the
/// "more in the full version" filler boxes are not hits.
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
///
/// Slots `0..count` are levels laid out four to a row (the last row may be short); slots
/// `count..` are the locked episode banners stacked below the grid. The rules:
///
/// - Left and right move within a row and stop at its ends (no wrapping); right also stops
///   at the last level of a short row.
/// - Up and down move by a whole row when the slot above or below exists.
/// - Down from the last row enters the first episode banner. Down from the row above it,
///   where the slot directly below is missing (a gap in a short last row), goes to the last
///   level instead.
/// - On the banners, down and up step between banners; up from the first banner returns to
///   the first slot of the last grid row. Left and right do nothing there.
///
/// `count` must be at least 1.
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

/// Draws every screen of the game. Holds the parsed sprites and the decoded painted
/// pictures, which are built once in [`Renderer::new`] and only read afterwards.
pub struct Renderer {
    /// All sprites.
    art: Art,
    /// The title screen painting.
    cover: picture::Picture,
    /// Episode intro cards, episode 1 first.
    intros: [picture::Picture; intro::EPISODES],
    /// The ending painting for episode one.
    victory: picture::Picture,
    /// The painting on the card shown when the mouse is caught.
    defeat: picture::Picture,
    /// The painting behind the "Onward!" card after the ending.
    onward: picture::Picture,
    /// The painting behind the "Farewell!" screen shown when quitting.
    farewell: picture::Picture,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    /// Parse all sprites and decode all embedded pictures. Done once at startup.
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
            onward: picture::Picture::onward_episode_1(),
            farewell: picture::Picture::farewell(),
        }
    }

    /// Fill the screen with the dark blue backdrop shared by the play and select screens: navy
    /// with a sparse stipple of slightly lighter dots. A dot is drawn where `(x + y) % 4 == 0`
    /// (diagonal lines of every fourth pixel) and `(x / 2 + y) % 8 < 2` (which keeps only short
    /// runs of those lines), giving a faint diagonal texture.
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

    /// Draw one frame of a level, back to front:
    ///
    /// 1. The backdrop, then the board: floor (two plank variants in a checkerboard), a
    ///    translucent shadow on floor tiles below or right of a wall, holes (with the "cheese
    ///    inside" sprite if the *current* state has a cheese there), and walls, but only walls
    ///    adjacent (including diagonally) to an inside tile, so the void around the building
    ///    stays empty.
    /// 2. Brie trails (see [`Trail`]).
    /// 3. Footprints: each cat's prints on the tile it just left, and fainter ones on the
    ///    tile it left the turn before, read from the undo history (see `paw_prints`).
    /// 4. Cheeses, interpolated from the previous state. A cheese whose move starts or ends on a
    ///    hole sinks into or rises out of it during the half of the move spent over the hole:
    ///    it is drawn up to `SINK` pixels lower and clipped at row 13 of the hole's tile, so
    ///    it vanishes behind the hole's lip from the bottom up. Cheeses at rest in a hole get an
    ///    occasional sparkle and a rising heart.
    /// 5. Cats, interpolated with the walk timing, with a stride frame for the middle of the
    ///    step. Facing follows `facing_left`.
    /// 6. The mouse: bobbing one pixel and using its stride frame while walking, twitching an
    ///    ear now and then while idle and rapidly while squeaking, with the "SQUEAK!" text
    ///    rising above her. If she was caught, she is drawn squashed with the cat pouncing on
    ///    top instead.
    /// 7. Particles, the HUD, the level hint (only before the first move), then the solved
    ///    panel or the defeat card, and finally the pause overlay.
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

        // Footprints: each cat leaves prints on the tile it just left, and fainter ones on
        // the tile it left the turn before, so the trail fades over two steps. They are
        // read from the undo history, so they follow undo; a pair of states only leaves
        // prints when it is one real move apart (not a restart) and the cat walked one tile.
        let steps = [(w.back(1), Some(state), 0.8), (w.back(2), w.back(1), 0.4)];
        for (older, newer, alpha) in steps {
            let (Some(older), Some(newer)) = (older, newer) else {
                continue;
            };
            if newer.moves != older.moves + 1 {
                continue;
            }
            for (o, n) in older.cats.iter().zip(&newer.cats) {
                if dist(o.pos, n.pos) == 1 {
                    let d = (n.pos.x - o.pos.x, n.pos.y - o.pos.y);
                    let at = (ox + o.pos.x * TILE, oy + o.pos.y * TILE);
                    paw_prints(s, &self.art.paw, at, d, alpha);
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
        // The cat that made the catch: the one on the mouse's tile, or (when the mouse
        // walked into a cat that then stepped off) the one right next to it. Once the turn
        // has played out it is drawn pouncing onto the mouse instead of walking, so it must
        // not be drawn twice.
        let pouncer = (caught && walk.is_none_or(Anim::done))
            .then(|| {
                state
                    .cats
                    .iter()
                    .position(|c| c.pos == state.mouse)
                    .or_else(|| {
                        state
                            .cats
                            .iter()
                            .position(|c| dist(c.pos, state.mouse) <= 1)
                    })
            })
            .flatten();
        for (i, cat) in state.cats.iter().enumerate() {
            let a = from.cats.get(i).map_or(cat.pos, |f| f.pos);
            let u = walk.map_or(1.0, Anim::walk);
            let (fx, fy) = lerp(a, cat.pos, u);
            let (x, y) = px(fx, fy);
            let left = facing_left(cat.dir, from.cats.get(i).map(|c| c.dir));
            if pouncer == Some(i) {
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
            if let Some(cat) = pouncer.map(|i| state.cats[i]) {
                // Crouched on top of the flattened mouse, facing the way it came from: a
                // cat on the mouse's tile faces its walking direction; one beside it faces
                // the mouse.
                let left = if cat.pos == state.mouse {
                    cat.dir == Dir::Left
                } else {
                    cat.pos.x > state.mouse.x
                };
                s.blit_ex(&self.art.cat_pounce, x, y - 4, left, None);
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

    /// The top status bar (level number and title on the left; holes filled, moves and par on
    /// the right, in pink once over par), the bottom key-help bar, and the best score just
    /// above the bottom bar's right end.
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

    /// The "Delicious!" panel shown [`SOLVED_DELAY`] seconds after solving: the title in a
    /// pulsing yellow gradient, up to three stars that fill in one every 0.25 s, the move count
    /// against par, a verdict line, and a blinking prompt.
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

    /// Draw the level select: the episode title bar with a "SHAREWARE" stamp, the grid of
    /// level boxes (selected one in cheese colors with an ink outline, locked ones dark with a
    /// padlock, each with its earned stars), "more in the full version" filler boxes to
    /// complete the last row, the locked-episode banners, an info bar at the bottom describing
    /// the slot under the cursor, and the order form on top if it is open.
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
                "ARROWS PICK  SPACE PLAY  ESC EXIT  -  CHEESE: I INTRO  E END  F FAREWELL",
                pal::PINK,
            )
        } else {
            ("ARROWS PICK  SPACE PLAY  ESC EXIT", pal::STEEL_LT)
        };
        font::draw_centered(s, Font::Tiny, W / 2, y + 28, help, color);

        if v.order {
            order_form(s, v.t);
        }
    }

    // ---- Title and ending --------------------------------------------------------------

    /// The title screen; `t` is seconds since it appeared.
    pub fn draw_title(&self, s: &mut Screen, t: f32) {
        title::draw(s, &self.cover, t);
    }

    /// The card before an episode's first level (`episode` is 1-based and clamped to the
    /// episodes that exist).
    pub fn draw_intro(&self, s: &mut Screen, episode: usize, t: f32) {
        let i = episode.clamp(1, intro::EPISODES);
        intro::draw(s, &self.intros[i - 1], i, t);
    }

    /// The end of episode one, with `earned` of `max` possible stars.
    pub fn draw_end(&self, s: &mut Screen, earned: u32, max: u32, t: f32) {
        ending::draw(s, &self.victory, earned, max, t);
    }

    /// "Farewell!", when leaving the game.
    pub fn draw_farewell(&self, s: &mut Screen, t: f32) {
        farewell::draw(s, &self.farewell, t);
    }

    /// The "Onward!" card after the ending.
    pub fn draw_onward(&self, s: &mut Screen, t: f32) {
        onward::draw(s, &self.onward, t);
    }
}

/// A screen-centered toast near the bottom: one line of yellow text in a rounded box just
/// above the key-help bar.
pub fn toast(s: &mut Screen, msg: &str) {
    let w = Font::Normal.measure(msg, 1) + 12;
    let (x, y) = (W / 2 - w / 2, H - FOOT_H - 18);
    s.round_box(x, y, w, 13, pal::NIGHT, pal::STEEL_LT);
    font::draw_centered(s, Font::Normal, W / 2, y + 3, msg, pal::YELLOW);
}

/// A hint in a speech bubble at the bottom of the board area, word-wrapped.
///
/// The text is uppercased and greedily wrapped at word boundaries to at most
/// `(320 - 24) / 6` = 49 characters per line (a single longer word gets a line to itself and
/// may overflow). The bubble is sized to the longest line plus padding, centered, placed
/// just above the key-help bar, and given a translucent drop shadow.
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

/// The (pretend) order form for the registered version, drawn over a dimmed screen: a
/// cream sheet with a drop shadow, a red title, the sales lines, fine print and a blinking
/// prompt.
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

/// A dialog panel: a translucent drop shadow, a dark rounded box with a yellow outline, and
/// an inner slate rule two pixels in. Used for the pause box and the solved panel.
fn panel(s: &mut Screen, x: i32, y: i32, w: i32, h: i32) {
    s.blend_rect(x + 3, y + 3, w, h, pal::INK, 0.5);
    s.round_box(x, y, w, h, pal::NIGHT, pal::YELLOW);
    s.rect_outline(x + 2, y + 2, w - 4, h - 4, pal::SLATE);
}

/// Whether blinking text is visible at time `t`: on for 1 s, off for 0.5 s, repeating
/// every 1.5 s.
fn blink(t: f32) -> bool {
    (t * 2.0) as i32 % 3 != 0
}

/// Whether a wall tile borders the inside (walls deep in the void aren't drawn). Checks the
/// 3x3 neighbourhood, so diagonal contact counts and outer corners are drawn.
fn touches_inside(level: &Level, p: Pos) -> bool {
    (-1..=1).any(|dy| (-1..=1).any(|dx| level.inside(Pos::new(p.x + dx, p.y + dy))))
}

/// Two paw prints on a tile (`x, y` its top-left pixel), staggered along the walking
/// direction `d` (a unit step in tiles) like a left and a right paw, blended over the floor
/// with opacity `alpha`.
fn paw_prints(s: &mut Screen, paw: &Sprite, (x, y): (i32, i32), d: (i32, i32), alpha: f32) {
    // Offsets of the two prints inside the 16x16 tile: one behind and to one side, one
    // ahead and to the other, so they read as a stride in the direction of travel.
    let spots = match d {
        (1, _) => [(2, 8), (9, 3)],
        (-1, _) => [(9, 8), (2, 3)],
        (_, 1) => [(3, 2), (8, 9)],
        _ => [(8, 9), (3, 2)],
    };
    for (dx, dy) in spots {
        for py in 0..paw.h {
            for px in 0..paw.w {
                if paw.px[(py * paw.w + px) as usize].is_some() {
                    s.blend(x + dx + px, y + dy + py, pal::INK, alpha);
                }
            }
        }
    }
}

/// Whether a cat's sprite should be mirrored to face left. A cat walking left faces left
/// and one walking right faces right. Cats only ever reverse direction (left/right or
/// up/down) and never change axis, so a cat on a vertical patrol has never moved sideways
/// and simply faces right; `prev` (its direction before this turn's animation) only
/// matters for consistency during the tween.
fn facing_left(dir: Dir, prev: Option<Dir>) -> bool {
    match dir {
        Dir::Left => true,
        Dir::Right => false,
        _ => prev == Some(Dir::Left),
    }
}

/// The colors of a cheese, for crumb bursts: particles pick from these so the crumbs match
/// the cheese that was pushed.
pub fn cheese_colors(kind: CheeseKind) -> &'static [Rgb] {
    match kind {
        CheeseKind::Wedge => &[pal::YELLOW, pal::YELLOW_LT, pal::CHEESE_DK],
        CheeseKind::Wheel => &[pal::RED, pal::PINK, pal::YELLOW],
        CheeseKind::Brie => &[pal::CREAM, pal::RIND, pal::YELLOW_LT],
    }
}

/// Pixel center of a tile, as floats for particle positions.
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
    /// Walks the cursor through an 11-level grid (rows of 4, 4 and 3) and the two banners.
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
    /// Pixel hit-testing finds level boxes and episode banners, and misses the title bar.
    fn select_hit_finds_boxes_and_banners() {
        let (x, y) = select_box(5);
        assert_eq!(select_hit(11, x + 2, y + 2), Some(5));
        let (x, y, ..) = episode_box(1);
        assert_eq!(select_hit(11, x + 20, y + 3), Some(12));
        assert_eq!(select_hit(11, 0, 0), None);
    }

    #[test]
    /// Every level fits in the [`MAX_COLS`] x [`MAX_ROWS`] board area.
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
