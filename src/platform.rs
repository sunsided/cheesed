//! Saving progress: a file in the user's data directory natively, `localStorage` on the web.
//!
//! Progress is the best move count per level, keyed by level title so reordering or adding
//! levels keeps old records. Stored as `title<TAB>moves` lines.

use std::collections::HashMap;

use crate::logic::Level;

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
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::PathBuf;

    fn path() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("cheese").join("progress.txt"))
    }

    pub fn load() -> Option<String> {
        std::fs::read_to_string(path()?).ok()
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

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load() -> Option<String> {
        storage()?.get_item(KEY).ok()?
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
