//! Quiescence-flush hardening of `capture_to_image` (Phase 3.3,
//! verification-design `determinism.md` § "Async-asset flush to quiescence").
//!
//! Two tiers:
//!   * the no-`Instant::now()` grep-lint runs HEADLESS (§ Verification #4 —
//!     the capture path must read the virtual clock, never wall time);
//!   * the never-loading-asset panic test is GPU `#[ignore]` (§ Verification
//!     #3 — the flush gate fails loudly naming the unmet condition, never
//!     greens on a missing precondition).

mod support;

/// § Verification #4: `Instant::now()` (and `SystemTime::now()`) must NOT
/// appear in the capture path source — a wall-clock read would make a
/// time-dependent capture non-reproducible. The fixed virtual clock
/// (`Time::<Virtual>`) is the only time source. A grep-lint over `golden.rs`,
/// the home of `capture_to_image` + its quiescence loop.
#[test]
fn capture_path_has_no_instant_now() {
    let src = include_str!("../src/render/golden.rs");
    // Strip line comments so a doc-comment MENTIONING the ban does not trip it;
    // we only care about real code reading wall time.
    for (lineno, line) in src.lines().enumerate() {
        let code = match line.split_once("//") {
            Some((before, _)) => before,
            None => line,
        };
        assert!(
            !code.contains("Instant::now"),
            "golden.rs:{} reads wall time via Instant::now() — the capture path \
             must drive Time::<Virtual> only (determinism.md § Verification #4): {line}",
            lineno + 1,
        );
        assert!(
            !code.contains("SystemTime::now"),
            "golden.rs:{} reads wall time via SystemTime::now() — the capture \
             path must drive Time::<Virtual> only: {line}",
            lineno + 1,
        );
    }
}

// § Verification #3: inject an asset that never finishes loading and assert
// `capture_to_image` PANICS naming the unmet quiescence condition (pending
// assets), rather than silently capturing a half-streamed frame. GPU lane.
//
// Run: cargo test -p buiy_core --test render_capture_quiescence -- --ignored \
//        --test-threads=1 --nocapture
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn quiescence_panics_on_never_loading_asset() {
    use bevy::prelude::*;
    use buiy_core::render::golden::{GoldenConfig, PendingCaptureAssets, capture_to_image};

    const W: u32 = 32;
    const H: u32 = 32;

    let mut app = support::gpu_render_app_scaled(W, H, 1.0);

    // Register a handle for a path that can never resolve (no AssetPlugin
    // source serves it), then declare it a capture precondition. The quiescence
    // loop must observe it stuck `Loading`/`Failed`-but-not-loaded and refuse
    // to capture — bounded by MAX_SETTLE_FRAMES, then panic.
    let never = app
        .world()
        .resource::<AssetServer>()
        .load::<Image>("buiy-determinism::never-arrives.png");
    app.world_mut()
        .resource_mut::<PendingCaptureAssets>()
        .require(never.untyped());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let cfg = GoldenConfig::deterministic();
        let _ = capture_to_image(&mut app, &cfg);
    }));
    let payload = result.expect_err("capture must panic on a never-loading asset");
    let msg = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        msg.contains("pending asset") || msg.contains("asset"),
        "panic must name the unmet condition (pending assets); got: {msg:?}"
    );
}
