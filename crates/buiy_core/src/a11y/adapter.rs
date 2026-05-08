//! Per-window AccessKit adapter bridge. Buiy pushes its `A11yTreeBuilder`
//! snapshot into bevy_winit's existing `ACCESS_KIT_ADAPTERS` thread-local
//! each frame so that real screen readers attached to the window see Buiy's
//! widget tree.
//!
//! Architecture: foundation spec architecture.md § 2.6, accessibility.md § 3.11.
//!
//! # Adapter lifecycle ownership
//!
//! Bevy 0.18 (`bevy_winit`) owns the `Adapter` objects — they are created in
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
use bevy::prelude::*;
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

fn push_tree_updates(builder: Res<A11yTreeBuilder>, focused: Res<FocusedEntity>) {
    use crate::a11y::translate::node_id_for;

    let focused_id = focused.0.map(node_id_for);
    let snapshot = builder.snapshot();

    // Always build and push, even when the snapshot is empty. An empty
    // TreeUpdate (root-only) is the correct signal for the AT to clear
    // stale state when all widgets are removed (dialog close, scene
    // transition, etc.). `accesskit_unix::Adapter::update_if_active` diffs
    // and emits `ChildRemoved` events when given a root-only TreeUpdate.
    // The `with_borrow_mut` loop is a no-op when no winit windows exist
    // (e.g. tests with `MinimalPlugins`).
    let update = build_tree_update(snapshot, focused_id);

    ACCESS_KIT_ADAPTERS.with_borrow_mut(|ak_adapters| {
        for (_window_id, adapter) in ak_adapters.iter_mut() {
            let cloned = update.clone();
            adapter.update_if_active(|| cloned);
        }
    });
}
