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
use std::collections::HashMap;

/// Registry of window entities that Buiy has successfully pushed at least one
/// tree update to. This is a `NonSend` resource for API symmetry with
/// bevy_winit's internal adapter storage; the actual `Adapter` objects live
/// in bevy_winit's `ACCESS_KIT_ADAPTERS` thread-local.
///
/// Phase 0 closeout: the map is populated during `push_tree_updates` and
/// exposed so callers can observe which windows received a tree update.
/// The `adapters` field mirrors the plan's naming (the plan assumed we'd
/// own the adapters directly; in Bevy 0.18 the adapter lifecycle belongs to
/// `bevy_winit::prepare_accessibility_for_window`).
#[derive(Default)]
pub struct AccessKitAdapters {
    /// Windows that have received at least one `TreeUpdate` this session.
    /// Empty when no winit windows are present (e.g. in tests with MinimalPlugins).
    pub adapters: HashMap<Entity, ()>,
}

/// Plugin that wires `A11yTreeBuilder` → bevy_winit's per-window
/// AccessKit adapters each frame.
pub struct AccessKitAdapterPlugin;

impl Plugin for AccessKitAdapterPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<AccessKitAdapters>()
            .add_systems(Update, push_tree_updates.in_set(BuiySet::A11yUpdate));
    }
}

fn push_tree_updates(
    builder: Res<A11yTreeBuilder>,
    focused: Res<FocusedEntity>,
    mut adapters: NonSendMut<AccessKitAdapters>,
) {
    use crate::a11y::translate::node_id_for;

    let focused_id = focused.0.map(node_id_for);
    let snapshot = builder.snapshot();
    if snapshot.is_empty() && focused_id.is_none() {
        return;
    }

    // Build the update once; `TreeUpdate` derives `Clone` in accesskit 0.21
    // so cloning per adapter is cheap for the typical single-window case.
    let update = build_tree_update(snapshot, focused_id);

    ACCESS_KIT_ADAPTERS.with_borrow_mut(|ak_adapters| {
        for (window_id, adapter) in ak_adapters.iter_mut() {
            let cloned = update.clone();
            adapter.update_if_active(|| cloned);
            // Record that we've pushed to this window.
            adapters.adapters.entry(*window_id).or_insert(());
        }
    });
}
