# Tier 3 — metamorphic & property invariants (`buiy_verify::invariant`)

**Date:** 2026-06-15
**Status:** landed (Phase 2; `crates/buiy_verify/src/invariant.rs` + `invariant/`)
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

> **As-landed reconciliation.** All three contract deviations below were resolved
> exactly as flagged: (#1) `compose_transform` is a `pub fn` in `layout/systems.rs`;
> (#2) `all_finite_packed` asserts `rect_size[1] ≥ 0` directly because the y-flip
> lives in the per-view uniform; (#3) `tier_rank` was promoted to the public
> `buiy_core::layout::top_layer_paint_rank(TopLayer) -> u8`
> (in `layout/systems.rs`) — the single source of truth consumed by both the
> layout sort and `top_layer_dominates`. The BiDi caret round-trip (#6) consumes
> `cosmic_text::Cursor` (cosmic-text's own type — `use cosmic_text::{Buffer,
> Cursor}` in `invariant/bidi.rs`), not a Buiy struct, so the invariant tests
> Buiy's *integration* of the shaper, not a re-implementation. The module is the
> `invariant.rs` `#[cfg(test)]` proptest harness plus the `invariant/{scene,
> predicates, bidi}.rs` children; `realize` threads a `Scene` through the
> production `context_tree_paint_order` / `partition_top_layer` /
> `top_layer_paint_rank`. All six predicates + their mutation fixtures gate
> headless (no `#[ignore]`); proptest persists any minimized counterexample under
> `crates/buiy_verify/proptest-regressions/` (no file yet — every property is
> green, so no counterexample has been recorded).

The `proptest`-driven middle tier (gate #12): generated scene strategies plus a
fixed set of predicate functions asserting *relations* over the CPU display-list
and shaper output — no golden, no oracle (report §3 Tier 3). It catches
paint-order/transform/top-layer/finiteness/BiDi-caret regressions over an
unbounded fixture space, pure-CPU and deterministic given a seed. This file
specifies the generators, the predicate signatures they feed, and how the
harness itself is verified.

## Contract deviations

Flagged for the synthesizer to reconcile — the shared contract cited stale
`origin/main` facts; the canonical code says otherwise:

1. **`compose_transform` is a `pub fn` in `layout/systems.rs`** (the contract's
   `:3691` line is stale). Signature
   `(&UiTransform, Option<&Translate>, Option<&Rotate>, Option<&Scale>) -> Mat4`,
   compose `T·R·S·M`. Cite the symbol, not a line number (the citation rot fixed in
   the docs audit).
2. **`PackedInstance.rect_size[1]` is POSITIVE on `main`, not negative.** The
   y-flip moved out of the instance into the per-view uniform
   (`render/instance.rs:35`–`:47`: "height is POSITIVE — the y-flip lives in the
   view uniform now"). The contract's "rect_size[1] deliberately negative by
   y-flip" is stale. Consequence is *favorable*: `all_finite` can assert
   `rect_size[1] ≥ 0` directly on `PackedInstance` — no un-flip needed. We keep
   the `DrawData`/`ExtractedNode.size ≥ 0` assertion as the primary
   non-negativity check and add the packed check as a stricter sibling.
3. **`tier_rank` was a private closure, and the `TopLayer` enum's `derive`d order
   is NOT the paint order.** `tier_rank` lived inside a layout system as a local
   `fn` (`Fullscreen→0, Tooltip→1, Popover→2, Modal→3, None→u8::MAX`), while
   `enum TopLayer` declares `None, Modal, Popover, Tooltip, Fullscreen`
   (`layout/types.rs`) — so `#[derive(Ord)]` would give the WRONG dominance.
   `top_layer_dominates` must compare via the documented tier rank, not enum
   discriminant. This spec required promoting `tier_rank` to a
   `pub fn buiy_core::layout::top_layer_paint_rank(TopLayer) -> u8` (single
   source of truth, consumed by both the layout sort and this invariant); the
   small `buiy_core` surface add landed (`top_layer_paint_rank` in
   `layout/systems.rs`).

## Module shape

`crates/buiy_verify/src/invariant/` — pure-CPU, no GPU, no window:

```
invariant/
  mod.rs          // re-exports; the `#[cfg(test)]` proptest harness lives here
  scene.rs        // Scene model + proptest Strategy generators (shrinkable)
  predicates.rs   // the predicate fns — each is `pub`, takes borrowed data, returns Result
  bidi.rs         // BiDi caret round-trip generators + predicates (shaper-coupled)
```

No new dependency. `proptest = "1"` is already a workspace dep
(`Cargo.toml:51`) and `buiy_verify` already pulls it
(`crates/buiy_verify/Cargo.toml:13`), alongside `buiy_core + bevy + serde`. Tier
3 adds **zero** crates, so no `cargo deny check` entry is needed; the determinism
font mode and GPU pieces that *do* add deps live in the `determinism`/`metric`
files, not here.

## Scene generators (`scene.rs`)

A generator produces a *headless* scene description that the same CPU extract
path Tier 2 uses can turn into an `ExtractedNodes` list, with no GPU. We
generate an abstract `Scene` (not raw Bevy `World`s) so shrinking yields a
minimal, printable counterexample and the predicates stay world-agnostic.

```rust
/// A generated node in a bounded hierarchy. `name` is the stable identity used
/// in diagnostics (mirrors Tier 2's `Name`-based dump — never raw `Entity` bits).
pub struct SceneNode {
    pub name: String,                 // unique within a Scene ("n0", "n1", …)
    pub children: Vec<SceneNode>,
    pub z_index: Option<i32>,         // positioned z; drives stacking + paint order
    pub isolation: bool,              // forces a stacking context
    pub top_layer: TopLayer,          // None for the bulk; non-None ⇒ escapes
    pub transform: GenTransform,      // the Translate/Rotate/Scale/Matrix inputs
    pub size: (f32, f32),             // logical-px box (always finite, ≥ 0 by gen)
    pub background: Option<TokenRef>, // resolved color token (never the magenta sentinel)
}

pub struct Scene { pub roots: Vec<SceneNode> }

/// Realize a `Scene` through the production CPU paint-order assembly
/// (`assemble_context_tree` / `partition_top_layer`) into the flat paint-ordered
/// node list the predicates assert on. No GPU, no readback.
pub fn realize(scene: &Scene) -> ExtractedNodes;
```

**Strategy budget (bounded, shrink-to-minimal).** Generators are explicitly
bounded so the property space is finite-depth and shrinking terminates fast:

```rust
pub struct SceneParams {
    pub max_depth: u32,     // default 4  — hierarchy depth cap
    pub max_breadth: u32,   // default 4  — children per node cap
    pub max_nodes: u32,     // default 24 — total-node guard (prevents blow-up)
    pub p_stacking: f64,    // default 0.3 — P(node forms a context via z/isolation)
    pub p_top_layer: f64,   // default 0.1 — P(node escapes to the top layer)
}
pub fn arb_scene(p: SceneParams) -> impl Strategy<Value = Scene>;
```

- Depth/breadth use `proptest::collection::vec(inner, 0..=breadth)` recursed via
  `Strategy::prop_recursive(depth, max_nodes, breadth, …)` so proptest's built-in
  recursion guard bounds the tree and shrinks toward the empty/shallow scene.
- `z_index` is drawn from a small set `{-1, 0, 1, 2}` (the interesting
  negative/zero/positive partition) rather than full `i32`, keeping shrinks
  legible while still exercising the negative-z-first rule.
- `GenTransform` draws from the `compose_transform` input space: a `Translate`
  with px components in `-512.0..512.0`, a `Rotate` quaternion built from an
  axis-angle (angle in `0..2π`), and a `Scale` in `0.1..8.0` per axis — values
  chosen finite and away from `0` so round-trips are well-conditioned. The
  identity case (all `None`) is always reachable for shrinking.
- `top_layer` is drawn from all five `TopLayer` variants (skewed to `None`);
  every variant must be reachable so `top_layer_dominates` exercises the full
  tier rank, not just `Modal`.
- Uniqueness of `name` is enforced by a post-generation pass that renames in
  pre-order (`n0..nK`), so a shrunk counterexample is reproducible and printable.

## Predicate functions (`predicates.rs`)

Each is a free `pub fn` taking borrowed data and returning
`Result<(), Violation>` (not a bare `bool`) so a failing property prints *which*
relation broke and the offending names/indices — the seed plus this message
reproduces it. `Violation` is a `thiserror`-free plain struct
(`{ rule: &'static str, detail: String }`) to keep the dep surface at zero.

```rust
/// #1 — paint order is a TOTAL order over painted entities.
/// No entity appears twice; the sort is stable: two nodes with an equal paint
/// key keep document (generation) order. Mirrors the non-re-sorting contract of
/// `ExtractedNodes.nodes` (render/extract.rs:139 "Never re-sorted by render").
pub fn paint_order_is_total(nodes: &ExtractedNodes) -> Result<(), Violation>;

/// #2 — transform round-trips on `compose_transform` (layout/systems.rs).
/// Asserts three metamorphic relations on the COMPOSED Mat4, within `EPS`:
///   • translate(d) · translate(-d) ≈ identity
///   • rotate(2π)                    ≈ identity
///   • scale(k) applied to a unit box scales every geometry component by k and
///     nothing else (off-diagonals stay 0).
/// Operates on `compose_transform` outputs, NOT `layout/translate.rs` (the Taffy
/// style translator, which has no Mat4 — report §3 Tier 3).
pub fn transform_roundtrips(t: &GenTransform) -> Result<(), Violation>;

/// #3 — top-layer dominance. Every `top_layer != None` node paints AFTER every
/// normal-stacking node, and the escaped tail is ordered by paint rank
/// Fullscreen < Tooltip < Popover < Modal — compared via the promoted
/// `buiy_core::layout::top_layer_paint_rank` (layout/systems.rs), never
/// the enum discriminant (see deviation #3).
pub fn top_layer_dominates(nodes: &ExtractedNodes) -> Result<(), Violation>;

/// #4 — finiteness / non-negativity. Every `ExtractedNode.size.{x,y} ≥ 0` and
/// finite (the un-flipped logical box, render/extract.rs:73). The companion
/// `all_finite_packed` asserts every `PackedInstance` field is finite and
/// `rect_size[1] ≥ 0` directly — valid because the y-flip now lives in the view
/// uniform, so packed height stays positive (render/instance.rs:46, deviation #2).
pub fn all_finite(nodes: &ExtractedNodes) -> Result<(), Violation>;
pub fn all_finite_packed(packed: &[PackedInstance]) -> Result<(), Violation>;

/// #5 — z-isolated containment (report §3): no entity of stacking context A
/// appears between two entities of context B in the flattened order. Asserted on
/// the same realized list, using the per-node context membership the generator
/// recorded. Guards against subtree leakage across an `isolation`/z boundary.
pub fn contexts_do_not_interleave(nodes: &ExtractedNodes, scene: &Scene)
    -> Result<(), Violation>;
```

### BiDi caret round-trip (`bidi.rs`, predicate #6)

Gate #12's named text invariant, on the **landed shaper** (`cosmic_text::Buffer`
laid out through the production text stack — same path as
`tests/text_shaping_snapshots.rs`). Relations over the shaper output, no
rasterizer:

```rust
/// Generate a mixed-direction string: alternating LTR (Latin) and RTL
/// (Arabic/Hebrew) runs of bounded length, plus neutrals — the BiDi stress space
/// the shaping `.snap` fixtures pin positions for, now exercised generatively.
pub fn arb_bidi_text(max_runs: u32, max_run_len: u32) -> impl Strategy<Value = String>;

/// #6a — logical↔visual caret round-trip is identity. For every grapheme
/// boundary, mapping the logical `Cursor { line, index }` to a visual x via the
/// run's glyph `start..end` (LayoutGlyph logical byte span) and `x` (visual
/// position), then hit-testing that x back, recovers the SAME logical cursor.
/// #6b — within one `LayoutRun`, visual caret order is MONOTONIC in logical
/// order for an LTR run (`run.rtl == false`) and strictly REVERSED for an RTL
/// run (`run.rtl == true`).
/// #6c — the run partition COVERS every codepoint exactly once (no gap, no
/// overlap across `Buffer::layout_runs()`).
pub fn bidi_caret_roundtrips(text: &str, metrics: Metrics) -> Result<(), Violation>;
```

The mapping uses cosmic-text's own `Buffer::layout_runs()` →
`LayoutRun { line_i, rtl, glyphs: [LayoutGlyph { start, end, x, … }] }` (the
exact structure `text/extract.rs:429`/`:797` consumes) and `cosmic_text::Cursor
{ line, index }` (re-exported at `text/components.rs:10`, not a Buiy struct — it
is cosmic-text's own type), so the invariant tests Buiy's *integration* of the
shaper, not a re-implementation of BiDi.

## The proptest harness (`mod.rs`)

One `proptest! { … }` block per predicate, each a `#[test]` so failures are
isolated and `cargo test -p buiy_verify` reports them individually. Default
config: `ProptestConfig { cases: 256, max_shrink_iters: 4096, .. }`, raised for
the cheap pure-CPU predicates.

```rust
proptest! {
    #[test]
    fn prop_paint_order_total(scene in arb_scene(SceneParams::default())) {
        let nodes = realize(&scene);
        prop_assert!(paint_order_is_total(&nodes).is_ok(),
            "{:?}", paint_order_is_total(&nodes).unwrap_err());
    }
    // … one per predicate #1–#6 …
}
```

**Failing-seed reproduction.** Rely on proptest's built-in persistence:
`proptest-regressions/invariant/<file>.txt` is committed, so any minimized
counterexample re-runs deterministically on the next `cargo test` (the
project's existing `cargo test` gate picks it up; no extra wiring). The
`Violation` message names the rule and the offending node names, and the shrunk
`Scene`/`String` prints via `Debug` — together these reproduce a failure from
the seed line alone. Document the persistence dir in the plan so it is committed,
not gitignored.

## Verification (testing the harness itself)

A property suite that never fails is worthless; we verify each predicate has
teeth with **mutation fixtures** — hand-built scenes that VIOLATE exactly one
relation, asserted to be rejected, plus a known-good control asserted to pass:

- `paint_order_is_total`: a fixture whose realized list duplicates one entity ⇒
  `Err`; the generator's output on a fixed seed ⇒ `Ok`. **Landed note:** the
  stability clause (a reversed-equal-key pair ⇒ `Err`) is NOT assertable at this
  predicate's input boundary — the predicate sees only the already-realized flat
  list, in which the stable z-tier sort has *already* run, so an equal-key pair
  is indistinguishable from a legitimately-ordered one. Stability is instead
  exercised by `realize`'s production-mirror stable sort itself; the predicate
  asserts only totality (no duplicates). The waiver lives in the predicate's
  code comment.
- `transform_roundtrips`: feed a deliberately mis-composed matrix (e.g. `S·R·T`
  instead of `T·R·S·M`) ⇒ `Err`; identity inputs ⇒ `Ok`. Pin `EPS` and add a
  boundary fixture at `EPS ± 1 ULP`.
- `top_layer_dominates`: a fixture with a `Modal` painted before a `Fullscreen`
  (rank 3 before rank 0) ⇒ `Err`; a normal node after a top-layer node ⇒ `Err`.
  This fixture also pins deviation #3 — it FAILS if anyone "fixes"
  `top_layer_dominates` to use the enum discriminant.
- `all_finite` / `all_finite_packed`: inject a `NaN` size and a negative
  `size.y` ⇒ `Err`; a positive packed `rect_size[1]` ⇒ `Ok` (regression-pins
  deviation #2).
- `contexts_do_not_interleave`: a hand-built interleaved list ⇒ `Err`.
- `bidi_caret_roundtrips`: the six shaping-snapshot scripts (Latin, Arabic,
  Devanagari, CJK, emoji-ZWJ, mixed-BiDi) as known-good controls ⇒ `Ok`; an
  off-by-one caret-map fixture ⇒ `Err`.

These mutation fixtures are ordinary `#[test]`s alongside the `proptest!` blocks,
so the harness's own correctness rides the same `cargo test -p buiy_verify` gate
(no GPU, no `#[ignore]`). They are the Tier-3 analogue of the half-size sign-bug
regression in `render_instance.rs` — the predicate must reject the known bug.

## Sources

- Report: `docs/reports/2026-06-14-visual-bug-detection-strategy.md` §3 Tier 3
  (lines 118–133), cross-cutting §"Animation/temporal determinism".
- Code: `compose_transform` (`crates/buiy_core/src/layout/systems.rs`),
  `transform_matrix_to_mat4`, `top_layer_paint_rank` (promoted from the private
  `tier_rank`, same file); `ExtractedNode`
  (`render/extract.rs:65`), `ExtractedNodes` (`:139`), `assemble_context_tree`
  (`:206`), `partition_top_layer` (`render/top_layer.rs:17`); `PackedInstance`
  (`render/instance.rs:40`), `packed_to_raw` (`render/buckets.rs:121`);
  `enum TopLayer` (`layout/types.rs:1265`); shaper structures consumed at
  `text/extract.rs:429`/`:797`, `cosmic_text::Cursor` (re-exported at
  `text/components.rs:10`); precedent
  `tests/text_shaping_snapshots.rs`, `tests/render_instance.rs`.
- Prior-art: `docs/prior-art/wgpu-testing/lessons.md` (lower tiers carry the
  correctness load; goldens prove "no-change, not correct").
