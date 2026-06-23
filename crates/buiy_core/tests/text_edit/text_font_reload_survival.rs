//! Tier B (C7 §2.2) — the FontsGeneration-bump content-survival / reshape /
//! empty-editor / preedit / editor-style tests, headless on the adapterless
//! TextExtractHarness. These are the content/preedit/style-survival tests the
//! audit says are MISSING (Bug 3: zero coverage). For Bug 2, the reshape arms
//! here (`label_reshapes_…`, `editor_style_stays_live_…`) are REGRESSION
//! guards, NOT the isolating `shape_stale` proof: the FontsGeneration sweep
//! auto-heals via `mark_dirty_for_entity` → Taffy re-measure (audit
//! Appendix-A.5), so they pass with or without the guard term. The directed,
//! isolating `shape_stale` proof is C2-owned.
//!
//! RED-first for C2: on current main (pre-C2) the FontsGeneration sweep
//! clobbers the editor-owned buffer to "" (the all-buffers sweep at sync.rs:251
//! set_text on the editor buffer), so the content-survival / preedit /
//! editor-style asserts FAIL until C2's TextSync fix lands. C2's PR un-ignores
//! them (deletes the #[ignore]) — it must NOT recreate this file or its tests.
//!
//! This file is a MODULE of the `text_edit` group binary (PR #77 consolidation),
//! registered via `#[path] mod text_font_reload_survival;` in `tests/text_edit.rs`.
//! It does NOT declare `mod support;` (the binary root owns it); it reaches the
//! harness via `crate::support::extract_harness::TextExtractHarness`, the same
//! way `tests/text/text_extract.rs` does.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::text::edit::{EditCommand, TextEditState};
use buiy_core::text::{SharedFontSystem, Text};

use crate::support::extract_harness::TextExtractHarness;

/// Spawn an editor entity in the PRODUCTION shape: `TextEditState` (the
/// editor-owned buffer) PLUS the display `Text("")` carrier every real
/// `text_input` widget requires (`buiy_widgets::text_input` — "`Text("")` is the
/// required display carrier"). The editor-owned buffer holds the typed
/// `content` while the display `Text` is "" — the exact divergence Bug 3
/// clobbers (C2 spec §1.1).
///
/// The `Text("")` carrier is LOAD-BEARING for the RED proof: the FontsGeneration
/// all-buffers sweep (`TextSync`, sync.rs:251-255) is a `Query<SyncedText>`
/// REQUIRING `&Text` (sync.rs:158) — an editor with NO `Text` component is not
/// in the sweep at all, so the clobber never fires and the proof would be
/// VACUOUS. With the carrier present the sweep matches the entity, the accessor
/// routes the write to the editor-owned buffer, and `apply_authored_to_buffer`
/// `set_text("")`s it to empty (sync.rs:512/530) — the Bug-3 clobber the test
/// asserts C2 must prevent.
fn spawn_seeded_editor(h: &mut TextExtractHarness, content: &str) -> Entity {
    let mut state = TextEditState::for_font_size(16.0);
    {
        let fonts = h.app.world().resource::<SharedFontSystem>().clone();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert(content.into()), false, false);
    }
    assert_eq!(
        state.value(),
        content,
        "seed sanity: editor holds the typed content"
    );
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(200.0).height_px(40.0),
            Text(String::new()), // the required display carrier (production shape)
            state,
        ))
        .id()
}

#[ignore = "RED until C2 lands: the FontsGeneration sweep clobbers the editor-owned buffer to \"\"; C2's TextSync fix preserves it and un-ignores this test"]
#[test]
fn editor_content_survives_a_fonts_generation_bump() {
    let mut h = TextExtractHarness::new();
    let editor = spawn_seeded_editor(&mut h, "Hello");
    h.settle();
    // The bump fires the all-buffers TextSync sweep (the runtime add_font
    // trigger; registry.rs apply_font_registry bumps FontsGeneration).
    h.bump_fonts_generation();
    h.settle();

    let value = h
        .app
        .world()
        .get::<TextEditState>(editor)
        .expect("editor still present")
        .value();
    assert_eq!(
        value, "Hello",
        "the editor-owned buffer must STILL hold the typed content after a \
         FontsGeneration bump; on pre-C2 main the sweep clobbers it to \"\" \
         (Bug 3) — this is the C2 content-survival gate"
    );
}

/// Reshape / shape-guard (Bug 2): after a bump, a NON-EDITOR text label
/// KEEPS its glyphs — it does not silently go to zero glyphs (silent-no-paint).
/// This is GREEN today and guards C2's shape-guard from regressing the reshape.
///
/// Note the assertion is `glyph_count == 3`, NOT a republish-counter bump: the
/// bump increments `FontsGeneration` with NO actual font-set change, so the
/// all-buffers sweep re-shapes to BYTE-IDENTICAL glyphs and the producer's
/// change-gate correctly suppresses a republish (verified: `changed_frames`
/// does not advance — a no-op font change must not spuriously republish). The
/// load-bearing property is that the glyphs SURVIVE — a silent-no-paint
/// regression (Bug 2) would drop them to 0 here.
#[test]
fn label_reshapes_and_keeps_glyphs_after_a_bump() {
    let mut h = TextExtractHarness::new();
    h.app.world_mut().spawn((
        Node,
        Style::default()
            .flex_column()
            .width_px(300.0)
            .height_px(100.0),
    ));
    h.app.world_mut().spawn((
        Node,
        Style::default(),
        buiy_core::text::Text(String::from("Hi!")),
        buiy_core::text::FontSize(16.0),
    ));
    h.settle();
    assert_eq!(
        h.glyph_count(),
        3,
        "label shapes to 3 glyphs before the bump"
    );
    h.bump_fonts_generation();
    h.settle();
    assert_eq!(
        h.glyph_count(),
        3,
        "label still shapes to 3 glyphs after the bump — not a silent no-paint"
    );
}

/// Empty-editor 0-vs-1 (audit Bug 2 critical refutation): an empty editor
/// with NO active placeholder emits 0 glyphs after the bump and does NOT
/// crash / assert-fire. GREEN today; pins the empty case the prototype's
/// "complete" fix accidentally relied on.
#[test]
fn empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump() {
    let mut h = TextExtractHarness::new();
    h.app.world_mut().spawn((
        Node,
        Style::default().width_px(200.0).height_px(40.0),
        TextEditState::for_font_size(16.0),
    ));
    h.settle();
    assert_eq!(h.glyph_count(), 0, "an empty editor emits no glyphs");
    h.bump_fonts_generation(); // must not panic
    h.settle();
    assert_eq!(h.glyph_count(), 0, "still zero after the bump, no crash");
}

/// Preedit-survival (C2 §2.6 / spec §2.2): a live IME preedit survives the
/// bump (a mid-composition set_text destroys composition). Committed #[ignore]
/// RED until C2 lands; C2's PR deletes the attribute as it goes GREEN.
#[ignore = "RED until C2 lands: the FontsGeneration sweep destroys a mid-composition preedit; C2's preedit-aware TextSync fix preserves it and un-ignores this test"]
#[test]
fn preedit_survives_a_fonts_generation_bump() {
    let mut h = TextExtractHarness::new();
    let mut state = TextEditState::for_font_size(16.0);
    {
        let fonts = h.app.world().resource::<SharedFontSystem>().clone();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
        state.splice_preedit(&mut fs, "X", None);
    }
    let editor = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(200.0).height_px(40.0),
            Text(String::new()), // the required display carrier (production shape)
            state,
        ))
        .id();
    h.settle();
    h.bump_fonts_generation();
    h.settle();
    let has_preedit = h
        .app
        .world()
        .get::<TextEditState>(editor)
        .expect("editor present")
        .with_buffer(|b| b.lines.iter().any(|l| l.text().contains('X')));
    assert!(has_preedit, "the live preedit must survive the bump");
}

/// Editor-style-stays-live (C2 spec §2.2 / §1.1): the editor's owned style
/// (font size) survives the bump — the sweep must reshape against the SAME
/// metrics, not reset the editor to defaults. RED until C2's style-preserving
/// TextSync fix lands; #[ignore]d like the survival test. The editor is seeded
/// at a NON-default font size so a clobber-to-default is observable.
#[ignore = "RED until C2 lands: the FontsGeneration sweep resets the editor's owned metrics; C2 preserves them and un-ignores"]
#[test]
fn editor_style_stays_live_after_a_bump() {
    let mut h = TextExtractHarness::new();
    let mut state = TextEditState::for_font_size(28.0); // non-default size
    {
        let fonts = h.app.world().resource::<SharedFontSystem>().clone();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert("Ag".into()), false, false);
    }
    let (size_before, _) = state.metrics_for_test();
    assert_eq!(
        size_before, 28.0,
        "seed sanity: the editor holds the 28px metrics"
    );
    let editor = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(200.0).height_px(60.0),
            Text(String::new()), // the required display carrier (production shape)
            state,
        ))
        .id();
    h.settle();
    h.bump_fonts_generation();
    h.settle();
    let (size_after, _) = h
        .app
        .world()
        .get::<TextEditState>(editor)
        .expect("editor present")
        .metrics_for_test();
    assert_eq!(
        size_after, 28.0,
        "the editor's owned font size must survive the bump; on pre-C2 main the \
         sweep resets it (the C2 editor-style-stays-live gate)"
    );
}
