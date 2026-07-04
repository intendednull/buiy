//! Windowed Dooduel (needs a display): `cargo run -p dooduel`.
//!
//! Installs Dooduel + its runtime plugins via [`dooduel::install_runtime`] — the
//! SAME set the wasm web bin (`dooduel_web`) uses, so native and web never drift.
//! (`install_runtime` = theme + the F7 poll-clock driver + the viewport/mobile
//! shell + the drawing canvases + podium confetti + persistence.)

use buiy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);
    dooduel::install_runtime(&mut app);
    app.run();
}
