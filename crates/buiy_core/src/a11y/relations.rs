//! Decomposed AccessKit **relation** component — the cross-reference edges
//! (`labelled_by`/`described_by`/`controls`/…) that one node makes to others.
//!
//! Unlike the per-concept [`states`](super::states) split, the eight relation
//! fields live in **one** component for an honest reason: **translation
//! locality, not co-variance** (semantic-tree.md §1). All are `Entity`-ref
//! vectors resolved to `accesskit::NodeId` in the *same* translate pass
//! (`build_tree` → `node_id_for`), so storage is `Entity` and the resolved
//! `NodeId` never lets an `Entity` leak past the seam. The weaker case
//! (`controls`/`owns`/`flow_to` don't co-vary with `labelled_by`/`described_by`)
//! is acknowledged, not claimed as co-variance — but it stays BSN-patchable
//! per-field via `Reflect`, so it is not a #17644-scale megacomponent violation.
//!
//! Spec: docs/specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md §3.
//!
//! ## P1a wiring status (the carried-but-unwired ledger)
//!
//! The struct carries **all eight** fields (cheap to `Reflect`, BSN-patchable,
//! forward-compatible), but **only four are wired** in Phase 1a — they get a
//! `build_tree` resolution (`Entity` → `NodeId`) and a fold arm in
//! `to_accesskit_node`:
//!
//! - **wired:** `labelled_by`, `described_by`, `controls`, `active_descendant`.
//! - **carried but unwired:** `owns`, `flow_to`, `details`, `error_message` —
//!   **deliberately deferred** (co-drive §3.2: no gallery consumer; `owns`
//!   re-parent only matters for a portalled dialog, and S4 is in-place). These
//!   have **no `build_tree` resolution and no fold arm** until they are
//!   un-deferred; they exist on the struct purely for BSN-patchability and
//!   forward-compat so adding the wiring later is additive, not a reshape.

use bevy::prelude::*;

/// Cross-reference relations a node makes to other entities (semantic-tree.md
/// §3). Storage is [`Entity`]; resolution to `accesskit::NodeId` happens at
/// translate time (`build_tree` via `node_id_for`), so the view stays winit-free
/// and `Entity` never leaks past the seam.
///
/// Field-name spelling follows ARIA: `labelled_by` is the **British double-l**
/// (matching `accesskit::Node::set_labelled_by`), mandatory.
///
/// Of the eight fields, **four are wired** in Phase 1a (`labelled_by`,
/// `described_by`, `controls`, `active_descendant`) and four are
/// **carried-but-unwired** (`owns`, `flow_to`, `details`, `error_message`) — see
/// the module-level ledger. The unwired fields have no populate-side resolution
/// and no fold arm; they are present only for per-field BSN-patchability and
/// forward-compat until un-deferred (co-drive §3.2).
// NOTE: `Reflect` already generates a `FromReflect` impl in Bevy 0.19, so the
// spec's `FromReflect` in the derive set is omitted here (deriving it explicitly
// conflicts — E0119). This matches the established `states.rs` components.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct A11yRelations {
    /// Nodes that label this one → `set_labelled_by`. **WIRED** (P1a).
    pub labelled_by: Vec<Entity>,
    /// Nodes that describe this one → `set_described_by`. **WIRED** (P1a).
    pub described_by: Vec<Entity>,
    /// Nodes this one controls → `set_controls`. **WIRED** (P1a).
    pub controls: Vec<Entity>,
    /// Nodes this one re-parents (`aria-owns`) → `set_owns`. **Carried but
    /// UNWIRED** in P1a (the re-parent is a P1b/portalled-dialog concern;
    /// co-drive §3.2). No `build_tree` resolution, no fold arm.
    pub owns: Vec<Entity>,
    /// Reading-order successors (`aria-flowto`) → `set_flow_to`. **Carried but
    /// UNWIRED** in P1a — no resolution, no fold arm (co-drive §3.2).
    pub flow_to: Vec<Entity>,
    /// Extended-detail nodes (`aria-details`) → `set_details`. **Carried but
    /// UNWIRED** in P1a — no resolution, no fold arm (co-drive §3.2).
    pub details: Vec<Entity>,
    /// The active descendant of a composite widget (`aria-activedescendant`) →
    /// `set_active_descendant`. **WIRED** (P1a).
    pub active_descendant: Option<Entity>,
    /// The node carrying this one's validation error (`aria-errormessage`) →
    /// `set_error_message`. **Carried but UNWIRED** in P1a — no resolution, no
    /// fold arm (co-drive §3.2).
    pub error_message: Option<Entity>,
}
