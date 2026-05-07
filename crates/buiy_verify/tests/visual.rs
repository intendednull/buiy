use buiy_verify::visual::{DiffResult, compare_images};
use image::open;

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
