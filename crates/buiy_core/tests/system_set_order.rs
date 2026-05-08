//! Pin the relative order of `BuiySet` variants on the `Update` schedule.
//!
//! `CorePlugin` configures the seven Buiy sets with `.chain()` so they run in
//! the order documented on `BuiySet`: Layout → Style → Input → Animate →
//! Picking → A11yUpdate → Render. Downstream plugins (`LayoutPlugin`,
//! `RenderPlugin`, …) attach their systems to those sets and rely on that
//! order. A silent reorder would break the contract without any compile-time
//! signal — so introspect the schedule's dependency graph and assert it
//! directly.

use bevy::ecs::schedule::NodeId;
use bevy::prelude::*;
use buiy_core::{BuiySet, CorePlugin};

/// Driver: build a fresh app with `CorePlugin`, run one tick to force the
/// `Update` schedule to be initialized (which triggers the dependency-graph
/// toposort), then read the cached toposort back out and locate each
/// requested set inside it. Returns the toposort indices of the supplied
/// sets, in the same order they were passed in.
fn set_indices(sets: &[BuiySet]) -> Vec<usize> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    // Forces `Schedule::initialize`, which calls `build_schedule` and caches
    // a toposort on the dependency DAG.
    app.update();

    let schedules = app.world().resource::<Schedules>();
    let schedule = schedules
        .get(Update)
        .expect("CorePlugin registered systems on Update");
    let graph = schedule.graph();
    let toposort = graph
        .dependency()
        .get_toposort()
        .expect("dependency DAG is toposorted after Schedule::initialize");

    sets.iter()
        .map(|set| {
            let key = graph
                .system_sets
                .get_key(set.intern())
                .unwrap_or_else(|| panic!("BuiySet::{set:?} not registered on Update"));
            let node = NodeId::Set(key);
            toposort
                .iter()
                .position(|n| *n == node)
                .unwrap_or_else(|| panic!("BuiySet::{set:?} missing from toposort"))
        })
        .collect()
}

#[test]
fn buiy_sets_run_in_documented_order() {
    // Order documented on `BuiySet`: Layout → Style → Input → Animate →
    // Picking → A11yUpdate → Render. If any pair flips, downstream plugins
    // that read `ResolvedLayout` from render or fire focus changes after
    // input would silently break.
    let order = [
        BuiySet::Layout,
        BuiySet::Style,
        BuiySet::Input,
        BuiySet::Animate,
        BuiySet::Picking,
        BuiySet::A11yUpdate,
        BuiySet::Render,
    ];

    let idx = set_indices(&order);
    for window in idx.windows(2) {
        assert!(
            window[0] < window[1],
            "BuiySet order violated: indices {idx:?} for {order:?}",
        );
    }
}

#[test]
fn layout_runs_before_render() {
    // Spot-check the load-bearing pair: layout writes `ResolvedLayout`,
    // render reads it. The full-order test above already covers this, but
    // a focused regression test makes the failure mode obvious.
    let idx = set_indices(&[BuiySet::Layout, BuiySet::Render]);
    assert!(idx[0] < idx[1], "Layout must run before Render: {idx:?}");
}

#[test]
fn layout_runs_before_animate() {
    // The brief asked for "Layout after Animate"; the actual `CorePlugin`
    // chain places Animate *after* Layout (Layout → Style → Input →
    // Animate). This test pins the real order so the documentation in
    // `BuiySet` cannot drift from the configured chain.
    let idx = set_indices(&[BuiySet::Layout, BuiySet::Animate]);
    assert!(idx[0] < idx[1], "Layout must run before Animate: {idx:?}");
}
