//! The `ColorToken` themeable color reference (the layout↔render paint
//! boundary's color seam) + the CSS `SystemColorKeyword` set. Canonical
//! owner per color-and-forced-colors.md § 2.0; the R11 forced-colors phase
//! EXTENDS this file with resolution logic (it must not redefine these
//! enums). `render/components.rs` imports `ColorToken` from here.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.

use crate::theme::Theme;
use bevy::prelude::*;
use std::borrow::Cow;

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

/// A themeable color reference, resolved against `Res<Theme>` at extract
/// time (color-and-forced-colors.md § 2.1). Default is `Transparent`,
/// matching `Visual.background_token == ""` and the CSS-initial "no fill"
/// semantics (component-model.md § 2 / § 3).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum ColorToken {
    /// CSS `transparent` (and the empty-token "skip the fill" case). The
    /// default; resolves to `Color::NONE` (alpha 0). Extract skips emitting
    /// a quad for a transparent fill.
    #[default]
    Transparent,
    /// A named theme token, e.g. `Token("color.surface.secondary")`.
    /// Resolves via `Theme::color(name)`; a miss is the magenta sentinel.
    Token(Cow<'static, str>),
    /// CSS `currentColor`: resolves to the inherited text color (v1 fallback
    /// = theme default foreground token). Carrier owned by
    /// `buiy-text-rendering-design`.
    CurrentColor,
    /// A CSS system-color keyword; under forced-colors the only set that
    /// resolves.
    SystemColor(SystemColorKeyword),
}

/// Sentinel color for a missing theme token (magenta = "missing", visible at a
/// glance in screenshots). The accompanying `warn!` surfaces the typo'd token
/// name. A missing token is an author bug that should be *loud*, never silently
/// transparent (§ 2.2). It is an ordinary `Color::srgb`, so it composites
/// through the same linear pipeline as any other color.
pub const MISSING_TOKEN_FALLBACK: Color = Color::srgb(1.0, 0.0, 1.0);

/// Resolve one [`ColorToken`] against the active [`Theme`] to a concrete
/// `Color` (§ 2.0). Called at extract time. Never panics; a miss returns the
/// magenta sentinel and emits a `warn!` naming the token (§ 2.2).
///
/// `CurrentColor` uses the v1 fallback (§ 2.0): the theme default foreground
/// token — `CanvasText` when a `CanvasText` entry exists in the active theme
/// (the forced-colors case), otherwise `color.text.primary`. When
/// `buiy-text-rendering-design` lands the inherited-text-color carrier this
/// rule switches to read it with no change to the variant set.
pub fn resolve_token(token: &ColorToken, theme: &Theme) -> Color {
    match token {
        ColorToken::Transparent => Color::NONE,
        ColorToken::Token(name) => resolve_named(name, theme),
        ColorToken::SystemColor(kw) => resolve_named(kw.token(), theme),
        ColorToken::CurrentColor => {
            // Forced-colors theme carries `CanvasText`; prefer it when present.
            if theme
                .color(SystemColorKeyword::CanvasText.token())
                .is_some()
            {
                resolve_named(SystemColorKeyword::CanvasText.token(), theme)
            } else {
                resolve_named("color.text.primary", theme)
            }
        }
    }
}

fn resolve_named(name: &str, theme: &Theme) -> Color {
    match theme.color(name) {
        Some(c) => c,
        None => {
            tracing::warn!(token = %name, "missing theme color token; falling back to magenta sentinel");
            MISSING_TOKEN_FALLBACK
        }
    }
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

    #[test]
    fn color_token_default_is_transparent() {
        assert_eq!(ColorToken::default(), ColorToken::Transparent);
    }

    #[test]
    fn color_token_from_static_token_round_trips() {
        let t = ColorToken::Token(Cow::Borrowed("color.surface.secondary"));
        assert_eq!(
            t,
            ColorToken::Token(Cow::Borrowed("color.surface.secondary"))
        );
        assert_ne!(t, ColorToken::Transparent);
    }

    #[test]
    fn system_color_keyword_default_is_canvas() {
        assert_eq!(SystemColorKeyword::default(), SystemColorKeyword::Canvas);
    }
}
