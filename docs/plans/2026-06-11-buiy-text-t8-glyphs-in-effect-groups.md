# Buiy Text T8: Glyphs in Effect Groups + Damage Hardening — Implementation Plan

**Date:** 2026-06-11
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md](../specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md) § 11.2 + [decoration-and-paint.md](../specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md) §§ 4.5, 6.3 + [verification.md](../specs/2026-06-09-buiy-text-rendering-design/verification.md) §§ 1.3, 4 + [render effect-compositor.md](../specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md) § 3 step 1
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T8 (depends on T4 + the landed R9/GPU-campaign compositor; the implementer starts from a branch with T1–T7 merged — T7 landed @ `f17e770`)
**Closes:** [follow-ups.md "Render — glyphs bypass effect-group compositing (text-seam follow-up)"](follow-ups.md) and the `TODO(text-seam)` block at the glyph draw (`render/node.rs:289–297`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Text inside an `Opacity(0.5)` card dims — exactly once. Today the
glyph draw in `BuiyNode::run` paints the WHOLE glyph buffer into the flat
window pass (`draw(0..4, 0..glyph_count)`, node.rs:309) with no group
mechanism, so a glyph (or a line-through/caret solid stamp — they are glyph
instances too) inside an `EffectGroup` subtree bypasses the group's
off-screen `Rgba16Float` target and the opacity composite. T8 partitions the
glyph buffer into per-group/flat instance ranges **exactly like the quad
path** (`pack_view_partitioned`'s contiguity-by-construction argument carries
over verbatim: glyph emission order is the SAME `context_tree_paint_order`
walk), draws each group's glyph range into its off-screen target in the
step-1 group pass via a `Glyph@Rgba16Float` pipeline specialization (the
existing `BuiyPrimitives` specializer already keys on `(kind, format,
samples)` — no new descriptor work, no new shader), and excludes group ranges
from the flat window glyph draw. The T6-pinned § 4.5 dim-asymmetry test
FLIPS (its line-through half currently asserts the wrong behavior as
expected, by design). T8 also hardens the damage story at the GPU lane: the
caret-blink-only-reupload assertion (a blink frame re-uploads the glyph
buffer and ONLY the glyph buffer — T7's value-compared publish made this true
on the CPU; T8 pins it through real `write_buffer` calls), and the gate-#14
typing-latency fixture (the MECHANISM: one frame from a `Text` edit to the
new `ExtractedGlyphs` publish — budget numbers stay with
`buiy-verification-design`).

**Architecture — the group-tagging mechanism (THE load-bearing decision,
D1):** glyph instances learn their entity's group at **prepare time, from the
fresh node list** — never at emission. `ExtractedGlyphs` grows per-entity
instance-run attribution (`entity_runs: Vec<GlyphEntityRun>` — one contiguous
`Range<u32>` per emitting entity, in emission order, covering the instance
vec exactly), and `prepare_buiy_instances` derives each run's group by
looking its entity up in `ExtractedNodesView` (every `ExtractedNode` already
carries the `group` tag that `extract_buiy_nodes` resolves via the
nearest-`EffectGroup`-ancestor `ChildOf` climb). This is decoration-and-paint
§ 4.6's `TextQuad` discipline applied to the glyph buffer: group membership
comes from the node record, re-derived from the FRESH list on every
recompute, so a glyph's partition placement can never disagree with its
entity's — and group truth stays single-sourced in `extract_buiy_nodes` (the
glyph producer learns NOTHING about groups: no new queries on a system
already at Bevy's 16-param cap, no duplicated entity-sorted index assignment,
no probe-union growth). The partition ranges (`glyph_group_ranges` /
`glyph_flat_ranges` on `BuiyInstanceBuffers`) are CPU-side draw bookkeeping,
recomputed under the UNION of the quad and glyph damage gates — the GPU glyph
re-upload stays gated on `glyphs.is_changed()` alone, so the independent
buffer gating (and with it the caret-blink damage property) is preserved
structurally.

**Where T8 ends (honesty pins — named seams, not built):**

- **A glyph's group is its ENTITY's group** — the nearest-former `ChildOf`
  climb, exactly the quad semantics (effect-compositor.md § 1.1 / decided
  fork 5). Nested-group text tags the inner group and composites up the
  parent chain like any quad member. No per-glyph or per-run group override
  exists or is wanted.
- **Within a group target, all quads draw before all glyphs** — the same
  shadow < quad < glyph rank the flat pass uses. glyph-pipeline § 11.3's
  flat quad-then-glyph limitation now holds *per region* (inside each group
  target, and inside the flat complement) rather than globally; the
  cross-layer interleaved batching remains the render paint-order phase's
  work, unchanged by T8.
- **Single page-0 bind unchanged** (glyph-pipeline § 11.1): the group glyph
  draw binds the same page-0 `AtlasGpu` coverage bind group the flat draw
  binds. The multi-page seam stays a named follow-up, triggered by the
  warn-once firing in practice.
- **Degraded groups (budget pressure) vanish — pre-existing, mirrored, not
  fixed.** As built, a `plan_allocation == false` group gets no target, the
  step-1 pass `continue`s, and its members are NOT in `flat_ranges` — so
  degraded members paint nowhere, despite the "drawn flat instead" comments
  (node.rs:127, compositor.rs:321). Latent under the 64 MiB budget. T8
  mirrors the quad semantics for glyphs (a degraded group's glyph range is
  likewise skipped) and FILES the discrepancy as a follow-ups entry (Task 7)
  rather than diverging or silently widening scope.
- **The latency fixture asserts the MECHANISM only** — one frame from edit
  to publish, frame-counted on the adapterless harness with the existing
  instruments. Wall-clock/percentile budgets are `buiy-verification-design`'s
  (verification § 4 gate #14: "Numbers stay with buiy-verification-design").
  `BufferUploadStats` counts uploads, not bytes; byte-level budget
  instrumentation is the same deferral.
- **T9 owns** the golden-suite expansion, the gate-#15 typing-churn fixture,
  the ASCII pre-warm decision, CLAUDE.md updates, and the prior-art errata
  patch. T8 ships exactly the campaign's T8 deliverable.
- **No per-entity patching** (wholesale rebuild stays, § 11.4), **no
  multi-page bind**, **no interleaved batching**, **no new shader** —
  `coverage.wgsl` is format-agnostic; the `Rgba16Float` variant differs only
  in the pipeline descriptor's `ColorTargetState.format`, which the existing
  key already carries.

**Tech stack:** existing workspace deps only. **No new dependencies** — if a
task appears to need one, STOP: that contradicts the charter. (`cargo deny
check` not required: no dep changes.)

**Test reality:** the producer's entity-run attribution and the pure
partition function are headless (the adapterless `TextExtractHarness` +
plain unit tests in `render_buckets.rs`); the typing-latency fixture is
headless on the same harness. The partition wiring, the group glyph draw,
the dim flip, the composite golden, and the blink-reupload assert all need a
real adapter — `#[ignore]` on the established GPU lane, built on
`tests/support/mod.rs`. Every GPU test keeps `#[ignore]`.

---

## The gate (run BOTH lanes at every task boundary)

T8 ships render-spine changes and GPU tests, so the per-task gate is the
headless gate **plus the GPU lane** (this host has the RX 6700 XT / RADV;
Vulkan render-to-texture needs no display):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace -j 2
```

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

Expected: both green. The headless gate must stay green **independently**
(CI has no adapter); the GPU lane is additive and must pass on this host
before the phase merges. A GPU-needing test without `#[ignore]` panics the
headless run at adapter init — self-policing.

---

## Orientation: verified facts this plan builds on

Verified against the as-built tree at `f17e770`. **The quad-path mechanics
below are THE mirror target — every T8 design choice is "do what the quads
do, derived the way the text quads derive."**

### 1. The quad partition as-built (`render/buckets.rs:220–362`)

`pack_view_partitioned(nodes, group_count, text_quads)` walks the fresh node
list in paint order, pushes each opaque node instance (and, immediately
after, its entity's spliced text quads) into a `Partitioner` under
`node.group.filter(|&g| g < group_count)`, and returns `PackedPartition {
instances, group_ranges, flat_ranges }`. `group_ranges[g]` is group `g`'s
contiguous `[start, end)` instance range (`0..0` when it has no opaque
member); `flat_ranges` is the exact complement as maximal runs.
**Contiguity holds by construction** (buckets.rs:186–195): every SC-forming
effect former is a stacking context (layout trigger 5), so a group's subtree
is one atomic `painters_z` entry and extract emits it as a contiguous run;
the `debug_assert_eq!` at buckets.rs:331–340 stays as a tripwire. The run
bookkeeping (group-range extension + flat-run coalescing + the tripwire)
lives in the private `Partitioner` struct (buckets.rs:277–362) — Task 2
hoists it into a blob-free `RangePartitioner` both paths share.

### 2. The group-membership source (`render/extract.rs:482–636`)

`extract_buiy_nodes` is the ONLY system that resolves effect-group
membership. Mechanics that the prepare-time derivation inherits for free:

- `group_formers` collects `(EffectReason, opacity)` per former entity —
  **after** the paint-skip `continue` (extract.rs:501–510), so a
  paint-skipped former is not a group. Replicating this subtlety in a second
  system is exactly the drift hazard D1 avoids.
- `nearest_group_entity` (extract.rs:529–540) climbs `ChildOf` to the
  nearest former (a former is its OWN group). SC-agnostic; also covers the
  `backdrop-filter` former.
- Group indices are **entity-sorted** (extract.rs:544–550, deterministic),
  and `ExtractedNode.group` (extract.rs:619–636) tags every assembled node
  with its nearest-former index. The index is the position in
  `ExtractedEffectGroups.0` — the same index `BuiyInstanceBuffers::
  group_ranges[g]` and `PreparedEffectGroups.groups[g]` use.
- **Fact (a) (§ 4.6):** `ExtractedNodes.nodes` retains a record for EVERY
  painted entity even with no background (`Color::NONE` records — only the
  pack skips the quad), so every text entity has a node record carrying its
  group. The entity→group lookup at prepare is total over live text
  entities.
- The nodes probe union already contains `Changed<EffectGroup>` and
  `Changed<Opacity>` (extract.rs:393–394): **any group flip marks the nodes
  carrier dirty**, which is what lets the glyph partition recompute ride the
  quad gate with no new glyph-probe members.

### 3. The glyph producer (`text/extract.rs` @ f17e770)

- Emission order is the SAME walk: `context_roots` +
  `context_tree_paint_order` (extract.rs hoisted them "so the two walks can
  never diverge", render/extract.rs:218–222). Therefore a group's text
  entities' instances are contiguous in `ExtractedGlyphs.glyphs` by the same
  SC-atomicity argument as quads — `RangePartitioner`'s tripwire holds.
- **All of one entity's glyph-tier instances are contiguous**: run glyphs,
  buffered line-through stamps (appended after each run's glyphs), and the
  caret stamp (appended last) are all emitted inside the one
  `for entity in order` iteration (text/extract.rs:352–662). One
  `GlyphEntityRun` per entity is exact.
- The producer sits at **Bevy's 16-param cap** (text/extract.rs:163–166 —
  carriers and removal streams are already tupled to fit). D1's "no new
  producer queries" is a hard constraint, not a preference.
- Publication is **value-compared per carrier** (text/extract.rs:670–689,
  T7 decision 4): a content-identical rebuild keeps the carrier's tick. The
  vanished-window arm clears once (text/extract.rs:273–282). Both must
  learn `entity_runs` in lockstep (Task 1).

### 4. The damage gates as-built (`render/prepare.rs:150–226`)

The quad buffer re-packs + re-uploads under
`nodes.is_changed() || groups.is_changed() || text_quads.is_changed()`; the
glyph buffer re-uploads under `glyphs.is_changed()` ALONE. "A caret blink
(T7) re-uploads glyphs only" is a load-bearing comment (prepare.rs:174–175)
— T8's partition recompute must not weaken it, which is why the recompute is
CPU-only under the union (D2) while the uploads keep their separate gates.
`BuiyInstanceBuffers` is `init_resource`'d in the plugin build
(render/mod.rs), so the new range fields default-construct everywhere the
buffers do.

### 5. The specialization machinery (one mechanism, already format-keyed)

- `BuiyPrimitives::specialize(BuiyPrimitiveKey { kind, format, samples })`
  (render/primitive.rs:219–290) already builds the Glyph variant for ANY
  target format: `kind: Glyph` selects `coverage.wgsl`, the 68 B
  `GlyphAlphaInstance` vertex layout, and the additive atlas `@group(1)`;
  `format`/`samples` land in `ColorTargetState`/`MultisampleState`. The
  `Glyph@Rgba16Float@1` key is **new key, zero new descriptor code**.
- `prepare_effect_groups` (render/compositor.rs:451–466) already
  specializes `Quad@Rgba16Float@1` through the shared
  `BuiySpecializedPipelines.primitives` cache and stores the id on
  `PreparedEffectGroups.quad_pipeline` for the node to read (prepare owns
  the mutable cache; the node's `&World` cannot). The glyph id is one more
  `specialize` call + one more field.
- The per-group view bind group (`buiy_group_view_bind_group`, node.rs:145–
  149) is built against `buiy_pipeline.view_layout` — the SAME `@group(0)`
  layout both the quad and glyph pipelines declare
  (`view_uniform_layout_descriptor`, primitive.rs:232–235), so it stays
  bound and valid across a `set_render_pipeline` switch inside the group
  pass. The glyph pipeline additionally needs the atlas `@group(1)`
  (`AtlasGpu::coverage_bind_group()`), exactly as in the flat draw
  (node.rs:298–310).
- Group targets are single-sampled (`group_target_descriptor`,
  `sample_count: 1`), so the glyph group key is `samples: 1` — only the
  window pass keys the view's `Msaa` (via `BuiyViewPipelines`).

### 6. The node as-built (`render/node.rs:110–315`) — what changes

- **Step 1** (node.rs:110–175): per group, skip if
  `placement.instance_range` (the QUAD range) is empty
  (`range.start == range.end → continue`, node.rs:130–133), else clear the
  target and draw the quad range. **Consequence today: a backgroundless,
  decoration-less text card forms a group whose quad range is empty — the
  whole step-1 pass is skipped and its glyphs are also absent from the flat
  draw's exclusion (they draw flat, undimmed).** T8's restructure must skip
  only when BOTH the quad and glyph ranges are empty (D5), and gate each
  half's draw on its own pipeline/buffer readiness.
- **Flat glyph draw** (node.rs:280–310): `draw(0..4, 0..glyph_count)` plus
  the `TODO(text-seam)` block (node.rs:289–297) this plan deletes. Becomes
  an iteration over `glyph_flat_ranges` — when no group is live the
  partition is the single full run, so the flat path stays byte-for-byte the
  pre-T8 draw (the quad precedent, node.rs:264–277).
- Steps 2a/2b/3 (the composites) are **untouched** — a group target that now
  also contains glyph ink composites identically.

### 7. The pinned dim-asymmetry test that FLIPS (`tests/text_decoration_gpu.rs:424–513`)

`opacity_group_dims_the_underline_but_not_the_line_through` pins § 4.5's
asymmetry AS EXPECTED, with the flip pre-announced in its own comments
("T8 flips THIS assertion — keep the two halves adjacent so it fails HERE,
loudly"). Post-T8 facts the flip must encode:

- Half 1 (underline dimmed via the quad splice) **stays as-is**.
- Half 2 inverts: the line-through stamp now rides the group's glyph range —
  `is_full_red` rows must be ABSENT, and a dimmed (`is_present_red`,
  ≈ sRGB 188 red: `composite_src_over(red, black, 0.5)`) band must sit over
  the ink.
- **The ink itself dims**, so `is_white` (all channels ≥ 200) finds nothing
  and the file's `white_rows` helper (which `expect`-panics on zero white
  rows) cannot locate the ink envelope in this fixture. Full-coverage dimmed
  white reads ≈ 188/channel (linear 0.5 → sRGB); the old ≥ 200 threshold
  (≈ 73 % coverage undimmed) maps to ≈ 162 dimmed — a `≥ 160` all-channel
  predicate recovers the same envelope. The flip adds `is_dim_white` +
  `dim_white_rows` and asserts zero `is_white` rows (the
  "dims exactly once" guard: undimmed 255 = bypass or double-paint).

T7 added no caret-in-group pin (its honesty pins deferred to this same T6
test), so this is the ONLY flipping test; the caret half of § 4.5 gets its
first end-to-end coverage in Task 4's new composite golden.

### 8. Harnesses + instruments (the seats for Tasks 5–6)

- `TextExtractHarness` (tests/support/extract_harness.rs): adapterless
  producer harness; `frame()` = one main-world `app.update()` (TextSync →
  measure → TextCommit) + one manual `ExtractSchedule` run.
  `GlyphChangeLog.changed_frames` mirrors the prepare glyph gate exactly —
  the typing-latency fixture's publish counter already exists.
- Main-world instruments: `TextSyncAppliedCount` (text/sync.rs:63),
  `TextMeasureCallCount` (text/measure.rs:34), `LayoutTaffyComputeCount`
  (layout/systems.rs:110) — all `pub usize`/`u32` newtype resources.
- `support::render_world_resource::<R>(&app)` (support/mod.rs:100) reads a
  render-world resource from a GPU test; `RtPoolStats`
  (render/compositor.rs:367–373) is the established "observable render-world
  stat for a test" precedent `BufferUploadStats` copies.
- The blink-edge GPU idiom (tests/text_selection_caret_gpu.rs:224–363):
  pause `Time<Virtual>`, `advance_by(500 ms)`, `app.update()` — the
  render-prep writer flips `CaretVisual` on the edge inside that update and
  the producer rebuilds the same frame.

---

## Decisions (with runner-ups) — read before implementing

1. **D1 — Glyph group tagging: prepare-time derivation from the fresh node
   list, keyed by per-entity instance runs.** The producer appends one
   `GlyphEntityRun { entity, instances }` per emitting entity (zero new
   system params); `prepare_buiy_instances` maps each run's entity to
   `ExtractedNode.group` off the CURRENT `ExtractedNodesView` and partitions
   the instance indices. *Runner-up A — producer-side climb* (query
   `Option<&EffectGroup>` + `ChildOf` in `extract_buiy_glyphs`, replicate
   `nearest_group_entity` + the entity-sorted index assignment): rejected —
   two systems independently computing group indices that must agree exactly
   (including the paint-skipped-former exclusion, Orientation § 2), the
   16-param cap (Orientation § 3), and probe-union growth
   (`Changed<EffectGroup>`/`Changed<Opacity>` would have to join the glyph
   union; under D1 a group flip rides the nodes gate and the glyph instances
   correctly don't rebuild at all — their bytes don't depend on the group).
   *Runner-up B — record the group index into the carrier/instances at
   emission*: rejected for exactly § 4.6's rejected-runner-up reason — a
   recorded index goes stale whenever the node walk rebuilds (a group
   forming on a non-text frame) while the glyph carrier is retained; and a
   group field in the GPU vertex record would force a glyph re-upload on
   every group flip, breaking the independent gate. The fresh-walk
   derivation is immune by construction.
2. **D2 — Partition recompute under the UNION gate, CPU-only.** Ranges
   recompute when `quad_dirty || glyphs.is_changed()` (a group can form or
   drop on a node-only frame while glyphs are retained — glyph-gate-only
   recompute would draw stale ranges; the nodes union already contains
   `Changed<EffectGroup>`/`Changed<Opacity>`, Orientation § 2). The
   recompute touches no GPU buffer, so steady frames stay O(0) and a blink
   frame still uploads the glyph buffer only. *Runner-up — recompute every
   frame*: rejected, an O(entities) walk per steady frame against the
   gate-#14 budget for zero benefit. *Runner-up — fold into the quad gate
   only*: rejected, a glyph-only rebuild (blink) changes instance counts and
   run boundaries; its ranges must follow.
3. **D3 — Hoist `Partitioner`'s run bookkeeping into a blob-free
   `RangePartitioner` shared by both paths.** The contiguity tripwire, the
   group-range extension, and the flat-run coalescing live ONCE;
   `Partitioner` becomes blob + `RangePartitioner`. *Runner-up — duplicate
   the ~50 lines for glyphs*: rejected, the tripwire message and semantics
   would drift (the exact failure mode the § 4.6 splice avoided by
   construction).
4. **D4 — One carrier, one tick: the value-compared publish covers
   `(glyphs, entity_runs)` together.** Instance bytes can coincide across
   different entity sets (despawn + respawn of an identical fixture), and
   group membership keys on the ENTITY — so runs inequality must republish
   even when instances compare equal, or prepare would never re-derive the
   partition. *Runner-up — separate resources/ticks per field*: rejected,
   two ticks for one logically-atomic carrier complicates every gate reader
   for nothing.
5. **D5 — The step-1 skip becomes "BOTH ranges empty"; each half gates on
   its own readiness.** A backgroundless text card is a group with an empty
   quad range and a non-empty glyph range (Orientation § 6) — the campaign's
   headline fixture. The pass structure: resolve quad/glyph pipelines +
   buffers + the atlas bind group up front as `Option`s; skip the group only
   when there is nothing to draw; draw quads then glyphs, each behind its
   own `Option` chain (async-compile frames skip that half, the established
   behavior class).
6. **D6 — Within-group order: all the group's quads, then all its glyphs.**
   Mirrors the global flat rank (shadow < quad < glyph) and § 11.3's
   limitation, now scoped per region. No interleaving machinery.
7. **D7 — The latency fixture is HEADLESS, on the extract harness.** The
   campaign listed T8's "latency fixture's budget wiring" under the GPU
   lane, but the budget *numbers* are explicitly deferred to
   `buiy-verification-design` (campaign + verification § 4), leaving the
   MECHANISM — one frame from `Text` edit to `ExtractedGlyphs` publish — as
   T8's assertion, and the mechanism is fully observable adapterless
   (`GlyphChangeLog` mirrors the prepare gate; CLAUDE.md: tests at the
   lowest tier covering the behavior). The GPU half of T8's damage surface
   is the blink-reupload assert (Task 5), which genuinely needs
   `write_buffer`.
8. **D8 — Blink-reupload observed via a `BufferUploadStats` render-world
   instrument** (the `RtPoolStats` precedent: prepare records, the test
   reads through `render_world_resource`). *Runner-up — inspect
   `RawBufferVec` internals*: not exposed. *Runner-up — a change-tick logger
   system in the real render app*: equivalent power, but the stats resource
   is reusable by `buiy-verification-design`'s gate-#14 wiring and reads as
   one line in prepare.
9. **D9 — Degraded groups: mirror the quad semantics (skip), file the
   discrepancy.** See the honesty pin. Fixing degraded-group fallback is a
   render-compositor follow-up touching the quad path too — out of T8's
   charter.

---

## File structure

```
crates/buiy_core/src/
├── render/
│   ├── buckets.rs        — RangePartitioner hoist + partition_glyph_ranges (Task 2)
│   ├── prepare.rs        — GlyphEntityRun + ExtractedGlyphs.entity_runs (Task 1);
│   │                       glyph_group_ranges/glyph_flat_ranges + union recompute (Task 3);
│   │                       BufferUploadStats (Task 5)
│   ├── compositor.rs     — PreparedEffectGroups.glyph_pipeline,
│   │                       GroupPlacement.glyph_range (Task 4)
│   ├── node.rs           — step-1 glyph draw, flat complement draw,
│   │                       TODO(text-seam) resolved (Task 4)
│   └── mod.rs            — init_resource::<BufferUploadStats> (Task 5)
└── text/
    └── extract.rs        — entity-run tracking + lockstep publish/clear (Task 1)

crates/buiy_core/tests/
├── text_extract.rs           — entity-run shape + identity-republish (Task 1)
├── render_buckets.rs         — partition_glyph_ranges unit tests (Task 2)
├── text_effect_group_gpu.rs  — NEW: partition wiring assert (Task 3) +
│                               the text-in-group composite golden (Task 4)
├── text_decoration_gpu.rs    — the § 4.5 asymmetry test FLIPS (Task 4)
├── text_selection_caret_gpu.rs — blink-reupload GPU assert (Task 5)
└── text_typing_latency.rs    — NEW: gate-#14 mechanism fixture (Task 6)

docs/ — follow-ups.md, the campaign plan, glyph-pipeline.md § 11,
        decoration-and-paint.md §§ 4.5/6.3, verification.md §§ 1.3/4,
        README.md (Task 7)
```

---

### Task 1: Producer — per-entity instance runs on `ExtractedGlyphs` (headless)

The carrier gains entity attribution; the producer records it; publish and
the vanished-window clear stay lockstep. No behavior change downstream yet.

**Files:**
- Modify: `crates/buiy_core/src/render/prepare.rs`
- Modify: `crates/buiy_core/src/text/extract.rs`
- Modify: `crates/buiy_core/tests/text_extract.rs`

- [ ] **Step 1: Write the failing tests** (`tests/text_extract.rs`):

  ```rust
  /// T8 D1: the producer attributes every instance to its source entity as
  /// one contiguous run per entity, in emission (paint) order, covering the
  /// instance vec exactly — the input the prepare-time group partition
  /// derives from the FRESH node list (decoration-and-paint § 4.6 applied
  /// to the glyph buffer).
  #[test]
  fn entity_runs_cover_all_instances_one_run_per_entity() {
      let mut h = TextExtractHarness::new();
      let a = spawn_text(&mut h); // "Hi!" → 3 instances
      // A second text sibling under the same root → a second, later run.
      let b = h
          .app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::from("Yo")), FontSize(16.0)))
          .id();
      let root = h.app.world_mut().query_filtered::<Entity, With<Node>>(); // see note
      // (in practice: keep a handle on the root from spawn_text and
      // add_child(b) — adjust spawn_text to return (text, root) or inline
      // the fixture; the assertion below is what matters)
      h.settle();

      let glyphs = h.glyphs();
      let runs = &glyphs.entity_runs;
      assert_eq!(runs.len(), 2, "one run per emitting entity");
      // Contiguous cover of [0, len), in emission order.
      let mut next = 0u32;
      for run in runs {
          assert_eq!(run.instances.start, next, "runs are gapless from 0");
          assert!(run.instances.start < run.instances.end, "runs are non-empty");
          next = run.instances.end;
      }
      assert_eq!(next as usize, glyphs.glyphs.len(), "runs cover every instance");
      // Attribution: the two runs name the two entities, in paint order.
      let entities: Vec<Entity> = runs.iter().map(|r| r.entity).collect();
      assert!(entities.contains(&a) && entities.contains(&b));
  }

  /// An entity emitting no instance (whitespace-only) gets NO run.
  #[test]
  fn whitespace_only_entity_emits_no_run() {
      let mut h = TextExtractHarness::new();
      // same shape as spawn_text but Text("   ")
      // …spawn…
      h.settle();
      assert_eq!(h.glyph_count(), 0);
      assert!(h.glyphs().entity_runs.is_empty());
  }

  /// D4: instance bytes can coincide across DIFFERENT entities (despawn +
  /// respawn an identical fixture in one frame) — the runs compare must
  /// republish so prepare re-derives the group partition for the new
  /// entity. Without entity_runs in the publish compare this is the silent
  /// stale-partition bug.
  #[test]
  fn respawn_with_identical_instances_republishes_for_entity_identity() {
      let mut h = TextExtractHarness::new();
      let (text, root) = /* spawn_text variant returning the root too */;
      h.settle();
      let before = h.glyphs().glyphs.clone();
      let publishes = h.changed_frames();

      // Despawn + respawn the IDENTICAL leaf in one main-world step: the
      // rebuild (despawn fires RemovedComponents<ResolvedLayout>) sees
      // byte-identical instances under a NEW entity id.
      h.app.world_mut().entity_mut(text).despawn();
      let text2 = h
          .app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::from("Hi!")), FontSize(16.0)))
          .id();
      h.app.world_mut().entity_mut(root).add_child(text2);
      h.frame();

      // Precondition: the pixels really are identical (same layout slot,
      // same font, atlas-resident) — if this ever fails the fixture, not
      // the contract, needs adjusting.
      assert_eq!(h.glyphs().glyphs, before, "identical instance bytes (precondition)");
      assert_eq!(h.glyphs().entity_runs.len(), 1);
      assert_eq!(h.glyphs().entity_runs[0].entity, text2, "the run names the NEW entity");
      assert!(
          h.changed_frames() > publishes,
          "the publish fired on runs inequality despite equal instance bytes (D4)"
      );
  }
  ```

  Also extend ONE existing steady-state test (e.g.
  `emits_one_instance_per_visible_glyph_with_resident_keys`) with
  `assert_eq!(h.glyphs().entity_runs.len(), 1)` so the field is exercised on
  the happy path. Run: red (`entity_runs` doesn't exist).

- [ ] **Step 2: The carrier** (`render/prepare.rs`, beside `ExtractedGlyphs`):

  ```rust
  /// One emitting entity's contiguous slice of [`ExtractedGlyphs::glyphs`]
  /// (T8 / D1). The producer emits each entity's glyph-tier instances (run
  /// glyphs, line-through stamps, the caret stamp) inside one walk
  /// iteration, so one run per entity is exact; runs are gapless from 0 and
  /// cover the instance vec. The prepare partition maps `entity` to its
  /// `ExtractedNode.group` off the FRESH node list — group membership is
  /// never recorded here (it would go stale against node-walk rebuilds, the
  /// § 4.6 rejected runner-up).
  #[derive(Clone, Debug, PartialEq)]
  pub struct GlyphEntityRun {
      /// The source main-world entity — the group-lookup key.
      pub entity: Entity,
      /// This entity's instance indices in [`ExtractedGlyphs::glyphs`].
      pub instances: Range<u32>,
  }
  ```

  `ExtractedGlyphs` grows the field (doc comment per above; emission order;
  covering):

  ```rust
  #[derive(Resource, Default)]
  pub struct ExtractedGlyphs {
      pub glyphs: Vec<GlyphAlphaInstance>,
      /// One run per emitting entity, in paint order, covering `glyphs`
      /// exactly (empty ⇔ `glyphs` empty). Published in lockstep with
      /// `glyphs` under ONE change tick (D4).
      pub entity_runs: Vec<GlyphEntityRun>,
  }
  ```

- [ ] **Step 3: The producer** (`text/extract.rs`):

  - Add `new_runs` beside `new_glyphs`/`new_quads`:
    `let mut new_runs: Vec<GlyphEntityRun> = Vec::new();`
  - At the top of the per-entity body (right after the `skip.is_some()`
    `continue`): `let entity_start = new_glyphs.len() as u32;`
  - At the END of the per-entity body (after the caret emission block):

    ```rust
    // T8 D1: attribute this entity's contiguous instance slice. The
    // prepare partition maps the entity to its ExtractedNode.group off
    // the fresh node list — the producer learns nothing about groups.
    let entity_end = new_glyphs.len() as u32;
    if entity_end > entity_start {
        new_runs.push(GlyphEntityRun {
            entity,
            instances: entity_start..entity_end,
        });
    }
    ```

  - The publish becomes a ONE-tick lockstep compare (D4):

    ```rust
    if glyphs.glyphs != new_glyphs || glyphs.entity_runs != new_runs {
        let glyphs = &mut *glyphs;
        glyphs.glyphs = new_glyphs;
        glyphs.entity_runs = new_runs;
    }
    ```

  - The vanished-window clear adds `glyphs.entity_runs.clear();` inside the
    existing once-guarded branch (and its guard grows
    `|| !glyphs.entity_runs.is_empty()` for symmetry).
  - Import `GlyphEntityRun` from `crate::render::prepare`.

- [ ] **Step 4: Run the new tests — green. Run BOTH gate lanes — green.**
  (`entity_runs` is additive; nothing constructs `ExtractedGlyphs` by struct
  literal outside the definition — verified at f17e770.) Commit:
  `feat(text): T8 — per-entity instance runs on ExtractedGlyphs (D1 input)`.

---

### Task 2: The pure partition — `RangePartitioner` + `partition_glyph_ranges` (headless)

The quad packer's run bookkeeping is hoisted blob-free and reused; the new
function maps entity runs + a group lookup to `(group_ranges, flat_ranges)`.
Pure CPU, fully headless.

**Files:**
- Modify: `crates/buiy_core/src/render/buckets.rs`
- Modify: `crates/buiy_core/tests/render_buckets.rs`

- [ ] **Step 1: Write the failing tests** (`tests/render_buckets.rs`; entity
  helper idiom already in the file — `Entity::from_raw_u32(n).unwrap()`):

  ```rust
  fn run(entity: u32, range: Range<u32>) -> (Entity, Range<u32>) {
      (Entity::from_raw_u32(entity).unwrap(), range)
  }

  /// No live group: ONE flat run covering everything — the flat glyph draw
  /// stays byte-for-byte the pre-T8 `0..glyph_count` (the quad precedent).
  #[test]
  fn glyph_partition_no_groups_is_single_full_flat_run() {
      let (groups, flat) =
          partition_glyph_ranges([run(1, 0..3), run(2, 3..5)], 5, 0, |_| None);
      assert!(groups.is_empty());
      assert_eq!(flat, vec![0..5]);
  }

  /// A grouped middle entity: its range lands in group_ranges[g]; the flat
  /// complement is the two surrounding maximal runs.
  #[test]
  fn glyph_partition_grouped_middle_run() {
      let g = |e: Entity| (e == Entity::from_raw_u32(2).unwrap()).then_some(0);
      let (groups, flat) =
          partition_glyph_ranges([run(1, 0..2), run(2, 2..6), run(3, 6..9)], 9, 1, g);
      assert_eq!(groups, vec![2..6]);
      assert_eq!(flat, vec![0..2, 6..9]);
  }

  /// A group with no glyph-emitting member keeps its empty `0..0` slot at
  /// its index (the group_ranges[g] == prepared group g alignment).
  #[test]
  fn glyph_partition_empty_group_slot() {
      let (groups, flat) = partition_glyph_ranges([run(1, 0..4)], 4, 2, |_| Some(1));
      assert_eq!(groups, vec![0..0, 0..4]);
      assert!(flat.is_empty());
  }

  /// Adjacent same-group entities coalesce into one contiguous group range
  /// (two text entities inside one card).
  #[test]
  fn glyph_partition_coalesces_adjacent_same_group_runs() {
      let (groups, flat) =
          partition_glyph_ranges([run(1, 0..2), run(2, 2..5)], 5, 1, |_| Some(0));
      assert_eq!(groups, vec![0..5]);
      assert!(flat.is_empty());
  }

  /// An out-of-bounds group index is filtered to flat — the
  /// `pack_view_partitioned` `g < group_count` filter, mirrored.
  #[test]
  fn glyph_partition_out_of_bounds_group_is_flat() {
      let (groups, flat) = partition_glyph_ranges([run(1, 0..3)], 3, 1, |_| Some(7));
      assert_eq!(groups, vec![0..0]);
      assert_eq!(flat, vec![0..3]);
  }

  /// The producer contract is load-bearing: gapless runs from 0 covering
  /// `total`. A gap is a producer bug — caught loudly in debug builds.
  #[test]
  #[should_panic(expected = "entity runs must be contiguous")]
  fn glyph_partition_gap_trips_the_debug_assert() {
      let _ = partition_glyph_ranges([run(1, 0..2), run(2, 3..4)], 4, 0, |_| None);
  }
  ```

  Run: red (no `partition_glyph_ranges`).

- [ ] **Step 2: Hoist `RangePartitioner`** (`render/buckets.rs`) — the run
  bookkeeping of `Partitioner`, blob-free, with the contiguity tripwire
  comment + message moved VERBATIM (it is the load-bearing documentation):

  ```rust
  /// The instance-index run bookkeeping shared by the quad packer
  /// ([`Partitioner`]) and the glyph partition
  /// ([`partition_glyph_ranges`]): per-group contiguous ranges (with the
  /// contiguity tripwire) and the complement flat runs. Blob-free — it
  /// tracks indices only, so the glyph path (whose instances already live
  /// in `ExtractedGlyphs`) reuses the exact quad semantics without copying
  /// bytes.
  pub(crate) struct RangePartitioner {
      next: u32,
      group_ranges: Vec<Range<u32>>,
      flat_ranges: Vec<Range<u32>>,
      /// Tracks the group of the previous index to coalesce contiguous runs.
      run_group: Option<Option<usize>>,
  }

  impl RangePartitioner {
      pub(crate) fn new(group_count: usize) -> Self {
          Self {
              next: 0,
              group_ranges: vec![0..0; group_count],
              flat_ranges: Vec::new(),
              run_group: None,
          }
      }

      /// Claim the next instance index under group `g` (already
      /// bounds-filtered by the caller), extending or starting the
      /// group/flat run it belongs to.
      pub(crate) fn push(&mut self, g: Option<usize>) {
          let idx = self.next;
          self.next += 1;
          match g {
              Some(gi) => {
                  let r = &mut self.group_ranges[gi];
                  if r.start == r.end {
                      *r = idx..idx + 1; // first member of this group
                  } else {
                      // CONTIGUITY INVARIANT … (the buckets.rs:305–340
                      // comment block + debug_assert_eq!, moved verbatim —
                      // including the trailing "Text quads cannot trip it"
                      // sentence, which Task 4's docs pass extends to
                      // glyph runs: a glyph run adopts its entity's node
                      // group at the same paint position.)
                      debug_assert_eq!(r.end, idx, /* …verbatim message… */);
                      r.end = idx + 1;
                  }
              }
              None => {
                  if self.run_group == Some(None) {
                      self.flat_ranges.last_mut().expect("open flat run").end = idx + 1;
                  } else {
                      self.flat_ranges.push(idx..idx + 1);
                  }
              }
          }
          self.run_group = Some(g);
      }

      pub(crate) fn finish(self) -> (Vec<Range<u32>>, Vec<Range<u32>>) {
          (self.group_ranges, self.flat_ranges)
      }
  }
  ```

  `Partitioner` shrinks to `{ instances: Vec<[f32; 13]>, ranges:
  RangePartitioner }`; its `push` forwards (`self.instances.push(instance);
  self.ranges.push(g);`), its `finish` destructures. **The quad path's
  output must be byte-identical** — the existing `pack_view_partitioned`
  tests are the refactor pin; touch none of them.

- [ ] **Step 3: `partition_glyph_ranges`** (`render/buckets.rs`):

  ```rust
  /// Partition the glyph instance buffer into per-effect-group contiguous
  /// ranges + the flat complement (T8 — the quad path's
  /// [`pack_view_partitioned`] partition applied to glyphs). `runs` is the
  /// producer's per-entity attribution (`ExtractedGlyphs::entity_runs` as
  /// `(entity, instance range)` pairs — carrier-agnostic so this module
  /// stays decoupled from the carrier type), `total` the instance count,
  /// and `group_of` resolves an entity to its `ExtractedNode.group` off
  /// the FRESH node list (decoration-and-paint § 4.6: membership derives
  /// from the node record at pack time, never from recorded indices —
  /// stale-proof by construction). Contiguity per group holds because the
  /// glyph producer walks the SAME `context_tree_paint_order` as the node
  /// extract (an SC-forming group's subtree is one atomic run in both);
  /// the [`RangePartitioner`] tripwire guards the residual drift cases
  /// exactly as it does for quads.
  ///
  /// An entity `group_of` cannot resolve maps to FLAT — a transient
  /// impossibility (despawn/paint-skip fire BOTH probe unions, so the two
  /// carriers rebuild together; fact (a): every painted entity has a node
  /// record), kept as the conservative fallback rather than a drop because
  /// the instances are already in the buffer.
  pub fn partition_glyph_ranges(
      runs: impl IntoIterator<Item = (Entity, Range<u32>)>,
      total: u32,
      group_count: usize,
      group_of: impl Fn(Entity) -> Option<usize>,
  ) -> (Vec<Range<u32>>, Vec<Range<u32>>) {
      let mut p = RangePartitioner::new(group_count);
      let mut covered = 0u32;
      for (entity, range) in runs {
          debug_assert_eq!(
              range.start, covered,
              "entity runs must be contiguous from 0 (the producer emits one \
               run per entity, gapless, in emission order)"
          );
          covered = range.end;
          let g = group_of(entity).filter(|&g| g < group_count);
          for _ in range {
              p.push(g);
          }
      }
      debug_assert_eq!(
          covered, total,
          "entity runs must cover every glyph instance"
      );
      (p.group_ranges, p.flat_ranges) // via finish()
  }
  ```

- [ ] **Step 4: Run the new tests + the whole `render_buckets` suite —
  green (refactor pin included). Run BOTH gate lanes — green.** Commit:
  `feat(render): T8 — RangePartitioner hoist + the glyph-range partition`.

---

### Task 3: Prepare wiring — `glyph_group_ranges`/`glyph_flat_ranges` under the union gate (GPU assert)

`BuiyInstanceBuffers` carries the glyph partition; `prepare_buiy_instances`
recomputes it CPU-only whenever EITHER carrier changed. Nothing consumes the
ranges yet (the node still draws `0..glyph_count`), so every existing test —
including the pinned asymmetry test — stays green: this task is additive.

**Files:**
- Modify: `crates/buiy_core/src/render/prepare.rs`
- Create: `crates/buiy_core/tests/text_effect_group_gpu.rs`

- [ ] **Step 1: Write the failing GPU test** (new file
  `tests/text_effect_group_gpu.rs`; module doc: "Glyphs in effect groups
  (T8): the partition wiring + the text-in-group composite golden.
  glyph-pipeline § 11.2; decoration-and-paint § 4.5; verification § 1.3.
  All #[ignore]: need a wgpu adapter (CLAUDE.md GPU lane)."):

  ```rust
  mod support;
  // imports per text_decoration_gpu.rs (Node, Style, Text, FontSize,
  // TextColor, Opacity, GoldenConfig, ColorToken, Cow) plus:
  use buiy_core::render::prepare::BuiyInstanceBuffers;

  const W: u32 = 128;
  const H: u32 = 64;

  /// Spawn the shared two-text fixture: white "Hi" inside a BACKGROUNDLESS
  /// `Opacity(0.5)` card (top half), white "Hi" flat sibling (bottom half).
  /// Returns (grouped_text, flat_text). Backgroundless on purpose: the
  /// group's QUAD range is empty while its GLYPH range is not — the D5
  /// step-1 skip fix's pin.
  fn spawn_group_and_flat_text(app: &mut App) -> (Entity, Entity) { /* …
      absolute-inset card at (0,0) W×(H/2) wrapping the first text;
      absolute flat wrapper at (0,H/2) wrapping the second;
      one sized root holding both (the text_decoration_gpu idiom) … */ }

  #[test]
  #[ignore = "needs a wgpu adapter; T8 glyph-partition wiring (D1/D2 — ranges mirror the entity group assignment)"]
  fn glyph_partition_mirrors_the_entity_group_assignment() {
      let _cfg = GoldenConfig::deterministic();
      let mut app = support::gpu_render_app(W, H);
      // theme: white text token (the text_decoration_gpu idiom)
      let (_grouped, _flat) = spawn_group_and_flat_text(&mut app);
      let target = support::render_to_image(&mut app, W, H);
      support::spawn_capture_camera(&mut app, target.clone());
      support::finish_and_run(&mut app, 4);
      support::wait_for_text_ready(&mut app, 60);

      let buffers = support::render_world_resource::<BuiyInstanceBuffers>(&app)
          .expect("BuiyInstanceBuffers");
      // One live group; its glyph range is non-empty (the card's ink).
      assert_eq!(buffers.glyph_group_ranges.len(), 1);
      let g = buffers.glyph_group_ranges[0].clone();
      assert!(g.start < g.end, "the grouped text's glyphs got a range: {g:?}");
      // The flat complement is non-empty (the sibling's ink) and the two
      // partitions disjointly cover [0, glyph_count) — the exact quad
      // group_ranges/flat_ranges contract.
      assert!(!buffers.glyph_flat_ranges.is_empty());
      let mut all: Vec<_> = buffers.glyph_flat_ranges.clone();
      all.push(g);
      all.sort_by_key(|r| r.start);
      let mut next = 0u32;
      for r in &all {
          assert_eq!(r.start, next, "gapless disjoint cover: {all:?}");
          next = r.end;
      }
      assert_eq!(next, buffers.glyph_count, "…ending at glyph_count");
  }
  ```

  Run on the GPU lane: red (the fields don't exist).

- [ ] **Step 2: The buffer fields** (`render/prepare.rs`,
  `BuiyInstanceBuffers`):

  ```rust
  /// Per-effect-group contiguous GLYPH-instance ranges (T8 —
  /// `glyph_group_ranges[g]` = group `g`'s glyph members), the glyph
  /// mirror of [`group_ranges`](Self::group_ranges). Recomputed CPU-only
  /// under the UNION of the quad and glyph gates (D2): membership derives
  /// from the fresh node list, instance indices from the (possibly
  /// retained) glyph carrier — either side changing re-derives. The node's
  /// step-1 group pass draws each range into the group's off-screen
  /// target via the `Glyph@Rgba16Float` specialization.
  pub glyph_group_ranges: Vec<Range<u32>>,
  /// The complement: maximal runs of non-group glyph instances — the flat
  /// window glyph draw covers exactly these (a group's glyph is never
  /// painted twice). When no group is live: the single full
  /// `0..glyph_count` run, so the flat path is byte-for-byte the pre-T8
  /// draw.
  pub glyph_flat_ranges: Vec<Range<u32>>,
  ```

  (+ `Vec::new()` defaults in the `Default` impl.)

- [ ] **Step 3: The recompute** (`prepare_buiy_instances` body — name the
  gates once, reuse):

  ```rust
  let quad_dirty = nodes.is_changed() || groups.is_changed() || text_quads.is_changed();
  let glyph_dirty = glyphs.is_changed();
  if quad_dirty { /* existing quad pack + upload, unchanged */ }
  if glyph_dirty { /* existing glyph upload, unchanged */ }

  // T8 (D1/D2): derive the glyph partition from the FRESH node list —
  // group membership is the entity's ExtractedNode.group (the § 4.6
  // discipline; never recorded into the carrier). Recompute under the
  // UNION: a group can form/drop on a node-only frame while glyphs are
  // retained (Changed<EffectGroup>/<Opacity> ride the nodes probe), and a
  // glyph-only rebuild (a caret blink) moves the run boundaries. CPU-only
  // — no upload rides this branch, so the independent buffer gating (and
  // the blink-reuploads-glyphs-only property) is untouched.
  if quad_dirty || glyph_dirty {
      let group_count = groups.0.len();
      let group_by_entity: std::collections::HashMap<Entity, Option<usize>> =
          nodes.0.nodes.iter().map(|n| (n.entity, n.group)).collect();
      let (group_ranges, flat_ranges) = partition_glyph_ranges(
          glyphs.entity_runs.iter().map(|r| (r.entity, r.instances.clone())),
          glyphs.glyphs.len() as u32,
          group_count,
          |e| group_by_entity.get(&e).copied().flatten(),
      );
      buffers.glyph_group_ranges = group_ranges;
      buffers.glyph_flat_ranges = flat_ranges;
  }
  ```

  Import `partition_glyph_ranges` from `crate::render::buckets`. Note
  `glyphs.glyphs.len()` (the carrier), not `buffers.glyph_count` — always
  consistent with `entity_runs` even on a quad-dirty-only frame.

- [ ] **Step 4: GPU test green; the WHOLE GPU lane green** (in particular
  `text_decoration_gpu.rs::opacity_group_dims_the_underline_but_not_the_line_through`
  still passes — the node hasn't changed). **Headless gate green.** Commit:
  `feat(render): T8 — glyph partition wired into prepare (union gate, CPU-only)`.

---

### Task 4: The `Glyph@Rgba16Float` group draw + the flat complement + the dim flips (GPU)

The TODO resolves: the step-1 group pass draws the group's glyph range into
its target (after its quads, atlas `@group(1)` bound), the flat glyph draw
excludes group ranges, and the two GPU fixtures land/flip as ONE gate-green
unit (implementing the draw makes the T6 pinned assertion fail BY DESIGN —
the flip ships in the same task).

**Files:**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Modify: `crates/buiy_core/src/render/node.rs`
- Modify: `crates/buiy_core/tests/text_effect_group_gpu.rs`
- Modify: `crates/buiy_core/tests/text_decoration_gpu.rs`

- [ ] **Step 1: Write the failing composite golden**
  (`tests/text_effect_group_gpu.rs` — the campaign's "text-in-effect-group
  composite golden", red until the draw lands):

  ```rust
  #[test]
  #[ignore = "needs a wgpu adapter; the campaign T8 text-in-effect-group composite golden (glyph-pipeline § 11.2 closed; the verification § 1.3 row is now claimable)"]
  fn text_in_opacity_group_dims_exactly_once() {
      let _cfg = GoldenConfig::deterministic();
      let mut app = support::gpu_render_app(W, H);
      // theme: white text token + a red caret token
      let (grouped, _flat) = spawn_group_and_flat_text(&mut app);
      // A caret on the GROUPED text — § 4.5's caret half, first end-to-end
      // coverage (T7 deferred it here): a 1×24 stamp right of the ink.
      app.world_mut().entity_mut(grouped).insert((
          CaretVisual { visible: true, rect: Rect::new(60.0, 0.0, 61.0, 24.0) },
          CaretColor(ColorToken::Token(Cow::Borrowed(CARET_TOKEN))),
      ));
      app.world_mut().resource_mut::<Time<Virtual>>().pause(); // hold the phase
      let target = support::render_to_image(&mut app, W, H);
      support::spawn_capture_camera(&mut app, target.clone());
      support::finish_and_run(&mut app, 4);
      support::wait_for_text_ready(&mut app, 60);
      let frame = support::readback_rgba(&mut app, target);

      // Expectations via the CPU port (the render_compositor_gpu idiom):
      // a full-coverage group texel composited ONCE at 0.5 over the black
      // clear. "Exactly once": undimmed (≈255) means the glyph bypassed
      // the group (the pre-T8 bug) — and a double-paint (flat AND
      // composited) ALSO reads ≈255 (white over white), so the ≈188 pin
      // catches both failure modes.
      let dim = |c: Color| -> [u8; 4] {
          let lin = composite_src_over(LinearRgba::from(c), LinearRgba::new(0.0, 0.0, 0.0, 1.0), 0.5);
          let s = Srgba::from(lin);
          [ (s.red * 255.0).round() as u8, (s.green * 255.0).round() as u8,
            (s.blue * 255.0).round() as u8, 255 ]
      };
      let dim_white = dim(Color::WHITE);   // ≈ [188, 188, 188]
      let dim_red = dim(caret_red());      // ≈ [188, 0, 0] for srgb(1,0,0)
      const TOL: i32 = 4;
      let near = |p: [u8; 4], e: [u8; 4]| (0..3).all(|c| (p[c] as i32 - e[c] as i32).abs() <= TOL);

      // (1) TOP half (the card): NO undimmed-white pixel anywhere…
      assert!(top_half_pixels(&frame).all(|p| !is_white(p)),
          "no grouped ink at full strength — the glyphs rode the group target");
      // …and the dimmed ink IS present at the composite-exact value.
      assert!(top_half_pixels(&frame).any(|p| near(p, dim_white)),
          "the grouped ink is present at exactly composite_src_over(white, black, 0.5)");
      // (2) The caret column dims identically (stamps are glyph instances).
      assert!(top_half_pixels(&frame).any(|p| near(p, dim_red)),
          "the grouped caret stamp dims with its group");
      assert!(top_half_pixels(&frame).all(|p| !(p[0] >= 230 && p[1] <= 20 && p[2] <= 20)),
          "no full-strength caret pixel — the stamp did not bypass the group");
      // (3) BOTTOM half (the flat sibling): full-strength ink survives —
      // the flat complement still draws non-group glyphs.
      assert!(bottom_half_pixels(&frame).any(is_white),
          "the flat sibling's ink stays undimmed (the complement draw)");
  }
  ```

  (`top_half_pixels`/`bottom_half_pixels` = trivial iterator helpers over
  the readback; `is_white` = the all-channels-≥200 predicate from
  text_decoration_gpu.) **The backgroundless card means the group's quad
  range is EMPTY — this golden simultaneously pins the D5 skip fix.** Run on
  the GPU lane: red (grouped ink reads 255).

- [ ] **Step 2: Compositor prepare growth** (`render/compositor.rs`):

  - `PreparedEffectGroups` gains:

    ```rust
    /// The `Glyph@Rgba16Float` pipeline id the step-1 group pass binds to
    /// draw the group's GLYPH range into its target (T8 — the glyph
    /// mirror of [`quad_pipeline`](Self::quad_pipeline); same machinery:
    /// specialized in prepare, the node only reads; `None` until the
    /// pipeline async-compiles).
    pub glyph_pipeline: Option<CachedRenderPipelineId>,
    ```

  - `GroupPlacement` gains:

    ```rust
    /// The glyph-instance range (`BuiyInstanceBuffers::glyph_group_ranges`
    /// index == this group's extract index) the step-1 pass draws into the
    /// target AFTER the quad range (T8; within-group order mirrors the
    /// global shadow < quad < glyph rank).
    pub glyph_range: Range<u32>,
    ```

  - In `prepare_effect_groups`, beside the existing `quad_pipeline`
    specialization (same key shape, `kind: Glyph`):

    ```rust
    let glyph_pipeline = Some(group_pipelines.primitives.specialize(
        &pipeline_cache,
        &BuiyPrimitives,
        BuiyPrimitiveKey {
            kind: BuiyPrimitiveKind::Glyph,
            format: TextureFormat::Rgba16Float,
            samples: 1,
        },
    ));
    ```

    …threaded into the `PreparedEffectGroups` literal; and in the per-group
    loop, beside `instance_range`:

    ```rust
    let glyph_range = buffers.glyph_group_ranges.get(i).cloned().unwrap_or(0..0);
    ```

    …into the `GroupPlacement` literal. (Ordering already correct:
    `prepare_effect_groups` runs `.after(prepare_buiy_instances)`, so the
    glyph ranges are written first — the same reason `group_ranges` works.)

- [ ] **Step 3: The node** (`render/node.rs`) — restructure step 1 (D5/D6)
  and the flat glyph draw:

  Step 1 (replacing node.rs:120–175; the per-group uniform/bind-group/pass
  creation is unchanged — only the gating and the draws move):

  ```rust
  // Effect-group composite — step 1: each group's DIRECT members rasterize
  // into the group's own off-screen `Rgba16Float` target — its QUADS, then
  // its GLYPHS (T8: the within-group order mirrors the global
  // shadow < quad < glyph rank). Each half gates on its own pipeline/buffer
  // readiness so a pure-text group (empty quad range — a backgroundless
  // Opacity card) still clears + draws its glyphs and composites. … (keep
  // the existing residency/nesting commentary)
  if let (Some(prepared), Some(targets)) = (prepared, prepared_targets) {
      let group_quad_pipeline = prepared
          .quad_pipeline
          .and_then(|id| pipeline_cache.get_render_pipeline(id));
      let group_glyph_pipeline = prepared
          .glyph_pipeline
          .and_then(|id| pipeline_cache.get_render_pipeline(id));
      // The same page-0 atlas bind group the flat glyph draw binds
      // (glyph-pipeline § 11.1 — the multi-page seam is unchanged by T8).
      let atlas_bind_group = world
          .get_resource::<AtlasGpu>()
          .and_then(|a| a.coverage_bind_group());
      for group in &prepared.groups {
          let Some(target) = targets.targets.get(group.index).and_then(|t| t.as_ref()) else {
              continue; // degraded group (no target) — see follow-ups: members are skipped.
          };
          let placement = &targets.placements[group.index];
          let quad_range = placement.instance_range.clone();
          let glyph_range = placement.glyph_range.clone();
          if quad_range.is_empty() && glyph_range.is_empty() {
              continue; // nothing of EITHER tier to draw (D5).
          }
          // …existing per-group view uniform + bind group + pass creation
          // (LoadOp::Clear) verbatim…
          pass.set_bind_group(0, &group_view_bg, &[]);
          pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));
          if !quad_range.is_empty()
              && let Some(pipeline) = group_quad_pipeline
              && let Some(quad_buffer) = buffers.quad.buffer()
          {
              pass.set_render_pipeline(pipeline);
              pass.set_vertex_buffer(1, quad_buffer.slice(..));
              pass.draw(0..4, quad_range);
          }
          // T8: the group's glyphs, into the same target. `@group(0)` stays
          // bound (both pipelines declare the same view layout); the glyph
          // pipeline adds the atlas `@group(1)` exactly like the flat draw.
          if !glyph_range.is_empty()
              && let Some(pipeline) = group_glyph_pipeline
              && let Some(atlas_bg) = atlas_bind_group
              && let Some(glyph_buffer) = buffers.glyph.buffer()
          {
              pass.set_render_pipeline(pipeline);
              pass.set_bind_group(1, atlas_bg, &[]);
              pass.set_vertex_buffer(1, glyph_buffer.slice(..));
              pass.draw(0..4, glyph_range);
          }
      }
  }
  ```

  Flat glyph draw (node.rs:280–310): **delete the `TODO(text-seam)` block**
  and replace the full-buffer draw with the complement iteration, mirroring
  the quad flat-draw comment (node.rs:264–277):

  ```rust
  // --- Glyph draw (paint order: glyph after quad) ----------------------
  // … (keep the existing readiness commentary) …
  //
  // Effect-group double-paint exclusion, glyph tier (T8 — the quad
  // precedent above, verbatim semantics): draw ONLY the non-group glyph
  // ranges. A group member's glyphs rasterized into its off-screen target
  // in step 1 and composite back in step 2. `glyph_flat_ranges` is the
  // complement of `glyph_group_ranges`: with no live group it is the
  // single full `0..glyph_count` run (byte-for-byte the pre-T8 draw);
  // when every glyph is a group member it is empty and this loop is
  // correctly a no-op.
  if buffers.glyph_count > 0
      && let Some(glyph_pipeline) = pipeline_cache.get_render_pipeline(view_pipelines.glyph)
      && let Some(atlas_gpu) = world.get_resource::<AtlasGpu>()
      && let Some(atlas_bind_group) = atlas_gpu.coverage_bind_group()
      && let Some(glyph_buffer) = buffers.glyph.buffer()
  {
      pass.set_render_pipeline(glyph_pipeline);
      pass.set_bind_group(1, atlas_bind_group, &[]);
      pass.set_vertex_buffer(1, glyph_buffer.slice(..));
      for r in &buffers.glyph_flat_ranges {
          pass.draw(0..4, r.clone());
      }
  }
  ```

- [ ] **Step 4: FLIP the pinned asymmetry test**
  (`tests/text_decoration_gpu.rs:424–513`) — the implementation just made
  its half 2 fail loudly, as designed. In the SAME task:

  - Rename → `opacity_group_dims_underline_line_through_and_ink`; update the
    `#[ignore]` note ("T6 groups term + the § 4.5 asymmetry, FLIPPED by T8 —
    everything in the group dims exactly once") and the body comment (the
    line-through now rides the group's glyph range).
  - Keep half 1 (the dimmed underline: no `is_strong_red` below the ink, a
    `is_present_red` band present) **unchanged**.
  - Add the predicates + helper (module-level, beside `is_white`):

    ```rust
    /// Dimmed glyph ink: the group's white text composited ONCE at 0.5 —
    /// full coverage reads ≈ sRGB 188/channel (linear 0.5); the old
    /// `is_white` ≥ 200 threshold (≈ 73 % coverage undimmed) maps to
    /// ≈ 162 dimmed, so ≥ 160 recovers the same row envelope.
    fn is_dim_white(p: [u8; 4]) -> bool {
        (0..3).all(|ch| p[ch] >= 160)
    }

    /// The dimmed glyph-ink row envelope (the `white_rows` mirror).
    fn dim_white_rows(pixels: &[u8]) -> Range<u32> { /* rows_where(is_dim_white), first..last+1 */ }
    ```

  - Replace half 2 with the flipped assertions:

    ```rust
    // Half 2 — FLIPPED by T8: everything inside the group dims exactly
    // once. (a) The ink itself: zero undimmed-white rows anywhere.
    assert!(rows_where(&frame, is_white).is_empty(),
        "no undimmed glyph-ink row — the group's glyphs rode its target");
    let ink = dim_white_rows(&frame);
    // (b) The line-through: zero FULL-strength stamp rows anywhere…
    assert!(rows_where(&frame, is_full_red).is_empty(),
        "no full-strength line-through row — the stamp rode the group's glyph range");
    // …and the DIMMED stamp band is present over the (dimmed) ink:
    // red @ alpha 1 in the target → composite 0.5 over black ≈ sRGB 188
    // red — passes is_present_red (≥140), fails is_strong_red (≥200).
    let present_over_ink: Vec<u32> = rows_where(&frame, is_present_red)
        .into_iter()
        .filter(|&r| r >= ink.start && r < ink.end)
        .collect();
    assert!(!present_over_ink.is_empty(),
        "the dimmed line-through band sits over the ink ({ink:?})");
    ```

- [ ] **Step 5: Run the FULL GPU lane — green** (the new golden, the
  flipped test, the Task 3 wiring test, and every pre-existing GPU test —
  `render_compositor_gpu`, `render_group_contiguity_gpu`, `text_gpu`,
  `text_selection_caret_gpu` etc. must be unaffected: no-group frames are
  byte-for-byte the pre-T8 draws). **Headless gate green.** Commit:
  `feat(render): T8 — Glyph@Rgba16Float group draw; glyphs join effect-group compositing`.

---

### Task 5: The caret-blink-only-reupload GPU assertion (`BufferUploadStats`)

T7 made the blink edge change only `ExtractedGlyphs` (CPU, value-compared
publish). T8 pins the GPU half: a blink frame issues exactly one GLYPH
`write_buffer` and ZERO quad uploads — observed through a render-world
instrument, the `RtPoolStats` pattern.

**Files:**
- Modify: `crates/buiy_core/src/render/prepare.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/tests/text_selection_caret_gpu.rs`

- [ ] **Step 1: Write the failing GPU test**
  (`tests/text_selection_caret_gpu.rs`, beside the blink pair — it reuses
  that test's fixture and clock idiom):

  ```rust
  #[test]
  #[ignore = "needs a wgpu adapter; T8 caret-blink GPU damage assert (decoration-and-paint § 6.3 damage property; verification § 1.3 'Caret-blink damage' row)"]
  fn caret_blink_reuploads_the_glyph_buffer_only() {
      // The blink-pair fixture + paused virtual clock, verbatim.
      /* … spawn, finish_and_run(1), wait_for_text_ready … */
      let stats = |app: &App| {
          *support::render_world_resource::<BufferUploadStats>(app)
              .expect("BufferUploadStats")
      };

      // Drain to steady state: run frames until an update uploads nothing
      // (pipeline warm-up + the readback poller can dirty early frames).
      let mut base = stats(&app);
      for _ in 0..10 {
          app.update();
          let now = stats(&app);
          if now == base { break; }
          base = now;
      }
      // Steady frame: O(0) — neither buffer re-uploads.
      app.update();
      assert_eq!(stats(&app), base, "a steady frame uploads NOTHING");

      // The blink edge (paused clock, explicit advance — the pair test's
      // idiom): the writer flips CaretVisual, the producer rebuilds, the
      // value-compared publish leaves the quad carrier untouched…
      app.world_mut().resource_mut::<Time<Virtual>>().advance_by(Duration::from_millis(500));
      app.update();
      let after_edge = stats(&app);
      // …so prepare re-uploads the GLYPH buffer exactly once and RETAINS
      // the quad buffer — the GPU half of § 6.3's damage property (T7
      // landed the CPU half; this is the campaign's T8 assertion).
      assert_eq!(after_edge.glyph_uploads, base.glyph_uploads + 1,
          "the blink edge re-uploaded the glyph buffer exactly once");
      assert_eq!(after_edge.quad_uploads, base.quad_uploads,
          "…and did NOT touch the quad buffer");

      // The next frame is steady again (the edge writer is edge-only).
      app.update();
      assert_eq!(stats(&app), after_edge, "post-edge frame is steady");
  }
  ```

  Run on the GPU lane: red (no `BufferUploadStats`).

- [ ] **Step 2: The instrument** (`render/prepare.rs`):

  ```rust
  /// Observable render-world stat (the `RtPoolStats` idiom): cumulative
  /// per-buffer GPU upload counts from [`prepare_buiy_instances`]. The
  /// caret-blink GPU damage test (verification § 1.3) reads it through the
  /// test harness to assert a blink frame re-uploads the glyph buffer ONLY
  /// (decoration-and-paint § 6.3); `buiy-verification-design` may grow it
  /// (byte counts, percentiles) for the gate-#14 budget wiring.
  #[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
  pub struct BufferUploadStats {
      /// Quad-buffer `write_buffer` calls (the quad gate fired).
      pub quad_uploads: u64,
      /// Glyph-buffer `write_buffer` calls (the glyph gate fired).
      pub glyph_uploads: u64,
  }
  ```

  `prepare_buiy_instances` gains `mut stats: ResMut<BufferUploadStats>`;
  `stats.quad_uploads += 1;` inside the quad-dirty branch (beside its
  `write_buffer`), `stats.glyph_uploads += 1;` inside the glyph-dirty
  branch. `render/mod.rs` adds
  `.init_resource::<prepare::BufferUploadStats>()` beside the
  `BuiyInstanceBuffers` init.

- [ ] **Step 3: GPU test green; both lanes green.** Commit:
  `feat(render): T8 — BufferUploadStats + the blink-reuploads-glyphs-only GPU pin`.

---

### Task 6: The typing-latency fixture (headless — gate #14's mechanism)

ONE frame from a `Text` edit to the new `ExtractedGlyphs` publish,
frame-counted on the adapterless harness with the existing instruments
(D7). Budget NUMBERS are explicitly deferred to `buiy-verification-design`.

**Files:**
- Create: `crates/buiy_core/tests/text_typing_latency.rs`

- [ ] **Step 1: Write the fixture** (it should pass against the already-built
  pipeline — if any assertion is red, that is a REAL latency bug to
  root-cause, not a tolerance to widen):

  ```rust
  //! Gate #14's text component — the typing-latency MECHANISM fixture
  //! (text verification.md § 4; campaign T8): a keystroke (a `Text` edit)
  //! reaches a freshly-published `ExtractedGlyphs` in ONE frame
  //! (TextSync → measure → TextCommit → extract, all within one
  //! frame()), and the steady tail after it re-publishes and re-measures
  //! NOTHING — the structural protection the per-frame budget relies on.
  //! Wall-clock budget NUMBERS stay with `buiy-verification-design`; this
  //! file pins the frame-count mechanism only, headless on the
  //! adapterless extract harness (the GlyphChangeLog mirrors the prepare
  //! glyph gate exactly).

  mod support;
  // … imports (the text_extract.rs set) plus:
  use buiy_core::layout::LayoutTaffyComputeCount;
  use buiy_core::text::{TextMeasureCallCount, TextSyncAppliedCount};
  use support::extract_harness::TextExtractHarness;

  #[test]
  fn one_frame_from_text_edit_to_glyph_publish() {
      let mut h = TextExtractHarness::new();
      let text = /* the spawn_text fixture: "Hi" under a sized column root */;
      h.settle();

      let count0 = h.glyph_count();
      let publishes0 = h.changed_frames();
      let sync0 = h.app.world().resource::<TextSyncAppliedCount>().0;
      let measure0 = h.app.world().resource::<TextMeasureCallCount>().0;

      // The keystroke: append one glyph's worth of text.
      h.app.world_mut().get_mut::<Text>(text).expect("Text").0.push('!');
      h.frame(); // ONE frame: Update (sync→measure→commit) + extract

      // THE mechanism: the edit reached ExtractedGlyphs in one frame…
      assert_eq!(h.changed_frames(), publishes0 + 1,
          "one frame from the Text edit to the ExtractedGlyphs publish");
      // …and the published set really is the new content.
      assert_eq!(h.glyph_count(), count0 + 1,
          "the new glyph's instance is in the published set");
      // The instrument trail: exactly one TextSync re-apply; the edit
      // re-measured (intrinsics invalidation fired).
      assert_eq!(h.app.world().resource::<TextSyncAppliedCount>().0, sync0 + 1);
      assert!(h.app.world().resource::<TextMeasureCallCount>().0 > measure0);

      // The steady tail (the gate-#14 structural budget): nothing
      // publishes, nothing measures, nothing recomputes.
      let publishes1 = h.changed_frames();
      let measure1 = h.app.world().resource::<TextMeasureCallCount>().0;
      let taffy1 = h.app.world().resource::<LayoutTaffyComputeCount>().0;
      h.frame();
      h.frame();
      assert_eq!(h.changed_frames(), publishes1, "steady frames publish nothing");
      assert_eq!(h.app.world().resource::<TextMeasureCallCount>().0, measure1,
          "steady frames measure nothing");
      assert_eq!(h.app.world().resource::<LayoutTaffyComputeCount>().0, taffy1,
          "steady frames recompute no layout");
  }
  ```

  (Check the real import paths for the three instrument resources —
  `TextMeasureCallCount` is `text/measure.rs:34`, `TextSyncAppliedCount`
  `text/sync.rs:63`, `LayoutTaffyComputeCount` `layout/systems.rs:110`; use
  whatever re-exports `tests/text_measure.rs` / `tests/text_sync.rs`
  already use.)

- [ ] **Step 2: Headless gate green (the new file runs adapterless); GPU
  lane green (unchanged).** Commit:
  `test(text): T8 — gate-#14 typing-latency mechanism fixture (headless)`.

---

### Task 7: Docs flip + errata + self-review

**Files:**
- Modify: `docs/plans/follow-ups.md`
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md`
- Modify: `docs/specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md`
- Modify: `docs/specs/2026-06-09-buiy-text-rendering-design/verification.md`
- Modify: `docs/README.md`

- [ ] **Step 1: follow-ups.md.** Flip the entry heading to
  `## Render — glyphs bypass effect-group compositing (text-seam follow-up) — LANDED`
  and append a **Status: Landed** paragraph (the established LANDED-entry
  format — keep the original body): landed by T8
  (`2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md`) — the glyph buffer
  is partitioned into flat/group ranges exactly like the quad path
  (`partition_glyph_ranges` over the producer's `entity_runs`, group
  membership derived from the FRESH node list at prepare — the § 4.6
  discipline, D1), the step-1 group pass draws each group's glyph range
  into its `Rgba16Float` target via the `Glyph@Rgba16Float` specialization
  (after its quads, atlas `@group(1)` bound), and the flat glyph draw
  covers the complement; the `TODO(text-seam)` block is deleted. GPU
  regressions: `tests/text_effect_group_gpu.rs` + the flipped
  `text_decoration_gpu.rs` asymmetry test.

  ADD a new entry (D9's finding):
  `## Render — degraded effect groups vanish instead of drawing flat` —
  **Originated:** T8 implementation reading. **Symptom:** a
  `plan_allocation == false` group gets no target, `BuiyNode::run` step 1
  `continue`s, and its members are excluded from `flat_ranges` /
  `glyph_flat_ranges` — so under RT-pool budget pressure a degraded group's
  quads AND glyphs paint nowhere, despite the "drawn flat instead" comments
  (node.rs / compositor.rs `PreparedEffectTargets`). Latent under the
  64 MiB budget. **Sketch:** either re-route a degraded group's ranges into
  the flat draw at prepare (forward compositing, accepting the double-dim
  approximation v1 rejected for targets) or document skip-as-degradation;
  decide with `buiy-verification-design`'s budget calibration.
  **Spec touchpoint:** `effect-compositor.md § 2.3`.

- [ ] **Step 2: Campaign plan.** Flip the phase-status row
  `T8 | Glyphs in effect groups + damage hardening | proposed → landed`, and
  append the errata block to the T8 section (the established "T*n* errata
  for the spec edit pass" pattern), seeded from this plan + anything further
  found while implementing:

  1. *The charter's "partition `ExtractedGlyphs` by effect-group ranges"* —
     as built, no group data is recorded INTO the carrier: it grows
     per-entity instance attribution only (`entity_runs`), and the
     partition is derived at prepare from the fresh node list (the § 4.6
     discipline — a recorded range/index would go stale whenever the node
     walk rebuilds while glyphs are retained; D1's rejected runner-up B).
  2. *The charter's "the typing-latency fixture … GPU lane"* — seated
     HEADLESS on the adapterless extract harness (D7): the budget numbers
     are deferred to `buiy-verification-design` by the charter itself,
     leaving a frame-count mechanism that needs no adapter; T8's GPU damage
     surface is the blink-reupload assert.
  3. *node.rs/compositor.rs "degraded groups draw flat instead"* — found
     aspirational (degraded members paint nowhere); filed as a follow-ups
     entry rather than fixed (quad-path scope).
  4. *glyph-pipeline § 11.3's global "flat quad-then-glyph order"* — now
     holds per region (within each group target and within the flat
     complement); the cross-layer interleave note is otherwise unchanged.

- [ ] **Step 3: Spec flips (supersede, don't silently contradict).**

  - `glyph-pipeline.md § 11` item 2 ("Glyphs bypass effect groups") →
    rewrite as LANDED (T8): the partition + `Glyph@Rgba16Float`
    specialization shipped; point at the T8 plan + the GPU regressions;
    drop the stale `node.rs:272-280` line reference. Item 3 gains the
    "per region" scoping sentence (erratum 4).
  - `decoration-and-paint.md § 4.5` bullet 2 ("Glyph stamps in effect
    groups") → flip to as-built: the glyph buffer IS partitioned by group
    ranges; underline, line-through, caret, and ink now dim together; the
    editable-text-in-effect-group fixture constraint is lifted.
  - `decoration-and-paint.md § 6.3` closing paragraph: the damage property
    is now ALSO pinned at the GPU lane (`BufferUploadStats`; the
    `caret_blink_reuploads_the_glyph_buffer_only` test).
  - `verification.md § 1.3`: the "Glyph-in-effect-group composite" row
    drops "not claimable before it lands" (now claimed:
    `text_effect_group_gpu.rs`); the "Caret-blink damage" row points at the
    landed test.
  - `verification.md § 4`: delete (or flip to landed) the "Effect-group
    ordering constraint" bullet.

- [ ] **Step 4: docs/README.md** — add the T8 plan line to the text-plans
  catalog block (after the T7 line, same format): plan path + one-sentence
  summary (glyph buffer partitioned by effect-group ranges via per-entity
  runs + fresh-node-list derivation; `Glyph@Rgba16Float` step-1 draw + flat
  complement; the § 4.5 asymmetry test flipped; blink-reupload GPU pin via
  `BufferUploadStats`; gate-#14 typing-latency mechanism fixture) +
  `[landed]`.

- [ ] **Step 5: Self-review.** Re-read the diff against glyph-pipeline
  § 11.2, decoration-and-paint §§ 4.5/6.3, and effect-compositor § 3
  step 1: the `TODO(text-seam)` is gone; no-group frames are byte-for-byte
  pre-T8 (the single-full-flat-run argument); the independent buffer gating
  holds (uploads keep their own gates; the partition recompute is CPU-only
  under the union); the producer gained ZERO system params and ZERO probe
  members; no cosmic type crossed the render seam (`text_touch_pass.rs`
  green); group truth has exactly ONE source (`extract_buiy_nodes`).
  Confirm the campaign's T8 test surface is fully covered: the
  text-in-effect-group composite golden (now claimable), the blink-damage
  assert, the latency fixture. Dispatch a fresh-context review subagent
  over the full T8 diff (the requesting-code-review skill).

- [ ] **Step 6: Run BOTH gate lanes one final time — green.** Commit:
  `docs(text): T8 — campaign status flip, follow-ups closed, errata`.
