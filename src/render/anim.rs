//! Tweens between two puzzle states, so a turn slides instead of snapping.

use crate::logic::{CheeseKind, Pos, State};

/// Seconds for one tile of walking.
pub const STEP_SECS: f32 = 0.11;
/// Seconds per tile for sliding brie (faster than walking: it's slippery).
const SLIDE_SECS: f32 = 0.05;

/// How long a turn takes to animate when something slides `tiles` tiles.
pub fn duration(tiles: f32) -> f32 {
    STEP_SECS.max(STEP_SECS * 0.5 + tiles * SLIDE_SECS)
}

/// The turn being animated: everything moves from `from` to the world's current state.
#[derive(Clone, Debug)]
pub struct Anim {
    pub from: State,
    pub t: f32,
    pub dur: f32,
}

impl Anim {
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

    pub fn update(&mut self, dt: f32) {
        self.t = (self.t + dt).min(self.dur);
    }

    pub fn done(&self) -> bool {
        self.t >= self.dur
    }

    /// Walking progress, 0..1 (the mouse and cats take one tile per turn).
    pub fn walk(&self) -> f32 {
        (self.t / STEP_SECS).min(1.0)
    }

    /// Progress for something that moved `tiles` tiles this turn: one tile at walking pace,
    /// longer slides spread over the whole animation, easing out.
    pub fn progress(&self, tiles: i32) -> f32 {
        if tiles <= 1 {
            self.walk()
        } else {
            let u = (self.t / self.dur).min(1.0);
            1.0 - (1.0 - u) * (1.0 - u)
        }
    }
}

/// Pixel offset (in tiles, fractional) between two tiles at progress `u`.
pub fn lerp(a: Pos, b: Pos, u: f32) -> (f32, f32) {
    (
        a.x as f32 + (b.x - a.x) as f32 * u,
        a.y as f32 + (b.y - a.y) as f32 * u,
    )
}

pub fn dist(a: Pos, b: Pos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
