# Atlas glyph/coverage GPU pipeline — design note

**Date:** 2026-06-08
**Status:** landed (GPU-verify campaign Phase 4, item 4) — the GPU atlas path is
real + green on the RX 6700 XT; the headless lane stays green. The glyph/retint GPU
coverage that drove this note now lives in the text crate (`text_gpu.rs`, e.g.
`hello_text_first_frame_is_deterministic_and_tinted` /
`retint_real_text_leaves_atlas_byte_identical`), which T8 took over when it landed
the glyph producer; `atlas_gpu.rs` retains the gate-#15 page-pool test
(`gate15_atlas_entries_return_to_baseline_after_idle`).
**Implements** atlas-and-text-seam.md §2/§4/§7 — the GPU half of the atlas, which
R10 left entirely CPU-side (allocator + LRU + pooling proven adapter-free in
`atlas_alloc.rs`; `AtlasPage.texture` is always `None`, no upload, no sampling
pipeline, no glyph draw).

## Scope

Build the coverage-glyph GPU path so the `#[ignore]` GPU stubs become
real on a wgpu adapter: page `Image` + blit + dirty/upload, a CoverageR8 sampling
pipeline + shader, an atlas bind group, a glyph instance buffer + pack + node draw
branch. The text crate (`buiy-text-rendering-design`, unbuilt) owns *producing*
coverage + emitting `GlyphAlphaInstance`; this note owns the render-world *consumer*
path. In the tests, the test plays the producer (builds the `AtlasBitmap` +
`GlyphAlphaInstance` directly), exactly as the headless `atlas_alloc.rs` tests
already do — no standing in-crate glyph producer (that would pre-empt the seam).

## Decided forks (from the exploration blueprint, workflow `wucy4tq5e`)

1. **Page upload: `RenderQueue::write_texture` in a Buiy prepare system**, NOT
   Bevy's `Assets<Image>` → `RenderAssets<GpuImage>` auto-extract. `BuiyAtlas` is a
   render-world resource and the blit happens render-world-side; routing through the
   main-world asset store adds a cross-world handle round-trip + a frame of latency
   that fights warmup determinism (gate #2). Upload only when the page is dirty
   (§2.2 "a page that gained entries this frame re-uploads; an unchanged page does
   not").
2. **New `@group(1)` for the atlas texture+sampler**, NOT extending `@group(0)`.
   `@group(0)` is the view uniform, shared byte-identically by the quad/shadow
   pipelines (`view_uniform_layout_descriptor`, "one source of truth"); they sample
   no texture. The atlas binding is additive on `@group(1)`, used only by the
   sampling pipelines — non-sampling primitives never declare a binding they don't
   use.
3. **Build the atlas bind group in prepare, stash it on a render-world resource the
   node reads** — mirrors the effect-group-target precedent (`node.rs:88-96`:
   acquisition needs `&mut`/`RenderDevice`, which `run(&World)` cannot get, so it
   lives in prepare). `run()` stays pure record-and-draw.
4. **Retint byte-identity is asserted on the page's CPU `Image` bytes**, paired with
   a framebuffer readback proving the tint visibly changed. A framebuffer-only test
   can't distinguish "atlas untouched" from "atlas regenerated identically" — the
   exact §7/§4.1 alpha-as-color regression the test guards.

## Byte-level contracts (the alignment-bug firewall)

The campaign's recurring bug class is GPU layout mismatch. These three must agree
exactly; a reviewer checks them against each other:

- **`GlyphAlphaInstance`** (`render/atlas/primitive.rs`, an existing `#[repr(C)]`
  POD) — its field order/offsets define the instance vertex-buffer layout. Confirm
  it carries: paint rect (xy pos, zw size or equivalent), atlas uv rect, color
  (CPU-prelinearized like the quad path), the clip AABB (`[±INFINITY]` sentinel
  when unclipped, identical encoding to `PackedInstance`), and the page index.
- **the Glyph specialization vertex layout** (`primitive.rs::specialize` for
  `BuiyPrimitiveKind::Glyph`) — `step_mode: Instance`, attribute offsets matching
  `GlyphAlphaInstance` byte-for-byte, plus the static unit-quad VBO0 (shared with
  the quad pipeline).
- **`coverage.wgsl`** — the `@location(...)` instance attributes must match the
  vertex layout; `@group(0) @binding(0)` view uniform (same `BuiyView` as
  `shader.wgsl`); `@group(1)` = `texture_2d<f32>` + `sampler`. Fragment:
  `out = color * textureSample(atlas, samp, uv).r` (alpha-as-color, §4.1), honoring
  the same clip-AABB fragment discard as `shader.wgsl`. New shader UUID octet `..03`.

## Files (see blueprint for per-file detail)

`atlas/page.rs` (Image + `blit` + dirty flag; `reset` keeps the handle), `atlas/atlas.rs`
(thread `bitmap.data` into `page.blit` instead of dropping it; expose resident page
handle + dirty set), `atlas/mod.rs` (prepare system: upload dirty pages + build the
`@group(1)` bind group into a resource; per-frame maintenance: `begin_frame` +
`drain_grace_expired` + `collect_emptied_pages` for gate #15), `coverage.wgsl` (NEW),
`pipeline.rs` (coverage shader UUID/handle + `@group(1)` layout + `load_internal_asset!`),
`primitive.rs` (`shader_for(Glyph)` → coverage handle; Glyph vertex + `@group(1)` in
`specialize`), `prepare.rs` (`glyph: RawBufferVec<GlyphAlphaInstance>` + pack + upload),
`buckets.rs` ((Glyph, layer) routing so the node draws glyphs in paint order
shadow<quad<glyph<path), `node.rs` (glyph draw branch after the quad draw: set
coverage pipeline, bind `@group(1)`, VBO1 = glyph buffer, draw). Tests: the GPU
glyph/retint coverage (using `support::gpu_render_app` + `render_to_image`/`readback_rgba`,
the test as glyph producer) landed in the text crate (`text_gpu.rs`) when T8 took the
glyph producer; `atlas_gpu.rs` keeps the device-agnostic page-pool gate-#15 test.

## Verification

The GPU glyph/retint tests (now in `text_gpu.rs`) green on the RX 6700 XT via
`--ignored`; the headless gate (no `--ignored`) stays green (the new pipeline/shader
compile + the device-free allocator tests unchanged). No new runtime deps. The non-sampling quad/shadow
pipelines and their `@group(0)` descriptor stay byte-identical (additive `@group(1)`).

## Implementation notes (as landed)

- **Page CPU store is a plain `Vec<u8>`, not a Bevy `Image`.** The file list said
  "real CPU `Image` per page". As built, `AtlasPage` owns its texels as a tightly
  packed `Vec<u8>` (the same row-major layout `write_texture` consumes), not a
  `Handle<Image>`. This is *more* faithful to fork #1, not less: a `Handle<Image>`
  would re-introduce the `Assets<Image>` machinery fork #1 explicitly avoids, and
  a byte buffer is exactly what the blit and the §7/§4.1 byte-identity test read.
  `BuiyAtlas`/`AtlasPage` therefore stay device-free (the headless allocator tests
  need no adapter); the GPU `Texture`s live in the separate `AtlasGpu` render
  resource, populated in prepare via `write_texture` (forks #1 + #3).
- **~~v1 binds a single CoverageR8 page (page 0).~~ RESOLVED (2026-07-09,
  [multi-page coverage atlas bind](../2026-07-09-multipage-coverage-atlas-bind-design.md)).**
  `GlyphAlphaInstance.page` rode the instance for future multi-page selection, but the
  `@group(1)` bind group was built against page 0 only — so glyphs/icons spilling to
  page ≥1 sampled wrong texels and rendered blank. Now all resident coverage pages are
  bound as a `texture_2d_array` (forked from the raster/image layout) and the shader
  samples the per-instance layer via explicit-LOD; layer count grows to high-water.
- **(Glyph, layer) paint order is enforced structurally in the node.** Glyph
  instances flow through `ExtractedGlyphs` → `BuiyInstanceBuffers.glyph` (a typed
  `RawBufferVec<GlyphAlphaInstance>`), not through the `[f32;13]`-typed quad
  `InstanceBuckets`. `BuiyNode::run` draws quads then glyphs in one pass, giving
  the shadow < quad < glyph < path order without routing glyphs through the quad
  bucket store (`BuiyPrimitiveKind::Glyph::paint_order() == 2` is unchanged).
- **`maintain_atlas` (ExtractSchedule) runs `begin_frame` + `drain_grace_expired`
  + `collect_emptied_pages`** chained after `warmup_atlas`, so an idle fixture's
  transient entries drain and their emptied pages return to the pool — the GPU
  `Texture` is reused at the same page index (gate #15 / § 2.5).
