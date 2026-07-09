# Multi-page coverage atlas bind — implementation plan

> **For agentic workers:** execute task-by-task with a fresh implementer subagent
> per wave, each gated by a review subagent (subagent-driven-development). Steps use
> checkbox (`- [ ]`) syntax. Drive the waves **sequentially in one warm worktree**
> (the current `feat/dooduel-multiplayer-m1`) — the waves are interdependent GPU
> changes, not independent units.

**Goal:** Bind all resident coverage atlas pages as a `texture_2d_array` and sample
the per-instance `page` layer, so glyphs/icons that spill past one 1024² page render
correctly instead of blank. Fixes the Dooduel "empty chat pill" framework bug.

**Spec:** [`docs/specs/2026-07-09-multipage-coverage-atlas-bind-design.md`](../specs/2026-07-09-multipage-coverage-atlas-bind-design.md)
(read it — it carries the rejected alternatives, the raster-layout-fork blocker two
spec reviewers caught, and the grow-to-high-water reasoning).

**Architecture:** Fork a new coverage-only `D2Array` bind-group layout (leaving the
shared `atlas_layout` `D2` for the raster/drawing-canvas pipeline), repackage the
coverage atlas's per-page 2D textures into one array texture (layer per page,
grow-to-high-water, re-upload-all on recreate), bind the array view, and switch the
shader to an explicit-LOD array sample of the per-instance `page`. `node.rs` and the
CPU `page`-plumbing are unchanged; no new vertex attribute.

**Tech stack:** Rust, Bevy 0.19-rc.3 render, wgpu 29, WGSL. GPU verified on the
`#[ignore]` lane (real adapter present in this repo).

---

## File structure / edit map

| File | Change |
|---|---|
| `crates/buiy_core/src/render/pipeline.rs` | **New** coverage-only `D2Array` layout (`coverage_atlas_layout_entries/_descriptor` + `build_coverage_atlas_layout`) + a new `BuiyPipeline::coverage_atlas_layout` field (built in `pipeline::register`, NOT `FromWorld`). Leave `atlas_layout_entries`/`atlas_layout_descriptor`/`build_atlas_layout`/`BuiyPipeline::atlas_layout` as `texture_2d` (D2) for raster. |
| `crates/buiy_core/src/render/primitive.rs` | Glyph `specialize` `@group(1)` → `coverage_atlas_layout_descriptor()` (both `Rgba8UnormSrgb` + `Rgba16Float` variants inherit it). |
| `crates/buiy_core/src/render/atlas/gpu.rs` | Coverage pages → one `texture_2d_array`; per-layer upload (`origin.z`); grow-to-high-water recreate + **re-upload ALL** resident pages; bind group against the array view + `coverage_atlas_layout`. |
| `crates/buiy_core/src/render/coverage.wgsl` | `atlas` → `texture_2d_array<f32>`; forward `page` to fragment as `@interpolate(flat) u32`; sample `textureSampleLevel(..., in.page, 0.0)`. |
| `crates/buiy_core/src/text/extract.rs` | Retire `warn_once_page_overflow` + `WARNED_PAGE_OVERFLOW` + 3 call sites. |
| `crates/buiy_core/tests/text/text_gpu.rs` | **New** `#[ignore]` GPU ink census + recreate/second-growth test. |
| headless guard (near `text_touch_pass.rs` or `content_presence.rs`) | **New** device-free page-index-encoding guard (glyph + icon). |
| Docs: `glyph-pipeline.md` §11.1, `render-atlas-glyph-gpu-design.md`, `follow-ups.md`, `README.md` | Flip to resolved/landed. |
| **NOT touched** | `render/node.rs`, `render/raster.rs`, `render/raster.wgsl`. |

Verify commands (used throughout):
- Headless gate: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && xvfb-run -a cargo test --workspace --locked`
- GPU lane (both legs): `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` and `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1`
- Single GPU file (fast loop): `cargo test -p buiy_core --test text_gpu -- --ignored --test-threads=1`

---

## Wave 0 — RED tests (write the failing proof first)

**Files:** `crates/buiy_core/tests/text/text_gpu.rs` (extend), a device-free guard test.

### Task 0.1 — GPU ink census (RED pre-fix)
Model on the existing painted-frame tests in `text_gpu.rs` (see `text_gpu.rs:224` for
reaching the render-world `BuiyAtlas.get(key)`).

- [ ] **Build the overflow scene.** In a new `#[ignore]` test, `gpu_render_app(W,H)`
  + `render_to_image` + `spawn_capture_camera`. Force coverage overflow cheaply by
  shrinking the atlas page size (the `crosscut/atlas_gpu.rs` 64-texel `AtlasConfig`
  trick) so a modest scene crosses to page ≥1. Spawn a batch of distinct text nodes,
  the **last** being a fresh short **probe** string. `wait_for_text_ready`.
- [ ] **Make the probe glyph UNIQUELY identifiable.** `ResidentTextKeys.keys` is a
  flat `Vec<AtlasKey>` with no key→entity map, so the probe's own key must be
  reconstructible. Use a probe **character that appears nowhere else in the scene**
  (a distinctive glyph at a distinctive px-size). Reconstruct its `AtlasKey` via
  `glyph_atlas_key(cache_key, interner)` (`atlas_key.rs:94`, pub) from the probe's
  cosmic-text `CacheKey` + the render-world `FontKeyInterner`, or — simpler — assert
  that exactly one resident key at the probe's unique cell size has `page > 0`.
- [ ] **Precondition on the probe specifically:** assert the probe glyph's own
  `AtlasEntry.page > 0` via the render-world `BuiyAtlas.get(key)` — NOT merely
  aggregate `page_count(CoverageR8) > 1` (which other nodes could satisfy while the
  probe sits on page 0, making the test vacuous). Fail loudly if the probe didn't
  overflow (tune the scene — the `page_size`-shrink trick below — so it does).
- [ ] **Pin the probe at a FIXED absolute screen rect in both passes.** Render the
  probe ALONE in a reference pass AND in the overflow scene at the **same absolute
  position** (a fixed-position node, not stacked at a column top — otherwise the
  in-scene y differs and, in a small `H` capture, the probe can clip off-screen →
  spurious RED). Keep the probe fully inside the viewport in both.
- [ ] **Footprint = lit-pixel COUNT within the pinned rect (AA-robust), not centroid.**
  Capture the probe's lit-pixel count (`channel_lit >= 128`, adapter-robust — no
  `on_pinned_lavapipe()` gate) in the alone pass, then assert the overflow-scene
  probe rect's count **matches within tolerance**. (Bare "non-zero ink" can
  FALSE-PASS pre-fix: a page-≥1 UV sampled against a dense page 0 often lands on
  another glyph's ink. Count-in-a-pinned-rect avoids both that and AA false-fails.)
- [ ] **Include one ICON in the overflow scene** (the icon page-index guard moved
  here from the headless tier — icons emit `GlyphAlphaInstance` through the same
  coverage bind, and no device-free icon harness exists). Force it onto page ≥1 and
  assert its ink is present post-fix (blank pre-fix).
- [ ] **Force overflow cheaply via `page_size` shrink** (`crosscut/atlas_gpu.rs`'s
  64-texel `AtlasConfig` trick) so a small, fully-on-screen scene still crosses to
  page ≥1 — reconciles "overflow" with "probe stays visible in the capture."
- [ ] **Run on the GPU lane, expect RED.** `cargo test -p buiy_core --test text_gpu
  -- --ignored --test-threads=1`. Pre-fix the probe/icon are blank / count-mismatched
  → the test FAILS. Record the failure.

### Task 0.2 — recreate / second-growth test (RED pre-fix)
- [ ] In `text_gpu.rs`, a second `#[ignore]` test that forces page count to cross
  **1→2** (a *second* growth, so page 0 is resident + clean when the recreate fires),
  then asserts a **page-0 node** AND a **page-1 node** both render ink (footprint
  match against alone-renders). Pre-fix: page-1 node blank → RED.

### Task 0.3 — headless page-index guard (GREEN regression-guard, GLYPHS only)
- [ ] A device-free test. **Prefer extending** `content_presence.rs`'s `glyph_census`
  or `text_touch_pass.rs` (both run `extract_buiy_glyphs` device-free) so no new test
  file needs registering; if instead it lands as a NEW file near `text_touch_pass.rs`,
  register it in the `tests/text.rs` mod tree. Use
  `ExtractHarness::with_atlas_config(AtlasConfig { page_size: <small>, .. })` to
  overflow page 0. Assert `page_count(CoverageR8) > 1` AND, for an overflow glyph,
  `instance.page == entry.page as u32` — **note the cast: `AtlasEntry.page` is `u16`,
  `GlyphAlphaInstance.page` is `u32`** (mirrors the producer's own `page: entry.page
  as u32`). `harness.glyphs().glyphs` is public; `AtlasEntry.page` via `atlas.get(key)`.
- [ ] **Icons are NOT covered here** — `TextExtractHarness` doesn't run the icon
  producer and has no `icons()` accessor (no device-free icon harness exists). The
  icon-overflow check lives in the GPU census (Task 0.1) instead.
- [ ] This PASSES already (CPU plumbing is correct + unchanged) — it is a regression
  guard, not fix-proof. Confirm it's green on the headless gate.

- [ ] **Commit** Wave 0: `test(render): RED multi-page coverage census + recreate + headless page-index guard`.

---

## Wave 1 — the fix (one coupled unit: layout fork + array texture + shader)

> These four tasks are interdependent: after 1.1/1.2 the glyph pipeline layout is
> D2Array but the bind group is still 2D (GPU-lane mismatch is EXPECTED between 1.2
> and 1.3). Verify the wave as a whole after 1.4. Do NOT expect the GPU lane green
> mid-wave.

### Task 1.1 — fork the coverage layout in `pipeline.rs`
**Files:** `crates/buiy_core/src/render/pipeline.rs`.

- [ ] Import `texture_2d_array` alongside `texture_2d` (line 19 `binding_types`).
- [ ] Add the new coverage layout functions next to the existing atlas ones
  (~lines 149-175). The new entries are byte-identical to `atlas_layout_entries`
  except `texture_2d` → `texture_2d_array`:

```rust
/// The bind-group-layout entries for the COVERAGE atlas `@group(1)`: a
/// fragment-stage `texture_2d_array<f32>` (all resident coverage pages, sampled
/// by the per-instance layer) + a filtering `sampler`. Forked from
/// `atlas_layout_entries` (which stays `texture_2d` for the raster/image
/// pipeline) so the multi-page coverage bind cannot break the drawing canvas.
fn coverage_atlas_layout_entries() -> BindGroupLayoutEntries<2> {
    BindGroupLayoutEntries::sequential(
        ShaderStages::FRAGMENT,
        (
            texture_2d_array(TextureSampleType::Float { filterable: true }),
            sampler(SamplerBindingType::Filtering),
        ),
    )
}

/// Pipeline-layout descriptor for the coverage atlas `@group(1)` (D2Array).
/// Declared by the glyph `specialize`; matched by [`build_coverage_atlas_layout`].
pub(crate) fn coverage_atlas_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("buiy_coverage_atlas_layout", &coverage_atlas_layout_entries())
}

/// Build the concrete coverage `@group(1)` (D2Array) bind-group layout. The atlas
/// prepare system builds the coverage bind group against
/// [`BuiyPipeline::coverage_atlas_layout`]; this is its constructor.
pub fn build_coverage_atlas_layout(device: &RenderDevice) -> BindGroupLayout {
    device.create_bind_group_layout("buiy_coverage_atlas_layout", &coverage_atlas_layout_entries())
}
```

- [ ] Add a `pub coverage_atlas_layout: BindGroupLayout` field to `BuiyPipeline` and
  build it in the free fn **`pipeline::register`** (NOT `FromWorld` — `BuiyPipeline`
  has no `FromWorld`; `register` is called from `BuiyRenderPlugin::finish`). Three
  edit sites, mirroring `atlas_layout`: the field declaration (~`pipeline.rs:240`),
  the `let coverage_atlas_layout = build_coverage_atlas_layout(world.resource::<RenderDevice>());`
  build (next to `atlas_layout` at ~`pipeline.rs:271`), and the struct literal
  (~`pipeline.rs:400-411`). Update the stale doc comment on `atlas_layout_entries` to
  say it now serves the **raster/image** pipeline (the coverage note moved to the new fn).
- [ ] `cargo check -p buiy_core` compiles.

### Task 1.2 — point the glyph pipeline at the coverage layout
**Files:** `crates/buiy_core/src/render/primitive.rs` (~line 764, the `is_glyph`
branch of `BuiyPrimitives::specialize`).

- [ ] Replace the `@group(1)` entry in the glyph pipeline `layout` vec from
  `atlas_layout_descriptor()` to `coverage_atlas_layout_descriptor()`. This covers
  both glyph variants (`Glyph@Rgba8UnormSrgb`, effect-group `Glyph@Rgba16Float`).
  Leave the raster pipeline's `atlas_layout_descriptor()` (`raster.rs:311`) alone.
- [ ] `cargo check -p buiy_core` compiles.

### Task 1.3 — array texture + per-layer upload + recreate in `atlas/gpu.rs`
**Files:** `crates/buiy_core/src/render/atlas/gpu.rs`. (Anchors from research:
`PageTexture`/`coverage_pages` line 42; `create_page_texture` line 90;
`upload_page` lines 113-134 (`Origin3d::ZERO`, extent depth 1); `prepare_atlas_textures`
grow loop lines 162-169; dirty-gated upload 173-191 + `clear_all_dirty`;
`coverage_bind_group` build lines 200-204.)

Read the real file, then apply this transformation:

- [ ] **Imports:** add `TextureViewDimension` to the `gpu.rs` import list (line ~18
  imports `TextureDimension` but not `TextureViewDimension`, which Task 1.3 uses).
- [ ] **One array texture, not a Vec of single-layer textures.** Replace the coverage
  `Vec<PageTexture>` with a single array texture + its D2Array view + a tracked
  `array_layers: u32` (high-water). The texture/view must be **`Option`/lazily
  created** — at startup nothing is resident (`array_layers == 0`) and a 0-layer
  texture is invalid; the first grow (0→N via the recreate path below) creates it.
  Create it with:
  ```rust
  TextureDescriptor {
      label: Some("buiy_coverage_atlas_array"),
      size: Extent3d { width: page_size, height: page_size, depth_or_array_layers: array_layers },
      mip_level_count: 1, sample_count: 1,
      dimension: TextureDimension::D2,           // layers are D2
      format: AtlasFormat::CoverageR8.texture_format(),  // R8Unorm
      usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
      view_formats: &[],
  }
  ```
  and the view with `TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), .. }`.
- [ ] **Per-layer upload.** Each page uploads to its layer: `origin: Origin3d { x:0, y:0, z: page_index }`, copy extent `depth_or_array_layers: 1` (rest of `write_texture` unchanged from `upload_page`).
- [ ] **Grow-to-high-water recreate.** Maintain the invariant `array_layers >= pages.len()`. When `pages.len() > array_layers` (a new high-water — mirrors today's `coverage_pages.len() < pages.len()` grow check): recreate the array texture at the new layer count, **mark ALL resident pages for re-upload and re-upload every one of them** (a fresh array texture has NO residual contents — the existing dirty-only loop is insufficient on a recreate frame; do not rely on `is_dirty()`), and rebuild the bind group. Do NOT shrink `array_layers` when `pages.len()` transiently drops (`collect_emptied_pages` pops trailing pages every frame — the array stays at high-water; the re-grown page re-uploads into its already-allocated layer when next dirty).
- [ ] **Bind group against the array view + the new layout.** Build
  `coverage_bind_group` from `pipeline.coverage_atlas_layout` (NOT `pipeline.atlas_layout`)
  with the D2Array view at binding 0 + the sampler at binding 1. Rebuild it on
  recreate (and keep the existing `bind_group_stale` invalidation).
- [ ] `cargo check -p buiy_core` compiles. Headless gate green (no adapter → no
  validation yet).

### Task 1.4 — shader: array binding + flat `page` varying + explicit-LOD sample
**Files:** `crates/buiy_core/src/render/coverage.wgsl`.

- [ ] Line 24: `@group(1) @binding(0) var atlas: texture_2d<f32>;` →
  `@group(1) @binding(0) var atlas: texture_2d_array<f32>;`
- [ ] `VertexOut` (lines 44-51): add `@location(5) @interpolate(flat) page: u32,`
  (must be `flat` — a per-instance integer is not perspective-interpolable).
- [ ] Vertex stage (line 80): replace `_ = i.page;` with `out.page = i.page;`
  (update the adjacent comment: `page` now selects the array layer).
- [ ] Fragment sample (line 97):
  `let coverage = textureSample(atlas, atlas_samp, in.atlas_uv).r;` →
  `let coverage = textureSampleLevel(atlas, atlas_samp, in.atlas_uv, in.page, 0.0).r;`
  Keep the "sample unconditionally, mask via alpha" structure (lines 89-99) exactly —
  update the comment to note the sample is now **explicit-LOD** (not a derivative op),
  so the mask pattern is retained for consistency, not uniformity necessity. The
  `render_shader_wgsl.rs:107` substring `"@group(1) @binding(0) var atlas"` still holds.

### Wave 1 verification (the whole coupled unit)
- [ ] `cargo check -p buiy_core` + headless gate green.
- [ ] GPU lane both legs green: `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`
  and `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1`. Specifically:
  - Wave-0 census (0.1) now PASSES (probe page>0 renders, footprint matches).
  - Recreate test (0.2) now PASSES (page-0 + page-1 nodes both inked).
  - **Existing coverage/text goldens BYTE-IDENTICAL** (page-0 output unchanged —
    `textureSampleLevel(array,…,0,0.0)` == today's `textureSample` on a single-mip
    nearest texture). No golden re-bless.
  - **`render_raster_gpu.rs` + `render_raster_interleave_gpu.rs` GREEN** (proof the
    layout fork left the drawing canvas undisturbed).
- [ ] **Commit** Wave 1: `feat(render): bind all coverage atlas pages as a texture_2d_array`.

---

## Wave 2 — retire the dead v1 mitigation

**Files:** `crates/buiy_core/src/text/extract.rs`.

- [ ] Remove `warn_once_page_overflow()` (fn ~2291), the `WARNED_PAGE_OVERFLOW`
  static (~2277), and the three call sites (~1557 strikethrough stamp, ~1644 caret
  stamp, ~1835 glyph emit). The `entry.page > 0` guard branches that only fired the
  warn go away; the `page: entry.page as u32` emission stays. (Grep-confirmed no test
  asserts on the warning.)
- [ ] **Keep the `use std::sync::atomic::{AtomicBool, Ordering}` import** (`extract.rs:14`)
  — it stays used by `WARNED_COLOR_EMOJI`/`warn_once_color_emoji` after the
  page-overflow warn is removed; do NOT remove it.
- [ ] `cargo clippy -p buiy_core --all-targets --locked -- -D warnings` clean (no
  dead-code/unused-import warnings from the removal). Headless gate green.
- [ ] **Commit** Wave 2: `chore(render): retire the page-0-bind overflow warning (multi-page bind lands)`.

---

## Wave 3 — WebGL2 check + docs + final gate

### Task 3.1 — WebGL2 empirical check
- [ ] Exercise a forced-overflow coverage scene (tiny page budget) through the
  SwiftShader WebGL2 gate (the enforced web-smoke / `gallery_web` lane) to confirm
  the `sampler2DArray` lowering renders (no D2-class blank screen). If the automated
  lane can't force overflow, run + document a manual `gallery_web` check with a small
  page budget. Record the result in the acceptance report.

### Task 3.2 — flip docs
- [ ] `docs/specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md` § 11 item 1
  → resolved, pointing at this spec.
- [ ] `docs/specs/2026-06-03-buiy-render-pipeline-design/2026-06-08-render-atlas-glyph-gpu-design.md`
  page-0-bind note (~106-109) → resolved.
- [ ] `docs/plans/follow-ups.md` "chat rows render as empty pills" → RESOLVED (root
  cause + landed fix + verification).
- [ ] `docs/README.md` — spec row `[draft]` → `[landed]`; add the plan row.
- [ ] Log the out-of-scope `primitive.rs:768` stale "stride 68" comment as a fresh
  follow-up (do not fix here).

### Task 3.3 — full gate + acceptance
- [ ] Full headless gate green: `cargo fmt --all -- --check && cargo clippy
  --workspace --all-targets --locked -- -D warnings && RUSTDOCFLAGS="-D warnings"
  cargo doc --workspace --no-deps --locked && xvfb-run -a cargo test --workspace --locked`.
- [ ] Both GPU legs green.
- [ ] **Run the real app**: `cargo run -p dooduel` (or the M1 networked path), drive
  the in-game chat past the overflow threshold, confirm chat text renders (the
  original bug is gone). This is the run-the-artifact acceptance.
- [ ] Write `docs/reports/2026-07-09-multipage-coverage-atlas-bind.md` (or fold into
  the M1 acceptance notes): what was verified, the WebGL2 result, before/after.
- [ ] **Commit** Wave 3: `docs(render): mark multi-page coverage atlas bind landed`.

---

## Self-review (author checklist — done before execution)

- **Spec coverage:** every §3.3 edit maps to a task (pipeline fork → 1.1; glyph
  descriptor → 1.2; gpu.rs → 1.3; shader → 1.4; warn removal → W2; docs → 3.2).
  Every §5 verification maps to a test (census → 0.1; recreate → 0.2; headless guard
  → 0.3; goldens byte-identical + raster guard → Wave-1 verification; WebGL2 → 3.1).
- **No placeholders:** the two exact edits (shader, layout fns) carry literal code;
  the gpu.rs restructure is a transformation spec against named anchors for the
  implementer to apply to the real source (it is too large to reproduce literally
  and the implementer reads it).
- **Type consistency:** `coverage_atlas_layout` / `coverage_atlas_layout_descriptor`
  / `build_coverage_atlas_layout` / `coverage_atlas_layout_entries` named
  consistently across 1.1/1.2/1.3; the D2Array view + `array_layers` high-water
  invariant is stated identically in 1.3 and the spec.
- **Ordering:** Wave 0 (RED) before Wave 1 (fix); Wave 1 verified as a coupled unit
  (mid-wave GPU redness expected); cleanup + docs last.
