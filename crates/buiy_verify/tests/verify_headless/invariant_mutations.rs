//! Task 2.9 — MANDATORY mutation meta-tests: prove each Tier-3 predicate has
//! teeth. A property suite that never fails is worthless, so for every predicate
//! we hand-build a fixture that VIOLATES exactly one relation and assert the
//! predicate REJECTS it (`Err`), plus a known-good control that PASSES (`Ok`).
//! These are the Tier-3 analogue of the half-size sign-bug regression in
//! `render_instance.rs` (invariants.md § Verification).
//!
//! Plain `#[test]`s (no proptest, no GPU) so the harness's own correctness rides
//! the same `cargo test -p buiy_verify` gate.

use std::collections::HashMap;

use bevy::prelude::*;
use buiy_core::layout::TopLayer;
use buiy_core::render::components::ClipRect;
use buiy_core::render::extract::{ExtractedNode, ExtractedNodes};
use buiy_core::render::instance::PackedInstance;
use buiy_verify::invariant::{
    EPS, Realized, all_finite, all_finite_packed, contexts_do_not_interleave, mat4_is_identity,
    mat4_is_pure_scale, paint_order_is_total, paint_order_respects_paint_key, top_layer_dominates,
};

// --- fixture builders -------------------------------------------------------

fn e(i: u32) -> Entity {
    Entity::from_raw_u32(i).expect("valid entity index")
}

/// A finite, well-formed node at a deterministic position with a given size.
fn node(entity: Entity, size: Vec2) -> ExtractedNode {
    ExtractedNode {
        entity,
        position: Vec2::new(1.0, 2.0),
        size,
        color: Color::WHITE,
        clip: Some(ClipRect {
            min: Vec2::ZERO,
            max: size,
        }),
        group: None,
    }
}

fn nodes(list: Vec<ExtractedNode>) -> ExtractedNodes {
    ExtractedNodes {
        nodes: list,
        ..Default::default()
    }
}

/// Build a `Realized` from an explicit paint-ordered entity list plus per-entity
/// top-layer assignments and the per-context painted-region map, so the
/// top-layer / interleave fixtures can inject a precise violation.
fn realized(
    order: &[Entity],
    top_layer_of: &[(Entity, TopLayer)],
    context_members: &[(Entity, Vec<Entity>)],
) -> Realized {
    let tl: HashMap<Entity, TopLayer> = top_layer_of.iter().copied().collect();
    let members: HashMap<Entity, Vec<Entity>> = context_members.iter().cloned().collect();
    // `context_of` is the nearest-context map; for these flat fixtures each
    // entity is its own context unless a region lists it, but the predicates
    // under test read `top_layer_of` / `context_members`, so a self-map is fine.
    let cx: HashMap<Entity, Entity> = order.iter().map(|&en| (en, en)).collect();
    let name: HashMap<Entity, String> = order.iter().map(|&en| (en, format!("{en:?}"))).collect();
    Realized {
        nodes: nodes(
            order
                .iter()
                .map(|&en| node(en, Vec2::splat(10.0)))
                .collect(),
        ),
        context_of: cx,
        context_members: members,
        top_layer_of: tl,
        // The top-layer / interleave fixtures do not exercise the paint-key
        // order predicate, so leave its inputs empty (an empty map is vacuously
        // ordered). The dedicated meta-test below builds these directly.
        painters_of_ctx: HashMap::new(),
        paint_key_of: HashMap::new(),
        name_of: name,
    }
}

// --- #1 paint_order_is_total ------------------------------------------------

#[test]
fn paint_order_rejects_a_duplicate_entity() {
    // Same entity painted twice — a partial-re-extract / walk bug.
    let dup = nodes(vec![
        node(e(1), Vec2::splat(10.0)),
        node(e(1), Vec2::splat(20.0)),
    ]);
    assert!(
        paint_order_is_total(&dup).is_err(),
        "a duplicated entity must be rejected"
    );
}

#[test]
fn paint_order_accepts_distinct_entities() {
    let ok = nodes(vec![
        node(e(1), Vec2::splat(10.0)),
        node(e(2), Vec2::splat(10.0)),
    ]);
    assert!(paint_order_is_total(&ok).is_ok(), "distinct entities pass");
}

// --- #2 transform_roundtrips (relation-check teeth) -------------------------

#[test]
fn identity_check_rejects_a_miscomposed_matrix() {
    // A deliberately non-identity matrix (a leaked translation) must NOT pass
    // the "≈ identity" relation — this is what catches a mis-composed
    // translate∘-translate or a rotate(2π) that did not return to I.
    let bad = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
    assert!(mat4_is_identity("test", bad).is_err());
    assert!(
        mat4_is_identity("test", Mat4::IDENTITY).is_ok(),
        "the true identity passes"
    );
}

#[test]
fn pure_scale_check_rejects_an_s_r_t_miscomposition() {
    // The spec's mutation: feed `S·R·T` instead of the pure diagonal `S`. The
    // rotation leaks off-diagonals, so the pure-scale relation rejects it.
    let k = [2.0f32, 3.0, 4.0];
    let s = Mat4::from_scale(Vec3::from_array(k));
    let r = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4);
    let tr = Mat4::from_translation(Vec3::new(7.0, 8.0, 0.0));
    let miscomposed = s * r * tr;
    assert!(
        mat4_is_pure_scale("test", miscomposed, k).is_err(),
        "S·R·T must be rejected as not a pure scale"
    );
    // The genuine pure scale passes.
    assert!(mat4_is_pure_scale("test", s, k).is_ok(), "pure S passes");
}

#[test]
fn identity_check_eps_boundary() {
    // A perturbation just OVER EPS is rejected; just UNDER is accepted — the
    // tolerance is real, not vacuous.
    let mut over = Mat4::IDENTITY;
    over.x_axis.y = EPS * 2.0;
    assert!(mat4_is_identity("test", over).is_err(), "> EPS rejected");

    let mut under = Mat4::IDENTITY;
    under.x_axis.y = EPS * 0.5;
    assert!(mat4_is_identity("test", under).is_ok(), "< EPS accepted");
}

// --- #3 top_layer_dominates -------------------------------------------------

#[test]
fn top_layer_rejects_a_normal_node_after_a_top_layer_node() {
    // Order: [top-layer modal, normal] — the normal node paints AFTER the top
    // layer, which violates dominance.
    let (modal, normal) = (e(1), e(2));
    let r = realized(
        &[modal, normal],
        &[(modal, TopLayer::Modal), (normal, TopLayer::None)],
        &[],
    );
    assert!(
        top_layer_dominates(&r).is_err(),
        "a normal node after a top-layer node must be rejected"
    );
}

#[test]
fn top_layer_rejects_modal_painted_before_fullscreen() {
    // The deviation-#3 PIN: a tail emitted [Modal(rank 3), Fullscreen(rank 0)]
    // is misordered (rank must be NON-DECREASING). This test FAILS if anyone
    // "fixes" the predicate to compare the ENUM DISCRIMINANT
    // (None,Modal,Popover,Tooltip,Fullscreen) — under the discriminant Modal(1)
    // would sort before Fullscreen(4) and look correct, so the predicate would
    // wrongly return Ok and this assert would fail.
    let (modal, full) = (e(1), e(2));
    let r = realized(
        &[modal, full],
        &[(modal, TopLayer::Modal), (full, TopLayer::Fullscreen)],
        &[],
    );
    assert!(
        top_layer_dominates(&r).is_err(),
        "Modal (rank 3) before Fullscreen (rank 0) must be rejected — \
         pins the paint-rank vs enum-discriminant deviation"
    );
}

#[test]
fn top_layer_accepts_well_ordered_tail() {
    // [normal, Fullscreen(0), Tooltip(1), Popover(2), Modal(3)] — the canonical
    // dominant order.
    let (n, fs, tt, pv, md) = (e(1), e(2), e(3), e(4), e(5));
    let r = realized(
        &[n, fs, tt, pv, md],
        &[
            (n, TopLayer::None),
            (fs, TopLayer::Fullscreen),
            (tt, TopLayer::Tooltip),
            (pv, TopLayer::Popover),
            (md, TopLayer::Modal),
        ],
        &[],
    );
    assert!(
        top_layer_dominates(&r).is_ok(),
        "the canonical dominant order passes"
    );
}

// --- #6 paint_order_respects_paint_key --------------------------------------
//
// These two tests check the PREDICATE in isolation: given a hand-built order +
// keys, does it reject descending and accept ascending? They do NOT prove the
// fault-injection property — that protection comes from `realize` calling the
// shared production assembly `painters_z_for_context` (so a 6f-sort regression
// reds the proptest invariant in `invariant_predicates.rs`), not from these
// fixtures. They exist only to pin the predicate's own logic.

/// Build a single-context `Realized` from one context root + its DIRECT painters
/// (in the order given) and an explicit `entity → paint_key` map, so the
/// paint-key-order predicate can be fed a precise order.
fn realized_one_context(
    ctx: Entity,
    painters: &[Entity],
    paint_keys: &[(Entity, (u8, i32))],
) -> Realized {
    let mut painters_of_ctx = HashMap::new();
    painters_of_ctx.insert(ctx, painters.to_vec());
    let paint_key_of: HashMap<Entity, (u8, i32)> = paint_keys.iter().copied().collect();
    let name: HashMap<Entity, String> =
        painters.iter().map(|&en| (en, format!("{en:?}"))).collect();
    Realized {
        nodes: nodes(vec![]),
        context_of: HashMap::new(),
        context_members: HashMap::new(),
        top_layer_of: HashMap::new(),
        painters_of_ctx,
        paint_key_of,
        name_of: name,
    }
}

#[test]
fn paint_key_order_rejects_a_descending_context() {
    // A context whose painters come out [tier 3, tier 1, tier 0] — the shape a
    // REVERSED production 6f sort would yield. Pins that the predicate REJECTS a
    // descending order; the invariant catches a real reversal because `realize`
    // runs the shared production assembly, not because of this unit fixture (#6).
    let (a, b, c) = (e(1), e(2), e(3));
    let r = realized_one_context(e(99), &[a, b, c], &[(a, (3, 5)), (b, (1, 0)), (c, (0, -1))]);
    assert!(
        paint_order_respects_paint_key(&r).is_err(),
        "a context painted in DESCENDING paint_key order must be rejected — \
         this is the reversed-6f-sort fault the invariant must catch"
    );
}

#[test]
fn paint_key_order_accepts_ascending_context() {
    // The canonical 6f order: negative-z (0) < in-flow (1) < auto-positioned
    // (2) < positive-z (3,n) ascending. Equal keys (document order) also pass.
    let (a, b, c, d, e2) = (e(1), e(2), e(3), e(4), e(5));
    let r = realized_one_context(
        e(99),
        &[a, b, c, d, e2],
        &[
            (a, (0, -1)),
            (b, (1, 0)),
            (c, (2, 0)),
            (d, (3, 1)),
            (e2, (3, 2)),
        ],
    );
    assert!(
        paint_order_respects_paint_key(&r).is_ok(),
        "the canonical ascending 6f order passes"
    );
}

// --- #4 all_finite / all_finite_packed --------------------------------------

#[test]
fn all_finite_rejects_nan_and_negative_size() {
    let nan = nodes(vec![node(e(1), Vec2::new(f32::NAN, 10.0))]);
    assert!(all_finite(&nan).is_err(), "NaN size rejected");

    let neg = nodes(vec![node(e(1), Vec2::new(10.0, -5.0))]);
    assert!(all_finite(&neg).is_err(), "negative size.y rejected");

    let ok = nodes(vec![node(e(1), Vec2::new(10.0, 20.0))]);
    assert!(all_finite(&ok).is_ok(), "finite non-negative size passes");
}

/// A finite packed instance with the full-view clip sentinel and POSITIVE
/// height (deviation #2: the y-flip lives in the view uniform).
fn packed(rect_size: [f32; 2]) -> PackedInstance {
    PackedInstance {
        rect_pos: [0.0, 0.0],
        rect_size,
        color: [1.0, 1.0, 1.0, 1.0],
        radius: 0.0,
        clip_min: [f32::NEG_INFINITY, f32::NEG_INFINITY],
        clip_max: [f32::INFINITY, f32::INFINITY],
    }
}

#[test]
fn all_finite_packed_rejects_nan_and_negative_height() {
    let mut nan = packed([10.0, 10.0]);
    nan.color[0] = f32::NAN;
    assert!(all_finite_packed(&[nan]).is_err(), "NaN color rejected");

    // Negative packed height is a real packing bug (height stays POSITIVE).
    let neg = packed([10.0, -10.0]);
    assert!(
        all_finite_packed(&[neg]).is_err(),
        "negative rect_size[1] rejected (deviation #2)"
    );
}

#[test]
fn all_finite_packed_accepts_positive_height_and_sentinel_clip() {
    // Positive height + the ±INFINITY full-view sentinel is VALID (regression-
    // pins deviation #2: the sentinel is the one allowed non-finite).
    let ok = packed([10.0, 10.0]);
    assert!(
        all_finite_packed(&[ok]).is_ok(),
        "positive height + sentinel clip passes"
    );
}

// --- #5 contexts_do_not_interleave ------------------------------------------

#[test]
fn contexts_rejects_an_interleaved_list() {
    // Order [a0, b0, a1]: context A's painted region {a0, a1} is SPLIT by b0
    // (a foreign entity) — its members do not form a contiguous run.
    let (a0, b0, a1) = (e(1), e(2), e(3));
    let r = realized(
        &[a0, b0, a1],
        &[
            (a0, TopLayer::None),
            (b0, TopLayer::None),
            (a1, TopLayer::None),
        ],
        // Context A's region is {a0, a1}; with b0 between them it spans 3 slots
        // for 2 members → interleaved.
        &[(a0, vec![a0, a1]), (b0, vec![b0])],
    );
    assert!(
        contexts_do_not_interleave(&r).is_err(),
        "an interleaved context region must be rejected"
    );
}

#[test]
fn contexts_accepts_contiguous_runs() {
    // Order [a0, a1, b0]: each context's region is a contiguous block.
    let (a0, a1, b0) = (e(1), e(2), e(3));
    let r = realized(
        &[a0, a1, b0],
        &[
            (a0, TopLayer::None),
            (a1, TopLayer::None),
            (b0, TopLayer::None),
        ],
        &[(a0, vec![a0, a1]), (b0, vec![b0])],
    );
    assert!(
        contexts_do_not_interleave(&r).is_ok(),
        "contiguous context regions pass"
    );
}
