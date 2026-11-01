//! Level design helper: solve every level in a pack and print the optimal move count.
//!
//! `cargo run --release --example solve` checks the built-in levels;
//! `cargo run --release --example solve -- my_levels.txt` checks a file of your own.
//!
//! For each level it prints the par, the solver's optimum (moves and pushes), whether they
//! match, the time taken and the solution in `udlr` notation. A level whose par differs
//! from the optimum, or that is unsolvable or too big to search, makes the process exit
//! with status 1, so the example can gate a level-editing workflow. The state limit here
//! is much higher than in the unit tests, for experimenting with bigger levels.

use cheese::logic::Level;
use cheese::logic::solver::{Outcome, notation, solve};

/// Loads the level pack (from the path argument or the built-in levels), solves each
/// level, prints a report line per level and exits non-zero on any problem.
fn main() {
    let levels = match std::env::args().nth(1) {
        Some(path) => {
            let src = std::fs::read_to_string(&path).expect("read level file");
            Level::parse_pack(&src).unwrap_or_else(|e| panic!("{path}: {e}"))
        }
        None => cheese::logic::levels()
            .iter()
            .map(|l| (**l).clone())
            .collect(),
    };
    let mut ok = true;
    for (i, l) in levels.iter().enumerate() {
        let t = std::time::Instant::now();
        let outcome = solve(l, 20_000_000);
        let secs = t.elapsed().as_secs_f32();
        match outcome {
            Outcome::Solved(path) => {
                // The solver returns moves only; replay them to count the pushes.
                let pushes = {
                    let mut s = l.start();
                    let mut ev = Vec::new();
                    for &d in &path {
                        s = s.step(l, d, &mut ev).expect("solution replays");
                    }
                    s.pushes
                };
                let flag = if path.len() as u32 == l.par {
                    "ok"
                } else {
                    ok = false;
                    "PAR MISMATCH"
                };
                println!(
                    "{:>2}. {:<24} par {:>3}  optimum {:>3} moves / {:>2} pushes  {flag}  ({secs:.2}s)\n    {}",
                    i + 1,
                    l.title,
                    l.par,
                    path.len(),
                    pushes,
                    notation(&path)
                );
            }
            other => {
                ok = false;
                println!("{:>2}. {:<24} {other:?} ({secs:.2}s)", i + 1, l.title);
            }
        }
    }
    if !ok {
        std::process::exit(1);
    }
}
