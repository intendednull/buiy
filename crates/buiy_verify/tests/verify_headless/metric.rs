//! Known-answer meta-tests for `buiy_verify::metric` (metric.md § Verification).
//! Pure CPU, no GPU lane.

use buiy_verify::metric::{CompareOpts, Diff, FuzzBudget, compare};
use image::{Rgba, RgbaImage};

fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba(px))
}

#[test]
fn identity_zero_diff_full_mssim() {
    let img = solid(8, 8, [12, 34, 56, 255]);
    let d = compare(&img, &img, &CompareOpts::default());
    assert_eq!(d.differing_pixels, 0);
    assert_eq!(d.max_channel_delta, 0);
    assert!(d.mssim.unwrap() > 0.999);
    assert!(d.passes(&FuzzBudget::EXACT));
}

#[test]
fn single_defect_survives_scale() {
    for n in [16u32, 256, 2048] {
        let a = solid(n, n, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(n / 2, n / 2, Rgba([200, 200, 200, 255]));
        let d = compare(
            &a,
            &b,
            &CompareOpts {
                include_aa: true,
                mssim: false,
                ..Default::default()
            },
        );
        assert_eq!(d.differing_pixels, 1, "N={n}");
        assert!(!d.passes(&FuzzBudget::EXACT), "N={n}");
    }
}

#[test]
fn dimension_mismatch_fails_every_budget() {
    let a = solid(4, 4, [0, 0, 0, 255]);
    let b = solid(4, 5, [0, 0, 0, 255]);
    let d = compare(&a, &b, &CompareOpts::default());
    assert_eq!(d.differing_pixels, d.total_pixels);
    assert_eq!(d.max_channel_delta, 255);
    assert!(!d.passes(&FuzzBudget {
        max_channel_delta: 255,
        max_diff_pixels: u32::MAX
    }));
}

/// Constants tripwire: a fixed 8×8 pair yields an exact integer Diff. A
/// pixelmatch-constant drift changes these numbers and reds this test. (Phase 2
/// upgrades this to the floats-redacted insta snapshot metric.md specifies.)
#[test]
fn vendored_constants_are_pinned() {
    let mut a = solid(8, 8, [0, 0, 0, 255]);
    let mut b = solid(8, 8, [0, 0, 0, 255]);
    // Three deterministic, isolated, non-AA defects of known magnitude.
    a.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
    b.put_pixel(1, 1, Rgba([255, 0, 0, 255])); // luma-heavy
    a.put_pixel(4, 4, Rgba([0, 0, 0, 255]));
    b.put_pixel(4, 4, Rgba([0, 255, 0, 255]));
    a.put_pixel(6, 2, Rgba([10, 10, 10, 255]));
    b.put_pixel(6, 2, Rgba([250, 250, 250, 255]));
    let d = compare(
        &a,
        &b,
        &CompareOpts {
            mssim: false,
            ..Default::default()
        },
    );
    // EXPECTED: re-bless intentionally if the algorithm changes.
    let Diff {
        differing_pixels,
        max_channel_delta,
        total_pixels,
        ..
    } = d;
    assert_eq!(
        (differing_pixels, max_channel_delta, total_pixels),
        (3, 255, 64),
        "vendored YIQ/AA constants drifted — re-derive deliberately, do not patch the number",
    );
}

#[test]
fn reftest_default_excludes_aa_and_skips_diff_image() {
    let opts = buiy_verify::metric::CompareOpts::reftest_default();
    assert!(!opts.include_aa, "reftest excludes AA-sibling pixels");
    assert!(opts.mssim, "MSSIM stays computed (advisory)");
    assert!(
        !opts.emit_diff_image,
        "hot reftest path allocates no diff image"
    );
    assert_eq!(opts.threshold, 0.1, "pixelmatch default sensitivity");
}
