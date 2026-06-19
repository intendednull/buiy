//! E6 Task 2 (M1) — `write_caret_blink` is the single, focus-AWARE owner of
//! `CaretVisual.visible`: an editor that is NOT the FocusedEntity is forced
//! steady-hidden (blurred carets do not blink); the focused editor blinks on
//! its per-entity phase. The bare-caret path (no TextEditState, or no
//! FocusedEntity resource) keeps the pre-E6 global-phase behavior.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::CaretVisual;
use buiy_core::text::edit::TextEditState;
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn blink_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    // `FocusPlugin`'s `handle_tab` reads `ButtonInput<KeyCode>` (the keyboard
    // resource a real app gets from `InputPlugin`, absent under MinimalPlugins).
    app.init_resource::<ButtonInput<KeyCode>>();
    // Pin the clock so the blink phase is deterministic, not MinimalPlugins luck.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

#[test]
fn unfocused_editor_caret_is_forced_hidden() {
    let mut app = blink_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            TextEditState::new(Metrics::new(16.0, 19.2)),
            CaretVisual {
                visible: true,
                rect: Rect::new(0.0, 0.0, 1.0, 16.0),
                secondary: None,
            },
        ))
        .id();
    // Nothing focused.
    app.update();
    let caret = app.world().get::<CaretVisual>(editor).unwrap();
    assert!(
        !caret.visible,
        "an unfocused editor's caret is steady-hidden"
    );
}

#[test]
fn focused_editor_caret_blinks_visible_at_phase_zero() {
    let mut app = blink_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            TextEditState::new(Metrics::new(16.0, 19.2)),
            CaretVisual {
                visible: false,
                rect: Rect::new(0.0, 0.0, 1.0, 16.0),
                secondary: None,
            },
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    // Clock paused at 0 ⇒ phase 0 ⇒ visible (even half-period).
    app.update();
    let caret = app.world().get::<CaretVisual>(editor).unwrap();
    assert!(
        caret.visible,
        "the focused editor's caret is visible at phase 0"
    );
}

#[test]
fn bare_caret_without_editor_keeps_global_phase() {
    // No TextEditState ⇒ the None arm ⇒ global phase (clock at 0 ⇒ visible),
    // regardless of focus. The E3/E5 GPU goldens rely on this.
    let mut app = blink_app();
    let caret_entity = app
        .world_mut()
        .spawn((
            Node,
            CaretVisual {
                visible: false,
                rect: Rect::new(0.0, 0.0, 1.0, 16.0),
                secondary: None,
            },
        ))
        .id();
    app.update();
    let caret = app.world().get::<CaretVisual>(caret_entity).unwrap();
    assert!(
        caret.visible,
        "a bare caret blinks on the global phase, focus-blind"
    );
}
