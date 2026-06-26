//! Phase 7 Task 2 regression: `PostTaffyPositionOverrides` is cleared
//! every frame by `clear_post_taffy_overrides` via the live `LayoutPlugin`.
//!
//! Background: Phase 6 had the override clear inlined at the top of
//! `anchor_resolution`. Phase 7 Task 2 extracted that clear into a
//! dedicated system (`clear_post_taffy_overrides`) under the
//! `BuiyLayoutStep::PostTaffyOverrides` set so future sub-passes
//! (sticky 6a, table 6b, multicol 6c) can be chained in without
//! ordering surprises. The regression this test guards against is the
//! scenario where the extracted system is defined but not wired into
//! `LayoutPlugin` — leaving the shipped binary unable to clear the
//! override map between frames, breaking the Phase 6 invariant.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.

use bevy::math::Vec2;
use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::{Node, PostTaffyPositionOverrides};

/// Seed `PostTaffyPositionOverrides` with a fake entry, run one update
/// of the live `LayoutPlugin`, and assert the map is empty afterwards.
/// Exercises `clear_post_taffy_overrides` end-to-end via the plugin
/// wiring — not via a direct system attach — so a missing
/// `.in_set(BuiyLayoutStep::PostTaffyOverrides)` on the clear system
/// would fail this test.
#[test]
fn post_taffy_position_overrides_clears_each_frame() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    // Spawn one node so the frame is DIRTY — perf audit #3 gates the
    // `PostTaffyOverrides` chain (including `clear_post_taffy_overrides`) on
    // `LayoutDirtyThisFrame`, which a freshly-spawned `Node` (all components
    // `Added`) sets. An empty world would be a legitimately-idle frame on which
    // the gate correctly skips the chain (the retained map is already valid),
    // so the wiring this test guards is only exercised on a dirty frame.
    app.world_mut().spawn((Node, Style::default()));

    // Seed the resource with a stale entry. We use `Entity::from_raw_u32`
    // to avoid spawning — the clear system doesn't care whether the
    // entity exists; it just unconditionally empties the map.
    let stale = Entity::from_raw_u32(99).unwrap();
    app.world_mut()
        .resource_mut::<PostTaffyPositionOverrides>()
        .by_entity
        .insert(stale, Vec2::ONE);

    // Sanity: confirm the seed landed.
    assert_eq!(
        app.world()
            .resource::<PostTaffyPositionOverrides>()
            .by_entity
            .len(),
        1,
        "test setup failed to seed PostTaffyPositionOverrides"
    );

    app.update();

    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        overrides.by_entity.is_empty(),
        "expected clear_post_taffy_overrides to wipe the map each frame; \
         found {} stale entries — likely missing .in_set(PostTaffyOverrides) \
         wiring in LayoutPlugin",
        overrides.by_entity.len()
    );
}
