//! The Buiy view uniform: the per-view logical-pixel -> clip-space affine that
//! replaces the Phase-0 per-instance y-flip / radius approximation in
//! `render::instance`. It plays the same role for Buiy that
//! `bevy_render::view::ViewUniform` plays for the engine: one per-view
//! transform uploaded to a UBO, applied in the vertex stage, so the per-
//! instance record can stay in LOGICAL-pixel units.
//!
//! Spec: architecture.md § 3.2 (the hybrid handoff retires the per-instance
//! coordinate hack) + color-and-forced-colors.md § 1.1 (color stays CPU-pre-
//! linearized; only the COORDINATE packing moves to this uniform).

use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// CPU size of [`BuiyViewUniform`] in bytes. Must be a multiple of 16 for
/// std140 UBO alignment. 2 columns of `vec4` (32 B) + `scale_factor` + 3 pad
/// (16 B) = 48 B.
pub const VIEW_UNIFORM_SIZE_BYTES: usize = 48;

// Compile-time guard against CPU/GPU layout drift: the std140 view uniform must
// stay 48 bytes = 12 f32. It is re-grouped into `[Vec4; 3]` (also 48 B) for
// upload in `render::prepare`, and `as_std140_array` emits exactly 12 floats.
// If a field is added without keeping the struct at 48 B, the GPU would read a
// mismatched layout — and there is no wgpu adapter on CI to catch it at runtime,
// so pin it here.
const _: () = assert!(
    core::mem::size_of::<BuiyViewUniform>() == VIEW_UNIFORM_SIZE_BYTES
        && VIEW_UNIFORM_SIZE_BYTES == 12 * core::mem::size_of::<f32>(),
);

/// Per-view logical-pixel -> clip-space transform, plus the view
/// `scale_factor`. Uploaded once per view per frame in
/// `RenderSystems::Prepare`; applied in the vertex stage so [`PackedInstance`]
/// stays in logical-pixel units.
///
/// The affine is stored as two `vec4` columns (`col0`, `col1`) encoding the
/// 2D affine `clip = M * logical + t`:
/// - `col0 = [m00, m01, 0, tx_unused]` — but Buiy's logical->clip is purely
///   diagonal-scale + translate (no shear), so `col0 = [sx, 0, 0, tx]` and
///   `col1 = [0, sy, 0, ty]` where `clip.x = sx*lx + tx`, `clip.y = sy*ly + ty`.
///
/// [`PackedInstance`]: crate::render::instance::PackedInstance
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct BuiyViewUniform {
    /// `[sx, 0, 0, tx]` — x maps as `clip.x = sx*logical.x + tx`.
    col0: [f32; 4],
    /// `[0, sy, 0, ty]` — y maps as `clip.y = sy*logical.y + ty`.
    col1: [f32; 4],
    /// Device pixels per logical pixel for this view. Carried so the GPU can
    /// keep the SDF / corner-radius math in logical px (no non-square hack).
    scale_factor: f32,
    _pad: [f32; 3],
}

impl BuiyViewUniform {
    /// Build the uniform for a view of the given **logical** window size and
    /// `scale_factor`. The y-flip lives here, once: logical (0,0) (top-left)
    /// maps to clip (-1, +1); logical (w,h) maps to clip (+1, -1).
    pub fn for_view(logical_size: Vec2, scale_factor: f32) -> Self {
        let sx = 2.0 / logical_size.x;
        let sy = -2.0 / logical_size.y; // single y-flip
        Self {
            col0: [sx, 0.0, 0.0, -1.0],
            col1: [0.0, sy, 0.0, 1.0],
            scale_factor,
            _pad: [0.0; 3],
        }
    }

    /// Apply the affine to a logical-pixel point, yielding a clip-space point.
    /// The CPU mirror of the vertex-stage transform — used by tests and by
    /// any CPU-side bounds math.
    pub fn apply(&self, logical: Vec2) -> Vec2 {
        Vec2::new(
            self.col0[0] * logical.x + self.col0[3],
            self.col1[1] * logical.y + self.col1[3],
        )
    }

    /// Device-pixels-per-logical-pixel for this view.
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Flatten to the std140 UBO array: `col0 ++ col1 ++ [scale_factor, 0, 0,
    /// 0]`. This is the payload the prepare phase writes into the per-view
    /// uniform buffer; the WGSL `BuiyView` struct reads it back as
    /// `col0: vec4 ++ col1: vec4 ++ params: vec4` (the `scale_factor` is
    /// `params.x`).
    pub fn as_std140_array(&self) -> [f32; 12] {
        [
            self.col0[0],
            self.col0[1],
            self.col0[2],
            self.col0[3],
            self.col1[0],
            self.col1[1],
            self.col1[2],
            self.col1[3],
            self.scale_factor,
            0.0,
            0.0,
            0.0,
        ]
    }
}
