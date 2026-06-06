//! Headless: pin that `write_clip_rects` is scheduled `.after(BuiySet::Animate)`
//! and `.before(BuiySet::Picking)` (architecture.md § 5.2), so picking + extract
//! always see settled `ClipRect`s. Without this guard a refactor could move the
//! writer into `BuiySet::Render` or drop `.before(Picking)` — both compile and
//! pass every behavioral test while feeding picking a stale or absent clip.
//!
//! System names are unavailable in this build (bevy_utils `debug` feature is
//! off, so `System::name()` is a placeholder), so we cannot locate the writer
//! by name the way `system_set_order.rs` locates `BuiySet` nodes. Instead we
//! assert the two ordering EDGES directly on the authored dependency graph:
//! exactly one system node sits between the `Animate` and `Picking` set nodes
//! (an `Animate → W` edge from `.after(Animate)` and a `W → Picking` edge from
//! `.before(Picking)`). `write_clip_rects` is the only system Buiy schedules
//! that way, so dropping either constraint — or moving the writer to another
//! set — removes its edge and the count falls to 0.

use bevy::ecs::schedule::NodeId;
use bevy::prelude::*;
use buiy_core::{BuiySet, CorePlugin, layout::LayoutPlugin, render::BuiyRenderPlugin};

/// Number of `Update` systems that have BOTH an incoming dependency edge from
/// `BuiySet::Animate` and an outgoing one to `BuiySet::Picking` — i.e. systems
/// scheduled `.after(Animate).before(Picking)`. Build a fresh app, run one tick
/// to force `Schedule::initialize` → `build_schedule` (which populates the
/// dependency graph), then read the edges back out.
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
        1,
        "exactly one system (write_clip_rects) must be ordered \
         .after(BuiySet::Animate).before(BuiySet::Picking); a count of 0 means a \
         constraint was dropped or the writer moved into another set, which would \
         feed picking/extract a stale or absent ClipRect",
    );
}
