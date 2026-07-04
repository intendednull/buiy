//! GPU lane (`--ignored`): the GPU rounded-rect render and the CPU SDF oracle
//! must agree within a documented AA fuzz budget — the golden-free oracle for
//! SDF corner AA (Tier 4.5). A wrong half-extent / radius-clamp / premultiply
//! in the shader would diverge here. reftests.md § CPU-vs-GPU SDF cross-check.
//!
//! Dooduel F3 tightened this guard. Before F3 the `spawn_single_primitive`
//! fixture (a `Border.radius` with a zero-width border) rendered a SQUARE fill —
//! the borderless-rounded-fill path was stubbed (`pack_extracted` packed radius
//! 0) — so this cross-check compared a SQUARE GPU fill against the ROUNDED CPU
//! oracle and only PASSED because a **200px `max_diff_pixels`** budget absorbed
//! the ~87px square-vs-rounded corner delta. That budget did not guard the
//! corner; it TOLERATED a square fill. F3 renders the fill rounded, so the diff
//! now drops to the true AA-rim residue and the budget is tightened toward it,
//! and a negative control asserts the tight budget rejects a square fill.

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_verify::metric::FuzzBudget;
use buiy_verify::reftest::{run_sdf_cross_check, run_sdf_cross_check_vs_oracle_radius};

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn gpu_rounded_rect_matches_cpu_oracle() {
    let draw = DrawData::new(
        Vec2::new(40.0, 20.0),
        Vec2::new(120.0, 80.0),
        Color::WHITE,
        16.0,
    );
    // AA band tolerance, TIGHTENED for the now-rounded fill (F3). The GPU renders
    // the fill rounded (borderless-rounded-fill path), so it agrees with the CPU
    // rounded oracle EXACTLY: the sub-pixel rounded-corner RIM (where the GPU
    // `fwidth` screen-space derivative and the CPU central-difference estimate AA
    // coverage slightly differently) falls entirely inside the metric's
    // AA-excluded band, so the measured `differing_pixels` is **0** of 24000 on
    // the pinned lavapipe (Mesa 24.3.4 / LLVM 18) — not the ~87px square-vs-rounded
    // delta the old 200px budget tolerated. `max_channel_delta` stays at the 255
    // ceiling (a single hard-edge rim pixel can flip fully on/off between the two
    // AA estimators — a true L∞ on one pixel; the meaningful axis is the pixel
    // COUNT). Bounded at 8 = measured 0 + sub-pixel driver-jitter headroom, and
    // 16x below the 129px a SQUARE fill would diverge by (the negative control) —
    // a real bound with teeth: a regression reverting the borderless-rounded fill
    // renders square and FAILS here.
    let fuzz = FuzzBudget {
        max_channel_delta: 255,
        max_diff_pixels: 8,
    };
    let rounded = run_sdf_cross_check(&draw, &fuzz);
    eprintln!(
        "sdf cross-check (rounded fill vs rounded oracle): differing_pixels={} max_channel_delta={}",
        rounded.diff.differing_pixels, rounded.diff.max_channel_delta
    );
    assert!(
        rounded.passed,
        "GPU vs CPU-SDF oracle diverged: {:?} (report: {:?})",
        rounded.diff, rounded.report_path
    );

    // Negative control — the tightened budget has TEETH. The SAME rounded GPU
    // fill diffed against a SQUARE oracle (radius 0) must NOT pass the tight
    // budget: the four rounded corners differ from square by 129px (measured),
    // far over the 8px bound. This is what makes the cross-check GUARD the corner
    // instead of tolerating a square fill — a regression reverting F3's
    // borderless-rounded fill would render square and FAIL the positive check.
    let vs_square = run_sdf_cross_check_vs_oracle_radius(&draw, 0.0, &fuzz);
    eprintln!(
        "sdf cross-check NEGATIVE control (rounded fill vs SQUARE oracle): differing_pixels={}",
        vs_square.diff.differing_pixels
    );
    assert!(
        !vs_square.passed,
        "the tightened budget FAILED to reject a square fill (differing_pixels={}) — \
         the cross-check does not guard the corner",
        vs_square.diff.differing_pixels
    );
}
