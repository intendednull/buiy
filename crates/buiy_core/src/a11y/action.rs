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
use crate::a11y::relations::A11yRelations;
use crate::a11y::states::{A11yDisabled, A11yExpanded, A11yReadOnly, A11yTooltipHost, A11yValue};
use crate::a11y::translate::{entity_for_node_id, node_id_for};
use crate::a11y::{A11yRole, contract_for};
use crate::focus::{FocusVisible, FocusedEntity};
use crate::interaction::OnPress;
use crate::render::components::CssVisibility;
use accesskit::{Action, ActionData, ActionRequest, TreeId};
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

/// Whether `action` is an **expand/collapse** verb — the state-keyed capability a
/// node advertises by carrying [`A11yExpanded`] (widget-contracts.md §5
/// "Disclosure-trigger"), honored **generically** here rather than through a role
/// contract. Expandability is modelled as a reusable state-driven capability
/// layered on the role contract (a Disclosure-trigger is `Role::Button`, so its
/// `Click` rides the Button contract; `{Expand, Collapse}` ride `A11yExpanded`),
/// so any future expandable (a tree item, an accordion section) reuses the same
/// route with no new role. These are **actionable** verbs (unlike `Focus`/`Blur`):
/// the disabled live filter drops them.
fn is_expand_verb(action: Action) -> bool {
    matches!(action, Action::Expand | Action::Collapse)
}

/// Whether `action` is a **show/hide tooltip** verb — the state-keyed capability a
/// node advertises by carrying [`A11yTooltipHost`] (widget-contracts.md §5
/// "Tooltip-trigger"), honored **generically** here rather than through a role
/// contract. A tooltip trigger keeps its existing role (a button/icon/input that
/// happens to host a tooltip), so `{ShowTooltip, HideTooltip}` ride the marker —
/// not the role — exactly like `{Expand, Collapse}` ride [`A11yExpanded`]. These
/// are **actionable** verbs (unlike `Focus`/`Blur`): the disabled live filter
/// drops them. They are **non-mutating** of value/text, so the read-only filter
/// leaves them (showing/hiding a tooltip is not a value edit).
fn is_tooltip_verb(action: Action) -> bool {
    matches!(action, Action::ShowTooltip | Action::HideTooltip)
}

/// Show/hide the tooltip node(s) a trigger `described_by`-references, by writing
/// each tooltip's [`CssVisibility`] to `want` (the generic `ShowTooltip`/
/// `HideTooltip` honor, widget-contracts.md §5 "Tooltip-trigger").
///
/// The trigger's [`A11yRelations::described_by`] edge is the source of truth for
/// "which node is this trigger's tooltip" — the same edge the outbound fold
/// publishes as `aria-describedby`, so the AT/agent reads the relationship and
/// drives the show/hide over it. A trigger with no `described_by` edge (a
/// malformed tooltip-trigger) is a graceful no-op (nothing to show), never a
/// panic. The write is gated on a real `CssVisibility` transition so a repaint
/// (the `Changed<CssVisibility>` render-prep path) fires at most once per change.
///
/// **Sink-resource discipline** (action-router.md §6): a referenced node that has
/// despawned, or carries no `CssVisibility`, is skipped — the honor still
/// succeeds (the absolute set-verb is a no-op on an absent target). The minimal
/// `CssVisibility` show/hide is this slice's scope; the placement/positioning +
/// hover/focus auto-show timing geometry is C5 (Wave 4).
fn set_described_tooltips_visibility(world: &mut World, trigger: Entity, want: CssVisibility) {
    // Copy the `described_by` targets out so the `&A11yRelations` borrow ends
    // before we mutate the tooltip nodes' `CssVisibility`.
    let Some(targets) = world
        .get::<A11yRelations>(trigger)
        .map(|r| r.described_by.clone())
    else {
        return; // No relations component ⇒ no tooltip edge ⇒ nothing to show/hide.
    };
    for tooltip in targets {
        if let Some(mut vis) = world.get_mut::<CssVisibility>(tooltip) {
            // Only write through `DerefMut` on a real transition (idempotency): an
            // absolute set-verb to the current state is a no-op, so no spurious
            // `Changed<CssVisibility>` tick / repaint.
            if *vis != want {
                *vis = want;
            }
        }
    }
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
///    set. A verb the role never advertises is [`ActionError::Unsupported`] —
///    *unless* it is an `Expand`/`Collapse` and the node carries [`A11yExpanded`],
///    or a `ShowTooltip`/`HideTooltip` and the node carries [`A11yTooltipHost`]
///    (the two state-keyed capabilities layered on the role, widget-contracts.md
///    §5). `Focus`/`Blur` are always permitted on any live node (they ride
///    `Focusable` outbound and are honored generically here), so they bypass the
///    contract check.
/// 3. **Live state.** An [`A11yDisabled`] instance drops every actionable verb
///    ([`NotActionableReason::Disabled`]) — including `Expand`/`Collapse` and
///    `ShowTooltip`/`HideTooltip`; `Focus`/`Blur` stay allowed so the node remains
///    addressable. A mutating verb (`SetValue`/`Increment`/… —
///    `mutates_value_or_text`) on an [`A11yReadOnly`] instance is
///    [`NotActionableReason::ReadOnly`] (the tooltip verbs are non-mutating, so
///    the read-only filter leaves them).
/// 4. **Dispatch.** `Focus`/`Blur` set/clear [`FocusedEntity`] generically;
///    `Expand`/`Collapse` set/clear [`A11yExpanded`] generically (the absolute
///    set-verb, idempotent at the target state); `ShowTooltip`/`HideTooltip`
///    show/hide the trigger's [`described_by`](A11yRelations) tooltip node's
///    [`CssVisibility`] generically; every other verb is lowered through the
///    role's [`A11yContract::honor`](super::A11yContract).
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

    // Expand/Collapse are the **state-keyed** capability (widget-contracts.md §5):
    // advertised + honored for any node carrying `A11yExpanded`, layered on the
    // role contract rather than belonging to it. They are honored generically
    // below (set/clear the bool), but — unlike Focus/Blur — they are *actionable*,
    // so the disabled live filter drops them.
    let expandable = world.get::<A11yExpanded>(entity).is_some();

    // ShowTooltip/HideTooltip are the second state-keyed capability
    // (widget-contracts.md §5 "Tooltip-trigger"): advertised + honored for any node
    // carrying `A11yTooltipHost`, layered on the role rather than belonging to it.
    // Honored generically below (show/hide the `described_by` tooltip node), and —
    // like Expand/Collapse — actionable (the disabled filter drops them).
    let tooltip_host = world.get::<A11yTooltipHost>(entity).is_some();

    // The live role drives the capability re-check AND the honor dispatch. Read
    // it once; a node with no `A11yRole` defaults to `Generic` (no contract).
    let role = world.get::<A11yRole>(entity).copied().unwrap_or_default();

    // 2. Capability — the verb must be advertised, by EITHER the role's contract
    //    (the same `actions()` list the outbound fold advertises) OR — for an
    //    Expand/Collapse — the node's `A11yExpanded` state-keyed capability. Focus/
    //    Blur are always allowed. An Expand/Collapse on a non-`A11yExpanded` node
    //    is Unsupported (it advertised no such capability), exactly as the fold
    //    would not have emitted the `add_action`.
    if !is_focus_verb {
        let advertised_by_role =
            contract_for(role).is_some_and(|entry| entry.actions.contains(&action));
        let advertised_by_state =
            (is_expand_verb(action) && expandable) || (is_tooltip_verb(action) && tooltip_host);
        if !advertised_by_role && !advertised_by_state {
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
        // Expand/Collapse are honored GENERICALLY (state-keyed, widget-contracts.md
        // §5): set/clear the node's `A11yExpanded` bool directly, exactly as a
        // pointer/keyboard activation does through the `OnPress` consumer. The
        // capability gate above already guaranteed the node carries `A11yExpanded`
        // (an Expand on a non-expandable node returned `Unsupported`), but stay
        // total — a missing component here is `Unsupported`, never a panic. Setting
        // an already-expanded node expanded (or collapsing an already-collapsed
        // one) is an **idempotent no-op success**: the AT/agent set-verb is
        // absolute, not a toggle.
        Action::Expand | Action::Collapse => {
            let want = action == Action::Expand;
            let Some(mut expanded) = world.get_mut::<A11yExpanded>(entity) else {
                return Err(ActionError::Unsupported { target, action });
            };
            // Avoid a spurious `Changed<A11yExpanded>` tick when already at `want`
            // (idempotency): only write through `DerefMut` on a real transition, so
            // the C4 caret/panel visual repaints exactly once per actual change.
            if expanded.0 != want {
                expanded.0 = want;
            }
            Ok(())
        }
        // ShowTooltip/HideTooltip are honored GENERICALLY (state-keyed,
        // widget-contracts.md §5 "Tooltip-trigger"): show/hide the trigger's
        // `described_by` tooltip node by writing its `CssVisibility`
        // (`Visible`/`Hidden`). The capability gate above already guaranteed the
        // node carries `A11yTooltipHost`, but stay total — a missing marker here is
        // `Unsupported`, never a panic. The minimal show/hide IS the slice's scope;
        // the placement/positioning + hover/focus auto-show timing geometry is C5
        // (Wave 4). Setting an already-visible tooltip visible (or hiding an
        // already-hidden one) is an **idempotent no-op success** (the set-verb is
        // absolute, not a toggle), and the `Changed<CssVisibility>` write is gated
        // on a real transition so a repaint fires at most once per actual change.
        Action::ShowTooltip | Action::HideTooltip => {
            if world.get::<A11yTooltipHost>(entity).is_none() {
                return Err(ActionError::Unsupported { target, action });
            }
            let want = if action == Action::ShowTooltip {
                CssVisibility::Visible
            } else {
                CssVisibility::Hidden
            };
            set_described_tooltips_visibility(world, entity, want);
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

/// The APG **slider** keyboard intent for one key (widget-contracts.md §5
/// "Slider"): the value-changing verb a key requests on a focused slider. This
/// is the value-changing counterpart of [`activation_keys`] — a slider's keys do
/// **not** activate (no `OnPress`); they change the value via the same router
/// `honor` path an AT `Increment`/`Decrement`/`SetValue` lowers into. `None` ⇒
/// the key has no slider meaning.
///
/// The APG slider contract (both axes accepted per APG — orientation may *prefer*
/// one axis, but accepting both is conformant and simplest):
///
/// - `ArrowRight` / `ArrowUp` → [`Action::Increment`] (one step up).
/// - `ArrowLeft` / `ArrowDown` → [`Action::Decrement`] (one step down).
/// - `Home` → [`Action::SetValue`] to the slider's `min`.
/// - `End` → [`Action::SetValue`] to the slider's `max`.
/// - `PageUp` → one large-step up; `PageDown` → one large-step down.
///
/// `Home`/`End` and the `PageUp`/`PageDown` page steps need the live `min`/`max`
/// (`Home`/`End`) and `jump` (the page step) — those are read from the focused
/// slider's [`A11yValue`] at the call site, so this fn returns a small intent enum
/// the caller resolves against the live value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SliderKey {
    /// `Increment`/`Decrement` — one regular step (the sign is the verb).
    Increment,
    Decrement,
    /// One large "page" step (PageUp/PageDown) — resolved to `now ± jump`.
    PageUp,
    PageDown,
    /// Jump to `min` (Home) / `max` (End) via `SetValue`.
    Home,
    End,
}

/// Map a key to its [`SliderKey`] intent, or `None` for a key with no slider
/// meaning (which leaves the slider inert, the keyboard event untouched).
fn slider_key(key: KeyCode) -> Option<SliderKey> {
    match key {
        KeyCode::ArrowRight | KeyCode::ArrowUp => Some(SliderKey::Increment),
        KeyCode::ArrowLeft | KeyCode::ArrowDown => Some(SliderKey::Decrement),
        KeyCode::PageUp => Some(SliderKey::PageUp),
        KeyCode::PageDown => Some(SliderKey::PageDown),
        KeyCode::Home => Some(SliderKey::Home),
        KeyCode::End => Some(SliderKey::End),
        _ => None,
    }
}

/// Lower a [`SliderKey`] intent to the concrete router `(action, data)` for a
/// slider whose live value is `value`. `Home`/`End` and the page steps resolve
/// against the live bounds/jump here, then dispatch as the SAME
/// `Increment`/`Decrement`/`SetValue` verbs an AT lowers into the slider
/// contract's `honor` — one route, both modalities. `Increment`/`Decrement` carry
/// no data (the contract steps by the slider's own `step`); `PageUp`/`PageDown`
/// and `Home`/`End` resolve to an absolute `SetValue(NumericValue)` so the page
/// step / bound jump applies even when the contract's per-arrow step differs.
fn slider_action(intent: SliderKey, value: &A11yValue) -> (Action, Option<ActionData>) {
    match intent {
        SliderKey::Increment => (Action::Increment, None),
        SliderKey::Decrement => (Action::Decrement, None),
        // Page + bound jumps resolve against the LIVE value here and dispatch as
        // an absolute `SetValue` (the contract clamps it into `[min, max]`).
        SliderKey::PageUp => {
            let mut next = value.clone();
            next.page_increment();
            (Action::SetValue, Some(ActionData::NumericValue(next.now)))
        }
        SliderKey::PageDown => {
            let mut next = value.clone();
            next.page_decrement();
            (Action::SetValue, Some(ActionData::NumericValue(next.now)))
        }
        SliderKey::Home => (Action::SetValue, Some(ActionData::NumericValue(value.min))),
        SliderKey::End => (Action::SetValue, Some(ActionData::NumericValue(value.max))),
    }
}

/// APG **slider** keyboard control (widget-contracts.md §5): on a KeyDown of an
/// arrow / Home / End / PageUp / PageDown while a `Slider` is focused, change the
/// slider's value by dispatching the matching `Increment`/`Decrement`/`SetValue`
/// verb through [`dispatch_action_request`] — the SAME inbound seam an AT drives,
/// so the keyboard and the agent converge on the slider contract's `honor`
/// (which mutates the live [`A11yValue`]). A slider's keys are **value** actions,
/// NOT activation: this path never writes `OnPress` (the activation keymap
/// `activation_keys` returns `None` for `Slider`, so [`keyboard_activation`]
/// is correctly inert for it).
///
/// An **exclusive** system (`&mut World`) because the dispatch seam takes
/// `&mut World` to lower a value mutation — the same shape as
/// [`route_action_requests`]. The focused-Slider gate is checked **first**, so a
/// non-slider focus touches no keyboard messages (it leaves them for
/// [`keyboard_activation`]'s reader); only once a Slider is confirmed focused are
/// the pending `KeyboardInput` events read out. Draining there is harmless to
/// `keyboard_activation` — a `Slider` is not in `activation_keys`, so that
/// sibling never activates it regardless of whether it sees the events.
///
/// Under a partial/headless harness with no `Messages<KeyboardInput>` or no
/// `FocusedEntity` resource (no `InputPlugin`/`FocusPlugin`), the system is inert
/// (it has no focus / reads nothing), matching [`keyboard_activation`]'s graceful
/// degradation. A disabled slider is gated inside `dispatch_action_request` (the
/// §3 live filter drops the actionable verb), so it is not re-checked here.
pub fn slider_keyboard(world: &mut World) {
    // The focused-Slider gate runs FIRST — before any message access — so a
    // non-slider focus leaves the keyboard message buffer untouched for
    // `keyboard_activation`'s reader. No focus resource / nothing focused ⇒ inert.
    let Some(focused) = world.get_resource::<FocusedEntity>().and_then(|f| f.0) else {
        return;
    };
    if world.get::<A11yRole>(focused).copied() != Some(A11yRole::Slider) {
        return;
    }

    // A Slider is focused: read out its KeyDown key codes. (`keyboard_activation`
    // is inert for `Slider`, so consuming the events here is safe.) Copy them out
    // so the message borrow ends before the `&mut World` dispatch.
    let keys_down: Vec<KeyCode> = {
        let Some(mut messages) =
            world.get_resource_mut::<bevy::ecs::message::Messages<KeyboardInput>>()
        else {
            return; // No keyboard infra (no `Messages<KeyboardInput>`) — inert.
        };
        messages
            .drain()
            .filter(|ev| ev.state == ButtonState::Pressed)
            .map(|ev| ev.key_code)
            .collect()
    };

    for key in keys_down {
        let Some(intent) = slider_key(key) else {
            continue;
        };
        // Resolve the intent against the LIVE value (bounds/jump), then dispatch
        // the value verb through the shared router seam. `clone()` the small
        // value to drop the borrow before the `&mut World` dispatch.
        let Some(value) = world.get::<A11yValue>(focused).cloned() else {
            continue;
        };
        let (action, data) = slider_action(intent, &value);
        let req = ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: node_id_for(focused),
            data,
        };
        if let Err(err) = dispatch_action_request(world, &req) {
            // A soft, typed failure (e.g. a disabled slider dropped at the §3
            // filter) — surface for diagnostics, never panic (mirrors
            // `route_action_requests`).
            warn!("slider keyboard action not honored: {err:?}");
        }
    }
}
