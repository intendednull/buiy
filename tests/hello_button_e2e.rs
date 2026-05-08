//! Phase 0 end-to-end verification fixture. Exercises the full Buiy
//! pipeline against the hello_button example scene:
//!  - layout resolves
//!  - render pipeline plugin loads (no panic on app.update())
//!  - AccessKit tree snapshot matches golden
//!  - Tab focuses the Button (FocusVisible = true)
//!
//! Coverage NOT in this file:
//! - Visual regression (CI gate #2): requires a wgpu adapter; covered by
//!   `crates/buiy_verify/tests/visual.rs` for the diff primitive and by
//!   the (currently `#[ignore]`d) `pipeline_registers_in_render_app` test
//!   in `crates/buiy_core/tests/render_smoke.rs`. Real screenshot e2e
//!   lands when CI runners with lavapipe / a real GPU come online.
//! - Click → OnPress: covered in `crates/buiy_widgets/tests/button.rs`.
//!   Driving a real click here would need synthetic pointer events;
//!   v0.x work per `buiy-input-events-design`.
//! - Contrast linter: covered in `crates/buiy_verify/tests/contrast.rs`
//!   (`lint_theme_passes_for_default_light`). Not duplicated here.

use bevy::prelude::*;
use buiy::*;
use buiy_verify::a11y::{diff_snapshots, snapshot_tree};

fn setup_scene(app: &mut App) {
    // BuiyPlugin reads `ButtonInput<KeyCode>` + `ButtonInput<MouseButton>`;
    // init the resources directly instead of pulling in InputPlugin so the
    // PreUpdate clear-system doesn't wipe test-driven presses before
    // `handle_tab` runs in `BuiySet::Input`.
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
    app.add_plugins(BuiyPlugin);
    app.world_mut().spawn(Button::new("Save"));
}

fn press_tab(app: &mut App) {
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        keys.press(KeyCode::Tab);
    }
    app.update();
}

#[test]
fn e2e_layout_and_a11y_tree_match_golden() {
    let mut app = App::new();
    setup_scene(&mut app);
    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snap = snapshot_tree(builder.snapshot());

    let golden = std::fs::read_to_string("tests/fixtures/hello_button/golden_a11y_tree.json")
        .expect("golden file present");
    let golden = canonicalize_entity_ids(&golden);
    let snap = canonicalize_entity_ids(&snap);

    assert!(
        diff_snapshots(&snap, &golden).is_none(),
        "AccessKit tree drift; expected = {golden}, actual = {snap}"
    );
}

#[test]
fn e2e_tab_focuses_button() {
    let mut app = App::new();
    setup_scene(&mut app);
    app.update();
    press_tab(&mut app);
    let focused = app.world().resource::<FocusedEntity>().0;
    assert!(focused.is_some(), "Tab focuses the Button");
    assert!(
        app.world().resource::<FocusVisible>().0,
        "focus-visible is set after Tab (keyboard-driven focus)"
    );
}

/// Replace entity-bit fields with a stable placeholder so goldens don't
/// drift across test runs (entities are allocated dynamically).
///
/// LINT: Naive scanner. Assumes `"entity"` values in the snapshot are
/// scalars (currently `u64` per `buiy_verify::a11y::WireNode`). Nested
/// objects or strings containing `,`/`}` would corrupt the output. If
/// the wire format ever grows non-scalar entity values, replace this
/// with a `serde_json::Value` round-trip.
fn canonicalize_entity_ids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            // Look for "entity":N pattern.
            let mut buf = String::from(c);
            while let Some(&n) = chars.peek() {
                if n == '"' {
                    buf.push(chars.next().unwrap());
                    break;
                }
                buf.push(chars.next().unwrap());
            }
            if buf == "\"entity\"" {
                out.push_str(&buf);
                // Skip until comma or }, replacing the value.
                while let Some(&n) = chars.peek() {
                    if n == ',' || n == '}' {
                        break;
                    }
                    chars.next();
                }
                out.push_str(":0");
            } else {
                out.push_str(&buf);
            }
        } else {
            out.push(c);
        }
    }
    out
}
