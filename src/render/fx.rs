//! Cosmetic effects: crumb particles, a little shake. Never affect the puzzle.
//!
//! Units: positions are screen pixels on the 320x200 [`Screen`] (top-left origin, y down),
//! velocities are pixels per second, times are seconds. `app.rs` owns one [`Fx`], feeds it
//! events (a cheese plopped: [`Fx::burst`] once the move's animation has finished; a
//! push that is too heavy: a small [`Fx::shake`]; caught: a bigger shake; solved:
//! [`Fx::celebrate`]), steps it with [`Fx::update`] every frame, and applies
//! [`Fx::shake_offset`] when it blits the finished screen to the window, so the shake moves
//! the whole picture rather than individual sprites. Particles are drawn into the screen by
//! the play-screen renderer with [`Fx::draw`].
//!
//! The effects use their own tiny random generator, so spawning particles never consumes
//! numbers from the game's RNG and cannot change anything deterministic.

use crate::gfx::palette::{self, Rgb};
use crate::gfx::{Screen, W};

/// One crumb or confetti piece: a point under gravity that dies after a while.
#[derive(Clone, Copy, Debug)]
struct Particle {
    /// Horizontal position, screen pixels.
    x: f32,
    /// Vertical position, screen pixels (down is positive).
    y: f32,
    /// Horizontal velocity, pixels per second.
    vx: f32,
    /// Vertical velocity, pixels per second; gravity adds 160 px/s every second.
    vy: f32,
    /// Seconds left before the particle disappears.
    life: f32,
    /// The lifetime it started with, to tell young particles from old ones in `draw`.
    max: f32,
    /// Its colour, picked from the palette the spawner passed in.
    color: Rgb,
}

/// All live particles and the current screen shake.
#[derive(Default)]
pub struct Fx {
    /// Live particles; dead ones are dropped in [`Fx::update`].
    particles: Vec<Particle>,
    /// Current shake amplitude in screen pixels; decays by 12 per second.
    shake: f32,
    /// Tiny LCG; effects don't need the game's RNG.
    seed: u32,
}

impl Fx {
    /// No particles, no shake, and a fixed seed, so the effects play out the same way on
    /// every run.
    pub fn new() -> Self {
        Self {
            seed: 0x9e37_79b9,
            ..Default::default()
        }
    }

    /// Next pseudo-random number in 0..1: a 32-bit linear congruential generator (the
    /// Numerical Recipes constants), keeping the top 24 bits.
    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A puff of cheese crumbs and sparkles at a pixel position.
    ///
    /// Spawns `n` particles at `(x, y)` (screen pixels), flying out in random directions at
    /// 20..70 px/s with an extra 30 px/s upward kick, so the puff pops up before gravity
    /// pulls it down. Each lives 0.35..0.75 s. Colours cycle through `colors` (which must
    /// not be empty).
    pub fn burst(&mut self, x: f32, y: f32, n: usize, colors: &[Rgb]) {
        for i in 0..n {
            let a = self.rand() * std::f32::consts::TAU;
            let s = 20.0 + self.rand() * 50.0;
            let life = 0.35 + self.rand() * 0.4;
            self.particles.push(Particle {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s - 30.0,
                life,
                max: life,
                color: colors[i % colors.len()],
            });
        }
    }

    /// Confetti across the top of the screen for a solved level.
    ///
    /// 60 pieces at random x across the screen width and y = 10..30, falling at 20..70 px/s
    /// with a sideways drift of up to 20 px/s either way, living 0.8..1.6 s, in cheese
    /// yellows and white.
    pub fn celebrate(&mut self) {
        let colors = [
            palette::YELLOW,
            palette::YELLOW_LT,
            palette::CHEESE_DK,
            palette::WHITE,
        ];
        for i in 0..60 {
            let x = self.rand() * W as f32;
            let life = 0.8 + self.rand() * 0.8;
            let vx = (self.rand() - 0.5) * 40.0;
            let vy = 20.0 + self.rand() * 50.0;
            let y = 10.0 + self.rand() * 20.0;
            self.particles.push(Particle {
                x,
                y,
                vx,
                vy,
                life,
                max: life,
                color: colors[i % colors.len()],
            });
        }
    }

    /// Start a shake of `amount` screen pixels. A weaker shake never cuts a stronger one
    /// short: the larger of the two wins. The game uses 1.5 for a push that is too heavy
    /// and 3.0 for being caught.
    pub fn shake(&mut self, amount: f32) {
        self.shake = self.shake.max(amount);
    }

    /// Advance by `dt` seconds: move the particles (simple Euler steps, gravity 160 px/s²),
    /// drop the dead ones, and let the shake decay by 12 px per second (a shake of 3 is gone
    /// in a quarter second).
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vy += 160.0 * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        self.shake = (self.shake - dt * 12.0).max(0.0);
    }

    /// Whole-pixel screen offset for the current shake.
    ///
    /// `t` is the app's running time in seconds. Two fast, unrelated oscillations (91 and
    /// 73 rad/s, about 14 and 12 Hz) jitter x and y; the amplitude is the current shake,
    /// capped at 3 px, with y at 60% of x. Rounded to whole pixels, so the low-resolution
    /// screen moves as a block.
    pub fn shake_offset(&self, t: f32) -> (i32, i32) {
        if self.shake <= 0.0 {
            return (0, 0);
        }
        let s = self.shake.min(3.0);
        (
            ((t * 91.0).sin() * s).round() as i32,
            ((t * 73.0).cos() * s * 0.6).round() as i32,
        )
    }

    /// Draw the particles: a 2x2 block while a particle has more than half its life left,
    /// a single pixel after that, so crumbs seem to shrink as they fade.
    pub fn draw(&self, s: &mut Screen) {
        for p in &self.particles {
            let a = (p.life / p.max).min(1.0);
            let (x, y) = (p.x as i32, p.y as i32);
            if a > 0.5 {
                s.fill_rect(x, y, 2, 2, p.color);
            } else {
                s.put(x, y, p.color);
            }
        }
    }
}
