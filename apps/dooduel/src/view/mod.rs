//! The view layer — the `Screen` router + shared widget helpers + the six
//! per-screen modules.
//!
//! One root, screens by kind-swap on `Screen`, with the floating
//! light/dark theme toggle over every screen.
//!
//! The avatar editor is a full in-flow SCREEN, not a top-layer overlay: RUNNING the
//! modal form showed the base screen's TEXT bleeding through the panel (glyphs draw
//! in one global tier after all quads, so a top-layer quad cannot occlude base
//! glyphs — see `avatar_editor`). When the editor is open the router shows it
//! INSTEAD of the underlying screen, so nothing is behind it to bleed.

use buiy::view::{Element, column};

use crate::{Dooduel, Msg, Screen};

pub mod widgets;

pub mod avatar_editor;
pub mod home;
pub mod in_game;
pub mod join;
pub mod lobby;
pub mod podium;

/// VIEW — one root, screens by kind-swap, with the floating theme toggle on top.
pub fn view(s: &Dooduel) -> Element<Msg> {
    let content = if s.avatar.editor_open {
        avatar_editor::avatar_editor_screen(s)
    } else {
        match s.screen {
            Screen::Home => home::home(s),
            Screen::Join => join::join(s),
            Screen::Lobby => lobby::lobby(s),
            Screen::InGame => in_game::in_game(s),
            Screen::Podium => podium::podium(s),
        }
    };
    column![content, widgets::theme_toggle(s)].fill()
}
