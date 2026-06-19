//! Gate #11(a): token-flow analyzer. Under the forced-colors theme, no widget
//! paints a color outside the system-color token set. Pure CPU, no GPU.
//! Spec: color-and-forced-colors.md § 3.1; verification.md gate #11.

use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::forced_colors_analyzer::{
    CatalogPaint, ForcedColorsViolation, analyze_forced_colors, analyze_shadow_only,
};
use buiy_core::theme::forced_colors_theme;

fn good_widget() -> CatalogPaint {
    CatalogPaint {
        widget: "button",
        state: "resting",
        background: ColorToken::SystemColor(SystemColorKeyword::ButtonText),
        border: ColorToken::SystemColor(SystemColorKeyword::ButtonBorder),
        outline: ColorToken::Transparent,
        has_shadow_only_state_delta: false,
    }
}

#[test]
fn all_system_color_tokens_pass_under_forced_theme() {
    let theme = forced_colors_theme();
    let report = analyze_forced_colors(&[good_widget()], &theme);
    assert!(
        report.is_empty(),
        "system-color tokens must pass: {report:?}"
    );
}

#[test]
fn non_system_token_under_forced_theme_is_a_violation() {
    // A brand token absent from the forced map resolves to magenta → violation.
    let theme = forced_colors_theme();
    let mut w = good_widget();
    w.background = ColorToken::Token(std::borrow::Cow::Borrowed("color.accent"));
    let report = analyze_forced_colors(&[w], &theme);
    assert_eq!(report.len(), 1);
    assert!(matches!(
        report[0],
        ForcedColorsViolation::NonSystemColor {
            widget: "button",
            ..
        }
    ));
}

#[test]
fn transparent_is_allowed_under_forced_theme() {
    // Transparent is the no-fill case, not a color outside the palette.
    let theme = forced_colors_theme();
    let mut w = good_widget();
    w.background = ColorToken::Transparent;
    assert!(analyze_forced_colors(&[w], &theme).is_empty());
}

#[test]
fn shadow_only_state_delta_is_a_violation() {
    // A focused state that differs from resting ONLY in BoxShadow is invisible
    // once shadows are suppressed under forced-colors (§ 3.2).
    let mut w = good_widget();
    w.state = "focus-visible";
    w.has_shadow_only_state_delta = true;
    let report = analyze_shadow_only(&[w]);
    assert_eq!(report.len(), 1);
    assert!(matches!(
        report[0],
        ForcedColorsViolation::ShadowOnlyAffordance {
            widget: "button",
            state: "focus-visible"
        }
    ));
}

#[test]
fn non_shadow_state_delta_passes() {
    // Resting state (no shadow-only delta) and a state with a border/outline
    // cue both pass.
    let resting = good_widget();
    let mut focus = good_widget();
    focus.state = "focus-visible";
    focus.outline = ColorToken::SystemColor(SystemColorKeyword::Highlight);
    focus.has_shadow_only_state_delta = false;
    assert!(analyze_shadow_only(&[resting, focus]).is_empty());
}
