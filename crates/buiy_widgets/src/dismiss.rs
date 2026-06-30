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
//!   anchor/trigger), the overlay is closed (see `close_overlay`). A press
//!   **inside** the overlay does not dismiss it.
//! - **Escape** ([`escape_dismiss`]): a keyboard handler in `BuiySet::Input` that
//!   closes the top-most open light-dismiss overlay on `Escape`.
//!
//! "Top-most" is the back of the layout `TopLayerActivation.order` deque (the
//! most-recently-activated top-layer entity), filtered to entities that are
//! **open** ([`is_open`]) and opt into light-dismiss ([`LightDismiss`]). Both
//! channels close through the model-agnostic `close_overlay` sink: a raw overlay
//! (a plain tooltip / popover) rides the existing [`CssVisibility`] show/hide channel
//! directly — the same channel the P1d tooltip honor flips — while a migrated machine
//! overlay (a `Menu`) closes through its `Model`'s `Msg` funnel via a registered
//! [`DismissRegistry`] hook (spec §9). A dismissed overlay leaves layout + a11y
//! presence intact and re-opens by flipping `CssVisibility` back to `Visible`.
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
/// Closing routes through the model-agnostic `close_overlay` sink: a raw overlay
/// sets [`CssVisibility::Hidden`] directly (the existing show/hide channel), while a
/// machine overlay closes through its `Model`'s `Msg` funnel via [`DismissRegistry`];
/// re-opening flips `CssVisibility` back to `Visible`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct LightDismiss {
    /// The trigger entity that opened this overlay, if any. A press on the
    /// trigger does **not** dismiss the overlay (the trigger owns the toggle —
    /// dismissing here would fight a trigger that re-opens on the same press).
    /// `None` = no trigger exemption.
    pub trigger: Option<Entity>,
}

/// Why a generic light-dismiss fired — the **model-agnostic** cause the two dismiss
/// channels carry into a registered [`DismissRegistry`] hook. A machine overlay's hook
/// maps it onto its own domain reason (the menu hook maps it to a `DismissReason`), so
/// `dismiss.rs` never names a widget's close vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DismissCause {
    /// The C5-b **pointer** light-dismiss channel ([`light_dismiss_on_press`]).
    OutsidePress,
    /// The C5-b **keyboard Escape** channel ([`escape_dismiss`]).
    Escape,
}

/// A type-erased **close-hook** for an overlay whose visibility is owned by a
/// machine-tier `Model` (spec §9). Given the overlay entity + the dismiss
/// [`DismissCause`], it enqueues that machine's close-`Msg` and returns `Some(())` if
/// it APPLIES to the overlay, or `None` if it does not (so the generic dismiss falls
/// through to the next hook / the default direct `CssVisibility::Hidden` write). It
/// receives `&mut World` so the hook writes the model's inbox directly (the
/// [`crate::menu`] hook does the `MenuModel` enqueue).
pub type DismissHook = Box<dyn Fn(&mut World, Entity, DismissCause) -> Option<()> + Send + Sync>;

/// The generic **dismiss-through-the-funnel** registry (spec §9, D9) — the un-invert of
/// the W6a `With<MenuModel>` stopgap. A resource of `buiy_widgets`-populated
/// [`DismissHook`]s the generic `close_overlay` consults **before** its default direct
/// `CssVisibility::Hidden` write, so a migrated overlay (a `Menu`) closes through its
/// machine's `Msg` funnel (single-writer, recorded, projected same-frame) while
/// `dismiss.rs` stays **model-agnostic** (it never names `MenuModel`).
///
/// **Why a registry (spec §9/§17).** A migrated `Menu`'s visibility must be driven by
/// `MenuModel.open` via an enqueued `MenuMsg::Close`, but a direct `CssVisibility::Hidden`
/// write here would be the exact second-writer that forced the old `sync_menu_dismissed`
/// reconciliation. Core's dismiss substrate cannot name a concrete widget model without
/// coupling, so each overlay machine registers a hook here once at plugin build; the sink
/// merely **consults** it. Mirrors
/// [`InlineActionRegistry`](buiy_core::a11y::InlineActionRegistry) /
/// [`ReplayRegistry`](buiy_core::mvu::ReplayRegistry) — entity-free, never recorded,
/// replay-safe infrastructure (NOT a per-entity `Box<dyn Fn>` component, which would be
/// non-`Reflect` and foul seed-scene serialization).
#[derive(Resource, Default)]
pub struct DismissRegistry {
    hooks: Vec<DismissHook>,
}

impl DismissRegistry {
    /// Register a close-hook. Called once per overlay machine by `buiy_widgets` at
    /// plugin build (the menu hook; `Dialog`/`Popover` as they migrate).
    pub fn register(&mut self, hook: DismissHook) {
        self.hooks.push(hook);
    }

    /// Consult the hooks in registration order; the first that returns `Some(())`
    /// HANDLED the overlay (it enqueued the close-`Msg`). `None` ⇒ no hook applies (the
    /// caller does the default direct `CssVisibility::Hidden` write). Takes `&mut World`
    /// (the hooks write a model inbox) — call inside a deferred `commands.queue` World
    /// closure, lifted via [`World::resource_scope`].
    fn consult(&self, world: &mut World, overlay: Entity, cause: DismissCause) -> Option<()> {
        self.hooks
            .iter()
            .find_map(|hook| hook(world, overlay, cause))
    }
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

/// Close `overlay` — the model-agnostic light-dismiss sink (spec §9, the un-invert of
/// the W6a `With<MenuModel>` stopgap). Defers a world step that first consults the
/// [`DismissRegistry`]: a registered overlay (a migrated machine like a `Menu`) closes
/// through its `Msg` funnel (the hook enqueues the close-`Msg`; the early machine drain
/// folds it same-frame and the bind projects `open=false` back onto `CssVisibility` +
/// the button's `A11yExpanded`, deleting the reconciliation by construction); an
/// unregistered raw overlay (a plain tooltip / popover / anchored panel) takes the
/// default direct `CssVisibility::Hidden` write (idempotent — inserted on a real
/// transition only).
///
/// **Why deferred (the §4.3 timing edge).** The whole step is queued via
/// `commands.queue` so (a) the registry consult gets `&mut World` (the hooks write a
/// model inbox), and (b) the light-dismiss-observer-sourced enqueue is flushed into the
/// machine inbox before the early machine drain reads it — the observer-command-flush
/// timing the §4.4 same-frame acceptance test pins. `dismiss.rs` never names
/// `MenuModel`; the menu's mapping lives in its registered hook.
fn close_overlay(commands: &mut Commands, overlay: Entity, cause: DismissCause) {
    commands.queue(move |world: &mut World| {
        // A registered overlay closes through its machine funnel (single-writer); the
        // first hook that claims it has enqueued the close-Msg.
        let handled = world.contains_resource::<DismissRegistry>()
            && world
                .resource_scope(|world, registry: Mut<DismissRegistry>| {
                    registry.consult(world, overlay, cause)
                })
                .is_some();
        if handled {
            return;
        }
        // The default direct show/hide write (a raw tooltip / popover). Idempotent —
        // only insert on a real transition (an overlay that defaulted to `Visible`
        // without an explicit `CssVisibility` still gets the component).
        if world.get::<CssVisibility>(overlay) != Some(&CssVisibility::Hidden) {
            world.entity_mut(overlay).insert(CssVisibility::Hidden);
        }
    });
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
    // A registered machine overlay (a Menu) routes its close through the funnel; a
    // raw overlay writes `CssVisibility::Hidden`. Model-agnostic — see [`close_overlay`].
    close_overlay(&mut commands, overlay, DismissCause::OutsidePress);
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
    // A registered machine overlay (a Menu) routes its close through the funnel; a
    // raw overlay writes `CssVisibility::Hidden`. Model-agnostic — see [`close_overlay`].
    close_overlay(&mut commands, overlay, DismissCause::Escape);
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
