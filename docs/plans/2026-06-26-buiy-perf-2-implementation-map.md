# #2 keyed partial re-extract — staged implementation map

**Status:** `[active]` — drives the staged implementation of audit #2 (the headline
cliff: one interactive change re-extracts the whole scene, 2.81 ms/5k). Realizes
[the final-pass design](../specs/2026-06-26-buiy-performance-final-design.md) § #2.
Current-code map (HEAD has #5/#9/#3 + measurement infra). Bevy 0.19.0.

## The footprint signature (the correctness keystone)
Per-entity `(emits_quad, gradient_count, border, shadow_count, outline, group,
text_quad_count)`. Sources on `ExtractedNode` (extract.rs:101-169): `emits_quad =
color != NONE` (:126); `gradients` (:168); `border` (:155); `shadows` (:161);
`outline` (:148); `group` (:141); `text_quad_count` is NOT on ExtractedNode — it's
this entity's TextQuads in `ExtractedTextQuads` (extract.rs:1638, produced by
`text::extract_buiy_glyphs`). Inputs = `NodePaintQuery` (extract.rs:1037-1091).

**v1 = quad-buffer-only.** Admissible Patch (else → Full): `emits_quad` unchanged AND
`text_quad_count` unchanged AND `group==None` AND `border/outline==None` AND
`shadow_count==0` AND `gradient_count==0`. (border/outline/shadow/gradient/grouped
buffers aren't slot-patched in v1.)

## Full-escalation conditions (any one → NodeDamage::Full)
1. Footprint mismatch (old retained vs newly-resolved 7-tuple). 2. v1 scope: changed
entity has border/outline/shadow/gradient/group. 3. despawn/spawn (`despawned` /
`skip_lifted`, extract.rs:1272-1273). 4. theme/forced-colors (`theme.is_changed()`
:1293). 5. group membership/contiguity (`Changed<EffectGroup|Opacity|StackingContext>`).
6. hierarchy/paint-order (`Changed<StackingContext|ClipRect|AncestorClip>`). 7. cold
buffer / `write_buffer_range` error. 8. text_quad_count change. "Any uncertainty → Full."

## Key seam / risk (the most error-prone)
Once extract mutates `ExtractedNodesView` via `ResMut`, `nodes.is_changed()` is true on
BOTH Full and Patch — so prepare can NO LONGER use `is_changed()` to pick repack-vs-patch.
A `NodeDamage` resource (`enum { Full, Patch(SmallVec<Entity>) }`) drives BOTH extract and
prepare. The prepare quad gate (prepare.rs:313-319) + partition gates (:446,:470) must
branch on `NodeDamage`, not `is_changed()`. On a Patch, do NOT deref-mut
`ExtractedEffectGroups` (keep `groups.is_changed()==false` so partition stays quiescent).
`write_buffer_range` never reserves → cold/None buffer MUST be Full.

## Staged plan (safest-first; each stage compiles + is independently verifiable)

### Stage A — retain into ResMut (+ EntityHashMap, audit #11). NO behavior change.
- `extract_buiy_nodes` params: `Commands` → `ResMut<ExtractedNodesView>` +
  `ResMut<ExtractedEffectGroups>` (extract.rs:1108). Reuse inner Vecs in place
  (replace the `ExtractedNodes` build :1546-1550 + inserts :1590/:1594 + no-window
  overwrite :1276-1277). **The gate-skip return (:1293-1296) must NOT deref the
  ResMut** (else is_changed() falsely trips → breaks O(0) idle). Add retained
  `index_of: EntityHashMap<u32>` resource (subsumes scratch `by_entity` :1306). Flip
  `by_entity/group_formers/group_index/sc_by_entity/rank_by_entity` → `EntityHashMap`
  (:1306,1315,1468,1519,1525). Register new resource in mod.rs:257-298 AND
  buiy_bench_support/src/lib.rs:85-99.
- Verify: work_counters tests unchanged; render_prepare tests
  (static_node_survives / one_node_change_keeps_unchanged_siblings / despawn_drops);
  dhat alloc_budget DROPS (per-frame Vec realloc + scratch maps gone); node_extract bench.

### Stage B — the NodeDamage classifier. Still does Full work.
- New `NodeDamage` resource; after the gate, classify dirty set Full-vs-Patch via the
  footprint (compute per changed entity vs retained). Always execute Full body for now;
  publish the tag. prepare reads NodeDamage but still full-repacks (rewire the :319 gate
  predicate to consult it). Add `node_patches` to RenderWorkCounters (counters.rs).
- Verify: a work_counters test — hover-retint solid bg ⇒ Patch classified; toggle
  quad / add border / grouped ⇒ Full. **Footprint-flip test authored here (RED).**

### Stage C — Patch extract branch (skip walk, in-place re-resolve).
- On `NodeDamage::Patch(es)`: SKIP extract.rs:1443-1580 (order/context/group walk);
  for each e, re-run the loop body (:1322-1440) and overwrite `view.0.nodes[index_of[e]]`.
  Never touch `groups_res`. Never rebuild the ordered Vec from the changed set (R5).
- Verify: **R5 sibling-retention test** (mutate 1 of N → other N-1 records byte-identical
  at same index); `node_rebuilds==0 && node_patches==1` on one hover; footprint-flip → Full.

### Stage D — prepare partial upload (set + write_buffer_range).
- Extend `PackedPartition` (buckets.rs:386) + `BuiyInstanceBuffers` (prepare.rs:93) with
  `quad_slots: EntityHashMap<Range<u32>>` filled on Full repack (derivable in the single
  `pack_view_partitioned` walk: start=p.len() before the own-quad push :454, end=p.len()
  after text-quad splice :474). Patch branch in prepare_buiy_instances: `set()` changed
  slots + ONE coalesced `write_buffer_range` per contiguous run; `debug_assert
  slot.len()==packed_count`; on Err → warn_once + escalate Full. Add quad_partial_uploads
  to BufferUploadStats (prepare.rs:249).
- Verify: render_prepare — hover Patch re-uploads only changed slots; the debug_assert
  test; **GPU `--ignored` goldens** (render_prepare/render_extract).

### Stage E — GPU lane + gallery verify.
- Both GPU legs (buiy_core + buiy_verify `--ignored --test-threads=1`); contiguity GPU
  (render_group_contiguity_gpu.rs); **gallery live-run** (`CAPTURE_SHELL_SCREEN=scroll
  cargo run -p buiy_gallery --bin capture_shell`); C8 acceptance (gallery_acceptance_c8d).
  iai EstimatedCycles patch ≪ full at 1k/5k/20k × {idle,hover,scroll,spawn-Full}.

## Correctness tests to add
R5 sibling-retention (extract layer); footprint-flip → Full; node_rebuilds==0 &&
node_patches==1 on one value-only change; prepare slot-len debug_assert (misclassification
loud not silent-garbage, pairs with the buckets.rs:585 contiguity tripwire); dhat Patch
idle/hover ≤ IDLE_BUDGET.

## Workload caveat
The win scales with the dirty-vs-idle and Patch-vs-Full ratio — measure the real
gallery/todomvc mix (`node_patches`/`node_rebuilds` counters) before extending past
quad-only. Full design-map archived in the run transcript (agent ae7b3688a69ee9bee).
