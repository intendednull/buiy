# Buiy MT-ceiling — Prototype Dev Journal

> PROTOTYPE — exploratory, DO NOT MERGE the benchmark code. The deliverable is
> this journal + the findings report (retrospective).

**Question (from the user):** Is Buiy *limiting the performance ceiling* of a
multi-threading-heavy application — the kind Bevy handles well? The existing
MT-safety campaign (`docs/specs/2026-06-30-mt-safety-design.md`, PR #88) proved
Buiy is *correct* under Bevy's `multi_threaded` executor but explicitly
disclaimed perf. Nobody has measured whether Buiy's serial / main-thread-pinned
per-frame work caps a parallel app's throughput (an Amdahl's-law floor).

**Worktree:** `worktree-mt-ceiling-experiment`, off `origin/main` @ a969cbf.
**Target/reference:** none — this is a measurement experiment; the artifact is a
headless scaling benchmark.

## Hypotheses (pre-registered, before measuring)

- **H-A (Amdahl floor / app-vs-UI ceiling).** Buiy's per-frame pipeline is a
  strictly-ordered `.chain()` (Layout→Style→Input→Animate→Picking→A11y→Render)
  with `NonSend` (main-thread-pinned) layout + text and exclusive/`ApplyDeferred`
  barriers. For an app whose non-UI work is embarrassingly parallel, Buiy adds a
  ~thread-independent serial cost `S`. As core count → ∞, frame time → `S`.
  A big/active UI makes `S` large → Buiy caps the frame rate below what the
  parallel workload alone would allow.
- **H-B (UI-internal ceiling).** Buiy's own layout/text/extract is serial and
  main-thread-pinned; it does **no** `par_iter` internally. So a *complex UI's
  own* per-frame cost cannot use extra cores at all — `S` grows with UI size and
  is invariant to thread count.
- **H-C (overlap / barrier penalty).** Beyond additive `S`, Buiy's barriers
  (exclusive systems `route_action_requests`/`slider_keyboard`, pinned
  `ApplyDeferred`, auto sync points from command-writing systems) chop the Update
  schedule, *stalling* user parallel systems and degrading their scaling
  efficiency below the no-Buiy baseline.

## Instrument

Headless `app.update()` loop timing (no GPU/vsync) to isolate CPU scheduler cost.
Reuses `buiy_bench_support::PipelineHarness` (full headless Buiy per-frame
pipeline) + `build_flat_scene`/`build_large_scene`. Adds a configurable
embarrassingly-parallel user workload (`par_iter_mut` over N entities × W cost).
Compute-pool thread count set per-process (`ComputeTaskPool` is a process
singleton → matrix swept across processes by a driver script). MT vs ST executor
chosen at runtime (`set_executor_kind`) on a single `multi_threaded`-compiled
binary.

## Running log

### 2026-07-02 — Wave 0: setup
- Built: read the MT-safety spec + follow-ups + threading prior-art; mapped
  Buiy's serial surface (NonSend LayoutTree across ~15 systems, exclusive a11y
  systems, pinned ApplyDeferred in MVU/widgets, BuiySet `.chain()` in Update).
- Next: build the `mt_ceiling` benchmark binary + sweep driver.

### 2026-07-02 — Wave 1: benchmark built + first signals (16-core host)
- Built: `examples/mt_ceiling` headless harness (reuses `PipelineHarness` +
  `build_flat_scene`/`build_large_scene`), + `par_iter_mut` user workload, +
  per-process thread-count pin (`ComputeTaskPool::get_or_init`), + runtime
  ST/MT executor swap (`set_executor(SingleThreadedExecutor)`), + env knobs +
  per-frame work-counter readout + update/extract timing split.
- Ran the artifact → found:
  - **Parallel workload scales cleanly**: bare app 298ms→46ms (1→8 threads,
    ~6.5×). Valid "app Bevy does well with."
  - **H-B CONFIRMED — Buiy's serial floor is thread-invariant.** flat_large
    (2000 static nodes) ≈0.86ms@1t / 0.91ms@8t — *slightly slower* with more
    threads (MT dispatch tax on serial work). No amount of cores shrinks it.
  - **Surprise #1 (killed a wrong inference).** I guessed a 1-node change
    re-shapes all text. Buiy's own counters refuted it: a 1-node change =
    **2–4 measure calls + 1 reshape**, NOT 240. Verified, didn't ship the guess.
  - **Surprise #2 — the serial cost is EXTRACT, not update.** Localized via an
    update/extract timing split: text_large dirty = 14.6ms total, of which
    **extract = 13.8ms**; `app.update()` (layout+measure+reshape) is only 0.5ms.
    Static text_large = 2.6ms total (extract 2.1ms). So Buiy's *main-world*
    per-frame logic is cheap + incremental; the thread-invariant serial hotspot
    is **glyph/quad extract, which is largely non-incremental** — a single change
    re-emits ~all glyph instances (6.5× jump). Extract is a chained,
    single-threaded, main-world-blocking stage → a genuine serial floor even
    under Bevy pipelined rendering.
- If we did this again: read the work counters BEFORE forming the "re-shape all"
  hypothesis — the counters were right there.
- Next: full threads-sweep for the ceiling curves (bare vs buiy-static vs
  buiy-dirty across 1..16 threads, at moderate + heavy parallel load).

### 2026-07-02 — Wave 2: full sweep (81 rows) + follow-ups (24 rows)
- Ran the artifact → found (all in `docs/reports/2026-07-02-mt-performance-ceiling.md`):
  - **H-A CONFIRMED, quantified.** Moderate load @16 threads: bare 7.83 ms
    (128 fps) vs Buiy-active-text 22.51 ms (44 fps) — a **2.9× ceiling**;
    bare scales 7.78× but Buiy-active-text only 3.45×.
  - **Serial floor S is perfectly thread-invariant** (0.90→0.98 ms flat static
    across 1→16 threads; 14.03→14.64 text dirty). Slightly *rising* — the MT tax.
  - **Overlap model: `frame ≈ parallel/threads + S_extract` (ADDITIVE).** The
    overlap-isolation runs show Buiy's *update* systems overlap the parallel work
    for free (`+text ≈ bare`) — so H-C (barrier stalls) is negligible; the ceiling
    is entirely the additive serial **extract** phase.
  - **Floor scales linearly with total text**: text_small/large/huge dirty =
    3.7 / 14.2 / 56.6 ms; update-only stays 0.3 / 0.5 / 1.25 ms → the whole cost
    is wholesale glyph extract, and the update-only column is the incremental-ideal
    target (10–40× headroom).
  - **MT taxes Buiy's serial path** 1.15–1.22× while helping parallel work 0.91×.
- Verdict: Buiy DOES cap the ceiling, but narrowly (active text-heavy UIs) and for
  one fixable reason (non-incremental glyph extract, "§ 6.2 v1 wholesale").

### 2026-07-03 — Wave 3: the recommended fix EXECUTED (glyph partial re-extract)
- The REDESIGN item below was built as a staged production change on this same
  worktree (design `docs/specs/2026-07-03-glyph-partial-reextract-design.md`,
  plan `docs/plans/2026-07-03-glyph-partial-reextract.md`): Stage 0 `c9e60ed`
  (H6 `PreparedDamage` fix) → A `b875a07` (`emit_one_entity` factor) → B
  `e519379` (observation-only classifier + un-scoped probes) → C `a864d93`
  (the splice Patch — THE win) → D `0f0ebeb` (suffix ranged upload) → E
  (gallery live-run acceptance + docs flip).
- Headline (Stage C, this bench, `BUIY_MT_THREADS=8 PAR_COST=0`, p50):
  text_large dirty 20.1 → 3.97 ms (**5.1×**), text_huge dirty 73.3 → 13.5 ms
  (**5.4×**), statics parity; extract-only 17.1 → 2.54 ms (**6.7×**). Dirty
  floor ≈ 1.15× the static floor — the measured ceiling is removed for
  value-change frames.
- Two pre-existing bugs found + fixed on the way: the **ancestor z-reorder
  under-trigger** (the `With<TextBuffer>`-scoped union never rebuilt on a pure
  ancestor paint reorder → stale glyph order; fixed by the un-scoped
  `Changed<StackingContext>` probe, Stage B) and the **degraded-fold mirror
  residue** (a Patch would retain `fold_degraded_groups`' in-place-folded
  bytes; guarded by `glyph_mirror_folded`, Stage D).
- The bench needed its own fix: `dirty_scene` mutated the victim via a
  whole-`Style`-bundle re-insert, which ticks `Changed<Stacking>` (bundle
  inserts mark every member changed) and would have force-Full'd every frame
  under the new structural probes. It now mutates `BoxModel` width in place —
  pre-fix numbers unaffected (any dirty frame took the same wholesale walk).

## Prototype retrospective — for any follow-up

### Verdict
The question is answered with measured evidence. Buiy limits a MT-heavy app's
ceiling **only** when (a) the UI changes every frame AND (b) it carries
substantial text AND (c) the app's parallel workload is light enough that Buiy's
serial floor dominates. In that corner the cap is real and large (2.9× at 16
threads; ~18 fps hard cap for a very large active text UI). Everywhere else Buiy
is a sub-millisecond-to-few-millisecond non-issue.

### Validated — KEEP (findings to carry forward)
- Buiy's per-frame pipeline is a thread-invariant serial chain (measured).
- The serial cost is EXTRACT, not update; update is cheap + incremental.
- Glyph/quad extract is change-gated but **wholesale** on any change — the sole
  root cause of the active-text ceiling. Node extract is already incremental (#84).
- Buiy is a good scheduling citizen during Update (no barrier stalls that matter).
- MT executor is the wrong lever — it taxes Buiy's serial path.

### REDESIGN / follow-up (NOT built here — recommendation only)
- **Make glyph/quad extract incremental** (keyed partial re-extract, the #84
  node-tier pattern applied to glyphs). This is a *perf-campaign* item, not an MT
  item; it lifts the ceiling AND speeds single-threaded rendering. Bound: it would
  collapse the dirty extract cost toward the update-only cost (10–40× on the big
  scenes). DO NOT parallelize Buiy's systems to chase this.

### Residual gaps
- Harness is headless CPU-scheduler timing; it does not model GPU submission/pacing
  or non-Buiy extract. Faithful for the CPU-serial ceiling question (extract is a
  main-world-blocking sync point even with pipelined rendering), not for absolute
  frame budgets on a specific GPU.
- Not measured: FixedUpdate-heavy sims, real inter-system parallelism mixes, or a
  windowed run with a live GPU (would confirm the extract-is-serial claim end-to-end).

### Build strategy for a fix (if pursued)
- It's a buiy_core `render`/`text` extract change under the perf-campaign, TDD'd
  with the existing `RenderWorkCounters` + this bench as the before/after gate.
  This prototype's CODE is throwaway; the report + this journal are the deliverable.
