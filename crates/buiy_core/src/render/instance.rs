//! Per-instance data layout for the rounded-rect pipeline. The struct stride
//! must equal the per-instance `array_stride` declared in
//! `pipeline.rs::register` (currently 36). Phase 0 closeout converts logical
//! pixels → clip-space units on the CPU here, per the path (a) decision in
//! `shader.wgsl`'s former TODO comment.
//!
//! v0.x will replace this with a view uniform (`buiy-render-pipeline-design`),
//! at which point the conversion moves to the vertex stage and `InstanceData`
//! shrinks back to logical-pixel units.

use crate::render::DrawData;
use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Stride must match `pipeline.rs::register` instance-buffer layout (36 B).
pub const INSTANCE_STRIDE_BYTES: usize = 36;

/// One instance record. Fields match `Instance` in `shader.wgsl` 1:1.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    /// Top-left in clip space (-1..+1, y up).
    pub rect_pos: [f32; 2],
    /// Width / height in clip space; height is negative because UI-space y is
    /// down-positive but clip-space y is up-positive (single y-flip lives in
    /// the size, not the position, so the shader's `rect_pos + uv * rect_size`
    /// remains correct top-down).
    pub rect_size: [f32; 2],
    /// Linear RGBA. Pipeline target is `Rgba8UnormSrgb`, so the GPU re-encodes
    /// to sRGB on write.
    pub color: [f32; 4],
    /// Corner radius in clip-space units. Phase 0 closeout uses
    /// `2.0 / min(window.x, window.y)` as the px→clip conversion to keep
    /// corners visually round on non-square windows; v0.x view uniform
    /// removes the approximation.
    pub radius: f32,
}

/// Convert one [`DrawData`] (logical-pixel UI space) into an [`InstanceData`]
/// (clip space) for the given window size in logical pixels.
pub fn to_instance(draw: &DrawData, window_size: Vec2) -> InstanceData {
    let inv_w = 2.0 / window_size.x;
    let inv_h = 2.0 / window_size.y;
    let inv_min = 2.0 / window_size.x.min(window_size.y);

    let rect_pos = [
        draw.position.x * inv_w - 1.0,
        // y-flip: UI top (px=0) → clip top (+1).
        1.0 - draw.position.y * inv_h,
    ];
    let rect_size = [draw.size.x * inv_w, -draw.size.y * inv_h];

    let lin = LinearRgba::from(draw.color);
    let color = [lin.red, lin.green, lin.blue, lin.alpha];

    InstanceData {
        rect_pos,
        rect_size,
        color,
        radius: draw.radius * inv_min,
    }
}
