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

use buiy::view::{Element, column, when};

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
    // The floating theme toggle floats bottom-right over every screen EXCEPT in-game:
    // there the chat pane (desktop) / chat card (mobile) fills that corner, and the
    // top-layer toggle would occlude — and steal clicks from — the chat Send control
    // (a click at Send's center folded SetTheme instead of submitting the guess). The
    // theme is reducer-owned and persists, so the player's menu choice carries into the
    // match. The design keeps the toggle in-game; suppressing it there is a deliberate,
    // documented divergence to protect the primary control — see
    // docs/specs/2026-07-10-dooduel-theme-toggle-occlusion-design.md.
    column![
        content,
        when(s.screen != Screen::InGame, widgets::theme_toggle(s)),
    ]
    .fill()
}
