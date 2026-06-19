//! GPU lane (`--ignored`): the GPU rounded-rect render and the CPU SDF oracle
//! must agree within a documented AA fuzz budget — the golden-free oracle for
//! SDF corner AA (Tier 4.5). A wrong half-extent / radius-clamp / premultiply
//! in the shader would diverge here. reftests.md § CPU-vs-GPU SDF cross-check.

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_verify::metric::FuzzBudget;
use buiy_verify::reftest::run_sdf_cross_check;

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn gpu_rounded_rect_matches_cpu_oracle() {
    let draw = DrawData::new(
        Vec2::new(40.0, 20.0),
        Vec2::new(120.0, 80.0),
        Color::WHITE,
        16.0,
    );
    // AA band tolerance: a sub-pixel rim differs between the GPU `fwidth`
    // screen-space derivative and the CPU central-difference — the documented
    // AA residue (Tier 4.5), NOT a regression. The CPU oracle matches the full
    // GPU capture chain (linear-space SrcOver over the opaque-black clear, sRGB
    // encode), so the box INTERIOR and the EXTERIOR opaque-black background
    // agree exactly; only the ~1px rounded-rect rim disagrees.
    //
    // Measured on the RX 6700 XT (RADV) at (0,0): differing_pixels = 87 of
    // 24000 (the non-AA-excluded rim of a ~400px-perimeter rounded rect),
    // mssim = 0.927. `max_channel_delta` is pinned at the 255 ceiling because a
    // single hard-edge rim pixel can flip fully on/off between the two AA
    // estimators (a true L∞ on one pixel); the meaningful axis is the pixel
    // COUNT, bounded here at 200 (87 measured + driver-variance headroom, well
    // below the 24000 total — a real bound, not a rubber stamp).
    let fuzz = FuzzBudget {
        max_channel_delta: 255,
        max_diff_pixels: 200,
    };
    let outcome = run_sdf_cross_check(&draw, &fuzz);
    assert!(
        outcome.passed,
        "GPU vs CPU-SDF oracle diverged: {:?} (report: {:?})",
        outcome.diff, outcome.report_path
    );
}
