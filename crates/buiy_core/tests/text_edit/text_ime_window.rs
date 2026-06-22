//! E5 — the preedit geometry projection + IME popup positioning
//! (editing-and-ime § 6.3). `PreeditVisual` mirrors the live span each
//! render-prep frame; `Window.ime_enabled` is true while a focused, non-
//! ReadOnly, non-Disabled editor exists, and `Window.ime_position` tracks
//! the caret bottom-left in logical window coords. Headless: a synthetic
//! Window entity, no winit — bevy_winit forwards these fields, but the math
//! is testable without it.

use bevy::prelude::*;
use bevy::window::{Ime, PrimaryWindow, Window};
use buiy_core::layout::Style;
use buiy_core::text::PreeditVisual;
use buiy_core::text::Text;
use buiy_core::text::edit::TextEditState;
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn app_with_window_and_editor() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(buiy_core::layout::LayoutPlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_message::<Ime>();
    // FocusPlugin's `handle_tab` needs `Res<ButtonInput<KeyCode>>` (no
    // InputPlugin under MinimalPlugins); seed it like `text_ime_system.rs`.
    app.insert_resource(ButtonInput::<KeyCode>::default());
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text(String::new()),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();
    (app, window, editor)
}

/// A focused, non-ReadOnly editor sets `ime_enabled = true`.
#[test]
fn focused_editor_enables_ime() {
    let (mut app, window, _editor) = app_with_window_and_editor();
    app.update();
    assert!(
        app.world().get::<Window>(window).unwrap().ime_enabled,
        "a focused editable enables IME (§ 6.3)"
    );
}

/// Unfocusing the editor turns IME off.
#[test]
fn unfocus_disables_ime() {
    let (mut app, window, _editor) = app_with_window_and_editor();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
    app.update();
    assert!(
        !app.world().get::<Window>(window).unwrap().ime_enabled,
        "no focused editor disables IME"
    );
}

/// A live preedit projects into a non-collapsed `PreeditVisual` on the editor.
#[test]
fn preedit_projects_into_preedit_visual() {
    let (mut app, _window, editor) = app_with_window_and_editor();
    // Type a logical char, then compose a preedit via the Ime message.
    app.world_mut().write_message(Ime::Preedit {
        window: Entity::PLACEHOLDER,
        value: "ni".to_string(),
        cursor: Some((0, 2)),
    });
    app.update(); // apply_ime splices
    app.update(); // measure/commit reshapes; geometry writer projects
    let pv = app.world().get::<PreeditVisual>(editor);
    assert!(pv.is_some(), "a live preedit yields a PreeditVisual");
    assert!(
        !pv.unwrap().is_collapsed(),
        "the underline span is non-empty"
    );

    // Commit clears it.
    app.world_mut().write_message(Ime::Commit {
        window: Entity::PLACEHOLDER,
        value: "你".to_string(),
    });
    app.update();
    app.update();
    assert!(
        app.world().get::<PreeditVisual>(editor).is_none(),
        "commit removes the PreeditVisual (no orphan underline)"
    );
}
