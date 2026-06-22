//! T6 decoration painting — the authoring component, the TextSync `Attrs`
//! lowering, the pure emission mirror (Task 2), and the solid stamp
//! (Task 4). Spec: decoration-and-paint.md §§ 2–4, 9.

use bevy::prelude::*;
use buiy_core::render::atlas::{AtlasConfig, AtlasFormat, BuiyAtlas};
use buiy_core::text::{
    DecorationKind, DecorationLineStyle, DecorationLines, FamilyEntry, FontFamily, FontStack,
    SharedFontSystem, Text, TextBuffer, TextDecorations, snap_thickness, snap_y,
    solid_stamp_bitmap, solid_stamp_key, span_decoration_rects, span_x_extent, stamp_uv,
};
use cosmic_text::{DecorationMetrics, GlyphDecorationData, TextDecoration, UnderlineStyle};

// Minimal headless app: the T2/T3 text pipeline (TextSync + measure +
// TextCommit need LayoutPlugin's step sets), no render half. The shared
// `crate::support::headless_text_app` builder (#35) — including the `ThemePlugin`
// this file's local `text_app` used to add by hand.
use crate::support::headless_text_app as text_app;
// The shared condition-polled `crate::support::settle` (#35): converges the
// layout-and-text pipeline (geometry + shaping quiescence) rather than
// hard-coding a magic frame count.
use crate::support::settle;

// --- the lowering: line bits reach cosmic, spans come back -----------------

#[test]
fn underline_component_produces_decoration_spans() {
    let mut app = text_app();
    let e = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi there".into()),
            TextDecorations {
                line: DecorationLines::UNDERLINE,
                ..Default::default()
            },
        ))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    // Uniform attrs across the line → upstream merges into ONE span
    // covering every glyph (Orientation § 2).
    assert_eq!(run.decorations.len(), 1, "one merged span");
    let span = &run.decorations[0];
    assert_eq!(span.glyph_range, 0..run.glyphs.len());
    assert_eq!(
        span.data.text_decoration.underline,
        cosmic_text::UnderlineStyle::Single
    );
    assert!(!span.data.text_decoration.strikethrough);
    assert!(!span.data.text_decoration.overline);
    // Decision 1: the color builders are NEVER called — tokens resolve at
    // extract, so the Attrs tier stays None.
    assert_eq!(span.data.text_decoration.underline_color_opt, None);
    // span.color_opt mirrors the first glyph's Attrs color — Buiy never
    // sets Attrs.color_opt (TextColor resolves at extract), so tier 2 is
    // structurally None in v1 (decision 1 corollary).
    assert_eq!(span.color_opt, None);
    assert_eq!(
        span.font_size, 16.0,
        "span font_size = the FontSize default"
    );
}

#[test]
fn no_decoration_means_no_spans() {
    // has_decoration() gates span creation upstream — the zero-cost path.
    let mut app = text_app();
    let e = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi".into()),
        ))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    assert!(run.decorations.is_empty());
}

#[test]
fn all_three_lines_and_double_style_lower_together() {
    let mut app = text_app();
    let e = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi".into()),
            TextDecorations {
                line: DecorationLines::UNDERLINE
                    | DecorationLines::OVERLINE
                    | DecorationLines::LINE_THROUGH,
                style: DecorationLineStyle::Double,
                color: None,
            },
        ))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    let td = &run.decorations[0].data.text_decoration;
    assert_eq!(td.underline, cosmic_text::UnderlineStyle::Double);
    assert!(td.strikethrough);
    assert!(td.overline);
}

#[test]
fn dotted_dashed_wavy_degrade_to_solid() {
    // Decision 2: the § 9 reservation arms degrade warn-once (the
    // TextWrap::Balance precedent) — assert the lowered VALUE; the warn
    // fires once process-wide and is not asserted (AtomicBool precedent).
    for style in [
        DecorationLineStyle::Dotted,
        DecorationLineStyle::Dashed,
        DecorationLineStyle::Wavy,
    ] {
        assert_eq!(
            style.to_cosmic_underline(),
            cosmic_text::UnderlineStyle::Single,
            "{style:?} degrades to Single"
        );
    }
    assert_eq!(
        DecorationLineStyle::Double.to_cosmic_underline(),
        cosmic_text::UnderlineStyle::Double
    );
    assert_eq!(
        DecorationLineStyle::Solid.to_cosmic_underline(),
        cosmic_text::UnderlineStyle::Single
    );
}

#[test]
fn decoration_lines_default_is_empty() {
    assert!(DecorationLines::default().is_empty());
    assert!(
        !TextDecorations::default()
            .line
            .contains(DecorationLines::UNDERLINE)
    );
}

// --- Task 2: the pure emission mirror (decoration-and-paint §§ 3.1–3.3) ----

/// Hand fixture: underline-only data with round-number EM metrics.
/// offset −0.1 em (post-table convention: negative = below baseline in
/// y-down screen space after the `line_y − offset×fs` mirror), thickness
/// 0.05 em, ascent 0.75 em.
fn underline_data(style: UnderlineStyle) -> GlyphDecorationData {
    GlyphDecorationData {
        text_decoration: TextDecoration {
            underline: style,
            ..TextDecoration::new()
        },
        underline_metrics: DecorationMetrics {
            offset: -0.1,
            thickness: 0.05,
        },
        strikethrough_metrics: DecorationMetrics {
            offset: 0.3,
            thickness: 0.05,
        },
        ascent: 0.75,
    }
}

const FS: f32 = 16.0; // span font_size: metrics × 16 → offset −1.6, thickness 0.8

// --- § 3.3: the snap helpers, in isolation --------------------------------

#[test]
fn thickness_floors_at_one_whole_physical_pixel() {
    // raw 0.8 logical @1.0 → 0.8 phys → round 1 → 1.0 logical
    assert_eq!(snap_thickness(0.8, 1.0), 1.0);
    // raw 0.8 @1.25 → 1.0 phys → exactly 1 → 0.8 logical (already integral)
    assert_eq!(snap_thickness(0.8, 1.25), 0.8);
    // raw 0.3 @2.0 → 0.6 phys → max(1, round) = 1 → 0.5 logical
    assert_eq!(snap_thickness(0.3, 2.0), 0.5);
    // THE § 3.3 pin: at scale 1.5 a 1-logical-px line must become 2 whole
    // physical px (2/1.5 logical), NOT upstream's verbatim `.max(1).ceil()`
    // = 1 logical px = 1.5 physical px (the AA blur the rule prevents).
    assert_eq!(snap_thickness(1.0, 1.5), 2.0 / 1.5);
}

#[test]
fn y_snaps_to_the_physical_grid() {
    assert_eq!(snap_y(10.3, 1.0), 10.0);
    assert_eq!(snap_y(10.5, 1.0), 11.0); // round-half-up at .5 (f32 round)
    // 10.3 @1.25 → 12.875 phys → round 13 → 10.4 logical
    assert_eq!(snap_y(10.3, 1.25), 13.0 / 1.25);
}

// --- the mirrored placement math, exact numbers ----------------------------

#[test]
fn single_underline_exact_rect() {
    // origin (10.0, 20.0), line_y 12.0, scale 1.0:
    //   y_raw = 20 + 12 − (−0.1 × 16) = 33.6 → snap 34.0
    //   t     = 0.05 × 16 = 0.8        → floor 1.0
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0, // line_y
        4.0,  // line_top
        3.0,  // x_start (run-local)
        50.0, // width
        &underline_data(UnderlineStyle::Single),
        FS,
        None, // span color
        1.0,  // scale
    );
    assert_eq!(rects.len(), 1);
    let r = &rects[0];
    assert_eq!(r.kind, DecorationKind::Underline);
    assert_eq!(r.rect, [13.0, 34.0, 50.0, 1.0]); // x = origin.x + x_start (UNsnapped)
    assert_eq!(r.color_opt, None);
}

#[test]
fn single_underline_fractional_scale_exact_rect() {
    // scale 1.25: y_raw 33.6 → 42 phys → exactly 33.6 logical;
    //             t_raw 0.8 → 1.0 phys → 0.8 logical.
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0,
        4.0,
        3.0,
        50.0,
        &underline_data(UnderlineStyle::Single),
        FS,
        None,
        1.25,
    );
    assert_eq!(rects[0].rect, [13.0, 33.6, 50.0, 0.8]);
}

#[test]
fn double_underline_gap_equals_thickness() {
    // § 3.2: two rects, gap = thickness ⇒ rect2.y = rect1.y + 2 × t.
    // scale 1.0 → t = 1.0 (floored), y = 34.0 ⇒ second at 36.0.
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0,
        4.0,
        3.0,
        50.0,
        &underline_data(UnderlineStyle::Double),
        FS,
        None,
        1.0,
    );
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].rect, [13.0, 34.0, 50.0, 1.0]);
    assert_eq!(rects[1].rect, [13.0, 36.0, 50.0, 1.0]);
    assert_eq!(rects[1].kind, DecorationKind::Underline);
}

#[test]
fn line_through_uses_strikeout_metrics() {
    // y_raw = 20 + 12 − (0.3 × 16) = 27.2 → snap 27.0; t = 0.8 → 1.0.
    let mut data = underline_data(UnderlineStyle::None);
    data.text_decoration.strikethrough = true;
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0,
        4.0,
        3.0,
        50.0,
        &data,
        FS,
        None,
        1.0,
    );
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].kind, DecorationKind::LineThrough);
    assert_eq!(rects[0].rect, [13.0, 27.0, 50.0, 1.0]);
}

#[test]
fn overline_clamps_to_line_top_and_reuses_underline_thickness() {
    let mut data = underline_data(UnderlineStyle::None);
    data.text_decoration.overline = true;
    // Unclamped: ascent 0.25 em → y_local = 12 − 0.25 × 16 = 8.0 ≥ line_top
    // 4.0 → y_raw = 20 + 8 = 28.0 → snap 28.0. (The plan's fixture ascent
    // 0.75 lands y_local at 0, BELOW line_top — upstream's `.max(line_top)`
    // would clamp it, so it cannot witness the unclamped arm.)
    data.ascent = 0.25;
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0,
        4.0,
        3.0,
        50.0,
        &data,
        FS,
        None,
        1.0,
    );
    assert_eq!(rects[0].kind, DecorationKind::Overline);
    assert_eq!(rects[0].rect, [13.0, 28.0, 50.0, 1.0]); // underline t reused

    // Clamped: ascent 0.9 em → line_y − 14.4 = −2.4 < line_top 4.0 →
    // y = origin.y + line_top = 24.0 (clamp BEFORE the origin fold + snap).
    data.ascent = 0.9;
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0,
        4.0,
        3.0,
        50.0,
        &data,
        FS,
        None,
        1.0,
    );
    assert_eq!(rects[0].rect[1], 24.0);
}

#[test]
fn color_precedence_mirrors_upstream_per_kind() {
    use cosmic_text::Color as CColor;
    let span_color = CColor::rgb(1, 2, 3);
    let kind_color = CColor::rgb(9, 9, 9);

    // Tier 2: span text color when the -color tier is empty.
    let rects = span_decoration_rects(
        Vec2::ZERO,
        12.0,
        4.0,
        0.0,
        10.0,
        &underline_data(UnderlineStyle::Single),
        FS,
        Some(span_color),
        1.0,
    );
    assert_eq!(rects[0].color_opt, Some(span_color));

    // Tier 1: the per-kind *_color_opt wins over the span color.
    let mut data = underline_data(UnderlineStyle::Single);
    data.text_decoration.underline_color_opt = Some(kind_color);
    let rects = span_decoration_rects(
        Vec2::ZERO,
        12.0,
        4.0,
        0.0,
        10.0,
        &data,
        FS,
        Some(span_color),
        1.0,
    );
    assert_eq!(rects[0].color_opt, Some(kind_color));

    // Tier 3 (both None) → None: the CALLER falls back to the resolved
    // entity foreground (currentColor) — asserted at the producer in Task 3.
    let rects = span_decoration_rects(
        Vec2::ZERO,
        12.0,
        4.0,
        0.0,
        10.0,
        &underline_data(UnderlineStyle::Single),
        FS,
        None,
        1.0,
    );
    assert_eq!(rects[0].color_opt, None);
}

#[test]
fn zero_width_spans_emit_nothing() {
    let rects = span_decoration_rects(
        Vec2::ZERO,
        12.0,
        4.0,
        0.0,
        0.0,
        &underline_data(UnderlineStyle::Single),
        FS,
        None,
        1.0,
    );
    assert!(rects.is_empty());
    // And the extent helper refuses empty/degenerate input upstream of it.
    assert_eq!(span_x_extent(&[], &(0..0)), None);
}

// --- the upstream-drift guard: real shaping, pinned numbers ----------------

#[test]
fn drift_guard_real_spans_from_the_embedded_font() {
    // Mirror-not-call (§ 3.1) means a cosmic-text bump that changes span
    // production or decoration_metrics must fail HERE, loudly, instead of
    // silently shifting goldens. Shape real text against the committed
    // embedded font (deterministic — registered_fonts_db only) and pin the
    // span's metric values as literals.
    use cosmic_text::{Attrs, Metrics, Shaping};
    let fonts = SharedFontSystem::new();
    let mut fs = fonts.lock();
    let mut buffer = cosmic_text::Buffer::new_empty(Metrics::new(16.0, 19.2));
    buffer.set_size(Some(400.0), Some(100.0));
    buffer.set_text(
        "Hi there",
        &Attrs::new()
            .underline(cosmic_text::UnderlineStyle::Single)
            .strikethrough()
            .overline(),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut fs, false);
    let run = buffer.layout_runs().next().expect("one run");
    assert_eq!(run.decorations.len(), 1, "uniform attrs merge to one span");
    let span = &run.decorations[0];
    assert_eq!(span.glyph_range, 0..run.glyphs.len());
    assert_eq!(span.font_size, 16.0);

    const EPS: f32 = 1e-6;
    let m = &span.data;
    // PIN(capture): Fira Sans Regular latin subset (the committed embedded
    // default font, upem 1000), via upstream decoration_metrics
    // (shape.rs:686): post.underlinePosition −75 / post.underlineThickness
    // 50 / OS/2 yStrikeoutPosition 316 / hhea ascent 935, each ÷ upem.
    // Captured once via `dbg!(span.data)`; bit-stable until the font
    // artifact or upstream's decoration_metrics changes — exactly the two
    // drifts this test exists to catch.
    const PINNED_UNDERLINE_OFFSET: f32 = -0.075;
    const PINNED_UNDERLINE_THICKNESS: f32 = 0.05;
    const PINNED_STRIKEOUT_OFFSET: f32 = 0.316;
    const PINNED_ASCENT: f32 = 0.935;
    assert!((m.underline_metrics.offset - PINNED_UNDERLINE_OFFSET).abs() < EPS);
    assert!((m.underline_metrics.thickness - PINNED_UNDERLINE_THICKNESS).abs() < EPS);
    assert!((m.strikethrough_metrics.offset - PINNED_STRIKEOUT_OFFSET).abs() < EPS);
    assert!((m.ascent - PINNED_ASCENT).abs() < EPS);

    // And the x-extent helper against the real glyph slice (the RTL-safe
    // min/max walk): equals the run's own extremes.
    let (x, w) = span_x_extent(run.glyphs, &span.glyph_range).expect("non-empty");
    let min = run.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
    let max = run
        .glyphs
        .iter()
        .map(|g| g.x + g.w)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((x - min).abs() < EPS && (w - (max - min)).abs() < EPS);
}

#[test]
fn rtl_span_extent_is_the_min_max_envelope() {
    // RTL paragraphs store glyphs in right-to-left order (upstream's own
    // comment) — `span_x_extent` must return the min/max envelope, NOT a
    // first/last walk (which would compute a negative width here). The T5
    // corpus idiom: the committed Arabic fixture font through the
    // production registration path.
    let mut app = text_app();
    app.update();
    crate::support::register_fixture_font(
        &mut app,
        "Noto Sans Arabic",
        "NotoSansArabic-arabic.ttf",
    );
    let e = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default()
                .width_px(400.0)
                .height_px(100.0),
            Text("السلام عليكم".into()),
            FontFamily(FontStack(vec![FamilyEntry::Named(
                "Noto Sans Arabic".into(),
            )])),
            TextDecorations {
                line: DecorationLines::UNDERLINE,
                ..Default::default()
            },
        ))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    assert!(run.rtl, "an all-Arabic paragraph is first-strong RTL");
    assert_eq!(run.decorations.len(), 1, "uniform attrs merge to one span");
    let span = &run.decorations[0];
    let glyphs = &run.glyphs[span.glyph_range.clone()];
    assert!(
        glyphs.first().expect("glyphs").x > glyphs.last().expect("glyphs").x,
        "the RTL premise: logical-first glyph sits visually RIGHT of the last"
    );
    let (x, w) = span_x_extent(run.glyphs, &span.glyph_range).expect("non-empty");
    let min = glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
    let max = glyphs
        .iter()
        .map(|g| g.x + g.w)
        .fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(x, min, "x_start is the envelope min, not the first glyph");
    assert_eq!(w, max - min, "width is the envelope, not first→last");
    assert!(w > 0.0);
}

// --- Task 4: the solid stamp (decoration-and-paint § 4.3) ------------------

#[test]
fn stamp_key_is_mask_kind_and_aliases_nothing() {
    let key = solid_stamp_key();
    assert_eq!(
        key.0.as_slice(),
        [3u8, 0u8].as_slice(),
        "Mask kind byte + sub-id 0"
    );
    assert_ne!(
        key.0.len(),
        buiy_core::text::GLYPH_KEY_LEN,
        "can never alias a glyph key"
    );
}

#[test]
fn stamp_bitmap_is_one_solid_white_texel() {
    let bmp = solid_stamp_bitmap();
    assert_eq!(bmp.size, UVec2::ONE);
    assert!(matches!(bmp.format, AtlasFormat::CoverageR8));
    assert_eq!(bmp.data, vec![255u8]);
}

#[test]
fn stamp_uv_is_the_cell_midpoint_replicated() {
    // Decision 9: constant interpolated uv → every fragment samples the
    // center texel — exact under the pinned Nearest sampler (and any other).
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let entry = atlas.get_or_insert(
        solid_stamp_key(),
        AtlasFormat::CoverageR8,
        solid_stamp_bitmap,
    );
    let uv = stamp_uv(&entry);
    let c = entry.uv.center();
    assert_eq!(uv, [c.x, c.y, c.x, c.y]);
}

#[test]
fn register_render_world_pushes_the_warmup_request() {
    // The finish-ordering seam, headless form (decision 10): a bare SubApp
    // (the register_render_world test idiom) receives the queue + exactly
    // one stamp request; draining it makes the stamp resident pre-paint.
    use bevy::app::SubApp;
    use buiy_core::render::atlas::AtlasWarmupQueue;
    use buiy_core::text::register_render_world;
    let mut render_app = SubApp::new();
    let fonts = SharedFontSystem::new();
    register_render_world(&mut render_app, &fonts);
    let mut queue = render_app
        .world_mut()
        .remove_resource::<AtlasWarmupQueue>()
        .expect("queue init'd by the text plugin half");
    assert_eq!(queue.len(), 1, "exactly one push: the solid stamp");
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    atlas.drain_warmup(&mut queue);
    let entry = atlas.get(&solid_stamp_key()).expect("warmup-pinned");
    assert_eq!(entry.px.size(), UVec2::ONE);
}
