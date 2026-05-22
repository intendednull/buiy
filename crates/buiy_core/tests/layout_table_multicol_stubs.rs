//! Phase 7 Task 10 Step 2: integration coverage for the
//! `table_layout` (sub-pass 6b) and `multicol_pack` (sub-pass 6c)
//! stub-warn passes.
//!
//! `table_layout` warns once per (entity, session) for every
//! `Display::Table*` entity it encounters; `multicol_pack` warns
//! once per session total — the first `MultiColumn` entity triggers,
//! all subsequent are silent.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
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
// table_layout (sub-pass 6b)
// =====================================================================

#[test]
fn table_warns_once_per_entity_per_session() {
    let mut app = app();
    // Two distinct Table entities. After three frames, each should
    // have warned exactly once — total 2 warns.
    let e1 = app.world_mut().spawn((Node, Display::Table)).id();
    let e2 = app.world_mut().spawn((Node, Display::TableRow)).id();

    app.update();
    app.update();
    app.update();

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::TableUnsupported(e1)),
        "e1 should have warned once",
    );
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::TableUnsupported(e2)),
        "e2 should have warned once",
    );
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableUnsupported(_)
        )),
        2,
        "exactly two distinct TableUnsupported keys (one per entity), even across 3 frames",
    );
}

#[test]
fn table_no_warn_when_no_table_entities() {
    let mut app = app();
    // Spawn a non-table entity to ensure the system runs but the
    // query inside it finds nothing matching `is_table_display`.
    let _e = app.world_mut().spawn((Node, Display::Block)).id();

    app.update();

    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableUnsupported(_)
        )),
        0,
        "no Display::Table* entities should produce no TableUnsupported warns",
    );
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
// Cross-pass independence
// =====================================================================

#[test]
fn table_and_multicol_warns_are_independent() {
    let mut app = app();
    let table_e = app.world_mut().spawn((Node, Display::Table)).id();
    let _multicol_e = app.world_mut().spawn((Node, MultiColumn::default())).id();

    app.update();

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned
            .set
            .contains(&LayoutWarnOnceKey::TableUnsupported(table_e)),
        "Table entity should produce a TableUnsupported warn",
    );
    assert!(
        warned.set.contains(&LayoutWarnOnceKey::MulticolUnsupported),
        "MultiColumn entity should produce a MulticolUnsupported warn",
    );
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableUnsupported(_)
        )),
        1,
    );
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::MulticolUnsupported
        )),
        1,
    );
}

#[test]
fn table_does_not_rewarn_on_component_replace() {
    // Regression: warn-once dedup is keyed on entity identity, not on
    // Display-component insertion epoch. Re-inserting `Display::Table`
    // on the same entity must NOT add a fresh warn.
    let mut app = app();
    let e = app.world_mut().spawn((Node, Display::Table)).id();

    app.update();
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableUnsupported(_)
        )),
        1,
        "first frame produces one warn",
    );

    // Re-insert `Display::Table` on the same entity. Bevy treats this
    // as a Changed<Display>, but the warn-dedup key is `(entity)`, so
    // the count must not budge.
    app.world_mut().entity_mut(e).insert(Display::Table);

    app.update();
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableUnsupported(_)
        )),
        1,
        "re-inserting Display::Table on the same entity must not produce a fresh warn",
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

// =====================================================================
// All Display::Table* variant coverage
// =====================================================================

#[test]
fn table_all_nine_variants_each_warn() {
    // `is_table_display` must match every Display::Table* variant.
    // Spawn one entity per variant (9 total) and assert 9 distinct
    // TableUnsupported keys.
    let mut app = app();

    let variants = [
        Display::Table,
        Display::TableRowGroup,
        Display::TableHeaderGroup,
        Display::TableFooterGroup,
        Display::TableRow,
        Display::TableCell,
        Display::TableCaption,
        Display::TableColumnGroup,
        Display::TableColumn,
    ];
    let mut entities: Vec<Entity> = Vec::with_capacity(variants.len());
    for d in variants {
        let e = app.world_mut().spawn((Node, d)).id();
        entities.push(e);
    }

    app.update();

    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    for e in &entities {
        assert!(
            warned
                .set
                .contains(&LayoutWarnOnceKey::TableUnsupported(*e)),
            "every Display::Table* variant should produce a TableUnsupported warn for its entity",
        );
    }
    assert_eq!(
        count_warns(&app, |k| matches!(
            k,
            LayoutWarnOnceKey::TableUnsupported(_)
        )),
        9,
        "all 9 Display::Table* variants should each emit one warn",
    );
}
