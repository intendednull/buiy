//! Join screen (stub — filled during the per-screen port).
use buiy::view::{Element, column, text};

use crate::{Dooduel, Msg};

pub fn join(_s: &Dooduel) -> Element<Msg> {
    column![text("join")]
}
