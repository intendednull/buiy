//! Deterministic work-unit-counter gates (perf-final P0b). Host-independent
//! integers asserted EXACTLY on a settled scene — the measurement gate the audit
//! (#1 finding: measurement blindness) calls for. Drives the shared
//! `buiy_bench_support` adapterless harness (the same one the criterion bench +
//! the coming dhat/iai gates use).

use bevy::prelude::DetectChangesMut;
use buiy_bench_support::{build_flat_bg_scene, build_large_scene};
use buiy_core::render::RenderWorkCounters;
use buiy_core::render::components::Background;

/// Atlas-residency invariant (NOT a #5-regression guard — see `RenderWorkCounters`
/// `atlas_touch_ops`). On an idle text frame the atlas-LRU touch loop runs once per
/// resident glyph key, so `atlas_touch_ops == resident_keys > 0`. Both are recorded
/// from `resident.keys.len()`, so this pins exactly one LOGICAL touch per resident key
/// (no dup / leak) AND that the steady frame does not rebuild the node set. It does NOT
/// catch a #5 regression: the O(visible-glyphs) VecDeque scan is a per-touch COST, not a
/// touch COUNT, so it never moves these integers — the iai instruction count (CI) + the
/// prototype's measured 8.6× wall-clock are the real #5 guards.
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
        "residency invariant: exactly one logical touch per resident key (this is a \
         dup/leak sanity check, NOT a #5 guard — see counters.rs atlas_touch_ops)"
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

/// Glyph partial-reextract Stage C (D1/D2/D3) — THE counter FLIP gate: an
/// idle text frame records ZERO glyph work (`GlyphDamage` untouched — the
/// § 6.2 O(0) contract); one value-tier change on one of N text entities
/// EXECUTES a Patch (`glyph_patches == 1`, `glyph_full_rebuilds == 0` — the
/// wholesale rebuild did NOT run) naming exactly the victim; a structural
/// change (a despawn ticking the parent's `Children`) still executes a Full
/// rebuild (`glyph_full_rebuilds == 1`, `glyph_patches == 0`).
///
/// History: the Stage-B ancestor of this test
/// (`glyph_classifier_idle_zeros_one_change_one_patch_candidate`) pinned
/// `glyph_full_rebuilds == 1` on the change frame while the verdict was
/// observation-only; Stage C's Patch execution flips that assertion —
/// updated consciously here, per the plan's "counter FLIP" verify.
#[test]
fn glyph_flip_idle_zeros_one_change_executes_patch_structural_full() {
    use bevy::prelude::Color;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::TextColor;
    use buiy_core::text::GlyphDamage;

    let mut h = build_large_scene(4); // 8 text entities: 1 change ≤ the D3 fraction bail
    for _ in 0..8 {
        h.frame(); // settle (the cosmic-text reshape echo quiesces in a few frames)
    }
    // Pick the victim AFTER settling: `TextBuffer` is inserted by TextSync on
    // the first Update, not at spawn.
    let victim = {
        let mut q = h
            .app
            .world_mut()
            .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<buiy_core::text::TextBuffer>>();
        q.iter(h.app.world()).next().expect("a text entity")
    };

    // Idle: no glyph rebuild, no patch, no patched entities.
    h.frame();
    let idle = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(idle.glyph_full_rebuilds, 0, "idle frame: no glyph rebuild");
    assert_eq!(idle.glyph_patches, 0, "idle frame: no executed patch");
    assert_eq!(
        idle.glyph_patch_candidates, 0,
        "idle frame: no Patch verdict"
    );
    assert_eq!(
        idle.glyph_patched_entities, 0,
        "idle frame: no patched entities"
    );

    // One value-tier change on ONE text entity → an EXECUTED Patch.
    h.app
        .world_mut()
        .entity_mut(victim)
        .insert(TextColor(ColorToken::Custom(Color::srgb(0.2, 0.7, 0.3))));
    h.frame();
    let changed = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(
        changed.glyph_full_rebuilds, 0,
        "Stage C FLIP: the wholesale rebuild did NOT run for a 1-entity value change"
    );
    assert_eq!(
        changed.glyph_patches, 1,
        "Stage C FLIP: the Patch EXECUTED (order walk skipped, victim spliced)"
    );
    assert_eq!(
        changed.glyph_patch_candidates, 1,
        "the classifier verdicted the frame Patch-eligible (D3)"
    );
    assert_eq!(
        changed.glyph_patched_entities, 1,
        "exactly the one changed entity is named by the verdict"
    );
    assert!(
        matches!(
            h.render.resource::<GlyphDamage>(),
            GlyphDamage::Patch { changed, removed }
                if changed.as_slice() == [victim] && removed.is_empty()
        ),
        "GlyphDamage::Patch names exactly the victim"
    );

    // A structural change — a despawn ticks the parent's `Children` — still
    // executes the Full rebuild (D3: "any uncertainty → Full").
    h.app.world_mut().entity_mut(victim).despawn();
    h.frame();
    let structural = *h.render.resource::<RenderWorkCounters>();
    assert_eq!(
        structural.glyph_full_rebuilds, 1,
        "a structural change still executes a Full rebuild"
    );
    assert_eq!(
        structural.glyph_patches, 0,
        "a structural change never executes a Patch"
    );
}
