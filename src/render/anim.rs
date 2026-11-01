//! Tweens between two puzzle states, so a turn slides instead of snapping.
//!
//! The puzzle logic resolves a move instantly. `app.rs` then keeps the previous [`State`]
//! in an [`Anim`] and the renderer draws every piece somewhere between its old and new tile,
//! using [`Anim::progress`] for how far along it is and [`lerp`] for where that is;
//! `app.rs` drops the animation once [`Anim::done`]. Positions here are in tiles
//! (fractional while moving); the renderer converts them to pixels. Times are seconds.
//!
//! Most things move one tile per turn and do so in [`STEP_SECS`]. Brie can slide several
//! tiles in one turn; the whole turn is then stretched to about `SLIDE_SECS` per slid
//! tile, while one-tile movers still finish at walking pace.

use crate::logic::{CheeseKind, Pos, State};

/// Seconds for one tile of walking.
pub const STEP_SECS: f32 = 0.11;
/// Seconds per tile for sliding brie (faster than walking: it's slippery).
const SLIDE_SECS: f32 = 0.05;

/// How long a turn takes to animate when something slides `tiles` tiles.
///
/// Half a step plus `SLIDE_SECS` per tile, but never shorter than one walking step. With
/// the current constants a slide only lengthens the turn from 2 tiles upwards (0.055 +
/// 2 * 0.05 = 0.155 s).
pub fn duration(tiles: f32) -> f32 {
    STEP_SECS.max(STEP_SECS * 0.5 + tiles * SLIDE_SECS)
}

/// The turn being animated: everything moves from `from` to the world's current state.
#[derive(Clone, Debug)]
pub struct Anim {
    /// The state before the turn; the "to" side is always the world's live state.
    pub from: State,
    /// Seconds since the turn started, clamped to `dur`.
    pub t: f32,
    /// Total seconds for this turn, from [`duration`] of the longest brie slide.
    pub dur: f32,
}

impl Anim {
    /// Start animating the turn from `from` to `to`.
    ///
    /// Finds the longest distance (Manhattan, in tiles) any brie moved, pairing the cheeses
    /// of both states by index (which assumes a turn keeps the cheese list in order), and
    /// sizes the animation with [`duration`].
    pub fn new(from: State, to: &State) -> Self {
        // Brie may slide several tiles; give it time to, but keep walking snappy.
        let slide = from
            .cheeses
            .iter()
            .zip(&to.cheeses)
            .filter(|(a, _)| a.kind == CheeseKind::Brie)
            .map(|(a, b)| ((a.pos.x - b.pos.x).abs() + (a.pos.y - b.pos.y).abs()) as f32)
            .fold(0.0f32, f32::max);
        Self {
            from,
            t: 0.0,
            dur: duration(slide),
        }
    }

    /// Advance by `dt` seconds, stopping at the end.
    pub fn update(&mut self, dt: f32) {
        self.t = (self.t + dt).min(self.dur);
    }

    /// Whether the turn has finished animating.
    pub fn done(&self) -> bool {
        self.t >= self.dur
    }

    /// Walking progress, 0..1 (the mouse and cats take one tile per turn).
    pub fn walk(&self) -> f32 {
        (self.t / STEP_SECS).min(1.0)
    }

    /// Progress for something that moved `tiles` tiles this turn: one tile at walking pace,
    /// longer slides spread over the whole animation, easing out.
    ///
    /// The ease-out is quadratic, `1 - (1 - u)^2`: a slide starts fast and decelerates
    /// into its final tile, which reads as slipping to a stop.
    pub fn progress(&self, tiles: i32) -> f32 {
        if tiles <= 1 {
            self.walk()
        } else {
            let u = (self.t / self.dur).min(1.0);
            1.0 - (1.0 - u) * (1.0 - u)
        }
    }
}

/// The position between tiles `a` and `b` at progress `u` (0 = at `a`, 1 = at `b`).
///
/// Despite being used for drawing, the result is in tiles (fractional), not pixels; the
/// renderer multiplies by the tile size.
pub fn lerp(a: Pos, b: Pos, u: f32) -> (f32, f32) {
    (
        a.x as f32 + (b.x - a.x) as f32 * u,
        a.y as f32 + (b.y - a.y) as f32 * u,
    )
}

/// Manhattan distance between two tiles. The renderer uses it as the number of tiles a
/// piece moved this turn, to pick between walking and sliding progress.
pub fn dist(a: Pos, b: Pos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
