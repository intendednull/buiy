//! E6 Task 3 — auto-scroll-into-view (editing-and-ime § 9). The pure clamp
//! (`clamp_into_view`) keeps the caret inside the viewport window with a
//! margin; the `auto_scroll_caret` system writes the result into the layout
//! `ScrollOffset` (x single-line / y multi-line), which does NOT invalidate
//! Taffy.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::layout::{LayoutPlugin, ScrollOffset, Style};
use buiy_core::text::edit::{EditCommand, SingleLine, TextEditState, clamp_into_view};
use buiy_core::text::{SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::{Metrics, Motion};

#[test]
fn clamp_pans_to_reveal_caret_past_the_right_edge() {
    // viewport [offset=0 .. 100]; caret at 130 (past the right edge); margin 4.
    // The window must shift so caret+margin <= offset+extent ⇒ offset = 130+4-100 = 34.
    let new = clamp_into_view(0.0, 100.0, 130.0, 1.0, 4.0);
    assert_eq!(new, 34.0);
}

#[test]
fn clamp_pans_to_reveal_caret_before_the_left_edge() {
    // viewport [offset=50 .. 150]; caret at 30 (before the left edge); margin 4.
    // offset must drop so caret-margin >= offset ⇒ offset = 30-4 = 26.
    let new = clamp_into_view(50.0, 100.0, 30.0, 1.0, 4.0);
    assert_eq!(new, 26.0);
}

#[test]
fn clamp_is_a_noop_when_caret_already_visible() {
    // caret comfortably inside [10 .. 110]; no change.
    let new = clamp_into_view(10.0, 100.0, 60.0, 1.0, 4.0);
    assert_eq!(new, 10.0);
}

#[test]
fn clamp_never_goes_negative() {
    // a caret near content start in a wide viewport keeps offset >= 0.
    let new = clamp_into_view(20.0, 100.0, 2.0, 1.0, 4.0);
    assert_eq!(new, 0.0);
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        // The full layout pipeline shapes the editor buffer and writes
        // ResolvedLayout — `caret_rect_for` needs a SHAPED buffer to publish a
        // real CaretVisual (the `text_caret_geometry` harness precedent; under
        // a bare BuiyTextPlugin `text_commit` is inert and the caret never lands).
        .add_plugins(LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    // `FocusPlugin`'s `handle_tab` reads `ButtonInput<KeyCode>` (the keyboard
    // resource a real app gets from `InputPlugin`, absent under MinimalPlugins).
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

#[test]
fn single_line_caret_past_right_edge_pans_x_only() {
    let mut app = app();
    // A narrow 40px-wide single-line field with a long value — the caret at End
    // is far past the right edge once we move there. A sized parent gives the
    // layout pass a real content box (the `text_caret_geometry` precedent).
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(40.0)
                .height_px(20.0)
                .overflow_hidden(),
            Text(String::from("the quick brown fox jumps")),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            SingleLine,
            ScrollOffset::default(),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_row().width_px(400.0).height_px(40.0),
        ))
        .add_child(editor);
    // Settle so TextSync lowers Text → editor buffer and TextCommit shapes it.
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // Move the caret to the End (far right).
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(
            &mut fs,
            EditCommand::Motion(Motion::End, false),
            true,
            false,
        );
    }
    app.update(); // E3 writes the caret rect; E6 auto-scroll pans
    app.update(); // (one-frame settle for the reshaped buffer)

    let offset = app.world().get::<ScrollOffset>(editor).unwrap();
    assert!(
        offset.x > 0.0,
        "single-line caret past the right edge pans x: {}",
        offset.x
    );
    assert_eq!(offset.y, 0.0, "single-line never pans y");
}
