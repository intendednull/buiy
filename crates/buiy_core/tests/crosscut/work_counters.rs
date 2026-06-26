//! Deterministic work-unit-counter gates (perf-final P0b). Host-independent
//! integers asserted EXACTLY on a settled scene — the measurement gate the audit
//! (#1 finding: measurement blindness) calls for. Drives the shared
//! `buiy_bench_support` adapterless harness (the same one the criterion bench +
//! the coming dhat/iai gates use).

use bevy::prelude::DetectChangesMut;
use buiy_bench_support::{build_flat_bg_scene, build_large_scene};
use buiy_core::render::RenderWorkCounters;
use buiy_core::render::components::Background;

/// The audit-#5 BLIND-SPOT lock: on an idle text frame the atlas-LRU touch loop
/// runs once per resident glyph key, so `atlas_touch_ops == resident_keys > 0`.
/// A naive "node rebuild == 0 on idle" gate is green here while the touch loop
/// runs (and dhat is blind — the loop is non-allocating); only this counter sees
/// the #5 cost. If a future commit re-introduces the O(visible-glyphs) VecDeque
/// scan, `atlas_touch_ops` would exceed `resident_keys` and this reddens.
#[test]
fn idle_text_frame_touches_one_per_resident_key() {
    let mut h = build_large_scene(8);
    for _ in 0..8 {
        h.frame(); // settle (the cosmic-text reshape echo quiesces in a few frames)
    }
    h.frame(); // one steady/idle frame
    let c = *h.render.resource::<RenderWorkCounters>();

    assert!(
        c.resident_keys > 0,
        "a text scene must have resident glyph keys (got {})",
        c.resident_keys
    );
    assert_eq!(
        c.atlas_touch_ops, c.resident_keys,
        "#5: exactly one atlas touch per resident key, no per-glyph reorder work"
    );
    assert_eq!(
        c.node_rebuilds, 0,
        "#2: an idle frame must NOT rebuild the node set (the damage gate skips)"
    );
}

/// The audit-#2 gate: a settled scene rebuilds 0×; a single group-free value change is
/// an in-place PATCH of exactly ONE record (#2 C3b — the O(N)->O(1) win), not a Full
/// rebuild of all N.
#[test]
fn idle_zero_rebuilds_one_change_exactly_one() {
    let (mut h, victim) = build_flat_bg_scene(64);
    for _ in 0..8 {
        h.frame();
    }

    // Idle: the damage gate skips, so no rebuild and no records built this frame.
    h.frame();
    let idle = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(idle.node_rebuilds, 0, "idle frame: no node rebuild");
    assert_eq!(idle.instances_built, 0, "idle frame: no records built");

    // One interactive change to a single node → an in-place PATCH of exactly that one
    // record (#2 C3b: the O(N)->O(1) win, NOT a Full rebuild of all 64).
    if let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
        bg.set_changed();
    }
    h.frame();
    let changed = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(
        changed.node_rebuilds, 0,
        "#2 C3b: a group-free value change is a Patch, not a Full rebuild"
    );
    assert_eq!(
        changed.node_patches, 1,
        "#2 C3b: the frame is classified + applied as a Patch"
    );
    assert_eq!(
        changed.instances_built, 1,
        "#2 C3b: a Patch re-resolves exactly the one changed record, not all 64"
    );
}

/// #2 C3b Patch classification: a value-only change (Background re-tint) on a group-free
/// scene is an in-place PATCH (`node_patches == 1`, `node_rebuilds == 0`), while a
/// structural/footprint change (a Border appearing) forces a Full rebuild
/// (`node_patches == 0`, `node_rebuilds == 1`).
#[test]
fn patch_classifies_value_change_patch_structural_full() {
    use buiy_core::render::components::Border;

    // Value-only: hover-retint a solid bg node, no effect groups -> in-place Patch.
    let (mut h, victim) = build_flat_bg_scene(32);
    for _ in 0..8 {
        h.frame();
    }
    if let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
        bg.set_changed();
    }
    h.frame();
    let c = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(
        c.node_rebuilds, 0,
        "#2 C3b: a group-free value change is a Patch, not a Full rebuild"
    );
    assert_eq!(
        c.node_patches, 1,
        "#2 C3b: a group-free value-only change is applied as a Patch"
    );

    // Structural: a Border appears (a footprint change) -> Full, not Patch.
    let (mut h2, victim2) = build_flat_bg_scene(32);
    for _ in 0..8 {
        h2.frame();
    }
    h2.app
        .world_mut()
        .entity_mut(victim2)
        .insert(Border::default());
    h2.frame();
    let c2 = *h2.render.resource::<RenderWorkCounters>();
    assert_eq!(c2.node_rebuilds, 1, "structural change rebuilds");
    assert_eq!(
        c2.node_patches, 0,
        "#2 C3b: a Border appearing (footprint change) forces a Full rebuild"
    );
}

/// #2 C3b R5 trap: an in-place Patch overwrites ONLY the changed entity's slot — every
/// retained sibling record stays byte-identical and the slot ORDER is preserved (the
/// Patch never rebuilds the ordered Vec from the changed set). Together with the
/// `instances_built == 1` counter (one record re-resolved), this proves the Patch is
/// surgical, not a disguised full rebuild.
#[test]
fn patch_retains_sibling_records_byte_identical() {
    use buiy_core::render::extract::ExtractedNodesView;

    let (mut h, victim) = build_flat_bg_scene(64);
    for _ in 0..8 {
        h.frame();
    }
    // The full ordered record set after a settled Full build.
    let before: Vec<_> = h.render.resource::<ExtractedNodesView>().0.nodes.clone();

    // One group-free value change -> a Patch.
    if let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
        bg.set_changed();
    }
    h.frame();
    let c = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(c.node_patches, 1, "the single value change is a Patch");
    assert_eq!(c.node_rebuilds, 0, "a Patch is not a Full rebuild");

    let after = &h.render.resource::<ExtractedNodesView>().0.nodes;
    assert_eq!(
        before.len(),
        after.len(),
        "R5: a Patch preserves the ordered record count (no sibling dropped)"
    );
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(
            b.entity, a.entity,
            "R5: a Patch preserves slot order (same entity at each slot)"
        );
        if a.entity != victim {
            assert_eq!(
                b, a,
                "R5: every sibling record is byte-identical — retained, not re-derived"
            );
        }
    }
}
