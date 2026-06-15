//! Migrated from the deleted RMSE `visual::compare_images` to the unified
//! `buiy_verify::metric` (metric.md § Migration). In-memory fixtures; the old
//! baseline/tinted PNGs are gone.

use buiy_verify::metric::{CompareOpts, FuzzBudget, compare};
use image::{Rgba, RgbaImage};

fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(px))
}

#[test]
fn identical_images_pass_exact() {
    let img = solid(16, 16, [30, 60, 90, 255]);
    let d = compare(&img, &img, &CompareOpts::default());
    assert_eq!(d.differing_pixels, 0);
    assert!(
        d.passes(&FuzzBudget::EXACT),
        "identical images pass the exact budget"
    );
}

#[test]
fn tinted_image_fails_exact() {
    let a = solid(16, 16, [40, 40, 40, 255]);
    let b = solid(16, 16, [40, 40, 200, 255]); // uniform blue tint
    let d = compare(
        &a,
        &b,
        &CompareOpts {
            include_aa: true,
            ..Default::default()
        },
    );
    assert!(d.differing_pixels > 0, "a uniform tint differs");
    assert!(
        !d.passes(&FuzzBudget::EXACT),
        "tinted image fails the exact budget"
    );
}

#[test]
fn dimension_mismatch_fails_every_budget() {
    let a = solid(2, 2, [0, 0, 0, 255]);
    let b = solid(3, 2, [0, 0, 0, 255]);
    let d = compare(&a, &b, &CompareOpts::default());
    assert!(
        !d.passes(&FuzzBudget {
            max_channel_delta: 255,
            max_diff_pixels: u32::MAX
        }),
        "mismatched dims saturate and fail even a maximal budget"
    );
}

#[test]
fn empty_vs_empty_is_zero_diff() {
    let e = RgbaImage::new(0, 0);
    let d = compare(&e, &e, &CompareOpts::default());
    assert_eq!(d.total_pixels, 0);
    assert!(
        d.passes(&FuzzBudget::EXACT),
        "empty-vs-empty observes no difference"
    );
}

#[test]
fn empty_capture_against_real_baseline_saturates_both_orders() {
    // A 0×0 image is a *dimension mismatch* against a real one — it must
    // saturate (loud-fail), not slip through the empty fast-path. The golden
    // gate (golden/check.rs) feeds the LIVE capture as arg `a`, so a render that
    // emits a 0×0 image MUST be rejected against any stored golden; the
    // mirror-image order must fail too. This is the asymmetric case the
    // `empty_vs_empty` / equal-dim `dimension_mismatch` tests never exercised.
    let real = solid(4, 4, [10, 20, 30, 255]);
    let empty = RgbaImage::new(0, 0);
    let maximal = FuzzBudget {
        max_channel_delta: 255,
        max_diff_pixels: u32::MAX,
    };

    let capture_empty = compare(&empty, &real, &CompareOpts::default());
    assert!(
        capture_empty.saturated,
        "a 0×0 capture vs a real baseline is a dimension mismatch — it must saturate"
    );
    assert!(
        !capture_empty.passes(&maximal),
        "an empty capture must fail even a maximal budget (golden gate safety)"
    );

    let baseline_empty = compare(&real, &empty, &CompareOpts::default());
    assert!(
        baseline_empty.saturated,
        "a real capture vs a 0×0 baseline must also saturate"
    );
    assert!(
        !baseline_empty.passes(&maximal),
        "the mirror-image order must fail too"
    );
}
