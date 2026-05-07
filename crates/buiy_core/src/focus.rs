//! Focus model: focus tree, Tab handling, focus-visible heuristic, focus
//! restoration. Phase 0 implements ordered Tab traversal; full focus tree
//! (roving tabindex, aria-activedescendant, traps, restoration, spatial nav)
//! lives in `buiy-focus-model-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.3 and
//! accessibility.md (Focus management).

use crate::BuiySet;
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
            .add_systems(Update, handle_tab.in_set(BuiySet::Input));
    }
}

fn handle_tab(
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

/// Test-friendly helper: advance focus without needing an input event loop.
pub fn advance_focus_for_test(app: &mut App, forward: bool) {
    let focusables: Vec<(Entity, Focusable)> = app
        .world_mut()
        .query::<(Entity, &Focusable)>()
        .iter(app.world())
        .map(|(e, f)| (e, f.clone()))
        .collect();
    let mut focused = app.world_mut().resource_mut::<FocusedEntity>();
    let prev = focused.0;
    let next = compute_next_focus(&focusables, prev, forward);
    focused.0 = next;
    app.world_mut().resource_mut::<FocusVisible>().0 = true;
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
