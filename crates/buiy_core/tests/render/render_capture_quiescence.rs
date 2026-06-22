//! Quiescence-flush hardening of `capture_to_image` (Phase 3.3,
//! verification-design `determinism.md` § "Async-asset flush to quiescence").
//!
//! Two tiers:
//!   * the no-`Instant::now()` grep-lint runs HEADLESS (§ Verification #4 —
//!     the capture path must read the virtual clock, never wall time);
//!   * the never-loading-asset panic test is GPU `#[ignore]` (§ Verification
//!     #3 — the flush gate fails loudly naming the unmet condition, never
//!     greens on a missing precondition).

use std::path::Path;

/// Scan one source file for a real (non-comment) wall-clock read and panic with a
/// precise `file:line` if found. Strips line comments so a doc-comment MENTIONING
/// the ban does not trip it; we only care about code that actually reads wall
/// time. Bans both `Instant::now` and `SystemTime::now` (the two
/// `std::time` wall-clock entry points).
fn assert_no_wall_clock_read(rel_path: &str, src: &str) {
    for (lineno, line) in src.lines().enumerate() {
        let code = match line.split_once("//") {
            Some((before, _)) => before,
            None => line,
        };
        for banned in ["Instant::now", "SystemTime::now"] {
            assert!(
                !code.contains(banned),
                "{rel_path}:{} reads wall time via {banned}() — determinism-sensitive \
                 render code must drive Time::<Virtual> only (determinism.md \
                 § Verification #4): {line}",
                lineno + 1,
            );
        }
    }
}

/// § Verification #4: `Instant::now()` (and `SystemTime::now()`) must NOT appear
/// in ANY determinism-sensitive render module — a wall-clock read would make the
/// rasterized frame time-dependent and the capture non-reproducible. The fixed
/// virtual clock (`Time::<Virtual>`) is the only time source on this path.
///
/// Audit #38 (T4.6): the lint previously scoped ONLY `golden.rs` (the home of
/// `capture_to_image`), but the determinism contract covers the WHOLE frame
/// production spine — extract → prepare → instance → bucket → composite → effect
/// → top-layer → clip → bridge → visibility — not just the capture loop. An
/// `Instant::now()` creeping into any of those (e.g. a per-frame "animate by
/// elapsed wall time" shortcut) would non-deterministically change WHAT gets
/// rasterized, defeating the `(0,0)` fuzz budget while leaving `golden.rs`
/// pristine. So the lint now walks every `.rs` in `src/render/` (any new module
/// is covered automatically — no allow-list to keep in sync).
///
/// Scope note: this bans wall time only in `buiy_core`'s render modules. The
/// bless-ledger timestamp in `buiy_verify` (`golden/check.rs::now_rfc3339`) is a
/// LEGITIMATE `SystemTime::now()` — it records when a golden was blessed and is
/// not on the deterministic capture path — so it is deliberately out of scope.
#[test]
fn render_path_has_no_wall_clock_read() {
    let render_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render");
    let mut scanned = 0usize;
    // Walk src/render/ RECURSIVELY (stack, no walkdir dep) so SUBDIRECTORIES are
    // covered — a non-recursive read_dir silently skips src/render/atlas/ (lru.rs
    // etc.), exactly where an "evict by elapsed wall-time" shortcut could creep in
    // on the determinism path.
    let mut stack = vec![render_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src/render/ subdir exists") {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue; // skip the .wgsl shader sources
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let rel = path.strip_prefix(&render_dir).unwrap().to_string_lossy();
            assert_no_wall_clock_read(&format!("render/{rel}"), &src);
            scanned += 1;
        }
    }
    // Non-vacuity guard: a renamed/moved render dir, or a walk that stopped
    // descending into subdirs, must fail loudly rather than silently pass over too
    // few files. The recursive render module set (incl. atlas/, effect/, …) is
    // ~29 .rs files; a floor of 25 sits well above the top-level-only count, so a
    // regression that drops the recursion trips it.
    assert!(
        scanned >= 25,
        "expected to scan the full recursive render module set, only saw \
         {scanned} files — did src/render/ move, or did the walk stop descending \
         into subdirectories?"
    );
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

    let mut app = crate::support::gpu_render_app_scaled(W, H, 1.0);

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
