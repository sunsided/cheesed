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

pub mod level;
pub mod solver;

pub use level::{Level, levels};

use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn step(self, d: Dir) -> Pos {
        let (dx, dy) = d.delta();
        Pos::new(self.x + dx, self.y + dy)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

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

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CheeseKind {
    Wedge,
    Wheel,
    Brie,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cheese {
    pub pos: Pos,
    pub kind: CheeseKind,
}

/// A cat walks straight ahead each turn and turns around when blocked.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cat {
    pub pos: Pos,
    pub dir: Dir,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    Playing,
    /// A cat got the mouse. Undo or restart.
    Caught,
    Solved,
}

/// What happened during a turn, for sound and effects.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Event {
    Step,
    /// Walked into a wall or pushed against something that won't move.
    Bump,
    /// Pushed a wheel without a run-up.
    TooHeavy,
    Pushed {
        kind: CheeseKind,
        from: Pos,
        to: Pos,
    },
    /// A cheese came to rest in a mouse hole.
    Plopped(Pos),
    /// A cheese left a mouse hole.
    Unplopped(Pos),
    CatTurned(usize),
    Caught(Pos),
    Solved,
    Undone,
    Restarted,
}

/// Everything that changes during play. Small, so undo keeps whole copies.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct State {
    pub mouse: Pos,
    pub facing: Dir,
    /// The direction of the previous move, if the mouse moved last turn: a wheel needs it.
    pub momentum: Option<Dir>,
    pub cheeses: Vec<Cheese>,
    pub cats: Vec<Cat>,
    pub moves: u32,
    pub pushes: u32,
    pub status: Status,
}

impl State {
    pub fn cheese_at(&self, p: Pos) -> Option<usize> {
        self.cheeses.iter().position(|c| c.pos == p)
    }

    pub fn cat_at(&self, p: Pos) -> Option<usize> {
        self.cats.iter().position(|c| c.pos == p)
    }

    /// Can a cheese (or a cat) enter `p`?
    fn free(&self, level: &Level, p: Pos) -> bool {
        !level.wall(p) && self.cheese_at(p).is_none() && self.cat_at(p).is_none()
    }

    pub fn is_solved(&self, level: &Level) -> bool {
        level.holes.iter().all(|&h| self.cheese_at(h).is_some())
    }

    /// One turn: the mouse tries to step `dir`, then the cats walk. Returns `None` when
    /// nothing changed (a bump, a too-heavy wheel, or the game is already over).
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
pub struct World {
    pub level: Arc<Level>,
    pub state: State,
    history: Vec<State>,
    /// Events since the app last drained them.
    pub events: Vec<Event>,
}

impl World {
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

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

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
    pub fn restart(&mut self) {
        let start = self.level.start();
        if self.state != start {
            let prev = std::mem::replace(&mut self.state, start);
            self.history.push(prev);
            self.events.push(Event::Restarted);
        }
    }

    /// Where each cat will step next turn, if it can move (for the paw-print hint).
    pub fn cat_intents(&self) -> Vec<Option<Pos>> {
        let s = &self.state;
        s.cats
            .iter()
            .map(|cat| {
                let ahead = cat.pos.step(cat.dir);
                if s.free(&self.level, ahead) {
                    return Some(ahead);
                }
                let back = cat.pos.step(cat.dir.reverse());
                s.free(&self.level, back).then_some(back)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut w = world("######\n#@$$.#\n######");
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
        let mut w = world("########\n#@b .$ #\n########");
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
