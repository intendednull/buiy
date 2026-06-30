//! MVU substrate wall-clock bench (the L1 pricer — criterion leg). Spec §11.
//!
//! ## What it measures
//! The funnel's per-frame cost: the idle floor's scaling in MODEL-TYPE count, a single
//! message's fold, an `Emit`/message fold-storm, and the record-OFF-vs-ON delta (spec §7.1).
//! Driven HEADLESS on the shared `buiy_bench_support` MVU scenes.
//!
//! ## Posture: INFORMATIONAL, never a CI gate (DG-3)
//! `cargo bench -p buiy_core --bench mvu`. NO CI step fails on a slower number — wall-clock is
//! host- and load-dependent. The deterministic host-INDEPENDENT MVU gate is `MvuWorkCounters`
//! (the crosscut `mvu` tests); the valgrind-priced twin is `benches/mvu_iai.rs`. This bench is
//! the reviewable trend.

use std::hint::black_box;

use buiy_core::mvu::{MsgLog, RecordMode};
use criterion::{Criterion, criterion_group, criterion_main};

use buiy_bench_support::mvu_scenes::{
    CounterMsg, build_mvu_idle_app, build_mvu_single_app, enqueue_direct,
};

/// Idle floor vs model-TYPE count: an app with `N` distinct model types (1 instance each) runs
/// `N` empty-inbox drains per frame and nothing else. Proves the idle cost is `O(N_model_types)`
/// — flat in widget INSTANCE count — and small (spec §3/§11: per-type, not per-instance).
fn bench_mvu_idle(c: &mut Criterion) {
    let mut group = c.benchmark_group("mvu_idle");
    for &types in &[1usize, 4, 16] {
        group.bench_function(format!("{types}_types"), |b| {
            let mut app = build_mvu_idle_app(types);
            b.iter(|| {
                app.update(); // no messages enqueued → pure idle drain cost
                black_box(app.world().entities().len())
            });
        });
    }
    group.finish();
}

/// One message: enqueue a single `Add(1)` then drive a frame — the cost of a lone interaction
/// folding through the funnel (the common case; message rate is O(interactions/frame)).
fn bench_mvu_one_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("mvu_one_message");
    group.bench_function("add_1", |b| {
        let (mut app, e) = build_mvu_single_app(RecordMode::Off);
        b.iter(|| {
            enqueue_direct(&mut app, e, CounterMsg::Add(1));
            app.update();
            black_box(())
        });
    });
    group.finish();
}

/// Fold storm: `K` messages drained in one frame — prices a burst (e.g. a fast input replay or
/// an `Emit` chain). `K ∈ {1, 10, 100}`.
fn bench_mvu_fold_storm(c: &mut Criterion) {
    let mut group = c.benchmark_group("mvu_fold_storm");
    for &k in &[1usize, 10, 100] {
        group.bench_function(format!("{k}"), |b| {
            let (mut app, e) = build_mvu_single_app(RecordMode::Off);
            b.iter(|| {
                for _ in 0..k {
                    enqueue_direct(&mut app, e, CounterMsg::Add(1));
                }
                app.update();
                black_box(())
            });
        });
    }
    group.finish();
}

/// Record OFF vs ON: a 100-message fold storm with `RecordMode::Off` (the production default,
/// zero serialize) vs `RecordMode::Full` (RON-serialize every fold). Sizes the record tap's
/// cost — the charter's #1 perf fear (spec §7.1: default-OFF pays zero).
fn bench_mvu_record_off_vs_on(c: &mut Criterion) {
    let mut group = c.benchmark_group("mvu_record_off_vs_on");
    for (label, mode) in [("off", RecordMode::Off), ("full", RecordMode::Full)] {
        group.bench_function(label, |b| {
            let (mut app, e) = build_mvu_single_app(mode);
            b.iter(|| {
                // Bound the record-ON log across criterion's many iterations (Vec::clear keeps
                // capacity, so memory caps at ~one storm; negligible vs 100 RON serializes).
                app.world_mut().resource_mut::<MsgLog>().entries.clear();
                for _ in 0..100 {
                    enqueue_direct(&mut app, e, CounterMsg::Add(1));
                }
                app.update();
                black_box(())
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_mvu_idle,
    bench_mvu_one_message,
    bench_mvu_fold_storm,
    bench_mvu_record_off_vs_on
);
criterion_main!(benches);
