use bevy::prelude::Color;
use buiy_core::render::color::{ColorToken, ThemeContract};
use buiy_core::theme::default_light_theme;
use buiy_verify::contrast::{
    ContrastSeverity, WCAG_AA_NORMAL, contrast_violations, lint_theme, wcag2_ratio,
};

#[test]
fn black_white_ratio_is_21() {
    let r = wcag2_ratio(Color::WHITE, Color::BLACK);
    assert!(
        (r - 21.0).abs() < 0.01,
        "white/black ratio is 21:1, got {r}"
    );
}

#[test]
fn equal_colors_ratio_is_1() {
    let r = wcag2_ratio(Color::WHITE, Color::WHITE);
    assert!((r - 1.0).abs() < 0.01);
}

#[test]
fn aa_passes_default_light_theme_text_on_surface() {
    let theme = default_light_theme();
    let bg = theme.resolve(ColorToken::SurfacePrimary);
    let fg = theme.resolve(ColorToken::TextPrimary);
    let r = wcag2_ratio(fg, bg);
    assert!(
        r >= WCAG_AA_NORMAL,
        "default theme text on surface is AA: ratio={r}"
    );
}

#[test]
fn linter_reports_violations_for_failing_pair() {
    let theme = default_light_theme();
    // A near-white "text" on the white surface fails AA. The failing fg color is
    // carried inline via the `Custom` escape hatch (the theme color HashMap is
    // gone), so the pair still fails independent of the palette.
    let bad_fg = Color::srgb(0.9, 0.9, 0.9);
    let pairs = vec![(ColorToken::SurfacePrimary, ColorToken::Custom(bad_fg))];
    let violations = contrast_violations(&theme, &pairs, WCAG_AA_NORMAL);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].severity, ContrastSeverity::Fail);
}

#[test]
fn lint_theme_passes_for_default_light() {
    assert!(lint_theme(&default_light_theme()).is_ok());
}
