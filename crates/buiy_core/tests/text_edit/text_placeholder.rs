//! E6 Task 4 — placeholder rendering (editing-and-ime § 10). When the editor's
//! logical value is empty (preedit excluded), the `Placeholder` string is
//! shaped into a display-only `PlaceholderBuffer`; the moment a real char
//! exists the placeholder buffer is cleared. The string NEVER enters the
//! editor buffer or the undo history.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::edit::{EditCommand, Placeholder, PlaceholderActive, TextEditState};
use buiy_core::text::{FontSize, SharedFontSystem, Text};
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
    app
}

#[test]
fn placeholder_is_active_when_value_empty() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Search…")),
        ))
        .id();
    app.update();
    app.update();

    assert!(
        app.world().get::<PlaceholderActive>(editor).is_some(),
        "an empty editor with a Placeholder shows it"
    );
    // The editor buffer is still empty — the placeholder never entered it.
    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().value(),
        ""
    );
    assert_eq!(
        app.world()
            .get::<TextEditState>(editor)
            .unwrap()
            .undo_depth(),
        0,
        "the placeholder is not an undoable edit"
    );
}

#[test]
fn placeholder_vanishes_on_first_real_char() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Search…")),
        ))
        .id();
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    assert!(app.world().get::<PlaceholderActive>(editor).is_some());

    // Type one real char.
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert("a".to_string()), false, false);
    }
    app.update();

    assert!(
        app.world().get::<PlaceholderActive>(editor).is_none(),
        "placeholder vanishes once a real char exists"
    );
}

#[test]
fn active_placeholder_buffer_shapes_to_at_least_one_run() {
    // M3 regression guard: the PlaceholderBuffer must SHAPE (its own
    // shape_until_scroll), not defer — else layout_runs() is empty and the
    // placeholder paints nothing. All-default fixtures would hide a 0-run bug.
    use buiy_core::text::edit::PlaceholderBuffer;

    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Search")),
        ))
        .id();
    app.update();
    app.update();

    let ph = app
        .world()
        .get::<PlaceholderBuffer>(editor)
        .expect("an active placeholder has a PlaceholderBuffer");
    let run_count = ph.buffer.layout_runs().count();
    assert!(
        run_count >= 1,
        "the placeholder buffer is shaped (>=1 run), got {run_count}"
    );
}
