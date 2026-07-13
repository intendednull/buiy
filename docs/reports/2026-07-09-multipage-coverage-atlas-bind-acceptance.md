# Multi-page coverage atlas bind — acceptance report

**Date:** 2026-07-09
**Branch:** `feat/dooduel-multiplayer-m1` (worktree `dooduel-app2`), unpushed.
**Spec:** [2026-07-09-multipage-coverage-atlas-bind-design.md](../specs/2026-07-09-multipage-coverage-atlas-bind-design.md)
· **Plan:** [2026-07-09-multipage-coverage-atlas-bind.md](../plans/2026-07-09-multipage-coverage-atlas-bind.md)
**Commits:** `90b1e44` (RED tests) · `9181f63` (fix) · `c592927` (retire warn) · plus this docs wave.

## What shipped

The Dooduel "empty chat pill" bug was a **framework** render defect: the coverage glyph
atlas is multi-page, but the GPU bound only page 0 and `coverage.wgsl` discarded the
per-instance `page`, so any glyph/icon spilling past one 1024² page sampled page-0 texels
at page-1 UVs → `alpha ≈ 0` → blank ink over a painted pill. A whole-screen glyph
working-set threshold (chat was just the highest-churn producer), permanent for the
session once tripped.

The fix binds **all resident coverage pages as a `texture_2d_array`** and samples the
per-instance layer via `textureSampleLevel(atlas, samp, uv, page, 0.0)` (explicit-LOD).
The array's layer count **grows to high-water** (recreate + re-upload-all on growth;
survives `collect_emptied_pages` shrink-then-regrow via `array_layers >= pages.len()`).
The coverage layout was **forked** from the shared `atlas_layout` (which stays
`texture_2d` for the raster/drawing-canvas pipeline — the blocker two spec reviewers
caught). `node.rs`, `raster.rs`, `raster.wgsl` untouched; no new vertex attribute.

Files: `render/pipeline.rs` (forked coverage layout + `BuiyPipeline::coverage_atlas_layout`),
`render/primitive.rs` (glyph specialize → coverage layout), `render/atlas/gpu.rs` (array
texture + per-layer upload + recreate), `render/coverage.wgsl` (array bind + flat `page`
varying + explicit-LOD sample), `text/extract.rs` (retire the dead v1 overflow warn).

## Verification — confirmed at every level

| Level | Result |
|---|---|
| **Headless unit guard** (Tier 3, device-free) | `overflow_glyph_instances_encode_their_atlas_page` — `instance.page == entry.page as u32` for overflow glyphs. PASS. |
| **GPU ink census** (`#[ignore]`, RX 6700 XT) | `coverage_pages_beyond_zero_render_their_own_texels` — real probe glyph + real icon forced onto page ≥1, probe-specific `AtlasEntry.page > 0` precondition, pinned-rect lit-pixel-count footprint vs a page-0 alone-render. **RED pre-fix** (448 vs 145 over-count) → **GREEN post-fix**. |
| **Recreate / re-upload-all** (`#[ignore]`) | `coverage_array_recreate_reuploads_clean_pages` — forces a 1→2 growth with a clean page 0 resident; guards that recreate re-uploads non-dirty pages. **RED pre-fix** (720 vs 240) → **GREEN**. |
| **Regression — existing goldens** | Full `buiy_core` GPU lane (render 47/47, text 17/17, text_edit 9/9) + `buiy_verify` 23/23 — **byte-identical, no re-bless** (`git status` clean after). Proves the page-0 path is unchanged. |
| **Regression — drawing canvas** | `render_raster_gpu` + `render_raster_interleave_gpu` GREEN — the layout fork left the raster/image path undisturbed. |
| **WebGL2 / SwiftShader** (D2 mitigation) | Built `gallery_web --features webgl2`, ran the enforced web-smoke on SwiftShader: coverage `texture_2d_array` → `sampler2DArray` explicit-LOD lowered with **0 GLSL-ES compile/link errors**, canvas painted, icon tiles render. The D2 blank-screen uniformity class is empirically refuted. (The web build has no page-budget hook, so page-≥1 overflow itself is proven natively by the census, not on WebGL2.) |
| **Native app smoke** | `cargo run -p dooduel --bin dooduel` on `DISPLAY=:0`: window renders, all text + roster icons crisp and fully visible, **no blank pills, no crash**. |
| **Mechanical gate** | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -D warnings`, headless `cargo test -p buiy_core --locked` — all clean. |

## Process notes

- Ran as a full `/staged-development` cycle: research (3-agent fleet confirmed the root
  cause + corrected the original note) → spec (two fresh reviewers **BLOCKED** the first
  draft on the shared raster-layout hazard → fork; four more corrections folded) → plan
  (a third reviewer caught executability fixes) → execution (implementer + a re-running
  review gate per wave). Prototype-first was considered and **declined** — research
  retired the one genuine unknown (WGSL/WebGL2), the approach had a single defensible
  choice, and the GPU census is the run-the-artifact gate inside one cycle.
- Nothing pushed/PR'd/merged — awaiting explicit go.

## Deferred (nothing blocking)

- `primitive.rs:768` stale "stride 68" comment (actual 84) — logged as a trivial
  follow-up; out of scope to preserve this change's boundary.
- A future polychrome `IconInstance`/`ColorRgba8` path (unbuilt) would need its own
  multi-page bind; not provided here.
