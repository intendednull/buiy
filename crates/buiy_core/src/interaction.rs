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
