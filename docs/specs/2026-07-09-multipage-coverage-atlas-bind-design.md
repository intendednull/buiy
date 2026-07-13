# Multi-page coverage atlas bind — design

**Status:** draft
**Date:** 2026-07-09
**Area:** render / text (`crates/buiy_core/src/render/`)
**Closes:** glyph-pipeline § 11.1 item 1 ("Single page-0 bind"); the `follow-ups.md`
"Dooduel — chat rows render as empty pills" entry.
**Related:** [text-rendering design](2026-06-09-buiy-text-rendering-design/glyph-pipeline.md),
[render-pipeline design](2026-06-03-buiy-render-pipeline-design/README.md),
[render-atlas-glyph-gpu design](2026-06-03-buiy-render-pipeline-design/2026-06-08-render-atlas-glyph-gpu-design.md),
[wasm browser support](2026-06-25-buiy-wasm-browser-support-design.md) § D2 (WGSL uniformity).

**Revision note (2026-07-09, post spec-gate):** two fresh-context reviewers both
BLOCKED the first draft on one issue — the coverage `@group(1)` layout is **shared
with the raster (drawing-canvas) pipeline**, so it must be *forked*, not mutated
(§ 3.3). This revision also corrects the page-count-monotonicity reasoning (§ 3.2),
the `IconInstance` attribution (§ 1), and strengthens verification against a pre-fix
false-pass (§ 5). The core approach (`texture_2d_array` + grow-to-high-water) was
validated and is unchanged.

---

## 1. Problem

The coverage glyph atlas (`BuiyAtlas`, `AtlasFormat::CoverageR8`) is paged
(`page_size = 1024`, `page_budget = 8`). Glyphs, the drawn vector icons, and
caret / strikethrough solid-stamps are all packed into pages on demand and emitted
as **`GlyphAlphaInstance`** records; each carries its resident page in
`GlyphAlphaInstance.page` (vertex attribute `@location(6)`, `Uint32`, offset 64,
stride 84).

**But the GPU never binds any page but page 0, and the shader discards the page
index.** All five coverage draw sites in `render/node.rs` (162→243, 274→356,
600→606, 625→629, 1051→1085) bind a single `AtlasGpu::coverage_bind_group()` built
against `coverage_pages[0].view` (`atlas/gpu.rs:200-204`), and `coverage.wgsl` does
`_ = i.page` (line 80) then samples `textureSample(atlas, atlas_samp, in.atlas_uv)`
(line 97) — i.e. page 0 at whatever UV the instance carries. A glyph resident on
page ≥1 has a UV normalized to *its* page's texel grid; sampled against page 0 that
UV lands on unrelated/empty texels, so `coverage ≈ 0` → `alpha ≈ 0` → **blank ink**.
The background quad (a separate quad-tier draw) still paints, so the symptom is an
**empty colored pill**.

This is a **framework** bug, not app code. It affects any text-heavy, long-running
Buiy screen. It first surfaced in Dooduel's in-game chat because chat is the
highest-churn text producer: the atlas is content-addressed on
`(FontId, px_size, subpixel_x_bin, glyph_id)`, so residency grows with *distinct*
cells across the **whole screen** (names + scores + word/timer chrome + buttons +
chat, across sizes/families/4 subpixel bins). Chat's constantly-fresh glyphs are
the first to spill onto page 1, so the *newest* chat rows go blank while older,
page-0-resident rows stay fine. With `page_budget = 8` and `eviction_grace = 60`
the atlas appends up to 8 pages before it evicts anything, so once the working set
crosses one page the blanking is **permanent for the session** ("goes blank and
stays blank").

### What is *not* wrong (corrected from the original follow-up note)
- `atlas/gpu.rs` uploads **every** page's texture correctly (including page ≥1).
  The page-1 texels are fully resident on the GPU — they are simply never bound.
  **The defect is isolated to the bind group + shader.** The fix touches no
  upload-content or allocation logic (it does change *how* the pages are packaged
  into one array texture — see § 3.3).
- The `page` field already flows end-to-end: allocation
  (`atlas/atlas.rs::entry_from` stamps `AtlasEntry.page`) → emission
  (`text/extract.rs`, `icon_producer.rs` copy it into the instance) → vertex
  attribute. Nothing new needs to be plumbed on the CPU side; **no new vertex
  attribute is added** (so the WebGL2 16-attribute cap is untouched).

### Blast radius: five draw sites, one instance type, three emission kinds
The single `coverage_bind_group()` is shared by **five** draw sites — flat glyph
(`node.rs:606`), flat **icon** (`629`), effect-group step-1 glyph (`243`),
effect-group degraded-inject glyph (`356`), and backdrop-filter fill (`1085`) —
and by three emission kinds: glyphs (`emit_glyph`, `extract.rs:1848`), **the drawn
icons**, and caret/strikethrough solid-stamps (`extract.rs:1560/1646/1664`).

- **The drawn icons carry the identical bug.** Buiy's SVG icons are rasterized to
  R8 coverage and emitted by `icon_producer.rs:350` as **`GlyphAlphaInstance`**
  records into `AtlasFormat::CoverageR8`, drawn through the same coverage pipeline
  + `coverage_bind_group()`. They spill to page ≥1 and blank exactly like glyphs;
  the single shader + bind fix covers them.
- **`IconInstance` / `ColorRgba8` is *not* in scope and *not* the drawn-icon path.**
  `IconInstance` is a distinct polychrome struct (`primitive.rs:99-109`,
  `ColorRgba8`, no `page`-array bind group) that is a **named-but-unbuilt C-tier
  seam** — never emitted, never drawn. If a future color-icon path is built it will
  need its own multi-page bind; this spec does not cover it and does not claim to.
- **Caret/strikethrough stamps** share one warmup-pinned `solid_stamp_key()`
  singleton kept LRU-warm by the un-gated touch pass, so it lands on page 0 and
  stays there — low-risk, and it renders correctly even if it ever overflowed.
- **Effect-group `Glyph@Rgba16Float`** (the group-target variant, `compositor.rs`)
  specializes through the same `BuiyPrimitives` specializer + coverage shader +
  atlas layout, so the shader/layout fix reaches it automatically.

A correct fix must render every page for all of these — which the chosen approach
does with one shader change + one *new* bind-group/layout, and **zero** `node.rs`
edits (every site binds the opaque bind group; only its contents change).
(Note: `node.rs:573` is a *raster* `@group(1)` bind — not coverage — and is left
alone; see § 3.3.)

---

## 2. Goal / non-goals

**Goal.** Any coverage instance resident on page N is sampled from page N, on both
render backends (native wgpu + WebGL2), with no per-draw-site changes and no new
vertex attribute. Existing single-page output stays byte-identical. **The raster
(drawing-canvas) pipeline is unaffected.**

**Non-goals.**
- No change to allocation, eviction, pooling, or the page budget.
- No new atlas format or a second atlas; no `IconInstance`/`ColorRgba8` work.
- No attempt to *prevent* overflow (that is the rejected app-side stopgap).
- No mip-mapping (coverage is nearest-sampled, single mip).

---

## 3. Approach: bind all resident coverage pages as a `texture_2d_array`

Replace the coverage atlas's per-page independent single-layer 2D textures with
**one layered texture** (a `texture_2d_array`), upload each page to its own array
layer, bind the array view once through a **new coverage-only layout**, and have
the shader sample the layer the instance names.

### 3.1 Why a texture array is the *only* correct choice here
This is not a taste call between three viable options — the repo's constraints
eliminate the others:

- **WebGL2 (second, unflagged backend).** `texture_2d_array<f32>` lowers to GLSL-ES
  `sampler2DArray`, whose layer index is a **dynamic** third coordinate component
  with no uniformity restriction. The per-page-bind / bindless alternatives use an
  array-of-samplers (`sampler2D tex[N]`), which GLSL-ES 3.0 restricts to
  **constant / dynamically-uniform** indices — our index is per-instance
  (non-uniform), so those approaches are **not compilable on WebGL2**.
- **WGSL uniformity (the WASM-campaign D2 hazard).** The prior blank-screen bug was
  *derivative ops* (`fwidth`/implicit-LOD `textureSample`) called in **non-uniform
  control flow**; Chrome's Tint enforces the uniform-control-flow rule, native naga
  is lenient. Our sample uses **explicit LOD** (`textureSampleLevel(..., 0.0)`),
  which is **not a derivative op** and is exempt from that rule entirely. The
  per-instance `array_index` argument is *not* constrained by the uniformity rule.
  So the fix is uniformity-clean by construction — and strictly safer than today's
  implicit-LOD sample. `coverage.wgsl` already samples **unconditionally** and
  masks the clip via alpha (the D2 fix is already in); we preserve that (no early
  return around the sample).
- **Draw calls / attributes.** One bind, one draw per existing range — no per-page
  batching, no re-sort, and **no new vertex attribute**. The one added inter-stage
  varying (`page`) brings the count to 6, well under WebGL2's
  `max_inter_stage_shader_variables = 15`.

`page_budget = 8` layers is safe everywhere: wgpu's `downlevel_webgl2_defaults`
inherits `max_texture_array_layers = 256`; R8Unorm as a sampled array is core GLES3.

### 3.2 Layer count: grow-to-high-water (the one real decision)

A `texture_2d_array`'s layer count is **fixed at creation**, but the atlas grows
and shrinks pages on demand. Decision: **the GPU array's layer count is grow-only
"high-water" — it grows to match a new maximum page count and never shrinks below
it.**

The CPU-side page count is **not monotonic**: `atlas.rs::collect_emptied_pages`
(run every frame via `maintain_atlas`) pops trailing empty pages back into the
pool, so `page_count(CoverageR8)` can shrink and later re-grow. The GPU side is
already grow-only today — `prepare_atlas_textures` only ever grows
(`gpu.coverage_pages.len() < pages.len()`), keeping over-budget pages. We mirror
exactly that: track the array with the invariant **`array_layers >= pages.len()`**,
and **recreate only when `pages.len()` exceeds the current `array_layers`** (a new
high-water). A transient shrink-then-regrow within the existing high-water needs no
recreate — the re-grown page re-uploads into its (already-allocated) layer when it
next goes dirty. (An `array_layers == pages.len()` invariant would be wrong — it
would panic/churn on the first shrink.)

Growth events are therefore rare and bounded (≤ the working-set high-water, which
is ≤ ~`page_budget` plus the rare over-budget append, itself bounded by the 256
layer ceiling). Each is a one-time `O(pages × page_size²)` ≤ 8 MB re-upload inside
`prepare` — not per frame. The over-budget append (`atlas.rs:193-212` can exceed
`page_budget` when the LRU is exhausted) is representable for free.

**Why not fixed-`page_budget`-layers-upfront (runner-up).** Simpler (allocate 8
layers once, never recreate, bind once) — but it commits **8 MB of VRAM always
resident**, even for the overwhelming majority of Buiy apps that never cross one
page, and it **cannot represent the over-budget edge** (page count can exceed 8),
so it needs an extra clamp/drop with a defined outcome for a page-≥8 glyph.
Grow-to-high-water uses ~1 MB in the common case, handles the over-budget append for
free, and its only cost is a rare recreate. Both reviewers endorsed it.

Because the bind-group *layout* is `view_dimension: D2Array` (a shape, carrying no
layer count), a recreate needs **no pipeline recompile** — only a new bind group.

### 3.3 The edits

1. **`render/pipeline.rs` — FORK the layout (do NOT mutate the shared one).**
   `atlas_layout_entries()` / `atlas_layout_descriptor()` / `build_atlas_layout()`
   (and `BuiyPipeline::atlas_layout`) are **also consumed by the raster
   (drawing-canvas) pipeline** — `raster.rs:311` (pipeline layout) and
   `raster.rs:504` (per-image bind group), with `raster.wgsl:23` binding a plain 2D
   `texture_2d<f32>` image. **Leave those `texture_2d<f32>` (D2) untouched.** Add a
   **new coverage-only layout**: a `coverage_atlas_layout_entries()` using
   `texture_2d_array(Float{filterable:true})` + the same `sampler(Filtering)`, a
   `coverage_atlas_layout_descriptor()`, a concrete `build_coverage_atlas_layout()`,
   and a **new `BuiyPipeline::coverage_atlas_layout` field**. Import
   `texture_2d_array` alongside `texture_2d`.

2. **`render/primitive.rs` — point the glyph pipeline at the new layout.** In the
   `is_glyph` branch of `BuiyPrimitives::specialize` (~line 764), replace
   `atlas_layout_descriptor()` with `coverage_atlas_layout_descriptor()` for the
   `@group(1)` entry. This covers both glyph pipeline variants
   (`Glyph@Rgba8UnormSrgb` and the effect-group `Glyph@Rgba16Float`). The raster
   pipeline's `atlas_layout_descriptor()` stays as-is.

3. **`render/atlas/gpu.rs` — the core change.** Replace the coverage
   `Vec<PageTexture>` (independent single-layer 2D textures) with one array texture
   created with `depth_or_array_layers = array_layers` (the high-water count),
   `dimension: D2`, and an array view
   (`TextureViewDescriptor { dimension: Some(D2Array), .. }`). Upload each page with
   `origin: Origin3d { z: page_index, .. }`, copy extent `depth_or_array_layers: 1`.
   On growth (`pages.len()` exceeds `array_layers`), recreate the array at the new
   high-water and **re-upload ALL resident pages** (not just dirty ones — a
   recreated texture has no residual contents; the current dirty-gated loop at
   `gpu.rs:173-191` is insufficient on a recreate frame), then rebuild
   `coverage_bind_group` against the new array view (against
   `build_coverage_atlas_layout()`). On a non-recreate frame, upload dirty pages to
   their layers as today. **This is where the restructure lives; every other edit is
   a one-liner-class change.**

4. **`render/coverage.wgsl` — binding + flat varying + sample.**
   - `@group(1) @binding(0) var atlas: texture_2d<f32>;` → `texture_2d_array<f32>;`
     (the `render_shader_wgsl.rs:107` substring assertion `"@group(1) @binding(0)
     var atlas"` still holds).
   - Add `@interpolate(flat) page: u32` to `VertexOut`; in the vertex stage replace
     `_ = i.page;` with `out.page = i.page;` (a per-instance integer **must** be
     `flat` — never perspective-interpolated).
   - Sample: `textureSample(atlas, atlas_samp, in.atlas_uv).r` →
     `textureSampleLevel(atlas, atlas_samp, in.atlas_uv, in.page, 0.0).r`
     (explicit LOD 0; nearest, single-mip — semantically identical on page 0,
     uniformity-clean). Keep the surrounding "sample unconditionally, mask via
     alpha" structure exactly as-is.

5. **`render/node.rs` — no change.** Every coverage draw site binds the opaque
   `coverage_bind_group()`; swapping its *contents* (array view) inside `gpu.rs` is
   invisible to all five sites. The `node.rs:573` `@group(1)` site is the *raster*
   bind (`draw.bind_group`), not coverage — untouched.

6. **`text/extract.rs` — retire the now-dead v1 mitigation.** Once page ≥1 renders
   correctly the "warn at first page-1 allocation" mitigation is obsolete and
   **misleading**. Remove `warn_once_page_overflow()` (2291), the
   `WARNED_PAGE_OVERFLOW` static (2277), and its three call sites (1557, 1644,
   1835). Grep-confirmed no test asserts on the warning; the icon producer never
   called it.

### Edit-list summary
`pipeline.rs` (new coverage layout + `BuiyPipeline` field), `primitive.rs` (glyph
descriptor → coverage layout), `atlas/gpu.rs` (array texture + per-layer upload +
recreate-with-full-reupload + array bind group), `coverage.wgsl` (array binding +
flat `page` varying + explicit-LOD array sample), `text/extract.rs` (retire warn).
**Raster (`raster.rs`, `raster.wgsl`) and `node.rs` are deliberately untouched.**

---

## 4. Rejected alternatives

- **App-side chat cap (the "stopgap").** Cap Dooduel chat to the last ~N messages so
  the working set stays under one page. Rejected as the *fix*: it is a
  Dooduel-only mitigation of a framework bug that affects every text-heavy Buiy
  screen; it papers the symptom, not the cause, and doesn't help icons or any other
  app. (Kept only as an emergency lever if the real fix must be deferred — it is
  not being deferred.)
- **Per-page draw batches** (one bind group per page, partition instances by page).
  Rejected: multiplies draw calls up to `page_budget`, needs a second sort key that
  fights the existing flat/effect-group range partitioning, adds per-page bind-group
  churn — and gains nothing on WebGL2, where `sampler2DArray` is already supported.
- **Bindless / `binding_array<texture_2d>`.** Rejected: requires
  partially-bound/indexing features unavailable on WebGL2 and gated on native; the
  non-constant sampler-array index is exactly the GLSL-ES restriction WebGL2 forbids.
  Wrong tool for a browser-and-native library.
- **Single grow-only page** (one texture, enlarge instead of paging). Rejected:
  `page_size²` is capped by `max_texture_dimension_2d` (~8192 on WebGL2-class HW),
  grow means expensive repack + full re-upload + residency churn, and it discards
  the existing page-pool/eviction machinery.
- **Mutating the shared `atlas_layout` instead of forking it.** Rejected — it breaks
  the raster/drawing-canvas pipeline (the blocker this revision fixes); see § 3.3.
- **Fixed-`page_budget`-layers upfront.** The runner-up, not rejected on
  feasibility — see § 3.2 for why grow-to-high-water wins on VRAM + the over-budget
  edge.

---

## 5. Verification

The bug lives entirely in the GPU bind + shader, so the decisive proof is on the
GPU `#[ignore]` lane (this repo has a real adapter). Layers:

1. **GPU ink census (the decisive test)** — new `#[ignore]` test in
   `crates/buiy_core/tests/text/text_gpu.rs`, modelled on the existing painted-frame
   tests there. Build `gpu_render_app(W, H)` + `render_to_image` +
   `spawn_capture_camera`; spawn a text working set whose distinct-glyph residency
   **exceeds one 1024² CoverageR8 page** (many sizes/families/characters — the
   Dooduel scoreboard+word+chat class; or shrink `page_size` à la
   `crosscut/atlas_gpu.rs`'s 64-texel trick to force overflow cheaply), the **last**
   node being a fresh short probe string. `wait_for_text_ready` until resident,
   then — **guarding against a pre-fix false pass** (a page-≥1 glyph sampled against
   a *dense* page 0 can land on *another glyph's* ink, so bare "non-zero ink" does
   not distinguish fixed from broken):
   - **Precondition on the probe specifically:** assert the probe glyph's own
     `AtlasEntry.page > 0` via the render-world `BuiyAtlas.get(key)` (reachable, cf.
     `text_gpu.rs:224`) — not merely aggregate `page_count > 1` (which could be
     satisfied by *other* nodes while the probe sits on page 0).
   - **Footprint match, not presence:** pin the probe at the **same absolute screen
     rect** in both a probe-alone reference pass (guaranteed page 0) and the overflow
     scene, then compare the **lit-pixel count** within that rect (AA-robust; absolute
     centroid is unreliable across the two layouts) — not just `channel_lit != 0`.
     (Force overflow cheaply via a shrunk `page_size` so the pinned probe stays
     on-screen in a small capture.) The probe uses a **unique glyph** so its own
     `AtlasEntry.page` is identifiable (`ResidentTextKeys` is a flat key list).
   - **Icon coverage:** include one icon node forced onto page ≥1 in the overflow
     scene and assert its ink post-fix (the icon path shares the coverage bind; this
     is where icons-on-page-≥1 is proven, since no device-free icon harness exists).
   - Pre-fix: fails (blank / count-mismatched garbage). Post-fix: passes. Adapter-robust
     (`channel_lit >= 128`, no `on_pinned_lavapipe()` gate).

2. **Recreate / second-growth test (required, not a risk note)** — force page count
   to cross **1→2** (a *second* growth, so page 0 is already-resident and clean when
   the recreate fires) and assert that a **page-0 node** and a **page-1 node** both
   still render ink after the recreate. This is the guard that the recreate
   re-uploads *clean* pages (the dirty-gated loop would silently drop page 0, and a
   single 0→1 crossing wouldn't catch it because page 0 is dirty that frame). Lives
   alongside the census in `text_gpu.rs`.

3. **Existing GPU goldens stay byte-identical.** The current coverage/text goldens
   all fit page 0; with only layer 0 live, `textureSampleLevel(array, …, 0, 0.0)` on
   a single-mip nearest texture must produce identical texels to today's
   `textureSample(2d, …)`. Running the `buiy_core` + `buiy_verify` GPU legs unchanged
   proves no regression on the common (single-page) path, including the
   `Glyph@Rgba16Float` effect-group path.

4. **Raster/drawing-canvas guard.** `render_raster_gpu.rs` and
   `render_raster_interleave_gpu.rs` must **stay green** — the explicit proof that
   forking the layout left the drawing canvas (the raster 2D-image path) undisturbed.

5. **Cheap headless guard (Tier 3, no adapter)** — extend the device-free atlas /
   producer path (near `buiy_verify/src/invariant/content_presence.rs`'s
   `glyph_census`, or a device-free atlas test near
   `crates/buiy_core/tests/text/text_touch_pass.rs`). Run production
   `extract_buiy_glyphs` over a scene that overflows page 0 (via
   `ExtractHarness::with_atlas_config(AtlasConfig { page_size: small, .. })`) and
   assert **page-index encoding correctness** for an overflow **glyph**:
   `page_count(CoverageR8) > 1` and `instance.page == entry.page as u32` (the `u16`
   `AtlasEntry.page` cast to the instance's `u32`; the instance names the page the
   shader will index by). Runs on every headless CI run. (This guards the CPU→instance
   plumbing — pre-existing and unchanged by this fix — so it is a regression guard, not
   proof of the fix; the GPU census is the proof.) **Icons are not covered here** —
   the extract harness doesn't run the icon producer and there is no device-free icon
   harness, so the icon-overflow check lives in the GPU census (§ 5.1) instead, where
   the full stack runs `extract_buiy_icons`.

   > **Wording reconciliation.** The follow-up note asked for "a cheap headless guard
   > that no live entry lands on page>0 within budget." That literally describes the
   > *stopgap's* packing invariant (keep everything on one page) — the **inverse** of
   > the fix, which *wants* page>0 to be reachable and correct. The guard above
   > (page-index correctness) supersedes that phrasing.

6. **WebGL2 empirical check.** The uniformity/WebGL2 argument (§ 3.1) is sound but
   argued, not run — and the repo has a D2 blank-screen history. Add a forced-overflow
   coverage scene (tiny `page_size`) exercised through the SwiftShader WebGL2 CI gate
   (the enforced `gallery_web` / web-smoke lane), or, if that can't force overflow,
   a documented manual `gallery_web` check with a small page budget. Named so it is
   not silently skipped.

**How to run** (from CLAUDE.md; both GPU legs, CI runs both):
```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1
```
plus the headless default gate (fmt / clippy / doc / `xvfb-run -a cargo test --workspace --locked`).

---

## 6. Docs to flip (part of the deliverable)

- `docs/specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md` § 11 item 1 —
  mark the single-page-0-bind limitation resolved, pointing here.
- `docs/specs/2026-06-03-buiy-render-pipeline-design/2026-06-08-render-atlas-glyph-gpu-design.md`
  (the page-0-bind note, ~lines 106-109) — same.
- `docs/plans/follow-ups.md` — mark the "chat rows render as empty pills" entry
  RESOLVED with the landed fix.
- `docs/README.md` — add the spec + plan rows.

---

## 7. Risks & residual items

- **Growth-path recreate correctness** — the recreate + re-upload-**all** path is
  exercised only when the high-water grows mid-session; the § 5.2 second-growth test
  is the hard gate for it. Confirm the recreate forces every resident page to
  re-upload (a recreated texture has no residual contents; do **not** rely on the
  per-page dirty flags).
- **Over-budget beyond the array** — grow-to-high-water always matches the peak page
  count, so the LRU-exhausted over-budget append is representable with no clamp. The
  256-layer ceiling is the only hard cap and is pathologically unreachable
  (`page_budget = 8`).
- **Byte-identical page-0** — the existing goldens (§ 5.3) are the check that the
  single-layer array view samples identically to today's 2D view.
- **Out-of-scope follow-up (logged, not fixed here):** `primitive.rs:768`'s comment
  says `GlyphAlphaInstance` "stride 68" while the asserted stride is 84 — a stale
  comment unrelated to this fix; note it for a separate cleanup.
- **Future color-icon path** — if `IconInstance`/`ColorRgba8` is ever built it will
  need its own multi-page bind; this fix does not provide it (§ 1).
