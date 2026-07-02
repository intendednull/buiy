//! The two F-tier atlas-sampling primitive shapes. The shapes are owned here
//! so text and render cannot drift (spec § 4). `buiy-text-rendering-design`
//! *emits* them (one per visible glyph); the batched node *consumes* them.

use bytemuck::{Pod, Zeroable};

/// Stride of [`GlyphAlphaInstance`] in bytes. MUST equal the per-instance
/// `array_stride` the Glyph vertex-buffer layout declares
/// (`primitive.rs::glyph_vertex_buffers`, 84 B) and the byte span
/// `coverage.wgsl`'s instance `@location`s read. `[f32;4]×5 + u32 = 84`
/// (`rect ++ uv ++ color ++ clip ++ page ++ affine`).
pub const GLYPH_ALPHA_INSTANCE_STRIDE_BYTES: usize = 84;

/// The identity 2D affine basis (`[m00, m10, m01, m11]` columns) for a glyph
/// instance — no rotation/scale. Byte-for-byte reproduces the pre-affine
/// axis-aligned coverage output (`coverage.wgsl` reduces to `rect.xy + v.uv*size`).
pub const GLYPH_IDENTITY_AFFINE: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

/// Float index of the per-glyph straight-alpha (`color[3]`) when a
/// [`GlyphAlphaInstance`] is viewed as a flat `[f32]` raw record. The fields
/// pack `rect[4] ++ uv[4] ++ color[4] ++ clip[4] ++ page` (contiguous `#[repr(C)]`,
/// no pad before `page`), so `color` is the **3rd** `[f32;4]` block and its
/// alpha lands at float index `8 + 3 = 11`. This is the GLYPH mirror of the
/// quad-tier `ALPHA_FLOAT_OFFSET` (= 7, a DIFFERENT offset on the `[f32;17]`
/// quad record): R2's degraded-group forward-composite re-tints glyph alpha,
/// and using the quad offset 7 on a glyph record would corrupt `uv[3]` (a
/// silent wrong-pixel bug). NAMED + compile-asserted so the offset is never a
/// literal `11` at the use site (R1's discipline). The fold itself writes the
/// typed `color[3]` field; this const documents the raw-view parity for the
/// spec and any byte-level reader.
///
/// [`ALPHA_FLOAT_OFFSET`]: crate::render::instance::ALPHA_FLOAT_OFFSET
pub const GLYPH_ALPHA_FLOAT_OFFSET: usize = 11;

// Tie `GLYPH_ALPHA_FLOAT_OFFSET` to the layout: `color` is the 3rd `[f32;4]`
// block (`rect`, `uv`, `color`), so its alpha (`color[3]`) is at float index
// `8 + 3`. A field reorder that moved `color` would fail this.
const _: () = assert!(
    GLYPH_ALPHA_FLOAT_OFFSET == 8 + 3,
    "GLYPH_ALPHA_FLOAT_OFFSET must index color[3] = the 3rd vec4 block's alpha"
);

/// One instance per visible glyph (or any single-channel coverage quad, e.g.
/// a generated mask stamp). The **alpha-as-color** primitive: the atlas
/// stores `R8` coverage and color is applied per-instance, so one resident
/// copy serves any tint and a theme color change never touches the atlas
/// (spec § 4.1). Not text-specific — any coverage stamp uses it.
///
/// **Byte-level contract (the alignment-bug firewall).** The `#[repr(C)]` field
/// offsets below are the single source of truth the Glyph vertex-buffer layout
/// (`render/primitive.rs::glyph_vertex_buffers`) and `coverage.wgsl`'s instance
/// `@location`s mirror byte-for-byte:
/// `rect` @0 (loc 2), `uv` @16 (loc 3), `color` @32 (loc 4), `clip` @48 (loc 5),
/// `page` @64 (loc 6), `affine` @68 (loc 7). Stride
/// [`GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`] = 84 B. The
/// `clip` AABB uses the SAME `[±INFINITY]` unclipped sentinel as
/// [`PackedInstance`](crate::render::instance::PackedInstance) (`clip = [min.x,
/// min.y, max.x, max.y]`).
///
/// `PartialEq` backs the glyph producer's value-compared publish
/// (decoration-and-paint § 6.3 damage): a content-identical rebuild keeps
/// the carrier's tick, so a caret-blink edge re-uploads the glyph buffer
/// only. Plain `[f32; 4]`s + `u32` — derive bit-equality is exactly the
/// "identical rebuilds produce bit-identical instances" compare.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GlyphAlphaInstance {
    /// Screen-space x, y, w, h (post-bridge `GlobalTransform`-resolved).
    pub rect: [f32; 4],
    /// `CoverageR8` page UV from `AtlasEntry.uv` — `[min.x, min.y, max.x, max.y]`.
    pub uv: [f32; 4],
    /// Linear-light, pre-linearized (sRGB→linear on the CPU) **straight-alpha**
    /// tint — the "alpha as color" value. NOT premultiplied: `coverage.wgsl`
    /// outputs `vec4(color.rgb, color.a * coverage)` (scales only alpha) and the
    /// Glyph pipeline blends straight-alpha `ALPHA_BLENDING`, exactly like the
    /// quad path's `PackedInstance.color`. The text-crate producer must store the
    /// straight-alpha linear color here — premultiplying would double-dim
    /// semi-transparent glyphs.
    pub color: [f32; 4],
    /// Per-instance clip AABB `[min.x, min.y, max.x, max.y]` in logical px (same
    /// space + `[±INFINITY]` unclipped sentinel as `PackedInstance`).
    pub clip: [f32; 4],
    /// Which `CoverageR8` page → selects the bind slot.
    pub page: u32,
    /// 2D affine basis (`[m00, m10, m01, m11]` columns of `GlobalTransform`'s
    /// linear part), applied about the entity origin in `coverage.wgsl` so a
    /// rotated/scaled text run or icon paints off-axis — the coverage-path mirror
    /// of [`PackedInstance.affine`](crate::render::instance::PackedInstance).
    /// [`GLYPH_IDENTITY_AFFINE`] (`[1,0,0,1]`) is the no-transform value and keeps
    /// the axis-aligned output byte-identical. Appended AFTER `page` so `color`
    /// (and thus [`GLYPH_ALPHA_FLOAT_OFFSET`]) keeps its offset.
    pub affine: [f32; 4],
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

// The alignment-bug firewall, asserted at compile time: `GlyphAlphaInstance`'s
// `#[repr(C)]` size must equal both the vertex-buffer `array_stride`
// (`GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`, 84 B) AND a contiguous pack — `5 f32×4 +
// u32 = 84` with align 4, so there is no trailing pad and instances tile the
// buffer with no gaps. A field reorder or width change that broke the agreement
// with the Glyph vertex layout / `coverage.wgsl` would fail here.
const _: () = assert!(
    core::mem::size_of::<GlyphAlphaInstance>() == GLYPH_ALPHA_INSTANCE_STRIDE_BYTES,
    "GlyphAlphaInstance size must equal the declared Glyph vertex-buffer stride"
);
const _: () = assert!(
    core::mem::size_of::<GlyphAlphaInstance>() == 4 * 4 * 4 + 4 + 4 * 4,
    "GlyphAlphaInstance must be 5 vec4 + u32 with no trailing padding"
);
