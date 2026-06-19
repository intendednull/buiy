//! Headless decoration GEOMETRY off `ExtractedTextQuads` (#28, audit
//! 2026-06-18 T2.9 — the decoration tier-misplacement push-down).
//!
//! `text_decoration_gpu.rs` previously asserted decoration band COUNT,
//! POSITION (relative to the glyph ink), THICKNESS, and the double-underline
//! GAP from rasterized PIXELS in the `#[ignore]` GPU lane. That geometry is
//! observable at a far cheaper tier: it rides `ExtractedTextQuads` (the
//! quad-tier carrier) and `ExtractedGlyphs` (the glyph ink envelope), both
//! produced CPU-side by `extract_buiy_glyphs` with NO wgpu adapter. The GPU
//! golden's value is the antialiasing residue confidence — NOT re-deriving
//! band positions from pixels a row-classifier had to reverse-engineer out of
//! the shader's AA math.
//!
//! So per the audit: the count/pos/thickness/gap assertions move HERE,
//! observed directly off the extract carriers (this is the verifiable
//! deliverable); the GPU file keeps exactly ONE golden per decoration kind for
//! the rasterization-residue confidence (verified in the Phase 3 GPU lane).
//!
//! This sits alongside the producer-tier `text_decoration.rs` (which pins the
//! exact `span_decoration_rects` algebra on hand fixtures) and the
//! emission-tier asserts in `text_extract.rs` (carrier wiring / damage gate):
//! THIS file is the END-TO-END geometry tier — real entities through TextSync
//! → TextCommit → extract — that the GPU goldens were over-serving.

mod support;

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::TextColor;
use buiy_core::render::extract::TextQuad;
use buiy_core::text::{
    DecorationLineStyle, DecorationLines, FontSize, Text, TextDecorations, solid_stamp_key,
};
use std::borrow::Cow;
use std::ops::Range;
use support::extract_harness::TextExtractHarness;

/// The `text_decoration_gpu.rs::spawn_decorated_fixture` shape, headless: "Hi"
/// at 40 px — no descenders, so glyph ink never crosses below the baseline and
/// band-vs-ink positioning is unambiguous (the same reasoning the GPU fixture
/// relied on, now observed off the extract carriers instead of pixels).
const TEXT_TOKEN: &str = "test.text";
const DECO_TOKEN: &str = "test.deco";

fn deco_red() -> Color {
    Color::srgb(1.0, 0.0, 0.0)
}

fn red_deco(line: DecorationLines) -> TextDecorations {
    TextDecorations {
        line,
        color: Some(ColorToken::Token(Cow::Borrowed(DECO_TOKEN))),
        ..Default::default()
    }
}

fn spawn_decorated_40px(h: &mut TextExtractHarness, deco: TextDecorations) -> Entity {
    {
        let mut theme = h.app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(DECO_TOKEN.into(), deco_red());
    }
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            deco,
        ))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(128.0)
                .height_px(64.0),
        ))
        .add_child(text);
    text
}

// --- ink / band geometry off the extract carriers (no adapter) --------------

/// The glyph INK envelope `[top, bottom)` in logical-px window space, read off
/// `ExtractedGlyphs` (real glyph instances only — stamps have a degenerate uv
/// `uv[0] == uv[2]`, excluded so a line-through stamp never widens the ink).
fn glyph_ink(h: &TextExtractHarness) -> Range<f32> {
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for g in &h.glyphs().glyphs {
        if g.uv[0] == g.uv[2] {
            continue; // a solid stamp, not glyph ink
        }
        top = top.min(g.rect[1]);
        bottom = bottom.max(g.rect[1] + g.rect[3]);
    }
    assert!(top.is_finite() && bottom.is_finite(), "glyph ink emitted");
    top..bottom
}

/// The decoration quads sorted top→bottom by `position.y` — the band list the
/// GPU test reverse-derived from strong-red pixel rows.
fn bands(h: &TextExtractHarness) -> Vec<TextQuad> {
    let mut q = h.text_quads().quads.clone();
    q.sort_by(|a, b| a.position.y.total_cmp(&b.position.y));
    q
}

// --- count + position + thickness (moved from text_decoration_gpu.rs) -------

#[test]
fn underline_emits_one_band_below_the_glyph_ink() {
    // Mirrors `underline_paints_one_band_below_the_glyphs` (GPU), off the
    // carriers: exactly one quad, BELOW the ink, height = the § 3.3 floored
    // thickness (0.05 em × 40 px = 2.0 logical → 2 whole physical px at
    // scale 1.0 = 2.0 logical — the GPU test's "2 physical px" claim).
    let mut h = TextExtractHarness::new();
    spawn_decorated_40px(&mut h, red_deco(DecorationLines::UNDERLINE));
    h.settle();

    let ink = glyph_ink(&h);
    let bands = bands(&h);
    assert_eq!(bands.len(), 1, "exactly one underline band: {bands:?}");
    let band = &bands[0];
    assert!(
        band.position.y >= ink.end,
        "the underline (y={}) sits BELOW the glyph ink ({ink:?}) — \
         'Hi' has no descenders",
        band.position.y
    );
    assert_eq!(
        band.size.y, 2.0,
        "band height = the § 3.3 floored thickness in logical px (2 phys @ scale 1)"
    );
    assert!(band.size.x > 0.0, "span-extent width");
    assert_eq!(band.color, deco_red(), "the resolved decoration color");
    // No glyph-tier stamp leaks into the underline path.
    assert!(
        !h.resident_keys().contains(&solid_stamp_key()),
        "underline is quad-tier — no solid stamp"
    );
}

#[test]
fn overline_emits_one_band_above_the_glyph_ink() {
    // Mirrors `overline_paints_above_the_glyphs` (GPU): one quad, fully ABOVE
    // the ink, height = the reused underline thickness (2 logical px).
    let mut h = TextExtractHarness::new();
    spawn_decorated_40px(&mut h, red_deco(DecorationLines::OVERLINE));
    h.settle();

    let ink = glyph_ink(&h);
    let bands = bands(&h);
    assert_eq!(bands.len(), 1, "exactly one overline band: {bands:?}");
    let band = &bands[0];
    assert!(
        band.position.y + band.size.y <= ink.start,
        "the overline (bottom={}) sits ABOVE the glyph ink ({ink:?})",
        band.position.y + band.size.y
    );
    assert_eq!(band.size.y, 2.0, "overline reuses the underline thickness");
}

#[test]
fn double_underline_emits_two_bands_with_gap_equal_to_thickness() {
    // Mirrors `double_underline_paints_two_bands_with_a_thickness_gap` (GPU):
    // two equal-thickness bands below the ink, separated by a gap of exactly
    // one thickness (§ 3.2: gap = thickness ⇒ second rect at y + 2 × t).
    let mut h = TextExtractHarness::new();
    spawn_decorated_40px(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE,
            style: DecorationLineStyle::Double,
            color: Some(ColorToken::Token(Cow::Borrowed(DECO_TOKEN))),
        },
    );
    h.settle();

    let ink = glyph_ink(&h);
    let bands = bands(&h);
    assert_eq!(bands.len(), 2, "Double = exactly two bands: {bands:?}");
    let (first, second) = (&bands[0], &bands[1]);
    assert!(
        first.position.y >= ink.end,
        "both bands below the ink ({ink:?})"
    );
    assert_eq!(first.size.y, second.size.y, "equal thicknesses: {bands:?}");
    let t = first.size.y;
    assert_eq!(t, 2.0, "the floored underline thickness");
    // § 3.2: gap == thickness ⇒ the second band's top sits one thickness
    // below the first band's bottom (second.y = first.y + 2·t).
    let gap = second.position.y - (first.position.y + first.size.y);
    assert!(
        (gap - t).abs() < 1e-3,
        "gap == band thickness (gap = thickness): gap={gap}, t={t}"
    );
}

#[test]
fn line_through_emits_a_stamp_over_the_glyph_ink_no_quad() {
    // Mirrors `line_through_paints_over_the_glyph_ink` (GPU), off the
    // carriers: line-through is GLYPH-tier (§ 4.2) — a solid stamp instance
    // (degenerate uv) INTERSECTING the ink, NOT a quad. The quad carrier
    // stays empty; the stamp key is resident. (The GPU golden keeps the
    // pixel-level "stamp paints OVER the glyph ink" paint-order confidence —
    // that ordering is the residue claim cheaper tiers cannot observe.)
    let mut h = TextExtractHarness::new();
    spawn_decorated_40px(&mut h, red_deco(DecorationLines::LINE_THROUGH));
    h.settle();

    assert!(
        h.text_quads().quads.is_empty(),
        "line-through is glyph-tier — the quad carrier stays empty"
    );
    let stamps: Vec<_> = h
        .glyphs()
        .glyphs
        .iter()
        .filter(|g| g.uv[0] == g.uv[2])
        .copied()
        .collect();
    assert_eq!(
        stamps.len(),
        1,
        "exactly one solid stamp (the line-through)"
    );
    assert!(
        h.resident_keys().contains(&solid_stamp_key()),
        "the stamp key is resident"
    );
    let stamp = stamps[0];
    let ink = glyph_ink(&h);
    let stamp_top = stamp.rect[1];
    let stamp_bottom = stamp.rect[1] + stamp.rect[3];
    assert!(
        stamp_top < ink.end && stamp_bottom > ink.start,
        "the line-through stamp ([{stamp_top}, {stamp_bottom}]) INTERSECTS the ink ({ink:?})"
    );
    assert_eq!(
        stamp.rect[3], 2.0,
        "stamp height = the § 3.3 floored strikeout thickness (2 phys @ scale 1)"
    );
}
