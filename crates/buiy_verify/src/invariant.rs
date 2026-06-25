//! Tier 3 — metamorphic & property invariants (invariants.md).
//!
//! The `proptest`-driven middle rung of the verification pyramid: generated
//! scene strategies plus a fixed set of predicate functions asserting
//! *relations* over the CPU display-list and shaper output — no golden, no
//! oracle. It catches paint-order / transform / top-layer / finiteness /
//! BiDi-caret regressions over an unbounded fixture space, pure-CPU and
//! deterministic given a seed (gate #12).
//!
//! The [`scene`] module holds the abstract [`Scene`] model + the `proptest`
//! generators ([`arb_scene`]), plus [`realize`], which threads a `Scene`
//! through the PRODUCTION CPU paint-order assembly
//! ([`context_tree_paint_order`](buiy_core::render::extract::context_tree_paint_order),
//! [`partition_top_layer`](buiy_core::render::top_layer::partition_top_layer),
//! and the promoted
//! [`top_layer_paint_rank`](buiy_core::layout::top_layer_paint_rank)) into the
//! flat paint-ordered node list the predicates assert on — no GPU, no `World`.
//!
//! The predicate functions, their `proptest!` harness, and the mutation
//! meta-tests land in their own tasks (2.9, 2.10); each predicate is a free
//! `pub fn` taking borrowed data and returning `Result<(), Violation>` so a
//! failing property prints *which* relation broke and the offending
//! names/indices. The harness + meta-tests live in the test crate
//! (`crates/buiy_verify/tests/invariant_*.rs`), not here, so a property failure
//! re-runs from its committed `proptest-regressions/` seed under the ordinary
//! `cargo test` gate.

pub mod scene;
pub use scene::{
    GenTransform, Realized, Scene, SceneNode, SceneParams, arb_scene, arb_transform, realize,
    realize_full,
};

pub mod predicates;
pub use predicates::{
    EPS, Violation, all_finite, all_finite_packed, contexts_do_not_interleave, mat4_is_identity,
    mat4_is_pure_scale, paint_order_is_total, paint_order_respects_paint_key, top_layer_dominates,
    transform_roundtrips,
};

pub mod bidi;
pub use bidi::{arb_bidi_text, bidi_caret_roundtrips, caret_in_cluster};

pub mod content_presence;
pub use content_presence::{content_is_present, glyph_census};
