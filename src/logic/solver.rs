//! Breadth-first solver: finds a solution with the fewest moves. Used by the tests to prove
//! every built-in level is solvable within its par, and by `cargo run --example solve` when
//! designing levels.
//!
//! # How it works
//!
//! Every move costs exactly one, so plain breadth-first search over [`State`]s finds a
//! shortest solution: states are expanded in order of distance from the start, and the
//! first solved state generated is at minimal depth. The search reuses [`State::step`]
//! directly, so the solver can never disagree with the game about the rules.
//!
//! Two things keep the search space manageable:
//!
//! - **Deduplication by `Key`.** Many move sequences lead to the same position. The key
//!   keeps only what influences future moves (see `key`), so states that differ only in
//!   move counters or the cosmetic facing direction are visited once.
//! - **Dead-square pruning.** A cheese pushed onto a tile from which no sequence of pushes
//!   could ever bring it to a hole makes the whole state hopeless (every cheese must end in
//!   a hole, since there are exactly as many cheeses as holes). `live_tiles` precomputes
//!   those tiles once per level and the search drops any state with a cheese on a dead
//!   one. This is the classic Sokoban "simple deadlock" check.
//!
//! States where a cat caught the mouse are dropped as well; they are terminal and never
//! lead to a solution.
//!
//! The visited map doubles as the parent-pointer tree used to rebuild the path, so memory
//! grows with the number of distinct states. `max_states` caps it.

use std::collections::{HashMap, VecDeque};

use super::{Dir, Level, Pos, State, Status};

/// The search key: everything that affects the future, minus the counters.
///
/// A flat byte vector, built by [`key`]. Coordinates are stored as `i8`, which assumes
/// levels stay smaller than 128 tiles in each direction.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key(Vec<i8>);

/// Builds the deduplication key for `s`.
///
/// Layout: mouse x, mouse y, momentum (direction discriminant or -1 for none), then one
/// `(kind, x, y)` triple per cheese, then one `(x, y, dir)` triple per cat.
///
/// What is deliberately left out, and why that is safe:
///
/// - `moves` and `pushes`: BFS reaches each key first at its smallest move count, so later
///   arrivals with larger counts are worse and can be discarded.
/// - `facing`: purely visual, never read by the rules.
/// - `status`: only `Playing` states are ever queued.
///
/// Momentum has to stay in: whether a wheel can be pushed depends on it, so the same
/// layout with different momentum can have different futures.
///
/// Cheeses are sorted, because two cheeses of the same kind swapped between two tiles are
/// the same position for the rules. Cats are not sorted: they move in index order, so the
/// order is part of the state.
fn key(s: &State) -> Key {
    let mut k = Vec::with_capacity(3 + s.cheeses.len() * 2 + s.cats.len() * 3);
    k.push(s.mouse.x as i8);
    k.push(s.mouse.y as i8);
    k.push(s.momentum.map_or(-1, |d| d as i8));
    // Cheeses of the same kind are interchangeable: sort so their order doesn't matter.
    let mut cheeses: Vec<_> = s
        .cheeses
        .iter()
        .map(|c| (c.kind as i8, c.pos.x as i8, c.pos.y as i8))
        .collect();
    cheeses.sort_unstable();
    for (kind, x, y) in cheeses {
        k.extend([kind, x, y]);
    }
    for c in &s.cats {
        k.extend([c.pos.x as i8, c.pos.y as i8, c.dir as i8]);
    }
    Key(k)
}

/// The result of a search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A shortest solution, as the sequence of moves from the start state. Empty if the
    /// start state was already solved.
    Solved(Vec<Dir>),
    /// The search space was exhausted without reaching a solved state.
    Unsolvable,
    /// Gave up after visiting this many states.
    TooBig(usize),
}

/// Shortest solution (by moves) from the level's start.
///
/// `max_states` bounds the number of distinct states recorded before the search gives up
/// with [`Outcome::TooBig`].
pub fn solve(level: &Level, max_states: usize) -> Outcome {
    solve_from(level, level.start(), max_states)
}

/// Tiles a cheese could still be pushed from into some mouse hole, ignoring everything but
/// walls. Pulling backwards from the holes: a cheese at `c` can go `d` if the mouse can
/// stand at `c - d`. Wheels and brie are pickier than wedges, so this over-approximates
/// their live tiles too, which keeps the pruning safe.
///
/// Returns a row-major grid of `w * h` flags; `true` means "live".
///
/// The computation is a reverse search ("pulling" a cheese away from the holes). Start
/// with every hole marked live. For a live tile `to` and each direction `d`, a cheese at
/// `from = to - d` could be pushed onto `to` by a mouse standing at `from - d`. If both of
/// those tiles are inside the building, `from` is live too. Repeat until nothing changes.
///
/// Why the pruning is safe: the relaxation ignores cheeses, cats, momentum and the mouse's
/// ability to reach the push position, all of which can only forbid pushes, never allow new
/// ones. So any tile from which a real sequence of pushes reaches a hole is marked live.
/// Brie needs a moment's thought, since one push can carry it many tiles: a slide from `c`
/// to a hole `h` crosses only free (hence inside) tiles, so every tile on the line from `c`
/// to `h` has an inside tile behind it (the previous tile on the line, or the mouse at
/// `c - d` for `c` itself), and the reverse search walks back along that line from `h`
/// to `c`. Hence a cheese on a tile that is not live can never be holed, and dropping such
/// states never loses a solution.
fn live_tiles(level: &Level) -> Vec<bool> {
    let idx = |p: Pos| (p.y * level.w + p.x) as usize;
    let mut live = vec![false; (level.w * level.h) as usize];
    let mut stack: Vec<Pos> = level.holes.clone();
    for &h in &level.holes {
        live[idx(h)] = true;
    }
    while let Some(to) = stack.pop() {
        for d in Dir::ALL {
            let from = to.step(d.reverse());
            let stand = from.step(d.reverse());
            if level.inside(from) && level.inside(stand) && !live[idx(from)] {
                live[idx(from)] = true;
                stack.push(from);
            }
        }
    }
    live
}

/// Shortest solution (by moves) from an arbitrary `start` state of `level`.
///
/// Breadth-first: states are taken from a FIFO queue, and each of the four moves is tried
/// with [`State::step`]. A successor is discarded if the move was refused, if it got the
/// mouse caught, if a cheese is on a dead tile (see `live_tiles`), or if its `Key` was
/// seen before. The goal test runs when a state is generated rather than when it is
/// dequeued, which is still optimal because all moves cost the same and saves expanding
/// a whole extra layer.
///
/// `seen` maps each key to its parent key and the move that led there (`None` for the
/// start). On success the path is rebuilt by following parents back to the start and
/// reversing. The `max_states` limit is checked against the size of `seen` after each new
/// state is recorded.
pub fn solve_from(level: &Level, start: State, max_states: usize) -> Outcome {
    if start.status == Status::Solved {
        return Outcome::Solved(Vec::new());
    }
    // Every cheese has to end up in a hole, so one stuck where it never can is a dead end.
    let live = live_tiles(level);
    let dead = |s: &State| {
        s.cheeses
            .iter()
            .any(|c| !live[(c.pos.y * level.w + c.pos.x) as usize])
    };
    // Parent links: key -> (parent key, move).
    let mut seen: HashMap<Key, Option<(Key, Dir)>> = HashMap::new();
    let mut queue = VecDeque::new();
    seen.insert(key(&start), None);
    queue.push_back(start);
    let mut events = Vec::new();
    while let Some(s) = queue.pop_front() {
        let sk = key(&s);
        for d in Dir::ALL {
            events.clear();
            let Some(next) = s.step(level, d, &mut events) else {
                continue;
            };
            if next.status == Status::Caught || dead(&next) {
                continue;
            }
            let nk = key(&next);
            if seen.contains_key(&nk) {
                continue;
            }
            seen.insert(nk.clone(), Some((sk.clone(), d)));
            if next.status == Status::Solved {
                let mut path = Vec::new();
                let mut cur = nk;
                while let Some(Some((parent, d))) = seen.get(&cur) {
                    path.push(*d);
                    cur = parent.clone();
                }
                path.reverse();
                return Outcome::Solved(path);
            }
            if seen.len() >= max_states {
                return Outcome::TooBig(seen.len());
            }
            queue.push_back(next);
        }
    }
    Outcome::Unsolvable
}

/// A solution as `udlr` letters.
pub fn notation(path: &[Dir]) -> String {
    path.iter().map(|d| d.letter()).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::logic::{World, levels};

    /// Generous: the built-in levels are small.
    const LIMIT: usize = 3_000_000;

    #[test]
    fn solves_a_trivial_level() {
        let l = Level::parse("######\n#@ $.#\n######").unwrap();
        assert_eq!(
            solve(&l, 1000),
            Outcome::Solved(vec![Dir::Right, Dir::Right])
        );
    }

    #[test]
    fn detects_a_dead_level() {
        let l = Level::parse("######\n#@  .#\n#$####\n######").unwrap();
        assert_eq!(solve(&l, 1000), Outcome::Unsolvable);
    }

    /// Every built-in level is solvable, its par is exactly the optimum, and replaying the
    /// solution through `World` solves it too.
    #[test]
    fn every_level_is_solvable_at_par() {
        for (i, level) in levels().iter().enumerate() {
            let n = i + 1;
            match solve(level, LIMIT) {
                Outcome::Solved(path) => {
                    assert_eq!(
                        path.len() as u32,
                        level.par,
                        "level {n} {:?}: par is {} but the optimum is {} ({})",
                        level.title,
                        level.par,
                        path.len(),
                        notation(&path)
                    );
                    let mut w = World::new(Arc::clone(level));
                    for d in path {
                        assert!(w.step(d));
                    }
                    assert_eq!(w.state.status, Status::Solved);
                }
                other => panic!("level {n} {:?}: {other:?}", level.title),
            }
        }
    }
}
