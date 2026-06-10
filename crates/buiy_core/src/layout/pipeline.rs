//! Layout pipeline ordering.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
//!
//! Twelve ordered sub-sets of `BuiySet::Layout`. Phase 1 wires the original
//! eight; Phase 4 inserts `WritingModeInherit` between `RemovedNodesGc`
//! and `SyncStyles` so step 1 sees the effective inherited writing-mode
//! for every entity. Steps 2 (`CqActivate`), 4 (`CqFlipCheck`), 5
//! (`CqFlipReRun`), and 6 (`PostTaffyOverrides`) remain no-ops in Phase 1.
//! Later phases attach systems to those sub-sets without reordering.
//! Text T2 inserts `TextSync` between `WritingModeInherit` and
//! `SyncStyles` (text architecture § 4.1); text T3 appends `TextCommit`
//! as the new final step.

use bevy::prelude::*;

/// Phase 1 ships every step as a system set; later phases populate the
/// stub steps. The order is asserted by `tests/layout_pipeline_order.rs`.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiyLayoutStep {
    /// Step 0 — drop despawned entities from `LayoutTree`.
    RemovedNodesGc,
    /// Pre-step-1 — populate `WritingModeResolved` by walking the
    /// hierarchy. Runs before `SyncStyles` so step 1 sees the effective
    /// inherited writing-mode for every entity.
    /// **Phase 4.**
    WritingModeInherit,
    /// Pre-step-1 (text) — create/update `TextBuffer` from the authored
    /// text components via the 0.19 lazy setters (lock-free) and mark the
    /// entity's Taffy node dirty when content changed (Taffy caches measure
    /// results — an un-dirtied node serves a stale measurement). After
    /// `WritingModeInherit` (the trigger union includes
    /// `Changed<WritingModeResolved>`), hard before `SyncStyles` (which
    /// must know whether an entity is a measured text leaf when creating
    /// its Taffy node — the T3 context migration).
    /// **Text T2** (text architecture § 4.1; measure-and-layout § 4.1).
    TextSync,
    /// Step 1 — translate changed Buiy components → `taffy::Style` and
    /// sync hierarchy.
    SyncStyles,
    /// Step 2 — set/clear container-query marker components.
    /// **Phase 5.**
    CqActivate,
    /// Step 3 — call `tree.compute_layout` from each root.
    TaffyCompute,
    /// Step 4 — re-evaluate queries against fresh sizes.
    /// **Phase 5.**
    CqFlipCheck,
    /// Step 5 — conditional re-run of steps 1+3.
    /// **Phase 5.** Phase 1 leaves this as an empty set.
    CqFlipReRun,
    /// Step 6 — sub-passes (sticky, table, multicol, anchor).
    /// **Phases 6/7.**
    PostTaffyOverrides,
    /// Step 7 — push positions+sizes to Bevy components.
    WriteResolvedLayout,
    /// Step 8 — multi-level container-query geometric-cascade invalidation:
    /// mark the descendants of every query container whose `ResolvedLayout`
    /// changed this frame as dirty. **Phase 14.**
    CqDescendantInvalidate,
    /// Step 9 — conditional same-frame re-run of the inner work of
    /// `sync_styles` + `taffy_compute` (+ `ResolvedLayout` re-write + CQ
    /// re-evaluation) for the entities `cq_descendant_invalidate` marked
    /// dirty. Gated on `CqDescendantReRunRequested`; capped at one re-run
    /// per frame (D4). **Phase 14.**
    CqDescendantReRun,
}

/// Configure the ordered step chain inside `BuiySet::Layout`.
pub fn configure_pipeline(app: &mut App) {
    app.configure_sets(
        Update,
        (
            BuiyLayoutStep::RemovedNodesGc,
            BuiyLayoutStep::WritingModeInherit,
            BuiyLayoutStep::TextSync,
            BuiyLayoutStep::SyncStyles,
            BuiyLayoutStep::CqActivate,
            BuiyLayoutStep::TaffyCompute,
            BuiyLayoutStep::CqFlipCheck,
            BuiyLayoutStep::CqFlipReRun,
            BuiyLayoutStep::PostTaffyOverrides,
            BuiyLayoutStep::WriteResolvedLayout,
            BuiyLayoutStep::CqDescendantInvalidate,
            BuiyLayoutStep::CqDescendantReRun,
        )
            .chain()
            .in_set(crate::BuiySet::Layout),
    );
}
