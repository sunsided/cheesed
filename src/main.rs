//! Cheesed! - a mouse pushes cheese into its mouse holes.

use cheese::app::{Cheese, config};
use game_toolkit::prelude::*;

fn main() -> Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    run::<Cheese>(config())
}
