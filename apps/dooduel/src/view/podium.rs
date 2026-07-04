//! Podium screen (stub — filled during the per-screen port).
use buiy::view::{Element, column, text};

use crate::{Dooduel, Msg};

pub fn podium(_s: &Dooduel) -> Element<Msg> {
    column![text("podium")]
}
