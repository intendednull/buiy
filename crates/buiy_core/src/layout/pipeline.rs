//! Layout pipeline ordering.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
//!
//! Eight ordered sub-sets of `BuiySet::Layout`. Phase 1 wires all eight;
//! steps 2 (`CqActivate`), 4 (`CqFlipCheck`), 5 (`CqFlipReRun`), and 6
//! (`PostTaffyOverrides`) are no-ops in Phase 1. Later phases attach
//! systems to those sub-sets without reordering.

use bevy::prelude::*;

/// Phase 1 ships every step as a system set; later phases populate the
/// stub steps. The order is asserted by `tests/layout_pipeline_order.rs`.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiyLayoutStep {
    /// Step 0 — drop despawned entities from `LayoutTree`.
    RemovedNodesGc,
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
}

/// Configure the 8-step chain inside `BuiySet::Layout`.
pub fn configure_pipeline(app: &mut App) {
    app.configure_sets(
        Update,
        (
            BuiyLayoutStep::RemovedNodesGc,
            BuiyLayoutStep::SyncStyles,
            BuiyLayoutStep::CqActivate,
            BuiyLayoutStep::TaffyCompute,
            BuiyLayoutStep::CqFlipCheck,
            BuiyLayoutStep::CqFlipReRun,
            BuiyLayoutStep::PostTaffyOverrides,
            BuiyLayoutStep::WriteResolvedLayout,
        )
            .chain()
            .in_set(crate::BuiySet::Layout),
    );
}
