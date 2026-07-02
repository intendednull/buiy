//! Interaction primitives shared across producers of widget activation.
//!
//! `OnPress` is the canonical **activation sink** (co-drive contract SC-1): a
//! single `Message` that every activation producer writes and widget logic
//! reads. It lives in `buiy_core` — not `buiy_widgets` — because more than one
//! producer outside the widget crate writes it:
//!
//! - the C3 pointer layer (pointer `Click` → `OnPress`), and
//! - the P1c action router (`Action::Click` → `OnPress`, plus Button
//!   Enter/Space → `OnPress`).
//!
//! The P1c router lives in `buiy_core` and **cannot** depend on `buiy_widgets`
//! (the dependency edge runs `buiy_widgets` → `buiy_core` only), so the sink
//! must live here. `buiy_widgets` re-exports `OnPress` for source-compat, so
//! existing consumers (`buiy_widgets::OnPress`, the `buiy` prelude) are
//! unchanged. There is deliberately **no** competing `Activate` event — a
//! second sink would fork the activation grounding loop (co-drive § 6).
//!
//! Bevy split buffered events into `Message`. `OnPress` is therefore a
//! `Message` (not an `Event` — `Event` is reserved for observer-style
//! triggers): it lives in `Messages<OnPress>`, written with `MessageWriter`
//! and read with `MessageReader` / a cursor.

use bevy::prelude::*;

/// Widget-activation sink. Carries the activated entity. Producers
/// (pointer, keyboard, AT router) `write` it; widget logic reads it
/// (a Button fires its callback, a Checkbox advances `A11yToggled`, …).
#[derive(Message, Debug, Clone, Copy)]
pub struct OnPress(pub Entity);

/// A **typed value-change notification** (Track C / F2): the outbound event a
/// widget emits when its value has changed — a `ValueChange<bool>` for a checkbox
/// or switch, a `ValueChange<f64>` for a slider — so an app author reads a typed
/// value instead of demuxing the one untyped [`OnPress`] sink.
///
/// This is a **notification of a committed change**, not a request to change:
/// the widget crate emits it from a post-commit `Changed<state>` system (after
/// the MVU funnel's single ordered writer has applied the transition), so it
/// never competes with that writer. It complements `OnPress` (which stays the
/// activation sink); it does not replace the MVU message vocabularies
/// (`ToggleMsg`/`MenuMsg`) a widget uses *internally*.
///
/// Like [`OnPress`] it is a buffered [`Message`] (read with `MessageReader`), and
/// like `OnPress` the type lives in `buiy_core` while its concrete registrations
/// + emitters live in `buiy_widgets` (which owns the value widgets).
///
/// - `source` — the widget entity whose value changed.
/// - `value` — the new, committed value (`bool` / `f64`).
/// - `is_final` — `true` when the value has settled (the only case today: every
///   value change is a discrete commit). Reserved for continuous input: when a
///   slider gains pointer-drag, drag steps will report `false` and the release
///   `true`, so a consumer can throttle expensive work to the final value.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct ValueChange<T: Send + Sync + 'static> {
    /// The widget entity whose value changed.
    pub source: Entity,
    /// The new, committed value.
    pub value: T,
    /// Whether this is the settled/final value (see the type docs).
    pub is_final: bool,
}

/// Registers the shared interaction primitives. Composed into `CorePlugin` so
/// `Messages<OnPress>` exists for `buiy_core` consumers (the P1c router, the
/// C3 pointer layer) regardless of whether `buiy_widgets` is present.
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        // `OnPress` is a `Message`, so it lives in `Messages<OnPress>` and is
        // read with `MessageReader` / a cursor.
        app.add_message::<OnPress>();
    }
}
