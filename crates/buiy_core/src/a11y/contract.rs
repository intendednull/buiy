//! `A11yContract` — the per-role authoring surface that drives BOTH the
//! outbound AccessKit `add_action` advertisement AND (in P1c-b) the inbound
//! `Action` dispatch. ONE declaration per interactive role keeps accessibility
//! and agent-control from drifting (widget-contracts.md §§1–2, the "lockstep
//! keystone").
//!
//! This module is the **contract surface** (co-drive P1c-a): the trait, the
//! `ContractEntry`/`contract_for` static registry, the `ActionError` taxonomy,
//! and the per-widget contracts. P1c-a wired [`Button`]; Wave-3 slice-1 (P1d)
//! added [`Checkbox`] + [`Switch`]; slice-2 adds [`Slider`] (the first
//! value-changing contract — its verbs mutate `A11yValue` directly, not the
//! `OnPress` activation sink); slice-4 adds the text-input contracts
//! ([`TextInputContract`] + [`MultilineTextInputContract`] — the role split is an
//! addressing distinction over one shared `{SetValue}` lowering through the
//! existing editor `SelectAll`+`Insert` channel) alongside their bundles; the
//! remaining widget contracts land with their bundles in later P1d slices. The
//! outbound [`to_accesskit_node`](super::translate::to_accesskit_node) fold now
//! derives its advertised verbs from `Focusable` (`{Focus, Blur}`) PLUS
//! [`contract_for`]`(role).actions`, replacing the old focusable-`Focus`
//! hardcode. `honor` is **defined but not yet called** — the inbound router
//! (`route_action_requests`, P1c-b) is the caller.

use crate::a11y::A11yRole;
use crate::a11y::states::A11yValue;
use crate::interaction::OnPress;
use crate::text::SharedFontSystem;
use crate::text::edit::{EditCommand, SingleLine, TextChanged, TextEditState};
use accesskit::{Action, ActionData, NodeId};
use bevy::ecs::message::Messages;
use bevy::ecs::world::World;
use bevy::prelude::{DetectChangesMut, Entity};

/// Why an advertised verb could not be honored on a specific instance this
/// frame (the [`ActionError::NotActionable`] payload). Distinct from
/// "the role never advertises this verb at all" ([`ActionError::Unsupported`]).
///
/// The router's **live per-instance filter** (action-router.md §3) produces
/// these: the role advertises the verb, but the live state of *this* entity
/// (disabled, read-only, …) forbids honoring it right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotActionableReason {
    /// The instance carries `A11yDisabled`, so every actionable verb is
    /// dropped (Focus stays allowed — the node remains addressable, §3).
    Disabled,
    /// A mutating verb (`SetValue`/`ReplaceSelectedText`/…) hit a read-only
    /// instance (`A11yReadOnly`). Selecting/copying is still allowed; mutation
    /// is forbidden (action-router.md §3).
    ReadOnly,
    /// A frame-loop wait exhausted its budget before the awaited semantic
    /// condition held (`inprocess::wait_for`, inprocess-api.md §5.2). Not a
    /// per-instance live-filter drop — a timeout of the in-process driver's
    /// condition poll, surfaced through the shared [`ActionError`] taxonomy.
    Timeout,
}

/// The typed outcome of an inbound action dispatch (action-router.md §3).
///
/// Every arm propagates **loudly** through the in-process API (and later MCP)
/// so an agent never silently no-ops. `honor` returns this instead of ever
/// panicking; `route_action_requests`/`dispatch_action_request` (P1c-b)
/// produce the `NotFound`/`Unsupported`/`NotActionable` arms from the liveness
/// and live-capability guard before `honor` is reached, and surface a `honor`
/// arm's own `BadData`/`NotActionable`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    /// The `target` `NodeId` resolves to no live entity this frame (a stale
    /// ref — the entity despawned between the AT's read and the request).
    NotFound { target: NodeId },
    /// The target's role never advertises `action` at all (it is not in the
    /// role's [`A11yContract::actions`]). Distinct from a live-filter drop.
    Unsupported { target: NodeId, action: Action },
    /// `action` is advertised by the role but the live state of *this*
    /// instance forbids honoring it this frame (see [`NotActionableReason`]).
    NotActionable {
        target: NodeId,
        action: Action,
        reason: NotActionableReason,
    },
    /// The `ActionData` payload was missing or the wrong variant for `action`
    /// (e.g. a `SetValue` with no `NumericValue`/`Value`).
    BadData { target: NodeId, action: Action },
}

/// The authoring surface every interactive widget role implements. ONE impl
/// per role; the impl is registered into the [`contract_for`] static table.
///
/// `actions()` is the single source of truth read in **both** directions
/// (widget-contracts.md §2): the outbound fold advertises it via `add_action`,
/// and the inbound router re-validates against it before calling `honor`. A
/// verb in `actions()` with no `honor` arm is an advertise-without-honor bug; a
/// `honor` arm for an un-advertised verb is dead code (the router re-reads the
/// advertised set first).
pub trait A11yContract: Send + Sync + 'static {
    /// The role this contract drives.
    fn role() -> A11yRole;

    /// Role-static advertised verbs. Emitted via `add_action`, re-validated
    /// inbound. Per-instance capability is the router's LIVE filter on top
    /// (action-router.md §3). Always includes the role-specific verbs; the
    /// implicit `{Focus, Blur}` come from `Focusable`, not from here, so a
    /// contract lists only its *additional* verbs.
    fn actions() -> &'static [Action];

    /// Lower one advertised verb into a real Buiy sink. Called by the inbound
    /// router (P1c-b) AFTER liveness + the live filter pass. Returns a typed
    /// [`ActionError`] on a data/state problem — **never panics**. `data` is
    /// the inbound `ActionData` payload (`None` for no-data verbs).
    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        data: Option<&ActionData>,
    ) -> Result<(), ActionError>;
}

/// A type-erased contract, as stored in the [`contract_for`] static table. The
/// registry keys on the role and yields the role-static advertised verb list
/// plus the `honor` lowering fn (widget-contracts.md §1). A static dispatch
/// table, not a per-entity handle: simpler, and no Phase-1 widget needs
/// same-role-different-honor (the open question deferred in phasing.md).
#[derive(Clone, Copy)]
pub struct ContractEntry {
    /// The role-static advertised verbs (`C::actions()`), read in both
    /// directions (advertise outbound, re-validate inbound).
    pub actions: &'static [Action],
    /// The role-static `honor` lowering fn (`C::honor`). Defined now; the
    /// inbound router calls it in P1c-b.
    pub honor: fn(&mut World, Entity, Action, Option<&ActionData>) -> Result<(), ActionError>,
}

impl ContractEntry {
    /// Build the entry from a contract type's static methods. Keeps the
    /// registry rows declarative — one `ContractEntry::of::<C>()` per role.
    fn of<C: A11yContract>() -> Self {
        Self {
            actions: C::actions(),
            honor: C::honor,
        }
    }
}

/// The role → contract static registry (widget-contracts.md §1). Consulted in
/// **both** directions: the outbound fold reads `.actions`; the inbound router
/// (P1c-b) resolves `NodeId → Entity → A11yRole → contract` and calls `.honor`.
///
/// A role with no interactive contract (a `Generic`/`Text`/`Region` container)
/// returns `None` — it advertises only the implicit `{Focus, Blur}` when
/// focusable, and nothing else. [`Button`] (P1c-a), [`Checkbox`], [`Switch`]
/// (Wave-3 slice-1), [`Slider`] (slice-2), and the text inputs
/// ([`TextInputContract`]/[`MultilineTextInputContract`], slice-4) are wired; the
/// remaining role contracts land with their bundles in later P1d slices (co-drive
/// §3 demand-pull).
pub fn contract_for(role: A11yRole) -> Option<ContractEntry> {
    match role {
        A11yRole::Button => Some(ContractEntry::of::<Button>()),
        A11yRole::Checkbox => Some(ContractEntry::of::<Checkbox>()),
        A11yRole::Switch => Some(ContractEntry::of::<Switch>()),
        A11yRole::Slider => Some(ContractEntry::of::<Slider>()),
        // The role split IS the multiline distinction: the single-line and
        // multi-line text inputs share ONE contract surface (same advertised
        // `{SetValue}`, same `SelectAll`+`Insert` lowering) but two roles, so
        // the AT/agent addressing vocabulary distinguishes them. The `honor`
        // reads the live `SingleLine` policy marker, so the single shared `honor`
        // applies the correct newline policy regardless of which role keyed it.
        A11yRole::TextInput => Some(ContractEntry::of::<TextInputContract>()),
        A11yRole::MultilineTextInput => Some(ContractEntry::of::<MultilineTextInputContract>()),
        _ => None,
    }
}

/// Write the shared [`OnPress`] activation sink (SC-1) for `entity`, if the sink
/// resource is present. Every `honor(Click)` lowers through this ONE message —
/// the same sink the pointer producer and the keyboard handlers write — so an
/// AT-driven `Click` converges with pointer/keyboard activation on the single
/// `OnPress` consumer (Button fires its callback; a Checkbox/Switch advances its
/// `A11yToggled`). Sink-resource discipline (action-router.md §6): under a
/// partial harness without [`InteractionPlugin`](crate::interaction::InteractionPlugin)
/// the resource is absent and this no-ops gracefully rather than panicking.
fn emit_on_press(world: &mut World, entity: Entity) {
    if let Some(mut messages) = world.get_resource_mut::<bevy::ecs::message::Messages<OnPress>>() {
        messages.write(OnPress(entity));
    }
}

/// The Button contract (widget-contracts.md §5 "Button"). Role `Button`; verbs
/// `{Click, Focus, Blur}` (the `{Focus, Blur}` are implicit via `Focusable`, so
/// [`actions`](A11yContract::actions) lists only `Click`). `honor(Click)` emits
/// the [`OnPress`] activation message — the SAME sink the pointer and keyboard
/// paths write (co-drive SC-1), so an AT-driven `Click` and a real pointer
/// click converge on one route.
///
/// A zero-sized marker, not a Bevy `Component`: the contract is keyed by role
/// in [`contract_for`], not attached per entity.
pub struct Button;

/// Button's role-static advertised verbs beyond the implicit `{Focus, Blur}`.
static BUTTON_ACTIONS: &[Action] = &[Action::Click];

impl A11yContract for Button {
    fn role() -> A11yRole {
        A11yRole::Button
    }

    fn actions() -> &'static [Action] {
        BUTTON_ACTIONS
    }

    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        _data: Option<&ActionData>,
    ) -> Result<(), ActionError> {
        match action {
            // `Click` activates the button by writing the shared `OnPress`
            // sink (SC-1) — the same message the pointer/keyboard paths emit,
            // which widget logic drains.
            Action::Click => {
                emit_on_press(world, entity);
                Ok(())
            }
            // Focus/Blur are honored generically by the router (set/clear
            // `FocusedEntity`), not here — they never reach a contract's
            // `honor`. Any other verb is not in Button's advertised set, so the
            // router rejects it at the §3 filter before `honor`; reaching here
            // is dead code, reported (not panicked) as `Unsupported`.
            _ => Err(ActionError::Unsupported {
                target: super::translate::node_id_for(entity),
                action,
            }),
        }
    }
}

/// The Checkbox contract (widget-contracts.md §5 "Checkbox"). Role `Checkbox`
/// (→ accesskit `Role::CheckBox`); verbs `{Click, Focus, Blur}` (the
/// `{Focus, Blur}` are implicit via `Focusable`, so [`actions`](A11yContract::actions)
/// lists only `Click`). State is the **tri-state** [`A11yToggled`](super::A11yToggled) — `Mixed`
/// (indeterminate) is a first-class checkbox value, never collapsed to a bool.
///
/// `honor(Click)` writes the shared [`OnPress`] sink (SC-1), exactly like
/// [`Button`]; the single `OnPress` consumer advances the checkbox's
/// `A11yToggled` (`False → True → False`, `Mixed → False`) via
/// [`A11yToggled::advance_checkbox`](super::A11yToggled::advance_checkbox). The
/// **APG keyboard asymmetry** — a checkbox toggles on **Space only**, NOT Enter
/// — is the keyboard layer's job (`keyboard_activation`, action-router.md), not
/// `honor`'s: every activation modality (pointer/Space/AT-`Click`) converges on
/// the one `OnPress` sink, so the advance happens once per activation regardless
/// of source.
///
/// A zero-sized marker, not a Bevy `Component`: keyed by role in [`contract_for`].
pub struct Checkbox;

/// Checkbox's role-static advertised verbs beyond the implicit `{Focus, Blur}`.
static CHECKBOX_ACTIONS: &[Action] = &[Action::Click];

impl A11yContract for Checkbox {
    fn role() -> A11yRole {
        A11yRole::Checkbox
    }

    fn actions() -> &'static [Action] {
        CHECKBOX_ACTIONS
    }

    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        _data: Option<&ActionData>,
    ) -> Result<(), ActionError> {
        match action {
            // Converge on the shared `OnPress` sink (SC-1); the single consumer
            // advances `A11yToggled` for the checkbox role. AT-`Click` therefore
            // toggles identically to a pointer click or a Space press.
            Action::Click => {
                emit_on_press(world, entity);
                Ok(())
            }
            _ => Err(ActionError::Unsupported {
                target: super::translate::node_id_for(entity),
                action,
            }),
        }
    }
}

/// The Switch contract (widget-contracts.md §5 "Switch"). Role `Switch`
/// (→ accesskit `Role::Switch`); verbs `{Click, Focus, Blur}` (`{Focus, Blur}`
/// implicit via `Focusable`). State is a **binary** [`A11yToggled`](super::A11yToggled) — a switch
/// has no `Mixed`.
///
/// `honor(Click)` writes the shared [`OnPress`] sink (SC-1); the single consumer
/// flips the switch's `A11yToggled` (`False ↔ True`) via
/// [`A11yToggled::toggle_switch`](super::A11yToggled::toggle_switch). Unlike the
/// checkbox, a switch toggles on **both Space and Enter** (the keyboard layer);
/// `honor` is modality-agnostic — it only feeds the one sink.
///
/// A zero-sized marker, not a Bevy `Component`: keyed by role in [`contract_for`].
pub struct Switch;

/// Switch's role-static advertised verbs beyond the implicit `{Focus, Blur}`.
static SWITCH_ACTIONS: &[Action] = &[Action::Click];

impl A11yContract for Switch {
    fn role() -> A11yRole {
        A11yRole::Switch
    }

    fn actions() -> &'static [Action] {
        SWITCH_ACTIONS
    }

    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        _data: Option<&ActionData>,
    ) -> Result<(), ActionError> {
        match action {
            Action::Click => {
                emit_on_press(world, entity);
                Ok(())
            }
            _ => Err(ActionError::Unsupported {
                target: super::translate::node_id_for(entity),
                action,
            }),
        }
    }
}

/// The Slider contract (widget-contracts.md §5 "Slider"). Role `Slider`
/// (→ accesskit `Role::Slider`); verbs `{Increment, Decrement, SetValue, Focus,
/// Blur}` (`{Focus, Blur}` implicit via `Focusable`, so
/// [`actions`](A11yContract::actions) lists `{Increment, Decrement, SetValue}`).
/// State is the valued range [`A11yValue`] (`now`/`min`/`max`, optional
/// `step`/`jump`/`text`) plus [`A11yOrientation`](super::A11yOrientation).
///
/// **Unlike the toggle widgets, a Slider does NOT lower through `OnPress`** — its
/// verbs change the *value*, so `honor` mutates the live `A11yValue` **directly**
/// (the component the outbound fold re-emits via `set_numeric_value`, so the
/// AT/agent observes the new value the same frame). The keyboard layer mirrors
/// this: the slider keymap dispatches `Increment`/`Decrement`/`SetValue` through
/// the router into THIS `honor`, never the `OnPress` activation sink.
///
/// - `Increment` → [`A11yValue::increment`] (`now = (now + step).min(max)`).
/// - `Decrement` → [`A11yValue::decrement`] (`now = (now − step).max(min)`).
/// - `SetValue` carrying [`ActionData::NumericValue`] → [`A11yValue::set_now`]
///   (clamped into `[min, max]`); a missing/wrong-variant payload is
///   [`ActionError::BadData`].
///
/// At-bounds is a **saturated no-op** (the clamp inside `A11yValue`), a success,
/// not an error — the router's live-filter §3 contract. A slider with no
/// `A11yValue` component (a contract error) reports [`ActionError::BadData`]
/// rather than panicking.
///
/// A zero-sized marker, not a Bevy `Component`: keyed by role in [`contract_for`].
pub struct Slider;

/// Slider's role-static advertised verbs beyond the implicit `{Focus, Blur}`.
static SLIDER_ACTIONS: &[Action] = &[Action::Increment, Action::Decrement, Action::SetValue];

impl A11yContract for Slider {
    fn role() -> A11yRole {
        A11yRole::Slider
    }

    fn actions() -> &'static [Action] {
        SLIDER_ACTIONS
    }

    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        data: Option<&ActionData>,
    ) -> Result<(), ActionError> {
        let target = super::translate::node_id_for(entity);
        // The value mutation funnels through the live `A11yValue` component, which
        // the outbound fold re-emits — so the AT/agent observes the new `now` the
        // same frame. A slider with no `A11yValue` is a contract error (the bundle
        // `#[require]`s it), reported as `BadData`, never a panic.
        let Some(mut value) = world.get_mut::<A11yValue>(entity) else {
            return Err(ActionError::BadData { target, action });
        };
        // Mutate a CLONE and commit via `set_if_neq`, so `Changed<A11yValue>` only
        // trips on a REAL change. The `A11yValue` mutators (`increment`/`set_now`)
        // clamp into `[min, max]`, so a verb that saturates (Right-arrow at the
        // maximum, `SetValue` to the current value) is a no-op — and must NOT trip
        // `Changed` (that would fire a phantom `ValueChange<f64>` and a redundant
        // thumb reposition). This mirrors the toggle leaf's `set_if_neq` drain, so
        // both value widgets are change-honest.
        match action {
            Action::Increment => {
                let mut next = value.clone();
                next.increment();
                value.set_if_neq(next);
                Ok(())
            }
            Action::Decrement => {
                let mut next = value.clone();
                next.decrement();
                value.set_if_neq(next);
                Ok(())
            }
            // `SetValue` carries the absolute target as a `NumericValue(f64)`; a
            // missing/wrong-variant payload is `BadData`. The set clamps into
            // `[min, max]` (an out-of-range request saturates, never errors).
            Action::SetValue => match data {
                Some(ActionData::NumericValue(v)) => {
                    let mut next = value.clone();
                    next.set_now(*v);
                    value.set_if_neq(next);
                    Ok(())
                }
                _ => Err(ActionError::BadData { target, action }),
            },
            // Any other verb is not in Slider's advertised set; the router rejects
            // it at the §3 filter before `honor`, so reaching here is dead code,
            // reported (not panicked) as `Unsupported`.
            _ => Err(ActionError::Unsupported { target, action }),
        }
    }
}

/// The advertised verbs shared by BOTH text-input roles, beyond the implicit
/// `{Focus, Blur}`: `{SetValue}`. The selection verbs (`SetTextSelection`,
/// `ReplaceSelectedText`) are **deferred** (co-drive §3.2 — no gallery editor
/// needs programmatic selection; the `EditCommand::SetSelection` slice they would
/// need is itself deferred), so they are neither advertised here nor honored.
static TEXT_INPUT_ACTIONS: &[Action] = &[Action::SetValue];

/// Lower a text `SetValue` into the editor through the **existing** `SelectAll` +
/// `Insert` channel (co-drive §3.1 / widget-contracts.md §5) — **no new
/// `EditCommand`**. Shared by both text-input contracts (the role split is only an
/// addressing distinction; the lowering is identical, and the `SingleLine` policy
/// is read live so the single-line newline strip applies to a single-line input
/// regardless of which role keyed the dispatch).
///
/// `SetValue` carries the replacement text as [`ActionData::Value`]; a
/// missing/wrong-variant payload is [`ActionError::BadData`]. The lowering:
///
/// 1. `EditCommand::SelectAll` — select the whole buffer.
/// 2. `EditCommand::Insert(text)` — replace the selection with `text` (a
///    single-line input strips embedded newlines, the `SingleLine` policy). When
///    `text` is **empty** an `Insert("")` is a no-op (cosmic's per-char insert
///    deletes the selection on the FIRST char, so an empty string never reaches
///    that delete), so the empty case lowers to `EditCommand::Delete` instead —
///    which deletes the whole selection (a clean clear). Both are existing verbs;
///    no new `EditCommand`.
///
/// All verbs go through `TextEditState::apply` (the facade boundary — `apply`
/// itself never names a cosmic type from outside `text::edit`), which records the
/// edit on the undo stack exactly as a keyboard edit would.
///
/// **Sink-resource discipline** (action-router.md §6): the lowering needs the
/// [`SharedFontSystem`] resource to apply an edit; under a partial harness without
/// `BuiyTextPlugin` it is absent, and an entity with no [`TextEditState`] is a
/// contract error — both are reported as [`ActionError::BadData`], never a panic.
/// The read-only live filter (`A11yReadOnly`) already dropped the verb in the
/// router before `honor` is reached, so this lowering is not re-guarded here.
fn honor_text_set_value(
    world: &mut World,
    entity: Entity,
    action: Action,
    data: Option<&ActionData>,
) -> Result<(), ActionError> {
    let target = super::translate::node_id_for(entity);
    // The payload must be a `Value(Box<str>)` — anything else (or none) is BadData.
    let Some(ActionData::Value(text)) = data else {
        return Err(ActionError::BadData { target, action });
    };
    let text = text.to_string();

    // The editor needs the FontSystem to apply an edit. Clone the cheap `Arc`
    // handle (and drop the resource borrow) so the subsequent `&mut TextEditState`
    // borrow does not alias the resource. Absent resource ⇒ no text infra ⇒
    // BadData (graceful, never a panic — the partial-harness discipline).
    let Some(fonts) = world.get_resource::<SharedFontSystem>().cloned() else {
        return Err(ActionError::BadData { target, action });
    };
    // The single-line policy marker drives the newline strip inside `Insert`.
    let single_line = world.get::<SingleLine>(entity).is_some();
    // The editor mechanism. An entity routed here with no `TextEditState` is a
    // contract error (the widget `#[require]`s it) — BadData, not a panic.
    let Some(mut state) = world.get_mut::<TextEditState>(entity) else {
        return Err(ActionError::BadData { target, action });
    };

    let mut font_system = fonts.lock();
    // SelectAll (non-mutating; seals the open undo run) selects the whole buffer.
    // `read_only = false`: the router's §3 live filter already dropped `SetValue`
    // on an `A11yReadOnly` instance, so by the time `honor` runs the field is
    // writable.
    state.apply(&mut font_system, EditCommand::SelectAll, single_line, false);
    // Replace the selection: a non-empty Insert deletes-then-inserts (cosmic's
    // per-char `insert_string` deletes the selection on the first char); an empty
    // target lowers to `Delete` (which deletes the whole selection) since an
    // `Insert("")` never reaches that delete — both are existing verbs.
    let replace = if text.is_empty() {
        EditCommand::Delete
    } else {
        EditCommand::Insert(text)
    };
    // The mutating verb carries the value-change signal (SelectAll is
    // non-mutating). `apply`'s `EditOutcome.value_changed` is `value() != before`
    // (input.rs) — false for a SetValue that replaces a value with the identical
    // string, so the emit below never fires on a no-op set.
    let value_changed = state
        .apply(&mut font_system, replace, single_line, false)
        .value_changed;
    drop(font_system);

    // Mirror the keyboard path (input.rs `apply_keyboard_edits`: after a
    // value-changing edit, `changed.write(TextChanged(entity))`). Without this the
    // host-facing bridges never see the programmatic set: `buiy_view`'s
    // `route_text_input` fires `on_input` ONLY on `TextChanged`, so an
    // assistive-tech `SetValue` (or `buiy::probe::set_value`) would update the
    // editor + a11y tree but never fold into the app's model. `TextChanged` is a
    // `Message`; from the `&mut World` dispatch context we write it onto the
    // resource the keyboard path's `MessageWriter` funnels into. Value-gated so a
    // no-op set (identical string) emits nothing — matching the keyboard path.
    // NOTE: the `Messages<TextChanged>` resource exists whenever `BuiyTextPlugin`
    // is installed (it registers the message); the SelectAll+Insert lowering above
    // already required `SharedFontSystem`, so any harness that reaches here has the
    // text infra and thus the message resource.
    if value_changed && let Some(mut changed) = world.get_resource_mut::<Messages<TextChanged>>() {
        changed.write(TextChanged(entity));
    }
    Ok(())
}

/// The single-line **TextInput** contract (widget-contracts.md §5 "TextInput").
/// Role `TextInput` (→ accesskit `Role::TextInput`); verbs `{SetValue, Focus,
/// Blur}` (`{Focus, Blur}` implicit via `Focusable`, so
/// [`actions`](A11yContract::actions) lists only `SetValue`). State is the live
/// [`A11yTextValue`](super::A11yTextValue) (synced from the editor's
/// `TextEditState`) + [`A11yPlaceholder`](super::A11yPlaceholder).
///
/// **Unlike the toggle widgets, it does NOT lower through `OnPress`** — `SetValue`
/// changes the *text*, so `honor` lowers into the editor through the existing
/// `SelectAll` + `Insert` channel (`honor_text_set_value`); the next frame's
/// `sync_text_input_a11y` reflects the new value into `A11yTextValue`, which the
/// outbound fold re-emits. The selection verbs (`SetTextSelection`,
/// `ReplaceSelectedText`) are deferred (co-drive §3.2).
///
/// A zero-sized marker, not a Bevy `Component`: keyed by role in [`contract_for`].
pub struct TextInputContract;

impl A11yContract for TextInputContract {
    fn role() -> A11yRole {
        A11yRole::TextInput
    }

    fn actions() -> &'static [Action] {
        TEXT_INPUT_ACTIONS
    }

    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        data: Option<&ActionData>,
    ) -> Result<(), ActionError> {
        match action {
            Action::SetValue => honor_text_set_value(world, entity, action, data),
            // Any other verb is not advertised; the router rejects it at the §3
            // filter before `honor`, so reaching here is dead code, reported (not
            // panicked) as `Unsupported`.
            _ => Err(ActionError::Unsupported {
                target: super::translate::node_id_for(entity),
                action,
            }),
        }
    }
}

/// The **MultilineTextInput** contract (widget-contracts.md §5 "TextInput", the
/// multi-line half of the role split). Role `MultilineTextInput` (→ accesskit
/// `Role::MultilineTextInput`); verbs `{SetValue, Focus, Blur}`. Identical
/// behavior to [`TextInputContract`] — the role split is only an addressing
/// distinction (so an AT/agent can tell a single-line field from a multi-line
/// one); the lowering is the SAME shared `honor_text_set_value` (a multi-line
/// input carries no `SingleLine` marker, so its `SetValue` keeps embedded
/// newlines). No multiline-specific editor behavior is built here (co-drive
/// ledger: the multiline-specific behavior is deferred; only the correct role
/// assignment completes the split).
///
/// A zero-sized marker, not a Bevy `Component`: keyed by role in [`contract_for`].
pub struct MultilineTextInputContract;

impl A11yContract for MultilineTextInputContract {
    fn role() -> A11yRole {
        A11yRole::MultilineTextInput
    }

    fn actions() -> &'static [Action] {
        TEXT_INPUT_ACTIONS
    }

    fn honor(
        world: &mut World,
        entity: Entity,
        action: Action,
        data: Option<&ActionData>,
    ) -> Result<(), ActionError> {
        match action {
            Action::SetValue => honor_text_set_value(world, entity, action, data),
            _ => Err(ActionError::Unsupported {
                target: super::translate::node_id_for(entity),
                action,
            }),
        }
    }
}
