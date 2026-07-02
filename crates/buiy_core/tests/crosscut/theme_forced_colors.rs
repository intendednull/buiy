//! The v1 stub forced-colors theme: every CSS system-color keyword must resolve
//! to a real high-contrast value under the typed `ThemeContract`, so every
//! forced-colors paint token resolves (§ 3.1). Pure-CPU, no GPU.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword, ThemeContract};
use buiy_core::theme::forced_colors_theme;

#[test]
fn forced_theme_resolves_all_sixteen_system_colors() {
    // Typed successor to the old "the `colors` map holds all 16 keys"
    // completeness check: resolution is now total (a closed enum + an exhaustive
    // match), so we assert every keyword resolves to a concrete, non-transparent
    // color — a `Color::NONE` here would be an invisible forced-colors paint.
    let theme = forced_colors_theme();
    for kw in SystemColorKeyword::ALL {
        assert_ne!(
            theme.resolve(ColorToken::SystemColor(kw)),
            Color::NONE,
            "system color {} must resolve to a real value",
            kw.token()
        );
    }
}

#[test]
fn forced_theme_canvas_and_canvastext_differ() {
    // High-contrast mode: Canvas (surface) and CanvasText (text) must be
    // distinct colors. This asserts *inequality* (`assert_ne!`), NOT a WCAG
    // contrast-ratio bound — the real ratio gate is deferred until the
    // forced-colors palette lands real values (`forced_colors_theme` is a
    // documented v1 stub; audit 2026-06-18 #36).
    let theme = forced_colors_theme();
    assert_ne!(
        theme.resolve(ColorToken::SystemColor(SystemColorKeyword::Canvas)),
        theme.resolve(ColorToken::SystemColor(SystemColorKeyword::CanvasText))
    );
}
