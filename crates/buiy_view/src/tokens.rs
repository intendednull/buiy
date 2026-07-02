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
//! So the view [`Color`] enum is a **typed facade** over the theme's string
//! keys: each variant is pinned to one fixed key (`Accent → "color.accent"`,
//! …), and the reconciler lowers `Color` into the matching
//! `ColorToken::Token(key)`. Storing the *token* (not a resolved literal) is
//! what makes a runtime theme swap re-derive the color at the next extract with
//! no reconcile — and it is the only shape the component model accepts, since
//! [`ColorToken`] has no "concrete `bevy::Color` literal" variant (see the
//! report's spec-deviation note on #8). A key the active theme is missing
//! surfaces as the loud magenta sentinel at extract (`render::color`'s
//! `MISSING_TOKEN_FALLBACK`), never silent transparency.

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

/// Semantic color token — a typed facade over the theme's string-keyed color
/// map (spec §3 #8). Each variant is pinned to one fixed theme key; the
/// reconciler lowers it into the matching [`ColorToken::Token`], which resolves
/// against the live `Theme` at extract (so a theme swap re-derives it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// The accent color (`color.accent`).
    Accent,
    /// The primary surface fill (`color.surface.primary`).
    Surface,
    /// The secondary surface fill (`color.surface.secondary`).
    SurfaceMuted,
    /// The primary text ink (`color.text.primary`).
    Text,
    /// The secondary text ink (`color.text.secondary`).
    TextMuted,
}

impl Color {
    /// Lower to the render color model's typed [`ColorToken`] variant. Each view
    /// `Color` is pinned to one theme token; the token resolves against the live
    /// `Theme` at extract, so a runtime theme swap re-derives the color with no
    /// reconcile. (Track B: `ColorToken` is a closed enum — no stringly key.)
    pub fn to_token(self) -> ColorToken {
        match self {
            Color::Accent => ColorToken::Accent,
            Color::Surface => ColorToken::SurfacePrimary,
            Color::SurfaceMuted => ColorToken::SurfaceSecondary,
            Color::Text => ColorToken::TextPrimary,
            Color::TextMuted => ColorToken::TextSecondary,
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
}

impl Radius {
    /// Resolve to logical pixels.
    pub fn px(self) -> f32 {
        match self {
            Radius::Sm => 4.0,
            Radius::Md => 8.0,
            Radius::Lg => 12.0,
        }
    }

    /// Lower to a uniform four-corner [`Corners`] for the render `Border`.
    pub fn to_corners(self) -> Corners {
        Corners::all(RenderRadius::circular(self.px()))
    }
}
