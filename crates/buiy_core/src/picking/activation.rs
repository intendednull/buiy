//! The pointer-side activation producer (input-event-model.md § 2.5, co-drive
//! SC-1). C3 owns only the **pointer** half of activation; the one activation
//! sink is the existing [`crate::interaction::OnPress`] message, which the P1c
//! action router (`Action::Click` → `OnPress`) and the keyboard Enter/Space
//! handlers also write. There is deliberately **no** competing `Activate` event
//! — a second sink would fork the activation grounding loop (§ 3.5).
//!
//! **Why `Pointer<Click>` is the press-arm:** bevy_picking's `Pointer<Click>`
//! already encodes "press-on-target then release-on-the-SAME-target" (its event
//! doc: a Click fires only when the press and release share a target entity). A
//! press-on-target followed by a release-OFF-target therefore produces no
//! `Click` — exactly the drag-cancel the spec's press-arm → release-on-target
//! semantics describe (§ 2.5; gate #8), for free. So the C3 pointer producer is
//! a single `Pointer<Click>` observer; no separate armed marker is needed at
//! this layer (per-widget press visuals — a pressed look while held — are C4's
//! `Pointer<Press/Release>` observers, layered on top, not a second activation
//! channel).
//!
//! **Widget root:** activation lowers only for entities carrying the button
//! activation role ([`A11yRole::Button`]), so a click on a text input or a plain
//! node does not spuriously activate. The role lives in `buiy_core`, so this
//! producer stays widget-crate-agnostic (C4 widgets carry the role via their
//! `#[require]` contract). Only the primary button activates.

use crate::a11y::A11yRole;
use crate::interaction::OnPress;
use bevy::picking::events::{Click, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

/// Observes the committed `Pointer<Click>` stream and writes [`OnPress`] for an
/// activatable widget root (`A11yRole::Button`) on a primary-button click.
/// Registered as an observer by [`crate::picking::PickingPlugin`].
///
/// This is the pointer producer into the shared `OnPress` sink (co-drive SC-1):
/// the SAME message the action router's `Action::Click` honor and the keyboard
/// Enter/Space handlers emit, so all three modalities converge on one sink.
pub fn pointer_click_emits_on_press(
    click: On<Pointer<Click>>,
    roles: Query<&A11yRole>,
    mut writer: MessageWriter<OnPress>,
) {
    if click.event.button != PointerButton::Primary {
        return;
    }
    let target = click.entity;
    // Only entities carrying the button activation role lower to `OnPress` —
    // a click on a text input / plain node must not activate.
    if roles.get(target) == Ok(&A11yRole::Button) {
        writer.write(OnPress(target));
    }
}
