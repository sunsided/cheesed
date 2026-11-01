//! Headless screenshots of the framebuffer, for checking art without opening a window:
//! `cargo run --example snapshot -- out_dir` writes title.png, select.png, a frame of every
//! level, the solved / caught panels, and the music and thunder as WAV files.
//!
//! The output directory defaults to `snapshots`. Besides the screens named above it writes
//! a sprite sheet (`sprites.png`), the order form, a brie mid-slide and with its trail, the
//! defeat card fading in, the idle squeak, the three episode cards, and the ending, onward
//! and farewell screens. PNGs are written at 3x.
//!
//! This works because nothing below the app layer touches the toolkit: the renderer draws
//! into a plain [`Screen`], and the views are built by hand here with fixed times instead
//! of a running clock, so every run produces the same images.

use std::sync::Arc;

use cheese::gfx::{Screen, png};
use cheese::logic::solver::{Outcome, solve};
use cheese::logic::{CheeseKind, Dir, Event, Status, World, levels};
use cheese::render::anim::Anim;
use cheese::render::fx::Fx;
use cheese::render::{PlayView, Renderer, SelectView, Trail};
use cheese::sound::tracker::Player;
use cheese::sound::{SAMPLE_RATE, songs};

/// Renders each scene into the framebuffer and saves it, printing every path written.
fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "snapshots".into());
    std::fs::create_dir_all(&dir).expect("create output dir");
    let r = Renderer::new();
    let mut s = Screen::new();
    let save = |s: &Screen, name: &str| {
        let path = format!("{dir}/{name}.png");
        std::fs::write(&path, png::encode(s, 3)).expect("write png");
        println!("{path}");
    };
    let fx = Fx::new();

    // Every sprite at 3x on the floor, for checking the art.
    let a = cheese::render::art::Art::new();
    s.clear(cheese::gfx::palette::WOOD);
    let sheet = [
        &a.mouse_right[0],
        &a.mouse_right[1],
        &a.mouse_down[0],
        &a.mouse_up[0],
        &a.mouse_twitch[0],
        &a.mouse_twitch[1],
        &a.mouse_twitch[2],
        &a.mouse_caught,
        &a.cat_right[0],
        &a.cat_right[1],
        &a.cat_pounce,
        &a.wedge,
        &a.wheel,
        &a.brie,
        &a.hole,
        &a.hole_full[0],
        &a.hole_full[1],
        &a.hole_full[2],
    ];
    for (i, sp) in sheet.iter().enumerate() {
        let (x, y) = ((i % 6) as i32 * 52 + 4, (i / 6) as i32 * 52 + 4);
        s.blit_scaled(sp, x, y, 3, false);
    }
    save(&s, "sprites");

    r.draw_title(&mut s, 1.3, false);
    save(&s, "title");

    let best: Vec<Option<u32>> = levels()
        .iter()
        .enumerate()
        .map(|(i, l)| match i {
            0 => Some(l.par),
            1 => Some(l.par + 3),
            2 => Some(l.par + 20),
            _ => None,
        })
        .collect();
    let sel = SelectView {
        levels: levels(),
        best: &best,
        cursor: 2,
        unlocked: 4,
        order: false,
        cheese: true,
        t: 0.0,
    };
    r.draw_select(&mut s, &sel);
    save(&s, "select");
    let order = SelectView {
        order: true,
        cursor: levels().len(),
        ..sel
    };
    r.draw_select(&mut s, &order);
    save(&s, "order");

    for (i, level) in levels().iter().enumerate() {
        let w = World::new(Arc::clone(level));
        r.draw_play(&mut s, &play_view(&w, &fx, i + 1, 0.0, None));
        save(&s, &format!("level_{:02}", i + 1));
    }

    // Solve the brie level and stop mid-slide, then at the end.
    let level = &levels()[3];
    let Outcome::Solved(path) = solve(level, 1_000_000) else {
        panic!("level 4 unsolvable")
    };
    let mut w = World::new(Arc::clone(level));
    for (k, &d) in path.iter().enumerate() {
        let before = w.state.clone();
        w.step(d);
        if k + 1 == path.len() {
            let mut a = Anim::new(before, &w.state);
            a.update(a.dur * 0.5);
            r.draw_play(&mut s, &play_view(&w, &fx, 4, 0.0, Some(&a)));
            save(&s, "midslide");
        }
    }
    assert_eq!(w.state.status, Status::Solved);
    // Every brie slide so far, fully grown.
    let trails: Vec<Trail> = w
        .events
        .iter()
        .filter_map(|e| match *e {
            Event::Pushed {
                kind: CheeseKind::Brie,
                from,
                to,
            } => Some(Trail {
                from,
                to,
                born: -10.0,
            }),
            _ => None,
        })
        .collect();
    let mut v = play_view(&w, &fx, 4, 0.1, None);
    v.trails = &trails;
    r.draw_play(&mut s, &v);
    save(&s, "plopped");
    r.draw_play(&mut s, &play_view(&w, &fx, 4, 2.0, None));
    save(&s, "solved");

    // Walk into the cat on "Here, Kitty".
    let mut w = World::new(Arc::clone(&levels()[5]));
    for d in [Dir::Up, Dir::Up] {
        w.step(d);
    }
    if w.state.status != Status::Caught {
        for d in [Dir::Left, Dir::Up, Dir::Right, Dir::Right, Dir::Down] {
            if w.state.status == Status::Caught {
                break;
            }
            w.step(d);
        }
    }
    r.draw_play(&mut s, &play_view(&w, &fx, 6, 0.5, None));
    save(&s, "caught");
    r.draw_play(&mut s, &play_view(&w, &fx, 6, 1.15, None));
    save(&s, "defeat_fading");
    r.draw_play(&mut s, &play_view(&w, &fx, 6, 3.0, None));
    save(&s, "defeat");

    // Idle on level 1: an ear twitch and a squeak.
    let w = World::new(Arc::clone(&levels()[0]));
    let mut v = play_view(&w, &fx, 1, 0.0, None);
    v.idle = 9.0;
    v.squeak_age = Some(0.2);
    v.t = 0.05;
    r.draw_play(&mut s, &v);
    save(&s, "squeak");

    r.draw_intro(&mut s, 1, 1.0);
    save(&s, "intro");
    r.draw_intro(&mut s, 2, 1.0);
    save(&s, "intro_episode_2");
    r.draw_intro(&mut s, 3, 1.0);
    save(&s, "intro_episode_3");

    r.draw_end(&mut s, 27, 33, 2.0);
    save(&s, "end");
    r.draw_onward(&mut s, 1.0);
    save(&s, "onward");
    r.draw_farewell(&mut s, 1.0);
    save(&s, "farewell");

    // Audio, to listen to without starting the game.
    let wav = |name: &str, pcm: &[f32]| {
        let path = format!("{dir}/{name}.wav");
        std::fs::write(&path, wav_bytes(pcm)).expect("write wav");
        println!("{path}");
    };
    for (name, song) in [
        ("music_title", songs::title()),
        ("music_game", songs::game()),
        ("music_extro", songs::extro()),
    ] {
        let secs = song.duration();
        let mut p = Player::new(song);
        let pcm: Vec<f32> = (0..(secs * SAMPLE_RATE as f32) as usize)
            .map(|_| p.next_sample())
            .collect();
        wav(name, &pcm);
    }
    let bank = cheese::sound::sfx::Bank::new();
    wav("thunder", &bank.thunder);
}

/// 16-bit mono PCM WAV.
///
/// Writes the canonical 44-byte RIFF header (PCM format, one channel, [`SAMPLE_RATE`],
/// two bytes per sample) followed by the samples, clamped to `-1.0..=1.0` and scaled to
/// `i16`, little-endian.
fn wav_bytes(pcm: &[f32]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

/// A [`PlayView`] of `world` with neutral settings (no best score, no trails, no idle
/// time, not paused, at `t = 1.0`). `end_t` is the time since the level ended, which
/// selects how far the solved or caught panel has progressed.
fn play_view<'a>(
    world: &'a World,
    fx: &'a Fx,
    number: usize,
    end_t: f32,
    anim: Option<&'a Anim>,
) -> PlayView<'a> {
    PlayView {
        world,
        anim,
        fx,
        number,
        best: None,
        t: 1.0,
        end_t,
        paused: false,
        trails: &[],
        idle: 0.0,
        squeak_age: None,
    }
}
