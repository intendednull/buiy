//! The `ColorToken` themeable color reference (the layout↔render paint
//! boundary's color seam) + the CSS `SystemColorKeyword` set. Canonical
//! owner per color-and-forced-colors.md § 2.0; the R11 forced-colors phase
//! EXTENDS this file with resolution logic (it must not redefine these
//! enums). `render/components.rs` imports `ColorToken` from here.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.

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
