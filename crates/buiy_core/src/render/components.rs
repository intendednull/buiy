//! Render-owned components (the layout↔render paint boundary).
//!
//! Replaces the temporary `crate::components::Visual`. Each author-set
//! component is a small public-fielded decomposed component deriving
//! `Reflect + Default + Clone + Component`; the computed components
//! (`ClipRect`, `AncestorClip`, `EffectGroup`, `ComputedPaintSkip`) carry
//! leaner derives (no `Reflect`/`Default`) because they are render-prep
//! outputs, never authored or serialized. `ColorToken`/`SystemColorKeyword` live in the sibling
//! `render/color.rs` (color-and-forced-colors.md § 2.0 owns them).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md.

use crate::Length;
use crate::render::color::ColorToken;
use bevy::prelude::*;

/// Solid background fill (v1). Replaces `Visual.background_token`.
/// Absent component == transparent.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 3.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Background {
    /// F: solid fill. `ColorToken::default()` (transparent) == no fill,
    /// matching `Visual.background_token == ""`. The reserved layered/
    /// gradient surface (`layers: Vec<BackgroundLayer>`, C-tier) lands with
    /// the gradient/image fast-follow — not a v1 field.
    pub color: ColorToken,
}

/// Glyph foreground color (v1 text) — the graduated `Visual.foreground_token`
/// reservation the atlas seam hands to the text spec
/// (atlas-and-text-seam.md § 1; glyph-pipeline.md § 7 owns the contract).
///
/// Consumed by `extract_buiy_glyphs` from T4: resolved at extract exactly
/// like `Background` (`render::color::resolve_token`), CPU-linearized, and
/// written **straight-alpha** into `GlyphAlphaInstance.color` — alpha-as-
/// color means a color change re-emits instances; the atlas is never
/// touched. Per-span `LayoutGlyph.color_opt` overrides it per-glyph when
/// rich-text spans land (C-tier).
///
/// Default is `CurrentColor` — the theme default foreground (`CanvasText`
/// under forced-colors, else `color.text.primary`) — NOT the derived
/// `Transparent` default, which would render text invisible.
#[derive(Component, Reflect, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct TextColor(pub ColorToken);

impl Default for TextColor {
    fn default() -> Self {
        Self(ColorToken::CurrentColor)
    }
}

/// Border / outline line style. Reuses the shape of `ColumnRuleStyle`
/// (layout/types.rs) extended with the remaining CSS keywords.
/// `Groove`/`Ridge`/`Inset`/`Outset` are C-tier and render as `Solid`
/// until the bevel shader lands.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

/// One corner's elliptical radii (CSS `border-radius` allows `rx / ry`).
/// Paint `Length`: only the resolvable subset (`Px`/`Percent`/`Cq*`)
/// applies; grid-only `Fr` is warned-and-resolved to `0`px downstream.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Radius {
    pub x: Length,
    pub y: Length,
}

impl Radius {
    /// A square corner (both radii zero) — the canonical "no rounding" value.
    pub const ZERO: Self = Self {
        x: Length::ZERO,
        y: Length::ZERO,
    };

    /// A circular (`x == y`) radius of `px` logical pixels.
    pub const fn circular(px: f32) -> Self {
        Self {
            x: Length::px(px),
            y: Length::px(px),
        }
    }
}

/// Elliptical per-corner radius (x and y radii per corner).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Corners {
    pub top_left: Radius,
    pub top_right: Radius,
    pub bottom_right: Radius,
    pub bottom_left: Radius,
}

impl Corners {
    /// All corners square (the default — matches `Visual.border_radius == 0`).
    pub const ZERO: Self = Self::all(Radius::ZERO);

    /// A uniform radius on all four corners.
    pub const fn all(r: Radius) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }
}

/// One side's paint description. Width is NOT here — it lives in
/// `BoxModel.border` because it affects layout.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, PartialEq, Debug)]
pub struct BorderSide {
    pub color: ColorToken,
    pub style: LineStyle,
}

/// Per-side border paint + elliptical per-corner radius. Replaces
/// `Visual.border_radius`. The border *band's* thickness is the layout
/// input `BoxModel.border`; this component paints into that band.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
    /// Elliptical per-corner radius. `Corners::ZERO` (default) == square
    /// corners, matching `Visual.border_radius == 0.0`. A uniform radius is
    /// `Corners::all(Radius::circular(px))`.
    pub radius: Corners,
}

/// One shadow term. Painted by the shadow primitive (architecture pillar 2).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 5.
#[derive(Reflect, Default, Clone, PartialEq, Debug)]
pub struct Shadow {
    pub color: ColorToken,
    pub offset_x: Length,
    pub offset_y: Length,
    /// CSS blur radius (>= 0).
    pub blur: Length,
    /// Grows (+) / shrinks (-) the shadow shape.
    pub spread: Length,
    /// `false` = outset (default), `true` = inner shadow.
    pub inset: bool,
}

/// Ordered box-shadow list. Index 0 paints on top (CSS paint order: first
/// shadow is frontmost). Empty / absent == no shadow.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 5.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BoxShadow(pub Vec<Shadow>);

/// Group opacity in `[0.0, 1.0]`. `1.0` (default) is a no-op. A value
/// `< 1.0` forms an `EffectGroup` (off-screen composite boundary) AND a
/// stacking context (layout sub-pass 6f, the trigger-5 clause — one shared
/// predicate, `render::effect`). Absent == opaque.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 6.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Opacity(pub f32);

impl Default for Opacity {
    fn default() -> Self {
        Opacity(1.0)
    }
}

/// Focus / selection outline, painted OUTSIDE the border box and never
/// clipped by the element's own `ClipRect` (the render pass uses the
/// companion `AncestorClip`). Absent == no outline.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 7.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Outline {
    pub color: ColorToken,
    pub style: LineStyle,
    pub width: Length,
    /// Gap between the border box and the outline. Positive pushes the
    /// outline outward (CSS `outline-offset`); negative draws it inset.
    pub offset: Length,
}

/// Angle in radians. Minimal v1 stub so `FilterFn::HueRotate(Angle)`
/// compiles. The full CSS angle-unit family (deg/grad/turn, C-tier) lands
/// with the units fast-follow, which may re-home this type.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Reflect, Clone, Copy, PartialEq, Debug)]
pub struct Angle(pub f32);

/// Reserved filter-function value. Shapes ship now so authors can write
/// filter-aware code that compiles against v1; evaluation is deferred.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Reflect, Clone, PartialEq, Debug)]
pub enum FilterFn {
    Blur(Length),
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    Invert(f32),
    Opacity(f32),
    Saturate(f32),
    Sepia(f32),
    HueRotate(Angle),
    DropShadow(Shadow),
}

/// C (reserved). Filter function list. Non-empty forms an `EffectGroup`
/// AND a stacking context (layout sub-pass 6f, the trigger-5 clause); the
/// filter shaders are deferred. Empty / absent == no filter.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Filter(pub Vec<FilterFn>);

/// C (reserved). Backdrop filter list — samples what is BEHIND the element.
/// Forms an `EffectGroup` (compositor holds a backdrop copy). Buiy treats
/// `backdrop-filter` as an effect-group former ONLY (it does NOT form a
/// stacking context, so layout 6f does not read it). Backdrop-sampling
/// shader deferred.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BackdropFilter(pub Vec<FilterFn>);

/// C (reserved). Blend mode against the backdrop. Any value other than
/// `Normal` forms an `EffectGroup` AND a stacking context (layout sub-pass
/// 6f, the trigger-5 clause); the blend shader is deferred. `Normal`
/// (default) is a no-op.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum MixBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/// CSS `visibility`. `Hidden` skips paint for this entity's subtree but
/// keeps its layout box and a11y presence (unlike `Display::None`).
/// `Collapse` is a deferred marker (table-row / flex-item collapse) —
/// named only in v1. Deliberately NOT `bevy::prelude::Visibility` (which
/// has different variants/semantics).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 12.1.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Component, Default)]
pub enum CssVisibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

/// Zero-field marker placed by LAYOUT on entities whose
/// `Containment.content_visibility == Auto` subtree is currently off-screen.
/// Render skips paint for an `OffscreenAuto` subtree: the `write_paint_skip`
/// render-prep pass resolves the marker subtree-scoped into
/// [`ComputedPaintSkip`], which extract consumes per entity. Layout-
/// written, render-read; NOT registered by this spec's render plugin
/// (layout owns its registration — README § 3.1). Defined here only so
/// the render-prep pass has the type to read.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 12.2.
#[derive(Component, Clone, Copy, Debug)]
pub struct OffscreenAuto;

/// Computed clip AABB in logical px. Written by the `WriteClipRects`
/// render-prep pass (a later phase) and read by render (scissor) and
/// picking. NOT author-set or serialized — hence the leaner derives (no
/// `Reflect`/`Default`). Absent ClipRect ⇔ no ancestor clips this entity ⇒
/// render applies no scissor.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 9
/// (type fields + accumulation algorithm owned by clip-and-transform.md § A.2).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    pub min: Vec2,
    pub max: Vec2,
}

/// Companion clip AABB holding the intersection of **ancestor** clip boxes
/// only (without the own-box step). Read by render for `Outline` painting
/// so a focus ring is cropped by ancestor clips but not by the element's
/// own clip. Written by `WriteClipRects` (a later phase). A plain `min`/`max`
/// struct (a DISTINCT type from `ClipRect`, NOT a newtype wrapper) per spec
/// clip-and-transform.md § A.2 + component-model.md § 13. NOT author-set.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 7 / § 9.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AncestorClip {
    pub min: Vec2,
    pub max: Vec2,
}

/// C (reserved). Rounded-clip corners — the sibling carrier for
/// rounded-corner clipping, not built in v1. The rounded-rect / `clip-path`
/// cases live here, NOT as a field on `ClipRect`.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 9.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct ClipRadius {
    pub corners: Corners,
}

bitflags::bitflags! {
    /// Which effect(s) caused an entity to form an off-screen compositing
    /// boundary. One entity can carry several at once (opacity<1 AND isolate).
    ///
    /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct EffectReason: u8 {
        /// v1: carried (group opacity).
        const OPACITY         = 1;
        /// v1: carried (`isolation: isolate`).
        const ISOLATION       = 2;
        /// Reserved: marks the group, no shader in v1.
        const FILTER          = 4;
        /// Reserved: marks the group, needs backdrop sample.
        const BACKDROP_FILTER = 8;
        /// Reserved: marks the group, no shader in v1.
        const MIX_BLEND       = 16;
    }
}

/// This entity establishes an off-screen compositing boundary. Written by
/// the render-prep pass that detects an effect-group former (a later phase;
/// canonical predicate owned by effect-compositor.md § 1), removed when none
/// holds. Read by the compositor to choose the composite op without
/// re-querying the five effect components. NOT author-set; NO `Default` (an
/// `EffectGroup` only exists when at least one reason holds). Absence == no
/// group.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
#[derive(Component, Clone, Copy, Debug)]
pub struct EffectGroup {
    /// The OR of every reason that formed this group.
    pub reason: EffectReason,
}

/// Why the forward paint walk skips an entity (paint-order-and-top-layer.md
/// § 5). `Display::None` is NOT a variant: such entities never reach extract
/// (no `ResolvedLayout`, absent from `painters_z`), so there is nothing to
/// skip — the absence IS the skip. `content-visibility: hidden` is likewise NOT
/// a variant: § 5.2 keeps the Hidden entity's own box painting and prunes its
/// descendants layout-side (they never enter `painters_z`), so render inherits
/// the prune for free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// `CssVisibility::Hidden` — render-owned paint-skip, keep the box (§ 5.4).
    CssHidden,
    /// Off-screen `content-visibility: auto` (the `OffscreenAuto` marker, § 5.3).
    OffscreenAuto,
}

/// Computed subtree paint-skip marker. Written by the `write_paint_skip`
/// render-prep pass onto a `CssVisibility::Hidden` / `OffscreenAuto` entity
/// AND every descendant in its subtree (the subtree-scoped paint skip of
/// paint-order-and-top-layer.md § 5.3 / § 5.4), removed when the entity is no
/// longer inside a suppressed subtree. Extract reads it as the SINGLE skip
/// source: presence ⇒ emit no primitives for this entity. NOT author-set or
/// serialized — hence the leaner derives (no `Reflect`/`Default`), matching
/// the computed `ClipRect`/`AncestorClip`/`EffectGroup`. v1 semantics are a
/// blanket subtree drop — no `visibility: visible` override until a
/// visibility cascade exists.
///
/// Design: docs/specs/2026-06-03-buiy-render-pipeline-design/
/// 2026-06-06-render-subtree-visibility-suppression-design.md (Option A).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComputedPaintSkip {
    /// Why this entity is paint-skipped: its own skip input when it has one
    /// (`node_skip_reason` precedence), else the nearest suppressing
    /// ancestor's reason.
    pub reason: SkipReason,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_default_is_transparent() {
        assert_eq!(Background::default().color, ColorToken::Transparent);
    }

    #[test]
    fn line_style_default_is_none() {
        assert_eq!(LineStyle::default(), LineStyle::None);
    }

    #[test]
    fn radius_default_is_zero_zero() {
        let r = Radius::default();
        assert_eq!(r.x, Length::ZERO);
        assert_eq!(r.y, Length::ZERO);
    }

    #[test]
    fn corners_zero_is_all_zero() {
        let c = Corners::ZERO;
        assert_eq!(c.top_left, Radius::default());
        assert_eq!(c.bottom_right, Radius::default());
        assert_eq!(Corners::default(), Corners::ZERO);
    }

    #[test]
    fn corners_all_sets_every_corner() {
        let r = Radius {
            x: Length::px(6.0),
            y: Length::px(6.0),
        };
        let c = Corners::all(r);
        assert_eq!(c.top_left, r);
        assert_eq!(c.top_right, r);
        assert_eq!(c.bottom_right, r);
        assert_eq!(c.bottom_left, r);
    }

    #[test]
    fn radius_circular_sets_x_and_y_equal() {
        let r = Radius::circular(6.0);
        assert_eq!(r.x, Length::px(6.0));
        assert_eq!(r.y, Length::px(6.0));
    }

    #[test]
    fn border_side_default_is_transparent_none() {
        let s = BorderSide::default();
        assert_eq!(s.color, ColorToken::Transparent);
        assert_eq!(s.style, LineStyle::None);
    }

    #[test]
    fn border_default_is_square_no_stroke() {
        let b = Border::default();
        assert_eq!(b.radius, Corners::ZERO);
        assert_eq!(b.top, BorderSide::default());
        assert_eq!(b.left.style, LineStyle::None);
    }

    #[test]
    fn box_shadow_default_is_empty() {
        assert!(BoxShadow::default().0.is_empty());
    }

    #[test]
    fn shadow_default_is_transparent_zero_outset() {
        let s = Shadow::default();
        assert_eq!(s.color, ColorToken::Transparent);
        assert_eq!(s.offset_x, Length::ZERO);
        assert_eq!(s.offset_y, Length::ZERO);
        assert_eq!(s.blur, Length::ZERO);
        assert_eq!(s.spread, Length::ZERO);
        assert!(!s.inset);
    }

    #[test]
    fn box_shadow_preserves_list_order() {
        let front = Shadow {
            offset_x: Length::px(1.0),
            ..Default::default()
        };
        let back = Shadow {
            offset_x: Length::px(2.0),
            ..Default::default()
        };
        let bs = BoxShadow(vec![front, back]);
        assert_eq!(bs.0[0].offset_x, Length::px(1.0));
        assert_eq!(bs.0[1].offset_x, Length::px(2.0));
    }

    #[test]
    fn opacity_default_is_one_not_zero() {
        // The whole reason Opacity has a manual Default: a derived Default
        // over f32 would be 0.0 (fully transparent), the wrong CSS initial.
        assert_eq!(Opacity::default().0, 1.0);
    }

    #[test]
    fn opacity_is_copy() {
        let a = Opacity(0.5);
        let b = a;
        assert_eq!(a.0, b.0);
    }

    #[test]
    fn outline_default_is_transparent_none_zero() {
        let o = Outline::default();
        assert_eq!(o.color, ColorToken::Transparent);
        assert_eq!(o.style, LineStyle::None);
        assert_eq!(o.width, Length::ZERO);
        assert_eq!(o.offset, Length::ZERO);
    }

    #[test]
    fn angle_holds_radians() {
        assert_eq!(Angle(std::f32::consts::PI).0, std::f32::consts::PI);
    }

    #[test]
    fn filter_default_is_empty() {
        assert!(Filter::default().0.is_empty());
    }

    #[test]
    fn backdrop_filter_default_is_empty() {
        assert!(BackdropFilter::default().0.is_empty());
    }

    #[test]
    fn mix_blend_mode_default_is_normal() {
        assert_eq!(MixBlendMode::default(), MixBlendMode::Normal);
    }

    #[test]
    fn filter_fn_blur_carries_length() {
        let f = FilterFn::Blur(Length::px(4.0));
        assert_eq!(f, FilterFn::Blur(Length::px(4.0)));
        assert_ne!(f, FilterFn::Brightness(0.5));
    }

    #[test]
    fn css_visibility_default_is_visible() {
        assert_eq!(CssVisibility::default(), CssVisibility::Visible);
    }

    #[test]
    fn css_visibility_has_hidden_and_collapse() {
        assert_ne!(CssVisibility::Hidden, CssVisibility::Collapse);
        assert_ne!(CssVisibility::Hidden, CssVisibility::Visible);
    }

    #[test]
    fn offscreen_auto_is_zero_field_marker() {
        let _m = OffscreenAuto;
    }

    #[test]
    fn ancestor_clip_holds_min_max() {
        let ac = AncestorClip {
            min: Vec2::ZERO,
            max: Vec2::splat(10.0),
        };
        assert_eq!(ac.max, Vec2::splat(10.0));
    }

    #[test]
    fn effect_reason_bits_are_distinct() {
        assert_ne!(EffectReason::OPACITY, EffectReason::ISOLATION);
        assert_ne!(EffectReason::FILTER, EffectReason::BACKDROP_FILTER);
        assert_ne!(EffectReason::MIX_BLEND, EffectReason::OPACITY);
    }

    #[test]
    fn effect_reason_ors_combine() {
        let r = EffectReason::OPACITY | EffectReason::ISOLATION;
        assert!(r.contains(EffectReason::OPACITY));
        assert!(r.contains(EffectReason::ISOLATION));
        assert!(!r.contains(EffectReason::FILTER));
    }

    #[test]
    fn effect_group_carries_reason() {
        let g = EffectGroup {
            reason: EffectReason::OPACITY | EffectReason::FILTER,
        };
        assert!(g.reason.contains(EffectReason::OPACITY));
        assert!(g.reason.contains(EffectReason::FILTER));
    }

    #[test]
    fn computed_paint_skip_carries_reason_and_compares_by_it() {
        // PartialEq is load-bearing: the write_paint_skip reconcile only
        // issues a structural op when the stored marker differs.
        let a = ComputedPaintSkip {
            reason: SkipReason::CssHidden,
        };
        let b = ComputedPaintSkip {
            reason: SkipReason::OffscreenAuto,
        };
        assert_eq!(
            a,
            ComputedPaintSkip {
                reason: SkipReason::CssHidden
            }
        );
        assert_ne!(a, b);
    }
}
