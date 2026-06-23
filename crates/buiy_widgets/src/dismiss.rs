//! Light-dismiss — close an open overlay on an outside press or Escape (C5-b,
//! scroll-overlay-modal.md §B.5).
//!
//! Two channels, both required (a pointer light-dismiss AND a keyboard Escape —
//! the overlay activation/dismiss duality):
//!
//! - **Pointer-outside** ([`light_dismiss_on_press`]): a global observer on the
//!   C3 `Pointer<Press>` (primary button). It uses the stacking-aware
//!   [`buiy_core::picking::hit_test`] (co-drive SC-3 — the SAME paint-order /
//!   `painters_z` derivation `emit_picks` uses, on the C1 absolute coordinate
//!   basis) to find what is actually under the press, then walks that target up
//!   the `ChildOf` chain. If the press lands **outside** the top-most open
//!   light-dismiss overlay (neither inside the overlay subtree nor on its
//!   anchor/trigger), the overlay is closed (`CssVisibility::Hidden`). A press
//!   **inside** the overlay does not dismiss it.
//! - **Escape** ([`escape_dismiss`]): a keyboard handler in `BuiySet::Input` that
//!   closes the top-most open light-dismiss overlay on `Escape`.
//!
//! "Top-most" is the back of the layout `TopLayerActivation.order` deque (the
//! most-recently-activated top-layer entity), filtered to entities that are
//! **open** ([`is_open`]) and opt into light-dismiss ([`LightDismiss`]). Closing
//! an overlay rides the existing [`CssVisibility`] show/hide channel — the same
//! channel the P1d tooltip honor flips — so a dismissed overlay leaves layout +
//! a11y presence intact and re-opens by flipping `CssVisibility` back to
//! `Visible`.
//!
//! # Why `hit_test`, not the focus-change fallback (§3.5)
//!
//! Detecting outside-clicks via focus changes (close when no descendant has
//! focus) fails for non-focusable outside targets — clicking inert decorative
//! content or empty canvas does not change focus, so a popover would not dismiss.
//! The C3 `Pointer<E>` model makes the strictly-more-correct `hit_test`-outside
//! observer available, so light-dismiss uses it.

use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::{layout::TopLayerActivation, picking::hit_test, render::components::CssVisibility};

use crate::popover::is_open;

/// Opt-in marker: an overlay carrying `LightDismiss` is closed by an outside
/// press or Escape (the `auto` `closedby` policy, §B.5). An overlay WITHOUT it
/// (a `manual`/`none` popover, or a modal that must be explicitly closed) is left
/// alone by both dismiss channels.
///
/// Closing sets [`CssVisibility::Hidden`] on the overlay root (the existing
/// show/hide channel); re-opening flips it back to `Visible`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct LightDismiss {
    /// The trigger entity that opened this overlay, if any. A press on the
    /// trigger does **not** dismiss the overlay (the trigger owns the toggle —
    /// dismissing here would fight a trigger that re-opens on the same press).
    /// `None` = no trigger exemption.
    pub trigger: Option<Entity>,
}

/// The top-most open light-dismiss overlay: the back-most entry of
/// `TopLayerActivation.order` (most-recently-activated) that carries
/// [`LightDismiss`] and is currently [`is_open`]. `None` when no such overlay is
/// up. Shared by both dismiss channels so they agree on which overlay is on top.
fn topmost_open_dismissable(
    activation: &TopLayerActivation,
    overlays: &Query<(&LightDismiss, Option<&CssVisibility>)>,
) -> Option<(Entity, LightDismiss)> {
    activation.order.iter().rev().find_map(|&e| {
        let (ld, vis) = overlays.get(e).ok()?;
        is_open(vis).then_some((e, *ld))
    })
}

/// Close `overlay` by flipping it to [`CssVisibility::Hidden`] (the existing
/// show/hide channel). Inserts the component if absent (an overlay that defaulted
/// to `Visible` without an explicit `CssVisibility`).
fn close_overlay(commands: &mut Commands, overlay: Entity, current: Option<&CssVisibility>) {
    if current != Some(&CssVisibility::Hidden) {
        commands.entity(overlay).insert(CssVisibility::Hidden);
    }
}

/// Global observer: a primary [`Pointer<Press>`] outside the top-most open
/// light-dismiss overlay closes it (§B.5, pointer channel). A press inside the
/// overlay subtree — or on its trigger — does not.
///
/// The press target is the event's **original** (leaf) target
/// ([`On::original_event_target`]) — the entity the C3 picking layer resolved
/// under the cursor for this press (the stacking-aware, `painters_z`-ordered,
/// C1-absolute-coordinate result the backend's `emit_picks` produced; the same
/// derivation `hit_test` uses — co-drive SC-3 keeps the two in lock-step). The
/// *original* target (not `press.entity`, the current bubble position) is used
/// because this is a GLOBAL observer that fires once per entity in the
/// capture→target→bubble chain; only the leaf identifies what was actually
/// pressed. It is walked up the `ChildOf` chain; if neither it nor any ancestor
/// is the overlay (or its trigger), the press is outside ⇒ dismiss. (The
/// `&World`-based [`press_is_outside`] is the equivalent check for a fixture that
/// drives `hit_test` directly.)
pub fn light_dismiss_on_press(
    press: On<Pointer<Press>>,
    overlays: Query<(&LightDismiss, Option<&CssVisibility>)>,
    parents: Query<&ChildOf>,
    activation: Option<Res<TopLayerActivation>>,
    mut commands: Commands,
) {
    if press.event.button != PointerButton::Primary {
        return;
    }
    let Some(activation) = activation else {
        return; // no top layer (no LayoutPlugin) — no overlays to dismiss
    };
    let Some((overlay, ld)) = topmost_open_dismissable(&activation, &overlays) else {
        return; // no open dismissable overlay — nothing to dismiss
    };
    let target = press.original_event_target();
    if is_inside(target, overlay, ld.trigger, &parents) {
        return; // press inside the overlay (or on its trigger) — keep it open
    }
    let current = overlays.get(overlay).ok().and_then(|(_, v)| v);
    close_overlay(&mut commands, overlay, current);
}

/// Whether `target` is `overlay`, a descendant of `overlay`, or the overlay's
/// `trigger` — i.e. a press on it must NOT light-dismiss the overlay.
fn is_inside(
    target: Entity,
    overlay: Entity,
    trigger: Option<Entity>,
    parents: &Query<&ChildOf>,
) -> bool {
    let mut current = target;
    loop {
        if current == overlay || Some(current) == trigger {
            return true;
        }
        match parents.get(current) {
            Ok(parent) => current = parent.parent(),
            Err(_) => return false,
        }
    }
}

/// Keyboard handler: `Escape` closes the top-most open light-dismiss overlay
/// (§B.5, keyboard channel). Runs in `BuiySet::Input`. Consults the SAME
/// top-most-open derivation as the pointer channel so the two agree.
///
/// `keys` is an `Option<Res<…>>`: the keyboard `ButtonInput` resource is provided
/// by `bevy::input::InputPlugin` (and seeded by the real app / the `PointerHarness`),
/// but a headless harness that adds `WidgetsPlugin` without an input stack has no
/// keyboard at all — there, the system is a no-op (no key can be pressed), which
/// keeps `WidgetsPlugin` composable in input-less test contexts (the same shape
/// the foundation's keyboard systems live under).
pub fn escape_dismiss(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    overlays: Query<(&LightDismiss, Option<&CssVisibility>)>,
    activation: Option<Res<TopLayerActivation>>,
    mut commands: Commands,
) {
    let Some(keys) = keys else {
        return; // no keyboard in this context — nothing to dismiss on
    };
    let Some(activation) = activation else {
        return; // no top layer (no LayoutPlugin) — no overlays to dismiss
    };
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let Some((overlay, _)) = topmost_open_dismissable(&activation, &overlays) else {
        return;
    };
    let current = overlays.get(overlay).ok().and_then(|(_, v)| v);
    close_overlay(&mut commands, overlay, current);
}

/// A `&World` light-dismiss check for tests / library consumers: given a press
/// `point` (window-logical) and the top-most open overlay, returns whether the
/// point is OUTSIDE the overlay (so a real press there would dismiss it). Uses
/// the production stacking-aware [`hit_test`]. Exposed so a headless fixture can
/// assert the inside/outside split without driving the full observer.
pub fn press_is_outside(
    world: &World,
    point: Vec2,
    overlay: Entity,
    trigger: Option<Entity>,
) -> bool {
    let Some(hit) = hit_test(world, point) else {
        // No node under the press at all ⇒ empty canvas ⇒ outside.
        return true;
    };
    // Walk the hit up the ChildOf chain looking for the overlay / trigger.
    let mut current = hit;
    loop {
        if current == overlay || Some(current) == trigger {
            return false; // inside (or on the trigger)
        }
        match world.get::<ChildOf>(current) {
            Some(parent) => current = parent.parent(),
            None => return true, // reached a root that is not the overlay ⇒ outside
        }
    }
}
