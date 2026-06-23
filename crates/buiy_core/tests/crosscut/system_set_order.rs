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

/// Audit #34 (T2.20): set *membership*, not just inter-set ordering. The tests
/// above pin that the seven sets run in the documented order, but they say
/// nothing about which plugin's systems actually land in each set — with only
/// CorePlugin (and its empty downstream sets) the membership is unexercised. A
/// plugin that silently drops `.in_set(BuiySet::X)` from its registration would
/// leave the ordering test green while its system runs unordered.
///
/// Mirrors the delta-count idiom in `tests/render_forced_colors_swap.rs`: count
/// a set's systems with the plugin minus without it, so CorePlugin's own
/// membership (and any unrelated growth) cancels and only the plugin's
/// contribution remains. In this Bevy build the system *objects* are moved into
/// the executable and aren't retained in `graph.systems` after `app.update()`
/// (see the note below), so membership can only be counted, not name-matched —
/// the delta-count is the deterministic discriminator available without GPU.
fn set_membership_delta(set: BuiySet, add_plugin: impl Fn(&mut App) + Copy) -> usize {
    let count = |with_plugin: bool| -> usize {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        if with_plugin {
            add_plugin(&mut app);
        }
        // Forces `Schedule::initialize` so set membership is populated.
        app.update();

        let schedules = app.world().resource::<Schedules>();
        let schedule = schedules
            .get(Update)
            .expect("CorePlugin registered systems on Update");
        schedule
            .graph()
            .systems_in_set(set.intern())
            .map(|keys| keys.len())
            .unwrap_or(0)
    };
    count(true) - count(false)
}

#[test]
fn layout_plugin_populates_layout_set() {
    // `LayoutPlugin` schedules its pipeline (sync_styles → Taffy → write
    // resolved layout → the sub-pass chain) into `BuiySet::Layout`
    // (`layout/pipeline.rs:99`). Dropping that `.in_set(BuiySet::Layout)` would
    // collapse the delta to 0. Asserting `>= 1` (not a hardcoded count) keeps
    // the test robust to the pipeline growing more sub-pass systems.
    let delta = set_membership_delta(BuiySet::Layout, |app| {
        app.add_plugins(buiy_core::layout::LayoutPlugin);
    });
    assert!(
        delta >= 1,
        "LayoutPlugin must place its pipeline systems in BuiySet::Layout (delta was {delta})"
    );
}

#[test]
fn focus_plugin_populates_input_set() {
    // `FocusPlugin::build` adds `handle_tab.in_set(BuiySet::Input)`
    // (`focus.rs:56`).
    let delta = set_membership_delta(BuiySet::Input, |app| {
        // `handle_tab` reads `Res<ButtonInput<KeyCode>>`; MinimalPlugins omits
        // InputPlugin, so seed the resource (mirrors tests/focus.rs) to keep
        // `app.update()` from panicking on a missing param.
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_plugins(buiy_core::focus::FocusPlugin);
    });
    assert!(
        delta >= 1,
        "FocusPlugin must place handle_tab in BuiySet::Input (delta was {delta})"
    );
}

#[test]
fn picking_plugin_registers_pointer_producers_as_observers() {
    // C3c retired `update_hovered` (the lone `BuiySet::Picking` system), so
    // `PickingPlugin` no longer populates that set — it now registers its C3
    // pointer producers as OBSERVERS on the `Pointer<E>` stream
    // (`pointer_click_emits_on_press` → `OnPress`, `derive_multi_click` →
    // `MultiClick`; `picking/mod.rs`). Observers are entities carrying the
    // `Observer` component, so count them with the plugin vs without it: the
    // delta is `PickingPlugin`'s contribution (plus whatever bevy_picking's
    // `InteractionPlugin` brings, which is also part of what `PickingPlugin`
    // composes). This replaces the old in-set membership check with the faithful
    // successor — the producers moved from a scheduled system to observers.
    //
    // `BuiySet::Picking` itself survives as the pure ordering anchor upstream
    // writers target via `.before(BuiySet::Picking)` (the transform bridge, clip
    // rects, visibility, the text caret); its inter-set placement is pinned by
    // `buiy_sets_run_in_documented_order` above.
    let count_observers = |with_plugin: bool| -> usize {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        if with_plugin {
            // bevy's `PickingPlugin` provides the `Messages<PointerHits>` +
            // `PickingSystems` Buiy's `PickingPlugin` builds on.
            app.add_plugins(bevy::picking::PickingPlugin);
            app.add_plugins(buiy_core::picking::PickingPlugin);
        }
        app.update();
        let mut q = app.world_mut().query::<&bevy::ecs::observer::Observer>();
        q.iter(app.world()).count()
    };
    let delta = count_observers(true) - count_observers(false);
    assert!(
        delta >= 2,
        "PickingPlugin must register its C3 pointer producers as observers \
         (pointer_click_emits_on_press + derive_multi_click); observed delta was {delta}"
    );
}

#[test]
fn a11y_plugin_populates_a11y_update_set() {
    // `A11yPlugin::build` adds `build_tree.in_set(BuiySet::A11yUpdate)`
    // (`a11y/mod.rs:85`).
    let delta = set_membership_delta(BuiySet::A11yUpdate, |app| {
        app.add_plugins(buiy_core::a11y::A11yPlugin);
    });
    assert!(
        delta >= 1,
        "A11yPlugin must place build_tree in BuiySet::A11yUpdate (delta was {delta})"
    );
}

#[test]
fn plugins_only_populate_their_own_set() {
    // Membership is *selective*: FocusPlugin must NOT inflate the A11yUpdate set.
    // This gives the membership assertions teeth — a plugin that mis-tags its
    // system into the wrong set would still pass a lone `>= 1` on the right set,
    // but reddens here.
    //
    // P1c-b INTENTIONALLY adds the inbound action router to `BuiySet::Input` from
    // `A11yPlugin` (action-router.md §7): the router (`route_action_requests`) +
    // its Button keyboard sibling (`button_keyboard_activation`) are Input-stage
    // PRODUCERS — they synthesize focus/activation in `Input` so an inbound
    // request reflects outbound in the SAME frame's `A11yUpdate`. So A11yPlugin
    // now legitimately contributes to BOTH `A11yUpdate` (the outbound
    // `build_tree`) and `Input` (the inbound router). The earlier "A11yPlugin
    // adds nothing to Input" invariant is superseded by the P1c-b router. Pin the
    // exact count so an accidental mis-tag (e.g. dropping `.in_set(Input)`, or a
    // future system landing in the wrong set) still reddens.
    let a11y_into_input = set_membership_delta(BuiySet::Input, |app| {
        app.add_plugins(buiy_core::a11y::A11yPlugin);
    });
    assert_eq!(
        a11y_into_input, 2,
        "A11yPlugin adds exactly the P1c-b inbound router systems \
         (route_action_requests + button_keyboard_activation) to BuiySet::Input"
    );
    let focus_into_a11y = set_membership_delta(BuiySet::A11yUpdate, |app| {
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_plugins(buiy_core::focus::FocusPlugin);
    });
    assert_eq!(
        focus_into_a11y, 0,
        "FocusPlugin must not add systems to BuiySet::A11yUpdate"
    );
}

// NOTE: the plan's belt-and-suspenders name-probe (locating
// `write_buiy_transform` / `mark_dirty_trees` / `propagate_parent_transforms`
// / `sync_simple_transforms` by name in the Update toposort) is intentionally
// NOT implemented here. In this build configuration `System::name()` returns
// the placeholder `<Enable the debug feature to see the name>` (Bevy strips
// system names without the `debug` feature), and the `dependency()` DAG's
// toposort holds only set + sync-point nodes — the real user systems do not
// resolve through `graph.systems`. The plan (clip-and-transform R3 plan,
// Task 3 Note) anticipates this: "If `graph.systems` is not directly
// iterable… drop the toposort-name probe. The behavior test (`GlobalTransform`
// final after `Update`) is the load-bearing gate." That gate lives in
// `tests/render_transform_bridge.rs`
// (`global_transform_is_final_after_update_no_postupdate_needed` +
// `nested_transforms_compose_through_global_transform`), and the
// chain's `.before(BuiySet::Picking)` ordering is pinned by the set-order
// tests above.
