//! Layout pipeline ordering.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
//!
//! Thirteen ordered sub-sets of `BuiySet::Layout`. Phase 1 wires the
//! original eight; Phase 4 inserts `WritingModeInherit` between
//! `RemovedNodesGc` and `SyncStyles` so step 1 sees the effective inherited
//! writing-mode for every entity. Steps 2 (`CqActivate`), 4 (`CqFlipCheck`),
//! 5 (`CqFlipReRun`), and 6 (`PostTaffyOverrides`) remain no-ops in Phase 1.
//! Later phases attach systems to those sub-sets without reordering.
//! Text T2 inserts `TextSync` between `WritingModeInherit` and
//! `SyncStyles` (text architecture § 4.1); text T3 appends `TextCommit`
//! as the new final step.

use bevy::prelude::*;

/// Phase 1 ships every step as a system set; later phases populate the
/// stub steps. The order is asserted by `tests/layout_pipeline_order.rs`.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiyLayoutStep {
    /// Pre-step-0 — seed the per-frame `LayoutDirtyThisFrame` gate flag (perf
    /// audit #3). Runs FIRST (before any pass mutates a layout input) so its
    /// `Changed`/`RemovedComponents` reads observe the prior frame's writes,
    /// and gates `PostTaffyOverrides`. Always runs (it must advance the removal
    /// cursors every frame). **Perf-final #3.**
    SeedLayoutDirty,
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
    /// Step 10 (text) — reshape each `TextBuffer` at its FINAL Taffy
    /// content-box (the measured width can differ under stretch/grow, and
    /// measure leaves `height_opt = None`), apply text-align (a finalize
    /// concern — cosmic `Align` needs the final line width), and write
    /// `ResolvedBaseline` + `ComputedTextLayout` idempotently. Must trail
    /// `CqDescendantReRun`: steps 8–9 can still rewrite `ResolvedLayout`,
    /// and committing earlier would shape against sizes step 9
    /// immediately invalidates (text measure § 4.2).
    /// **Text T3** (architecture § 4.2).
    TextCommit,
}

/// Run condition for `BuiyLayoutStep::PostTaffyOverrides` (perf audit #3):
/// gate the ~7-pass post-Taffy override chain on the per-frame dirty flag
/// `seed_layout_dirty` seeds. An idle frame (no override input changed) skips
/// the whole chain — output-identical, because every pass writes idempotently.
fn layout_is_dirty(dirty: Res<super::systems::LayoutDirtyThisFrame>) -> bool {
    dirty.0
}

/// Configure the ordered step chain inside `BuiySet::Layout`.
pub fn configure_pipeline(app: &mut App) {
    app.configure_sets(
        Update,
        (
            BuiyLayoutStep::SeedLayoutDirty,
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
            BuiyLayoutStep::TextCommit,
        )
            .chain()
            .in_set(crate::BuiySet::Layout),
    );

    // Perf audit #3 — gate ONLY the post-Taffy override chain on the per-frame
    // dirty flag. `write_resolved_layout`, `inherit_writing_mode`, and
    // `taffy_compute` stay UNGATED (increment 1): the first keeps
    // `Changed<ResolvedLayout>` a complete self-healing geometry proxy, and the
    // last keeps its per-frame counter resets from being stranded.
    app.configure_sets(
        Update,
        BuiyLayoutStep::PostTaffyOverrides.run_if(layout_is_dirty),
    );
}
