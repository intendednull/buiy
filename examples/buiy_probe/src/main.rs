//! `cargo run -p buiy_probe` — the reference agent loop end to end.
//!
//! Stands up [`buiy_probe::scene`] under [`BuiyProbePlugin`] (GPU-free), prints
//! the semantic-tree report, then demonstrates *driving* the scene: locate the
//! "Save" button by role + name and click it, and re-read the tree. No window,
//! no adapter, no display — this runs anywhere `cargo test` does.

use bevy::prelude::*;
use buiy::prelude::*;
use buiy::probe::*;

fn main() {
    // The minimal GPU-free substrate `BuiyProbePlugin` documents: `MinimalPlugins`
    // (no render/winit), `AssetPlugin` (the text stack's fallback font), and
    // `InputPlugin` (focus/keymap read `Res<ButtonInput<KeyCode>>`).
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BuiyProbePlugin);

    buiy_probe::scene(app.world_mut());

    // Layout + a11y build + text shaping settle within a few frames.
    for _ in 0..8 {
        app.update();
    }

    println!(
        "== snapshot after author + run ==\n{}",
        snapshot_report(app.world_mut())
    );

    // Drive step: locate a widget by role + accessible name (the strict locator
    // refuses to guess when the match is ambiguous), then click it. The checkbox
    // is the target because the click is *observable* — its state flips
    // `[unchecked]` → `[checked]` in the very next snapshot, proving the drive
    // loop mutates real ECS state (not just that the verb returned `Ok`).
    match get_by_role(app.world_mut(), A11yRole::Checkbox, Some("Dark mode"), None) {
        Ok(checkbox) => {
            if let Err(err) = click(app.world_mut(), checkbox) {
                eprintln!("click(\"Dark mode\") failed: {err:?}");
            } else {
                app.update();
                println!(
                    "\n== snapshot after clicking \"Dark mode\" (state flips) ==\n{}",
                    snapshot_report(app.world_mut())
                );
            }
        }
        Err(err) => eprintln!("could not locate the \"Dark mode\" checkbox: {err:?}"),
    }
}
