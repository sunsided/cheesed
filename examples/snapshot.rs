//! Headless screenshots of the framebuffer, for checking art without opening a window:
//! `cargo run --example snapshot -- out_dir` writes title.png, select.png, a frame of every
//! level, and the solved / caught panels.

use std::sync::Arc;

use cheese::gfx::{Screen, png};
use cheese::logic::solver::{Outcome, solve};
use cheese::logic::{CheeseKind, Dir, Event, Status, World, levels};
use cheese::render::anim::Anim;
use cheese::render::fx::Fx;
use cheese::render::{PlayView, Renderer, SelectView, Trail};

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
        &a.mouse_sit,
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

    r.draw_title(&mut s, 1.3);
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
                turn: 0,
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
    r.draw_play(&mut s, &play_view(&w, &fx, 6, 1.0, None));
    save(&s, "caught");

    r.draw_end(&mut s, 27, 33, 2.0);
    save(&s, "end");
}

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
    }
}
