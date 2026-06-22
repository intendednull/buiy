//! WCAG 2.x relative-luminance contrast ratio over two resolved colors. Pure
//! CPU. The gate-#9 token-pair lint and the focus-ring ≥3:1 claim (§ 3.2) rest
//! on this. Reference values from the WCAG 2.1 definition.

use bevy::prelude::*;
use buiy_core::render::color::contrast_ratio;

#[test]
fn black_on_white_is_twenty_one_to_one() {
    let r = contrast_ratio(Color::BLACK, Color::WHITE);
    assert!(
        (r - 21.0).abs() < 0.01,
        "black/white must be 21:1 (got {r})"
    );
}

#[test]
fn identical_colors_are_one_to_one() {
    let r = contrast_ratio(Color::WHITE, Color::WHITE);
    assert!((r - 1.0).abs() < 1e-6);
}

#[test]
fn ratio_is_symmetric() {
    let a = Color::srgb(0.2, 0.45, 0.95);
    let b = Color::srgb(0.96, 0.96, 0.96);
    assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-6);
}

#[test]
fn focus_ring_pair_meets_three_to_one() {
    // The default focus ring (accent on white surface) must clear WCAG 2.4.11
    // non-text 3:1 — the render-side reason the foundation marks Focus Visible F.
    let ring = Color::srgb(0.20, 0.45, 0.95);
    let surface = Color::WHITE;
    assert!(contrast_ratio(ring, surface) >= 3.0);
}
