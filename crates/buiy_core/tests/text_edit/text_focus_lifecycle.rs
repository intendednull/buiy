//! E6 Task 2 — focus lifecycle (editing-and-ime § 10). On focus LOSS: the open
//! undo group seals and any live preedit is removed; the selection / buffer is
//! RETAINED (web parity). Caret VISIBILITY is owned by `write_caret_blink`
//! (M1) — proven separately in `text_caret_blink_focus.rs`; this file proves
//! the seal + preedit-removal edges.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::SharedFontSystem;
use buiy_core::text::edit::{EditCommand, TextEditState};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    // `FocusPlugin`'s `handle_tab` reads `ButtonInput<KeyCode>` (the keyboard
    // resource a real app gets from `InputPlugin`, absent under MinimalPlugins).
    app.init_resource::<ButtonInput<KeyCode>>();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

#[test]
fn focus_loss_seals_undo_and_retains_buffer() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2))))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();

    // Drive a typing run directly so an undo TypingRun group is OPEN.
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        for ch in "abc".chars() {
            state.apply(&mut fs, EditCommand::Insert(ch.to_string()), false, false);
        }
    }
    assert!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .undo_open_for_test(),
        "a TypingRun group is open before blur"
    );

    // Blur.
    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
    app.update();

    let state = app.world().get::<TextEditState>(editor).unwrap();
    assert!(
        !state.undo_open_for_test(),
        "focus loss seals the open undo group"
    );
    // Retention (web parity): the buffer / selection survive blur.
    assert_eq!(state.value(), "abc", "blur retains the buffer");
}

#[test]
fn focus_loss_removes_an_active_preedit() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2))))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();

    // Splice a preedit directly (simulating an in-flight composition).
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.splice_preedit(&mut fs, "ぁ", None);
    }
    assert!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .has_preedit()
    );

    // Blur ⇒ focus_lifecycle removes the orphan span (E5's deferred removal).
    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
    app.update();

    let state = app.world().get::<TextEditState>(editor).unwrap();
    assert!(
        !state.has_preedit(),
        "focus loss removes the preedit (no orphan)"
    );
    assert_eq!(state.value(), "", "the preedit was never part of the value");
}

/// **The reshape-ordering regression guard (review finding #1).** `focus_lifecycle`
/// is a SECOND post-`TextCommit` editor-buffer mutator: on focus loss with a LIVE
/// IME preedit it `remove_preedit`s, which UN-shapes the editor buffer (deferring
/// reshape to next frame). `reshape_edited_editors` must run `.after(focus_lifecycle)`
/// or the scheduler is free to reshape BEFORE the blur un-shapes — leaving the
/// buffer unshaped at the render extract (the crash this whole system prevents).
/// This drives the path on a COMMITTED editor (LayoutPlugin → a real
/// `ComputedTextLayout`, unlike `focus_loss_seals_undo_and_retains_buffer` which
/// uses a no-layout entity both systems filter out), so the debug-only `Last`
/// coherence invariant AND the explicit assert below catch a left-unshaped frame.
#[test]
fn focus_loss_with_live_preedit_leaves_the_committed_editor_shaped() {
    use buiy_core::layout::{LayoutPlugin, Style};
    use buiy_core::text::{ComputedTextLayout, Text};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.init_resource::<ButtonInput<KeyCode>>();

    // A focused editor: seeded content + a LIVE preedit composition, sized so it
    // becomes a committed layout root with a real ComputedTextLayout.
    let mut state = TextEditState::for_font_size(16.0);
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
        state.splice_preedit(&mut fs, "X", None);
    }
    assert!(state.has_preedit(), "seed: a live preedit composition");
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(200.0).height_px(40.0),
            Text(String::new()),
            state,
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();
    app.update(); // settle: commit (ComputedTextLayout present, preedit shaped)
    assert!(
        app.world().get::<ComputedTextLayout>(editor).is_some(),
        "the editor committed (has a ComputedTextLayout)"
    );

    // BLUR: focus_lifecycle removes the live preedit this frame — un-shaping the
    // buffer AFTER TextCommit. reshape_edited_editors (.after(focus_lifecycle))
    // must repair it before frame end (the `Last` invariant fires here otherwise).
    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
    app.update();

    // Explicit coherence assert (the extract invariant): the editor buffer's
    // shaped run count matches its committed line count — i.e. it was reshaped.
    let runs = app
        .world()
        .get::<TextEditState>(editor)
        .unwrap()
        .with_buffer(|b| b.layout_runs().count());
    let lines = app
        .world()
        .get::<ComputedTextLayout>(editor)
        .unwrap()
        .lines
        .len();
    assert_eq!(
        runs, lines,
        "after focus-loss preedit removal, the committed editor is reshaped \
         coherent (runs={runs} lines={lines}) — not left unshaped for extract"
    );
}
