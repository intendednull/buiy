//! 9-step pipeline order asserted at the integration level.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node,
    layout::{BuiyLayoutStep, ContainerQuery, LayoutPlugin, Length, QueryCondition, Sizing, Style},
};

#[test]
fn layout_steps_are_chained_in_declared_order() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    // Force an Update build so set ordering is materialized.
    app.update();

    // The Schedule API in 0.18 doesn't expose a stable enumeration of
    // SystemSet ordering directly. We use the existence-and-ordering
    // contract: every BuiyLayoutStep set is configured, and configuring
    // a contradictory order fails schedule build. The smoke check here
    // is that adding a tracker system to each set runs them in the
    // declared order.
    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));

    fn make_tracker(
        order: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
        label: &'static str,
    ) -> impl Fn() + Send + Sync + 'static {
        move || {
            order.lock().unwrap().push(label);
        }
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let o = order.clone();
    app.add_systems(
        Update,
        make_tracker(o.clone(), "gc").in_set(BuiyLayoutStep::RemovedNodesGc),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "wmi").in_set(BuiyLayoutStep::WritingModeInherit),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "sync").in_set(BuiyLayoutStep::SyncStyles),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "cq_activate").in_set(BuiyLayoutStep::CqActivate),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "taffy").in_set(BuiyLayoutStep::TaffyCompute),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "cq_flip").in_set(BuiyLayoutStep::CqFlipCheck),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "cq_rerun").in_set(BuiyLayoutStep::CqFlipReRun),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "post_taffy").in_set(BuiyLayoutStep::PostTaffyOverrides),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "write").in_set(BuiyLayoutStep::WriteResolvedLayout),
    );

    // Phase 5 Task 10: spawn one Container + one ContainerQuery + one
    // descendant with Cqw so cq_activate / cq_flip_check / cq_flip_rerun
    // (and `translate_one_entity`'s `Cq*` resolution) all have reachable
    // work. The order assertion below stays unchanged; this addition
    // makes the order test also a smoke test that the cq systems
    // compile and run with realistic data.
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
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    app.update();

    let observed = order.lock().unwrap().clone();
    assert_eq!(
        observed,
        vec![
            "gc",
            "wmi",
            "sync",
            "cq_activate",
            "taffy",
            "cq_flip",
            "cq_rerun",
            "post_taffy",
            "write",
        ],
        "BuiyLayoutStep sets did not run in declared order",
    );
}
