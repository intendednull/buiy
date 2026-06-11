//! `extract_buiy_glyphs` — emission + THE § 6.2 damage gate, headless on
//! the adapterless extract harness (verification § 1.2; § 12 headless c).

mod support;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{ClipRect, CssVisibility, TextColor};
use buiy_core::text::{FontDbLineage, FontSize, FontsGeneration, Text};
use buiy_core::theme::Theme;
use std::borrow::Cow;
use support::extract_harness::TextExtractHarness;

/// "Hi!" under a sized column root: 3 non-whitespace glyphs.
fn spawn_text(h: &mut TextExtractHarness) -> Entity {
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
