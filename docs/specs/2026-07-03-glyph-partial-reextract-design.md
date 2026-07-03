# Glyph-tier keyed partial re-extract ("the glyph-buffer mirror") — design

Date: 2026-07-03
Status: active
Realizes: the **named fast-follow** in
`docs/specs/2026-06-26-buiy-performance-final-design.md` (decision row *"#2 vs #6:
sequence the glyph-buffer mirror AFTER the quad path proves out"* — the quad path
landed in PR #84, so the precondition is met) and the **sanctioned v1 successor**
in `docs/specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md` § 6.2
(*"v1 keeps wholesale rebuild-on-any-text-damage (per-entity patching is the same
deferred optimization architecture.md § 3.1 names for nodes)"*). Subsumes perf
audit **#6** (caret blink trips a full glyph re-extract ~2×/sec).
Motivated by: `docs/reports/2026-07-02-mt-performance-ceiling.md` — the wholesale
rebuild is a thread-invariant serial floor (3.7 / 14 / 56 ms for 60 / 240 / 960
text nodes) that caps active-text apps at 2.9× below their parallel-workload
ceiling; the update-only floors (0.3 / 0.5 / 1.25 ms) bound the payoff at 10–40×.

## Problem

`extract_buiy_glyphs` (`crates/buiy_core/src/text/extract.rs:186-904`) is
change-**gated** (steady-state O(0)) but non-**incremental**: any dirty frame does
`glyphs.clear()` + a wholesale re-walk of every painted entity — per-glyph
`physical()` quantization, atlas-key interning + hashing, instance emission — for
the whole scene, regardless of what changed. The profile shows this CPU walk (not
GPU upload, which is sub-ms) is ~95 % of the active-text serial floor.

## Decisions

**D1 — Two-tier Full/Patch with a `GlyphDamage` resource.** Mirror
`NodeDamage{Full|Patch(SmallVec<Entity>)}` (`render/extract.rs:1757-1762`) with a
`GlyphDamage` published by the extract producer and consumed by prepare +
compositor. Prerequisite hardening folded in: the **H6 fix**
(`docs/reports/2026-06-30-mt-safety-followups.md`) — `prepare_buiy_instances`
publishes the per-tier dirty bits it actually used into a resource;
`prepare_effect_groups` reads that instead of re-deriving `is_changed()` — becomes
load-bearing once damage is no longer binary.

**D2 — Splice semantics, not slot overwrite.** The node tier's fixed-slot
overwrite cannot work here: text run lengths change on the flagship triggers
(typing, caret blink ±1–2 instances). Factor the per-entity emission body
(extract.rs:437-868) into a shared `emit_one_entity → (glyphs, quads, keys)` (the
C3a `resolve_one` discipline — Full and Patch byte-identical by construction). A
Patch **replaces the entity's slice** in the retained `ExtractedGlyphs.glyphs`,
renumbers subsequent `entity_runs` ranges (O(runs)), memmoves the tail (84 B
instances — cheap vs the shaping/hashing skipped), and co-splices:
- `resident.keys` — restructured to record **per-entity key ranges at emission**
  (keys are NOT 1:1 with instances: the bidi secondary caret stamp pushes an
  instance but deliberately no second key, extract.rs:838-856);
- `ExtractedTextQuads` — the easy half (cross-entity order not load-bearing;
  per-entity contiguity only), patched under the same damage decision.

**D3 — Patch eligibility ("any uncertainty → Full").** Frame-level: no global
trigger (theme change, scale-factor change, `FontsGeneration`, `FontDbLineage`
reseat, vanished window — all whole-set by nature); **no structural change** — a
NEW un-scoped structural probe (`Changed<StackingContext/Stacking/Children/
EffectGroup/Opacity/…>`, the node-tier probe list at `render/extract.rs:1370-1387`)
because the existing glyph union is `With<TextBuffer>`-scoped and blind to
ancestor-driven paint reorders; no hide→show re-insertion (order position unknown
without the walk); no `Added` text entities; **no live effect-group degradation**
(the alpha-fold's "whole buffer repacked from source" invariant, compositor.rs:
308-332 — degradation never occurs at the default 64 MiB budget, so Full there is
free). Per-entity: resident in the retained runs. Plus a **changed-set-fraction
bail** (scroll ticks `GlobalTransform` on every text descendant; splice-all must
not cost more than wholesale) — Full above a measured threshold (~50 % of runs,
tuned in Stage B). Despawns (`RemovedComponents<ResolvedLayout>`) ARE patchable:
splice-delete keyed by the removal stream's entity ids (read, currently discarded).

**D4 — Publication discipline preserved.** Per-entity byte-compare before splice;
the resource tick moves only on real change (pins:
`text_decorations_change_republishes_quads_and_retains_glyphs`,
`blink_edges_rebuild_glyphs_only_and_steady_phases_rebuild_nothing`). Glyphs +
`entity_runs` stay under ONE tick (T8 D4, extract.rs:887-896); quads compared
separately. On Patch frames the retained order substitutes for the O(scene)
painters-z walk — the order maps aren't built at all.

**D5 — Atlas: touch-before-insert.** Under a partial walk, retained entities'
keys carry frame-old LRU stamps, so the walk's own inserts would evict them first
under page-budget pressure (silent stale-UV corruption of retained instances, no
grace involved). On Patch frames, touch ALL retained keys BEFORE any
`get_or_insert`; the `GlyphMetaCache` residency prune stays valid downstream of
that ordering.

**D6 — Prepare: suffix ranged upload.** `GlyphDamage::Patch` yields the first
shifted slot; upload `write_buffer_range(first_changed..len)` (one contiguous
span — handles length changes within capacity); growth past capacity or any
ranged-write error → full `write_buffer` fallback with `warn_once`. Extends
`BufferUploadStats` + `RenderWorkCounters` with glyph analogs (`glyph_patches`,
glyph instances uploaded). The extract-side win (D2) is independent of this and
correct with a full re-upload — same seam-independence the node tier proved.

**D7 — Counters, harness homes, param caps.** New counters registered in ALL
harness homes (RenderApp, `buiy_bench_support`, `tests/support/extract_harness.rs`,
`buiy_verify` content_presence) or taken as `Option<ResMut<_>>` (the
`RetainedNodeIndex`/`NodeDamage` precedent). `extract_buiy_glyphs` sits at Bevy's
16-param cap — new inputs tuple into existing params or a `SystemParam` bundle.

**D8 — Docs flip ships with the change.** Supersede the "v1 wholesale" language in
`glyph-pipeline.md` § 6.2 (+ its § 509 limitation row), `architecture.md` § 3.1
summary row, the `icon_producer.rs` module doc ("rebuilds wholesale" justification),
and mark the perf plan's Phase-6+ glyph-mirror item landed.

## Non-goals

- Icon-tier mirror (`ExtractedIcons` — identical wholesale pattern): separate
  follow-up; this change must not regress it.
- Per-slot fold freshness for degraded groups (D3 forces Full instead).
- Shape-run cache (explicitly rejected in the audit — unbounded p99), threading
  (the ceiling report's F5), any new per-instance field (wasm ≤16-attr posture),
  compute-shader compaction.

## Verification (the #84 standard)

Exact-integer counter gates (idle zeros; 1-change ⇒ patch==1, rebuild==0;
structural ⇒ rebuild==1), a glyph R5 sibling-retention test, a
footprint/reorder-escalation test authored RED (incl. probing the suspected
pre-existing ancestor-reorder under-trigger), the GPU partial-upload reftest
(upload delta == changed AND pixel-identical to a cold Full render) + the
fallback reftest (#84's unlanded review ask), both GPU legs, the gallery
live-run, and the `mt_ceiling` bench before/after (the ceiling report's floors
are the baseline). Every existing § 12 damage-gate retention pin is consciously
extended ("rebuild" → "rebuild or patch"), never deleted.
