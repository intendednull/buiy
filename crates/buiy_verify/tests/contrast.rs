use bevy::prelude::Color;
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
    let bg = theme.color("color.surface.primary").unwrap();
    let fg = theme.color("color.text.primary").unwrap();
    let r = wcag2_ratio(fg, bg);
    assert!(
        r >= WCAG_AA_NORMAL,
        "default theme text on surface is AA: ratio={r}"
    );
}

#[test]
fn linter_reports_violations_for_failing_pair() {
    let mut theme = default_light_theme();
    // Insert a known-failing pair.
    theme
        .colors
        .insert("color.text.bad".into(), Color::srgb(0.9, 0.9, 0.9));
    let pairs = vec![("color.surface.primary", "color.text.bad")];
    let violations = contrast_violations(&theme, &pairs, WCAG_AA_NORMAL);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].severity, ContrastSeverity::Fail);
}

// Suppress unused-import warning for `lint_theme` — kept in scope to match
// the public-API contract documented in the task spec.
#[allow(dead_code)]
fn _lint_theme_kept_in_scope() {
    let theme = default_light_theme();
    let _ = lint_theme(&theme);
}
