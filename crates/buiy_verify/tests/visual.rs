use buiy_verify::visual::{DiffResult, compare_images};
use image::{DynamicImage, RgbaImage, open};

#[test]
fn identical_images_diff_zero() {
    let baseline = open("tests/fixtures/visual/baseline.png").unwrap();
    let result: DiffResult = compare_images(&baseline, &baseline);
    assert_eq!(result.score, 0.0);
    assert!(result.passed(0.01), "identical images pass 0.01 tolerance");
}

#[test]
fn tinted_image_diff_nonzero() {
    let a = open("tests/fixtures/visual/baseline.png").unwrap();
    let b = open("tests/fixtures/visual/tinted.png").unwrap();
    let result = compare_images(&a, &b);
    assert!(result.score > 0.0, "different images produce nonzero diff");
}

#[test]
fn dimension_mismatch_returns_one() {
    let a = DynamicImage::ImageRgba8(RgbaImage::new(2, 2));
    let b = DynamicImage::ImageRgba8(RgbaImage::new(3, 2));
    let result = compare_images(&a, &b);
    assert_eq!(result.score, 1.0);
    assert!(!result.passed(0.5), "mismatched-dim sentinel exceeds 0.5");
}

#[test]
fn empty_images_compare_identical_without_nan() {
    let a = DynamicImage::ImageRgba8(RgbaImage::new(0, 0));
    let b = DynamicImage::ImageRgba8(RgbaImage::new(0, 0));
    let result = compare_images(&a, &b);
    assert_eq!(result.score, 0.0, "0x0 vs 0x0 is identical, not NaN");
    assert!(
        result.passed(0.01),
        "empty-vs-empty must pass any non-negative tolerance"
    );
}
