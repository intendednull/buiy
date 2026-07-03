# Does Buiy cap a multi-threaded app's performance ceiling? — investigation

Date: 2026-07-02
Status: report (one-shot investigation)
Prototype: `worktree-mt-ceiling-experiment` (off `origin/main` @ a969cbf) — the
`examples/mt_ceiling` benchmark + `docs/prototypes/2026-07-02-mt-ceiling-journal.md`.
Companion to: `docs/specs/2026-06-30-mt-safety-design.md` (the MT *correctness*
campaign, PR #88), whose explicit non-goal — "making Buiy faster via threads" —
this report picks up on the *ceiling* side.

## Question

The MT-safety campaign proved Buiy is **correct** under Bevy's `multi_threaded`
executor (which downstream apps enable via feature unification) but disclaimed
perf. The open question, from the user: **is Buiy limiting the performance
ceiling of a multi-threading-heavy application** — the kind Bevy handles well —
especially when a complex UI runs alongside a parallel non-UI workload (a game
sim, procedural generation, physics)? Could Buiy's serial / main-thread-pinned
per-frame work become an Amdahl's-law floor the app can't beat no matter how many
cores it has?

## TL;DR

**Yes, but narrowly and for a specific, fixable reason.**

- Buiy's per-frame work is a strictly-ordered serial chain that is **thread-invariant**
  — more cores never shrink it. That is an Amdahl floor by construction.
- For **static or lightly-changing UIs the floor is tiny** (~1 ms for 2000 nodes)
  and Buiy is a non-issue next to any real parallel workload.
- For an **actively-changing, text-heavy UI the floor jumps to ~14 ms** and
  becomes a real ceiling: on a 16-core host a moderate parallel workload that
  *alone* runs at 128 fps is dragged to **44 fps** — **2.9× slower** — purely by
  Buiy's serial cost.
- The floor is **not** where you'd guess. Buiy's main-world logic (layout,
  measure, reshape) is cheap and well-cached/incremental (~0.5 ms even when
  dirty). **~95 % of the serial cost is the `extract` stage**, and specifically
  **glyph/quad extraction, which is change-*gated* but non-*incremental*: any
  single change rebuilds the entire glyph set** (the code says so:
  `text/extract.rs` — "Rebuild (wholesale, § 6.2 v1)"). Node extraction already
  got a keyed-incremental path in PR #84; glyphs did not.
- So the ceiling is a **known, bounded, fixable perf gap** (make glyph extract
  incremental, mirroring the node path), **not** a fundamental limit of Buiy's
  architecture, and **not** a reason to reach for threading Buiy's own systems.

## Method

Headless `app.update()` loop timing (no GPU / no vsync) isolates the CPU
scheduler cost — the only surface an Amdahl ceiling can live on. The benchmark
(`examples/mt_ceiling`, throwaway) reuses `buiy_bench_support::PipelineHarness`
(the full headless Buiy per-frame pipeline: layout → text → render-extract) plus
`build_flat_scene` / `build_large_scene`, and adds:

- an **embarrassingly-parallel user workload** — `par_iter_mut` over N=4000
  entities × W busy iterations (the "app Bevy does well with"); scales cleanly
  with the compute pool.
- **per-process compute-thread pinning** (`ComputeTaskPool::get_or_init` before
  any plugin builds it — the pool is a process singleton, so the thread-count
  matrix is swept across separate processes).
- **runtime ST/MT executor swap** (`set_executor(SingleThreadedExecutor)`) on one
  `multi_threaded`-compiled binary.
- an **update-vs-extract timing split** and a **per-frame work-counter readout**
  (`TextMeasureCallCount`, `TextCommitReshapeCount`, `LayoutTaffyComputeCount`, …)
  to attribute the cost.

Host: 16 physical-thread Linux box (`nproc`=16). Scenes: `flat_large` = 2000
plain nodes; `text_large` = 120 paragraphs (240 text nodes, multi-thousand-glyph
shaping). "DIRTY" = mutate one node's style/text every frame (models an active
UI). Load regimes: **moderate** (`cost`=600 → bare 61 ms→8 ms across 1–16
threads) and **heavy** (`cost`=3000 → bare ~300 ms, dwarfs Buiy).

Caveat honestly stated: in a real windowed app Bevy's `PipelinedRenderingPlugin`
runs prepare/queue/render on a separate thread — but **`extract` is still a
main-world-blocking sync point** (the render thread borrows the main world; the
main app cannot proceed during extract). So extract remains on the serial
critical path, and the harness's synchronous extract is faithful to that. What
the harness does *not* model: real GPU submission/pacing, and non-Buiy extract
work. It measures Buiy's CPU serial contribution, which is the thing in question.

## Findings

### F1 — Buiy's serial floor is thread-invariant (the Amdahl floor is real)

The whole Buiy per-frame pipeline is a `.chain()` in `Update`
(Layout→Style→Input→Animate→Picking→A11y→Render), with `NonSend`
(main-thread-pinned) layout + text, exclusive a11y systems, and pinned
`ApplyDeferred` sync points. It does **no** `par_iter` internally. So its cost
cannot shrink with cores — measured directly, a 2000-node static UI costs the
same ~0.9 ms at 1 thread and at 16 (in fact marginally *more* at 16 — the MT
executor's per-frame task-dispatch tax on serial work; see F5).

### F2 — The ceiling curve (the headline)

Frame time p50 (ms) vs compute threads, **moderate parallel load** (final):

| config | t=1 | t=2 | t=4 | t=8 | t=16 | speedup(16) |
|---|---|---|---|---|---|---|
| bare (no Buiy) | 60.96 | 32.14 | 17.04 | 9.66 | **7.83** | **7.78×** |
| Buiy flat static | 63.39 | 33.31 | 17.91 | 10.72 | 9.03 | 7.02× |
| Buiy flat DIRTY | 65.36 | 36.70 | 20.49 | 12.34 | 10.88 | 6.01× |
| Buiy text static | 64.00 | 37.01 | 20.25 | 11.84 | 10.86 | 5.89× |
| Buiy text DIRTY | 77.68 | 47.24 | 33.32 | 24.37 | **22.51** | **3.45×** |

Read the last row against the first: the parallel workload *alone* reaches
7.83 ms (128 fps) on 16 threads, but with an **active text UI present it flattens
at 22.5 ms (44 fps)** and only scales 3.45×. Buiy's ~14 ms serial extract is the
ceiling. The combined time is **additive** — `frame ≈ (parallel work / threads) +
(Buiy serial extract, thread-invariant)` — because extract is a serial phase the
parallel work cannot overlap. As cores → ∞ the parallel term → 0 and the frame
converges on Buiy's serial floor.

Under **heavy** parallel load the same floor is a smaller *relative* penalty —
the parallel term dominates, so Buiy matters less (this host is 8 cores / 16
threads, so past t=8 hyperthreading gives little and bare bottoms out ~34 ms):

| config | t=1 | t=4 | t=8 | t=16 |
|---|---|---|---|---|
| bare (no Buiy) | 298.0 | 79.9 | 39.8 | 33.9 |
| Buiy text static | 302.1 | 86.7 | 47.8 | 35.6 |
| Buiy text DIRTY | 309.5 | 99.1 | 57.9 | 46.7 |

The rule is Amdahl's: **the ceiling bites hardest when the parallel workload is
light relative to Buiy's serial floor.** Heavy-compute app → Buiy is ~13 ms of
noise on a 34 ms frame (1.38×). Light-compute app with an active text UI → Buiy
*is* the frame (2.9×).

### F3 — The floor is EXTRACT, and it is non-incremental (the root cause)

Update-vs-extract split, `text_large` @ 8 threads, no user work:

| | total p50 | of which extract | `app.update()` |
|---|---|---|---|
| static (no change) | 2.45 ms | 2.14 ms | 0.31 ms |
| DIRTY (1 node/frame) | 14.16 ms | **13.66 ms** | 0.50 ms |

Buiy's **main-world logic is cheap and incremental** — a one-node change triggers
only **2–4 text measure calls and 1 reshape** (verified via Buiy's own counters,
which refuted an initial "re-shapes everything" guess), and `app.update()` stays
~0.5 ms. The cost is **`extract_buiy_glyphs`**, which is change-*gated*
(steady-state O(0), returns without touching the buffer) but **non-*incremental***:
on any dirty frame it does `glyphs.clear()` + a **wholesale rebuild of the entire
glyph/quad set** (`crates/buiy_core/src/text/extract.rs` — "Rebuild (wholesale,
§ 6.2 v1)"). One changed node ⇒ re-emit all ~thousands of glyph instances. That
is the 2.45 ms → 14.16 ms jump, and it scales with **total** on-screen text, not
with what changed. Node extraction got a keyed partial re-extract in PR #84
(audit #2); the glyph tier did not — it is still the "v1" wholesale path.

The dirty cost scales **linearly with total on-screen text** (t=8, thread-invariant),
and it is *all* extract — the main-world update stays ~1 ms even at 960 text nodes:

| scene (text nodes) | static total | DIRTY total | DIRTY update-only | ⇒ extract |
|---|---|---|---|---|
| text_small (60) | 0.83 | 3.72 | 0.31 | 3.4 ms |
| text_large (240) | 2.45 | 14.16 | 0.50 | 13.7 ms |
| text_huge (960) | 9.61 | 56.55 | 1.25 | **55.3 ms** |

An active `text_huge` UI floors at ~56 ms (**~18 fps**) on any number of cores.
The "update-only" column is the **incremental-ideal target**: making glyph
extract incremental would collapse the DIRTY cost toward it — a **10–40×**
reduction — and remove the ceiling.

### F4 — Barrier-induced stalls (H-C) are second-order

Buiy's `Update` schedule has exclusive `&mut World` systems (`route_action_requests`,
`slider_keyboard` in `BuiySet::Input`) and pinned `ApplyDeferred` sync points
(MVU / widgets) — each a point where the MT executor drops to parallelism-1 and
can stall user parallel systems. Empirically this is **small** in steady state:
the exclusive systems early-return when idle, and the overlap analysis shows the
combined frame is ≈ additive (parallel-phase + serial-extract) with only a
~0.3 ms overlap penalty. The dominant ceiling is the additive serial extract
(F3), not barrier stalls.

Isolating the **update phase alone** (extract skipped) makes this explicit —
Buiy's text update systems overlap the parallel workload essentially for free:

| threads | bare | + Buiy text update-phase | + Buiy flat update-phase |
|---|---|---|---|
| 8 | 9.52 | 9.95 | 11.60 |
| 16 | 8.05 | 8.53 | 9.39 |

`+text ≈ bare` — no measurable stall. (The flat scene adds ~1 ms of transform
propagation over 2000 nodes, still largely overlapped.) So the combined frame
obeys `measured ≈ (parallel work / threads) + S_extract`: **the serial extract is
purely additive**, which is exactly why it becomes the ceiling as cores rise.

### F5 — Threading Buiy is the wrong lever (and MT even taxes it slightly)

ST vs MT executor, p50 (ms):

| workload | threads | ST | MT | MT/ST |
|---|---|---|---|---|
| Buiy flat floor (serial) | 1 | 0.76 | 0.92 | **1.22×** |
| Buiy flat floor (serial) | 8 | 0.86 | 0.99 | 1.15× |
| bare heavy parallel | 8 | 43.26 | 39.32 | 0.91× |

The MT executor is *slower* on Buiy's own serial work (per-frame task-dispatch
overhead with nothing to parallelize), consistent with the MT-safety spec's
"10–45 % slower" note. Buiy is single-threaded by default (correct for wasm,
determinism, and its latency-bound per-frame work); the ceiling is not addressed
by parallelizing Buiy's systems but by **not rebuilding wholesale**.

## Recommendation

1. **The one fix that lifts the ceiling: make glyph/quad extract incremental**
   (keyed partial re-extract, the PR #84 node-tier pattern applied to the glyph
   tier). This turns the ~14 ms active-text floor toward the ~2 ms static floor —
   i.e. it removes the ceiling for the exact workload where it bites. This is a
   **perf-campaign item**, not an MT item; it belongs with
   `docs/.../perf` work, and it helps single-threaded rendering just as much.
2. **Do NOT parallelize Buiy's per-frame systems** to chase this. The serial cost
   is wasted *rebuild* work, not un-parallelized *necessary* work; the MT executor
   already taxes Buiy's serial path. Threading is the wrong tool and carries the
   wasm/determinism costs the project deliberately avoids.
3. **For app authors today**: keep large text subtrees static per frame where
   possible; localize animation to non-text or small-text regions. A static
   UI has a sub-millisecond floor and does not cap anything.
4. **No change to the MT-safety posture.** Buiy stays correct under MT and
   single-threaded by default; this is purely about the extract hotspot.

## Reproduce

```sh
# --features mt-exec is REQUIRED (compiles Bevy's MT executor in; main() aborts
# otherwise). It is off-by-default so the bench can't unify multi_threaded across
# the workspace gate.
cargo build -p mt_ceiling --release --features mt-exec
bash examples/mt_ceiling/run_sweep.sh out.csv
bash examples/mt_ceiling/run_followups.sh fu.csv
python3 examples/mt_ceiling/analyze.py out.csv
```
Knobs are env vars (see `examples/mt_ceiling/src/main.rs` header). Raw data from
this run is archived under `examples/mt_ceiling/results/`.

## Addendum (2026-07-03) — the fix landed

Recommendation 1 is implemented: the glyph tier now does keyed partial re-extract
(two-tier Full/Patch), landed as stages 0/A–E of the
[glyph partial re-extract design](../specs/2026-07-03-glyph-partial-reextract-design.md)
([plan](../plans/2026-07-03-glyph-partial-reextract.md)) — commits `c9e60ed`
(Stage 0, the H6 `PreparedDamage` fix), `b875a07` (A, `emit_one_entity` factor),
`e519379` (B, classifier + probes), `a864d93` (C, the splice Patch), `0f0ebeb`
(D, suffix ranged upload), plus Stage E acceptance (gallery live-run + interaction
tier + this docs flip).

Stage-C measured before/after on this report's own bench (same host, interleaved,
`BUIY_MT_THREADS=8 PAR_COST=0 WARMUP=20 FRAMES=100`; p50 — note the protocol
differs from § F3's isolation runs, so the "before" columns differ from F3's
floors):

| scene | dirty before | dirty after | speedup | static before → after |
|---|---|---|---|---|
| text_large (240) | 20.1 ms | 3.97 ms | **5.1×** | 3.28 → 3.42 ms (parity) |
| text_huge (960) | 73.3 ms | 13.5 ms | **5.4×** | 12.3 → 11.9 ms (parity) |

Extract-only p50 (text_large dirty): 17.1 ms → 2.54 ms (**6.7×**). The dirty
floor now sits ~1.15× the static floor on both scenes — the ceiling this report
measured is removed for the value-change workload; structural/global changes
still take the Full walk by design (the any-uncertainty→Full rule).

**Final acceptance re-measurement** (idle host, the original sweep's exact
parameters — WARMUP=32, FRAMES=150, t=8, no user work; p50 vs the F3 baselines):

| scene (text nodes) | static before → after | DIRTY before → after | dirty/static after |
|---|---|---|---|
| text_small (60) | 0.83 → 0.81 ms | 3.72 → **0.87 ms** (4.3×) | 1.08× |
| text_large (240) | 2.45 → 2.44 ms | 14.16 → **2.68 ms** (5.3×) | 1.10× |
| text_huge (960) | 9.61 → 9.39 ms | 56.55 → **10.06 ms** (5.6×) | 1.07× |

And the headline ceiling itself (F2's moderate-load row, active text UI):

| | bare | Buiy text DIRTY | ceiling factor |
|---|---|---|---|
| t=16 before | 7.83 ms (128 fps) | 22.51 ms (44 fps) | **2.9×** |
| t=16 after | 7.64 ms (131 fps) | **10.40 ms (96 fps)** | **1.36×** |
| t=8 after | 9.91 ms | 12.35 ms | 1.25× |

The 2.9× multi-threaded-app ceiling this report measured is reduced to 1.36×;
the residue is the pre-existing *static* extract floor (the O(resident-keys)
touch pass + publish value-compare), not the wholesale walk.

Two pre-existing bugs were found and fixed on the way: (1) a pure **ancestor
z-reorder** over overlapping text entities never rebuilt the glyph carrier —
stale paint order on screen; the § 6.2 union was `With<TextBuffer>`-scoped and
blind to it; fixed by the un-scoped `Changed<StackingContext>` order probe
(Stage B, RED-reproduced against pre-change code). (2) The **degraded-fold
mirror residue**: `fold_degraded_groups` mutates the glyph CPU mirror in place,
so a later Patch would have retained folded (dimmed) bytes; guarded by the new
`glyph_mirror_folded` flag forcing the full upload path until repaired (Stage D).
The fold's own cross-frame stale-dim when a group drops on a glyph-clean frame
remains a filed follow-up (degradation-only; `docs/plans/follow-ups.md`).

The bench itself gained a fix: `dirty_scene` mutated its victim via a
whole-`Style`-bundle re-insert, which ticks `Changed<Stacking>` (a bundle insert
marks every member changed) and would have force-Full'd every frame under the new
structural probes — a bench artifact, not an active-UI model. It now mutates
`BoxModel` width in place. Pre-fix numbers are unaffected (any dirty frame took
the same wholesale walk regardless of which components ticked).
