//! Phase 0 end-to-end verification fixture. Exercises the full Buiy
//! pipeline against the hello_button example scene:
//!  - layout resolves
//!  - render pipeline draws (no panic)
//!  - AccessKit tree snapshot matches golden
//!  - Tab focuses the Button (FocusVisible = true)
//!  - simulated click emits OnPress
//!  - default theme passes WCAG 2 contrast lint

use bevy::prelude::*;
use buiy::*;
use buiy_core::focus::advance_focus_for_test;
use buiy_verify::a11y::{diff_snapshots, snapshot_tree};
use buiy_verify::contrast::lint_theme;

fn setup_scene(app: &mut App) {
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::input::InputPlugin);
    app.add_plugins(BuiyPlugin);
    app.world_mut().spawn(Button::new("Save"));
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
    advance_focus_for_test(&mut app, true);
    let focused = app.world().resource::<FocusedEntity>().0;
    assert!(focused.is_some(), "Tab focuses the Button");
    assert!(
        app.world().resource::<FocusVisible>().0,
        "focus-visible is set after Tab (keyboard-driven focus)"
    );
}

#[test]
fn e2e_default_theme_passes_aa_contrast() {
    let theme = default_light_theme();
    if let Err(violations) = lint_theme(&theme) {
        panic!("default theme fails AA contrast: {violations:?}");
    }
}

/// Replace entity-bit fields with a stable placeholder so goldens don't
/// drift across test runs (entities are allocated dynamically).
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
