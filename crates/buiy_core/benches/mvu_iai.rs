//! MVU substrate instruction-count pricer (the L1 gate — iai-callgrind leg). Spec §11.
//!
//! ============================================================================================
//! REQUIRES valgrind. If valgrind is not installed on the dev host this bench cannot RUN
//! locally — it stays `cargo check`-green as a compile target. Run it on a host with valgrind:
//! `pacman -S valgrind` (Arch/Manjaro) or in CI, then
//! `cargo bench -p buiy_core --bench mvu_iai`. It also needs the runner binary:
//! `cargo install --version <matching> iai-callgrind-runner`.
//! ============================================================================================
//!
//! ## Why this is the load-bearing L1 gate
//! Instruction counts are HOST-INDEPENDENT — the weak-machine pricer the perf design mandates
//! (spec §11). The criterion twin
//! (`benches/mvu.rs`) is the wall-clock trend; this is the deterministic CI-gateable number.
//! Same cases as the criterion bench: idle floor / one message / fold storm / record off-vs-on.

use std::hint::black_box;

use bevy::prelude::*;
use buiy_core::mvu::RecordMode;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};

use buiy_bench_support::mvu_scenes::{
    BlinkMsg, CounterMsg, build_blink_app, build_mvu_idle_app, build_mvu_single_app,
    enqueue_blink_direct, enqueue_direct,
};
use std::time::Duration;

// `setup` is a FUNCTION PATH (iai-callgrind rejects closures); the `args = (..)` are passed to
// it. Setup builds the scene OUTSIDE the measured region; the bench-fn body is what callgrind
// prices. These small adapters give each case a no-/one-arg setup fn returning the bench input.

fn setup_single() -> (App, Entity) {
    build_mvu_single_app(RecordMode::Off)
}
fn setup_storm(k: usize) -> (App, Entity, usize) {
    let (app, e) = build_mvu_single_app(RecordMode::Off);
    (app, e, k)
}

// Idle floor vs model-TYPE count: prices `N` empty-inbox drains per frame.
// (Plain `//` comments — `#[library_benchmark]` rejects `#[doc]`/`///` on the bench fn.)
#[library_benchmark]
#[bench::types_1(args = (1usize), setup = build_mvu_idle_app)]
#[bench::types_4(args = (4usize), setup = build_mvu_idle_app)]
#[bench::types_16(args = (16usize), setup = build_mvu_idle_app)]
fn mvu_idle(mut app: App) -> u32 {
    app.update();
    black_box(app.world().entities().len())
}

// One message folded through the funnel.
#[library_benchmark]
#[bench::add_1(setup = setup_single)]
fn mvu_one_message(input: (App, Entity)) {
    let (mut app, e) = input;
    enqueue_direct(&mut app, e, CounterMsg::Add(1));
    app.update();
    black_box(());
}

// Fold storm: `K` messages drained in one frame.
#[library_benchmark]
#[bench::storm_1(args = (1usize), setup = setup_storm)]
#[bench::storm_10(args = (10usize), setup = setup_storm)]
#[bench::storm_100(args = (100usize), setup = setup_storm)]
fn mvu_fold_storm(input: (App, Entity, usize)) {
    let (mut app, e, k) = input;
    for _ in 0..k {
        enqueue_direct(&mut app, e, CounterMsg::Add(1));
    }
    app.update();
    black_box(());
}

// Record OFF (production default, zero serialize) vs FULL (RON every fold) — a 100-msg storm.
#[library_benchmark]
#[bench::off(args = (RecordMode::Off), setup = build_mvu_single_app)]
#[bench::full(args = (RecordMode::Full), setup = build_mvu_single_app)]
fn mvu_record_off_vs_on(input: (App, Entity)) {
    let (mut app, e) = input;
    for _ in 0..100 {
        enqueue_direct(&mut app, e, CounterMsg::Add(1));
    }
    app.update();
    black_box(());
}

// Spec §11 — price ONE steady blink tick's funnel FIXED cost. The two cases share the
// SAME settled blink app and differ only in whether a per-frame-changing `Tick(now)` is
// routed through the funnel before `app.update()`:
//   - `idle`   = the bare minimal-app frame (scheduler + message GC + the empty-inbox
//                drain), the per-frame floor paid regardless of signals.
//   - `steady` = the same frame PLUS one `Tick(now)` (changed payload, SAME 500ms bucket
//                ⇒ derived phase unchanged ⇒ the drain folds it to a `set_if_neq` no-op).
// The per-routed-signal funnel fixed cost is the DELTA `steady - idle` — NOT the raw
// `steady` number (which is dominated by Bevy's once-per-frame scheduler overhead). That
// delta is what bounds how many such signals fit per weak/wasm-single-threaded frame.
// SOFT ceiling on the delta ≤ ~5K instr (≈0.03% of the ~16M weak-machine frame budget).
fn setup_blink(tick: bool) -> (App, Entity, bool) {
    let (app, e) = build_blink_app(RecordMode::Off);
    (app, e, tick)
}

#[library_benchmark]
#[bench::idle(args = (false), setup = setup_blink)]
#[bench::steady(args = (true), setup = setup_blink)]
fn mvu_blink_cadence(input: (App, Entity, bool)) {
    let (mut app, e, tick) = input;
    if tick {
        // A CHANGED payload vs the 100ms settle, but the SAME 500ms bucket ⇒ derived
        // phase unchanged ⇒ steady (set_if_neq no-op).
        enqueue_blink_direct(&mut app, e, BlinkMsg::Tick(Duration::from_millis(200)));
    }
    app.update();
    black_box(());
}

library_benchmark_group!(
    name = mvu;
    benchmarks =
        mvu_idle,
        mvu_one_message,
        mvu_fold_storm,
        mvu_record_off_vs_on,
        mvu_blink_cadence
);

main!(library_benchmark_groups = mvu);
