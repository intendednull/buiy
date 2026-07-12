# Buiy render — top-layer stacking composite (all-tiers occlusion)

- **Date:** 2026-07-10
- **Status:** draft (rev-2 — folds both spec reviews; the flip to accepted waits
  on the prototype spike + the team-lead's read)
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

### 3.2 The tier packers that gain the per-block partition

Every tier's blob is partitioned by the base↔top-layer boundary (and, for
per-context v1, by top-layer root):

- **quad** — `buckets.rs` `pack_view_partitioned` (extend `PackedPartition`).
- **square shadow** — `pack_shadow_instances`.
- **rounded shadow** — `pack_rounded_shadow_instances` (drawn flat at
  `node.rs:448`; a top-layer rounded caster's shadow would bleed without this).
- **gradient** — `pack_gradient_instances`.
- **band** (border/outline) — `pack_band_instances`.
- **glyph + icon** — the coverage tier's partition is **`partition_glyph_ranges`
  in `text/extract.rs:442`** (NOT `buckets.rs`); it already carries its own
  pack-time contiguity `debug_assert`. Extend it with the same per-block axis.

### 3.3 The three sub-passes that become block-partitioned (design-F2)

The `node.rs` flat pass restructures from "one draw per tier" to "one **tier
stack** per block." Three sub-passes stop being global and run **per block** (base
versions before the top-layer block; a top-layer subtree's own versions within its
block):

1. the **gradient/raster interleave** `interleave_flat_draw` (`node.rs:546`);
2. `run_backdrop_blurs` (`node.rs:668`);
3. the effect-group **step-2b** root-group composite (`node.rs:696`).

**Intra-block order is spike-locked (F2 / § 6).** Today's *global* order is
`tier-stack → backdrop-blur → root-composite`. Whether the per-block order keeps
that or becomes `tier-stack → group-composite → backdrop` (a base group under a
top-layer overlay + a top-layer backdrop over base is the fixture that pins it) is
one of the two things the prototype spike LOCKS. The spec does not assert it; the
spike's base-group-under-overlay + backdrop-both-directions fixtures decide it.

### 3.4 Load-bearing invariant — no group straddles the boundary (design-F5)

A top-layer subtree is a stacking-context boundary, so **no `EffectGroup` can
straddle the base↔top-layer boundary**: any effect group is wholly base OR wholly
top-layer. This is what keeps the group-range axis and the per-context axis
independent (§ 2.1). The packer can enforce it with a `debug_assert` — a straddling
group is a tripwire, exactly like the existing `PackedPartition` contiguity
assert (`buckets.rs:576`, `738`; `tests/render_group_contiguity_gpu.rs`).

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
- **Draw-call-count stability (design-F9):** the `iai-callgrind` gate asserts a
  **no-top-layer scene issues the SAME draw calls** as today (the partition adds
  zero draws when the top-layer block is empty), and a top-layer scene adds only a
  bounded delta (≈ tiers × blocks) with **no off-screen-target allocation** — not
  just a pixel golden.
- **New reftests / goldens (design-F3):**
  - (a) **multi-overlay** (a tooltip over a dialog): a permanent gate ONLY if the
    spike adopts per-context-v1; otherwise a *deferred-follow-up* gate that
    documents the known single-boundary bleed-between-overlays.
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

**The prototype spike decides single-boundary-v1 vs per-context-v1**, by measuring
whether per-root identity is a cheap add.

**Per-root-id design sketch (named either way).** The tail is already
**context-contiguous**: `context_tree_paint_order` descends each top-layer root's
subtree *as a unit* (`assemble_context_tree`), and roots are ordered by
`(cross_root_rank, entity)`, so each top-layer root's instances form a contiguous
run at the tail. Per-root ranges are therefore well-defined **if** we carry the
escaped top-layer **root entity** (or a monotonic per-root ordinal assigned during
the tail walk) on the extract record (§ 3.1); a change in that value marks a new
per-context range — no new sort, no new walk. The spike CONFIRMS the tail is
context-contiguous (so these ranges are well-defined) and measures the cost of
carrying the id; if cheap, per-context-v1 ships, else single-boundary-v1 ships and
per-context is a named follow-up.

### 6.1 Spike charter alignment

The prototype spike (chartered separately) should: carve the single base↔top-layer
boundary + ONE per-context split; run on the GPU lane the § 4 fixtures
(base-group-under-overlay, backdrop+overlay both directions, raster-inside-overlay);
confirm byte-stability on the existing suite; and confirm the top-layer tail is
context-contiguous (so per-root ranges are well-defined). It **LOCKS** the § 3.3
intra-block sequencing and **DECIDES** single-boundary-v1 vs per-context-v1. This
matches § 6 above.

## 7. Open questions / risks

- **Intra-block sequencing (§ 3.3)** — locked by the spike.
- **Granularity (§ 6)** — decided by the spike.
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

---

*Consumes `architecture.md` § 2.3 (one ordered top-layer composite pass),
`paint-order-and-top-layer.md` § 3 (top-layer tail materialization), and the
`effect-compositor.md` machinery (for the rejected Approach B). Schedules
`docs/plans/follow-ups.md:2311`. Target state only; the migration plan lives in
`docs/plans/`.*
