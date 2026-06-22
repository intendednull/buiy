# Render extract/prepare damage retention — design note

**Date:** 2026-06-07
**Status:** landed (GPU-verify campaign Phase 4, item 1)
**Supersedes nothing; implements** architecture.md § 3.1 (the `Changed<T>`-gated
per-frame instance set), which the campaign built after it was found
specified-but-never-built. The bug below is the pre-fix state; "The fix" is the
shipped design.

## The bug (found by the Phase-4 spine GPU test)

architecture.md § 3.1 ratifies the v1 damage mechanism: the extract query is
`Changed<T>`-gated so a steady-state frame does near-zero work, and *"a frame
where no paint input changed re-extracts nothing and re-uploads nothing; the
persistent buffers from the prior frame are re-bound and re-drawn"* (§ 3.1 ¶
"This is the v1 damage mechanism"). Line 243: *"an unchanged entity contributes
its cached instance record rather than being re-resolved... a partial re-extract
patches only changed entities."*

That retention was **not implemented at the time** — the campaign built it (see
"The fix"). The pre-fix code was:

- `extract_buiy_nodes` (extract.rs:258-298) gates the `nodes` query with
  `Or<(Changed<…>)>`, builds `by_entity` from **only the changed entities**, walks
  the context tree, and **unconditionally** `insert_resource(ExtractedNodesView(all))`
  (extract.rs:398) — a full *replace* with the changed-only set.
- `prepare_buiy_instances` (prepare.rs:106-146) reads that resource,
  **unconditionally** `clear()`s + repacks + `write_buffer`s, and on the first
  frame the resource is absent it `init_resource`s and **returns without
  uploading** (the "one-frame warmup", prepare.rs:118-127).

Net effect, observed on the RX 6700 XT with a single static opaque node
(`gpu_test_app_with_layout` + one `(Node, Style, Background)`):

| frame | extract emits | prepare | `quad_count` |
|-------|---------------|---------|--------------|
| 0 | 1 (node just spawned → Changed) | init-empty, **return** | 0 |
| 1 | **0** (unchanged → gate yields nothing) | clear+upload from empty | 0 |
| 2+ | 0 | clear from empty | 0 |

**A static UI is extracted once, then vanishes** — `quad_count` is `0` on every
frame the buffer is actually read. The frame-0 warmup-return and the changed-only
replace never align. This is a real production correctness bug, not a test
artifact; the Phase-0 path (`extract_buiy_draws`) has no gate and does not have
it, which is why earlier smoke tests (single `update()`) never surfaced it.

## Why not "just remove the gate"

A gate-free full re-extract+re-upload every frame is the **runner-up the spec
explicitly rejected** (§ 3.1 ¶ "full immediate rebuild was rejected as wasting
the gate-#14 render-time budget"). It would re-upload unchanged buffers every
frame. Rejected — it contradicts the ratified design and the gate-#14 budget.

The other runner-up — a full retained scene graph with hand-rolled invalidation —
is also rejected by § 3.1 (premature invalidation complexity). The chosen design
sits between them, exactly as the spec intends.

## The fix

Implement the spec's observable contract — **idle ⇒ retain, change ⇒ rebuild** —
with the minimum machinery, deferring only the per-entity *patch* micro-optimization.

1. **Initialize the persistent buffer up front.** `BuiyInstanceBuffers::default()`
   is device-free (`RawBufferVec`/`UniformBuffer` create their GPU buffers lazily
   on first `write_buffer`). `init_resource::<BuiyInstanceBuffers>()` in the
   plugin's RenderApp `build` branch, so it always exists. This **removes the
   one-frame warmup return** in prepare — the first changed frame uploads
   immediately. (`ExtractedNodesView` is already `init_resource`'d at mod.rs:195.)

2. **Extract: build the full set, overwrite only on change.** Replace the single
   gated query with two reads:
   - an **un-gated** full query (`With<Node>`, all the paint-input components) that
     builds the complete `by_entity` set, and
   - a **change signal**, the union of three sources:
     - a gated probe query (`With<Node>` + the same `Or<(Changed<…>)>` set,
       `Entity`-only) — paint-input mutations and paint-skip flips;
     - `RemovedComponents<ResolvedLayout>` non-empty — **entity despawn** (a despawn
       drops every component; this is the only removal the `Changed` gate cannot
       see). NOTE: a `Display::None` *transition* does **not** remove
       `ResolvedLayout` (nothing in the crate removes it — layout keeps the entity
       in the Taffy tree at zero size); it is instead caught by
       `Changed<ResolvedLayout>` (the zero-size re-write) in the probe above. So the
       removal stream is strictly for despawn.
     - **`theme.is_changed()`** — a theme / forced-colors swap re-resolves every
       token-bearing fill globally and must bypass the per-entity `Changed` gate
       (color-and-forced-colors.md § 3 / mod.rs:313-315). Without this, the new
       retain early-return would leave stale colors on a light↔dark or
       forced-colors switch.

   Logic: if the primary window is gone → insert empty (unchanged, keeps the
   vanished-window-clears contract, extract.rs:318-321). Else if **none** of the
   Changed-probe, the despawn stream, or `theme.is_changed()` fired → **return early
   without inserting** → the prior `ExtractedNodesView` stays resident and
   `is_changed()` is false downstream. Else → rebuild the full set from the un-gated
   query and `insert_resource(ExtractedNodesView(all))`.

   *Why full-rebuild-on-any-change rather than per-entity patch:* the spec's
   `Entity`-keyed patch cache (resolve only the changed entity's record, keep the
   rest) is a further optimization. Rebuilding the **full** set when *anything*
   changed is correct, far cheaper than the bug's behavior, and satisfies § 3.1's
   real cost concern (idle frames are O(0), which the early-return delivers). The
   per-entity patch stays a documented deferral — it only matters under a workload
   that mutates a few of very many nodes per frame, which v1 has no benchmark for.

3. **Prepare: gate the GPU upload on change, retain otherwise.** Take
   `nodes: Res<ExtractedNodesView>` and, when `!nodes.is_changed()` (and the buffer
   is already populated), **return early** — leaving the persistent buffer intact
   for the node to re-bind and re-draw (§ 3.1). When it *is* changed, clear +
   repack + `write_buffer` as today. Because extract now overwrites the resource
   only on a real change (step 2), `is_changed()` is the exact per-frame damage
   signal — no content diffing needed.

### Frame table after the fix (single static node)

| frame | extract | `ExtractedNodesView.is_changed()` | prepare | `quad_count` |
|-------|---------|-----------------------------------|---------|--------------|
| 0 | rebuild (1, Changed) → insert | true | upload | **1** |
| 1 | no change → retain (no insert) | false | retain | **1** |
| 2+ | retain | false | retain | **1** |

> **Extension (2026-06-09).** The subtree visibility suppression
> ([2026-06-06-render-subtree-visibility-suppression-design.md](2026-06-06-render-subtree-visibility-suppression-design.md))
> added a SECOND removal stream beside the despawn one:
> `RemovedComponents<ComputedPaintSkip>` — a hide→show flip removes the computed
> paint-skip marker, which (like despawn) emits no `Changed`. The change-signal
> union below reads with that stream included; everything else is as designed
> here.

## Removal-detection caveat

`RemovedComponents<ResolvedLayout>` read inside `Extract<…>` observes the **main**
world's removal events for this frame (the same world the Changed ticks come
from); it is a `ReadOnlySystemParam` whose `Local` cursor persists in the
render-world system across frames (the idiom Bevy's own `extract_removed_*`
systems use). It fires on **entity despawn** — the case the `Changed` gate cannot
see. It does **not** fire on a `Display::None` transition: layout never removes
`ResolvedLayout` (the entity stays in the Taffy tree at zero size, so
`write_resolved_layout` re-`insert`s a zero-size `ResolvedLayout`), so the hide is
caught by `Changed<ResolvedLayout>` in the probe instead. Two regression tests pin
this: "spawn → frames → **despawn** → frames → `quad_count == 0`" (removal stream)
and "spawn → frames → set **`Display::None`** → frames → node leaves the set"
(`Changed` path).

## Tests (TDD)

- **Regression (the bug):** `prepare_uploads_persistent_buffers` — one static
  opaque node, `finish_and_run(3)`, assert `quad_count == 1` on the steady-state
  frame (fails today with `0`).
- **Retain across idle:** drive 5 frames, assert `quad_count` stays `1` every
  frame after the first (no fl​icker to 0).
- **Removal:** spawn → frames → despawn the node → frames → assert `quad_count == 0`.
- The two other spine stubs (`node_draws_persistent_buffers_with_view_uniform`,
  `top_layer_composites_last_over_in_flow`) then become assertable.
- Headless: a device-free unit test that extract returns early (no resource
  overwrite) when nothing changed, and rebuilds the full set when one of several
  nodes changes (asserts all N nodes are present, not just the changed one).

## Files

- `crates/buiy_core/src/render/mod.rs` — `init_resource::<BuiyInstanceBuffers>()`
  in the RenderApp `build` branch.
- `crates/buiy_core/src/render/extract.rs` — split the gated query into un-gated
  full + gated probe; add `RemovedComponents<ResolvedLayout>`; conditional insert.
- `crates/buiy_core/src/render/prepare.rs` — drop the warmup return; gate upload on
  `nodes.is_changed()`.
- Update `docs/plans/follow-ups.md` (note the § 3.1 retain is now built; the
  per-entity patch stays deferred) and architecture.md § 3.1 (point to this note).
