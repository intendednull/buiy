//! Headless (no GPU): the Background `ColorToken` resolves to the same
//! `Color` the Phase-0 `Visual.background_token` string did, and a miss
//! still yields the magenta sentinel. Pure resolution; no RenderApp.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword, resolve_token};
use buiy_core::theme::Theme;
use std::borrow::Cow;

fn theme_with(token: &str, color: Color) -> Theme {
    let mut t = Theme::default();
    t.colors.insert(token.to_string(), color);
    t
}

#[test]
fn token_resolves_to_theme_color() {
    let theme = theme_with("color.surface.secondary", Color::srgb(0.2, 0.3, 0.4));
    let tok = ColorToken::Token(Cow::Borrowed("color.surface.secondary"));
    assert_eq!(resolve_token(&tok, &theme), Color::srgb(0.2, 0.3, 0.4));
}

#[test]
fn missing_token_falls_back_to_magenta_sentinel() {
    let theme = Theme::default();
    let tok = ColorToken::Token(Cow::Borrowed("nope.not.here"));
    assert_eq!(resolve_token(&tok, &theme), Color::srgb(1.0, 0.0, 1.0));
}

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    assert_eq!(resolve_token(&ColorToken::Transparent, &theme), Color::NONE);
}

#[test]
fn system_color_misses_to_sentinel_when_theme_lacks_the_key() {
    // The canonical resolver routes `SystemColor(kw)` through the theme's
    // system-color map (`resolve_named(kw.token(), …)`). With a bare `Theme`
    // (no system-color keys) the lookup misses → magenta sentinel + warn. The
    // forced-colors stub theme (Task 3) is what supplies those keys; this pins
    // the miss-path for a theme that lacks them.
    let theme = Theme::default();
    assert_eq!(
        resolve_token(&ColorToken::SystemColor(SystemColorKeyword::Canvas), &theme),
        Color::srgb(1.0, 0.0, 1.0)
    );
}
