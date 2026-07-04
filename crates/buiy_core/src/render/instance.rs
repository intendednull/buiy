//! Per-instance data layout for the rounded-rect pipeline (the view-uniform
//! path). The struct stride must equal the per-instance `array_stride` declared
//! in `pipeline.rs::register` (68 B). Records stay in LOGICAL-pixel units: the
//! per-view [`BuiyViewUniform`] does the logical → clip transform in the vertex
//! stage, so the Phase-0 per-instance y-flip / `2/min(w,h)` radius hack is
//! retired (`buiy-render-pipeline-design`, architecture.md § 3).
//!
//! Each instance also carries its per-primitive clip AABB (`clip_min`/`clip_max`,
//! R8b): the fragment shader discards pixels outside it, so clipping needs no
//! per-batch scissor or re-sort (one order-safe draw). A node with no clip packs
//! the full-view sentinel (`[±INFINITY]`) so the discard never fires.
//!
//! It also carries the 2D affine basis (`affine`, R1 — the `[m00,m10,m01,m11]`
//! columns of `GlobalTransform`'s 2D linear part), APPENDED after the clip
//! fields so every prior field offset stays byte-stable (notably
//! [`COLOR_FLOAT_OFFSET`] / [`ALPHA_FLOAT_OFFSET`], which R2's degraded-group
//! re-tint reads). The vertex stage transforms each box-local corner by it, so
//! a rotated/scaled element paints with the right orientation/size. Identity
//! `[1,0,0,1]` == no transform (the byte-identical fast path).
//!
//! [`BuiyViewUniform`]: crate::render::view_uniform::BuiyViewUniform

use crate::render::DrawData;
use crate::render::extract::{
    ExtractedBorder, ExtractedNode, ExtractedOutline, ExtractedShadow, TextQuad,
};
use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Stride of the logical-pixel [`PackedInstance`] in bytes. Must match the
/// per-instance `array_stride` declared in `pipeline.rs::register` (68 B). The
/// values are LOGICAL pixels — the GPU view uniform
/// ([`crate::render::view_uniform::BuiyViewUniform`]) applies the logical->clip
/// transform in the vertex stage.
pub const PACKED_INSTANCE_STRIDE_BYTES: usize = 68;

/// Float index of the per-instance color block (`color[0]`) in the raw
/// [`crate::render::buckets::packed_to_raw`] record. NAMED so the color/alpha
/// offset is referenced symbolically everywhere (R1 HARD CONSTRAINT): the
/// append-after-13 affine layout exists precisely to keep this offset stable so
/// R2's degraded-group re-tint can index it.
pub const COLOR_FLOAT_OFFSET: usize = 4;

/// Float index of the per-instance alpha (`color[3]`) in the raw record —
/// `COLOR_FLOAT_OFFSET + 3`. R2's degraded-group forward-composite re-tints by
/// reading the alpha at this offset; it MUST stay `7` across any layout growth
/// (the affine basis appends after the clip fields, never before color).
pub const ALPHA_FLOAT_OFFSET: usize = COLOR_FLOAT_OFFSET + 3;

/// The identity 2D affine basis `[m00, m10, m01, m11] = [1, 0, 0, 1]` — no
/// rotation/scale. Quads with no `GlobalTransform` linear part (the `DrawData`
/// and text-quad packers) carry this, so their packed bytes are unchanged by
/// R1's growth except for the four appended identity floats.
const IDENTITY_AFFINE: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

/// Full-view clip sentinel for an unclipped instance (`ExtractedNode.clip ==
/// None`): `clip_min = [-INFINITY; 2]`, `clip_max = [+INFINITY; 2]`. For any
/// finite fragment position both `any(< -INF)` and `any(> +INF)` are `false`, so
/// the fragment discard never fires and the instance paints unclipped. Both are
/// valid `bytemuck::Pod` f32 (paint-order-and-top-layer.md § 3.2).
const CLIP_SENTINEL_MIN: [f32; 2] = [f32::NEG_INFINITY, f32::NEG_INFINITY];
const CLIP_SENTINEL_MAX: [f32; 2] = [f32::INFINITY, f32::INFINITY];

/// One instance record in LOGICAL-pixel units (the view-uniform handoff). There
/// is no per-instance y-flip and no `2/min(w,h)` radius approximation:
/// position/size/radius are forwarded raw and the GPU view uniform does the
/// clip transform. Color is CPU-pre-linearized (color-and-forced-colors.md
/// § 1.1).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct PackedInstance {
    /// Top-left in logical pixels (window-relative, y-down).
    pub rect_pos: [f32; 2],
    /// Width / height in logical pixels (height is POSITIVE — the y-flip lives
    /// in the view uniform now, not in a negative height).
    pub rect_size: [f32; 2],
    /// Linear RGBA, pre-linearized on the CPU.
    pub color: [f32; 4],
    /// Corner radius in LOGICAL pixels (no clip-space approximation).
    pub radius: f32,
    /// Clip AABB minimum in LOGICAL px (same space as [`crate::render::components::ClipRect`]).
    /// The fragment discards `frag_pos < clip_min`; `[-INFINITY; 2]` = no lower bound.
    pub clip_min: [f32; 2],
    /// Clip AABB maximum in LOGICAL px. The fragment discards `frag_pos > clip_max`;
    /// `[+INFINITY; 2]` = no upper bound (the full-view sentinel).
    pub clip_max: [f32; 2],
    /// The 2D affine basis `[m00, m10, m01, m11]` (the column vectors of
    /// `GlobalTransform`'s 2D linear part) — R1. APPENDED after the clip fields
    /// so every prior offset stays byte-stable (the R2 dependency). The vertex
    /// stage maps each box-local corner `c` to `mat2(col0, col1) * c`. Identity
    /// `[1, 0, 0, 1]` paints axis-aligned (no rotation/scale).
    pub affine: [f32; 4],
}

/// Pack one [`DrawData`] into a logical-pixel [`PackedInstance`]. The clip
/// transform is deferred to the GPU view uniform; only the sRGB->linear color
/// conversion happens here. `DrawData` carries no clip, so the instance gets the
/// full-view sentinel (paints unclipped).
pub fn pack_instance(draw: &DrawData) -> PackedInstance {
    let lin = LinearRgba::from(draw.color);
    PackedInstance {
        rect_pos: [draw.position.x, draw.position.y],
        rect_size: [draw.size.x, draw.size.y],
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        radius: draw.radius,
        clip_min: CLIP_SENTINEL_MIN,
        clip_max: CLIP_SENTINEL_MAX,
        // `DrawData` has no transform; paint axis-aligned (identity basis).
        affine: IDENTITY_AFFINE,
    }
}

/// Pack one R5 [`ExtractedNode`] (the per-painted-entity CPU record) into a
/// logical-pixel [`PackedInstance`] — the prepare seam R6 packs through with no
/// `DrawData` adapter. `ExtractedNode` carries the solid-fill quad inputs
/// (position / size / color / a fill corner radius). The radius is non-zero for a
/// **borderless-rounded** node ([`ExtractedNode::radius`] — a `Border.radius`
/// with no painting side, F3) OR a **bordered-rounded** node (the fill rounds to
/// the band's uniform inner radius so no square "ears" poke past a rounded border,
/// F4b — `extract.rs`). A **square** node (no radius, or a square border) still
/// packs `0`, so every non-rounded golden stays byte-identical; a rounded-bordered
/// fixture legitimately shifts (the ear pixels vanish — the `golden_card_bordered`
/// re-bless). Color is CPU-pre-linearized exactly as in
/// [`pack_instance`] (color-and-forced-colors.md § 1.1).
pub fn pack_extracted(node: &ExtractedNode) -> PackedInstance {
    let lin = LinearRgba::from(node.color);
    // The per-primitive clip AABB rides the instance; `None` is the full-view
    // sentinel (no ancestor clips, or a top-layer member — paint-order § 3.2).
    let (clip_min, clip_max) = match node.clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    PackedInstance {
        rect_pos: [node.position.x, node.position.y],
        rect_size: [node.size.x, node.size.y],
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        // Non-zero only for a borderless-rounded node (a `Border.radius` with no
        // painting side); a bordered / square node packs `0` exactly as before,
        // so every existing golden is byte-identical.
        radius: node.radius,
        clip_min,
        clip_max,
        // The 2D affine basis, flattened to columns [m00, m10, m01, m11] (R1):
        // col0 = node.affine[0], col1 = node.affine[1]. The vertex stage applies
        // it about the box-local origin so rotation/scale paint correctly.
        affine: [
            node.affine[0][0],
            node.affine[0][1],
            node.affine[1][0],
            node.affine[1][1],
        ],
    }
}

/// Pack one [`TextQuad`] (decoration-and-paint § 4.6) exactly like a node
/// quad: CPU-linearized color, radius 0, clip sentinel. Same blob, same
/// pipeline, no new GPU anything.
pub fn pack_text_quad(quad: &TextQuad) -> PackedInstance {
    let lin = LinearRgba::from(quad.color);
    let (clip_min, clip_max) = match quad.clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    PackedInstance {
        rect_pos: [quad.position.x, quad.position.y],
        rect_size: [quad.size.x, quad.size.y],
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        radius: 0.0,
        clip_min,
        clip_max,
        // Text quads carry no transform; paint axis-aligned (identity basis).
        affine: IDENTITY_AFFINE,
    }
}

/// Pack one [`ExtractedShadow`] into a [`PackedInstance`] for the reserved
/// `(Shadow, layer)` bucket (styling-f-tier.md § 2.2 — C6-b). Reuses the frozen
/// 68 B quad layout with ZERO stride change: the `radius` slot carries the
/// effective blur SIGMA (`shadow.wgsl` reinterprets `@location(5)` as the blur),
/// `rect_pos`/`rect_size` are the spread-and-offset-expanded box (CPU-computed by
/// `resolve_shadows`), `color` is the CPU-linearized shadow color, and
/// `clip`/`affine` ride the node's fields. One `PackedInstance` per shadow term.
pub fn pack_shadow(shadow: &ExtractedShadow) -> PackedInstance {
    let (clip_min, clip_max) = match shadow.clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    PackedInstance {
        rect_pos: [shadow.rect_pos.x, shadow.rect_pos.y],
        rect_size: [shadow.rect_size.x, shadow.rect_size.y],
        color: shadow.color,
        // The radius slot IS the effective blur sigma for the shadow primitive
        // (`shadow.wgsl:5,31`) — NOT a corner radius.
        radius: shadow.sigma,
        clip_min,
        clip_max,
        affine: [
            shadow.affine[0][0],
            shadow.affine[0][1],
            shadow.affine[1][0],
            shadow.affine[1][1],
        ],
    }
}

/// `true` iff the raw `[f32; 17]` bucket layout is byte-equal to
/// [`PackedInstance`]'s stride (the pipeline-descriptor invariant). Pins the
/// agreement the instanced draw relies on.
pub fn packed_raw_stride_agrees() -> bool {
    std::mem::size_of::<PackedInstance>() == std::mem::size_of::<[f32; 17]>()
        && PACKED_INSTANCE_STRIDE_BYTES == std::mem::size_of::<[f32; 17]>()
}

/// Stride of [`BorderBandInstance`] in bytes (the band/outline quad-variant
/// record). MUST match the per-instance `array_stride` the band pipeline
/// declares in `BuiyBandPipeline::band_vertex_buffers` and the
/// `@location`-bound fields of `band.wgsl`.
///
/// 52 f32 = 208 B: `rect_pos`(2) + `rect_size`(2) + 4 per-side colors (16) +
/// `width`(4) + `outer_radius`(8) + `inner_radius`(8) + `clip_min`(2) +
/// `clip_max`(2) + `affine`(4) + `style`(4). Computed from the struct so the two
/// can never disagree (asserted by [`border_band_stride_agrees`]).
pub const BORDER_BAND_INSTANCE_STRIDE_BYTES: usize = std::mem::size_of::<BorderBandInstance>();

/// The border / outline quad-variant instance — a record DISTINCT from
/// [`PackedInstance`] (NOT a stride bump): its own [`bevy::render::render_resource::RawBufferVec`],
/// its own `VertexBufferLayout`, its own shader (`band.wgsl`, octet `..06`),
/// painted through a dedicated band pipeline. The no-border/outline quad path
/// is UNTOUCHED — `PackedInstance` stays byte-identical `[f32;17]`, so R1/R2
/// byte-stability (umbrella § 6.7) holds.
///
/// The band fragment is the outer-minus-inner rounded-rect SDF (the
/// `render_border_sdf.rs` oracle made GPU): a fragment is painted iff
/// `inside(outer_rounded_rect) AND NOT inside(inner_rounded_rect)`. For an
/// **outline** all four `color_*` are equal (a uniform stroke); per-side
/// colors exist so C6-b's border feeds the SAME record with no further layout
/// change (styling-f-tier.md § 2.3 / § 3.1).
///
/// Spec: docs/specs/2026-06-22-buiy-widget-catalog-design/styling-f-tier.md § 2.3 / § 2.4.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BorderBandInstance {
    /// Outer-box top-left, logical px (for an outline this is the border box
    /// grown outward by `width + offset`).
    pub rect_pos: [f32; 2],
    /// Outer-box size, logical px (positive).
    pub rect_size: [f32; 2],
    /// Per-side resolved linear color (top / right / bottom / left). For an
    /// outline all four are the resolved ring color.
    pub color_top: [f32; 4],
    pub color_right: [f32; 4],
    pub color_bottom: [f32; 4],
    pub color_left: [f32; 4],
    /// Band thickness per side, `[top, right, bottom, left]` logical px. For an
    /// outline all four are the ring `width`.
    pub width: [f32; 4],
    /// Per-corner OUTER elliptical radius `(rx, ry) × 4` (TL, TR, BR, BL).
    pub outer_radius: [f32; 8],
    /// Per-corner INNER elliptical radius (outer shrunk by the adjacent width —
    /// the oracle's load-bearing shrink, `render_border_sdf.rs:66-101`).
    pub inner_radius: [f32; 8],
    /// Clip AABB min in logical px. For an OUTLINE this is the entity's
    /// **`AncestorClip`** (never its own `ClipRect`) so a focus ring survives an
    /// `overflow:hidden` ancestor (WCAG 2.4.7 / 2.4.11, styling-f-tier.md § 2.4).
    /// `[-INFINITY; 2]` = the full-view sentinel (unclipped / top-layer).
    pub clip_min: [f32; 2],
    /// Clip AABB max in logical px. `[+INFINITY; 2]` = the full-view sentinel.
    pub clip_max: [f32; 2],
    /// The 2D affine basis `[m00, m10, m01, m11]` (the column vectors of
    /// `GlobalTransform`'s 2D linear part) — the band rides the same transform
    /// path as the fill so a rotated/scaled element's ring stays aligned.
    pub affine: [f32; 4],
    /// Per-side dash-stipple flag `[top, right, bottom, left]` (F4b-3): `0.0`
    /// solid (a continuous ring, byte-identical to the pre-F4b band), `1.0`
    /// dashed, `2.0` dotted. APPENDED after `affine` so every prior field offset
    /// stays byte-stable (existing band goldens unchanged); `band.wgsl` selects
    /// the fragment's side flag by quadrant and stipples the coverage. An OUTLINE
    /// (focus ring) is always solid (`[0; 4]`).
    pub style: [f32; 4],
}

/// Pack one [`ExtractedOutline`] into a [`BorderBandInstance`]. The outline is
/// a uniform ring: the outer box is the border box grown by `width + offset`,
/// the inner edge is `width` thick, and the clip is the OUTLINE clip (the
/// entity's `AncestorClip`, resolved at extract — never its own box).
/// Color is already CPU-linearized by the extract producer.
pub fn pack_outline(outline: &ExtractedOutline) -> BorderBandInstance {
    let c = outline.color;
    let (clip_min, clip_max) = match outline.clip {
        Some(clip) => ([clip.min.x, clip.min.y], [clip.max.x, clip.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    let w = outline.width;
    BorderBandInstance {
        rect_pos: [outline.outer_pos.x, outline.outer_pos.y],
        rect_size: [outline.outer_size.x, outline.outer_size.y],
        color_top: c,
        color_right: c,
        color_bottom: c,
        color_left: c,
        width: [w, w, w, w],
        outer_radius: outline.outer_radius,
        inner_radius: outline.inner_radius,
        clip_min,
        clip_max,
        affine: [
            outline.affine[0][0],
            outline.affine[0][1],
            outline.affine[1][0],
            outline.affine[1][1],
        ],
        // A focus ring / selection outline is always a continuous stroke.
        style: [0.0; 4],
    }
}

/// Pack one [`ExtractedBorder`] into a [`BorderBandInstance`] (styling-f-tier.md
/// § 2.3 — C6-b). The border feeds the SAME band record + `band.wgsl` shader the
/// outline rides, but AT the box edge: the outer box is the border box itself
/// (NOT grown), the band is `width` thick INWARD (`inner_half = outer_half -
/// width` in the shader), and the per-side colors + per-corner inner radii come
/// straight off the extract record (already CPU-linearized + width-shrunk). The
/// clip is the entity's OWN clip (the band is inside the border box).
pub fn pack_border(border: &ExtractedBorder) -> BorderBandInstance {
    let (clip_min, clip_max) = match border.clip {
        Some(clip) => ([clip.min.x, clip.min.y], [clip.max.x, clip.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    BorderBandInstance {
        rect_pos: [border.outer_pos.x, border.outer_pos.y],
        rect_size: [border.outer_size.x, border.outer_size.y],
        color_top: border.color_top,
        color_right: border.color_right,
        color_bottom: border.color_bottom,
        color_left: border.color_left,
        width: border.width,
        outer_radius: border.outer_radius,
        inner_radius: border.inner_radius,
        clip_min,
        clip_max,
        affine: [
            border.affine[0][0],
            border.affine[0][1],
            border.affine[1][0],
            border.affine[1][1],
        ],
        // Per-side dash-stipple flag (F4b-3); `[0; 4]` for an all-solid border.
        style: border.style,
    }
}

/// `true` iff [`BORDER_BAND_INSTANCE_STRIDE_BYTES`] equals the actual
/// [`BorderBandInstance`] size and is the value the band pipeline declares
/// (52 f32 = 208 B). The parallel of [`packed_raw_stride_agrees`] for the
/// distinct band record (styling-f-tier.md § 4 — C6 adds this).
pub fn border_band_stride_agrees() -> bool {
    // 2 + 2 + 4*4 + 4 + 8 + 8 + 2 + 2 + 4 + 4 = 52 f32 = 208 B.
    BORDER_BAND_INSTANCE_STRIDE_BYTES == std::mem::size_of::<BorderBandInstance>()
        && BORDER_BAND_INSTANCE_STRIDE_BYTES == 52 * std::mem::size_of::<f32>()
}

/// Gradient instance-kind discriminant (the `params.x` flag the gradient shader
/// branches on). `Linear` is the only kind the design uses; `Radial` is the
/// seam B2 (dotted-grid pattern) fills the shader branch for.
pub const GRADIENT_KIND_LINEAR: f32 = 0.0;
/// Radial gradient kind (B2 seam — distance-to-center). Reserved here so the
/// instance + pipeline + shader carry the flag in B1; B1 only paints `Linear`.
pub const GRADIENT_KIND_RADIAL: f32 = 1.0;

/// Stride of [`GradientInstance`] in bytes (the gradient quad-variant record).
/// MUST match the per-instance `array_stride` the gradient pipeline declares in
/// `BuiyGradientPipeline::gradient_vertex_buffers` and the `@location`-bound
/// fields of `gradient.wgsl`. Computed from the struct so the two can never
/// disagree (asserted by [`gradient_stride_agrees`]).
pub const GRADIENT_INSTANCE_STRIDE_BYTES: usize = std::mem::size_of::<GradientInstance>();

/// The gradient quad-variant instance — a record DISTINCT from [`PackedInstance`]
/// (NOT a stride bump on the 68 B quad), exactly like [`BorderBandInstance`]: its
/// own [`bevy::render::render_resource::RawBufferVec`], its own
/// `VertexBufferLayout`, its own shader (`gradient.wgsl`), painted through a
/// dedicated gradient pipeline in the quad paint slot. The no-gradient quad path
/// is UNTOUCHED — `PackedInstance` stays byte-identical `[f32;17]`, so R1/R2
/// byte-stability holds and a 1000-quad scene carries ZERO gradient bytes.
///
/// **Two-stop fast path.** The widget-catalog design only ever uses 2-stop
/// linear gradients, so the record inlines exactly 2 resolved (CPU-linearized)
/// stop colors + their normalized positions — no variable-length stop buffer.
/// A future N-stop need extends the record (or adds a stop SSBO) without
/// disturbing the 2-stop path.
///
/// **CPU-precomputed axis.** The shader does NO trig: the CPU resolves the CSS
/// angle to a unit gradient-axis direction in the box's y-DOWN fragment space
/// (`axis = (sinθ, -cosθ)` — CSS `0deg` points up / y-up, flipped to y-down) and
/// the gradient-line length (`|W·sinθ| + |H·cosθ|`). The fragment projects its
/// box-local centered point onto the axis: `t = 0.5 + dot(p, axis)/line_len`,
/// then interpolates the 2 stops by `t`.
///
/// Spec: docs/specs/2026-06-25-widget-catalog-parity-design.md § 3.2;
/// docs/specs/2026-06-25-widget-catalog-values.md § 8.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GradientInstance {
    /// Box top-left, logical px (the fill box origin — `ExtractedNode.position`).
    pub rect_pos: [f32; 2],
    /// Box size, logical px (positive) — `ResolvedLayout.size`.
    pub rect_size: [f32; 2],
    /// Stop 0 resolved linear RGBA (CPU-pre-linearized — the gradient START,
    /// position `stops[0]`).
    pub color0: [f32; 4],
    /// Stop 1 resolved linear RGBA (the gradient END, position `stops[1]`).
    pub color1: [f32; 4],
    /// The two stop positions `[pos0, pos1]` (normalized `0..1` along the
    /// gradient line). For the design's 2-stop gradients this is `[0, 1]`.
    pub stops: [f32; 2],
    /// CPU-precomputed unit gradient axis in the box's y-DOWN fragment space
    /// `[ax, ay] = [sinθ, -cosθ]` (the direction from the 0%-stop end toward the
    /// 100%-stop end). Linear gradients only; ignored by the radial branch.
    pub axis: [f32; 2],
    /// `params = [kind, line_len]`: the gradient-kind flag
    /// ([`GRADIENT_KIND_LINEAR`] / [`GRADIENT_KIND_RADIAL`]) and the
    /// CPU-precomputed CSS gradient-line length `|W·sinθ| + |H·cosθ|` (linear) /
    /// the max radius (radial, B2).
    pub params: [f32; 2],
    /// Clip AABB min in logical px (same space as
    /// [`crate::render::components::ClipRect`]); `[-INFINITY; 2]` = full-view
    /// sentinel (unclipped / top-layer).
    pub clip_min: [f32; 2],
    /// Clip AABB max in logical px; `[+INFINITY; 2]` = full-view sentinel.
    pub clip_max: [f32; 2],
    /// The 2D affine basis `[m00, m10, m01, m11]` — the gradient box rides the
    /// SAME transform path as the fill so a rotated/scaled element's gradient
    /// stays aligned. Identity `[1, 0, 0, 1]` paints axis-aligned.
    pub affine: [f32; 4],
}

/// Pack one [`ExtractedGradient`](crate::render::extract::ExtractedGradient) into
/// a [`GradientInstance`]. The colors are already CPU-linearized and the axis /
/// line-length already CPU-computed by the extract producer
/// ([`resolve_gradients`](crate::render::extract::resolve_gradients)); this is a
/// pure field copy (the clip sentinel + affine flatten mirror `pack_outline` /
/// `pack_shadow`).
pub fn pack_gradient(g: &crate::render::extract::ExtractedGradient) -> GradientInstance {
    let (clip_min, clip_max) = match g.clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    GradientInstance {
        rect_pos: [g.rect_pos.x, g.rect_pos.y],
        rect_size: [g.rect_size.x, g.rect_size.y],
        color0: g.color0,
        color1: g.color1,
        stops: g.stops,
        axis: g.axis,
        params: [g.kind, g.line_len],
        clip_min,
        clip_max,
        affine: [
            g.affine[0][0],
            g.affine[0][1],
            g.affine[1][0],
            g.affine[1][1],
        ],
    }
}

/// `true` iff [`GRADIENT_INSTANCE_STRIDE_BYTES`] equals the actual
/// [`GradientInstance`] size and the value the gradient pipeline declares
/// (26 f32 = 104 B). The parallel of [`border_band_stride_agrees`] for the
/// distinct gradient record.
pub fn gradient_stride_agrees() -> bool {
    // 2 + 2 + 4 + 4 + 2 + 2 + 2 + 2 + 2 + 4 = 26 f32 = 104 B.
    GRADIENT_INSTANCE_STRIDE_BYTES == std::mem::size_of::<GradientInstance>()
        && GRADIENT_INSTANCE_STRIDE_BYTES == 26 * std::mem::size_of::<f32>()
}

/// Stride of [`RoundedShadowInstance`] in bytes (the rounded-shadow quad-variant
/// record). MUST match the per-instance `array_stride` the rounded-shadow pipeline
/// declares in `BuiyRoundedShadowPipeline::rounded_shadow_vertex_buffers` and the
/// `@location`-bound fields of `rounded_shadow.wgsl`. Computed from the struct so
/// the two can never disagree (asserted by [`rounded_shadow_stride_agrees`]).
pub const ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES: usize =
    std::mem::size_of::<RoundedShadowInstance>();

/// The ROUNDED box-shadow instance (F4b-6) — a record DISTINCT from
/// [`PackedInstance`] (the shape the SQUARE shadow reuses), exactly like
/// [`GradientInstance`] / [`BorderBandInstance`]: its own
/// [`bevy::render::render_resource::RawBufferVec`], its own `VertexBufferLayout`,
/// its own shader (`rounded_shadow.wgsl`), painted through a dedicated pipeline in
/// the SHADOW tier. The square-shadow path (the 68 B quad layout with the radius
/// slot reused as the blur sigma) is UNTOUCHED, so every existing shadow golden is
/// byte-identical (Option B, spec §2.5.1 — do NOT widen the byte-stable
/// `PackedInstance`).
///
/// Unlike the square shadow, this carries BOTH a blur `sigma` AND a corner
/// `radius`, so a rounded caster's shadow rounds its corners to match (the crisp
/// zero-blur 3D-press edge = `sigma == 0`, a rounded blurred card = `sigma > 0`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RoundedShadowInstance {
    /// Shadow box top-left, logical px (spread-and-offset-expanded).
    pub rect_pos: [f32; 2],
    /// Shadow box size, logical px (positive).
    pub rect_size: [f32; 2],
    /// Pre-linearized shadow color (linear RGBA).
    pub color: [f32; 4],
    /// Effective Gaussian blur sigma in logical px (`0` = the crisp edge).
    pub sigma: f32,
    /// Uniform corner radius in logical px (the caster radius grown by spread,
    /// clamped to the box) — the reason this is a distinct record.
    pub radius: f32,
    /// Clip AABB min in logical px; `[-INFINITY; 2]` = the full-view sentinel.
    pub clip_min: [f32; 2],
    /// Clip AABB max in logical px; `[+INFINITY; 2]` = the full-view sentinel.
    pub clip_max: [f32; 2],
    /// The 2D affine basis `[m00, m10, m01, m11]` — the shadow rides the same
    /// transform path as the caster. Identity `[1, 0, 0, 1]` paints axis-aligned.
    pub affine: [f32; 4],
}

/// Pack one rounded [`ExtractedShadow`] term (a term whose `radius > 0`) into a
/// [`RoundedShadowInstance`]. The caller ([`crate::render::buckets::pack_rounded_shadow_instances`])
/// filters to `radius > 0` terms; a `radius == 0` term stays on the square
/// [`pack_shadow`] path. Pure field copy (the clip sentinel + affine flatten
/// mirror [`pack_shadow`] / [`pack_gradient`]).
pub fn pack_rounded_shadow(shadow: &ExtractedShadow) -> RoundedShadowInstance {
    let (clip_min, clip_max) = match shadow.clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    RoundedShadowInstance {
        rect_pos: [shadow.rect_pos.x, shadow.rect_pos.y],
        rect_size: [shadow.rect_size.x, shadow.rect_size.y],
        color: shadow.color,
        sigma: shadow.sigma,
        radius: shadow.radius,
        clip_min,
        clip_max,
        affine: [
            shadow.affine[0][0],
            shadow.affine[0][1],
            shadow.affine[1][0],
            shadow.affine[1][1],
        ],
    }
}

/// `true` iff [`ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES`] equals the actual
/// [`RoundedShadowInstance`] size and the value the pipeline declares (18 f32 =
/// 72 B). The parallel of [`gradient_stride_agrees`] for the rounded-shadow record.
pub fn rounded_shadow_stride_agrees() -> bool {
    // 2 + 2 + 4 + 1 + 1 + 2 + 2 + 4 = 18 f32 = 72 B.
    ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES == std::mem::size_of::<RoundedShadowInstance>()
        && ROUNDED_SHADOW_INSTANCE_STRIDE_BYTES == 18 * std::mem::size_of::<f32>()
}
