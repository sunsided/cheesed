//! Cosmetic effects: crumb particles, a little shake. Never affect the puzzle.

use crate::gfx::palette::{self, Rgb};
use crate::gfx::{Screen, W};

#[derive(Clone, Copy, Debug)]
struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
    color: Rgb,
}

#[derive(Default)]
pub struct Fx {
    particles: Vec<Particle>,
    shake: f32,
    /// Tiny LCG; effects don't need the game's RNG.
    seed: u32,
}

impl Fx {
    pub fn new() -> Self {
        Self {
            seed: 0x9e37_79b9,
            ..Default::default()
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A puff of cheese crumbs and sparkles at a pixel position.
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

    pub fn shake(&mut self, amount: f32) {
        self.shake = self.shake.max(amount);
    }

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
