//! The `ColorToken` themeable color reference (the layout↔render paint
//! boundary's color seam) + the CSS `SystemColorKeyword` set, plus the
//! [`ThemeContract`] resolution trait. Canonical owner per
//! color-and-forced-colors.md § 2.0; `render/components.rs` imports
//! `ColorToken` from here.
//!
//! Track B made `ColorToken` a **closed enum** resolved through a
//! compiler-enforced [`ThemeContract`]: a typo/missing token is a compile
//! error, never a silent magenta ship (spec § 3.2, F6).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.

use crate::theme::Theme;
use bevy::prelude::*;

/// CSS system-color keyword. Foundation-**F** set (16 keywords). Defined
/// here as a v1 unit prerequisite so `ColorToken::SystemColor(_)` compiles;
/// its *resolution* against the active theme's system-color map is owned by
/// color-and-forced-colors.md § 3 / `buiy-theme-tokens-design`.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SystemColorKeyword {
    #[default]
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
}

impl SystemColorKeyword {
    /// The 16 CSS system-color keywords in declaration order. The forced-colors
    /// stub theme keys its map by these (`theme.rs`), and the gate-#11
    /// token-flow analyzer uses it as the allow-list.
    pub const ALL: [SystemColorKeyword; 16] = [
        Self::Canvas,
        Self::CanvasText,
        Self::LinkText,
        Self::ButtonText,
        Self::ButtonBorder,
        Self::GrayText,
        Self::Highlight,
        Self::HighlightText,
        Self::Field,
        Self::FieldText,
        Self::Mark,
        Self::MarkText,
        Self::SelectedItem,
        Self::SelectedItemText,
        Self::AccentColor,
        Self::AccentColorText,
    ];

    /// The CSS keyword spelling, which is also this keyword's theme-map key
    /// (e.g. `Canvas` → `"Canvas"`). `ColorToken::SystemColor(_)` resolves by
    /// looking this up in the active theme's `colors` map.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Canvas => "Canvas",
            Self::CanvasText => "CanvasText",
            Self::LinkText => "LinkText",
            Self::ButtonText => "ButtonText",
            Self::ButtonBorder => "ButtonBorder",
            Self::GrayText => "GrayText",
            Self::Highlight => "Highlight",
            Self::HighlightText => "HighlightText",
            Self::Field => "Field",
            Self::FieldText => "FieldText",
            Self::Mark => "Mark",
            Self::MarkText => "MarkText",
            Self::SelectedItem => "SelectedItem",
            Self::SelectedItemText => "SelectedItemText",
            Self::AccentColor => "AccentColor",
            Self::AccentColorText => "AccentColorText",
        }
    }
}

/// A themeable color reference — the layout↔render paint boundary's color seam
/// (color-and-forced-colors.md § 2.0). Resolved against the active [`Theme`]
/// via [`ThemeContract::resolve`] at extract time.
///
/// This is a **closed vocabulary**: three non-semantic kinds (`Transparent` —
/// the `#[default]`, `CurrentColor`, `SystemColor`), a
/// [`Custom`](ColorToken::Custom) escape hatch for genuinely-dynamic / test
/// colors, and one variant per semantic design token (`values.md` § 1). Because
/// the vocabulary is closed and [`Theme`] resolves it through an exhaustive
/// match, a missing/typo'd token is a **compile error**, never a silent magenta
/// ship (Track B, spec § 3.2 F6). There is deliberately no `caret` /
/// `preedit-underline` variant — those consumers default to `CurrentColor`
/// (CSS `auto`; see [`resolve_caret_color`] / [`resolve_preedit_underline`]).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Default)]
pub enum ColorToken {
    /// CSS `transparent` (the empty-token "skip the fill" case, and the former
    /// `color.surface.transparent`). The default; resolves to `Color::NONE`.
    /// Extract skips emitting a quad for a transparent fill.
    #[default]
    Transparent,
    /// CSS `currentColor`: the inherited text color; the auto default for
    /// text/caret/icon. Resolves to the system `CanvasText` under forced-colors,
    /// else to `color.text.primary` (the v1 foreground fallback).
    CurrentColor,
    /// A CSS system-color keyword. Under forced-colors the whole semantic
    /// vocabulary maps onto this set; it is one of the kinds the gate-#11
    /// analyzer treats as forced-colors-safe.
    SystemColor(SystemColorKeyword),
    /// A genuinely-dynamic or test-only color, carried inline. The escape hatch
    /// that replaced the old stringly `Token(Cow<str>)`; NOT forced-colors-safe
    /// (a concrete color, not a system reference).
    Custom(Color),

    // --- Closed semantic vocabulary (values.md § 1). One variant per design
    //     token; resolved to the active theme's palette by `Theme::resolve`. ---

    // Surface
    SurfaceApp,
    SurfacePrimary,
    SurfaceSecondary,
    SurfaceCard,
    SurfaceRaised,
    SurfaceRaisedAlt,
    SurfaceInset,
    SurfaceChrome,
    SurfaceChromeTranslucent,
    SurfaceDanger,
    SurfaceDangerSoft,
    SurfaceDangerStrong,

    // Text ink ladder
    TextPrimary,
    TextSecondary,
    TextMuted,
    TextDim,
    TextDimmer,
    TextFaint,
    TextBright,
    TextPlaceholder,
    TextDanger,
    TextDangerDim,
    TextOnAccent,

    // Border
    BorderDefault,
    BorderSubtle,
    BorderSubtle2,
    BorderMuted,
    BorderStrong,
    BorderStrong2,
    BorderDanger,

    // Accent — LIVE ramp (derived from the theme's runtime `accent` via
    // `derive_accent_ramp`; moved by `SetAccent`).
    Accent,
    AccentLighter,
    AccentSoft,
    AccentGlow,
    // Accent — FIXED swatches (constant design literals; the selectable accent
    // options — NOT moved by `SetAccent`).
    AccentBlue,
    AccentGreen,
    AccentViolet,
    AccentCoral,

    // Status
    StatusOk,
    StatusWarn,
    StatusError,

    // Shadow (the box-shadow catalog colors incl. alpha; values.md § 2)
    ShadowCard,
    ShadowMenu,
    ShadowModal,
    ShadowSliderThumb,
    ShadowSwitchThumb,
    ShadowDangerButton,

    // Selection (::selection)
    SelectionBg,
    SelectionFg,

    // Scrollbar
    ScrollbarThumb,
    ScrollbarThumbHover,
    ScrollbarTrack,

    // Misc / specials
    FocusRing,
    Scrim,
    White,
    DotBg,
}

impl ColorToken {
    /// Gate-#11: a token is forced-colors-safe iff its **kind** is a
    /// system/neutral reference — a `SystemColor`, `Transparent`,
    /// `CurrentColor`, or [`FocusRing`](ColorToken::FocusRing) — rather than a
    /// concrete semantic color. This is the analyzer's criterion
    /// (color-and-forced-colors.md § 3.1), replacing the old resolve-to-magenta
    /// equality check.
    ///
    /// `FocusRing` is included deliberately: the forced theme maps it to the
    /// system `Highlight` so the ring stays visible under forced-colors
    /// (`theme.rs`, styling-f-tier.md § 2.6), so flagging it would contradict
    /// that contract. Note the resolve-vs-analyzer split: forced *resolution*
    /// maps EVERY semantic token to a system color (a safety net — never
    /// magenta), while this *analyzer* predicate still flags concrete semantic
    /// tokens (a best-practice nudge). The two are deliberately non-equivalent.
    pub fn is_forced_colors_safe(&self) -> bool {
        matches!(
            self,
            ColorToken::SystemColor(_)
                | ColorToken::Transparent
                | ColorToken::CurrentColor
                | ColorToken::FocusRing
        )
    }

    /// A stable debug name for introspection sites that used to read the token
    /// string (e.g. `ColorToken::SurfaceCard.debug_name() == "SurfaceCard"`).
    pub fn debug_name(&self) -> String {
        format!("{self:?}")
    }
}

/// The color-resolution contract: turn a [`ColorToken`] into a concrete
/// `Color` against a palette. Implemented by [`Theme`] — a `ColorToken` is
/// meaningless without a theme to resolve it. The exhaustive match in the impl
/// is what makes a missing token a compile error (Track B, spec § 3.2 F6).
pub trait ThemeContract {
    /// Resolve `token` to its concrete `Color` under this theme.
    fn resolve(&self, token: ColorToken) -> Color;
}

/// Resolve one [`ColorToken`] against the active [`Theme`] to a concrete
/// `Color` (§ 2.0). Called at extract time. A thin shim over
/// [`ThemeContract::resolve`], retained because callers already hold a
/// `&ColorToken`.
pub fn resolve_token(token: &ColorToken, theme: &Theme) -> Color {
    theme.resolve(*token)
}

/// `::selection` background token name (decoration-and-paint § 5.1). Retained
/// as the canonical dotted name for the Wave-2/3 call-site migration; live
/// resolution goes through [`ColorToken::SelectionBg`].
pub const SELECTION_BG_TOKEN: &str = "color.selection.bg";
/// `::selection` foreground (selected-text re-tint) token name (§ 5.2) —
/// [`ColorToken::SelectionFg`].
pub const SELECTION_FG_TOKEN: &str = "color.selection.fg";
/// The opt-in theme caret token name (§ 6.2). Deliberately absent from the
/// vocabulary — `caret-color: auto` (= `CurrentColor`) is the only default.
pub const CARET_COLOR_TOKEN: &str = "color.caret";
/// `::placeholder` foreground token name (§ 7) — [`ColorToken::TextPlaceholder`].
pub const PLACEHOLDER_COLOR_TOKEN: &str = "color.text.placeholder";
/// The focus-ring color token name (styling-f-tier.md § 2.6) —
/// [`ColorToken::FocusRing`], which stays visible under forced-colors.
pub const FOCUS_RING_TOKEN: &str = "color.focus.ring";
/// Preedit (IME composition) underline token name. Like the caret, absent from
/// the vocabulary — the composing text is underlined in its own ink
/// (`CurrentColor`).
pub const PREEDIT_UNDERLINE_TOKEN: &str = "color.text.preedit-underline";

/// `::selection` background (§ 5.1): the theme's
/// [`SelectionBg`](ColorToken::SelectionBg) — which under forced-colors already
/// resolves to the system `Highlight` (the resolve branches on `Theme::mode`).
pub fn resolve_selection_bg(theme: &Theme) -> Color {
    theme.resolve(ColorToken::SelectionBg)
}

/// `::selection` foreground (§ 5.2): the theme's
/// [`SelectionFg`](ColorToken::SelectionFg) — system `HighlightText` under
/// forced-colors. See [`resolve_selection_bg`].
pub fn resolve_selection_fg(theme: &Theme) -> Color {
    theme.resolve(ColorToken::SelectionFg)
}

/// `caret-color` (§ 6.2): an explicit token resolves against the theme;
/// otherwise the caret uses `current` (the entity's resolved foreground —
/// `caret-color: auto`, CSS parity). There is no theme-caret middle tier: the
/// closed vocabulary carries no caret token.
pub fn resolve_caret_color(explicit: Option<&ColorToken>, theme: &Theme, current: Color) -> Color {
    match explicit {
        Some(token) => theme.resolve(*token),
        None => current,
    }
}

/// `preedit-underline` color (editing-and-ime § 6.2; decoration-and-paint § 8):
/// always `current` (the entity's resolved foreground) — the composing text is
/// underlined in its own ink (`currentColor` parity). The closed vocabulary
/// carries no preedit token; `theme` is unused but kept for signature stability
/// with the extract-time caller.
pub fn resolve_preedit_underline(_theme: &Theme, current: Color) -> Color {
    current
}

/// WCAG 2.x relative luminance of a color (sRGB → linear, then the 0.2126 /
/// 0.7152 / 0.0722 weighting). Operates on the sRGB-decoded channels; alpha is
/// ignored (contrast is defined over opaque colors).
fn relative_luminance(color: Color) -> f32 {
    let lin = LinearRgba::from(color);
    0.2126 * lin.red + 0.7152 * lin.green + 0.0722 * lin.blue
}

/// WCAG 2.x contrast ratio between two colors, `(L_lighter + 0.05) /
/// (L_darker + 0.05)`, in `[1.0, 21.0]`. Symmetric in its arguments.
///
/// The single source of truth for WCAG 2 contrast across the workspace: it
/// backs the focus-ring ≥3:1 claim (§ 3.2) on the render side and the gate-#9
/// token-pair contrast lint, which reaches it through the f64 wrapper
/// `buiy_verify::contrast::wcag2_ratio` (`buiy_verify` depends on `buiy_core`,
/// so the dependency points the only way it can). It checks authored token
/// *values* independent of the render path.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::default_dark_theme;

    #[test]
    fn color_token_default_is_transparent() {
        assert_eq!(ColorToken::default(), ColorToken::Transparent);
    }

    #[test]
    fn custom_color_round_trips_and_resolves_to_itself() {
        // The `Custom` escape hatch replaces the old stringly `Token`: it round-
        // trips by value and resolves to itself under any theme.
        let c = Color::srgb(0.1, 0.2, 0.3);
        let t = ColorToken::Custom(c);
        assert_eq!(t, ColorToken::Custom(c));
        assert_ne!(t, ColorToken::Transparent);
        assert_eq!(default_dark_theme().resolve(t), c);
    }

    #[test]
    fn system_color_keyword_default_is_canvas() {
        assert_eq!(SystemColorKeyword::default(), SystemColorKeyword::Canvas);
    }

    #[test]
    fn is_forced_colors_safe_is_a_kind_check() {
        // System/neutral kinds are safe...
        assert!(ColorToken::Transparent.is_forced_colors_safe());
        assert!(ColorToken::CurrentColor.is_forced_colors_safe());
        assert!(ColorToken::SystemColor(SystemColorKeyword::Canvas).is_forced_colors_safe());
        assert!(ColorToken::FocusRing.is_forced_colors_safe());
        // ...concrete semantic colors and `Custom` are not.
        assert!(!ColorToken::SurfaceCard.is_forced_colors_safe());
        assert!(!ColorToken::Accent.is_forced_colors_safe());
        assert!(!ColorToken::SelectionBg.is_forced_colors_safe());
        assert!(!ColorToken::Custom(Color::WHITE).is_forced_colors_safe());
    }

    #[test]
    fn dark_theme_resolves_key_tokens_byte_identical() {
        // Byte-identical parity guard (Track B W1.1). Each RHS is the exact
        // literal moved out of `default_dark_theme`'s former HashMap inserts, so
        // this proves the typed port preserved the gallery palette exactly.
        let t = default_dark_theme();
        assert_eq!(
            t.resolve(ColorToken::SurfaceCard),
            Color::srgb_u8(0x16, 0x18, 0x1c)
        );
        assert_eq!(
            t.resolve(ColorToken::TextPrimary),
            Color::srgb_u8(0xf1, 0xf3, 0xf6)
        );
        assert_eq!(
            t.resolve(ColorToken::BorderStrong),
            Color::srgb_u8(0x2c, 0x31, 0x3a)
        );
        assert_eq!(
            t.resolve(ColorToken::TextOnAccent),
            Color::srgb_u8(0x07, 0x10, 0x1f)
        );
        // Live accent base + fixed swatch (initially the same blue).
        assert_eq!(
            t.resolve(ColorToken::Accent),
            Color::srgb_u8(0x5b, 0x86, 0xf5)
        );
        assert_eq!(
            t.resolve(ColorToken::AccentBlue),
            Color::srgb_u8(0x5b, 0x86, 0xf5)
        );
        // Alpha specials.
        assert_eq!(
            t.resolve(ColorToken::SelectionBg),
            Color::srgba_u8(0x5b, 0x86, 0xf5, (0.32 * 255.0_f32).round() as u8)
        );
        assert_eq!(
            t.resolve(ColorToken::ShadowModal),
            Color::srgba_u8(0x00, 0x00, 0x00, (0.85 * 255.0_f32).round() as u8)
        );
        // Transparent-family.
        assert_eq!(t.resolve(ColorToken::Transparent), Color::NONE);
        assert_eq!(t.resolve(ColorToken::ScrollbarTrack), Color::NONE);
    }
}
