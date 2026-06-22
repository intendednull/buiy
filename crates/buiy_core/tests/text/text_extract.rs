//! `extract_buiy_glyphs` — emission + THE § 6.2 damage gate, headless on
//! the adapterless extract harness (verification § 1.2; § 12 headless c).

use crate::support::extract_harness::TextExtractHarness;
use bevy::asset::uuid_handle;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::{AtlasConfig, GlyphAlphaInstance};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{ClipRect, CssVisibility, TextColor};
use buiy_core::text::edit::{Placeholder, PlaceholderActive, TextEditState};
use buiy_core::text::{
    BuiyFont, DecorationLines, FamilyEntry, FontDbLineage, FontDisplay, FontFaceDescriptors,
    FontFamily, FontRegistry, FontSize, FontStack, FontsGeneration, GenericFamily, Text,
    TextDecorations, solid_stamp_key, stamp_uv,
};
use buiy_core::theme::Theme;
use cosmic_text::Metrics;
use std::borrow::Cow;
use std::time::Duration;

/// "Hi!" under a sized column root: 3 non-whitespace glyphs. Returns the
/// root too — the T8 entity-run tests add siblings / respawn under it.
fn spawn_text_with_root(h: &mut TextExtractHarness) -> (Entity, Entity) {
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
        ))
        .id();
    let root = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text)
        .id();
    (text, root)
}

/// "Hi!" under a sized column root: 3 non-whitespace glyphs.
fn spawn_text(h: &mut TextExtractHarness) -> Entity {
    spawn_text_with_root(h).0
}

fn set_cursor(h: &mut TextExtractHarness, pos: Vec2) {
    let mut q = h
        .app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>();
    q.single_mut(h.app.world_mut())
        .unwrap()
        .set_cursor_position(Some(pos));
}

fn set_scale(h: &mut TextExtractHarness, scale: f32) {
    let mut q = h
        .app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>();
    q.single_mut(h.app.world_mut())
        .unwrap()
        .resolution
        .set_scale_factor(scale);
}

#[test]
fn emits_one_instance_per_visible_glyph_with_resident_keys() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();

    assert_eq!(h.glyph_count(), 3, "one instance per non-whitespace glyph");
    assert_eq!(h.resident_keys().len(), 3);
    assert_eq!(
        h.glyphs().entity_runs.len(),
        1,
        "one entity-run for the single emitting entity (T8 D1)"
    );
    for key in h.resident_keys() {
        assert!(
            h.atlas().get(&key).is_some(),
            "every emitted key is resident"
        );
    }
    // Geometry sanity: a 16px line near the content origin — the exact
    // numbers are pinned by text_glyph_math.rs; here we bound the fold.
    let first = h.glyphs().glyphs[0];
    assert!(
        first.rect[2] > 0.0 && first.rect[3] > 0.0,
        "non-degenerate rect"
    );
    // Loose bound: baseline-folded y of a 16px line-1 glyph (bearings can
    // nudge a texel or two above the integer baseline truncation).
    assert!(
        first.rect[1] > -4.0 && first.rect[1] < 24.0,
        "baseline-folded y in line 1"
    );
    // Unclipped fixture: the ±INF sentinel.
    assert_eq!(first.clip[0], f32::NEG_INFINITY);
    assert_eq!(first.clip[3], f32::INFINITY);
    assert_eq!(first.page, 0);
}

#[test]
fn steady_state_retains_extracted_glyphs_untouched() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    let settled = h.changed_frames();
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(
        h.changed_frames(),
        settled,
        "5 steady frames left ExtractedGlyphs untouched (is_changed stayed false — § 6.2 O(0))"
    );
}

#[test]
fn cursor_move_only_frame_does_not_rebuild() {
    // THE § 6.2 Changed<Window> regression pin: a Window component tick with
    // an unchanged scale_factor() must NOT fire the gate.
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    let settled = h.changed_frames();
    for i in 0..4 {
        set_cursor(&mut h, Vec2::new(i as f32 * 7.0, 5.0));
        h.frame();
    }
    assert_eq!(
        h.changed_frames(),
        settled,
        "mouse-move frames retained the carrier"
    );
}

#[test]
fn each_union_member_fires_exactly_one_rebuild() {
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h);
    h.settle();
    let mut expect = h.changed_frames();

    // Text edit → ComputedTextLayout changes.
    h.app.world_mut().get_mut::<Text>(text).unwrap().0 = String::from("Hey!!");
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "text edit fired");
    assert_eq!(h.glyph_count(), 5);
    h.frame();
    assert_eq!(h.changed_frames(), expect, "…and settled");

    // TextColor (Added counts as Changed) + the resolved value lands.
    h.app
        .world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert("test.text".into(), Color::srgb(0.2, 0.8, 0.4));
    h.app
        .world_mut()
        .entity_mut(text)
        .insert(TextColor(ColorToken::Token(Cow::Borrowed("test.text"))));
    h.frame();
    // theme.is_changed() and Changed<TextColor> may land on the same frame.
    assert!(h.changed_frames() > expect, "color/theme fired");
    expect = h.changed_frames();
    let lin = LinearRgba::from(Color::srgb(0.2, 0.8, 0.4));
    assert_eq!(
        h.glyphs().glyphs[0].color,
        [lin.red, lin.green, lin.blue, lin.alpha],
        "straight-alpha CPU-linearized token resolve (§ 7)"
    );
    h.frame();
    assert_eq!(h.changed_frames(), expect);

    // Scale flip: value compare fires; every key re-keys (§ 6.2 + § 6 arch).
    let keys_1x = h.resident_keys();
    set_scale(&mut h, 2.0);
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "scale flip fired");
    let keys_2x = h.resident_keys();
    assert_eq!(keys_2x.len(), keys_1x.len());
    assert!(
        keys_1x.iter().all(|k| !keys_2x.contains(k)),
        "a scale change re-keys every glyph (physical font size is in the key)"
    );
    h.frame();
    assert_eq!(
        h.changed_frames(),
        expect,
        "scale cache updated — no re-fire"
    );

    // Hide (paint-skip ADD) → zero instances; show (REMOVE stream) → back.
    h.app
        .world_mut()
        .entity_mut(text)
        .insert(CssVisibility::Hidden);
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "paint-skip add fired");
    assert_eq!(h.glyph_count(), 0);
    h.app.world_mut().entity_mut(text).remove::<CssVisibility>();
    h.frame();
    expect += 1;
    assert_eq!(
        h.changed_frames(),
        expect,
        "hide→show rides the removal stream"
    );
    assert_eq!(h.glyph_count(), 5);

    // Despawn → rebuild to empty, then steady.
    h.app.world_mut().entity_mut(text).despawn();
    h.frame();
    expect += 1;
    assert_eq!(
        h.changed_frames(),
        expect,
        "despawn rides RemovedComponents"
    );
    assert_eq!(h.glyph_count(), 0);
    h.frame();
    assert_eq!(h.changed_frames(), expect, "empty steady state retains");
}

#[test]
fn generation_bump_rebuilds_and_lineage_bump_reseats_the_interner() {
    // § 6.2 grows two value-compare probes (T5): FontsGeneration (rebuild)
    // and FontDbLineage (interner clear + monotonic reseat). A steady frame
    // after the storm settles back to zero-change.
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    let keys_before = h.resident_keys();
    assert!(!keys_before.is_empty());
    let settled = h.changed_frames();

    // Simulate the swap's main-world face: bump both counters (the
    // apply_system_font_scan contract — every lineage bump rides a
    // generation bump).
    h.app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    h.app.world_mut().resource_mut::<FontDbLineage>().0 += 1;
    h.frame();

    assert!(
        h.changed_frames() > settled,
        "generation probe forced a rebuild"
    );
    let keys_after = h.resident_keys();
    assert_eq!(keys_after.len(), keys_before.len(), "same glyphs, re-keyed");
    assert!(
        keys_before.iter().all(|k| !keys_after.contains(k)),
        "lineage advance re-seated every key (the font-u32 bytes moved monotonically)"
    );

    let after_storm = h.changed_frames();
    h.frame();
    assert_eq!(
        h.changed_frames(),
        after_storm,
        "steady again on the very next frame"
    );
}

#[test]
fn clipped_text_packs_its_self_inclusive_clip() {
    // § 8: glyphs are CONTENT — clipped by the entity's OWN computed
    // ClipRect (own box ∩ ancestors), produced by write_clip_rects (the sole
    // producer — never inserted manually here, it would be overwritten).
    // The leaf's initial ClipRect insert also exercises the union's
    // Changed<ClipRect> member (Changed includes Added).
    use buiy_core::layout::OverflowMode;

    let mut h = TextExtractHarness::new();
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "clipped text wraps and overflows its tiny box",
            )),
        ))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(60.0)
                .height_px(24.0)
                .overflow_x(OverflowMode::Hidden)
                .overflow_y(OverflowMode::Hidden),
        ))
        .add_child(text);
    h.settle();
    assert!(h.glyph_count() > 0);

    let clip = h
        .app
        .world()
        .get::<ClipRect>(text)
        .expect("write_clip_rects clipped the text leaf under the hidden-overflow box");
    let expected = [clip.min.x, clip.min.y, clip.max.x, clip.max.y];
    for inst in &h.glyphs().glyphs {
        assert_eq!(
            inst.clip, expected,
            "self-inclusive clip (§ 8) packed verbatim"
        );
    }
}

// --- T6: the decoration quad carrier (decoration-and-paint §§ 3–4, § 6.2) ---

/// The spawn_text fixture plus a `TextDecorations` component.
fn spawn_decorated(h: &mut TextExtractHarness, deco: TextDecorations) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
            deco,
        ))
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

#[test]
fn decorated_text_emits_quads_alongside_glyphs() {
    // One run walk emits both (§ 4.6): underline + overline = 2 quads,
    // entity-keyed, world logical px, the entity's (here absent) clip.
    let mut h = TextExtractHarness::new();
    let text = spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE | DecorationLines::OVERLINE,
            ..Default::default()
        },
    );
    h.settle();

    assert_eq!(
        h.glyph_count(),
        3,
        "glyph emission untouched by the quad walk"
    );
    let quads = &h.text_quads().quads;
    assert_eq!(quads.len(), 2, "underline + overline = 2 quads");
    assert!(quads.iter().all(|q| q.entity == text), "entity-keyed");
    // Emission order mirrors span_decoration_rects (underline first); the
    // underline sits BELOW the baseline, the overline at the line top.
    assert!(
        quads[0].position.y > quads[1].position.y,
        "underline below overline"
    );
    // § 3.3 at scale 1.0: the embedded font's 0.05 em × 16 px = 0.8 logical
    // raw → floored to one whole physical px = 1.0 logical (the drift-guard
    // pin); the overline REUSES the underline thickness.
    assert_eq!(quads[0].size.y, 1.0);
    assert_eq!(quads[1].size.y, 1.0);
    assert!(quads.iter().all(|q| q.size.x > 0.0), "span-extent width");
    assert!(
        quads.iter().all(|q| q.clip.is_none()),
        "unclipped fixture: the full-view sentinel"
    );
}

#[test]
fn decoration_quads_carry_the_entitys_self_inclusive_clip() {
    // § 8 applied to decorations: the quad rides the SAME self-inclusive
    // clip resolution as the entity's glyphs.
    use buiy_core::layout::OverflowMode;

    let mut h = TextExtractHarness::new();
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "clipped text wraps and overflows its tiny box",
            )),
            TextDecorations {
                line: DecorationLines::UNDERLINE,
                ..Default::default()
            },
        ))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(60.0)
                .height_px(24.0)
                .overflow_x(OverflowMode::Hidden)
                .overflow_y(OverflowMode::Hidden),
        ))
        .add_child(text);
    h.settle();

    let clip = *h
        .app
        .world()
        .get::<ClipRect>(text)
        .expect("write_clip_rects clipped the text leaf");
    assert!(!h.text_quads().quads.is_empty());
    for q in &h.text_quads().quads {
        assert_eq!(q.clip, Some(clip), "self-inclusive clip, verbatim");
    }
}

#[test]
fn steady_state_retains_both_carriers_untouched() {
    // After settle: N frames with no trigger → glyph AND quad changed
    // counts both stay flat (the O(0) contract extends to the new carrier).
    let mut h = TextExtractHarness::new();
    spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE,
            ..Default::default()
        },
    );
    h.settle();
    let glyphs_settled = h.changed_frames();
    let quads_settled = h.quad_changed_frames();
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(h.changed_frames(), glyphs_settled, "glyph carrier retained");
    assert_eq!(
        h.quad_changed_frames(),
        quads_settled,
        "quad carrier retained (the § 6.2 O(0) contract extends to it)"
    );
}

#[test]
fn text_decorations_change_republishes_quads_and_retains_glyphs() {
    // A color-only TextDecorations edit reaches extract via
    // Changed<TextDecorations> even though ComputedTextLayout is idempotent
    // (the line bits didn't move, so sync's lazy setters reshape nothing).
    // Both carriers REBUILD wholesale (one damage decision, T6 decision 12)
    // but publication is value-compared (T7 decision 4): only the quad
    // content changed here, so the glyph carrier keeps its tick.
    let mut h = TextExtractHarness::new();
    h.app
        .world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert("deco.test".into(), Color::srgb(1.0, 0.0, 0.0));
    let text = spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE,
            ..Default::default()
        },
    );
    h.settle();
    let glyphs_settled = h.changed_frames();
    let quads_settled = h.quad_changed_frames();

    h.app
        .world_mut()
        .get_mut::<TextDecorations>(text)
        .unwrap()
        .color = Some(ColorToken::Token(Cow::Borrowed("deco.test")));
    h.frame();
    assert_eq!(
        h.changed_frames(),
        glyphs_settled,
        "glyph carrier RETAINED — the rebuilt instances compare equal \
         (value-compared publish, T7 decision 4)"
    );
    assert_eq!(
        h.quad_changed_frames(),
        quads_settled + 1,
        "one quad-carrier republish"
    );
    assert_eq!(
        h.text_quads().quads[0].color,
        Color::srgb(1.0, 0.0, 0.0),
        "the quad color re-resolved"
    );
    h.frame();
    assert_eq!(h.changed_frames(), glyphs_settled, "back to steady");
    assert_eq!(h.quad_changed_frames(), quads_settled + 1);
}

#[test]
fn decoration_color_precedence_at_the_producer() {
    let mut h = TextExtractHarness::new();
    {
        let mut theme = h.app.world_mut().resource_mut::<Theme>();
        theme
            .colors
            .insert("deco.line".into(), Color::srgb(1.0, 0.0, 0.0));
        theme
            .colors
            .insert("text.fg".into(), Color::srgb(0.0, 1.0, 0.0));
    }
    let text = spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE,
            color: Some(ColorToken::Token(Cow::Borrowed("deco.line"))),
            ..Default::default()
        },
    );
    h.app
        .world_mut()
        .entity_mut(text)
        .insert(TextColor(ColorToken::Token(Cow::Borrowed("text.fg"))));
    h.settle();

    // (a) Tier 1: the resolved -color token wins.
    assert_eq!(h.text_quads().quads[0].color, Color::srgb(1.0, 0.0, 0.0));

    // (c) Retheme = re-emit, never reshape (decision 1): swapping the
    // token's value re-resolves on the theme.is_changed() rebuild.
    h.app
        .world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert("deco.line".into(), Color::srgb(0.0, 0.0, 1.0));
    h.frame();
    assert_eq!(h.text_quads().quads[0].color, Color::srgb(0.0, 0.0, 1.0));

    // (b) color = None → the entity's resolved TextColor (tier 3,
    // currentColor — tier 2 is structurally None in v1).
    h.app
        .world_mut()
        .get_mut::<TextDecorations>(text)
        .unwrap()
        .color = None;
    h.frame();
    assert_eq!(h.text_quads().quads[0].color, Color::srgb(0.0, 1.0, 0.0));
}

#[test]
fn scale_change_refloors_decoration_thickness() {
    let mut h = TextExtractHarness::new();
    spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE,
            ..Default::default()
        },
    );
    h.settle();
    // Scale 1.0: raw 0.05 em × 16 px = 0.8 logical → 1 whole physical px.
    assert_eq!(h.text_quads().quads[0].size.y, 1.0);

    set_scale(&mut h, 1.25);
    h.frame();
    // Scale 1.25: 0.8 × 1.25 = 1.0 physical — already whole, so the § 3.3
    // floor keeps the exact 0.8 logical thickness (no logical-px floor).
    assert_eq!(h.text_quads().quads[0].size.y, 0.8);
}

#[test]
fn blocked_text_zero_alphas_decorations() {
    // Decision 11: PendingFontBlock present → quads emit at alpha exactly
    // 0.0 (layout-identical, paint-invisible, buffers warm); the timeout
    // lift (the § 7 "then swap" arm) restores full alpha.
    const NEVER_LOADS: Handle<BuiyFont> = uuid_handle!("7d3a5b2c-1e4f-4a6b-8c9d-0e1f2a3b4c5d");

    let mut h = TextExtractHarness::new();
    h.app
        .world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset(
            "Pending Sans",
            NEVER_LOADS.clone(),
            FontFaceDescriptors {
                font_display: FontDisplay::Block,
                ..Default::default()
            },
        );
    let text = spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::UNDERLINE,
            ..Default::default()
        },
    );
    h.app
        .world_mut()
        .entity_mut(text)
        .insert(FontFamily(FontStack(vec![
            FamilyEntry::Named(String::from("Pending Sans")),
            FamilyEntry::Generic(GenericFamily::Serif),
        ])));
    h.settle();

    assert!(
        !h.text_quads().quads.is_empty(),
        "quads ARE emitted while blocked (the zero-alpha skip is bypassed)"
    );
    assert!(
        h.text_quads().quads.iter().all(|q| q.color.alpha() == 0.0),
        "every blocked quad paints zero-alpha"
    );

    h.app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_secs_f32(3.5));
    h.frame();
    assert!(
        h.text_quads().quads.iter().all(|q| q.color.alpha() > 0.0),
        "the lifted block repaints decorations at full alpha"
    );
}

#[test]
fn undecorated_text_emits_no_quads() {
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h);
    h.settle();
    assert!(h.text_quads().quads.is_empty());
    // Value-compared publication (T7 decision 4, refining decision 12's
    // lockstep republish): the carrier is still REBUILT on every dirty
    // frame, but an empty-to-empty rebuild compares equal and keeps its
    // tick — a glyph-only edit retains the quad carrier.
    let g0 = h.changed_frames();
    let q0 = h.quad_changed_frames();
    h.app.world_mut().get_mut::<Text>(text).unwrap().0 = String::from("Hey");
    h.frame();
    assert_eq!(
        h.changed_frames(),
        g0 + 1,
        "the text edit republished glyphs"
    );
    assert_eq!(
        h.quad_changed_frames(),
        q0,
        "the (empty, content-identical) quad carrier is RETAINED"
    );
    assert!(h.text_quads().quads.is_empty());
}

// --- T6 Task 4: the solid stamp + line-through over the text (§§ 4.3–4.4) ---

/// The midpoint-uv signature (uv min == max, decision 9) picks stamp
/// instances out of the glyph blob — a real glyph always samples a
/// non-degenerate uv rect.
fn stamp_instances(h: &TextExtractHarness) -> Vec<GlyphAlphaInstance> {
    h.glyphs()
        .glyphs
        .iter()
        .filter(|g| g.uv[0] == g.uv[2])
        .copied()
        .collect()
}

#[test]
fn line_through_emits_a_stamp_instance_after_the_runs_glyphs() {
    // The § 4.4 seat-5 order: the LAST instance of the entity's emission is
    // the stamp — uv min == max (the midpoint signature), color = the § 3.2
    // resolved decoration color (currentColor here), rect height = the
    // § 3.3 floored strikeout thickness. resident_keys gains one
    // solid_stamp_key() entry PER stamp instance (the one-key-per-instance
    // invariant).
    let mut h = TextExtractHarness::new();
    spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::LINE_THROUGH,
            ..Default::default()
        },
    );
    h.settle();

    let glyphs = &h.glyphs().glyphs;
    assert_eq!(glyphs.len(), 4, "3 glyph instances + 1 solid stamp");
    for g in &glyphs[..3] {
        assert!(
            g.uv[0] < g.uv[2] && g.uv[1] < g.uv[3],
            "real glyphs sample a full uv rect"
        );
    }
    let stamp = glyphs[3];
    assert_eq!(stamp.uv[0], stamp.uv[2], "midpoint uv: constant x");
    assert_eq!(stamp.uv[1], stamp.uv[3], "midpoint uv: constant y");
    // § 3.3 at scale 1.0: the embedded font's strikeout thickness 0.05 em
    // × 16 px = 0.8 logical raw → floored to one whole physical px.
    assert_eq!(stamp.rect[3], 1.0);
    // currentColor: the same resolved entity color the glyphs carry.
    assert_eq!(stamp.color, glyphs[0].color);
    // Line-through is glyph-tier (§ 4.2) — the quad carrier stays empty.
    assert!(h.text_quads().quads.is_empty());

    let keys = h.resident_keys();
    assert_eq!(keys.len(), 4, "one key per instance, stamps included");
    assert_eq!(keys[3], solid_stamp_key());
    let entry = h
        .atlas()
        .get(&solid_stamp_key())
        .expect("the stamp is resident");
    assert_eq!(
        stamp.uv,
        stamp_uv(&entry),
        "instance uv = the entry midpoint"
    );
    assert_eq!(stamp.page, entry.page as u32);
}

#[test]
fn live_stamp_survives_eviction_grace_via_the_touch_pass() {
    // glyph-pipeline § 6.3's join: a live stamp instance touches the key
    // every frame (retained frames included), so it never grace-evicts
    // while painted — the stale-uv cell-reuse hazard.
    let mut h = TextExtractHarness::with_atlas_config(AtlasConfig {
        eviction_grace: 5,
        ..Default::default()
    });
    spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::LINE_THROUGH,
            ..Default::default()
        },
    );
    h.settle();
    assert!(h.atlas().get(&solid_stamp_key()).is_some());
    let settled = h.changed_frames();
    for _ in 0..15 {
        h.frame(); // 3× grace, all steady — the carriers are retained
    }
    assert_eq!(h.changed_frames(), settled, "steady frames: no rebuild");
    assert!(
        h.atlas().get(&solid_stamp_key()).is_some(),
        "the un-gated touch pass kept the live stamp resident"
    );
}

#[test]
fn idle_stamp_evicts_and_reinserts_on_miss() {
    // § 4.3 "re-inserted on miss": warmup-pinned is not pin-forever (gate
    // #15) — with no live stamp instance the key ages out like any entry.
    // The harness never runs warmup_atlas, so the re-add exercises the
    // producer's get_or_insert-on-miss path end-to-end.
    let mut h = TextExtractHarness::with_atlas_config(AtlasConfig {
        eviction_grace: 5,
        ..Default::default()
    });
    let text = spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::LINE_THROUGH,
            ..Default::default()
        },
    );
    h.settle();
    assert!(h.atlas().get(&solid_stamp_key()).is_some());

    // Drop the line: the rebuilt carriers emit no stamp instance, so the
    // touch pass no longer covers the key.
    h.app
        .world_mut()
        .get_mut::<TextDecorations>(text)
        .unwrap()
        .line = DecorationLines::empty();
    h.frame();
    assert!(
        stamp_instances(&h).is_empty(),
        "no stamp instances after the line drop"
    );
    for _ in 0..10 {
        h.frame(); // idle past grace
    }
    assert!(
        h.atlas().get(&solid_stamp_key()).is_none(),
        "the idle stamp drained back out"
    );

    // Re-add → the rebuild's get_or_insert self-heals the miss; the new
    // instance uv is valid for the NEW entry.
    h.app
        .world_mut()
        .get_mut::<TextDecorations>(text)
        .unwrap()
        .line = DecorationLines::LINE_THROUGH;
    h.frame();
    let entry = h
        .atlas()
        .get(&solid_stamp_key())
        .expect("re-inserted on miss");
    let stamps = stamp_instances(&h);
    assert_eq!(stamps.len(), 1, "the re-added line stamps again");
    assert_eq!(stamps[0].uv, stamp_uv(&entry), "uv valid for the NEW entry");
}

#[test]
fn blocked_text_zero_alphas_the_stamp_too() {
    // Decision 11, stamp half: Block → stamp instances emit at alpha 0 (the
    // stamp stays resident + touched); the timeout lift → full alpha.
    const NEVER_LOADS: Handle<BuiyFont> = uuid_handle!("8e4b6c3d-2f5a-4b7c-9d0e-1f2a3b4c5d6e");

    let mut h = TextExtractHarness::new();
    h.app
        .world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset(
            "Pending Sans",
            NEVER_LOADS.clone(),
            FontFaceDescriptors {
                font_display: FontDisplay::Block,
                ..Default::default()
            },
        );
    let text = spawn_decorated(
        &mut h,
        TextDecorations {
            line: DecorationLines::LINE_THROUGH,
            ..Default::default()
        },
    );
    h.app
        .world_mut()
        .entity_mut(text)
        .insert(FontFamily(FontStack(vec![
            FamilyEntry::Named(String::from("Pending Sans")),
            FamilyEntry::Generic(GenericFamily::Serif),
        ])));
    h.settle();

    let blocked = stamp_instances(&h);
    assert!(
        !blocked.is_empty(),
        "stamps ARE emitted while blocked (buffers warm, paint invisible)"
    );
    assert!(
        blocked.iter().all(|s| s.color[3] == 0.0),
        "every blocked stamp paints zero-alpha"
    );
    assert!(
        h.atlas().get(&solid_stamp_key()).is_some(),
        "the stamp stays resident + touched while blocked"
    );

    h.app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_secs_f32(3.5));
    h.frame();
    let lifted = stamp_instances(&h);
    assert!(!lifted.is_empty());
    assert!(
        lifted.iter().all(|s| s.color[3] > 0.0),
        "the lifted block repaints the stamp at full alpha"
    );
}

// --- T8 Task 1: per-entity instance runs on ExtractedGlyphs (D1/D4) ---

/// T8 D1: the producer attributes every instance to its source entity as
/// one contiguous run per entity, in emission (paint) order, covering the
/// instance vec exactly — the input the prepare-time group partition
/// derives from the FRESH node list (decoration-and-paint § 4.6 applied
/// to the glyph buffer).
#[test]
fn entity_runs_cover_all_instances_one_run_per_entity() {
    let mut h = TextExtractHarness::new();
    let (a, root) = spawn_text_with_root(&mut h);
    // A second text sibling under the same root → a second, later run.
    let b = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Yo")),
            FontSize(16.0),
        ))
        .id();
    h.app.world_mut().entity_mut(root).add_child(b);
    h.settle();

    let glyphs = h.glyphs();
    let runs = &glyphs.entity_runs;
    assert_eq!(runs.len(), 2, "one run per emitting entity");
    // Contiguous cover of [0, len), in emission order.
    let mut next = 0u32;
    for run in runs {
        assert_eq!(run.instances.start, next, "runs are gapless from 0");
        assert!(
            run.instances.start < run.instances.end,
            "runs are non-empty"
        );
        next = run.instances.end;
    }
    assert_eq!(
        next as usize,
        glyphs.glyphs.len(),
        "runs cover every instance"
    );
    // Attribution: the two runs name the two entities, in paint order.
    let entities: Vec<Entity> = runs.iter().map(|r| r.entity).collect();
    assert!(entities.contains(&a) && entities.contains(&b));
}

/// An entity emitting no instance (whitespace-only) gets NO run.
#[test]
fn whitespace_only_entity_emits_no_run() {
    let mut h = TextExtractHarness::new();
    // The spawn_text shape, but whitespace-only content: zero-coverage
    // glyphs emit no instance, so the entity must get no run either.
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("   ")),
            FontSize(16.0),
        ))
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
    h.settle();
    assert_eq!(h.glyph_count(), 0);
    assert!(h.glyphs().entity_runs.is_empty());
}

/// D4: instance bytes can coincide across DIFFERENT entities (despawn +
/// respawn an identical fixture in one frame) — the runs compare must
/// republish so prepare re-derives the group partition for the new
/// entity. Without entity_runs in the publish compare this is the silent
/// stale-partition bug.
#[test]
fn respawn_with_identical_instances_republishes_for_entity_identity() {
    let mut h = TextExtractHarness::new();
    let (text, root) = spawn_text_with_root(&mut h);
    h.settle();
    let before = h.glyphs().glyphs.clone();
    let publishes = h.changed_frames();

    // Despawn + respawn the IDENTICAL leaf in one main-world step: the
    // rebuild (despawn fires RemovedComponents<ResolvedLayout>) sees
    // byte-identical instances under a NEW entity id.
    h.app.world_mut().entity_mut(text).despawn();
    let text2 = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
        ))
        .id();
    h.app.world_mut().entity_mut(root).add_child(text2);
    h.frame();

    // Precondition: the pixels really are identical (same layout slot,
    // same font, atlas-resident) — if this ever fails the fixture, not
    // the contract, needs adjusting.
    assert_eq!(
        h.glyphs().glyphs,
        before,
        "identical instance bytes (precondition)"
    );
    assert_eq!(h.glyphs().entity_runs.len(), 1);
    assert_eq!(
        h.glyphs().entity_runs[0].entity,
        text2,
        "the run names the NEW entity"
    );
    assert!(
        h.changed_frames() > publishes,
        "the publish fired on runs inequality despite equal instance bytes (D4)"
    );
}

#[test]
fn vanished_window_clears_once_then_retains() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    assert!(h.glyph_count() > 0);

    let window = {
        let mut q = h
            .app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>();
        q.single(h.app.world()).unwrap()
    };
    h.app.world_mut().entity_mut(window).despawn();
    h.frame();
    assert_eq!(h.glyph_count(), 0, "no primary window ⇒ carrier cleared");
    let after_clear = h.changed_frames();
    h.frame();
    h.frame();
    assert_eq!(
        h.changed_frames(),
        after_clear,
        "the clear happens ONCE, not per frame"
    );
}

// --- E1 Task 4: the editor entity emits glyphs through the same producer ---

/// E1 flagship invariant, glyph half (E1 plan § Task 4.3): an editor entity
/// (`Text` + `TextEditState`) contributes glyph instances IDENTICALLY to the
/// equivalent display-only entity, because `extract_buiy_glyphs` reads the
/// authoritative buffer through the read-only `TextBufferAccess`. The seam is
/// transparent at the glyph tier too — same producer, same emission, same
/// per-entity run length.
#[test]
fn editor_entity_emits_the_same_glyph_run_as_a_display_entity() {
    let mut h = TextExtractHarness::new();
    let display = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
        ))
        .id();
    let editor = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
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
        .add_child(display)
        .add_child(editor);
    h.settle();

    let runs = &h.glyphs().entity_runs;
    let span = |e: Entity| {
        runs.iter()
            .find(|r| r.entity == e)
            .map(|r| r.instances.end - r.instances.start)
    };
    let d_span = span(display).expect("the display entity emitted a glyph run");
    let e_span = span(editor).expect("the editor entity emitted a glyph run");
    assert_eq!(
        e_span, d_span,
        "the editor entity emits the same glyph count as the display entity \
         (the producer read the editor-owned buffer through the read-only accessor)"
    );
    // The display fixture is 3 non-whitespace glyphs — the editor matches it.
    assert_eq!(d_span, 3, "the 'Hi!' fixture is three glyph instances");
}

/// E6 / M4 damage-gate regression: editing the `Placeholder` string WHILE the
/// placeholder is already active must re-emit. The reshape happens entirely
/// off the producer's other triggers — `PlaceholderActive` stays present (no
/// toggle), and the empty editor value leaves `ComputedTextLayout` idempotent
/// (no tick) — so only a `Changed<Placeholder>` probe wakes the producer.
/// Without it the screen keeps the OLD placeholder glyphs. (FontSize rides the
/// same nested-Or group for the same reason.)
#[test]
fn already_active_placeholder_string_reshape_re_emits() {
    let mut h = TextExtractHarness::new();
    let editor = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Hi")),
        ))
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
        .add_child(editor);
    h.settle();

    // Steady: the placeholder is active and its glyphs are resident. "Hi" is
    // two non-whitespace glyphs; the editor value is empty so it adds no ink.
    assert!(
        h.app.world().get::<PlaceholderActive>(editor).is_some(),
        "an empty editor with a Placeholder is active"
    );
    assert_eq!(h.glyph_count(), 2, "the 'Hi' placeholder is two glyphs");
    // Quiesce: prove we are at a steady frame (no pending rebuild).
    h.frame();
    let baseline = h.changed_frames();
    h.frame();
    assert_eq!(
        h.changed_frames(),
        baseline,
        "steady state — no rebuild without a trigger"
    );

    // Edit the Placeholder STRING while still active: no toggle, no value
    // change. Pre-fix this re-shapes the PlaceholderBuffer but the producer
    // never re-runs, so the old "Hi" glyphs stay on screen.
    h.app.world_mut().get_mut::<Placeholder>(editor).unwrap().0 = String::from("Hello");
    h.frame();
    assert_eq!(
        h.changed_frames(),
        baseline + 1,
        "the already-active Placeholder reshape fired exactly one rebuild"
    );
    assert!(
        h.app.world().get::<PlaceholderActive>(editor).is_some(),
        "still active (the marker never toggled — this is the gate gap)"
    );
    assert_eq!(
        h.glyph_count(),
        5,
        "the new 'Hello' placeholder (five glyphs) reached the carrier"
    );
    h.frame();
    assert_eq!(
        h.changed_frames(),
        baseline + 1,
        "…and settled — the reshape is a one-shot rebuild"
    );
}
