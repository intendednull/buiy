//! Lobby screen (stub — filled during the per-screen port).
use buiy::view::{Element, column, text};

use crate::{Dooduel, Msg};

pub fn lobby(_s: &Dooduel) -> Element<Msg> {
    column![text("lobby")]
}
