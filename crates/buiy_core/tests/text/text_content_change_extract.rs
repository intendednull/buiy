//! A geometry-stable `Text` content change must still re-extract new glyphs
//! (`docs/specs/2026-07-10-dooduel-countdown-render-invalidation-design.md`):
//! `extract_buiy_glyphs`'s dirty-gate union relies on `Changed<ComputedTextLayout>`
//! as "the text-changed signal", but `ComputedTextLayout` carries only layout
//! geometry with a value-compare guard — so a content swap that lays out to the
//! SAME geometry (e.g. an equal-width monospace digit swap, the in-game countdown
//! number's pathological case) never ticks it, and the extract skips the entity,
//! leaving stale glyphs on screen. This is the Tier-2 HEADLESS gate (lowest tier
//! that observes the dropped re-extract) via the adapterless `TextExtractHarness`.

use crate::support::extract_harness::TextExtractHarness;
use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::text::{
    ComputedTextLayout, FamilyEntry, FontFamily, FontSize, FontStack, GenericFamily, Text,
};

/// A display `Text` under a sized column root (mirrors `text_transform_extract`'s
/// `spawn_text`), pinned to the monospace generic (`GenericFamily::Monospace`
/// resolves to the embedded true-monospace Geist Mono face,
/// `font_system.rs:27-31`) so an equal-length digit swap is geometry-stable —
/// every glyph advances the same width regardless of which digit it is.
fn spawn_text(h: &mut TextExtractHarness, content: &str) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(content)),
            FontSize(16.0),
            FontFamily(FontStack(vec![FamilyEntry::Generic(
                GenericFamily::Monospace,
            )])),
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
fn geometry_stable_content_change_reextracts_glyphs() {
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h, "88");
    h.settle();

    let glyphs_before = h.glyphs().glyphs.clone();
    assert!(!glyphs_before.is_empty(), "baseline glyphs emitted");
    let ctl_before = h
        .app
        .world()
        .get::<ComputedTextLayout>(text)
        .cloned()
        .expect("text entity has a ComputedTextLayout after settle");

    // Equal-length digit swap: same glyph count, same advances in a true
    // monospace face — the countdown's exact pathological case.
    h.app.world_mut().get_mut::<Text>(text).unwrap().0 = "99".into();
    h.settle();

    let glyphs_after = h.glyphs().glyphs.clone();
    let ctl_after = h
        .app
        .world()
        .get::<ComputedTextLayout>(text)
        .cloned()
        .expect("text entity still has a ComputedTextLayout after the content change");

    // Precondition: prove this is genuinely the geometry-stable path — if the
    // layout itself moved, `Changed<ComputedTextLayout>` would legitimately
    // re-trigger the extract and the test wouldn't be exercising the bug.
    assert_eq!(
        ctl_before, ctl_after,
        "precondition: the content swap must be geometry-stable (same ComputedTextLayout) — \
         otherwise the test isn't exercising the bug"
    );

    // The bug: with only `Changed<ComputedTextLayout>` gating the re-extract,
    // this stayed the STALE "88" glyph set. A geometry-stable content change
    // must still re-extract the NEW glyphs.
    assert_ne!(
        glyphs_before, glyphs_after,
        "a geometry-stable Text content change must re-extract new glyphs, not keep the stale set"
    );
}
