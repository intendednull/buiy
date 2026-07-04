//! The typed routers (spec §2 "Routers", DX-3).
//!
//! The reconciler stamps a value component onto each interactive entity —
//! [`PressAction<M>`] on buttons + checkboxes, [`InputAction<M>`] /
//! [`SubmitAction<M>`] on text inputs — and these library-generic routers lower
//! the widget events into the MVU funnel via the real [`enqueue`]:
//!
//! - [`route_presses`] — `OnPress` → `PressAction` (buttons **and** checkbox
//!   toggles, whose `on_toggle` resolved eagerly to a value).
//! - [`route_text_input`] — `TextChanged` → read the editor value → the bare-fn
//!   `on_input` → the resulting draft `Msg`.
//! - [`route_text_submit`] — `EditSubmitted` → the `on_submit` value.
//!
//! The app author writes NO routing system — that is the DX-3 win (the
//! `route_counter_press` / `route_add_submit` a hand-bind demo needs are exactly
//! what these delete).
//!
//! **Replay-safety by type (spec §2 "the replay-safety rule").** A handler
//! stores a `Msg` *value*, never a captured closure: a value cannot close over
//! a `Res` snapshot that would diverge on a fresh-process replay, and the
//! recorded thing is always the resulting `Msg` folded through the funnel.

use bevy::prelude::*;
use buiy_core::interaction::OnPress;
use buiy_core::mvu::{Model, enqueue};
use buiy_core::text::edit::{EditSubmitted, TextChanged, TextEditState};

use crate::element::{InputHandler, SubmitHandler};

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

/// A text-input's per-keystroke handler (`fn(new_value) -> Msg`) + its owning
/// model. A **bare fn pointer**, not a captured closure — `Copy`,
/// determinism-safe by type, so storing it on the entity is replay-clean (the
/// recorded thing is the *result* `Msg` folded onto the model, not the fn).
#[derive(Component)]
pub(crate) struct InputAction<M: Model> {
    pub(crate) handler: InputHandler<M::Msg>,
    pub(crate) model: Entity,
}

/// A text-input's submit (Enter) handler + its owning model. The handler is either a
/// static `Msg` value ([`Element::on_submit`](crate::element::Element::on_submit), like
/// [`PressAction`]) or a capturing fold of the submitted text
/// ([`Element::on_submit_with`](crate::element::Element::on_submit_with)) — both replay-safe
/// by the same rule as [`InputAction`].
#[derive(Component)]
pub(crate) struct SubmitAction<M: Model> {
    pub(crate) handler: SubmitHandler<M::Msg>,
    pub(crate) model: Entity,
}

/// The editor→MVU **input bridge**: `TextChanged(entity)` → read the editor's
/// live value → apply the entity's `on_input` fn → [`enqueue`] the resulting
/// `Msg`. The value flows out of the command-sourced editor and INTO the model
/// as a funneled (recorded) `Msg` — so the model's draft is a pure function of
/// the message log (which is what keeps whole-UI replay holding, spec §5).
pub(crate) fn route_text_input<M: Model>(
    mut changes: MessageReader<TextChanged>,
    actions: Query<(&InputAction<M>, &TextEditState)>,
    mut commands: Commands,
) {
    for TextChanged(e) in changes.read() {
        if let Ok((action, editor)) = actions.get(*e) {
            let msg = action.handler.call(editor.value());
            enqueue::<M>(&mut commands, action.model, msg);
        }
    }
}

/// The editor→MVU **submit bridge**: `EditSubmitted(entity)` (Enter on a
/// single-line input) → resolve the entity's `on_submit` handler against the
/// editor's live value → [`enqueue`] the resulting message onto its model. A
/// static `on_submit(msg)` ignores the value (unchanged behaviour); a capturing
/// `on_submit_with(f)` folds the submitted text in. The editor is `Option`al so a
/// static submit still routes even on the (impossible-in-practice) entity that
/// lacks a `TextEditState`, preserving the pre-F7 static path exactly.
pub(crate) fn route_text_submit<M: Model>(
    mut submits: MessageReader<EditSubmitted>,
    actions: Query<(&SubmitAction<M>, Option<&TextEditState>)>,
    mut commands: Commands,
) {
    for EditSubmitted(e) in submits.read() {
        if let Ok((action, editor)) = actions.get(*e) {
            let value = editor.map(|ed| ed.value()).unwrap_or_default();
            enqueue::<M>(&mut commands, action.model, action.handler.resolve(value));
        }
    }
}
