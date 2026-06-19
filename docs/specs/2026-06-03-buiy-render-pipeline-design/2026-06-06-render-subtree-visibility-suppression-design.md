# Render subtree visibility suppression — design note

**Status:** landed — Option A, 2026-06-09 (see "As landed" below).
**Date:** 2026-06-06 (decision); 2026-06-09 (landed).
**Owners:** render-pipeline.
**Related:** [paint-order-and-top-layer.md § 5.3 / § 5.4](paint-order-and-top-layer.md), [architecture.md § 3.1](architecture.md) (Changed-gated extract), R5 plan ([2026-06-03-buiy-render-r5-extract.md](../../plans/2026-06-03-buiy-render-r5-extract.md)), R6/R8 plans, [2026-06-07-render-extract-retain-damage-design.md](2026-06-07-render-extract-retain-damage-design.md) (the damage gate this extends).

## Problem

paint-order-and-top-layer.md § 5.4 requires render to drop a `CssVisibility::Hidden`
entity **and its descendants** from primitive emission ("subtree-scoped paint skip"),
explicitly noting the descendants *stay in* `painters_z` and keep a valid
`ResolvedLayout` (unlike `Display::None` and `content-visibility: hidden`, which are
pruned layout-side). § 5.3 says the same for the future `OffscreenAuto` marker.

R5 shipped only the **per-entity leaf** skip: `node_skip_reason(css_vis, offscreen)`
drops the entity that carries the marker, but its descendants — which remain in
`painters_z` — are not dropped. Because Buiy has **no visibility cascade** (only
`inherit_writing_mode` propagates anything; `CssVisibility` is author-set on a single
entity and not in the `Style` bundle), a `Visible`/default child of a `Hidden` parent
keeps its own `painters_z` entry and would paint. This diverges from § 5.4.

**Current blast radius: latent.** No v1 widget sets `CssVisibility::Hidden` on a
non-leaf entity, and layout does not yet emit `OffscreenAuto` (grep-confirmed; a
tracked cross-spec layout dependency). So the leaf skip is correct for every reachable
case today. The gap becomes reachable the moment an author sets `CssVisibility::Hidden`
on a container with painting descendants.

## Why it was not fixed inline in R5

R5's extract is `Changed`-gated (architecture § 3.1): each frame it rebuilds only the
entities whose inputs changed and emits that set; the **full-set assembly + the
persistent cache of unchanged painters is R6/R8's deliverable** (the system body marks
the seam: "R6/R8: merge cached records for unchanged painters here"). A *complete*
subtree suppression has to remove **unchanged** descendants too — and those live only
in the R6/R8 cache, not in R5's changed-set output. Implementing a partial
changed-set-only descendant drop in R5 would be incoherent with the cache R6/R8 builds.
So the fix belongs where the full painter set lives, or in a dedicated propagation pass
(below).

## Options

### A. Render-prep visibility-propagation pass (recommended)
A top-down `Children` walk in the render-prep window (`.after(Animate).before(Picking)`),
exactly the shape of `write_clip_rects` / `write_buiy_transform`, that resolves a
**computed** visibility per entity and writes a computed skip marker (e.g.
`ComputedPaintSkip { reason }`) onto every entity in a hidden/offscreen subtree.
Extract then keeps its per-entity `node_skip_reason` but reads the *computed* marker.
- **Pro:** consistent with the existing render-prep passes; independent of the R6/R8
  cache (the marker is a normal component, change-detected, so the cache sees its
  `Changed` and re-extracts correctly); naturally extends to a real `visibility:visible`
  override when a cascade lands (the pass stops propagating at an explicit `Visible`).
- **Con:** a new computed component + a new render-prep system; another tree walk
  (cheap — gated to seeded subtrees like the clip pass).

### B. Cache-coordinated descendant drop in R6/R8 assembly
Do the descendant suppression inside R6/R8's full-set assembler, where every painter
(changed + cached) is in hand: when a painter is marked skipped, also evict its
`Children`-subtree from the assembled output and the cache.
- **Pro:** no new component; lives exactly where the full painter set is.
- **Con:** couples visibility semantics into the cache/assembly hot path; must re-walk
  `Children` there anyway; harder to extend to `visibility:visible` overrides.

## Recommendation

**Option A.** It matches the established render-prep idiom (clip + transform are both
top-down `Children` walks in the same window), keeps extract a thin per-entity consumer,
is cache-agnostic, and is the cleanest base for a future visibility cascade. Land it as
a small dedicated phase (its own plan) alongside or just before R6/R8, since R6/R8 is
the first phase whose output actually paints the full set and would surface the gap.

v1 semantics either way: a **blanket** subtree drop (no `visibility:visible` override),
matching § 5.4's "entity and its descendants" — overrides wait for a visibility cascade.

## As landed (2026-06-09)

Option A, with two refinements over the sketch above:

- **The marker is extract's SINGLE skip source.** The pass writes
  `ComputedPaintSkip { reason: SkipReason }` (`render/components.rs`, lean
  derives like the other computed components — not registered, § 13) on the
  hidden/offscreen ROOT **and** every descendant, so extract reads only
  `Option<&ComputedPaintSkip>` and no longer queries `CssVisibility` /
  `OffscreenAuto` at all. The per-entity predicate `node_skip_reason` moved
  producer-side unchanged (`render/visibility.rs`): it now answers "does this
  entity ROOT a suppressed subtree". Runner-up — keeping the leaf predicate in
  extract alongside the marker — was rejected as a second decision point that
  the marker (present on the root too) makes redundant. An entity's OWN reason
  wins over the inherited one, so an ancestor un-hide leaves a still-suppressed
  descendant marked with its own reason.
- **The walk is seed-gated, not run every frame.** `write_paint_skip` early-
  returns unless a seed fired: `Changed<CssVisibility>` / `Changed<OffscreenAuto>`
  / `Changed<Children>` / `Changed<ChildOf>` (adds included), or a
  `RemovedComponents` event for `CssVisibility` / `OffscreenAuto` / `ChildOf` /
  `Children` (un-hide by component removal; reparent/detach out of a hidden
  subtree). Any seed triggers a FULL all-roots walk whose writes are
  change-gated by the shared `reconcile_one` (render/clip.rs), so re-visits are
  op-free and a steady-state frame is O(0) — no walk, no structural ops, no
  `Changed<ComputedPaintSkip>` for extract's gate to hear.
- **The damage gate covers both marker transitions.** Extract's `Or<…>` probe
  swaps `Changed<CssVisibility>`/`Changed<OffscreenAuto>` for
  `Changed<ComputedPaintSkip>` (the ADD direction), and a new
  `Extract<RemovedComponents<ComputedPaintSkip>>` stream covers the REMOVE
  direction (hide→show — invisible to `Changed`, exactly like the despawn
  stream in the retain-damage design). Missing the stream leaves a re-shown
  subtree permanently vanished; the GPU smoke in
  `tests/render_paint_skip.rs` pins that failure mode.

Coverage: headless pass/marker tests + steady-state quietness probe in
`crates/buiy_core/tests/render_paint_skip.rs`; predicate unit tests in
`render/visibility.rs`; one `#[ignore]` GPU smoke (hidden subtree packs 0
quads → show packs 2 → re-hide packs 0).
