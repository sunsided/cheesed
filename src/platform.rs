//! The few things that differ between native and the browser: launch options (command-line
//! flags natively, the URL query on the web) and saving progress (a file in the user's data
//! directory natively, `localStorage` on the web).
//!
//! Progress is the best move count per level, keyed by level title so reordering or adding
//! levels keeps old records. Stored as `title<TAB>moves` lines.

use std::collections::HashMap;

use crate::logic::Level;

/// Launch options: `--cheese --reset` natively, `?cheese&reset` in the browser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Testing mode: every level unlocked, and the level menu can jump to the episode
    /// intro (`I`) and ending (`E`).
    pub cheese: bool,
    /// Wipe saved progress at launch, to test level progression from scratch.
    pub reset: bool,
}

impl Options {
    /// Parse flags or query pairs (`--cheese`, `cheese`, `cheese=1`, `--reset`).
    pub fn parse<'a>(args: impl IntoIterator<Item = &'a str>) -> Self {
        let mut o = Options::default();
        for arg in args {
            let arg = arg.trim_start_matches('-');
            let (key, value) = arg.split_once('=').unwrap_or((arg, ""));
            let on = !matches!(value, "0" | "false" | "no");
            if key.eq_ignore_ascii_case("cheese") {
                o.cheese = on;
            } else if key.eq_ignore_ascii_case("reset") {
                o.reset = on;
            }
        }
        o
    }

    pub fn load() -> Self {
        imp::options()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    best: HashMap<String, u32>,
}

impl Progress {
    pub fn parse(src: &str) -> Self {
        let best = src
            .lines()
            .filter_map(|l| {
                let (title, moves) = l.rsplit_once('\t')?;
                Some((title.to_string(), moves.trim().parse().ok()?))
            })
            .collect();
        Self { best }
    }

    pub fn serialize(&self) -> String {
        let mut lines: Vec<String> = self.best.iter().map(|(t, m)| format!("{t}\t{m}")).collect();
        lines.sort();
        lines.join("\n")
    }

    pub fn best(&self, level: &Level) -> Option<u32> {
        self.best.get(&level.title).copied()
    }

    /// Record a solve. Returns whether it beat the previous best.
    pub fn record(&mut self, level: &Level, moves: u32) -> bool {
        let entry = self.best.entry(level.title.clone()).or_insert(u32::MAX);
        let better = moves < *entry;
        *entry = (*entry).min(moves);
        better
    }

    pub fn load() -> Self {
        imp::load().map(|s| Self::parse(&s)).unwrap_or_default()
    }

    pub fn save(&self) {
        imp::save(&self.serialize());
    }

    /// Forget all saved progress, on disk or in the browser.
    pub fn wipe() {
        imp::wipe();
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::PathBuf;

    pub fn options() -> super::Options {
        let args: Vec<String> = std::env::args().skip(1).collect();
        super::Options::parse(args.iter().map(String::as_str))
    }

    fn path() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("cheese").join("progress.txt"))
    }

    pub fn load() -> Option<String> {
        std::fs::read_to_string(path()?).ok()
    }

    pub fn wipe() {
        if let Some(p) = path()
            && let Err(e) = std::fs::remove_file(&p)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("reset progress at {}: {e}", p.display());
        }
    }

    pub fn save(data: &str) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, data) {
            log::warn!("save progress to {}: {e}", p.display());
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    const KEY: &str = "cheese.progress";

    pub fn options() -> super::Options {
        let search = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        super::Options::parse(search.trim_start_matches('?').split('&'))
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load() -> Option<String> {
        storage()?.get_item(KEY).ok()?
    }

    pub fn wipe() {
        if let Some(s) = storage() {
            let _ = s.remove_item(KEY);
        }
    }

    pub fn save(data: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(KEY, data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_parse() {
        assert!(Options::parse(["--cheese"]).cheese);
        assert!(Options::parse(["foo=1", "cheese"]).cheese);
        assert!(!Options::parse(["cheese=0"]).cheese);
        assert!(!Options::parse(["--cheesy"]).cheese);
        assert!(!Options::parse([]).cheese);
        let both = Options::parse(["--cheese", "--reset"]);
        assert!(both.cheese && both.reset);
        assert!(!Options::parse(["--cheese"]).reset);
    }

    #[test]
    fn round_trips_and_keeps_the_best() {
        let a = Level::parse("title: A\n####\n#@$.#\n#####").unwrap();
        let b = Level::parse("title: B b\n####\n#@$.#\n#####").unwrap();
        let mut p = Progress::default();
        assert!(p.record(&a, 10));
        assert!(!p.record(&a, 12));
        assert!(p.record(&a, 8));
        p.record(&b, 3);
        let q = Progress::parse(&p.serialize());
        assert_eq!(q.best(&a), Some(8));
        assert_eq!(q.best(&b), Some(3));
        assert_eq!(Progress::parse("garbage\nx\tnope").best(&a), None);
    }
}
