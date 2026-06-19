//! Degenerate-size pipeline robustness (audit #32, campaign T2.4).
//!
//! No fixture previously fed width/height of 0, negative, NaN, or infinity
//! into the real `sync_styles → Taffy → write_resolved_layout` path; the
//! finiteness invariant only ever validated OUTPUT scenes generated in the
//! `0.0..512.0` range, never pathological INPUT. This drives each degenerate
//! `Length::Px` value through the production pipeline and asserts the resulting
//! `ResolvedLayout` is FINITE and NON-NEGATIVE — no NaN/inf leak, no negative
//! size — and that the pipeline does not panic.
//!
//! These are robustness oracles: if a degenerate input DID leak a non-finite
//! or negative `ResolvedLayout`, that is a real bug to root-cause at the
//! production seam, not a value to bake into the assertion.

mod support;

use bevy::math::Vec2;
use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{Length, Sizing, Style};

/// Every degenerate f32 we push through the size pipeline, with a label.
const DEGENERATE: &[(&str, f32)] = &[
    ("zero", 0.0),
    ("negative", -100.0),
    ("nan", f32::NAN),
    ("pos_inf", f32::INFINITY),
    ("neg_inf", f32::NEG_INFINITY),
    ("tiny_negative", -0.0001),
    ("huge", 1.0e30),
];

/// Assert one `ResolvedLayout` field-vector is finite and non-negative.
fn assert_finite_non_negative(label: &str, what: &str, v: Vec2) {
    assert!(
        v.x.is_finite() && v.y.is_finite(),
        "{label}: {what} leaked a non-finite value: {v:?}",
    );
    // Size must never be negative; position is a top-left and is likewise
    // expected non-negative for a single root box at the origin (no negative
    // margins/insets in these fixtures).
    assert!(
        v.x >= 0.0 && v.y >= 0.0,
        "{label}: {what} leaked a negative value: {v:?}",
    );
}

/// One entity's resolved `(position, size)` — both `Vec2` (Copy), so callers
/// can read them out of the World without cloning the non-`Copy`
/// `ResolvedLayout`.
type Geometry = (Vec2, Vec2);

fn geometry(app: &mut App, e: Entity) -> Geometry {
    let rl = app
        .world()
        .get::<ResolvedLayout>(e)
        .expect("ResolvedLayout written (pipeline did not skip the entity)");
    (rl.position, rl.size)
}

/// Spawn a single root box whose WIDTH is the degenerate value (height fixed at
/// a sane 100px), settle, and return its resolved `(position, size)`.
fn resolve_with_width(px: f32) -> Geometry {
    let mut app = support::bare_layout_app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width(Sizing::Length(Length::Px(px)))
                .height_px(100.0),
        ))
        .id();
    support::settle(&mut app);
    geometry(&mut app, e)
}

/// Spawn a root with a degenerate width AND height plus a normal child, settle,
/// and return both `(position, size)` pairs — exercises a degenerate PARENT
/// feeding a child through the tree (not just a leaf).
fn resolve_parent_child(px: f32) -> (Geometry, Geometry) {
    let mut app = support::bare_layout_app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .width(Sizing::Length(Length::Px(px)))
                .height(Sizing::Length(Length::Px(px))),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
    support::settle(&mut app);
    (geometry(&mut app, parent), geometry(&mut app, child))
}

#[test]
fn degenerate_leaf_width_yields_finite_non_negative_layout() {
    for &(label, px) in DEGENERATE {
        let (pos, size) = resolve_with_width(px);
        assert_finite_non_negative(label, "leaf size", size);
        assert_finite_non_negative(label, "leaf position", pos);
    }
}

#[test]
fn degenerate_parent_size_does_not_corrupt_child_layout() {
    for &(label, px) in DEGENERATE {
        let ((p_pos, p_size), (c_pos, c_size)) = resolve_parent_child(px);
        assert_finite_non_negative(label, "parent size", p_size);
        assert_finite_non_negative(label, "parent position", p_pos);
        assert_finite_non_negative(label, "child size", c_size);
        assert_finite_non_negative(label, "child position", c_pos);
    }
}

#[test]
fn degenerate_min_max_sizes_are_clamped() {
    // Degenerate values on the MIN/MAX size axes (a separate Taffy code path
    // from the base size) must also resolve finite + non-negative.
    for &(label, px) in DEGENERATE {
        let mut app = support::bare_layout_app();
        let e = app
            .world_mut()
            .spawn((
                Node,
                Style::default()
                    .width_px(50.0)
                    .height_px(50.0)
                    .min_width(Sizing::Length(Length::Px(px)))
                    .max_width(Sizing::Length(Length::Px(px))),
            ))
            .id();
        support::settle(&mut app);
        let (pos, size) = geometry(&mut app, e);
        assert_finite_non_negative(label, "min/max size", size);
        assert_finite_non_negative(label, "min/max position", pos);
    }
}

#[test]
fn degenerate_percent_sizes_are_finite() {
    // Degenerate PERCENT values (NaN/inf percent of a parent axis) must not
    // leak through the `percent(p/100)` conversion either.
    for &(label, pct) in DEGENERATE {
        let mut app = support::bare_layout_app();
        let parent = app
            .world_mut()
            .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
            .id();
        let child = app
            .world_mut()
            .spawn((
                Node,
                Style::default()
                    .width(Sizing::Length(Length::Percent(pct)))
                    .height_px(50.0),
            ))
            .id();
        app.world_mut().entity_mut(parent).add_children(&[child]);
        support::settle(&mut app);
        let (pos, size) = geometry(&mut app, child);
        assert_finite_non_negative(label, "percent size", size);
        assert_finite_non_negative(label, "percent position", pos);
    }
}
