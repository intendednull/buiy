//! Typed styling tokens (spec §2 / §3 #8) — the whole styling vocabulary the
//! view surface exposes in PR1.
//!
//! Styling is **typed enums resolved at build/patch time**, never stringly keys:
//! a modifier takes a token value, and the reconciler lowers it into a concrete
//! decomposed component. `Space` resolves to logical pixels; `Color` and
//! `Radius` resolve into the render component model.
//!
//! ## `Color` is a facade over the theme's semantic tokens (spec §3 #8)
//!
//! The `Background`/`TextColor` components store a [`ColorToken`], which is
//! itself resolved against `Res<Theme>` at **extract** time (`render::color`).
//! So the view [`Color`] enum is a **typed facade** over the theme's semantic
//! tokens: each variant is pinned to one fixed token (`Accent → ColorToken::Accent`,
//! …), and the reconciler lowers `Color` into the matching typed [`ColorToken`]
//! variant. Storing the *token* (not a resolved literal) is what makes a runtime
//! theme swap re-derive the color at the next extract with no reconcile. Under
//! Track B, [`ColorToken`] is a **closed enum** (no stringly key), so an invalid
//! token cannot be constructed — the old magenta-miss path is gone; a genuinely
//! dynamic color uses `ColorToken::Custom(Color)`.

use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Corners, Radius as RenderRadius};

/// Spacing token (gap / padding). Resolves to logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl Space {
    /// Resolve to logical pixels.
    pub fn px(self) -> f32 {
        match self {
            Space::Xs => 4.0,
            Space::Sm => 8.0,
            Space::Md => 16.0,
            Space::Lg => 24.0,
            Space::Xl => 32.0,
        }
    }
}

/// Semantic color token — a typed facade over the theme's closed [`ColorToken`]
/// vocabulary (spec §3 #8). Each *semantic* variant is pinned to one fixed token;
/// the reconciler lowers it into the matching typed [`ColorToken`] variant, which
/// resolves against the live `Theme` at extract (so a theme swap re-derives it).
///
/// ## Two layers: the protokit-role facade + the [`Color::Custom`] escape (F3)
///
/// The facade names the protokit token *roles* the design ladder speaks — an app
/// writes [`Color::Surface2`] / [`Color::OnAccent`] / [`Color::Positive`] rather
/// than a magic literal, and every one re-themes for dark by construction (the
/// accent moves live via `SetAccent`). For a genuinely one-off design color the
/// facade has no role for, [`Color::Custom`] carries an exact sRGB literal
/// straight to [`ColorToken::Custom`] (bypassing the theme — it does NOT re-theme).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// The accent color (`color.accent`) — swapped live by `SetAccent`, so it
    /// re-themes for dark.
    Accent,
    /// The app canvas / page background (protokit "canvas").
    Canvas,
    /// The primary surface fill (protokit "surface").
    Surface,
    /// The secondary surface fill (protokit "surface-2"). Alias-named
    /// [`Color::SurfaceMuted`] is retained for PR1 callers.
    Surface2,
    /// The secondary surface fill (PR1 name; == [`Color::Surface2`]).
    SurfaceMuted,
    /// A raised card surface (protokit "card").
    Card,
    /// The primary text ink (protokit "ink").
    Text,
    /// The secondary text ink (protokit "ink-2"). Alias-named [`Color::TextMuted`]
    /// is retained for PR1 callers.
    Ink2,
    /// The secondary text ink (PR1 name; == [`Color::Ink2`]).
    TextMuted,
    /// The muted caption ink (protokit "muted" — dimmer than ink-2).
    Muted,
    /// The on-accent foreground (white-on-accent button labels; protokit
    /// "on-accent").
    OnAccent,
    /// A positive / success color (protokit "pos" — a green score).
    Positive,
    /// A warning color (protokit "warn").
    Warn,
    /// A danger / error color (protokit "danger").
    Danger,
    /// An exact sRGB literal (`r, g, b, a` as 0–255), lowered verbatim to
    /// [`ColorToken::Custom`] — the escape for a design color the semantic facade
    /// can't name. `u8` channels keep the facade `Eq`. Does NOT re-theme.
    Custom(u8, u8, u8, u8),
}

impl Color {
    /// An opaque exact sRGB color from 0–255 channels ([`Color::Custom`] with full
    /// alpha) — the common case for a design hex.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Custom(r, g, b, 255)
    }

    /// Lower to the render color model's typed [`ColorToken`] variant. A semantic
    /// variant pins one theme token (resolved against the live `Theme` at extract,
    /// so a theme swap re-derives it); [`Color::Custom`] carries an exact sRGB
    /// literal to [`ColorToken::Custom`]. (Track B: `ColorToken` is a closed enum.)
    pub fn to_token(self) -> ColorToken {
        match self {
            Color::Accent => ColorToken::Accent,
            Color::Canvas => ColorToken::SurfaceApp,
            Color::Surface => ColorToken::SurfacePrimary,
            Color::Surface2 | Color::SurfaceMuted => ColorToken::SurfaceSecondary,
            Color::Card => ColorToken::SurfaceCard,
            Color::Text => ColorToken::TextPrimary,
            Color::Ink2 | Color::TextMuted => ColorToken::TextSecondary,
            Color::Muted => ColorToken::TextMuted,
            Color::OnAccent => ColorToken::TextOnAccent,
            Color::Positive => ColorToken::StatusOk,
            Color::Warn => ColorToken::StatusWarn,
            Color::Danger => ColorToken::StatusError,
            Color::Custom(r, g, b, a) => {
                ColorToken::Custom(bevy::color::Color::srgba_u8(r, g, b, a))
            }
        }
    }
}

/// Corner-radius token. Resolves to logical pixels, lowered into the render
/// `Border`'s [`Corners`].
///
/// (Spec §3 #8 names "`Radius → render::components::Radius`"; the render
/// `Radius` is a *value* type, not a `Component`, so a rounded box is realized
/// by writing the entity's `Border.radius: Corners` — see the report's note.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radius {
    Sm,
    Md,
    Lg,
    /// A large card / panel corner (`--r-xl: 22px`) — rounded cards.
    Xl,
    /// A fully-rounded corner (`--r-full`) — a pill button or a circular avatar
    /// badge. The render clamps it to a half-side, so any wide box becomes a pill
    /// and any square box a circle (Dooduel F3). Resolves to a large fixed px.
    Full,
}

impl Radius {
    /// Resolve to logical pixels.
    pub fn px(self) -> f32 {
        match self {
            Radius::Sm => 4.0,
            Radius::Md => 8.0,
            Radius::Lg => 12.0,
            Radius::Xl => 22.0,
            Radius::Full => 9999.0,
        }
    }

    /// Lower to a uniform four-corner [`Corners`] for the render `Border`.
    pub fn to_corners(self) -> Corners {
        Corners::all(RenderRadius::circular(self.px()))
    }
}

/// Per-corner radius `(tl, tr, br, bl)` in logical px — the [`Corners`] the
/// render `Border` carries, authored directly for the design's asymmetric wobble
/// (F3). Lower via [`corners_from_px`].
pub fn corners_from_px(tl: f32, tr: f32, br: f32, bl: f32) -> Corners {
    Corners {
        top_left: RenderRadius::circular(tl),
        top_right: RenderRadius::circular(tr),
        bottom_right: RenderRadius::circular(br),
        bottom_left: RenderRadius::circular(bl),
    }
}

/// Font-weight token — the variable-font weight axis (F3). Lowered to
/// `buiy_core::text::FontWeight` (a `u16` on the OpenType `wght` scale), which the
/// shaper already threads to cosmic-text. The design uses Caveat 600/700 and
/// Shantell 400–700; unset text renders at the family's default instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    /// 400 — the regular / book instance.
    Regular,
    /// 500 — medium.
    Medium,
    /// 600 — semibold.
    SemiBold,
    /// 700 — bold.
    Bold,
}

impl Weight {
    /// Resolve to the OpenType `wght` value the shaper reads.
    pub fn value(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::Medium => 500,
            Weight::SemiBold => 600,
            Weight::Bold => 700,
        }
    }
}
