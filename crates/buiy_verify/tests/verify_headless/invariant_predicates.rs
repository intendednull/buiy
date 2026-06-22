//! Task 2.9 — the Tier-3 `proptest!` harness for predicates #1–#5. One block
//! per predicate, each a `#[test]` so failures are isolated and report
//! individually. A failing case's MINIMIZED counterexample is persisted in
//! `invariant_predicates.proptest-regressions` (committed, not gitignored) so it
//! re-runs deterministically on the next `cargo test`.
//!
//! These exercise the predicates over the UNBOUNDED generated scene space; the
//! teeth (that each predicate actually REJECTS a known break) are proven by the
//! hand-built mutation fixtures in `invariant_mutations.rs`.

use buiy_core::render::instance::pack_extracted;
use buiy_verify::invariant::{
    SceneParams, all_finite, all_finite_packed, arb_scene, arb_transform,
    contexts_do_not_interleave, paint_order_is_total, paint_order_respects_paint_key, realize,
    realize_full, top_layer_dominates, transform_roundtrips,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, max_shrink_iters: 4096, ..ProptestConfig::default() })]

    /// #1 — the realized paint order never lists an entity twice.
    #[test]
    fn prop_paint_order_total(scene in arb_scene(SceneParams::default())) {
        let nodes = realize(&scene);
        prop_assert!(
            paint_order_is_total(&nodes).is_ok(),
            "{}", paint_order_is_total(&nodes).unwrap_err()
        );
    }

    /// #2 — the production `compose_transform` round-trips on every generated
    /// transform (translate∘-translate ≈ I, rotate(2π) ≈ I, pure diagonal scale).
    #[test]
    fn prop_transform_roundtrips(t in arb_transform()) {
        prop_assert!(
            transform_roundtrips(&t).is_ok(),
            "{}", transform_roundtrips(&t).unwrap_err()
        );
    }

    /// #3 — top-layer nodes paint after every normal node, tail ranked
    /// Fullscreen<Tooltip<Popover<Modal.
    #[test]
    fn prop_top_layer_dominates(scene in arb_scene(SceneParams::default())) {
        let r = realize_full(&scene);
        prop_assert!(
            top_layer_dominates(&r).is_ok(),
            "{}", top_layer_dominates(&r).unwrap_err()
        );
    }

    /// #4 — every realized node size is finite + non-negative, and the packed
    /// instances are all finite with positive height.
    #[test]
    fn prop_all_finite(scene in arb_scene(SceneParams::default())) {
        let nodes = realize(&scene);
        prop_assert!(all_finite(&nodes).is_ok(), "{}", all_finite(&nodes).unwrap_err());

        let packed: Vec<_> = nodes.nodes.iter().map(pack_extracted).collect();
        prop_assert!(
            all_finite_packed(&packed).is_ok(),
            "{}", all_finite_packed(&packed).unwrap_err()
        );
    }

    /// #5 — no stacking context interleaves another in the flattened order.
    #[test]
    fn prop_contexts_no_interleave(scene in arb_scene(SceneParams::default())) {
        let r = realize_full(&scene);
        prop_assert!(
            contexts_do_not_interleave(&r).is_ok(),
            "{}", contexts_do_not_interleave(&r).unwrap_err()
        );
    }

    /// #6 — within every context, painters come out non-decreasing in the
    /// PRODUCTION `paint_key` (the observable 6f tier sort). `realize` builds each
    /// context by CALLING the shared production assembly `painters_z_for_context`,
    /// so a regression in that production code (reversing the sort, descending
    /// past a nested context) reds this property; the other five predicates are
    /// all order-insensitive within a context and miss it (testing-audit #6).
    #[test]
    fn prop_paint_order_respects_paint_key(scene in arb_scene(SceneParams::default())) {
        let r = realize_full(&scene);
        prop_assert!(
            paint_order_respects_paint_key(&r).is_ok(),
            "{}", paint_order_respects_paint_key(&r).unwrap_err()
        );
    }
}
