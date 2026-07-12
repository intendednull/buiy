# Top-Layer Stacking Composite — Implementation Plan

> **STATUS: ✅ COMPLETE (2026-07-12).** All 7 waves (W0–W6) executed on
> `feat/dooduel-multiplayer-m1`, each RED-first + fresh-review-gated. Final verification:
> full workspace gate green (fmt/clippy/doc + `cargo test --workspace` 2328/0/138 across
> 153 binaries), both GPU legs byte-stable on the RX 6700 XT (buiy_core 95/0 + buiy_verify
> 24/0, zero non-top-layer shift), `scrim_tier_bleed` acceptance flipped BLEED→DIM, 8
> `toplayer_occludes_all_tiers_gpu` fixtures GREEN, and the real dooduel app proven (base
> top-bar text dims ~34% under the word-pick scrim, `in_game_picking` vs `in_game_drawer`).
> Drift #1 (bare gradient/raster-only occlusion) was found in W2 review and closed with the
> authoritative `any_top_layer` gate (`aacddfc`). Self-review at the end of this file. Wave
> commits: W0 `c72461f`/`27126ba` · W1 `b86b879`/`70e6c34`/`998eba2`/`7d2c9bb`/`391bc22`/`926e820` ·
> W2 `d3991c3`/`ef8c282`/`556fd22`/`aacddfc` · W3 `96147d0` · W4 `3d7b252` · W5 `b2f06b5` ·
> W6 (this docs close-out). UNPUSHED — awaiting the user's push/PR/merge go.

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **This is a render-pipeline refactor — RUN the GPU lane, do not trust headless alone.**

**Goal:** Make a `.top_layer()` subtree occlude the base across ALL paint tiers (text, icons, borders — not just fills), by drawing each block's complete tier-stack in sequence on the same window surface, instead of the current global-tier-per-primitive draw.

**Architecture:** Approach A (same-surface per-block tier ordering), single-boundary-v1. Persist a **top-layer discriminator** on the extract record — computed by a `ChildOf` ancestor **climb** (mirroring the landed `nearest_group_entity` climb), run after `assemble_context_tree`. Each tier packer partitions its instance blob into a **base range** + a **top-layer range** at the boundary (with a tail-contiguity `debug_assert` tripwire). `node.rs::buiy_pass` then draws the **base block's full tier-stack** (shadows → quads+raster+gradient → glyphs → icons → bands → backdrop → root-composite) then the **top-layer block's full tier-stack** over it. All top-layer content is ONE block (per-context is a deferred follow-up). No off-screen target.

**Tech Stack:** Rust, Bevy 0.19 render (`Core2d` systems), wgpu, `buiy_core` render pipeline; `buiy_verify` GPU reftests; deterministic headless `FlatDrawStep`-count gate (F9, rev-4/M4 — not iai). GPU host: AMD RX 6700 XT / RADV, `DISPLAY=:0`, `env RUST_MIN_STACK=33554432`.

**Spec:** `docs/specs/2026-07-10-toplayer-stacking-composite-design.md` (rev-4, active). Read §§ 3.1–3.6, 4, 6 before starting. **Schedules `docs/plans/follow-ups.md:2311`.**

**Revised per plan-review (rev-4, all 12 findings + 1 note verified against the branch APIs, none skipped):** the extract climb collects a `top_layer_formers` set in the extract loop (Stacking access); the packer signature change updates `prepare.rs` + `buiy_verify/src/snapshot.rs` + the `modal_showcase` count helpers (NOT node.rs); `partition_glyph_ranges` is in **buckets.rs:786** (entity-keyed, needs a parallel `top_layer_of` closure + a `top_layer_by_entity` map); the **gradient boundary is dropped** (block_interleave splits it); the node restructure has **FOUR** per-block sub-passes (adds `draw_backdrop_filter_fills`), stamps `PreparedBackdropBlur.top_layer` for the blur split, reuses `get_color_attachment()` (Clear-then-Load), and `cut_ranges`-slices straddling glyph/icon runs; the Patch fix is a node-side guard (`extract.rs:1643`, no `text/extract.rs` change); F9 is a **deterministic headless** `FlatDrawStep` test (not iai).

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
- `crates/buiy_core/src/render/buckets.rs` — `PackedPartition` gains `top_layer_boundary: u32`; `pack_view_partitioned` computes it + a tail-contiguity `debug_assert`; the shadow/rounded-shadow/band packers read `node.top_layer` **directly** and return their per-tier boundary (**NO gradient** — rev-4/m2: `block_interleave` splits gradients); `partition_glyph_ranges` (**here, ~786** — entity-keyed) gains a parallel `top_layer_of` closure; new `block_interleave` + a `cut_ranges` straddle helper.
- `crates/buiy_core/src/text/extract.rs` — **no change** (rev-4/M3: `partition_glyph_ranges` is in `buckets.rs`, not here; rev-4/m5: the Patch path is in `render/extract.rs`, not here — `text/extract.rs` does not call `resolve_one`).
- `crates/buiy_core/src/render/prepare.rs` — retain the per-tier boundaries on `BuiyInstanceBuffers`; build the `top_layer_by_entity` map (mirroring `group_by_entity`) and pass it as `top_layer_of` to both `partition_glyph_ranges` calls (`:782`/`:806`) + to `prepare_backdrop_blurs` (the blur flag, M1). (rev-4/m3: this entity map feeds the entity-keyed **blur + composite** paths — NOT rasters, which split by anchor inside `block_interleave`.)
- `crates/buiy_core/src/render/node.rs` — the `buiy_pass` per-block restructure; the `block_interleave` wrapper; `run_backdrop_blurs` per-block; **`draw_backdrop_filter_fills` per-block (rev-4/M2)**; the step-2b composite per-block.
- `crates/buiy_core/src/render/blur.rs` — add `pub top_layer: bool` to `PreparedBackdropBlur` (rev-4/M1), stamp it in `prepare_backdrop_blurs`; `run_backdrop_blurs` signature → `&[PreparedBackdropBlur]` + a per-block filter on the flag.

**Modified (tests, existing):**
- `crates/buiy_core/tests/render/scrim_tier_bleed_gpu.rs` — flip the band/glyph/icon assertions to DIM (the acceptance witness).
- Any test constructing `ExtractedNode` literally (~13 files — see Task 0.2): add `top_layer: false`. Prefer a constructor helper to minimize churn.

**Created (tests):**
- `crates/buiy_core/tests/render/toplayer_block_partition.rs` — headless unit tests (climb classification, packer boundaries, `block_interleave`, tripwire panics).
- `crates/buiy_verify/tests/verify_gpu/toplayer_occludes_all_tiers.rs` (or the existing GPU-reftest module) — base-group-under-scrim, backdrop both directions, raster-inside-overlay, single-boundary-bleed (deferred gate), paint==pick.
- `crates/buiy_core/tests/render/render_buckets.rs` (existing) — the F9 draw-STEP-count-stability headless assertions (rev-4/M4: a deterministic `FlatDrawStep` test, not an iai bench).

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

- [ ] **Step 2: Compile-check — the field is missing** (a compile-fail, not a runtime RED — `ExtractedNode` has no `top_layer` field yet).

- [ ] **Step 3a: Add the field → runtime RED (rev-4/m7).** Add `pub top_layer: bool` to `ExtractedNode`; set `top_layer: false` in the ONE prod constructor `extracted_node_for` (`extract.rs:527`; its external caller `snapshot.rs:710` needs no change). Compile + run — the CHILD assertion now FAILS at runtime (the child is `false`: per-node, not inherited). THIS is the real RED.
Run: `env RUST_MIN_STACK=33554432 cargo test -p buiy_core --locked --test render toplayer_child_inherits -- --nocapture` → FAIL on the child assertion.

- [ ] **Step 3b: Collect the former set + climb → GREEN (rev-4 note).** In `extract_buiy_nodes`, INSIDE the `for item in nodes.iter()` loop (where `Stacking` IS accessible — mirror the `group_formers` collection at `extract.rs:1685-1698`), collect a `top_layer_formers: EntityHashSet` of entities whose OWN `Stacking.top_layer != TopLayer::None`. THEN, AFTER `assemble_context_tree` assigns `node.group` (the `nearest_group_entity` climb), add a second pass setting `node.top_layer` by a `ChildOf` ancestor climb: a node is top-layer iff itself or any ancestor is in `top_layer_formers`. Reuse the exact climb structure `nearest_group_entity` uses (an `EntityHashMap` memo + O(depth) parent walk). Do NOT compute it in `resolve_one` (no ancestor access there).

- [ ] **Step 4: Run it — expect PASS** (the child now inherits `top_layer`).

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
- [ ] **Step 3:** Change both to return `(Vec, u32 boundary)` — the instance index of the first top-layer node's first shadow term — same tripwire. **Update ALL callers (rev-4/M3):** `prepare.rs:573` (shadow) + `:590` (rounded); `crates/buiy_verify/src/snapshot.rs:517`/`:524`; the `modal_showcase_c8c.rs:849`/`:853` count helpers. **NOT `node.rs`** — packing lives in `prepare.rs`; node.rs never calls these packers.
- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit** — `feat(render): shadow + rounded-shadow packer boundaries`

### Task 1.3: Gradient partition — REMOVED (rev-4/m2)

No separate gradient boundary is needed: `block_interleave` (Task 2.1) already splits the gradient blob base/top-layer by `partition_point`-ing `gradient_anchors` at the quad boundary, and `node.rs` draws gradients ONLY via `block_interleave` steps — a retained gradient boundary would never be consumed. `pack_gradient_instances` is UNCHANGED. (The 1.4/1.5/1.6 labels are kept for cross-reference stability.)

### Task 1.4: Band partition

**Files:** `crates/buiy_core/src/render/buckets.rs` (`pack_band_instances` ~211); Test: `toplayer_block_partition.rs`.

- [ ] **Step 1: Failing test** — a top-layer node's border/outline band lands in the top-layer range; `pack_band_instances` returns `(Vec, u32 boundary)`.
- [ ] **Step 2: FAIL.**
- [ ] **Step 3:** Add the boundary + the tail-contiguity tripwire (mirror Task 1.1). **Update ALL callers (rev-4/M3):** `prepare.rs:558`; `snapshot.rs:554`; `modal_showcase_c8c.rs:857`. NOT `node.rs`.
- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit** — `feat(render): band packer boundary`

### Task 1.5: Glyph/icon partition (entity-keyed) + the straddle-cut helper

**Files:** `crates/buiy_core/src/render/buckets.rs` (`partition_glyph_ranges` **~786** — rev-4/M3 location fix; it is entity-keyed via a `group_of: Fn(Entity) -> Option<usize>` closure and has a contiguity `debug_assert`); Test: `toplayer_block_partition.rs`.

- [ ] **Step 1: Failing test (boundary).** `partition_glyph_ranges` takes a **parallel** `top_layer_of: Fn(Entity) -> bool` closure and returns the glyph top-layer boundary; glyphs/icons of a top-layer subtree are a contiguous tail. (Glyph + icon are **separate carriers / instance spaces** — same FUNCTION + closure, called twice, distinct boundary each.)
- [ ] **Step 2: FAIL.**
- [ ] **Step 3:** Add the `top_layer_of` param + the per-block boundary (reuse the existing contiguity assert). Callers `prepare.rs:782` (glyph) / `:806` (icon) build a `top_layer_by_entity: HashMap<Entity,bool>` from `nodes.iter().map(|n|(n.entity, n.top_layer))`, mirroring the existing `group_by_entity` maps there.
- [ ] **Step 4: PASS.**
- [ ] **Step 5 (rev-4/m4): the straddle-cut helper + its test.** The `RangePartitioner` splits flat runs on GROUP only (`buckets.rs:751-757`), NOT on `top_layer` — so a base + top-layer non-group run COALESCES into one flat run that STRADDLES the boundary. Add a pure helper `cut_ranges(ranges: &[Range<u32>], lo: u32, hi: u32) -> Vec<Range<u32>>` that intersects a range-list with `[lo,hi)` (cutting a straddling run), for the glyph/icon flat-range block-slice (and the quad `flat_ranges` inside `block_interleave`). RED-first: a unit test that `[2..8]` cut at boundary 5 yields base `[2..5]` + top `[5..8]` (the straddle is CUT, not dropped).
- [ ] **Step 6: Commit** — `feat(render): glyph/icon top-layer boundary + cut_ranges straddle helper`

### Task 1.6: Retain the boundaries in prepare + the entity→top_layer map

**Files:** `crates/buiy_core/src/render/prepare.rs` (`BuiyInstanceBuffers` ~233; packer calls ~558/573/590/613, glyph/icon partition ~782/806); test = the existing prepare tests stay green.

- [ ] **Step 1:** The per-tier boundaries are returned from the packers directly off `ExtractedNode.top_layer` (no closure — the flag rides the record). Retain them on `BuiyInstanceBuffers` as `TopLayerBoundaries { quad, shadow, rounded_shadow, band, glyph, icon }` — **NO `gradient`** (rev-4/m2: `block_interleave` splits it). Build the `top_layer_by_entity` map once and pass it as `top_layer_of` to both `partition_glyph_ranges` calls (Task 1.5).
- [ ] **Step 2 (rev-4/m3):** The entity-keyed `top_layer` lookup is for the **blur** (Task 2.2 / M1) and **composite** (step-2b, per-group) paths — NOT rasters (rasters split by anchor vs the quad boundary inside `block_interleave`, so `build_raster_draws` needs NO `top_layer_of`).
- [ ] **Step 3:** `cargo test -p buiy_core --locked` (headless) green.
- [ ] **Step 4: Commit** — `feat(render): retain per-tier top-layer boundaries on BuiyInstanceBuffers`

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

- [ ] **Step 1a (rev-4/M1): stamp the flag.** `PreparedBackdropBlur` (`blur.rs:406`) has NO `entity` field, so it can't be filtered by `top_layer_of(b.entity)`. Add `pub top_layer: bool` to `PreparedBackdropBlur`; set it in `prepare_backdrop_blurs` (`blur.rs:467`) from the former entity's `ExtractedNode.top_layer` (via the `top_layer_by_entity` map, Task 1.6).
- [ ] **Step 1b:** Change `run_backdrop_blurs` to take `&[PreparedBackdropBlur]` (from `Option<&PreparedBackdropBlurs>`). The caller builds two filtered clones — base + top-layer — via `blurs.iter().filter(|b| b.top_layer == want).cloned().collect()` (clone is handle-vecs, cheap).
- [ ] **Step 2:** GPU: `render_backdrop_blur_gpu` unchanged (no top-layer former → all base → byte-identical).
- [ ] **Step 3: Commit** — `refactor(render): run_backdrop_blurs takes a per-block blur slice`

### Task 2.3: Restructure `buiy_pass` into base-block → top-layer-block

**Files:** `crates/buiy_core/src/render/node.rs` (`buiy_pass` ~75; the flat pass ~428-655; backdrop ~668; `draw_backdrop_filter_fills` ~683 / defn ~1036; step-2b ~696). Acceptance test = the FLIPPED (+ICON-extended) `scrim_tier_bleed_gpu.rs`.

- [ ] **Step 1: RED — flip the acceptance witness (+ extend for icon, rev-4/m8).** `scrim_tier_bleed_gpu.rs` currently probes QUAD/BAND/GLYPH/RASTER only (lines 92-131, **no icon element**). So: (i) ADD a base ICON element (a vector `Icon`) under the scrim + a readback point over it; (ii) flip the BAND, GLYPH, and new ICON assertions from BLEED (Δ0) to DIM (dominant-channel Δ>0), matching quad/raster. Run on GPU: FAILS on current code (band/glyph/icon still bleed).
Run: `env RUST_MIN_STACK=33554432 cargo test -p buiy_core -j 2 -- --ignored --test-threads=1 scrim_tier_bleed`
Expected: FAIL (bands/glyphs/icons still bleed).

- [ ] **Step 2: Implement the two-block draw (FOUR per-block sub-passes).** Restructure `buiy_pass` so the flat pass runs TWICE, gated on the boundaries. **Base block:** shadows[0..b_shadow] + rounded[0..b_rshadow] → `block_interleave` base steps (quads+gradient+raster) → glyphs (base flat ranges) → icons (base) → bands[0..b_band]; then base `run_backdrop_blurs(base_blurs)`; then base **`draw_backdrop_filter_fills`** (rev-4/M2 — its group-range draws + `!blurs.is_empty()` guard become per-block); then step-2b composite for BASE groups. **Top-layer block:** the same four tiers over the `[boundary..]` ranges + top blurs + top backdrop-filter fills + top composite. Intra-block order = today's `tier-stack → backdrop-blur → backdrop-filter-fills → composite` (§3.3 LOCKED). **(rev-4/m6)** the top block's flat pass MUST reuse `view_target.get_color_attachment()` (Clear-then-Load; precedent `node.rs:732`), NEVER a hand-built `RenderPassColorAttachment { load: LoadOp::Clear }` (that wipes the base). **(rev-4/m4)** glyph/icon draw a LIST of flat ranges (`buffers.glyph_flat_ranges`, `node.rs:608`) and a run can STRADDLE the boundary — slice each with the `cut_ranges` helper (Task 1.5), not whole-range selection. Skip an empty block (boundary==count) so a no-top-layer scene issues the SAME draws (byte-stability + F9).
- [ ] **Step 3: GREEN.** Re-run the flipped `scrim_tier_bleed` on GPU → PASS (bands/glyphs/icons now DIM).
- [ ] **Step 4: Byte-stability check** — run BOTH full GPU legs; confirm zero non-top-layer shift (spike: 89/89 + 24/24). Any shift must be an intended top-layer fixture.
- [ ] **Step 5: Commit** — `feat(render): draw base then top-layer tier-stacks per block (all-tiers occlusion)`

---

## Wave 3 — Patch-path exclusion (plan-critical, §3.6)

### Task 3.1: Exclude top-layer subtrees from the partial re-extract Patch path

**Files:** `crates/buiy_core/src/render/extract.rs` ONLY (rev-4/m5 — the Patch group guard `if old.group.is_some()` at `:1643`; `text/extract.rs` does NOT call `resolve_one`, so no change there).

- [ ] **Step 1: RED — a Patch-frame test.** Build an app, settle a frame with a top-layer overlay present, then trigger a Patch frame (a change that does NOT rebuild the node list — mirror the retain-damage tests). Assert the overlay's descendants stay tagged `top_layer` after the Patch (today they'd be misclassified because the Patch path skips the post-assembly climb).
- [ ] **Step 2: FAIL** (Patch path bypasses the climb).
- [ ] **Step 3: Implement (rev-4/m5).** Extend the node-side Patch guard at `extract.rs:1643` from `if old.group.is_some()` to `if old.group.is_some() || old.top_layer` — a changed top-layer node forces a Full rebuild (which re-runs the climb, setting `top_layer` correctly). A NEW overlay is a structural change that already forces Full. No `text/extract.rs` change (the glyph top-layer signal is re-derived in `prepare.rs:782/806` from the retained-or-Full node records).
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

### Task 5.1: F9 draw-STEP-count stability (deterministic headless — rev-4/M4)
**Files:** `crates/buiy_core/tests/render/render_buckets.rs` (it already asserts exact `Vec<FlatDrawStep>` sequences at `:546-635`).
- [ ] **Step 1:** Add a deterministic HEADLESS test — NOT iai-callgrind (that counts valgrind CPU *instructions*, not draw calls): (a) `block_interleave` with an empty top-layer block == byte-identical steps to `interleave_flat_draw` (also Task 2.1 Step 4); (b) a no-top-layer scene's per-tier draw/step count == the baseline, and a top-layer scene adds only a bounded delta with NO off-screen-target allocation. CPU-only + deterministic.
- [ ] **Step 2: Commit** — `test(render): draw-step-count stability gate for the top-layer block split`

### Task 5.2: Full gate + both GPU legs
- [ ] Run the full gate command + BOTH GPU legs. Confirm: fmt/clippy/doc/workspace green; buiy_core GPU + buiy_verify GPU green; the flipped acceptance GREEN; zero non-top-layer golden shift. Fix any fallout. Commit any golden re-bless (top-layer fixtures only) with justification.

---

## Wave 6 — docs + self-review + close-out

### Task 6.1: Retire the follow-up + note the dependent app PRs
- [ ] Mark `docs/plans/follow-ups.md:2311` (single-tier glyph occlusion / pick≠paint) as CLOSED-by this work. Add two OPEN app follow-ups: (F4) avatar-editor-as-overlay (restructure `apps/dooduel/src/view/avatar_editor.rs` back to a top-layer overlay — the true end-to-end raster-in-overlay acceptance); (F7) dark-mode scrim iso-luminance (fast-follow, with the render-the-dark-screen done-verification). Add the two named framework follow-ups: `same-block backdrop-vs-composite spatial overlap` and `per-context-v1` (overlapping-overlays bleed). Update the spec Status if the review requires. Commit.

### Task 6.2: Plan self-review (checklist, not a subagent)
- [ ] Spec coverage: every §3 scope item has a task (extract signal 0.1; tier packers 1.1/1.2/1.4/1.5 — gradient 1.3 removed per m2; block_interleave 2.1; backdrop 2.2; node restructure 2.3 incl the 4th sub-pass `draw_backdrop_filter_fills`; Patch 3.1; §4 gates 4.x/5.x). Confirm no gap.
- [ ] Type consistency: `top_layer` (field), `top_layer_boundary` (per-tier), `TopLayerBoundaries` (struct, no gradient), `block_interleave` + `cut_ranges` (fns), `top_layer_of` (closure), `top_layer_by_entity` (map), `top_layer_formers` (set) used consistently across tasks.
- [ ] Placeholder scan: no TBD/TODO in the executed plan.

---

## Notes for the executor

- **RED-first discipline:** the acceptance is the FLIPPED `scrim_tier_bleed_gpu.rs` (Task 2.3 Step 1) — flip it to DIM and watch it FAIL before the restructure, PASS after. Each Wave-0/1/2.1 task has a headless unit RED→GREEN.
- **Byte-stability is the gate you cannot skip:** after Task 2.3, run BOTH GPU legs; a non-top-layer golden shift is a real regression, not a re-bless.
- **The tripwire is your friend:** keep the tail-contiguity `debug_assert` in every partition — it turns the §3.1-class bug into a loud panic, not a silent wrong pixel.
- **Don't route top-layer through the effect-group compositor** (Approach B, rejected — drops rasters). This is same-surface paint-order relocation only.
- **Commit per task; do NOT push** (the team-lead gates push/merge).

---

## Self-review (W6, 2026-07-12)

**Spec coverage — every §3/§4 item landed:**
- §3.1 extract top-layer signal (ancestor climb, after `assemble_context_tree`, `top_layer_formers` set) → **W0** (`c72461f`).
- §3.2 per-tier packer boundaries (quad/shadow/rounded-shadow/band + entity-keyed glyph/icon `top_layer_of`; gradient dropped per rev-4/m2) + `cut_ranges` straddle helper + tail-contiguity tripwire → **W1**.
- §3.3 the FOUR per-block sub-passes (tier-stack → backdrop-blur → `draw_backdrop_filter_fills` → composite), `block_interleave` re-offset, `PreparedBackdropBlur.top_layer` flag, Clear/Load reuse → **W2**.
- §3.4 no-group-straddles-boundary invariant + `debug_assert` tripwire (ran active on GPU, never fired) → **W1/W2**.
- §3.6 Patch-path exclusion (`extract.rs` `|| old.top_layer`) → **W3** (`96147d0`).
- §4 verification: flipped `scrim_tier_bleed` acceptance, byte-stability both legs, base-group-under-scrim, backdrop both directions, raster-inside-overlay, single-boundary-bleed characterization gate, paint==pick, F9 draw-step-count → **W4/W5**.

**Deviations from the written plan (all deliberate, all better):**
- **Drift #1 (found in W2 review):** the `has_top_layer` gate must be the authoritative `any_top_layer` bit (on `PackedPartition`, tracker-sourced), NOT the per-tier disjunction — gradient/raster have no retained boundary (rev-4/m2), so a bare gradient/raster-only overlay would fail to occlude. Closed in `aacddfc`. The `u32::MAX` no-top sentinel is kept (not `quad_count`, which would drop a trailing `Color::NONE` gradient). The invisible-container empty-pass is a documented pixel-safe v1 non-case (a non-emptiness guard would re-introduce per-tier + composite-membership fragility and risk silently dropping top content).
- **F9 (rev-4/M4):** a deterministic headless `FlatDrawStep`-count test in `render_buckets.rs`, NOT an iai-callgrind bench (iai counts CPU instructions, not draw calls).
- **Additional packer callers** beyond the plan's list (found via `rg`): `render_focus_ring`, `render_border_shadow`, 6 `render_buckets` test sites (W1).
- **Line drift:** the Patch guard is at `extract.rs:1669` (plan cited 1643); `partition_glyph_ranges` is `buckets.rs:786` (rev-3 docs wrongly said `text/extract.rs:442`).

**Real-app proof (the cycle's origin):** the user reported a see-through overlay in the actual dooduel app. Offscreen capture (`in_game_picking` = `.top_layer()` word-pick scrim vs `in_game_drawer` = no scrim, light theme) measures the base top-bar glyph luminance dropping **~34%** under the scrim (175→115; bright glyphs 230→152). The tier-bleed is fixed end-to-end; the dark-mode near-iso-luminance is a separate app follow-up (filed).

**Deferred (filed in `follow-ups.md`):** per-context ordering for overlapping overlays (single-boundary-v1); same-block backdrop-vs-composite spatial overlap; avatar-editor-as-overlay (now unblocked); dark-mode scrim color. The pick≠paint framework follow-up (`follow-ups.md`) is retired as LANDED.

---

## Wave 7 — the single-boundary suffix must be MATERIALIZED, not assumed (multi-root podium fix, 2026-07-12)

§3.4's tail-contiguity `debug_assert` ("once a top-layer node is seen no base node may follow") is the invariant W0–W6 *rely on* — but W6 only ever verified it on SINGLE-root scenes, where a top-layer subtree is already the escaped `painters_z` tail. The dooduel **podium** crashed it: the `.top_layer()` theme toggle is a **parented** node that escapes to the tail of the MAIN root's `painters_z` (rank 0), while the ~110 confetti are **independent rank-0 `Translate` roots** spawned at podium entry. `context_roots` sorts roots by `(cross_root_rank, entity)`, so the confetti roots sort into a SEPARATE cross-root slot after the main root's escaped top-layer tail — a base node follows a top-layer one and the quad-tier tripwire panics. (`Entity`'s `Ord` is NonMaxU32-inverted, so the *later*-spawned root sorts first; the crash is the same either way — nothing keeps top-layer content contiguous ACROSS roots.)

Root cause: top-layer content is a global contiguous suffix only WITHIN one root's walk; the cross-root concatenation (`context_roots` + `context_tree_paint_order`) does not preserve it. The fix **makes the invariant true** rather than weakening the tripwire: after the §3.1 ancestor climb tags the assembled list, a **stable partition** hoists every top-layer element to the trailing global suffix (base + top-layer relative order both preserved).

- **Shared helpers** (`render/top_layer.rs`): `in_top_layer` (the §3.1 ancestor climb, factored out of the node producer) + `stable_top_layer_suffix` (`sort_by_cached_key` on the `bool` classification — stable, one classify per element). One source of truth for all three tiers.
- **All three producers hoist**, so the tiers agree: the node producer stable-partitions `all.nodes` by `ExtractedNode.top_layer`; the glyph producer stable-partitions its paint-order `order` walk; the icon producer (query-order, no walk) collects emits and stable-partitions them. Glyph/icon derive the former set from the SC's `cross_root_rank > 0` (layout 6f stamps that iff `top_layer != None`, so it agrees with the node tier's `Stacking.top_layer` on every EMITTED entity — a paint-skipped former's subtree emits nothing, the only place they could differ). All node-order-derived indices (`quad_slot_of` / `node_quad_anchors` / `top_layer_boundary`, `RetainedNodeIndex`) rebuild from the post-partition order.
- **Effect-group safe:** a group is uniformly base or top-layer (top-layer is inherited per subtree; a mixed group would already break group contiguity via escape), and a stable partition preserves relative order within each class, so it never splits a group's contiguous run.
- **Byte-stable:** a no-top-layer scene skips the partition entirely; an already-suffix (single-root) scene reorders to the identical order — no boundary and no golden shift. Verified: both GPU legs byte-identical, podium capture succeeds end-to-end (panic-count 0, `home_dark.png` written after `podium.png`).
- **Tests** (`toplayer_block_partition.rs` + `top_layer.rs`): RED→GREEN multi-root reproductions for the quad AND glyph tiers (a view root with a parented `.top_layer()` node + a later independent base root), plus `stable_top_layer_suffix`/`in_top_layer` unit tests.
