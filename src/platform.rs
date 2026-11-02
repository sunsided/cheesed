//! The few things that differ between native and the browser: launch options (command-line
//! flags natively, the URL query on the web) and saving progress (a file in the user's data
//! directory natively, `localStorage` on the web).
//!
//! Progress is the best move count per level, keyed by level title so reordering or adding
//! levels keeps old records. Stored as `title<TAB>moves` lines.
//!
//! The platform-specific code lives in a private `imp` module that exists twice, once per
//! target, with the same four functions (`options`, `load`, `save`, `wipe`). Everything
//! above it (parsing, the progress map, serialization) is shared and unit tested natively.
//!
//! Storage is best-effort throughout: a missing data directory, unreadable file or
//! unavailable `localStorage` simply means no saved progress, and failed writes are at most
//! logged. The game never fails to start because of storage.

use std::collections::HashMap;

use crate::logic::Level;

/// Launch options: `--cheese --reset` natively, `?cheese&reset` in the browser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Testing mode: every level unlocked, and the level menu can jump to the episode
    /// intro (`I`), the ending (`E`) and a preview of the farewell screen (`F`). On an
    /// intro card, `I` then flips through the episode cards.
    pub cheese: bool,
    /// Wipe saved progress at launch, to test level progression from scratch.
    pub reset: bool,
}

impl Options {
    /// Parse flags or query pairs (`--cheese`, `cheese`, `cheese=1`, `--reset`).
    ///
    /// The same parser serves both platforms: leading dashes are stripped, so `--cheese`,
    /// `-cheese` and `cheese` are equivalent, and an optional `=value` turns the flag off
    /// when it is `0`, `false` or `no` (any other value, or none, turns it on). Names are
    /// case-insensitive. Unknown names are ignored, so a stray argument or an unrelated
    /// URL parameter never stops the game. When a flag appears more than once, the last
    /// occurrence wins.
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

    /// The options this process was launched with: the command-line arguments natively,
    /// the page's URL query (`?cheese&reset`) on the web.
    pub fn load() -> Self {
        imp::options()
    }
}

/// The player's best solve per level, as loaded from and saved to storage.
///
/// A level counts as solved once it has an entry. The app derives level unlocking from
/// this (see `unlocked` in [`crate::app`]), so no separate "unlocked up to" value is
/// stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Fewest moves per level, keyed by [`Level::title`].
    best: HashMap<String, u32>,
}

impl Progress {
    /// Read the `title<TAB>moves` format written by [`Progress::serialize`].
    ///
    /// The split happens at the last tab, so a title may itself contain tabs. Lines without
    /// a tab or with an unparsable move count are skipped silently, so a damaged file loses
    /// only the damaged lines.
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

    /// One `title<TAB>moves` line per level, sorted so the output is stable and diffable.
    pub fn serialize(&self) -> String {
        let mut lines: Vec<String> = self.best.iter().map(|(t, m)| format!("{t}\t{m}")).collect();
        lines.sort();
        lines.join("\n")
    }

    /// The fewest moves `level` has been solved in, or `None` if it was never solved.
    pub fn best(&self, level: &Level) -> Option<u32> {
        self.best.get(&level.title).copied()
    }

    /// Record a solve. Returns whether it beat the previous best.
    ///
    /// A first solve always counts as beating the previous best, so the return value also
    /// tells the caller when there is something new to save.
    pub fn record(&mut self, level: &Level, moves: u32) -> bool {
        let entry = self.best.entry(level.title.clone()).or_insert(u32::MAX);
        let better = moves < *entry;
        *entry = (*entry).min(moves);
        better
    }

    /// The saved progress, or empty progress if there is none or it cannot be read.
    pub fn load() -> Self {
        imp::load().map(|s| Self::parse(&s)).unwrap_or_default()
    }

    /// Write the whole progress map to storage, replacing what was there. Errors are
    /// logged (natively) or ignored (on the web), never returned.
    pub fn save(&self) {
        imp::save(&self.serialize());
    }

    /// Forget all saved progress, on disk or in the browser.
    pub fn wipe() {
        imp::wipe();
    }
}

/// Things an embedding page may want to react to. On the web each becomes a bubbling
/// `cheese:<name>` DOM event on the game's canvas (with the fields in `event.detail`), and
/// a `postMessage({ type: "cheese:<name>", ... })` to the parent when the game runs in an
/// iframe. Natively they are only logged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostEvent {
    /// The player started a level (`level` is 1-based).
    Start { level: u32 },
    /// The player left the game (Esc in the level menu, after the farewell). On the web the
    /// game then goes dormant: frozen and silent until the player comes back.
    Exit,
    /// The player reached the end of the episode, with this many of the possible stars.
    Complete { stars: u32, max: u32 },
}

impl HostEvent {
    /// The DOM event / message type for this event.
    pub fn name(self) -> &'static str {
        match self {
            HostEvent::Start { .. } => "cheese:start",
            HostEvent::Exit => "cheese:exit",
            HostEvent::Complete { .. } => "cheese:complete",
        }
    }
}

/// Requests from the embedding page. On the web a `cheese:restart` DOM event on the canvas
/// (or, in an iframe, a `postMessage({ type: "cheese:restart" })` from the parent) becomes
/// [`HostCommand::Restart`]. Natively there is no host and none arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCommand {
    /// Start over at the title as if freshly launched, but keep the running loop, the GPU
    /// context, saved progress and the already unlocked audio.
    Restart,
}

impl HostCommand {
    /// The DOM event / message type that asks for a restart.
    pub const RESTART: &str = "cheese:restart";
}

/// Tell the embedding page what happened (see [`HostEvent`]).
pub fn notify(event: HostEvent) {
    imp::notify(event);
}

/// Requests from the embedding page since the last call (see [`HostCommand`]). On the web
/// the first call also installs the listeners.
pub fn commands() -> Vec<HostCommand> {
    imp::commands()
}

/// Native storage (a text file in the user's data directory), options from the command
/// line, and no-op stand-ins for the host-page integration.
#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::PathBuf;

    /// Options from the process arguments, skipping the program name.
    pub fn options() -> super::Options {
        let args: Vec<String> = std::env::args().skip(1).collect();
        super::Options::parse(args.iter().map(String::as_str))
    }

    /// Natively there is no host page: events are only logged.
    pub fn notify(event: super::HostEvent) {
        log::info!("host event: {event:?}");
    }

    /// Natively no host sends commands.
    pub fn commands() -> Vec<super::HostCommand> {
        Vec::new()
    }

    /// `<data dir>/cheese/progress.txt`, where the data directory is the platform's usual
    /// one (for example `~/.local/share` on Linux). `None` if the platform has none.
    fn path() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("cheese").join("progress.txt"))
    }

    /// The progress file's contents, or `None` if it is missing or unreadable.
    pub fn load() -> Option<String> {
        std::fs::read_to_string(path()?).ok()
    }

    /// Delete the progress file. A file that does not exist is not an error; anything
    /// else is logged.
    pub fn wipe() {
        if let Some(p) = path()
            && let Err(e) = std::fs::remove_file(&p)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("reset progress at {}: {e}", p.display());
        }
    }

    /// Write `data` to the progress file, creating its directory first. The file is
    /// overwritten in place (not via a temporary file), and failures are only logged.
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

/// Browser storage (one `localStorage` entry), options from the page URL's query string, and
/// the host-page integration: events on the canvas and `postMessage` to a parent frame, a
/// and a restart request.
#[cfg(target_arch = "wasm32")]
mod imp {
    /// The `localStorage` key holding the serialized progress.
    const KEY: &str = "cheese.progress";

    /// Options from `location.search`: the leading `?` is dropped and the rest split at
    /// `&` into `name` or `name=value` pairs. Values are not URL-decoded.
    pub fn options() -> super::Options {
        let search = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        super::Options::parse(search.trim_start_matches('?').split('&'))
    }

    /// The page's `localStorage`, or `None` where it is unavailable (for example when the
    /// browser blocks storage for the page).
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    /// The stored progress string, if any.
    pub fn load() -> Option<String> {
        storage()?.get_item(KEY).ok()?
    }

    /// Remove the stored progress; errors are ignored.
    pub fn wipe() {
        if let Some(s) = storage() {
            let _ = s.remove_item(KEY);
        }
    }

    /// Store `data` under [`KEY`]; errors (such as a full quota) are ignored.
    pub fn save(data: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(KEY, data);
        }
    }

    use wasm_bindgen::JsValue;

    /// The game's canvas, looked up by its id.
    fn canvas() -> Option<web_sys::Element> {
        web_sys::window()?
            .document()?
            .get_element_by_id(crate::app::CANVAS_ID)
    }

    /// Dispatch the event on the canvas (bubbling, fields in `detail`) and, inside an
    /// iframe, post it to the parent page as `{ type, ...fields }`.
    pub fn notify(event: super::HostEvent) {
        let detail = js_sys::Object::new();
        let set = |k: &str, v: JsValue| {
            let _ = js_sys::Reflect::set(&detail, &JsValue::from_str(k), &v);
        };
        match event {
            super::HostEvent::Start { level } => set("level", level.into()),
            super::HostEvent::Complete { stars, max } => {
                set("stars", stars.into());
                set("max", max.into());
            }
            super::HostEvent::Exit => {}
        }
        if let Some(c) = canvas() {
            let init = web_sys::CustomEventInit::new();
            init.set_bubbles(true);
            init.set_detail(&detail);
            match web_sys::CustomEvent::new_with_event_init_dict(event.name(), &init) {
                Ok(e) => {
                    let _ = c.dispatch_event(&e);
                }
                Err(e) => log::warn!("host event {}: {e:?}", event.name()),
            }
        }
        if let Some(w) = web_sys::window()
            && let Ok(Some(parent)) = w.parent()
            && parent != w
        {
            set("type", JsValue::from_str(event.name()));
            let _ = parent.post_message(&detail, "*");
        }
    }

    thread_local! {
        /// Host requests received since the last [`commands`] call.
        static COMMANDS: std::cell::RefCell<Vec<super::HostCommand>> =
            const { std::cell::RefCell::new(Vec::new()) };
        /// The listeners are installed (once).
        static LISTENING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// Queue a restart request.
    fn queue_restart() {
        COMMANDS.with_borrow_mut(|q| q.push(super::HostCommand::Restart));
    }

    /// Listen for host requests (once): `cheese:restart` on the canvas and, inside an
    /// iframe, a `{ type: "cheese:restart" }` message from the parent window only.
    fn listen() {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;
        if LISTENING.replace(true) {
            return;
        }
        if let Some(c) = canvas() {
            let on_restart =
                Closure::<dyn FnMut(web_sys::CustomEvent)>::new(|_: web_sys::CustomEvent| {
                    queue_restart()
                });
            let _ = c.add_event_listener_with_callback(
                super::HostCommand::RESTART,
                on_restart.as_ref().unchecked_ref(),
            );
            // The listener lives as long as the page: the canvas is never torn down.
            on_restart.forget();
        }
        if let Some(w) = web_sys::window()
            && let Ok(Some(parent)) = w.parent()
            && parent != w
        {
            let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
                move |e: web_sys::MessageEvent| {
                    let from_parent = e.source().is_some_and(|s| js_sys::Object::is(&s, &parent));
                    let kind = js_sys::Reflect::get(&e.data(), &JsValue::from_str("type"))
                        .ok()
                        .and_then(|t| t.as_string());
                    if from_parent && kind.as_deref() == Some(super::HostCommand::RESTART) {
                        queue_restart();
                    }
                },
            );
            let _ =
                w.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref());
            on_message.forget();
        }
    }

    /// Host requests since the last call; installs the listeners on first use.
    pub fn commands() -> Vec<super::HostCommand> {
        listen();
        COMMANDS.with_borrow_mut(std::mem::take)
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
