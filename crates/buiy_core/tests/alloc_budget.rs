//! Allocation-count gate (perf-final P0c).
//!
//! An ISOLATED test binary: `#[global_allocator] dhat::Alloc` measures EVERY
//! allocation in this binary, so it must be its own target (never a module under
//! another test binary, never the library). In dhat `testing()` mode the heap
//! stats are deterministic, so "one steady frame allocates ≤ N blocks" is a
//! stable, CROSS-PLATFORM pass/fail gate (no Valgrind, unlike the iai gate).
//!
//! It gates the per-frame allocation surface the work-unit counters structurally
//! cannot see (the #16 scratch fan) and the "idle frame is free" contract (a
//! regression re-introducing a per-frame rebuild/alloc reddens here).
//!
//! Bands are committed baselines measured on the settled scene; re-bless
//! deliberately (in its own commit) on a std/bevy bump, never silently widen.
//!
//! ## MT-safety scoping (compiled out under `multi_threaded`)
//!
//! This gate is a SINGLE-THREADED deterministic measurement (dhat `testing()`
//! mode) of Buiy's per-frame allocation surface — a contract identical under
//! either executor, since Buiy's own allocations don't change with the executor.
//! Under the MT executor the per-frame task-dispatch allocations across the
//! schedule set add irreducible noise (≈155 idle blocks unpinned; ≈69 even with
//! the harness schedules pinned — vs the 33-block single-threaded baseline),
//! which a tight per-frame budget cannot distinguish from a real regression.
//! Rather than widen the budget (which would hide regressions under executor
//! noise) or chase every executor allocation out of the harness (leaky — the
//! `Main`/finish-added schedules aren't all reachable from the harness ctor), the
//! gate is scoped to single-threaded: it runs on EVERY CI run via the default
//! `test` job, and the `multi_threaded` CI lane — which exists to prove
//! CORRECTNESS, not allocation budgets — skips this binary. See
//! docs/specs/2026-06-30-mt-safety-design.md (D4).
#![cfg(not(feature = "multi_threaded"))]

use buiy_bench_support::build_flat_bg_scene;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Allocations during ONE `frame()` after settling `nodes` painting nodes, for
/// `idle` (no change → damage gate skips) vs a one-entity-change rebuild.
fn frame_alloc_blocks(nodes: usize, mutate: bool) -> u64 {
    use bevy::prelude::DetectChangesMut;
    use buiy_core::render::components::Background;

    let (mut h, victim) = build_flat_bg_scene(nodes);
    for _ in 0..8 {
        h.frame(); // settle OUTSIDE the measurement (cold allocs happen here)
    }
    let before = dhat::HeapStats::get();
    if mutate && let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
        bg.set_changed();
    }
    h.frame();
    let after = dhat::HeapStats::get();
    after.total_blocks - before.total_blocks
}

#[test]
fn idle_and_rebuild_frame_allocations_within_budget() {
    let _profiler = dhat::Profiler::builder().testing().build();

    let idle = frame_alloc_blocks(1000, false);
    let rebuild = frame_alloc_blocks(1000, true);
    eprintln!("ALLOC-BUDGET: idle={idle} blocks, one-change rebuild={rebuild} blocks (1000 nodes)");

    // Committed bands. Measured baseline (1000 nodes, this toolchain): idle = 33
    // blocks, one-change rebuild = 64 blocks. dhat `testing()` mode is
    // deterministic, so these are stable; the bands carry ~1.5–2× headroom for
    // minor toolchain/bevy drift. A real regression is ORDER-OF-MAGNITUDE: a
    // per-frame rebuild on idle would push idle to rebuild-level (~64+); a
    // per-node alloc fan (the #16 fear) would push rebuild into the thousands.
    // NOTE: the rebuild's per-frame allocation COUNT is low — #16's true cost is
    // bytes/cache-pressure (iai EstimatedCycles), not alloc count. Re-bless
    // deliberately on a std/bevy bump (its own commit), never silently widen.
    const IDLE_BUDGET: u64 = 50;
    const REBUILD_BUDGET: u64 = 120;
    assert!(
        idle <= IDLE_BUDGET,
        "idle frame allocated {idle} blocks (budget {IDLE_BUDGET}) — a per-frame \
         allocation regression? the damage gate should make an idle frame ~free"
    );
    assert!(
        rebuild <= REBUILD_BUDGET,
        "one-change rebuild allocated {rebuild} blocks (budget {REBUILD_BUDGET}) — \
         the #16 per-frame scratch fan grew"
    );
}
