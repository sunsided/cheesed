//! Cheesed! - a mouse pushes cheese into its mouse holes.
//!
//! The binary is only a launcher: everything lives in the library crate so the examples
//! and tests can use it too. See [`cheese::app`] for the game itself.

use cheese::app::{Cheese, config};
use game_toolkit::prelude::*;

/// Sets up logging (native only; on the web the toolkit logs to the browser console) and hands the
/// game to the toolkit's run loop, which returns when the window closes. Logging defaults
/// to warnings and can be raised with `RUST_LOG`.
fn main() -> Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    cheese::platform::silence_when_detached();
    run::<Cheese>(config())
}
