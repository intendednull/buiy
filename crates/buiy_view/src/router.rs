//! The typed press router (spec §2 "Routers", DX-3).
//!
//! The reconciler stamps a [`PressAction<M>`] value component onto each enabled
//! interactive entity; [`route_presses`] reads `OnPress` and lowers it into the
//! MVU funnel via the real [`enqueue`]. The app author writes NO routing system
//! — that is the DX-3 win (the `route_counter_press` a hand-bind demo needs is
//! exactly what this deletes).
//!
//! **Replay-safety by type (spec §2 "the replay-safety rule").** A handler
//! stores a `Msg` *value*, never a captured closure: a value cannot close over
//! a `Res` snapshot that would diverge on a fresh-process replay, and the
//! recorded thing is always the resulting `Msg` folded through the funnel.

use bevy::prelude::*;
use buiy_core::interaction::OnPress;
use buiy_core::mvu::{Model, enqueue};

/// The message a press on this entity enqueues, and onto which model. Generic
/// over the model `M` so the value is the concrete `M::Msg` — no type erasure,
/// no stored closure (a value suffices for `on_press`).
#[derive(Component)]
pub(crate) struct PressAction<M: Model> {
    pub(crate) msg: M::Msg,
    pub(crate) model: Entity,
}

/// The library-generic press router: `OnPress(entity)` → look up the entity's
/// [`PressAction`] → [`enqueue`] the message onto its owning model. Installed by
/// `ui()` in [`buiy_core::mvu::MvuSet::Enqueue`]; the app author writes nothing.
pub(crate) fn route_presses<M: Model>(
    mut presses: MessageReader<OnPress>,
    actions: Query<&PressAction<M>>,
    mut commands: Commands,
) {
    for OnPress(e) in presses.read() {
        if let Ok(action) = actions.get(*e) {
            enqueue::<M>(&mut commands, action.model, action.msg.clone());
        }
    }
}
