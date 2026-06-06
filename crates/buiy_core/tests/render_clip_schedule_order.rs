//! Headless: pin that render-prep writers are scheduled `.after(BuiySet::Animate)`
//! and `.before(BuiySet::Picking)` (architecture.md § 5.2), so picking + extract
//! always see settled `ClipRect`s. Without this guard a refactor could move a
//! writer into `BuiySet::Render` or drop `.before(Picking)` — both compile and
//! pass every behavioral test while feeding picking a stale or absent clip.
//!
//! System names are unavailable in this build (bevy_utils `debug` feature is
//! off, so `System::name()` is a placeholder), so we cannot locate a writer by
//! name the way `system_set_order.rs` locates `BuiySet` nodes. Instead we assert
//! the two ordering EDGES directly on the authored dependency graph: at least
//! one node sits between the `Animate` and `Picking` set nodes (an `Animate → W`
//! edge from `.after(Animate)` and a `W → Picking` edge from `.before(Picking)`).
//! Both `write_clip_rects` and the R3 transform bridge
//! (`seed_scroll_dirty → write_buiy_transform → mark_dirty_trees →
//! propagate_parent_transforms → sync_simple_transforms`, see
//! clip-and-transform.md § B) are deliberately scheduled in this same window —
//! they touch disjoint components (`ClipRect` vs `Transform`) and intentionally
//! share the `.after(Animate).before(Picking)` slot. Because every `Commands`-
//! using writer in the window also pulls in an auto-inserted `ApplyDeferred`
//! sync point that carries both edges, the count is larger than the number of
//! authored systems.
//!
//! A `>= 1` lower bound would be too loose to be a guard: the bridge alone
//! contributes several in-window nodes, so moving `write_clip_rects` out of the
//! window (the exact regression this test exists to catch) would still leave the
//! count non-zero and the assertion green. We therefore pin the EXACT in-window
//! node count (`EXPECTED_NODES_IN_WINDOW`). A drop from N to N−1 — one writer
//! leaving the `.after(Animate).before(Picking)` slot — fails the assertion. A
//! legitimate addition of a new render-prep writer to the window is also a
//! deliberate change to this invariant: bump the constant *and* confirm the new
//! writer really belongs between `Animate` and `Picking`.

use bevy::ecs::schedule::NodeId;
use bevy::prelude::*;
use buiy_core::{BuiySet, CorePlugin, layout::LayoutPlugin, render::BuiyRenderPlugin};

/// Exact number of `Update` system nodes (authored systems + the
/// `ApplyDeferred` sync points their `Commands` pull in) that sit between the
/// `Animate` and `Picking` set nodes with this plugin stack. Pinned so that one
/// writer leaving the `.after(Animate).before(Picking)` window (N → N−1) fails
/// the assertion. Authored in-window systems: the R3 bridge chain
/// (`seed_scroll_dirty`, `write_buiy_transform`, `mark_dirty_trees`,
/// `propagate_parent_transforms`, `sync_simple_transforms`) plus
/// `write_clip_rects`; the remainder are auto-inserted `ApplyDeferred`
/// boundaries. If you intentionally add or remove a render-prep writer in this
/// window, update this constant in the same change.
const EXPECTED_NODES_IN_WINDOW: usize = 6;

/// Number of `Update` nodes that have BOTH an incoming dependency edge from
/// `BuiySet::Animate` and an outgoing one to `BuiySet::Picking` — i.e. systems
/// (and the `ApplyDeferred` sync points their `Commands` pull in) scheduled
/// `.after(Animate).before(Picking)`. Build a fresh app, run one tick to force
/// `Schedule::initialize` → `build_schedule` (which populates the dependency
/// graph), then read the edges back out.
fn systems_after_animate_before_picking() -> usize {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();

    let schedules = app.world().resource::<Schedules>();
    let schedule = schedules
        .get(Update)
        .expect("BuiyRenderPlugin scheduled write_clip_rects on Update");
    let graph = schedule.graph();
    let dependency = graph.dependency().graph();

    let set_node = |set: BuiySet| {
        let key = graph
            .system_sets
            .get_key(set.intern())
            .unwrap_or_else(|| panic!("BuiySet::{set:?} not registered on Update"));
        NodeId::Set(key)
    };
    let animate = set_node(BuiySet::Animate);
    let picking = set_node(BuiySet::Picking);

    dependency
        .nodes()
        .filter(|n| n.as_system().is_some())
        .filter(|&n| dependency.contains_edge(animate, n) && dependency.contains_edge(n, picking))
        .count()
}

#[test]
fn write_clip_rects_is_scheduled_after_animate_and_before_picking() {
    assert_eq!(
        systems_after_animate_before_picking(),
        EXPECTED_NODES_IN_WINDOW,
        "exactly {EXPECTED_NODES_IN_WINDOW} render-prep nodes must sit \
         .after(BuiySet::Animate).before(BuiySet::Picking) (write_clip_rects, the \
         R3 transform bridge chain, and their ApplyDeferred sync points). A LOWER \
         count means a writer left the window — e.g. write_clip_rects moved into \
         BuiySet::Render or lost its .before(Picking) edge — which would feed \
         picking/extract a stale or absent ClipRect. A HIGHER count means a new \
         in-window writer was added; if intentional, bump EXPECTED_NODES_IN_WINDOW.",
    );
}
