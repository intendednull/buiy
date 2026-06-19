//! GPU lane (`--ignored`): two real Tier-4 reftest pairings
//! (reftests.md § Authoring patterns).
//!
//! 1. flex `justify-content: SpaceBetween` == three literal-offset boxes
//!    (reference routes through the literal-Node layer — NOT flex). `match`.
//! 2. `content-visibility: hidden` != the identical VISIBLE subtree — the
//!    `!=` anti-test proving the feature suppresses paint. `mismatch`.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{
    Containment, ContentVisibility, Inset, JustifyContent, Length, Sizing, Style,
};
use buiy_core::render::ColorToken;
use buiy_core::render::components::Background;
use std::borrow::Cow;

/// Install the shared fill token both halves of every pairing reference.
fn install_fill(app: &mut App) {
    let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
    theme
        .colors
        .insert("test.fill.a".into(), Color::srgb(0.90, 0.10, 0.10));
}

/// A block-flow `width × 40` fill box (a flex child).
fn fill_box(width: f32) -> impl Bundle {
    (
        Node,
        Style::default().width_px(width).height_px(40.0),
        Background {
            color: ColorToken::Token(Cow::Borrowed("test.fill.a")),
        },
    )
}

/// An absolutely-positioned 40×40 fill box at literal `(left, 0)` — the
/// primitive / literal-offset layer that bypasses the flex solver entirely.
fn abs_box(app: &mut App, left: f32) -> Entity {
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    left: Sizing::Length(Length::px(left)),
                    top: Sizing::Length(Length::px(0.0)),
                    ..default()
                })
                .width_px(40.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.fill.a")),
            },
        ))
        .id()
}

// ---- Case 1: flex justify-content: SpaceBetween == three literal offsets ----

fn flex_justify(app: &mut App) {
    install_fill(app);
    let a = app.world_mut().spawn(fill_box(40.0)).id();
    let b = app.world_mut().spawn(fill_box(40.0)).id();
    let c = app.world_mut().spawn(fill_box(40.0)).id();
    // Three 40px boxes in a 200px row, SpaceBetween → x = 0, 80, 160.
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .justify_content(JustifyContent::SpaceBetween)
                .width_px(200.0)
                .height_px(40.0),
        ))
        .add_children(&[a, b, c]);
}

fn literal_offsets(app: &mut App) {
    install_fill(app);
    // The disjoint oracle: three boxes at the SpaceBetween-resolved literal
    // coordinates via the absolute / literal-Node layer — no flex solver, so a
    // flex-justify bug cannot be shared by this reference.
    let a = abs_box(app, 0.0);
    let b = abs_box(app, 80.0);
    let c = abs_box(app, 160.0);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[a, b, c]);
}

// ---- Case 2: content-visibility: hidden != the visible subtree ----

fn subtree(app: &mut App, hidden: bool) {
    install_fill(app);
    let child = app.world_mut().spawn(fill_box(80.0)).id();
    // `Style` is a Bundle that already supplies `Containment`; set the
    // content-visibility via the builder (a second `Containment` alongside
    // would be a duplicate-component panic).
    let containment = if hidden {
        Containment {
            content_visibility: ContentVisibility::Hidden,
            ..default()
        }
    } else {
        Containment::default()
    };
    let p = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    left: Sizing::Length(Length::px(20.0)),
                    top: Sizing::Length(Length::px(20.0)),
                    ..default()
                })
                .width_px(80.0)
                .height_px(40.0)
                .containment(containment),
        ))
        .id();
    app.world_mut().entity_mut(p).add_children(&[child]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[p]);
}

fn cv_visible(app: &mut App) {
    subtree(app, false);
}
fn cv_hidden(app: &mut App) {
    subtree(app, true);
}

buiy_verify::reftest!(
    match,
    flex_justify_eq_literal,
    flex_justify,
    literal_offsets
);
buiy_verify::reftest!(mismatch, cv_hidden_actually_hides, cv_visible, cv_hidden);

#[test]
fn cv_hidden_reference_is_independent() {
    use buiy_verify::metric::FuzzBudget;
    use buiy_verify::reftest::{RefCase, RefKind, assert_reference_independent, default_rules};
    // The REFERENCE in case 2 is `cv_visible`; it must carry NO Hidden marker.
    let case = RefCase {
        name: "cv_hidden_actually_hides",
        kind: RefKind::Mismatch,
        test: cv_hidden,
        reference: cv_visible,
        fuzz: FuzzBudget::EXACT,
    };
    assert_reference_independent(&case, &default_rules());
}
