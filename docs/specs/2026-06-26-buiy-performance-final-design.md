# Buiy performance — final-pass design

**Status:** `[landed]` — merged in PR #84
**Date:** 2026-06-26
**Realizes:** [performance audit](../reports/2026-06-25-performance-audit.md) (the 16
findings + 6-phase measurement roadmap) and the prototype-first retrospective
(`worktree-perf-proto:RETROSPECTIVE.md`, which PROVED #5 = 8.6× and measured #3/#2).
**Plan:** [2026-06-26-buiy-performance-final.md](../plans/2026-06-26-buiy-performance-final.md).
**Base:** `origin/main` 7752c01 (#83). The #5 atlas O(1) touch + the bench scale
scenes are already cherry-picked onto the campaign branch (27656f6, e9f15f6).

This is the production (Phase B) design, re-decided from the full picture after the
throwaway prototype. The audit's #1 finding is **measurement blindness**, so the
final builds the measurement infrastructure FIRST and lands every optimization
**red→green** against it.

## 0. Constraints (load-bearing — every section honors these)

- **60 Hz / 16.7 ms is a HARD FLOOR, never "good enough."** Always optimize toward
  optimal and explicitly target **weaker machines** — do not trust the dev box's
  RX 6700 XT headroom. Therefore the **load-bearing gate is the iai-callgrind
  instruction count** (`EventKind::Ir` + `EstimatedCycles`), which is *identical* on
  the dev GPU and a weak Celeron — it measures "work the CPU must do," host-independent.
  The spec derives an explicit **weak-machine 60 Hz instruction budget** from a STATED
  reference (≈2-wide in-order, ≈1.4 GHz, ≈0.7 IPC ⇒ ≈16 M instr/frame @ 60 Hz) and
  expresses each scale point as a % of it — the reference clock/IPC is the named,
  re-tunable scaling assumption. `EstimatedCycles` (Ir + L1 + 5×LL + 35×RAM) prices the
  cache pressure that pure Ir is blind to (the suspected cause of #2's super-linearity).
  Wall-clock + the p99/worst-frame overlay are **observability only**, never a gate
  (runner-noisy, lavapipe-unrepresentative — consistent with DG-3).
- **wasm: zero new obstacles, by construction.** Counters are plain always-on ECS
  resources (no platform dep). dhat / iai-callgrind / criterion are dev-deps / isolated
  test or `[[bench]]` targets — never in the production graph; Valgrind is a host tool,
  Linux-CI-only. The `profiling` feature is OFF by default (production/wasm graph
  unchanged); when ON, `trace_chrome`/`tracing-wasm` for wasm, `trace_tracy` cfg-gated
  `#[cfg(not(target_arch="wasm32"))]`. Every runtime primitive used (run conditions,
  `Changed`/`RemovedComponents`, `EntityHashMap`, `RawBufferVec::write_buffer_range`,
  retained `ResMut` resources) is core bevy_ecs/bevy_render. **No new per-instance
  field** — `PackedInstance` stays 68 B / `[f32;17]` (the #82 WASM ≤16-attr-band
  constraint is untouched). No compute/threads dep ever enters the production graph.
- **Regression-guarded.** Every change keeps the full headless gate + the GPU `--ignored`
  lane + the gallery live-interaction tier green, and is eyeballed via a real gallery
  render (`capture_shell`) — the "headless-green ≠ works" lesson. See the plan's gate.
- **Merge-gated on human review.** No self-merge.

## 1. Measurement stack (lands FIRST)

### 1.0 Prerequisite refactor — shared `bench_support`
`crates/buiy_core/benches/pipeline.rs` REPLICATES the adapterless harness
(`PipelineHarness`, pipeline.rs:65-144) + scene-builders because a `benches/` target
cannot `mod` `tests/support/extract_harness.rs`. Adding dhat (a test) and iai (a bench)
would make it a **triple** copy. Root-cause first: extract the harness + the three
scene-builders (`build_large_scene`/`build_flat_scene`/`build_flat_bg_scene`) into ONE
shared home — a `bench-support`-feature-gated `pub mod bench_support` in `buiy_core` (or
a `buiy_bench_support` dev-crate) that owns the **single counter-registration list** —
and repoint the criterion bench at it. criterion/dhat/iai then consume one harness.
Pure de-duplication, no behavior change.

### 1.1 Work-unit counters — the cheapest, highest-leverage gate
Copy the existing always-on-integer discipline (`LayoutTaffyComputeCount`
systems.rs:110, `SyncStylesIterCount` :120, `TextMeasureCallCount`/`TextSyncAppliedCount`/
`TextCommitReshapeCount`), pushing each counter **into the world its system runs in**:

- **Render-world** — one grouped `RenderWorkCounters` resource (read together in the
  extract gate), init'd in BOTH the real RenderApp (render/mod.rs:282) AND `bench_support`'s
  render world (avoids the registration-drift panic):
  - `node_rebuilds` — +1 when `extract_buiy_nodes` passes the damage gate (extract.rs:1286)
    instead of early-returning (0 on idle = the #2 rebuild-rate signal).
  - `instances_built` — count of `by_entity.insert` (extract.rs:1432).
  - `atlas_touch_ops` — **the #5 blind-spot closer.** A single post-loop overwrite =
    `resident.keys.len()` (NEVER per-iteration `+=1`, or the counter taxes the hot loop
    it measures). Covers BOTH the glyph producer (text/extract.rs:355-356) AND the icon
    producer mirror (icon_producer.rs:248-249).
  - `resident_keys` — `= resident.keys.len()` after rebuild (text/extract.rs:854); guards
    unbounded growth / a missing dedup.
  - `upload_instances`/`upload_bytes` — promote the existing stats-only `BufferUploadStats`
    (prepare.rs:250) to gateable (0 on a retained idle frame).
- **Main-world** (per-resource, matching precedent): `PickNodesTested`/`PickEmitSkipped`/
  `PaintOrderRebuildCount` (picking/backend.rs), `A11yTreeBuildCount`/`A11yNodesVisited`
  (a11y/mod.rs:445).

Headless gate tests (in `bench_support`'s harness): settle a scene, run one steady frame,
assert EXACT integers. **Acceptance criterion (tracked, not an afterthought):** on an idle
text frame `node_rebuilds==0 && upload_instances==0 && atlas_touch_ops==resident_keys &&
A11yTreeBuildCount==0`. Deterministic, host-independent, wasm-safe.

> **Why `atlas_touch_ops` is non-negotiable:** a naive "extract rebuild == 0 on idle"
> assertion is GREEN on the exact 10.9 ms static-text frame #5 measures — the atlas-touch
> loop runs *downstream* of the rebuild gate and is *non-allocating* (so dhat is blind
> too). Only `atlas_touch_ops` (or iai `EstimatedCycles`) can see #5.

### 1.2 dhat allocation-count gate (cross-platform)
Isolated `crates/buiy_core/tests/alloc_budget.rs` with `#[global_allocator] static: dhat::Alloc`
and NOTHING else (the global allocator must be scoped to its own target). Settle + one frame
+ `dhat::HeapStats`, assert `total_blocks`/`total_bytes` ≤ a committed band. Catches the
per-frame scratch fan (#16). Determinism: settle + one-frame + single-thread + adapterless +
tolerance band; documented re-bless workflow for std/bevy bumps.

### 1.3 iai-callgrind instruction-count gate (the weak-machine backbone)
`crates/buiy_core/benches/pipeline_iai.rs` — `library_benchmark` twins of the criterion
benches, scene build+settle in `setup()` OUTSIDE the timed `app.update()`, scale capped at
10k (Valgrind is 50–100×). Linux-CI-only (Valgrind), SHA-pinned runner via
`taiki-e/install-action`. **Land informational-first** (print Ir/`EstimatedCycles`, no
fail) to learn the noise floor, **then flip to `--regression=Ir` AFTER #11** (SipHash's
per-process seed is the main Ir jitter source — #11 both optimizes and stabilizes the
baseline). Gate cadence: counters + dhat every PR; iai push/schedule (+ optional 1k smoke
on PR).

### 1.4 Observability (`profiling` feature, OFF by default — trailing)
bevy `FrameTime`/`EntityCount`/`LogDiagnostics` + per-`BuiySet` & per-RenderApp-stage
`info_span!` aggregated into the Flutter **build-vs-raster two-number split**; gallery
overlay with 8.33/16.66/33.33 ms budget lines + p95/p99/worst-frame. Never a CI gate.

## 2. The optimizations (each gated; ordered by risk)

### #9 — delete the dead `extract_buiy_draws` path  *(zero-risk)*
Remove `extract_buiy_draws` (render/mod.rs:323) + the `ExtractedDraws` carrier (mod.rs:63/282):
it walks all `Background` nodes, String-hashes a token each, allocs a fresh `Vec`, and inserts
a resource nothing live reads (the live path reads `BuiyInstanceBuffers`). **Verify-then-delete**:
confirm `extract_buiy_nodes` has no scheduler-ordering/resource dependency on it (comments at
extract.rs:1257/1294 reference its always-insert contract) before deleting; tripwire test if any
doubt. Gate: dhat shows the per-frame `Vec` gone.

### #11 — `EntityHashMap` on hot Entity-keyed maps  *(low-risk; stabilizes iai)*
Switch std-SipHash `HashMap<Entity,_>` → bevy `EntityHashMap`/`EntityHashSet`: extract.rs:1298
(`by_entity`), :1460 (`group_index`), :1511 (`sc_by_entity`), :1517 (`rank_by_entity`),
prepare.rs `group_by_entity`, the a11y maps. `FxHash` for non-Entity content keys
(`AtlasKey`/`FontKey`). Equality-preserving (low risk). **Do this BEFORE flipping iai to
gating** — SipHash's random seed is the main Ir jitter; #11 both IS the optimization and
STABILIZES the baseline (precedent: Bevy PR #17078 = 25% bevymark framerate / 20.2% frame
time from the wrong hasher+tuple key). Gate: iai Ir delta.

### #5 — lock in the already-ported atlas-touch O(1)  *(no new opt code)*
The O(1) LRU touch is cherry-picked (10.6→1.24 ms, −88%). Add the regression GUARD only:
the `atlas_touch_ops == resident_keys` counter assertion on an idle text frame + an iai
`EstimatedCycles` gate on the steady-text bench so a future re-introduction of the
O(visible-glyphs) `VecDeque` scan reddens. Cover the icon-producer mirror too.

### #3 — gate the ~12 ungated full-tree post-Taffy layout passes  *(medium-risk)*
**Keystone:** every post-Taffy pass already writes idempotently (inserts only when output
differs — `write_resolved_layout` systems.rs:3022, `transform_composition`, `stacking_context`,
`inherit_writing_mode`), so gating OFF a frame where no input changed is **output-identical by
construction**. One shared `LayoutDirtyThisFrame(bool)` seeded once/frame by `seed_layout_dirty`
(new first step `BuiyLayoutStep::SeedLayoutDirty`), consumed via
`configure_sets(PostTaffyOverrides.run_if(layout_dirty))`. The seed ORs: the decomposed
layout-input `Changed` filters (Style is a Bundle → OR the 14 `#[require]`'d parts, split
across query params to beat the 15-Or/16-param caps) + `Changed<Children/ChildOf/ScrollOffset/
Window>` + **`Changed<ResolvedLayout>` (the keystone self-heal term)** + a `RemovedComponents`
tuple (bind-each-then-OR, NEVER short-circuit `||`, or a cursor is stranded).
`Changed<ResolvedLayout>` makes the union **self-healing**: any geometry cause the union
under-covers (notably text edits, which produce NO layout-component `Changed` — they go via
`tree.mark_dirty_for_entity`, text/sync.rs) fires the gate the NEXT frame (a one-frame lag,
never a permanent miss) — **provided `write_resolved_layout` stays ungated (Option B)**.

- **Correctness risk = UNDER-GATING = permanent stale layout** (the worst bug). Mitigation:
  keep `write_resolved_layout` UNGATED in v1 (Option B) so `Changed<ResolvedLayout>` is a
  COMPLETE self-healing geometry proxy; a **differential property test authored FIRST**
  (mutate each seed input incl. text-edit/resize/despawn/reparent, assert gated pipeline ==
  ungated pipeline); a single source-of-truth input list + a steady-state-`Changed`-count
  tripwire (the gate degrades to always-on, no correctness loss, if a pass starts refreshing
  a tick unconditionally).
- Staged increments: **1** = gate only `PostTaffyOverrides` (the cleanly-measured ~0.75 ms/5k,
  all passes pure+idempotent, self-heals via ungated `write_resolved`); **2** = gate
  `inherit_writing_mode`; **3** = ground-truth-gate `write_resolved` on a `TaffyGeometryChanged`
  flag captured inside `taffy_compute` (Option A) only if the residual O(N) Taffy-read scan
  shows on iai. Keep `taffy_compute` ungated in v1 so its counter resets aren't stranded.
- Gate: `LayoutPostTaffyRunCount==0` steady / `>=1` mutated; iai locks steady-frame Ir
  near-constant in N at 1k/5k/10k; differential test green; gallery RUN + GPU lane clean.

### #16 — SoA-split the fat `ExtractedNode`  *(cache enabler for #2)*
The ~480 B `ExtractedNode` (~73% rarely-populated, ~2.4 MB/5k) is the suspected cache-bound
cause of #2's super-linear 3.18 ms/5k. SoA-split the cold outline/border/shadow/gradient
channels out of the hot quad-geometry struct (Vello stream-of-arrays precedent) and hoist the
per-frame scratch `HashMap`s (extract.rs:1298/1460/1511/1517) into reused render-world
resources. **dhat-gated — attack only the sites the gate flags, no blind 215-site rewrite.**
`PackedInstance` stays 68 B. Sequence immediately before / co-with #2 so the SoA layout IS the
retained patch target. Gate: dhat blocks/bytes drop; iai `EstimatedCycles` improves; pixel
goldens unchanged.

### #2 — keyed partial re-extract  *(the big redesign; highest value + risk)*
**Two-tier.** Each frame, extract classifies damage **Full** (structural) vs **Patch**
(value-only):
- **Full** = today's walk (extract.rs:1106-1585) but written into RETAINED resources via
  `ResMut` (kills the per-frame realloc, #16) — the safe fallback.
- **Patch** re-resolves ONLY changed entities, writes records in-place at STABLE DENSE slots,
  and prepare `write_buffer_range`'s only those slots.

**Load-bearing insight:** keep instance slots **dense + paint-order-contiguous** (no holes) —
re-pack on ANY structure change, reuse stable slots on pure value change. This avoids **Trap 1**
(R5 sibling-drop: the retained ordered list is never rebuilt from the changed set — patch-in-place
or full-rebuild, no insert-changed-only path) and **Trap 2** (effect-group contiguity: a group's
members must be one contiguous run — buckets.rs `RangePartitioner` — so despawn/spawn/reorder/
footprint-change ESCALATE to Full).

**Data:** `ExtractedNodesView` → persistent `ResMut`-mutated + `NodeDamage{Full|Patch(SmallVec<Entity>)}`;
`RetainedExtract{index_of: EntityHashMap<u32>}` (subsumes `by_entity`); prepare
`quad_slots: EntityHashMap<Range<u32>>`. **FOOTPRINT signature** (the correctness keystone) =
`(emits_quad, gradient_count, border, shadow_count, outline, group, text_quad_count)`; any
old/new mismatch → Full, guaranteeing a Patch never changes per-buffer instance count. On Patch:
SKIP `assemble_context_tree`/`context_roots`/`sc_by_entity`/nearest-group climb/group-bounds
(extract.rs:1443-1572) — order+group reused; **this is where most of the 3.18 ms/5k disappears.**
CRITICAL: do NOT `deref_mut` `ExtractedEffectGroups` on a Patch (keeps `groups.is_changed()==false`
so prepare's quad gate sees a pure node patch). Prepare: `quad_patch` = `set()` into stable slots +
coalesced `write_buffer_range` (scroll→1 range, hover→1 slot); flat/group/gradient ranges + view
uniform unchanged on Patch. **v1 = quad-buffer-only**; band/shadow/gradient/outline/grouped/
text-quad-count-change escalate to Full.

**Why it matches the workload:** buiy has NO tween/momentum; live triggers are hover (1 quad
value-only Patch), scroll (contiguous run of value-only slots, 1 `write_buffer_range`), typing,
caret-blink. Precedents: WebRender retained-DL + `GpuCache` `pending_blocks` partial upload +
`saved_block_count` counter; Bevy `SyncWorldPlugin` mirror-don't-rebuild. Knowledge-lineage
caution (decisive): WebRender ABANDONED "repaint everything every frame" *specifically for
low-end/integrated GPUs* — the exact hardware buiy's hard-60 Hz constraint targets.

- **Risks:** FOOTPRINT MISCLASSIFICATION (highest) — a footprint-changing edit mis-tagged Patch
  writes a wrong instance count at a slot → garbage/out-of-range `set()`. Mitigate: the EXHAUSTIVE
  signature, a prepare `debug_assert slot.len()==packed_count`, a footprint-flip regression test
  (hover toggling a quad on/off must take Full), and "any uncertainty → Full." R5-reborn — single
  in-place-or-full-rebuild path, pinned with the sibling-retention test. extract/prepare divergence
  — `NodeDamage` drives both + a debug frame-epoch assert-match. `write_buffer_range` edges
  (empty/range>capacity/uninit, buffer_vec.rs:196-214) — first overwrite always Full, a Patch never
  grows the buffer; escalate-to-Full + `warn_once`. WORKLOAD DEPENDENCE (audit-open) — the win
  scales with the dirty-vs-idle ratio; measure the real per-frame Full/Patch/retain mix on the
  gallery + todomvc before sizing the glyph-buffer mirror fast-follow.
- Gate: iai proves patch-instructions ≪ full-instructions at 1k/5k/20k × {idle, hover-retint,
  scroll-subtree, spawn/despawn-Full}; `node_rebuilds==1` on one-change; R5 + footprint-flip tests
  green; GPU-lane goldens clean.

## 3. Decisions adopted (resolving the research's open questions)

| Decision | Adopted |
|---|---|
| `bench_support` home | Feature-gated `pub mod bench_support` in `buiy_core` (`bench-support` feature) — one harness + one counter list for criterion/dhat/iai. Resolve FIRST. |
| Counter granularity | Grouped `RenderWorkCounters` (render-world, read as a unit); per-resource main-world (matches precedent). |
| iai/dhat CI cadence | counters + dhat every PR; iai push/schedule (+ optional 1k PR smoke). |
| iai gate posture | Informational-first → flip to `--regression=Ir` AFTER #11 stabilizes the seed jitter. |
| Weak-machine budget | 60 Hz / 16.7 ms (user's hard floor); reference clock/IPC stated as the re-tunable scaling assumption. |
| #3 `write_resolved` | Option B first (leave ungated = complete self-heal proxy); Option A (ground-truth `TaffyGeometryChanged`) only if iai flags the residual scan. |
| #3 increments | Staged: 1 = PostTaffyOverrides → 2 = inherit_writing_mode → 3 = ground-truth write_resolved. |
| #2 v1 scope | Quad-buffer-only Patch; band/shadow/gradient/outline/grouped → Full. Measure real mix, then extend. |
| #2 vs #6 (caret) | Sequence the glyph-buffer mirror AFTER the quad path proves out; #6 may get an early targeted fix (caret/selection in its own small buffer). |
| #16 depth | Conservative cold-channel SoA split first; WebRender ivec4-indices model only as a fast-follow if iai still flags cache pressure. |
| #9 | Verify-no-live-read + no-ordering-edge, THEN delete; tripwire if any doubt. |
| Observability | Gates (1.1–1.3) are load-bearing; the overlay (1.4) trails. |

See the [plan](../plans/2026-06-26-buiy-performance-final.md) for the phase-by-phase
execution + gates.
