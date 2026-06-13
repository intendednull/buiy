//! Gate #15's text component — the typing-churn fixture (text
//! verification.md §§ 1.3, 4; campaign T9): an edit loop churns disjoint
//! glyph keys through the device-free atlas, and idle drains it back to the
//! EXACT baseline — entries, pages, and the resident key set.
//!
//! Seated HEADLESS per the T9 plan's D2: the entry-count property is fully
//! CPU-visible (`BuiyAtlas` is device-free; the extract harness runs
//! `maintain_atlas`), and § 1.1's lowest-layer principle wins over § 1.3's
//! GPU seating — CI never runs the GPU lane, so this is the gate-#15
//! protection every PR actually gets. The pixels/upload half (stale-UV
//! corruption under churn) is GPU-observable only and lives in
//! `text_gpu.rs`'s churn twin.

mod support;

use std::collections::HashSet;

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::{AtlasConfig, AtlasFormat, AtlasKey};
use buiy_core::text::Text;
use support::extract_harness::TextExtractHarness;

const GRACE: u32 = 3;

fn harness() -> TextExtractHarness {
    TextExtractHarness::with_atlas_config(AtlasConfig {
        page_size: 1024,
        page_budget: 8,
        eviction_grace: GRACE,
    })
}

fn spawn_text(h: &mut TextExtractHarness, s: &str) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from(s))))
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
fn typing_churn_returns_atlas_to_baseline() {
    let mut h = harness();
    let text = spawn_text(&mut h, "abc");
    h.settle();

    let baseline_entries = h.atlas().live_entry_count();
    let baseline_pages = h.atlas().page_count(AtlasFormat::CoverageR8);
    let baseline_keys: HashSet<AtlasKey> = h.resident_keys().into_iter().collect();
    assert!(baseline_entries > 0, "the baseline text rasterized");

    // The edit loop: every string is letter-disjoint from the baseline AND
    // from every other step, so each edit inserts fresh atlas keys — the
    // churn is real, not re-touches of already-resident entries. The last
    // edit returns to the baseline string (the ε = 0 premise below).
    let edits = ["dgq", "hkx", "mvz", "rtw", "ufy", "jpn", "els", "abc"];
    for (i, s) in edits.iter().copied().enumerate() {
        h.app.world_mut().get_mut::<Text>(text).expect("Text").0 = String::from(s);
        h.frame();
        if i == edits.len() / 2 {
            // Non-vacuity, built in: mid-loop the atlas really grew past
            // the baseline (fresh inserts + grace-resident stale entries).
            assert!(
                h.atlas().live_entry_count() > baseline_entries,
                "mid-loop entry count exceeds baseline — the fixture churned"
            );
        }
    }

    // Idle: the atlas-and-text-seam § 2.4 settle window. Only the resident
    // "abc" keys get the per-frame touch; every churned key goes untouched
    // and grace-evicts well within 4 × GRACE frames.
    for _ in 0..(GRACE * 4) {
        h.frame();
    }

    // ε = 0 (T9 plan D2): the loop ends ON the baseline string, so the
    // resident key set — and with it the live entry count — must return
    // EXACTLY, not approximately (the rebuild-storm assert_eq precedent).
    // The spec's ε allowance is for fixtures whose end state isn't the
    // start state.
    assert_eq!(
        h.atlas().live_entry_count(),
        baseline_entries,
        "entry count returned to baseline after idle (gate #15)"
    );
    assert_eq!(
        h.atlas().page_count(AtlasFormat::CoverageR8),
        baseline_pages,
        "page count returned to baseline (pages pooled, never leaked)"
    );
    let keys_after: HashSet<AtlasKey> = h.resident_keys().into_iter().collect();
    assert_eq!(
        keys_after, baseline_keys,
        "the resident key set is exactly the baseline set"
    );
}
