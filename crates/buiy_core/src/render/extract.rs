//! The per-view extract mapping, factored into pure functions so the
//! device-independent half (`painters_z` ordering, per-entity record build) is
//! unit-testable on CI runners with no wgpu adapter. Color resolution is
//! delegated to the single canonical resolver
//! [`color::resolve_token`](crate::render::color::resolve_token) — this module
//! holds no second token→`Color` mapping. The paint-skip decision is likewise
//! delegated: the `write_paint_skip` render-prep pass
//! ([`crate::render::visibility`]) resolves the subtree-scoped
//! `CssVisibility::Hidden` / `OffscreenAuto` suppression into the computed
//! [`ComputedPaintSkip`] marker, and extract reads ONLY that marker (single
//! skip source — paint-order-and-top-layer.md § 5.3 / § 5.4). The
//! `extract_buiy_nodes` system (Task 6) is a thin wrapper that calls these.
//!
//! Spec: architecture.md § 1.2/§ 3/§ 4, paint-order-and-top-layer.md § 1/§ 5.

use crate::theme::Theme;
use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use crate::animation::AnimatedBackgroundColor;
use crate::components::{Node, ResolvedLayout};
use crate::layout::{BoxModel, Stacking};
use crate::render::components::{
    AncestorClip, BackdropFilter, Background, BackgroundLayers, Border, BoxShadow, ClipRect,
    ComputedPaintSkip, EffectGroup, EffectReason, FilterFn, Opacity, Outline, QuadAlpha,
};
use crate::render::counters::{RenderWorkCounters, record_node_counts};
use crate::theme::UserPreferences;

/// One extracted effect group's CPU record (effect-compositor.md § 1.1). Emitted
/// alongside the flat node list by [`extract_buiy_nodes`]; the prepare pass turns
/// it into a `PreparedEffectGroup` (painted-bounds → bucket_extent → target). The
/// index of a record in the per-view `Vec<EffectGroupExtract>` is the value a
/// node's [`ExtractedNode::group`] holds.
///
/// `bounds` are logical-px, already folded through `GlobalTransform` by the
/// caller (the union of the group entity's own box and every descendant box the
/// group encloses — the v1 painted-bounds input; ink-expansion terms are an
/// upstream tier, not added here). `parent` is the index of the nearest
/// ENCLOSING group, or `None` for a root group (composites into the window).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectGroupExtract {
    /// The main-world group entity (the `EffectGroup` former).
    pub entity: Entity,
    /// Index of the enclosing group's record, or `None` if this group is not
    /// nested inside another effect group (composites into the window target).
    pub parent: Option<usize>,
    /// Group opacity in `[0,1]` (the `Opacity` value; `1.0` if the group formed
    /// for a non-opacity reason only). Applied once at the parent composite.
    pub opacity: f32,
    /// The OR of every reason that formed this group (from the `EffectGroup`
    /// marker). Drives the `plan_allocation` degradation ranking.
    pub reason: EffectReason,
    /// Logical-px painted bounds: the union of the group entity's own box and
    /// every box it transitively encloses, folded through `GlobalTransform`.
    pub bounds: Rect,
    /// Backdrop-blur radius in LOGICAL px (parity Wave B4): the first
    /// `FilterFn::Blur(Px)` in the former's `BackdropFilter`, or `None` if this
    /// group is not a backdrop-filter former (or carries no px blur). The blur
    /// samples the painted window backdrop within the FORMER'S OWN box (NOT the
    /// transitive `bounds`, which includes descendants painting over the blur) —
    /// see [`Self::backdrop_box`].
    pub backdrop_blur_px: Option<f32>,
    /// The backdrop-filter former's OWN box in logical px (the rect the blur is
    /// sampled + blitted over), or `None` if not a backdrop-filter former.
    /// Distinct from `bounds` (which grows to the descendant union): the blur
    /// region is the element's own box, not its content's bounds.
    pub backdrop_box: Option<Rect>,
}

/// Resolve a [`BackdropFilter`] list to its blur radius in logical px (parity
/// Wave B4): the first `FilterFn::Blur(Length::Px)` term. Non-px lengths
/// (`%`/`cq*`) and the other filter functions (brightness/saturate/…) resolve
/// to no blur in v1 — the gallery's two uses are `blur(2px)`/`blur(6px)`, both
/// px; the wider filter family is a documented follow-up. `None` == no blur.
pub fn backdrop_blur_px(filter: &BackdropFilter) -> Option<f32> {
    filter.0.iter().find_map(|f| match f {
        FilterFn::Blur(crate::layout::Length::Px(r)) if *r > 0.0 => Some(*r),
        _ => None,
    })
}

/// Per-view list of extracted effect groups (effect-compositor.md § 1.1),
/// carried as a render-world resource shim alongside [`ExtractedNodesView`]
/// (the v1 carrier shape — the per-view-component flip is the shared view-routing
/// follow-up). `prepare_effect_groups` reads this to size + acquire targets. The
/// vec index is the value `ExtractedNode::group` references.
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedEffectGroups(pub Vec<EffectGroupExtract>);

/// One painted entity's CPU record — the per-frame instance the per-view
/// `ExtractedNodes` (Task 5) holds, keyed by `Entity` so a partial re-extract
/// patches only changed entities (architecture.md § 3.1). v1 carries the
/// solid-fill quad inputs; shadow/border/glyph fields are added by their tier.
///
/// NOT `Copy`: the F-tier `shadows: Vec<ExtractedShadow>` (styling-f-tier.md
/// § 2.1) carries a heap list, so the record is `Clone` only. Most nodes carry
/// no shadow (an empty `Vec` does not allocate), so the assemble-time `clone`
/// is cheap for the common case.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtractedNode {
    /// The source main-world entity (the partial-re-extract key).
    pub entity: Entity,
    /// Painted top-left, in logical px — `GlobalTransform.translation.xy`
    /// (pillar 5: render reads the propagated transform, not
    /// `ResolvedLayout.position`).
    pub position: Vec2,
    /// Box size in logical px, from `ResolvedLayout.size`.
    pub size: Vec2,
    /// Uniform corner radius for the FILL quad, in logical px (`0` == square).
    /// Non-zero ONLY for a **borderless-rounded** node (a `Border.radius` with no
    /// painting side): the fill quad rounds itself through the existing
    /// `PackedInstance.radius` slot. A node WITH a painting border keeps `0` here
    /// (its border band already traces the rounding), so every existing bordered
    /// display-list / GPU golden stays byte-identical — only a rounded-but-
    /// borderless fill is newly rounded (Dooduel F3; closes the "per-node corner
    /// radius is not yet on the extract record" stub `pack_extracted` flagged).
    /// F4b's bordered-rounded fill reuses this SAME field + slot on the disjoint
    /// bordered-node set (the F3→F4b edge, spec §2.3/§6).
    pub radius: f32,
    /// The 2D linear part of `GlobalTransform`'s affine — the box-local →
    /// window-logical basis, as column vectors `[col0, col1]` where
    /// `col0 = [m00, m10]` and `col1 = [m01, m11]`. Applied per-vertex in the
    /// quad/shadow vertex stage about the box-local origin (the corner the
    /// composed matrix maps `0` to), so a rotated/scaled element paints with the
    /// right orientation and size. Identity `[[1,0],[0,1]]` == no rotation/scale
    /// (the byte-identical fast path). Pillar 5: this reads the propagated
    /// `GlobalTransform`, NOT `ResolvedTransform` — the bridge already folded
    /// `ResolvedTransform.matrix` into `Transform` so render == picking by
    /// construction. FIDELITY: faithful for rotation + (non-)uniform scale;
    /// skew / general `TransformMatrix::Matrix` are bounded by the bridge's
    /// TRS-only `Transform::from_matrix` decompose (a lossy shear) — a separate
    /// residual (clip-and-transform.md § B.5).
    pub affine: [[f32; 2]; 2],
    /// Resolved background fill (already theme-resolved; `Color::NONE` ==
    /// transparent, extract emits no quad for it downstream).
    pub color: Color,
    /// The per-primitive clip AABB (own border box ∩ ancestor clips, from
    /// `clip_for_primitive`). `None` == the full-view sentinel: no ancestor
    /// clips this entity, OR it is a top-layer member (always `None`, per
    /// paint-order-and-top-layer.md § 3.2). Downstream (`pack_extracted`)
    /// packs `None` to `[±INFINITY]` so the fragment discard never fires.
    pub clip: Option<ClipRect>,
    /// The enclosing effect group's index into the per-view
    /// [`EffectGroupExtract`] list (effect-compositor.md § 1.1), or `None` for
    /// an in-flow / top-layer node that no `EffectGroup` ancestor encloses. The
    /// index is the NEAREST `EffectGroup` ancestor: a node nested two groups
    /// deep tags the inner group, and that group's record carries its own
    /// `parent` link up the chain. `pack_view` partitions the flat instance blob
    /// into contiguous per-group ranges off this tag (off-screen targets), so a
    /// group member is drawn once into its target, never flat.
    pub group: Option<usize>,
    /// Resolved `Outline` (the focus ring / selection outline), painted OUTSIDE
    /// the border box through the distinct band-pipeline record
    /// [`BorderBandInstance`](crate::render::instance::BorderBandInstance).
    /// `None` == no outline (the byte-stable fast path). The outline is clipped
    /// by the entity's `AncestorClip`, NOT its own box, so a focus ring survives
    /// an `overflow:hidden` ancestor (styling-f-tier.md § 2.4 — C6-a).
    pub outline: Option<ExtractedOutline>,
    /// Resolved per-side `Border` (color + width + per-corner radius), painted
    /// AT the box edge (inside the border box) through the SAME distinct
    /// band-pipeline record [`BorderBandInstance`](crate::render::instance::BorderBandInstance)
    /// the outline rides. `None` == no border band (the byte-stable fast path).
    /// Unlike the outline, the border uses the entity's OWN clip (`ExtractedNode::clip`),
    /// since the band sits inside the border box (styling-f-tier.md § 2.3 — C6-b).
    pub border: Option<ExtractedBorder>,
    /// Resolved, spread/offset-expanded, blur-sigma'd box-shadow terms, in CSS
    /// list order (index 0 frontmost). Each term packs one `(Shadow, layer)`
    /// instance drawn BEHIND the box (styling-f-tier.md § 2.2 — C6-b). Empty ==
    /// no shadow. Suppressed at the PRODUCER when forced-colors is active (the
    /// vec is then empty — § 2.5), and outset-only in v1 (inset warns-once).
    /// Each term carries a corner `radius` (F4b-6): `0.0` terms pack to the
    /// byte-stable SQUARE shadow pipeline, `> 0.0` terms to the distinct ROUNDED
    /// pipeline. A caster's terms are homogeneous (all square OR all rounded, by
    /// its corner radius), so a square caster's shadow blob is byte-identical.
    pub shadows: Vec<ExtractedShadow>,
    /// Resolved background gradient layers (parity Wave B1), in BACK-to-front
    /// draw order (the gradient pipeline draws this list in order, so index 0
    /// here is the bottom-most layer and the last entry paints on top). Each
    /// resolves its stop tokens to linear color + a CPU-precomputed axis. Painted
    /// ABOVE the solid `Background.color` quad, BELOW glyphs/bands. Empty == no
    /// gradient (the byte-stable solid-only path).
    pub gradients: Vec<ExtractedGradient>,
}

/// One resolved `Outline` (styling-f-tier.md § 2.4), built at extract time from
/// the author/framework `Outline` component + the entity's border box +
/// `AncestorClip`. A flat `Copy` record (no token, no `Length`) — every term is
/// pre-resolved to logical px / linear color so the packer
/// ([`pack_outline`](crate::render::instance::pack_outline)) and the band shader
/// are pure consumers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedOutline {
    /// Outer-box top-left, logical px = border-box origin shifted out by
    /// `width + offset` on the top/left.
    pub outer_pos: Vec2,
    /// Outer-box size, logical px = border-box size grown by `2*(width+offset)`.
    pub outer_size: Vec2,
    /// Pre-linearized ring color (uniform — the outline is a single stroke).
    pub color: [f32; 4],
    /// Ring thickness, logical px (`>= 2px` for the focus ring — WCAG 2.4.11).
    pub width: f32,
    /// Per-corner OUTER elliptical radius `(rx, ry) × 4` (TL, TR, BR, BL): the
    /// border radius grown by `(width + offset)` (CSS-faithful outline
    /// rounding). `[0; 8]` == square.
    pub outer_radius: [f32; 8],
    /// Per-corner INNER radius — the outer edge shrunk inward by `width` so the
    /// ring is `width` thick.
    pub inner_radius: [f32; 8],
    /// The OUTLINE clip = the entity's `AncestorClip` (never its own
    /// `ClipRect`); `None` = the full-view sentinel (no ancestor clips it, or a
    /// top-layer member). styling-f-tier.md § 2.4.
    pub clip: Option<ClipRect>,
    /// The 2D affine basis (same source as [`ExtractedNode::affine`]).
    pub affine: [[f32; 2]; 2],
}

/// One resolved per-side `Border` (styling-f-tier.md § 2.3 — C6-b), built at
/// extract time from the render `Border` component (per-side color + style +
/// per-corner radius) and the layout-owned `BoxModel.border` (the per-side
/// WIDTH, a Taffy input). A flat `Copy` record — every term is pre-resolved to
/// logical px / linear color so the packer
/// ([`pack_border`](crate::render::instance::pack_border)) and the band shader
/// are pure consumers.
///
/// Unlike the outline, the border band sits AT the box edge (inside the border
/// box): the outer box is the border box itself, and the band is `width` thick
/// INWARD (`inner_half = outer_half - width`). It uses the entity's OWN clip
/// (`ExtractedNode::clip`), not the `AncestorClip`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedBorder {
    /// Border-box top-left, logical px (the painted origin — `ExtractedNode::position`).
    pub outer_pos: Vec2,
    /// Border-box size, logical px (`ResolvedLayout.size` — the border box).
    pub outer_size: Vec2,
    /// Pre-linearized per-side color (top / right / bottom / left). A side whose
    /// style is `None` / zero width / transparent contributes a transparent
    /// color so the band's per-side selection paints nothing there.
    pub color_top: [f32; 4],
    pub color_right: [f32; 4],
    pub color_bottom: [f32; 4],
    pub color_left: [f32; 4],
    /// Per-side WIDTH `[top, right, bottom, left]`, logical px, from
    /// `BoxModel.border` (the layout-owned Taffy input — § 3.5).
    pub width: [f32; 4],
    /// Per-side line-STYLE stipple flag `[top, right, bottom, left]` (F4b-3):
    /// `0.0` = solid (the byte-stable path — the band draws a continuous ring),
    /// `1.0` = dashed, `2.0` = dotted ([`encode_line_style`]). Every non-dash CSS
    /// style (Solid/Double/Groove/…) encodes `0.0`, so a border with no dashed
    /// side is byte-identical to before.
    pub style: [f32; 4],
    /// Per-corner OUTER elliptical radius `(rx, ry) × 4` (TL, TR, BR, BL),
    /// resolved from `Border.radius` (clamped to the box). `[0; 8]` == square.
    pub outer_radius: [f32; 8],
    /// Per-corner INNER radius — the outer radius shrunk inward by the adjacent
    /// width (clamped `>= 0`, the oracle's load-bearing shrink).
    pub inner_radius: [f32; 8],
    /// The border clip = the entity's OWN clip (own box ∩ ancestors), since the
    /// band sits inside the border box. `None` = the full-view sentinel.
    pub clip: Option<ClipRect>,
    /// The 2D affine basis (same source as [`ExtractedNode::affine`]).
    pub affine: [[f32; 2]; 2],
}

/// One resolved, spread/offset-expanded, blur-sigma'd box-shadow term
/// (styling-f-tier.md § 2.2 — C6-b), built at extract time from one `Shadow`
/// entry + the entity's border box. A flat `Copy` record — every term is
/// pre-resolved to logical px / linear color / blur sigma so the packer
/// ([`pack_shadow`](crate::render::instance::pack_shadow)) and the shadow shader
/// are pure consumers. v1 ships OUTSET shadows only (inset warns-once at the
/// producer); the shadow draws BEHIND the box through the `(Shadow, layer)`
/// bucket.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedShadow {
    /// The spread-and-offset-expanded shadow box top-left, logical px: the
    /// border box translated by `(offset_x, offset_y)` and grown by `spread`.
    pub rect_pos: Vec2,
    /// The expanded shadow box size, logical px (border box grown by
    /// `2*spread`, clamped `>= 0`).
    pub rect_size: Vec2,
    /// Pre-linearized shadow color.
    pub color: [f32; 4],
    /// The EFFECTIVE Gaussian blur sigma in logical px = `blur / 2` (the CSS
    /// blur-radius → sigma factor, § 3.2). The shadow shader reads this as
    /// `@location(5) blur` (the radius slot reinterpreted).
    pub sigma: f32,
    /// The entity's OWN clip (own box ∩ ancestors). `None` = the full-view
    /// sentinel. A shadow is NOT clipped to the caster's own box in CSS, but it
    /// IS cropped by ancestor `overflow:hidden`; v1 carries the entity's
    /// resolved fill clip (own box ∩ ancestors) as the conservative bound —
    /// the exact "ancestor-only, not own-box" shadow clip is a fast-follow
    /// (the outline already proves the `AncestorClip` path; the shadow's common
    /// case is an un-clipped card/button).
    pub clip: Option<ClipRect>,
    /// The 2D affine basis (same source as [`ExtractedNode::affine`]).
    pub affine: [[f32; 2]; 2],
    /// Uniform corner radius in logical px (F4b-6): `0.0` routes this term to the
    /// byte-stable SQUARE shadow pipeline (every pre-F4b shadow golden is a square
    /// caster); `> 0.0` routes it to the distinct ROUNDED-shadow pipeline
    /// (`RoundedShadowInstance`), so the shadow rounds its corners to match a
    /// rounded caster instead of drawing a rectangular blur that pokes past the
    /// box. The value = the caster's uniform corner radius grown by `spread` (CSS:
    /// a shadow's corner radius = border-radius + spread), clamped to
    /// `<= min(half_w, half_h)`. It exists to render, among other cases, the CRISP
    /// zero-blur 3D-press "sticker" edge (spec §2.5.1, §5.f — the decided Option B).
    pub radius: f32,
}

/// One resolved background gradient layer (parity Wave B1), built at extract
/// time from a [`BackgroundLayer`](crate::render::components::BackgroundLayer) +
/// the entity's border box + clip + affine. A flat `Copy` record — every term is
/// pre-resolved to logical px / linear color / a CPU-precomputed gradient axis,
/// so the packer ([`pack_gradient`](crate::render::instance::pack_gradient)) and
/// `gradient.wgsl` are pure consumers (no trig on the GPU).
///
/// **Two-stop fast path.** The design only uses 2-stop linear gradients, so the
/// record carries exactly 2 resolved stop colors + positions. A `Solid` layer is
/// lowered to a degenerate 2-stop gradient (both stops the same color), so the
/// gradient pipeline paints it with no special case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedGradient {
    /// Box top-left, logical px (`ExtractedNode::position`).
    pub rect_pos: Vec2,
    /// Box size, logical px (`ResolvedLayout.size`).
    pub rect_size: Vec2,
    /// Stop 0 (start) pre-linearized RGBA.
    pub color0: [f32; 4],
    /// Stop 1 (end) pre-linearized RGBA.
    pub color1: [f32; 4],
    /// The two stop positions `[pos0, pos1]`, normalized `0..1` along the line.
    pub stops: [f32; 2],
    /// LINEAR: the CPU-precomputed unit gradient axis in y-DOWN fragment space
    /// `[sinθ, -cosθ]`. RADIAL: the **tile size** in logical px (`[tile_w,
    /// tile_h]`) when the gradient repeats per-tile (the dotted-grid, B2), or
    /// `[0, 0]` for a single (non-tiled) radial over the box. (One reused slot —
    /// the linear axis is meaningless for a radial, and a radial has no angle.)
    pub axis: [f32; 2],
    /// The gradient-kind flag ([`GRADIENT_KIND_LINEAR`](crate::render::instance::GRADIENT_KIND_LINEAR)
    /// / [`GRADIENT_KIND_RADIAL`](crate::render::instance::GRADIENT_KIND_RADIAL)).
    pub kind: f32,
    /// LINEAR: the CSS gradient-line length `|W·sinθ| + |H·cosθ|`. RADIAL: the
    /// gradient extent (the dot radius for the tiled dotted-grid; the box
    /// farthest-corner extent `0.5·|size|` for a single radial) — both in
    /// logical px (B2).
    pub line_len: f32,
    /// The entity's OWN clip (own box ∩ ancestors). `None` = full-view sentinel.
    pub clip: Option<ClipRect>,
    /// The 2D affine basis (same source as [`ExtractedNode::affine`]).
    pub affine: [[f32; 2]; 2],
}

/// Resolve one entity's [`BackgroundLayers`] component into the per-layer
/// [`ExtractedGradient`] list (parity Wave B1), in CSS
/// `background-image` paint order (index 0 frontmost, so the list is REVERSED to
/// paint back-to-front: the gradient pipeline draws this list in order, last
/// entry on top). Each layer's token(s) resolve to concrete linear `Color` here,
/// and the gradient axis + line length are precomputed from the box size, so the
/// GPU does no token lookup and no trig.
///
/// - `Linear`: the CSS angle → a y-down unit axis `(sinθ, -cosθ)` (CSS `0deg`
///   points up; the box fragment space is y-down) + line length
///   `|W·sinθ| + |H·cosθ|`. Fewer than 2 stops degenerates to a flat fill of the
///   single (or transparent) stop; more than 2 uses the first + last for the B1
///   2-stop fast path (the design never exceeds 2 — a `warn!`-free clamp, not a
///   silent drop of a case the design needs).
/// - `Solid`: a degenerate 2-stop gradient (both stops the layer color), so the
///   one pipeline paints layered solids too.
/// - `Radial` (B2): carries the radial kind flag + first/last stop, the gradient
///   extent in `line_len` (the explicit `radius` — the dot radius for the
///   dotted-grid — else the box farthest-corner default), and the TILE size in
///   the `axis` slot (`[0,0]` = a single radial over the box; `[w,h]` = repeat
///   per `w×h` cell — the viewport's dotted radial-grid bg, § 3.8). The shader's
///   radial branch consumes these (distance-to-center, per-tile modulo when
///   tiled, smoothstep-AA at `radius`).
///
/// A fully-transparent resolved layer (e.g. `Solid(Transparent)`) contributes no
/// instance (it would paint nothing — mirrors the fill's `Color::NONE` skip).
/// Pure: no ECS / GPU access beyond the borrowed inputs (unit-testable headless).
pub fn resolve_gradients(
    layers: &crate::render::components::BackgroundLayers,
    position: Vec2,
    size: Vec2,
    clip: Option<ClipRect>,
    affine: [[f32; 2]; 2],
    theme: &Theme,
) -> Vec<ExtractedGradient> {
    use crate::render::components::BackgroundLayer;
    use crate::render::instance::{GRADIENT_KIND_LINEAR, GRADIENT_KIND_RADIAL};

    // Resolve one stop token → linear RGBA (the fill/band linearization path).
    let lin = |token: &crate::render::color::ColorToken| -> [f32; 4] {
        let c = crate::render::color::resolve_token(token, theme);
        let l = LinearRgba::from(c);
        [l.red, l.green, l.blue, l.alpha]
    };
    // `true` iff a resolved color is fully transparent (paints nothing).
    let is_transparent = |c: &[f32; 4]| c[3] <= 0.0;

    // Pick the 2-stop fast-path endpoints from a stop list: first + last (the
    // design only uses 2). Empty → both transparent; one → both that stop.
    let endpoints =
        |stops: &[crate::render::components::ColorStop]| -> ([f32; 4], [f32; 4], [f32; 2]) {
            match stops {
                [] => ([0.0; 4], [0.0; 4], [0.0, 1.0]),
                [only] => {
                    let c = lin(&only.color);
                    (c, c, [0.0, 1.0])
                }
                [first, .., last] => (
                    lin(&first.color),
                    lin(&last.color),
                    [first.position, last.position],
                ),
            }
        };

    // CSS-paint order is index 0 frontmost; the pipeline draws the returned list
    // front-to-back (later entries on top), so emit in REVERSE so the last list
    // entry is layer index 0 (the frontmost). The solid `Background.color` is
    // drawn separately, beneath all of these (the quad pass, before gradients).
    let mut out = Vec::new();
    for layer in layers.0.iter().rev() {
        let (color0, color1, stops, kind, axis, line_len) = match layer {
            BackgroundLayer::Solid(token) => {
                let c = lin(token);
                (c, c, [0.0, 1.0], GRADIENT_KIND_LINEAR, [1.0, 0.0], 1.0)
            }
            BackgroundLayer::Linear(g) => {
                let (c0, c1, stops) = endpoints(&g.stops);
                let (axis, line_len) = linear_axis(g.angle_deg, size);
                (c0, c1, stops, GRADIENT_KIND_LINEAR, axis, line_len)
            }
            BackgroundLayer::Radial(g) => {
                let (c0, c1, stops) = endpoints(&g.stops);
                // RADIAL extent (`line_len`): the explicit `radius` (the dot
                // radius for the dotted-grid) when set, else the box
                // farthest-corner default `0.5·|size|`. The `axis` slot carries
                // the TILE size in logical px (`[0,0]` = a single, non-tiled
                // radial over the box) — the shader's radial branch reads it to
                // repeat per `tile×tile` cell (the dotted-grid).
                let line_len = g.radius.unwrap_or(0.5 * size.length());
                let tile = g.tile.map(|t| [t.x, t.y]).unwrap_or([0.0, 0.0]);
                (c0, c1, stops, GRADIENT_KIND_RADIAL, tile, line_len)
            }
        };
        // A gradient whose BOTH stops are transparent paints nothing — skip.
        if is_transparent(&color0) && is_transparent(&color1) {
            continue;
        }
        out.push(ExtractedGradient {
            rect_pos: position,
            rect_size: size,
            color0,
            color1,
            stops,
            axis,
            kind,
            line_len,
            clip,
            affine,
        });
    }
    out
}

/// The CSS linear-gradient axis + line length for an angle (degrees) over a box
/// of `size` logical px. CSS `0deg` points UP and angles go CLOCKWISE, so the
/// y-UP unit axis is `(sinθ, cosθ)`; the box fragment space is y-DOWN, so the
/// returned axis is `(sinθ, -cosθ)` — the direction from the 0%-stop end toward
/// the 100%-stop end. The gradient-line length is the CSS formula
/// `|W·sinθ| + |H·cosθ|` (the projection of the box onto the axis). Pure +
/// trig-only so the GPU never computes it. Split out for unit testing the
/// angle→axis mapping at 0/45/90/135/150°.
pub fn linear_axis(angle_deg: f32, size: Vec2) -> ([f32; 2], f32) {
    let theta = angle_deg.to_radians();
    let (s, c) = theta.sin_cos();
    // y-up axis (sinθ, cosθ) flipped to the box's y-down fragment space.
    let axis = [s, -c];
    let line_len = (size.x * s).abs() + (size.y * c).abs();
    (axis, line_len)
}

/// Resolve a node's painted background fill color, compositing a live
/// [`AnimatedBackgroundColor`] over the static token when one is present.
///
/// **Auto-composite (spec § 2 REFINE).** The design's `background .12s/.15s`
/// transitions (switch/nav/filter tracks) are driven by a
/// [`BackgroundColorTween`](crate::animation::BackgroundColorTween) that writes a
/// resolved `Color` into [`AnimatedBackgroundColor`] each frame. That color
/// CANNOT live in `Background.color` because the parity design locks the token
/// surface (no `ColorToken::Literal`, spec § 8) — a crossfade is intrinsically a
/// resolved-`Color` op. So the animated color rides its own component and this
/// resolver prefers it: present ⇒ paint the interpolated color (auto, for EVERY
/// node — not an opt-in widget special-case the prototype left unconsumed);
/// absent ⇒ resolve the `Background` token (no background ⇒ transparent). A live
/// theme/accent swap still re-resolves the token endpoints through the tween's
/// own re-spawn, and the `theme.is_changed()` re-extract picks up the rest.
///
/// Pure: no ECS / GPU access beyond the borrowed inputs, so it is unit-testable
/// headless. The `animated_bg` arg is `None` on the Tier-2 snapshot path (which
/// has no live tweens) and the per-entity component in the production loop.
pub fn resolve_background_color(
    background: Option<&Background>,
    animated_bg: Option<&AnimatedBackgroundColor>,
    theme: &Theme,
) -> Color {
    if let Some(animated) = animated_bg {
        return animated.0;
    }
    match background {
        Some(bg) => crate::render::color::resolve_token(&bg.color, theme),
        None => Color::NONE,
    }
}

/// Build one [`ExtractedNode`] from the layout box + composed transform + the
/// (optional) background token + the (optional) per-primitive clip AABB. Pure:
/// no GPU, no ECS access beyond the borrowed components. `position` is the
/// `GlobalTransform` translation; `size` is `ResolvedLayout.size`; `color`
/// resolves the `Background` token (absent background == transparent); `clip`
/// is carried verbatim (`None` == full-view sentinel, see [`ExtractedNode::clip`]).
/// The effect-group tag starts `None`; `extract_buiy_nodes` overwrites it from
/// the per-entity nearest-`EffectGroup`-ancestor map after this builds the record
/// (the group membership needs the main-world `ChildOf` chain, not just the
/// borrowed paint inputs).
pub fn extracted_node_for(
    entity: Entity,
    global_transform: &GlobalTransform,
    layout: &ResolvedLayout,
    background: Option<&Background>,
    clip: Option<&ClipRect>,
    theme: &Theme,
) -> ExtractedNode {
    let translation = global_transform.translation();
    // The 2D linear part of the composed affine (glam `Affine3A.matrix3`): xy of
    // the x-axis is col0, xy of the y-axis is col1 (COLUMNS, not rows — a
    // transpose would rotate the wrong way). For a pure rotation/scale the
    // matrix maps box-local `0 -> 0`, so `translation.xy` stays the painted
    // top-left and an identity transform yields the `[[1,0],[0,1]]` fast path.
    let m = global_transform.affine().matrix3;
    let affine = [[m.x_axis.x, m.x_axis.y], [m.y_axis.x, m.y_axis.y]];
    let color = resolve_background_color(background, None, theme);
    ExtractedNode {
        entity,
        position: translation.truncate(),
        size: layout.size,
        // Square by default; `resolve_one` sets it for a borderless-rounded node.
        // This builder (also the Tier-2 snapshot harness) leaves the fill square.
        radius: 0.0,
        color,
        clip: clip.copied(),
        group: None,
        affine,
        // The outline rides a distinct record + clip (`AncestorClip`, not the
        // own box), so it is resolved separately by `extract_buiy_nodes` via
        // `resolve_outline` and assigned post-build — `extracted_node_for`
        // (also called by the Tier-2 snapshot harness) stays outline-free.
        outline: None,
        // The border band + shadow list + gradient layers are resolved
        // separately by `extract_buiy_nodes` (they need `Border`/`BoxModel`/
        // `BoxShadow`/`BackgroundLayers` + the forced-colors flag), so this
        // builder leaves them empty.
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    }
}

/// Resolve one entity's [`ExtractedOutline`] from its `Outline` component, the
/// border box (`position`/`size`, the painted origin + `ResolvedLayout.size`),
/// the entity's `AncestorClip`-derived outline clip, the resolved 2D affine
/// basis, and the active theme. Returns `None` when the outline paints nothing
/// (no style / zero width / a fully-transparent color), so a node with a
/// defaulted `Outline` rides the byte-stable no-band path.
///
/// Geometry (styling-f-tier.md § 2.4): the outer box is the border box grown by
/// `width + offset` on every side; `outer_radius` is the border radius grown by
/// `(width + offset)`; `inner_radius` is the outer radius shrunk by `width`
/// (clamped `>= 0`) so the ring is exactly `width` thick. The clip is the
/// caller-resolved outline clip (the entity's `AncestorClip`, NEVER its own
/// box), so a focus ring survives an `overflow:hidden` ancestor. Pure: no ECS /
/// GPU access beyond the borrowed inputs, so it is unit-testable headless.
pub fn resolve_outline(
    outline: &Outline,
    position: Vec2,
    size: Vec2,
    outline_clip: Option<ClipRect>,
    affine: [[f32; 2]; 2],
    theme: &Theme,
) -> Option<ExtractedOutline> {
    use crate::render::clip::px_or_zero;
    use crate::render::components::LineStyle;

    // No stroke: `style: None` or a non-positive width paints nothing.
    let width = px_or_zero(outline.width);
    if outline.style == LineStyle::None || width <= 0.0 {
        return None;
    }
    let color = crate::render::color::resolve_token(&outline.color, theme);
    if color == Color::NONE {
        return None; // fully transparent ⇒ no band.
    }
    let lin = LinearRgba::from(color);

    let offset = px_or_zero(outline.offset);
    // The ring sits `width + offset` outside the border box on every side.
    let out = width + offset;
    let outer_pos = position - Vec2::splat(out);
    let outer_size = size + Vec2::splat(2.0 * out);

    // No author `Border.radius` is fed in C6-a, so the focus ring is a square
    // ring (the common case for buttons/inputs). Outline rounding off a real
    // `Border.radius` lands when C6-b feeds the border channel; until then the
    // outer/inner radii are square (`0`), which is the correct CSS rendering of
    // an outline on a square box.
    let outer_radius = [0.0f32; 8];
    let inner_radius = [0.0f32; 8];

    Some(ExtractedOutline {
        outer_pos,
        outer_size,
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        width,
        outer_radius,
        inner_radius,
        clip: outline_clip,
        affine,
    })
}

/// Resolve a `Border.radius` ([`Corners`](crate::render::components::Corners)) to
/// the per-corner OUTER elliptical radius array `(rx, ry) × 4` (TL, TR, BR, BL),
/// each `px_or_zero`-resolved and clamped to `<= min(half_w, half_h)` per the
/// CSS overlap rule so a radius can never exceed the box. Shared by
/// [`resolve_border`] (and the future borderless-rounded-fill path). Pure.
fn resolve_corner_radii(corners: &crate::render::components::Corners, size: Vec2) -> [f32; 8] {
    use crate::render::clip::px_or_zero;
    // CSS clamps each corner radius to half the box dimension on its axis.
    let max_r = (size * 0.5).max(Vec2::ZERO);
    let clamp = |r: &crate::render::components::Radius| -> [f32; 2] {
        [
            px_or_zero(r.x).clamp(0.0, max_r.x),
            px_or_zero(r.y).clamp(0.0, max_r.y),
        ]
    };
    let tl = clamp(&corners.top_left);
    let tr = clamp(&corners.top_right);
    let br = clamp(&corners.bottom_right);
    let bl = clamp(&corners.bottom_left);
    [tl[0], tl[1], tr[0], tr[1], br[0], br[1], bl[0], bl[1]]
}

/// The ONE uniform corner radius the FILL quad rounds to for a **borderless-
/// rounded** node (Dooduel F3): the SMALLEST resolved corner radius across all
/// four corners and both axes. Each corner is already clamped to
/// `<= min(half_w, half_h)` by `resolve_corner_radii`, so the min is a single
/// uniform radius that pills a wide box and circles a square one — never the
/// per-axis elliptical "lens". Returns `0.0` for a square (no-radius) `Corners`.
///
/// This is applied ONLY when no border side paints (the caller guards on
/// `ExtractedBorder`-is-`None`): a bordered node's band already traces the
/// rounding, so its fill stays square (radius `0`) until F4b packs the band's
/// inner radius. Pure — unit-testable headless.
pub fn borderless_fill_radius(corners: &crate::render::components::Corners, size: Vec2) -> f32 {
    let radii = resolve_corner_radii(corners, size);
    let uniform = radii.iter().copied().fold(f32::INFINITY, f32::min);
    if uniform.is_finite() && uniform > 0.0 {
        uniform
    } else {
        0.0
    }
}

/// Resolve one entity's [`ExtractedBorder`] from its render `Border` component
/// (per-side color + style + per-corner radius), the layout-owned per-side
/// WIDTH (`BoxModel.border`, a Taffy input — § 3.5), the border box
/// (`position`/`size`), the entity's OWN clip (own box ∩ ancestors — NOT the
/// `AncestorClip` the outline uses, since the band sits inside the border box),
/// the 2D affine basis, and the active theme. Returns `None` when no side paints
/// (every side `None`-style / zero-width / transparent), so a borderless node
/// rides the byte-stable no-band path.
///
/// Geometry (styling-f-tier.md § 2.3): the OUTER box is the border box itself
/// (the band draws AT the edge, inward), so `inner_half = outer_half - width`
/// and `inner_radius = max(outer_radius - adjacent_width, 0)` (the oracle's
/// load-bearing shrink). Per-side: a side whose style is `None`, zero width, or
/// transparent contributes a TRANSPARENT color so the band's per-side selection
/// paints nothing on that edge while the other sides still paint. Pure: no ECS /
/// GPU access beyond the borrowed inputs, so it is unit-testable headless.
#[allow(clippy::too_many_arguments)]
pub fn resolve_border(
    border: &Border,
    border_widths: crate::layout::Edges,
    position: Vec2,
    size: Vec2,
    border_clip: Option<ClipRect>,
    affine: [[f32; 2]; 2],
    theme: &Theme,
) -> Option<ExtractedBorder> {
    use crate::render::clip::px_or_zero;
    use crate::render::components::BorderSide;

    // Resolve one side to (linear color, width, paints, style-flag). A side paints
    // iff its style is not `None`, its width is positive, AND its color is not
    // transparent — otherwise it contributes a transparent color + its width (so
    // the inner hole still shrinks correctly even if the side does not paint a
    // color). The style flag (F4b-3) drives the band's dash stipple: `0.0` solid
    // (the byte-stable ring), `1.0` dashed, `2.0` dotted.
    let side = |s: &BorderSide, width_len: crate::Length| -> ([f32; 4], f32, bool, f32) {
        let w = px_or_zero(width_len).max(0.0);
        if s.style == crate::render::components::LineStyle::None || w <= 0.0 {
            return ([0.0; 4], w, false, 0.0);
        }
        let color = crate::render::color::resolve_token(&s.color, theme);
        if color == Color::NONE {
            return ([0.0; 4], w, false, 0.0);
        }
        let lin = LinearRgba::from(color);
        (
            [lin.red, lin.green, lin.blue, lin.alpha],
            w,
            true,
            encode_line_style(s.style),
        )
    };

    let (c_top, w_top, p_top, s_top) = side(&border.top, border_widths.top);
    let (c_right, w_right, p_right, s_right) = side(&border.right, border_widths.right);
    let (c_bottom, w_bottom, p_bottom, s_bottom) = side(&border.bottom, border_widths.bottom);
    let (c_left, w_left, p_left, s_left) = side(&border.left, border_widths.left);

    // No side paints ⇒ no band (the byte-stable fast path).
    if !(p_top || p_right || p_bottom || p_left) {
        return None;
    }

    let width = [w_top, w_right, w_bottom, w_left];
    let style = [s_top, s_right, s_bottom, s_left];
    let outer_radius = resolve_corner_radii(&border.radius, size);
    // Inner radius shrinks per corner by the adjacent border width (the oracle's
    // load-bearing shrink, `render_border_sdf.rs`): each corner touches two
    // sides; shrink each axis by the side that bounds it. TL touches top+left,
    // TR touches top+right, BR touches bottom+right, BL touches bottom+left.
    let shrink = |outer: [f32; 2], wx: f32, wy: f32| -> [f32; 2] {
        [(outer[0] - wx).max(0.0), (outer[1] - wy).max(0.0)]
    };
    let or = &outer_radius;
    let tl = shrink([or[0], or[1]], w_left, w_top);
    let tr = shrink([or[2], or[3]], w_right, w_top);
    let br = shrink([or[4], or[5]], w_right, w_bottom);
    let bl = shrink([or[6], or[7]], w_left, w_bottom);
    let inner_radius = [tl[0], tl[1], tr[0], tr[1], br[0], br[1], bl[0], bl[1]];

    Some(ExtractedBorder {
        outer_pos: position,
        outer_size: size,
        color_top: c_top,
        color_right: c_right,
        color_bottom: c_bottom,
        color_left: c_left,
        width,
        style,
        outer_radius,
        inner_radius,
        clip: border_clip,
        affine,
    })
}

/// Encode a border [`LineStyle`](crate::render::components::LineStyle) into the
/// band shader's per-side stipple flag (F4b-3): `1.0` = dashed, `2.0` = dotted,
/// and **everything else** (`Solid`/`Double`/`Groove`/`Ridge`/`Inset`/`Outset`/
/// `None`) = `0.0` = a continuous ring — the pre-F4b behavior, so any border with
/// no dashed/dotted side is byte-identical. (The advanced multi-stroke styles
/// `Double`/`Groove`/… are not modeled by the single band; they render solid, as
/// before.)
pub fn encode_line_style(style: crate::render::components::LineStyle) -> f32 {
    use crate::render::components::LineStyle;
    match style {
        LineStyle::Dashed => 1.0,
        LineStyle::Dotted => 2.0,
        _ => 0.0,
    }
}

/// Resolve one entity's [`BoxShadow`] component into the per-term
/// [`ExtractedShadow`] list (styling-f-tier.md § 2.2 — C6-b), in CSS list order
/// (index 0 frontmost). The border box (`position`/`size`) is grown by `spread`
/// and translated by `(offset_x, offset_y)`; the CSS blur radius maps to the
/// Gaussian sigma `blur / 2` (the § 3.2 factor, pinned at the producer). The
/// entity's OWN clip is carried verbatim.
///
/// Forced-colors suppression (§ 2.5): when `forced_colors` is set the list is
/// EMPTY (one branch at the producer) — a shadow-only affordance is then
/// invisible, which the structural-cue guarantee relies on (border / outline /
/// fill survive the forced-colors swap untouched).
///
/// v1 ships OUTSET shadows only: an `inset` term is skipped with a one-time
/// `warn!` (the inset padding-box clip + inner SDF is a deferred fast-follow,
/// § 3.1). A fully-transparent / zero-coverage term contributes nothing. Pure:
/// no ECS / GPU access beyond the borrowed inputs, so it is unit-testable
/// headless.
#[allow(clippy::too_many_arguments)]
pub fn resolve_shadows(
    shadows: &BoxShadow,
    position: Vec2,
    size: Vec2,
    caster_radius: f32,
    clip: Option<ClipRect>,
    affine: [[f32; 2]; 2],
    forced_colors: bool,
    theme: &Theme,
) -> Vec<ExtractedShadow> {
    use crate::render::clip::px_or_zero;

    // Forced-colors: suppress every shadow at the producer (§ 2.5) — structural,
    // not per-widget. Border/Outline/Background survive untouched.
    if forced_colors {
        return Vec::new();
    }

    let mut out = Vec::new();
    for shadow in &shadows.0 {
        if shadow.inset {
            // v1: outset only. Warn ONCE (the layout warn-and-fallback idiom)
            // and skip — the inset padding-box clip + inner SDF is a fast-follow.
            inset_shadow_warn_once();
            continue;
        }
        let color = crate::render::color::resolve_token(&shadow.color, theme);
        if color == Color::NONE {
            continue; // fully transparent ⇒ contributes nothing.
        }
        let lin = LinearRgba::from(color);

        let offset = Vec2::new(px_or_zero(shadow.offset_x), px_or_zero(shadow.offset_y));
        let spread = px_or_zero(shadow.spread);
        // The shadow box = border box ⊕ spread ⊕ offset. Spread grows the box on
        // every side; a spread that would invert the box clamps to zero size.
        let rect_pos = position + offset - Vec2::splat(spread);
        let rect_size = (size + Vec2::splat(2.0 * spread)).max(Vec2::ZERO);
        if rect_size.x <= 0.0 || rect_size.y <= 0.0 {
            continue; // collapsed by a negative spread ⇒ nothing to draw.
        }
        // CSS blur radius → Gaussian sigma (§ 3.2): sigma = blur / 2.
        let sigma = px_or_zero(shadow.blur) * 0.5;
        // Shadow corner radius (F4b-6) = the caster's uniform corner radius grown
        // by `spread` (CSS), clamped to the shadow box. `0.0` for a SQUARE caster
        // ⇒ the byte-stable square pipeline; `> 0.0` routes to the rounded one.
        let radius = if caster_radius > 0.0 {
            let half = 0.5 * rect_size.min_element();
            (caster_radius + spread).clamp(0.0, half)
        } else {
            0.0
        };

        out.push(ExtractedShadow {
            rect_pos,
            rect_size,
            color: [lin.red, lin.green, lin.blue, lin.alpha],
            sigma,
            clip,
            affine,
            radius,
        });
    }
    out
}

/// One-time `warn!` for an unsupported inset box-shadow term (v1 ships outset
/// only — § 3.1). Mirrors layout's warn-and-fallback idiom so a missing inset
/// shadow is diagnosable without spamming the log every frame.
fn inset_shadow_warn_once() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing::warn!(
            "inset box-shadow is not supported in v1 (outset only); the inset \
             term is skipped — styling-f-tier.md § 3.1"
        );
    });
}

/// Per-view CPU instance set — the `Changed`-gated per-frame product of
/// extract, stored as a COMPONENT on the per-view render entity (architecture
/// § 4, R8), NOT a global resource, so each window's set is isolated. v1 writes
/// every `Node` into the PRIMARY view's `ExtractedNodes` (architecture § 4,
/// D2); a second window's view runs `BuiyNode` but receives an empty set until
/// the per-window partition is wired.
///
/// **R5 owns this type; R6 consumes it.** Single carrier — there is no parallel
/// `ExtractedNodes`/`ExtractedNodesResource` wrapper.
/// `Default` is MANUAL so `scale_factor` is `1.0` (a derived `Default` would be
/// `0.0` and divide-by-zero the logical→physical map).
#[derive(Component, Clone, Debug)]
pub struct ExtractedNodes {
    /// In `painters_z` forward order (index 0 bottom-most). Never re-sorted by
    /// render (pillar 1); hit-test order is this reversed (paint-order § 2).
    pub nodes: Vec<ExtractedNode>,
    /// The view's logical (CSS-px) size — used by R6 to build the view uniform.
    pub logical_size: Vec2,
    /// Device pixel ratio (logical→physical). `1.0` until the window scale is
    /// wired; the manual `Default` keeps it `1.0`, never `0.0`.
    pub scale_factor: f32,
}

impl Default for ExtractedNodes {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            logical_size: Vec2::ZERO,
            scale_factor: 1.0,
        }
    }
}

/// Walk `painters_z` front-to-back (paint-order § 1) and emit each painter's
/// record, skipping entities for which `build` returns `None` (the skip rules,
/// § 5). Pure: the caller supplies `build` closing over the extract query so
/// this assembler stays device- and ECS-free and unit-testable. Emission order
/// is exactly `painters_z` index order — never a re-sort (pillar 1). The
/// view-level `logical_size` / `scale_factor` are filled by the system that
/// owns the window (Task 6); this assembler only orders the node list.
pub fn assemble_in_paint_order(
    painters_z: &[Entity],
    mut build: impl FnMut(Entity) -> Option<ExtractedNode>,
) -> ExtractedNodes {
    let mut nodes = Vec::with_capacity(painters_z.len());
    for &painter in painters_z {
        if let Some(node) = build(painter) {
            nodes.push(node);
        }
    }
    ExtractedNodes {
        nodes,
        ..Default::default()
    }
}

/// Assemble a whole stacking-context TREE into one flat, paint-ordered node
/// list (paint-order-and-top-layer.md § 1.1). Starting at `root`, this emits the
/// root context's own box, then walks its `painters_z` forward; when a painter
/// is itself a stacking-context root (`painters_z_of` returns `Some`), render
/// **descends into that nested context AT THAT POSITION as a unit** — painting
/// the entire nested context before returning to the parent list — rather than
/// flattening or re-sorting across the boundary.
///
/// This mirrors sub-pass 6f's `painters_of`, which stops descending the moment
/// it hits a child that itself forms a context (`if !forming.contains(&node)`),
/// so each `painters_z` is exactly one context's slice and a nested root appears
/// in its parent's list as a single atomic entry (its descendants live only in
/// its own `painters_z`). A naive flat-concatenation of every context's
/// `painters_z` would instead paint nested descendants at the end of their own
/// context's iteration — the wrong global order. Recursion terminates because
/// the context tree is a finite DAG over distinct entities and a context's
/// `painters_z` never lists the context root itself (§ 1.1).
///
/// Pure: `painters_z_of` resolves an entity to its `StackingContext.painters_z`
/// (or `None` for a non-context painter) and `build` resolves an entity to its
/// record (or `None` to skip, § 5); neither touches the GPU or the ECS beyond
/// the borrowed lookups, so the whole tree walk is unit-testable headless.
pub fn assemble_context_tree<'a>(
    root: Entity,
    painters_z_of: &impl Fn(Entity) -> Option<&'a [Entity]>,
    build: &mut impl FnMut(Entity) -> Option<ExtractedNode>,
    out: &mut Vec<ExtractedNode>,
) {
    let mut order = Vec::new();
    context_tree_paint_order(root, painters_z_of, &mut order);
    out.extend(order.into_iter().filter_map(build));
}

/// Flatten one stacking-context tree into entity paint order: the root's own
/// box first, then its `painters_z` forward, descending into each nested
/// context AS A UNIT at its position (paint-order § 1.1). The entity-order
/// core of [`assemble_context_tree`], shared with the glyph producer
/// (`text::extract_buiy_glyphs`) so the two walks can never diverge.
pub fn context_tree_paint_order<'a>(
    root: Entity,
    painters_z_of: &impl Fn(Entity) -> Option<&'a [Entity]>,
    out: &mut Vec<Entity>,
) {
    // The context root paints its OWN box first (CSS painter's algorithm: the
    // SC root's background/borders sit at the bottom of its context). 6f builds
    // a context's `painters_z` from its DESCENDANTS, excluding the root itself,
    // so the root is emitted here, never via the list below.
    out.push(root);
    let Some(painters) = painters_z_of(root) else {
        return;
    };
    for &painter in painters {
        if painters_z_of(painter).is_some() {
            // Nested SC root: descend as a unit at this position (§ 1.1).
            context_tree_paint_order(painter, painters_z_of, out);
        } else {
            out.push(painter);
        }
    }
}

/// The root context entities of a forming-context map — those no other context
/// lists as a painter (a nested root appears in exactly its parent's list,
/// paint-order § 1.1) — ordered for cross-root paint.
///
/// **Cross-root order:** roots sort by `(rank_of(e), e)`. `rank_of` is the root's
/// [`StackingContext::cross_root_rank`] (set by layout 6f): `0` for an in-flow
/// root (paints first / bottom) and a higher value for a TOP-LAYER root so it
/// paints LAST (topmost). The entity id is the deterministic tiebreak within a
/// rank (replaces archetype order).
///
/// Why rank matters: a top-layer node that is its OWN root (a *parentless*
/// `TopLayer::Modal`/`Popover` tree — e.g. a dialog authored outside the main
/// content tree) cannot escape into a parent root's `painters_z` tail (it has no
/// parent). Without rank it would sort by raw entity id and could paint UNDER the
/// main content root (the M6 modal bug: the dialog painted first, then the whole
/// shell painted over it). Ranking top-layer roots LAST makes a parentless modal
/// paint on top of the entire window — the cross-ROOT companion to the within-root
/// escaped tail (stacking-and-top-layer.md § 4). A parented top-layer node still
/// escapes to its root's tail as before; this only affects the cross-ROOT order.
///
/// `rank_of` resolves a root entity to its `cross_root_rank`. Both producers
/// (render node walk, text glyph walk) and the picking depth derivation pass a
/// lookup over the SAME `StackingContext` query, so all three order roots
/// identically (the "paint == hit-test" invariant).
/// The #2 partial-re-extract FOOTPRINT of a node: the per-entity quantities that fix
/// its slot layout in the instance buffers — whether it emits a solid background quad,
/// and how many gradient / border / shadow / outline band slots it occupies. Two
/// records with the SAME footprint occupy the SAME slots, so a value change between
/// them (color / position / size) can overwrite those slots in place; a footprint
/// CHANGE (a border appears, a shadow is added/removed) changes the slot count and
/// forces a Full re-pack. Position / size / color VALUES are deliberately excluded —
/// those are exactly what a Patch updates.
fn node_footprint(r: &ExtractedNode) -> (bool, usize, bool, usize, bool) {
    (
        r.color != Color::NONE,
        r.gradients.len(),
        r.border.is_some(),
        r.shadows.len(),
        r.outline.is_some(),
    )
}

/// Resolve ONE painted node's full [`ExtractedNode`] record from its paint-query item
/// — the body of the Full build loop, factored out (#2 Stage C3a) so the Patch path can
/// re-resolve a single changed entity through the SAME code (Full and Patch records are
/// byte-identical by construction). Returns `None` for a paint-skipped entity (it emits
/// nothing). Group membership is NOT set here — it is the Full build's group-tag walk's
/// job; a Patch only re-resolves group-free nodes, whose `group` stays `None`.
fn resolve_one(
    item: NodePaintQueryItem,
    theme: &Theme,
    forced_colors: bool,
) -> Option<ExtractedNode> {
    if item.paint_skip.is_some() {
        return None;
    }
    let clip = effective_clip(item.stacking, item.clip_rect, item.ancestor_clip);
    let mut node = extracted_node_for(
        item.entity,
        item.global_transform,
        item.layout,
        item.bg,
        clip.as_ref(),
        theme,
    );
    if item.animated_bg.is_some() {
        node.color = resolve_background_color(item.bg, item.animated_bg, theme);
    }
    // F4b-5: the cheap per-quad FILL alpha multiplier (the particle fade). Absent
    // ⇒ no change (byte-identical); present ⇒ scale the resolved fill alpha, with
    // NO `EffectGroup` (unlike `Opacity`). Fill-quad only — border/glyph/shadow
    // are untouched. Applied after the animated-color composite so it fades the
    // final fill.
    if let Some(qa) = item.quad_alpha {
        use bevy::prelude::Alpha;
        let scaled = (node.color.alpha() * qa.0).clamp(0.0, 1.0);
        node.color = node.color.with_alpha(scaled);
    }
    if let Some(outline) = item.outline {
        let outline_clip = effective_outline_clip(item.stacking, item.ancestor_clip);
        node.outline = resolve_outline(
            outline,
            node.position,
            node.size,
            outline_clip,
            node.affine,
            theme,
        );
    }
    if let Some(border) = item.border {
        let widths = item
            .box_model
            .map(|b| b.border)
            .unwrap_or(crate::layout::Edges::ZERO);
        node.border = resolve_border(
            border,
            widths,
            node.position,
            node.size,
            node.clip,
            node.affine,
            theme,
        );
        // Fill corner radius (the F3 borderless case + F4b's bordered "ears" fix).
        // The `PackedInstance` carries ONE uniform fill radius (the slot is shared
        // with the shadow-sigma / text paths, so it stays a single f32 — no
        // per-corner stride). Two disjoint node sets, so there is no conflict:
        match &node.border {
            // A painting border (F4b-1): round the FILL to the band's uniform
            // INNER radius so no square-corner "ears" poke past a rounded border.
            // For a UNIFORM border every inner corner is equal, so this is EXACT
            // (the fill boundary coincides with the band's inner edge); a square
            // bordered box has `inner_radius == [0; 8]` ⇒ `node.radius` stays 0 ⇒
            // byte-identical to before (only rounded-bordered fixtures shift — the
            // designed `golden_card_bordered` re-bless). A per-corner *wobble*
            // border uses the smallest inner corner — safe (never leaves a
            // background gap; a slight fill ear can remain at a much-larger wobble
            // corner, journaled), since the shared slot is a single f32.
            Some(b) => {
                let uniform = b.inner_radius.iter().copied().fold(f32::INFINITY, f32::min);
                if uniform.is_finite() && uniform > 0.0 {
                    node.radius = uniform;
                }
            }
            // Borderless-rounded fill (F3): a `Border.radius` with NO painting side
            // rounds the FILL quad itself. Guarded `> 0.0` inside
            // `borderless_fill_radius`, so a square/no-radius node stays
            // byte-identical.
            None => {
                node.radius = borderless_fill_radius(&border.radius, node.size);
            }
        }
    }
    if let Some(box_shadow) = item.box_shadow {
        // The caster's uniform OUTER corner radius (F4b-6): a rounded caster's
        // shadow rounds to match it. Read from the `Border.radius` (present for a
        // bordered OR a borderless-rounded node) regardless of whether the border
        // paints; `0.0` (no `Border`) ⇒ the byte-stable square shadow path.
        let caster_radius = item
            .border
            .map(|b| borderless_fill_radius(&b.radius, node.size))
            .unwrap_or(0.0);
        node.shadows = resolve_shadows(
            box_shadow,
            node.position,
            node.size,
            caster_radius,
            node.clip,
            node.affine,
            forced_colors,
            theme,
        );
    }
    if let Some(background_layers) = item.background_layers {
        node.gradients = resolve_gradients(
            background_layers,
            node.position,
            node.size,
            node.clip,
            node.affine,
            theme,
        );
    }
    Some(node)
}

pub fn context_roots(
    // std HashMap (not EntityHashMap): this is a cross-module helper also called by
    // picking/depth.rs + text/extract.rs; `sc_by_entity` is small (one entry per
    // forming stacking context, not per node), so the hasher barely matters here.
    sc_by_entity: &std::collections::HashMap<Entity, &[Entity]>,
    rank_of: impl Fn(Entity) -> u8,
) -> Vec<Entity> {
    let nested: std::collections::HashSet<Entity> = sc_by_entity
        .values()
        .flat_map(|painters| painters.iter().copied())
        .filter(|e| sc_by_entity.contains_key(e))
        .collect();
    let mut roots: Vec<Entity> = sc_by_entity
        .keys()
        .copied()
        .filter(|e| !nested.contains(e))
        .collect();
    roots.sort_unstable_by_key(|&e| (rank_of(e), e));
    roots
}

/// The cross-root paint rank for a top-layer-aware root sort (the value layout 6f
/// stamps into [`StackingContext::cross_root_rank`]): an IN-FLOW context (not a
/// top-layer member) ranks `0` so it paints FIRST (bottom); a top-layer context
/// ranks by its tier so it paints LAST (topmost), tiers ordered the same way as
/// the escaped tail (Fullscreen < Tooltip < Popover < Modal). `1 + paint_rank`
/// keeps every top-layer tier strictly above the `0` in-flow rank while preserving
/// the tier order (`top_layer_paint_rank` ranks Fullscreen 0 < … < Modal 3).
pub fn cross_root_rank(top_layer: crate::layout::TopLayer) -> u8 {
    use crate::layout::TopLayer;
    if top_layer == TopLayer::None {
        0
    } else {
        1 + crate::layout::top_layer_paint_rank(top_layer)
    }
}

use crate::components::StackingContext;
use crate::layout::TopLayer;
use crate::render::clip::clip_for_primitive;
use bevy::render::Extract;
use bevy::window::PrimaryWindow;

/// Resolve one entity's per-primitive clip AABB from its stacking + clip inputs
/// (the decision `extract_buiy_nodes` runs before [`extracted_node_for`]). Pure:
/// `Option<&Stacking>`/`Option<&ClipRect>`/`Option<&AncestorClip>` →
/// `Option<ClipRect>`, no ECS access. A top-layer member (any non-`None`
/// [`TopLayer`]) escapes every ancestor clip and paints over the full view
/// (paint-order-and-top-layer.md § 3.2), so it ALWAYS resolves to the `None`
/// full-view sentinel — even with a (stale) `ClipRect`/`AncestorClip` present.
/// An in-flow member takes its fill clip straight from
/// `clip_for_primitive(false, …)` (own-box ∩ ancestor clips; the
/// `Outline`/`is_outline = true` path is a later tier), which is `None` when
/// nothing clips it.
pub fn effective_clip(
    stacking: Option<&Stacking>,
    clip_rect: Option<&ClipRect>,
    ancestor_clip: Option<&AncestorClip>,
) -> Option<ClipRect> {
    let is_top_layer = stacking.is_some_and(|s| s.top_layer != TopLayer::None);
    if is_top_layer {
        None
    } else {
        clip_for_primitive(false, clip_rect, ancestor_clip)
    }
}

/// The OUTLINE clip for one entity (the focus-ring / selection-outline band).
/// Like [`effective_clip`] this forces the full-view sentinel for a top-layer
/// member, but otherwise takes the `Outline` path of `clip_for_primitive`
/// (`is_outline = true`): the entity's **`AncestorClip`** WITHOUT the own-box
/// step, so a ring drawn outside the border box is cropped by ancestor clips
/// (e.g. a scroll container) yet NOT erased by the element's own
/// `overflow:hidden` box (styling-f-tier.md § 2.4 / clip-and-transform.md § A.2).
pub fn effective_outline_clip(
    stacking: Option<&Stacking>,
    ancestor_clip: Option<&AncestorClip>,
) -> Option<ClipRect> {
    let is_top_layer = stacking.is_some_and(|s| s.top_layer != TopLayer::None);
    if is_top_layer {
        None
    } else {
        clip_for_primitive(true, None, ancestor_clip)
    }
}

/// The per-entity paint read for one node, as a single `#[derive(QueryData)]`
/// projection — the LOGICAL PARTITION of the extract fan, by concern.
///
/// **Why a struct, not a tuple (parity REDESIGN, spec § 2).** The paint fan grew
/// to Bevy's 15-term `QueryData` tuple-arity ceiling; the parity-prototype's
/// mid-flight patch nested the two Wave-B terms in a sub-tuple to stay under it
/// (always flagged as a stopgap). A flat, named projection has **no arity
/// ceiling** — each field is one term, but the derive expands them without the
/// tuple cap — so a future paint input is added by adding a field here, never by
/// re-nesting another sub-tuple. The fields are grouped by the same logical
/// sub-system the REDESIGN names (base / colors / effects / gradients), so the
/// query reads as that partition while keeping `extract_buiy_nodes` ONE system
/// behind ONE all-or-nothing damage gate — the partition is at the data-projection
/// layer, not split into separate systems (a multi-system split would desync the
/// retain-damage gate against the GPU-proven paint-order walk; see the journal
/// follow-up). This mirrors the established `a11y::A11yNodeQuery` fix for the
/// identical ceiling (a11y/mod.rs § "Why a struct, not a tuple").
///
/// Read-only (no `#[query_data(mutable)]`): the derive generates the item type
/// `NodePaintQueryItem<'_>` the build loop destructures.
#[derive(QueryData)]
pub struct NodePaintQuery {
    // --- base: geometry + the single skip source (the REDESIGN `…_base` set:
    // Node membership is the query filter; ResolvedLayout + GlobalTransform are
    // the required terms; the clip inputs resolve the box's scissor rect). ---
    entity: Entity,
    /// Painted top-left + the 2D affine — folded geometry (pillar 5). A
    /// `Display::None` entity has no `ResolvedLayout` and is dropped by the query.
    global_transform: &'static GlobalTransform,
    layout: &'static ResolvedLayout,
    /// The computed subtree paint-skip marker (§ 5.3 / § 5.4) — the SINGLE skip
    /// source. Its presence ⇒ emit nothing for this entity (the whole subtree is
    /// stamped by `write_paint_skip`), so extract never reads `CssVisibility` /
    /// `OffscreenAuto` directly.
    paint_skip: Option<&'static ComputedPaintSkip>,
    /// Clip inputs: the computed per-entity clip AABB + its ancestor-only
    /// companion (consumed by `clip_for_primitive`), and `Stacking` so a
    /// top-layer member is forced to the full-view sentinel (`clip = None`,
    /// paint-order § 3.2).
    clip_rect: Option<&'static ClipRect>,
    ancestor_clip: Option<&'static AncestorClip>,
    stacking: Option<&'static Stacking>,
    // --- colors: the solid + band fills (the REDESIGN `…_colors` set —
    // Background/Border/TextColor; the band thickness is the layout-owned
    // `BoxModel.border`, read alongside). ---
    /// The solid background fill quad (a node with none rides the byte-stable
    /// solid-only path; `BackgroundLayers` paints ABOVE it, in `gradients`).
    bg: Option<&'static Background>,
    /// The crossfaded background color a live [`BackgroundColorTween`](crate::animation::BackgroundColorTween)
    /// writes (the design's `background .12s/.15s` transitions). When present it
    /// AUTO-COMPOSITES over the `bg` token in `resolve_background_color` — every
    /// node, not an opt-in widget special-case (spec § 2 REFINE) — so any node a
    /// tween touches paints the interpolated color for the tween's duration and
    /// falls back to its token `bg` at rest.
    animated_bg: Option<&'static AnimatedBackgroundColor>,
    /// The focus-ring / selection outline band (rides a DISTINCT band record +
    /// the entity's `AncestorClip`, not its own box — styling-f-tier.md § 2.4).
    outline: Option<&'static Outline>,
    /// The per-side border paint (color/style/radius); its band THICKNESS is the
    /// layout-owned `BoxModel.border` (a Taffy input, § 3.5), read below.
    border: Option<&'static Border>,
    box_model: Option<&'static BoxModel>,
    /// The box-shadow list (outset-only, drawn BEHIND the box — § 2.2).
    box_shadow: Option<&'static BoxShadow>,
    // --- effects: the off-screen compositor fan (the REDESIGN `…_effects` set —
    // BackdropFilter/EffectGroup; `Opacity` is the alpha applied at composite). ---
    effect_group: Option<&'static EffectGroup>,
    opacity: Option<&'static Opacity>,
    /// The cheap per-quad FILL alpha multiplier (F4b-5 — the particle fade). It
    /// multiplies straight into the fill color's alpha in [`resolve_one`] and
    /// forms NO `EffectGroup`, so a node with none rides the byte-stable path
    /// unchanged.
    quad_alpha: Option<&'static QuadAlpha>,
    /// B4: the backdrop-filter list (samples the painted window backdrop),
    /// resolved into the group's `backdrop_blur_px` below.
    backdrop_filter: Option<&'static BackdropFilter>,
    // --- gradients: the layered fills (the REDESIGN `…_gradients` set —
    // `BackgroundLayers`, the gradient / layered fills painted above the solid
    // `Background.color` quad). ---
    background_layers: Option<&'static BackgroundLayers>,
}

/// Per-frame, `Changed`-gated extract (architecture.md § 1.2/§ 3/§ 4). Reads
/// the main world's layout + render-owned components through `Extract`, walks
/// the primary view's stacking order, and writes the per-view `ExtractedNodes`.
///
/// v1 resolves every `Node` to the PRIMARY window's view (architecture § 4,
/// D2). The query reads ALL windows (reserved per-window structure) so the
/// partition can be turned on without a query change; v1 still targets primary.
///
/// Extraction is one-directional and read-only (pillar 1): it never mutates the
/// main world, never re-sorts `painters_z`, never re-derives stacking/geometry.
// A Bevy extract system reads many independently-tracked inputs (the paint-input
// fan, the damage probe, the despawn stream, the context tree, the theme, the
// window set); splitting them into a bundle param would obscure, not clarify.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_buiy_nodes(
    // #2 Stage A: the node + effect-group carriers are RETAINED resources written
    // via `ResMut` (not `Commands::insert_resource`), so they persist across frames
    // — the foundation for the keyed partial re-extract (a later Patch stage mutates
    // them in place). On a dirty frame they are overwritten (so `is_changed()` is
    // true → prepare repacks, exactly as before); the idle gate-skip below MUST NOT
    // deref either, keeping `is_changed()` false (the O(0) steady-state contract).
    mut view: ResMut<ExtractedNodesView>,
    mut groups_res: ResMut<ExtractedEffectGroups>,
    // The author-set + handoff fan: Option<&T> for every independently-inserted
    // component (architecture § 1.2 — a non-Option term would silently drop a
    // Node missing that component). Required terms: &ResolvedLayout (a
    // Display::None entity has no ResolvedLayout and is dropped here) and
    // &GlobalTransform (pillar 5).
    // The FULL node set, UN-gated: when the damage gate below decides this frame
    // is dirty, the whole set is re-extracted so a single changed entity does not
    // drop its unchanged siblings (the changed-only replace was the R5 bug —
    // 2026-06-07-render-extract-retain-damage-design.md). The `Changed`-gating is
    // a *whether-to-rebuild* probe (`changed`/`removed`/`theme` below), not a
    // *what-to-include* filter — building the full set is CPU-cheap; the GPU
    // re-upload is what the gate protects (architecture.md § 3.1).
    // The paint fan is the `NodePaintQuery` projection (grouped by concern: base /
    // colors / effects / gradients) — one `#[derive(QueryData)]` struct with no
    // 15-tuple arity ceiling. A new paint input is a new field there, never another
    // nested sub-tuple. `With<Node>` is the membership filter.
    nodes: Extract<Query<NodePaintQuery, With<Node>>>,
    // The parent link, used to resolve each painted node's nearest `EffectGroup`
    // ancestor (the off-screen group it belongs to). Read over ALL `Node`s so the
    // walk can climb through non-group ancestors to the enclosing group.
    child_of: Extract<Query<&bevy::prelude::ChildOf, With<Node>>>,
    // Damage probe: did any paint input change this frame? Entity-only, gated by
    // the architecture.md § 3.1 trigger union. Non-empty ⇒ a paint value or
    // paint-skip flipped (a `Display::None` transition lands here too, via the
    // zero-size `Changed<ResolvedLayout>` re-write — layout never *removes*
    // `ResolvedLayout`).
    changed: Extract<
        Query<
            Entity,
            (
                With<Node>,
                Or<(
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<Background>,
                    // The paint-skip flip, ADD direction: an entity (or its
                    // subtree) newly suppressed gets the computed marker
                    // inserted by `write_paint_skip` the same frame
                    // (`Changed` includes `Added`). The REMOVE direction
                    // (hide→show) cannot fire `Changed` — it rides the
                    // `RemovedComponents<ComputedPaintSkip>` stream below.
                    Changed<ComputedPaintSkip>,
                    Changed<StackingContext>,
                    Changed<Stacking>,
                    Changed<ClipRect>,
                    Changed<AncestorClip>,
                    // Effect-compositor damage (effect-compositor.md § 1.1),
                    // grouped into a NESTED `Or` sub-union. `Or<T>` is itself a
                    // `QueryFilter`, so an inner `Or<(..)>` is one element of the
                    // outer tuple with no change to the flat OR semantics — and it
                    // keeps the outer tuple under Bevy's 15-arity `QueryFilter`
                    // ceiling (the FILTER-side mirror of the `NodePaintQuery`
                    // `QueryData` partition; FAN: this trio tracks the
                    // `effect_group` / `opacity` / `backdrop_filter` fields of the
                    // `nodes` projection). A group forming/dropping
                    // (`EffectGroup`) or an opacity change re-extracts so group
                    // membership + the composite alpha never go stale.
                    Or<(
                        Changed<EffectGroup>,
                        Changed<Opacity>,
                        // F4b-5: a per-quad particle-alpha edit (the `QuadAlpha`
                        // multiplier or its tween) re-extracts so the fill fades.
                        // Its `QuadAlphaTween` re-inserts `QuadAlpha` every advanced
                        // frame (marking it `Changed`), like `AnimatedBackgroundColor`.
                        Changed<QuadAlpha>,
                        // Parity Wave B4: a backdrop-filter EDIT (a runtime
                        // blur-radius change on an EXISTING former).
                        // `Changed<EffectGroup>` only fires when the group FORMS
                        // or DROPS — the marker stores the reason BITSET, which a
                        // 6px→12px radius change does not touch — so without this
                        // term an isolated `BackdropFilter` edit is the one
                        // NodePaintQuery paint input with no matching gate term,
                        // and a radius edit would not re-capture `backdrop_blur_px`
                        // (the value prepare plans the dual-Kawase pyramid from).
                        // NB: today the main-world render-prep passes re-insert
                        // their markers every frame, so an effect-group node
                        // already re-extracts each frame; this term makes the
                        // radius edge PRINCIPLED (and robust if that re-insertion
                        // is ever change-gated).
                        Changed<BackdropFilter>,
                    )>,
                    // C6-a: an outline insert/remove/edit (the focus ring is a
                    // framework-owned `Outline` the ring lowering toggles) must
                    // re-extract so the band appears/vanishes. Kept in lockstep
                    // with the `nodes` fan above. The `AncestorClip` the outline
                    // clips against already rides `Changed<AncestorClip>` above.
                    Changed<Outline>,
                    // C6-b: a border-paint edit (`Border` — color/style/radius)
                    // re-extracts so the band updates. A border-WIDTH edit lives
                    // in `BoxModel.border` (a Taffy input), so it re-runs layout →
                    // `Changed<ResolvedLayout>` above — `Changed<BoxModel>` is NOT
                    // added (it would be redundant, § 3.5). A `BoxShadow` edit
                    // re-extracts so the shadow appears/vanishes/moves.
                    Changed<Border>,
                    Changed<BoxShadow>,
                    // Parity Wave B1: a gradient-layer edit (add/remove/restyle)
                    // re-extracts so the gradient appears/vanishes/recolors. Kept
                    // in lockstep with the `nodes` fan. A live accent swap rides
                    // the `theme.is_changed()` re-extract (the stop tokens
                    // re-resolve), not this per-entity term.
                    Changed<BackgroundLayers>,
                    // Parity § 2 REFINE: a live background-color crossfade. The
                    // `BackgroundColorTween` system re-inserts `AnimatedBackgroundColor`
                    // every frame it advances (marking it `Changed`), so this term
                    // re-extracts the node each frame of the transition — the fill
                    // paints the interpolated color (resolve_background_color), then
                    // the tween completes + removes the component and the node falls
                    // back to its `Background` token on the next change.
                    Changed<AnimatedBackgroundColor>,
                    // FAN: extend the Or-set in lockstep with the `nodes` tuple
                    // (architecture § 3.1 trigger union).
                )>,
            ),
        >,
    >,
    // Entity despawn — the one damage source the `Changed` probe cannot see (a
    // despawn drops every component, emitting no `Changed`). A `Display::None`
    // hide is NOT here (it keeps `ResolvedLayout`); it rides `changed` above.
    mut removed: Extract<RemovedComponents<ResolvedLayout>>,
    // The paint-skip flip, REMOVE direction: a hide→show transition removes
    // the computed marker, which emits no `Changed` — without this stream the
    // re-shown subtree would never re-extract and stay vanished forever (the
    // exact mirror of the despawn stream above; retain-damage design § "The
    // fix", trigger-union extension).
    mut removed_skip: Extract<RemovedComponents<ComputedPaintSkip>>,
    // EVERY forming entity's StackingContext, keyed by its entity. The root
    // context(s) are the ones not listed as a painter in any other context's
    // painters_z; nested-context roots appear as a single atomic entry in their
    // parent's list (paint-order § 1.1). The walk descends the tree from the
    // root(s) — it does NOT flat-concatenate this query in archetype order.
    contexts: Extract<Query<(Entity, &StackingContext)>>,
    theme: Extract<Res<Theme>>,
    // C6-b forced-colors (§ 2.5): the shadow producer emits NO `ExtractedShadow`
    // when `forced_colors` is set (one structural branch). It is NOT a per-entity
    // `Changed` term — the forced-colors swap mutates `Res<Theme>`, so a flip
    // already rides the `theme.is_changed()` re-extract edge below (§ 2.1).
    prefs: Extract<Res<UserPreferences>>,
    // Reserved per-window structure: read ALL windows, not just primary
    // (architecture § 4) — v1 still resolves every Node to the primary view.
    _windows: Extract<Query<(Entity, &Window)>>,
    // The primary window's `Window`: its presence gates extraction (a vanished
    // window clears the carrier, below) AND its resolution fills the per-view
    // `logical_size` / `scale_factor` that build the logical→clip view uniform
    // in prepare (`pack_extracted_nodes`). Reading the resolution here is what
    // the assembler doc ("filled by the system that owns the window") defers to
    // this system: without it `logical_size` stays `Vec2::ZERO` and the view
    // transform divides by zero (`sx = 2/0 = ∞`), collapsing every quad off the
    // GPU — invisible to the CPU-only buffer assertions but fatal on a real
    // adapter (caught by the gate-#2 readback harness).
    primary: Extract<Query<&Window, With<PrimaryWindow>>>,
    // P0b work-unit counters: `node_rebuilds` (0 idle / 1 rebuild) +
    // `instances_built` (0 idle / N rebuild), set in every return path below.
    // `Option` so a harness without the resource is unaffected (no registration
    // drift, no missing-resource skip).
    mut counters: Option<ResMut<RenderWorkCounters>>,
    // #2 Stage B classifier: the STRUCTURAL damage probe. A change to any of these
    // reorders paint, changes effect-group membership, or changes a node's
    // per-buffer footprint (border/outline/shadow band slots) — so the frame is NOT
    // Patch-eligible (a Patch reuses paint order + slot layout). Value-only changes
    // (Background / GlobalTransform / ResolvedLayout) are absent here, so they stay
    // Patch-eligible. Entity-only; used as an `is_empty()` probe.
    structural_changed: Extract<
        Query<
            (),
            Or<(
                Changed<StackingContext>,
                Changed<Stacking>,
                Changed<ClipRect>,
                Changed<AncestorClip>,
                Changed<ComputedPaintSkip>,
                Changed<bevy::prelude::Children>,
                Changed<EffectGroup>,
                Changed<Opacity>,
                Changed<Outline>,
                Changed<Border>,
                Changed<BoxShadow>,
            )>,
        >,
    >,
    // #2 Stage C: entity -> slot map, rebuilt on Full, retained on idle/Patch. The
    // gate-skip path below must NOT deref it (leave it retained). `Option` so the
    // manual extract test harnesses (which don't register it) still run the system —
    // no registration drift, no missing-resource skip (the counters pattern).
    mut index: Option<ResMut<RetainedNodeIndex>>,
    // #2 Stage C3b: the Full-vs-Patch tag for prepare (Stage D). `Option` for the same
    // test-harness reason as `index`.
    mut damage: Option<ResMut<NodeDamage>>,
) {
    // Resolve the primary window's view target entity. v1: all Nodes paint into
    // the primary view (D2). If there is no primary window this frame, overwrite
    // the carrier with an EMPTY set rather than early-returning: this system always
    // publishes the carrier (it never early-returns past the publish leaving stale
    // data), so a vanished window clears to empty. Returning here
    // would instead leave the prior frame's nodes resident once the carrier is
    // `init_resource`'d (Task 7), and render would keep painting stale nodes.
    // Drain the removal streams FIRST, before any early-return, so the
    // `RemovedComponents` cursors advance every frame and events never
    // accumulate (incl. the vanished-window path below). A despawn drops
    // `ResolvedLayout`; a hide→show flip drops `ComputedPaintSkip` — the two
    // damage sources the `Changed` probe cannot see.
    let despawned = removed.read().count() > 0;
    let skip_lifted = removed_skip.read().count() > 0;

    let Ok(primary_window) = primary.single() else {
        *view = ExtractedNodesView::default();
        *groups_res = ExtractedEffectGroups::default();
        if let Some(idx) = index.as_deref_mut() {
            idx.0.clear();
        }
        if let Some(d) = damage.as_deref_mut() {
            *d = NodeDamage::Full;
        }
        record_node_counts(&mut counters, 0, 0, 0);
        return;
    };

    // Damage gate (architecture.md § 3.1 / 2026-06-07-render-extract-retain-damage
    // -design.md). Rebuild only when something this view paints actually changed:
    // a `Changed` paint input (incl. a paint-skip ADD), a paint-skip LIFT (the
    // marker-removal stream — a re-shown subtree must reappear), an entity
    // despawn, or a theme/forced-colors swap (a global token re-resolve that
    // bypasses per-entity change detection — color-and-forced-colors.md § 3).
    // On a steady-state frame,
    // return WITHOUT touching the resource — the prior `ExtractedNodesView` stays
    // resident and `is_changed()` is false in prepare, which retains the persistent
    // buffers (the node re-binds + re-draws them). This is the O(0) steady-state
    // the spec's gate-#14 budget requires.
    // Capture before the `let theme: &Theme = &theme` shadow below — the #2 Stage B
    // classifier at the publish needs it (a theme swap is a Full rebuild, not Patch).
    let theme_changed = theme.is_changed();
    if changed.is_empty() && !despawned && !skip_lifted && !theme_changed {
        record_node_counts(&mut counters, 0, 0, 0);
        return;
    }

    // Build a per-entity index so the painters_z walk can look each painter up.
    // (A HashMap keyed by Entity; the partial-re-extract cache keyed by Entity
    // inside ExtractedNodes is R6/R8's optimization — v1 rebuilds the changed set.)
    // `&Theme` via the `Extract<Res<Theme>>` deref chain; `Res::into_inner`
    // can't be called here because it would move out of the `Extract` deref.
    let theme: &Theme = &theme;
    // `std::collections::HashMap` matches the convention in layout/systems.rs.
    let mut by_entity: EntityHashMap<ExtractedNode> = EntityHashMap::default();
    // Effect-group formers seen this frame, keyed by entity → (reason, opacity).
    // The painted-bounds union + parent links are derived below from the
    // `ChildOf` chain; the per-entity `EffectReason`/`Opacity` are captured here
    // while the fan is borrowed (effect-compositor.md § 1.1).
    // entity → (reason, opacity, backdrop_blur_px). The blur radius (logical px)
    // is captured here while the `BackdropFilter` fan is borrowed — the prepare
    // pass needs it to plan the dual-Kawase pyramid (parity Wave B4).
    let mut group_formers: EntityHashMap<(EffectReason, f32, Option<f32>)> =
        EntityHashMap::default();
    let forced_colors = prefs.forced_colors;

    // #2 Stage C3b: attempt an in-place PATCH before the O(N) Full build. A Patch
    // re-resolves ONLY the changed entities and overwrites their existing slots in the
    // retained `ExtractedNodesView`, leaving every untouched sibling record + the slot
    // ORDER intact (R5 trap — never rebuild the ordered Vec from the changed set). It is
    // eligible iff there is no structural / hierarchy / group-membership / despawn /
    // paint-skip-lift / theme change AND every changed entity is a group-free,
    // footprint-stable painting node already resident in the prior index (so its slot is
    // stable and its band layout unchanged). Anything else falls through to the Full
    // build. The overwrite marks `view` changed, so prepare still does a FULL repack from
    // the patched records (text quads are a separate carrier, re-spliced normally) — the
    // extract O(N)->O(changed) win is HERE; the partial UPLOAD is Stage D.
    let patch_candidate =
        structural_changed.is_empty() && !despawned && !skip_lifted && !theme_changed;
    if patch_candidate && let Some(idx) = index.as_deref() {
        let mut patches: Vec<(usize, ExtractedNode)> = Vec::new();
        let mut patchable = true;
        // Read the prior records IMMUTABLY here (no `is_changed` trip) to decide + resolve.
        {
            let prior = &view.0.nodes;
            for e in changed.iter() {
                let Some(&slot) = idx.0.get(&e) else {
                    patchable = false;
                    break;
                };
                let Some(old) = prior.get(slot as usize) else {
                    patchable = false;
                    break;
                };
                // v1 scope: group-free quad-only. A grouped node would re-pack its group's
                // off-screen target; defer to Full.
                if old.group.is_some() {
                    patchable = false;
                    break;
                }
                let Ok(item) = nodes.get(e) else {
                    patchable = false;
                    break;
                };
                let Some(new) = resolve_one(item, theme, forced_colors) else {
                    patchable = false;
                    break;
                };
                // A footprint change (a band appeared/vanished, emits_quad flipped) shifts
                // the slot count -> Full repack.
                if node_footprint(&new) != node_footprint(old) {
                    patchable = false;
                    break;
                }
                patches.push((slot as usize, new));
            }
        }
        if patchable && !patches.is_empty() {
            // APPLY: overwrite each changed entity's slot in place. DerefMut marks `view`
            // changed so prepare repacks; the retained siblings + order are untouched.
            let patched = patches.len();
            for (slot, new) in patches {
                view.0.nodes[slot] = new;
            }
            record_node_counts(&mut counters, 0, patched, 1);
            if let Some(d) = damage.as_deref_mut() {
                *d = NodeDamage::Patch(changed.iter().collect());
            }
            return;
        }
        // Not all changed entities were patchable — fall through to the Full build.
    }

    for item in nodes.iter() {
        // #2 Stage C3a: capture each effect-group former while the fan is borrowed —
        // Full-build group-tag state, NOT part of the per-node record (a Patch only
        // re-resolves group-free nodes). Guarded by `paint_skip.is_none()` to match the
        // old `continue` (a paint-skipped entity forms no group and emits no record).
        if item.paint_skip.is_none()
            && let Some(eg) = item.effect_group
        {
            // `Opacity` default is 1.0 (no-op); capture unconditionally so a group that
            // ALSO has opacity composites at the right alpha. Parity Wave B4: capture
            // the backdrop-blur radius (logical px) for a backdrop-filter former.
            let a = item.opacity.map(|o| o.0).unwrap_or(1.0);
            let blur = if eg.reason.contains(EffectReason::BACKDROP_FILTER) {
                item.backdrop_filter.and_then(backdrop_blur_px)
            } else {
                None
            };
            group_formers.insert(item.entity, (eg.reason, a, blur));
        }
        // #2 Stage C3a: the per-node record via the shared single-entity resolver.
        // The Patch path re-resolves a changed entity through the SAME `resolve_one`,
        // so Full-build and Patch records are byte-identical by construction.
        if let Some(node) = resolve_one(item, theme, forced_colors) {
            by_entity.insert(node.entity, node);
        }
    }

    // Resolve each painted node's NEAREST `EffectGroup` ancestor (the group it
    // belongs to). Membership is the `ChildOf` subtree of a former, NOT an SC
    // boundary: the nearest-former climb (a node is its OWN group if it is a
    // former) is the v1 nesting source (effect-compositor.md § 1.1, decided
    // fork 5). The trigger-5 SC formers have since landed (layout/systems.rs
    // `forms_stacking_context`), making every SC-forming group's subtree one
    // atomic `painters_z` slice — the climb stays because it is SC-agnostic
    // and also covers the `backdrop-filter` former, which forms no SC.
    let nearest_group_entity = |start: Entity| -> Option<Entity> {
        let mut cur = start;
        loop {
            if group_formers.contains_key(&cur) {
                return Some(cur);
            }
            match child_of.get(cur) {
                Ok(parent) => cur = parent.parent(),
                Err(_) => return None,
            }
        }
    };

    // Assign each former a stable index, then compute its parent link (the
    // nearest enclosing former of its OWN parent) and seed its painted bounds.
    let mut group_entities: Vec<Entity> = group_formers.keys().copied().collect();
    group_entities.sort_unstable(); // deterministic indices
    let group_index: EntityHashMap<usize> = group_entities
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, i))
        .collect();
    let mut groups: Vec<EffectGroupExtract> = group_entities
        .iter()
        .map(|&e| {
            let (reason, opacity, blur_px) = group_formers[&e];
            // The enclosing group is the nearest former STRICTLY above `e`.
            let parent = child_of
                .get(e)
                .ok()
                .and_then(|p| nearest_group_entity(p.parent()))
                .and_then(|pe| group_index.get(&pe).copied());
            // Seed bounds with the former's own box (a former with no painted
            // record — transparent fill — still bounds-anchors its subtree).
            let own = by_entity.get(&e).map(|n| Rect {
                min: n.position,
                max: n.position + n.size,
            });
            // Parity Wave B4: a backdrop-filter former blurs the painted backdrop
            // within its OWN box (own.unwrap), distinct from the transitive
            // `bounds`. `backdrop_box`/`backdrop_blur_px` are `Some` only when this
            // former actually carries a px blur.
            let (backdrop_blur_px, backdrop_box) = match (blur_px, own) {
                (Some(px), Some(b)) => (Some(px), Some(b)),
                _ => (None, None),
            };
            EffectGroupExtract {
                entity: e,
                parent,
                opacity,
                reason,
                bounds: own.unwrap_or(Rect {
                    min: Vec2::ZERO,
                    max: Vec2::ZERO,
                }),
                backdrop_blur_px,
                backdrop_box,
            }
        })
        .collect();

    // Index every forming context by its root entity, then drive the recursive
    // tree walk (paint-order § 1.1): the root context paints its own box, then
    // its painters_z forward, descending into each nested-SC painter AS A UNIT
    // at that position. A nested root is a single atomic entry in its parent's
    // list, so descending there is what places its contents at the right global
    // index — flat-concatenating every context's painters_z would instead emit
    // nested descendants at the end of their own list (the wrong order).
    let sc_by_entity: std::collections::HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };
    // The cross-root rank lookup (layout 6f stamps `cross_root_rank` per context).
    let rank_by_entity: EntityHashMap<u8> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.cross_root_rank))
        .collect();

    // Root contexts (the shared helper): the forming entities no other context
    // lists as a painter. Ranked so a parentless TOP-LAYER root (a dialog authored
    // outside the main content tree) paints LAST — over the whole window — rather
    // than wherever its raw entity id falls (the M6 modal-under-shell bug). A
    // parented top-layer node escapes to its root's painters_z tail (layout 6f)
    // and never reaches this sort, so this only fixes the cross-ROOT case.
    let roots = context_roots(&sc_by_entity, |e| {
        rank_by_entity.get(&e).copied().unwrap_or(0)
    });

    // The view-level logical→clip terms (architecture § 4, D2: every Node
    // resolves to the primary window's view). `BuiyViewUniform::for_view`
    // consumes these in prepare; leaving them at the `Vec2::ZERO` default makes
    // the transform degenerate. Logical (CSS-px) size + the device-pixel ratio
    // come straight off the primary window's resolution. `nodes` is filled by
    // the `assemble_context_tree` walk below (starts empty, Default).
    let mut all = ExtractedNodes {
        logical_size: primary_window.resolution.size(),
        scale_factor: primary_window.resolution.scale_factor(),
        ..Default::default()
    };
    for root in roots {
        // R6/R8: merge cached records for unchanged painters here.
        assemble_context_tree(
            root,
            &painters_z_of,
            // `ExtractedNode` is no longer `Copy` (it carries the F-tier
            // `shadows: Vec`), so the assemble build clones the cached record.
            &mut |e| by_entity.get(&e).cloned(),
            &mut all.nodes,
        );
    }

    // Tag every assembled node with its nearest-`EffectGroup`-ancestor index and
    // grow that group's painted bounds by the member box. The walk emits a
    // group's subtree contiguously (it descends a node's children before its
    // siblings), so `pack_view` partitions the flat blob into contiguous per-group
    // ranges off this tag (effect-compositor.md § 1.1 / decided fork 3). Bounds
    // grow to the UNION of every member box (painted_bounds' descendant term).
    if !groups.is_empty() {
        for node in &mut all.nodes {
            if let Some(ge) = nearest_group_entity(node.entity)
                && let Some(&gi) = group_index.get(&ge)
            {
                node.group = Some(gi);
                let b = &mut groups[gi].bounds;
                b.min = b.min.min(node.position);
                b.max = b.max.max(node.position + node.size);
            }
        }
    }

    // Write the per-view ExtractedNodes onto the primary render view entity.
    // R6/R8 wire the exact main<->render view mapping and consume this component;
    // v1 inserts the single ExtractedNodes carrier (R5 owns the type — there is
    // no ExtractedNodesPrimary/ExtractedNodesResource wrapper). The precise
    // target-entity resolution is the one piece that needs the render world and
    // is exercised only under the GPU e2e path (Task 8 / R6/R8).
    // P0b rebuild path: this frame passed the damage gate and built the full set.
    // #2 Stage B classifier (OBSERVATION-ONLY — the rebuild above is still Full):
    // a dirty frame is Patch-ELIGIBLE if the damage is value-only — no structural /
    // hierarchy / footprint change (`structural_changed`), no effect groups in the
    // scene (`group_formers`), and not a despawn / paint-skip-lift / theme swap. A
    // later Patch stage will additionally require a per-entity footprint match, so
    // this coarse signal is an upper-ish bound used to size the Patch-path payoff
    // (the `node_patches` counter) before building the in-place Patch path.
    // #2 Stage C3b: reaching here means the Patch attempt above bailed (a footprint /
    // group mismatch or a non-resident changed entity) or the frame was never a Patch
    // candidate (structural / despawn / theme) — so this is a Full rebuild, node_patches = 0.
    record_node_counts(&mut counters, 1, all.nodes.len(), 0);
    if let Some(d) = damage.as_deref_mut() {
        *d = NodeDamage::Full;
    }
    *view = ExtractedNodesView(all);
    // #2 Stage C: rebuild the entity->slot index from the freshly-published ordered
    // nodes — the foundation for a later in-place Patch overwrite at `index[e]`.
    if let Some(idx) = index.as_deref_mut() {
        idx.0.clear();
        for (i, n) in view.0.nodes.iter().enumerate() {
            idx.0.insert(n.entity, i as u32);
        }
    }
    // The per-view effect-group list (effect-compositor.md § 1.1). Emitted on
    // EVERY rebuild frame (incl. when empty) so a frame that drops the last group
    // clears the carrier — mirrors the `ExtractedNodesView` overwrite contract.
    *groups_res = ExtractedEffectGroups(std::mem::take(&mut groups));
}

/// v1 carrier-by-resource: the primary view's `ExtractedNodes`, inserted by
/// `extract_buiy_nodes` until R6/R8 wire it onto the resolved render-view entity
/// as a per-view component (architecture § 4). This is a thin newtype over the
/// R5-owned `ExtractedNodes` — NOT a parallel definition. R6 reads the inner
/// `ExtractedNodes`; the type itself stays R5's single carrier.
/// SUPERSEDED-BY: R6/R8 (node.rs/buckets read the per-view `ExtractedNodes`).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedNodesView(pub ExtractedNodes);

/// #2 Stage C: entity -> its slot (index) in the retained `ExtractedNodesView.0.nodes`
/// ordered Vec. Rebuilt on every Full extract; retained (untouched) on idle + (later)
/// Patch frames, where the slots are paint-order-stable. Lets a future Patch stage
/// find a changed entity's record and overwrite it IN PLACE without rebuilding the
/// ordered Vec from the changed set (the R5 sibling-drop trap).
#[derive(Resource, Default)]
pub struct RetainedNodeIndex(pub EntityHashMap<u32>);

/// #2 Stage C3b: how this frame re-extracted the node set, for prepare (Stage D) to
/// size its instance-buffer upload. `Full` = the whole set was rebuilt (cold frame,
/// structural change, or a Patch-ineligible change) — prepare repacks + uploads all.
/// `Patch(entities)` = only these entities' records were overwritten in place (a
/// group-free, footprint-stable value change) — Stage D will upload only their slots.
/// In C3b prepare still does a FULL repack on Patch (the extract O(N)->O(changed) win
/// lands here; the partial UPLOAD is Stage D); the tag is published now so D can consume
/// it. Published every dirty frame, mirroring the `ExtractedNodesView` overwrite contract.
#[derive(Resource, Clone, Debug, Default)]
pub enum NodeDamage {
    #[default]
    Full,
    Patch(Vec<Entity>),
}

/// One text quad-tier visual (decoration-and-paint § 4.6): selection rects
/// (T7) and underline/overline (T6), keyed by the SOURCE entity. A flat
/// `Copy` record — deliberately NO order and NO group field: paint order is
/// the implicit `Vec` order of `ExtractedNodes.nodes`, and BOTH derive from
/// the fresh node list at pack time (a recorded index would go stale
/// whenever the node walk rebuilds while text quads are retained — the
/// spec's rejected round-1 design). Carries no cosmic-text type (the seam
/// contract).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextQuad {
    /// The source main-world entity — the splice key.
    pub entity: Entity,
    /// Painted top-left, logical px, window space (origin-folded by the
    /// producer; § 3.3 y already snapped).
    pub position: Vec2,
    /// Quad size, logical px (height = the § 3.3 floored thickness).
    pub size: Vec2,
    /// Resolved paint color (§ 3.2 precedence applied at extract);
    /// `Color::NONE` = skip at pack (mirrors `ExtractedNode.color`).
    pub color: Color,
    /// The entity's SELF-INCLUSIVE clip (same resolution as its glyphs,
    /// glyph-pipeline § 8); `None` = the full-view sentinel.
    pub clip: Option<ClipRect>,
}

/// Render-world carrier for text's quad-tier visuals (decoration-and-paint
/// § 4.6). Producer: `text::extract_buiy_glyphs` — rebuilt alongside
/// `ExtractedGlyphs` under the same § 6.2 probe union (one damage decision),
/// retained untouched on steady frames so `is_changed()` is the third quad
/// gate term in `prepare_buiy_instances`. ENTITY-GROUPED: each entity's
/// quads are contiguous, in § 4.4 emission order (the pack debug_asserts
/// the grouping).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedTextQuads {
    pub quads: Vec<TextQuad>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Length;

    #[test]
    fn backdrop_blur_resolves_first_px_blur() {
        // parity Wave B4: the resolver picks the first `Blur(Px)` term.
        let f = BackdropFilter(vec![FilterFn::Blur(Length::px(6.0))]);
        assert_eq!(backdrop_blur_px(&f), Some(6.0));
    }

    #[test]
    fn backdrop_blur_skips_non_px_and_non_blur() {
        // No blur term → None.
        assert_eq!(
            backdrop_blur_px(&BackdropFilter(vec![FilterFn::Brightness(0.5)])),
            None
        );
        // A zero-radius blur is a no-op (treated as no blur).
        assert_eq!(
            backdrop_blur_px(&BackdropFilter(vec![FilterFn::Blur(Length::ZERO)])),
            None
        );
        // A percent blur (no concrete px) is not resolved in v1.
        assert_eq!(
            backdrop_blur_px(&BackdropFilter(vec![FilterFn::Blur(Length::Percent(10.0))])),
            None
        );
        // An empty list → None.
        assert_eq!(backdrop_blur_px(&BackdropFilter(vec![])), None);
    }

    #[test]
    fn backdrop_blur_picks_blur_among_other_filters() {
        // A blur AFTER a brightness term is still found.
        let f = BackdropFilter(vec![
            FilterFn::Brightness(0.8),
            FilterFn::Blur(Length::px(2.0)),
        ]);
        assert_eq!(backdrop_blur_px(&f), Some(2.0));
    }
}
