//! The **stateful-leaf** MVU tier (spec §3): the drain as the SOLE writer of a leaf's
//! single-source-of-truth state component.
//!
//! Design + rationale: `docs/specs/2026-06-29-mvu-as-core-design.md` (§3 "Tiered
//! granularity" — the tier table's "shared role-keyed reducer, no per-widget Model struct";
//! single writer through one drain).
//!
//! ## What the leaf tier is
//! Checkbox/Switch (and, later, Disclosure/ScrollArea) already own exactly one tiny
//! single-source-of-truth ARIA-state component ([`A11yToggled`](crate::a11y::A11yToggled)) that a `Changed`-gated
//! visual reads. The defect this tier closes is **multiple writers** of that
//! component (the flicker class; the gallery's direct `A11yToggled::True` write
//! racing `advance_toggle_on_press`). The cure is "single writer": every activation
//! **enqueues** a [`ToggleMsg`]; the one shared reducer folds it; the drain commits via
//! `set_if_neq` as the sole writer. The flicker cannot occur because the visual only ever
//! observes a folded value.
//!
//! ## "No per-widget `Model` struct"
//! There is NO per-checkbox / per-switch `Model` type. Both widget kinds reuse the existing
//! [`A11yToggled`](crate::a11y::A11yToggled) component as the model and share **one** reducer ([`toggle_reducer`]).
//! Per the per-TYPE transport, 5000 checkboxes share one inbox + one
//! drain. The role-gating (only Checkbox/Switch toggle on press) stays on the **enqueue**
//! side (the activation router), NOT in the reducer — the reducer is pure value-folding and
//! is shared *because* `advance_checkbox` and `toggle_switch` have an identical body.

use accesskit::Toggled;
use bevy::prelude::*;

use super::{Cmd, Model, MvuAppExt};
use crate::BuiySet;
use crate::a11y::A11yToggled;

/// The toggle leaf's **early, activation-stage** scheduling window (the early-window model, spec §4).
///
/// Unlike the generic late [`MvuSet::Drain`](super::MvuSet::Drain) — correct for the machine
/// tier, whose model feeds a *later* bind — the toggle leaf must fold BEFORE the a11y tree is
/// built, so an AT-driver `click` (which writes `OnPress` no later than [`BuiySet::Picking`]) is
/// reflected in the tree the SAME frame. The late drain would lag it one frame.
///
/// These sub-sets are chained `Enqueue → Drain` and pinned
/// `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` by [`register_toggle_leaf`], with a
/// pinned `ApplyDeferred` between them so a `commands`-deferred enqueue flushes before the drain
/// reads the inbox — the whole click→OnPress→enqueue→fold completes before `A11yUpdate`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToggleLeafSet {
    /// The enqueue-only edge: the activation router (`advance_toggle_on_press`) routes
    /// `OnPress → ToggleMsg`. Runs after `BuiySet::Picking`, where every `OnPress` producer
    /// (AT router + keyboard, in `BuiySet::Input`; the pointer `Pointer<Click>` observer, at
    /// `Picking`) has already written.
    Enqueue,
    /// The early ordered drain: the shared [`toggle_reducer`] folds + `set_if_neq` commits, the
    /// SOLE writer of [`A11yToggled`] — before `BuiySet::A11yUpdate` builds the tree.
    Drain,
}

/// The leaf tier reuses the existing single-source-of-truth component AS the model — no
/// per-widget `Model` struct (D2). [`A11yToggled`] already derives everything the `Model`
/// bound needs (`Component<Mutability = Mutable> + Reflect + GetTypeRegistration + Clone +
/// PartialEq`); this impl only names its [`ToggleMsg`] inbox.
impl Model for A11yToggled {
    type Msg = ToggleMsg;
}

/// Messages that fold a toggle leaf's [`A11yToggled`] state.
///
/// `Set` carries a `bool` (not `accesskit::Toggled`, which is a foreign non-`Reflect`
/// enum — the reason [`A11yToggled`] is registered `#[reflect(opaque)]`); `true ⇒ True`,
/// `false ⇒ False`. `Mixed` is reachable only as a *seeded/authored at-rest* value, never
/// a message target, so the `Msg` surface stays `Reflect`-clean for the record log.
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum ToggleMsg {
    /// Advance one activation step. The shared checkbox/switch contract:
    /// `False → True`, and `{True, Mixed} → False` (the identical body of both
    /// [`A11yToggled::advance_checkbox`] and [`A11yToggled::toggle_switch`]).
    Toggle,
    /// Set the absolute toggled value — the controlled-parent / seed write path.
    /// A redundant `Set(current)` is an idempotent fold: `set_if_neq` makes it a no-op,
    /// so it cannot trip `Changed<A11yToggled>` and cannot flicker.
    Set(bool),
}

/// The ONE shared toggle-leaf reducer (D2 "shared role-keyed reducer, no per-widget Model").
///
/// Env-free and pure: it folds [`ToggleMsg`] into the new [`A11yToggled`] value and returns
/// no effect. Shared across **all** checkboxes and switches — which is sound precisely
/// because `advance_checkbox` and `toggle_switch` fold identically; if they ever diverged,
/// the reducer would need the role in its env (or two `Msg` variants). The drain commits the
/// result via `set_if_neq`, making this the SOLE writer.
pub fn toggle_reducer(state: &mut A11yToggled, msg: ToggleMsg) -> Cmd<ToggleMsg> {
    match msg {
        // `False → True`, `{True, Mixed} → False` — kept in lock-step with
        // `A11yToggled::{advance_checkbox, toggle_switch}` (states.rs), whose bodies are
        // identical. (APG: activating a mixed checkbox sets it unchecked.)
        ToggleMsg::Toggle => {
            state.0 = match state.0 {
                Toggled::False => Toggled::True,
                Toggled::True | Toggled::Mixed => Toggled::False,
            };
        }
        ToggleMsg::Set(on) => {
            state.0 = if on { Toggled::True } else { Toggled::False };
        }
    }
    Cmd::none()
}

/// Wire the toggle-leaf model: register the [`A11yToggled`] inbox + [`ToggleMsg`] reflect
/// registration + the bind-counter, and install the ONE shared [`toggle_reducer`] drain in the
/// **early** [`ToggleLeafSet::Drain`] window (the early-window model, spec §4).
///
/// `A11yToggled` itself is already `register_type`'d by the a11y plugin, so this does NOT
/// re-register it (it uses `add_model` + `add_reducer_in_set`, not the `mvu_model` inference
/// path that would `register_type::<M>()` again). Call once from `WidgetsPlugin`, after the MVU
/// chain exists ([`super::MvuCorePlugin`]).
///
/// **The early window (spec §4).** The leaf's drain is installed in
/// [`ToggleLeafSet::Drain`], NOT the generic late [`MvuSet::Drain`](super::MvuSet::Drain). The
/// `Enqueue → ApplyDeferred → Drain` triple is pinned
/// `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)`, so the whole
/// click→`OnPress`→enqueue→fold completes BEFORE `build_tree` (in `BuiySet::A11yUpdate`) reads
/// `A11yToggled` — the toggle is reflected in the a11y tree the SAME frame. (The late
/// `MvuSet::Drain` runs after `A11yUpdate`, lagging the tree one frame: the staleness this
/// fixes.) The activation enqueue (`buiy_widgets::advance_toggle_on_press`) sits in
/// [`ToggleLeafSet::Enqueue`]; the C4 visuals run `.after(ToggleLeafSet::Drain)`.
pub fn register_toggle_leaf(app: &mut App) {
    app.add_model::<A11yToggled>();

    // The early activation-stage window: fold the leaf BEFORE `A11yUpdate` builds the tree.
    app.configure_sets(
        Update,
        (ToggleLeafSet::Enqueue, ToggleLeafSet::Drain)
            .chain()
            .after(BuiySet::Picking)
            .before(BuiySet::A11yUpdate),
    );
    // Flush the enqueue's deferred `commands.queue` writes into `Messages<Envelope<A11yToggled>>`
    // BEFORE the drain reads the inbox, so an enqueue from `ToggleLeafSet::Enqueue` is drained in
    // the SAME frame (mirrors the `MvuCorePlugin` sync point for the late chain).
    app.add_systems(
        Update,
        ApplyDeferred
            .after(ToggleLeafSet::Enqueue)
            .before(ToggleLeafSet::Drain),
    );
    app.add_reducer_in_set::<A11yToggled, _>(toggle_reducer, ToggleLeafSet::Drain);
}
