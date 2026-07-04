//! PROTOTYPE — MT-ceiling measurement benchmark (throwaway; DO NOT MERGE).
//!
//! Measures whether Buiy's serial / main-thread-pinned per-frame work caps the
//! throughput of a multi-threaded app whose *non-UI* work is embarrassingly
//! parallel. Headless `app.update()` loop timing (no GPU/vsync) isolates the CPU
//! scheduler cost — exactly the surface an Amdahl's-law ceiling lives on.
//!
//! Every knob is an env var so a driver script can sweep the matrix across
//! separate processes (required: `ComputeTaskPool` is a process-global singleton,
//! so thread count can only be set once per process).
//!
//! | env | meaning | default |
//! |-----|---------|---------|
//! | `BUIY_MT_THREADS`  | compute-pool thread count | 8 |
//! | `BUIY_MT_EXEC`     | `mt` (multi-threaded executor) or `st` (single) | mt |
//! | `BUIY_MT_BUIY`     | `1` = add Buiy plugins + a UI scene, `0` = bare app | 1 |
//! | `BUIY_MT_UI`       | `flat_small`/`flat_large`/`text_large` (when BUIY=1) | flat_large |
//! | `BUIY_MT_PAR_ENTITIES` | # entities in the parallel user workload | 4000 |
//! | `BUIY_MT_PAR_COST` | per-entity busy-loop iterations (0 = no user work) | 4000 |
//! | `BUIY_MT_DIRTY`    | `1` = mutate the UI every frame (force relayout/reshape) | 0 |
//! | `BUIY_MT_WARMUP`   | warmup frames (not recorded) | 64 |
//! | `BUIY_MT_FRAMES`   | measured frames | 400 |
//! | `BUIY_MT_LABEL`    | free-text label echoed into the CSV row | "" |
//! | `BUIY_MT_HEADER`   | `1` = print the CSV header line then the row | 0 |

use std::time::Instant;

use bevy::ecs::schedule::SingleThreadedExecutor;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPoolBuilder};

use buiy_bench_support::{PipelineHarness, build_flat_scene, build_large_scene};
use buiy_core::layout::{BoxModel, Length, Sizing};
use buiy_core::text::Text;

/// One entity of the embarrassingly-parallel user workload.
#[derive(Component)]
struct ParWork(f64);

#[derive(Resource, Clone, Copy)]
struct ParConfig {
    cost: u32,
}

/// The user's parallel workload: a `par_iter_mut` over N entities, each running a
/// fixed busy computation. Distinct data (its own component + resource) and no
/// ordering edge to `BuiySet`, so the MT executor is free to overlap it with
/// Buiy's systems — which is exactly what we want to test.
fn par_system(cfg: Res<ParConfig>, mut q: Query<&mut ParWork>) {
    let cost = cfg.cost;
    if cost == 0 {
        return;
    }
    q.par_iter_mut().for_each(|mut w| {
        let mut x = std::hint::black_box(w.0);
        for i in 0..cost {
            // Non-elidable transcendental churn; genuine CPU, scales cleanly.
            x = (x + i as f64).sqrt().sin().mul_add(1.000_001, 1.0);
        }
        w.0 = std::hint::black_box(x);
    });
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
fn env_num<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn force_single_threaded(app: &mut App) {
    // Swap each main-app schedule's executor to the single-threaded one. Bevy
    // 0.19 has no `set_executor_kind`; `set_executor` takes an executor value.
    let set_st = |s: &mut Schedule| {
        s.set_executor(SingleThreadedExecutor::default());
    };
    app.edit_schedule(First, set_st);
    app.edit_schedule(PreUpdate, set_st);
    app.edit_schedule(Update, set_st);
    app.edit_schedule(PostUpdate, set_st);
    app.edit_schedule(Last, set_st);
}

fn main() {
    let threads: usize = env_num("BUIY_MT_THREADS", 8usize).max(1);
    let exec = env_str("BUIY_MT_EXEC", "mt");
    let with_buiy: u32 = env_num("BUIY_MT_BUIY", 1u32);
    let ui = env_str("BUIY_MT_UI", "flat_large");
    let par_entities: usize = env_num("BUIY_MT_PAR_ENTITIES", 4000usize);
    let par_cost: u32 = env_num("BUIY_MT_PAR_COST", 4000u32);
    let dirty: u32 = env_num("BUIY_MT_DIRTY", 0u32);
    let warmup: usize = env_num("BUIY_MT_WARMUP", 64usize);
    let frames: usize = env_num("BUIY_MT_FRAMES", 400usize);
    let label = env_str("BUIY_MT_LABEL", "");
    let header: u32 = env_num("BUIY_MT_HEADER", 0u32);

    // Footgun guard: without `mt-exec` the MT executor is not compiled in, so
    // `BUIY_MT_EXEC=mt` silently runs single-threaded and every "mt" datapoint is
    // wrong. Fail loudly rather than emit misleading numbers.
    if !cfg!(feature = "mt-exec") {
        eprintln!(
            "FATAL: mt_ceiling built WITHOUT --features mt-exec — Bevy's MT executor \
             is absent, so all runs are single-threaded and 'mt' data is invalid. \
             Rebuild: cargo build -p mt_ceiling --release --features mt-exec"
        );
        std::process::exit(2);
    }

    // Pin the compute pool size BEFORE any plugin builds it (get_or_init wins).
    ComputeTaskPool::get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(threads)
            .thread_name("mt-ceiling-compute".to_string())
            .build()
    });
    let pool_threads = ComputeTaskPool::get().thread_num();

    // Build the app + the frame-stepping closure.
    let mut samples: Vec<u128> = Vec::with_capacity(frames);

    if with_buiy == 0 {
        // Bare Bevy app: pure parallel workload, no Buiy. The "app Bevy does well
        // with" baseline.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(ParConfig { cost: par_cost });
        app.add_systems(Update, par_system);
        for _ in 0..par_entities {
            app.world_mut().spawn(ParWork(0.0));
        }
        if exec == "st" {
            force_single_threaded(&mut app);
        }
        for i in 0..(warmup + frames) {
            let t0 = Instant::now();
            app.update();
            let dt = t0.elapsed().as_nanos();
            if i >= warmup {
                samples.push(dt);
            }
        }
    } else {
        // Buiy present: full per-frame pipeline + a UI scene + the parallel work.
        let mut h: PipelineHarness = match ui.as_str() {
            "flat_small" => build_flat_scene(100),
            "flat_large" => build_flat_scene(2000),
            "text_small" => build_large_scene(30),
            "text_large" => build_large_scene(120),
            "text_huge" => build_large_scene(480),
            other => {
                eprintln!("unknown BUIY_MT_UI={other}; using flat_large");
                build_flat_scene(2000)
            }
        };
        h.app.insert_resource(ParConfig { cost: par_cost });
        h.app.add_systems(Update, par_system);
        for _ in 0..par_entities {
            h.app.world_mut().spawn(ParWork(0.0));
        }
        if exec == "st" {
            force_single_threaded(&mut h.app);
        }

        // Pick a victim node for the dirty knob: prefer a text-bearing node so the
        // text scene exercises per-frame reshape (the expensive dynamic case).
        let victim = pick_victim(&mut h.app);

        // `BUIY_MT_EXTRACT=0` times ONLY the main-world `app.update()` and skips
        // the extract step. Real windowed apps pipeline extract into the render
        // sub-app (it overlaps the *next* frame's main work), so update-only is
        // the more faithful "main-thread serial cost" number; total (=1) upper-
        // bounds it by counting extract synchronously.
        let with_extract = env_num::<u32>("BUIY_MT_EXTRACT", 1) == 1;
        let mut extract_samples: Vec<u128> = Vec::with_capacity(frames);
        for i in 0..(warmup + frames) {
            if dirty == 1 {
                dirty_scene(&mut h.app, victim, i);
            }
            let t0 = Instant::now();
            h.app.update();
            let upd = t0.elapsed().as_nanos();
            let mut ext = 0u128;
            if with_extract {
                let t1 = Instant::now();
                h.extract_only();
                ext = t1.elapsed().as_nanos();
            }
            if i >= warmup {
                samples.push(upd + ext);
                extract_samples.push(ext);
            }
        }
        if env_num::<u32>("BUIY_MT_COUNTERS", 0) == 1 {
            extract_samples.sort_unstable();
            let en = extract_samples.len().max(1);
            eprintln!(
                "SPLIT,{label},ui={ui},dirty={dirty}: extract_p50_us={:.1}",
                extract_samples[en / 2] as f64 / 1000.0
            );
        }

        // Per-frame work-counter delta over exactly one more frame — attributes
        // the serial cost to measure/reshape/relayout work units.
        if env_num::<u32>("BUIY_MT_COUNTERS", 0) == 1 {
            let snap = |app: &App| {
                (
                    app.world()
                        .get_resource::<buiy_core::text::TextMeasureCallCount>()
                        .map(|c| c.0)
                        .unwrap_or(0),
                    app.world()
                        .get_resource::<buiy_core::text::TextCommitReshapeCount>()
                        .map(|c| c.0)
                        .unwrap_or(0),
                    app.world()
                        .get_resource::<buiy_core::text::TextSyncAppliedCount>()
                        .map(|c| c.0)
                        .unwrap_or(0),
                    app.world()
                        .get_resource::<buiy_core::layout::LayoutTaffyComputeCount>()
                        .map(|c| c.0 as usize)
                        .unwrap_or(0),
                    app.world()
                        .get_resource::<buiy_core::layout::LayoutPostTaffyRunCount>()
                        .map(|c| c.0)
                        .unwrap_or(0),
                )
            };
            // Run one controlled frame and snapshot ABSOLUTE values right after.
            // The per-frame-reset counters (measure_calls, taffy_compute) then
            // hold exactly this frame's count; accumulating ones hold a running
            // total (printed for reference).
            if dirty == 1 {
                dirty_scene(&mut h.app, victim, warmup + frames);
            }
            h.frame();
            let b = snap(&h.app);
            eprintln!(
                "COUNTERS,{label},ui={ui},dirty={dirty}: measure_calls={} reshapes(tot)={} sync_applied(tot)={} taffy_compute={} post_taffy_runs={}",
                b.0, b.1, b.2, b.3, b.4,
            );
        }
    }

    samples.sort_unstable();
    let n = samples.len().max(1);
    let pct = |p: f64| samples[((n as f64 * p) as usize).min(n - 1)] as f64 / 1000.0;
    let min_us = samples.first().copied().unwrap_or(0) as f64 / 1000.0;
    let mean_us = samples.iter().sum::<u128>() as f64 / n as f64 / 1000.0;

    if header == 1 {
        println!(
            "label,buiy,ui,exec,threads_req,pool_threads,par_entities,par_cost,dirty,frames,min_us,p50_us,p90_us,p99_us,mean_us"
        );
    }
    println!(
        "{label},{with_buiy},{ui},{exec},{threads},{pool_threads},{par_entities},{par_cost},{dirty},{n},{min_us:.1},{:.1},{:.1},{:.1},{mean_us:.1}",
        pct(0.50),
        pct(0.90),
        pct(0.99),
    );
}

fn pick_victim(app: &mut App) -> Option<Entity> {
    // A text-bearing node if any (text scene), else any Node.
    let world = app.world_mut();
    if let Some(e) = world
        .query_filtered::<Entity, (With<Text>,)>()
        .iter(world)
        .next()
    {
        return Some(e);
    }
    world
        .query_filtered::<Entity, With<buiy_core::Node>>()
        .iter(world)
        .next()
}

/// Force per-frame layout dirtiness (and text reshape when the victim bears
/// text). Mutating the victim's `BoxModel` width in place marks the layout
/// component changed (→ relayout); setting `Text` forces cosmic-text to
/// reshape. This models an *active* UI updating every frame.
///
/// Deliberately NOT a whole-`Style`-bundle re-insert (the pre-Stage-C form):
/// `Style` is a Bundle that includes `Stacking`, and a bundle insert marks
/// EVERY member `Changed` regardless of value — which trips the glyph/node
/// tiers' conservative structural probes (`Changed<Stacking>` ⇒ paint order
/// may have moved ⇒ Full) every frame. A real active UI mutates the fields
/// it animates (width here, text below), not its z-index; the in-place write
/// is the faithful model. The pre-Stage-C DIRTY baselines are unaffected by
/// this fix: before Patch execution existed, ANY dirty frame took the same
/// wholesale walk regardless of which components ticked.
fn dirty_scene(app: &mut App, victim: Option<Entity>, frame: usize) {
    let Some(v) = victim else { return };
    // `BUIY_MT_DIRTY_KIND` = full (default) | style | text — decompose the
    // per-frame cost into "relayout" (style) vs "reshape" (text).
    let kind = env_str("BUIY_MT_DIRTY_KIND", "full");
    let w = 40.0 + (frame % 7) as f32;
    let world = app.world_mut();
    let has_text = world.get::<Text>(v).is_some();
    if (kind == "full" || kind == "style")
        && let Some(mut bm) = world.get_mut::<BoxModel>(v)
    {
        bm.width = Sizing::Length(Length::Px(w));
        bm.height = Sizing::Length(Length::Px(20.0));
    }
    if (kind == "full" || kind == "text") && has_text {
        world.entity_mut(v).insert(Text(format!(
            "Section {frame}: a representative heading that reshapes each frame"
        )));
    }
}
