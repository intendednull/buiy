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
//! frame contract.
//!
//! The contract (REVISED — the shape-coherence fix): a keystroke applied in
//! `BuiySet::Input` (the editor path) reaches a freshly-published
//! `ExtractedGlyphs` the SAME frame. `reshape_edited_editors` runs
//! `.after(BuiySet::Input).before(write_caret_and_selection)` and reshapes the
//! just-edited editor buffer + rewrites its `ComputedTextLayout` BEFORE extract,
//! so the new glyph publishes on frame N — not N+1.
//!
//! WHY THE CHANGE: the old design deferred the edit's reshape to next frame's
//! TextCommit, leaving the editor buffer UNSHAPED at frame end. The render
//! extract reads every text entity on any damage frame (extract.rs § 6.2), so a
//! keystroke that coincided with any other damage (the first char's
//! empty↔non-empty `PlaceholderActive` toggle, a sibling row, a theme tick) read
//! the transiently-unshaped editor and tripped the `layout_runs().count() ==
//! ComputedTextLayout.lines.len()` invariant — a live crash. Coherence and the
//! old one-frame GLYPH latency are mutually exclusive (extract rebuilds ALL
//! entities on ANY damage), so the buffer is now made coherent the same frame.
//! Only the BOX LAYOUT still re-measures next frame (an edit reshapes at the
//! prior commit's content box; the already-accepted one-frame LAYOUT latency).
//! This is DISTINCT from T8's `text_typing_latency` fixture, which mutates
//! `Text` BEFORE Layout (the sync-side path).
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
    use buiy_core::text::SharedFontSystem;
    use buiy_core::text::edit::EditCommand;

    let editor = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::new()), // inert display carrier (editor owns its content)
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    // Seed the editor's OWNED content via the explicit verb (C2 § 2.3): the
    // display `Text`→editor seam is gone (C2 § 2.1). Unlike the old `set_text`
    // seam (which left the caret at 0), `Insert` leaves the caret AFTER the
    // inserted text, so a subsequently-typed char APPENDS.
    {
        let fonts = h.app.world().resource::<SharedFontSystem>().clone();
        let mut fs = fonts.lock();
        let mut state = h.app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        state.apply(&mut fs, EditCommand::Insert("Hi".into()), false, false);
    }
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
fn editor_input_edit_publishes_its_glyph_the_same_frame() {
    let mut h = TextExtractHarness::new();
    let editor = spawn_focused_editor(&mut h);
    h.settle();

    let count0 = h.glyph_count();
    let publishes0 = h.changed_frames();
    let window = h.app.world_mut().spawn(()).id();

    // THE keystroke: a synthetic KeyboardInput '!' enqueued so that the
    // edit is applied by apply_keyboard_edits in BuiySet::Input THIS frame
    // (frame N), AFTER Layout already ran. `reshape_edited_editors` then runs
    // (still frame N, after Input) and reshapes the edited buffer + rewrites
    // ComputedTextLayout before extract.
    h.app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Digit1,
        logical_key: Key::Character("!".into()),
        state: ButtonState::Pressed,
        text: Some("!".into()),
        repeat: false,
        window,
    });

    // Frame N: Update applies the edit in Input (post-Layout), then
    // reshape_edited_editors reshapes the editor buffer and ticks
    // ComputedTextLayout — so THIS frame's extract publishes the new glyph AND
    // the editor buffer is coherent (layout_runs == ComputedTextLayout.lines)
    // when extract reads it (no dirty-at-extract crash).
    h.frame();
    // The '!' inserts at the editor's caret, which sits AFTER the seed: the
    // editor is seeded via `EditCommand::Insert("Hi")` (the editor owns its
    // content, C2 § 2.1/§2.3), and `Insert` leaves the caret after the inserted
    // text. So the typed char APPENDS ⇒ "Hi!".
    assert_eq!(
        h.app.world().get::<TextEditState>(editor).unwrap().value(),
        "Hi!",
        "the edit applied to the editor buffer on frame N (in BuiySet::Input); \
         caret sits after the Insert-seeded content (post-seed)"
    );
    assert_eq!(
        h.changed_frames(),
        publishes0 + 1,
        "frame N publishes the edit the SAME frame — reshape_edited_editors \
         reshaped the buffer + ticked ComputedTextLayout after Input, before \
         extract (the shape-coherence fix; the old one-frame GLYPH latency is \
         gone — only the BOX re-measures next frame)"
    );
    assert_eq!(
        h.glyph_count(),
        count0 + 1,
        "the '!' glyph is in frame N's published set (same-frame convergence)"
    );

    // Frame N+1: the box re-measures (the one-frame LAYOUT latency that
    // remains), but the glyph set is already steady — no spurious republish,
    // no extra glyph (a coherent, idempotent steady frame).
    h.frame();
    assert_eq!(
        h.changed_frames(),
        publishes0 + 1,
        "frame N+1 is steady — the edit already published on N; the box \
         re-measure does not republish identical glyphs"
    );
    assert_eq!(
        h.glyph_count(),
        count0 + 1,
        "still exactly one new glyph after the box re-measure settles"
    );
}
