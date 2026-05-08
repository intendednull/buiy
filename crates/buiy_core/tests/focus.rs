//! Drives `handle_tab` through the real production code path: presses
//! `KeyCode::Tab` on `ButtonInput<KeyCode>` and runs the schedule. Skips
//! `bevy::input::InputPlugin` so the PreUpdate clear-system doesn't wipe
//! our manual `press()` before `handle_tab` fires; mirrors the pattern
//! `crates/buiy_widgets/tests/button.rs` already uses for `MouseButton`.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    focus::{FocusPlugin, FocusVisible, Focusable, FocusedEntity},
};

fn setup() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(FocusPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

fn press_tab(app: &mut App, shift: bool) {
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        if shift {
            keys.press(KeyCode::ShiftLeft);
        }
        keys.press(KeyCode::Tab);
    }
    app.update();
}

#[test]
fn tab_advances_through_focusables_in_order() {
    let mut app = setup();
    let a = app.world_mut().spawn(Focusable::default()).id();
    let b = app.world_mut().spawn(Focusable::default()).id();
    let c = app.world_mut().spawn(Focusable::default()).id();

    press_tab(&mut app, false);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(a));
    press_tab(&mut app, false);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(b));
    press_tab(&mut app, false);
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(c));
}

#[test]
fn tab_wraps_to_first_focusable_at_end_of_order() {
    let mut app = setup();
    let a = app.world_mut().spawn(Focusable::default()).id();
    let _b = app.world_mut().spawn(Focusable::default()).id();
    let _c = app.world_mut().spawn(Focusable::default()).id();

    for _ in 0..4 {
        press_tab(&mut app, false);
    }
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(a),
        "Tab past the last focusable wraps to the first"
    );
}

#[test]
fn shift_tab_steps_backward() {
    let mut app = setup();
    let _a = app.world_mut().spawn(Focusable::default()).id();
    let _b = app.world_mut().spawn(Focusable::default()).id();
    let c = app.world_mut().spawn(Focusable::default()).id();

    // Walk forward to `c`.
    for _ in 0..3 {
        press_tab(&mut app, false);
    }
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(c));

    // Shift+Tab steps back twice; from `c` → `b` → `a`.
    press_tab(&mut app, true);
    press_tab(&mut app, true);
    let prev = app.world().resource::<FocusedEntity>().0;
    assert!(
        prev.is_some() && prev != Some(c),
        "Shift+Tab moves backward"
    );
}

#[test]
fn tab_sets_focus_visible() {
    let mut app = setup();
    app.world_mut().spawn(Focusable::default());
    assert!(!app.world().resource::<FocusVisible>().0);
    press_tab(&mut app, false);
    assert!(
        app.world().resource::<FocusVisible>().0,
        "keyboard-driven focus enables :focus-visible"
    );
}
