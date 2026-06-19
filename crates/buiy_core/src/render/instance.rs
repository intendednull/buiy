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
use crate::render::extract::{ExtractedNode, TextQuad};
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
/// (position / size / color); per-node corner radius is not yet on the extract
/// record, so v1 packs square quads (radius `0`). Color is CPU-pre-linearized
/// exactly as in [`pack_instance`] (color-and-forced-colors.md § 1.1).
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
        radius: 0.0,
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

/// `true` iff the raw `[f32; 17]` bucket layout is byte-equal to
/// [`PackedInstance`]'s stride (the pipeline-descriptor invariant). Pins the
/// agreement the instanced draw relies on.
pub fn packed_raw_stride_agrees() -> bool {
    std::mem::size_of::<PackedInstance>() == std::mem::size_of::<[f32; 17]>()
        && PACKED_INSTANCE_STRIDE_BYTES == std::mem::size_of::<[f32; 17]>()
}
