# Buiy text — architecture

**Parent:** [README.md](README.md)

The structural skeleton of the text spec: who owns the cosmic-text engine state
(`FontSystem` / `SwashCache`) across Bevy's two worlds, the per-text-entity
state shape, WHERE text work runs in the `BuiySet` chain, the
change-detection/damage discipline (render § 3.1 applied to a third subsystem),
the scale-factor model, and the crate/dependency pin. Every sibling
([measure-and-layout.md](measure-and-layout.md),
[font-assets.md](font-assets.md),
[glyph-pipeline.md](glyph-pipeline.md), [editing-and-ime.md](editing-and-ime.md)) assumes the
seams pinned here. The render-side seam this spec produces into is **built and
GPU-verified** — `BuiyAtlas`, `AtlasWarmupQueue`, `GlyphAlphaInstance`, and the
glyph draw branch all exist
([render atlas-and-text-seam.md § 1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#1-the-seam-stated-once));
this spec fills the producer slot, it does not redesign the warehouse.

All cosmic-text facts below are verified against docs.rs for **0.19.0** (the
pinned version, § 7), not against the prior-art folder — which is stale on one
load-bearing point (see [Open questions](#open-questions)). Tier legend per
[foundation/README.md](../2026-05-07-buiy-foundation/README.md#tier-legend).
This file owns the F-tier engine backbone the
[foundation text.md § 3.4](../2026-05-07-buiy-foundation/text.md#34-typography)
rows ride on — not the rows themselves (font matching, measure body,
rasterization, and the editor belong to the siblings above).

## 1. Engine state ownership

### 1.1 `SharedFontSystem(Arc<Mutex<FontSystem>>)`, cloned into both worlds

**Decision: one `FontSystem`, owned by a newtype resource
`SharedFontSystem(Arc<Mutex<FontSystem>>)`, inserted into the main `App` with
the `Arc` clone inserted into the `RenderApp`** (during render-side text
registration, which sees a live `RenderApp` via the
[render architecture.md § 1.1](../2026-06-03-buiy-render-pipeline-design/architecture.md#11-plugin-shape-buiyplugins-finish-adds-buiyrenderplugin-whose-build-sees-a-live-renderapp)
`finish`-ordering seam).

Two committed facts force dual-world `&mut` access:

1. **Measurement is main-world-synchronous.** Content sizing runs *inside* the
   Taffy measure pass in `BuiySet::Layout`
   ([measure-and-layout.md](measure-and-layout.md)), and shaping entry points
   are `&mut FontSystem` by signature (`shape_until_scroll(&mut self, &mut
   FontSystem, prune)`, `line_layout(..)`).
2. **Rasterization is render-world-lazy.** The GPU-verified seam puts
   `BuiyAtlas` in the render world
   ([`atlas/mod.rs`](../../../crates/buiy_core/src/render/atlas/mod.rs)
   `register`, ~:95) with a rasterize-on-miss closure (`get_or_insert`,
   [`atlas.rs`](../../../crates/buiy_core/src/render/atlas/atlas.rs) ~:58;
   [seam § 3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#3-the-insertlookup-api--the-only-handle-the-seam-touches)),
   and `SwashCache::get_image_uncached(&mut FontSystem, CacheKey)` needs the
   **same** `FontSystem`: `fontdb::ID`s are stable only within one engine, so a
   second instance would mis-key every glyph
   ([prior-art/bevy-cosmic-edit/lessons.md](../../prior-art/bevy-cosmic-edit/lessons.md)
   lists two-`FontSystem` coexistence as an Avoid row).

`FontSystem` is **verified `Send + Sync` in 0.19** (docs.rs auto-traits), so
`Arc<Mutex<_>>` is sound — no `NonSend` pinning. Contention is bounded by
construction: the render world locks only on atlas misses (steady state = zero
misses = zero locks), the main world only when text changed (§ 5). Under
pipelined rendering the overlap is frame-N extract/raster vs frame-N+1
`Layout` — both gated on changed text. Gate #14 watches a text-heavy fixture
regardless (§ 8).

> **Runner-up rejected: the main-world-staging model** (bevy_text 0.15–0.18):
> rasterize main-side, stage `(key, bitmap)` pairs across extract, feed
> eviction back render→main. It needs three mechanisms the mutex doesn't — a
> residency mirror, a bitmap staging queue, an eviction-feedback channel — and
> its correctness rests on a subtle grace-window argument instead of on types.
> It remains the named **escape hatch** if measured contention breaks the
> gate-#14 budget. **Also rejected:** `NonSendResource` (strictly dominated now
> that `Sync` is verified); render-world-only ownership (impossible — measure
> is main-world-synchronous).

### 1.2 The three lock sites — exhaustive

Exactly three places take the `SharedFontSystem` lock; reviewers reject a fourth:

| # | World | Site | When it locks |
|---|---|---|---|
| 1 | main | the Taffy measure closure inside `TaffyCompute` ([measure-and-layout.md](measure-and-layout.md)) | only for text nodes Taffy re-measures (dirty-marked by `TextSync`, § 4.1) |
| 2 | main | `TextCommit`'s `shape_until_scroll` (§ 4.2) | only for buffers in the commit trigger set (§ 5.1) |
| 3 | render | the `get_or_insert` miss closure in `extract_buiy_glyphs` (§ 4.4), via `SwashCache::get_image_uncached` ([glyph-pipeline.md § 3.2](glyph-pipeline.md)) | only on an atlas miss (new glyph / new scale) |

The 0.19 **lazy setters** keep `TextSync` lock-free: `set_text` / `set_size` /
`set_metrics` / `set_wrap` take no `FontSystem` by signature — mutation is
recorded, shaping deferred to the next lock-bearing site. **F**

### 1.3 `SwashCache`: a render-world-only `Resource`

**Decision: `SwashCache` is a plain render-world `Resource`** (verified
`Send + Sync`), `ResMut` only in the glyph producer's miss path
([glyph-pipeline.md](glyph-pipeline.md)). Rasterization is its only consumer
and happens render-side; main-world measure/caret math reads `Buffer` layout,
never raster output. Keeping it outside the `FontSystem` mutex means a
main-world shape pass never serializes against the raster cache.

> **Runner-up rejected:** bundling it into the same `Arc<Mutex<…>>` — one lock
> instead of two, but it couples the main-world shaping lock to a cache only
> the render world uses, widening contention for zero benefit.

**Gate-#15 note — the caching path is unused (adjudicated, review round 1).**
`SwashCache.image_cache` grows monotonically *if used*
([critiques.md](../../prior-art/cosmic-text/critiques.md) § Atlas churn) — so
Buiy never uses it: the producer rasterizes via `get_image_uncached`
([glyph-pipeline.md § 3.2](glyph-pipeline.md)), and `BuiyAtlas` — already
content-addressed, deduplicating, and LRU-bounded — is the **one** bitmap
cache. The resource is kept solely for API access (its internal scale
context); `image_cache` stays empty by construction, so there is no trim to
build and no gate-#15 growth to police beyond the atlas's own audited budget.

> **Runner-up rejected:** `get_image` + a Buiy-owned trim draining entries
> whose `CacheKey`s have left `BuiyAtlas` — a second cache duplicating the
> atlas's eviction policy, plus trim machinery, for bitmaps the atlas already
> dedups and retains. **F**

## 2. `FontSystem` lifecycle

### 2.1 Construction: registered fonts only, at plugin init

**Decision: construct via `FontSystem::new_with_locale_and_db(locale, db)` at
plugin init with only bundled/registered fonts; the system-font scan is opt-in
and asynchronous** (§ 2.2). Locale via the `sys-locale` default feature.

`FontSystem::new()` mmaps every system font on the calling thread —
cosmic-text issue #505, ~1.3 s
([critiques.md](../../prior-art/cosmic-text/critiques.md)), and the prior-art
lesson commits the mitigation: system fonts are opt-in. The lifecycle invariant
owned here: **`SharedFontSystem` exists and is lockable (registered fonts
resident) before the first `BuiySet::Layout` that measures text.** Registration
mechanics (the `@font-face` analogue, fallback stacks) are
[font-assets.md](font-assets.md)'s.

> **Runners-up rejected:** `FontSystem::new()` at startup — the documented
> anti-pattern (#505). Fully-lazy construction on first text entity — saves
> nothing (the registered-only constructor is cheap), adds a first-text-frame
> hitch plus an `Option<SharedFontSystem>` everywhere.

### 2.2 The async system-scan merge and `FontsGeneration`

An opted-in system scan runs on `AsyncComputeTaskPool` and merges via
`font_system.db_mut()` under the mutex when complete. A font-set change is a
**reshape trigger**: the merge bumps a `FontsGeneration` counter resource that
`TextSync`'s trigger set includes (§ 5.1), so every `TextBuffer` reshapes once
against the enriched fallback set — late fonts never leave stale tofu. The
same bump fires when [font-assets.md](font-assets.md)
registers an asset font at runtime. **F**

### 2.3 Warmup

The atlas spec owns the pre-paint warmup *mechanism*
([seam § 2.3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#23-warmup),
[`warmup.rs`](../../../crates/buiy_core/src/render/atlas/warmup.rs) ~:19); this
spec decides *what* to warm: the ASCII range of the default theme font at the
primary window's scale factor, pushed as `AtlasWarmupRequest`s.
[glyph-pipeline.md](glyph-pipeline.md) owns request construction (it owns the
`AtlasKey` shape). **F**

## 3. Per-text-entity state

### 3.1 `TextBuffer(cosmic_text::Buffer)` — a retained component

**Decision: per-text-entity state is a retained component
`TextBuffer(cosmic_text::Buffer)`, mutated in place** (the precise field
shape — the buffer plus the cached intrinsics — is owned by
[measure-and-layout.md § 2.3](measure-and-layout.md); this section pins only
the retained-component decision) — `Buffer` is verified
`Send + Sync`, so a plain `Component` works (Bevy 0.15+'s `CosmicBuffer` is the
precedent, [prior-art/cosmic-text/integration.md](../../prior-art/cosmic-text/integration.md)
§ Bevy). cosmic-text's per-`BufferLine` shape/layout caches are the
typing-latency win ([lessons.md](../../prior-art/cosmic-text/lessons.md)
Borrow #1); rebuilding the `Buffer` discards them and re-pays full shaping per
edit. A component also gives despawn cleanup for free (no GC system — contrast
`LayoutTree.by_entity` + `RemovedNodesGc`) and Bevy change detection as the
dirty signal (§ 5).

> **Runners-up rejected:** rebuild-per-change (discards the per-line caches —
> the full-buffer-reshape cost [critiques.md](../../prior-art/cosmic-text/critiques.md)
> warns about); a central `HashMap<Entity, Buffer>` resource — only forced if
> `Buffer` were `!Send` (it isn't), and it loses change detection, parallel
> query access, and automatic despawn cleanup.

### 3.2 The lazy-setter contract, and `Shaping::Advanced`

The 0.19 split this component leans on: **setters are lockless, shaping is
lock-bearing** (§ 1.2). `layout_runs(&self)` takes neither `&mut` nor a
`FontSystem` — by signature it **cannot shape**, so unshaped lines are silently
absent from its iterator. The producer therefore `debug_assert!`s that no
visible `TextBuffer` is dirty-unshaped at extract (§ 4.4) — the tripwire for
any future system mutating a buffer after `TextCommit`. `Shaping::Advanced` is
hard-pinned for every `set_text`; `Shaping::Basic` is never exposed
([lessons.md](../../prior-art/cosmic-text/lessons.md): Basic breaks complex
scripts for a micro-optimization). **F**

### 3.3 `ComputedTextLayout` — the idempotent output

`TextCommit` (§ 4.2) writes the settled line/run geometry to a
`ComputedTextLayout` component (read by caret math, picking, a11y bounds,
extract). The write is **idempotent — bump the change tick only when the runs
actually changed** — mirroring `write_resolved_layout`'s guard
([`layout/systems.rs`](../../../crates/buiy_core/src/layout/systems.rs)
~:2643–2656): an unconditional re-insert keeps `Changed<ComputedTextLayout>`
perpetually true and cascades a full extract rebuild every frame — the exact
bug that guard exists to prevent. **F**

### 3.4 `Editor` — deferred to [editing-and-ime.md](editing-and-ime.md)

`Editor<'buffer>` takes `impl Into<BufferRef<'buffer>>` (owned-or-borrowed), so
the editable-entity composition (Editor-optional / Buffer-required) is open to
[editing-and-ime.md](editing-and-ime.md); nothing here assumes a text entity is read-only.
**F (seam)**

## 4. Pipeline placement

No new top-level `BuiySet` variant — the same posture as
[render architecture.md § 5.1](../2026-06-03-buiy-render-pipeline-design/architecture.md#51-the-foundation-order-unchanged).
Text adds **two named layout steps** and **one extract system**:

```
BuiySet::Layout [ RemovedNodesGc → WritingModeInherit → TextSync → SyncStyles → …
                  → CqDescendantInvalidate → CqDescendantReRun → TextCommit ]
  → Style → Input → Animate → (render-prep) → Picking → A11yUpdate → Render
  → ExtractSchedule [ extract_buiy_glyphs ]
```

`(render-prep)` in the sketch is the `.after(Animate).before(Picking)` system
*window*
([render architecture.md § 5.2](../2026-06-03-buiy-render-pipeline-design/architecture.md#52-the-render-prep-stage-between-animate-and-picking)),
not a `BuiySet` variant — systems land there by ordering constraints, not by
membership in a named set.

### 4.1 `BuiyLayoutStep::TextSync` — before `SyncStyles`

**Decision: a new named layout sub-step, `BuiyLayoutStep::TextSync`, chained
between `WritingModeInherit` and `SyncStyles`**
([`layout/pipeline.rs`](../../../crates/buiy_core/src/layout/pipeline.rs)
~:17–76; the Phase-4 `WritingModeInherit` insertion is the precedent for
growing the enum). It creates/updates `TextBuffer` from the authored text
components via the lazy setters (no lock) and **marks the Taffy node dirty**
when content changed — Taffy caches measure results, so an un-dirtied node
serves a stale measurement.

It is a hard before-`SyncStyles` dependency: `sync_styles`
([`layout/systems.rs`](../../../crates/buiy_core/src/layout/systems.rs)
~:2344–2356) must know whether an entity is a measured text leaf when creating
its Taffy node, and the dirty-mark must land before `TaffyCompute`. It runs
after `WritingModeInherit` because its trigger set includes
`Changed<WritingModeResolved>` (§ 5.1).

> **Runners-up rejected:** bare `.before(BuiyLayoutStep::SyncStyles)` —
> functionally identical but invisible to `tests/layout_pipeline_order.rs` and
> the step table; rejected for the same reason layout named all eleven steps. A
> separate pre-`Layout` `BuiySet` — adds a top-level set the foundation order
> doesn't have.

### 4.2 `BuiyLayoutStep::TextCommit` — the new final step

**Decision: a trailing `BuiyLayoutStep::TextCommit` after `CqDescendantReRun`
(the current final step).** It `set_size`s each changed `TextBuffer` to its
resolved box, runs `shape_until_scroll` (lock site #2), and writes
`ComputedTextLayout` idempotently (§ 3.3).

It must trail `CqDescendantReRun` because steps 8–9 can still rewrite
`ResolvedLayout`; placing it *inside* `Layout` preserves the invariant **all
layout — including text line layout — is settled when `BuiySet::Layout` ends**:
`Input`'s click→caret hit-testing, `Picking`, `A11yUpdate`'s text bounds, and
extract all consume `ComputedTextLayout` the same frame.

> **Runners-up rejected:** the render-prep window
> (`.after(Animate).before(Picking)`,
> [render architecture.md § 5.2](../2026-06-03-buiy-render-pipeline-design/architecture.md#52-the-render-prep-stage-between-animate-and-picking))
> — it runs after `Input`, so click-to-caret would read frame-stale line
> layout; and an `Animate` system mutating font-size invalidates layout anyway
> (next-frame full pass, like any layout input). A `BuiySet::Render` main-world
> system — strictly later, same staleness, no benefit.

### 4.3 The measure-readiness contract

What this file hands to [measure-and-layout.md](measure-and-layout.md) — true
when `TaffyCompute` runs: (1) every text entity's `TextBuffer` reflects this
frame's authored content; (2) its Taffy node is dirty-marked iff content
changed; (3) `SharedFontSystem` is lockable inside `taffy_compute`. The measure
seam owns everything past that: the `compute_layout_with_measure` switch, the
measure body, and the `TaffyTree` **context-type change** — today the tree is
`TaffyTree<()>` ([`layout/tree.rs`](../../../crates/buiy_core/src/layout/tree.rs)
~:14–18) and `taffy_compute` calls plain `compute_layout` per root
([`layout/systems.rs`](../../../crates/buiy_core/src/layout/systems.rs)
~:2596–2641). `TextSync`'s "register text context on the node" half must match
whatever context shape that file picks — a coordination pin the plan resolves
before implementation, not an open question.

### 4.4 `extract_buiy_glyphs` — the glyph producer, in `ExtractSchedule`

**Decision: the render-world glyph producer is a single `ExtractSchedule`
system, `extract_buiy_glyphs`** — `Extract<Query<(TextBufferAccess,
&ComputedTextLayout, &GlobalTransform, …)>>` in the accessor's **read-only
form** ([measure-and-layout.md § 2.3](measure-and-layout.md) pins
`TextBufferAccess`; on editable entities the authoritative `Buffer` lives in
`TextEditState`, so binding bare `&TextBuffer` would read the display
component instead), plus
`ResMut<BuiyAtlas>` / `ResMut<ExtractedGlyphs>` / `ResMut<SwashCache>` and the
`SharedFontSystem` clone for the miss closure. It fills the GPU-verified
producer slot ([`render/prepare.rs`](../../../crates/buiy_core/src/render/prepare.rs)
~:46–51) exactly as that slot's docs describe; per-glyph mechanics (`AtlasKey`,
rasterization, instance packing) are [glyph-pipeline.md](glyph-pipeline.md)'s.

`ExtractSchedule` is the only place one system can simultaneously read
main-world components and mutate render-world resources — and
`layout_runs(&self)` is read-only by signature (shaping already happened in
`TextCommit`), so the `Extract` read-only contract on the *main* world holds
([render architecture.md § 1.2](../2026-06-03-buiy-render-pipeline-design/architecture.md#12-the-extract-boundary-extractschedule--extractquery)).
Instance `uv`/`page` can only be filled **after** `get_or_insert` returns the
`AtlasEntry`, so residency and emission must be one pass; doing it at extract
means the prepare systems (`RenderSystems::Prepare`, inherently after
`ExtractSchedule`) see settled data with no new ordering edges.

> **Runner-up rejected:** extract a runs snapshot, emit in a
> `RenderSystems::Prepare` system — needed only if emission required
> prepare-only data (`ViewTarget`, canonical per-view scale). v1 is D2
> primary-window-only (§ 6), so the scale factor is readable from the
> main-world `Window` at extract; the split costs a second per-frame carrier
> and a copy of every run, buying nothing until true multi-view lands.

### 4.5 Order is test-pinned

`tests/layout_pipeline_order.rs` gains the two new steps; the producer's
position gets the same order-assertion treatment render-side
([verification.md](verification.md)).

## 5. Change detection and damage

The render damage-gate discipline
([render architecture.md § 3.1](../2026-06-03-buiy-render-pipeline-design/architecture.md#31-per-frame-instance-set-changedt-gated)
+ the [2026-06-07 retention design](../2026-06-03-buiy-render-pipeline-design/2026-06-07-render-extract-retain-damage-design.md))
and layout's trigger-set discipline
([layout architecture.md § 1.2](../2026-05-08-buiy-layout-design/architecture.md))
applied to a third subsystem. **Steady-state contract: no text changed → zero
shaping, zero extract rebuild, an O(visible-keys) touch pass only.**

### 5.1 Three trigger sets, one per pass

| Pass | Fires on | Action |
|---|---|---|
| `TextSync` (§ 4.1) | `Or<(Changed<Text>, Changed<text-style carriers (font family/size/weight/line-height/wrap/align/direction)>, Added<TextBuffer>, Changed<WritingModeResolved>)>` ∪ `FontsGeneration` bump ∪ theme font-token swap | apply lazy setters; mark Taffy node dirty |
| `TextCommit` (§ 4.2) | the `TextSync`-dirty set ∪ `Changed<ResolvedLayout>` on text entities | `set_size` to resolved box; `shape_until_scroll` (lock #2); write `ComputedTextLayout` **idempotently** (§ 3.3) |
| `extract_buiy_glyphs` probe (§ 4.4) | **summary — the normative union is [glyph-pipeline.md § 6.2](glyph-pipeline.md)**: the layout/transform/clip/visibility/stacking/color `Changed<>` probes ∪ `theme.is_changed()` ∪ scale-factor change (a **value compare** against the producer's cached last-seen factor for the primary window, § 6 — never `Changed<Window>`, whose tick fires on every cursor move because bevy_winit writes `Window.physical_cursor_position` per `CursorMoved`) ∪ the caret/selection visual state ∪ the `RemovedComponents` streams | rebuild `ExtractedGlyphs` + `ExtractedTextQuads`; else retain (§ 5.2) |

Deliberate **exclusion**: `Changed<ScrollOffset>` is *not* a reshape trigger —
scroll moves glyph rects via `GlobalTransform`; layout and shaping are
unchanged (the same exclusion rationale as layout's trigger table). Idempotent
writes at every stage are load-bearing, or downstream `Changed<>` probes go
perpetually hot.

> **Runners-up rejected:** reshape-every-frame (the full-buffer-reshape cost
> paid unconditionally — kills gate #14 on static-text fixtures); content-hash
> comparison instead of change ticks (catches interior-mutability mutations
> Buiy doesn't have on text inputs; ticks are free; redundant machinery).

**One-frame-latency pin (so it is never filed as a bug):** `BuiySet::Style`
runs *after* `Layout`, so a font-style value resolved in `Style` reaches
shaping on the **next** frame's `TextSync` — consistent with the existing
style→layout latency for every other styled layout input.

### 5.2 `ExtractedGlyphs`: retain-with-probe + the every-frame key touch

**Decision: the producer retains `ExtractedGlyphs` across frames, rebuilding
only when the § 5.1 probe fires (mirroring `ExtractedNodesView`,
[`render/extract.rs`](../../../crates/buiy_core/src/render/extract.rs)
~:405–429 + ~:620–634), and keeps a retained `Vec<AtlasKey>` of visible keys
alongside, touched every frame.**

Glyph counts dwarf node counts (a productivity fixture is ~10⁴ glyphs), so a
per-frame rebuild violates the committed steady-state discipline — the
2026-06-07 retention design exists precisely because R5/R6 got this wrong for
nodes. But retention creates a trap the node path lacks: **retained instances
embed `uv`/`page`.** If the producer skips frames, `get_or_insert` never
touches the keys; `drain_grace_expired`
([`atlas.rs`](../../../crates/buiy_core/src/render/atlas/atlas.rs) ~:105)
evicts them, the allocator reuses the cells, and the retained UVs sample
**someone else's glyph** — a true stale-paint bug, not a perf miss. The seam's
"correctness never depends on residency" clause
([seam § 2.4](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#24-lru-eviction--the-gate-15-contract))
holds only if stale entries are never *sampled*.

The fix is as-built: `touch_existing(key)`
([`atlas.rs`](../../../crates/buiy_core/src/render/atlas/atlas.rs) ~:97). A
tiny always-running system touches every retained visible key each frame,
keeping visible glyphs LRU-warm so eviction can only reclaim genuinely
invisible glyphs. This touch pass is the *only* unconditional per-frame text
work — O(visible keys) over a flat `Vec`. A dedicated headless test pins the
hazard: retain N > `eviction_grace` frames, assert the keys stay resident (§ 8).

> **Runner-up rejected (as the spec target):** rebuild-every-frame — correct,
> simple, no stale-uv hazard, and **acceptable as the plan's first landing
> step**; rejected as the target because it burns the gate-#14 budget on every
> static-text frame. The plan may stage through rebuild; retention + touch land
> in the same phase, before the gate.

## 6. Scale factor and multi-window

**Decision: shape in logical px, rasterize physical at emission.** `Metrics` /
`Buffer` state is logical px end-to-end — matching Taffy's logical-px tree and
`ResolvedLayout`/`ClipRect` (the view uniform carries `scale_factor` for the
GPU transform). cosmic-text 0.19 is built for exactly this split: `LayoutGlyph`
`x`/`y`/`w` are logical, and `physical((x, y), scale) -> PhysicalGlyph {
cache_key, x: i32, y: i32 }` produces the scale-aware `CacheKey` (physical font
size + subpixel bin) the `AtlasKey` derives from
([glyph-pipeline.md](glyph-pipeline.md) owns the derivation;
[seam § 4.3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#43-subpixel-snapping-note)
carries the pixel rect through). Pinned consequences:

- **A scale change re-rasterizes, never reshapes.** New `CacheKey`s; old
  entries age out via the LRU grace. `Buffer` state is scale-independent.
- **The global `BuiyAtlas` stays correct under future mixed-DPI multi-window**:
  the key embeds scale, so two windows at different factors are disjoint key
  sets — preserving the window-independent-coverage rationale for the shared
  atlas ([seam § 2.1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#21-type-and-placement)).
- **v1 reads the primary window's scale factor** (D2 — parity with
  `TopLayerActivation` and the render spec's primary-view extract,
  [render architecture.md § 4](../2026-06-03-buiy-render-pipeline-design/architecture.md#4-per-window-node-group-ownership-cross-cutting--318-f-tier)).
  Per-view scale at emission is reserved structure; do not design new text
  state that assumes one window.

> **Runner-up rejected:** scale-baked `Metrics` (bevy_text's approach). It
> makes measure return physical px (a units mismatch with Taffy), forces a full
> **reshape** on every scale change, and breaks the window-independence that
> justifies the shared atlas.

## 7. Crate placement and the dependency pin

**Decision: text lives at `buiy_core::text` for v1 — a module, not a crate.**
The measure seam creates a genuine dependency **cycle** with layout: layout's
`taffy_compute` calls into text shaping (measure), and text's `TextCommit`
reads layout's `ResolvedLayout`. Within one crate that's two modules; across
crates it's an inversion no dependency direction fixes — a `buiy_text` crate
above `buiy_core` can't be called from layout, below it can't read
`ResolvedLayout`. Same non-inversion constraint the render spec pins for the
SC-trigger components (render architecture.md § 6); workspace precedent agrees
(layout, render, atlas all live in `buiy_core`).

> **Runner-up rejected:** a separate `buiy_text` workspace crate — attractive
> for compile-time isolation of the cosmic-text tree, but it loses on the
> cycle. Recorded escape: if a split ever happens, text and layout move
> **together**, or the measure callback becomes a dependency-injected trait
> object.

**Version-pinning policy** (the prior-art fast-moving-substrate lesson made
mechanical):

- `cosmic-text = "0.19"` declared **explicitly in `buiy_core`'s `Cargo.toml`**
  — never ridden transitively (riding bevy_text's transitive pin is the
  bevy_cosmic_edit decay path,
  [prior-art/bevy-cosmic-edit/lessons.md](../../prior-art/bevy-cosmic-edit/lessons.md)
  Avoid row). It is **not in `Cargo.lock` today**; `cargo deny check` runs
  before the dep lands (CLAUDE.md, Build & Test).
- **Default features kept** (`std`, `swash`, `fontconfig`, `sys-locale`).
  `shape-run-cache` stays **OFF** in v1 — **decided in review round 1; this
  section is the record** (font-assets § 1 and measure-and-layout § 3.2 were
  edited to match). The retained `TextBuffer` already provides the
  amortization that matters: per-line `shape_opt` survives width changes
  (`set_size` invalidates only `layout_opt`), so unchanged text never
  re-shapes — while the run cache grows `FontSystem`-side without bound,
  against gate #15. Turning it ON later is a one-line feature flip; revisit
  on measurement, not speculation. **Runner-up rejected: ON-with-trim** (the
  prior-art critiques row "most embedders should turn it on") — unmeasured
  memory complexity duplicating an amortization the retained-Buffer damage
  discipline already delivers.
- **Every version bump re-verifies the load-bearing API facts against docs.rs**
  (auto-traits, setter laziness, the `physical()` split) rather than trusting
  the prior-art folder — the 0.19 `Sync` flip (Open questions) is the live
  example. Buiy **stays on cosmic-text** even as Bevy 0.19 moves to Parley —
  the documented bet in
  [prior-art/cosmic-text/README.md](../../prior-art/cosmic-text/README.md);
  inherited, not relitigated.

## 8. Verification

Owned in full by [verification.md](verification.md); the architecture-level
obligations:

- **Headless (cosmic-text never instantiates an adapter):** the layout-chain
  order assertion grows `TextSync`/`TextCommit`; trigger-set tests per § 5.1
  row; `ComputedTextLayout` idempotency (steady frame → tick unchanged); the
  stale-uv retention test (§ 5.2); a steady-state zero-reshape assertion via a
  shape-call counter (the `LayoutTaffyComputeCount` precedent); the stub-atlas
  seam contract test
  ([seam § 7](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#7-verification)).
- **GPU lane (`#[ignore]`, on `crates/buiy_core/tests/support/mod.rs`):**
  `extract_buiy_glyphs` → atlas → glyph draw → readback golden for a one-word
  fixture, extending the verified spine. Gate #14 adds a text-heavy fixture
  watching the § 1.1 mutex; gate #15 rides the atlas typing-churn fixture
  alone — `SwashCache.image_cache` stays empty by construction (§ 1.3).

## Open questions

1. **Prior-art staleness — needs a correction pass.** The cosmic-text folder
   ([integration.md](../../prior-art/cosmic-text/integration.md) canonical
   shape, [lessons.md](../../prior-art/cosmic-text/lessons.md)
   FontSystem-singleton row) states `FontSystem` is non-`Sync` ("pin to UI
   thread or `Arc<Mutex>`"). **Falsified for 0.19** by docs.rs auto-traits
   (`impl Sync for FontSystem`); § 1.1 rests on the verified fact, not the
   folder. The folder needs a correction pass (worth-promoting finding per
   `using-prior-art`) or the next reader re-derives `NonSend` wrongly.
2. **Cross-layer glyph/quad interleaving.** `ExtractedGlyphs` is a flat global
   list drawn as one glyph batch after the quad batch
   ([`prepare.rs`](../../../crates/buiy_core/src/render/prepare.rs) ~:46–51:
   "in paint order … after the quad draw"). Correct per-layer
   shadow < quad < glyph interleave **across overlapping stacking layers** is
   buckets/`painters_z` work owned by the render spec — until it lands, text
   producing into the flat list can surface as a z-order artifact on layered
   fixtures. Not a contradiction with this file's placement decisions, but a
   sequencing dependency the plan must order against.

## Sources

- docs.rs/cosmic-text/0.19.0 — `FontSystem` (auto-traits `Send + Sync`,
  constructors, `db_mut`), `Buffer` (lazy setters, `shape_until_scroll`,
  `line_layout`, `layout_runs(&self)`, auto-traits), `SwashCache`
  (`get_image` / `get_image_uncached` signatures, auto-traits, public
  `image_cache`), `Metrics` (px fields),
  `LayoutGlyph::physical` / `PhysicalGlyph`, `Editor`/`BufferRef`. Fetched
  2026-06-09.
- docs.rs/crate/cosmic-text/0.19.0/features — default `{std, swash,
  fontconfig, sys-locale}`; optional `shape-run-cache`/`vi`/`no_std`.
- github.com/pop-os/cosmic-text CHANGELOG.md — 0.19.0 "Buffer setter methods
  are now lazy"; issue #505 (`FontSystem::new` startup cost).

---

*Scope: engine ownership (`SharedFontSystem`, `SwashCache`), `FontSystem`
lifecycle, the `TextBuffer`/`ComputedTextLayout` per-entity state, the
`TextSync`/`TextCommit` layout steps and the `extract_buiy_glyphs` producer, the
three trigger tables + `ExtractedGlyphs` retention, the scale-factor model, and
the `buiy_core::text` + `cosmic-text = "0.19"` pin. The measure body is
[measure-and-layout.md](measure-and-layout.md)'s; rasterization and `AtlasKey`
are [glyph-pipeline.md](glyph-pipeline.md)'s; font matching is
[font-assets.md](font-assets.md)'s; the editor surface is
[editing-and-ime.md](editing-and-ime.md)'s.*
