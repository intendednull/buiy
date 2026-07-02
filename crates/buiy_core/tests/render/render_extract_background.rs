//! Headless (no GPU): the Background `ColorToken` resolves to a concrete
//! `Color` through the typed `ThemeContract`. Pure resolution; no RenderApp.
//! (Track B closed the vocabulary — a missing/typo'd token is now a compile
//! error, so the former magenta-miss assertions have no runtime meaning and
//! are gone.)

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword, resolve_token};
use buiy_core::theme::Theme;

#[test]
fn custom_token_resolves_to_its_carried_color() {
    // The `Custom` escape hatch carries a concrete color and resolves to itself
    // under any theme — the decoupled-from-palette successor to the old
    // "inject a color, resolve a named token" idiom.
    let theme = Theme::default();
    let tok = ColorToken::Custom(Color::srgb(0.2, 0.3, 0.4));
    assert_eq!(resolve_token(&tok, &theme), Color::srgb(0.2, 0.3, 0.4));
}

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    assert_eq!(resolve_token(&ColorToken::Transparent, &theme), Color::NONE);
}

#[test]
fn system_color_resolves_to_the_system_stub_value() {
    // The canonical resolver routes `SystemColor(kw)` through the system-color
    // stub palette (`system_color_value`), independent of the authored theme
    // palette. `Canvas` is the high-contrast black backdrop — never a magenta
    // miss (that path was removed when the vocabulary closed).
    let theme = Theme::default();
    assert_eq!(
        resolve_token(&ColorToken::SystemColor(SystemColorKeyword::Canvas), &theme),
        Color::srgb(0.0, 0.0, 0.0)
    );
}
