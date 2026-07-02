//! `buiy_probe` — the reference **agent feedback loop** (spec §4 Track A).
//!
//! This example is the front door for a coding agent (human or LLM) that wants
//! to *see* what a Buiy scene renders **without a GPU, a window, or a display**.
//! The loop is three moves:
//!
//! 1. **Author** — edit [`scene`] below (the one slot you change).
//! 2. **Run** — `cargo run -p buiy_probe`, which stands the scene up under
//!    [`BuiyProbePlugin`](buiy::BuiyProbePlugin) (GPU-free, no adapter).
//! 3. **Inspect** — read the printed [`snapshot_report`](buiy::probe::snapshot_report):
//!    a Playwright-style semantic tree (roles / accessible names / state / layout
//!    rects) plus a `--- text & layout ---` section that surfaces plain text and
//!    flags zero-size ("invisible") content.
//!
//! Layout, the a11y tree, and widget state are **pure ECS projections** (a Taffy
//! solve + an accessibility pass) — no rasterization is involved, so the report
//! is exact and the run needs no wgpu. To *drive* the scene (click a button, set
//! a value, wait for a condition) use the verbs in [`buiy::probe`] over the same
//! world. See `src/main.rs` for the end-to-end loop.

use bevy::prelude::*;
use buiy::prelude::*;

/// The one slot an agent edits: spawn the UI to inspect.
///
/// Widgets are spawned as roots here for brevity — each lays itself out and
/// projects its own a11y node. Real apps nest them under layout containers; the
/// probe reads whatever tree you build. This demo authors a settings-card shape:
/// a title, a toggle, and two actions, plus one **plain, role-less** label to
/// show what the bare semantic tree drops but the report's text section catches.
pub fn scene(world: &mut World) {
    // A plain, role-less title. It becomes NO a11y node (no `A11yRole`), so only
    // the report's `--- text & layout ---` section can observe it — exactly the
    // gap the serializer exists to close.
    world.spawn((
        Node,
        Style::default().width_px(220.0),
        Text("Settings".to_string()),
        FontSize(18.0),
    ));

    // A toggle and two actions — real widgets, each with a role + accessible name
    // (and, for the checkbox, checked/unchecked state) in the semantic tree.
    world.spawn(Checkbox::new("Dark mode"));
    world.spawn(Button::new("Save"));
    world.spawn(Button::new("Cancel"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use buiy::probe::snapshot_report;

    /// The example is self-verifying in the headless gate: authoring [`scene`],
    /// running it under the probe preset, and reading the report surfaces every
    /// widget's role + name AND the plain-text title — proving the reference loop
    /// works with no GPU. (Kept in lock-step with `main.rs`'s plugin set.)
    #[test]
    fn scene_report_surfaces_roles_and_plain_text() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(BuiyProbePlugin);

        scene(app.world_mut());
        for _ in 0..8 {
            app.update();
        }

        let report = snapshot_report(app.world_mut());
        let (tree, text) = report
            .split_once("--- text & layout ---")
            .unwrap_or_else(|| panic!("report missing text & layout section:\n{report}"));

        assert!(
            tree.contains("Button \"Save\""),
            "tree must carry the Save button:\n{report}",
        );
        assert!(
            tree.contains("Checkbox \"Dark mode\""),
            "tree must carry the Dark-mode checkbox:\n{report}",
        );
        assert!(
            text.contains("Settings"),
            "text section must surface the plain-text title:\n{report}",
        );
    }
}
