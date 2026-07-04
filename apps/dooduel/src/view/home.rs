//! Home screen (stub — filled during the per-screen port).
use buiy::view::{Element, column, text};

use crate::{Dooduel, Msg};

pub fn home(_s: &Dooduel) -> Element<Msg> {
    column![text("home")]
}
