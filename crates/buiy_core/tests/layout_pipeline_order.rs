//! 8-step pipeline order asserted at the integration level.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    layout::{BuiyLayoutStep, LayoutPlugin},
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
        make_tracker(o.clone(), "0").in_set(BuiyLayoutStep::RemovedNodesGc),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "1").in_set(BuiyLayoutStep::SyncStyles),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "2").in_set(BuiyLayoutStep::CqActivate),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "3").in_set(BuiyLayoutStep::TaffyCompute),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "4").in_set(BuiyLayoutStep::CqFlipCheck),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "5").in_set(BuiyLayoutStep::CqFlipReRun),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "6").in_set(BuiyLayoutStep::PostTaffyOverrides),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "7").in_set(BuiyLayoutStep::WriteResolvedLayout),
    );

    app.update();

    let observed = order.lock().unwrap().clone();
    assert_eq!(
        observed,
        vec!["0", "1", "2", "3", "4", "5", "6", "7"],
        "BuiyLayoutStep sets did not run in declared order",
    );
}
