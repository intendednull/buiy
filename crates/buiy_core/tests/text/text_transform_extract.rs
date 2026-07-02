//! `extract_buiy_glyphs` threads the entity's 2D affine (rotate/scale) onto every
//! emitted `GlyphAlphaInstance`, and re-pivots each glyph's origin about the
//! entity's transform-origin, so a rotated/scaled text run paints off-axis
//! (`docs/specs/2026-07-01-glyph-affine-transform-design.md`). This is the Tier-2
//! HEADLESS gate (lowest tier that observes the dropped affine); the pixel
//! application through `coverage.wgsl` is the GPU `#[ignore]` lane
//! (`render_transform_paint_gpu.rs`).

use crate::support::extract_harness::TextExtractHarness;
use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::GLYPH_IDENTITY_AFFINE;
use buiy_core::text::{FontSize, Text};

/// "Hi!" (3 visible glyphs) carrying `style` on the TEXT entity itself, under a
/// sized column root (so layout has a box). Returns the text entity.
fn spawn_text(h: &mut TextExtractHarness, style: Style) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((Node, style, Text(String::from("Hi!")), FontSize(16.0)))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    text
}

/// The entity's 2D affine basis `[m00, m10, m01, m11]` — the value the producer
/// should stamp onto each glyph.
fn entity_affine(h: &TextExtractHarness, e: Entity) -> [f32; 4] {
    let gt = h
        .app
        .world()
        .get::<GlobalTransform>(e)
        .expect("GlobalTransform");
    let m = gt.affine().matrix3;
    [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y]
}

#[test]
fn unrotated_text_glyphs_carry_identity_affine() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h, Style::default());
    h.settle();
    let glyphs = &h.glyphs().glyphs;
    assert!(!glyphs.is_empty(), "visible glyphs emitted");
    for g in glyphs {
        assert_eq!(
            g.affine, GLYPH_IDENTITY_AFFINE,
            "an untransformed glyph carries the identity affine (byte-stable path)"
        );
    }
}

#[test]
fn rotated_text_glyphs_carry_the_entity_affine_and_repivot() {
    // Unrotated baseline: capture the glyph origins.
    let mut base = TextExtractHarness::new();
    spawn_text(&mut base, Style::default());
    base.settle();
    let base_origins: Vec<[f32; 2]> = base
        .glyphs()
        .glyphs
        .iter()
        .map(|g| [g.rect[0], g.rect[1]])
        .collect();
    assert!(!base_origins.is_empty(), "baseline glyphs emitted");

    // Rotated 0.5 rad (a non-right angle — a 90° single-glyph turn could alias).
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h, Style::default().rotate_z(0.5));
    h.settle();
    let affine = entity_affine(&h, text);
    assert_ne!(
        affine, GLYPH_IDENTITY_AFFINE,
        "the rotated entity's GlobalTransform has a non-identity 2D affine"
    );
    let glyphs = &h.glyphs().glyphs;
    assert_eq!(
        glyphs.len(),
        base_origins.len(),
        "same glyph count as the baseline"
    );
    for g in glyphs {
        assert_eq!(
            g.affine, affine,
            "each glyph carries the entity's composed 2D affine (not dropped/identity)"
        );
    }
    // The re-pivot moved at least one glyph's ORIGIN off the axis-aligned baseline
    // (a dropped-affine producer would leave the origins where they were).
    let moved = glyphs
        .iter()
        .zip(&base_origins)
        .any(|(g, b)| (g.rect[0] - b[0]).abs() > 0.5 || (g.rect[1] - b[1]).abs() > 0.5);
    assert!(
        moved,
        "the rotation re-pivots the glyph origins off the unrotated positions"
    );
}

#[test]
fn scaled_text_glyphs_carry_the_scale_affine() {
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h, Style::default().scale(2.0));
    h.settle();
    let affine = entity_affine(&h, text);
    // A uniform 2x scale: the basis diagonal is 2, off-diagonal 0.
    assert!(
        (affine[0] - 2.0).abs() < 1e-4 && (affine[3] - 2.0).abs() < 1e-4,
        "2x scale basis, got {affine:?}"
    );
    let glyphs = &h.glyphs().glyphs;
    assert!(!glyphs.is_empty(), "scaled glyphs emitted");
    for g in glyphs {
        assert_eq!(g.affine, affine, "each glyph carries the 2x scale affine");
    }
}
