# Buiy render — atlas and the text seam

**Parent:** [README.md](README.md)

This file owns the F-tier texture atlas the render pipeline shares across all
coverage-and-image primitives (glyph / icon / gradient / generated-mask), and
the **seam** where text rendering plugs into it without this spec owning glyph
shaping. It elaborates the atlas half of [README § 2 pillar 3](README.md#2-architectural-pillars-one-line-summaries)
(the hybrid handoff: "persistent GPU buffers + atlas + view-uniform") and the
atlas half of gate #15 (README § 5 open #4). The companion primitive shapes that
sample the atlas — quad, shadow, path — live in [architecture.md](architecture.md)
§ "typed-primitive batched node"; this file adds the three atlas-sampling
primitives (glyph-alpha, icon/sprite, and the reserved gradient/mask).

It maps the foundation row [visuals.md § 3.3 "Texture atlases for glyphs, icons,
gradients, generated masks" (**F**)](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering)
and the render-side of the [text.md typography rows](../2026-05-07-buiy-foundation/text.md#34-typography).

The two production Rust GPU UIs this spec converges on both center an atlas:
GPUI's single-channel alpha glyph atlas + alpha-as-color trick
([prior-art/gpui/gpu-rendering.md § "Glyph atlas and the alpha-as-color trick"](../../prior-art/gpui/gpu-rendering.md)),
and WebRender's "rasterize glyphs into a texture atlas and batch the resulting
quads" ([prior-art/servo-stylo/rendering.md § "Brief note on Servo's text stack"](../../prior-art/servo-stylo/rendering.md)).
Buiy reuses Bevy 0.18's *existing* atlas machinery rather than inventing one.

---

## 1. The seam, stated once

The single most important contract in this file is **who owns the atlas vs. who
owns the glyphs**, so the atlas is not designed twice:

| Concern | Owner |
|---|---|
| The shared `TextureAtlas` render resource (allocation, warmup, eviction, pooling) | **this spec** |
| The glyph-alpha primitive *shape* (instance layout, shader contract, the alpha-as-color trick) | **this spec** |
| Icon/sprite primitive shape | **this spec** |
| Gradient + generated-mask atlas *entries* and their primitive shapes | **this spec** (component model + reserved entry kind; C-tier — see § 6) |
| Glyph **shaping** (cosmic-text `Buffer`, `harfrust` driving), line layout, font **fallback**, **BiDi** | `buiy-text-rendering-design` |
| Producing glyph **coverage bitmaps** (`swash` rasterization) and **inserting** them into this atlas | `buiy-text-rendering-design` |
| **Emitting** glyph-alpha primitives (one per visible glyph, with the atlas key + tint) | `buiy-text-rendering-design` |

Read the boundary as: **text rendering produces coverage and primitives; this
spec provides the warehouse they live in and the primitive *kind* they speak.**
`buiy-text-rendering-design` depends on the public API in § 2–§ 4 here; it does
**not** redefine `TextureAtlas`, the allocator, the eviction policy, or the
glyph-alpha instance layout. Conversely, this spec never imports cosmic-text,
never reasons about scripts, fonts, or shaping, and stores only *opaque coverage
bitmaps keyed by an opaque key* (§ 3).

> **`Visual.foreground_token` reservation moves to the text spec.** The Phase-0
> `Visual` carrier reserved a `foreground_token` field for text color
> ([README § 3.3](README.md#33-the-visual-migration)). That reservation is **not**
> owned here — it graduates into `buiy-text-rendering-design` as the per-glyph
> tint source the glyph-alpha primitive samples (§ 4.2). This file fixes only the
> *primitive's* tint slot, not where text color is authored or resolved.

---

## 2. The shared `TextureAtlas` render resource

### 2.1 Type and placement

The atlas is **one render-world resource** (not per-primitive, not per-window):
all coverage-and-image primitives sample the same texture so a single bind
group serves a frame's worth of glyphs, icons, gradients, and masks — the
GPUI/WebRender "share atlas binds across all instances" batching win
([prior-art/gpui/gpu-rendering.md § "Batching and draw call count"](../../prior-art/gpui/gpu-rendering.md)).

```rust
/// Render-world resource. One per `RenderApp`; lives in `BuiyRenderLabel`'s crate
/// (the same crate home pinned for the effect components — see architecture.md § crate placement).
#[derive(Resource)]
pub struct BuiyAtlas {
    /// One backing texture per `AtlasFormat`. Glyph/mask coverage is R8;
    /// icon/gradient is RGBA8. Distinct formats cannot share a page.
    pages: HashMap<AtlasFormat, Vec<AtlasPage>>,
    /// Key → where it lives. The seam's only handle (§ 3).
    entries: HashMap<AtlasKey, AtlasEntry>,
    /// LRU recency ring; evicted oldest-first under pressure (§ 2.4).
    lru: LruQueue<AtlasKey>,
    config: AtlasConfig,
}

struct AtlasPage {
    /// `guillotiere::AtlasAllocator` — Bevy 0.18's own dynamic-atlas allocator,
    /// a guillotine/shelf hybrid. Real API (bevy_image 0.18.1
    /// `dynamic_texture_atlas_builder.rs` wraps exactly this).
    allocator: guillotiere::AtlasAllocator,
    /// The CPU-side `Image`; uploaded to its `GpuImage` each frame it changes.
    texture: Handle<Image>,
    /// guillotiere AllocId → the URect it owns, so eviction can `deallocate(id)`.
    live: HashMap<AtlasKey, (guillotiere::AllocId, URect)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtlasFormat { CoverageR8, ColorRgba8 }
```

`AtlasEntry` is the value the seam reads back after an insert:

```rust
#[derive(Clone, Copy)]
pub struct AtlasEntry {
    pub page: u16,        // index into pages[format]
    pub format: AtlasFormat,
    pub uv: Rect,         // normalized [0,1] UV rect into that page
    pub px: URect,        // pixel rect, for the subpixel-snap math text needs
}
```

**Why reuse `guillotiere`, not invent an allocator.** The allocation strategy the
SCOPE asks for ("shelf/guillotine or similar") is *already decided for us*: Bevy
0.18.1's `DynamicTextureAtlasBuilder` (`bevy_image/src/dynamic_texture_atlas_builder.rs`)
is built on `guillotiere::AtlasAllocator`, a guillotine allocator with shelf-like
coalescing, and crucially it exposes `allocate(size) -> Option<Allocation>` (with
an `AllocId`) **and** `deallocate(AllocId)` — the deallocate path is what makes
LRU eviction (§ 2.4) buildable rather than clear-the-world. Buiy uses
`guillotiere` directly (it is already in the dependency tree via Bevy) and reuses
`DynamicTextureAtlasBuilder::add_texture` semantics for the place-and-blit step:
that function takes a source `&Image` and an `&mut Image` atlas page, writes the
coverage in, and returns a `usize` index into a `TextureAtlasLayout.textures:
Vec<URect>`. Buiy wraps this so the returned rect becomes an `AtlasEntry`.

> **Runner-up rejected:** a bespoke shelf-packer tuned for monospaced glyph rows.
> Rejected — `guillotiere` already handles the mixed glyph/icon/gradient size
> distribution, ships in-tree, and gives `deallocate` for free; a bespoke packer
> would re-solve a solved problem and forfeit the eviction primitive.

### 2.2 The page texture and its format

Per [prior-art/gpui/gpu-rendering.md § "Glyph atlas and the alpha-as-color trick"](../../prior-art/gpui/gpu-rendering.md):
glyph and mask coverage is **single-channel** (`AtlasFormat::CoverageR8`,
`TextureFormat::R8Unorm`); icon/sprite and baked gradient stops are **full
color** (`AtlasFormat::ColorRgba8`, `TextureFormat::Rgba8UnormSrgb`). They never
share a page — a `guillotiere` page is one format.

Each page's CPU `Image` is created with `RenderAssetUsages::MAIN_WORLD |
RenderAssetUsages::RENDER_WORLD` (the `add_texture` blit path asserts
`MAIN_WORLD`), and its `GpuImage` is the texture the batched node binds. A page
that gained entries this frame re-uploads; an unchanged page does not (the same
`Changed<T>`-gated discipline as the instance buffers — [README § 2 pillar 3](README.md#2-architectural-pillars-one-line-summaries)).
**F**

Default page size is 1024×1024; a new page is appended when the current page's
allocator returns `None` and eviction (§ 2.4) cannot free enough contiguous
space. Page count is the lever gate #15 watches.

### 2.3 Warmup

Gate #2 (visual-regression) lists "atlas warmup" as an explicit flake-mitigation
([foundation/verification.md gate #2](../2026-05-07-buiy-foundation/verification.md#ci-gates)):
a golden image must not differ because a glyph happened to land in the atlas one
frame later. Warmup is the contract that makes the first painted frame
deterministic.

```rust
/// Drains a queue of "insert this now" requests before the first paint of a
/// fixture, so steady-state and golden frames never race a cold atlas.
fn warmup_atlas(atlas: ResMut<BuiyAtlas>, requests: Res<AtlasWarmupQueue>) { /* … */ }
```

This spec owns the *mechanism* (a pre-paint drain that forces requested entries
resident and blocks the frame's first paint until they are). It does **not** own
*what* to warm for text — `buiy-text-rendering-design` decides which (font, size,
glyph) tuples to pre-rasterize (e.g. the ASCII range of the default theme font)
and pushes coverage into the queue via the § 3 insert API. Icon warmup (theme
icon set) is pushed the same way by whoever owns the icon registry. **F**

### 2.4 LRU eviction — the gate-#15 contract

Gate #15 requires "atlas entries return within ε of baseline" after a
long-running fixture goes idle ([foundation/verification.md gate #15](../2026-05-07-buiy-foundation/verification.md#ci-gates)):
no monotonic growth, no leak. The atlas satisfies this with **LRU eviction keyed
by last-use frame**:

1. Every frame, each primitive that samples an entry **touches** it
   (`lru.touch(key)`), moving it to the most-recently-used end.
2. On an allocation that fails (`allocate` returns `None`) **and** would push a
   page over budget, the allocator evicts least-recently-used entries —
   `page.allocator.deallocate(alloc_id)` for each, dropping them from `entries`
   and `live` — until the new entry fits or the LRU queue is empty (then a new
   page is appended).
3. An entry **untouched for `config.eviction_grace` frames** is eligible even
   without pressure, so an idle fixture's transient glyphs/icons drain back out.
   This is the clause that makes "return to baseline" hold: when a fixture stops
   exercising a glyph, that glyph's coverage leaves the atlas within the grace
   window, and the page's freed rects coalesce.

Eviction is **safe because the key is content-addressed** (§ 3): an evicted
entry that is needed again next frame is simply re-inserted from its source
bitmap — correctness never depends on residency, only performance does. This is
why eviction can be aggressive without a stale-paint bug (the failure mode
[README § 2 pillar 3](README.md#2-architectural-pillars-one-line-summaries) calls
out for retained scenes). **F**

> **Open — concrete budget numbers.** `config.page_budget`,
> `config.eviction_grace`, and the per-fixture "ε of baseline" tolerance are
> calibration owned by `buiy-verification-design` ([README § 5 open #4](README.md#5-open-questions)).
> This file commits the *eviction mechanism* so gate #15 is **satisfiable**; the
> numbers tune over time on the fixed runner.

### 2.5 Pooling

Page textures are **pooled, not freed**: when eviction empties a page entirely,
its `Image` handle and `guillotiere::AtlasAllocator` are reset (`allocator =
AtlasAllocator::new(size)`) and returned to a free list for the next page-growth
request, rather than dropped. This keeps RSS flat across the alloc/free cycles
gate #15 stresses (RSS slope < 1 MB/min) — the GPU texture is the expensive
object, so reusing it beats reallocating. Pooling is the atlas-side analogue of
the persistent-buffer pooling [architecture.md](architecture.md) owns for the
instance buffers. **F**

---

## 3. The insert/lookup API — the only handle the seam touches

`buiy-text-rendering-design` interacts with the atlas through exactly one pair of
calls. The key is **opaque to this spec**: text owns its meaning, the atlas only
hashes it.

```rust
/// Content-addressed, opaque to the atlas. The producer (text) defines what the
/// bytes mean; the atlas treats it as an Eq+Hash identity for dedup + eviction.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AtlasKey(pub SmallVec<[u8; 24]>);

impl BuiyAtlas {
    /// Idempotent. If `key` is resident, touch + return its entry (no blit).
    /// Else allocate (evicting LRU under pressure, § 2.4), blit `coverage` into
    /// the page, record the entry, and return it. `coverage` is an opaque
    /// single-format bitmap — the atlas never interprets it.
    pub fn get_or_insert(
        &mut self,
        key: AtlasKey,
        format: AtlasFormat,
        coverage: impl FnOnce() -> AtlasBitmap, // lazy: only rasterize on miss
    ) -> AtlasEntry { /* … */ }

    /// Residency probe for warmup/eviction tests; does not touch LRU.
    pub fn get(&self, key: &AtlasKey) -> Option<AtlasEntry> { /* … */ }
}

/// A CPU coverage/color bitmap handed to the atlas. `R8` for glyph/mask,
/// `Rgba8` for icon/gradient. The atlas wraps it as a Bevy `Image` for the blit.
pub struct AtlasBitmap { pub size: UVec2, pub format: AtlasFormat, pub data: Vec<u8> }
```

The `coverage` argument is a **closure** so that on the common path (glyph
already resident) the producer never rasterizes — the text spec passes a closure
that runs `swash` only on a miss. This keeps the expensive rasterization on the
*text* side of the seam (where the font machinery lives) while the *residency
decision* stays on the atlas side (where the allocator and LRU live). Neither
side reaches across.

For glyphs, `buiy-text-rendering-design` builds `AtlasKey` from its own identity
tuple — `(FontId, subpixel_bucket, glyph_id, px_size)`, mirroring GPUI's
`(font, size, glyph_id, subpixel_x_offset)` rasterization key — and that
construction is **its** concern, not this spec's. This spec only guarantees:
equal keys → one resident copy → one blit. **F**

---

## 4. Atlas-sampling primitives

The batched node in [architecture.md](architecture.md) renders a small typed set.
This file owns the three that *sample the atlas* (the quad/shadow/path
non-sampling primitives are architecture.md's). All three are instanced and
batched per (primitive-type, page, layer); they share the one atlas bind group.

### 4.1 The glyph-alpha primitive — the alpha-as-color trick

This is the centerpiece of the seam. Per
[prior-art/gpui/gpu-rendering.md](../../prior-art/gpui/gpu-rendering.md): the
atlas stores **single-channel coverage** (`R8`), and **color is applied
per-instance** — "storing alpha means one copy serves any tint, and theme color
changes don't require atlas regeneration." Buiy adopts this exactly.

```rust
/// One instance per visible glyph (or any single-channel coverage quad, e.g. a
/// generated mask stamp). Emitted by buiy-text-rendering-design; consumed by the
/// batched node. The shape is owned here so text and render cannot drift.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphAlphaInstance {
    pub rect: [f32; 4],     // screen-space x, y, w, h (post-bridge GlobalTransform-resolved)
    pub uv: [f32; 4],       // CoverageR8 page UV from AtlasEntry.uv
    pub color: [f32; 4],    // linear-light premultiplied tint (the "alpha as color" value)
    pub clip: [f32; 4],     // ClipRect, the GPUI per-instance clip (clip-and-transform.md)
    pub page: u32,          // which CoverageR8 page → selects the bind slot
}
```

The fragment shader is one line of intent: `coverage = textureSample(atlas_r8,
uv).r; out = color * coverage;` — the alpha sampled from the atlas modulates the
per-instance linear-light color. A theme color change re-emits instances with a
new `color`; **the atlas is never touched**. This is what makes the
`foreground_token`→tint path (whose authoring moves to the text spec, § 1) a
pure per-instance value, never an atlas key input.

The glyph-alpha primitive is **not text-specific**: any single-channel coverage
stamp uses it (a generated mask, § 6). Text is its first and primary producer.
**F**

### 4.2 Icon / sprite primitive

For full-color content — themed raster icons, color emoji glyph bitmaps that
`swash` produces as `Rgba8` (COLRv0/CBDT, per
[prior-art/cosmic-text/README.md](../../prior-art/cosmic-text/README.md) key
facts) — there is **no recolor trick**: the atlas stores the color, the
primitive samples it straight. This mirrors GPUI's `PolychromeSprite`
([prior-art/gpui/gpu-rendering.md § "The primitive set"](../../prior-art/gpui/gpu-rendering.md)):
"colored sprite … indexing into a full-color atlas."

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct IconInstance {
    pub rect: [f32; 4],
    pub uv: [f32; 4],       // ColorRgba8 page UV
    pub tint: [f32; 4],     // multiplied over the sampled color (1,1,1,1 = no tint)
    pub clip: [f32; 4],
    pub page: u32,
}
```

A color-emoji glyph is therefore an `IconInstance` keyed into a `ColorRgba8`
page, while a monochrome glyph is a `GlyphAlphaInstance` keyed into a
`CoverageR8` page. The text spec decides which kind a given shaped glyph is (it
knows whether `swash` returned coverage or color); this spec provides both
primitive shapes and both page formats so the decision has a home on either
side. **F**

### 4.3 Subpixel snapping note

The atlas stores one coverage bitmap per subpixel bucket (the text spec's key
includes a `subpixel_bucket`), and `AtlasEntry.px` exposes the pixel rect so the
text spec can position the quad on the correct subpixel-snapped origin. This
spec carries the **pixel rect through**; it does not compute snap offsets — that
is layout/text positioning, upstream of the primitive.

---

## 5. Where text plugs in (the seam, mechanically)

End-to-end, for one frame of text, with the boundary marked:

1. **[text spec]** cosmic-text shapes a `Buffer` → laid-out `LayoutGlyph`s with
   positions, font ids, subpixel buckets. (Shaping, fallback, BiDi — entirely
   the text spec's, [text.md § 3.4](../2026-05-07-buiy-foundation/text.md#34-typography).)
2. **[text spec]** For each glyph, build `AtlasKey` and call
   `atlas.get_or_insert(key, format, || swash_rasterize(...))`. On a miss the
   closure rasterizes; on a hit nothing rasterizes. **[this spec]** allocates,
   evicts LRU if needed, blits, returns the `AtlasEntry`.
3. **[text spec]** Emit one `GlyphAlphaInstance` (or `IconInstance` for color
   glyphs) per glyph, filling `uv`/`page` from the `AtlasEntry`, `color` from the
   resolved text color (the migrated `foreground_token` path), `rect` from the
   `GlobalTransform`-resolved glyph box, `clip` from `ClipRect`.
4. **[this spec]** The batched node groups instances by (type, page, layer),
   binds the one atlas, and issues one instanced draw per group — the
   single-digit-draw-call shape ([prior-art/gpui/gpu-rendering.md § "Batching"](../../prior-art/gpui/gpu-rendering.md)).

The seam is exactly steps 2 and 3's API surface: `get_or_insert` / `AtlasEntry`
inbound, `GlyphAlphaInstance` / `IconInstance` outbound. Everything left of that
is the text spec; everything right is this spec. **No type crosses the boundary
except the ones defined in this file.**

---

## 6. Gradient and mask entries — C-tier, reserved

Gradients and generated masks ride the **same atlas** but are deferred: the
foundation tiers gradients (`background-image` gradients) and the `mask` family
at **C** ([visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering)),
and [README § 2.6 / § 1 non-goals](README.md#1-goals-and-non-goals) defers their
shaders. What ships **now** is the *reservation* so the atlas is not re-architected
when they land:

- **Gradient (C, reserved).** A linear/radial/conic gradient with baked color
  stops rasterizes once into a `ColorRgba8` strip and becomes an atlas entry
  sampled by the quad primitive ([architecture.md](architecture.md)) as its
  background source. The atlas key is the gradient's stop list + geometry; the
  same `get_or_insert` + LRU machinery applies unchanged. Reserved alongside the
  `Background` component's gradient fields ([README § 3.2](README.md#32-render-owned-this-spec-introduces):
  "`Background` … reserved layered/gradient fields (C)"). **No baking shader in
  v1.**
- **Generated mask (C, reserved).** A `clip-path`/`mask-image` coverage mask
  rasterizes into a `CoverageR8` entry and is sampled exactly like a glyph — it
  *is* a `GlyphAlphaInstance` with a mask key (§ 4.1 notes the primitive is not
  text-specific). Reserved against the (deferred) clip-path/mask path that
  [clip-and-transform.md](clip-and-transform.md) and the effect components own.
  **No mask-generation shader in v1.**

Reserving these as atlas *entry kinds* (not new resources) is the simplification
that keeps one atlas, one allocator, one eviction policy, one bind group. When
the C-tier shaders land they add a *producer* and a *key constructor*; they add
no atlas machinery. **C (reserved)**

---

## 7. Verification

Per [foundation/verification.md](../2026-05-07-buiy-foundation/verification.md),
the atlas is provable at two layers:

- **Headless, no GPU (gate #1 unit, #15 leak).** The allocator + LRU + pooling
  logic is pure CPU: `guillotiere::AtlasAllocator` and the `entries`/`lru` maps
  need no wgpu adapter. Tests assert (a) `get_or_insert` is idempotent (second
  call with an equal key blits zero bytes and returns the same `AtlasEntry`);
  (b) under forced pressure the LRU evicts the least-recently-touched entry and
  the freed `AllocId` is `deallocate`d; (c) after a scripted insert→idle cycle
  the live-entry count returns within ε of baseline and page count does not grow
  monotonically — the **headless half of gate #15** (the RSS half needs the
  running fixture). Pooling is asserted by checking an emptied page's texture
  handle is reused, not reallocated.
- **On GPU, golden image (gate #2).** Warmup determinism is proven by the
  visual-regression harness: a fixture's first painted frame matches its golden
  because `warmup_atlas` forced residency pre-paint. The alpha-as-color trick is
  proven by re-tinting the same glyph across two themes and asserting the atlas
  texture is byte-identical between them (only the instance `color` differs).

The seam itself is verified by a contract test in `buiy-text-rendering-design`
that drives `get_or_insert` + emits `GlyphAlphaInstance`s against a stub atlas —
asserting this spec's public API is sufficient to render text without any
cosmic-text type crossing into the render crate.

---

*Scope: the shared `TextureAtlas` render resource (allocation via
`guillotiere`, warmup, LRU eviction, pooling), the glyph-alpha + icon
primitives, and the reserved gradient/mask entry kinds. The atlas is F-tier;
gradient/mask entries are C-tier reserved. Glyph shaping/layout/fallback/BiDi and
coverage-bitmap production belong to `buiy-text-rendering-design`, which consumes
the § 3 insert API and emits the § 4 primitives. `Visual.foreground_token`'s
reservation moves to that spec.*
