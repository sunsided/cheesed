//! The game-toolkit glue: input, audio device, and presenting the framebuffer.

use std::sync::Arc;

use game_toolkit::prelude::*;

use crate::gfx::{H, Screen, W};
use crate::logic::{CheeseKind, Dir, Event, Level, Pos, Status, World, levels, stars};
use crate::platform::{Options, Progress};
use crate::render::anim::Anim;
use crate::render::fx::Fx;
use crate::render::title;
use crate::render::{self, PlayView, Renderer, SelectView, Trail};
use crate::sound::tracker::Player;
use crate::sound::{SAMPLE_RATE, sfx, songs};

pub const CANVAS_ID: &str = "cheese";
const SFX_VOLUME: f32 = 0.6;
const ROCK_VOLUME: f32 = 0.42;
const MINUET_VOLUME: f32 = 0.55;
/// Holding a direction: first repeat after this long, then every `REPEAT_EVERY`.
const REPEAT_DELAY: f32 = 0.24;
const REPEAT_EVERY: f32 = 0.12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Title,
    Select,
    /// The episode's title card, before its first level.
    Intro,
    Play,
    End,
}

/// Where the framebuffer sits in the window.
#[derive(Clone, Copy, Debug)]
struct Viewport {
    x: f32,
    y: f32,
    scale: f32,
}

impl Viewport {
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

    fn to_fb(self, x: f32, y: f32) -> (i32, i32) {
        (
            ((x - self.x) / self.scale).floor() as i32,
            ((y - self.y) / self.scale).floor() as i32,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tune {
    /// The title and menus: heroic chiptune rock.
    Rock,
    /// The game itself: a music box. Deflating, on purpose.
    Minuet,
    /// The episode ending: the epic, back and in major.
    Extro,
}

/// The music stream: one song at a time, synthesized on the audio thread.
#[derive(Default)]
struct Music {
    handle: Option<StreamHandle>,
    tune: Option<Tune>,
    volume: f32,
}

impl Music {
    /// Switch to `tune` unless it's already playing.
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
        let mut player = Player::new(song);
        let stream = move |out: &mut [f32]| player.fill(out);
        // Deferred: on the web it waits for the first key press or tap.
        match audio.play_stream_with(
            stream,
            SAMPLE_RATE,
            PlayOptions::default().volume(self.volume).deferred(),
        ) {
            Ok(h) => self.handle = Some(h),
            Err(e) => log::warn!("music: {e}"),
        }
    }

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
#[derive(Default)]
struct Intent {
    dir: Option<Dir>,
    confirm: bool,
    back: bool,
    undo: bool,
    restart: bool,
    pause: bool,
    /// A tap or click, in framebuffer pixels.
    tap: Option<(i32, i32)>,
}

pub struct Cheese {
    screen: Screen,
    rgba: Vec<u8>,
    tex: TextureId,
    renderer: Renderer,
    viewport: Viewport,
    mode: Mode,
    t: f32,
    options: Options,
    /// Which episode's intro card is showing (1-based).
    intro_episode: usize,
    /// Seconds on the title screen, which times the lightning and its thunder.
    title_t: f32,
    music: Music,
    levels: &'static [Arc<Level>],
    progress: Progress,
    /// Level-select cursor; also the level being played.
    cursor: usize,
    world: World,
    anim: Option<Anim>,
    fx: Fx,
    /// Crumb bursts waiting for the cheese to arrive: (seconds left, tile, kind).
    pending: Vec<(f32, Pos, CheeseKind)>,
    end_t: f32,
    paused: bool,
    /// Greasy streaks left by sliding brie.
    trails: Vec<Trail>,
    /// The level menu is showing the (pretend) order form.
    order: bool,
    /// The direction being held and the time until it repeats.
    held: Option<(Dir, f32)>,
    /// Last analog stick direction, for edge-triggering.
    stick: Option<Dir>,
    sfx: sfx::Bank,
    muted: bool,
    scanlines: bool,
    toast: Option<(String, f32)>,
}

impl Cheese {
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

    fn best(&self) -> Vec<Option<u32>> {
        self.levels.iter().map(|l| self.progress.best(l)).collect()
    }

    /// Levels `0..n` are playable: everything up to and including the first unsolved one.
    fn unlocked(&self) -> usize {
        if self.options.cheese {
            return self.levels.len();
        }
        self.levels
            .iter()
            .position(|l| self.progress.best(l).is_none())
            .map_or(self.levels.len(), |i| i + 1)
    }

    fn first_unsolved(&self) -> usize {
        self.unlocked().min(self.levels.len()) - 1
    }

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

    fn goto_title(&mut self) {
        self.mode = Mode::Title;
        self.title_t = 0.0;
    }

    fn goto_select(&mut self) {
        self.mode = Mode::Select;
        self.paused = false;
        self.order = false;
        if self.cursor < self.levels.len() {
            self.cursor = self.cursor.min(self.unlocked() - 1);
        }
    }

    fn start_level(&mut self, i: usize) {
        self.cursor = i;
        self.world = World::new(Arc::clone(&self.levels[i]));
        self.anim = None;
        self.fx = Fx::new();
        self.pending.clear();
        self.trails.clear();
        self.end_t = 0.0;
        self.paused = false;
        self.held = None;
        self.mode = Mode::Play;
    }

    fn update_title(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        let t0 = self.title_t;
        self.title_t += dt;
        if title::struck(t0, self.title_t) {
            self.sound(&mut ctx.audio, &self.sfx.thunder, 1.0);
        }
        if it.confirm || it.tap.is_some() {
            self.sound(&mut ctx.audio, &self.sfx.confirm, 1.0);
            self.cursor = self.first_unsolved();
            self.goto_select();
        } else if it.back && !cfg!(target_arch = "wasm32") {
            ctx.quit();
        }
    }

    fn update_select(&mut self, ctx: &mut Context, it: &Intent) {
        let n = self.levels.len();
        if self.order {
            if it.confirm || it.back || it.tap.is_some() || it.dir.is_some() {
                self.order = false;
                self.sound(&mut ctx.audio, &self.sfx.menu, 0.6);
            }
            return;
        }
        if let Some(d) = it.dir {
            let next = render::select_move(n, self.cursor, d);
            if next != self.cursor {
                self.cursor = next;
                self.sound(&mut ctx.audio, &self.sfx.menu, 0.6);
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
                self.order = true;
                self.sound(&mut ctx.audio, &self.sfx.fanfare, 0.7);
            } else if self.cursor < self.unlocked() {
                self.sound(&mut ctx.audio, &self.sfx.confirm, 1.0);
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
            self.goto_title();
        } else if self.options.cheese && ctx.input.key_pressed(Key::KeyI) {
            self.show_intro(1);
        } else if self.options.cheese && ctx.input.key_pressed(Key::KeyE) {
            self.mode = Mode::End;
            self.end_t = 0.0;
        }
    }

    fn show_intro(&mut self, episode: usize) {
        self.intro_episode = episode;
        self.mode = Mode::Intro;
        self.end_t = 0.0;
    }

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
            self.sound(&mut ctx.audio, &self.sfx.menu, 0.6);
        } else if self.end_t > 0.4 && (it.confirm || it.tap.is_some()) {
            self.sound(&mut ctx.audio, &self.sfx.confirm, 1.0);
            if self.intro_episode == 1 {
                self.start_level(0);
            } else {
                // Later episodes are "registered version only": straight to the order form.
                self.goto_select();
                self.cursor = self.levels.len() + self.intro_episode - 2;
                self.order = true;
            }
        }
    }

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
            self.end_t += dt;
        }
        if status == Status::Solved && self.end_t >= render::SOLVED_DELAY {
            let next = it.confirm || it.tap.is_some();
            if next {
                self.sound(&mut ctx.audio, &self.sfx.confirm, 1.0);
                if self.cursor + 1 < self.levels.len() {
                    self.start_level(self.cursor + 1);
                } else {
                    self.mode = Mode::End;
                    self.end_t = 0.0;
                }
                return;
            }
        }

        let before = self.world.state.clone();
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
        if changed {
            self.anim = Some(Anim::new(before, &self.world.state));
            if self.world.state.status == Status::Playing {
                self.end_t = 0.0;
            }
        }

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
                        turn: self.world.state.moves,
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
                // Smears from undone turns un-happen too.
                let moves = self.world.state.moves;
                self.trails.retain(|tr| tr.turn <= moves);
            }
        }
    }

    fn toast(&mut self, msg: &str, secs: f32) {
        self.toast = Some((msg.to_string(), secs));
    }

    fn update_end(&mut self, ctx: &mut Context, it: &Intent, dt: f32) {
        self.end_t += dt;
        if self.end_t > 1.0 && (it.confirm || it.back || it.tap.is_some()) {
            self.sound(&mut ctx.audio, &self.sfx.confirm, 1.0);
            self.goto_select();
        }
    }

    fn apply_volume(&self, ctx: &mut Context) {
        if let Some(a) = &mut ctx.audio {
            a.set_master_volume(if self.muted { 0.0 } else { 1.0 });
        }
    }
}

impl Game for Cheese {
    fn init(ctx: &mut Context) -> Result<Self> {
        let rgba = vec![0; (W * H * 4) as usize];
        let tex = ctx
            .gfx
            .create_texture_rgba(W as u32, H as u32, &rgba, Some("framebuffer"));
        let (ww, wh) = ctx.gfx.size();
        let levels = levels();
        Ok(Self {
            screen: Screen::new(),
            rgba,
            tex,
            renderer: Renderer::new(),
            viewport: Viewport::fit(ww, wh),
            mode: Mode::Title,
            t: 0.0,
            options: Options::load(),
            intro_episode: 1,
            title_t: 0.0,
            music: Music {
                volume: ROCK_VOLUME,
                ..Default::default()
            },
            levels,
            progress: Progress::load(),
            cursor: 0,
            world: World::new(Arc::clone(&levels[0])),
            anim: None,
            fx: Fx::new(),
            pending: Vec::new(),
            end_t: 0.0,
            paused: false,
            trails: Vec::new(),
            order: false,
            held: None,
            stick: None,
            sfx: sfx::Bank::new(),
            muted: false,
            scanlines: true,
            toast: None,
        })
    }

    fn update(&mut self, ctx: &mut Context, dt: f32) {
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
            Mode::Title => self.update_title(ctx, &it, dt),
            Mode::Select => self.update_select(ctx, &it),
            Mode::Play => self.update_play(ctx, &it, dt),
            Mode::Intro => self.update_intro(ctx, &it, dt),
            Mode::End => self.update_end(ctx, &it, dt),
        }
        if !self.paused {
            self.fx.update(dt);
        }
        let tune = match self.mode {
            Mode::Title | Mode::Select | Mode::Intro => Tune::Rock,
            Mode::Play => Tune::Minuet,
            Mode::End => Tune::Extro,
        };
        let volume = match (self.paused, tune) {
            (true, _) => 0.0,
            (false, Tune::Rock | Tune::Extro) => ROCK_VOLUME,
            (false, Tune::Minuet) => MINUET_VOLUME,
        };
        self.music.set_volume(volume);
        self.music.play(&mut ctx.audio, tune);
    }

    fn event(&mut self, _ctx: &mut Context, e: &GameEvent) {
        match *e {
            GameEvent::Resized { width, height } => self.viewport = Viewport::fit(width, height),
            GameEvent::FocusChanged(false) if self.mode == Mode::Play => self.paused = true,
            _ => {}
        }
    }

    fn render(&mut self, ctx: &mut Context, frame: &mut Frame) {
        match self.mode {
            Mode::Title => self.renderer.draw_title(&mut self.screen, self.title_t),
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
                };
                self.renderer.draw_play(&mut self.screen, &view);
            }
            Mode::Intro => {
                self.renderer
                    .draw_intro(&mut self.screen, self.intro_episode, self.end_t)
            }
            Mode::End => {
                let earned: u32 = self
                    .levels
                    .iter()
                    .map(|l| {
                        self.progress
                            .best(l)
                            .map_or(0, |b| u32::from(stars(b, l.par)))
                    })
                    .sum();
                let max = self.levels.len() as u32 * 3;
                self.renderer
                    .draw_end(&mut self.screen, earned, max, self.end_t);
            }
        }
        if let Some((msg, _)) = &self.toast {
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
        p.sprite(
            self.tex,
            [vp.x + sx as f32 * vp.scale, vp.y + sy as f32 * vp.scale],
            [dw, dh],
        );
        if self.scanlines && vp.scale >= 3.0 {
            let thick = (vp.scale / 3.0).floor();
            for row in 0..H {
                let y = vp.y + (row as f32 + 1.0) * vp.scale - thick;
                p.rect_ex(
                    [vp.x, y],
                    [dw, thick],
                    [0.0, 0.0, 0.0, 0.22],
                    1,
                    BlendMode::Alpha,
                );
            }
        }
    }
}

pub fn config() -> AppConfig {
    AppConfig {
        title: "Cheesed!".into(),
        width: (W * 4) as u32,
        height: (H * 4) as u32,
        asset_root: asset_root!(),
        canvas_id: Some(CANVAS_ID.into()),
        // Everything is generated in code: no asset files to fetch.
        web_preload: Some(Vec::new()),
        ..Default::default()
    }
}
