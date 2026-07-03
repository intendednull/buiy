# Buiy performance — final-pass plan

**Status:** `[landed]` — merged in PR #84
**Date:** 2026-06-26
**Realizes:** [performance final-pass design](../specs/2026-06-26-buiy-performance-final-design.md)
(which realizes the [audit](../reports/2026-06-25-performance-audit.md)).
**Branch:** `worktree-perf-final` off `origin/main` 7752c01. Merge-gated on human review.

Each phase lands red→green against the gate built in Phase 0. The loop runs
autonomously through these phases AND continues into sustained perf iteration
afterward, stopping only at the **architectural wall** (gains needing major,
human-attention changes).

## Standing regression-guard (run after EVERY change before committing)

```sh
# 1. Full headless gate (incl. gallery layout/behavior + the live-interaction tier)
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets --locked -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked && \
  cargo test --workspace --locked            # add `xvfb-run -a` only if a test needs X
# 2. GPU lane — BOTH legs (real adapter / lavapipe), additive
cargo test -p buiy_core   -j 2 -- --ignored --test-threads=1
cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1
# 3. RUN the gallery (the "headless-green ≠ works" guard)
CAPTURE_SHELL_SCREEN=scroll cargo run -p buiy_gallery --bin capture_shell   # eyeball the PNG
# 4. Perf signal (informational only)
cargo bench -p buiy_core --bench pipeline -- --baseline <prev>
```

The **complex perf workload** is the gallery S2 Virtual List
(`spawn_scroll_screen` + `fill_scroll_list(world, 1000)` ≈ 8k entities, reusable
headless). Note its `ContentVisibility::Auto` windows paint to ~11 rows — add a
paint-stressing variant (widen the viewport / unwindow) when measuring the paint path.
The gallery-scene bench lives in `examples/buiy_gallery/benches/` (NOT `buiy_core/benches`
— would cycle), sharing `bench_support`.

## Phase 0 — measurement infrastructure (lands FIRST)

| Sub | Deliverable | Gate |
|---|---|---|
| **0a** prerequisite | Extract the adapterless harness + 3 scene-builders out of `benches/pipeline.rs` into a shared `bench_support` (`bench-support` feature module in `buiy_core`) owning the single counter-registration list; repoint the criterion bench. Pure de-dup. | criterion bench builds+runs against `bench_support`; `cargo check --target wasm32` confirms it's dev-only / production graph unchanged. |
| **0b** counters | `RenderWorkCounters` (node_rebuilds, instances_built, atlas_touch_ops [glyph+icon], resident_keys, upload_instances/bytes) init'd in RenderApp AND `bench_support`; main-world a11y/pick/paint-order counters; headless gate tests asserting exact idle integers. | Headless counter tests green every PR. **Acceptance: `atlas_touch_ops==resident_keys` on an idle text frame (the #5 lock).** |
| **0c** dhat + iai | Isolated `tests/alloc_budget.rs` (dhat global allocator, band assert); `benches/pipeline_iai.rs` (iai-callgrind library_benchmark twins, setup-outside-timed, 1k/10k); Linux CI job (informational-first); scale matrix → 1k/10k/100k; documented weak-machine 60 Hz instruction budget. | dhat band green every PR; iai prints Ir/EstimatedCycles (informational); committed baselines + re-bless doc. Flip iai→`--regression=Ir` only after Phase 1's #11. |

## Phase 1 — mechanical wins (against the gates)

- **#9** delete dead `extract_buiy_draws` (verify-then-delete).
- **#11** `EntityHashMap` on the hot extract/prepare/a11y maps.
- **#5** regression lock (counters + iai `EstimatedCycles` on steady-text; no new opt code; cover the icon mirror).

**Gate:** node_rebuilds/upload counters + dhat show #9's per-frame `Vec` gone; iai Ir drops on
#11 and run-to-run jitter collapses (SipHash seed removed) — **THEN flip iai to gating** with a
tolerance band. Standing regression-guard green.

## Phase 2 — #3 layout-pass gating (output-identical by idempotence)

`LayoutDirtyThisFrame` seed + `BuiyLayoutStep::SeedLayoutDirty` + `configure_sets` `run_if`.
**Differential property test authored FIRST** (mutate each seed input incl. text-edit / resize /
despawn / reparent; assert gated == ungated). Increments: 1 = `PostTaffyOverrides` → 2 =
`inherit_writing_mode` → 3 = ground-truth-gate `write_resolved` (only if iai flags the residual
scan). Keep `write_resolved_layout` ungated in v1 (the self-heal proxy).

**Gate:** `LayoutPostTaffyRunCount==0` steady / `>=1` mutated; iai locks steady-frame Ir
near-constant in N (1k/5k/10k); differential test green; gallery RUN + GPU lane = no paint-order
regression.

## Phase 3 — #16 SoA `ExtractedNode` (cache enabler for #2)

SoA-split the cold outline/border/shadow/gradient channels out of the hot quad struct; hoist the
per-frame scratch `HashMap`s into reused render-world resources. **dhat-gated — only the sites the
gate flags.** `PackedInstance` stays 68 B. Sequence immediately before / co-with #2.

**Gate:** dhat blocks/bytes drop below a new band; iai `EstimatedCycles` improves on the hot
extract path; pixel goldens unchanged.

## Phase 4 — #2 keyed partial re-extract (the big redesign)

Steps: (1) retained `ResMut` `ExtractedNodesView`/`EffectGroups` + `NodeDamage` tag; (2)
`RetainedExtract.index_of` (`EntityHashMap`); (3) gated changed-only query + structural classifier;
(4) footprint signature + Patch branch (in-place patch, skip the order walk); (5) prepare
`quad_slots` + coalesced `write_buffer_range` + `debug_assert` + frame-epoch match; (6) GPU-lane
verification (hover=1 range, scroll=1 contiguous range, despawn/spawn/theme/group via Full, pixel
goldens unchanged); (7) docs flip. v1 = quad-buffer-only.

**Gate:** iai proves patch-instructions ≪ full-instructions at 1k/5k/20k × {idle, hover-retint,
scroll-subtree, spawn/despawn-Full}; `node_rebuilds==1` on one-change; R5 sibling-retention +
footprint-flip regression tests green; GPU-lane goldens clean; **record the measured Full/Patch/
retain mix on the real gallery + todomvc** to size the glyph-buffer mirror fast-follow.

## Phase 5 (trailing, optional) — observability

`profiling` cargo feature (OFF by default): bevy diagnostics + per-`BuiySet`/per-stage `info_span!`
→ build-vs-raster two-number split; gallery overlay with budget lines + p95/p99/worst-frame;
`trace_chrome` (wasm) / cfg-gated `trace_tracy` (native). Not a CI gate; verify OFF = no-ops and
`cargo check --target wasm32` green with the feature both off and on.

## Phase 6+ — sustained perf iteration (autonomous, until the architectural wall)

After Phase 5 looks good, keep finding + landing gains, each regression-guarded and gate-proven,
optimizing toward optimal (60 Hz is a floor; target weak machines via the iai instruction budget).
Candidate follow-ups surfaced by the gates: the #2 glyph-buffer mirror (subsumes #6 caret-blink) —
**LANDED 2026-07-03** via `docs/specs/2026-07-03-glyph-partial-reextract-design.md` (two-tier
Full/Patch splice + suffix upload; `mt_ceiling` dirty floors 20.1→3.97 ms / 73.3→13.5 ms),
the #5 `resident.keys` dedup, deeper #16 (ivec4-indices), the #82 band-pipeline ≤16-attr work,
overdraw/opaque-pass (#13, needs GPU timing), the double transform propagation (#14). **STOP and
surface to the human only when the remaining gain requires a major architectural change** (e.g. a
parallel/Send layout spine, a compute-rasterizer, a render-graph restructure) — that is the wall.
