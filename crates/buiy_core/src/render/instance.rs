//! Per-instance data layout for the rounded-rect pipeline (the view-uniform
//! path). The struct stride must equal the per-instance `array_stride` declared
//! in `pipeline.rs::register` (52 B). Records stay in LOGICAL-pixel units: the
//! per-view [`BuiyViewUniform`] does the logical → clip transform in the vertex
//! stage, so the Phase-0 per-instance y-flip / `2/min(w,h)` radius hack is
//! retired (`buiy-render-pipeline-design`, architecture.md § 3).
//!
//! Each instance also carries its per-primitive clip AABB (`clip_min`/`clip_max`,
//! R8b): the fragment shader discards pixels outside it, so clipping needs no
//! per-batch scissor or re-sort (one order-safe draw). A node with no clip packs
//! the full-view sentinel (`[±INFINITY]`) so the discard never fires.
//!
//! [`BuiyViewUniform`]: crate::render::view_uniform::BuiyViewUniform

use crate::render::DrawData;
use crate::render::extract::{ExtractedNode, TextQuad};
use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Stride of the logical-pixel [`PackedInstance`] in bytes. Must match the
/// per-instance `array_stride` declared in `pipeline.rs::register` (52 B). The
/// values are LOGICAL pixels — the GPU view uniform
/// ([`crate::render::view_uniform::BuiyViewUniform`]) applies the logical->clip
/// transform in the vertex stage.
pub const PACKED_INSTANCE_STRIDE_BYTES: usize = 52;

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
    }
}

/// `true` iff the raw `[f32; 13]` bucket layout is byte-equal to
/// [`PackedInstance`]'s stride (the pipeline-descriptor invariant). Pins the
/// agreement the instanced draw relies on.
pub fn packed_raw_stride_agrees() -> bool {
    std::mem::size_of::<PackedInstance>() == std::mem::size_of::<[f32; 13]>()
        && PACKED_INSTANCE_STRIDE_BYTES == std::mem::size_of::<[f32; 13]>()
}
