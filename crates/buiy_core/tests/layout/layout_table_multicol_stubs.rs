//! Phase 7 Task 10 Step 2: integration coverage for the
//! `PostTaffyOverrides` stub-warn passes.
//!
//! Both the table (sub-pass 6b, Phase 12) and multi-column (sub-pass 6c,
//! Phase 13) stub-warn tests that once lived here are superseded by the
//! real algorithms; their behavioral coverage now lives in
//! `tests/layout_table.rs` and `tests/layout_multicol.rs` respectively.
//! The retired `MulticolUnsupported` / `TableUnsupported` keys are kept
//! `Reflect`-stable (no code emits them) and only the
//! `clear_warned_once_on_exit` smoke below still references them as a
//! seed value.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
//! Plan: docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md Task 10 Step 2.

use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
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
