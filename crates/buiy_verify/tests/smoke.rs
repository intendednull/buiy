#[test]
fn re_exports_compile() {
    use buiy_verify::{a11y, contrast, visual};
    let _ = visual::compare_images;
    let _ = a11y::snapshot_tree;
    let _ = contrast::wcag2_ratio;
}
