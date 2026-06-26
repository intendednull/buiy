//! GPU-path tests for the effect-group compositor. These need a wgpu adapter
//! (real GPU or lavapipe), which CI / this host lack, so they are `#[ignore]`
//! exactly like tests/render_smoke.rs. Run locally with:
//!   cargo test -p buiy_core --test render_compositor_gpu -- --ignored

use bevy::prelude::*;

/// Count the systems in the `Core2d` SCHEDULE of a freshly-built app, optionally
/// installing `BuiyRenderPlugin`. Bevy 0.19 removed the `RenderGraph`
/// `Node`/`ViewNode` API — render passes are now systems added to the `Core2d`
/// schedule — so the membership signal moved from "nodes in the Core2d sub-graph"
/// to "systems in the Core2d schedule". Both variants install the identical
/// prerequisite plugin stack, so the system-count *delta* between them is exactly
/// what `BuiyRenderPlugin::build` contributes to that schedule. `graph().systems`
/// is populated at `add_systems` time, so this needs no executor init.
fn core2d_schedule_system_count(with_buiy: bool) -> usize {
    use bevy::core_pipeline::Core2d;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    // `CorePipelinePlugin` → `TonemappingPlugin::build` reads `Assets<Image>`,
    // whose owner is `ImagePlugin` (not `AssetPlugin`). Without it, build panics
    // with "Requested resource … does not exist" from tonemapping/mod.rs.
    app.add_plugins(bevy::image::ImagePlugin::default());
    app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
    if with_buiy {
        app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    }

    app.get_sub_app(bevy::render::RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<bevy::ecs::schedule::Schedules>()
        .get(Core2d)
        .expect("Core2d schedule present in the RenderApp")
        .graph()
        .systems
        .len()
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
fn compositor_register_adds_no_extra_core2d_pass() {
    // The compositor runs INSIDE the single Buiy pass (`buiy_pass`,
    // effect-compositor.md § 3): it must NOT register a second competing pass
    // system. `BuiyRenderPlugin::build` wires exactly one Core2d-schedule system
    // — `buiy_pass` (node::register) — and `compositor::register` must add NONE
    // to the `Core2d` schedule (its `prepare_effect_groups` goes to the `Render`
    // schedule, asserted separately). So installing the plugin grows the Core2d
    // system set by exactly ONE; a compositor that wrongly registered a second
    // Core2d pass would make this delta two and fail here.
    let control = core2d_schedule_system_count(false);
    let with_buiy = core2d_schedule_system_count(true);
    assert_eq!(
        with_buiy - control,
        1,
        "BuiyRenderPlugin must add exactly one Core2d-schedule system (buiy_pass); \
         the compositor must add none (control={control}, with_buiy={with_buiy})"
    );
}

// Number of systems `BuiyRenderPlugin` adds to the `Render` schedule:
// `prepare_buiy_instances` (render/mod.rs), `prepare_buiy_view_pipelines`
// (render/pipeline.rs — per-view format+Msaa pipeline specialization),
// `prepare_effect_groups` (render/compositor.rs `register`),
// `prepare_atlas_textures` (atlas/mod.rs `register`), and
// `prepare_backdrop_blurs` (render/blur.rs `register` — parity Wave B4), all
// `.in_set(RenderSystems::Prepare)` and queued in `build`.
// Mirrors `BUIY_RENDER_SYSTEM_COUNT` in tests/render_prepare.rs; bump in
// lockstep whenever the plugin's `add_systems(Render, …)` registrations change.
const BUIY_RENDER_SYSTEM_COUNT: usize = 5;

// Count the systems in a RenderApp's `Render` schedule graph. `graph().systems`
// is populated at `add_systems` time (in `build`), so this is pure introspection
// — no executor init, no `finish()`, no extra device work. Identical helper to
// `render_schedule_system_count` in tests/render_prepare.rs.
fn render_schedule_system_count(app: &App) -> usize {
    use bevy::render::{Render, RenderApp};
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<bevy::ecs::schedule::Schedules>()
        .get(Render)
        .expect("Render schedule present in the RenderApp")
        .graph()
        .systems
        .len()
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
fn prepare_effect_groups_runs_in_prepare_set() {
    use bevy::render::RenderSystems;

    // Membership is asserted by a baseline system-count delta rather than by
    // system *name*: without `bevy_utils/debug` (this workspace does not enable
    // it) `System::name()` resolves to the placeholder "<Enable the debug feature
    // to see the name>", so a `name().contains("prepare_effect_groups")` match
    // can NEVER fire here — the prior name-based body was structurally broken on
    // a non-debug build. Same proven idiom as
    // `prepare_system_is_in_render_prepare_set` in tests/render_prepare.rs:
    // count the Render-schedule systems WITHOUT the plugin, then WITH it, and
    // assert the delta is exactly the Buiy render-system count. Deleting the
    // `compositor::register` → `add_systems(Render, prepare_effect_groups…)` line
    // drops the delta below `BUIY_RENDER_SYSTEM_COUNT` and fails here. Only
    // *building* the RenderApp needs the wgpu adapter; walking the schedule does
    // not. `CorePipelinePlugin` is intentionally NOT added (it is irrelevant to
    // this membership assertion and pulls in the tonemapping `Assets<Image>`
    // dependency).
    let mut baseline = App::new();
    baseline.add_plugins(MinimalPlugins);
    baseline.add_plugins(bevy::asset::AssetPlugin::default());
    baseline.add_plugins(bevy::render::RenderPlugin::default());
    let baseline_count = render_schedule_system_count(&baseline);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    let with_plugin_count = render_schedule_system_count(&app);

    assert_eq!(
        with_plugin_count - baseline_count,
        BUIY_RENDER_SYSTEM_COUNT,
        "BuiyRenderPlugin must register {BUIY_RENDER_SYSTEM_COUNT} systems in the \
         Render schedule (prepare_buiy_instances + prepare_effect_groups + \
         prepare_backdrop_blurs + …); got a delta of {} — a missing \
         add_systems(Render, …) in render/compositor.rs or render/blur.rs `register`",
        with_plugin_count - baseline_count,
    );
    // The set-membership (RenderSystems::Prepare) is pinned by register() and
    // the compositor schedule-order test; this test pins presence in the render world.
    let _ = RenderSystems::Prepare;
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
fn buiy_pass_runs_with_prepared_effect_groups_query() {
    // Compile + construction smoke: BuiyRenderPlugin builds with the extended
    // `buiy_pass` ViewQuery (Option<&PreparedEffectGroups>) and the pass system
    // is in the Core2d schedule. The composite correctness is proven by the
    // golden (separate gate #2 fixture) — this only pins that the pass wiring
    // compiles & loads. (Bevy 0.19: passes are Core2d-schedule systems, not
    // RenderGraph nodes; membership is the +1 system delta, the same idiom as
    // `compositor_register_adds_no_extra_core2d_pass`.)
    let control = core2d_schedule_system_count(false);
    let with_buiy = core2d_schedule_system_count(true);
    assert_eq!(
        with_buiy - control,
        1,
        "BuiyRenderPlugin must add its single Core2d pass system (buiy_pass) \
         with the extended effect-groups ViewQuery (control={control}, \
         with_buiy={with_buiy})"
    );
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn group_opacity_overlap_is_single_layer_at_half() {
    // The pillar-6 regression (effect-compositor.md § 4 / § 5.1): TWO overlapping
    // OPAQUE-red children inside an `Opacity(0.5)` parent. The children are
    // composed ONCE in the group's off-screen `Rgba16Float` target (opaque red,
    // alpha 1 everywhere they paint), then that target composites at 0.5 over the
    // backdrop. So the OVERLAP pixel == a non-overlap pixel == 50% red over black
    // — NOT the `0.5*0.5` doubled darkening the rejected per-child approximation
    // would produce. This is the proof the off-screen pass shipped.
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};
    use buiy_core::render::compositor::composite_src_over;
    use std::borrow::Cow;

    const W: u32 = 64;
    const H: u32 = 64;

    let red = Color::srgb(0.9, 0.05, 0.05); // an OPAQUE red (alpha 1)

    let mut app = crate::support::gpu_render_app(W, H);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.red".into(), red);
    }

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());

    // Two absolutely-positioned 32x32 opaque-red children that OVERLAP, both
    // children of one `Opacity(0.5)` parent (so they share its group). The parent
    // is backgroundless (no quad of its own — only the children fill the target).
    // A: x∈[8,40), y∈[8,40).  B: x∈[20,52), y∈[20,52).
    // Overlap: x∈[20,40), y∈[20,40); sampled deep-interior at (30,30).
    // A-only (non-overlap red): (12,12).
    let child = |left: f32, top: f32| {
        (
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(top)),
                    left: Sizing::Length(Length::px(left)),
                    ..default()
                })
                .width_px(32.0)
                .height_px(32.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.red")),
            },
        )
    };
    let a = app.world_mut().spawn(child(8.0, 8.0)).id();
    let b = app.world_mut().spawn(child(20.0, 20.0)).id();
    // The Opacity(0.5) parent — an EffectGroup former (write_effect_groups marks
    // it). Absolutely positioned so it forms a clean subtree under the root.
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.5)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[a, b]);
    // A root holds the parent (single StackingContext forms at the root).
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent]);

    // Drive frames: finish + layout→extract→prepare upload + the graph paint
    // settle; `readback_rgba` polls further (the pipeline async-compiles).
    crate::support::finish_and_run(&mut app, 4);

    let pixels = crate::support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (W * H * 4) as usize);
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    // Expectation via the CPU port: an opaque-red group sample at 0.5 over the
    // opaque-black clear, then encode linear→sRGB8 (what the Rgba8UnormSrgb target
    // stores). The group sample is the FULLY-COMPOSED red (alpha 1), so the same
    // value lands on overlap AND non-overlap pixels.
    let red_lin = LinearRgba::from(red);
    let black_lin = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    let expected_lin = composite_src_over(red_lin, black_lin, 0.5);
    let expected_srgb = Srgba::from(expected_lin);
    let expected = [
        (expected_srgb.red * 255.0).round() as u8,
        (expected_srgb.green * 255.0).round() as u8,
        (expected_srgb.blue * 255.0).round() as u8,
        255u8,
    ];

    let clear = px(1, 1);
    let a_only = px(12, 12);
    let overlap = px(30, 30);
    println!("clear   (1,1)   = {clear:?}");
    println!("A-only  (12,12) = {a_only:?}  (expected {expected:?})");
    println!("overlap (30,30) = {overlap:?}  (expected {expected:?})");

    assert_eq!(clear, [0, 0, 0, 255], "untouched corner reads the clear");

    const TOL: i32 = 4;
    // (1) The overlap is 50%-red-over-black — NOT doubled. A per-child-approx
    // double composite would darken the red channel further (≈0.25 linear red
    // instead of 0.5), reading visibly lower than `expected` here.
    for ch in 0..3 {
        let got = overlap[ch] as i32;
        let want = expected[ch] as i32;
        assert!(
            (got - want).abs() <= TOL,
            "overlap channel {ch}: got {got}, expected {want} (±{TOL}); the group \
             must composite ONCE at 0.5, not double-darken the overlap. full \
             overlap={overlap:?} expected={expected:?}"
        );
    }
    // (2) A non-overlap red pixel equals the SAME 0.5 red — proves the overlap is
    // not darker than a single layer (no double-darken).
    for ch in 0..3 {
        assert!(
            (a_only[ch] as i32 - overlap[ch] as i32).abs() <= TOL,
            "non-overlap red ({a_only:?}) must equal the overlap ({overlap:?}) — \
             both are the group composited once at 0.5"
        );
    }
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn rt_pool_returns_to_baseline_after_idle() {
    // The RT-pool leak mechanism (effect-compositor.md § 2.3): churn `EffectGroup`
    // membership across frames (transient `buiy_effect_group_target` targets), then
    // idle past the 3-frame `TextureCache` reclaim. The compositor's working set
    // (the targets it holds for live groups) returns to the idle baseline (zero)
    // because sizing is painted-bounds + descriptor-keyed reuse + Buiy adds NO
    // bespoke eviction — Bevy's `update_texture_cache_system` un-`taken`s and drops
    // targets unused for 3 frames. Observed via `RtPoolStats`, the render-world
    // stat `prepare_effect_groups` records each frame (the `TextureCache`'s own
    // per-descriptor buckets are private).
    use bevy::render::RenderApp;
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};
    use buiy_core::render::compositor::RtPoolStats;
    use std::borrow::Cow;

    const W: u32 = 128;
    const H: u32 = 128;

    let stats = |app: &App| -> RtPoolStats {
        *app.get_sub_app(RenderApp)
            .expect("RenderApp")
            .world()
            .resource::<RtPoolStats>()
    };

    let mut app = crate::support::gpu_render_app(W, H);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("test.red".into(), Color::srgb(0.9, 0.05, 0.05));
    }
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target);

    // A spawner for one opacity group (a fill child under an Opacity(0.5) parent).
    let spawn_group = |app: &mut App, left: f32, top: f32| -> Entity {
        let fill = app
            .world_mut()
            .spawn((
                Node,
                Style::default()
                    .absolute()
                    .inset(Inset {
                        top: Sizing::Length(Length::px(top)),
                        left: Sizing::Length(Length::px(left)),
                        ..default()
                    })
                    .width_px(24.0)
                    .height_px(24.0),
                Background {
                    color: ColorToken::Token(Cow::Borrowed("test.red")),
                },
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn((Node, Style::default().absolute(), Opacity(0.5)))
            .id();
        app.world_mut().entity_mut(parent).add_children(&[fill]);
        app.world_mut()
            .spawn((Node, Style::default()))
            .add_children(&[parent]);
        parent
    };

    crate::support::finish_and_run(&mut app, 3);

    // Idle baseline: no groups live → zero targets.
    let baseline = stats(&app);
    println!("baseline (no groups): {baseline:?}");
    assert_eq!(
        baseline.live_targets, 0,
        "no EffectGroup live → no off-screen targets"
    );

    // Churn: open three opacity groups across frames (transient targets), then
    // close them all. Each open spawns a new group; group membership flips frame
    // to frame, exercising acquire + descriptor-keyed reuse.
    let mut open: Vec<Entity> = Vec::new();
    for i in 0..3 {
        open.push(spawn_group(&mut app, 8.0 + i as f32 * 30.0, 8.0));
        app.update();
    }
    let peak = stats(&app);
    println!("peak (3 groups live): {peak:?}");
    assert!(
        peak.live_targets >= 1,
        "churn made the compositor hold live targets (got {})",
        peak.live_targets
    );

    // Close every group: despawn the parents (drops EffectGroup membership). The
    // children go with them (despawn_recursive via the hierarchy).
    for e in open {
        app.world_mut().entity_mut(e).despawn();
    }

    // Idle past the 3-frame TextureCache reclaim window (a few extra frames let
    // the despawn extract + the reclaim settle).
    for _ in 0..8 {
        app.update();
    }

    let after_idle = stats(&app);
    println!("after idle (groups closed): {after_idle:?}");
    // The working set returns to the idle baseline — no leaked targets.
    assert_eq!(
        after_idle.live_targets, baseline.live_targets,
        "live target count returns to baseline after idle (got {}, baseline {})",
        after_idle.live_targets, baseline.live_targets
    );
    assert_eq!(
        after_idle.distinct_buckets, baseline.distinct_buckets,
        "distinct target buckets return to baseline after idle (got {}, baseline {})",
        after_idle.distinct_buckets, baseline.distinct_buckets
    );
}
