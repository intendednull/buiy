//! Gate #14's text component — the typing-latency MECHANISM fixture
//! (text verification.md § 4; campaign T8): a keystroke (a `Text` edit)
//! reaches a freshly-published `ExtractedGlyphs` in ONE frame
//! (TextSync → measure → TextCommit → extract, all within one `frame()`),
//! and the steady tail after it re-publishes and re-measures NOTHING — the
//! structural protection the per-frame budget relies on. Wall-clock budget
//! NUMBERS stay with `buiy-verification-design`; this file pins the
//! frame-count mechanism only, headless on the adapterless extract harness
//! (the `GlyphChangeLog` mirrors the prepare glyph gate exactly).
//!
//! The instrument resources are PER-FRAME counters (overwritten / reset
//! each frame, the `SyncStylesIterCount` precedent), so the edit frame is
//! pinned by exact same-frame counts and the steady tail by exact zeros —
//! not by cumulative deltas.

mod support;

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{LayoutTaffyComputeCount, Style};
use buiy_core::text::{FontSize, Text, TextMeasureCallCount, TextSyncAppliedCount};
use support::extract_harness::TextExtractHarness;

/// "Hi" under a sized column root: 2 non-whitespace glyphs — the keystroke
/// appends '!' for exactly one new instance.
fn spawn_text(h: &mut TextExtractHarness) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
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

/// Entities `text_sync_buffers` applied the lazy setters to THIS frame
/// (overwritten per invocation, sync.rs).
fn sync_applied(h: &TextExtractHarness) -> usize {
    h.app.world().resource::<TextSyncAppliedCount>().0
}

/// Measure-closure invocations THIS frame (reset by `taffy_compute` at
/// frame start, measure.rs).
fn measure_calls(h: &TextExtractHarness) -> usize {
    h.app.world().resource::<TextMeasureCallCount>().0
}

/// Taffy `compute_layout` invocations THIS frame (per-frame; the steady
/// baseline is one per layout root, systems.rs).
fn taffy_computes(h: &TextExtractHarness) -> u32 {
    h.app.world().resource::<LayoutTaffyComputeCount>().0
}

#[test]
fn one_frame_from_text_edit_to_glyph_publish() {
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h);
    h.settle();

    let count0 = h.glyph_count();
    let publishes0 = h.changed_frames();

    // The keystroke: append one glyph's worth of text.
    h.app
        .world_mut()
        .get_mut::<Text>(text)
        .expect("Text")
        .0
        .push('!');
    h.frame(); // ONE frame: Update (sync → measure → commit) + extract

    // THE mechanism: the edit reached ExtractedGlyphs in one frame…
    assert_eq!(
        h.changed_frames(),
        publishes0 + 1,
        "one frame from the Text edit to the ExtractedGlyphs publish"
    );
    // …and the published set really is the new content.
    assert_eq!(
        h.glyph_count(),
        count0 + 1,
        "the new glyph's instance is in the published set"
    );
    // The instrument trail: exactly one TextSync re-apply on the edit
    // frame; the edit re-measured (intrinsics invalidation fired inside
    // this frame's Taffy compute).
    assert_eq!(
        sync_applied(&h),
        1,
        "exactly one TextSync re-apply on the edit frame"
    );
    assert!(
        measure_calls(&h) > 0,
        "the edit re-measured (intrinsics invalidation fired)"
    );

    // The steady tail (the gate-#14 structural budget): nothing publishes,
    // nothing re-applies, nothing measures, and layout stays at the
    // one-per-root Taffy baseline (the cache holds — no re-run).
    let publishes1 = h.changed_frames();
    for i in 0..2 {
        h.frame();
        assert_eq!(
            h.changed_frames(),
            publishes1,
            "steady frame {i} publishes nothing"
        );
        assert_eq!(sync_applied(&h), 0, "steady frame {i} re-applies no text");
        assert_eq!(measure_calls(&h), 0, "steady frame {i} measures nothing");
        assert_eq!(
            taffy_computes(&h),
            1,
            "steady frame {i} stays at the one-per-root Taffy baseline"
        );
    }
}
