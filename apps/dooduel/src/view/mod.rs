//! The view layer — the `Screen` router + shared widget helpers + the six
//! per-screen modules.
//!
//! One root, screens by kind-swap on [`Screen`](crate::Screen). The floating
//! light/dark theme toggle and the avatar-editor modal overlay every screen. The
//! avatar editor is a real **top-layer modal** (F4a retired the raster-under-modal
//! limit): an opaque panel over a translucent scrim sibling — the F4a boundary
//! requires the panel be OPAQUE (an effect-group-nested raster still drops).

use buiy::view::{Element, column, when};

use crate::{Dooduel, Msg, Screen};

pub mod widgets;

pub mod avatar_editor;
pub mod home;
pub mod in_game;
pub mod join;
pub mod lobby;
pub mod podium;

/// VIEW — one root, screens by kind-swap, with the floating theme toggle and the
/// avatar-editor modal overlaid on top.
pub fn view(s: &Dooduel) -> Element<Msg> {
    let content = match s.screen {
        Screen::Home => home::home(s),
        Screen::Join => join::join(s),
        Screen::Lobby => lobby::lobby(s),
        Screen::InGame => in_game::in_game(s),
        Screen::Podium => podium::podium(s),
    };
    column![
        content,
        widgets::theme_toggle(s),
        when(s.avatar.editor_open, avatar_editor::avatar_editor_modal(s)),
    ]
    .fill()
}
