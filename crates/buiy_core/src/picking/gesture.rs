//! The widget-agnostic multi-click pointer gesture (input-event-model.md § 2.11,
//! co-drive § 6.9). bevy_picking 0.19 ships **no** `Pointer<DoubleClick>` event
//! — `Pointer<Click>` carries a `count`, but it uses bevy's own (non-tunable)
//! timing and applies no adjacency radius, so a widget keying off it would
//! disagree with the editor about what "a double-click" is. So C3 owns one
//! widget-agnostic [`MultiClick`] [`EntityEvent`], derived from the SAME
//! timing+radius heuristic the editor uses (the already-`pub`
//! [`crate::text::edit::ClickTracker`]), so there is exactly one definition of
//! "double-click" in Buiy: the editor's intra-text selection and a widget's
//! edit-in-place agree.
//!
//! **Derivation:** [`derive_multi_click`] observes the committed `Pointer<Click>`
//! stream, feeds each click's `(target, position, time)` through a per-system
//! `ClickTracker`, and emits `MultiClick { count }` on the click's target entity
//! when the run reaches `count >= 2`. A single click is plain `Pointer<Click>`;
//! `MultiClick` is only emitted for double/triple/… runs.
//!
//! **Owner / consumers:** C3 owns the gesture; C8's todo row observes
//! `MultiClick { count: 2 }` for edit-in-place (audit W17); the editor uses the
//! underlying `ClickTracker` run directly for intra-text selection and does not
//! round-trip through this event.

use crate::text::edit::{ClickTracker, PointerGesture};
use bevy::picking::events::{Click, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

/// A committed multi-click on an entity — the widget-agnostic double/triple-click
/// signal. Bubbles up the entity hierarchy like `Pointer<E>`. Emitted for ANY
/// picked entity (a todo row, a list item, a button), not only the editor, so a
/// non-editor widget can drive edit-in-place / expand / select-all gestures.
///
/// Derived from the SAME timing+radius heuristic the editor uses (the already-
/// public [`crate::text::edit::ClickTracker`]), so single-source multi-click
/// semantics: the editor's intra-text selection and a widget's edit-in-place
/// agree on what "a double-click" is.
#[derive(EntityEvent, Clone, Debug)]
#[entity_event(propagate, auto_propagate)]
pub struct MultiClick {
    /// The entity the gesture targets (the `EntityEvent` target).
    pub entity: Entity,
    /// Click run length: 2 = double, 3 = triple, … (a single click is plain
    /// `Pointer<Click>`; `MultiClick` is only emitted for count >= 2).
    pub count: u32,
    /// The pointer button that produced the run.
    pub button: PointerButton,
}

/// Observes the committed `Pointer<Click>` stream and emits [`MultiClick`] on the
/// click target when the [`ClickTracker`] heuristic classifies the run as a
/// double (or triple). Registered as an observer by [`PickingPlugin`].
///
/// The tracker state is a `Local`, so it persists across clicks within one
/// system instance — the same role the editor's `Local<ClickTracker>` plays.
/// Tracking from the click's window-space location (`pointer_location.position`)
/// keeps the adjacency-radius gate identical to the editor's, so the two never
/// disagree (§ 2.11 "the SAME ClickTracker heuristic, not bevy's untuned
/// Click.count").
pub fn derive_multi_click(
    click: On<Pointer<Click>>,
    time: Res<Time>,
    mut tracker: Local<ClickTracker>,
    mut commands: Commands,
) {
    // `Pointer<Click>` bubbles (capture→target→bubble), so a global observer
    // fires once per ancestor hop with `entity` rewritten to the current hop.
    // The gesture is about the COMMITTED click, classified ONCE — so act only at
    // the original (leaf) target. Without this gate the `ClickTracker` would see
    // N classify calls per click (one per ancestor), corrupting the streak.
    if click.entity != click.original_event_target() {
        return;
    }
    let target = click.original_event_target();
    let pos = click.pointer_location.position;
    let gesture = tracker.classify(pos, time.elapsed());
    let count = match gesture {
        PointerGesture::Click => return, // a single click is plain Pointer<Click>
        PointerGesture::DoubleClick => 2,
        PointerGesture::TripleClick => 3,
        // `classify` never returns `Drag` (that is a held-move gesture, not a
        // press classification); guard defensively.
        PointerGesture::Drag => return,
    };
    commands.trigger(MultiClick {
        entity: target,
        count,
        button: click.event.button,
    });
}
