//! Headless T1 flex justify-content / align-items child-distribution tests
//! (audit #2, campaign T2.1).
//!
//! Before this file the ONLY distribution assertion in the suite was the GPU
//! `#[ignore]` reftest (`crates/buiy_verify/tests/reftest_cases_gpu.rs:137`,
//! `flex_justify_eq_literal`) — which the headless merge gate never runs. A
//! child-distribution regression (a plain layout number) therefore shipped
//! green on every PR.
//!
//! These assert the FLEXBOX-SPEC-CORRECT `ResolvedLayout` positions computed
//! from first principles — they are correctness oracles, not snapshots, so a
//! flipped/dropped distribution case reddens here. The GPU reftest is kept as
//! the rasterized cross-check (it is NOT replaced by this file).
//!
//! Geometry: three 40px boxes in a 200px flex row → free main-axis space is
//! `200 - 3*40 = 80`. The cross axis is the 100px tall row with 40px children.

mod support;

use bevy::math::Vec2;
use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{AlignItems, JustifyContent, Style};

/// Tolerance for the float layout-number comparisons. The expected values are
/// exact in spec terms, but the production pipeline runs Taffy's default
/// **pixel rounding** on the resolved geometry, so a spec value like the `80/3`
/// SpaceAround half-unit edge (13.3333…) lands at 13 in `ResolvedLayout`. A
/// half-pixel tolerance (matching `layout_grid.rs`'s `0.5` convention) keeps
/// each assertion bound to the spec-correct value modulo that rounding, while
/// the smallest distinct expected x's differ by tens of pixels — so a flipped
/// or dropped distribution case still reddens.
const EPS: f32 = 0.5;

/// Spawn a 200x100 flex-row root with three 40x40 children under the given
/// `justify_content` / `align_items`, settle, and return the children's
/// resolved `(position, size)` in spawn order.
fn distribute(justify: JustifyContent, align: AlignItems) -> Vec<(Vec2, Vec2)> {
    let mut app = support::bare_layout_app();

    let root = app
        .world_mut()
        .spawn((
            Node,
            Name::new("row"),
            Style::default()
                .flex_row()
                .width_px(200.0)
                .height_px(100.0)
                .justify_content(justify)
                .align_items(align),
        ))
        .id();

    let mut children = Vec::new();
    for i in 0..3 {
        let c = app
            .world_mut()
            .spawn((
                Node,
                Name::new(format!("item[{i}]")),
                Style::default().width_px(40.0).height_px(40.0),
            ))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(root).add_children(&children);

    support::settle(&mut app);

    children
        .iter()
        .map(|e| {
            let rl = app
                .world()
                .get::<ResolvedLayout>(*e)
                .expect("child ResolvedLayout written");
            (rl.position, rl.size)
        })
        .collect()
}

fn assert_xs(layouts: &[(Vec2, Vec2)], expected: [f32; 3], label: &str) {
    for (i, (&(pos, _), want)) in layouts.iter().zip(expected).enumerate() {
        assert!(
            (pos.x - want).abs() < EPS,
            "{label}: child {i} x = {} (expected {want})",
            pos.x
        );
    }
}

#[test]
fn space_between_distributes_0_80_160() {
    // free space 80 split into 2 equal gaps of 40; first flush-left, last
    // flush-right. x = 0, 40+40 = 80, 2*40+2*40 = 160.
    let layouts = distribute(JustifyContent::SpaceBetween, AlignItems::FlexStart);
    assert_xs(&layouts, [0.0, 80.0, 160.0], "SpaceBetween");
}

#[test]
fn space_around_distributes_with_half_unit_edges() {
    // free space 80 split into 3 equal "around" units of 80/3 ≈ 26.6667; each
    // edge gets a half-unit (≈13.3333). x0 = 13.3333, x1 = 13.3333 + 40 +
    // 26.6667 = 80, x2 = 80 + 40 + 26.6667 = 146.6667.
    let unit = 80.0 / 3.0;
    let layouts = distribute(JustifyContent::SpaceAround, AlignItems::FlexStart);
    assert_xs(
        &layouts,
        [
            unit / 2.0,
            unit / 2.0 + 40.0 + unit,
            unit / 2.0 + 80.0 + 2.0 * unit,
        ],
        "SpaceAround",
    );
    // Pin the convenient exact midpoint too: the center child sits at 80.
    assert!(
        (layouts[1].0.x - 80.0).abs() < EPS,
        "SpaceAround center = 80"
    );
}

#[test]
fn space_evenly_distributes_with_equal_edges() {
    // free space 80 split into 4 equal gaps (n_items + 1) of 20. x0 = 20,
    // x1 = 20 + 40 + 20 = 80, x2 = 80 + 40 + 20 = 140.
    let layouts = distribute(JustifyContent::SpaceEvenly, AlignItems::FlexStart);
    assert_xs(&layouts, [20.0, 80.0, 140.0], "SpaceEvenly");
}

#[test]
fn align_items_center_centers_children_on_cross_axis() {
    // cross axis = 100px tall row, 40px children → y = (100 - 40)/2 = 30.
    let layouts = distribute(JustifyContent::FlexStart, AlignItems::Center);
    for (i, (pos, _)) in layouts.iter().enumerate() {
        assert!(
            (pos.y - 30.0).abs() < EPS,
            "AlignItems::Center: child {i} y = {} (expected 30)",
            pos.y
        );
    }
}

#[test]
fn align_items_flex_end_bottom_aligns_children() {
    // cross axis = 100px tall row, 40px children → y = 100 - 40 = 60.
    let layouts = distribute(JustifyContent::FlexStart, AlignItems::FlexEnd);
    for (i, (pos, _)) in layouts.iter().enumerate() {
        assert!(
            (pos.y - 60.0).abs() < EPS,
            "AlignItems::FlexEnd: child {i} y = {} (expected 60)",
            pos.y
        );
    }
}

#[test]
fn flex_start_packs_children_flush_left() {
    // Baseline / control: default main-axis packing leaves x = 0, 40, 80 with
    // no distribution. Distinguishes "distribution applied" from "no free
    // space" failure modes in the cases above.
    let layouts = distribute(JustifyContent::FlexStart, AlignItems::FlexStart);
    assert_xs(&layouts, [0.0, 40.0, 80.0], "FlexStart");
}
