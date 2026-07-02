# Nested degraded effect-group forward-composite — plan

**Spec:** [`docs/specs/2026-07-01-nested-degraded-forward-composite-design.md`](../specs/2026-07-01-nested-degraded-forward-composite-design.md)
**Branch:** `feat/nested-degraded-forward-composite` (off `origin/main`)
**Gate:** headless `cargo test -p buiy_core` (unit + `plan_allocation` pin) + GPU
`--ignored` lane (RX 6700 XT / RADV locally, pinned lavapipe in CI); fmt + clippy.

Fix **case A** — a degraded nested effect group whose immediate parent kept a
target — by injecting its folded members into the parent's `Rgba16Float` target
at node step-2a. Also removes the `debug_assert!(false)` that currently panics
debug builds on *any* nested degraded group. Degraded chains `(None,None)` and
kept-child-under-degraded-parent `(Some,None)` are deferred (documented + filed).

TDD, RED-first at each slice. Small verifiable units; commit per green slice.

## S1 — Generalize the fold (compositor.rs) + flip the headless unit test

**RED first.** In `crates/buiy_core/tests/render/render_compositor.rs`, re-point
`degraded_fold_skips_nested_group_in_release_path`:
- Remove the `if cfg!(debug_assertions) { return; }` early-return (the default
  gate runs with `debug_assertions` ON — the flipped asserts must execute there).
- Assert a NESTED (`parent = Some`) degraded group's quad alpha **IS** folded
  (`[ALPHA_FLOAT_OFFSET] == source × opacity`) and its range is **NOT** merged
  into `quad_flat`/`glyph_flat`.
- Add a glyph-tier variant (`color[3]` at `GLYPH_ALPHA_FLOAT_OFFSET`), mirroring
  `degraded_fold_multiplies_glyph_alpha_at_offset_11`.
- Add a `(None,None)` chain case: a nested group whose parent is also degraded is
  folded-but-not-merged (still not drawn).
Rename the test to reflect the new contract (e.g. `degraded_fold_folds_nested_alpha_but_never_merges_it`).
Run `cargo test -p buiy_core --test render degraded_fold` → RED (today's fold
debug-asserts / skips nested).

**GREEN.** In `crates/buiy_core/src/render/compositor.rs`:
- Rename `fold_root_degraded_into_flat` → `fold_degraded_groups` (update the one
  caller in `prepare_effect_groups`).
- Fold **every** degraded group's own opacity into its members' alpha (gated on
  `fold_quad`/`fold_glyph` as today) — drop the `group.parent.is_some()`
  `debug_assert!/continue`.
- Gate the two flat-range **merges** on `group.parent.is_none()` (ROOT only).
- Rewrite the fn doc block + the `DegradedGroup.parent` doc for the widened
  charter (folds all degraded; merges flat only for roots; nested handled/skipped
  by the node).
Run the unit test → GREEN. `cargo test -p buiy_core` (headless, no GPU) stays green.

## S2 — Pin the case-A allocation (headless plan_allocation test)

In `render_compositor.rs` (or the compositor unit tests), add a pure-function test
with two `(extent, EffectReason::OPACITY)` pairs matching the S4 GPU fixture's
**bucketed** extents. Concrete, provably-different buckets (the gate's nit — the two
bounds must NOT both round to the same pow2). Note each group's bounds are SEEDED
with the group's own box at its (0,0) origin (extract.rs) and then grown by its own
direct members, so INNER's bounds = (0,0)..(inner_fill.max):

- `outer` bounds **60×60** → `next_power_of_two(60) = 64` → `(64,64)` →
  `target_bytes = 64·64·8 = 32768`.
- `inner` bounds **24×24** (origin ∪ a 16×16 fill at (8,8)) →
  `next_power_of_two(24) = 32` → `(32,32)` → `target_bytes = 32·32·8 = 8192`.
- budget **33000** ∈ `[32768, 32768+8192) = [32768, 40960)`.

Assert `plan_allocation(&[(uvec2(64,64), OPACITY), (uvec2(32,32), OPACITY)], 33000)
== vec![true, false]` (outer kept, inner degraded → case A). Deterministic, no GPU.
Pins the budget↔extent relationship so a later bucket/threshold change that would
silently move S4 onto a deferred path fails here loudly. Also assert the window
edges: budget `< 8192` (below both) → `[false,false]`; budget `≥ 40960` (above
both) → `[true,true]`; and `8192 ≤ budget < 32768` → still `[false,false]` (outer,
the larger, degrades whenever the pair doesn't fit and outer alone exceeds budget).

## S3 — Node injection (node.rs step-2a)

In `crates/buiy_core/src/render/node.rs`, step-2a nested-composite loop
(~`node.rs:255-305`): replace the single
`let (Some(src), Some(parent_tex)) = (…) else { continue }` guard with a match on
`(child_target, parent_target)`:
- `(Some(src), Some(parent_tex))` → existing composite (unchanged).
- `(None, Some(parent_tex))` → **NEW**: re-fetch the group quad/glyph pipelines
  (`prepared.quad_pipeline`/`glyph_pipeline` via `pipeline_cache`) + the page-0
  atlas bind group (`AtlasGpu::coverage_bind_group()`), build a per-pass group-view
  UBO from the **parent's** `placements[parent_idx].target_view_columns`, open a
  `LoadOp::Load` pass into `parent_tex.default_view`, and draw the child's
  `placements[gi].instance_range` (quads) + `.glyph_range` (glyphs). Mirror
  step-1's per-half pipeline-readiness skips + the empty-range skip. Opacity is
  already folded → no per-draw opacity.
- `(Some(src), None)` / `(None, None)` → `continue` (deferred; unchanged behavior).
Update the step-1 degraded-skip comment (`node.rs:167-177`) and the step-2a
comment to reflect that a nested degraded child (case A) is now injected, not
skipped. `is_pure_backdrop_filter` groups stay skipped at the top of the loop.

No dedicated test here — S4's GPU test is the observable check.

## S4 — GPU positive test (rebuild the nested test into a case-A positive)

**RED first (after S1, before/with S3).** In
`crates/buiy_core/tests/render_degraded_group_gpu.rs`, rebuild
`nested_degraded_group_does_not_corrupt_parent` into a case-A positive:
1. Give `outer` (the `Opacity(0.8)` group) its **own** larger `Background` (a 60×60
   fill at (0,0)) that **spatially contains** the inner (`inner_fill` 16×16 at
   (8,8), so inner bounds = (0,0)..(24,24)) so `outer.bounds` ⊋ `inner.bounds` and
   `target_bytes(outer) = 32768 > target_bytes(inner) = 8192`. Keep inner ⊆ outer's
   own-paint box (avoids the pre-existing parent-target-undersizing clip).
2. Set the budget into `[target_bytes(outer), target_bytes(outer) +
   target_bytes(inner))` so `plan_allocation` keeps outer, degrades only inner
   (the exact budget cross-checked by the S2 pin).
3. Remove the `if cfg!(debug_assertions) { return; }` guard (no debug_assert now).
4. Assert case A was exercised: read the render world's `PreparedEffectTargets`
   (via the RenderApp sub-app / an added support accessor) and assert exactly one
   group has a target (outer=`Some`) and the inner is `None`. If direct access is
   impractical, the S2 pin + the interior pixel assertion together stand in — but
   prefer the direct observable so the pixel test can't drift onto a deferred path.
5. Keep the corner-clean assertion (inner not mis-placed at window level).
6. Add the interior assertion at the inner fill's device position: red is
   **present** (clearly lit above the outer-only region) **and** at roughly the
   expected composed level (`inner.opacity` folded into the outer target, then the
   outer target composited at `outer.opacity`), within an adapter-tolerant band
   computed via `composite_src_over` + linear→sRGB the same way the sibling GPU
   tests do (`render_degraded_group_gpu.rs` root cases).
Rename to a positive name (e.g. `nested_degraded_child_forward_composites_into_parent`).
Run `cargo test -p buiy_core --test render_degraded_group_gpu -- --ignored
--test-threads=1` → RED before S3 (inner vanishes → interior assert fails), GREEN
after S3.

## S5 — Docs

- `docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 2.3` —
  flip the "As landed (R2)" nested paragraph: nested case A landed; `(Some,None)` /
  chain deferred.
- `docs/plans/follow-ups.md` — flip the nested-degraded entry to LANDED (case A);
  add the three follow-ups (chain, kept-child-under-degraded-parent,
  parent-target undersizing) with the corrected frequency note.
- `docs/README.md` — register this spec + plan.

## Full gate before PR

```sh
cargo fmt --all -- --check
cargo clippy -p buiy_core --all-targets --locked -- -D warnings
cargo test -p buiy_core                                   # headless: unit + plan_allocation pin
cargo test -p buiy_core --test render_degraded_group_gpu -- --ignored --test-threads=1
# broad GPU sanity (both legs the CLAUDE.md GPU lane runs):
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

## Done when

- Headless unit tests (flipped fold + `plan_allocation` pin) green in the default
  (debug-assertions ON) gate.
- The rebuilt GPU test proves case A (outer=`Some`/inner=`None`) and the inner
  fill is present at the composed level; corner clean.
- No debug-build panic on any nested degraded group.
- fmt + clippy clean; effect-compositor.md § 2.3 + follow-ups.md + docs/README
  updated; the three follow-ups filed.
- Fresh-context plan gate + execution gate passed.
