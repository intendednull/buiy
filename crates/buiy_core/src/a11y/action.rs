//! The **inbound action router** — the single ingress by which screen readers,
//! in-process agents, and Buiy's own headless test driver drive the running
//! app (action-router.md §1, LOCKED #6). The consumer half of
//! one-tree/N-consumers: the same nodes the outbound fold publishes
//! ([`super::translate`]) are the dispatch targets here.
//!
//! Two surfaces:
//!
//! - [`dispatch_action_request`] — the **headless seam** (action-router.md §5).
//!   A free function over `&mut World` that resolves the target, runs the
//!   liveness + live-capability filter (§3), and lowers the verb into a real
//!   Buiy sink. Runs with **no winit adapter, no GPU** — the P1c-c in-process
//!   driver calls it directly as act-then-observe.
//! - [`route_action_requests`] — the one ECS reader **system** that drains the
//!   **existing** `bevy_winit` `MessageReader<ActionRequestWrapper>` channel
//!   (no competing `AccessKit` `ActionHandler` — the slot is structurally
//!   single-occupant, LOCKED #6) and calls [`dispatch_action_request`] per
//!   request. Errors are logged, never panicked.
//!
//! Focus/Blur are honored **generically** here (set/clear [`FocusedEntity`] per
//! the keyboard=visible / programmatic convention); every other advertised verb
//! is lowered through the role's [`A11yContract::honor`](super::A11yContract).

use crate::a11y::contract::{ActionError, NotActionableReason};
use crate::a11y::states::{A11yDisabled, A11yReadOnly};
use crate::a11y::translate::entity_for_node_id;
use crate::a11y::{A11yRole, contract_for};
use crate::focus::{FocusVisible, FocusedEntity};
use crate::interaction::OnPress;
use accesskit::{Action, ActionRequest};
use bevy::a11y::ActionRequest as ActionRequestWrapper;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

/// Whether `action` *mutates* the target's value or text — the verbs the
/// read-only live filter (action-router.md §3) rejects on an [`A11yReadOnly`]
/// instance. Selecting/copying/focusing is non-mutating and stays allowed; only
/// these change state. (Only `Click` is advertised by any pre-P1d widget, so no
/// real widget reaches this branch yet; the filter is implemented generically so
/// the value/text widgets land in P1d already gated.)
fn mutates_value_or_text(action: Action) -> bool {
    matches!(
        action,
        Action::SetValue
            | Action::Increment
            | Action::Decrement
            | Action::ReplaceSelectedText
            | Action::SetTextSelection
    )
}

/// Resolve, guard, and lower one inbound [`ActionRequest`] — the **headless
/// dispatch seam** (action-router.md §5). Returns a typed [`ActionError`] on any
/// failure, **never panics** (a stale ref, an unadvertised verb, a disabled /
/// read-only instance are all soft, typed outcomes the caller surfaces loudly).
///
/// The order is liveness → capability → live state → dispatch (action-router.md
/// §3), so a verb is only honored after every gate passes:
///
/// 1. **Liveness.** [`entity_for_node_id`] maps the [`ActionRequest::target_node`]
///    back to an [`Entity`]; a `None` (the synthetic root, or a stale id whose
///    entity despawned) is [`ActionError::NotFound`].
/// 2. **Capability.** Re-read the target's role + its [`contract_for`] advertised
///    set. A verb the role never advertises is [`ActionError::Unsupported`].
///    `Focus`/`Blur` are always permitted on any live node (they ride
///    `Focusable` outbound and are honored generically here), so they bypass the
///    contract check.
/// 3. **Live state.** An [`A11yDisabled`] instance drops every actionable verb
///    ([`NotActionableReason::Disabled`]) — `Focus`/`Blur` stay allowed so the
///    node remains addressable. A mutating verb (`SetValue`/`Increment`/… —
///    `mutates_value_or_text`) on an [`A11yReadOnly`] instance is
///    [`NotActionableReason::ReadOnly`].
/// 4. **Dispatch.** `Focus`/`Blur` set/clear [`FocusedEntity`] generically;
///    every other verb is lowered through the role's
///    [`A11yContract::honor`](super::A11yContract).
pub fn dispatch_action_request(world: &mut World, req: &ActionRequest) -> Result<(), ActionError> {
    let target = req.target_node;
    let action = req.action;

    // 1. Liveness — resolve the NodeId to a live entity (a despawned/stale ref
    //    or the synthetic root → NotFound, never a panic).
    let Some(entity) = entity_for_node_id(target) else {
        return Err(ActionError::NotFound { target });
    };
    if world.get_entity(entity).is_err() {
        return Err(ActionError::NotFound { target });
    }

    // Focus/Blur are addressability verbs, permitted on ANY live node (they ride
    // `Focusable` outbound) and honored generically below — they do not consult
    // a role contract and are NOT dropped by the actionable-verb gates.
    let is_focus_verb = matches!(action, Action::Focus | Action::Blur);

    // The live role drives the capability re-check AND the honor dispatch. Read
    // it once; a node with no `A11yRole` defaults to `Generic` (no contract).
    let role = world.get::<A11yRole>(entity).copied().unwrap_or_default();

    // 2. Capability — the verb must be advertised by the role's contract (the
    //    same `actions()` list the outbound fold advertises), except the
    //    always-allowed Focus/Blur.
    if !is_focus_verb {
        let advertised = contract_for(role).is_some_and(|entry| entry.actions.contains(&action));
        if !advertised {
            return Err(ActionError::Unsupported { target, action });
        }
    }

    // 3. Live state — disabled drops every actionable verb (Focus/Blur survive,
    //    keeping the node addressable); read-only drops only mutating verbs.
    if !is_focus_verb {
        if world.get::<A11yDisabled>(entity).is_some() {
            return Err(ActionError::NotActionable {
                target,
                action,
                reason: NotActionableReason::Disabled,
            });
        }
        if world.get::<A11yReadOnly>(entity).is_some() && mutates_value_or_text(action) {
            return Err(ActionError::NotActionable {
                target,
                action,
                reason: NotActionableReason::ReadOnly,
            });
        }
    }

    // 4. Dispatch.
    match action {
        // Focus/Blur are honored generically (they never reach a contract's
        // `honor`): set/clear `FocusedEntity` + the keyboard=visible /
        // programmatic-focus-visible convention (focus.rs — `handle_tab` sets
        // the `true` half; a programmatic Focus is treated the same).
        // `FocusedEntity`/`FocusVisible` are owned by `FocusPlugin`; under a
        // partial/headless harness without it they are simply absent and the
        // verb no-ops gracefully (sink-resource discipline, action-router.md §6).
        Action::Focus => {
            if let Some(mut focused) = world.get_resource_mut::<FocusedEntity>() {
                focused.0 = Some(entity);
            }
            if let Some(mut visible) = world.get_resource_mut::<FocusVisible>() {
                visible.0 = true;
            }
            Ok(())
        }
        Action::Blur => {
            // Clear focus iff the target is the focused entity (action-router.md
            // §4): blurring a non-focused node must not steal focus from another.
            if let Some(mut focused) = world.get_resource_mut::<FocusedEntity>()
                && focused.0 == Some(entity)
            {
                focused.0 = None;
            }
            Ok(())
        }
        // Every other advertised verb lowers through the role's contract.
        _ => {
            // Guaranteed `Some` by the capability gate above (an unadvertised
            // verb already returned `Unsupported`), but stay total — a role with
            // no contract that somehow reached here is `Unsupported`, not a panic.
            let Some(entry) = contract_for(role) else {
                return Err(ActionError::Unsupported { target, action });
            };
            (entry.honor)(world, entity, action, req.data.as_ref())
        }
    }
}

/// The one inbound reader **system** (action-router.md §1): drains the
/// **existing** `bevy_winit` `MessageReader<ActionRequestWrapper>` channel and
/// calls [`dispatch_action_request`] for each. An exclusive system so it can
/// take `&mut World` for the seam; the requests are copied out of the reader
/// first so the borrow on the message buffer is released before dispatch
/// mutates the world.
///
/// **No competing `AccessKit` `ActionHandler`** — the adapter slot is
/// structurally single-occupant and already filled by `bevy_winit`; Buiy only
/// *reads* the channel it fills (LOCKED #6). Errors are **logged**, not
/// panicked: a stale ref or rejected verb must never crash the app.
///
/// Scheduled first-in-`BuiySet::Input` (see [`super::A11yPlugin`]'s
/// `.before(...)` constraints) so an action consumed here reflects outbound in
/// the *same* frame's `A11yUpdate` (action-router.md §7).
pub fn route_action_requests(world: &mut World) {
    // Copy the pending requests out so the `Messages` borrow ends before we
    // mutate the world per request. `ActionRequestWrapper` is the bevy_a11y
    // newtype around `accesskit::ActionRequest`; its `.0` is the inner request.
    let requests: Vec<ActionRequest> = {
        let Some(mut reader) =
            world.get_resource_mut::<bevy::ecs::message::Messages<ActionRequestWrapper>>()
        else {
            return; // No bevy_winit a11y channel registered (headless) — inert.
        };
        reader.drain().map(|wrapper| wrapper.0).collect()
    };

    for req in &requests {
        if let Err(err) = dispatch_action_request(world, req) {
            // A soft, typed failure (stale ref, unadvertised/forbidden verb).
            // Surface it for diagnostics but never panic — the AT/agent path
            // must degrade gracefully (action-router.md §3).
            warn!("inbound action request not honored: {err:?}");
        }
    }
}

/// The APG keyboard-activation keymap (co-drive SC-1, WAI-ARIA APG): the set of
/// keys that **activate** the focused widget of `role`, lowering to the shared
/// [`OnPress`] sink. `None` ⇒ the role has no keyboard-activation contract (it is
/// not an activatable widget — a plain `Generic`/`Text` node).
///
/// This is the **single source of the keyboard asymmetry** the gate-#7 fixture
/// asserts, role-keyed rather than per-widget special-cased so a new activatable
/// widget adds exactly one arm here:
///
/// - **Button** — `Enter` AND `Space` (APG button).
/// - **Checkbox** — `Space` ONLY. **Enter does NOTHING** — the canonical APG
///   asymmetry (a checkbox does not toggle on Enter; in a form Enter submits, it
///   does not check the box). Load-bearing vs Button.
/// - **Switch** — `Space` AND `Enter` (APG switch — like a button, both keys
///   toggle).
///
/// Every activating key writes the SAME `OnPress` message the pointer producer
/// ([`crate::picking::pointer_click_emits_on_press`]) and the inbound router's
/// `Action::Click` honor emit, so all three modalities converge on one route and
/// the single `OnPress` consumer advances the widget's state once per activation.
fn activation_keys(role: A11yRole) -> Option<&'static [KeyCode]> {
    match role {
        // APG button: both Enter and Space.
        A11yRole::Button => Some(&[KeyCode::Enter, KeyCode::Space]),
        // APG checkbox: Space ONLY (Enter is inert — the canonical asymmetry).
        A11yRole::Checkbox => Some(&[KeyCode::Space]),
        // APG switch: Space and Enter both toggle.
        A11yRole::Switch => Some(&[KeyCode::Enter, KeyCode::Space]),
        _ => None,
    }
}

/// Keyboard activation for the focused widget (co-drive SC-1, WAI-ARIA APG): on a
/// **KeyDown** of one of the role's `activation_keys`, write the shared
/// [`OnPress`] sink — the SAME message the pointer path
/// ([`crate::picking::pointer_click_emits_on_press`]) and the inbound router's
/// `Action::Click` honor emit, so all three modalities converge on one route.
///
/// The per-role keymap encodes the **APG asymmetry** the gate-#7 fixture asserts:
/// a Button activates on Enter *or* Space; a **Checkbox toggles on Space only**
/// (Enter does nothing — the canonical asymmetry); a Switch toggles on *both*.
/// A key while a non-activatable role (or nothing) is focused writes nothing.
///
/// `FocusedEntity` is owned by `FocusPlugin`; under a harness without it the
/// `Option<Res<FocusedEntity>>` param leaves the system inert (no focus ⇒ no
/// activation). `events` is `Option<MessageReader<KeyboardInput>>` (the
/// `apply_keyboard_edits` precedent, input.rs): a `MinimalPlugins`/`A11yPlugin`
/// harness with no `InputPlugin` and no manual `add_message::<KeyboardInput>()`
/// has no `Messages<KeyboardInput>` resource, so the param must be optional or
/// the system fails param validation. A disabled widget (the a11y router gates
/// `A11yDisabled` inbound) is out of this path's scope — the keyboard path
/// mirrors the pointer producer, which likewise keys only on the activation role.
pub fn keyboard_activation(
    events: Option<MessageReader<KeyboardInput>>,
    focused: Option<Res<FocusedEntity>>,
    roles: Query<&A11yRole>,
    mut writer: MessageWriter<OnPress>,
) {
    // No keyboard infra (no `Messages<KeyboardInput>`) ⇒ nothing to read, inert.
    let Some(mut events) = events else {
        return;
    };
    // No focus resource (partial harness) or nothing focused ⇒ inert. Drain the
    // reader regardless so events don't pile up across frames.
    let Some(focused_entity) = focused.and_then(|f| f.0) else {
        events.clear();
        return;
    };
    // The focused widget's role decides WHICH keys activate it (the APG keymap).
    // A non-activatable role yields `None` ⇒ inert (drain so events don't pile up).
    let Some(role) = roles.get(focused_entity).ok().copied() else {
        events.clear();
        return;
    };
    let Some(keys) = activation_keys(role) else {
        events.clear();
        return;
    };
    // Activate on a key-DOWN of one of the role's activation keys. Releases and
    // every other key are ignored — so a checkbox's Enter (not in its key set)
    // is correctly inert.
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if keys.contains(&ev.key_code) {
            writer.write(OnPress(focused_entity));
        }
    }
}
