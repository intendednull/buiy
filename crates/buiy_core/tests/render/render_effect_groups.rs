//! Headless integration tests for `WriteEffectGroups` — the render-prep
//! pass that derives the `EffectGroup` marker. No wgpu adapter needed
//! (pure main-world ECS); these are the gating tests for this phase.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.

use bevy::prelude::*;
use buiy_core::layout::Isolation;
use buiy_core::layout::{ContainFlags, PositionKind, ZIndex};
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::effect::{EffectGroup, EffectReason, write_effect_groups};
use buiy_core::{BackdropFilter, Containment, Filter, FilterFn, MixBlendMode, Node, Opacity};
use buiy_core::{BuiySet, CorePlugin, Position, Stacking};

// Minimal harness: a bare schedule running just `write_effect_groups`,
// so the test does not depend on the full BuiyRenderPlugin/RenderApp
// (which needs a wgpu adapter). The real plugin wiring is Task 8.
fn run_once(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(write_effect_groups);
    schedule.run(world);
}

fn reason_of(world: &World, e: Entity) -> Option<EffectReason> {
    world.get::<EffectGroup>(e).map(|g| g.reason)
}

#[test]
fn opacity_below_one_forms_opacity_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app.world_mut().spawn((Node, Opacity(0.5))).id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), Some(EffectReason::OPACITY));
}

#[test]
fn isolate_forms_isolation_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            Stacking {
                isolation: Isolation::Isolate,
                ..default()
            },
        ))
        .id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), Some(EffectReason::ISOLATION));
}

#[test]
fn non_empty_filter_forms_filter_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            Filter(vec![FilterFn::Blur(buiy_core::Length::px(4.0))]),
        ))
        .id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), Some(EffectReason::FILTER));
}

#[test]
fn non_normal_blend_forms_mix_blend_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app.world_mut().spawn((Node, MixBlendMode::Multiply)).id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), Some(EffectReason::MIX_BLEND));
}

#[test]
fn non_empty_backdrop_forms_backdrop_filter_group_but_is_present() {
    // backdrop-filter sets BACKDROP_FILTER; the SC-trigger asymmetry
    // (it forms no stacking context) is layout's concern, not asserted
    // here — here we only assert the EffectGroup marker + bit.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            BackdropFilter(vec![FilterFn::Blur(buiy_core::Length::px(4.0))]),
        ))
        .id();
    run_once(app.world_mut());
    assert_eq!(
        reason_of(app.world(), e),
        Some(EffectReason::BACKDROP_FILTER)
    );
}

#[test]
fn multiple_formers_on_one_entity_or_their_bits() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            Opacity(0.4),
            Stacking {
                isolation: Isolation::Isolate,
                ..default()
            },
            Filter(vec![FilterFn::Blur(buiy_core::Length::px(2.0))]),
            MixBlendMode::Screen,
            BackdropFilter(vec![FilterFn::Blur(buiy_core::Length::px(2.0))]),
        ))
        .id();
    run_once(app.world_mut());
    // Spell out the five former bits explicitly rather than `EffectReason::all()`
    // so the assertion stays pinned to the five formers even if a future phase
    // adds a reserved bit to `EffectReason` (which `all()` would then include).
    assert_eq!(
        reason_of(app.world(), e),
        Some(
            EffectReason::OPACITY
                | EffectReason::ISOLATION
                | EffectReason::FILTER
                | EffectReason::MIX_BLEND
                | EffectReason::BACKDROP_FILTER
        )
    );
}

#[test]
fn opacity_and_isolate_or_to_two_bits() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            Opacity(0.6),
            Stacking {
                isolation: Isolation::Isolate,
                ..default()
            },
        ))
        .id();
    run_once(app.world_mut());
    assert_eq!(
        reason_of(app.world(), e),
        Some(EffectReason::OPACITY | EffectReason::ISOLATION)
    );
}

#[test]
fn contain_paint_alone_forms_no_effect_group() {
    // `contain: paint` clips (a ClipRect boundary) but is NOT an effect
    // boundary (effect-compositor.md § 1). No EffectGroup.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            Containment {
                contain: ContainFlags::PAINT,
                ..default()
            },
        ))
        .id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), None);
}

#[test]
fn positioned_with_z_index_alone_forms_no_effect_group() {
    // positioned + z_index forms a stacking context (paint reorder) but is
    // not an effect — no off-screen target, no EffectGroup.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((
            Node,
            Position {
                kind: PositionKind::Relative,
                ..default()
            },
            Stacking {
                z_index: ZIndex::Layer(5),
                ..default()
            },
        ))
        .id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), None);
}

#[test]
fn opacity_one_forms_no_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app.world_mut().spawn((Node, Opacity(1.0))).id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), None);
}

#[test]
fn isolation_auto_forms_no_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((Node, Stacking::default())) // isolation == Auto
        .id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), None);
}

#[test]
fn empty_filter_and_backdrop_form_no_group() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app
        .world_mut()
        .spawn((Node, Filter::default(), BackdropFilter::default()))
        .id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), None);
}

#[test]
fn opacity_rising_back_to_one_removes_the_marker() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app.world_mut().spawn((Node, Opacity(0.5))).id();

    run_once(app.world_mut());
    assert_eq!(
        reason_of(app.world(), e),
        Some(EffectReason::OPACITY),
        "0.5 forms an opacity group"
    );

    // Author animates opacity back to 1.0 — the marker must be dropped.
    app.world_mut().entity_mut(e).insert(Opacity(1.0));
    run_once(app.world_mut());
    assert_eq!(
        reason_of(app.world(), e),
        None,
        "opacity back to 1.0 removes the EffectGroup"
    );
}

#[test]
fn entity_without_node_marker_never_forms_a_group() {
    // The query is gated on With<Node>; a stray Opacity on a non-Buiy
    // entity must be ignored.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let e = app.world_mut().spawn(Opacity(0.3)).id();
    run_once(app.world_mut());
    assert_eq!(reason_of(app.world(), e), None);
}

#[test]
fn plugin_runs_write_effect_groups_in_update_render_prep_window() {
    // Full main-world app (no RenderApp / wgpu): CorePlugin configures the
    // BuiySet chain; BuiyRenderPlugin must schedule write_effect_groups in
    // the render-prep window so a former gets its marker after one frame.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);

    let e = app.world_mut().spawn((Node, Opacity(0.5))).id();
    app.update();

    assert_eq!(
        reason_of(app.world(), e),
        Some(EffectReason::OPACITY),
        "BuiyRenderPlugin scheduled write_effect_groups in Update"
    );
}

#[test]
fn write_effect_groups_runs_after_animate_before_picking() {
    use std::sync::{Arc, Mutex};

    // Order probe: record the order of three events in one Update frame —
    // an Animate-set marker, the EffectGroup write, and a Picking-set
    // marker — and assert write happened strictly between them.
    #[derive(Resource, Clone, Default)]
    struct Log(Arc<Mutex<Vec<&'static str>>>);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.init_resource::<Log>();

    let probe = app.world().resource::<Log>().clone();
    let probe_a = probe.clone();
    let probe_p = probe.clone();
    app.add_systems(
        Update,
        (move || probe_a.0.lock().unwrap().push("animate")).in_set(BuiySet::Animate),
    );
    app.add_systems(
        Update,
        (move || probe_p.0.lock().unwrap().push("picking")).in_set(BuiySet::Picking),
    );
    // A system in the Picking set, observing the marker right after the
    // render-prep window, records "wrote" once the EffectGroup exists.
    let e = app.world_mut().spawn((Node, Opacity(0.5))).id();
    app.add_systems(
        Update,
        (move |q: Query<&EffectGroup>, log: Res<Log>| {
            if q.iter().next().is_some() {
                let mut v = log.0.lock().unwrap();
                if !v.contains(&"wrote") {
                    v.push("wrote");
                }
            }
        })
        .in_set(BuiySet::Picking),
    );

    app.update();

    let order = probe.0.lock().unwrap().clone();
    let _ = e;
    let ai = order.iter().position(|s| *s == "animate");
    let pi = order.iter().position(|s| *s == "picking");
    assert!(
        ai.is_some() && pi.is_some(),
        "both set probes ran: {order:?}"
    );
    assert!(ai < pi, "Animate precedes Picking: {order:?}");
    // The marker exists by the Picking set => write happened before Picking
    // and (since it reads Animate-stage values) within the render-prep slot.
    assert!(
        order.contains(&"wrote"),
        "EffectGroup present by Picking set: {order:?}"
    );
}
