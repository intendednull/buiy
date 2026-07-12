# Top-Layer Stacking Composite — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **This is a render-pipeline refactor — RUN the GPU lane, do not trust headless alone.**

**Goal:** Make a `.top_layer()` subtree occlude the base across ALL paint tiers (text, icons, borders — not just fills), by drawing each block's complete tier-stack in sequence on the same window surface, instead of the current global-tier-per-primitive draw.

**Architecture:** Approach A (same-surface per-block tier ordering), single-boundary-v1. Persist a **top-layer discriminator** on the extract record — computed by a `ChildOf` ancestor **climb** (mirroring the landed `nearest_group_entity` climb), run after `assemble_context_tree`. Each tier packer partitions its instance blob into a **base range** + a **top-layer range** at the boundary (with a tail-contiguity `debug_assert` tripwire). `node.rs::buiy_pass` then draws the **base block's full tier-stack** (shadows → quads+raster+gradient → glyphs → icons → bands → backdrop → root-composite) then the **top-layer block's full tier-stack** over it. All top-layer content is ONE block (per-context is a deferred follow-up). No off-screen target.

**Tech Stack:** Rust, Bevy 0.19 render (`Core2d` systems), wgpu, `buiy_core` render pipeline; `buiy_verify` GPU reftests; `iai-callgrind` perf gate. GPU host: AMD RX 6700 XT / RADV, `DISPLAY=:0`, `env RUST_MIN_STACK=33554432`.

**Spec:** `docs/specs/2026-07-10-toplayer-stacking-composite-design.md` (rev-3, active). Read §§ 3.1–3.6, 4, 6 before starting. **Schedules `docs/plans/follow-ups.md:2311`.**

**Gate command (full):**
```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --locked -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked && \
  xvfb-run -a cargo test --workspace --locked
```
**GPU legs (both, on the GPU host — additive):**
```sh
env RUST_MIN_STACK=33554432 cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
env RUST_MIN_STACK=33554432 cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1
```

**Spike patterns to reuse (proven on the RX 6700 XT, then reverted):**
- The extract signal is an **inherited** bool via an ancestor climb — NOT each node's own `Stacking.top_layer` (that is per-node; a child raster/panel/text of an overlay reads `None` → misclassified → the tripwire panics). Mirror `nearest_group_entity`.
- `block_interleave` wrapper: build `gradient_anchors` + `sorted_rasters` **once, unconditionally** (regardless of pipeline readiness); `partition_point`-split each against the quad boundary; call the untouched `interleave_flat_draw` per sub-array; then **RE-OFFSET** the returned `Gradients`/`Raster` step indices back to absolute (the sliced sub-array resets indices to 0).
- `run_backdrop_blurs` takes a per-block **filtered clone** of `PreparedBackdropBlur` (clone is just handle-vecs — cheap).
- Keep a **tail-contiguity `debug_assert` tripwire** in every partition (it caught the §3.1 bug in one GPU run).
- GPU dim assertions use a **dominant-/per-channel** delta, NOT color-sum.
- Byte-stability is the load-bearing gate: the whole existing GPU suite must not shift for any non-top-layer fixture (spike: buiy_core 89/89, buiy_verify 24/24, zero shift).

---

## File structure

**Modified (production):**
- `crates/buiy_core/src/render/extract.rs` — add `ExtractedNode.top_layer: bool`; add the post-assembly ancestor-climb pass in `extract_buiy_nodes` (after `assemble_context_tree`), mirroring the `nearest_group_entity` climb; the Patch-path exclusion.
- `crates/buiy_core/src/render/buckets.rs` — `PackedPartition` gains `top_layer_boundary: u32`; `pack_view_partitioned` computes it + a tail-contiguity `debug_assert`; the shadow/rounded-shadow/gradient/band packers read `node.top_layer` **directly** (the flag rides the `ExtractedNode` record) and return their per-tier boundary.
- `crates/buiy_core/src/text/extract.rs` — `partition_glyph_ranges` gains the per-block boundary (glyph + icon share it), read off `node.top_layer`.
- `crates/buiy_core/src/render/prepare.rs` — retain the per-tier boundaries on `BuiyInstanceBuffers`; provide the **raster path's** entity→`top_layer` lookup (an `EntityHashMap<bool>` mirroring `group_by_entity` — the raster draw is entity-keyed, unlike the node-iterating tier packers).
- `crates/buiy_core/src/render/node.rs` — the `buiy_pass` per-block restructure; the `block_interleave` wrapper; `run_backdrop_blurs` per-block; the step-2b composite per-block.
- `crates/buiy_core/src/render/blur.rs` — `run_backdrop_blurs` signature (`&[PreparedBackdropBlur]` + a block filter).

**Modified (tests, existing):**
- `crates/buiy_core/tests/render/scrim_tier_bleed_gpu.rs` — flip the band/glyph/icon assertions to DIM (the acceptance witness).
- Any test constructing `ExtractedNode` literally (~13 files — see Task 0.2): add `top_layer: false`. Prefer a constructor helper to minimize churn.

**Created (tests):**
- `crates/buiy_core/tests/render/toplayer_block_partition.rs` — headless unit tests (climb classification, packer boundaries, `block_interleave`, tripwire panics).
- `crates/buiy_verify/tests/verify_gpu/toplayer_occludes_all_tiers.rs` (or the existing GPU-reftest module) — base-group-under-scrim, backdrop both directions, raster-inside-overlay, single-boundary-bleed (deferred gate), paint==pick.
- `crates/buiy_core/benches/` (existing iai harness) — the F9 draw-call-count-stability gate.

---

## Wave 0 — the extract top-layer signal (the foundation)

### Task 0.1: Add `ExtractedNode.top_layer` + the ancestor-climb classifier

**Files:**
- Modify: `crates/buiy_core/src/render/extract.rs` (`ExtractedNode` struct ~102-185; `extract_buiy_nodes` ~1350, the post-`assemble_context_tree` region where `node.group` is assigned via the `nearest_group_entity` map)
- Test: `crates/buiy_core/tests/render/toplayer_block_partition.rs` (new)

- [ ] **Step 1: Write the failing test** — a child of a top-layer entity is classified top-layer (the raster-inside-overlay case the spike found).

Mirror the adapterless extract harness in `crates/buiy_verify/tests/verify_headless/modal_showcase_c8c.rs:767` (`ShowcaseExtractHarness`) OR the `buiy_view` extract harness. Spawn: a base `Node`, and a `.top_layer()` parent `Node` with a plain child `Node` (no own `Stacking.top_layer`). Run `extract_buiy_nodes`. Assert:
```rust
// The top-layer PARENT and its plain CHILD are BOTH tagged top_layer;
// the base node is not. (Per-node Stacking.top_layer would miss the child.)
assert!(node_for(parent).top_layer,  "the top-layer root is tagged");
assert!(node_for(child).top_layer,   "a plain CHILD of a top-layer root inherits top_layer (ancestor climb, not per-node)");
assert!(!node_for(base).top_layer,   "a base node is not top-layer");
```

- [ ] **Step 2: Run it — expect FAIL** (`top_layer` field does not exist / is false for the child).
Run: `env RUST_MIN_STACK=33554432 cargo test -p buiy_core --locked --test render toplayer_child_inherits -- --nocapture`

- [ ] **Step 3: Add the field + the climb.** Add `pub top_layer: bool` to `ExtractedNode` (default `false` in `extracted_node_for`). In `extract_buiy_nodes`, AFTER `assemble_context_tree` builds `all.nodes` and the `nearest_group_entity` climb assigns `node.group`, add a second pass that sets `node.top_layer` by an ancestor climb over the SAME `ChildOf` chain: a node is top-layer iff itself or any ancestor has `Stacking.top_layer != TopLayer::None`. Reuse the exact climb structure `nearest_group_entity` uses (an `EntityHashMap` cache keyed by entity + an O(depth) parent walk with memoization). Do NOT compute it in `resolve_one` (no ancestor access there).

- [ ] **Step 4: Run it — expect PASS.**

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat(render): ExtractedNode.top_layer via ancestor climb (top-layer subtree tag)"`

### Task 0.2: Fix `ExtractedNode` literal constructors

**Files:** every test that constructs `ExtractedNode { .. }` literally (grep: `ExtractedNode {`). The spike counted ~13.

- [ ] **Step 1:** Run `rg -l 'ExtractedNode \{' crates/` to list them.
- [ ] **Step 2:** Add `top_layer: false,` to each literal (base fixtures are not top-layer). If churn is high, add a `#[cfg(test)] impl ExtractedNode { fn test_at(...) }` helper OR `..ExtractedNode::default()` if a `Default` is acceptable — but keep the field non-Default in prod so a real omission is a compile error.
- [ ] **Step 3:** `cargo test -p buiy_core --locked --no-run` compiles.
- [ ] **Step 4: Commit** — `git commit -am "test(render): thread top_layer:false through ExtractedNode fixtures"`

---

## Wave 1 — per-tier packer partitions (each with a tail-contiguity tripwire)

Each task is a pure-function change with a headless unit test. The pattern: the packer walks its producer in paint order, tracks the running instance count, and records the index where the first top-layer-tagged instance begins = the tier's `top_layer_boundary`; a `debug_assert` fires if a base instance appears AFTER a top-layer one (tail-contiguity).

### Task 1.1: Quad partition — `PackedPartition.top_layer_boundary`

**Files:**
- Modify: `crates/buiy_core/src/render/buckets.rs` (`PackedPartition` struct ~504; `pack_view_partitioned` ~558; `Partitioner` ~637)
- Test: `crates/buiy_core/tests/render/toplayer_block_partition.rs`

- [ ] **Step 1: Failing test.**
```rust
// nodes = [base_fill, base_fill, TOP_fill, TOP_fill] (top_layer flag on the last two)
let p = pack_view_partitioned(&nodes, 0, &[]);
assert_eq!(p.top_layer_boundary, 2, "boundary at the first top-layer instance");
```
Plus a `#[should_panic]` test: a node list `[base, TOP, base]` (a base after a top-layer) panics the tail-contiguity `debug_assert`.

- [ ] **Step 2: Run — FAIL** (`top_layer_boundary` absent).
- [ ] **Step 3: Implement.** Add `pub top_layer_boundary: u32` to `PackedPartition`. In `pack_view_partitioned`, as each node's quad (and its spliced text quads) is pushed, if `node.top_layer` and the boundary is unset, set `top_layer_boundary = p.len()` (the current instance index). `debug_assert!(!(seen_top_layer && !node.top_layer), "top-layer instances must be a contiguous tail")`. Default `top_layer_boundary = instance_count` when no top-layer node (empty top-layer block).
- [ ] **Step 4: Run — PASS.**
- [ ] **Step 5: Commit** — `feat(render): quad packer top_layer_boundary + tail-contiguity tripwire`

### Task 1.2: Shadow + rounded-shadow partition

**Files:** `crates/buiy_core/src/render/buckets.rs` (`pack_shadow_instances` ~237, `pack_rounded_shadow_instances` ~259); test file.

- [ ] **Step 1: Failing test** — `pack_shadow_instances` and `pack_rounded_shadow_instances` each return `(Vec<..>, u32 boundary)`; a top-layer rounded caster's shadow lands in the top-layer range.
- [ ] **Step 2: FAIL.**
- [ ] **Step 3:** Change both to return the boundary (the instance index of the first top-layer node's first shadow term), same tripwire. Update callers (`node.rs`, the `ShowcaseExtractHarness` count helpers).
- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit** — `feat(render): shadow + rounded-shadow packer boundaries`

### Task 1.3: Gradient partition

**Files:** `crates/buiy_core/src/render/buckets.rs` (`pack_gradient_instances` ~295) + `text/extract.rs` anchor source; test.

- [ ] **Step 1: Failing test** — `pack_gradient_instances` returns the gradient-blob top-layer boundary alongside its `(Vec, anchors)`.
- [ ] **Step 2–4:** implement + tripwire + PASS.
- [ ] **Step 5: Commit** — `feat(render): gradient packer boundary`

### Task 1.4: Band partition

**Files:** `crates/buiy_core/src/render/buckets.rs` (`pack_band_instances` ~211); test.

- [ ] **Step 1: Failing test** — a top-layer node's border/outline band lands in the top-layer range; boundary returned.
- [ ] **Step 2–4:** implement + tripwire + PASS.
- [ ] **Step 5: Commit** — `feat(render): band packer boundary`

### Task 1.5: Glyph/icon partition

**Files:** `crates/buiy_core/src/text/extract.rs` (`partition_glyph_ranges` ~442 — it already has a contiguity `debug_assert`); test.

- [ ] **Step 1: Failing test** — glyphs/icons of a top-layer subtree are a contiguous tail; `partition_glyph_ranges` returns the glyph boundary (and the icon boundary if icons are a separate blob).
- [ ] **Step 2–4:** extend the existing partition with the per-block boundary axis (reuse its contiguity assert); PASS.
- [ ] **Step 5: Commit** — `feat(text): glyph/icon top-layer boundary in partition_glyph_ranges`

### Task 1.6: Thread `top_layer_of` + retain the boundaries in prepare

**Files:** `crates/buiy_core/src/render/prepare.rs` (`group_by_entity` threading ~780-813; `BuiyInstanceBuffers` ~233); test = the existing prepare tests stay green.

- [ ] **Step 1:** The per-tier boundaries are returned from the packers directly off `ExtractedNode.top_layer` (no separate closure — the flag rides the record). Retain each tier's `top_layer_boundary` on `BuiyInstanceBuffers` (a small struct `TopLayerBoundaries { quad, shadow, rounded_shadow, gradient, band, glyph, icon }`). The ONE exception is the RASTER draw path (`build_raster_draws`, entity-keyed) — give it an `EntityHashMap<bool>` `top_layer_of` lookup, built exactly like its existing `node_quad_anchor_of` lookup, so a top-layer raster splices into the top-layer block.
- [ ] **Step 2:** `cargo test -p buiy_core --locked` (headless) green.
- [ ] **Step 3: Commit** — `feat(render): retain per-tier top-layer boundaries on BuiyInstanceBuffers`

---

## Wave 2 — node.rs per-block draw restructure (the GPU-observable change)

### Task 2.1: The `block_interleave` wrapper (pure, headless)

**Files:** `crates/buiy_core/src/render/buckets.rs` (new `block_interleave` next to `interleave_flat_draw`); test.

- [ ] **Step 1: Failing test.** `block_interleave(flat_ranges, gradient_anchors, raster_anchors, quad_boundary)` returns TWO ordered `Vec<FlatDrawStep>` (base steps, top-layer steps). Each step's `Quads`/`Gradients`/`Raster` indices are ABSOLUTE (re-offset). Assert: with `quad_boundary=2` and a raster at anchor 1 (base) + a raster at anchor 3 (top-layer), the base vec draws `Quads(0..2)` + the base raster, the top-layer vec draws `Quads(2..N)` + the top-layer raster, and NO base step references a top-layer instance.
- [ ] **Step 2: FAIL.**
- [ ] **Step 3: Implement** per the spike pattern: `partition_point` the `gradient_anchors`/`sorted_rasters` at `quad_boundary`; slice `flat_ranges` at the boundary; call the UNTOUCHED `interleave_flat_draw` on each sub-array; RE-OFFSET the returned `Gradients(g..g+1)`/`Raster(k)` indices back to absolute (add the base gradient/raster count). Keep `interleave_flat_draw` byte-identical.
- [ ] **Step 4: PASS.** Also assert `quad_boundary == N` (empty top-layer block) yields base==today's single interleave + an empty top-layer vec (byte-stability).
- [ ] **Step 5: Commit** — `feat(render): block_interleave (base/top-layer split of the flat draw)`

### Task 2.2: `run_backdrop_blurs` per block

**Files:** `crates/buiy_core/src/render/blur.rs` (`run_backdrop_blurs` signature) + `node.rs` caller; test = the existing `render_backdrop_blur_gpu` stays green (byte-stable when no top-layer).

- [ ] **Step 1:** Change `run_backdrop_blurs` to take `&[PreparedBackdropBlur]` (from `Option<&PreparedBackdropBlurs>`). The caller builds two filtered clones — base blurs (former is base) and top-layer blurs (former is top-layer) — via `blurs.iter().filter(|b| top_layer_of(b.entity) == want).cloned().collect()` (clone is handle-vecs, cheap).
- [ ] **Step 2:** GPU: `render_backdrop_blur_gpu` unchanged (no top-layer former → all base → byte-identical).
- [ ] **Step 3: Commit** — `refactor(render): run_backdrop_blurs takes a per-block blur slice`

### Task 2.3: Restructure `buiy_pass` into base-block → top-layer-block

**Files:** `crates/buiy_core/src/render/node.rs` (`buiy_pass` ~75; the flat pass ~428-655; backdrop ~668; step-2b ~696). Acceptance test = the FLIPPED `scrim_tier_bleed_gpu.rs`.

- [ ] **Step 1: RED — flip the acceptance witness.** In `crates/buiy_core/tests/render/scrim_tier_bleed_gpu.rs`, change the BAND, GLYPH, and (add) ICON assertions from BLEED (Δ0) to DIM (dominant-channel Δ>0), matching the quad/raster assertions. Run on GPU: it FAILS on current code (bleed).
Run: `env RUST_MIN_STACK=33554432 cargo test -p buiy_core -j 2 -- --ignored --test-threads=1 scrim_tier_bleed`
Expected: FAIL (bands/glyphs still bleed).

- [ ] **Step 2: Implement the two-block draw.** Restructure `buiy_pass` so the flat pass runs TWICE, gated on the boundaries: **base block** = shadows[0..b_shadow] → `block_interleave` base steps (quads+gradient+raster) → glyphs[0..b_glyph] → icons[0..b_icon] → bands[0..b_band]; then base `run_backdrop_blurs(base_blurs)`; then step-2b composite for BASE groups. **Top-layer block** = the same tiers over `[boundary..]` ranges; then top-layer backdrop; then top-layer composite. Intra-block order = today's `tier-stack → backdrop → composite` (§3.3 LOCKED). Use the `block_interleave` result for the quad/gradient/raster tier; slice each other tier's `pass.draw(0..4, base_range)` / `(top_range)`. Skip an empty block (boundary==count) so a no-top-layer scene issues the SAME draws (byte-stability + F9).
- [ ] **Step 3: GREEN.** Re-run the flipped `scrim_tier_bleed` on GPU → PASS (bands/glyphs/icons now DIM).
- [ ] **Step 4: Byte-stability check** — run BOTH full GPU legs; confirm zero non-top-layer shift (spike: 89/89 + 24/24). Any shift must be an intended top-layer fixture.
- [ ] **Step 5: Commit** — `feat(render): draw base then top-layer tier-stacks per block (all-tiers occlusion)`

---

## Wave 3 — Patch-path exclusion (plan-critical, §3.6)

### Task 3.1: Exclude top-layer subtrees from the partial re-extract Patch path

**Files:** `crates/buiy_core/src/render/extract.rs` + `crates/buiy_core/src/text/extract.rs` (the Patch / retain-damage fast path that calls `resolve_one` directly).

- [ ] **Step 1: RED — a Patch-frame test.** Build an app, settle a frame with a top-layer overlay present, then trigger a Patch frame (a change that does NOT rebuild the node list — mirror the retain-damage tests). Assert the overlay's descendants stay tagged `top_layer` after the Patch (today they'd be misclassified because the Patch path skips the post-assembly climb).
- [ ] **Step 2: FAIL** (Patch path bypasses the climb).
- [ ] **Step 3: Implement.** Mirror the existing group exclusion: the Patch fast path must either (a) exclude a top-layer subtree from the in-place patch and force a Full rebuild for it, or (b) run the ancestor climb on the patched entity. Prefer the same shape the group exclusion already uses (the Patch path is "group-free-only" today — extend that guard to "group-free AND top-layer-free", falling back to Full).
- [ ] **Step 4: GREEN.**
- [ ] **Step 5: Commit** — `fix(render): exclude top-layer subtrees from the Patch fast path`

---

## Wave 4 — GPU acceptance fixtures (graduate the spike fixtures)

Each is a `buiy_verify` (or `buiy_core`) GPU reftest, `#[ignore]`, dominant-channel assertions. Build against the `scrim_tier_bleed_gpu.rs` / `render_raster_interleave_gpu.rs` harness. RED where it exercises the NEW behavior (these pass once Wave 2.3 lands — write them RED-first against a pre-2.3 checkout only if practical; otherwise they are GREEN acceptance gates added after 2.3).

### Task 4.1: base-effect-group-under-top-layer-scrim
- [ ] Fixture: a base element with `Opacity(0.5)` (forms an effect group), composited in the base block; a translucent top-layer scrim over it. Assert the scrim DIMS the composited group (spike: 146→96). Commit.

### Task 4.2: backdrop-blur both directions
- [ ] (a) a top-layer subtree with `backdrop-filter: blur` over base content — its blur samples the base beneath (variance drop). (b) a base backdrop element under a top-layer overlay. Assert both blur+dim (spike: 59.9→1.3). Commit.

### Task 4.3: raster-inside-a-top-layer-overlay
- [ ] A `RasterImage` node INSIDE a `.top_layer()` overlay over base content. Assert the raster still paints (non-canvas pixels present) AND the overlay dims the base — the decisive A-over-B proof (and the fixture that exposed the §3.1 bug). Commit.

### Task 4.4: single-boundary-bleed (deferred-follow-up gate)
- [ ] Two overlapping top-layer overlays (a Tooltip-tier bordered overlay under a Modal-tier scrim). Assert the CURRENT single-boundary behavior: the Tooltip border is NOT dimmed by the Modal scrim (Δ0, spike-confirmed). Add `// FIXME(per-context-v1): flip to DIM when per-context ships`. Commit.

### Task 4.5: paint==pick across tiers
- [ ] A headless test: for a top-layer-over-base fixture, the new paint order equals the pick order across tiers (the pick≠paint seam closed at the paint layer). Commit.

---

## Wave 5 — perf gate + full gates

### Task 5.1: F9 draw-call-count-stability (iai)
- [ ] **Step 1:** Add an `iai-callgrind` bench asserting: a NO-top-layer scene issues the SAME draw calls as the pre-refactor baseline (empty-block-skip → zero extra draws), and a top-layer scene adds only a bounded delta (≈ tiers × 1 extra block) with NO off-screen-target allocation.
- [ ] **Step 2: Commit** — `test(perf): draw-call-count-stability gate for the top-layer block split`

### Task 5.2: Full gate + both GPU legs
- [ ] Run the full gate command + BOTH GPU legs. Confirm: fmt/clippy/doc/workspace green; buiy_core GPU + buiy_verify GPU green; the flipped acceptance GREEN; zero non-top-layer golden shift. Fix any fallout. Commit any golden re-bless (top-layer fixtures only) with justification.

---

## Wave 6 — docs + self-review + close-out

### Task 6.1: Retire the follow-up + note the dependent app PRs
- [ ] Mark `docs/plans/follow-ups.md:2311` (single-tier glyph occlusion / pick≠paint) as CLOSED-by this work. Add two OPEN app follow-ups: (F4) avatar-editor-as-overlay (restructure `apps/dooduel/src/view/avatar_editor.rs` back to a top-layer overlay — the true end-to-end raster-in-overlay acceptance); (F7) dark-mode scrim iso-luminance (fast-follow, with the render-the-dark-screen done-verification). Add the two named framework follow-ups: `same-block backdrop-vs-composite spatial overlap` and `per-context-v1` (overlapping-overlays bleed). Update the spec Status if the review requires. Commit.

### Task 6.2: Plan self-review (checklist, not a subagent)
- [ ] Spec coverage: every §3 scope item has a task (extract signal 0.1; each tier packer 1.1-1.5; node restructure 2.3; Patch 3.1; §4 gates 4.x/5.x). Confirm no gap.
- [ ] Type consistency: `top_layer` (field), `top_layer_boundary` (per-tier), `block_interleave` (fn), `TopLayerBoundaries` (struct) used consistently across tasks.
- [ ] Placeholder scan: no TBD/TODO in the executed plan.

---

## Notes for the executor

- **RED-first discipline:** the acceptance is the FLIPPED `scrim_tier_bleed_gpu.rs` (Task 2.3 Step 1) — flip it to DIM and watch it FAIL before the restructure, PASS after. Each Wave-0/1/2.1 task has a headless unit RED→GREEN.
- **Byte-stability is the gate you cannot skip:** after Task 2.3, run BOTH GPU legs; a non-top-layer golden shift is a real regression, not a re-bless.
- **The tripwire is your friend:** keep the tail-contiguity `debug_assert` in every partition — it turns the §3.1-class bug into a loud panic, not a silent wrong pixel.
- **Don't route top-layer through the effect-group compositor** (Approach B, rejected — drops rasters). This is same-surface paint-order relocation only.
- **Commit per task; do NOT push** (the team-lead gates push/merge).
