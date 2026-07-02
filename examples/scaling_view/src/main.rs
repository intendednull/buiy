//! Windowed `buiy_view` scaling/composition demo (needs a display):
//! `cargo run -p scaling_view --bin scaling_view`.
//!
//! Two embedded Counters (message-lifted via `.map`), a `when`-gated details panel, and an
//! async "Load" that folds its result back through the funnel as a value (`Cmd::task`) — the
//! counters stay responsive while it loads.

use bevy::prelude::*;
use buiy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);
    scaling_view::install(&mut app);
    app.run();
}
