//! In-game screen (stub — filled during the per-screen port).
use buiy::view::{Element, column, text};

use crate::{Dooduel, Msg};

pub fn in_game(_s: &Dooduel) -> Element<Msg> {
    column![text("in_game")]
}
