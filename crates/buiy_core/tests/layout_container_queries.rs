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
/// (decision documented in the plan's CHANGELOG block). The geometric
/// equivalent — an ancestor's direct resize flowing through a
/// `Cqw`-sized intermediate to a grandchild — also stays stale in
/// Phase 5 because the intermediate is never re-translated when only
/// the ancestor's `ResolvedLayout` changes (the per-entity changed-set
/// filter on `sync_styles` doesn't include "ancestor's ResolvedLayout
/// changed").
///
/// This test ASSERTS that documented lag: after A is widened, C's
/// activation does NOT propagate within the frames Phase 5 owns. A
/// future phase (when style-bundle application + descendant
/// invalidation land) is expected to promote this assertion's polarity
/// from "stays stale" to "catches up within frame N+1".
///
/// Scenario:
/// - A: outer container, width 700, container_size.
/// - B: child of A; width = `Cqw(80)` of A; container_size; no rule on B.
/// - C: child of B; `ContainerQuery MinWidth(700)`.
///
/// Steady-state: A=700, B=`Cqw(80)` of A = 560, C inactive (560 < 700).
/// After widening A to 1000, C stays inactive because B's translated
/// width is not re-evaluated against the new A snapshot (the geometric
/// cascade is not currently propagated to B).
#[test]
fn cq_transitive_cascade_is_one_frame_stale() {
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
            // No rule on B itself — B is the container for C.
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

    // Settle frames: frame 1 populates ResolvedLayout; frame 2 lets the
    // cq systems see the previous-frame snapshot and mark C inactive.
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

    // Widen A. The geometric cascade lag means C stays inactive on this
    // frame and the next — B's Cqw is not re-evaluated because B itself
    // is not in any `Changed<>` filter on sync_styles when only A's
    // ResolvedLayout changed.
    app.world_mut().entity_mut(a).insert(
        Style::default()
            .width_px(1000.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update();
    let a_after = app.world().get::<ResolvedLayout>(a).map(|l| l.size.x);
    let b_after = app.world().get::<ResolvedLayout>(b).map(|l| l.size.x);
    assert_eq!(
        a_after,
        Some(1000.0),
        "A's new resolved width should equal the styled width"
    );
    assert_eq!(
        b_after,
        Some(560.0),
        "B should still report its pre-resize width (geometric cascade lag)"
    );
    assert!(
        app.world().get::<ContainerQueryInactive>(c).is_some(),
        "C should still be inactive immediately after A's resize \
         (geometric cascade lag — B's Cqw was not re-evaluated)"
    );

    // One more frame: the lag persists in Phase 5 — there is no system
    // that fires `Changed<>` on B in response to A's `ResolvedLayout`
    // change, so B is never re-translated and the cascade never
    // propagates within Phase 5's scope. (A future phase adding
    // descendant invalidation will flip this assertion's polarity.)
    app.update();
    assert!(
        app.world().get::<ContainerQueryInactive>(c).is_some(),
        "C should still be inactive — Phase 5 does not propagate the \
         geometric cascade through Cqw-sized intermediates. This \
         matches the documented divergence; a future phase promotes \
         this assertion to ContainerQueryActive."
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
