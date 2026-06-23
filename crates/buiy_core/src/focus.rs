//! Focus model: focus tree, Tab handling, focus-visible heuristic, focus
//! restoration. Phase 0 implements ordered Tab traversal; full focus tree
//! (roving tabindex, aria-activedescendant, traps, restoration, spatial nav)
//! lives in `buiy-focus-model-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.3 and
//! accessibility.md (Focus management).
//!
//! # Phase 0 deferred behavior
//!
//! - **Auto tab order is `entity.index()`-based, not full "document order".**
//!   Bevy reuses entity indices after despawn; for two `Focusable`s with
//!   `tab_order = 0`, the resolved order depends on entity-index allocation,
//!   not insertion order. Insertion-order stability is owned by
//!   `buiy-focus-model-design`.
//! - **`FocusVisible` decay (`:focus-visible`).** Keyboard focus IS
//!   focus-visible: `handle_tab` sets `FocusVisible(true)`. Pointer focus is
//!   NOT: the shared [`focus_on_click`] observer (C3d, input-event-model.md
//!   § 2.7 / co-drive SC-2) sets `FocusVisible(false)` when a primary
//!   `Pointer<Press>` focuses a `Focusable`. C6 reads `entity ==
//!   FocusedEntity.0 && FocusVisible.0` to gate the focus ring. (The richer
//!   focus tree — roving tabindex, scopes, restoration — is still
//!   `buiy-focus-model-design`'s; C3d ships only the resource-level decay
//!   signal, not the ring shape or a `FocusVisible` representation change.)
//! - **Shift detection covers `ShiftLeft`/`ShiftRight` only.** Sticky-keys /
//!   accessibility-shell remappings of Shift to other key codes are out of
//!   scope; full key-binding abstraction lives in `buiy-input-events-design`.

use crate::BuiySet;
use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

/// Marks an entity as part of the focus tree.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Focusable {
    /// Phase 0: 0 = Auto (in document order); negative = Skip; positive = explicit.
    pub tab_order: i32,
}

/// Currently focused entity (None = nothing focused).
#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct FocusedEntity(pub Option<Entity>);

/// Tracks whether the most recent focus change was keyboard / programmatic
/// (true) or pointer (false). Drives the `:focus-visible` heuristic — focus
/// rings render only when this is true.
#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct FocusVisible(pub bool);

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Focusable>()
            .register_type::<FocusedEntity>()
            .register_type::<FocusVisible>()
            .init_resource::<FocusedEntity>()
            .init_resource::<FocusVisible>()
            .add_systems(Update, handle_tab.in_set(BuiySet::Input))
            // C3d (input-event-model.md § 2.7): the single, widget-agnostic
            // focus-on-click observer. Owns `FocusedEntity` for ALL pointer
            // focus — the editor's `editor_pointer_press` and the `TextInput`
            // widget no longer set it themselves; they keep only their
            // non-focus logic (cursor placement / nothing). Lives here in
            // `FocusPlugin` so it covers every `Focusable`, not just editors.
            .add_observer(focus_on_click);
    }
}

/// The single, widget-agnostic focus-on-click observer (input-event-model.md
/// § 2.7 / co-drive SC-2). On a primary [`Pointer<Press>`] it walks from the
/// picked target up the [`ChildOf`] chain to the nearest [`Focusable`] and, if
/// one is found, sets `FocusedEntity` to it AND `FocusVisible(false)` — pointer
/// focus is NOT keyboard-`:focus-visible` (the decay half § 2.7 / C6 needs;
/// `handle_tab` sets the `true` half).
///
/// This consolidates focus-on-click that C3c had split across two per-widget
/// observers (the editor's `editor_pointer_press` and the `TextInput`
/// `focus_on_click`). Both were `Focusable`, so both now focus through this one
/// path; the editor keeps its click-to-place-cursor and the `TextInput` widget
/// no longer needs a focus observer at all.
///
/// **Nearest-`Focusable`-ancestor target:** the picked entity is often a
/// decorative leaf inside a focusable widget root (the picked target need not be
/// the `Focusable` itself). Walking up `ChildOf` focuses the widget, not its
/// inner glyph/child. A press that resolves to no `Focusable` ancestor (a plain
/// node) leaves focus untouched — clicking empty chrome does not steal focus.
/// (Spec § 2.7 notes C3 "ships the leaf version"; the ancestor walk is the
/// robust generalization — for a bare `Focusable` it reduces to the leaf, and it
/// pre-satisfies the C5 "nearest focusable ancestor" refinement without a
/// per-entity focus component.)
///
/// `FocusedEntity`/`FocusVisible` are init by this same `FocusPlugin`, so the
/// resources are always present when this observer is registered — no
/// `Option<Res…>` guard is needed (unlike the editor/widget observers, which ran
/// in harnesses that add `BuiyTextPlugin`/`WidgetsPlugin` without `FocusPlugin`).
/// Observers fire only when the picking pipeline is present, so a headless
/// harness without it is inert by construction.
pub fn focus_on_click(
    press: On<Pointer<Press>>,
    focusables: Query<(), With<Focusable>>,
    parents: Query<&ChildOf>,
    mut focused: ResMut<FocusedEntity>,
    mut visible: ResMut<FocusVisible>,
) {
    if press.event.button != PointerButton::Primary {
        return;
    }
    let Some(target) = nearest_focusable(press.entity, &focusables, &parents) else {
        return; // pressed a non-focusable subtree — leave focus untouched
    };
    focused.0 = Some(target);
    // Pointer focus is NOT focus-visible (the `:focus-visible` decay, § 2.7).
    visible.0 = false;
}

/// Walk from `entity` up the [`ChildOf`] chain, returning the first entity that
/// is itself `Focusable` (including `entity`), or `None` if no ancestor is.
fn nearest_focusable(
    entity: Entity,
    focusables: &Query<(), With<Focusable>>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    let mut current = entity;
    loop {
        if focusables.contains(current) {
            return Some(current);
        }
        current = parents.get(current).ok()?.parent();
    }
}

/// `pub(crate)` so the P1c action router (`a11y::action`) can name it in its
/// intra-`BuiySet::Input` `.before(handle_tab)` ordering constraint
/// (action-router.md §7): the router must drain inbound requests *before* the
/// keyboard focus/edit handlers so a synthesized focus/activation is consumed
/// the same frame. Referencing it across plugins is sound — both systems live in
/// `Update`; a `.before` on a system that isn't scheduled (a harness without
/// `FocusPlugin`) is silently ignored.
pub(crate) fn handle_tab(
    keys: Res<ButtonInput<KeyCode>>,
    focusables: Query<(Entity, &Focusable)>,
    mut focused: ResMut<FocusedEntity>,
    mut visible: ResMut<FocusVisible>,
) {
    let pressed_tab = keys.just_pressed(KeyCode::Tab);
    if !pressed_tab {
        return;
    }
    let forward = !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight);
    advance_focus(&focusables, &mut focused, forward);
    visible.0 = true;
}

fn advance_focus(
    focusables: &Query<(Entity, &Focusable)>,
    focused: &mut FocusedEntity,
    forward: bool,
) {
    let entries: Vec<(Entity, Focusable)> =
        focusables.iter().map(|(e, f)| (e, f.clone())).collect();
    focused.0 = compute_next_focus(&entries, focused.0, forward);
}

fn compute_next_focus(
    focusables: &[(Entity, Focusable)],
    current: Option<Entity>,
    forward: bool,
) -> Option<Entity> {
    let mut entries: Vec<(Entity, Focusable)> = focusables
        .iter()
        .filter(|(_, f)| f.tab_order >= 0)
        .cloned()
        .collect();
    if entries.is_empty() {
        return None;
    }
    // Sort: explicit positive tab_order first (ascending), then Auto (0) in document order.
    entries.sort_by_key(|(e, f)| (if f.tab_order > 0 { 0 } else { 1 }, f.tab_order, e.index()));

    let idx = current.and_then(|e| entries.iter().position(|(x, _)| *x == e));
    let n = entries.len();
    let next_idx = match (idx, forward) {
        (None, true) => 0,
        (None, false) => n - 1,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    };
    Some(entries[next_idx].0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Entity;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    fn f(tab_order: i32) -> Focusable {
        Focusable { tab_order }
    }

    /// Audit #5 (T2.17): the `Skip` branch — a `Focusable` with a negative
    /// `tab_order` is filtered out (`tab_order >= 0`, line 92) and must never
    /// be returned by traversal, even when it is the only other candidate and
    /// even across a wrap. With the skip filter removed this test reddens:
    /// the negative entity would re-enter the candidate list and be reachable.
    #[test]
    fn negative_tab_order_is_skipped() {
        // One focusable, one Skip(-1). The skipped entity is given the LOWER
        // entity index so a broken filter (or an `e.index()`-only sort) would
        // surface it first.
        let skip = e(1);
        let auto = e(2);
        let entries = vec![(skip, f(-1)), (auto, f(0))];

        // From nothing, forward, the only reachable focusable is the Auto one.
        assert_eq!(
            compute_next_focus(&entries, None, true),
            Some(auto),
            "negative tab_order must be skipped, leaving only the Auto focusable"
        );
        // Advancing from the Auto entity wraps back to itself — the skip
        // candidate is never reached.
        assert_eq!(
            compute_next_focus(&entries, Some(auto), true),
            Some(auto),
            "skip candidate must not appear in the wrap"
        );
        // Backward is identical: still only the Auto focusable.
        assert_eq!(
            compute_next_focus(&entries, None, false),
            Some(auto),
            "negative tab_order is skipped in both directions"
        );
    }

    /// Audit #5 (T2.17): the explicit-priority sort — positive `tab_order`s
    /// come before Auto(0), and positives are ordered ascending by their value
    /// (sort key `(if >0 {0} else {1}, tab_order, index)`, line 99). The
    /// entity indices are chosen to FIGHT the sort key: the Auto entity has the
    /// lowest index and the higher-priority positive (tab_order=1) has the
    /// highest index, so an index-only or group-dropped sort would order them
    /// differently and redden this test.
    #[test]
    fn positive_tab_orders_precede_auto_in_ascending_order() {
        let auto = e(1); // tab_order 0, lowest index
        let pos2 = e(2); // tab_order 2
        let pos1 = e(3); // tab_order 1, highest index

        let entries = vec![(auto, f(0)), (pos2, f(2)), (pos1, f(1))];

        // Resolved traversal order must be: pos1 (1) -> pos2 (2) -> auto (0).
        let first = compute_next_focus(&entries, None, true);
        assert_eq!(first, Some(pos1), "lowest positive tab_order comes first");
        let second = compute_next_focus(&entries, first, true);
        assert_eq!(second, Some(pos2), "positives ascend by tab_order value");
        let third = compute_next_focus(&entries, second, true);
        assert_eq!(third, Some(auto), "Auto(0) comes after all positives");
        // Wrap back to the first positive.
        let wrapped = compute_next_focus(&entries, third, true);
        assert_eq!(wrapped, Some(pos1), "traversal wraps to the first positive");
    }
}
