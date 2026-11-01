//! Cheesed! - a mouse pushes cheese into its mouse holes.
//!
//! The crate is split so everything except [`app`] is free of toolkit types and can be unit
//! tested and rendered headlessly (see `examples/snapshot.rs`):
//!
//! - [`logic`]: the puzzle rules, levels and a solver; deterministic and tested.
//! - [`gfx`]: a 320x200 CPU framebuffer, palette, bitmap fonts and ASCII-art sprites.
//! - [`render`]: draws the screens into the framebuffer.
//! - [`sound`]: every sound is synthesized in code.
//! - [`platform`]: launch options and progress storage, natively and in the browser.
//! - [`app`]: the game-toolkit glue (window, input, audio device, presenting the framebuffer).
//!
//! The data flows one way each frame: [`app`] turns input into moves on a
//! [`logic::World`], reacts to the world's events with sounds and effects, then asks
//! [`render`] to draw the current mode into a [`gfx::Screen`]. That 320x200 RGB
//! image is converted to RGBA and drawn as one scaled texture, which is why the game looks
//! identical on every window size and on the web.

// Pixel-drawing calls take x, y, w, h plus colors; bundling them would only add noise.
#![allow(clippy::too_many_arguments)]

pub mod app;
pub mod gfx;
pub mod logic;
pub mod platform;
pub mod render;
pub mod sound;
