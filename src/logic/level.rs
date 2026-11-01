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
//!
//! Details of the format, as implemented by [`Level::parse`]:
//!
//! - Metadata lines (`title:`, `par:`, `hint:`) and `;` comments may appear anywhere in a
//!   level, but are conventionally written above the grid. Every other line is a grid row.
//!   A missing `title:` gives "Untitled"; a missing `par:` gives 0, which the built-in level
//!   test rejects.
//! - Rows may have different lengths; the grid is as wide as the longest row and shorter
//!   rows are padded with plain floor. The flood fill (below) is what keeps that padding
//!   harmless.
//! - Cats cannot start on a hole: there is no "cat on a hole" character.
//!
//! A parsed level is validated so the game never has to cope with a broken one:
//!
//! - exactly one mouse, at least one hole, and exactly as many cheeses as holes;
//! - the outer wall is closed. This is checked by flood-filling from the mouse through
//!   everything that is not a wall; if the fill reaches the edge of the grid, there is a
//!   gap. The filled area is kept as the level's "inside", which the renderer uses to tell
//!   floor from the void around the building and the solver uses for its dead-square
//!   analysis;
//! - every hole and every cheese lies inside. Cats are not checked.

use std::sync::{Arc, OnceLock};

use super::{Cat, Cheese, CheeseKind, Dir, Pos, State, Status};

/// One puzzle: its metadata, the static map, and the starting positions of everything that
/// moves. Immutable after parsing; the moving parts are copied into a [`State`] by
/// [`Level::start`].
///
/// The map is stored as row-major `Vec<bool>` grids of `w * h` cells, indexed by
/// `Level::idx`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    /// The level's name. Also the key under which progress is saved, so renaming a level
    /// orphans its saved best score.
    pub title: String,
    /// Moves for three stars (the solver's optimum when it was written down).
    pub par: u32,
    /// Shown in a bubble until the first move.
    pub hint: Option<String>,
    /// Grid width in tiles (the length of the longest row).
    pub w: i32,
    /// Grid height in tiles (the number of grid rows).
    pub h: i32,
    /// `true` for `#` tiles.
    walls: Vec<bool>,
    /// Floor the mouse could ever reach; everything else outside the walls is the void.
    ///
    /// Computed ignoring cheeses and cats, so it is an upper bound on where anything can
    /// go, not a statement about what is reachable in a particular position.
    inside: Vec<bool>,
    /// Every mouse hole, in reading order (top to bottom, left to right).
    pub holes: Vec<Pos>,
    /// The mouse's start tile.
    pub mouse: Pos,
    /// The cheeses' start positions, in reading order.
    pub cheeses: Vec<Cheese>,
    /// The cats' start positions and directions, in reading order. This is also the order
    /// they move in each turn.
    pub cats: Vec<Cat>,
}

impl Level {
    /// Whether `p` lies on the grid.
    pub fn in_bounds(&self, p: Pos) -> bool {
        (0..self.w).contains(&p.x) && (0..self.h).contains(&p.y)
    }

    /// Row-major index of `p` into the `walls` and `inside` grids. Only valid for in-bounds
    /// positions; callers check [`Level::in_bounds`] first.
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

    /// Whether `p` is a mouse hole. A linear scan over the few holes a level has.
    pub fn is_hole(&self, p: Pos) -> bool {
        self.holes.contains(&p)
    }

    /// The state at the beginning of the level: everything at its start position, the
    /// mouse facing right with no momentum, counters at zero.
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
    ///
    /// Blank lines before the grid are skipped and trailing blank rows are dropped; a blank
    /// line in the middle of the grid becomes an all-floor row. Errors are human-readable
    /// strings prefixed with the level's title (as far as it was read), meant for whoever
    /// is editing `levels.txt`. See the module docs for the validation rules.
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
        // Exactly one cheese per hole: a cheese left over at the end reads as unfinished.
        if cheeses.len() != holes.len() {
            return Err(format!(
                "{title}: {} holes but {} cheeses (every cheese needs a hole)",
                holes.len(),
                cheeses.len()
            ));
        }

        // Flood-fill from the mouse to tell the inside of the building from the void. The
        // fill passes through everything but walls, so reaching the grid edge means the
        // mouse could walk off the map: the outer wall has a gap.
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
    ///
    /// The source is cut into chunks at blank lines. A chunk without any `#` holds no grid
    /// (a block of comments, say) and is skipped; every other chunk must parse, and the
    /// first error aborts the whole pack. Because blank lines separate levels, a level's
    /// grid itself cannot contain a fully blank line here.
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
///
/// `levels.txt` is compiled into the binary and parsed once, on first use; later calls
/// return the cached list. Each level sits in an `Arc` so a [`super::World`] can hold on to
/// it cheaply. Panics if the built-in pack does not parse, which the tests catch.
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
        let l = Level::parse("title: T\npar: 3\n######\n#+*ob#\n# .> #\n######").unwrap();
        assert_eq!(l.title, "T");
        assert_eq!(l.par, 3);
        assert_eq!((l.w, l.h), (6, 4));
        assert_eq!(l.mouse, Pos::new(1, 1));
        assert_eq!(
            l.holes,
            vec![Pos::new(1, 1), Pos::new(2, 1), Pos::new(2, 2)]
        );
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
        assert!(
            Level::parse("######\n#@$$.#\n######").is_err(),
            "a spare cheese"
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
