//! GPU lane (`--ignored`): proves the reftest engine can both PASS and FAIL.
//! reftests.md § Verification #3 — a scene-vs-itself match passes at (0,0); a
//! scene-vs-different match fails (guards a vacuous green); a scene-vs-itself
//! mismatch fails. Real adapter (RX 6700 XT here) / pinned lavapipe in CI.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::Background;
use buiy_verify::metric::FuzzBudget;
use buiy_verify::reftest::{RefCase, RefKind, run_reftest};
use std::borrow::Cow;

/// A single 40×40 fill at (left,8) in `token` color. Installs the token so the
/// scene is self-contained across the two captures `run_reftest` drives.
fn box_at(app: &mut App, left: f32, token: &'static str) {
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert(token.into(), Color::srgb(0.90, 0.10, 0.10));
    }
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(8.0)),
                    left: Sizing::Length(Length::px(left)),
                    ..default()
                })
                .width_px(40.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed(token)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[e]);
}

fn red_at_8(app: &mut App) {
    box_at(app, 8.0, "test.fill.a");
}
fn red_at_120(app: &mut App) {
    box_at(app, 120.0, "test.fill.a");
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn match_of_scene_with_itself_passes() {
    let case = RefCase {
        name: "self_match",
        kind: RefKind::Match,
        test: red_at_8,
        reference: red_at_8,
        fuzz: FuzzBudget::EXACT,
    };
    let outcome = run_reftest(&case);
    assert!(
        outcome.passed,
        "self-match must pass at (0,0): {:?}",
        outcome.diff
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn match_of_two_different_scenes_fails() {
    let case = RefCase {
        name: "different_match_fails",
        kind: RefKind::Match,
        test: red_at_8,
        reference: red_at_120,
        fuzz: FuzzBudget::EXACT,
    };
    let outcome = run_reftest(&case);
    assert!(
        !outcome.passed,
        "differing scenes must NOT match (vacuous-green guard)"
    );
    assert!(
        outcome.report_path.is_some(),
        "failure emits a triage report"
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn mismatch_of_scene_with_itself_fails() {
    let case = RefCase {
        name: "self_mismatch_fails",
        kind: RefKind::Mismatch,
        test: red_at_8,
        reference: red_at_8,
        fuzz: FuzzBudget::EXACT,
    };
    let outcome = run_reftest(&case);
    assert!(!outcome.passed, "a scene cannot mismatch itself");
}
