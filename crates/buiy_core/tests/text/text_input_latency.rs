//! E2 — the EDITOR-INPUT **frame-count convergence** gate (OQ#1,
//! editing-and-ime § 12 / readiness report).
//!
//! NAMING CAVEAT (audit #40): "latency" here means FRAME COUNT, **not**
//! wall-clock time. This test pins the SCHEDULE INVARIANT — *how many*
//! `app.update()` frames separate an editor keystroke from the frame its glyph
//! publishes — and asserts nothing about milliseconds, allocations, or
//! throughput. Wall-clock performance of the shape→layout→extract hot path is
//! the criterion bench `crates/buiy_core/benches/pipeline.rs` (`cargo bench -p
//! buiy_core --bench pipeline`); cite THAT for perf, this file for the
//! N → N+1 frame contract.
//!
//! The contract: a keystroke applied in `BuiySet::Input` (the editor path)
//! reaches a freshly-published `ExtractedGlyphs` in exactly ONE more frame
//! (N → N+1), because Input runs two sets AFTER Layout, so the edit's reshape is
//! picked up by NEXT frame's TextSync → measure → TextCommit → extract. This is
//! DISTINCT from T8's `text_typing_latency` fixture, which mutates `Text` BEFORE
//! Layout (the sync-side path) — that fixture must not be cited as editor-path
//! proof (readiness § gate caveat).
//!
//! Headless on the adapterless extract harness; the edit is driven through
//! the real `apply_keyboard_edits` system (a synthetic `KeyboardInput` +
//! `FocusedEntity`), so the WHOLE editor pipeline is exercised, not a `Text`
//! poke.

use crate::support::extract_harness::TextExtractHarness;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::layout::Style;
use buiy_core::text::Text;
use buiy_core::text::edit::TextEditState;
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

/// Spawn a focused editable "Hi" under a sized column root — two glyphs, so
/// one typed char appends exactly one new instance (the T8 fixture shape,
/// but EDITABLE and focused).
fn spawn_focused_editor(h: &mut TextExtractHarness) -> Entity {
    let editor = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
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
        .add_child(editor);
    // The harness (CorePlugin + LayoutPlugin + BuiyTextPlugin) provides NO
    // FocusedEntity (FocusPlugin owns it — M2) and no InputPlugin. Add
    // FocusPlugin so `FocusedEntity` exists (and `handle_tab` is harmless —
    // no Tab is sent here), register `KeyboardInput` so `write_message` is
    // not dropped, and insert `ButtonInput<KeyCode>` for the modifier read /
    // `handle_tab`. Then focus the editor so `apply_keyboard_edits` targets
    // it. Plugins/resources may be added before the first update; the
    // harness has not updated yet (settle() runs after this).
    h.app.add_plugins(buiy_core::focus::FocusPlugin);
    h.app.add_message::<KeyboardInput>();
    h.app
        .world_mut()
        .insert_resource(ButtonInput::<KeyCode>::default());
    h.app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    editor
}

#[test]
fn one_frame_from_input_edit_to_glyph_publish() {
    let mut h = TextExtractHarness::new();
    let editor = spawn_focused_editor(&mut h);
    h.settle();

    let count0 = h.glyph_count();
    let publishes0 = h.changed_frames();
    let window = h.app.world_mut().spawn(()).id();

    // THE keystroke: a synthetic KeyboardInput '!' enqueued so that the
    // edit is applied by apply_keyboard_edits in BuiySet::Input THIS frame
    // (frame N) — AFTER Layout already ran. The reshape is therefore picked
    // up by frame N+1's TextSync.
    h.app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Digit1,
        logical_key: Key::Character("!".into()),
        state: ButtonState::Pressed,
        text: Some("!".into()),
        repeat: false,
        window,
    });

    // Frame N: Update applies the edit in Input (post-Layout) — so this
    // frame's extract sees the OLD glyph set (the edit missed this frame's
    // TextSync/measure/commit).
    h.frame();
    // The edit DID land in the editor buffer this frame (apply_keyboard_edits
    // ran in Input) — this is the M1 proof half: the buffer changed, the
    // node was dirty-marked, but the glyphs have NOT flowed yet.
    //
    // The '!' inserts at the editor's caret, which is at buffer index 0: a
    // freshly-seeded editor (Text("Hi") lowered via the sync `set_text` path)
    // leaves the cosmic cursor at (0,0) — `set_text` seeds the buffer but does
    // NOT move the editor caret, and caret-on-focus-gain (move to end) is an
    // E6 lifecycle behavior, not E2's. So the typed char prepends ⇒ "!Hi".
    // (The plan's `"Hi!"` presumed a caret-at-end this substrate does not yet
    // establish; the latency gate — Task 8's subject — is insert-position-
    // independent: one inserted char publishes exactly one new glyph wherever
    // it lands.)
    assert_eq!(
        h.app.world().get::<TextEditState>(editor).unwrap().value(),
        "!Hi",
        "the edit applied to the editor buffer on frame N (in BuiySet::Input); \
         caret sits at index 0 post-seed (caret-on-focus-gain is E6)"
    );
    assert_eq!(
        h.changed_frames(),
        publishes0,
        "frame N (edit applied post-Layout) does NOT republish — the edit \
         missed this frame's TextSync"
    );
    assert_eq!(
        h.glyph_count(),
        count0,
        "frame N still shows the pre-edit glyphs"
    );

    // Frame N+1: the node was Taffy-dirtied by apply_keyboard_edits (M1), so
    // even though TextSyncTriggers do NOT fire (Text is unchanged), this
    // frame's measure → TextCommit → extract reshape and publish the new
    // glyph. WITHOUT the dirty-mark this assertion fails (the cache holds and
    // nothing republishes) — it is the M1 regression guard.
    h.frame();
    assert_eq!(
        h.changed_frames(),
        publishes0 + 1,
        "frame N+1 publishes the edit (one-FRAME editor-input convergence — \
         OQ#1, a frame count not a wall-clock latency; proves the M1 dirty-mark \
         entered the measure path)"
    );
    assert_eq!(
        h.glyph_count(),
        count0 + 1,
        "the '!' glyph is in the N+1 published set"
    );
}
