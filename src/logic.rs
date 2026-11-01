//! The rules of Cheesed!, free of toolkit types so they can be unit tested and solved.
//!
//! Turn-based: every step the mouse takes is one move, and after every move each cat takes
//! one step along its patrol. Cheese comes in three kinds:
//!
//! - **Wedge**: the classic crate. One push moves it one tile.
//! - **Wheel**: heavy. The mouse needs a run-up: it only budges when the mouse's previous
//!   move went the same way.
//! - **Brie**: slippery. Once pushed it slides until something stops it, gliding straight
//!   over empty mouse holes.
//!
//! The level is solved when every mouse hole has a cheese in it.
//!
//! # Design
//!
//! The rules are split in two layers:
//!
//! - [`State`] is a plain value (positions, counters, status) and [`State::step`] is a pure
//!   function from one state to the next. Nothing in it knows about time, animation or
//!   sound; it only reports what happened as a list of [`Event`]s. Because it is pure and
//!   cheap to clone, the [`solver`] can explore it breadth-first and the tests can replay
//!   solutions through it.
//! - [`World`] wraps a state with the level it belongs to and an undo history of whole
//!   `State` copies. The app drives the game through `World` and drains `World::events`
//!   each frame to trigger sounds and effects.
//!
//! The static parts of a level (walls, holes, the start layout) live in [`Level`], which is
//! shared behind an `Arc` and never changes during play.
//!
//! # Turn order
//!
//! One call to [`State::step`] resolves in this order:
//!
//! 1. The mouse tries to move. Walls, an immovable cheese or a wheel without a run-up stop
//!    it, and then the whole turn is cancelled: the cats do not move either, and no move is
//!    counted.
//! 2. If the mouse moved, each cat in turn takes one step (see [`Cat`]).
//! 3. The mouse is caught if it walked onto a cat or a cat ended its step on the mouse.
//!    Otherwise the level is solved if every hole now holds a cheese. Being caught wins
//!    over solving when both happen on the same turn.

pub mod level;
pub mod solver;

pub use level::{Level, levels};

use std::sync::Arc;

/// A tile coordinate on the level grid. `(0, 0)` is the top-left tile; `x` grows to the
/// right and `y` grows downwards, matching the row/column order of the level text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Pos {
    /// Column, counted from the left edge of the level text.
    pub x: i32,
    /// Row, counted from the top line of the level grid.
    pub y: i32,
}

impl Pos {
    /// A position at column `x`, row `y`.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The neighbouring tile one step in direction `d`. Not bounds-checked: the result may
    /// lie off the map, which [`Level::wall`] treats as a wall.
    pub fn step(self, d: Dir) -> Pos {
        let (dx, dy) = d.delta();
        Pos::new(self.x + dx, self.y + dy)
    }
}

/// One of the four grid directions the mouse and the cats can move in.
///
/// The declaration order matters in two places: [`Dir::ALL`] (and so the order in which
/// the solver tries moves) and the solver's state key, which stores a direction as its
/// discriminant.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dir {
    /// Towards row 0 (negative `y`).
    Up,
    /// Towards higher rows (positive `y`).
    Down,
    /// Towards column 0 (negative `x`).
    Left,
    /// Towards higher columns (positive `x`).
    Right,
}

impl Dir {
    /// All four directions, in declaration order.
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

    /// The `(dx, dy)` offset of one step in this direction, in tiles. `y` grows downwards,
    /// so `Up` is `(0, -1)`.
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    /// The opposite direction. Used when a cat turns around and by the solver when it
    /// works backwards from the holes.
    pub fn reverse(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }

    /// The solution-notation letter (`u d l r`).
    pub fn letter(self) -> char {
        match self {
            Dir::Up => 'u',
            Dir::Down => 'd',
            Dir::Left => 'l',
            Dir::Right => 'r',
        }
    }
}

/// The three kinds of cheese. They differ only in how a push moves them; see the module
/// docs and [`State::step`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CheeseKind {
    /// Moves exactly one tile per push.
    Wedge,
    /// Moves one tile per push, but only if the mouse's previous move was in the same
    /// direction as the push (see [`State::momentum`]).
    Wheel,
    /// Slides in the push direction until the next tile is a wall, another cheese or a cat.
    /// Holes do not stop it, so it only stays in a hole that is backed by an obstacle.
    Brie,
}

/// A piece of cheese on the board. Cheeses of the same kind are interchangeable; any
/// cheese can fill any hole.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cheese {
    /// The tile the cheese occupies.
    pub pos: Pos,
    /// How the cheese reacts to a push.
    pub kind: CheeseKind,
}

/// A cat walks straight ahead each turn and turns around when blocked.
///
/// Concretely, on every turn the mouse actually moves: if the tile ahead is free (no wall,
/// cheese or other cat), the cat steps onto it. Otherwise it reverses its direction and
/// steps backwards if that tile is free, or stays put (still reversed) if it is boxed in.
/// The mouse is not an obstacle: a cat stepping onto the mouse catches it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cat {
    /// The tile the cat stands on.
    pub pos: Pos,
    /// The direction the cat will try to walk next turn.
    pub dir: Dir,
}

/// Whether a level is still in progress. Once it is not `Playing`, [`State::step`] refuses
/// all moves; only undo or restart (through [`World`]) leave the end states.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    /// The level is in progress and accepts moves.
    Playing,
    /// A cat got the mouse. Undo or restart.
    Caught,
    /// Every mouse hole holds a cheese.
    Solved,
}

/// What happened during a turn, for sound and effects.
///
/// Events are pushed in the order things happen within the turn: the mouse's action first
/// (`Step`, `Pushed` with any `Unplopped`/`Plopped`), then each cat's `CatTurned`, then
/// `Caught` or `Solved`. A refused move produces only `Bump` or `TooHeavy`. The logic never
/// reads events back; they are purely an output channel for the app.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    /// The mouse moved onto an empty tile without pushing anything.
    Step,
    /// Walked into a wall or pushed against something that won't move.
    Bump,
    /// Pushed a wheel without a run-up.
    TooHeavy,
    /// The mouse pushed a cheese. For brie, `to` is where the slide ended, which may be
    /// several tiles from `from`.
    Pushed {
        /// The kind of cheese that moved.
        kind: CheeseKind,
        /// Where the cheese was before the push (the tile the mouse moves onto).
        from: Pos,
        /// Where the cheese came to rest.
        to: Pos,
    },
    /// A cheese came to rest in a mouse hole.
    Plopped(Pos),
    /// A cheese left a mouse hole.
    Unplopped(Pos),
    /// The cat with this index in [`State::cats`] was blocked and reversed direction.
    CatTurned(usize),
    /// A cat caught the mouse at this tile (the mouse's position after its move).
    Caught(Pos),
    /// The move filled the last empty hole.
    Solved,
    /// [`World::undo`] stepped back one state. Never produced by [`State::step`].
    Undone,
    /// [`World::restart`] reset the level. Never produced by [`State::step`].
    Restarted,
}

/// Everything that changes during play. Small, so undo keeps whole copies.
///
/// The derived `Eq`/`Hash` compare every field, including the counters and `facing`; the
/// solver therefore builds its own, smaller key instead of hashing `State` directly.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct State {
    /// The mouse's tile.
    pub mouse: Pos,
    /// Which way the mouse sprite looks. Cosmetic: it follows the last attempted move, even
    /// a refused one (see [`World::step`]), and has no effect on the rules.
    pub facing: Dir,
    /// The direction of the previous successful move, `None` only before the first move
    /// of a level. A wheel only moves when this equals the push direction, so the mouse
    /// needs a one-tile run-up. Refused moves (bumps, too-heavy pushes) leave it unchanged.
    pub momentum: Option<Dir>,
    /// All cheeses, in level order. Indices are stable for the lifetime of a level, which
    /// the animation code relies on to match cheeses between two states.
    pub cheeses: Vec<Cheese>,
    /// All cats, in level order. They move in this order each turn, so the order is part
    /// of the rules.
    pub cats: Vec<Cat>,
    /// Successful moves so far; this is what par and the stars are measured against.
    pub moves: u32,
    /// How many of those moves pushed a cheese.
    pub pushes: u32,
    /// Whether the level is still being played.
    pub status: Status,
}

impl State {
    /// Index into [`State::cheeses`] of the cheese at `p`, if there is one. A linear scan;
    /// levels hold only a handful of cheeses.
    pub fn cheese_at(&self, p: Pos) -> Option<usize> {
        self.cheeses.iter().position(|c| c.pos == p)
    }

    /// Index into [`State::cats`] of the cat at `p`, if there is one.
    pub fn cat_at(&self, p: Pos) -> Option<usize> {
        self.cats.iter().position(|c| c.pos == p)
    }

    /// Can a cheese (or a cat) enter `p`? True when `p` is not a wall and holds neither a
    /// cheese nor a cat. The mouse does not count as an obstacle.
    fn free(&self, level: &Level, p: Pos) -> bool {
        !level.wall(p) && self.cheese_at(p).is_none() && self.cat_at(p).is_none()
    }

    /// True when every mouse hole holds a cheese (of any kind).
    pub fn is_solved(&self, level: &Level) -> bool {
        level.holes.iter().all(|&h| self.cheese_at(h).is_some())
    }

    /// One turn: the mouse tries to step `dir`, then the cats walk. Returns `None` when
    /// nothing changed (a bump, a too-heavy wheel, or the game is already over).
    ///
    /// `self` is never modified; the new state is a modified clone. Events describing the
    /// turn are appended to `events` (also when the move is refused, so the app can play a
    /// bump sound).
    ///
    /// How the mouse's move resolves:
    ///
    /// - A wall at the target refuses the move with [`Event::Bump`].
    /// - A cheese at the target is pushed one tile further. A wheel refuses with
    ///   [`Event::TooHeavy`] unless [`State::momentum`] equals `dir`. If the tile beyond the
    ///   cheese is not free (wall, cheese or cat, using the cats' positions before they
    ///   move), the push is refused with [`Event::Bump`]. Brie then keeps sliding while the
    ///   next tile is free. A successful push counts in both `moves` and `pushes`.
    /// - Otherwise the mouse simply steps (also onto a cat, which catches it).
    ///
    /// After a successful move the cats walk in index order. Each one sees the cats before it
    /// at their new positions and the cats after it at their old ones, and the cheeses at
    /// their post-push positions. The mouse is caught if a cat stood on its target before
    /// the cats moved (walking into a cat, which also covers a cat and mouse trying to swap
    /// tiles) or if any cat ends its step on the target.
    pub fn step(&self, level: &Level, dir: Dir, events: &mut Vec<Event>) -> Option<State> {
        if self.status != Status::Playing {
            return None;
        }
        let target = self.mouse.step(dir);
        if level.wall(target) {
            events.push(Event::Bump);
            return None;
        }
        let mut next = self.clone();
        next.facing = dir;
        if let Some(i) = self.cheese_at(target) {
            let kind = self.cheeses[i].kind;
            if kind == CheeseKind::Wheel && self.momentum != Some(dir) {
                events.push(Event::TooHeavy);
                return None;
            }
            let mut dest = target.step(dir);
            if !self.free(level, dest) {
                events.push(Event::Bump);
                return None;
            }
            if kind == CheeseKind::Brie {
                while self.free(level, dest.step(dir)) {
                    dest = dest.step(dir);
                }
            }
            next.cheeses[i].pos = dest;
            next.pushes += 1;
            events.push(Event::Pushed {
                kind,
                from: target,
                to: dest,
            });
            if level.is_hole(target) {
                events.push(Event::Unplopped(target));
            }
            if level.is_hole(dest) {
                events.push(Event::Plopped(dest));
            }
        } else {
            events.push(Event::Step);
        }
        next.mouse = target;
        next.moves += 1;
        next.momentum = Some(dir);

        // The cats walk, in order; each one sees the others' new positions.
        let mut caught = self.cat_at(target).is_some();
        for i in 0..next.cats.len() {
            let cat = next.cats[i];
            let mut dir = cat.dir;
            let mut to = cat.pos.step(dir);
            if !next.free(level, to) {
                dir = dir.reverse();
                to = cat.pos.step(dir);
                events.push(Event::CatTurned(i));
            }
            next.cats[i].dir = dir;
            if next.free(level, to) {
                next.cats[i].pos = to;
            }
            caught |= next.cats[i].pos == target;
        }

        if caught {
            next.status = Status::Caught;
            events.push(Event::Caught(target));
        } else if next.is_solved(level) {
            next.status = Status::Solved;
            events.push(Event::Solved);
        }
        Some(next)
    }
}

/// How well a level was solved: 3 stars at or under par.
///
/// Two stars allow a slack of a quarter of par, but never less than 4 moves, so short
/// levels still leave room for a small detour. Anything slower earns one star; a solve
/// never earns zero.
pub fn stars(moves: u32, par: u32) -> u8 {
    if moves <= par {
        3
    } else if moves <= par + (par / 4).max(4) {
        2
    } else {
        1
    }
}

/// A level being played, with undo history.
///
/// Undo is snapshot-based: before every change the previous [`State`] is pushed onto
/// `history`, and undo pops it back. States are a few small vectors, so copying beats
/// recording and inverting moves. The history is unbounded and survives restarts, so a
/// restart can itself be undone.
pub struct World {
    /// The level being played; shared with the level list and never modified.
    pub level: Arc<Level>,
    /// The current position of everything.
    pub state: State,
    /// Earlier states, oldest first. The last entry is what [`World::undo`] restores.
    history: Vec<State>,
    /// Events since the app last drained them.
    pub events: Vec<Event>,
}

impl World {
    /// A fresh play-through of `level`, at its start position with empty history.
    pub fn new(level: Arc<Level>) -> Self {
        let state = level.start();
        Self {
            level,
            state,
            history: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Take a step. Returns whether anything changed.
    ///
    /// A successful step pushes the old state onto the undo history. A refused step does
    /// not, but while the level is still being played it turns the mouse to face `dir`, so
    /// the player sees which way it tried to go. The `false` return then still means
    /// "nothing undoable happened".
    pub fn step(&mut self, dir: Dir) -> bool {
        match self.state.step(&self.level, dir, &mut self.events) {
            Some(next) => {
                let prev = std::mem::replace(&mut self.state, next);
                self.history.push(prev);
                true
            }
            None => {
                // Bumping still turns the mouse around.
                if self.state.status == Status::Playing {
                    self.state.facing = dir;
                }
                false
            }
        }
    }

    /// Whether there is an earlier state to go back to.
    /// The state `n` steps back in the undo history (1 = the state before the last change),
    /// if there is one. Used for cosmetic look-backs such as the cats' footprints.
    pub fn back(&self, n: usize) -> Option<&State> {
        self.history.len().checked_sub(n).map(|i| &self.history[i])
    }

    /// How many states the undo history holds; grows by one for every step or restart that
    /// changed something, shrinks by one per undo.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Restore the previous state and emit [`Event::Undone`]. Works in any status, so it
    /// also takes back the move that got the mouse caught (or that solved the level).
    /// Returns `false`, without an event, when the history is empty.
    pub fn undo(&mut self) -> bool {
        match self.history.pop() {
            Some(prev) => {
                self.state = prev;
                self.events.push(Event::Undone);
                true
            }
            None => false,
        }
    }

    /// Back to the start. Undoable, so a hasty restart costs nothing.
    ///
    /// Does nothing (no history entry, no event) when the state already equals the start.
    /// The mouse's `facing` is left out of that comparison: turning on the spot by bumping
    /// into a wall isn't progress worth restarting (or undoing back to).
    pub fn restart(&mut self) {
        let start = self.level.start();
        let mut current = self.state.clone();
        current.facing = start.facing;
        if current != start {
            let prev = std::mem::replace(&mut self.state, start);
            self.history.push(prev);
            self.events.push(Event::Restarted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world for a level given inline as text; panics if it does not parse.
    fn world(src: &str) -> World {
        World::new(Arc::new(Level::parse(src).expect("level parses")))
    }

    #[test]
    fn walking_and_bumping() {
        let mut w = world("######\n#@ .$#\n######");
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.mouse, Pos::new(2, 1));
        assert!(!w.step(Dir::Up), "wall");
        assert_eq!(w.events.last(), Some(&Event::Bump));
        assert_eq!(w.state.moves, 1);
        assert_eq!(w.state.facing, Dir::Up, "bumping turns the mouse");
    }

    #[test]
    fn wedge_pushes_one_tile_and_solves() {
        let mut w = world("#####\n#@$.#\n#####");
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.cheeses[0].pos, Pos::new(3, 1));
        assert_eq!(w.state.status, Status::Solved);
        assert_eq!(w.state.pushes, 1);
        assert!(w.events.contains(&Event::Plopped(Pos::new(3, 1))));
        assert!(!w.step(Dir::Left), "no moves after solving");
    }

    #[test]
    fn cannot_push_two_cheeses() {
        let mut w = world("#######\n#@$$..#\n#######");
        assert!(!w.step(Dir::Right));
        assert_eq!(w.state.mouse, Pos::new(1, 1));
    }

    #[test]
    fn wheel_needs_a_run_up() {
        let mut w = world("#######\n# @o .#\n#######");
        assert!(!w.step(Dir::Right), "standing start");
        assert_eq!(w.events.last(), Some(&Event::TooHeavy));
        // Back off one tile, then run at it.
        assert!(w.step(Dir::Left));
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.momentum, Some(Dir::Right));
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.cheeses[0].pos, Pos::new(4, 1));
        assert!(w.step(Dir::Right), "momentum carries on");
        assert_eq!(w.state.status, Status::Solved);
    }

    #[test]
    fn brie_slides_over_holes_until_blocked() {
        let mut w = world("########\n#@b .  #\n########");
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.cheeses[0].pos, Pos::new(6, 1));
        let mut w = world("########\n#@b .$.#\n########");
        assert!(w.step(Dir::Right));
        assert_eq!(
            w.state.cheeses[0].pos,
            Pos::new(4, 1),
            "stops before the wedge"
        );
    }

    #[test]
    fn cats_patrol_and_turn() {
        let mut w = world("#######\n#@  $.#\n#  >  #\n#######");
        w.step(Dir::Down);
        assert_eq!(w.state.cats[0].pos, Pos::new(4, 2));
        w.step(Dir::Up);
        assert_eq!(w.state.cats[0].pos, Pos::new(5, 2));
        w.step(Dir::Down);
        assert_eq!(w.state.cats[0].pos, Pos::new(4, 2), "turned at the wall");
        assert_eq!(w.state.cats[0].dir, Dir::Left);
    }

    #[test]
    fn cat_catches_mouse_and_undo_rescues() {
        let mut w = world("######\n#@ <.#\n#   $#\n######");
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.status, Status::Caught);
        assert!(!w.step(Dir::Left), "game over until undo");
        assert!(w.undo());
        assert_eq!(w.state.status, Status::Playing);
        assert_eq!(w.state.mouse, Pos::new(1, 1));
    }

    #[test]
    fn walking_into_a_cat_or_swapping_is_caught() {
        let mut w = world("#####\n#@<.#\n#  $#\n#####");
        assert!(w.step(Dir::Right));
        assert_eq!(w.state.status, Status::Caught);
    }

    #[test]
    fn cheese_blocks_cats() {
        let mut w = world("#######\n#@   .#\n#  $< #\n#######");
        w.step(Dir::Right);
        assert_eq!(
            w.state.cats[0].pos,
            Pos::new(5, 2),
            "turned back from the cheese"
        );
    }

    #[test]
    fn turning_on_the_spot_is_not_worth_a_restart() {
        let mut w = world("######\n#@ $.#\n######");
        assert!(!w.step(Dir::Up), "bump into the wall: only turns");
        w.events.clear();
        w.restart();
        assert!(w.events.is_empty(), "no restart, no restart sound");
        assert!(!w.can_undo());
    }

    #[test]
    fn restart_is_undoable() {
        let mut w = world("######\n#@ $.#\n######");
        w.step(Dir::Right);
        w.restart();
        assert_eq!(w.state.mouse, Pos::new(1, 1));
        assert!(w.undo());
        assert_eq!(w.state.mouse, Pos::new(2, 1));
    }

    #[test]
    fn star_rating() {
        assert_eq!(stars(10, 10), 3);
        assert_eq!(stars(14, 10), 2);
        assert_eq!(stars(15, 10), 1);
        assert_eq!(stars(40, 40), 3);
        assert_eq!(stars(50, 40), 2);
        assert_eq!(stars(51, 40), 1);
    }
}
