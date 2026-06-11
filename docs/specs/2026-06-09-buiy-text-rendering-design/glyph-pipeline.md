# Buiy text rendering — the glyph pipeline (producer side of the atlas seam)

**Parent:** [README.md](README.md)

This file owns the **glyph producer**: the per-frame flow from a shaped
cosmic-text `Buffer` to `GlyphAlphaInstance`s in the render world — replacing
the test-filled `ExtractedGlyphs` slot
(`crates/buiy_core/src/render/prepare.rs:46-51`, today populated only by
`tests/atlas_gpu.rs:54-63`'s `set_glyphs` test-as-producer) with a real system.
It realizes, with verified cosmic-text 0.19.0 types, the five-step seam table in
[render atlas-and-text-seam.md § 5](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#5-where-text-plugs-in-the-seam-mechanically).

The consumer side of that seam is **built and GPU-verified** and is *not*
redesigned here: `BuiyAtlas::get_or_insert` + `AtlasWarmupQueue` + `CoverageR8`
pages (`crates/buiy_core/src/render/atlas/atlas.rs:58-76`), the 68 B `#[repr(C)]`
`GlyphAlphaInstance` (`render/atlas/primitive.rs:30-48`, offsets compile-asserted
at `:72-79`), the independent glyph-buffer damage gate (`prepare.rs:207-216`),
and the live glyph draw branch in paint order shadow < quad < glyph
(`render/node.rs:263-293`). This file designs only what fills them.

What this file does **not** own: shaping, `Buffer` lifecycle, and the
measure-into-Taffy seam ([measure-and-layout.md](measure-and-layout.md)
— this file's input contract is *"a shaped `Buffer` readable at extract"*, § 2
step 0); font registration and fallback
([font-assets.md](font-assets.md)); `FontSystem`/
`SwashCache` ownership and scheduling
([architecture.md](architecture.md) — consumed here per § 3);
decoration/selection/caret painting
([decoration-and-paint.md](decoration-and-paint.md); § 10 names the quad
seam). BiDi is inherited: `layout_runs` yields glyphs in visual order, so the
producer is direction-blind by construction.

---

## § 1 F-tier scope and the v1 slice

This file owns the render half of the foundation's typography F rows
([foundation text.md § 3.4](../2026-05-07-buiy-foundation/text.md#34-typography)):
the paint path that makes `font-family`/`font-size`/`font-weight` visible
pixels, the producer half of "texture atlases for glyphs", text-color delivery
(the graduated `Visual.foreground_token`, § 7), and the producer-side
obligations of gates #2 (first-frame determinism + retint byte-identity), #14
(retention/damage), and #15 (LRU touch + key hygiene).

**v1 slice:** monochrome `SwashContent::Mask` glyphs only; one window (D2); one
`TextColor` token per entity (`color_opt` honored when present); `Shaping::Advanced`
pinned; 4-bin x-subpixel; physical-px rasterization; single `CoverageR8` page
(warn at page-1 allocation, § 11); wholesale rebuild on text damage + un-gated
touch pass; no warmup-queue use; no color emoji; no decorations.
**Deliverables:** `extract_buiy_glyphs` + `FontKeyInterner` + `ResidentTextKeys`
+ `TextColor`, deleting the test fill in `tests/atlas_gpu.rs` in favor of
real-entity fixtures. **Exit criterion:** a `hello_text` example renders a themed
paragraph whose golden passes gate #2 and whose retint run leaves the atlas
byte-identical (§ 12).

## § 2 One frame, end to end

The producer's anatomy, per visible text entity in `painters_z` order (the same
order discipline as `extract_buiy_nodes`):

| Step | What happens | Types (verified, cosmic-text 0.19.0) |
|---|---|---|
| 0 | **Input contract** (sibling-owned): the main world has already shaped the entity's `Buffer` for its final wrap width — `shape_until_scroll(&mut FontSystem, _)` ran in `BuiySet` before extract ([measure-and-layout.md](measure-and-layout.md)) | `Buffer` is `Send + Sync`; read at extract via the read-only `TextBufferAccess` form ([measure-and-layout.md § 2.3](measure-and-layout.md) — editable entities' authoritative `Buffer` lives in `TextEditState`, and the accessor dispatches editor-first, so the producer never binds bare `&TextBuffer`; `layout_runs` is `&self`) |
| 1 | Walk `buffer.layout_runs()` → per-run glyph slices | `LayoutRun { glyphs: &[LayoutGlyph], line_y, … }` |
| 2 | Quantize each glyph to a physical position + cache key: `glyph.physical(offset_physical_px, scale_factor)` (§ 5.1) | `PhysicalGlyph { cache_key: CacheKey, x: i32, y: i32 }` |
| 3 | Build the `AtlasKey` from the `CacheKey` fields via the `FontKeyInterner` (§ 4) | `AtlasKey(SmallVec<[u8; 24]>)`, ≤ 19 B inline |
| 4 | `atlas.get_or_insert(key, CoverageR8, rasterize_closure)` — the closure runs **only on a miss**, locking the `SharedFontSystem` and calling `SwashCache::get_image_uncached` (§ 3) | `AtlasEntry { page, format, uv, px }` |
| 5 | Emit one `GlyphAlphaInstance`: `rect` from the § 5.2 formula, `uv`/`page` from the entry, `color` from § 7, `clip` from § 8 — pushed into `ExtractedGlyphs` in paint order | `GlyphAlphaInstance` (`primitive.rs:30-48`) |

Whitespace and other zero-coverage glyphs (rasterizer returns `None` or a
zero-area `Placement`) emit no instance and insert nothing.

Everything downstream is the frozen consumer: `prepare_buiy_instances` packs
`ExtractedGlyphs` into the persistent glyph buffer when `glyphs.is_changed()`
(`prepare.rs:207-216`), `prepare_atlas_textures` uploads dirty pages
(`render/atlas/gpu.rs:143-207`), the node draws (`node.rs:281-293`).

## § 3 Engine access and rasterization

### § 3.1 `SharedFontSystem` lock discipline (consumed, not owned)

[architecture.md](architecture.md) owns the decision: one
`SharedFontSystem(Arc<Mutex<FontSystem>>)` resource cloned into both worlds
(legal — `FontSystem` is verified `Send + Sync` in 0.19; the prior-art folder's
"non-Sync" note and its swash 0.2.6 figure are version drift from older releases
and are superseded — 0.19.0 resolves swash 0.2.7). The pinned invariant this
file *consumes*: the main world locks around measure/shape; **render-world code
locks only inside atlas miss closures** (§ 2 step 4). Those closures run in
`ExtractSchedule` — pipelined rendering's sync window, where the main world is
parked — so the lock is uncontended and deadlock-free by construction, and a
hit-only frame (every visible glyph resident) takes **zero** locks.

One shared `FontSystem` is load-bearing for the key scheme: `CacheKey.font_id`
is a `fontdb::ID` into one database. Two `FontSystem`s (main for shaping, render
for raster) would produce incoherent ids between the two halves, plus double
font memory — rejected in the engine file; restated here because § 4's interner
silently depends on it.

### § 3.2 Rasterize with `get_image_uncached` — one cache, not two

**Decision.** Inside the miss closure, rasterize via
`SwashCache::get_image_uncached(&mut FontSystem, cache_key) -> Option<SwashImage>`
(owned result, no internal insert). The render-world `SwashCache` resource
(placement owned by [architecture.md](architecture.md)) is kept
solely for its internal scale context; its caching path (`get_image`) is never
used. The `Mask` bitmap becomes
`AtlasBitmap { size: (placement.width, placement.height), format: CoverageR8, data }`.

**Rationale.** `BuiyAtlas` is already a content-addressed, LRU-bounded,
gate-#15-audited bitmap cache. `get_image` would double-store every bitmap in
SwashCache's monotonically growing `HashMap` — no eviction, no trim API, exactly
the leak the cosmic-text prior-art flags
(`docs/prior-art/cosmic-text/lessons.md`, "Buiy must add its own LRU"). One
cache, one eviction policy, one budget — a simplification cascade.

**Rejected runner-up:** `get_image` (SwashCache as a second CPU-side cache).
Cost of its absence: a glyph evicted after 60 idle frames
(`AtlasConfig::eviction_grace`, `render/atlas/types.rs:64-72`) re-rasterizes on
return — acceptable by the definition of idle. Cost of its presence: unbounded
memory growth under churn.

## § 4 The `AtlasKey` byte scheme

**Decision.** Structured, fixed-layout bytes (little-endian), built from the
verified `CacheKey` fields plus a kind discriminant:

```text
[kind: u8 = AtlasEntryKind::Glyph][font: u32 interned][glyph_id: u16]
[font_size_bits: u32][x_bin: u8][y_bin: u8][weight: u16][flags: u32]  = 19 B
```

A `FontKeyInterner` render-world resource maps `fontdb::ID` → sequential `u32`
(monotonic, never evicted — fonts number in the dozens). The leading kind byte
partitions the opaque key space against the other producers `AtlasEntryKind`
reserves (`types.rs:83-93`): a future Icon/Gradient/Mask key can never alias a
glyph key. 19 B fits `AtlasKey`'s `SmallVec<[u8; 24]>` inline capacity
(`types.rs:16`), so the hot path never heap-allocates. `weight` and `flags` are
in the key because both are shape-affecting `CacheKey` inputs; `y_bin` is
carried even though § 5.1 makes it structurally zero — one byte buys layout
stability if vertical binning ever changes. This realizes the seam doc's
`(FontId, subpixel_bucket, glyph_id, px_size)` sketch
([seam § 3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#3-the-insertlookup-api--the-only-handle-the-seam-touches))
with the real field set.

**Rejected runner-ups.** (a) *Hash the whole `CacheKey` to a `u64`* — content
addressing requires **equality**, not hashing; a collision silently aliases two
glyphs' coverage, an unfindable rendering bug. (b) *Serialize `fontdb::ID`'s
internal repr directly* — the repr is private and version-fragile; the interner
costs one `HashMap` lookup and survives fontdb upgrades.

## § 5 Subpixel and hiDPI math

### § 5.1 Adopt `physical()`'s binning verbatim

**Decision.** Quantize with cosmic-text's own
`LayoutGlyph::physical(offset, scale)`, passing the text entity's content
origin in **physical px** folded with the run baseline:
`glyph.physical((origin.x * scale, (origin.y + run.line_y) * scale), scale_factor)`.
The verified body (docs.rs 0.19.0 `src/layout.rs`) applies `offset` **after**
scale — i.e. offset is physical px — and `truncf`s the y coordinate **before**
binning, so `y_bin` is structurally `SubpixelBin::Zero` while x gets honest
4-bin subpixel treatment (`SubpixelBin { Zero, One, Two, Three }`). Atlas
fan-out is therefore bounded at ≤ 4 variants per (font, size, weight, glyph).

**Rationale.** Flooring to whole pixels (runner-up: zero both bins) causes
visible inter-glyph spacing jitter at fractional advances and fractional hiDPI
scales — the artifact GPUI's `(font, size, glyph_id, subpixel_x_offset)` key
exists to prevent, and the seam doc already budgets a `subpixel_bucket`
([seam § 4.3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#43-subpixel-snapping-note)).
The retint byte-identity and alpha-as-color contracts are unaffected: tint is
per-instance, never a key input, so two themes hit byte-identical pages.
**Rejected outright:** subpixel-RGB (`SwashContent::SubpixelMask`) — it cannot
live in an R8 page and breaks alpha-as-color structurally. The producer never
requests it and `debug_assert!(image.content != SwashContent::SubpixelMask)`.

### § 5.2 The rect formula — rasterize physical, position logical

**Decision.** Rasterize at `font_size × scale_factor` (the `physical()` call
already bakes the scale into `CacheKey.font_size_bits`). Per glyph:

```text
rect_px      = (phys.x + placement.left,  phys.y - placement.top,
                placement.width,          placement.height)
rect_logical = rect_px / scale_factor     → GlyphAlphaInstance.rect
```

`Placement { left, top, width, height }` offsets are relative to the glyph
origin with **top pointing up** (hence the subtraction); `line_y` is already
folded into the `physical()` offset (§ 5.1). `uv`/`page` copy from the returned
`AtlasEntry`; `AtlasEntry.px` (`types.rs:43-44`) exists precisely for this snap
math.

**Rationale.** `coverage.wgsl:35` fixes `rect` in logical px, and the view
uniform's logical→clip map is exact-linear carrying `scale_factor`
(`prepare.rs:77-89`, `coverage.wgsl:51-53`) — so a physical-grid-aligned rect
divided by `scale_factor` lands back on the same physical texels: crisp text
under the pinned Nearest/ClampToEdge sampler (`render/atlas/gpu.rs:62-73`).
**Rejected runner-up:** rasterize at logical size and let the GPU scale the
quad — bilinear-stretched coverage is visibly blurry on hiDPI, and the nearest
sampler would render it blocky instead.

## § 6 `extract_buiy_glyphs` — placement, damage gate, retention

### § 6.1 Where it runs

**Decision.** A render-world system in `ExtractSchedule`,
`.after(maintain_atlas)` (the `warmup_atlas → maintain_atlas` chain at
`render/atlas/mod.rs:103`). Signature shape:
`ResMut<BuiyAtlas>` + `ResMut<ExtractedGlyphs>` + `ResMut<FontKeyInterner>` +
`ResMut<ResidentTextKeys>` (render-world, mutable — legal in extract) +
`Extract<Query<…>>` over the main-world text entities, binding the read-only
`TextBufferAccess` form ([measure-and-layout.md § 2.3](measure-and-layout.md)
— never bare `&TextBuffer`, which on editable entities is the display
component, not the editor-owned authoritative `Buffer`; `layout_runs` is
`&self`, so read-only suffices) + the `SharedFontSystem`/`SwashCache` handles
of § 3.
Ordering after `maintain_atlas` means inserts and touches use the
just-advanced frame clock — the same reasoning that ordered the existing chain
(`atlas/mod.rs:99-103`).

**Rationale.** Only extract gives all three at once: a **mutable atlas** (the
residency decision stays atlas-side, the lazy-closure contract works as
designed), **pre-paint timing** (insert before Prepare's upload and the node's
draw — the structural gate-#2 determinism § 6.4 leans on), and a **parked main
world** (the pipelining sync point), which makes the `FontSystem` lock
contention-free. **Rejected runner-ups.** (a) *Main-world producer + staging
resource copied at extract* — the main world cannot answer residency, so it
needs a mirror `HashSet` plus a render→main eviction-feedback channel: two
sources of truth for one cache, and the lazy closure dies. (b) *A
`RenderSystems::Prepare` system* — rejected outright: under pipelined rendering
the `Render` schedule runs in parallel with the **next** main frame, so locking
the shared `FontSystem` there races frame N+1's shaping.

### § 6.2 The damage gate — a dedicated trigger union, retention on steady state

**Decision — this union is THE normative trigger set
([architecture.md § 5.1](architecture.md) row 3 is a summary deferring
here).** Mirror the `extract_buiy_nodes` retention pattern exactly
(`render/extract.rs:276-431`; design:
[2026-06-07-render-extract-retain-damage-design.md](../2026-06-03-buiy-render-pipeline-design/2026-06-07-render-extract-retain-damage-design.md)):
an **un-gated full fan** over visible text entities, plus an entity-only
`Changed` probe as the *whether-to-rebuild* signal —
`Or<(Changed<ComputedTextLayout>, Changed<GlobalTransform>, Changed<ResolvedLayout>,
Changed<TextColor>, Changed<ClipRect>, Changed<AncestorClip>,
Changed<ComputedPaintSkip>, Changed<Stacking>, Changed<CaretVisual>,
Changed<SelectionVisual>)>` (`Changed<TextBuffer>` is
deliberately **not** in the union — measure/commit writes bypass `TextBuffer`
ticks, [measure-and-layout.md § 7](measure-and-layout.md); the idempotent
`ComputedTextLayout` write is the text-changed signal,
[architecture.md § 3.3](architecture.md); `CaretVisual`/`SelectionVisual` are
the render-prep-written editor visual state,
[decoration-and-paint.md § 6.3](decoration-and-paint.md) — a caret-blink edge
fires here) — plus
`RemovedComponents<ResolvedLayout>` (despawn),
`RemovedComponents<ComputedPaintSkip>` (hide→show), `theme.is_changed()`
(text-color tokens re-resolve, § 7), and the **scale-factor probe — a value
compare, never `Changed<Window>`**: the producer retains the last-seen scale
factor (a cached `f32` beside its other retained state — `ResidentTextKeys`,
§ 6.3) and folds "the primary window's `scale_factor()` actually differs
from the cached value" into the union, updating the cache when it fires (the
scale source is the one [architecture.md § 6](architecture.md) names — a
scale change re-keys every glyph, so it must rebuild; the first frame
rebuilds regardless via the `Added`/`Changed` fan, which seeds the cache).
A `Changed<Window>` probe is **wrong** here, not merely unidiomatic:
bevy_winit writes `Window.physical_cursor_position` on every `CursorMoved`,
so the `Window` component's change tick fires on every mouse-move frame — a
tick probe would rebuild `ExtractedGlyphs` + `ExtractedTextQuads` and
re-upload the GPU glyph/quad buffers while the user merely moves the cursor,
violating this section's own O(0) steady-state contract and gate #14. On
fire: rebuild the whole
`ExtractedGlyphs` vec (§ 2 walk) **and `ExtractedTextQuads` alongside it**
([decoration-and-paint.md § 4.6](decoration-and-paint.md) — same producer,
same probe, one damage decision). On steady state: **return without touching
either resource**, so `glyphs.is_changed()` stays false in prepare
(`prepare.rs:209`) and the GPU glyph buffer is retained — the same O(0)
steady state as `extract.rs:429-431`.

**Rationale.** **Rejected runner-up:** riding `extract_buiy_nodes`' gate (one
shared damage signal) — rejected by the as-built prepare contract:
`prepare.rs:166-168` gates quad and glyph buffers **independently** so a
caret/retint frame re-uploads only glyphs; collapsing the gates would re-upload
quads on every text edit and vice versa. *Ungated per-frame rebuild* wastes the
gate-#14 budget the retention design protects. v1 keeps wholesale
rebuild-on-any-text-damage (per-entity patching is the same deferred
optimization architecture.md § 3.1 names for nodes).
**Rejected runner-up (scale-probe form):**
`Extract<MessageReader<WindowScaleFactorChanged>>` — the message-stream
equivalent of the value compare, and equally immune to the cursor-position
ticks. Rejected in favor of the cached-`f32` compare: a reader adds
message-timing subtlety (reader state must persist in the render world and
never miss a message across the extract boundary), while the value compare is
self-contained per frame and mirrors the explicit-signal style the union
already uses (`theme.is_changed()`). Either form is correct; the compare is
the normative one.

### § 6.3 The un-gated touch pass — the eviction-under-retention hazard

**Hazard (real and silent).** Retained glyph buffers reference atlas UVs, but a
retained frame never calls `get_or_insert` — so after
`eviction_grace = 60` untouched frames (`types.rs:64-72`,
`atlas.rs:105-110`) the atlas would evict still-painted glyphs. Eviction does
**not** clear texels (the blit happens only on insert —
`atlas.rs:113-125` frees the cell, `allocate_and_record` blits), so the failure
mode is a *later* insert overwriting a cell still referenced by a live
instance: corrupted glyphs, no error.

**Decision.** `extract_buiy_glyphs` maintains a
`ResidentTextKeys(Vec<AtlasKey>)` resource, rebuilt alongside `ExtractedGlyphs`
on damage; an **un-gated** per-frame pass (every frame, including retained
ones) calls `atlas.touch_existing(key)` for each — implementing
[seam § 2.4 step 1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#24-lru-eviction--the-gate-15-contract)
("every frame, each primitive that samples an entry touches it") literally;
`touch_existing` (`atlas.rs:97-101`) was built for exactly this caller. Cost:
O(visible glyphs) hash lookups per frame — microseconds at productivity-app
scale. Runs in the same system, after `maintain_atlas` so touches use the
fresh frame clock. The 1×1 solid-stamp sentinel key
([decoration-and-paint.md § 4.3](decoration-and-paint.md)) joins
`ResidentTextKeys` whenever any stamp instance (caret, line-through — any
§ 4.2 stamp seat) is live in `ExtractedGlyphs`: a retained caret idling past
`eviction_grace` would otherwise lose the stamp cell to reuse, and the
retained instances' UVs would sample someone else's glyph.

**Rejected runner-ups.** (a) *Pin/refcount semantics in `BuiyAtlas`* —
duplicates the LRU with a second residency mechanism and changes a seam
contract the render side already shipped and GPU-tested; scope creep on a
frozen seam. (b) *`eviction_grace = ∞` for glyph entries* — breaks gate #15
(idle text that despawns would never drain back to baseline).

### § 6.4 Warmup queue: not used for correctness

**Decision.** v1 pushes nothing into `AtlasWarmupQueue`. The gate-#2 "atlas
warmup" flake mitigation is satisfied **structurally**: the producer inserts in
`ExtractSchedule`, before Prepare's upload and the node's draw, so a fixture's
first painted frame already has every visible glyph resident — there is no
cold-atlas race to warm against. `AtlasWarmupQueue` remains the seam's named
hook for an **optional** ASCII/default-font pre-warm (push
`AtlasWarmupRequest { key, format, bitmap }`, `render/atlas/warmup.rs:11-15`) —
a first-keystroke-latency optimization owned alongside the `FontSystem`
startup-cost work ([architecture.md](architecture.md); prior-art
flags `FontSystem::new` slowness, cosmic-text issue #505).

**Rejected runner-up:** mandatory ASCII pre-warm in phase 1 — rasterizes ~95
glyphs × sizes × fonts speculatively, couples the producer to theme font/size
enumeration that belongs to [font-assets.md](font-assets.md),
and buys nothing for goldens (determinism already holds).

## § 7 Text color — `TextColor` token, resolved at extract, straight alpha

**Decision.** A render-owned `TextColor(ColorToken)` component — the graduated
`Visual.foreground_token` reservation the seam doc explicitly hands to this
spec ([seam § 1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#1-the-seam-stated-once)).
Resolved at extract exactly like `Background`:
`render::color::resolve_token` (`render/color.rs:127`; precedent
`extract.rs:105-126`), CPU-linearized exactly as the quad path does
(`LinearRgba::from`, `render/instance.rs:65`), written **straight-alpha** into
`GlyphAlphaInstance.color`. When `LayoutGlyph.color_opt` is `Some` (cosmic-text
carries per-span `Attrs` color through shaping), it overrides the entity token
per-glyph — rich-text spans ride through with zero extra plumbing (the span
*authoring* model is C-tier, owned by the sibling files; the producer just
honors the field).

**Rationale and rejected runner-ups.** (a) *Bake resolved color into the atlas
key/bitmap* — rejected by the seam's central contract: alpha-as-color exists so
"a theme color change re-emits instances; **the atlas is never touched**", and
the § 12 retint byte-identity test enforces it. (b) *Shader-side token resolve*
— a new uniform-table mechanism with no precedent in the as-built pipeline;
`theme.is_changed()` already re-fires the § 6.2 gate so token changes re-emit
instances the same frame, matching quads. Straight-alpha (NOT premultiplied) is
non-negotiable: `primitive.rs:35-42` documents the producer obligation and
`coverage.wgsl:88` scales only alpha — premultiplying would double-dim
semi-transparent glyphs.

## § 8 Glyph clip — self-inclusive `ClipRect`

**Decision.** `GlyphAlphaInstance.clip` is filled from the text entity's own
computed `ClipRect` — **self-inclusive**, because glyphs are *content*: text
overflowing its container (`white-space: nowrap` in a fixed-width box, the
canonical case) must be cut by the container's own overflow clip, as well as by
ancestors. `±INFINITY` sentinel when unclipped; top-layer members forced
unclipped per the `Stacking` rule quads use. The fan binds
`ClipRect` + `AncestorClip` + `Stacking` exactly as `extract.rs:303-309` does
for this resolution. The encoding is fixed by the consumer: logical-px AABB,
fragment discard in `coverage.wgsl:81-83` — the producer only fills the slot.

**Rejected runner-up:** `AncestorClip`-only (correct for an entity's *own*
background/outline, which must not be clipped by its own overflow) — would let
glyphs escape their own scrollport. The formal self-vs-ancestor rule statement
stays owned by
[render clip-and-transform.md](../2026-06-03-buiy-render-pipeline-design/clip-and-transform.md);
this section pins only the v1 producer behavior.

## § 9 Color emoji — the C-tier seam, named not built

**Decision.** v1 renders `Mask`-content glyphs only. The producer branches on
the verified `SwashContent` discriminant:

| `SwashContent` | v1 behavior | Target (C-tier) |
|---|---|---|
| `Mask` (8-bit alpha) | R8 `AtlasBitmap` → `GlyphAlphaInstance` | — (shipped, F) |
| `Color` (32-bit RGBA) | **skip + rate-limited `warn!`** | `ColorRgba8` page + `IconInstance` (key kind byte `Icon`, same `get_or_insert`) |
| `SubpixelMask` | `debug_assert!` unreachable — never requested (§ 5.1) | never |

Emoji/ZWJ/variation selectors are C-tier in the foundation inventory
([foundation text.md](../2026-05-07-buiy-foundation/text.md)), and the GPU
consumer half genuinely does not exist: `prepare_atlas_textures` uploads
`CoverageR8` pages only (`gpu.rs:150`), there is no icon pipeline octet and no
`IconInstance` buffer in `BuiyInstanceBuffers`. The atlas **CPU** side is
already reserved (`ColorRgba8` pages, `AtlasEntryKind::Icon`,
`types.rs:83-104`; `IconInstance` shape, `primitive.rs:54-64`), so the C-tier
landing adds only a producer branch + the GPU upload/pipeline/draw pieces the
render spec's
[seam § 4.2](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#42-icon--sprite-primitive)
assigns to itself — no atlas re-architecture. COLRv1 stays gapped upstream
regardless (`docs/prior-art/cosmic-text/README.md`).

**Rejected runner-ups.** (a) *Build the full color path now* — render-side
scope inside a text phase. (b) *Grayscale-downconvert into R8* — a gray
flag/emoji is worse than a tofu-visible gap, and it would bake wrong pixels
into a content-addressed cache.

## § 10 Decorations — a quad seam, not a glyph one

`LayoutRun.decorations` (`DecorationSpan`, new in 0.19) feeds underline /
strikethrough geometry. Underline/overline/selection rects are **quad
instances** carried by `ExtractedTextQuads`
([decoration-and-paint.md § 4.6](decoration-and-paint.md)) — emitted by this
same producer at extract, packed into the **existing** quad instance buffer,
never touching the atlas or `ExtractedGlyphs`; line-through and the caret are
glyph-tier solid stamps and *do* ride `ExtractedGlyphs`
(decoration-and-paint § 4.2). Design owned by
[decoration-and-paint.md](decoration-and-paint.md); named here only so
no one routes a quad-seat visual through the glyph path or vice versa.

## § 11 Known limitations and follow-ups

1. **Single page-0 bind.** `AtlasGpu` builds the `@group(1)` bind group against
   coverage page 0 only (`gpu.rs:47-51, 198-206`) and `coverage.wgsl:72`
   ignores `i.page`. Heavy glyph load (CJK, many sizes) overflowing to page 1
   would silently sample wrong texels. v1 mitigation: the producer **warns at
   first page-1 allocation** (`AtlasEntry.page > 0`). Multi-page bind
   (texture-array or per-page batches) is the first follow-up, triggered by
   that warning firing in practice.
2. **Glyphs bypass effect groups — LANDED (T8,
   [2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md](../../plans/2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md)).**
   The glyph buffer is partitioned into flat/group ranges exactly like the
   quad path (`partition_glyph_ranges` over the producer's per-entity
   `entity_runs`; group membership derived from the fresh node list at
   prepare — the decoration-and-paint § 4.6 discipline), and the step-1
   group pass draws each group's glyph range into its `Rgba16Float` target
   via the `Glyph@Rgba16Float` pipeline specialization (after its quads,
   atlas `@group(1)` bound) while the flat draw covers the complement —
   text inside an `Opacity(0.5)` card dims exactly once. GPU regressions:
   `tests/text_effect_group_gpu.rs` + the flipped `text_decoration_gpu.rs`
   asymmetry test; the follow-ups.md entry is closed.
3. **Flat quad-then-glyph order.** All glyphs draw after all quads
   (shadow < quad < glyph), so a later sibling's background cannot cover an
   earlier sibling's text. Since T8 this holds **per region** — within each
   effect-group target and within the flat complement — rather than
   globally. Pending the per-(primitive, layer) interleaved batching the
   render architecture targets.
4. **Wholesale rebuild on any text damage** (§ 6.2) — per-entity patching is
   the named deferred optimization, same as the nodes path.
5. **Cold-glyph storm.** Rasterization runs in the extract sync window; the
   first frame of a large document serializes O(new glyphs) swash scaling into
   the pipeline sync point. Bounded by residency afterward. If gate-#14
   measurement flags it, the escape is a parallel pre-extract rasterize task
   feeding bitmaps — **do not build speculatively**.
6. **Eviction churn cost.** `get_image_uncached` re-rasterizes on every
   eviction return (§ 3.2), coupling gate-#15 grace tuning to text CPU cost
   under scroll patterns — a calibration note for `buiy-verification-design`,
   which must also calibrate the atlas page budget above the worst-case
   per-frame visible glyph footprint (a same-frame pressure eviction of an
   already-emitted entry is the § 6.3 hazard without the grace window).
7. **Fractional scale factors** (1.25/1.5) make logical rects non-integral;
   the logical→clip→viewport roundtrip is exact-linear but f32 rounding can
   shift a texel at extreme coordinates — covered by a fractional-scale golden
   fixture (§ 12).

## § 12 Verification

Per the project's two-lane test discipline (CLAUDE.md "GPU lane"):

- **Headless (no adapter; the default gate).** (a) `AtlasKey` scheme:
  round-trip + uniqueness across distinct `CacheKey`s; `FontKeyInterner`
  stability across calls. (b) Rect math against hand-computed
  `PhysicalGlyph`/`Placement` fixtures, including a fractional `scale_factor`.
  (c) Damage-gate retention: steady-state frame leaves `ExtractedGlyphs`
  untouched (`is_changed()` false); each trigger-union member fires a rebuild,
  including a scale-factor value flip — and a cursor-move-only frame (a
  `Window` component tick with an unchanged `scale_factor()`) must **not**
  fire, pinning the § 6.2 `Changed<Window>` regression — run adapterless on
  the manual extract harness
  ([verification.md § 1.2](verification.md#12-the-headless-inventory)).
  (d) Touch-pass: against the real `BuiyAtlas` (device-free by design), a
  retained visible glyph survives > `eviction_grace` frames; an off-screen one
  drains. (e) The seam contract test the render spec calls for
  ([seam § 7](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#7-verification)):
  no cosmic-text type crosses into the render module's public seam types.
- **GPU lane (`#[ignore]`, `tests/support/mod.rs` harness).** (a) Real-font
  end-to-end golden: a `hello_text` fixture's first painted frame matches its
  golden (gate-#2 determinism, § 6.4). (b) **Retint byte-identity**: render the
  same text under two themes; assert `atlas.page_pixels(...)` byte-identical
  between runs, only instance `color` differs (§ 7's contract). (c)
  Eviction-under-retention regression: force the § 6.3 hazard (disable the
  touch pass in a test harness, idle past grace, insert a new glyph) and assert
  the corruption is caught — then assert the touch pass prevents it.

## Open questions

1. **Resource naming drift:** this area's exploration blueprint names the
   shared engine resource `BuiyFontSystem(Arc<Mutex<FontSystem>>)`, while the
   decided engine-ownership pin names it `SharedFontSystem`. This file adopts
   **`SharedFontSystem`** (the pinned decision wins);
   [architecture.md](architecture.md) is the naming owner —
   reconcile there if it lands differently.
2. **Cross-area input contract (tracked, not contradictory):** § 2 step 0
   assumes a sibling area delivers a *shaped* `Buffer` component before extract
   (0.19's deferred-shaping API means someone must call `shape_until_scroll`
   with `&mut FontSystem` in the main world). If the measure seam does not
   guarantee shaping for the **final** post-layout wrap width, the producer's
   input contract is unmet — [measure-and-layout.md](measure-and-layout.md)
   must pin this explicitly.

## Sources

- <https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.FontSystem.html> — `Send + Sync` auto-traits (supersedes the prior-art non-Sync note)
- <https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.Buffer.html> — deferred-shaping setters; `shape_until_scroll`; `layout_runs(&self)`
- <https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.LayoutGlyph.html>, `struct.LayoutRun.html` — fields incl. `color_opt`, `decorations`
- <https://docs.rs/cosmic-text/0.19.0/src/cosmic_text/layout.rs.html> — `physical()` body: offset applied post-scale; `truncf` on y pre-bin
- <https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.CacheKey.html>, `struct.PhysicalGlyph.html`, `enum.SubpixelBin.html` — key fields + 4-bin quantization
- <https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.SwashCache.html> — `get_image` / `get_image_uncached` signatures; `Send + Sync`
- <https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.SwashImage.html>, `enum.SwashContent.html`, `struct.Placement.html` — swash 0.2.7 raster output types
