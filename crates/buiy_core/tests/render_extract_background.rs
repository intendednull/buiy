//! Headless (no GPU): the Background `ColorToken` resolves to the same
//! `Color` the Phase-0 `Visual.background_token` string did, and a miss
//! still yields the magenta sentinel. Pure resolution; no RenderApp.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword};
use buiy_core::render::resolve_token;
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
    let (color, missed) = resolve_token(&tok, &theme);
    assert_eq!(color, Color::srgb(0.2, 0.3, 0.4));
    assert!(!missed);
}

#[test]
fn missing_token_falls_back_to_magenta_sentinel() {
    let theme = Theme::default();
    let tok = ColorToken::Token(Cow::Borrowed("nope.not.here"));
    let (color, missed) = resolve_token(&tok, &theme);
    assert_eq!(color, Color::srgb(1.0, 0.0, 1.0));
    assert!(missed);
}

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    let (color, missed) = resolve_token(&ColorToken::Transparent, &theme);
    assert_eq!(color, Color::NONE);
    assert!(!missed);
}

#[test]
fn system_color_resolves_to_sentinel_until_forced_colors_map_lands() {
    // v1 deferral: the system-color map is owned by buiy-theme-tokens-design,
    // so every SystemColor token misses → magenta sentinel + warn. This pins
    // the deferred miss-path so the R11 rewrite (route present entries through
    // the map) has a regression guard.
    let theme = Theme::default();
    let (color, missed) =
        resolve_token(&ColorToken::SystemColor(SystemColorKeyword::Canvas), &theme);
    assert_eq!(color, Color::srgb(1.0, 0.0, 1.0));
    assert!(missed);
}
