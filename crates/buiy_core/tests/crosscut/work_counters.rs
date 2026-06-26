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

/// The audit-#2 rebuild-rate gate: a settled scene rebuilds 0×; a single changed
/// entity rebuilds exactly ONCE (not N×), and a rebuild builds every node record.
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

    // One interactive change to a single node → exactly one full rebuild.
    if let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
        bg.set_changed();
    }
    h.frame();
    let changed = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(
        changed.node_rebuilds, 1,
        "#2: one changed entity triggers exactly one rebuild, not N"
    );
    assert!(
        changed.instances_built >= 64,
        "a rebuild builds every node record (got {} for 64 painting nodes)",
        changed.instances_built
    );
}
