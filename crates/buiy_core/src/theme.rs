//! Token-based theming.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.5 and
//! cross-cutting.md § 3.14. Phase 0 ships a minimal token surface — the
//! full token taxonomy lives in `buiy-theme-tokens-design`.

use bevy::prelude::*;
use std::collections::HashMap;

/// A single theme variant. Phase 0 stores tokens as flat string-keyed maps.
/// The full token system replaces this with typed scales in
/// `buiy-theme-tokens-design`.
///
/// Marked `#[non_exhaustive]` because the field set is explicitly Phase 0
/// minimal — typed token scales (typography, motion, elevation, etc.) will
/// add fields pre-1.0. External callers should use `Theme::default()` or
/// `default_light_theme()` and mutate fields rather than struct literals.
#[derive(Resource, Reflect, Clone, Debug, Default)]
#[reflect(Resource)]
#[non_exhaustive]
pub struct Theme {
    pub colors: HashMap<String, Color>,
    pub spaces: HashMap<String, f32>,
    pub radii: HashMap<String, f32>,
}

impl Theme {
    pub fn color(&self, token: &str) -> Option<Color> {
        self.colors.get(token).copied()
    }
    pub fn space(&self, token: &str) -> Option<f32> {
        self.spaces.get(token).copied()
    }
    pub fn radius(&self, token: &str) -> Option<f32> {
        self.radii.get(token).copied()
    }
}

/// OS preference resource. Updated by a system in BuiySet::Input that reads
/// from winit (or platform-specific sources). Phase 0 populates with
/// defaults; full OS-pref plumbing is `buiy-clipboard-and-os-integration-design`.
///
/// Marked `#[non_exhaustive]` because additional preferences (caret
/// blink, pointer fine/coarse, prefers-color-scheme states beyond
/// dark/light, NVDA / VoiceOver-specific hints) will be added pre-1.0.
/// External callers should construct via `UserPreferences::default()`
/// and override fields rather than using struct literals.
#[derive(Resource, Reflect, Clone, Debug, Default)]
#[reflect(Resource)]
#[non_exhaustive]
pub struct UserPreferences {
    pub prefers_dark: bool,
    pub prefers_reduced_motion: bool,
    pub prefers_reduced_transparency: bool,
    pub prefers_more_contrast: bool,
    pub forced_colors: bool,
    pub inverted_colors: bool,
}

/// Default light theme. Phase 0 is intentionally bare; v1 ships a full token
/// scale set validated against WCAG 2.2 AA in CI.
pub fn default_light_theme() -> Theme {
    let mut t = Theme::default();
    t.colors
        .insert("color.surface.primary".into(), Color::WHITE);
    t.colors.insert(
        "color.surface.secondary".into(),
        Color::srgb(0.96, 0.96, 0.96),
    );
    t.colors
        .insert("color.text.primary".into(), Color::srgb(0.10, 0.10, 0.10));
    t.colors
        .insert("color.text.secondary".into(), Color::srgb(0.40, 0.40, 0.40));
    t.colors
        .insert("color.accent".into(), Color::srgb(0.20, 0.45, 0.95));
    t.colors
        .insert("color.focus.ring".into(), Color::srgb(0.20, 0.45, 0.95));

    t.spaces.insert("space.0".into(), 0.0);
    t.spaces.insert("space.1".into(), 4.0);
    t.spaces.insert("space.2".into(), 8.0);
    t.spaces.insert("space.3".into(), 12.0);
    t.spaces.insert("space.4".into(), 16.0);

    t.radii.insert("radius.sm".into(), 2.0);
    t.radii.insert("radius.md".into(), 6.0);
    t.radii.insert("radius.lg".into(), 12.0);
    t
}

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Theme>()
            .register_type::<UserPreferences>()
            .insert_resource(default_light_theme())
            .insert_resource(UserPreferences::default());
    }
}
