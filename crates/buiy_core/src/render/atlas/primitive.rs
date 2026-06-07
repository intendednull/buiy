//! The two F-tier atlas-sampling primitive shapes. The shapes are owned here
//! so text and render cannot drift (spec § 4). `buiy-text-rendering-design`
//! *emits* them (one per visible glyph); the batched node *consumes* them.

use bytemuck::{Pod, Zeroable};

/// One instance per visible glyph (or any single-channel coverage quad, e.g.
/// a generated mask stamp). The **alpha-as-color** primitive: the atlas
/// stores `R8` coverage and color is applied per-instance, so one resident
/// copy serves any tint and a theme color change never touches the atlas
/// (spec § 4.1). Not text-specific — any coverage stamp uses it.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GlyphAlphaInstance {
    /// Screen-space x, y, w, h (post-bridge `GlobalTransform`-resolved).
    pub rect: [f32; 4],
    /// `CoverageR8` page UV from `AtlasEntry.uv`.
    pub uv: [f32; 4],
    /// Linear-light premultiplied tint — the "alpha as color" value.
    pub color: [f32; 4],
    /// `ClipRect`, the per-instance clip (clip-and-transform.md).
    pub clip: [f32; 4],
    /// Which `CoverageR8` page → selects the bind slot.
    pub page: u32,
}

/// One instance per full-color stamp — themed raster icons, color-emoji
/// glyph bitmaps the text spec produces as `Rgba8`. **No recolor trick**:
/// the atlas stores the color and the primitive samples it straight, with an
/// optional multiplied tint (spec § 4.2). Mirrors GPUI's `PolychromeSprite`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct IconInstance {
    pub rect: [f32; 4],
    /// `ColorRgba8` page UV.
    pub uv: [f32; 4],
    /// Multiplied over the sampled color (`[1,1,1,1]` = no tint).
    pub tint: [f32; 4],
    pub clip: [f32; 4],
    pub page: u32,
}
