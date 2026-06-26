//! Deterministic per-frame render-world work-unit counters (perf-final P0b).
//!
//! The render-world half of the audit's measurement gate (the audit's #1 finding
//! is measurement blindness). These are integers a steady-frame test asserts
//! EXACTLY — identical on any CPU, so they are host-independent and non-flaky (a
//! re-introduced ungated O(N) pass reddens on a slow shared runner just as on the
//! dev box). They copy the existing always-on-integer discipline
//! (`LayoutTaffyComputeCount`, `SyncStylesIterCount`).
//!
//! Each field is OVERWRITTEN by its owning extract system every frame (the
//! `SyncStylesIterCount` overwrite idiom), so there is no reset system to keep in
//! sync. The systems read the resource as `Option<ResMut<RenderWorkCounters>>`, so
//! a harness that does not register it simply does not count — no missing-resource
//! skip, no registration drift breaking existing render tests. Registered in the
//! real `RenderApp` AND the `buiy_bench_support` harness (the gate tests' home).

use bevy::prelude::*;

use crate::text::ResidentTextKeys;

/// Per-frame render-world work counts. Read in a gate test via
/// `harness.render.resource::<RenderWorkCounters>()`.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct RenderWorkCounters {
    /// `1` if `extract_buiy_nodes` rebuilt the node set this frame, `0` if the
    /// damage gate skipped it. The audit-#2 rebuild-rate signal — `0` on an idle
    /// frame, `1` (not `N`) for a single-entity change.
    pub node_rebuilds: u32,
    /// Number of `ExtractedNode` records built this frame (`== N` on a rebuild,
    /// `0` on an idle frame — set in every `extract_buiy_nodes` return path).
    pub instances_built: usize,
    /// Logical atlas-LRU touches this frame — recorded as `resident.keys.len()` (one
    /// `touch_existing` per resident glyph key), a single post-extract read so the
    /// counter never taxes the hot loop it measures.
    ///
    /// HONEST SCOPE (do not over-claim). Because this is DERIVED from
    /// `resident.keys.len()` (see `record_text_work_counters`), the
    /// `atlas_touch_ops == resident_keys` assert is a RESIDENCY INVARIANT — one
    /// logical touch per resident key, no dup / leak / unbounded growth — and can
    /// NEVER diverge, so it is **not** a #5-regression guard. The #5 regression (the
    /// O(V·E) per-touch `VecDeque` scan) is a per-touch COST, not a touch COUNT, so it
    /// is invisible to any work-counter. The real #5 guards are the iai instruction
    /// count (CI lane, which prices the scan) and the prototype's measured 8.6×
    /// wall-clock — not this gate.
    pub atlas_touch_ops: usize,
    /// Resident glyph-instance key count after the frame's text extract. Guards
    /// unbounded growth / a missing dedup; the `atlas_touch_ops == resident_keys`
    /// residency invariant holds on an idle text frame (one logical touch per
    /// resident key). See `atlas_touch_ops` for why this is NOT a #5-regression guard.
    pub resident_keys: usize,
    /// `1` if this dirty frame is Patch-ELIGIBLE (audit #2 Stage B classifier):
    /// the damage is value-only (no structural/hierarchy/group/despawn/theme
    /// change), so a future Patch stage COULD re-extract only the changed slots
    /// instead of the whole scene. `0` on a Full (structural) rebuild and on idle.
    /// Stage B is observation-only — the extract still does a Full rebuild; this
    /// counter measures the Patch-vs-Full mix to size the C/D Patch-path payoff
    /// before building it.
    pub node_patches: u32,
}

/// Set `node_rebuilds` + `instances_built` (the `extract_buiy_nodes` work counts)
/// in every return path: `(0, 0)` on the idle/gate-skip and no-window paths,
/// `(1, N)` on a rebuild. Inert when the counter is unregistered.
pub(crate) fn record_node_counts(
    counters: &mut Option<ResMut<RenderWorkCounters>>,
    rebuilds: u32,
    built: usize,
    patches: u32,
) {
    if let Some(c) = counters.as_deref_mut() {
        c.node_rebuilds = rebuilds;
        c.instances_built = built;
        c.node_patches = patches;
    }
}

/// Record the text/atlas work counts (`atlas_touch_ops`, `resident_keys`) AFTER
/// `extract_buiy_glyphs` has refreshed `ResidentTextKeys`. A separate tiny system
/// (not a param on `extract_buiy_glyphs`, which sits at Bevy's 16-param cap), and
/// `Option<ResMut<_>>` so it is inert when the counter is unregistered.
pub fn record_text_work_counters(
    resident: Res<ResidentTextKeys>,
    counters: Option<ResMut<RenderWorkCounters>>,
) {
    if let Some(mut c) = counters {
        let n = resident.keys.len();
        // One `touch_existing` per resident key ran in `extract_buiy_glyphs` (both
        // the idle and the dirty branch loop the full resident set once).
        c.atlas_touch_ops = n;
        c.resident_keys = n;
    }
}
