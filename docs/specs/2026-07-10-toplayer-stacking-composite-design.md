# Buiy render — top-layer stacking composite (all-tiers occlusion)

- **Date:** 2026-07-10
- **Status:** **LANDED** 2026-07-12 (rev-4; Approach A, single-boundary-v1). Waves
  W0–W7 executed on `feat/dooduel-multiplayer-m1`; GPU-verified on the RX 6700 XT
  (buiy_core 95/0 + buiy_verify 24/0 byte-stable, `scrim_tier_bleed` acceptance
  BLEED→DIM + 8 `toplayer_occludes_all_tiers_gpu` fixtures GREEN) and proven in the
  real dooduel app (base top-bar text dims ~34% under the word-pick scrim). Drift #1
  (bare gradient/raster-only occlusion) closed with the authoritative `any_top_layer`
  gate. **W7 (`9e6b16e`) closed a PODIUM multi-root contiguity regression (caught by the
  real-app capture, not the goldens): the parented-escaped `.top_layer()` toggle + independent
  base confetti roots violated the "global contiguous suffix" invariant — fixed by materializing
  top-layer as a TRUE global suffix (stable partition across the node/glyph/icon producers, §3.4).**
  See the plan for the wave commits.
- **Area:** `buiy_core` render pipeline — the draw sequencing in
  `crates/buiy_core/src/render/node.rs`, the extract record in
  `crates/buiy_core/src/render/extract.rs`, and the per-tier instance partition
  in `crates/buiy_core/src/render/buckets.rs` + `prepare.rs` +
  `crates/buiy_core/src/text/extract.rs`.
- **Schedules / supersedes:** the OPEN framework follow-up "single-tier glyph
  paint can't be occluded by a top-layer quad (pick≠paint)"
  (`docs/plans/follow-ups.md:2311`, originated 2026-07-10). **Enables removing**
  the `apps/dooduel/src/view/mod.rs:7-11` workaround (the avatar editor forced to
  a full in-flow screen) — via a separate dependent app PR (§ 5), not here.
  Completes the architecturally-specified "one ordered top-layer composite pass"
  (`architecture.md` § 2.3) that v1 stubbed as a per-tier tail.
- **Found by:** the Dooduel modal-scrim root-cause investigation (the "transparent
  scrim" that was in fact a correctly-blended scrim whose base text/borders/icons
  bled through it).

## 1. The problem

### 1.1 What the symptom actually is

The Dooduel modal **scrim** — a `.fill().fixed().top_layer().background(SCRIM)`
translucent dark backdrop (`apps/dooduel/src/view/widgets.rs:189`) — was reported
as "renders transparent: the game behind is not dimmed." It is **not** a
render bug: the scrim IS drawn and IS alpha-blended byte-correctly. Measured on
the real app rendered offscreen on the GPU (`cargo run -p dooduel --bin capture`
→ `in_game_picking.png` vs `in_game_drawer.png`): a canvas-background **quad**
dims `244 → 161`, which is the *exact* linear-space alpha-over of
`SCRIM = srgba(20,22,27, α=156/255)` over `244` (`0.612·0.0070 + 0.388·0.905 =
0.3556` linear → sRGB `160.9`). Blend state (`primitive.rs:426`
`BlendState::ALPHA_BLENDING`), the straight-alpha shader (`shader.wgsl:94`), no
depth test (`primitive.rs:415`), the CPU packing, and the raster interleave are
all correct.

The actual defect: the scrim dims base **quads** and the **raster** canvas but
**not** base **text (glyphs)**, **icons**, or **borders (bands)** — those bleed
through the overlay at full brightness, so a modal backdrop leaves every salient
element bright and reads as "no scrim."

### 1.2 Root cause — global-tier rendering

`node.rs`'s flat window pass draws in **fixed global tiers**, one instanced draw
per primitive kind across the *whole* view (line numbers are the section headers;
the `pass.draw` call sits a few lines below each):

| tier | `node.rs` site | what draws |
|------|----------------|------------|
| shadow (square) | ~428 | all square box-shadows (behind) |
| shadow (rounded) | ~448 | all rounded-caster box-shadows (`pack_rounded_shadow_instances`) |
| quad (+ gradient + raster interleaved) | ~465–546 | all solid fills, gradients, raster canvases |
| glyph | ~580 | all text |
| icon | ~613 | all vector icons |
| band | ~636 | all borders + outlines (on top) |

After the flat pass ends (`drop(pass)`, ~659) come `run_backdrop_blurs` (~668)
and the effect-group **step-2b** root-group composites (~696) — see § 3.3.

A `.top_layer()` subtree escapes to the **tail** of every tier's blob (layout
6f + `cross_root_rank`; `context_tree_paint_order` is shared by the quad walk
`extract.rs` and the glyph walk `text/extract.rs`, so top-layer content is at the
tail of *every* blob). That makes a top-layer **quad** paint over base **quads** —
but a top-layer quad is still only a *quad*, so it draws in the quad tier and the
later global **glyph / icon / band** tiers (base *and* top-layer) paint over it.
The overlay therefore occludes base fills but never base text/icons/borders. This
is the documented limitation (`apps/dooduel/src/view/mod.rs:7-11`) and the logged
open item (`follow-ups.md:2311`); it is the *paint* half of the pick≠paint seam
that also let the theme-toggle win a *pick* while its underlying "Send" glyphs
painted through it.

### 1.3 Evidence — per-tier controlled A/B (GPU, real `Msaa::Sample4`)

`crates/buiy_core/tests/render/scrim_tier_bleed_gpu.rs` renders ONE scene twice
(scrim α156 vs α0, everything else identical) and reads back one pixel per tier:

| tier | with scrim (α156) | without (α0) | result |
|------|-------------------|--------------|--------|
| QUAD base | `[14,98,19]` | `[0,150,0]` | **DIMS** (150→98) |
| QUAD box-fill | `[20,30,139]` | `[20,40,210]` | **DIMS** (210→139) |
| RASTER canvas | `[145,30,33]` | `[220,40,40]` | **DIMS** (220→145) |
| BAND border | `[240,220,20]` | `[240,220,20]` | **BLEEDS** (Δ0) |
| GLYPH text | `[245,245,245]` | `[245,245,245]` | **BLEEDS** (Δ0) |

Falsified along the way: effect-groups (zero formers in the real pick scene),
MSAA (identical dim at `Off` and `Sample4`), and the CPU path (the scrim quad is
proven at flat slot 56 of `[0..61]`, `group=None`, α0.612, drawn last, nothing
overpaints it).

### 1.4 What the target behavior is

A top-layer subtree must occlude the base **across all tiers**: a top-layer
translucent quad must dim base text, icons, and borders exactly as it dims base
fills; a top-layer opaque element must fully hide base content of every tier
under it. This is precisely the "one ordered top-layer composite pass"
`architecture.md` § 2.3 already specifies as a **paint-order relocation on the
same surface** (distinct from the effect-group intermediate-target mechanism) —
which v1 approximated with the per-tier tail and never completed.

## 2. Approaches

Both candidates preserve the extract *walk*, the display list, picking, and the
primitive shaders unchanged. They differ in how the top-layer subtree is
sequenced against the base.

### 2.1 Approach A — same-surface per-context tier ordering (RECOMMENDED)

Draw the **base** context's *complete* tier stack (shadows → quad+raster+gradient
→ glyph → icon → band) plus base backdrop/group composites, then draw each
**top-layer** block's *complete* tier stack over it, on the **same window
surface**, in `cross_root_rank` (z) order. No off-screen target: the top-layer
subtree's translucent quads blend directly over the already-painted base
(including base text/borders), and its opaque elements overwrite it.

Concretely, top-layer content is already segregated to the tail of every tier
blob; today only the quad tier's tail is drawn "late enough" to matter. Approach A
carves a **base↔top-layer boundary** in *every* tier's instance blob and
restructures the `node.rs` flat pass to iterate `(block, tier)` instead of `tier`.

- **What "reuse the partition infra" means precisely.** The ONLY landed
  *draw-time* instance-range split is the effect-group `RangePartitioner`
  (`buckets.rs` `pack_view_partitioned` → `PackedPartition.{group_ranges,
  flat_ranges}`) — plus the glyph/icon tier's own `partition_glyph_ranges`
  (`text/extract.rs:442`). The entity-level `partition_top_layer`
  (`render/top_layer.rs`) splits *entity lists*, is **test-only**, and is **not
  wired into the draw path** (the `node.rs:749-768` comment referencing it is
  stale and must be superseded — [design-F8]). So A does not "generalize an
  existing top-layer split"; it **extends the effect-group RangePartitioner to a
  second (per-context) partition axis orthogonal to the group ranges**, guarded by
  the no-straddle invariant (§ 3.4 / F5) so the two axes never disagree.

### 2.2 Approach B — off-screen top-layer composite pass

Render the base fully to the window, render each top-layer subtree to an
off-screen `Rgba16Float` target, then composite each over the window — reusing the
effect-group compositor (`node.rs` step-1 / step-2b, `compositor.rs`,
`composite.wgsl`). A top-layer root is treated as an implicit isolation group.

### 2.3 The six evaluation axes

| axis | A (same-surface tier ordering) | B (off-screen composite) |
|------|-------------------------------|--------------------------|
| **(i) golden byte-stability** | Base draws with identical instances/order/tiers; only top-layer draw position moves. No-top-layer scenes byte-identical; base half of top-layer scenes byte-identical. **Preserved by construction.** | Base draws to window unchanged, BUT top-layer via an `Rgba16Float` linear intermediate → composite introduces a linear→sRGB round-trip that can shift the top-layer region sub-pixel even where the pixels "should" match. Riskier. |
| **(ii) raster interleave** | Rasters interleave within their block's quad sub-range (`interleave_flat_draw` run per block); a top-layer overlay containing a raster works. **Correct.** | A raster on a grouped element is UNHANDLED — dropped (`node.rs` finding #5 / effect-compositor v1 follow-up). A top-layer overlay containing a raster (the avatar-editor draw canvas, an image modal) **loses the raster** unless B *also* fixes raster-in-group first. |
| **(iii) effect-group interaction** | Base groups composite within the base block; a top-layer subtree's own group composites within the top-layer block. The base↔top-layer split is orthogonal to group membership AND non-straddling (invariant F5), so group ranges live wholly within a block. One new sequencing rule (§ 3.3). | Target-in-target nesting; extra RT-pool pressure (a target per top-layer root ON TOP of group targets) → earlier degradation under `rt_pool_budget`. |
| **(iv) perf (60 Hz hard floor, weak machines, iai gates)** | No new GPU targets. Splits each tier's one instanced draw into (base, top-layer) range draws — a handful more draw calls, **same total instances, zero extra memory/bandwidth.** Effectively free; hw-independent iai gate stays flat (F9 asserts the no-top-layer draw-call count is unchanged). | An off-screen `Rgba16Float` target per top-layer root (memory ∝ overlay bounds) + a composite blit (bandwidth), scaling with overlay count. Real cost on integrated GPUs / wasm. |
| **(v) MT-safety** | Pure draw-order in `node.rs` + a pack-time range partition (both existing MT-safe patterns; mirrors the group partition). No new shared state. | More RT-pool allocation/contention per frame; no new hazard beyond the existing compositor but heavier. |
| **(vi) wasm / WebGL2** | No new GPU features — no compute, no new target formats, band ≤16-attr fold untouched. Same primitives, reordered. **Cleanest.** | An extra `Rgba16Float` render target + sample per overlay; needs `EXT_color_buffer_float` (already required for the compositor) and more GL state — works, but heavier and more WebGL2 surface. |

### 2.4 Recommendation — Approach A; why B loses

**Adopt Approach A.** It *completes the design the architecture already
committed to*: `architecture.md` § 2.3 specifies top-layer as a **paint-order
relocation on the same surface** and deliberately keeps it separate from the
effect-group intermediate-target mechanism ("top-layer is a paint-order
relocation; effect-group is an intermediate-target operation … the two are kept
separate as mechanisms"). Approach A relocates the *entire* top-layer tier-stack
after the base tier-stack; the v1 per-tier tail was the same idea applied to only
one tier.

B loses decisively on three counts: (1) it inherits the **raster-drop-in-group**
regression, which would break the very avatar-editor-as-overlay use case this
refactor is meant to re-enable; (2) it pays a **per-overlay off-screen target +
blit** against the 60 Hz weak-machine and wasm floor, where A is effectively
free; and (3) it **conflates** the two mechanisms `architecture.md` § 2.3 keeps
apart, routing a pure paint-order concern through the intermediate-target
compositor. B's only edge — "reuse the compositor" — is outweighed by having to
*extend* that compositor (raster-in-group) to reach parity with A's zero-new-GPU
path. **Approach B is recorded as the rejected alternative**, on the raster-drop
reasoning above.

*Runner-up rejected for the record:* a **depth-buffer** z-ordering (give each
context a depth and let the GPU sort) is unusable for a translucent-over-base UI
— alpha blending and depth testing do not compose, and it adds a depth
attachment. Not considered further.

## 3. Scope and blast radius

### 3.1 The extract record DOES change (correcting rev-1's "extract unchanged")

`ExtractedNode` (`extract.rs:102-185`) has **no top-layer discriminator**: its
`clip: Option<ClipRect>` is `None` for *both* an unclipped in-flow node and a
top-layer member, and `group: Option<usize>` is orthogonal (effect-group index).
The extract walk *computes* `is_top_layer` (in `effective_clip`, `extract.rs:1227`,
and `effective_outline_clip`, `1246`) but **drops it** — it is only used to force
`clip = None`. So A must **persist a top-layer discriminator on the extract
record** so the packers can partition by it: either

- add a field to `ExtractedNode` (e.g. `top_layer: TopLayer`, or a `top_layer_root:
  Option<Entity>` for the per-context variant — see § 6), **or**
- carry a per-view boundary/partition scalar on `ExtractedNodes` (the base↔top-layer
  split index), if the single-boundary v1 is chosen.

Each tier packer then takes a `top_layer_of` (or `top_layer_root_of`) input — a
closure mirroring the existing `group_by_entity` map threaded to the packers
(`prepare.rs:780-791`, `804-813`).

**Correction (rev-3, spike-proven) — the discriminator is INHERITED, computed by
an ancestor CLIMB, not each node's own `Stacking.top_layer`.** `Stacking.top_layer`
is a **per-node** component: layout only tags the entity that itself called
`.top_layer(...)` — it is NOT propagated to descendants. So a plain CHILD of a
top-layer overlay — a raster canvas, a nested panel, a text run — reads
`top_layer == None` on its OWN component and would be **misclassified as base**,
splitting what must be one contiguous top-layer tail. (The spike hit this as a hard
`debug_assert` panic — § 3.4 — the instant it tested a raster INSIDE an overlay; it
was masked earlier only because the first fixtures used childless flat quads.) The
correct signal: a node is top-layer iff **itself or any ancestor** has
`top_layer != None`, derived by a `ChildOf` ancestor **climb structurally identical
to the landed `nearest_group_entity` EffectGroup-membership climb**, run **after**
`assemble_context_tree` (where `ExtractedNode.group` is assigned) — NOT in
`resolve_one`, which has no ancestor access. The spike confirmed this shape makes
all six GPU fixtures pass clean.

### 3.2 The tier packers that gain the per-block partition

Every tier's blob is partitioned by the base↔top-layer boundary (and, for
per-context v1, by top-layer root):

- **quad** — `buckets.rs` `pack_view_partitioned` (extend `PackedPartition`).
- **square shadow** — `pack_shadow_instances`.
- **rounded shadow** — `pack_rounded_shadow_instances` (drawn flat at
  `node.rs:448`; a top-layer rounded caster's shadow would bleed without this).
- **gradient** — `pack_gradient_instances`. **(rev-4/m2: no retained boundary —
  gradients split base/top-layer INSIDE `block_interleave` by anchor vs the quad
  boundary, § 3.3; a separate gradient boundary would never be consumed.)**
- **band** (border/outline) — `pack_band_instances`.
- **glyph + icon** — the coverage tier's partition is **`partition_glyph_ranges` in
  `buckets.rs:786`** (rev-4/M3 location fix — `text/extract.rs` only *mentions* it
  in doc-comments). It is **entity-keyed** via a `group_of: Fn(Entity) ->
  Option<usize>` closure (not node-keyed), so it needs a **parallel**
  `top_layer_of: Fn(Entity) -> bool` closure + a `top_layer_by_entity` map built at
  BOTH `prepare.rs` call sites (`:782` glyph, `:806` icon), mirroring the existing
  `group_by_entity` maps. Glyph and icon are **separate carriers / instance
  spaces** — they share the FUNCTION + the map, NOT a boundary value. It already
  carries a pack-time contiguity `debug_assert`.

### 3.3 The FOUR sub-passes that become block-partitioned (design-F2)

The `node.rs` flat pass restructures from "one draw per tier" to "one **tier
stack** per block." **Four** sub-passes (rev-4/M2 — was three) stop being global
and run **per block** (base versions before the top-layer block; a top-layer
subtree's own versions within its block):

1. the **gradient/raster interleave** `interleave_flat_draw` (`node.rs:546`);
2. `run_backdrop_blurs` (`node.rs:668`);
3. **(rev-4/M2)** `draw_backdrop_filter_fills` (`node.rs:683`, defn `:1036`) — it
   draws each backdrop-filter former's fill out of the group / glyph-group /
   icon-group ranges, BETWEEN the blur and the composite; a top-layer
   backdrop-filter former's fill must draw in the TOP block (after the top blur),
   else the top flat pass overpaints it. Its `!blurs.is_empty()` guard + group-range
   draws become per-block.
4. the effect-group **step-2b** root-group composite (`node.rs:696`).

**(rev-4/M1) The blur block-split needs a flag on the prepared record.**
`PreparedBackdropBlur` (`blur.rs:406`) has NO `entity` field, so it cannot be
filtered by `top_layer_of(entity)`. Stamp `pub top_layer: bool` on each
`PreparedBackdropBlur` in `prepare_backdrop_blurs` (`blur.rs:467`, where the former
entity → its `ExtractedNode.top_layer` is known) and split the blur slice on that
flag.

**(rev-4/m6) Two-flat-pass Clear/Load footgun.** The restructure opens the flat
window pass TWICE (base, then top-layer), because the blur/composite between them
need the pass closed to sample. The top block's flat pass MUST reuse
`view_target.get_color_attachment()` (`node.rs:417`) — which auto-returns `Clear`
on the FIRST call and `Load` after (precedent: the second call at `node.rs:732`) —
never a hand-built `RenderPassColorAttachment { load: LoadOp::Clear }`, which would
wipe the base block.

**Intra-block order — LOCKED (rev-3) to today's global order, per block:**
`tier-stack → backdrop-blur → root-composite`. The spike ran base-group-under-
overlay (dims 146→96) and backdrop-blur both directions (variance 59.9→1.3 each
way) GREEN under this order. **Honest caveat (do not overclaim):** those fixtures
exercise *cross-block* ordering (the base block fully finishes — in either
intra-block order — before the top-layer block starts), which today's order
satisfies trivially; they do **not** discriminate the `composite-before-backdrop`
alternative, because backdrop-filter groups and opacity/isolation groups are
**disjoint** mechanisms that can only conflict when BOTH live in the SAME block AND
spatially overlap — a case no fixture constructed. Ship today's order (zero
incremental risk vs current behavior); `same-block backdrop-vs-composite spatial
overlap` is a **named open follow-up** (§ 7), not resolved by this effort.

### 3.4 Load-bearing invariant — no group straddles the boundary (design-F5)

A top-layer subtree is a stacking-context boundary, so **no `EffectGroup` can
straddle the base↔top-layer boundary**: any effect group is wholly base OR wholly
top-layer. This is what keeps the group-range axis and the per-context axis
independent (§ 2.1). The packer can enforce it with a `debug_assert` — a straddling
group is a tripwire, exactly like the existing `PackedPartition` contiguity
assert (`buckets.rs:576`, `738`; `tests/render_group_contiguity_gpu.rs`).

**Ship the tripwire in production (rev-3, spike-proven).** The spike added an
equivalent tail-contiguity `debug_assert` and it caught the § 3.1
per-node-vs-climb bug in ONE GPU run — a hard panic, not a silent wrong-pixel
regression. Keep the boundary/contiguity `debug_assert` in the production packers.

**The single-boundary suffix must be MATERIALIZED across roots, not assumed
(W7, 2026-07-12).** The tail-contiguity invariant ("once a top-layer node is seen
no base node may follow") holds automatically only WITHIN one root's walk. Across
roots it does NOT: `context_tree_paint_order` + `cross_root_rank` (§ 3.5) produce a
top-layer suffix per root, but the cross-root concatenation can place a base root
(a rank-0 stacking context) AFTER a different root's escaped top-layer tail — the
dooduel podium (a parented `.top_layer()` toggle in the main root + ~110
independent rank-0 confetti roots) trips the quad-tier tripwire exactly this way.
The fix makes the invariant TRUE rather than weakening the tripwire: after the
§ 3.1 ancestor climb, each producer applies a **stable partition**
(`top_layer::stable_top_layer_suffix`) that hoists every top-layer element to the
trailing global suffix, base + top-layer relative order both preserved. All three
tiers (node quad, glyph, icon) share the one `top_layer::in_top_layer` climb so
their orders agree. Byte-stable: a no-top-layer scene skips it; an already-suffix
single-root scene reorders to the identical order. See the plan's **Wave 7**.

### 3.5 What stays; blast radius

**Stays unchanged:** the extract *walk* + display list ordering;
`context_tree_paint_order` + `cross_root_rank` (they already produce the tail
order A consumes); picking (the pick order already treats top-layer as on top —
this makes *paint* match it, closing the pick≠paint seam); every primitive shader
+ pipeline; the effect-group machinery itself (only its *sequencing* is
partitioned by block).

**Blast radius — everything top-layer:** the Dooduel pick/waiting/reveal/
getting-words scrims; and, framework-wide, tooltips, menus, dialogs, popovers
(`buiy_widgets`). It closes the paint half of the pick≠paint seam
(`follow-ups.md:2311`) and **enables** removing the avatar-editor full-screen
workaround (via the app follow-up, § 5). It is the "top-layer stacking" render
item — schedule it and retire the follow-up.

### 3.6 Patch-path exclusion (rev-3, plan-critical; rev-4/m5 refined)

The partial re-extract **Patch** fast path in `render/extract.rs` re-resolves a
changed entity through `resolve_one` (`extract.rs:1651`/`1702`), which does NOT set
the post-assembly ancestor-climb `top_layer` (§ 3.1). Today it is guarded to
group-free nodes (`extract.rs:1643` `if old.group.is_some()` → force Full) but has
**no top-layer exclusion**. FIX (rev-4/m5): extend that node-side guard to
`if old.group.is_some() || old.top_layer` — forcing a Full rebuild (which re-runs
the climb) for any changed top-layer node; a NEW overlay is a structural change that
already forces Full. **`text/extract.rs` does NOT call `resolve_one`** (its Patch
classifier is `GlyphDamage`; the glyph top-layer signal is re-derived in
`prepare.rs:782`/`806` from the retained-or-Full node records), so **no**
`text/extract.rs` change is needed — the node-side guard suffices. The spike
exercised only first-frame / Full builds, so this stays a required task with its own
test.

## 4. Verification design

- **Acceptance witness (RED → GREEN):** `scrim_tier_bleed_gpu.rs`, assertions
  **flipped to the target behavior** — BAND, GLYPH, and ICON under a top-layer
  scrim must **DIM** (Δ > 0), matching QUAD and RASTER. This FAILS today (Δ0
  bleed) and PASSES after the refactor. (During this spec phase the file stays a
  GREEN witness of *current* behavior; the plan introduces the flip as its first
  RED step.)
- **Base byte-stability (the load-bearing gate):** the ENTIRE existing golden +
  reftest suite — the `buiy_core` `#[ignore]` GPU lane AND the `buiy_verify` GPU
  lane — must not shift for any **non-top-layer** fixture. A golden that shifts
  MUST be a top-layer fixture and is blessed with justification (it now occludes).
  **Spike-confirmed clean:** under the throwaway carve, buiy_core GPU 89/89 +
  buiy_verify GPU 24/24 + the full headless workspace passed with **zero**
  non-top-layer shift (incl. `render_group_contiguity`, `render_msaa`,
  `render_compositor`, `render_degraded_group`, `render_backdrop_blur`, the text
  goldens).
- **Draw-step-count stability (design-F9, rev-4/M4 reframed):** a **deterministic
  HEADLESS** `FlatDrawStep` / draw-count test in the existing `render_buckets.rs`
  style (it already asserts exact `Vec<FlatDrawStep>` sequences at `:546-635`) —
  NOT an `iai-callgrind` bench (that counts valgrind CPU *instructions*, not draw
  calls — the wrong tool). Assert: (a) `block_interleave` with an empty top-layer
  block == byte-identical steps to `interleave_flat_draw` (also Task 2.1); (b) a
  no-top-layer scene issues the SAME tier draws as the baseline, and a top-layer
  scene adds only a bounded delta with **no off-screen-target allocation**.
  CPU-only + deterministic.
- **New reftests / goldens (design-F3):**
  - (a) **multi-overlay** (a tooltip over a dialog): since v1 ships single-boundary
    (§ 6), this is the **deferred-follow-up gate** documenting the known
    bleed-between-overlapping-overlays (spike-confirmed Δ0) — it asserts the CURRENT
    single-boundary behavior and carries a `// FIXME(per-context-v1)` so it flips
    when per-context lands.
  - (b) **base effect group UNDER a top-layer overlay** — the prototype spike's
    fixture graduates into a permanent golden (locks the § 3.3 ordering).
  - (c) **backdrop-blur × top-layer, both directions** — a top-layer subtree with
    `backdrop-filter` over base content; AND a base backdrop element under a
    top-layer overlay.
  - (d) **raster INSIDE a top-layer overlay** — A's decisive win over B; the true
    render-tier proof that a raster-bearing overlay keeps its canvas.
- **Paint == pick across tiers:** a test asserting the new paint order equals the
  existing pick order for a top-layer-over-base fixture (the pick≠paint seam is
  now closed at the paint layer).
- **Assertion metric (spike gotcha).** GPU dim assertions use a **dominant- /
  per-channel** delta, NOT a color-sum: a dark scrim ADDS R/B while cutting G on a
  saturated base, so a naive sum under-reports the dim (`scrim_tier_bleed_gpu.rs`
  already does this right).

## 5. Dependent follow-ups (separate PRs, not in the framework PR)

- **Avatar-editor-as-overlay (design-F4).** "Enables removing the
  `mod.rs:7-11` workaround" is **not done here.** Restructuring
  `apps/dooduel/src/view/avatar_editor.rs` from a full in-flow screen back to a
  top-layer overlay is a **separate dependent APP PR** — and it is the true
  end-to-end acceptance for the raster-in-overlay path (the avatar editor's draw
  canvas is a raster). Keep it OUT of the framework PR.
- **Dark-mode scrim iso-luminance (design-F7) — FAST-FOLLOW, app-side.** The
  theme-invariant `SCRIM = 0x14161b` (α0.61) is near-iso-luminant with the dark
  theme's backgrounds (dark canvas `0x1b1e25 = (27,30,37)` is *lighter* than the
  scrim; surface `0x262a33 = (38,42,51)`), so even the correct quad blend is
  near-invisible in dark mode (surface `38→30`, canvas `27→24`). Keep the fix a
  **separate app PR** (do not couple a render refactor to a color constant), but
  reclassified from "low-priority" to **fast-follow**, with an explicit *done*
  verification: **after A lands, render the ACTUAL reported dark in-game screen and
  confirm the overlay reads as dimmed.** If the residual flat-background
  iso-luminance still reads transparent, the dark-scrim tweak **ships WITH** this
  effort — "done for the user's bug" is the framework fix *plus* whatever the dark
  screen still needs, not the framework fix alone.

## 6. The v1 granularity decision — the spike decides

Two things the model **lacks** for per-context (per-overlay) granularity:
`TopLayer` (`layout/types.rs:1319`) is a **category enum**
(`None/Modal/Popover/Fullscreen/Tooltip`) — two modals share a category — and
`cross_root_rank` (`extract.rs:1196`) ranks by **tier**, not per root. Neither
gives the **per-top-layer-ROOT identity** needed to detect root transitions in the
tail. So there is a real choice:

- **Single-boundary v1 (default):** one base↔top-layer boundary — all top-layer
  content drawn as ONE block over the base. This fixes the reported scrim bug, the
  common single-overlay case, and the avatar-editor. Its known gap: two
  *overlapping* top-layer subtrees at different z (a tooltip over a dialog) would
  still bleed *between each other* (same global-tier problem, one level in).
- **Per-context v1:** each top-layer ROOT drawn as its own tier-stack in z-order —
  fixes the overlapping case too.

**DECIDED (rev-3): single-boundary-v1 ships; per-context is a deferred follow-up.**
The spike confirmed single-boundary fixes the reported bug + the base-group +
backdrop-both + raster-in-overlay cases, and it empirically reproduced the
single-boundary gap (a Tooltip-tier bordered overlay under a Modal-tier scrim →
Δ0, still bleeds between the two overlays). Per-context is a **cheap, well-scoped
follow-up**, not this PR: the tail is already **context-contiguous**
(`context_tree_paint_order` descends each top-layer root's subtree *as a unit* via
`assemble_context_tree`, roots ordered by `(cross_root_rank, entity)` — the
existing landed tests prove it), so per-root ranges are well-defined by carrying
the escaped top-layer **root entity** (or a per-root ordinal) on the extract record
(§ 3.1); a change in that value marks a new per-context range, no new sort/walk.
The per-root partition then mirrors the effect-group `RangePartitioner` N-range
walk. Low-risk, but out of scope for the first PR.

### 6.1 Spike outcome (DONE)

The prototype spike ran (throwaway carve on the RX 6700 XT, reverted clean): it
carved the single base↔top-layer boundary + one per-context split, ran the § 4
fixtures on the GPU lane, confirmed byte-stability on the existing suite, and
confirmed the tail is context-contiguous. Outcomes folded into this rev-3: the
§ 3.3 order is LOCKED (today's, per block); § 6 is DECIDED (single-boundary-v1);
the § 3.1 signal is CORRECTED to an ancestor climb; the § 3.6 Patch-path risk +
the § 3.4 production tripwire were surfaced.

## 7. Open questions / risks / named follow-ups

- **Intra-block sequencing (§ 3.3)** — LOCKED to today's order; the residual
  `same-block backdrop-vs-composite spatial overlap` case is a **named follow-up**
  (disjoint mechanisms; no fixture constructs it today).
- **Granularity (§ 6)** — DECIDED single-boundary-v1; **per-context-v1 is a named
  follow-up** (overlapping overlays bleed between each other; cheap per-root-id add).
- **Patch-path exclusion (§ 3.6)** — a required implementation task; untested by
  the spike.
- **`::backdrop`.** Out of scope (an open question, render README § 5 #3); this
  spec does not add it, but the same-surface top-layer pass is where it would land.

## Change log — rev-2

Folds both spec reviews (both APPROVE-WITH-FIXES; Approach A adopted, no re-open).
Each item verified against code before folding.

1. **[MAJOR] "extract unchanged" was FALSE — corrected (§ 3.1).** `ExtractedNode`
   (extract.rs:102-185) has no top-layer discriminator; `is_top_layer` is computed
   (extract.rs:1227/1246) but dropped. A now adds a top-layer discriminator to the
   extract record + a `top_layer_of` packer input mirroring `group_by_entity`
   (prepare.rs:780-791). The "extract unchanged" claim is removed; the extract
   change is listed in scope.
2. **[MAJOR] per-context needs per-root identity the model lacks — reframed
   (§ 6).** `TopLayer` is a category enum (types.rs:1319), `cross_root_rank` ranks
   by tier (extract.rs:1196) — no per-root identity. v1 defaults to the single
   base↔top-layer boundary; per-context is deferred UNLESS the spike shows per-root
   id is a cheap add — the SPIKE decides. Per-root-id design sketch named (carry
   the escaped root entity / a per-root ordinal on the extract record; the tail is
   context-contiguous). Dropped the "infra is identical" assertion.
3. **[MINOR] rounded-shadow tier added** to the § 3.2 packer list
   (`pack_rounded_shadow_instances`, node.rs:448) — a top-layer rounded caster's
   shadow would otherwise bleed.
4. **[MINOR] § 1.2 tier line numbers fixed:** rounded-shadow ~448, glyph ~580,
   icon ~613, band ~636 (was "~700"). Added the backdrop (~668) / step-2b (~696)
   sites.
5. **[MINOR] § 2.1 "generalizes the split" tightened.** The only landed draw-time
   split is the effect-group `RangePartitioner`; `partition_top_layer`
   (top_layer.rs) is entity-level + test-only + not in the draw path (node.rs:749-768
   stale comment to be superseded — design-F8). Reframed as "extend the
   RangePartitioner to a second axis orthogonal to group ranges."
6. **[HIGH/F2] "one new sequencing rule" replaced (§ 3.3)** with the explicit
   three block-partitioned sub-passes: interleave_flat_draw (546), run_backdrop_blurs
   (668), step-2b root composite (696). Intra-block order flagged as spike-locked
   (current global order is tier-stack→backdrop→composite; the base-group-under-
   overlay fixture pins the per-block order).
7. **[MEDIUM/F5] no-straddle invariant stated (§ 3.4)** — no EffectGroup straddles
   the base↔top-layer boundary; packer `debug_assert` like the contiguity tripwire
   (buckets.rs:576/738).
8. **[MEDIUM/F6] file list completed (§ 3.2).** The glyph/icon partition is
   `partition_glyph_ranges` in text/extract.rs:442, NOT buckets.rs; noted the
   test-only entity-level `partition_top_layer` is superseded, not extended.
   "Reuse RangePartitioner" → "extend it to a second per-context axis."
9. **[HIGH/F3 + F9] verification gates added (§ 4):** multi-overlay reftest (gated
   on the spike's per-context decision), base-group-under-overlay golden,
   backdrop×top-layer both directions, raster-inside-overlay; plus an explicit
   no-top-layer draw-CALL-count-stability assertion in the iai gate.
10. **[MEDIUM/F4] avatar-editor flip = separate dependent APP PR (§ 5)** and the
    true end-to-end raster-in-overlay acceptance; "obsoletes mod.rs:7-11" = "enables
    removing the workaround in a follow-up," not done here.
11. **[MEDIUM/F7] dark-mode scrim reclassified to FAST-FOLLOW (§ 5)**, still a
    separate app PR, with a done-verification (render the actual dark in-game screen
    after A lands; if still iso-luminant, the dark-scrim tweak ships WITH this).

## Change log — rev-3

Folds the prototype-spike findings (throwaway carve run on the RX 6700 XT, reverted
clean to `ad170d3`; the empirical results are the validation — no re-review). Status
flipped draft→active.

1. **§ 3.1 CORRECTED (load-bearing):** the top-layer discriminator is INHERITED —
   computed by a `ChildOf` ancestor CLIMB (node is top-layer iff itself-or-any-
   ancestor has `top_layer != None`), mirroring the landed `nearest_group_entity`
   climb, run AFTER `assemble_context_tree` (NOT `resolve_one`). rev-2's "persist
   `is_top_layer` verbatim" was WRONG: `Stacking.top_layer` is per-node, not
   inherited, so a child raster/panel/text of an overlay misclassifies as base and
   the tail-contiguity `debug_assert` PANICS on raster-inside-overlay.
2. **§ 3.3 LOCKED:** intra-block order = today's `tier-stack → backdrop → composite`
   per block (base-group-under-scrim 146→96; backdrop both directions 59.9→1.3).
   Honest caveat recorded: fixtures test cross-block only; `same-block backdrop-vs-
   composite spatial overlap` is a named follow-up (§ 7).
3. **§ 6 DECIDED:** single-boundary-v1 ships; per-context = a cheap deferred
   follow-up (overlapping-overlays bleed empirically confirmed Δ0; tail already
   context-contiguous; per-root-id mirrors the group climb + `RangePartitioner`).
4. **§ 3.6 NEW — Patch-path exclusion (plan-critical):** the partial re-extract
   fast path calls `resolve_one` directly, bypasses the climb, has no top-layer
   exclusion (untested by the spike) → a required impl task.
5. **§ 3.4 + § 4:** keep the tail-contiguity `debug_assert` as a production tripwire
   (it caught the § 3.1 bug in one GPU run); GPU dim assertions use a dominant-/
   per-channel metric, not color-sum.
6. **Byte-stability CONFIRMED:** buiy_core GPU 89/89, buiy_verify GPU 24/24, full
   headless workspace green, zero non-top-layer shift.
7. **§ 6.1** rewritten from "spike charter alignment" to "spike outcome (DONE)".

## Change log — rev-4

Folds the plan-review (APPROVE-WITH-FIXES, 4 MAJOR + 8 MINOR + 1 note; each
verified against the branch APIs — all correct, none skipped). Spec-touching items:

1. **§ 3.3 → FOUR sub-passes (M2).** Added `draw_backdrop_filter_fills`
   (`node.rs:683`, defn `:1036`) as the 4th block-partitioned sub-pass (it draws
   backdrop-filter-former fills between the blur and the composite — a top-layer
   former's fill must draw in the top block).
2. **§ 3.3 blur flag (M1).** `PreparedBackdropBlur` has no `entity` field → stamp
   `pub top_layer: bool` in `prepare_backdrop_blurs` and split the blur slice on it.
3. **§ 3.3 Clear/Load footgun (m6).** The top block's flat pass must reuse
   `view_target.get_color_attachment()` (Clear-then-Load), never a hand-built Clear.
4. **§ 3.2 location fix (M3).** `partition_glyph_ranges` lives in `buckets.rs:786`
   (entity-keyed), not `text/extract.rs:442`; it needs a parallel `top_layer_of`
   closure + a `top_layer_by_entity` map at both `prepare.rs` call sites; glyph +
   icon share the function + map, NOT a boundary.
5. **§ 3.2 gradient (m2).** No retained gradient boundary — `block_interleave`
   already splits gradients by anchor vs the quad boundary.
6. **§ 4 F9 reframe (M4).** From an `iai-callgrind` "draw-call" bench (wrong tool —
   iai counts CPU instructions) to a deterministic headless `FlatDrawStep` /
   draw-count test in the `render_buckets.rs` style.

Plan-only items (M3 caller list drops `node.rs` + adds `snapshot.rs` /
`modal_showcase`; m1/m3/m4/m5/m7/m8 + the group-formers-collection note) are folded
into `docs/plans/2026-07-10-toplayer-stacking-composite.md` directly.

---

*Consumes `architecture.md` § 2.3 (one ordered top-layer composite pass),
`paint-order-and-top-layer.md` § 3 (top-layer tail materialization), and the
`effect-compositor.md` machinery (for the rejected Approach B). Schedules
`docs/plans/follow-ups.md:2311`. Target state only; the migration plan lives in
`docs/plans/`.*
