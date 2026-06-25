//! Property test for the render-extract shape-coherence invariant (the crash
//! this campaign fixed). For an ARBITRARY script of editor edits, focus changes,
//! and live IME preedits, a committed editor buffer must NEVER reach a frame end
//! UNSHAPED — `layout_runs().count()` must always equal
//! `ComputedTextLayout.lines.len()` once the frame's systems have run.
//!
//! This exhaustively exercises `commit::reshape_edited_editors` and its
//! load-bearing `.after(BuiySet::Input).after(focus_lifecycle)` ordering across
//! edit paths the hand-written tests can't enumerate: the in-`Input` keyboard
//! edits AND the post-`Input` focus-loss preedit removal, interleaved at random.
//! The app carries the debug-only `Last` invariant (`debug_assert_shape_coherence`),
//! so any script that leaves a buffer unshaped panics the case — and the explicit
//! per-frame check below makes the failure legible even in a release build.
//!
//! **The script models the real IME invariant** (it does NOT fuzz impossible
//! states): a platform IME owns the keyboard *during* composition, so raw
//! keyboard edits never interleave with a live preedit. The loop tracks
//! focus/composing state and only sends a keyboard edit when NOT composing, and
//! only starts/updates a preedit when focused — exactly the states the running
//! editor can reach. (An earlier unconstrained version surfaced a `value()`
//! out-of-bounds slice on a stale preedit span — but only by driving a raw
//! `Backspace` through a live composition, which a real IME prevents; that is an
//! unreachable state, not an editor bug, so the generator excludes it.)
//!
//! Headless: cosmic shaping is CPU (no adapter). Module of the `text_edit` group
//! binary; registered via `#[path] mod text_coherence_property;` in
//! `tests/text_edit.rs`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::edit::{TextBufferAccessReadOnly, TextEditState};
use buiy_core::text::{ComputedTextLayout, SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;
use proptest::prelude::*;

/// One scripted step. Keyboard edits route through the REAL `apply_keyboard_edits`
/// (in `BuiySet::Input`); focus changes drive `focus_lifecycle` (the post-`Input`
/// preedit-removal un-shaper); preedit splices stand in for a live IME composition.
#[derive(Debug, Clone)]
enum Op {
    Type(char),
    Backspace,
    Focus,
    Blur,
    /// Start/extend a live IME preedit composition (un-shapes the buffer).
    Preedit(char),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => prop::char::range('a', 'z').prop_map(Op::Type),
        2 => Just(Op::Backspace),
        1 => Just(Op::Focus),
        2 => Just(Op::Blur),
        2 => prop::char::range('a', 'z').prop_map(Op::Preedit),
    ]
}

/// A headless app with the full editor pipeline (Input edits + focus lifecycle +
/// the reshape repair + the `Last` coherence invariant) and a focused, COMMITTED
/// editor (sized so it gets a real `ComputedTextLayout`).
fn build() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();
    let window = app.world_mut().spawn(()).id();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(200.0).height_px(40.0),
            Text(String::new()),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();
    app.update(); // settle: the editor commits (ComputedTextLayout present)
    (app, editor, window)
}

fn key(window: Entity, code: KeyCode, logical: Key, text: Option<&str>) -> KeyboardInput {
    KeyboardInput {
        key_code: code,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window,
    }
}

/// The extract invariant, checked in the main world: the editor's authoritative
/// buffer's shaped run count equals its committed line count.
fn editor_is_coherent(app: &mut App) -> bool {
    let mut q = app
        .world_mut()
        .query::<(TextBufferAccessReadOnly, &ComputedTextLayout)>();
    let world = app.world();
    q.iter(world).all(|(access, computed)| {
        access.with_buffer(|b| b.layout_runs().count()) == computed.lines.len()
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn no_edit_sequence_leaves_the_editor_unshaped_at_frame_end(
        script in prop::collection::vec(op_strategy(), 1..14),
    ) {
        let (mut app, editor, window) = build();
        // Mirror the running editor's reachable states: it starts focused (build
        // sets `FocusedEntity`) and not composing. A keyboard edit is only sent
        // when NOT composing (the IME owns the keyboard mid-composition); a
        // preedit is only started/updated when focused. Ops invalid for the
        // current state are skipped — the frame still pumps + re-checks coherence.
        let mut focused = true;
        let mut composing = false;

        for op in &script {
            match op {
                Op::Type(c) if focused && !composing => {
                    let ev = key(window, KeyCode::KeyA, Key::Character(c.to_string().into()), Some(&c.to_string()));
                    app.world_mut().write_message(ev);
                }
                Op::Backspace if focused && !composing => {
                    let ev = key(window, KeyCode::Backspace, Key::Backspace, None);
                    app.world_mut().write_message(ev);
                }
                Op::Focus => {
                    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
                    focused = true;
                }
                Op::Blur => {
                    // Focus-loss: `focus_lifecycle` removes any live preedit (the
                    // HIGH-bug path) and the composition ends.
                    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
                    focused = false;
                    composing = false;
                }
                Op::Preedit(c) if focused => {
                    // Start/replace a live IME composition (`splice_preedit` itself
                    // removes any prior span). A later `Blur` then drives
                    // `focus_lifecycle`'s `remove_preedit`.
                    let fonts = app.world().resource::<SharedFontSystem>().clone();
                    let mut fs = fonts.lock();
                    if let Some(mut state) = app.world_mut().get_mut::<TextEditState>(editor) {
                        state.splice_preedit(&mut fs, &c.to_string(), None);
                    }
                    composing = true;
                }
                // Op invalid for the current state (e.g. a raw edit mid-composition,
                // or a preedit while blurred) — unreachable in the real editor.
                _ => {}
            }
            // Pump the frame: the edit/focus systems run, then reshape_edited_editors
            // repairs any unshaped buffer, then the `Last` invariant asserts coherence
            // (panicking the case on any miss). The explicit check below is the
            // release-build-visible mirror.
            app.update();
            prop_assert!(
                editor_is_coherent(&mut app),
                "editor buffer left UNSHAPED at frame end after op {op:?} in script {script:?}",
            );
        }
    }
}
