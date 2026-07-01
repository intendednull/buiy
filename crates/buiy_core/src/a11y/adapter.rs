//! Per-window AccessKit adapter bridge. Buiy pushes its `A11yTreeBuilder`
//! snapshot into bevy_winit's existing `ACCESS_KIT_ADAPTERS` thread-local
//! each frame so that real screen readers attached to the window see Buiy's
//! widget tree.
//!
//! Architecture: foundation spec architecture.md § 2.6, accessibility.md § 3.11.
//!
//! # Adapter lifecycle ownership
//!
//! Bevy 0.19 (`bevy_winit`) owns the `Adapter` objects — they are created in
//! `prepare_accessibility_for_window` (called from the winit runner with
//! `ActiveEventLoop` in hand) and stored in the `ACCESS_KIT_ADAPTERS`
//! thread-local. `AccessKitAdapterPlugin` does *not* create or destroy
//! adapters; it only pushes `TreeUpdate` payloads each frame via
//! `update_if_active`.
//!
//! # Test-friendliness note
//!
//! Tests using `MinimalPlugins` (no winit) see an empty `ACCESS_KIT_ADAPTERS`
//! thread-local, so `push_tree_updates` is a no-op and the pure-translation
//! behavior is covered by `tests/a11y_translate.rs` independently.

use crate::BuiySet;
use crate::a11y::A11yTreeBuilder;
use crate::a11y::translate::build_tree_update;
use crate::focus::FocusedEntity;
use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::accessibility::ACCESS_KIT_ADAPTERS;

/// Plugin that wires `A11yTreeBuilder` → bevy_winit's per-window
/// AccessKit adapters each frame.
///
/// `push_tree_updates` is ordered `.after(crate::a11y::build_tree)` so it
/// observes the freshly built snapshot for the current frame, not the
/// previous one. Both systems live in `BuiySet::A11yUpdate`; without the
/// explicit dependency Bevy's scheduler is free to reorder them
/// (ambiguity-detection defaults to `LogLevel::Ignore`).
pub struct AccessKitAdapterPlugin;

impl Plugin for AccessKitAdapterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            push_tree_updates
                .in_set(BuiySet::A11yUpdate)
                .after(crate::a11y::build_tree),
        );
    }
}

fn push_tree_updates(
    // MT-safety (D2): pin to the main thread. `ACCESS_KIT_ADAPTERS` is a
    // bevy_winit thread-local owned by the main thread (populated in the winit
    // runner). Under the `multi_threaded` executor a system with no `NonSend`
    // param is eligible to run on any worker thread, where this thread-local is
    // empty — silently dropping every AccessKit `TreeUpdate`. The marker pins
    // this system to the main thread, the sanctioned mechanism (same as the
    // layout systems' `NonSend<LayoutTree>`). See
    // docs/specs/2026-06-30-mt-safety-design.md (D2).
    _main_thread: NonSendMarker,
    builder: Res<A11yTreeBuilder>,
    focused: Res<FocusedEntity>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
) {
    use crate::a11y::translate::node_id_for;

    let focused_id = focused.0.map(node_id_for);
    let snapshot = builder.snapshot();

    // Key the synthetic root off the primary window entity (semantic-tree.md
    // §7.2) when one exists; `None` under `MinimalPlugins` falls back to the
    // stable `ROOT_NODE_ID`.
    let root_entity = primary_window.single().ok();

    // Always build and push, even when the snapshot is empty. An empty
    // TreeUpdate (root-only) is the correct signal for the AT to clear
    // stale state when all widgets are removed (dialog close, scene
    // transition, etc.). `accesskit_unix::Adapter::update_if_active` diffs
    // and emits `ChildRemoved` events when given a root-only TreeUpdate.
    // The `with_borrow_mut` loop is a no-op when no winit windows exist
    // (e.g. tests with `MinimalPlugins`).
    let update = build_tree_update(snapshot, focused_id, root_entity);

    ACCESS_KIT_ADAPTERS.with_borrow_mut(|ak_adapters| {
        for (_window_id, adapter) in ak_adapters.iter_mut() {
            let cloned = update.clone();
            adapter.update_if_active(|| cloned);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MT-safety (D2) regression guard. `push_tree_updates` MUST stay
    /// main-thread-pinned (`!Send`): it touches bevy_winit's main-thread
    /// `ACCESS_KIT_ADAPTERS` thread-local, and under the `multi_threaded` executor
    /// a `Send` system is eligible for any worker thread — where that thread-local
    /// is empty, silently dropping every AccessKit `TreeUpdate`. The `NonSendMarker`
    /// param is what makes the system `!Send`; deleting it reddens HERE. This
    /// asserts the structural guarantee because no headless test can observe the
    /// dropped-update bug directly (none drives a real winit adapter on a worker
    /// thread). See docs/specs/2026-06-30-mt-safety-design.md (D2).
    #[test]
    fn push_tree_updates_is_main_thread_pinned() {
        let mut world = World::new();
        let mut system = IntoSystem::into_system(push_tree_updates);
        system.initialize(&mut world);
        assert!(
            !system.is_send(),
            "push_tree_updates must be !Send (NonSend-pinned) so the multi_threaded \
             executor never runs it on a worker thread where ACCESS_KIT_ADAPTERS \
             (a main-thread thread-local) is empty — restore the NonSendMarker param"
        );
    }
}
