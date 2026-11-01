//! Level text format and the built-in levels (`levels.txt`).
//!
//! ```text
//! ; comment
//! title: First Bite
//! par: 4
//! hint: Push the cheese into the mouse hole!
//! #####
//! #@$.#
//! #####
//! ```
//!
//! Levels are separated by blank lines. Tiles:
//!
//! | char | tile | on a hole |
//! |------|------|-----------|
//! | `#` | wall | |
//! | ` ` / `-` / `_` | floor | `.` |
//! | `@` | mouse | `+` |
//! | `$` | wedge | `*` |
//! | `o` | wheel | `O` |
//! | `b` | brie | `B` |
//! | `< > ^ v` | cat, facing that way | |

use std::sync::{Arc, OnceLock};

use super::{Cat, Cheese, CheeseKind, Dir, Pos, State, Status};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub title: String,
    /// Moves for three stars (the solver's optimum when it was written down).
    pub par: u32,
    /// Shown in a bubble until the first move.
    pub hint: Option<String>,
    pub w: i32,
    pub h: i32,
    walls: Vec<bool>,
    /// Floor the mouse could ever reach; everything else outside the walls is the void.
    inside: Vec<bool>,
    pub holes: Vec<Pos>,
    pub mouse: Pos,
    pub cheeses: Vec<Cheese>,
    pub cats: Vec<Cat>,
}

impl Level {
    pub fn in_bounds(&self, p: Pos) -> bool {
        (0..self.w).contains(&p.x) && (0..self.h).contains(&p.y)
    }

    fn idx(&self, p: Pos) -> usize {
        (p.y * self.w + p.x) as usize
    }

    /// Walls, and everything off the map.
    pub fn wall(&self, p: Pos) -> bool {
        !self.in_bounds(p) || self.walls[self.idx(p)]
    }

    /// Floor inside the building (as opposed to the void outside the outer walls).
    pub fn inside(&self, p: Pos) -> bool {
        self.in_bounds(p) && self.inside[self.idx(p)]
    }

    pub fn is_hole(&self, p: Pos) -> bool {
        self.holes.contains(&p)
    }

    pub fn start(&self) -> State {
        State {
            mouse: self.mouse,
            facing: Dir::Right,
            momentum: None,
            cheeses: self.cheeses.clone(),
            cats: self.cats.clone(),
            moves: 0,
            pushes: 0,
            status: Status::Playing,
        }
    }

    /// Parse one level (metadata lines, then the grid).
    pub fn parse(src: &str) -> Result<Level, String> {
        let mut title = String::from("Untitled");
        let mut par = 0;
        let mut hint = None;
        let mut rows: Vec<&str> = Vec::new();
        for line in src.lines() {
            let line = line.trim_end();
            if line.starts_with(';') || (rows.is_empty() && line.trim().is_empty()) {
                continue;
            }
            if let Some(t) = line.strip_prefix("title:") {
                title = t.trim().to_string();
            } else if let Some(h) = line.strip_prefix("hint:") {
                hint = Some(h.trim().to_string());
            } else if let Some(p) = line.strip_prefix("par:") {
                par = p
                    .trim()
                    .parse()
                    .map_err(|e| format!("bad par {p:?}: {e}"))?;
            } else {
                rows.push(line);
            }
        }
        while rows.last().is_some_and(|r| r.trim().is_empty()) {
            rows.pop();
        }
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut walls = vec![false; (w * h) as usize];
        let mut holes = Vec::new();
        let mut mouse = None;
        let mut cheeses = Vec::new();
        let mut cats = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let p = Pos::new(x as i32, y as i32);
                let mut cheese = |kind| cheeses.push(Cheese { pos: p, kind });
                match ch {
                    '#' => walls[(p.y * w + p.x) as usize] = true,
                    ' ' | '-' | '_' => {}
                    '.' => holes.push(p),
                    '@' | '+' => {
                        if mouse.replace(p).is_some() {
                            return Err(format!("{title}: more than one mouse"));
                        }
                        if ch == '+' {
                            holes.push(p);
                        }
                    }
                    '$' => cheese(CheeseKind::Wedge),
                    'o' => cheese(CheeseKind::Wheel),
                    'b' => cheese(CheeseKind::Brie),
                    '*' | 'O' | 'B' => {
                        cheese(match ch {
                            '*' => CheeseKind::Wedge,
                            'O' => CheeseKind::Wheel,
                            _ => CheeseKind::Brie,
                        });
                        holes.push(p);
                    }
                    '<' | '>' | '^' | 'v' => {
                        let dir = match ch {
                            '<' => Dir::Left,
                            '>' => Dir::Right,
                            '^' => Dir::Up,
                            _ => Dir::Down,
                        };
                        cats.push(Cat { pos: p, dir });
                    }
                    _ => return Err(format!("{title}: unknown tile {ch:?} at {x},{y}")),
                }
            }
        }
        let mouse = mouse.ok_or_else(|| format!("{title}: no mouse"))?;
        if holes.is_empty() {
            return Err(format!("{title}: no mouse holes"));
        }
        if cheeses.len() < holes.len() {
            return Err(format!(
                "{title}: {} holes but only {} cheeses",
                holes.len(),
                cheeses.len()
            ));
        }

        // Flood-fill from the mouse to tell the inside of the building from the void.
        let mut inside = vec![false; (w * h) as usize];
        let mut stack = vec![mouse];
        while let Some(p) = stack.pop() {
            if !(0..w).contains(&p.x) || !(0..h).contains(&p.y) {
                return Err(format!(
                    "{title}: the outer wall has a gap near {},{}",
                    p.x, p.y
                ));
            }
            let i = (p.y * w + p.x) as usize;
            if walls[i] || inside[i] {
                continue;
            }
            inside[i] = true;
            stack.extend(Dir::ALL.map(|d| p.step(d)));
        }
        let outside = |p: &Pos| !inside[(p.y * w + p.x) as usize];
        if let Some(p) = holes.iter().find(|p| outside(p)) {
            return Err(format!("{title}: hole at {},{} is outside", p.x, p.y));
        }
        if let Some(c) = cheeses.iter().find(|c| outside(&c.pos)) {
            return Err(format!(
                "{title}: cheese at {},{} is outside",
                c.pos.x, c.pos.y
            ));
        }

        Ok(Level {
            title,
            par,
            hint,
            w,
            h,
            walls,
            inside,
            holes,
            mouse,
            cheeses,
            cats,
        })
    }

    /// Parse a level pack: levels separated by blank lines.
    pub fn parse_pack(src: &str) -> Result<Vec<Level>, String> {
        let mut out = Vec::new();
        let mut chunk = String::new();
        for line in src.lines().chain(std::iter::once("")) {
            if line.trim().is_empty() {
                if chunk.lines().any(|l| l.contains('#')) {
                    out.push(Level::parse(&chunk)?);
                }
                chunk.clear();
            } else {
                chunk.push_str(line);
                chunk.push('\n');
            }
        }
        Ok(out)
    }
}

/// The built-in levels, in order.
pub fn levels() -> &'static [Arc<Level>] {
    static LEVELS: OnceLock<Vec<Arc<Level>>> = OnceLock::new();
    LEVELS.get_or_init(|| {
        Level::parse_pack(include_str!("levels.txt"))
            .expect("built-in levels parse")
            .into_iter()
            .map(Arc::new)
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tiles() {
        let l = Level::parse("title: T\npar: 3\n######\n#+*ob#\n#  > #\n######").unwrap();
        assert_eq!(l.title, "T");
        assert_eq!(l.par, 3);
        assert_eq!((l.w, l.h), (6, 4));
        assert_eq!(l.mouse, Pos::new(1, 1));
        assert_eq!(l.holes, vec![Pos::new(1, 1), Pos::new(2, 1)]);
        assert_eq!(l.cheeses.len(), 3);
        assert_eq!(l.cats[0].dir, Dir::Right);
        assert!(l.wall(Pos::new(0, 0)) && l.wall(Pos::new(-1, 2)));
        assert!(l.inside(Pos::new(2, 2)) && !l.inside(Pos::new(0, 0)));
    }

    #[test]
    fn rejects_broken_levels() {
        assert!(Level::parse("####\n#$.#\n####").is_err(), "no mouse");
        assert!(Level::parse("####\n#@$#\n####").is_err(), "no hole");
        assert!(
            Level::parse("#####\n#@..$\n#####").is_err(),
            "too few cheeses"
        );
        assert!(Level::parse("####\n#@$.\n####").is_err(), "gap in the wall");
    }

    #[test]
    fn built_in_levels_parse() {
        assert!(levels().len() >= 10);
        for l in levels() {
            assert!(l.par > 0, "{} has no par", l.title);
        }
    }
}
