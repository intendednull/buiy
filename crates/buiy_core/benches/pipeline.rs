//! Shape→layout→extract performance bench (audit finding #40, decision gate
//! DG-3). The workspace's wall-clock performance signal.
//!
//! ## What it measures, and what it is NOT
//!
//! It times the per-frame hot path
//!
//!   TextSync → Taffy measure (cosmic-text SHAPING) → TextCommit → write layout
//!     → `extract_buiy_glyphs` / `extract_buiy_nodes`
//!
//! over LARGE scenes (text-heavy, flat-layout #3, and node-extract #2), driven
//! HEADLESS (no wgpu adapter, no `RenderApp`) on the shared `buiy_bench_support`
//! harness. An O(n²) regression or a per-frame allocation blow-up shows up as a
//! number that moves.
//!
//! ## Posture: INFORMATIONAL, never a CI gate (DG-3)
//!
//! `cargo bench -p buiy_core --bench pipeline`. NO CI step fails the build on a
//! slower number — wall-clock is host- and load-dependent. The deterministic,
//! host-INDEPENDENT gates (work-unit counters, dhat, iai-callgrind) are what gate
//! CI; this wall-clock bench is the reviewable trend (diff with `-- --baseline`).
//!
//! The adapterless harness + scenes live in the shared `buiy_bench_support`
//! dev-crate so this bench, the dhat alloc-budget test, and the iai-callgrind
//! bench all drive ONE harness (perf-final Phase 0a).

use std::hint::black_box;

use bevy::prelude::DetectChangesMut;
use buiy_core::render::components::Background;
use criterion::{Criterion, criterion_group, criterion_main};

use buiy_bench_support::{build_flat_bg_scene, build_flat_scene, build_large_scene};

/// Bench the COLD first-pass: spawn the large scene then drive frames to
/// quiescence from scratch — the full shape→layout→extract cost of bringing a
/// fresh scene up. Re-built per iteration so each sample is a genuine cold pass.
fn bench_cold_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape_layout_extract");
    group.sample_size(20);

    for &paragraphs in &[64usize, 256] {
        group.bench_function(format!("cold/{paragraphs}_paragraphs"), |b| {
            b.iter(|| {
                let mut h = build_large_scene(paragraphs);
                for _ in 0..4 {
                    h.frame();
                }
                black_box(h.glyph_count())
            });
        });
    }
    group.finish();
}

/// Bench the per-frame STEADY hot path: settle a large scene ONCE, then time a
/// single full pipeline frame on the already-shaped scene. Isolates the recurring
/// per-frame cost from the one-time cold build.
fn bench_steady_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape_layout_extract");

    for &paragraphs in &[64usize, 256] {
        group.bench_function(format!("steady/{paragraphs}_paragraphs"), |b| {
            let mut h = build_large_scene(paragraphs);
            for _ in 0..6 {
                h.frame();
            }
            b.iter(|| {
                h.frame();
                black_box(h.glyph_count())
            });
        });
    }
    group.finish();
}

/// Bench the per-frame STEADY layout cost on a large flat (text-free) scene:
/// settle once, then time a single `app.update()` (the full BuiySet chain) on the
/// already-laid-out tree. An ungated full-tree pass (#3) shows up here as a
/// per-frame cost that grows with node count even though nothing changed.
fn bench_flat_steady(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_flat");
    for &nodes in &[1000usize, 5000] {
        group.bench_function(format!("steady/{nodes}_nodes"), |b| {
            let mut h = build_flat_scene(nodes);
            for _ in 0..6 {
                h.app.update();
            }
            b.iter(|| {
                h.app.update();
                black_box(h.app.world().entities().len())
            });
        });
    }
    group.finish();
}

/// Bench audit #2 (all-or-nothing extract). `extract_buiy_nodes` is gated: a
/// CLEAN frame early-returns (the O(0) damage gate); ANY changed entity rebuilds
/// the FULL `by_entity` record set + re-clones every ~480 B `ExtractedNode`. So
/// `steady_gated` (no change → gate skips) vs `one_change` (mutate ONE node →
/// full N-node rebuild) sizes the CPU cost a single hover/caret-blink/scroll pays
/// per interactive frame. Both run `app.update()`, so the DELTA isolates the
/// extract rebuild. Adapterless: the prepare GPU re-upload is NOT measured here.
fn bench_node_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_extract");
    for &nodes in &[1000usize, 5000] {
        group.bench_function(format!("steady_gated/{nodes}"), |b| {
            let (mut h, _victim) = build_flat_bg_scene(nodes);
            for _ in 0..6 {
                h.frame();
            }
            b.iter(|| {
                h.app.update();
                h.extract_only();
                black_box(h.app.world().entities().len())
            });
        });
        group.bench_function(format!("one_change/{nodes}"), |b| {
            let (mut h, victim) = build_flat_bg_scene(nodes);
            for _ in 0..6 {
                h.frame();
            }
            b.iter(|| {
                h.app.update();
                // One interactive change: mark a single node's paint dirty so the
                // damage gate trips and re-extracts the whole scene.
                if let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
                    bg.set_changed();
                }
                h.extract_only();
                black_box(h.app.world().entities().len())
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cold_pipeline,
    bench_steady_pipeline,
    bench_flat_steady,
    bench_node_extract
);
criterion_main!(benches);
