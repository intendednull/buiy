//! The v1 stub forced-colors theme: its `colors` map must hold exactly the 16
//! CSS system-color keys so every forced-colors paint token resolves (§ 3.1).
//! Pure-CPU, no GPU.

use bevy::prelude::*;
use buiy_core::render::color::SystemColorKeyword;
use buiy_core::theme::forced_colors_theme;

#[test]
fn forced_theme_holds_all_sixteen_system_colors() {
    let theme = forced_colors_theme();
    for kw in SystemColorKeyword::ALL {
        assert!(
            theme.color(kw.token()).is_some(),
            "forced-colors theme must define system color {}",
            kw.token()
        );
    }
}

#[test]
fn forced_theme_values_are_not_magenta_sentinel() {
    // The stub must resolve to *real* placeholder values, not the missing-token
    // sentinel — otherwise the forced path is indistinguishable from a miss.
    let theme = forced_colors_theme();
    for kw in SystemColorKeyword::ALL {
        assert_ne!(
            theme.color(kw.token()).unwrap(),
            Color::srgb(1.0, 0.0, 1.0),
            "system color {} must not be the magenta sentinel",
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
        theme.color("Canvas").unwrap(),
        theme.color("CanvasText").unwrap()
    );
}
