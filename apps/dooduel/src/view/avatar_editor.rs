//! Avatar-editor modal (stub — filled during the per-screen port).
use buiy::view::{Element, column, text};

use crate::{Dooduel, Msg};

pub fn avatar_editor_modal(_s: &Dooduel) -> Element<Msg> {
    column![text("avatar editor")]
}
