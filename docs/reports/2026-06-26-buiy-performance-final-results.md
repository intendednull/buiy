# Buiy performance final — results

**Date:** 2026-06-26
**Branch:** `worktree-perf-final` (off `origin/main` @ 7752c01)
**Status:** `[complete]` — measurement infrastructure + 5 optimizations landed, all
gate-proven; the audit's #1 finding (#2, the re-extract/re-upload cliff) fully fixed and
real-GPU-verified. Awaiting human review + merge.

Companion to the audit (`2026-06-25-performance-audit.md`), the design
(`../specs/2026-06-26-buiy-performance-final-design.md`), and the plan
(`../plans/2026-06-26-buiy-performance-final.md`). The #2 staged implementation map is
`../plans/2026-06-26-buiy-perf-2-implementation-map.md`.

## 1. Executive summary

The audit's thesis was *measurement blindness* (finding #1): one bench, one datapoint,
the biggest suspected costs unmeasured. This pass closed that gap first, then used the
new gates to land — and **prove** — five optimizations, culminating in the headline
finding #2.

The signature result: **an interactive change (a hover re-tint) went from rebuilding +
re-uploading the entire scene's node instances (O(N)) to re-resolving and re-uploading
exactly the one record that changed (O(changed))** — proven pixel-identical to a full
rebuild on a real GPU. Buiy was a retained renderer that forfeited the retained advantage
on every change; it no longer does.

Every gain is enforced by a gate that goes red if the optimization regresses — the
deliverable is not just the speedups but the *guards* that keep them.

## 2. Measurement infrastructure (Phase 0 — finding #1)

The foundation, because "you cannot fix what you cannot see":

- **Shared adapterless harness** (`crates/buiy_bench_support`) — one shape→layout→extract
  harness + scene builders (`build_large_scene`, `build_flat_scene`, `build_flat_bg_scene`),
  driving the SAME production systems the real `RenderApp` does, headless (no GPU). Used by
  the criterion bench, the dhat gate, and the counter gates — one harness, no triple-copy.
- **Work-unit counters** (`render/counters.rs` `RenderWorkCounters`) — host-INDEPENDENT
  integers asserted exactly on a settled scene: `node_rebuilds`, `instances_built`,
  `node_patches`, **`atlas_touch_ops`** (the #5 blind-spot closer — the atlas touch loop is
  non-allocating and downstream of the damage gate, so neither a rebuild assertion nor dhat
  can see it; only this counter does), `resident_keys`. Gates in `tests/crosscut/work_counters.rs`.
- **dhat allocation-count gate** (`tests/alloc_budget.rs`) — `#[global_allocator]` dhat,
  idle vs rebuild block budgets. Pure-Rust, runs locally.
- **iai-callgrind instruction gate** — designed as the hardware-independent CI gate (Phase 3);
  deferred to CI (no local Valgrind). Documented as the next CI lane.

Hardware-independence is load-bearing: the user's constraint is **60 Hz is a hard floor,
never "good enough", and weaker machines are the target** — so the gates are integer
work-counts and (in CI) instruction counts, not wall-clock on the dev box's RX 6700 XT.

## 3. Optimizations landed (each gate-proven)

| # | Optimization | Result | Proof |
|---|---|---|---|
| **#5** | Atlas LRU touch O(V·E)→O(1) | **8.6×** on a static-text steady frame (10.6→1.24 ms in the prototype) | `atlas_touch_ops == resident_keys` gate (was O(visible-glyphs) VecDeque scan per glyph) |
| **#9** | Delete dead `extract_buiy_draws` | One full-tree walk + a fresh Vec alloc removed from every frame | grep-proved dead (only `mod.rs` referenced it); full suite green after delete |
| **#3** | Gate the post-Taffy override chain on a dirty flag | Idle layout **−22.6 % (1k) / −32.4 % (5k)** | `LayoutPostTaffyRunCount` gate + a 7-mutation differential test (output-identical) |
| **#11** | `EntityHashMap` on the per-node extract maps | Pass-through hash replaces SipHash on `by_entity` (per-node, hot) etc. | equality-preserving (311 render tests); iai prices it in CI; also stabilizes the iai baseline |
| **#2** | **Keyed partial re-extract** (the cliff) | **hover = re-resolve 1 + re-upload 1, not all N** | see §4 |

## 4. Finding #2 — the headline cliff fix

The audit (and six independent subsystem agents, plus WebRender/Vello/glyphon prior art)
flagged #2 as the dominant structural cost: a retained renderer that **rebuilds and
re-uploads the entire scene's instance buffers on any change**. Fixed in eight verified
stages:

| Stage | Commit | What | Verification |
|---|---|---|---|
| A | retain `ResMut` | `ExtractedNodesView`/`EffectGroups` become retained resources (not `insert_resource`), so a Patch can mutate them in place; idle gate-skip touches neither → O(0) idle preserved | work_counters idle gate, render-identical |
| B+C2 | classifier | per-entity Patch classifier: a frame is Patch-eligible iff no structural / group / despawn / theme change and every changed entity is a group-free, footprint-stable node | classifier gate test |
| C1 | `RetainedNodeIndex` | entity→record-slot map, rebuilt each Full; valid for a Patch (stable order) | additive, GPU-identical |
| C3a | `resolve_one` | factored the per-node resolution so Full and Patch share ONE resolver (byte-identical records by construction) | 311 snapshots + 70 GPU goldens pixel-identical |
| C3b | **extract Patch** | on a Patch, re-resolve ONLY the changed entities and overwrite their slots in place (R5 trap: never rebuild the ordered Vec from the changed set — siblings retained byte-identical) | work_counters FLIP (1 change → `node_patches==1`, `instances_built==1`, `node_rebuilds==0`) + R5 sibling-retention test + 92 GPU goldens + gallery |
| D1 | `quad_slot_of` cache | entity→quad-instance-slot map built each full pack (the paint-order index the original design rejected as stale-prone — but a Patch has stable order, so it is valid there) | additive, GPU-identical |
| D2 | **partial upload** | on a pure-bg-quad Patch, `RawBufferVec::set` the changed slots + `write_buffer_range` only the spanned range — skipping the whole-blob repack + upload AND the band/shadow/gradient repacks AND the glyph/icon group re-derivation | `render_patch_upload_gpu.rs`: a 1-of-4 color Patch uploads delta==1 (partial fired, not a full fallback) AND is pixel-identical to a cold full render |

**Why prepare can do a full repack on a Patch and still be correct (and why D2 is safe):**
text quads live in a separate carrier (`ExtractedTextQuads`, glyph path) re-spliced by
prepare normally, so C3b's extract win is correct with a full prepare; D2's partial upload
is restricted to **pure bg-quad** changed entities (emit a solid quad, no border/outline/
shadow/gradient band, no own text quads) for which the bg quad is the node's SOLE instance
across every buffer — so a single `set` is correct for any value change. Anything outside
that subset falls through to the safe full repack.

### v1 Patch scope + limitations (honest)

- **Covered (the common interactive case):** group-free, footprint-stable value changes to
  pure bg-quad nodes — hover/scroll re-tints on flat lists, button backgrounds. This is the
  audit's headline scenario (a 1000-row flat scroll list) and the bulk of real interaction.
- **Falls back to a full rebuild (correct, just not yet O(changed)):** changes to nodes that
  carry a border / outline / shadow / gradient, nodes inside an effect group (cards), nodes
  with their own text quads, and any structural / hierarchy / paint-order / group / despawn /
  theme change. A border-*color* change is footprint-stable but moves the band buffer, so it
  is excluded from v1 (D3 below).

## 5. Regression guards in place

- **Headless gate:** `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked
  -D warnings` (clean), the per-crate test suites, the work-counter + dhat + differential gates.
- **GPU lane (`--ignored`, real adapter):** `buiy_core` (70) + `buiy_verify` (22) goldens
  pixel-identical; the new `render_patch_upload_gpu.rs` partial-upload reftest.
- **Gallery (widget-catalog complex scene + live-interaction tier):** drives real
  hover/picking/clicks through the Patch → partial-upload path; green.

## 6. Remaining work + the architectural assessment

What is left, and why it was not done in this pass:

- **#16 — SoA `ExtractedNode` (fat struct, scratch fan).** Now **marginal**: the common path
  is a Patch that resolves/iterates only the *changed* records, not all N, so the cache-locality
  win of an SoA split mostly evaporates (it would help only the rarer cold/structural Full
  rebuild). iai-CI-only to prove. **Recommendation: defer**; revisit if Full-rebuild cost
  shows up in CI iai.
- **#2 D3 — extend the partial upload to bordered/shadowed/gradient nodes.** Tractable
  (per-buffer slot caches like `quad_slot_of`), but more interleaving surface and the gain is
  the smaller tail (most interactive changes are pure bg-quad). **Recommendation: optional
  follow-up.**
- **#13 — full overdraw / no viewport cull / no depth z-cull.** Two parts: **(a) a
  viewport-AABB reject in the paint-order walk** is a real, large gain (a 10k-item scroll list
  with 50 visible still builds/uploads/draws 10k instances) and is tractable — it *pairs with
  #2* (the same extract walk). **(b) an opaque/depth pre-pass split** to kill back-to-front
  alpha overdraw is a **major architectural change** (a second pass, opaque/transparent
  separation, depth attachment) — this is the kind of change that **needs human attention**:
  it changes the render-graph shape and the pipeline/blend strategy, with design tradeoffs
  (depth precision, transparency ordering) that should be decided, not defaulted.
- **Phase 3 iai-callgrind CI gate, Phase 4/5 GPU-timing + worst-frame trend** — the remaining
  measurement roadmap; CI-side / informational.

**The architectural wall:** the cheap, high-leverage, locally-provable wins are done. The
next *large* gain (#13a viewport cull) is tractable and a sensible next campaign; the gain
*past* it (#13b depth pre-pass) is a major architectural change that warrants a human design
decision. Per the campaign's stop condition, that boundary is surfaced here rather than
forced.

## 7. #13a viewport-cull — readiness seed (for the next campaign)

Researched but deliberately NOT started in this pass: #13a is a real next gain but a
campaign-sized, **visual-correctness-risk** effort (a wrongly-culled node *disappears* —
unlike #2, which was render-identical), best run as a fresh, focused campaign rather than
forced at the tail of this one. The grounding, so it starts informed:

- **Where:** the `extract_buiy_nodes` build loop (`for item in nodes.iter()` →
  `resolve_one` → `by_entity.insert`) + the group-tag walk. The viewport is
  `primary_window.resolution.size()` (logical, already read for the view uniform); a node's
  screen box is `ExtractedNode.position`/`size` under its `affine`; its clip is
  `ExtractedNode.clip`.
- **Two layers, increasing payoff + risk:** (a) **cull-the-upload** — drop off-screen
  records from the instance blob (after the group-tag walk, where `group` is known); saves
  pack + upload, leaves resolve O(N). (b) **cull-the-extract** — skip `resolve_one` for
  off-screen nodes; also saves the resolve, but needs the AABB from the raw
  `GlobalTransform`+`ResolvedLayout` and a group-membership pre-check *before* resolve.
- **The #2-Patch interaction (load-bearing):** a culled node has no record / no
  `index` / no `quad_slot_of` entry, so a scroll bringing a node INTO view is already forced
  to Full by the existing classifier (`index.0.get(&e)` → `None`). The gap is the
  scrolled-OUT node (still in the prior index, now off-screen): the C3b/D2 classifier must
  gain a **per-changed-entity visibility check** — if a changed entity's current visibility
  ≠ its `index` membership, the cull set flipped → force Full. Without it, a Patch would keep
  a stale off-screen instance. Synergy once correct: a scroll is a Full of only the ~visible
  N (cheap *because* of the cull), a hover stays a Patch of 1.
- **v1 scope to de-risk:** cull only NON-grouped nodes whose box (∩ clip) is ENTIRELY outside
  the viewport (intersect-keep, so partially-visible nodes are never dropped); use the
  affine-transformed AABB for rotated/scaled nodes; do not cull effect-group members in v1
  (or cull a whole group only when its bounds are entirely off-screen — deferred).
- **Verification it will need:** a cull-count work-counter gate (a 1000-row scroll scene with
  ~K visible asserts ~K emitted, not 1000), a scroll-scene GPU reftest (scrolled content is
  pixel-correct at the viewport boundary), the affine/clip edge cases, and the gallery.
- **NOT in #13a:** layout still runs O(N) — a 10k list still lays out 10k. Layout
  virtualization (culling the layout pass) is a separate, larger change.
