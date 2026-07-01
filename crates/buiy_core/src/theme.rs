//! Token-based theming.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.5 and
//! cross-cutting.md § 3.14. Colors are now a **typed closed vocabulary**
//! ([`ColorToken`]) resolved through [`ThemeContract`]; a [`Palette`] holds the
//! Normal-mode values and [`PaletteMode`] selects Normal vs forced-colors
//! resolution (Track B). Spacing/radius scales stay stringly HashMaps
//! (`Theme::space`/`radius`) pending their own typed-scale follow-on.

use crate::render::color::{ColorToken, SystemColorKeyword, ThemeContract};
use bevy::prelude::*;
use std::collections::HashMap;

/// The FIXED accent swatch literals — the gallery's selectable accent options
/// (values.md § 1.1). Constant across themes and NOT moved by [`SetAccent`]
/// (unlike the live accent ramp), so a swatch chip keeps its identity while the
/// live accent re-themes. `ACCENT_BLUE` is also the dark theme's default live
/// accent ([`DARK_ACCENT_DEFAULT`]).
pub const ACCENT_BLUE: Color = Color::srgb_u8(0x5b, 0x86, 0xf5);
/// See [`ACCENT_BLUE`].
pub const ACCENT_GREEN: Color = Color::srgb_u8(0x45, 0xc0, 0x7d);
/// See [`ACCENT_BLUE`].
pub const ACCENT_VIOLET: Color = Color::srgb_u8(0xb9, 0x8a, 0xff);
/// See [`ACCENT_BLUE`].
pub const ACCENT_CORAL: Color = Color::srgb_u8(0xf0, 0x65, 0x5b);

/// Which resolution mode a [`Theme`] is in. Set by the forced-colors swap
/// (`render/forced_colors.rs`); read by [`ThemeContract::resolve`].
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PaletteMode {
    /// The authored (light/dark) palette: semantic tokens resolve to
    /// [`Palette`] values, the live accent ramp, or the fixed swatches.
    #[default]
    Normal,
    /// OS forced-colors (high-contrast): every semantic token resolves to a CSS
    /// system color (color-and-forced-colors.md § 3.1), ignoring the palette.
    ForcedColors,
}

/// The typed color palette — the Normal-mode value of every **stored** semantic
/// token. The live accent ramp ([`ColorToken::Accent`] etc.) is derived from
/// [`Theme::accent`] and the fixed swatches ([`ColorToken::AccentBlue`] etc.)
/// are constants, so neither is stored here.
///
/// Fields are private: colors are read only through [`ThemeContract::resolve`].
/// Construct a theme via [`default_light_theme`] / [`default_dark_theme`].
#[derive(Reflect, Clone, Debug)]
pub struct Palette {
    // --- Surface ---
    surface_app: Color,
    surface_primary: Color,
    surface_secondary: Color,
    surface_card: Color,
    surface_raised: Color,
    surface_raised_alt: Color,
    surface_inset: Color,
    surface_chrome: Color,
    surface_chrome_translucent: Color,
    surface_danger: Color,
    surface_danger_soft: Color,
    surface_danger_strong: Color,
    // --- Text ink ladder ---
    text_primary: Color,
    text_secondary: Color,
    text_muted: Color,
    text_dim: Color,
    text_dimmer: Color,
    text_faint: Color,
    text_bright: Color,
    text_placeholder: Color,
    text_danger: Color,
    text_danger_dim: Color,
    text_on_accent: Color,
    // --- Border ---
    border_default: Color,
    border_subtle: Color,
    border_subtle_2: Color,
    border_muted: Color,
    border_strong: Color,
    border_strong_2: Color,
    border_danger: Color,
    // --- Status ---
    status_ok: Color,
    status_warn: Color,
    status_error: Color,
    // --- Shadow ---
    shadow_card: Color,
    shadow_menu: Color,
    shadow_modal: Color,
    shadow_slider_thumb: Color,
    shadow_switch_thumb: Color,
    shadow_danger_button: Color,
    // --- Selection ---
    selection_bg: Color,
    selection_fg: Color,
    // --- Scrollbar ---
    scrollbar_thumb: Color,
    scrollbar_thumb_hover: Color,
    scrollbar_track: Color,
    // --- Misc / specials ---
    focus_ring: Color,
    scrim: Color,
    white: Color,
    dot_bg: Color,
}

/// A single theme variant.
///
/// Colors are a typed [`Palette`] + a live [`accent`](Theme::accent) +
/// resolution [`mode`](Theme::mode); spacing/radius scales remain flat
/// string-keyed maps (a typed-scale follow-on is tracked). Resolve a
/// [`ColorToken`] via [`ThemeContract::resolve`]; read spacing/radius via
/// [`Theme::space`] / [`Theme::radius`].
///
/// Marked `#[non_exhaustive]` because the field set is still evolving — typed
/// scales (typography, motion, elevation, etc.) will add fields pre-1.0.
/// External callers should use [`Theme::default`] / [`default_light_theme`] and
/// mutate fields rather than struct literals.
#[derive(Resource, Reflect, Clone, Debug)]
#[reflect(Resource)]
#[non_exhaustive]
pub struct Theme {
    palette: Palette,
    /// The live accent base (`--ac`). [`SetAccent`] moves this; the live accent
    /// ramp ([`ColorToken::Accent`] / `AccentLighter` / `AccentSoft` /
    /// `AccentGlow`) is derived from it at resolve time. The fixed accent
    /// swatches ([`ColorToken::AccentBlue`] etc.) do NOT track it.
    pub accent: Color,
    /// Normal vs forced-colors resolution (`render/forced_colors.rs` sets it).
    pub mode: PaletteMode,
    pub spaces: HashMap<String, f32>,
    pub radii: HashMap<String, f32>,
}

impl Default for Theme {
    /// The app default is the LIGHT theme (the gallery opts into dark). Kept as
    /// the documented `Theme::default()` entry point.
    fn default() -> Self {
        default_light_theme()
    }
}

impl Theme {
    pub fn space(&self, token: &str) -> Option<f32> {
        self.spaces.get(token).copied()
    }
    pub fn radius(&self, token: &str) -> Option<f32> {
        self.radii.get(token).copied()
    }
}

/// The v1 **stub** forced-colors (high-contrast) system palette — the value of
/// each of the 16 CSS system-color keywords (color-and-forced-colors.md § 3.1).
/// Every [`ColorToken::SystemColor`] resolves through this, and under
/// [`PaletteMode::ForcedColors`] so does every semantic token.
///
/// Values are placeholders modeled on a Windows-High-Contrast black palette;
/// the *authoritative* values are owned by `buiy-theme-tokens-design`. The
/// mapping (which keyword → which color), not the exact color, is the contract.
fn system_color_value(kw: SystemColorKeyword) -> Color {
    use SystemColorKeyword::*;
    let black = Color::srgb(0.0, 0.0, 0.0);
    let white = Color::WHITE;
    let yellow = Color::srgb(1.0, 1.0, 0.0);
    let cyan = Color::srgb(0.0, 1.0, 1.0);
    let gray = Color::srgb(0.5, 0.5, 0.5);
    match kw {
        Canvas => black,
        CanvasText => white,
        LinkText => cyan,
        ButtonText => white,
        ButtonBorder => white,
        GrayText => gray,
        Highlight => yellow,
        HighlightText => black,
        Field => black,
        FieldText => white,
        Mark => yellow,
        MarkText => black,
        SelectedItem => yellow,
        SelectedItemText => black,
        AccentColor => cyan,
        AccentColorText => black,
    }
}

impl ThemeContract for Theme {
    fn resolve(&self, token: ColorToken) -> Color {
        match self.mode {
            PaletteMode::Normal => self.resolve_normal(token),
            PaletteMode::ForcedColors => self.resolve_forced(token),
        }
    }
}

impl Theme {
    /// Normal-mode resolution: the authored palette, the live accent ramp, and
    /// the fixed swatch constants. Exhaustive over `ColorToken` — the compile
    /// error a new-token-without-a-value would raise is the whole point (F6).
    fn resolve_normal(&self, token: ColorToken) -> Color {
        use ColorToken as T;
        let p = &self.palette;
        match token {
            T::Transparent => Color::NONE,
            // currentColor v1 fallback: the theme default foreground.
            T::CurrentColor => p.text_primary,
            T::SystemColor(kw) => system_color_value(kw),
            T::Custom(c) => c,
            // Live accent ramp (derived from the runtime accent).
            T::Accent => self.accent,
            T::AccentLighter => derive_accent_ramp(self.accent).0,
            T::AccentSoft => derive_accent_ramp(self.accent).1,
            T::AccentGlow => derive_accent_ramp(self.accent).2,
            // Fixed accent swatches (constant literals).
            T::AccentBlue => ACCENT_BLUE,
            T::AccentGreen => ACCENT_GREEN,
            T::AccentViolet => ACCENT_VIOLET,
            T::AccentCoral => ACCENT_CORAL,
            // Surface
            T::SurfaceApp => p.surface_app,
            T::SurfacePrimary => p.surface_primary,
            T::SurfaceSecondary => p.surface_secondary,
            T::SurfaceCard => p.surface_card,
            T::SurfaceRaised => p.surface_raised,
            T::SurfaceRaisedAlt => p.surface_raised_alt,
            T::SurfaceInset => p.surface_inset,
            T::SurfaceChrome => p.surface_chrome,
            T::SurfaceChromeTranslucent => p.surface_chrome_translucent,
            T::SurfaceDanger => p.surface_danger,
            T::SurfaceDangerSoft => p.surface_danger_soft,
            T::SurfaceDangerStrong => p.surface_danger_strong,
            // Text
            T::TextPrimary => p.text_primary,
            T::TextSecondary => p.text_secondary,
            T::TextMuted => p.text_muted,
            T::TextDim => p.text_dim,
            T::TextDimmer => p.text_dimmer,
            T::TextFaint => p.text_faint,
            T::TextBright => p.text_bright,
            T::TextPlaceholder => p.text_placeholder,
            T::TextDanger => p.text_danger,
            T::TextDangerDim => p.text_danger_dim,
            T::TextOnAccent => p.text_on_accent,
            // Border
            T::BorderDefault => p.border_default,
            T::BorderSubtle => p.border_subtle,
            T::BorderSubtle2 => p.border_subtle_2,
            T::BorderMuted => p.border_muted,
            T::BorderStrong => p.border_strong,
            T::BorderStrong2 => p.border_strong_2,
            T::BorderDanger => p.border_danger,
            // Status
            T::StatusOk => p.status_ok,
            T::StatusWarn => p.status_warn,
            T::StatusError => p.status_error,
            // Shadow
            T::ShadowCard => p.shadow_card,
            T::ShadowMenu => p.shadow_menu,
            T::ShadowModal => p.shadow_modal,
            T::ShadowSliderThumb => p.shadow_slider_thumb,
            T::ShadowSwitchThumb => p.shadow_switch_thumb,
            T::ShadowDangerButton => p.shadow_danger_button,
            // Selection
            T::SelectionBg => p.selection_bg,
            T::SelectionFg => p.selection_fg,
            // Scrollbar
            T::ScrollbarThumb => p.scrollbar_thumb,
            T::ScrollbarThumbHover => p.scrollbar_thumb_hover,
            T::ScrollbarTrack => p.scrollbar_track,
            // Misc
            T::FocusRing => p.focus_ring,
            T::Scrim => p.scrim,
            T::White => p.white,
            T::DotBg => p.dot_bg,
        }
    }

    /// Forced-colors resolution (color-and-forced-colors.md § 3.1): map every
    /// semantic token to a role-appropriate CSS system color so nothing ever
    /// resolves to magenta. This is the safety net; the gate-#11 analyzer still
    /// FLAGS concrete semantic tokens (see [`ColorToken::is_forced_colors_safe`]).
    /// The role map is a lossy judgment call, NOT a parity target — no real
    /// widget paints a semantic token under forced-colors (proved by the
    /// live-catalog analyzer), so no golden depends on these choices.
    fn resolve_forced(&self, token: ColorToken) -> Color {
        use ColorToken as T;
        use SystemColorKeyword as K;
        let role = match token {
            // Mode-independent kinds.
            T::Transparent => return Color::NONE,
            T::Custom(c) => return c,
            T::SystemColor(kw) => return system_color_value(kw),
            // currentColor → the high-contrast foreground.
            T::CurrentColor => K::CanvasText,
            // Surfaces + backdrop-ish fills → Canvas.
            T::SurfaceApp
            | T::SurfacePrimary
            | T::SurfaceSecondary
            | T::SurfaceCard
            | T::SurfaceRaised
            | T::SurfaceRaisedAlt
            | T::SurfaceInset
            | T::SurfaceChrome
            | T::SurfaceChromeTranslucent
            | T::SurfaceDanger
            | T::SurfaceDangerSoft
            | T::SurfaceDangerStrong
            | T::Scrim
            | T::DotBg
            | T::ScrollbarTrack => K::Canvas,
            // Readable text / ink / status → CanvasText.
            T::TextPrimary
            | T::TextSecondary
            | T::TextMuted
            | T::TextDim
            | T::TextDimmer
            | T::TextFaint
            | T::TextBright
            | T::TextDanger
            | T::TextDangerDim
            | T::TextOnAccent
            | T::White
            | T::StatusOk
            | T::StatusWarn
            | T::StatusError => K::CanvasText,
            // Placeholder = the grayed/hint tier → GrayText.
            T::TextPlaceholder => K::GrayText,
            // Borders → CanvasText (visible outlines).
            T::BorderDefault
            | T::BorderSubtle
            | T::BorderSubtle2
            | T::BorderMuted
            | T::BorderStrong
            | T::BorderStrong2
            | T::BorderDanger => K::CanvasText,
            // Accent / focus / selection background → Highlight. FocusRing→
            // Highlight (yellow) reproduces the prior forced `color.focus.ring`.
            T::Accent
            | T::AccentLighter
            | T::AccentSoft
            | T::AccentGlow
            | T::AccentBlue
            | T::AccentGreen
            | T::AccentViolet
            | T::AccentCoral
            | T::FocusRing
            | T::SelectionBg => K::Highlight,
            T::SelectionFg => K::HighlightText,
            // Shadows blend into the background (also draw-skipped under forced).
            T::ShadowCard
            | T::ShadowMenu
            | T::ShadowModal
            | T::ShadowSliderThumb
            | T::ShadowSwitchThumb
            | T::ShadowDangerButton => K::Canvas,
            // Scrollbar thumb: resting de-emphasized, hover emphasized (a
            // non-shadow inter-state cue survives forced-colors).
            T::ScrollbarThumb => K::GrayText,
            T::ScrollbarThumbHover => K::CanvasText,
        };
        system_color_value(role)
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

/// The standard spacing scale shared by the light + dark themes (px). Dark adds
/// the two denser stops on top (`space.5`/`space.6`).
fn base_spaces() -> HashMap<String, f32> {
    let mut spaces = HashMap::new();
    spaces.insert("space.0".into(), 0.0);
    spaces.insert("space.1".into(), 4.0);
    spaces.insert("space.2".into(), 8.0);
    spaces.insert("space.3".into(), 12.0);
    spaces.insert("space.4".into(), 16.0);
    spaces
}

/// Default light theme. The ~9 originally-authored tokens are preserved
/// byte-identically; the remaining semantic tokens are **completeness defaults
/// derived by family** (surface → white/light-gray ladder, text → dark-ink
/// ladder, border → light-gray, status/accent → shared hues, shadow → low-alpha
/// black, scrollbar → gray). They are not pixel-critical — no light-default
/// consumer paints them today; the parity path is the dark/gallery theme.
pub fn default_light_theme() -> Theme {
    let palette = Palette {
        // Surface (white → light-gray ladder).
        surface_app: Color::srgb(0.98, 0.98, 0.98),
        surface_primary: Color::WHITE, // preserved (original)
        surface_secondary: Color::srgb(0.96, 0.96, 0.96), // preserved (original)
        surface_card: Color::WHITE,
        surface_raised: Color::WHITE,
        surface_raised_alt: Color::srgb(0.98, 0.98, 0.99),
        surface_inset: Color::srgb(0.94, 0.94, 0.95),
        surface_chrome: Color::srgb(0.97, 0.97, 0.98),
        surface_chrome_translucent: Color::srgba(0.97, 0.97, 0.98, 0.80),
        surface_danger: Color::srgb(0.99, 0.93, 0.93),
        surface_danger_soft: Color::srgb(0.99, 0.96, 0.96),
        surface_danger_strong: ACCENT_CORAL, // the delete-confirm red, shared
        // Text (dark-ink ladder).
        text_primary: Color::srgb(0.10, 0.10, 0.10), // preserved (original)
        text_secondary: Color::srgb(0.40, 0.40, 0.40), // preserved (original)
        text_muted: Color::srgb(0.45, 0.45, 0.48),
        text_dim: Color::srgb(0.55, 0.55, 0.55),
        text_dimmer: Color::srgb(0.65, 0.65, 0.68),
        text_faint: Color::srgb(0.55, 0.55, 0.58),
        text_bright: Color::srgb(0.05, 0.05, 0.05),
        text_placeholder: Color::srgb(0.55, 0.55, 0.55), // preserved (original)
        text_danger: Color::srgb(0.75, 0.20, 0.16),
        text_danger_dim: Color::srgb(0.60, 0.35, 0.33),
        text_on_accent: Color::WHITE,
        // Border (light-gray ladder).
        border_default: Color::srgb(0.85, 0.85, 0.87),
        border_subtle: Color::srgb(0.90, 0.90, 0.92),
        border_subtle_2: Color::srgb(0.94, 0.94, 0.95),
        border_muted: Color::srgb(0.80, 0.80, 0.83),
        border_strong: Color::srgb(0.72, 0.72, 0.75),
        border_strong_2: Color::srgb(0.62, 0.62, 0.66),
        border_danger: Color::srgb(0.90, 0.70, 0.68),
        // Status (shared semantic hues).
        status_ok: ACCENT_GREEN,
        status_warn: Color::srgb_u8(0xd7, 0xa2, 0x3f),
        status_error: ACCENT_CORAL,
        // Shadow (low-alpha black on a light backdrop).
        shadow_card: Color::srgba(0.0, 0.0, 0.0, 0.12),
        shadow_menu: Color::srgba(0.0, 0.0, 0.0, 0.16),
        shadow_modal: Color::srgba(0.0, 0.0, 0.0, 0.24),
        shadow_slider_thumb: Color::srgba(0.0, 0.0, 0.0, 0.18),
        shadow_switch_thumb: Color::srgba(0.0, 0.0, 0.0, 0.12),
        shadow_danger_button: Color::srgba_u8(0xcf, 0x3a, 0x36, (0.35 * 255.0_f32).round() as u8),
        // Selection (preserved originals).
        selection_bg: Color::srgb(0.20, 0.45, 0.95),
        selection_fg: Color::WHITE,
        // Scrollbar (gray).
        scrollbar_thumb: Color::srgb(0.75, 0.75, 0.78),
        scrollbar_thumb_hover: Color::srgb(0.60, 0.60, 0.63),
        scrollbar_track: Color::NONE,
        // Misc.
        focus_ring: Color::srgb(0.20, 0.45, 0.95), // preserved (original)
        scrim: Color::srgba(0.0, 0.0, 0.0, 0.50),
        white: Color::WHITE,
        dot_bg: Color::srgb(0.90, 0.90, 0.92),
    };

    let mut radii = HashMap::new();
    radii.insert("radius.sm".into(), 2.0);
    radii.insert("radius.md".into(), 6.0);
    radii.insert("radius.lg".into(), 12.0);

    Theme {
        palette,
        accent: Color::srgb(0.20, 0.45, 0.95), // preserved (original color.accent)
        mode: PaletteMode::Normal,
        spaces: base_spaces(),
        radii,
    }
}

/// The default accent (`color.accent`) for the dark theme — the design's blue
/// (`#5b86f5`), per values.md § 1.1 (`color.accent.blue`, the runtime `--ac`
/// default). `default_dark_theme` seeds `accent` from this; a [`SetAccent`]
/// message swaps it at runtime (see [`apply_set_accent`]).
pub const DARK_ACCENT_DEFAULT: Color = ACCENT_BLUE;

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

/// Default **dark** theme — the Widget Catalog parity palette (values.md § 1).
///
/// Not the app default (the app stays light); the gallery opts into this and a
/// [`SetAccent`] message re-themes it live. Every value here is the **exact**
/// literal the pre-Track-B `colors` HashMap held (the byte-identical parity
/// target); the migration moved them from map inserts to typed [`Palette`]
/// fields without changing a single one.
pub fn default_dark_theme() -> Theme {
    let palette = Palette {
        // --- surface.* (values.md § 1.1) ---
        surface_app: Color::srgb_u8(0x0b, 0x0c, 0x0e),
        surface_primary: Color::srgb_u8(0x0b, 0x0c, 0x0e), // == surface.app
        surface_secondary: Color::srgb_u8(0x16, 0x18, 0x1c), // == surface.card
        surface_card: Color::srgb_u8(0x16, 0x18, 0x1c),
        surface_raised: Color::srgb_u8(0x1a, 0x1d, 0x22),
        surface_raised_alt: Color::srgb_u8(0x1e, 0x21, 0x27),
        surface_inset: Color::srgb_u8(0x12, 0x14, 0x17),
        surface_chrome: Color::srgb_u8(0x0d, 0x0e, 0x11),
        surface_chrome_translucent: Color::srgba_u8(0x0d, 0x0e, 0x11, 0xcc), // #0d0e11cc (80% α)
        surface_danger: Color::srgb_u8(0x39, 0x1b, 0x1a),
        surface_danger_soft: Color::srgb_u8(0x1a, 0x12, 0x13),
        // The delete-confirm button face — saturated red distinct from the dim
        // `surface.danger` tile (values.md § 7.2 Modal `#cf3a36`).
        surface_danger_strong: Color::srgb_u8(0xcf, 0x3a, 0x36),
        // --- text.* ink ladder (values.md § 1.1) ---
        text_primary: Color::srgb_u8(0xf1, 0xf3, 0xf6),
        text_secondary: Color::srgb_u8(0xc2, 0xc8, 0xd2),
        text_muted: Color::srgb_u8(0x86, 0x8d, 0x99),
        text_dim: Color::srgb_u8(0x55, 0x5c, 0x67),
        text_dimmer: Color::srgb_u8(0x3a, 0x40, 0x49),
        text_faint: Color::srgb_u8(0x6f, 0x77, 0x83),
        text_bright: Color::srgb_u8(0xe7, 0xea, 0xef),
        text_placeholder: Color::srgb_u8(0x55, 0x5c, 0x67), // == text.dim
        text_danger: Color::srgb_u8(0xf0, 0x65, 0x5b),
        text_danger_dim: Color::srgb_u8(0x7a, 0x3a, 0x36),
        text_on_accent: Color::srgb_u8(0x07, 0x10, 0x1f),
        // --- border.* (values.md § 1.1) ---
        border_default: Color::srgb_u8(0x26, 0x2a, 0x31),
        border_subtle: Color::srgb_u8(0x1c, 0x1f, 0x24),
        border_subtle_2: Color::srgb_u8(0x14, 0x16, 0x1a),
        border_muted: Color::srgb_u8(0x39, 0x40, 0x4a),
        border_strong: Color::srgb_u8(0x2c, 0x31, 0x3a),
        border_strong_2: Color::srgb_u8(0x3a, 0x41, 0x50),
        border_danger: Color::srgb_u8(0x3a, 0x24, 0x22),
        // --- status.* (values.md § 1.1) ---
        status_ok: Color::srgb_u8(0x45, 0xc0, 0x7d),
        status_warn: Color::srgb_u8(0xd7, 0xa2, 0x3f),
        status_error: Color::srgb_u8(0xf0, 0x65, 0x5b),
        // --- shadow.* — box-shadow catalog colors incl. alpha (values.md § 2).
        // The offset/blur/spread live on each `Shadow` term; the COLOR is here.
        shadow_card: Color::srgba_u8(0x00, 0x00, 0x00, (0.70 * 255.0_f32).round() as u8),
        shadow_menu: Color::srgba_u8(0x00, 0x00, 0x00, (0.80 * 255.0_f32).round() as u8),
        shadow_modal: Color::srgba_u8(0x00, 0x00, 0x00, (0.85 * 255.0_f32).round() as u8),
        shadow_slider_thumb: Color::srgba_u8(0x00, 0x00, 0x00, (0.50 * 255.0_f32).round() as u8),
        shadow_switch_thumb: Color::srgba_u8(0x00, 0x00, 0x00, (0.40 * 255.0_f32).round() as u8),
        // Colored delete-confirm glow keyed to `#cf3a36` (values.md § 2).
        shadow_danger_button: Color::srgba_u8(0xcf, 0x3a, 0x36, (0.50 * 255.0_f32).round() as u8),
        // --- ::selection (values.md § 1.1 — fixed blue rgba(91,134,245,.32),
        // NOT accent-derived) ---
        selection_bg: Color::srgba_u8(0x5b, 0x86, 0xf5, (0.32 * 255.0_f32).round() as u8),
        selection_fg: Color::srgb_u8(0xf1, 0xf3, 0xf6),
        // --- scrollbar.* (values.md § 1.1 / § 7.3) ---
        scrollbar_thumb: Color::srgb_u8(0x26, 0x2a, 0x31), // == border.default
        scrollbar_thumb_hover: Color::srgb_u8(0x39, 0x40, 0x4a), // == border.muted
        scrollbar_track: Color::NONE,
        // --- misc / specials (values.md § 1.1) ---
        focus_ring: DARK_ACCENT_DEFAULT,
        // scrim — rgba(4,5,7,.66) modal backdrop dim.
        scrim: Color::srgba_u8(0x04, 0x05, 0x07, (0.66 * 255.0_f32).round() as u8),
        white: Color::WHITE,
        dot_bg: Color::srgb_u8(0x16, 0x18, 0x1c),
    };

    // --- space scale (design uses a denser set; keeps the light-theme keys) ---
    let mut spaces = base_spaces();
    spaces.insert("space.5".into(), 18.0);
    spaces.insert("space.6".into(), 24.0);

    // --- radius scale (values.md § 3) ---
    let mut radii = HashMap::new();
    radii.insert("radius.xs".into(), 2.0);
    radii.insert("radius.sm".into(), 5.0);
    radii.insert("radius.md".into(), 6.0);
    radii.insert("radius.md-2".into(), 7.0);
    radii.insert("radius.lg".into(), 8.0);
    radii.insert("radius.lg-2".into(), 9.0);
    radii.insert("radius.xl".into(), 10.0);
    radii.insert("radius.xl-2".into(), 11.0);
    radii.insert("radius.2xl".into(), 12.0);
    radii.insert("radius.3xl".into(), 14.0);
    radii.insert("radius.pill".into(), 99.0);

    Theme {
        palette,
        accent: DARK_ACCENT_DEFAULT,
        mode: PaletteMode::Normal,
        spaces,
        radii,
    }
}

/// Runtime accent-swap message. Carries the new base accent color; the
/// [`apply_set_accent`] system reads it and sets `Theme::accent`, so the
/// existing `theme.is_changed()` re-extract re-resolves every accent-bearing
/// paint (the live ramp derives from `accent` at resolve time). Producers (e.g.
/// the gallery's accent swatches) `write` it; the swap system reads it.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetAccent(pub Color);

/// Reads [`SetAccent`] messages and mutates the global [`Theme`] resource by
/// setting `Theme::accent` — the live accent ramp ([`ColorToken::Accent`] /
/// `AccentLighter` / `AccentSoft` / `AccentGlow`) derives from it at resolve
/// time, while the fixed swatches and every other token stay put.
///
/// Taking `ResMut<Theme>` and writing through it marks the resource changed, so
/// the render extract's `theme.is_changed()` guard re-resolves all paint. If
/// several `SetAccent` messages arrive in one frame the **last** wins (the
/// accent is a single global value). Only touches the theme when at least one
/// message is present, to avoid spuriously marking it changed.
pub fn apply_set_accent(mut messages: MessageReader<SetAccent>, theme: Option<ResMut<Theme>>) {
    let Some(SetAccent(accent)) = messages.read().last().copied() else {
        return;
    };
    let Some(mut theme) = theme else {
        return;
    };
    theme.accent = accent;
}

/// v1 **stub** forced-colors (high-contrast) theme: a [`Theme`] in
/// [`PaletteMode::ForcedColors`], where [`ThemeContract::resolve`] maps every
/// semantic token to a CSS system color (color-and-forced-colors.md § 3.1 — the
/// hard v1 prerequisite), so no forced-colors paint token resolves to magenta.
///
/// The carried `palette`/`accent` are unused under forced mode (resolution goes
/// through [`system_color_value`]); we reuse the dark palette rather than invent
/// a placeholder. `spaces`/`radii` are left empty, matching the v1 stub's prior
/// shape (the swap replaced the whole theme). The system-color VALUES are owned
/// by `buiy-theme-tokens-design`; the role mapping is the contract.
pub fn forced_colors_theme() -> Theme {
    Theme {
        mode: PaletteMode::ForcedColors,
        spaces: HashMap::new(),
        radii: HashMap::new(),
        ..default_dark_theme()
    }
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
    fn light_theme_preserves_original_tokens() {
        // The 9 originally-authored light tokens must remain byte-identical; the
        // family-derived completeness defaults just need to resolve (finite).
        let t = default_light_theme();
        assert_eq!(t.resolve(ColorToken::SurfacePrimary), Color::WHITE);
        assert_eq!(
            t.resolve(ColorToken::SurfaceSecondary),
            Color::srgb(0.96, 0.96, 0.96)
        );
        assert_eq!(
            t.resolve(ColorToken::TextPrimary),
            Color::srgb(0.10, 0.10, 0.10)
        );
        assert_eq!(
            t.resolve(ColorToken::TextSecondary),
            Color::srgb(0.40, 0.40, 0.40)
        );
        assert_eq!(t.resolve(ColorToken::Accent), Color::srgb(0.20, 0.45, 0.95));
        assert_eq!(
            t.resolve(ColorToken::FocusRing),
            Color::srgb(0.20, 0.45, 0.95)
        );
        assert_eq!(
            t.resolve(ColorToken::SelectionBg),
            Color::srgb(0.20, 0.45, 0.95)
        );
        assert_eq!(t.resolve(ColorToken::SelectionFg), Color::WHITE);
        assert_eq!(
            t.resolve(ColorToken::TextPlaceholder),
            Color::srgb(0.55, 0.55, 0.55)
        );
        // A family-derived default resolves to a real (non-transparent) color.
        assert_ne!(t.resolve(ColorToken::SurfaceCard), Color::NONE);
        // Fixed swatches are shared literals across themes.
        assert_eq!(t.resolve(ColorToken::AccentBlue), ACCENT_BLUE);
    }

    #[test]
    fn dark_theme_resolves_sample_tokens() {
        // A representative sample across each family (values.md § 1.1),
        // byte-identical to the pre-migration HashMap values.
        let dark = default_dark_theme();
        assert_eq!(hex(dark.resolve(ColorToken::SurfaceApp)), "#0b0c0e");
        assert_eq!(hex(dark.resolve(ColorToken::SurfaceCard)), "#16181c");
        assert_eq!(hex(dark.resolve(ColorToken::BorderSubtle)), "#1c1f24");
        assert_eq!(hex(dark.resolve(ColorToken::BorderStrong)), "#2c313a");
        assert_eq!(hex(dark.resolve(ColorToken::TextBright)), "#e7eaef");
        assert_eq!(hex(dark.resolve(ColorToken::TextMuted)), "#868d99");
        assert_eq!(hex(dark.resolve(ColorToken::TextOnAccent)), "#07101f");
        assert_eq!(hex(dark.resolve(ColorToken::AccentGreen)), "#45c07d");
        assert_eq!(hex(dark.resolve(ColorToken::StatusWarn)), "#d7a23f");
        assert_eq!(hex(dark.resolve(ColorToken::StatusError)), "#f0655b");

        // Alpha specials (values.md § 1.1): scrim rgba(4,5,7,.66) + selection.
        let scrim = dark.resolve(ColorToken::Scrim);
        assert_eq!(hex(scrim), "#040507");
        assert_eq!(alpha_u8(scrim), (0.66 * 255.0_f32).round() as u8);
        assert_eq!(
            alpha_u8(dark.resolve(ColorToken::SelectionBg)),
            (0.32 * 255.0_f32).round() as u8
        );

        // Transparent family.
        assert_eq!(dark.resolve(ColorToken::ScrollbarTrack), Color::NONE);
        assert_eq!(dark.resolve(ColorToken::Transparent), Color::NONE);

        // Radius scale (values.md § 3) — unchanged stringly map.
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
    fn dark_theme_resolves_blue_accent_ramp() {
        // default_dark_theme resolves the live ramp for the default blue accent.
        let dark = default_dark_theme();
        assert_eq!(hex(dark.resolve(ColorToken::Accent)), "#5b86f5");
        assert_eq!(hex(dark.resolve(ColorToken::AccentLighter)), "#7fa1f7");
        assert_eq!(hex(dark.resolve(ColorToken::AccentSoft)), "#5b86f5");
        assert_eq!(
            alpha_u8(dark.resolve(ColorToken::AccentSoft)),
            (0.16 * 255.0_f32).round() as u8
        );
        assert_eq!(
            alpha_u8(dark.resolve(ColorToken::AccentGlow)),
            (0.55 * 255.0_f32).round() as u8
        );
    }

    #[test]
    fn set_accent_moves_live_accent_but_not_the_fixed_swatch() {
        // Must-fix #7: `SetAccent` re-themes the live accent ramp while the
        // fixed swatch (`AccentBlue`) keeps its identity. Also records, per
        // frame, whether a downstream `Res<Theme>` reader saw the theme as
        // changed — exactly how the render extract observes the swap edge.
        #[derive(Resource, Default)]
        struct SawThemeChanged(bool);

        fn record_change(theme: Res<Theme>, mut saw: ResMut<SawThemeChanged>) {
            saw.0 = theme.is_changed();
        }

        let mut app = App::new();
        app.add_message::<SetAccent>()
            .insert_resource(default_dark_theme())
            .init_resource::<SawThemeChanged>()
            .add_systems(Update, (apply_set_accent, record_change).chain());

        // Initial state: live accent + swatch are both the default blue.
        app.update();
        {
            let theme = app.world().resource_ref::<Theme>();
            assert_eq!(hex(theme.resolve(ColorToken::Accent)), "#5b86f5");
            assert_eq!(hex(theme.resolve(ColorToken::AccentBlue)), "#5b86f5");
        }

        // A no-message frame must NOT mark the theme changed (early return).
        app.update();
        assert!(
            !app.world().resource::<SawThemeChanged>().0,
            "an empty frame must not mark Theme changed"
        );

        // Swap to green: the live ramp recomputes AND the reader sees it changed.
        app.world_mut()
            .write_message(SetAccent(Color::srgb_u8(0x45, 0xc0, 0x7d)));
        app.update();
        assert!(
            app.world().resource::<SawThemeChanged>().0,
            "SetAccent must mark Theme changed for the downstream reader"
        );

        let theme = app.world().resource_ref::<Theme>();
        // Live accent moved...
        assert_eq!(hex(theme.resolve(ColorToken::Accent)), "#45c07d");
        assert_eq!(hex(theme.resolve(ColorToken::AccentLighter)), "#6ece9a");
        // ...but the fixed swatch did NOT.
        assert_eq!(hex(theme.resolve(ColorToken::AccentBlue)), "#5b86f5");
    }

    #[test]
    fn forced_theme_maps_every_semantic_token_to_a_system_color() {
        // Under forced-colors, semantic tokens resolve to CSS system colors
        // (a spot check across roles); FocusRing → the system Highlight (yellow)
        // reproduces the prior forced `color.focus.ring`.
        let forced = forced_colors_theme();
        assert_eq!(forced.mode, PaletteMode::ForcedColors);
        let canvas = system_color_value(SystemColorKeyword::Canvas);
        let canvas_text = system_color_value(SystemColorKeyword::CanvasText);
        let highlight = system_color_value(SystemColorKeyword::Highlight);
        assert_eq!(forced.resolve(ColorToken::SurfaceCard), canvas);
        assert_eq!(forced.resolve(ColorToken::TextPrimary), canvas_text);
        assert_eq!(forced.resolve(ColorToken::FocusRing), highlight);
        assert_eq!(forced.resolve(ColorToken::SelectionBg), highlight);
        assert_eq!(
            forced.resolve(ColorToken::SelectionFg),
            system_color_value(SystemColorKeyword::HighlightText)
        );
        // currentColor → the high-contrast foreground.
        assert_eq!(forced.resolve(ColorToken::CurrentColor), canvas_text);
        // Transparent stays transparent; Custom passes through.
        assert_eq!(forced.resolve(ColorToken::Transparent), Color::NONE);
        let c = Color::srgb(0.3, 0.6, 0.9);
        assert_eq!(forced.resolve(ColorToken::Custom(c)), c);
    }
}
