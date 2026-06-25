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
    t.colors.insert(
        "color.selection.bg".into(),
        Color::srgb(0.20, 0.45, 0.95), // the accent blue — web-typical highlight
    );
    t.colors.insert("color.selection.fg".into(), Color::WHITE);
    t.colors.insert(
        "color.text.placeholder".into(),
        Color::srgb(0.55, 0.55, 0.55),
    );
    // NO "color.caret" entry — caret-color: auto parity (T7 decision 7).

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

/// v1 **stub** forced-colors (high-contrast) theme. Its `colors` map holds
/// exactly the 16 CSS system-color keys so every forced-colors paint token
/// resolves (color-and-forced-colors.md § 3.1 — the hard v1 prerequisite).
///
/// Values are placeholders modeled on a Windows-High-Contrast black palette;
/// the *authoritative* system-color values are owned by
/// `buiy-theme-tokens-design`. This stub exists only so the forced-colors path
/// resolves to real colors (not magenta) and the gate-#11 analyzer is
/// meaningful. The keys, not the values, are the contract.
pub fn forced_colors_theme() -> Theme {
    use crate::render::color::SystemColorKeyword::*;
    let mut t = Theme::default();
    let black = Color::srgb(0.0, 0.0, 0.0);
    let white = Color::WHITE;
    let yellow = Color::srgb(1.0, 1.0, 0.0);
    let cyan = Color::srgb(0.0, 1.0, 1.0);
    let gray = Color::srgb(0.5, 0.5, 0.5);
    let pairs = [
        (Canvas, black),
        (CanvasText, white),
        (LinkText, cyan),
        (ButtonText, white),
        (ButtonBorder, white),
        (GrayText, gray),
        (Highlight, yellow),
        (HighlightText, black),
        (Field, black),
        (FieldText, white),
        (Mark, yellow),
        (MarkText, black),
        (SelectedItem, yellow),
        (SelectedItemText, black),
        (AccentColor, cyan),
        (AccentColorText, black),
    ];
    for (kw, color) in pairs {
        t.colors.insert(kw.token().to_string(), color);
    }
    // The C6-a focus ring resolves `Token("color.focus.ring")` (NOT a
    // `SystemColor` token — that would disturb the `Highlight`-prefers-when-
    // present resolvers like `resolve_selection_bg`). Under forced-colors the
    // wholesale swap drops the default theme's `color.focus.ring`, so map it to
    // the high-contrast `Highlight` value here — the ring stays visible (≥ 3:1)
    // under forced-colors without the lowering choosing a different token
    // (styling-f-tier.md § 2.6). The authoritative value is
    // `buiy-theme-tokens-design`'s; the KEY (the focus ring resolves) is the
    // contract.
    t.colors.insert("color.focus.ring".to_string(), yellow);
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
