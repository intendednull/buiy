//! `ColorToken` typed-variant + default tests. Pure-CPU, no GPU adapter.
//! Verifies the R1-owned shape this plan's resolver (Task 2) extends.
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword, ThemeContract, resolve_token};
use buiy_core::theme::default_light_theme;

#[test]
fn color_token_default_is_transparent() {
    // CSS-initial "no fill" == empty-token skip case (component-model § 2/§ 3).
    assert_eq!(ColorToken::default(), ColorToken::Transparent);
}

#[test]
fn color_token_variants_construct() {
    let _ = ColorToken::SurfacePrimary;
    let _ = ColorToken::CurrentColor;
    let _ = ColorToken::SystemColor(SystemColorKeyword::CanvasText);
}

#[test]
fn system_color_keyword_set_has_all_sixteen() {
    // The foundation-F 16-keyword CSS system-color set (visuals.md § 3.3).
    use SystemColorKeyword::*;
    let all = [
        Canvas,
        CanvasText,
        LinkText,
        ButtonText,
        ButtonBorder,
        GrayText,
        Highlight,
        HighlightText,
        Field,
        FieldText,
        Mark,
        MarkText,
        SelectedItem,
        SelectedItemText,
        AccentColor,
        AccentColorText,
    ];
    assert_eq!(all.len(), 16);
}

#[test]
fn transparent_resolves_to_none() {
    let theme = default_light_theme();
    assert_eq!(resolve_token(&ColorToken::Transparent, &theme), Color::NONE);
}

#[test]
fn token_hit_resolves_to_theme_color() {
    let theme = default_light_theme();
    let got = resolve_token(&ColorToken::SurfacePrimary, &theme);
    assert_eq!(got, Color::WHITE);
}

#[test]
fn current_color_default_path_falls_back_to_foreground_token() {
    // v1 fallback: non-forced theme → the primary text ink (§ 2.0).
    let theme = default_light_theme();
    let got = resolve_token(&ColorToken::CurrentColor, &theme);
    assert_eq!(got, theme.resolve(ColorToken::TextPrimary));
}
