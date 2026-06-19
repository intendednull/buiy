//! Task 2.6 self-test for per-timestamp animation snapshots
//! (`assert_display_list_snapshot_at`). The determinism check is a PLAIN
//! `assert_eq!` over the per-step dumps captured on two fresh apps — so the
//! meta-test of the temporal-snapshot tooling cannot pass vacuously
//! (snapshots.md § Per-timestamp, Decision 8).

use std::time::Duration;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_verify::snapshot::{NameLookup, assert_display_list_snapshot_at};

/// A pure-CPU "animation": a system that drives the box `size.x` from the
/// virtual clock (`10 + elapsed_ms/10`), so the display-list dump changes per
/// virtual timestamp — the temporal behavior under test. Deterministic: the
/// size is a pure function of `Time<Virtual>.elapsed()`, which the harness
/// advances to explicit absolute timestamps (no wall-clock).
fn animate_width(time: Res<Time<Virtual>>, mut q: Query<&mut ResolvedLayout, With<Node>>) {
    let ms = time.elapsed().as_millis() as f32;
    for mut layout in &mut q {
        layout.size.x = 10.0 + ms / 10.0;
    }
}

fn anim_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    // Pause the virtual clock so the ONLY time progression is the harness's
    // explicit advance_to steps (the determinism guarantee).
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.add_systems(Update, animate_width.after(buiy_core::BuiySet::Layout));
    app.world_mut().spawn((
        Node,
        Name::new("animated"),
        Style::default().width_px(10.0).height_px(10.0),
    ));
    app
}

/// Snapshot the three logical timestamps on a throwaway app — used by the
/// determinism test to capture dumps WITHOUT going through insta (so the test
/// can `assert_eq!` two runs directly). Mirrors `assert_display_list_snapshot_at`
/// step-driving, but returns the dumps instead of asserting them.
fn capture_steps(app: &mut App, steps: &[Duration]) -> Vec<String> {
    use buiy_core::render::components::Background;
    use buiy_core::render::extract::{ExtractedNode, ExtractedNodes, extracted_node_for};
    use buiy_core::theme::Theme;
    use buiy_verify::snapshot::display_list_dump;

    let mut out = Vec::new();
    for &t in steps {
        // Advance the virtual clock to the ABSOLUTE timestamp, then update.
        let mut virt = app.world_mut().resource_mut::<Time<Virtual>>();
        let elapsed = virt.elapsed();
        virt.advance_by(t.checked_sub(elapsed).unwrap_or(Duration::ZERO));
        app.update();

        let world = app.world();
        let names = NameLookup::from_world(world);
        let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
        let mut rows: Vec<(String, ExtractedNode)> = Vec::new();
        let mut q = world
            .try_query::<(Entity, &ResolvedLayout, Option<&Name>)>()
            .unwrap();
        for (e, layout, name) in q.iter(world) {
            let gt = world
                .get::<GlobalTransform>(e)
                .copied()
                .unwrap_or(GlobalTransform::IDENTITY);
            let bg = world.get::<Background>(e);
            let label = name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("entity#{}", e.index().index()));
            rows.push((label, extracted_node_for(e, &gt, layout, bg, None, &theme)));
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let nodes = ExtractedNodes {
            nodes: rows.into_iter().map(|(_, n)| n).collect(),
            ..Default::default()
        };
        out.push(display_list_dump(&nodes, &names));
    }
    out
}

#[test]
fn per_timestamp_is_deterministic() {
    // snapshots.md § Per-timestamp: the same timestamps reproduce byte-identical
    // dumps across runs — the determinism the fixed virtual clock guarantees.
    let steps = [
        Duration::ZERO,
        Duration::from_millis(250),
        Duration::from_millis(500),
    ];
    let a = capture_steps(&mut anim_app(), &steps);
    let b = capture_steps(&mut anim_app(), &steps);
    assert_eq!(a.len(), 3);
    assert_eq!(
        a, b,
        "per-timestamp dumps must be deterministic across runs"
    );
    // And the animation actually MOVES (guards a vacuous all-identical pass):
    // width grows 10 → 35 → 60 across t=0/250/500.
    assert!(a[0].contains("size=10,"), "t=0 width 10, got:\n{}", a[0]);
    assert!(a[1].contains("size=35,"), "t=250 width 35, got:\n{}", a[1]);
    assert!(a[2].contains("size=60,"), "t=500 width 60, got:\n{}", a[2]);
}

#[test]
fn assert_display_list_snapshot_at_keys_per_step() {
    // The public entry point: one `.snap` per step keyed `<name>@<t_ms>`, so a
    // timing regression shows in exactly the drifted frame. Opt-in: this fixture
    // enrolls BECAUSE its timing curve (the width ramp) is the behavior tested.
    let mut app = anim_app();
    let steps = [
        Duration::ZERO,
        Duration::from_millis(250),
        Duration::from_millis(500),
    ];
    assert_display_list_snapshot_at(&mut app, "width_ramp", &steps);
}
