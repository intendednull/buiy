//! `A11yContract` — the per-role authoring surface that drives BOTH the
//! outbound AccessKit `add_action` advertisement AND (in P1c-b) the inbound
//! `Action` dispatch. ONE declaration per interactive role keeps accessibility
//! and agent-control from drifting (widget-contracts.md §§1–2, the "lockstep
//! keystone").
//!
//! This module is the **contract surface** (co-drive P1c-a): the trait, the
//! `ContractEntry`/`contract_for` static registry, the `ActionError` taxonomy,
//! and the Phase-1 contracts that exist pre-P1d (only [`Button`] today; the
//! other widget contracts land with their bundles in P1d / Wave 3). The
//! outbound [`to_accesskit_node`](super::translate::to_accesskit_node) fold now
//! derives its advertised verbs from `Focusable` (`{Focus, Blur}`) PLUS
//! [`contract_for`]`(role).actions`, replacing the old focusable-`Focus`
//! hardcode. `honor` is **defined but not yet called** — the inbound router
//! (`route_action_requests`, P1c-b) is the caller.

use crate::a11y::A11yRole;
use crate::interaction::OnPress;
use accesskit::{Action, ActionData, NodeId};
use bevy::ecs::world::World;
use bevy::prelude::Entity;

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
/// focusable, and nothing else. Only [`Button`] is wired in P1c-a (the only
/// widget that exists pre-P1d); the remaining role contracts land with their
/// bundles in P1d (co-drive §3 demand-pull).
pub fn contract_for(role: A11yRole) -> Option<ContractEntry> {
    match role {
        A11yRole::Button => Some(ContractEntry::of::<Button>()),
        _ => None,
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
            // sink (SC-1) — the same message the pointer path emits, which
            // widget logic (`emit_on_press_on_click`'s consumer) drains.
            Action::Click => {
                // Sink-resource discipline (action-router.md §6): the message
                // collection is owned by `InteractionPlugin`; under a partial
                // harness without it, no-op gracefully rather than panic.
                if let Some(mut messages) =
                    world.get_resource_mut::<bevy::ecs::message::Messages<OnPress>>()
                {
                    messages.write(OnPress(entity));
                }
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
