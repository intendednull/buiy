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

/// The default accent (`color.accent`) for the dark theme — the design's blue
/// (`#5b86f5`), per values.md § 1.1 (`color.accent.blue`, the runtime `--ac`
/// default). `default_dark_theme` seeds the accent ramp from this; a `SetAccent`
/// message swaps it at runtime (see [`apply_set_accent`]).
pub const DARK_ACCENT_DEFAULT: Color = Color::srgb_u8(0x5b, 0x86, 0xf5);

/// Derives the accent ramp from a base accent color, matching the design's
/// `applyAccent(hex)` JS (values.md § 1.2). Returns
/// `(accent_lighter /*--ac2*/, accent_soft /*--acsoft*/, accent_glow /*--acglow*/)`.
///
/// The math runs in **0–255 integer space** exactly like the source JS:
/// - `--ac2`  = `lighten` each channel, where `lighten(v) = round(v + (255-v)*0.22)`
///   (clamped to 255) — i.e. +22 % toward white.
/// - `--acsoft` = the accent rgb at alpha `0.16`.
/// - `--acglow` = the accent rgb at alpha `0.55`.
///
/// Channels are read from the sRGB-encoded color (rounding `chan*255`), so a
/// color built via [`Color::srgb_u8`] round-trips its hex exactly. Verified
/// against values.md: blue `#5b86f5` → `#7fa1f7`, green `#45c07d` → `#6ece9a`.
pub fn derive_accent_ramp(accent: Color) -> (Color, Color, Color) {
    let s = Srgba::from(accent);
    let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b) = (to_u8(s.red), to_u8(s.green), to_u8(s.blue));

    // lighten(v) = min(255, round(v + (255 - v) * 0.22)), in 0–255 space.
    let lighten = |v: u8| -> u8 {
        let v = v as f32;
        (v + (255.0 - v) * 0.22).round().min(255.0) as u8
    };
    let lighter = Color::srgb_u8(lighten(r), lighten(g), lighten(b));
    let soft = Color::srgba_u8(r, g, b, (0.16 * 255.0_f32).round() as u8);
    let glow = Color::srgba_u8(r, g, b, (0.55 * 255.0_f32).round() as u8);
    (lighter, soft, glow)
}

/// Seeds `color.accent` + the three derived ramp tokens into a theme's color
/// map from a base accent. Shared by [`default_dark_theme`] (initial seed) and
/// [`apply_set_accent`] (runtime swap) so the ramp is computed in exactly one
/// place. Token names (the gallery binds these):
/// - `color.accent`         — the base accent (`--ac`)
/// - `color.accent.lighter` — `--ac2`, the lightened ramp stop
/// - `color.accent.soft`    — `--acsoft`, accent @ α.16
/// - `color.accent.glow`    — `--acglow`, accent @ α.55
fn seed_accent_tokens(colors: &mut HashMap<String, Color>, accent: Color) {
    let (lighter, soft, glow) = derive_accent_ramp(accent);
    colors.insert("color.accent".into(), accent);
    colors.insert("color.accent.lighter".into(), lighter);
    colors.insert("color.accent.soft".into(), soft);
    colors.insert("color.accent.glow".into(), glow);
}

/// Default **dark** theme — the Widget Catalog parity palette (values.md § 1).
///
/// Not the app default (the app stays light); the gallery opts into this and a
/// `SetAccent` message re-themes it live. It defines a dark value for **every**
/// token key `default_light_theme` defines (so existing widgets/tests resolve
/// without a magenta miss), **plus** the full design token taxonomy
/// (`surface.*`, `border.*`, the text ink ladder, `accent.*`, `status.*`,
/// scrim, on-accent, scrollbar, the accent ramp).
pub fn default_dark_theme() -> Theme {
    let mut t = Theme::default();
    let c = &mut t.colors;

    // --- Canonical keys shared with the light theme (must all resolve) ---
    // Mapped to sensible dark values per the design.
    c.insert(
        "color.surface.primary".into(),
        Color::srgb_u8(0x0b, 0x0c, 0x0e),
    ); // surface.app
    c.insert(
        "color.surface.secondary".into(),
        Color::srgb_u8(0x16, 0x18, 0x1c), // surface.card
    );
    c.insert(
        "color.text.primary".into(),
        Color::srgb_u8(0xf1, 0xf3, 0xf6),
    );
    c.insert(
        "color.text.secondary".into(),
        Color::srgb_u8(0xc2, 0xc8, 0xd2),
    );
    // color.accent + the ramp tokens are seeded together below.
    c.insert("color.focus.ring".into(), DARK_ACCENT_DEFAULT);
    // ::selection is hard-coded blue in the design (values.md § 1.1, ambiguity
    // flag) — fixed blue rgba(91,134,245,.32), NOT accent-derived.
    c.insert(
        "color.selection.bg".into(),
        Color::srgba_u8(0x5b, 0x86, 0xf5, (0.32 * 255.0_f32).round() as u8),
    );
    c.insert(
        "color.selection.fg".into(),
        Color::srgb_u8(0xf1, 0xf3, 0xf6),
    );
    c.insert(
        "color.text.placeholder".into(),
        Color::srgb_u8(0x55, 0x5c, 0x67), // text.dim — search/draft placeholder hint
    );
    // NO "color.caret" entry — caret-color: auto parity (T7 decision 7).

    // --- surface.* (values.md § 1.1) ---
    c.insert("color.surface.app".into(), Color::srgb_u8(0x0b, 0x0c, 0x0e));
    c.insert(
        "color.surface.chrome".into(),
        Color::srgb_u8(0x0d, 0x0e, 0x11),
    );
    c.insert(
        "color.surface.chrome-translucent".into(),
        Color::srgba_u8(0x0d, 0x0e, 0x11, 0xcc), // #0d0e11cc (80% alpha)
    );
    c.insert(
        "color.surface.card".into(),
        Color::srgb_u8(0x16, 0x18, 0x1c),
    );
    c.insert(
        "color.surface.inset".into(),
        Color::srgb_u8(0x12, 0x14, 0x17),
    );
    c.insert(
        "color.surface.raised".into(),
        Color::srgb_u8(0x1a, 0x1d, 0x22),
    );
    c.insert(
        "color.surface.raised-alt".into(),
        Color::srgb_u8(0x1e, 0x21, 0x27),
    );
    c.insert(
        "color.surface.danger".into(),
        Color::srgb_u8(0x39, 0x1b, 0x1a),
    );
    c.insert(
        "color.surface.danger-soft".into(),
        Color::srgb_u8(0x1a, 0x12, 0x13),
    );
    c.insert("color.surface.transparent".into(), Color::NONE);

    // --- border.* (values.md § 1.1) ---
    c.insert(
        "color.border.subtle".into(),
        Color::srgb_u8(0x1c, 0x1f, 0x24),
    );
    c.insert(
        "color.border.subtle-2".into(),
        Color::srgb_u8(0x14, 0x16, 0x1a),
    );
    c.insert(
        "color.border.default".into(),
        Color::srgb_u8(0x26, 0x2a, 0x31),
    );
    c.insert(
        "color.border.strong".into(),
        Color::srgb_u8(0x2c, 0x31, 0x3a),
    );
    c.insert(
        "color.border.strong-2".into(),
        Color::srgb_u8(0x3a, 0x41, 0x50),
    );
    c.insert(
        "color.border.muted".into(),
        Color::srgb_u8(0x39, 0x40, 0x4a),
    );
    c.insert(
        "color.border.danger".into(),
        Color::srgb_u8(0x3a, 0x24, 0x22),
    );

    // --- text.* ink ladder (values.md § 1.1) ---
    c.insert("color.text.bright".into(), Color::srgb_u8(0xe7, 0xea, 0xef));
    c.insert("color.text.muted".into(), Color::srgb_u8(0x86, 0x8d, 0x99));
    c.insert("color.text.faint".into(), Color::srgb_u8(0x6f, 0x77, 0x83));
    c.insert("color.text.dim".into(), Color::srgb_u8(0x55, 0x5c, 0x67));
    c.insert("color.text.dimmer".into(), Color::srgb_u8(0x3a, 0x40, 0x49));
    c.insert("color.text.danger".into(), Color::srgb_u8(0xf0, 0x65, 0x5b));
    c.insert(
        "color.text.danger-dim".into(),
        Color::srgb_u8(0x7a, 0x3a, 0x36),
    );
    c.insert(
        "color.text.on-accent".into(),
        Color::srgb_u8(0x07, 0x10, 0x1f),
    );

    // --- accent.* selectable options (values.md § 1.1) ---
    c.insert("color.accent.blue".into(), Color::srgb_u8(0x5b, 0x86, 0xf5));
    c.insert(
        "color.accent.green".into(),
        Color::srgb_u8(0x45, 0xc0, 0x7d),
    );
    c.insert(
        "color.accent.violet".into(),
        Color::srgb_u8(0xb9, 0x8a, 0xff),
    );
    c.insert(
        "color.accent.coral".into(),
        Color::srgb_u8(0xf0, 0x65, 0x5b),
    );

    // --- status.* (values.md § 1.1) ---
    c.insert("color.status.ok".into(), Color::srgb_u8(0x45, 0xc0, 0x7d));
    c.insert("color.status.warn".into(), Color::srgb_u8(0xd7, 0xa2, 0x3f));
    c.insert(
        "color.status.error".into(),
        Color::srgb_u8(0xf0, 0x65, 0x5b),
    );

    // --- misc / specials (values.md § 1.1) ---
    c.insert("color.misc.white".into(), Color::WHITE);
    c.insert("color.misc.dot-bg".into(), Color::srgb_u8(0x16, 0x18, 0x1c));
    // scrim — rgba(4,5,7,.66) modal backdrop dim.
    c.insert(
        "color.scrim".into(),
        Color::srgba_u8(0x04, 0x05, 0x07, (0.66 * 255.0_f32).round() as u8),
    );

    // --- scrollbar.* (values.md § 1.1 / § 7.3) ---
    c.insert("color.scrollbar.track".into(), Color::NONE);
    c.insert(
        "color.scrollbar.thumb".into(),
        Color::srgb_u8(0x26, 0x2a, 0x31), // == border.default
    );
    c.insert(
        "color.scrollbar.thumb-hover".into(),
        Color::srgb_u8(0x39, 0x40, 0x4a), // == border.muted
    );

    // --- accent ramp (color.accent + lighter/soft/glow), seeded for blue ---
    seed_accent_tokens(c, DARK_ACCENT_DEFAULT);

    // --- space scale (design uses a denser set; keep the light-theme keys too) ---
    t.spaces.insert("space.0".into(), 0.0);
    t.spaces.insert("space.1".into(), 4.0);
    t.spaces.insert("space.2".into(), 8.0);
    t.spaces.insert("space.3".into(), 12.0);
    t.spaces.insert("space.4".into(), 16.0);
    t.spaces.insert("space.5".into(), 18.0);
    t.spaces.insert("space.6".into(), 24.0);

    // --- radius scale (values.md § 3) ---
    t.radii.insert("radius.xs".into(), 2.0);
    t.radii.insert("radius.sm".into(), 5.0);
    t.radii.insert("radius.md".into(), 6.0);
    t.radii.insert("radius.md-2".into(), 7.0);
    t.radii.insert("radius.lg".into(), 8.0);
    t.radii.insert("radius.lg-2".into(), 9.0);
    t.radii.insert("radius.xl".into(), 10.0);
    t.radii.insert("radius.xl-2".into(), 11.0);
    t.radii.insert("radius.2xl".into(), 12.0);
    t.radii.insert("radius.3xl".into(), 14.0);
    t.radii.insert("radius.pill".into(), 99.0);

    t
}

/// Runtime accent-swap message. Carries the new base accent color; the
/// [`apply_set_accent`] system reads it and re-seeds `color.accent` + the ramp
/// tokens into `Res<Theme>`, so the existing `theme.is_changed()` re-extract
/// re-resolves every accent-bearing paint. Producers (e.g. the gallery's accent
/// swatches) `write` it; the swap system reads it.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetAccent(pub Color);

/// Reads [`SetAccent`] messages and mutates the global [`Theme`] resource:
/// re-seeds `color.accent` and the derived ramp tokens (`color.accent.lighter`/
/// `.soft`/`.glow`) from the new accent, leaving every other token untouched.
///
/// Taking `ResMut<Theme>` and writing through it marks the resource changed, so
/// the render extract's `theme.is_changed()` guard re-resolves all paint. If
/// several `SetAccent` messages arrive in one frame the **last** wins (the
/// accent is a single global value). Only touches the map when at least one
/// message is present, to avoid spuriously marking the theme changed.
pub fn apply_set_accent(mut messages: MessageReader<SetAccent>, theme: Option<ResMut<Theme>>) {
    let Some(SetAccent(accent)) = messages.read().last().copied() else {
        return;
    };
    let Some(mut theme) = theme else {
        return;
    };
    seed_accent_tokens(&mut theme.colors, accent);
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
        // App default stays LIGHT — the gallery opts into the dark theme in a
        // later wave. `default_dark_theme` + `SetAccent` are made available here
        // but do not change the default insert.
        app.register_type::<Theme>()
            .register_type::<UserPreferences>()
            .insert_resource(default_light_theme())
            .insert_resource(UserPreferences::default())
            .add_message::<SetAccent>()
            .add_systems(Update, apply_set_accent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Color` → `#rrggbb` (matches the values.md hex notation).
    fn hex(color: Color) -> String {
        let s = Srgba::from(color);
        let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02x}{:02x}{:02x}",
            to_u8(s.red),
            to_u8(s.green),
            to_u8(s.blue)
        )
    }

    /// `Color` alpha channel as a rounded u8 (for the rgba specials).
    fn alpha_u8(color: Color) -> u8 {
        (Srgba::from(color).alpha.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    #[test]
    fn dark_theme_resolves_every_light_theme_key() {
        // No magenta miss: the dark theme must define a value for EVERY token
        // key the light theme defines, so widgets/tests authored against the
        // canonical keys still resolve under the dark theme.
        let light = default_light_theme();
        let dark = default_dark_theme();
        for key in light.colors.keys() {
            assert!(
                dark.colors.contains_key(key),
                "dark theme is missing canonical color key `{key}`"
            );
        }
        for key in light.spaces.keys() {
            assert!(
                dark.spaces.contains_key(key),
                "dark theme is missing canonical space key `{key}`"
            );
        }
        for key in light.radii.keys() {
            assert!(
                dark.radii.contains_key(key),
                "dark theme is missing canonical radius key `{key}`"
            );
        }
    }

    #[test]
    fn dark_theme_seeds_sample_new_tokens() {
        let dark = default_dark_theme();
        // A representative sample across each new family (values.md § 1.1).
        assert_eq!(hex(dark.color("color.surface.app").unwrap()), "#0b0c0e");
        assert_eq!(hex(dark.color("color.surface.card").unwrap()), "#16181c");
        assert_eq!(hex(dark.color("color.border.subtle").unwrap()), "#1c1f24");
        assert_eq!(hex(dark.color("color.border.strong").unwrap()), "#2c313a");
        assert_eq!(hex(dark.color("color.text.bright").unwrap()), "#e7eaef");
        assert_eq!(hex(dark.color("color.text.muted").unwrap()), "#868d99");
        assert_eq!(hex(dark.color("color.text.on-accent").unwrap()), "#07101f");
        assert_eq!(hex(dark.color("color.accent.green").unwrap()), "#45c07d");
        assert_eq!(hex(dark.color("color.status.warn").unwrap()), "#d7a23f");
        assert_eq!(hex(dark.color("color.status.error").unwrap()), "#f0655b");

        // Alpha specials (values.md § 1.1): scrim rgba(4,5,7,.66) + selection.
        let scrim = dark.color("color.scrim").unwrap();
        assert_eq!(hex(scrim), "#040507");
        assert_eq!(alpha_u8(scrim), (0.66 * 255.0_f32).round() as u8);
        assert_eq!(
            alpha_u8(dark.color("color.selection.bg").unwrap()),
            (0.32 * 255.0_f32).round() as u8
        );

        // Radius scale (values.md § 3).
        assert_eq!(dark.radius("radius.pill").unwrap(), 99.0);
        assert_eq!(dark.radius("radius.2xl").unwrap(), 12.0);
    }

    #[test]
    fn derive_accent_ramp_matches_values_md_blue_and_green() {
        // Blue #5b86f5 → ac2 #7fa1f7 (values.md § 1.2 authoritative recompute).
        let (blue_ac2, blue_soft, blue_glow) = derive_accent_ramp(Color::srgb_u8(0x5b, 0x86, 0xf5));
        assert_eq!(hex(blue_ac2), "#7fa1f7");
        assert_eq!(hex(blue_soft), "#5b86f5");
        assert_eq!(alpha_u8(blue_soft), (0.16 * 255.0_f32).round() as u8);
        assert_eq!(hex(blue_glow), "#5b86f5");
        assert_eq!(alpha_u8(blue_glow), (0.55 * 255.0_f32).round() as u8);

        // Green #45c07d → ac2 #6ece9a (values.md § 1.2).
        let (green_ac2, _, _) = derive_accent_ramp(Color::srgb_u8(0x45, 0xc0, 0x7d));
        assert_eq!(hex(green_ac2), "#6ece9a");

        // Spot the other two accents from values.md § 1.2 for completeness.
        let (violet_ac2, _, _) = derive_accent_ramp(Color::srgb_u8(0xb9, 0x8a, 0xff));
        assert_eq!(hex(violet_ac2), "#c8a4ff");
        let (coral_ac2, _, _) = derive_accent_ramp(Color::srgb_u8(0xf0, 0x65, 0x5b));
        assert_eq!(hex(coral_ac2), "#f3877f");
    }

    #[test]
    fn dark_theme_seeds_blue_accent_ramp() {
        // default_dark_theme seeds the ramp for the default blue accent.
        let dark = default_dark_theme();
        assert_eq!(hex(dark.color("color.accent").unwrap()), "#5b86f5");
        assert_eq!(hex(dark.color("color.accent.lighter").unwrap()), "#7fa1f7");
        assert_eq!(hex(dark.color("color.accent.soft").unwrap()), "#5b86f5");
        assert_eq!(
            alpha_u8(dark.color("color.accent.soft").unwrap()),
            (0.16 * 255.0_f32).round() as u8
        );
        assert_eq!(
            alpha_u8(dark.color("color.accent.glow").unwrap()),
            (0.55 * 255.0_f32).round() as u8
        );
    }

    #[test]
    fn set_accent_mutates_theme_and_marks_changed() {
        // Records, per-frame, whether a downstream `Res<Theme>` reader saw the
        // theme as changed — exactly how the render extract observes the swap
        // edge (`theme.is_changed()` inside a system, before `clear_trackers`).
        #[derive(Resource, Default)]
        struct SawThemeChanged(bool);

        fn record_change(theme: Res<Theme>, mut saw: ResMut<SawThemeChanged>) {
            saw.0 = theme.is_changed();
        }

        let mut app = App::new();
        app.add_message::<SetAccent>()
            .insert_resource(default_dark_theme())
            .init_resource::<SawThemeChanged>()
            // `record_change` runs after `apply_set_accent` in the same frame,
            // so it observes the mutation just like the extract would.
            .add_systems(Update, (apply_set_accent, record_change).chain());

        // First update: the initial insert reads as changed for the first reader.
        app.update();

        // A no-message frame must NOT mark the theme changed (early return,
        // `ResMut` never dereferenced).
        app.update();
        assert!(
            !app.world().resource::<SawThemeChanged>().0,
            "an empty frame must not mark Theme changed"
        );

        // Swap to green: ramp must recompute AND the reader must see it changed.
        app.world_mut()
            .write_message(SetAccent(Color::srgb_u8(0x45, 0xc0, 0x7d)));
        app.update();
        assert!(
            app.world().resource::<SawThemeChanged>().0,
            "SetAccent must mark Theme changed for the downstream reader"
        );

        let theme = app.world().resource_ref::<Theme>();
        assert_eq!(hex(theme.color("color.accent").unwrap()), "#45c07d");
        assert_eq!(hex(theme.color("color.accent.lighter").unwrap()), "#6ece9a");

        // And the frame after the swap must settle (no message → no change).
        app.update();
        assert!(
            !app.world().resource::<SawThemeChanged>().0,
            "the frame after a swap must not mark Theme changed"
        );
    }
}
