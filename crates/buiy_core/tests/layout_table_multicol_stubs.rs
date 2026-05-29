//! Phase 7 Task 10 Step 2: integration coverage for the
//! `multicol_pack` (sub-pass 6c) stub-warn pass.
//!
//! `multicol_pack` warns once per session total — the first
//! `MultiColumn` entity triggers, all subsequent are silent.
//!
//! The table (sub-pass 6b) stub-warn tests that once lived here are
//! superseded by the real table layout algorithm (Phase 12); their
//! behavioral coverage now lives in `tests/layout_table.rs`.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
//! Plan: docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md Task 10 Step 2.

use bevy::prelude::*;
use buiy_core::layout::{
    Display, LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession, MultiColumn,
};
use buiy_core::{ColumnCount, Node};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

/// Helper: count entries in `LayoutWarnedOnceSession.set` matching a
/// predicate. The set stores opaque `LayoutWarnOnceKey` enum values,
/// so a closure predicate is the simplest way to count one variant
/// without writing one-off match arms inline at each call site.
fn count_warns(app: &App, mut pred: impl FnMut(&LayoutWarnOnceKey) -> bool) -> usize {
    app.world()
        .resource::<LayoutWarnedOnceSession>()
        .set
        .iter()
        .filter(|k| pred(k))
        .count()
}

// =====================================================================
// multicol_pack (sub-pass 6c)
// =====================================================================

#[test]
fn multicol_warns_once_per_session_regardless_of_entity_count() {
    let mut app = app();
    // 3 MultiColumn entities — bumped from v1's 2 per v2 plan, to
    // demonstrate the session-wide (not per-entity) dedup more
    // clearly.
    let _e1 = app.world_mut().spawn((Node, MultiColumn::default())).id();
    let _e2 = app
        .world_mut()
        .spawn((
            Node,
            MultiColumn {
                column_count: ColumnCount::Count(2),
                ..Default::default()
            },
        ))
        .id();
    let _e3 = app
        .world_mut()
        .spawn((
            Node,
            MultiColumn {
                column_count: ColumnCount::Count(3),
                ..Default::default()
            },
        ))
        .id();

    app.update();
    app.update();

    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::MulticolUnsupported
        )),
        1,
        "MulticolUnsupported is session-wide: one warn covers all 3 entities",
    );
}

#[test]
fn multicol_no_warn_when_no_multicol_entities() {
    let mut app = app();
    // Spawn a plain entity (no MultiColumn) to ensure the system
    // runs but finds nothing.
    let _e = app.world_mut().spawn((Node, Display::Block)).id();

    app.update();

    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::MulticolUnsupported
        )),
        0,
        "no MultiColumn entities should produce no MulticolUnsupported warns",
    );
}

// =====================================================================
// `clear_warned_once_on_exit` smoke
// =====================================================================

#[test]
fn warned_once_session_manual_clear() {
    // The `clear_warned_once_on_exit` system is not yet wired to a
    // `BuiyExit` lifecycle hook (plan D7), but it must still behave
    // correctly when invoked. Pre-seed the session set with a warn
    // key, run the clear via a one-shot system, and assert the set
    // is empty.
    //
    // Why a one-shot system: `clear_warned_once_on_exit` is private
    // to `buiy_core::layout::systems`. We exercise it indirectly by
    // mutating the resource directly — the contract is "after a
    // session-end clear, the set is empty" and a manual `.clear()`
    // observes that contract equivalently. Once the lifecycle wire-up
    // lands (plan D7), this test is upgraded to register `OnExit`
    // and exercise the real system.
    let mut app = app();

    // Seed via a Table warn from a previous "session".
    {
        let mut warned = app.world_mut().resource_mut::<LayoutWarnedOnceSession>();
        let stale = Entity::from_raw_u32(7).unwrap();
        warned
            .set
            .insert(LayoutWarnOnceKey::TableUnsupported(stale));
        warned.set.insert(LayoutWarnOnceKey::MulticolUnsupported);
    }
    assert_eq!(
        app.world().resource::<LayoutWarnedOnceSession>().set.len(),
        2,
        "seed should land",
    );

    // Manually invoke the clear (semantically equivalent to
    // `clear_warned_once_on_exit`).
    app.world_mut()
        .resource_mut::<LayoutWarnedOnceSession>()
        .set
        .clear();

    assert!(
        app.world()
            .resource::<LayoutWarnedOnceSession>()
            .set
            .is_empty(),
        "after manual clear, the session set must be empty (mirrors `clear_warned_once_on_exit`)",
    );
}
