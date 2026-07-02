# Nested degraded effect-group forward-composite — design

**Date:** 2026-07-01
**Status:** target state (spec)
**Area:** render / effect-compositor (GPU)
**Spec it closes (partially):** `docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md` § 2.3
**Tracker:** `docs/plans/follow-ups.md` § "Render — nested degraded effect group must forward-composite into the parent target (not the window)"

## Problem

Under render-target-pool (RT-pool) budget pressure, the effect-compositor
*degrades* the lowest-cost effect groups: instead of allocating an off-screen
`Rgba16Float` target, a degraded group forward-composites its subtree directly,
folding its `opacity` per-instance (effect-compositor.md § 2.3). R2 landed this
for **ROOT** degraded groups (`parent == None`): `fold_root_degraded_into_flat`
folds the group's opacity into its members' alpha and merges its instance ranges
into `flat_ranges`/`glyph_flat_ranges`, so the window flat pass paints it.

A **nested** degraded group (`parent == Some`) is not handled, and the current
containment is worse than a silent vanish: `fold_root_degraded_into_flat`
`debug_assert!(false, …)`s on **any** nested degraded group, so in a **debug
build (including the default test gate)** a nested group that degrades **panics
in prepare**. In release it is left untouched → its subtree **vanishes**. § 2.3
mandates a degraded group forward-composite "directly into its parent target";
for a nested group that parent target is another group's `Rgba16Float` target,
not the window, so R2's flat-merge is the wrong mechanism for it (it would paint
in the window's space/clip, and the parent would then composite a target the
child never reached).

This slice has **two wins**: (1) a nested degraded child whose parent kept its
target ("case A", defined below) forward-composites correctly instead of
vanishing; (2) removing the `debug_assert!` makes **every** nested degraded case
(case A + the deferred shapes) skip cleanly in debug rather than panicking —
debug behavior matches release.

## Key structural facts (verified against the code)

- **Group bounds grow by OWN DIRECT members only.** A node tags its NEAREST
  enclosing effect group and enlarges only *that* group's bounds
  (`extract.rs:1685-1689`). The `painted_bounds` helper that would add a nested
  child's extent to its parent's bounds is **unused in production**
  (`compositor.rs:80` — a comment reference only, no prod caller). So a parent
  group's bounds = the union of its **own direct** paint, which can be *smaller*
  than a content-bearing nested child.
- **Therefore degrade order follows the direct-paint distribution, NOT nesting
  depth.** `plan_allocation` (compositor.rs) degrades **lowest-`target_bytes`
  first** (opacity-only before structural; smaller before larger within a class).
  A bare `Opacity` **wrapper** parent (no direct paint) has ~empty bounds → tiny
  `target_bytes` → it degrades **before** its content-bearing child. Which nested
  group degrades first depends on where the direct paint sits.
  - **Case A** (this slice): the parent carries direct paint that is both larger
    (higher `target_bytes`) than AND spatially contains the nested child → the
    smaller child degrades while the parent keeps its target. E.g. a card with a
    substantial background (parent `Opacity` group) containing a small nested
    `Opacity` group (a fading badge). Under pressure the badge degrades first,
    the card keeps its target.
  - The **bare-wrapper shapes** — parent degrades first, child kept `(Some,None)`;
    or both degrade, `(None,None)` chain — are **deferred** (see below). They are
    often the *more common* real shape, so case A is a correctness-completeness +
    strictly-better improvement for its shape, **not** "the fix for the common
    degraded case." The broader shapes get tracked follow-ups.
- **Ranges are disjoint per group; a group is root-xor-nested.** Each instance
  carries its nearest group (`extract.rs:137-138`); `RangePartitioner::push`
  (`buckets.rs`) extends exactly that group's contiguous range. Folding a group's
  own disjoint range touches each instance **exactly once** — root and nested
  folds cannot double-apply. Folding *every* degraded group's own opacity is safe.
- **Case A needs NO cumulative opacity.** Non-degraded, a child's members reach
  the window at `member_alpha × child.opacity × parent.opacity` (child target
  composites into parent at `child.opacity`, parent target composites at
  `parent.opacity`). Case-A injection folds `member_alpha × child.opacity` into
  the buffer and draws it into the parent target (`LoadOp::Load`); the parent's
  own composite then supplies `× parent.opacity` **for free**. Identical effective
  factor. (Same documented § 2.3 approximation as R2: overlapping members *within*
  a degraded group flat-fold individually rather than sampling-as-one-unit — not a
  new error.)
- **The fold must be prepare-time into the buffer.** The group quad/glyph
  pipelines carry no per-draw opacity uniform; opacity lives only as the
  per-instance alpha in the shared buffer. A draw-time fold isn't expressible
  without a shader change.

## Decision

Handle **case A** — a degraded nested group whose **immediate parent kept a
target** — by injecting the child's folded members into the parent's
`Rgba16Float` target at node step-2a. Concretely:

1. **`compositor.rs` — generalize the fold (rename `fold_root_degraded_into_flat`
   → `fold_degraded_groups`).** Fold **every** degraded group's own opacity into
   its members' alpha (unchanged mechanics + the existing two-gate `fold_*` /
   `merge_*` dirty discipline). Gate the flat-range **merge** on `parent.is_none()`
   (ROOT only) — a nested group's members are never merged into `flat_ranges`; the
   node injects them (case A) or skips them (deferred). **Remove the nested
   `debug_assert!/continue`** — this also stops debug builds panicking on the
   deferred nested shapes (they now skip like release). A nested group's
   own-opacity fold is harmless when the node later skips it: folded-but-undrawn
   instances (neither merged to flat nor injected) contribute nothing.

2. **`node.rs` — step-2a injection.** Replace the single
   `let (Some(src), Some(parent_tex)) = … else { continue }` guard
   (`node.rs:265-270`) with a match on `(child_target, parent_target)`:
   - `(Some(src), Some(parent_tex))` → **existing** composite (child target → parent target).
   - `(None, Some(parent_tex))` → **NEW (case A)**: draw the child's
     `instance_range` / `glyph_range` directly into `parent_tex` (`LoadOp::Load`,
     preserving the parent's step-1 content), using the **parent's**
     `target_view_columns` (the child's members are at logical positions inside
     the parent's bounds; the parent view maps logical → parent-target texels).
     Opacity is already folded → no per-draw opacity.
   - `(Some(src), None)` and `(None, None)` → `continue` (Deferred).
   **Re-fetch the group pipelines + atlas bind group in step-2a** — the step-1
   block scopes `group_quad_pipeline`/`group_glyph_pipeline`/`atlas_bind_group`
   locally (`node.rs:146-156`); the step-2a loop is a separate `if let` scope, so
   the injection arm must re-fetch them (`prepared.quad_pipeline`/`glyph_pipeline`
   via `pipeline_cache.get_render_pipeline` + `AtlasGpu::coverage_bind_group()`)
   and build a fresh per-pass group-view UBO from the parent's columns. Mirror
   step-1's **per-half pipeline-readiness skips** (an async-compile frame skips the
   injection half cleanly). `is_pure_backdrop_filter` groups are already skipped at
   the top of the loop.

3. **Ordering & idempotency.** Injection happens at the child's **post-order
   position** inside step-2a — after the parent's step-1 members are drawn and in
   the same sibling order the existing composite uses. Because step 1 (a complete
   loop) fills every kept target before step-2a begins, and post-order visits a
   child before its parent, the injected child lands in the parent target
   **before** the parent composites upward — so it rides along in the parent's
   composite (correct at any grandparent depth **provided the ancestors above the
   immediate parent themselves composite normally**; a degraded ancestor higher up
   is the deferred `(Some,None)` level — the child then vanishes with its parent,
   no worse than today). The fold keeps R2's gate-#14 discipline (runs only under
   `allocate.iter().any(|a| !a)`, re-tints only on the per-tier buffer-repack
   signal); step 1 clears + redraws the parent target each frame, and step-2a
   re-injects the retained folded buffer → idempotent, zero-upload steady state.

## Deferred (documented, "no worse than today")

- **`(None,None)` degraded chain** (nested group whose immediate parent is also
  degraded). Skipped cleanly (still vanishes, as today). It needs **cumulative
  opacity** (the child gets no free parent composite) AND has a **z-order hazard**
  (post-order visits a descendant before its ancestor, so injecting both into a
  shared nearest-kept target would paint the descendant *under* its ancestor —
  verified against `post_order_indices`). Doing it right means an ancestor-first
  injection pass + cumulative fold + a wider stale-fold blast radius.
- **`(Some,None)` kept-child-under-degraded-parent** (a group kept its own target
  but its parent degraded away its target — the common bare-`Opacity`-wrapper
  shape). The child's target is orphaned (the parent has no target to composite
  into) → it vanishes, as today. Fixing it means routing the child's composite past
  the degraded parent to the nearest kept ancestor (or window) with the parent's
  opacity folded — a distinct forward-composite case, not this follow-up's charter.

Both are the **same skip as today** (`node.rs:265-270` already `continue`s on any
`None` end), so v1 introduces no new breakage — it strictly improves case A and
de-panics debug. Both get tracked follow-ups **with the corrected frequency note**
(they are often the more common shapes).

## Rejected alternatives

- **Nearest-kept-target-ancestor routing + cumulative opacity in v1** (the fully
  general rule unifying root/nested/chain). Correct in principle, but pays the
  chain z-order + cumulative-fold + wider-idempotency cost, and would also need to
  solve the pre-existing parent-target-undersizing issue (below) to be sound.
  Deferred to a dedicated spec; revisit when a measured/real degraded scene needs it.
- **Keep R2's flat-merge for nested groups.** Wrong mechanism — paints the child in
  the window's space/clip and leaves the parent compositing a target the child
  never reached (§ 2.3; R2's scope note already rejected this).
- **A per-draw opacity uniform on the group pipelines.** Would allow draw-time
  folding but is a shader/pipeline change far beyond this fix, duplicating the
  per-instance-alpha mechanism the compositor already relies on.

## Verification

GPU `--ignored` lane (real wgpu adapter; RX 6700 XT / RADV locally, pinned
lavapipe in CI). Lowest tier that observes the bug:

- **Headless unit — flip the fold's nested test + PIN the allocation.** In
  `render_compositor.rs`, re-point `degraded_fold_skips_nested_group_in_release_path`:
  a nested degraded group's alpha **IS** folded (quad `[ALPHA_FLOAT_OFFSET] ==
  source × opacity`; glyph variant at `color[3]`/`GLYPH_ALPHA_FLOAT_OFFSET`) and
  its range is **NOT** merged into `quad_flat`/`glyph_flat`. Add a `(None,None)`
  chain case asserting the deferred group is folded-but-not-merged (still not
  drawn). **Remove the `if cfg!(debug_assertions) { return; }` early-returns** —
  the default gate runs with `debug_assertions` ON, so the flipped asserts must
  execute there. Add a **`plan_allocation` pin**: with the two representative
  `(extent, EffectReason::OPACITY)` pairs matching the GPU fixture (outer larger,
  inner smaller) and the chosen budget, assert `plan_allocation(...) == [true,
  false]` (outer kept, inner degraded) — deterministic, no GPU, proves the fixture
  reaches case A.

- **GPU positive test — rebuild `nested_degraded_group_does_not_corrupt_parent`
  (`render_degraded_group_gpu.rs`) into a case-A positive.** The current fixture
  cannot reach case A: `outer` has no fill → ~empty bounds → it is the *smaller*
  target, so `budget=4096` degrades outer-first-or-both (a `(None,None)` chain,
  where the inner correctly still vanishes). Rebuild it so case A is provably
  exercised:
  1. Give `outer` its **own** larger `Background` that **spatially contains** the
     inner (e.g. a ~60×60 fill at (8,8) with `inner_fill` 16×16 at (20,20) inside
     it) so `outer.bounds` ⊋ `inner.bounds` and `target_bytes(outer) >
     target_bytes(inner)`. (Keeping inner ⊆ outer's own-paint box also avoids the
     pre-existing parent-target-undersizing clip — see follow-up.)
  2. Choose a budget in `[target_bytes(outer), target_bytes(outer) +
     target_bytes(inner))` so `plan_allocation` keeps outer and degrades only inner.
  3. **Remove the `if cfg!(debug_assertions) { return; }` guard** (no debug_assert
     to fear now).
  4. Assert case A was actually exercised — read the render world's
     `PreparedEffectTargets` (or an equivalent observable) and assert **exactly one
     group has a target (outer) and the inner is `None`** — so the pixel assertion
     can't silently drift onto a deferred path.
  5. Keep the corner-clean assertion (inner not mis-placed at window level). **Add**
     an interior assertion at the inner fill's device position: the inner fill is
     **present** (clearly lit above the outer-only region — proves it no longer
     vanishes) **and** at roughly the expected composed level (inner folded at
     `inner.opacity` into the outer target, then the outer target composited at
     `outer.opacity`), within an adapter-tolerant band computed via the same
     `composite_src_over` + linear→sRGB encode the sibling GPU tests use. This
     discriminates both vanish (fails "present") and gross opacity error (fails the
     band).

Full gate: headless `cargo test -p buiy_core` (unit + the `plan_allocation` pin) +
the GPU lane `cargo test -p buiy_core --test render_degraded_group_gpu --
--ignored --test-threads=1`; fmt + clippy.

## Follow-ups to file

- **`(None,None)` degraded chain** — cumulative opacity + ancestor-first injection
  (z-order). Note it is often the more common shape (bare-wrapper nesting).
- **`(Some,None)` kept-child-under-degraded-parent** — route the kept child's
  composite past the degraded parent to the nearest kept ancestor/window with the
  parent's opacity folded.
- **Parent-target undersizing (pre-existing, out of scope)** — because parent
  bounds omit nested-child extent (`extract.rs:1685-1689`), a parent's pooled
  target can be too small to hold a nested child that exceeds the parent's own
  paint box; this already threatens the `(Some,Some)` composite path and would
  equally clip a case-A injection. Filed separately; the fixture sidesteps it by
  keeping inner ⊆ outer's own paint.

## Doc touchpoints

- `effect-compositor.md § 2.3` — flip the "As landed (R2)" nested paragraph from
  "not yet implemented" to "landed (case A); `(Some,None)`/chain deferred".
- `docs/plans/follow-ups.md` — flip this entry to LANDED (case A); add the three
  follow-ups above with the corrected frequency note.
- `docs/README.md` — register this spec + its plan.
- The `fold_root_degraded_into_flat` doc block — rewrite for the widened
  `fold_degraded_groups` charter (folds all degraded, merges flat only for roots).
