//! Track A — the agent probe / "eyes." Proves the `BuiyProbePlugin` preset stands
//! up a Buiy scene **headless + GPU-free** (no wgpu adapter, no window, no
//! `RenderApp`) and that the agent-facing feedback loop — `snapshot` +
//! `snapshot_report` — reads it back. These tests RUN in the ordinary headless
//! gate; there is nothing to `#[ignore]` because the probe never rasterizes.

use bevy::prelude::*;
use buiy::{BuiyProbePlugin, Button, FontSize, Node, Style, Text};
use buiy_core::a11y::A11yRole;
use buiy_core::a11y::inprocess::{TreeView, snapshot};
use buiy_core::a11y::snapshot_report;

/// The minimal GPU-free substrate `BuiyProbePlugin` documents: `MinimalPlugins`
/// (no render/winit), `AssetPlugin` (the text stack's fallback font), and
/// `InputPlugin` (focus/keymap read `Res<ButtonInput<KeyCode>>`).
fn probe_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BuiyProbePlugin);
    app
}

/// Wave 1 — the preset composes a working GPU-free stack: a spawned `Button` lays
/// out and projects an `A11yRole::Button` node named "Save" through the shipped
/// in-process `snapshot`, with **no camera, no window, no adapter**.
#[test]
fn probe_snapshot_sees_button_headless_gpu_free() {
    let mut app = probe_app();
    app.world_mut().spawn(Button::new("Save"));

    // Layout + a11y build + text shaping settle within a few frames.
    for _ in 0..8 {
        app.update();
    }

    let tree = snapshot(app.world_mut(), TreeView::Unmerged);
    assert!(
        tree.by_role(A11yRole::Button).any(|n| n.name == "Save"),
        "probe snapshot must expose the Button node named \"Save\"; tree = {tree:#?}",
    );
}

/// Wave 2 — the serializer surfaces the two gaps the bare `SemanticTree` leaves:
/// the `Button` shows up as an a11y node in the tree section, and a **plain,
/// role-less `Text`** ("Settings") — which the semantic tree drops entirely —
/// shows up in the `--- text & layout ---` section.
#[test]
fn snapshot_report_surfaces_a11y_node_and_plain_text() {
    let mut app = probe_app();
    app.world_mut().spawn(Button::new("Save"));
    // A plain, role-less label: it becomes no a11y node (no `A11yRole`), so only
    // the report's text section can observe it.
    app.world_mut().spawn((
        Node,
        Style::default().width_px(200.0),
        Text("Settings".to_string()),
        FontSize(16.0),
    ));

    for _ in 0..8 {
        app.update();
    }

    let report = snapshot_report(app.world_mut());

    // Split at the section marker so each assertion targets the right half.
    let (tree_section, text_section) = report
        .split_once("--- text & layout ---")
        .unwrap_or_else(|| panic!("report is missing the text & layout section:\n{report}"));

    assert!(
        tree_section.contains("Button \"Save\""),
        "semantic-tree section must carry the Button a11y node; report =\n{report}",
    );
    assert!(
        text_section.contains("Settings"),
        "text section must surface the plain (non-a11y) Text \"Settings\"; report =\n{report}",
    );
}

/// F1 regression — a **nested** node's reported `@x,y` is its ABSOLUTE top-left
/// (accumulated through its ancestors), not its parent-relative offset. The
/// `ResolvedLayout.position` a naive serializer reads is parent-relative; the
/// probe must source absolute coords from `GlobalTransform` (which the transform
/// bridge populates GPU-free under `BuiyProbePlugin`). Depth-0 scenes can't
/// observe this — only a nested one where absolute ≠ relative.
#[test]
fn nested_node_reports_absolute_position() {
    let mut app = probe_app();

    // root(padding 40) → middle(padding 30) → Button("Nested"). The button's
    // parent-relative offset within `middle` is (30,30); its ABSOLUTE top-left
    // accumulates both paddings to (70,70).
    app.world_mut()
        .spawn((Node, Style::default().padding(40.0)))
        .with_children(|root| {
            root.spawn((Node, Style::default().padding(30.0)))
                .with_children(|middle| {
                    middle.spawn(Button::new("Nested"));
                });
        });

    for _ in 0..8 {
        app.update();
    }

    let report = snapshot_report(app.world_mut());
    let line = report
        .lines()
        .find(|l| l.contains("Button \"Nested\""))
        .unwrap_or_else(|| panic!("report has no Nested button line:\n{report}"));

    assert!(
        line.contains("@70,70"),
        "nested button must report its ABSOLUTE top-left @70,70 (40+30 padding), \
         not its parent-relative @30,30; line = {line:?}\nfull report:\n{report}",
    );
}
