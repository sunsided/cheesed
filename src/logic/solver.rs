//! Breadth-first solver: finds a solution with the fewest moves. Used by the tests to prove
//! every built-in level is solvable within its par, and by `cargo run --example solve` when
//! designing levels.

use std::collections::{HashMap, VecDeque};

use super::{Dir, Level, Pos, State, Status};

/// The search key: everything that affects the future, minus the counters.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key(Vec<i8>);

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Solved(Vec<Dir>),
    Unsolvable,
    /// Gave up after visiting this many states.
    TooBig(usize),
}

/// Shortest solution (by moves) from the level's start.
pub fn solve(level: &Level, max_states: usize) -> Outcome {
    solve_from(level, level.start(), max_states)
}

/// Tiles a cheese could still be pushed from into some mouse hole, ignoring everything but
/// walls. Pulling backwards from the holes: a cheese at `c` can go `d` if the mouse can
/// stand at `c - d`. Wheels and brie are pickier than wedges, so this over-approximates
/// their live tiles too, which keeps the pruning safe.
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

pub fn solve_from(level: &Level, start: State, max_states: usize) -> Outcome {
    if start.status == Status::Solved {
        return Outcome::Solved(Vec::new());
    }
    // With spare cheeses any one of them may end up as a doorstop, so only prune when
    // every cheese has to end up in a hole.
    let live = live_tiles(level);
    let dead = |s: &State| {
        s.cheeses.len() == level.holes.len()
            && s.cheeses
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
