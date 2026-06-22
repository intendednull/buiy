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
