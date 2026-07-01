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
//! **Widget root:** activation lowers only for entities carrying an
//! **activatable** role ([`is_activatable_role`] — Button/Checkbox/Switch), so a
//! click on a text input or a plain node does not spuriously activate. The role
//! lives in `buiy_core`, so this producer stays widget-crate-agnostic (C4
//! widgets carry the role via their `#[require]` contract). Only the primary
//! button activates.

use crate::a11y::A11yRole;
use crate::interaction::OnPress;
use bevy::picking::events::{Click, Pointer, Press};
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use std::collections::HashMap;

/// Whether a primary-click on a node of `role` lowers to the shared [`OnPress`]
/// activation sink. The activatable roles are exactly those with a keyboard
/// activation contract (`A11yRole::Button`/`Checkbox`/`Switch`): a click on a
/// text input or a plain `Generic`/`Text` node must not activate. This is the
/// pointer-side mirror of the keyboard's `activation_keys` keymap — both gate on
/// the same set of activatable roles, so pointer/keyboard/AT converge on the one
/// `OnPress` consumer that advances the widget's state (a Button fires its
/// callback; a Checkbox/Switch advances its `A11yToggled`).
pub fn is_activatable_role(role: A11yRole) -> bool {
    matches!(
        role,
        A11yRole::Button | A11yRole::Checkbox | A11yRole::Switch
    )
}

/// Observes the committed `Pointer<Click>` stream and writes [`OnPress`] for an
/// activatable widget root ([`is_activatable_role`]) on a primary-button click.
/// Registered as an observer by [`crate::picking::PickingPlugin`].
///
/// This is the pointer producer into the shared `OnPress` sink (co-drive SC-1):
/// the SAME message the action router's `Action::Click` honor and the keyboard
/// activation handlers emit, so all three modalities converge on one sink.
pub fn pointer_click_emits_on_press(
    click: On<Pointer<Click>>,
    roles: Query<&A11yRole>,
    mut writer: MessageWriter<OnPress>,
) {
    if click.event.button != PointerButton::Primary {
        return;
    }
    // TOUCH activates via the press/release path ([`touch_press_records_target`] +
    // [`touch_release_activates`]), NOT `Click`: bevy_picking derives `Pointer<Click>`
    // from the PREVIOUS frame's hover map, which a first-touch tap never populates
    // (the `PointerId::Touch` pointer is spawned on press and despawned after
    // release), so a tap produces no `Click`. Skipping touch here is also what keeps
    // touch from double-activating when a multi-frame tap DID yield a `Click`.
    if matches!(click.pointer_id, PointerId::Touch(_)) {
        return;
    }
    let target = click.entity;
    // Only entities carrying an activatable role lower to `OnPress` — a click on
    // a text input / plain node must not activate.
    if roles.get(target).copied().is_ok_and(is_activatable_role) {
        writer.write(OnPress(target));
    }
}

/// Per-touch-pointer press target (the touch-activation half of the touch-input
/// fix; the location half is `picking/backend.rs::sync_pointer_location_on_button`).
/// A `PointerId::Touch` press over an activatable widget records its target here so
/// the matching release can activate it — see [`touch_tap_activates`] for why this
/// can't ride `Pointer<Click>`/`Pointer<Release>` (both need the PREVIOUS frame's
/// hover map, which a first-touch tap never populates).
#[derive(Resource, Default)]
pub struct TouchPressTargets(HashMap<PointerId, Entity>);

/// Records the press target of a TOUCH pointer over an activatable widget root, so
/// [`touch_tap_activates`] can fire `OnPress` if the release still hovers it.
/// `Pointer<Press>` targets the CURRENT hover map (events.rs:928), so unlike
/// `Pointer<Release>`/`Click` it DOES fire for a cold tap. Mouse/pen are untouched
/// — they keep the `Click` path ([`pointer_click_emits_on_press`]). The event
/// propagates child→parent; the (typically single) activatable ancestor is recorded.
pub fn touch_press_records_target(
    press: On<Pointer<Press>>,
    roles: Query<&A11yRole>,
    mut targets: ResMut<TouchPressTargets>,
) {
    if press.event.button != PointerButton::Primary {
        return;
    }
    let PointerId::Touch(_) = press.pointer_id else {
        return;
    };
    let target = press.entity;
    if roles.get(target).copied().is_ok_and(is_activatable_role) {
        targets.0.insert(press.pointer_id, target);
    }
}

/// Activates a touch-tapped widget on the raw `Release` [`PointerInput`], gated on
/// the CURRENT [`HoverMap`]. bevy_picking's `Pointer<Release>` and `Pointer<Click>`
/// BOTH target the PREVIOUS frame's hover map (events.rs:656), which a first-touch
/// tap never populates — the `PointerId::Touch` pointer is spawned on press and
/// despawned right after release — so neither event fires for a tap (verified by
/// running it). This reads the raw `Release` action + the current `HoverMap` (which
/// `sync_pointer_location_on_button` → `emit_picks` populate for the release
/// location) and fires `OnPress` iff the release still hovers the pressed target
/// (drag-cancel: a release dragged off the pressed widget does not activate). Runs
/// after `PickingSystems::Hover`, so the hover map is current and the press record
/// (written by [`touch_press_records_target`], a `Pointer<Press>` observer) exists.
pub fn touch_tap_activates(
    mut inputs: MessageReader<PointerInput>,
    hover_map: Res<HoverMap>,
    mut targets: ResMut<TouchPressTargets>,
    mut writer: MessageWriter<OnPress>,
) {
    for input in inputs.read() {
        let PointerId::Touch(_) = input.pointer_id else {
            continue;
        };
        if !matches!(input.action, PointerAction::Release(PointerButton::Primary)) {
            continue;
        }
        let Some(pressed) = targets.0.remove(&input.pointer_id) else {
            continue;
        };
        let still_over = hover_map
            .0
            .get(&input.pointer_id)
            .is_some_and(|hits| hits.contains_key(&pressed));
        if still_over {
            writer.write(OnPress(pressed));
        }
    }
}
