//! Per-instance data layout for the rounded-rect pipeline (the view-uniform
//! path). The struct stride must equal the per-instance `array_stride` declared
//! in `pipeline.rs::register` (36 B). Records stay in LOGICAL-pixel units: the
//! per-view [`BuiyViewUniform`] does the logical → clip transform in the vertex
//! stage, so the Phase-0 per-instance y-flip / `2/min(w,h)` radius hack is
//! retired (`buiy-render-pipeline-design`, architecture.md § 3).
//!
//! [`BuiyViewUniform`]: crate::render::view_uniform::BuiyViewUniform

use crate::render::DrawData;
use crate::render::extract::ExtractedNode;
use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Stride of the logical-pixel [`PackedInstance`] in bytes. Must match the
/// per-instance `array_stride` declared in `pipeline.rs::register` (36 B). The
/// values are LOGICAL pixels — the GPU view uniform
/// ([`crate::render::view_uniform::BuiyViewUniform`]) applies the logical->clip
/// transform in the vertex stage.
pub const PACKED_INSTANCE_STRIDE_BYTES: usize = 36;

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
}

/// Pack one [`DrawData`] into a logical-pixel [`PackedInstance`]. The clip
/// transform is deferred to the GPU view uniform; only the sRGB->linear color
/// conversion happens here.
pub fn pack_instance(draw: &DrawData) -> PackedInstance {
    let lin = LinearRgba::from(draw.color);
    PackedInstance {
        rect_pos: [draw.position.x, draw.position.y],
        rect_size: [draw.size.x, draw.size.y],
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        radius: draw.radius,
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
    PackedInstance {
        rect_pos: [node.position.x, node.position.y],
        rect_size: [node.size.x, node.size.y],
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        radius: 0.0,
    }
}

/// `true` iff the raw `[f32; 9]` bucket layout is byte-equal to
/// [`PackedInstance`]'s stride (the pipeline-descriptor invariant). Pins the
/// agreement the instanced draw relies on.
pub fn packed_raw_stride_agrees() -> bool {
    std::mem::size_of::<PackedInstance>() == std::mem::size_of::<[f32; 9]>()
        && PACKED_INSTANCE_STRIDE_BYTES == std::mem::size_of::<[f32; 9]>()
}
