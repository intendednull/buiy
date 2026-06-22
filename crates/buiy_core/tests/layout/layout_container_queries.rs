//! Phase 5 integration tests — container queries and container units.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.5.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::ResolvedLayout;
use buiy_core::layout::{
    ContainerQuery, ContainerQueryActive, ContainerQueryInactive, LayoutPlugin,
    LayoutTaffyComputeCount, Length, QueryCondition, Sizing, Style, SyncStylesIterCount,
    WritingModeKind,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

#[test]
fn cq_activate_marks_active_when_container_meets_min_width() {
    let mut app = app();

    // Container: 700 x 400, marked as size-container.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    // Child carries a rule: activate when min-width >= 600 px.
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Need two frames: frame 1 establishes ResolvedLayout for the
    // container; frame 2 lets cq_activate read it.
    app.update();
    app.update();

    let world = app.world();
    assert!(
        world.get::<ContainerQueryActive>(child).is_some(),
        "child should be marked active because parent width 700 >= 600"
    );
    assert!(world.get::<ContainerQueryInactive>(child).is_none());
}

#[test]
fn container_unit_cqw_resolves_against_queried_ancestor() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(800.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Two frames: 1) parent ResolvedLayout populated. 2) child reads
    // it for Cqw resolution.
    app.update();
    app.update();

    let child_layout = app.world().get::<ResolvedLayout>(child).unwrap();
    assert!(
        (child_layout.size.x - 400.0).abs() < 0.5,
        "child width should resolve to 50% of parent width 800 = 400, got {}",
        child_layout.size.x
    );
}

#[test]
fn cq_same_frame_relayout_caps_at_2x_taffy() {
    let mut app = app();

    // Establish a rule whose activation flips when the container
    // crosses 600 px. Spawn with a container width that starts at
    // 500 px (rule inactive last frame) and is set to 700 px this
    // frame (rule active, flip detected at step 4).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(500.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    app.update(); // Frame 1: ResolvedLayout populated for parent at 500.
    app.update(); // Frame 2: settle — cq_activate sees parent's 500-px
    // ResolvedLayout from frame 1 and marks child Inactive.
    assert!(
        app.world().get::<ContainerQueryInactive>(child).is_some(),
        "after the settle frame, child should be Inactive (parent 500 < 600)"
    );

    // Frame 3: bump parent to 700. cq_activate (step 2) reads the
    // previous frame's ResolvedLayout (500) and still sees Inactive.
    // taffy_compute (step 3) resolves parent to 700. cq_flip_check
    // (step 4) reads fresh `tree.layout(parent_id)` -> 700 -> rule
    // active_now=true, was=false -> flip child to Active + signal
    // re-run. cq_flip_rerun (step 5) re-runs sync_styles +
    // taffy_compute. Net: at end of frame 3, child is Active AND
    // LayoutTaffyComputeCount == 2.
    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .width_px(700.0)
            .height_px(400.0)
            .container_size(),
    );

    app.update();

    assert!(
        app.world().get::<ContainerQueryActive>(child).is_some(),
        "after same-frame re-layout, child should be Active"
    );
    assert!(app.world().get::<ContainerQueryInactive>(child).is_none());

    let count = app
        .world()
        .get_resource::<LayoutTaffyComputeCount>()
        .expect("LayoutTaffyComputeCount registered")
        .0;
    assert_eq!(
        count, 2,
        "flip frame must run Taffy exactly twice (cap), got {count}"
    );
}

#[test]
fn cq_non_flip_frame_runs_taffy_exactly_once() {
    let mut app = app();
    // Scenario with no active container query — every frame should
    // run Taffy exactly once.
    app.world_mut()
        .spawn((Node, Style::default().width_px(100.0)));
    app.update();
    app.update(); // steady-state

    let count = app
        .world()
        .get_resource::<LayoutTaffyComputeCount>()
        .expect("LayoutTaffyComputeCount registered")
        .0;
    assert_eq!(
        count, 1,
        "non-flip frame must run Taffy exactly once, got {count}"
    );
}

/// Spec § 1.3: "transitive cascade is one-frame stale". The spec's
/// transitive scenario assumes a chain where activation of one rule
/// changes the container's size, which then flips a descendant's rule.
/// Phase 5 explicitly does NOT ship the `when_active`/`when_inactive`
/// style-bundle application path that would cause such a cascade
/// (decision documented in the plan's CHANGELOG block).
///
/// Phase-14 regression: the multi-level container-query geometric cascade
/// now catches up IN-FRAME. When query container `A` resizes, the
/// `Cqw`-sized intermediate `B` is re-translated by the step-9 descendant
/// re-run (seeded by step 8's `ContainerSizeDirty`), so `B`'s width
/// re-resolves against the new `A` size and `C`'s `ContainerQuery`
/// re-evaluates — all within the same frame.
///
/// This is the polarity flip of the former
/// `cq_transitive_cascade_is_one_frame_stale` negative assertion (Phase 5
/// documented the gap; Phase 14 closes it — see
/// docs/plans/follow-ups.md "Descendant invalidation on
/// ancestor-resolved-size changes").
///
/// Scenario:
/// - A: outer container, width 700 → 1000, container_size.
/// - B: child of A; width = `Cqw(80)` of A; container_size; no rule.
/// - C: child of B; `ContainerQuery MinWidth(700)`.
///
/// Steady-state: A=700, B=560 (Cqw(80) of 700), C inactive (560 < 700).
/// After widening A to 1000: B=800 (Cqw(80) of 1000) same frame, C ACTIVE
/// same frame (800 ≥ 700).
#[test]
fn cq_transitive_cascade_catches_up_in_frame() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width(Sizing::Length(Length::Cqw(80.0)))
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let c = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(a).add_children(&[b]);
    app.world_mut().entity_mut(b).add_children(&[c]);

    app.update();
    app.update();
    let b_settled = app.world().get::<ResolvedLayout>(b).map(|l| l.size.x);
    assert_eq!(
        b_settled,
        Some(560.0),
        "B should settle to Cqw(80) of A(700) = 560, got {b_settled:?}"
    );
    assert!(
        app.world().get::<ContainerQueryInactive>(c).is_some(),
        "C should be inactive at steady-state (B=560 < 700)"
    );

    // Widen A. Phase 14: the descendant re-run re-resolves B's Cqw and
    // re-evaluates C's rule THIS frame.
    app.world_mut().entity_mut(a).insert(
        Style::default()
            .width_px(1000.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update();
    assert_eq!(
        app.world().get::<ResolvedLayout>(a).map(|l| l.size.x),
        Some(1000.0),
        "A's new resolved width equals the styled width"
    );
    assert_eq!(
        app.world().get::<ResolvedLayout>(b).map(|l| l.size.x),
        Some(800.0),
        "B re-resolves to Cqw(80) of A(1000) = 800 in the same frame (geometric cascade caught up)"
    );
    assert!(
        app.world().get::<ContainerQueryActive>(c).is_some(),
        "C activates in the same frame A resized (B=800 >= 700)"
    );
    assert!(
        app.world().get::<ContainerQueryInactive>(c).is_none(),
        "the inactive marker is removed on activation"
    );
}

/// Phase-14 (T5): the `Cqw`-sized intermediate `B` re-resolves its width
/// in the SAME frame its query-container ancestor `A` resizes. Asserts the
/// geometric half of the cascade in isolation (the rule flip is covered by
/// `cq_descendant_c_activates_in_frame_after_a_resize`).
#[test]
fn cq_intermediate_b_reresolves_cqw_in_frame() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width(Sizing::Length(Length::Cqw(80.0)))
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let c = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(a).add_children(&[b]);
    app.world_mut().entity_mut(b).add_children(&[c]);

    app.update();
    app.update();
    assert_eq!(
        app.world().get::<ResolvedLayout>(b).map(|l| l.size.x),
        Some(560.0),
        "B settles to Cqw(80) of A(700) = 560"
    );

    // Widen A to 1000. Cqw(80) of 1000 = 800. With the Phase-14 descendant
    // re-run, B re-resolves THIS frame.
    app.world_mut().entity_mut(a).insert(
        Style::default()
            .width_px(1000.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update();
    assert_eq!(
        app.world().get::<ResolvedLayout>(b).map(|l| l.size.x),
        Some(800.0),
        "B re-resolves Cqw(80) of A(1000) = 800 in the SAME frame A resized"
    );
    let _ = c;
}

/// Phase-14 (T6): the descendant re-run caps Taffy compute at 2× per frame
/// (step 3 + step 9 re-run) on a cascade frame, and returns to 1× on
/// steady-state / post-cascade frames — the spec § 1.3 cost ceiling.
#[test]
fn cq_descendant_rerun_caps_at_2x_taffy() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width(Sizing::Length(Length::Cqw(80.0)))
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    app.world_mut().entity_mut(a).add_children(&[b]);

    app.update();
    app.update();
    // Steady state: one Taffy compute, no descendant re-run.
    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        1,
        "steady-state frame runs Taffy once"
    );

    // Resize A → B is dirtied → descendant re-run fires → 2 Taffy computes.
    app.world_mut().entity_mut(a).insert(
        Style::default()
            .width_px(1000.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update();
    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        2,
        "cascade frame caps at 2x Taffy (step 3 + step 9 re-run)"
    );

    // Next steady frame settles back to one compute (cascade did not recur).
    app.update();
    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        1,
        "post-cascade frame returns to one Taffy compute"
    );
}

/// Phase-14 (T7): the rule-bearing grandchild `C` flips to `Active` in the
/// SAME frame its grand-ancestor `A` resizes, because the descendant re-run
/// re-evaluates container queries against the freshly recomputed sizes (D5).
#[test]
fn cq_descendant_c_activates_in_frame_after_a_resize() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width(Sizing::Length(Length::Cqw(80.0)))
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let c = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(a).add_children(&[b]);
    app.world_mut().entity_mut(b).add_children(&[c]);

    app.update();
    app.update();
    // Steady state: B = Cqw(80) of 700 = 560 < 700 → C inactive.
    assert!(
        app.world().get::<ContainerQueryInactive>(c).is_some(),
        "C inactive at steady-state (B=560 < 700)"
    );

    // Resize A → B re-resolves to 800 ≥ 700 → C activates THIS frame.
    app.world_mut().entity_mut(a).insert(
        Style::default()
            .width_px(1000.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update();
    assert!(
        app.world().get::<ContainerQueryActive>(c).is_some(),
        "C activates in the SAME frame A resized (B re-resolved to 800 >= 700)"
    );
    assert!(
        app.world().get::<ContainerQueryInactive>(c).is_none(),
        "the inactive marker is removed on activation"
    );
}

/// Spec § 1.4: container units fall back to viewport when no queried
/// ancestor exists. The fallback resolves directly against the primary
/// window's resolution (Phase 5 does not yet introduce `Length::Vw/Vh`
/// — Phase 10 will rewrite the inline read without changing behavior).
///
/// We spawn a synthetic `Window` with `PrimaryWindow` directly on
/// `MinimalPlugins`; the `sync_styles` system reads it via
/// `Query<&Window, With<PrimaryWindow>>`. `MinimalPlugins` is sufficient
/// because no `WindowPlugin` machinery is needed to expose a
/// component-only `Window` to a `Query` — the system polls it directly.
#[test]
fn container_unit_falls_back_to_viewport_when_no_ancestor() {
    use bevy::window::{PrimaryWindow, Window, WindowResolution};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    // Synthetic primary window. Component-only — no WindowPlugin needed
    // because `sync_styles`'s viewport read is a plain `Query<&Window,
    // With<PrimaryWindow>>`.
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1000, 600),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    let lone = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .id();
    app.update();
    app.update();

    let lone_layout = app.world().get::<ResolvedLayout>(lone).unwrap();
    assert!(
        (lone_layout.size.x - 500.0).abs() < 0.5,
        "lone Cqw(50) should resolve against viewport width 1000 -> 500, got {}",
        lone_layout.size.x
    );
}

/// Phase 5 idempotent-insert invariant (mirror of Phase 4 inherit_writing_mode
/// at systems.rs:319-321). After a Container + ContainerQuery scenario
/// settles, advancing more frames must not re-fire
/// `Changed<ContainerQueryActive>` / `Changed<ContainerQueryInactive>` —
/// either would cascade into `sync_styles` via the widened Or-filter and
/// void Phase 2's O(0) steady-state contract. The compare-before-insert
/// guard in `cq_activate` ensures this; this test fails if that guard
/// regresses.
#[test]
fn cq_activate_idempotent_no_redundant_inserts_in_steady_state() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Two frames to settle (frame 1 populates ResolvedLayout, frame 2
    // activates the rule).
    app.update();
    app.update();
    assert!(
        app.world().get::<ContainerQueryActive>(child).is_some(),
        "scenario should be settled by frame 2"
    );

    // Frame 3: no inputs changed. Steady-state. sync_styles' filter
    // (after Task 9 widening) must NOT pick up any entity, because
    // cq_activate's compare-before-insert guard prevented a redundant
    // Changed<ContainerQueryActive> tick.
    app.update();
    let count = app
        .world()
        .get_resource::<SyncStylesIterCount>()
        .expect("SyncStylesIterCount resource registered")
        .0;
    assert_eq!(
        count, 0,
        "sync_styles must iterate 0 entities on a steady-state frame; \
         got {count} (cq_activate's compare-before-insert guard may have \
         regressed — see systems.rs idempotent-flip block)"
    );
}

/// Spec § 1.4: `cqi`/`cqb` resolve against the *inline*/*block* axis,
/// which depends on writing-mode. Under `HorizontalTb`: inline = width.
/// Under `VerticalRl` / `VerticalLr` / `SidewaysRl` / `SidewaysLr`:
/// inline = height. This test exercises the wm-conditional branch in
/// `resolve_cq_unit_px` directly.
///
/// Setup: 800x400 container with Container::Size. Child carries
/// `WritingModeKind::VerticalRl` and width = Cqi(50). Under VerticalRl,
/// cqi(50) = 50% of container's *height* axis (400) = 200 px.
#[test]
fn container_unit_cqi_swaps_axis_under_vertical_writing_mode() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(800.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width(Sizing::Length(Length::Cqi(50.0)))
                .writing_mode_kind(WritingModeKind::VerticalRl),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    app.update();
    app.update();

    let layout = app.world().get::<ResolvedLayout>(child).unwrap();
    assert!(
        (layout.size.x - 200.0).abs() < 0.5,
        "Cqi(50) under VerticalRl should resolve to 50% of *height* (400) = 200, got {}",
        layout.size.x
    );
}
