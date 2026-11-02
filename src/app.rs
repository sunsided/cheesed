//! The game-toolkit glue: input, audio device, and presenting the framebuffer.
//!
//! [`Cheese`] implements the toolkit's `Game` trait. Each frame, `update` reads the raw
//! input once into an `Intent`, then runs the handler for the current `Mode`;
//! `render` draws that mode into the CPU framebuffer ([`Screen`]), uploads it as a texture
//! and draws it scaled into the window.
//!
//! # Modes
//!
//! The game is a small state machine. Transitions (all set by the `update_*` methods or
//! the `goto_*`/`show_*`/`start_level` helpers):
//!
//! ```text
//! Title --any key--> Select
//! Select --confirm on level 1--> Intro(1) --confirm--> Play(level 1)
//! Select --confirm on another unlocked level--> Play
//! Select --confirm on a locked episode--> Intro(n) --confirm--> Select with the order form
//! Select --back--> Farewell --key or timeout--> quit (native) / Title (web)
//! Play --solved, confirm--> Play(next level), or End after the last level
//! Play --back--> Select
//! End --confirm--> Onward --confirm--> Select with the order form
//! End / Onward / Intro --back--> Select
//! ```
//!
//! In cheese mode ([`Options::cheese`]) the level menu also jumps straight to `Intro` (`I`),
//! `End` (`E`) and a `Farewell` preview (`F`) that returns to the menu instead of quitting.
//!
//! # The shareware flow
//!
//! The game presents itself as episode one of a 90s shareware release. After the real
//! levels, the level menu lists further episodes that are always locked. Choosing one
//! shows its title card; confirming the card opens a pretend order form on the level menu
//! instead of a level. The ending leads the same way, via the "Onward!" teaser. Nothing is
//! actually for sale.
//!
//! # Timers
//!
//! All times are in seconds of `dt`. `end_t` is shared by the modes that need a "time in
//! this screen" clock (`Intro`, `End`, `Onward`, `Farewell`, and the solved/caught panels
//! in `Play`); every transition into one of them resets it. Each of those screens ignores
//! confirmation for a short moment after it appears, so a key press meant for the previous
//! screen, or a held key, does not skip it unseen.
//!
//! # Audio
//!
//! Sound effects are pre-rendered PCM buffers ([`sfx::Bank`]) played fire-and-forget.
//! Music is a tracker song synthesized on the audio thread (`Music`); one of three tunes
//! is chosen from the mode every frame, and its volume is turned down to silence while
//! paused or after defeat. `M` mutes everything through the master volume.

use std::sync::Arc;

use game_toolkit::prelude::*;

use crate::gfx::{H, Screen, W};
use crate::logic::{CheeseKind, Dir, Event, Level, Pos, Status, World, levels, stars};
use crate::platform::{self, HostCommand, HostEvent, Options, Progress};
use crate::render::anim::Anim;
use crate::render::fx::Fx;
use crate::render::title;
use crate::render::{self, PlayView, Renderer, SelectView, Trail};
use crate::sound::{SAMPLE_RATE, music, sfx, songs};

/// The `id` of the HTML `<canvas>` the game draws into on the web; the page must contain a
/// canvas with this id.
pub const CANVAS_ID: &str = "cheese";
/// Overall loudness of sound effects, multiplied into every per-effect volume so effects
/// sit at a sensible level against the music.
const SFX_VOLUME: f32 = 0.6;
/// Volume of the short blip when the menu cursor moves. Full volume: it lasts only 30 ms.
const MENU_VOLUME: f32 = 1.0;
/// Volume of the confirm arpeggio (picking an entry, leaving the title). Its raw level is
/// about 1.6 times the menu blip's, so 0.6 lands both at about the same loudness.
const CONFIRM_VOLUME: f32 = 0.6;
/// Music volume for the title/menu tune. The volumes are matched to the old tracker's
/// loudness: synthie's mix is less squashed, so it needs a higher volume for the same level.
const ROCK_VOLUME: f32 = 0.66;
/// Music volume for the in-game music box tune, which is quieter in character and so
/// played a little louder.
const MINUET_VOLUME: f32 = 1.57;
/// Music volume for the extro (same character as the title rock).
const EXTRO_VOLUME: f32 = 0.63;
/// Holding a direction: first repeat after this long, then every `REPEAT_EVERY`.
/// In seconds; the gap lets a quick tap produce exactly one move.
const REPEAT_DELAY: f32 = 0.24;
/// Seconds between repeated moves while a direction stays held, after the first
/// `REPEAT_DELAY`.
const REPEAT_EVERY: f32 = 0.12;

/// Which screen the game is on. See the module docs for the transitions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// The title screen with its lightning and thunder; any key continues.
    Title,
    /// The level menu. Also hosts the locked episodes and the order form.
    Select,
    /// The episode's title card, before its first level.
    Intro,
    /// A level is being played (including its paused, solved and caught overlays).
    Play,
    /// The episode ending, after the last level is solved, with the star tally.
    End,
    /// "Onward!": the teaser for episode two, after the ending.
    Onward,
    /// "Farewell!": shown when leaving from the title.
    Farewell,
}

/// Where the framebuffer sits in the window.
///
/// The 320x200 image is scaled uniformly and centred, with black borders filling the rest.
/// The same mapping is used in both directions: [`Cheese::render`] draws the texture with
/// it, and [`Viewport::to_fb`] maps pointer and touch positions back to framebuffer pixels.
#[derive(Clone, Copy, Debug)]
struct Viewport {
    /// Left edge of the image in window pixels.
    x: f32,
    /// Top edge of the image in window pixels.
    y: f32,
    /// Window pixels per framebuffer pixel.
    scale: f32,
}

impl Viewport {
    /// The largest scale at which the framebuffer fits a `win_w` x `win_h` window, centred.
    /// Whole-number scales are used whenever the window is at least 320x200; the offsets
    /// are floored so the image starts on a whole window pixel.
    fn fit(win_w: u32, win_h: u32) -> Self {
        let s = (win_w as f32 / W as f32).min(win_h as f32 / H as f32);
        // Integer scaling keeps every pixel the same size; below 1x there's no choice.
        let scale = if s >= 1.0 { s.floor() } else { s };
        Self {
            x: ((win_w as f32 - W as f32 * scale) / 2.0).floor(),
            y: ((win_h as f32 - H as f32 * scale) / 2.0).floor(),
            scale,
        }
    }

    /// Converts a window position to framebuffer pixel coordinates. Positions on the black
    /// borders map outside `0..W` / `0..H`; callers filter those out.
    fn to_fb(self, x: f32, y: f32) -> (i32, i32) {
        (
            ((x - self.x) / self.scale).floor() as i32,
            ((y - self.y) / self.scale).floor() as i32,
        )
    }
}

/// The three pieces of music. Which one plays is derived from the [`Mode`] every frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tune {
    /// The title and menus: heroic chiptune rock.
    Rock,
    /// The game itself: a music box. Deflating, on purpose.
    Minuet,
    /// The episode ending: the epic, back and in major.
    Extro,
}

/// The music: one song at a time, played live on synthie through kira.
///
/// The tracker songs in [`songs`] are converted to synthie songs by [`music::to_synthie`]
/// and handed to `Audio::play_synth`, which sequences them sample-accurately on the audio
/// thread. The game thread keeps only a [`SynthHandle`] to stop the song or change its
/// volume.
#[derive(Default)]
struct Music {
    /// The playing song, if one could be started.
    handle: Option<SynthHandle>,
    /// The tune last asked for. Set even when there is no audio device, so `play` does not
    /// retry every frame.
    tune: Option<Tune>,
    /// The current stream volume, also applied to the next stream that starts.
    volume: f32,
}

impl Music {
    /// Switch to `tune` unless it's already playing.
    ///
    /// Switching stops the old stream and starts the new song from the beginning. The new
    /// stream is created with the current `volume`, so calling [`Music::set_volume`] first
    /// (as `update` does) avoids a frame at the wrong loudness. Without an audio device this
    /// only remembers the tune.
    fn play(&mut self, audio: &mut Option<Audio>, tune: Tune) {
        if self.tune == Some(tune) {
            return;
        }
        if let Some(h) = self.handle.take() {
            h.stop();
        }
        self.tune = Some(tune);
        let Some(audio) = audio else { return };
        let song = match tune {
            Tune::Rock => songs::title(),
            Tune::Minuet => songs::game(),
            Tune::Extro => songs::extro(),
        };
        // Played on synthie (game-toolkit's `synth` feature), converted from the tracker song.
        // Deferred: on the web it waits for the first key press or tap. Browsers refuse to
        // start audio before a user gesture, so the toolkit holds the stream until then.
        //
        // Voice normalization is off: synthie's default divides each channel by its four
        // voices, so a lone note would play at a quarter of its level. Off matches the
        // offline renders (`Synth::render_song`) the music volumes are tuned against.
        let data = synthie::kira::SynthSoundData::new()
            .with_sequencer(synth::SongPlayer::new(music::to_synthie(&song)))
            .with_voice_normalization(synthie::prelude::VoiceNormalization::Off);
        match audio.play_synth_sound(data, PlayOptions::default().volume(self.volume).deferred()) {
            Ok(h) => self.handle = Some(h),
            Err(e) => log::warn!("music: {e}"),
        }
    }

    /// Set the music volume. Called every frame, so it only talks to the audio stream when
    /// the value actually changed.
    fn set_volume(&mut self, v: f32) {
        if (v - self.volume).abs() > 1e-3 {
            self.volume = v;
            if let Some(h) = &self.handle {
                h.set_volume(v);
            }
        }
    }
}

/// What the player asked for this frame, from keyboard, gamepad or pointer.
///
/// Built once per frame by [`Cheese::read_intent`], so the mode handlers never look at
/// devices directly (except the title screen and the cheese-mode debug keys). The boolean
/// flags are edge-triggered: they are true only on the frame the key or button went down.
#[derive(Default)]
struct Intent {
    /// A move or menu step: a fresh press, an auto-repeat of a held direction, or the
    /// analog stick crossing into a new direction. Arrow keys, WASD, vi keys (HJKL) and
    /// the d-pad all count.
    dir: Option<Dir>,
    /// Space, Enter or the gamepad's south button (A on Xbox layouts).
    confirm: bool,
    /// Escape or the gamepad's Select button.
    back: bool,
    /// Z, U, Backspace or the gamepad's east button.
    undo: bool,
    /// R or the gamepad's north button.
    restart: bool,
    /// P or the gamepad's Start button.
    pause: bool,
    /// A tap or click, in framebuffer pixels.
    ///
    /// Only the first tap or click this frame that lands on the framebuffer is kept;
    /// touches on the black borders are ignored.
    tap: Option<(i32, i32)>,
}

/// The whole game: the [`Game`] implementation that `main.rs` hands to the toolkit with
/// `run::<Cheese>(config())`.
///
/// Holds both the per-level play state and the state of the surrounding screens; which
/// fields matter depends on `Mode`.
pub struct Cheese {
    /// The 320x200 framebuffer everything is drawn into.
    screen: Screen,
    /// The framebuffer converted to RGBA bytes, reused every frame for the texture upload.
    rgba: Vec<u8>,
    /// The GPU texture showing the framebuffer.
    tex: TextureId,
    /// Draws each mode's screen into `screen`; holds the decoded art.
    renderer: Renderer,
    /// Where the framebuffer sits in the window, recomputed on resize and every frame.
    viewport: Viewport,
    /// The current screen.
    mode: Mode,
    /// Seconds since launch, for animations that run regardless of mode (menu and sprite
    /// animation, screen shake, the age of brie trails).
    t: f32,
    /// Launch options (cheese mode, reset).
    options: Options,
    /// Which episode's intro card is showing (1-based).
    intro_episode: usize,
    /// The farewell is a cheese-mode preview: return to the level menu instead of quitting.
    farewell_preview: bool,
    /// Audio was running last frame. On the web the toolkit starts audio during the very
    /// press that unlocks it, so "is it running now" alone can't tell an unlocking press
    /// from a later one; comparing with the previous frame can.
    sound_was_on: bool,
    /// A title press was already spent on unlocking audio, so the next one goes on even if
    /// the browser never let the sound start (a muted tab, a strict autoplay policy).
    unlock_press_spent: bool,
    /// Web only: the player left (after the farewell) and the page got `cheese:exit`. The
    /// game stays frozen and silent behind a "come back" note until the player presses
    /// something, then starts over at the title.
    dormant: bool,
    /// Audio is muted because the host has the game hidden (dormant, or its canvas detached
    /// from the page). Separate from the player's own mute (`M`).
    host_muted: bool,
    /// Seconds on the title screen, which times the lightning and its thunder.
    title_t: f32,
    /// The background music stream.
    music: Music,
    /// The built-in levels.
    levels: &'static [Arc<Level>],
    /// Best solves, saved whenever a record improves.
    progress: Progress,
    /// Level-select cursor; also the level being played.
    ///
    /// Values from `levels.len()` up point at the locked episode entries after the real
    /// levels: `levels.len()` is episode two, `levels.len() + 1` episode three, and so on.
    cursor: usize,
    /// The level being played. Outside `Play` it holds the last level played (or level one
    /// at launch) and is ignored.
    world: World,
    /// The tween for the last turn, while it is still playing. `None` when the board is at
    /// rest; the idle squeak waits for that.
    anim: Option<Anim>,
    /// Particles and screen shake.
    fx: Fx,
    /// Crumb bursts waiting for the cheese to arrive: (seconds left, tile, kind).
    ///
    /// The logic reports a plop at once, but the cheese is still sliding on screen, so the
    /// burst is delayed by the turn's animation length.
    pending: Vec<(f32, Pos, CheeseKind)>,
    /// Seconds spent on the current "end-ish" screen: the intro card, ending, onward,
    /// farewell, or the solved/caught panel during play. Reset on entering each of them;
    /// the renderer uses it to fade and animate those screens.
    end_t: f32,
    /// Play is paused (by the player or by losing window focus). Freezes the level, its
    /// effects and the music.
    paused: bool,
    /// Greasy streaks left by sliding brie.
    trails: Vec<Trail>,
    /// The trails as they were before each entry in the world's undo history, kept in step
    /// with it: pushed whenever the history grows (a step or a restart), popped and restored
    /// on undo. That way undoing a restart brings the streaks back, and undoing a slide
    /// wipes exactly its own streak.
    trail_history: Vec<Vec<Trail>>,
    /// The level menu is showing the (pretend) order form.
    order: bool,
    /// Seconds the player hasn't touched anything during a level.
    idle_t: f32,
    /// At this much idle time the mouse squeaks.
    ///
    /// Compared against `idle_t`, so after a squeak it is pushed further out (8 to 14
    /// seconds past the current idle time), and any input sets it to 6 to 11 seconds.
    next_squeak: f32,
    /// Seconds since the last squeak, while its pop-up shows.
    squeak_age: Option<f32>,
    /// The direction being held and the time until it repeats.
    held: Option<(Dir, f32)>,
    /// Last analog stick direction, for edge-triggering.
    ///
    /// The stick produces a move only when its direction changes to a new non-neutral one,
    /// so holding it does not repeat; the player returns to centre to move again.
    stick: Option<Dir>,
    /// Every sound effect, synthesized once at startup.
    sfx: sfx::Bank,
    /// `M` toggles this; it zeroes the master volume, silencing music and effects alike.
    muted: bool,
    /// `C` toggles the CRT scanline overlay drawn in [`Cheese::render`].
    scanlines: bool,
    /// A short message drawn over every screen: (text, seconds left). A new toast
    /// replaces the old one.
    toast: Option<(String, f32)>,
}

impl Cheese {
    /// Play a sound effect once at `volume` (scaled by [`SFX_VOLUME`]). Effects overlap
    /// freely; there is no voice limit or priority. Silently does nothing without an audio
    /// device.
    fn sound(&self, audio: &mut Option<Audio>, pcm: &[f32], volume: f32) {
        if let Some(a) = audio {
            // Before the first gesture on the web this fails; a missed blip is fine.
            let _ = a.play_samples_with(
                pcm,
                SAMPLE_RATE,
                PlayOptions::default().volume(volume * SFX_VOLUME),
            );
        }
    }

    /// The best move count of every built-in level, in level order (`None` if unsolved),
    /// for the level menu.
    fn best(&self) -> Vec<Option<u32>> {
        self.levels.iter().map(|l| self.progress.best(l)).collect()
    }

    /// Levels `0..n` are playable: everything up to and including the first unsolved one.
    ///
    /// Unlocking is derived from saved progress on every call rather than stored. A gap
    /// (a later level solved but an earlier one not, which can happen when levels are
    /// added or reordered) locks everything after the first unsolved level. In cheese mode
    /// every level is open.
    fn unlocked(&self) -> usize {
        if self.options.cheese {
            return self.levels.len();
        }
        self.levels
            .iter()
            .position(|l| self.progress.best(l).is_none())
            .map_or(self.levels.len(), |i| i + 1)
    }

    /// Where the level-menu cursor starts when coming from the title: the last unlocked
    /// level, which is the first unsolved one (or the last level once all are solved, or
    /// the last level in cheese mode).
    fn first_unsolved(&self) -> usize {
        self.unlocked().min(self.levels.len()) - 1
    }

    /// Read this frame's input into an [`Intent`], updating the auto-repeat and stick
    /// state on the way. `dt` advances the repeat timer.
    ///
    /// Directions, in priority order:
    ///
    /// 1. A fresh press of any direction key or d-pad button. It also (re)arms auto-repeat
    ///    for that direction with [`REPEAT_DELAY`].
    /// 2. Otherwise, if the remembered direction is still held, its timer counts down and
    ///    fires a repeat on reaching zero, then re-arms with [`REPEAT_EVERY`]. Releasing it
    ///    clears the repeat. Only the most recently pressed direction repeats.
    /// 3. The left stick, edge-triggered (see `stick`). It is checked last and overrides
    ///    the keys on the frame it changes direction.
    ///
    /// The stick needs more than half deflection; the dominant axis wins, with ties going
    /// to the vertical axis. Positive Y is up, following the gamepad convention.
    ///
    /// Taps come from new touches and left mouse clicks, converted to framebuffer pixels
    /// through the current [`Viewport`].
    fn read_intent(&mut self, input: &Input, dt: f32) -> Intent {
        let pad = input.first_gamepad();
        let btn = |b: Button| pad.is_some_and(|p| p.button_pressed(b));
        let held = |b: Button| pad.is_some_and(|p| p.button_held(b));
        let key = |k: Key| input.key_pressed(k);
        let any = |ks: &[Key]| ks.iter().any(|&k| input.key_pressed(k));
        let any_held = |ks: &[Key]| ks.iter().any(|&k| input.key_held(k));

        const DIRS: [(Dir, [Key; 3], Button); 4] = [
            (
                Dir::Up,
                [Key::ArrowUp, Key::KeyW, Key::KeyK],
                Button::DPadUp,
            ),
            (
                Dir::Down,
                [Key::ArrowDown, Key::KeyS, Key::KeyJ],
                Button::DPadDown,
            ),
            (
                Dir::Left,
                [Key::ArrowLeft, Key::KeyA, Key::KeyH],
                Button::DPadLeft,
            ),
            (
                Dir::Right,
                [Key::ArrowRight, Key::KeyD, Key::KeyL],
                Button::DPadRight,
            ),
        ];

        let mut intent = Intent::default();
        // Fresh presses win; otherwise a held direction auto-repeats.
        if let Some((d, ..)) = DIRS.iter().find(|(_, keys, b)| any(keys) || btn(*b)) {
            intent.dir = Some(*d);
            self.held = Some((*d, REPEAT_DELAY));
        } else if let Some((d, left)) = self.held {
            let (_, keys, b) = DIRS.iter().find(|(dd, ..)| *dd == d).expect("all dirs");
            if any_held(keys) || held(*b) {
                let left = left - dt;
                if left <= 0.0 {
                    intent.dir = Some(d);
                    self.held = Some((d, REPEAT_EVERY));
                } else {
                    self.held = Some((d, left));
                }
            } else {
                self.held = None;
            }
        }
        // The analog stick, edge-triggered.
        if let Some(p) = pad {
            let (x, y) = (p.axis(Axis::LeftStickX), p.axis(Axis::LeftStickY));
            let dir = if x.abs() > 0.5 && x.abs() > y.abs() {
                Some(if x > 0.0 { Dir::Right } else { Dir::Left })
            } else if y.abs() > 0.5 {
                Some(if y > 0.0 { Dir::Up } else { Dir::Down })
            } else {
                None
            };
            if dir != self.stick && dir.is_some() {
                intent.dir = dir;
            }
            self.stick = dir;
        }

        intent.confirm = key(Key::Space) || key(Key::Enter) || btn(Button::South);
        intent.back = key(Key::Escape) || btn(Button::Select);
        intent.undo = any(&[Key::KeyZ, Key::KeyU, Key::Backspace]) || btn(Button::East);
        intent.restart = key(Key::KeyR) || btn(Button::North);
        intent.pause = key(Key::KeyP) || btn(Button::Start);

        let mut taps: Vec<(f32, f32)> = input.touches_started().iter().map(|t| t.pos()).collect();
        if input.mouse_pressed(MouseButton::Left) {
            taps.push(input.mouse_pos());
        }
        intent.tap = taps
            .into_iter()
            .map(|(x, y)| self.viewport.to_fb(x, y))
            .find(|&(x, y)| (0..W).contains(&x) && (0..H).contains(&y));
        intent
    }

    /// Switch to the title screen, restarting its clock so the lightning schedule starts
    /// over.
    fn goto_title(&mut self) {
        self.mode = Mode::Title;
        self.title_t = 0.0;
    }

    /// Switch to the level menu with no pause and no order form showing.
    ///
    /// A cursor on a real level is clamped to the unlocked range; a cursor on a locked
    /// episode entry is left alone, so callers can open the order form on that entry.
    fn goto_select(&mut self) {
        self.mode = Mode::Select;
        self.paused = false;
        self.order = false;
        if self.cursor < self.levels.len() {
            self.cursor = self.cursor.min(self.unlocked() - 1);
        }
    }

    /// Start level `i` from scratch and switch to [`Mode::Play`]: a fresh [`World`] (and so
    /// an empty undo history), and every per-level bit of presentation state cleared.
    fn start_level(&mut self, i: usize) {
        platform::notify(HostEvent::Start {
            level: i as u32 + 1,
        });
        self.cursor = i;
        self.world = World::new(Arc::clone(&self.levels[i]));
        self.anim = None;
        self.fx = Fx::new();
        self.pending.clear();
        self.trails.clear();
        self.trail_history.clear();
        self.end_t = 0.0;
        self.paused = false;
        self.held = None;
        self.idle_t = 0.0;
        // A fresh level gets a fresh squeak timer, not one pushed far out by idling on the
        // previous level.
        self.next_squeak = 8.0;
        self.squeak_age = None;
        self.mode = Mode::Play;
    }

    /// Title screen: advance its clock, play thunder when the painting's lightning strikes,
    /// and go to the level menu (cursor on the first unsolved level) on any key.
    ///
    /// The lightning schedule lives in [`title::struck`], which the renderer also uses for
    /// the flash, so sound and picture cannot drift apart. It is asked whether a strike
    /// happened between the previous and the current time, so no strike is missed or
    /// doubled whatever the frame rate.
    ///
    /// This mode reads raw input instead of the [`Intent`] so that anything counts: any
    /// key, mouse button, gamepad button or touch (the toolkit's `any_pressed`). Only `M`
    /// and `C` are excluded, because they toggle mute and scanlines globally.
    fn update_title(&mut self, ctx: &mut Context, dt: f32) {
        let t0 = self.title_t;
        self.title_t += dt;
        if title::struck(t0, self.title_t) {
            self.sound(&mut ctx.audio, &self.sfx.thunder, 1.0);
        }
        // Press any key: on to the level menu. Esc too; leaving is the menu's business. M and
        // C (mute, scanlines) are settings, so they don't count.
        //
        // In a browser, audio may only start after a user gesture. While it hasn't, the
        // first press only switches the sound on (the toolkit starts audio on that very
        // press) and restarts the title, so thunder and music play from the top; the next
        // press goes on. With audio already running (natively, or a page that had a
        // gesture before) or no audio device at all, the first press goes on directly.
        let sound_on = ctx.audio.as_ref().is_none_or(Audio::is_started);
        let input = &ctx.input;
        let settings = input.key_pressed(Key::KeyM) || input.key_pressed(Key::KeyC);
        if input.any_pressed() && !settings {
            if (self.sound_was_on && sound_on) || self.unlock_press_spent {
                self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
                self.cursor = self.first_unsolved();
                self.goto_select();
            } else {
                self.unlock_press_spent = true;
                self.title_t = 0.0;
            }
        }
        self.sound_was_on = sound_on;
    }

    /// Level menu.
    ///
    /// While the order form is open, any confirm, back, tap or direction just closes it
    /// and nothing else happens that frame. Otherwise:
    ///
    /// - Directions move the cursor through the grid laid out by [`render::select_move`],
    ///   which also covers the locked episode entries after the real levels.
    /// - Confirm, or a tap on an entry ([`render::select_hit`]), picks it. Level 1 opens
    ///   episode one's title card first; other unlocked levels start directly; a locked
    ///   level only plays a "nope" sound. A locked episode entry (cursor `>= n`) opens that
    ///   episode's title card, which in turn leads to the order form (see
    ///   [`Cheese::update_intro`]). Episode numbers start at 2 for the first locked entry.
    /// - Back leaves for the farewell screen, which closes the game.
    /// - In cheese mode, `I`, `E` and `F` jump to episode one's card, the ending and a
    ///   farewell preview.
    fn update_select(&mut self, ctx: &mut Context, it: &Intent) {
        let n = self.levels.len();
        if self.order {
            if it.confirm || it.back || it.tap.is_some() || it.dir.is_some() {
                self.order = false;
                self.sound(&mut ctx.audio, &self.sfx.menu, MENU_VOLUME);
            }
            return;
        }
        if let Some(d) = it.dir {
            let next = render::select_move(n, self.cursor, d);
            if next != self.cursor {
                self.cursor = next;
                self.sound(&mut ctx.audio, &self.sfx.menu, MENU_VOLUME);
            }
        }
        let mut play = it.confirm;
        if let Some((x, y)) = it.tap
            && let Some(i) = render::select_hit(n, x, y)
        {
            self.cursor = i;
            play = true;
        }
        if play {
            if self.cursor >= n {
                // A locked episode: its title card first, then the nag.
                self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
                self.show_intro(self.cursor - n + 2);
            } else if self.cursor < self.unlocked() {
                self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
                if self.cursor == 0 {
                    // The episode begins: its title card first.
                    self.show_intro(1);
                } else {
                    self.start_level(self.cursor);
                }
            } else {
                self.sound(&mut ctx.audio, &self.sfx.nope, 1.0);
            }
        } else if it.back {
            // Leaving: the farewell, then the game closes.
            self.mode = Mode::Farewell;
            self.farewell_preview = false;
            self.end_t = 0.0;
        } else if self.options.cheese && ctx.input.key_pressed(Key::KeyI) {
            self.show_intro(1);
        } else if self.options.cheese && ctx.input.key_pressed(Key::KeyE) {
            self.mode = Mode::End;
            self.end_t = 0.0;
        } else if self.options.cheese && ctx.input.key_pressed(Key::KeyF) {
            self.mode = Mode::Farewell;
            self.farewell_preview = true;
            self.end_t = 0.0;
        }
    }

    /// Farewell: any key (or waiting) closes the game. The web has nothing to close, so
    /// there it returns to the title.
    ///
    /// Input counts only after 0.6 seconds, so the Escape that opened this screen (or a
    /// quick second press) does not dismiss it before it is seen. Without input the screen
    /// closes by itself after [`render::farewell::LINGER`] seconds. A cheese-mode preview
    /// goes back to the level menu instead.
    fn update_farewell(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        self.end_t += dt;
        let pressed = it.confirm || it.back || it.tap.is_some() || it.dir.is_some();
        if (self.end_t > 0.6 && pressed) || self.end_t > render::farewell::LINGER {
            if self.farewell_preview {
                self.goto_select();
            } else {
                platform::notify(HostEvent::Exit);
                // There is no window to close on the web: go quiet until the player is back.
                if cfg!(target_arch = "wasm32") {
                    self.dormant = true;
                } else {
                    ctx.quit();
                }
            }
        }
    }

    /// Show the title card of `episode` (1-based) and restart its clock.
    fn show_intro(&mut self, episode: usize) {
        self.intro_episode = episode;
        self.mode = Mode::Intro;
        self.end_t = 0.0;
    }

    /// Episode title card.
    ///
    /// Back returns to the level menu. Confirm or a tap, accepted after 0.4 seconds,
    /// starts level 1 for episode one; for any later episode it plays a fanfare and opens
    /// the order form on the level menu, with the cursor on that episode's locked entry.
    /// Episode one's card reuses the title's lightning schedule (on this screen's own
    /// clock), so it gets thunder too.
    fn update_intro(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        let t0 = self.end_t;
        self.end_t += dt;
        // Episode one's storm cloud flashes on the title's schedule: thunder too.
        if self.intro_episode == 1 && title::struck(t0, self.end_t) {
            self.sound(&mut ctx.audio, &self.sfx.thunder, 0.8);
        }
        if it.back {
            self.goto_select();
        } else if self.options.cheese && ctx.input.key_pressed(Key::KeyI) {
            // Testing: flip through the episode cards.
            self.show_intro(self.intro_episode % render::intro::EPISODES + 1);
            self.sound(&mut ctx.audio, &self.sfx.menu, MENU_VOLUME);
        } else if self.end_t > 0.4 && (it.confirm || it.tap.is_some()) {
            if self.intro_episode == 1 {
                self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
                self.start_level(0);
            } else {
                // Later episodes are "registered version only": on to the order form.
                self.sound(&mut ctx.audio, &self.sfx.fanfare, 0.7);
                self.goto_select();
                self.cursor = self.levels.len() + self.intro_episode - 2;
                self.order = true;
            }
        }
    }

    /// Playing a level. In order, each frame:
    ///
    /// 1. Back returns to the level menu at once (the level is abandoned, not saved).
    /// 2. Pause toggles the pause; while paused, confirm or a tap resumes, and nothing else
    ///    runs, so animations, pending crumb bursts and the idle timer all freeze.
    /// 3. After the level ends, `end_t` counts up. When caught, the defeat sound plays as
    ///    the defeat card appears, [`render::defeat::DELAY`] seconds after the catch. When
    ///    solved, confirm or a tap moves on once the solved panel has had
    ///    [`render::SOLVED_DELAY`] seconds: to the next level, or to the ending after the
    ///    last one.
    /// 4. Undo, restart or a move is applied to the [`World`]. A tap moves one step
    ///    towards the tapped tile along whichever axis is farther away (horizontal on a
    ///    tie). These are still applied after the level ended, which is how undo rescues a
    ///    caught mouse; moves are simply refused by the logic then.
    /// 5. If the state changed, a new [`Anim`] tweens from the old state to the new one.
    ///    A restart cuts any running animation instead, so the board snaps back.
    /// 6. The idle squeak timer runs (see `next_squeak`).
    /// 7. The turn's events are handed to [`Cheese::react`], then the animation and the
    ///    pending crumb bursts advance.
    fn update_play(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        if it.back {
            self.goto_select();
            return;
        }
        if it.pause {
            self.paused = !self.paused;
        }
        if self.paused {
            if it.tap.is_some() || it.confirm {
                self.paused = false;
            }
            return;
        }

        let status = self.world.state.status;
        if status != Status::Playing {
            let t0 = self.end_t;
            self.end_t += dt;
            if status == Status::Caught
                && t0 < render::defeat::DELAY
                && self.end_t >= render::defeat::DELAY
            {
                self.sound(&mut ctx.audio, &self.sfx.defeat, 1.0);
            }
        }
        if status == Status::Solved && self.end_t >= render::SOLVED_DELAY {
            let next = it.confirm || it.tap.is_some();
            if next {
                self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
                if self.cursor + 1 < self.levels.len() {
                    self.start_level(self.cursor + 1);
                } else {
                    self.mode = Mode::End;
                    self.end_t = 0.0;
                    platform::notify(HostEvent::Complete {
                        stars: self.earned_stars(),
                        max: self.levels.len() as u32 * 3,
                    });
                }
                return;
            }
        }

        let before = self.world.state.clone();
        let depth = self.world.history_len();
        let mut changed = false;
        if it.undo {
            changed = self.world.undo();
        } else if it.restart {
            self.world.restart();
            self.anim = None;
        } else {
            let dir = it.dir.or_else(|| {
                // A tap steps towards it along the longer axis.
                let (x, y) = it.tap?;
                let level = &self.world.level;
                let target = render::tile_at(level, x, y);
                let m = self.world.state.mouse;
                let (dx, dy) = (target.x - m.x, target.y - m.y);
                match (dx.abs() >= dy.abs(), dx, dy) {
                    (_, 0, 0) => None,
                    (true, dx, _) => Some(if dx > 0 { Dir::Right } else { Dir::Left }),
                    (false, _, dy) => Some(if dy > 0 { Dir::Down } else { Dir::Up }),
                }
            });
            if let Some(d) = dir {
                changed = self.world.step(d);
            }
        }
        // Keep the trails in step with the undo history (see `trail_history`). New streaks
        // from this turn's slide are added afterwards, by `react`.
        let new_depth = self.world.history_len();
        if new_depth > depth {
            self.trail_history.push(self.trails.clone());
            if it.restart {
                self.trails.clear();
            }
        } else if new_depth < depth
            && let Some(prev) = self.trail_history.pop()
        {
            self.trails = prev;
        }
        if changed {
            self.anim = Some(Anim::new(before, &self.world.state));
            // Back in play (say, undo after a catch): the end-of-level clock starts over,
            // so the next defeat or solve gets its full delay again.
            if self.world.state.status == Status::Playing {
                self.end_t = 0.0;
            }
        }

        // Left alone for a while, the mouse squeaks. Any input (even a refused move) resets
        // the idle clock and schedules the first squeak 6 to 11 seconds out; after each
        // squeak the next one comes 8 to 14 seconds later. The clock only runs while the
        // level is in play and the board is at rest. The squeak pop-up shows for one
        // second, or until the player does something.
        let acted = changed || it.dir.is_some() || it.undo || it.restart || it.tap.is_some();
        if acted {
            self.idle_t = 0.0;
            self.next_squeak = 6.0 + ctx.rng.next_f32() * 5.0;
        } else if self.world.state.status == Status::Playing && self.anim.is_none() {
            self.idle_t += dt;
            if self.idle_t >= self.next_squeak {
                self.sound(&mut ctx.audio, &self.sfx.squeak, 0.8);
                self.squeak_age = Some(0.0);
                self.next_squeak = self.idle_t + 8.0 + ctx.rng.next_f32() * 6.0;
            }
        }
        if let Some(a) = &mut self.squeak_age {
            *a += dt;
            if *a > 1.0 || acted {
                self.squeak_age = None;
            }
        }

        // The animation length tells `react` when a plopped cheese visually arrives.
        let dur = self.anim.as_ref().map_or(0.0, |a| a.dur);
        let events = std::mem::take(&mut self.world.events);
        for e in events {
            self.react(ctx, e, dur);
        }

        if let Some(a) = &mut self.anim {
            a.update(dt);
            if a.done() {
                self.anim = None;
            }
        }
        let level = Arc::clone(&self.world.level);
        self.pending.retain_mut(|(left, pos, kind)| {
            *left -= dt;
            if *left <= 0.0 {
                let (x, y) = render::tile_center(&level, *pos);
                self.fx.burst(x, y, 14, render::cheese_colors(*kind));
                false
            } else {
                true
            }
        });
    }

    /// Sounds, effects and bookkeeping for one turn's events.
    ///
    /// Called after the world has already moved to the new state, so `self.world.state`
    /// is the result of the turn. `anim_secs` is the length of that turn's animation.
    ///
    /// Besides sounds, this is where the app keeps its own state in step with the logic:
    ///
    /// - A brie push leaves a [`Trail`]. Undo and restart are handled by `update_play`,
    ///   which keeps a copy of the trails for every undo-history entry: undoing a slide
    ///   wipes exactly its streak, and undoing a restart brings the streaks back.
    /// - A plop queues a crumb burst for `anim_secs` later, when the cheese reaches the
    ///   hole on screen, coloured after the cheese now sitting there.
    /// - Caught and solved restart the `end_t` clock that times their panels.
    /// - Solved records the move count and saves progress if it is a new best (or the
    ///   first solve). This is the only place progress is written.
    fn react(&mut self, ctx: &mut Context, e: Event, anim_secs: f32) {
        let audio = &mut ctx.audio;
        let b = &self.sfx;
        match e {
            Event::Step => self.sound(audio, &b.step, 0.5),
            Event::Bump => self.sound(audio, &b.bump, 0.6),
            Event::TooHeavy => {
                self.sound(audio, &b.too_heavy, 0.8);
                self.fx.shake(1.5);
                self.toast("TOO HEAVY! TAKE A RUN-UP.", 1.6);
            }
            Event::Pushed {
                kind,
                from: e_from,
                to: e_to,
            } => match kind {
                CheeseKind::Wedge => self.sound(audio, &b.push, 0.8),
                CheeseKind::Wheel => self.sound(audio, &b.roll, 0.9),
                CheeseKind::Brie => {
                    self.sound(audio, &b.slide, 0.8);
                    self.trails.push(Trail {
                        from: e_from,
                        to: e_to,
                        born: self.t,
                    });
                }
            },
            Event::Plopped(pos) => {
                self.sound(audio, &b.plop, 0.9);
                let kind = self
                    .world
                    .state
                    .cheese_at(pos)
                    .map_or(CheeseKind::Wedge, |i| self.world.state.cheeses[i].kind);
                self.pending.push((anim_secs, pos, kind));
            }
            Event::Unplopped(_) => self.sound(audio, &b.unplop, 0.6),
            Event::CatTurned(_) => {}
            Event::Caught(_) => {
                self.sound(audio, &b.meow, 1.0);
                self.fx.shake(3.0);
                self.end_t = 0.0;
            }
            Event::Solved => {
                self.sound(audio, &b.solved, 1.0);
                self.fx.celebrate();
                self.end_t = 0.0;
                let level = Arc::clone(&self.world.level);
                let moves = self.world.state.moves;
                if self.progress.record(&level, moves) {
                    self.progress.save();
                }
            }
            Event::Undone | Event::Restarted => {
                let sfx = if e == Event::Undone {
                    &b.undo
                } else {
                    &b.restart
                };
                self.sound(audio, sfx, 0.7);
            }
        }
    }

    /// Show `msg` over the screen for `secs` seconds, replacing any current toast.
    fn toast(&mut self, msg: &str, secs: f32) {
        self.toast = Some((msg.to_string(), secs));
    }

    /// Episode ending. Back returns to the level menu; confirm or a tap, accepted after
    /// one second so the star tally can be read, continues to the "Onward!" teaser.
    fn update_end(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        self.end_t += dt;
        if it.back {
            self.goto_select();
        } else if self.end_t > 1.0 && (it.confirm || it.tap.is_some()) {
            self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
            self.mode = Mode::Onward;
            self.end_t = 0.0;
        }
    }

    /// Onward... to the order form, since episode two is "registered version only".
    ///
    /// Confirm or a tap (after 0.6 seconds) opens the order form on the level menu with
    /// the cursor on episode two's entry (`levels.len()`); back goes to the menu without it.
    fn update_onward(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        self.end_t += dt;
        if it.back {
            self.goto_select();
        } else if self.end_t > 0.6 && (it.confirm || it.tap.is_some()) {
            self.sound(&mut ctx.audio, &self.sfx.confirm, CONFIRM_VOLUME);
            self.goto_select();
            self.cursor = self.levels.len();
            self.order = true;
        }
    }

    /// Apply the mute setting through the master volume, which covers music and effects
    /// alike. Music ducking is separate (the music stream's own volume), so muting and
    /// ducking do not interfere.
    fn apply_volume(&self, ctx: &mut Context) {
        if let Some(a) = &mut ctx.audio {
            let silent = self.muted || self.host_muted;
            a.set_master_volume(if silent { 0.0 } else { 1.0 });
        }
    }

    /// Mute while the host has the game hidden, without touching the player's own mute.
    fn set_host_muted(&mut self, ctx: &mut Context, muted: bool) {
        if self.host_muted != muted {
            self.host_muted = muted;
            self.apply_volume(ctx);
        }
    }

    /// Stars earned over all levels, from the saved best move counts.
    fn earned_stars(&self) -> u32 {
        self.levels
            .iter()
            .map(|l| {
                self.progress
                    .best(l)
                    .map_or(0, |b| u32::from(stars(b, l.par)))
            })
            .sum()
    }

    /// The host asked to start over: back to the title as if freshly launched. Saved
    /// progress, the player's mute and scanline choices, and the unlocked audio all stay.
    fn restart(&mut self) {
        self.dormant = false;
        self.paused = false;
        self.order = false;
        self.farewell_preview = false;
        self.unlock_press_spent = false;
        self.toast = None;
        self.held = None;
        self.fx = Fx::new();
        // The page may have had a gesture since launch (the player clicked a button).
        platform::probe_audio();
        self.goto_title();
    }

    /// On the web, start audio without waiting for a key in the game when the page
    /// already had a gesture and the browser lets it play (see [`platform::audio_allowed`]).
    /// Otherwise the toolkit starts it on the first key press as usual.
    fn start_audio_early(&mut self, ctx: &mut Context) {
        if let Some(a) = &mut ctx.audio
            && !a.is_started()
            && platform::audio_allowed()
            && let Err(e) = a.start()
        {
            log::warn!("early audio start: {e}");
        }
    }

    /// A deliberate "I'm back" from a dormant player: a key the game uses, a click, a tap
    /// or a gamepad button. (Modifier keys and the like don't count.)
    fn wake_pressed(ctx: &Context) -> bool {
        let i = &ctx.input;
        let pad = i
            .first_gamepad()
            .is_some_and(|p| p.button_pressed(Button::South) || p.button_pressed(Button::Start));
        let keys = [
            Key::Space,
            Key::Enter,
            Key::Escape,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
        ];
        keys.iter().any(|&k| i.key_pressed(k))
            || pad
            || i.mouse_pressed(MouseButton::Left)
            || !i.touches_started().is_empty()
    }
}

impl Game for Cheese {
    /// Create the framebuffer texture, load launch options and progress, and start on the
    /// title screen.
    ///
    /// With `--reset` the saved progress is deleted before anything else reads it, and a
    /// toast confirms it. The world is initialised with level 1 only because the field
    /// needs a value; it is replaced when a level actually starts.
    fn init(ctx: &mut Context) -> Result<Self> {
        let rgba = vec![0; (W * H * 4) as usize];
        let tex = ctx
            .gfx
            .create_texture_rgba(W as u32, H as u32, &rgba, Some("framebuffer"));
        let (ww, wh) = ctx.gfx.size();
        let levels = levels();
        let options = Options::load();
        // On the web, find out early whether the page's earlier gestures already unlock
        // audio, so the title's thunder and music need not wait for a key press.
        platform::probe_audio();
        Ok(Self {
            screen: Screen::new(),
            rgba,
            tex,
            renderer: Renderer::new(),
            viewport: Viewport::fit(ww, wh),
            mode: Mode::Title,
            t: 0.0,
            options,
            intro_episode: 1,
            farewell_preview: false,
            sound_was_on: ctx.audio.as_ref().is_none_or(Audio::is_started),
            unlock_press_spent: false,
            dormant: false,
            host_muted: false,
            title_t: 0.0,
            music: Music {
                volume: ROCK_VOLUME,
                ..Default::default()
            },
            levels,
            progress: if options.reset {
                Progress::wipe();
                Progress::default()
            } else {
                Progress::load()
            },
            cursor: 0,
            world: World::new(Arc::clone(&levels[0])),
            anim: None,
            fx: Fx::new(),
            pending: Vec::new(),
            end_t: 0.0,
            paused: false,
            trails: Vec::new(),
            trail_history: Vec::new(),
            order: false,
            idle_t: 0.0,
            next_squeak: 8.0,
            squeak_age: None,
            held: None,
            stick: None,
            sfx: sfx::Bank::new(),
            muted: false,
            scanlines: true,
            toast: options.reset.then(|| ("PROGRESS RESET".to_string(), 2.5)),
        })
    }

    /// One frame of game logic, with `dt` in seconds (variable step).
    ///
    /// Global keys come first and work on every screen: `M` toggles mute, `C` toggles the
    /// scanlines. Then the toast timer runs down, input is read into an `Intent` and the
    /// current mode's handler runs. Effects advance unless paused.
    ///
    /// Music is decided last, from the mode as it is after the handler ran: rock for the
    /// title and menus, the minuet in play, the extro for the ending screens. While paused,
    /// or once the defeat card is up, the music volume drops to zero. The stream keeps
    /// running underneath, so the tune picks up mid-song when the volume returns rather
    /// than starting over.
    fn update(&mut self, ctx: &mut Context, dt: f32) {
        for cmd in platform::commands() {
            match cmd {
                HostCommand::Restart => self.restart(),
            }
        }
        self.start_audio_early(ctx);
        // Hidden by the host (left the game, or the canvas was taken off the page): freeze
        // and stay silent; a level in progress waits behind the pause screen.
        if self.dormant || !platform::is_active() {
            let woke = self.dormant && platform::is_active() && Self::wake_pressed(ctx);
            if woke {
                self.dormant = false;
                self.set_host_muted(ctx, false);
                self.goto_title();
            } else {
                if self.mode == Mode::Play {
                    self.paused = true;
                }
                self.set_host_muted(ctx, true);
            }
            // The waking press only wakes.
            return;
        }
        self.set_host_muted(ctx, false);
        self.t += dt;
        if ctx.input.key_pressed(Key::KeyM) {
            self.muted = !self.muted;
            self.apply_volume(ctx);
            self.toast(if self.muted { "SOUND OFF" } else { "SOUND ON" }, 1.2);
        }
        if ctx.input.key_pressed(Key::KeyC) {
            self.scanlines = !self.scanlines;
        }
        if let Some((_, left)) = &mut self.toast {
            *left -= dt;
            if *left <= 0.0 {
                self.toast = None;
            }
        }
        let it = self.read_intent(&ctx.input, dt);
        match self.mode {
            Mode::Title => self.update_title(ctx, dt),
            Mode::Select => self.update_select(ctx, &it),
            Mode::Play => self.update_play(ctx, &it, dt),
            Mode::Intro => self.update_intro(ctx, &it, dt),
            Mode::End => self.update_end(ctx, &it, dt),
            Mode::Onward => self.update_onward(ctx, &it, dt),
            Mode::Farewell => self.update_farewell(ctx, &it, dt),
        }
        if !self.paused {
            self.fx.update(dt);
        }
        let tune = match self.mode {
            Mode::Title | Mode::Select | Mode::Intro => Tune::Rock,
            Mode::Play => Tune::Minuet,
            Mode::End | Mode::Onward | Mode::Farewell => Tune::Extro,
        };
        // The music stops for the defeat card.
        let defeated = self.mode == Mode::Play
            && self.world.state.status == Status::Caught
            && self.end_t >= render::defeat::DELAY;
        let volume = match (self.paused || defeated, tune) {
            (true, _) => 0.0,
            (false, Tune::Rock) => ROCK_VOLUME,
            (false, Tune::Extro) => EXTRO_VOLUME,
            (false, Tune::Minuet) => MINUET_VOLUME,
        };
        self.music.set_volume(volume);
        self.music.play(&mut ctx.audio, tune);
    }

    /// Window events: refit the viewport on resize, and pause a level when the window
    /// loses focus so the player does not come back to a squeaking mouse and wonder what
    /// happened. Regaining focus does not unpause; the player does that.
    fn event(&mut self, _ctx: &mut Context, e: &GameEvent) {
        match *e {
            GameEvent::Resized { width, height } => self.viewport = Viewport::fit(width, height),
            GameEvent::FocusChanged(false) if self.mode == Mode::Play => self.paused = true,
            _ => {}
        }
    }

    /// Draw the frame in two stages.
    ///
    /// First the current mode is drawn on the CPU into the 320x200 [`Screen`], with the
    /// toast on top, converted to RGBA and uploaded into the framebuffer texture. Then the
    /// GPU part: clear the window to black, draw that texture at the viewport's integer
    /// scale (offset by the screen shake, in whole framebuffer pixels), and, at 3x or more,
    /// darken the bottom third of every framebuffer row for a CRT scanline look. The
    /// scanlines move with the shake, so they stay on their pixel rows.
    ///
    /// The viewport is refitted here every frame as well as on resize, so it is right even
    /// if a resize event was missed.
    fn render(&mut self, ctx: &mut Context, frame: &mut Frame) {
        match self.mode {
            Mode::Title => {
                let need_sound =
                    !ctx.audio.as_ref().is_none_or(Audio::is_started) && !self.unlock_press_spent;
                self.renderer
                    .draw_title(&mut self.screen, self.title_t, need_sound)
            }
            Mode::Select => {
                let best = self.best();
                let view = SelectView {
                    levels: self.levels,
                    best: &best,
                    cursor: self.cursor,
                    unlocked: self.unlocked(),
                    order: self.order,
                    cheese: self.options.cheese,
                    t: self.t,
                };
                self.renderer.draw_select(&mut self.screen, &view);
            }
            Mode::Play => {
                let view = PlayView {
                    world: &self.world,
                    anim: self.anim.as_ref(),
                    fx: &self.fx,
                    number: self.cursor + 1,
                    best: self.progress.best(&self.world.level),
                    t: self.t,
                    end_t: self.end_t,
                    paused: self.paused,
                    trails: &self.trails,
                    idle: self.idle_t,
                    squeak_age: self.squeak_age,
                };
                self.renderer.draw_play(&mut self.screen, &view);
            }
            Mode::Intro => {
                self.renderer
                    .draw_intro(&mut self.screen, self.intro_episode, self.end_t)
            }
            Mode::Onward => self.renderer.draw_onward(&mut self.screen, self.end_t),
            Mode::Farewell => self.renderer.draw_farewell(&mut self.screen, self.end_t),
            Mode::End => {
                let earned = self.earned_stars();
                let max = self.levels.len() as u32 * 3;
                self.renderer
                    .draw_end(&mut self.screen, earned, max, self.end_t);
            }
        }
        if self.dormant {
            self.screen
                .blend_rect(0, 0, W, H, crate::gfx::palette::NAVY, 0.7);
            render::toast(&mut self.screen, "SEE YOU! PRESS ANY KEY TO COME BACK");
        } else if let Some((msg, _)) = &self.toast {
            render::toast(&mut self.screen, msg);
        }
        self.screen.to_rgba(&mut self.rgba, None);
        ctx.gfx.update_texture_rgba(self.tex, &self.rgba);

        let (ww, wh) = ctx.gfx.size();
        self.viewport = Viewport::fit(ww, wh);
        let vp = self.viewport;
        let (sx, sy) = self.fx.shake_offset(self.t);
        let (dw, dh) = (W as f32 * vp.scale, H as f32 * vp.scale);
        let mut p = frame.painter(&mut ctx.gfx);
        p.clear([0.0, 0.0, 0.0, 1.0]);
        let (shake_x, shake_y) = (sx as f32 * vp.scale, sy as f32 * vp.scale);
        p.sprite(self.tex, [vp.x + shake_x, vp.y + shake_y], [dw, dh]);
        if self.scanlines && vp.scale >= 3.0 {
            let thick = (vp.scale / 3.0).floor();
            for row in 0..H {
                // Shaken along with the picture, so the lines stay on their pixel rows.
                let y = vp.y + shake_y + (row as f32 + 1.0) * vp.scale - thick;
                p.rect_ex(
                    [vp.x + shake_x, y],
                    [dw, thick],
                    [0.0, 0.0, 0.0, 0.22],
                    1,
                    BlendMode::Alpha,
                );
            }
        }
    }
}

/// The toolkit configuration: window title, a 4x-scaled initial window (1280x800), the web
/// canvas to attach to, and no assets to load. Everything else uses the toolkit defaults,
/// including a variable time step.
pub fn config() -> AppConfig {
    AppConfig {
        title: "Cheesed!".into(),
        width: (W * 4) as u32,
        height: (H * 4) as u32,
        // Everything is generated in code or embedded with `include_bytes!`: no asset files,
        // so there's no assets/ folder. Natively point the (unused) root at the working
        // directory, which always exists, instead of warning about a missing folder.
        asset_root: if cfg!(target_arch = "wasm32") {
            asset_root!()
        } else {
            ".".into()
        },
        canvas_id: Some(CANVAS_ID.into()),
        // Nothing to fetch on the web either.
        web_preload: Some(Vec::new()),
        ..Default::default()
    }
}
