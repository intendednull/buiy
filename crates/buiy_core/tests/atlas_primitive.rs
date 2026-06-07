//! Headless layout tests for the two atlas-sampling primitive shapes.
//! Pure-CPU POD layout; no GPU adapter. Spec atlas-and-text-seam.md § 4.
use buiy_core::render::atlas::{GlyphAlphaInstance, IconInstance};

#[test]
fn glyph_alpha_instance_layout() {
    // rect[4] + uv[4] + color[4] + clip[4] = 16 f32 = 64 B, + page u32 = 4 B,
    // total 68 B before alignment. repr(C) with f32 fields aligns to 4, so
    // size = 68. Lock it so a field reorder/addition is caught.
    assert_eq!(std::mem::size_of::<GlyphAlphaInstance>(), 68);
    assert_eq!(std::mem::align_of::<GlyphAlphaInstance>(), 4);
    // Construct one; proves the public field set matches the spec.
    let g = GlyphAlphaInstance {
        rect: [0.0; 4],
        uv: [0.0; 4],
        color: [1.0, 1.0, 1.0, 1.0],
        clip: [0.0; 4],
        page: 0,
    };
    let _bytes: &[u8] = bytemuck::bytes_of(&g); // Pod
}

#[test]
fn icon_instance_layout() {
    assert_eq!(std::mem::size_of::<IconInstance>(), 68);
    assert_eq!(std::mem::align_of::<IconInstance>(), 4);
    let i = IconInstance {
        rect: [0.0; 4],
        uv: [0.0; 4],
        tint: [1.0, 1.0, 1.0, 1.0],
        clip: [0.0; 4],
        page: 0,
    };
    let _bytes: &[u8] = bytemuck::bytes_of(&i);
}
