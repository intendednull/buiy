# Buiy performance audit

**Date:** 2026-06-25
**Trigger:** a deliberate first pass over Buiy with performance as the lens —
(1) where the per-frame costs are today, (2) how to *measure* performance so the
team can iterate against numbers instead of guesses, and (3) what prior art
(WebRender, Zed/GPUI, Flutter/Impeller, cosmic-text/glyphon, Vello/Linebender,
Rust perf methodology) teaches us, mapped onto our gaps.
**Scope:** the whole per-frame hot path on `main` — layout (Taffy bridge),
render (extract → batch → GPU: atlas/compositor/effects/pipeline), text
(shaping/measure/commit + editing/IME), ECS scheduling, a11y tree, picking/
scroll/focus — plus build/binary/wasm as a separate cost axis, plus the existing
measurement signals. **Analysis only; no code was modified.**
**Verdict:** The architecture is sound and the damage-gating discipline is real,
but **Buiy is measurement-blind** (zero `tracing` spans, zero `run_if` gates, no
frame-time/GPU/allocation instrumentation, one informational non-gating bench),
so almost every in-code performance claim is an *assertion*. The one number we
do have is alarming: **a static, warm-cache text screen of 128 nodes already
costs 10.9 ms/frame** — over the 120 Hz budget, on a scene that is not changing.
The headline recommendation is to **install measurement first** (ending in a
deterministic, non-flaky CI regression gate), fix the two costs we have already
measured, and only then chase the larger suspected cliffs.
**Status:** `[active]` — findings open; awaiting a go-ahead on the
measurement-infrastructure work (no implementation has started).

> **Every claim below is tagged MEASURED** (read directly from a benchmark, a
> `perf` profile, or a structural grep/graph fact) **or SUSPECTED** (reasoned
> from code reading). That distinction is the whole point: today the
> MEASURED set is tiny, and growing it is finding P0-#1.

## Method & confidence

Multi-agent audit from fresh cold context, plus one real measurement and an
adversarial re-check:

1. **19-agent fan-out** — 10 subsystem performance auditors (one per hot-path
   area, each reading the actual source and returning file:line findings tagged
   measured/suspected with scale behavior), 2 measurement agents (in-repo signal
   inventory + a Rust/Bevy/wgpu perf-tooling web survey), 7 prior-art web
   researchers (WebRender/Servo, Zed/GPUI, Flutter/Skia/Impeller,
   egui/bevy_ui/Taffy, cosmic-text/glyphon, Rust perf methodology,
   Vello/Xilem/Linebender). **108 raw findings**, deduped and ranked by
   (impact at scale) × (confidence) × (cost-to-fix).
2. **One real measurement** — the existing `crates/buiy_core/benches/pipeline.rs`
   criterion bench was compiled (optimized bench profile) and run on a single
   Linux host. This is the only independently-measured datapoint in the report
   (see §2) — and it changed two conclusions.
3. **Completeness critic** — a fresh agent re-examined the synthesis against the
   measured number, ran `perf` on the bench to root-cause it, and checked for
   missed subsystems and over-stated claims. Its corrections are folded into the
   body (it promoted finding #5 from suspected to *measured-dominant*, caught a
   gap in the Phase-1 gate, and corrected the "any animation" framing of #2).

Confidence is honest and uneven: the *measurement-blindness* thesis (P0-#1) is
MEASURED-as-absent and certain; the *atlas-touch* cost (#5) and the *dead extract
path* (#9) are now MEASURED; everything else is high-quality SUSPICION from code
reading and must be validated by the very instrumentation this report
recommends. **That is not a weakness of the audit — it is the finding.**

---

## 1. Executive summary

Buiy's render/layout/text architecture is fundamentally sound, and it has real
damage-gating discipline: the `extract`/`prepare` `changed.is_empty()` gate,
idempotent `ResolvedLayout` inserts, per-line cosmic-text shape caching, and
retained `RawBufferVec` instance buffers genuinely deliver a near-O(0) idle frame
**for a static, text-free, effect-free scene**. But that is the only regime it is
provably good at, and the team cannot see any other regime, because the codebase
is **measurement-blind**:

- **Zero `tracing` spans** in production (`info_span`/`debug_span`/`trace_span`
  grep = none; only comments match).
- **Zero `run_if` run conditions** (grep returns one comment in `render/golden.rs`).
- **No `FrameTimeDiagnostics`**, no allocation tracking, **no GPU timing**
  (`depth_stencil: None` and `timestamp_writes: None` on every render pass).
- **One informational bench** that never instantiates the RenderApp, caps at
  ~512 nodes, and reports the mean.

So every performance claim in the code — *"O(0) steady state"*, *"the walk visits
no entities"*, *"an accepted cost"* — is an assertion, not a measurement, and
several are provably wrong at the ECS-scan level.

**The one number we measured contradicts the rosy half of that story.** A static,
warm-cache, *text* screen of 128 nodes costs **10.9 ms/frame**, scaling ~linearly
to **45 ms** at 512 nodes (§2) — over the 120 Hz budget at 128 nodes and ~2.7× over
the *60 Hz* budget at 512, **while nothing is changing**. `perf` root-causes it:
~50–60 % is a single O(V·E) bookkeeping pass — the glyph-atlas LRU "touch" that
runs one linear `VecDeque` scan **per visible glyph** every frame (finding #5).
The "near-O(0) idle" contract holds for flat quads but **never holds for text**,
which is the most common screen there is.

Surrounding that are two larger *suspected* cliffs the bench does not even
exercise: **(a)** the extract→prepare pipeline is **all-or-nothing** — one
hovered button, one blinking caret, one scrolled list re-extracts and re-uploads
the *entire* scene's instance buffers (finding #2, flagged independently by six
auditors); and **(b)** **~12 unconditional full-tree O(N) passes per idle frame**
that no run condition gates (finding #3), which bite at high node count rather
than at 128 nodes.

The correct sequencing is **not** to chase any one hot path by guessing. It is to
**install measurement first** (frame-time + spans; the two-number build-vs-raster
split Flutter and Vello use), then build a **deterministic, non-flaky CI gate**
on instruction counts (iai-callgrind), allocation counts (dhat), and an extension
of Buiy's *existing* work-unit-counter pattern — and only then spend optimization
effort against numbers. Wall-clock cannot be the gate (it flakes on shared
runners; the team already decided this correctly in DG-3). GPU timing cannot be a
gate either (the CI GPU lane is lavapipe software Vulkan). The two costs already
measured (#5 atlas touch, #9 dead extract path) need **no** new infrastructure and
should be fixed now.

---

## 2. The one measured datapoint (and what it proves)

The repo's sole wall-clock signal, `crates/buiy_core/benches/pipeline.rs`, run on
the optimized bench profile, single Linux host:

| Scenario | Nodes | Per-frame cost | vs 60 Hz (16.7 ms) | vs 120 Hz (8.3 ms) |
|---|---|---|---|---|
| `steady/64 paragraphs` | 128 text nodes | **10.87 ms** [10.66, 11.09] | 0.65× | **1.31× — over** |
| `steady/256 paragraphs` | 512 text nodes | **45.38 ms** [44.76, 46.05] | **2.72× — over** | 5.5× |
| `cold/64` (4-frame bring-up) | 128 | 84.6 ms | — | — |
| `cold/256` (4-frame bring-up) | 512 | 317 ms | — | — |

**Three facts the numbers establish:**

1. **The steady cost is recurring per-frame work, not amortized warm-up.**
   Criterion's per-iteration timing is flat across all 100 steady samples; the
   `steady` bench settles the scene *first* (6 frames) and then times one
   `app.update()`. A fully incremental retained renderer should do ~0 work on an
   unchanged frame. 10.9 ms is not ~0. Scaling is ~linear at **~85 µs per
   text-node per frame**.

2. **It is the atlas-touch pass (#5), not re-shaping (#12) or node re-extract (#2).**
   `perf` self-time is dominated by `LruQueue::touch` + an AVX `memcmp` (30 % of
   total, the `self.order.iter().position(|k| *k == key)` linear scan in
   `lru.rs:22-24`) + a `VecDeque::remove` memmove + SipHash over per-glyph
   `AtlasKey`s. The steady path is `for key in &resident.keys {
   atlas.touch_existing(key) }` (`text/extract.rs:355`), where `resident.keys`
   holds **one entry per glyph instance** — so the pass is **O(visible glyphs ×
   distinct cells)** every frame. Re-shaping is *not* the cost: the existing test
   `steady_state_zero_measure_calls_and_zero_reshapes`
   (`tests/text/text_commit.rs:278`) proves `TextMeasureCallCount == 0` and
   `TextCommitReshapeCount == 0` on a steady frame.

3. **The bench is blind to the biggest suspected findings.** Its `ExtractSchedule`
   is only `maintain_atlas` + `extract_buiy_glyphs` (`pipeline.rs:100`); there is
   **no RenderApp**, no node extract/prepare/queue/draw, and no picking/a11y/
   animation plugins loaded. So findings #2 (all-or-nothing node re-extract), #4
   (a11y), #7 (picking), #8 (effects), #13 (overdraw) are **entirely unmeasured**.
   Finding #3's full-tree passes *are* partly in the bench but at 128 nodes are
   microseconds — invisible here; they belong to a *node-count* regime this
   text-heavy/low-node scene does not probe.

> **Why this matters for sequencing:** the audit's instinct — "measure before you
> optimize" — is right as a *meta*-thesis, but it over-applies to the two findings
> already measured. #5 (atlas touch) and #9 (dead extract path) are confirmed,
> root-caused, and each has a trivial fix that needs no new infrastructure. Fix
> those now; build infrastructure for the rest.

---

## 3. Measurement posture (current state)

| Axis | Exists today | Gap |
|---|---|---|
| Frame time (end-to-end) | None | No `FrameTimeDiagnostics`; bevy is pulled `default-features=false`, omitting `bevy_diagnostic`/`bevy_dev_tools`. Cannot answer "do we fit budget, and which BuiySet dominates?" |
| Per-phase attribution | None | Zero spans. A slow frame cannot be attributed to Layout vs Style vs extract vs prepare vs draw. |
| GPU timing | None | Every pass `timestamp_writes: None` (`node.rs:199/275/300/450`). The entire GPU half (upload, draws, atlas, effect compositor/blur) is unobservable. |
| Allocation | None | No counting allocator / dhat. The ~215 clone/collect/`Vec::new` sites (87 layout / 68 render / 60 text) are un-instrumented. |
| Wall-clock bench | `benches/pipeline.rs` | Informational only (no CI step), CPU-only (no RenderApp), ~512-node cap, mean-centric, single scene. **The only measured signal (§2).** |
| Deterministic counters | `TextMeasureCallCount`, `TextCommitReshapeCount`, `TextSyncAppliedCount`, `LayoutTaffyComputeCount`, `SyncStylesIterCount` + frame-count latency tests | **Test-only** (no CI reader), text+layout only, count *work units* not time/bytes, no history. **The single best pattern in the repo — and the template for the gates below.** |
| Percentile / worst-frame | None | Every signal is a frame COUNT or a criterion mean. Nothing captures p99/worst-frame — the metric GUIs are actually judged by. |

The good news: the **work-unit-counter discipline already proves Buiy can build
deterministic gates.** The gap is breadth (extend to render/picking/a11y/atlas),
unit (add time + bytes), and surfacing (make them CI-readable). The "O(0)
steady-state" framing is the one dangerous self-deception: it is true for avoided
*work* (no Taffy recompute, no GPU upload) but false for the per-frame *scans*
that run regardless — and §2 proves the atlas touch alone makes a static text
frame cost ~85 µs/node.

---

## 4. Prioritized findings (deduped across subsystems)

Tiered by confidence first, then impact. **Tier A** is measured/confirmed and
cheap — do it now. **Tier B/C** is the suspected work that the measurement
roadmap (§5) must validate and police before large fixes are committed.

### Tier A — measured/confirmed, cheap, no infrastructure needed

| # | Finding | Evidence | Fix |
|---|---|---|---|
| **9** | **Dead `extract_buiy_draws` runs full-tree every frame; its output is never consumed.** `render/mod.rs:284`, ungated in `ExtractSchedule`; every frame it walks all `Background` nodes, String-hashes a token each, allocates a fresh `ExtractedDraws` Vec, and inserts it — a duplicate of `extract_buiy_nodes`' work, thrown away. **MEASURED-as-dead** (the only non-`mod.rs` references to `ExtractedDraws` are comments; the live path reads `BuiyInstanceBuffers`). Also flagged in the 2026-06-18 spec-code-findings report. | grep + read | **Delete it.** Lowest-effort, zero-risk win in the report. |
| **5** | **Atlas LRU "touch" pass is O(visible glyphs × distinct cells) on every idle text frame.** `text/extract.rs:355` touches one entry *per glyph instance*; `LruQueue::touch` (`lru.rs:22-28`) does a linear `VecDeque` position scan + O(E) remove + 2 `AtlasKey` clones each. **MEASURED-DOMINANT: ~50–60 % of the 10.9 ms static-text frame (§2).** | `perf` profile | Make touch O(1): write `last_touched[key]=frame`, drop the reorder, derive LRU order lazily only under eviction pressure; dedup `resident.keys` to the distinct-cell set; key the LRU by an interned `u32` cell handle to also kill the per-instance 24-byte `AtlasKey` clones. |

### Tier B — high impact, mostly mechanical (validate with Phase 1, then sweep)

| # | Finding | Tag | Notes |
|---|---|---|---|
| **2** | **All-or-nothing extract→prepare rebuild.** The damage gate is binary (`extract.rs:941`): any one changed entity re-walks every `Node`, rebuilds the `by_entity` map, re-resolves every token, re-clones every ~480 B `ExtractedNode`, and `prepare.rs:260-344` clears + re-pushes every instance and `write_buffer`s the **full** active range (`buffer_vec.rs:181`). The keyed partial re-extract the data model anticipates (`extract.rs:1138` "R6/R8: merge cached records") was never built. | SUSPECTED (6-auditor consensus) | **The biggest interactive cliff** — but the *blast radius is smaller than "any animation" implies: Buiy has **no animation/tween system today** (`BuiySet::Animate` carries only a11y-scroll + text-input-a11y sync), and no scroll momentum. The real live triggers are **caret blink (#6), hover, typing, and wheel-scroll**. Implement the keyed partial re-extract + the existing-but-unused `write_buffer_range`. Gate the work behind the Phase-1 rebuild-rate counter (see §5). |
| **3** | **No run conditions → ~12 unconditional full-tree O(N) passes per idle frame.** `Node` `#[require]`s 5 components (`components.rs:42-57`) so every post-Taffy system matches every node: `stacking_context` (`systems.rs:4428`, 3 scans + 7-query fan-out + DFS + sort), `transform_composition` (`4369`, Mat4/node), `write_resolved_layout` (`3000`), `inherit_writing_mode` (`3337`, +HashMap/frame), sticky/table/multicol (`649/1283/1396`), `anchor_resolution` full-WORLD scan (`1813`), plus `clip.rs:232`, `effect.rs:131`, `bridge.rs:116`. | MEASURED-absent gate + SUSPECTED cost | A *node-count* regime (invisible at 128 nodes; ~150k+ entity-visits/frame at 10k). The good `write_paint_skip` seed-gate already exists — **port it**, treat `run_if` as the first-class gate. Mechanical. |
| **4** | **a11y rebuilds + re-serializes the whole semantic tree every frame, even with no screen reader attached.** `build_tree` (`a11y/mod.rs:445`) clears+rebuilds from a full-WORLD scan; `push_tree_updates` (`adapter.rs:74`) builds + per-adapter clones the `TreeUpdate` regardless, defeating accesskit's lazy `update_if_active`. O(E + N·D) in total world entities. | SUSPECTED, cheap | Gate `build_tree` on a Changed/removed probe; build the `TreeUpdate` *inside* `update_if_active`. Caveat: "no AT" must be "no **consumer**" — the in-process MCP driver also reads the snapshot. |
| **6** | **Caret blink trips a full-scene glyph re-extract + GPU re-upload ~2×/sec.** `Changed<CaretVisual>` (`extract.rs:250`) is in the damage union, so each blink flip trips the #2 wholesale rebuild for a one-instance change. | SUSPECTED, local fix | Move caret/selection stamps into their own small instance buffer. Subsumed by #2 but worth a targeted early fix. |
| **7** | **`emit_picks` brute-force O(N) hit-test + paint-order rebuild every frame, parked cursor included.** `backend.rs:48/98`, no run condition, reads persistent `PointerLocation` (not events), no spatial index; the same paint order is rebuilt 2–3× (picking + extract + each `hit_test`). | SUSPECTED | Add a run condition (skip when no pointer moved + no layout/stacking change); compute the global paint order **once/frame** into a shared resource; add a coarse spatial grid over the AABBs. |
| **11** | **Hot Entity-keyed maps use std SipHash, not `EntityHashMap`.** `by_entity`/`group_index`/`sc_by_entity` (extract), `GlyphMetaCache`/`FontKeyInterner`/atlas/LRU (text), all four a11y maps. **MEASURED-as-true** (0 `EntityHashMap`/`FxHashMap` in render/ or a11y/). Content addressing needs equality, not flood resistance. | MEASURED structure | Switch to bevy `EntityHashMap`/`EntityHashSet` (or `FxHash` for non-Entity keys). Best done with #2/#4 so the maps are also persisted+cleared, not reallocated/frame. |

### Tier C — real, but lower leverage or larger fix (price before committing)

| # | Finding | Tag | Notes |
|---|---|---|---|
| **8** | **Effect-group off-screen targets cleared, re-rastered, re-composited every frame regardless of damage.** `node.rs:149-226`; transient GPU buffers/bind groups per group per frame; ~2×groups+1 passes. Scales with *effect-group* count (opacity/blend/filter/isolation), not node count. | SUSPECTED | Hash each group's instance ranges + view + opacity; skip re-raster when unchanged (acquire-without-clear), re-run only the cheap composite; pool per-group UBOs. Needs Phase-4 GPU timing to price. |
| **10** | **Atlas re-uploads the entire 1 MiB page on a single new glyph.** `atlas/gpu.rs:113-134` `write_texture`s the full page though one cell blitted (`page.rs:122`). | SUSPECTED | Track a per-page dirty sub-rect and upload only that region (origin+extent already supported). |
| **12** | **Per-keystroke whole-buffer min+max-content relayout in editors.** `measure.rs:147` calls `cached_intrinsics` unconditionally → recomputes min-content (width-0 wrap at every break-opportunity, O(words)) + max-content + the definite-width pass = ~3–5 whole-buffer re-wraps/keystroke; plus double full-document `value()` materialization (`input.rs:281,307`). | SUSPECTED | ~O(1) for a single-line input; catastrophic for a 10k-line editor. Skip `cached_intrinsics` when width is definite + box isn't Min/MaxContent; cache per-line intrinsics. |
| **13** | **Full overdraw: every primitive alpha-blended back-to-front, no opaque/depth z-cull, no viewport cull.** **MEASURED architecture** (`depth_stencil: None` at `primitive.rs:398/469`, `composite.rs:147`; `BlendState::ALPHA_BLENDING` everywhere; only cull is `ComputedPaintSkip`). | MEASURED arch, SUSPECTED cost | A 10k-item scroll list with 50 visible still builds/uploads/draws 10k instances. Add the overdraw + draw-call counters (Phase 0) **before** the non-trivial depth-pass split; add a viewport-AABB reject in the paint-order walk (pairs with #2). |
| **14** | **Double transform propagation per frame.** `lib.rs:123-144` runs the propagate chain in Update **and** TransformPlugin re-runs it in PostUpdate — "an accepted cost" (`lib.rs:136`) with no number attached. | MEASURED-present, cost unmeasured | Absent from the 128-node profile (negligible at small scale). Run propagation once, or gate the PostUpdate pass. Convert "accepted" into "measured-and-accepted" once a span prices it at 10k. |
| **15** | **Build/binary/wasm.** No `[profile.release]`/wasm profile (only dev/test `debug=0`); `image` default features pull the **rav1e AV1 encoder** + rayon into the wasm production graph for PNG-only use; `arboard` unconditional = **hard wasm compile block**; no wasm CI lane. | MEASURED graph facts | Separate axis from per-frame runtime. Add `[profile.release]` (lto=thin, codegen-units=1, strip); `image = { default-features=false, features=["png"] }`; cfg-gate `arboard`/x11/wayland; add a `cargo check --target wasm32` tripwire. **Jumps to P0 the moment wasm is on the roadmap** — the build is uncompilable for wasm today. |
| **16** | **Fat `ExtractedNode` (~480 B, ~73 % rarely-populated) + per-frame scratch-allocation fan** across extract/prepare/picking/a11y; materialized twice per dirty frame (`extract.rs:1046`, `1144`). The ~215 clone/collect sites the audit flags. | SUSPECTED | **Do NOT grep-and-rewrite all 215 sites.** Stand up the dhat alloc gate (Phase 2) first, then SoA-split the cold outline/border/shadow channels out of the hot struct and hoist scratch containers into reused render-world resources. |

---

## 5. Measurement-infrastructure roadmap

The goal: go from measurement-blind to measurement-driven, ending in a
**deterministic, non-flaky CI regression gate**. Phases 1–3 are the gates; 0/4/5
are observability and trend. This is the part the request specifically asked for
("how we can start measuring … to iterate better").

- **Phase 0 — Observability foundation (no gate).** A `profiling` cargo feature
  that adds bevy `FrameTimeDiagnosticsPlugin` + `EntityCountDiagnosticsPlugin` +
  `LogDiagnosticsPlugin` (added *explicitly* — Buiy uses `MinimalPlugins`),
  `tracing::info_span!` per BuiySet **and** per RenderApp stage (extract/prepare/
  queue/draw), capturable via `bevy/trace_tracy` or `trace_chrome`, and a gallery
  frame-time overlay with the 8.33/16.66/33.33 ms budget lines (the Vello/Flutter
  pattern) reporting **p95/p99/worst-frame, not mean**. Splits the **two numbers**
  Flutter/Vello insist on: BuiySet (build) vs RenderApp (raster). Off by default;
  release/headless pays nothing. *This is the prerequisite that turns every
  SUSPECTED finding into a validated, prioritized one.*

- **Phase 1 — Deterministic work-unit count gates (first hard CI gate).** Extend
  Buiy's proven counter pattern to render (extracted-instance / batch / draw-call
  counts), layout (Taffy `compute_layout` count), picking (`emit_picks`
  nodes-tested), a11y (tree-rebuild count), an entity-count leak gate — **and an
  `AtlasTouchOps` / `resident.keys.len()` counter** (see the gap note below).
  Headless lane, zero new deps, near-zero runtime, host-independent integers.
  Polices #2/#3/#4/#6/#7/#9 and #5. **The cheapest, highest-leverage gate.**

  > ⚠️ **Gap the measured number exposed:** the natural Phase-1 assertion
  > "extract rebuild fired 0× on an idle frame" is **green on the exact 10.9 ms
  > frame** — `extract_buiy_glyphs` early-returns at the `!dirty` branch
  > (`extract.rs:348`) *before* the atlas-touch pass (`extract.rs:355`). The one
  > cost we measured would **pass** a gate meant to police idle frames. The
  > explicit atlas-touch-ops counter closes this; without it, the dominant
  > measured cost is invisible to both lead gates (#5 is CPU-bound and
  > **non-allocating** — `AtlasKey` is an inline `SmallVec<[u8;24]>` — so Phase 2
  > below cannot see it either; only Phase 3 catches it, and that is sequenced
  > last and Linux-only).

- **Phase 2 — Allocation-count gate (dhat-rs; second hard gate).** An isolated
  test target (dhat's allocator is global) that settles a large scene and asserts
  one steady `app.update()` performs ≤ a recorded baseline of allocations/bytes.
  Deterministic, cross-platform (no Valgrind). Catches the per-frame `Vec::new`
  growth (#16) invisible to both wall-clock and frame-count signals. *Blind to
  #5 (non-allocating) — that is what the Phase-1 counter is for.*

- **Phase 3 — Instruction-count gate (iai-callgrind; third hard gate).** A
  wall-clock-free, host-independent gate on the shape→layout→extract path,
  reusing the bench harness — the thing the criterion bench *cannot* be (it
  flakes on shared runners; DG-3 was right to keep it non-gating). Linux leg only
  (Valgrind; callgrind is 50–100× slower, so a small workload). Instruction count
  is a **proxy** (won't see parallelism/bandwidth/GPU) — pair it with the
  informational criterion trend, never treat as latency. Grow `pipeline.rs` into
  a 1k/10k/100k scale matrix with `many_buttons`-style knobs (force-relayout,
  recompute-text, respawn, display-none, glyph-count) as the wall-clock trend.

- **Phase 4 — GPU timing (informational, real-GPU host only; never a gate).**
  Wire bevy `RenderDiagnosticsPlugin` / wgpu `TIMESTAMP_QUERY` into the passes
  that already thread `timestamp_writes` (currently `None` at `node.rs:199/275/
  300/450`), behind the Phase-0 feature. GPU time is wall-clock-noisy **and** the
  CI GPU lane is lavapipe (software Vulkan, CPU-emulated, unrepresentative), so it
  is strictly a developer deep-dive. Validates/prioritizes #8/#10/#13.

- **Phase 5 — Worst-frame / percentile macro-bench trend (informational).**
  End-to-end build-half + raster-half frame-interval capture over a real example
  (gallery/todomvc) + deep-tree / many-small-nodes / scroll scenes, reporting
  average/p90/p99/worst per half, persisted per-commit as a trend artifact
  (Bencher-style sustained-regression detection, à la WebRender's Perfherder — not
  a single-run threshold). Matches DG-3 / WebRender / Flutter posture: wall-clock
  perf is a reviewable time-series, not a per-PR flake-prone threshold. This is
  also the *only* thing that exercises #2/#4/#7/#8/#13 end-to-end, which nothing
  measures today. Optionally promote to a managed gate via CodSpeed/Bencher
  simulated-CPU instruction counting if an external service dependency is OK.

---

## 6. Prior-art lessons (applied to our gaps)

| Lesson | Source | Buiy action |
|---|---|---|
| Re-encoding/re-uploading the whole scene per frame is "a substantial fraction of total time"; retain + tile/picture-cache + intern so only damaged entities rebuild. WebRender re-architected around this; Vello calls it its #1 finding; glyphon rebuilds its instance buffer every frame *because* it is not retained. | WebRender/Servo, Vello, glyphon | Backs **#2**: Buiy is retained but forfeits the advantage on any change. Build the keyed partial re-extract + `write_buffer_range` partial upload the data model already anticipates (`extract.rs:1138`). |
| Measure **two numbers** per frame (build vs raster) — bottleneck and fix live in different stages — and judge by the **tail (p90/p99/worst-frame)**, not the mean. Flutter rolls gate on 99th-pct rasterizer time. | Flutter/Skia/Impeller | Phase 0 must split BuiySet (build) vs RenderApp (raster) and capture p99/worst-frame, not FPS. |
| GPU-under-budget ≠ smooth: presentation cadence / triple-buffering is a separate axis the CPU bench can never catch. Institutionalize a cfg-gated, **importance-tiered**, baseline-diffed bench harness. | Zed/GPUI | Phase 5 end-to-end frame interval; tier the Phase 1–3 gates so only Critical/Important block a PR. |
| A parametric stress harness with knobs that isolate each subsystem is the template for attributing a regression; gate CI on deterministic **counters**, not wall-clock. Taffy's `mark_dirty` walks to root (unbounded invalidation); high-cardinality lists need an O(1) virtualization escape hatch. | egui, bevy_ui, Taffy | `bevy_ui`'s `many_buttons` = copy-paste template for the Phase-5 scale matrix; track Taffy #917 (contain:size scoping); **plan a virtualized-list primitive — none exists today, a latent cliff for any large data grid.** |
| Validate cache decisions with FPS + frame-time **variance** on a dedicated text stress example, A/B'd on one flag: the cosmic-text shape-run-cache raises mean fps but **worsens p99** and is unbounded without a frame-boundary trim. Every serious embedder forked the GPU text renderer (atlas/trim policy is app-specific). | cosmic-text/glyphon | Buiy keeps the shape-run-cache OFF; the one workload it helps is exactly Buiy's `measure.rs` triple-pass — don't flip it blind; A/B with variance + add a trim first. Use WebRender's record-and-replay method to choose the atlas allocator (#5/#10), not guessing. |
| The state-of-the-art regression gate that **does not flake** is instruction-count benchmarking (iai-callgrind/Valgrind): single-run, host-independent, stable on virtualized CI. Allocation counts are gateable via dhat. Two-tier: instruction = hard PR gate, wall-clock = informational trend. | Rust perf methodology | Directly shapes §5: Buiy's instinct to keep `pipeline.rs` non-gating (DG-3) is right, but it leaves **no** gate at all. Phases 1–3 are the fix — all in the headless Linux lane. |
| Optimize only what profiling flags ("the parts that would benefit aren't where the time goes"); build an in-app overlay with the budget lines drawn in; GPU measurement is genuinely hard. | Vello/Xilem/Linebender | **Do not** grep-rewrite the ~215 alloc sites (#16) blind — stand up Phase 0 spans + Phase 2 dhat first, then attack only the hot-path sites. Lift Vello's overlay-with-budget-lines into Phase 0. |

---

## 7. Recommended next steps (in order)

1. **Tier A now (no infrastructure):** delete `extract_buiy_draws` + `ExtractedDraws`
   (#9, verified dead), and make the atlas LRU `touch` O(1) + dedup `resident.keys`
   (#5, the measured-dominant cost of a static text screen). Together these should
   meaningfully cut the 10.9 ms static-text frame — and #5's fix is the one place
   the report can claim a *measured* before/after.
2. **Phase 0 observability** behind a `profiling` feature (frame-time + per-set/
   per-stage spans, gallery overlay with budget lines + p99, the build-vs-raster
   two-number split). Everything else depends on it.
3. **Phase 1 count gates**, authored *as the red→green gate for the fixes they
   police* — extend the counter pattern to render/picking/a11y **and include the
   `AtlasTouchOps` counter** so the gate can actually see #5. This is the cheapest
   high-leverage gate and closes the "headless-complete ≠ works" gap the
   widget-catalog campaign already hit.
4. **With #2/#3/#4 measured**, do the mechanical sweeps the data justifies: port
   `write_paint_skip` to clip/effect/stacking/transform-bridge/a11y (#3/#4) and
   switch hot Entity-keyed maps to `EntityHashMap` (#11).
5. **Then** the keyed partial re-extract (#2) — the largest single runtime win —
   against the Phase-1 rebuild-rate counter and the Phase-2 alloc gate.
6. **Defer** the depth-pass/overdraw work (#13) until Phase-4 GPU timing prices it
   on a real adapter; defer build/wasm (#15) unless wasm lands on the roadmap
   (then it is P0 — the build is uncompilable for wasm today).

## 8. Open questions (decisions that change the plan)

- **Target frame budget — 60 Hz/16.7 ms or 120 Hz/8.3 ms?** Several findings are
  comfortably within 60 Hz at moderate node counts but not at 120 Hz/10k nodes.
  The budget choice sets the bar every gate measures against. (This report assumed
  120 Hz.)
- **Does Buiy intend to ship to wasm in the near term?** #15 is P2 if wasm is
  aspirational, P0 the moment a web target is real — today the build does not
  compile for wasm (`arboard`).
- **What fraction of real gallery/todomvc frames actually hit the all-or-nothing
  rebuild (#2)?** The fix's value depends on the dirty-vs-idle ratio under real
  interaction; the Phase-1 rebuild-rate counter answers this *before* the
  substantial partial-re-extract work is committed.
- **Should the gates (Phases 1–3) be authored first as a standalone
  measurement-infrastructure campaign, or alongside the fixes they police?**
  First → fixes land red→green (TDD-style) and regressions are caught immediately;
  alongside → risks the same "headless-complete ≠ works" gap.
- **Is the NonSend single-thread layout spine (12.7 K LOC, all systems take
  `NonSend<LayoutTree>`) a structural ceiling worth re-architecting** — splitting
  the Send-able O(N) passes (content-vis classify, paint-order derivation,
  `ResolvedLayout` readback) off the `!Send` Taffy mutation so they parallelize?
  A design-spec question, only after Phase 0 prices the serial spine at 10k nodes.

---

*Produced by a 19-agent audit + prior-art workflow, one real benchmark run, and an
adversarial completeness critic (`perf`-validated). Raw per-subsystem findings
(108 total) are retained in the workflow transcript. This report is analysis only;
no code was modified.*
