//! Phase 7 Task 10 Step 1: integration coverage for the `sticky_offset`
//! (sub-pass 6a) pipeline.
//!
//! Fixture shape mirrors `tests/layout_anchor_positioning.rs`: spin up
//! a `LayoutPlugin` app, spawn a scroll-container + sticky-child tree,
//! drive `app.update()`, and assert the per-frame override map and/or
//! `ResolvedLayout.position` matches the expected sticky displacement.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.3.
//! Plan: docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md Task 10 Step 1.

use bevy::prelude::*;
use buiy_core::layout::{
    Anchor, AnchorRef, Display, Inset, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession,
    Length, OverflowMode, Position, PositionKind, PositionTry, PostTaffyPositionOverrides,
    ScrollOffset, Sizing, Style,
};
use buiy_core::{Node, ResolvedLayout};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

/// Construct a sticky `Position` value with the given top inset (px).
fn sticky_top(top_px: f32) -> Position {
    Position {
        kind: PositionKind::Sticky,
        inset: Inset {
            top: Sizing::Length(Length::Px(top_px)),
            ..Default::default()
        },
    }
}

/// Construct a sticky `Position` value with the given bottom inset (px).
fn sticky_bottom(bottom_px: f32) -> Position {
    Position {
        kind: PositionKind::Sticky,
        inset: Inset {
            bottom: Sizing::Length(Length::Px(bottom_px)),
            ..Default::default()
        },
    }
}

/// Construct a sticky `Position` with an arbitrary `Sizing` top inset
/// (used by the Cq/Fr deferral tests).
fn sticky_top_sizing(top: Sizing) -> Position {
    Position {
        kind: PositionKind::Sticky,
        inset: Inset {
            top,
            ..Default::default()
        },
    }
}

/// Spawn a 300x500 scroll container (`overflow-y: scroll`) with the
/// given `ScrollOffset.y`. The scroll viewport is `300x500`; nest
/// taller content inside to make scrolling meaningful.
fn scroll_container(app: &mut App, scroll_y: f32) -> Entity {
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(300.0)
                .height_px(500.0)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset {
                x: 0.0,
                y: scroll_y,
            },
        ))
        .id()
}

/// Spawn a child content block under `parent` with the given height
/// (300 px wide). Returns the spawned entity.
fn content_block(app: &mut App, parent: Entity, height: f32) -> Entity {
    let e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(height)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[e]);
    e
}

/// Spawn a sticky child under `parent` with `Display::Block`, size
/// (`width` x `height`), and the given `Position`. Returns the entity.
fn sticky_child(
    app: &mut App,
    parent: Entity,
    width: f32,
    height: f32,
    position: Position,
) -> Entity {
    let mut style = Style::default().width_px(width).height_px(height);
    style.position = position;
    let e = app.world_mut().spawn((Node, style)).id();
    app.world_mut().entity_mut(parent).add_children(&[e]);
    e
}

// =====================================================================
// Top-pin scenarios
// =====================================================================

#[test]
fn sticky_pins_to_top_during_scroll() {
    // Setup: 300x500 scroll container, 300x1000 content block, sticky
    // 100x30 inside block. Block has block-layout children, so sticky
    // is the only child and starts at y=0. We need it at y=50 — add
    // a 50px spacer first.
    //
    // visible_top = 100, threshold = 100. natural_y_in_S = 50.
    // desired_y_in_S = max(50, 100) = 100, clamped within parent.
    // displacement = (0, 50).
    // ResolvedLayout.position (in parent frame) = (0, 50) + (0, 50) =
    // (0, 100). The override map records the same value (sticky writes
    // `e_natural_rel_to_parent + displacement`).
    let mut app = app();
    let scroll = scroll_container(&mut app, 100.0);
    let block = content_block(&mut app, scroll, 1000.0);
    // 50px spacer to push the sticky element down to y=50 inside block.
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_top(0.0));

    // Single update suffices: `sticky_offset` (6a) writes the override,
    // and `write_resolved_layout` (step 7) consumes it, all on the same
    // frame.
    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let pos = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky entity in override map, got none"));
    assert_eq!(
        pos.y, 100.0,
        "scrolled-past-threshold sticky should pin to y=100 in parent frame; got {:?}",
        pos
    );

    let rl = app.world().get::<ResolvedLayout>(sticky).unwrap();
    assert_eq!(
        rl.position.y, 100.0,
        "ResolvedLayout.position reflects override; got {:?}",
        rl.position
    );
}

#[test]
fn sticky_does_not_pull_up_before_scroll() {
    // Same fixture, scroll_offset.y = 0. visible_top = 0, threshold =
    // 0 + 0 = 0. natural_y = 50. max(50, 0) = 50. displacement = 0.
    // Override map should NOT contain the sticky entity (sticky_offset
    // skips zero-displacement writes per the no-spurious-overrides
    // invariant).
    let mut app = app();
    let scroll = scroll_container(&mut app, 0.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_top(0.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&sticky),
        "no scroll → no displacement → no override entry; got {:?}",
        overrides.by_entity.get(&sticky),
    );
}

#[test]
fn sticky_clamped_by_parent_bottom() {
    // Small parent: block height 100. Sticky at natural y=10 (no
    // spacer needed since block is the only child), size 30.
    // scroll_offset.y = 100, top inset = 5.
    // visible_top = 100, threshold = 105. natural_y = 10.
    // max(10, 105) = 105, clamped by parent_bottom - size = 100 - 30 =
    // 70 → 70. max with parent_top (0) = 70. displacement = 70 - 10 = 60.
    // Block height (100) < scroll viewport (500) but Taffy still gives
    // block the requested height. Override map carries (0, 70).
    //
    // Note: the helper `content_block` already starts the block at y=0
    // within scroll container. The sticky's natural y inside its parent
    // depends on Taffy block-layout: as the only child, it sits at y=0
    // (not y=10). To get natural_y = 10 in parent's frame, we need a
    // 10-px spacer before the sticky.
    let mut app = app();
    let scroll = scroll_container(&mut app, 100.0);
    let block = content_block(&mut app, scroll, 100.0);
    let _spacer = content_block(&mut app, block, 10.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_top(5.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let pos = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky in overrides for parent-clamp case"));
    // Sticky natural y in block frame = 10. desired_y_in_block = 70
    // (clamped). Override is stored as `e_natural_rel + displacement` —
    // displacement is in block frame, so override.y = 70.
    assert_eq!(
        pos.y, 70.0,
        "parent-bottom clamp: sticky should be at y=70 in block frame; got {:?}",
        pos,
    );
}

// =====================================================================
// Bottom-pin scenarios
// =====================================================================

#[test]
fn sticky_bottom_pins_when_scroll_near_bottom() {
    // Element at natural y=700 in scroll-container content frame, size
    // 30. Parent height 1000, scroll container 500, scroll_offset.y =
    // 300. visible_bottom = 800, bottom_inset = 10, threshold = 790.
    // threshold - size = 760. min(natural=700, 760) = 700.
    // wait — natural < threshold-size means the element is "above"
    // its sticky bottom position and bottom-pin doesn't fire. Let me
    // re-pick numbers so the bottom-pin branch actually displaces.
    //
    // Correct setup for bottom-pin to fire: natural_y > threshold - size.
    // Pick natural_y = 900 (parent y=900 + sticky height 30 = 930 <= 1000).
    // visible_bottom = scroll_offset + S.y = 0 + 500 = 500. inset = 10,
    // threshold = 490. threshold - h = 460. min(natural=900, 460) = 460.
    // clamped by parent_top (0) and parent_bottom-h (970) → 460.
    // displacement = 460 - 900 = -440.
    //
    // Simpler: with scroll_offset.y = 400, visible_bottom = 900,
    // threshold = 890, threshold-h = 860. min(natural=900, 860) = 860.
    // clamped by parent: max(0, 860) = 860, min(970, 860) = 860.
    // displacement = 860 - 900 = -40.
    let mut app = app();
    let scroll = scroll_container(&mut app, 400.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 900.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_bottom(10.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let pos = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky in overrides for bottom-pin case"));
    // Override stores e_natural_rel_to_parent + displacement. Natural
    // y_in_block = 900 (after 900-px spacer); displacement in block
    // frame = -40. So override.y = 900 + (-40) = 860.
    assert_eq!(
        pos.y, 860.0,
        "bottom-pin: sticky should sit at y=860 in block frame; got {:?}",
        pos,
    );
}

#[test]
fn sticky_bottom_does_not_push_down_before_scroll() {
    // natural y=300, scroll_offset.y=0. visible_bottom=500, inset=10,
    // threshold=490, threshold-h=460. min(natural=300, 460) = 300.
    // displacement = 0 — element is already above the bottom threshold,
    // so no push-down is needed.
    let mut app = app();
    let scroll = scroll_container(&mut app, 0.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 300.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_bottom(10.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&sticky),
        "bottom inset before scroll → no push-down → no override entry",
    );
}

#[test]
fn sticky_bottom_clamped_by_parent_top() {
    // Parent has nonzero top in scroll-container frame (block sits
    // below another sibling). natural in parent's frame = 0 (no
    // spacer), but in scroll frame = parent_top.
    //
    // Setup: a 100-px sibling first, then the sticky's parent block at
    // y=100 of height 200. Sticky natural y_in_block = 0 (no spacer),
    // so y_in_S = 100.
    // scroll_offset.y = 0, visible_bottom = 0 + 500 = 500, inset = 10,
    // threshold = 490, threshold - h = 460. min(natural=100, 460) = 100.
    // max(parent_top=100) → 100. min(parent_bottom-h=270) → 100.
    // displacement = 100 - 100 = 0.
    //
    // Concrete fixture: S.height=100, scroll_offset=0, inset=10.
    // visible_bottom = 0 + 100 = 100, threshold = 100 - 10 = 90,
    // threshold - h = 60. Parent box: spacer block of 100, then 200-tall
    // parent → parent_top_in_S = 100, parent_bottom_in_S = 300, h=30,
    // natural_in_S = 200 (parent_top + 100 spacer). min(natural=200, 60)
    // = 60. max(parent_top=100, 60) → 100 (clamp fires). min(parent_bottom
    // - h = 270, 100) → 100. desired_in_S = 100; displacement_in_S =
    // 100 - 200 = -100. In the parent-relative override frame: override.y
    // = 100 (natural_in_parent) + (-100) = 0.
    let mut app = app();
    // Small scroll container so visible_bottom is tiny.
    let scroll = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(300.0)
                .height_px(100.0)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset { x: 0.0, y: 0.0 },
        ))
        .id();
    // First, a 100-px sibling so the parent block starts at y=100.
    let _sibling = content_block(&mut app, scroll, 100.0);
    // Parent block of height 200.
    let block = content_block(&mut app, scroll, 200.0);
    // Spacer to push sticky to y=200 in block frame.
    let _spacer = content_block(&mut app, block, 200.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_bottom(10.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let pos = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky entry for parent-top clamp; got none"));
    // In block frame, natural y = 200; clamp pulls to y=0 (parent_top
    // in block frame). displacement = -200 in block frame, so override
    // = 200 + (-200) = 0.
    assert_eq!(
        pos.y, 0.0,
        "parent-top clamp: sticky should be clamped to parent_top; got {:?}",
        pos,
    );
}

// =====================================================================
// Both-active conflict
// =====================================================================

#[test]
fn sticky_both_top_and_bottom_inset_top_wins() {
    // v1 deviation per CHANGELOG: when both insets are set, the
    // top-pin branch fires first and the bottom inset is ignored.
    // Future correct dual-clamp implementation will replace this test
    // with one that asserts an "upper-stuck vs lower-stuck, smallest
    // perturbation wins" rule.
    //
    // Setup: sticky at natural y_in_S = 50, top inset 10, bottom inset
    // 10, scroll_offset.y = 100. Top-pin branch: visible_top=100,
    // threshold=110, max(50, 110)=110. Clamped by parent_bottom-size
    // = 1000-30 = 970 → 110. displacement = 60.
    //
    // If bottom-pin were applied first (wrong), it'd push the element
    // up — different displacement. The test pins the top-wins choice.
    let mut app = app();
    let scroll = scroll_container(&mut app, 100.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = app
        .world_mut()
        .spawn((Node, {
            let mut s = Style::default().width_px(100.0).height_px(30.0);
            s.position = Position {
                kind: PositionKind::Sticky,
                inset: Inset {
                    top: Sizing::Length(Length::Px(10.0)),
                    bottom: Sizing::Length(Length::Px(10.0)),
                    ..Default::default()
                },
            };
            s
        }))
        .id();
    app.world_mut().entity_mut(block).add_children(&[sticky]);

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let pos = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky entry for both-active case"));
    // Top branch wins: y_in_block = 50 + 60 = 110.
    assert_eq!(
        pos.y, 110.0,
        "both insets set → top wins (v1 deviation); got {:?}",
        pos,
    );
}

// =====================================================================
// No-scroll-container — silent no-op
// =====================================================================

#[test]
fn sticky_no_scroll_container_is_no_op() {
    // No scroll-container ancestor. Sticky resolves to relative
    // (silent no-op per D5). The strengthened assertion (v2 BLOCKER
    // B5): explicitly check the override map is empty, NOT that
    // ResolvedLayout.position equals zero (which would also pass for
    // a "did nothing" stub-skip).
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(500.0)))
        .id();
    // No `overflow: scroll` on the parent — not a scroll container.
    let sticky = sticky_child(&mut app, parent, 100.0, 30.0, sticky_top(10.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        overrides.by_entity.is_empty(),
        "no scroll-container ancestor → sticky pass writes no override; \
         got {} entries (entry for sticky? {:?})",
        overrides.by_entity.len(),
        overrides.by_entity.get(&sticky),
    );
}

// =====================================================================
// Percent inset against the scroll-container content box
// =====================================================================

#[test]
fn sticky_percent_inset_against_scroll_container() {
    // Scroll container height 200, sticky top inset 10%. Threshold =
    // 200 * 0.10 = 20px in scroll-container axis size. With
    // scroll_offset.y = 100, visible_top = 100, threshold = 100 + 20 =
    // 120. natural y_in_block = 50. max(50, 120) = 120, clamped by
    // parent.
    let mut app = app();
    let scroll = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(300.0)
                .height_px(200.0)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset { x: 0.0, y: 100.0 },
        ))
        .id();
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = app
        .world_mut()
        .spawn((Node, {
            let mut s = Style::default().width_px(100.0).height_px(30.0);
            s.position = Position {
                kind: PositionKind::Sticky,
                inset: Inset {
                    top: Sizing::Length(Length::Percent(10.0)),
                    ..Default::default()
                },
            };
            s
        }))
        .id();
    app.world_mut().entity_mut(block).add_children(&[sticky]);

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let pos = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky in overrides for percent-inset case"));
    // y_in_block: natural=50, displaced to 120. override = 120.
    assert_eq!(
        pos.y, 120.0,
        "10%% of 200px scroll-container height = 20px inset → \
         threshold 120 at scroll_y=100; got {:?}",
        pos,
    );
}

// =====================================================================
// Cq / Fr deferral paths — warn-once-per-session, inset → 0
// =====================================================================

#[test]
fn sticky_cq_inset_deferred_resolves_to_zero_with_warn() {
    // Cqw inset on sticky → resolves to 0 and warns. With inset 0,
    // top-pin threshold = visible_top + 0 = scroll_offset. natural
    // y_in_block = 50, scroll_offset = 0 → no displacement.
    let mut app = app();
    let scroll = scroll_container(&mut app, 0.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = sticky_child(
        &mut app,
        block,
        100.0,
        30.0,
        sticky_top_sizing(Sizing::Length(Length::Cqw(20.0))),
    );

    app.update();

    // Inset resolves to 0; with scroll_offset=0 there's no
    // displacement, so the override map should NOT contain the sticky
    // entry. (Test ensures the no-op-write invariant holds in this
    // path.) The warn is the focus assertion.
    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&sticky),
        "Cq inset → 0; scroll_offset=0 → no displacement → no override entry",
    );

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::StickyCqDeferred(sticky)),
        "Cq inset should record a StickyCqDeferred warn for the sticky entity",
    );
}

#[test]
fn sticky_fr_inset_resolves_to_zero_with_warn() {
    // Same shape as Cq test but with Fr (grid-only) inset. Resolves
    // to 0 and warns under a different key.
    let mut app = app();
    let scroll = scroll_container(&mut app, 0.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = sticky_child(
        &mut app,
        block,
        100.0,
        30.0,
        sticky_top_sizing(Sizing::Length(Length::Fr(2.0))),
    );

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&sticky),
        "Fr inset → 0; scroll_offset=0 → no displacement → no override entry",
    );

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::StickyFrUnsupported(sticky)),
        "Fr inset should record a StickyFrUnsupported warn for the sticky entity",
    );
}

// =====================================================================
// Nested scroll containers — innermost wins (D9)
// =====================================================================

#[test]
fn sticky_in_nested_scroll_containers_uses_innermost() {
    // Two nested scroll containers. Sticky lives inside the inner.
    // The OUTER has nonzero scroll_offset — but D9 says sticky uses
    // the INNERMOST scroll container as its reference. With the inner
    // at scroll_offset.y=0, the sticky doesn't displace.
    let mut app = app();
    // Outer scroll container, 600x800 with non-zero scroll.
    let outer = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(600.0)
                .height_px(800.0)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset { x: 0.0, y: 200.0 },
        ))
        .id();
    // Inner scroll container, 300x500 with zero scroll.
    let inner = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(300.0)
                .height_px(500.0)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset { x: 0.0, y: 0.0 },
        ))
        .id();
    app.world_mut().entity_mut(outer).add_children(&[inner]);
    let block = content_block(&mut app, inner, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_top(0.0));

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&sticky),
        "sticky should resolve against innermost (inner scroll_offset=0) \
         and NOT the outer (scroll_offset=200) — D9; got {:?}",
        overrides.by_entity.get(&sticky),
    );
}

// =====================================================================
// Display::None skip (D10)
// =====================================================================

#[test]
fn sticky_display_none_is_skipped() {
    // Sticky element with Display::None — skipped by sub-pass 6a even
    // if it has a scroll-container ancestor and scroll offset.
    let mut app = app();
    let scroll = scroll_container(&mut app, 100.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = app
        .world_mut()
        .spawn((Node, {
            let mut s = Style::default().width_px(100.0).height_px(30.0);
            s.position = sticky_top(0.0);
            s.display = Display::None;
            s
        }))
        .id();
    app.world_mut().entity_mut(block).add_children(&[sticky]);

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        !overrides.by_entity.contains_key(&sticky),
        "Display::None sticky must be skipped; got override {:?}",
        overrides.by_entity.get(&sticky),
    );
}

// =====================================================================
// Cross-phase: anchor target IS sticky (closes Phase 6 follow-up,
// integrates Task 9's D1 fix).
// =====================================================================

#[test]
fn anchor_target_is_sticky_anchored_tracks_displaced_position() {
    use buiy_core::layout::AnchorName;

    // Setup: a scroll container with a sticky element acting as an
    // anchor target. An anchored entity is positioned 5px BELOW that
    // sticky target. After scrolling, the sticky displaces — and the
    // anchored entity must follow the *displaced* position, not the
    // natural Taffy position. This is what Task 9's D1 fix wires.
    let mut app = app();
    let scroll = scroll_container(&mut app, 100.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    // Sticky target with `anchor_name = "sticky-target"`.
    let target = app
        .world_mut()
        .spawn((
            Node,
            {
                let mut s = Style::default().width_px(100.0).height_px(30.0);
                s.position = sticky_top(0.0);
                s
            },
            Anchor {
                anchor_name: Some(AnchorName::Named("sticky-target".into())),
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().entity_mut(block).add_children(&[target]);

    // Anchored entity, anchored 5px below the sticky target.
    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(80.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("sticky-target".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(5.0)),
                    conditions: vec![], // always passes
                }],
                ..Default::default()
            },
        ))
        .id();

    // Single update suffices: `sticky_offset` (6a) and
    // `anchor_resolution` (6d) are chained in `PostTaffyOverrides` via
    // `.chain()` (see layout/mod.rs ~line 180), so the anchor pass
    // reads the displacement the sticky pass just wrote, on the same
    // frame.
    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let target_pos = overrides
        .by_entity
        .get(&target)
        .copied()
        .unwrap_or_else(|| panic!("sticky target should have a displaced override"));
    // Target's natural y_in_block = 50 (after 50-px spacer); displaced
    // to y=100 in block frame.
    assert_eq!(target_pos.y, 100.0, "target displaced by sticky pass");

    // Anchored entity is a sibling of the scroll-container at the
    // root, so its parent-relative override is in the *root* frame —
    // which equals the scroll-container's parent frame in this
    // fixture (single-child-of-root layout). Anchor resolution reads
    // the target's position via the new D1 fix (PostTaffyPositionOverrides
    // → falls back to Taffy when empty). Target's displaced position
    // is y=100 in the parent-relative override frame, which equals the
    // root frame here. Anchored entity sits below the target with a
    // 5px gap: anchored_y = target_y (100) + target_h (30) + 5 = 135.
    let anchored_pos = overrides
        .by_entity
        .get(&anchored)
        .copied()
        .unwrap_or_else(|| panic!("anchored entity should have an anchor override"));
    assert_eq!(
        anchored_pos.y, 135.0,
        "anchored entity tracks displaced target (135 = 100 + 30 + 5); \
         if it shows 80 (=50+30-5) the anchor read the natural target position. got {:?}",
        anchored_pos,
    );
}

// =====================================================================
// Clear-ordering regression — frame 2 override is fresh, not leftover.
// =====================================================================

#[test]
fn clear_ordering_regression_two_frames() {
    // Spawn a sticky entity. Run frame 1 with scroll_offset=100.
    // Mutate scroll_offset to 200 for frame 2. Assert the override
    // map's entry for the sticky entity on frame 2 reflects frame
    // 2's displacement (not the leftover from frame 1).
    //
    // Why this matters: if `clear_post_taffy_overrides` were moved to
    // AFTER `sticky_offset` in the chain, frame 2 would write its
    // entry on top of a non-empty map; subsequent overrides would
    // accumulate. The test would still pass for a single sticky
    // because frame 2's write overwrites frame 1's. The real
    // regression appears when an entity STOPS being sticky between
    // frames: frame 1's stale entry would persist into frame 2. We
    // assert the simpler fresh-write invariant here; the dedicated
    // `layout_post_taffy_overrides_clear.rs` test covers the
    // stale-entry case.
    let mut app = app();
    let scroll = scroll_container(&mut app, 100.0);
    let block = content_block(&mut app, scroll, 1000.0);
    let _spacer = content_block(&mut app, block, 50.0);
    let sticky = sticky_child(&mut app, block, 100.0, 30.0, sticky_top(0.0));

    // Frame 1: scroll_offset=100 → displacement 50 → override y=100.
    app.update();
    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let frame_1_y = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .expect("frame 1 should have an override")
        .y;
    assert_eq!(frame_1_y, 100.0, "frame 1 override: y=100");

    // Mutate ScrollOffset to 200 for frame 2.
    {
        let mut so = app
            .world_mut()
            .get_mut::<ScrollOffset>(scroll)
            .expect("scroll container has ScrollOffset");
        so.y = 200.0;
    }

    // Frame 2: scroll_offset=200 → visible_top=200 → threshold=200 →
    // max(50, 200) = 200 → displacement 150 → override y = 50 + 150 = 200.
    app.update();
    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let frame_2_y = overrides
        .by_entity
        .get(&sticky)
        .copied()
        .expect("frame 2 should have an override")
        .y;
    assert_eq!(
        frame_2_y, 200.0,
        "frame 2 must reflect fresh scroll_offset=200 (override y=200), \
         not leftover {} from frame 1",
        frame_1_y,
    );
}
