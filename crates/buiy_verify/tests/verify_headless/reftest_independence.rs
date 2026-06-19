//! Pure-CPU lint self-test (NOT #[ignore]): a reference that ILLEGALLY carries
//! the forbidden marker trips assert_reference_independent (RED); the canonical
//! disjoint reference passes (GREEN). reftests.md § Verification #4. The lint
//! is itself tested, not trusted.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Containment, ContentVisibility, Style};
use buiy_verify::metric::FuzzBudget;
use buiy_verify::reftest::{
    ComponentMarker, IndependenceRule, RefCase, RefKind, assert_reference_independent,
    default_rules,
};

fn empty(_: &mut App) {}

fn visible_box(app: &mut App) {
    // A plain `Style` carries a default `Containment` (content_visibility:
    // Visible) — the lint's check is on the FIELD VALUE (Hidden), so this
    // legitimately-disjoint reference does not trip it.
    app.world_mut().spawn((Node, Style::default()));
}

fn hidden_box(app: &mut App) {
    // `Style` is a Bundle that already supplies `Containment`; set the field
    // via the builder (spawning a second `Containment` alongside would be a
    // duplicate-component panic, NOT a lint trip).
    app.world_mut().spawn((
        Node,
        Style::default().containment(Containment {
            content_visibility: ContentVisibility::Hidden,
            ..default()
        }),
    ));
}

#[test]
fn legal_reference_passes_the_lint() {
    let case = RefCase {
        name: "cv_green",
        kind: RefKind::Mismatch,
        test: empty,
        reference: visible_box,
        fuzz: FuzzBudget::EXACT,
    };
    assert_reference_independent(&case, &default_rules());
}

#[test]
#[should_panic(expected = "reference for `content-visibility` illegally contains")]
fn illegal_reference_trips_the_lint() {
    let case = RefCase {
        name: "cv_red",
        kind: RefKind::Mismatch,
        test: empty,
        reference: hidden_box,
        fuzz: FuzzBudget::EXACT,
    };
    assert_reference_independent(
        &case,
        &[IndependenceRule {
            feature: "content-visibility",
            forbidden_in_reference: &[ComponentMarker::ContentVisibilityHidden],
        }],
    );
}
