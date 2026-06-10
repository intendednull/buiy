# Buiy Text T4: First Pixels — the Glyph Producer Implementation Plan

**Date:** 2026-06-10
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md](../specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md) (all of it) + [architecture.md](../specs/2026-06-09-buiy-text-rendering-design/architecture.md) §§ 1.2–1.3, 3.2–3.3, 4.4, 5.1–5.2, 6 + [verification.md](../specs/2026-06-09-buiy-text-rendering-design/verification.md) §§ 1–3
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T4 (depends on T2 + T3; the implementer starts from a branch with T1–T3 merged — T3 landed @ `16094c9`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Text paints. Land `extract_buiy_glyphs` in `ExtractSchedule` `.after(maintain_atlas)`: the `physical()` 4-bin quantization, the 19 B structured `AtlasKey` + `FontKeyInterner`, the `get_image_uncached`-on-miss rasterize path (lock site #3 — the last of the three), `GlyphAlphaInstance` emission (§ 5.2 rect math, straight-alpha `TextColor` resolve, self-inclusive clip), the retain-with-probe damage gate (§ 6.2 — THE normative trigger union, incl. the cached-`f32` scale compare) plus the **un-gated** `ResidentTextKeys` touch pass (§ 6.3), color-emoji skip + warn-once (§ 9 — the `IconInstance` seam stays named, not built), real-entity GPU fixtures replacing `tests/atlas_gpu.rs`'s test-as-producer fill, the `wait_for_fonts` predicate + `warm_atlas` realization for the golden harness (verification § 3), and the `hello_text` example.

**Architecture:** The producer is one render-world `ExtractSchedule` system in `buiy_core::text` (the module that owns every cosmic-text type — none crosses into `render::atlas`'s seam types). Per visible text entity in `painters_z` order it walks `buffer.layout_runs()`, quantizes each glyph with cosmic-text's own `LayoutGlyph::physical()` (x gets honest 4-bin subpixel, y is structurally `Zero` via the upstream `truncf`), builds the structured key through the `FontKeyInterner`, resolves residency against the GPU-verified `BuiyAtlas` (rasterizing via `SwashCache::get_image_uncached` only on a miss, under a lazily-taken once-per-frame `SharedFontSystem` lock), and pushes one straight-alpha `GlyphAlphaInstance` into the existing `ExtractedGlyphs` slot. Everything downstream is frozen: `prepare_buiy_instances` packs on `glyphs.is_changed()`, `prepare_atlas_textures` uploads dirty pages, the node draws shadow < quad < glyph.

**v1 slice (glyph-pipeline § 1, verbatim):** monochrome `SwashContent::Mask` only; one window (D2); one `TextColor` token per entity (`color_opt` honored when present); `Shaping::Advanced` pinned; 4-bin x-subpixel; physical-px rasterization; single `CoverageR8` page (warn-once at page-1 allocation, § 11.1); wholesale rebuild on text damage + un-gated touch pass; no warmup-queue use; no color emoji; no decorations.

**Where T4 ends (honesty pins):**

- **Color emoji / `IconInstance` / `ColorRgba8` pages** — named seam (§ 9), skip + warn-once only. The exploration blueprint's "IconInstance split" is superseded by § 9 (campaign charter).
- **`ExtractedTextQuads`** — T6's carrier (decoration-and-paint § 4.6). § 6.2's "rebuild `ExtractedTextQuads` alongside" is a ledger comment in the gate, not code.
- **`Changed<CaretVisual>` / `Changed<SelectionVisual>`** — T7 union members; their components do not exist yet. Ledger comment.
- **ASCII pre-warm** — stays OUT (§ 6.4 rejected it as mandatory; the optional latency work is T9's decide/land item). v1 pushes **nothing** into `AtlasWarmupQueue`.
- **Glyphs bypass effect groups** — text inside `Opacity(0.5)` paints undimmed until T8 partitions the glyph buffer (`node.rs` TODO(text-seam); follow-ups.md entry exists). NOT a T4 bug.
- **Stored-PNG golden machinery** — still deferred (render campaign posture): the gate-#2 hello-text fixture asserts INLINE against computed pixels + a same-process re-capture, exactly like `render_golden_harness.rs`.
- **Wholesale rebuild on any text damage** (§ 6.2 / § 11.4) — per-entity patching is the named deferred optimization. Corollary: zero-coverage glyphs (spaces) re-rasterize on every *damage* frame (they insert nothing, so there is nothing to hit) — bounded by typing cadence, steady frames pay zero; noted, not "fixed".
- **Overflow-windowed lines** (T3 erratum 3): lines past the content-box height are absent from `ComputedTextLayout` and therefore from this producer's emission until the overflow seam is revisited.

**Sequencing note — cross-layer interleave (architecture Open Question 2, REQUIRED reading for whoever files the first z-order bug):** `ExtractedGlyphs` is a flat global list drawn as **one glyph batch after the quad batch** (shadow < quad < glyph globally, `node.rs:280–310`). Correct per-layer shadow < quad < glyph interleave **across overlapping stacking layers** is the render spec's buckets/`painters_z` work, not text's: until it lands, a later sibling's *background* cannot cover an earlier sibling's *text*, and layered fixtures can show text-always-on-top artifacts. **These are expected, are not T4 bugs, and must not be "fixed" inside the producer.** Within the glyph batch itself, instances ARE in `painters_z` order (§ 2) — glyph-over-glyph order is correct.

**Tech stack:** cosmic-text 0.19.0 (dep since T1; resolves swash **0.2.8** — the spec's Sources note says 0.2.7; `Placement`/`SwashContent`/`SwashImage` shapes verified identical, recorded as erratum), taffy 0.10.1, Bevy 0.18.1. **No new dependencies** — if a task appears to need one, STOP: that contradicts the charter.

**Test reality:** T4 is the first text phase with GPU-runtime deliverables. The *whole CPU producer* (keys, rect math, damage gate, rasterization — swash is pure CPU — atlas residency, touch pass) is headless; the GPU lane carries pixels only (hello-text inline golden, retint byte-identity, eviction-under-retention). Every GPU test keeps `#[ignore]` and builds on `tests/support/mod.rs`.

---

## The gate (run BOTH lanes at every task boundary)

T4 ships GPU-runtime code, so the per-task gate is the headless gate **plus the GPU lane** (this host has the RX 6700 XT / RADV; Vulkan render-to-texture needs no display):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace -j 2
```

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

Expected: both green. The headless gate must stay green **independently** (CI has no adapter); the GPU lane is additive and must pass on this host before the phase merges. A GPU-needing test without `#[ignore]` panics the headless run at adapter init — that boundary is self-policing.

---

## Orientation: verified facts this plan builds on

cosmic-text facts source-verified against the vendored **0.19.0** (`~/.cargo/registry/src/*/cosmic-text-0.19.0/`); swash against vendored **0.2.8** / zeno 0.3.3; Bevy against **0.18.1**. Re-verify file/line refs before editing — they drift.

| Fact | Verified shape |
|---|---|
| `LayoutGlyph::physical(offset: (f32, f32), scale: f32) -> PhysicalGlyph` | layout.rs:89–106 — `CacheKey::new(font_id, glyph_id, font_size * scale, ((x + x_offset).mul_add(scale, offset.0), truncf((y - y_offset).mul_add(scale, offset.1))), font_weight, cache_key_flags)`. Offset is applied **after** scale (i.e. offset is **physical px**); y is `truncf`'d **before** binning ⇒ `y_bin` structurally `Zero`, x gets honest 4-bin treatment |
| `PhysicalGlyph` | layout.rs:79–86 — `{ cache_key: CacheKey, x: i32, y: i32 }` (integer parts of the binned position) |
| `CacheKey` (all fields pub) | glyph_cache.rs:19–34 — `{ font_id: fontdb::ID, glyph_id: u16, font_size_bits: u32, x_bin: SubpixelBin, y_bin: SubpixelBin, font_weight: fontdb::Weight, flags: CacheKeyFlags }` |
| `SubpixelBin` | glyph_cache.rs:65–70 — `Zero \| One \| Two \| Three`; `new(pos)` bins fract at 0.125/0.375/0.625/0.875 and carries to the next integer above 0.875 |
| `CacheKeyFlags` | glyph_cache.rs:7–14 — `bitflags!` over `u32` (`FAKE_ITALIC`/`DISABLE_HINTING`/`PIXEL_FONT`); `.bits() -> u32` |
| `SwashCache::get_image_uncached(&mut self, &mut FontSystem, CacheKey) -> Option<SwashImage>` | swash.rs:155–161 — owned result, **no internal insert** (`get_image` is the caching path Buiy never calls — architecture § 1.3) |
| `SwashImage` / `SwashContent` / `Placement` | swash.rs:15–16 re-exports; swash image.rs:11–37 — `Image { content, placement, data: Vec<u8>, .. }`, `Content::{Mask, SubpixelMask, Color}`; zeno geometry.rs:459–470 — `Placement { left: i32, top: i32, width: u32, height: u32 }` (**top points up** ⇒ the § 5.2 subtraction). `Mask` data is `width*height` bytes, row-major |
| `LayoutRun` | buffer.rs:36–55 — `{ line_i, text, rtl, glyphs: &[LayoutGlyph], decorations, line_y, line_top, line_height, line_w }`; `layout_runs(&self)` is read-only and **terminates** at the first unshaped line |
| `LayoutGlyph.color_opt` | layout.rs:57–58 — `Option<cosmic_text::Color>`; `Color(pub u32)` with `r()/g()/b()/a() -> u8` (attrs.rs:16, 45–67) — sRGB8 components |
| Crate-root re-exports | lib.rs:94–137 — `CacheKey`, `CacheKeyFlags`, `SubpixelBin`, `PhysicalGlyph`, `SwashCache`, `SwashContent`, `SwashImage`, `Placement`, `FontSystem`, `fontdb` all at `cosmic_text::` |
| `fontdb::FaceInfo.source` | pub field — the test route to a second distinct `fontdb::ID` (re-load the same `Source` into one db; fontdb does not dedup) |
| taffy `Layout` | layout.rs:226+, 310–322 — pub `location/size/border/padding` (`Rect<f32>`); content-box offset from the node's own border-box top-left = `(border.left + padding.left, border.top + padding.top)` (what `content_box_x/y` add to `location`) |
| `bevy::render::MainWorld` | bevy_render lib.rs:263–280 — `pub struct MainWorld(World)` (private field), but `Default + Deref + DerefMut<Target = World>` ⇒ a test can `init_resource::<MainWorld>()` and `mem::swap` the app world in/out — exactly what bevy_render's own `extract()` does (lib.rs:453–463) |
| `Window` cursor tick hazard | bevy_window window.rs:638–641 — `set_cursor_position` writes `internal.physical_cursor_position` ⇒ a `Changed<Window>` probe fires on every mouse-move frame. § 6.2 therefore pins the **value compare** on `resolution.scale_factor()` (window.rs:965; `set_scale_factor` :1007 is the test's flip lever) |
| `URect::size() -> UVec2` | bevy_math urect.rs:184 — `AtlasEntry.px` carries the cell's pixel rect; its size == the rasterized `Placement{width,height}` |

As-built code consumed (read before editing, confirm current — **extend, never redefine**):

- `crates/buiy_core/src/render/atlas/types.rs` — **`AtlasKey(pub SmallVec<[u8; 24]>)` with only `from_bytes`** — today the key is fully opaque; the structured 19 B scheme does NOT exist yet (the GPU tests key with literals like `b"glyph-tint-test"`). The delta = this plan's Task 1. `AtlasEntry { page: u16, format, uv: Rect, px: URect }`; `AtlasConfig { page_size: 1024, page_budget: 8, eviction_grace: 60 }`; `AtlasEntryKind::{Glyph, Icon, Gradient, Mask}` (no key-byte mapping yet).
- `crates/buiy_core/src/render/atlas/atlas.rs` — `BuiyAtlas`: `get` (:50, **no LRU touch** — the `wait_for_fonts` probe leans on this), `get_or_insert(key, format, FnOnce() -> AtlasBitmap)` (:58 — hit touches LRU, closure runs only on a miss), `touch_existing` (:97 — built for exactly the § 6.3 caller), `drain_grace_expired` (:105), `evict_for_test` (:250), `page_pixels` (:262), `live_entry_count`/`page_count`. Eviction does NOT clear texels — the § 6.3 hazard is real.
- `crates/buiy_core/src/render/atlas/mod.rs` — `register` chains `(warmup_atlas, maintain_atlas)` in `ExtractSchedule` (:103); `maintain_atlas` (gpu.rs:214–218) = `begin_frame` + `drain_grace_expired` + `collect_emptied_pages` — `.after(maintain_atlas)` means producer inserts/touches use the just-advanced frame clock. `maintain_atlas` is `pub`; `warmup_atlas` is private (the harness doesn't need it — v1 pushes nothing).
- `crates/buiy_core/src/render/atlas/primitive.rs` — `GlyphAlphaInstance { rect, uv, color, clip: [f32;4] each, page: u32 }`, 68 B `#[repr(C)]`, compile-asserted; `color` is **linear-light, pre-linearized, STRAIGHT-alpha** (NOT premultiplied — `coverage.wgsl` scales only alpha); `clip` uses the `[±INFINITY]` unclipped sentinel.
- `crates/buiy_core/src/render/prepare.rs:46–51, 207–216` — `ExtractedGlyphs { glyphs: Vec<GlyphAlphaInstance> }`, the v1 producer slot this plan fills for real; the glyph buffer re-uploads on `glyphs.is_changed()` **independently** of the quad gate.
- `crates/buiy_core/src/render/extract.rs` — the damage-gate idioms to mirror exactly: un-gated full fan + entity-only `Changed` probe (:334–365), `RemovedComponents` streams drained FIRST (:409–410), early-return retention (:429–431), `effective_clip` (:249–260, pub — top-layer → `None` sentinel, else `clip_for_primitive(false, …)` = own box ∩ ancestors, i.e. **already self-inclusive**, § 8), the context-tree walk + sorted-roots tiebreak (:552–574).
- `crates/buiy_core/src/render/components.rs:45–53` — `TextColor(pub ColorToken)` exists since T2, `Default = CurrentColor`; `render/color.rs:127` `resolve_token` (CurrentColor → `CanvasText` if present else `color.text.primary`; miss → magenta sentinel + warn).
- `crates/buiy_core/src/text/` (T1–T3 as-built) — `SharedFontSystem::lock()` (panics on poison; **exactly three lock sites**, #3 is this plan's); `BuiySwashCache(pub SwashCache)` (render-world, uncached-only contract); `TextBuffer { pub buffer, intrinsics }`; `ComputedTextLayout { lines, size }` (idempotent writer = `text_commit`); `text_commit`'s lazy `Option<MutexGuard>` lock pattern (commit.rs:56, 90) — Task 4 copies it; `register_render_world(render_app, &fonts)` (mod.rs:129–133) — Task 4 grows it.
- `crates/buiy_core/tests/support/mod.rs` — `gpu_test_app` / `gpu_render_app(w,h)` / `finish_and_run` / `render_to_image` / `spawn_capture_camera` / `readback_rgba` (row-padding-stripping). **Neither app builder adds `BuiyTextPlugin` today** — Task 7 adds it.
- `crates/buiy_core/tests/atlas_gpu.rs` — the test-as-producer fill this plan deletes: `warmup_coverage` + `set_glyphs` + tests (1) upload+tint, (2) retint byte-identity, (3) warmup determinism. Test (4) `gate15_atlas_entries_return_to_baseline_after_idle` is atlas-mechanics-only (direct `get_or_insert`, no glyph instances) and **stays**.
- `crates/buiy_core/src/render/golden.rs` — `GoldenConfig { fixed_clock, wait_for_fonts, warm_atlas, accept }` + `perceptual_diff`; `wait_for_fonts`/`warm_atlas` are declared flags with no mechanism — Task 6 realizes them.
- `examples/hello_button/` — the example-crate shape `hello_text` copies (workspace-member crate, `publish = false`, bevy + buiy deps).
- `tests/layout_container_queries.rs:500–516` — the synthetic `(Window, PrimaryWindow)` spawn idiom for headless apps (no `WindowPlugin` machinery needed for a component-only `Window`).

## Decisions (with runner-ups) — read before implementing

1. **The structured key lives in `text/atlas_key.rs`; render only gains `AtlasEntryKind::key_byte()`.** The atlas's key stays opaque (`AtlasKey` unchanged); the *construction* is the text spec's concern (types.rs:10–14 says so verbatim). The kind→byte mapping lives on the render-owned enum so a future Icon producer can't drift. *Runner-up rejected:* hardcoding `0u8` in text — silent aliasing the day Icon picks the same byte.
2. **Content origin = `ComputedTextLayout.content_offset` (border + padding), written by `text_commit`.** Glyph/run coordinates are content-box-relative; `GlobalTransform` lands on the border box; the render world has no Taffy access and no padding/border components. `text_commit` already holds the Taffy `Layout` and the idempotent-write guard, and damage already keys on `Changed<ComputedTextLayout>` — a padding change that shifts glyphs re-fires the gate for free. **Erratum for the spec edit pass:** glyph-pipeline § 5.1 names "the text entity's content origin" without pinning its source; this field is the pin. *Runner-ups rejected:* a separate component (a second idempotent writer + a second probe for one Vec2); resolving padding at extract (extract would re-derive layout — pillar violation).
3. **Glyph bearings ride a producer-owned `GlyphMetaCache`, pruned to atlas residency.** § 5.2's rect math needs `placement.left/top`, but `AtlasEntry` carries only `uv`/`px` (size) — on a cache **hit** the bearings are not derivable from the seam. **Erratum for the spec edit pass:** "`AtlasEntry.px` exists precisely for this snap math" covers width/height only. Bearings are a pure function of the `CacheKey`, so a `HashMap<AtlasKey, GlyphBearing>` written on rasterize and pruned (`retain`) to `atlas.get(key).is_some()` after each rebuild is exactly coherent with residency (glyphon stores `GlyphDetails` the same way). *Runner-ups rejected:* widening `AtlasEntry` with bearing fields — producer-specific data on a frozen, producer-agnostic, GPU-tested seam; re-rasterizing on every rebuild hit — pays full swash scaling per damage frame, gutting the atlas's reason to exist.
4. **Rasterize-on-miss = residency probe + prebuilt-bitmap closure.** § 2 step 4's literal "one `get_or_insert` closure" cannot represent the two skip outcomes (zero-coverage and `Color` content insert *nothing*, but the closure must return a bitmap). As built: `atlas.get(&key)` (+ bearing-cache hit) short-circuits; otherwise rasterize (lock site #3, taken **lazily once per frame** — the `text_commit` guard pattern, so a hit-only frame takes zero locks and a miss-heavy frame takes one), then `get_or_insert(key, CoverageR8, move || bitmap)` (the `drain_warmup` precedent). Semantics preserved exactly: raster work runs only on a miss. **Erratum** (mechanical shape, not a contradiction).
5. **The producer binds `&TextBuffer` directly, not `TextBufferAccess`.** glyph-pipeline § 6.1 names the accessor's read-only form, but T3 decision 12 (recorded T3 erratum, superseding) deferred `TextBufferAccess` to the editing campaign — its `edit` arm binds `TextEditState`, which does not exist. The swap is mechanical when the editor lands. **Erratum** (restate for § 6.1).
6. **Paint order is shared, not duplicated:** factor `context_tree_paint_order` + `context_roots` out of `extract_buiy_nodes`' walk (pure, already-tested semantics) and consume them from both producers. *Runner-up rejected:* iterating the text query in archetype order — wrong the day two text entities overlap (tooltip over text).
7. **The GPU eviction-under-retention test simulates "touch pass disabled" via `evict_for_test`,** not a production flag: force-evict a still-referenced key, insert a same-size filler (guillotiere reuses the freed cell — asserted), never re-extract, and watch the stale UVs sample the filler. *Runner-up rejected:* a `TouchPassDisabled` resource in production code — test-only code in prod (testing-anti-patterns).
8. **`tests/atlas_gpu.rs` migration:** tests (1)–(3) + the `set_glyphs`/`warmup_coverage` helpers are **deleted**, superseded by real-entity equivalents in the new `tests/text_gpu.rs` (upload+tint and warmup-determinism collapse into the hello-text first-frame fixture — first-frame residency is now *structural*, § 6.4; retint byte-identity is re-proven with real text). Test (4) gate-#15 churn stays (atlas mechanics, no producer). The warmup **queue**'s GPU consumer coverage returns with T6's solid-stamp push (the queue's CPU drain stays covered headless in `atlas_alloc.rs`).
9. **Scale-factor probe = cached `f32` value compare** stored beside the producer's other retained state (`ResidentTextKeys.last_scale_factor`) — never `Changed<Window>` (the cursor-tick trap is test-pinned), never a `WindowScaleFactorChanged` reader (§ 6.2 names the compare as normative).

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/buiy_core/src/render/atlas/types.rs` | modify | `AtlasEntryKind::key_byte()` — the kind-partition byte |
| `crates/buiy_core/src/text/atlas_key.rs` | create | `FontKeyInterner`, `glyph_atlas_key` (the 19 B scheme), `GLYPH_KEY_LEN` |
| `crates/buiy_core/src/text/extract.rs` | create | pure math (`physical_offset`, `glyph_rect_logical`, `pack_clip`, `GlyphBearing`), `ResidentTextKeys`, `GlyphMetaCache`, `extract_buiy_glyphs`, rasterize helper, warn-onces |
| `crates/buiy_core/src/text/components.rs` | modify | `ComputedTextLayout.content_offset` |
| `crates/buiy_core/src/text/commit.rs` | modify | write `content_offset`; widen the steady-state guard |
| `crates/buiy_core/src/text/mod.rs` | modify | module decls, exports, render-world registration |
| `crates/buiy_core/src/render/extract.rs` | modify | factor `context_tree_paint_order` + `context_roots` (shared walk) |
| `crates/buiy_core/src/render/prepare.rs`, `render/mod.rs`, `render/node.rs` | modify | stale "(unbuilt)" producer comments |
| `crates/buiy_core/src/render/golden.rs` | modify | `fonts_ready` — the realized `wait_for_fonts` predicate |
| `crates/buiy_core/tests/text_atlas_key.rs` | create | key scheme + interner (headless) |
| `crates/buiy_core/tests/text_glyph_math.rs` | create | rect math + binning pins (headless) |
| `crates/buiy_core/tests/support/extract_harness.rs` | create | the adapterless extract harness (verification § 1.2) |
| `crates/buiy_core/tests/text_extract.rs` | create | damage gate / retention / emission (headless) |
| `crates/buiy_core/tests/text_touch_pass.rs` | create | § 6.3 survival/drain + seam contract (headless) |
| `crates/buiy_core/tests/text_gpu.rs` | create | hello-text inline golden, retint byte-identity, eviction-under-retention (`#[ignore]`) |
| `crates/buiy_core/tests/atlas_gpu.rs` | modify | delete the test-as-producer fill; keep gate-#15 |
| `crates/buiy_core/tests/support/mod.rs` | modify | `BuiyTextPlugin` in both app builders; `px`/`expected_full_coverage_srgb`/`wait_for_text_ready` helpers |
| `crates/buiy_core/tests/text_commit.rs`, `tests/render_golden_harness.rs` | modify | `content_offset` tests; `fonts_ready` tests |
| `examples/hello_text/` + root `Cargo.toml` | create/modify | the exit-criterion example |
| `docs/plans/2026-06-09-buiy-text-campaign.md`, `docs/README.md`, `CLAUDE.md` | modify | docs flip (Task 9) |

---

### Task 1: The structured `AtlasKey` + `FontKeyInterner` (headless)

**Files:**
- Modify: `crates/buiy_core/src/render/atlas/types.rs`
- Create: `crates/buiy_core/src/text/atlas_key.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Test: `crates/buiy_core/tests/text_atlas_key.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_atlas_key.rs`:

```rust
//! The structured glyph `AtlasKey` scheme (glyph-pipeline § 4): 19 B
//! fixed-layout little-endian bytes from the verified `CacheKey` fields,
//! kind-partitioned, font-id interned. Headless — no adapter anywhere.

use buiy_core::render::atlas::AtlasEntryKind;
use buiy_core::text::{FontKeyInterner, GLYPH_KEY_LEN, glyph_atlas_key, registered_fonts_db};
use cosmic_text::{CacheKey, CacheKeyFlags, SubpixelBin, fontdb};
use std::collections::HashSet;

/// One face id from the embedded deterministic db (default_font is a
/// default-on feature, so the db always has exactly one face here).
fn embedded_face() -> fontdb::ID {
    registered_fonts_db()
        .faces()
        .next()
        .expect("the embedded default font is registered")
        .id
}

fn key_with(font_id: fontdb::ID, glyph_id: u16, size: f32, x_bin: SubpixelBin) -> CacheKey {
    CacheKey {
        font_id,
        glyph_id,
        font_size_bits: size.to_bits(),
        x_bin,
        y_bin: SubpixelBin::Zero,
        font_weight: fontdb::Weight(400),
        flags: CacheKeyFlags::empty(),
    }
}

#[test]
fn kind_bytes_partition_the_key_space() {
    // The leading byte is the producer partition (§ 4): four kinds, four
    // distinct stable bytes. Renumbering is a cache-invalidation bug.
    let bytes = [
        AtlasEntryKind::Glyph.key_byte(),
        AtlasEntryKind::Icon.key_byte(),
        AtlasEntryKind::Gradient.key_byte(),
        AtlasEntryKind::Mask.key_byte(),
    ];
    assert_eq!(bytes.iter().collect::<HashSet<_>>().len(), 4);
    assert_eq!(AtlasEntryKind::Glyph.key_byte(), 0, "glyph byte is pinned");
}

#[test]
fn glyph_key_is_19_bytes_inline_and_byte_exact() {
    let mut interner = FontKeyInterner::default();
    let key = glyph_atlas_key(
        &key_with(embedded_face(), 7, 20.0, SubpixelBin::One),
        &mut interner,
    );
    assert_eq!(key.0.len(), GLYPH_KEY_LEN);
    assert!(!key.0.spilled(), "19 B must fit SmallVec<[u8; 24]> inline");
    // [kind=0][font=0 u32][glyph=7 u16][20.0f32 bits][x_bin=1][y_bin=0]
    // [weight=400 u16][flags=0 u32], all little-endian.
    assert_eq!(
        key.0.as_slice(),
        &[
            0, // kind: Glyph
            0, 0, 0, 0, // interned font 0
            7, 0, // glyph_id
            0x00, 0x00, 0xA0, 0x41, // 20.0f32.to_bits() LE
            1, // x_bin One
            0, // y_bin Zero
            0x90, 0x01, // weight 400 LE
            0, 0, 0, 0, // flags
        ]
    );
}

#[test]
fn distinct_cache_keys_make_distinct_atlas_keys() {
    let mut interner = FontKeyInterner::default();
    let font = embedded_face();
    let variants = [
        key_with(font, 7, 20.0, SubpixelBin::Zero),
        key_with(font, 8, 20.0, SubpixelBin::Zero),  // glyph_id
        key_with(font, 7, 25.0, SubpixelBin::Zero),  // size
        key_with(font, 7, 20.0, SubpixelBin::Two),   // x_bin
        CacheKey { font_weight: fontdb::Weight(700), ..key_with(font, 7, 20.0, SubpixelBin::Zero) },
        CacheKey { flags: CacheKeyFlags::FAKE_ITALIC, ..key_with(font, 7, 20.0, SubpixelBin::Zero) },
    ];
    let keys: HashSet<_> = variants
        .iter()
        .map(|ck| glyph_atlas_key(ck, &mut interner))
        .collect();
    assert_eq!(keys.len(), variants.len(), "every shape-affecting field is in the key");
}

#[test]
fn interner_is_stable_and_monotonic() {
    // Two distinct ids in ONE db: fontdb does not dedup a re-loaded source,
    // so loading the embedded face's own Source again yields a second id.
    let mut db = registered_fonts_db();
    let first = db.faces().next().unwrap().id;
    let source = db.faces().next().unwrap().source.clone();
    let second = db.load_font_source(source)[0];
    assert_ne!(first, second);

    let mut interner = FontKeyInterner::default();
    let a0 = interner.intern(first);
    let b0 = interner.intern(second);
    let a1 = interner.intern(first);
    assert_eq!(a0, 0, "sequential from zero");
    assert_eq!(b0, 1);
    assert_eq!(a0, a1, "stable across calls — the content-address contract");
    assert_eq!(interner.len(), 2, "monotonic, never evicted");

    // Same CacheKey through the same interner ⇒ identical key bytes
    // (round-trip half of § 12 a).
    let mut i2 = FontKeyInterner::default();
    i2.intern(first);
    let ck = key_with(first, 3, 16.0, SubpixelBin::Three);
    assert_eq!(glyph_atlas_key(&ck, &mut interner), glyph_atlas_key(&ck, &mut i2));
}
```

- [ ] **Step 2: Run the tests, verify they fail to compile**

Run: `cargo test -p buiy_core --test text_atlas_key`
Expected: compile error — `key_byte`, `FontKeyInterner`, `glyph_atlas_key`, `GLYPH_KEY_LEN` do not exist.

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/render/atlas/types.rs`, extend the existing `impl AtlasEntryKind` (below `format()`):

```rust
    /// The leading `AtlasKey` byte partitioning the opaque key space per
    /// producer kind (glyph-pipeline § 4): a future Icon/Gradient/Mask key
    /// can never alias a glyph key. STABLE contract — extend, never
    /// renumber (a renumber silently invalidates every content address).
    pub fn key_byte(self) -> u8 {
        match self {
            AtlasEntryKind::Glyph => 0,
            AtlasEntryKind::Icon => 1,
            AtlasEntryKind::Gradient => 2,
            AtlasEntryKind::Mask => 3,
        }
    }
```

Create `crates/buiy_core/src/text/atlas_key.rs`:

```rust
//! The structured glyph `AtlasKey` (glyph-pipeline § 4): fixed-layout
//! little-endian bytes built from the verified `CacheKey` fields plus the
//! `AtlasEntryKind::Glyph` discriminant, with `fontdb::ID` interned to a
//! stable `u32` (the id's repr is private and version-fragile; the interner
//! costs one HashMap lookup and survives fontdb upgrades — § 4's rejected
//! runner-up (b)). Content addressing requires EQUALITY, not hashing —
//! a hashed-u64 key (rejected runner-up (a)) would silently alias two
//! glyphs' coverage on collision.
//!
//! cosmic-text types stay on THIS side of the seam: the render atlas only
//! ever sees the opaque byte key (atlas/mod.rs seam doc).

use std::collections::HashMap;

use bevy::prelude::Resource;
use cosmic_text::{CacheKey, SubpixelBin, fontdb};

use crate::render::atlas::{AtlasEntryKind, AtlasKey};

/// Exact byte length of a structured glyph key:
/// `[kind u8][font u32][glyph_id u16][font_size_bits u32][x_bin u8][y_bin u8][weight u16][flags u32]`.
pub const GLYPH_KEY_LEN: usize = 19;

/// Render-world interner: `fontdb::ID` → sequential `u32` (monotonic, never
/// evicted — fonts number in the dozens, glyph-pipeline § 4). One shared
/// `FontSystem` is load-bearing here: ids are stable only within one engine
/// (§ 3.1), so the interner is coherent for both shaping and rasterization.
#[derive(Resource, Default)]
pub struct FontKeyInterner {
    ids: HashMap<fontdb::ID, u32>,
}

impl FontKeyInterner {
    /// The stable `u32` for `font` — allocated on first sight, identical
    /// forever after.
    pub fn intern(&mut self, font: fontdb::ID) -> u32 {
        let next = self.ids.len() as u32;
        *self.ids.entry(font).or_insert(next)
    }

    /// Number of fonts interned so far.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when no font has been interned yet.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// `SubpixelBin` → stable byte. Explicit match — the upstream enum carries
/// no guaranteed discriminants.
fn bin_byte(bin: SubpixelBin) -> u8 {
    match bin {
        SubpixelBin::Zero => 0,
        SubpixelBin::One => 1,
        SubpixelBin::Two => 2,
        SubpixelBin::Three => 3,
    }
}

/// Build the structured glyph `AtlasKey` from a quantized `CacheKey`
/// (glyph-pipeline § 4). 19 B — fits `AtlasKey`'s `SmallVec<[u8; 24]>`
/// inline capacity, so the hot path never heap-allocates. `weight` and
/// `flags` are in the key because both are shape-affecting `CacheKey`
/// inputs; `y_bin` is carried even though § 5.1 makes it structurally zero
/// (one byte buys layout stability if vertical binning ever changes).
pub fn glyph_atlas_key(cache_key: &CacheKey, interner: &mut FontKeyInterner) -> AtlasKey {
    let mut bytes = [0u8; GLYPH_KEY_LEN];
    bytes[0] = AtlasEntryKind::Glyph.key_byte();
    bytes[1..5].copy_from_slice(&interner.intern(cache_key.font_id).to_le_bytes());
    bytes[5..7].copy_from_slice(&cache_key.glyph_id.to_le_bytes());
    bytes[7..11].copy_from_slice(&cache_key.font_size_bits.to_le_bytes());
    bytes[11] = bin_byte(cache_key.x_bin);
    bytes[12] = bin_byte(cache_key.y_bin);
    bytes[13..15].copy_from_slice(&cache_key.font_weight.0.to_le_bytes());
    bytes[15..19].copy_from_slice(&cache_key.flags.bits().to_le_bytes());
    AtlasKey::from_bytes(&bytes)
}
```

In `crates/buiy_core/src/text/mod.rs`: add `mod atlas_key;` to the module list (alphabetical, before `mod commit;`) and the export:

```rust
pub use atlas_key::{FontKeyInterner, GLYPH_KEY_LEN, glyph_atlas_key};
```

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p buiy_core --test text_atlas_key`
Expected: 4 passed.

- [ ] **Step 5: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(text): T4 task 1 — structured 19B AtlasKey + FontKeyInterner

glyph-pipeline § 4: kind-partitioned fixed-layout LE bytes from the
verified CacheKey fields; fontdb::ID interned to a stable u32.
Runner-ups (hashed u64 key; raw fontdb repr) rejected per spec."
```

---

### Task 2: Rect math + subpixel binning pure functions (headless)

**Files:**
- Create: `crates/buiy_core/src/text/extract.rs` (pure-math half; the system joins in Task 4)
- Modify: `crates/buiy_core/src/text/mod.rs`
- Test: `crates/buiy_core/tests/text_glyph_math.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_glyph_math.rs`:

```rust
//! glyph-pipeline § 5 — the subpixel/hiDPI math, pinned against
//! hand-computed fixtures (incl. a fractional scale factor) and the
//! upstream 4-bin quantizer (a cosmic-text-bump drift tripwire).

use bevy::math::{UVec2, Vec2};
use buiy_core::render::components::ClipRect;
use buiy_core::text::{GlyphBearing, glyph_rect_logical, pack_clip, physical_offset};
use cosmic_text::{CacheKey, CacheKeyFlags, SubpixelBin, fontdb};

#[test]
fn physical_offset_folds_origin_and_baseline_in_physical_px() {
    // § 5.1: physical() applies offset AFTER scale, so the offset handed in
    // must already be physical px: (origin.x*s, (origin.y + line_y)*s).
    let (x, y) = physical_offset(Vec2::new(10.0, 20.0), 12.8, 1.25);
    assert_eq!((x, y), (12.5, 41.0));
    // Identity at scale 1.
    let (x, y) = physical_offset(Vec2::new(3.0, 4.0), 6.0, 1.0);
    assert_eq!((x, y), (3.0, 10.0));
}

#[test]
fn rect_formula_rasterize_physical_position_logical() {
    // § 5.2 verbatim: rect_px = (phys.x + left, phys.y - top, w, h);
    // rect_logical = rect_px / scale. Placement top points UP — hence the
    // subtraction. Fractional 1.25 scale (the § 11.7 hiDPI case).
    let rect = glyph_rect_logical(
        130,
        56,
        GlyphBearing { left: 2, top: 13 },
        UVec2::new(9, 12),
        1.25,
    );
    assert_eq!(rect, [105.6, 34.4, 7.2, 9.6]);

    // Scale 1, negative left bearing (italic overhang).
    let rect = glyph_rect_logical(40, 30, GlyphBearing { left: -1, top: 8 }, UVec2::new(5, 7), 1.0);
    assert_eq!(rect, [39.0, 22.0, 5.0, 7.0]);
}

#[test]
fn upstream_quantizer_bins_x_four_ways_and_y_truncation_zeroes_y_bin() {
    // Pins the upstream CacheKey::new quantizer the producer rides
    // (glyph_cache.rs:36–60): fract 0.25 ⇒ One; integer ⇒ Zero. physical()
    // truncf's y BEFORE binning (layout.rs:99), so the y the producer hands
    // the quantizer is always integral ⇒ y_bin structurally Zero — § 5.1's
    // claim, drift-checked on every cosmic-text bump.
    let font_id = buiy_core::text::registered_fonts_db().faces().next().unwrap().id;
    let (key, x, y) = CacheKey::new(
        font_id,
        7,
        20.0,
        (19.25, 33.9_f32.trunc()), // x fract .25; y pre-truncated as physical() does
        fontdb::Weight(400),
        CacheKeyFlags::empty(),
    );
    assert_eq!((x, y), (19, 33));
    assert_eq!(key.x_bin, SubpixelBin::One);
    assert_eq!(key.y_bin, SubpixelBin::Zero);
    // Carry above 0.875 (the bin-table edge).
    let (key, x, _) = CacheKey::new(
        font_id, 7, 20.0, (19.9, 0.0), fontdb::Weight(400), CacheKeyFlags::empty(),
    );
    assert_eq!(x, 20);
    assert_eq!(key.x_bin, SubpixelBin::Zero);
}

#[test]
fn clip_packs_aabb_or_infinity_sentinel() {
    // § 8: encoding fixed by the consumer — logical-px AABB, ±INF sentinel
    // (identical to PackedInstance / the coverage.wgsl discard).
    assert_eq!(
        pack_clip(None),
        [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY]
    );
    let clip = ClipRect { min: Vec2::new(1.0, 2.0), max: Vec2::new(30.0, 40.0) };
    assert_eq!(pack_clip(Some(&clip)), [1.0, 2.0, 30.0, 40.0]);
}
```

- [ ] **Step 2: Run, verify compile failure**

Run: `cargo test -p buiy_core --test text_glyph_math`
Expected: compile error — `text/extract.rs` and its exports do not exist.

- [ ] **Step 3: Implement the pure half**

Create `crates/buiy_core/src/text/extract.rs`:

```rust
//! The glyph producer (glyph-pipeline; architecture § 4.4): per-frame, per
//! visible text entity in `painters_z` order — quantize via `physical()`,
//! key via the `FontKeyInterner`, rasterize on miss (lock site #3), emit
//! straight-alpha `GlyphAlphaInstance`s into the GPU-verified
//! `ExtractedGlyphs` slot. This file owns the producer; the atlas/consumer
//! seam it fills is frozen (render/atlas, prepare.rs, node.rs).
//!
//! This module is the cosmic-text boundary: no cosmic type crosses into
//! `render::atlas`'s seam types (pinned by tests/text_touch_pass.rs's seam
//! contract test).

use bevy::math::{UVec2, Vec2};
use bevy::prelude::*;

use crate::render::components::ClipRect;

/// A rasterized glyph's bearing — `Placement{left, top}` (top points UP),
/// the § 5.2 terms `AtlasEntry` does not carry. Cached per `AtlasKey`
/// (bearings are a pure function of the `CacheKey`, so the cache can never
/// go stale) in [`GlyphMetaCache`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphBearing {
    /// Horizontal offset from the glyph origin, physical px.
    pub left: i32,
    /// Vertical offset from the glyph origin, physical px, top-up.
    pub top: i32,
}

/// The `physical()` offset for one run (glyph-pipeline § 5.1): the entity's
/// content-box origin folded with the run baseline, in PHYSICAL px —
/// `physical()` applies its offset after scale (layout.rs:98–99 verified).
pub fn physical_offset(content_origin: Vec2, line_y: f32, scale_factor: f32) -> (f32, f32) {
    (
        content_origin.x * scale_factor,
        (content_origin.y + line_y) * scale_factor,
    )
}

/// The § 5.2 rect formula — rasterize physical, position logical:
/// `rect_px = (phys.x + left, phys.y - top, w, h)`, divided by the scale
/// factor into `GlyphAlphaInstance.rect`'s logical px. `size` is the atlas
/// cell's pixel extent (`AtlasEntry.px.size()` == the rasterized
/// `Placement{width, height}`). A physical-grid-aligned rect divided by the
/// scale lands back on the same physical texels under the exact-linear view
/// transform — crisp text under the pinned Nearest sampler.
pub fn glyph_rect_logical(
    phys_x: i32,
    phys_y: i32,
    bearing: GlyphBearing,
    size: UVec2,
    scale_factor: f32,
) -> [f32; 4] {
    let inv = 1.0 / scale_factor;
    [
        (phys_x + bearing.left) as f32 * inv,
        (phys_y - bearing.top) as f32 * inv,
        size.x as f32 * inv,
        size.y as f32 * inv,
    ]
}

/// Pack the resolved per-glyph clip (glyph-pipeline § 8) into the instance
/// slot: logical-px AABB, `±INFINITY` sentinel when unclipped — the SAME
/// encoding as `PackedInstance` (the coverage shader's discard reads it).
pub fn pack_clip(clip: Option<&ClipRect>) -> [f32; 4] {
    match clip {
        Some(c) => [c.min.x, c.min.y, c.max.x, c.max.y],
        None => [
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::INFINITY,
        ],
    }
}
```

In `crates/buiy_core/src/text/mod.rs`: add `mod extract;` and

```rust
pub use extract::{GlyphBearing, glyph_rect_logical, pack_clip, physical_offset};
```

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p buiy_core --test text_glyph_math`
Expected: 4 passed.

- [ ] **Step 5: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(text): T4 task 2 — § 5 rect math + binning pins (pure fns)

physical_offset / glyph_rect_logical / pack_clip with hand-computed
Placement fixtures incl. fractional scale; upstream 4-bin quantizer +
y-truncf pinned as a cosmic-text drift tripwire."
```

---

### Task 3: `ComputedTextLayout.content_offset` — the producer's content origin (headless)

**Files:**
- Modify: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/commit.rs`
- Test: `crates/buiy_core/tests/text_commit.rs` (extend)

- [ ] **Step 1: Write the failing test**

Append to `crates/buiy_core/tests/text_commit.rs`:

```rust
/// Decision 2 (T4): glyph/run coordinates are content-box relative while
/// GlobalTransform lands on the border box — TextCommit writes the
/// border+padding offset so the producer can fold the § 5.1 content origin
/// without Taffy access.
#[test]
fn commit_writes_the_content_box_offset() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().padding(6.0).border(2.0),
            Text(String::from("offset")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let layout = app.world().get::<ComputedTextLayout>(text).unwrap();
    assert_eq!(layout.content_offset, Vec2::new(8.0, 8.0), "border 2 + padding 6");
}

/// The steady-state short-circuit must not strand a stale offset: grow the
/// padding while growing the box so the CONTENT size is unchanged — the
/// buffer target compares equal, but the offset moved and must re-commit.
#[test]
fn padding_change_with_constant_content_box_updates_the_offset() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(100.0).height_px(40.0).padding(5.0),
            Text(String::from("x")),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default().flex_column().width_px(300.0).height_px(100.0)))
        .add_child(text);
    settle(&mut app);
    assert_eq!(
        app.world().get::<ComputedTextLayout>(text).unwrap().content_offset,
        Vec2::splat(5.0)
    );

    // 90x30 content box both times: (100-2*5) → (110-2*10).
    app.world_mut().entity_mut(text).insert(
        Style::default().width_px(110.0).height_px(50.0).padding(10.0),
    );
    settle(&mut app);
    assert_eq!(
        app.world().get::<ComputedTextLayout>(text).unwrap().content_offset,
        Vec2::splat(10.0),
        "offset re-committed even though the content-box size held"
    );
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p buiy_core --test text_commit commit_writes_the_content_box_offset`
Expected: compile error — `content_offset` field does not exist.

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/text/components.rs`, add the field to `ComputedTextLayout` (the derives — incl. `PartialEq` and `Default` — already give the idempotent-write compare and a `Vec2::ZERO` default):

```rust
pub struct ComputedTextLayout {
    /// One entry per laid-out line, visual top-to-bottom order.
    pub lines: Vec<ComputedTextLine>,
    /// Laid-out extent: (max line width, Σ line heights).
    pub size: Vec2,
    /// Content-box top-left offset from the entity's border-box top-left
    /// (border + padding, logical px). The glyph producer's § 5.1 content
    /// origin term: run/glyph coordinates are content-box relative, while
    /// `GlobalTransform` lands on the border box (T4 decision 2 — the
    /// spec's unpinned "content origin" source, pinned here).
    pub content_offset: Vec2,
}
```

In `crates/buiy_core/src/text/commit.rs`:

(a) compute the offset next to the existing `content`/`target` lines and widen the guard:

```rust
        let content = layout.content_box_size();
        let target = (Some(content.width.max(0.0)), Some(content.height.max(0.0)));
        // T4: the content origin the producer folds (decision 2). Part of
        // the steady-state guard: a padding change with a constant content
        // box moves the offset without moving the buffer target.
        let content_offset = Vec2::new(
            layout.border.left + layout.padding.left,
            layout.border.top + layout.padding.top,
        );
```

(b) replace the short-circuit:

```rust
        let offset_stale =
            existing_layout.is_none_or(|current| current.content_offset != content_offset);
        // § 4.2's steady-state short-circuit (+ the T4 offset term).
        if !align_changed && !offset_stale && text.buffer.size() == target {
            continue;
        }
```

(c) thread the offset into the fold — change the call to
`let (computed, baseline) = computed_outputs(&text.buffer, content_offset);` and the helper:

```rust
fn computed_outputs(
    buffer: &cosmic_text::Buffer,
    content_offset: Vec2,
) -> (ComputedTextLayout, Option<ResolvedBaseline>) {
```

with the struct literal at the bottom becoming
`(ComputedTextLayout { lines, size, content_offset }, baseline)`.

(d) fix any `ComputedTextLayout { .. }` struct literals elsewhere in tests (grep `ComputedTextLayout {` across `crates/buiy_core` — field-read sites are unaffected).

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test -p buiy_core --test text_commit`
Expected: all pass (old + 2 new).

- [ ] **Step 5: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(text): T4 task 3 — ComputedTextLayout.content_offset

TextCommit writes border+padding so the producer folds the § 5.1 content
origin without Taffy access; rides the existing idempotent write + the
Changed<ComputedTextLayout> probe. Runner-ups (separate component;
extract-side padding resolve) rejected — recorded in the plan."
```

---

### Task 4: `extract_buiy_glyphs` — the system, the damage gate, the rasterize-on-miss path (headless via the adapterless harness)

This is the keystone task: the full producer plus its registration, the shared paint-order walk, and the verification § 1.2 harness. The § 6.3 touch pass is wired here too (it is five lines inside the same system); its dedicated survival tests are Task 5.

**Files:**
- Modify: `crates/buiy_core/src/render/extract.rs` (factor the walk)
- Modify: `crates/buiy_core/src/text/extract.rs` (the system)
- Modify: `crates/buiy_core/src/text/mod.rs` (exports + registration)
- Modify: `crates/buiy_core/src/render/prepare.rs`, `render/mod.rs`, `render/node.rs` (stale comments)
- Create: `crates/buiy_core/tests/support/extract_harness.rs`
- Modify: `crates/buiy_core/tests/support/mod.rs` (declare the module)
- Test: `crates/buiy_core/tests/text_extract.rs`

- [ ] **Step 1: Factor the shared paint-order walk (refactor, existing tests are the net)**

In `crates/buiy_core/src/render/extract.rs`:

(a) Add the entity-order core + the roots helper:

```rust
/// Flatten one stacking-context tree into entity paint order: the root's own
/// box first, then its `painters_z` forward, descending into each nested
/// context AS A UNIT at its position (paint-order § 1.1). The entity-order
/// core of [`assemble_context_tree`], shared with the glyph producer
/// (`text::extract_buiy_glyphs`) so the two walks can never diverge.
pub fn context_tree_paint_order<'a>(
    root: Entity,
    painters_z_of: &impl Fn(Entity) -> Option<&'a [Entity]>,
    out: &mut Vec<Entity>,
) {
    out.push(root);
    let Some(painters) = painters_z_of(root) else {
        return;
    };
    for &painter in painters {
        if painters_z_of(painter).is_some() {
            context_tree_paint_order(painter, painters_z_of, out);
        } else {
            out.push(painter);
        }
    }
}

/// The root context entities of a forming-context map — those no other
/// context lists as a painter — sorted by entity so a (degenerate)
/// multi-root tree assembles deterministically (the extract_buiy_nodes
/// tiebreak, hoisted so both producers share it).
pub fn context_roots(
    sc_by_entity: &std::collections::HashMap<Entity, &[Entity]>,
) -> Vec<Entity> {
    let nested: std::collections::HashSet<Entity> = sc_by_entity
        .values()
        .flat_map(|painters| painters.iter().copied())
        .filter(|e| sc_by_entity.contains_key(e))
        .collect();
    let mut roots: Vec<Entity> = sc_by_entity
        .keys()
        .copied()
        .filter(|e| !nested.contains(e))
        .collect();
    roots.sort_unstable();
    roots
}
```

(b) Rewrite `assemble_context_tree`'s body in terms of the walker (signature and doc unchanged):

```rust
    let mut order = Vec::new();
    context_tree_paint_order(root, painters_z_of, &mut order);
    out.extend(order.into_iter().filter_map(build));
```

(c) In `extract_buiy_nodes`, replace the inline `nested`/`roots` block (~:564–574) with `let roots = context_roots(&sc_by_entity);`.

Run: `cargo test -p buiy_core --test render_paint_order --test render_extract` — Expected: pass unchanged (the refactor net).

- [ ] **Step 2: Write the producer system + retained state**

Append to `crates/buiy_core/src/text/extract.rs`:

```rust
use std::sync::MutexGuard;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::render::Extract;
use bevy::window::PrimaryWindow;
use cosmic_text::{CacheKey, FontSystem, SwashContent};

use crate::components::{Node, ResolvedLayout, StackingContext};
use crate::layout::Stacking;
use crate::render::atlas::{
    AtlasBitmap, AtlasEntry, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance,
};
use crate::render::color::resolve_token;
use crate::render::components::{AncestorClip, ComputedPaintSkip, TextColor};
use crate::render::extract::{context_roots, context_tree_paint_order, effective_clip};
use crate::render::prepare::ExtractedGlyphs;
use crate::theme::Theme;

use super::atlas_key::{FontKeyInterner, glyph_atlas_key};
use super::components::{ComputedTextLayout, TextBuffer};
use super::font_system::SharedFontSystem;
use super::swash::BuiySwashCache;

/// The producer's retained state (glyph-pipeline § 6.3 + § 6.2): every
/// `AtlasKey` the current `ExtractedGlyphs` samples (rebuilt alongside it on
/// damage; touched UN-gated every frame so retained-but-painted glyphs stay
/// LRU-warm), plus the cached last-seen primary-window scale factor — the
/// § 6.2 VALUE-COMPARE probe (never `Changed<Window>`: bevy_winit writes
/// `Window.physical_cursor_position` per CursorMoved, so a tick probe would
/// rebuild on every mouse-move frame).
#[derive(Resource, Default)]
pub struct ResidentTextKeys {
    /// One entry per emitted instance, in emission order.
    pub keys: Vec<AtlasKey>,
    /// `None` until the first rebuild seeds it (the first frame rebuilds
    /// regardless via the Added/Changed fan).
    pub last_scale_factor: Option<f32>,
}

/// Producer-owned bearing cache (T4 decision 3): `Placement{left, top}` per
/// glyph key — the § 5.2 terms a cache HIT cannot recover from `AtlasEntry`
/// (which carries uv + pixel rect only). Bearings are a pure function of the
/// `CacheKey`, so entries can never go stale; the map is pruned to atlas
/// residency after every rebuild, bounding it by the atlas's own audited
/// budget (no second eviction policy).
#[derive(Resource, Default)]
pub struct GlyphMetaCache(pub std::collections::HashMap<AtlasKey, GlyphBearing>);

/// The render-world glyph producer (architecture § 4.4; glyph-pipeline § 6).
/// Runs in `ExtractSchedule` `.after(maintain_atlas)` so inserts and touches
/// use the just-advanced frame clock.
///
/// Binds `&TextBuffer` directly — `TextBufferAccess` is deferred to the
/// editing campaign (T3 decision 12 supersedes § 6.1's accessor mention;
/// the swap is mechanical when `TextEditState` exists). `layout_runs` is
/// `&self`, so the main-world read stays read-only.
///
/// § 6.2 ledger — union members that join later, in lockstep with their
/// carriers: `Changed<CaretVisual>` / `Changed<SelectionVisual>` (T7);
/// `ExtractedTextQuads` rebuilt alongside `ExtractedGlyphs` (T6, same
/// producer, same probe, one damage decision).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn extract_buiy_glyphs(
    mut atlas: ResMut<BuiyAtlas>,
    mut glyphs: ResMut<ExtractedGlyphs>,
    mut interner: ResMut<FontKeyInterner>,
    mut resident: ResMut<ResidentTextKeys>,
    mut meta: ResMut<GlyphMetaCache>,
    fonts: Res<SharedFontSystem>,
    mut swash: ResMut<BuiySwashCache>,
    // The un-gated full fan (the extract_buiy_nodes discipline): WHETHER to
    // rebuild is the probe below; WHAT to include is always the full set.
    texts: Extract<
        Query<
            (
                &GlobalTransform,
                &TextBuffer,
                &ComputedTextLayout,
                Option<&TextColor>,
                Option<&ComputedPaintSkip>,
                Option<&ClipRect>,
                Option<&AncestorClip>,
                Option<&Stacking>,
            ),
            With<Node>,
        >,
    >,
    // § 6.2 — THE normative trigger union (architecture § 5.1 row 3 defers
    // here). `Changed<TextBuffer>` is deliberately ABSENT: measure/commit
    // writes bypass its ticks; the idempotent ComputedTextLayout write is
    // the text-changed signal.
    changed: Extract<
        Query<
            Entity,
            (
                With<TextBuffer>,
                Or<(
                    Changed<ComputedTextLayout>,
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<TextColor>,
                    Changed<ClipRect>,
                    Changed<AncestorClip>,
                    Changed<ComputedPaintSkip>,
                    Changed<Stacking>,
                )>,
            ),
        >,
    >,
    // Despawn + hide→show: the two damage sources Changed cannot see.
    mut removed: Extract<RemovedComponents<ResolvedLayout>>,
    mut removed_skip: Extract<RemovedComponents<ComputedPaintSkip>>,
    contexts: Extract<Query<(Entity, &StackingContext)>>,
    theme: Extract<Res<Theme>>,
    primary: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    // Drain the removal streams FIRST so the cursors advance on every frame,
    // including early returns (the extract.rs:409 discipline).
    let despawned = removed.read().count() > 0;
    let skip_lifted = removed_skip.read().count() > 0;

    let Ok(window) = primary.single() else {
        // Vanished window: clear the carrier ONCE (an unconditional clear
        // would mark ExtractedGlyphs changed and re-upload an empty buffer
        // every frame).
        if !glyphs.glyphs.is_empty() {
            glyphs.glyphs.clear();
            resident.keys.clear();
        }
        return;
    };
    let scale_factor = window.resolution.scale_factor();
    let scale_changed = resident.last_scale_factor != Some(scale_factor);

    let dirty = !changed.is_empty()
        || despawned
        || skip_lifted
        || theme.is_changed()
        || scale_changed;
    if !dirty {
        // Steady state: return WITHOUT touching ExtractedGlyphs (so
        // `glyphs.is_changed()` stays false in prepare and the GPU glyph
        // buffer is retained — the O(0) contract)… except for the § 6.3
        // UN-gated touch pass: retained instances embed uv/page, and an
        // untouched key would grace-evict while still painted — the
        // stale-uv corruption hazard. O(visible keys) hash lookups.
        for key in &resident.keys {
            atlas.touch_existing(key);
        }
        return;
    }
    resident.last_scale_factor = Some(scale_factor);

    // ---- Rebuild (wholesale, § 6.2 v1) -------------------------------
    let mut new_glyphs: Vec<GlyphAlphaInstance> = Vec::new();
    let mut new_keys: Vec<AtlasKey> = Vec::new();
    // Lock site #3 (architecture § 1.2 — the LAST of the exhaustive three):
    // taken LAZILY, once per frame, only when at least one glyph misses the
    // atlas (the text_commit guard pattern). A hit-only rebuild takes ZERO
    // locks; extract runs in the pipelining sync window, so the lock is
    // uncontended by construction.
    let mut font_guard: Option<MutexGuard<'_, FontSystem>> = None;
    let fonts: &SharedFontSystem = &fonts;
    let theme: &Theme = &theme;

    // painters_z order — the same walk extract_buiy_nodes runs (§ 2).
    let sc_by_entity: std::collections::HashMap<Entity, &[Entity]> = contexts
        .iter()
        .map(|(e, sc)| (e, sc.painters_z.as_slice()))
        .collect();
    let painters_z_of = |e: Entity| -> Option<&[Entity]> { sc_by_entity.get(&e).copied() };
    let mut order = Vec::new();
    for root in context_roots(&sc_by_entity) {
        context_tree_paint_order(root, &painters_z_of, &mut order);
    }

    let default_color = TextColor::default();
    for entity in order {
        let Ok((gt, buffer, computed, color, skip, clip_rect, ancestor_clip, stacking)) =
            texts.get(entity)
        else {
            continue; // not a text painter
        };
        if skip.is_some() {
            continue; // the single computed skip source (§ 5.3/§ 5.4)
        }
        // § 8: glyphs are CONTENT — self-inclusive clip (own ClipRect ∩
        // ancestors); top-layer members force the unclipped sentinel.
        let clip = pack_clip(effective_clip(stacking, clip_rect, ancestor_clip).as_ref());
        // § 7: resolved at extract like Background, CPU-linearized,
        // STRAIGHT alpha (premultiplying would double-dim — primitive.rs).
        let entity_color = linear_color(resolve_token(&color.unwrap_or(&default_color).0, theme));
        let origin = gt.translation().truncate() + computed.content_offset;

        let mut runs = 0usize;
        for run in buffer.buffer.layout_runs() {
            runs += 1;
            for glyph in run.glyphs.iter() {
                // § 5.1: cosmic-text's own binning, verbatim.
                let phys = glyph.physical(
                    physical_offset(origin, run.line_y, scale_factor),
                    scale_factor,
                );
                let key = glyph_atlas_key(&phys.cache_key, &mut interner);
                let Some((entry, bearing)) = resolve_glyph(
                    &mut atlas,
                    &mut meta,
                    fonts,
                    &mut font_guard,
                    &mut swash,
                    &key,
                    phys.cache_key,
                ) else {
                    continue; // zero coverage (whitespace) or color-emoji skip (§ 9)
                };
                if entry.page > 0 {
                    warn_once_page_overflow(); // § 11.1 v1 mitigation
                }
                // Per-span Attrs color override rides through (§ 7).
                let color = glyph.color_opt.map(span_color).unwrap_or(entity_color);
                if color[3] == 0.0 {
                    continue; // fully transparent: nothing to paint
                }
                new_glyphs.push(GlyphAlphaInstance {
                    rect: glyph_rect_logical(phys.x, phys.y, bearing, entry.px.size(), scale_factor),
                    uv: [entry.uv.min.x, entry.uv.min.y, entry.uv.max.x, entry.uv.max.y],
                    color,
                    clip,
                    page: entry.page as u32,
                });
                new_keys.push(key);
            }
        }
        // architecture § 3.2 tripwire: layout_runs TERMINATES at the first
        // unshaped line — a mismatch means something mutated the buffer
        // after TextCommit.
        debug_assert_eq!(
            runs,
            computed.lines.len(),
            "TextBuffer dirty-unshaped at extract (mutated after TextCommit?)"
        );
    }
    drop(font_guard);

    // Bearing-cache hygiene (decision 3): prune to atlas residency, so the
    // map is bounded by the atlas's own budget — invariant: every resident
    // glyph key has a bearing.
    meta.0.retain(|key, _| atlas.get(key).is_some());

    // Publish, then the § 6.3 touch pass over the NEW visible set (covers
    // this frame's hits — `atlas.get` deliberately does not touch the LRU).
    glyphs.glyphs = new_glyphs;
    resident.keys = new_keys;
    for key in &resident.keys {
        atlas.touch_existing(key);
    }
}

/// Residency + bearing for one glyph key. A hit with a cached bearing is
/// lock-free; otherwise rasterize via `SwashCache::get_image_uncached`
/// (lock site #3 — one cache, not two: the atlas is the only bitmap cache,
/// § 3.2) and insert. `None` = emit nothing, insert nothing: zero-coverage
/// (whitespace) or `SwashContent::Color` (§ 9: skip + warn-once — the
/// C-tier IconInstance/ColorRgba8 seam, named not built).
fn resolve_glyph<'a>(
    atlas: &mut BuiyAtlas,
    meta: &mut GlyphMetaCache,
    fonts: &'a SharedFontSystem,
    font_guard: &mut Option<MutexGuard<'a, FontSystem>>,
    swash: &mut BuiySwashCache,
    key: &AtlasKey,
    cache_key: CacheKey,
) -> Option<(AtlasEntry, GlyphBearing)> {
    if let (Some(entry), Some(bearing)) = (atlas.get(key), meta.0.get(key).copied()) {
        return Some((entry, bearing));
    }
    let font_system = font_guard.get_or_insert_with(|| fonts.lock());
    let image = swash.0.get_image_uncached(font_system, cache_key)?;
    debug_assert!(
        image.content != SwashContent::SubpixelMask,
        "the producer never requests subpixel-RGB (glyph-pipeline § 5.1)"
    );
    if image.placement.width == 0 || image.placement.height == 0 {
        return None; // § 2: zero-coverage glyphs emit no instance, insert nothing
    }
    match image.content {
        SwashContent::Mask => {
            let bearing = GlyphBearing {
                left: image.placement.left,
                top: image.placement.top,
            };
            let bitmap = AtlasBitmap {
                size: UVec2::new(image.placement.width, image.placement.height),
                format: AtlasFormat::CoverageR8,
                data: image.data,
            };
            // The closure moves the prebuilt bitmap (the drain_warmup
            // precedent) — it still runs only on a miss; on the
            // meta-miss-but-resident edge it is simply not called.
            let entry = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, move || bitmap);
            meta.0.insert(key.clone(), bearing);
            Some((entry, bearing))
        }
        SwashContent::Color => {
            warn_once_color_emoji_skipped();
            None
        }
        SwashContent::SubpixelMask => None,
    }
}

/// CPU-linearize a resolved color into the straight-alpha instance slot —
/// exactly the quad path (`render/instance.rs` `LinearRgba::from`).
fn linear_color(color: Color) -> [f32; 4] {
    let lin = LinearRgba::from(color);
    [lin.red, lin.green, lin.blue, lin.alpha]
}

/// Per-span `LayoutGlyph.color_opt` override (§ 7) — cosmic carries sRGB8.
fn span_color(c: cosmic_text::Color) -> [f32; 4] {
    linear_color(Color::srgba_u8(c.r(), c.g(), c.b(), c.a()))
}

static WARNED_COLOR_EMOJI: AtomicBool = AtomicBool::new(false);
static WARNED_PAGE_OVERFLOW: AtomicBool = AtomicBool::new(false);

/// § 9's rate-limited warn (the components.rs warn-once precedent).
fn warn_once_color_emoji_skipped() {
    if !WARNED_COLOR_EMOJI.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: color (emoji) glyphs are skipped in v1 — the ColorRgba8/\
             IconInstance path is a named C-tier seam (glyph-pipeline § 9; \
             warned once)"
        );
    }
}

/// § 11.1's v1 mitigation: the @group(1) bind group samples page 0 only.
fn warn_once_page_overflow() {
    if !WARNED_PAGE_OVERFLOW.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: a glyph allocated on coverage page > 0, but the glyph draw \
             binds page 0 only — those glyphs will sample wrong texels. Time \
             to build the multi-page bind (glyph-pipeline § 11.1; warned once)"
        );
    }
}
```

- [ ] **Step 3: Register, export, fix stale comments**

(a) `crates/buiy_core/src/text/mod.rs` — extend the exports and the render-world registration:

```rust
pub use extract::{
    GlyphBearing, GlyphMetaCache, ResidentTextKeys, extract_buiy_glyphs, glyph_rect_logical,
    pack_clip, physical_offset,
};
```

```rust
/// The render-world half of text registration (mirrors `atlas::register`):
/// the `SharedFontSystem` Arc clone — one engine, two worlds — plus the T4
/// glyph producer and its retained state. `.after(maintain_atlas)` so
/// inserts/touches use the just-advanced atlas frame clock (glyph-pipeline
/// § 6.1; ordering against an absent system set is vacuously satisfied, so
/// a bare SubApp without the atlas systems still registers cleanly).
pub fn register_render_world(render_app: &mut SubApp, fonts: &SharedFontSystem) {
    render_app.insert_resource(fonts.clone());
    render_app.init_resource::<BuiySwashCache>();
    render_app
        .init_resource::<FontKeyInterner>()
        .init_resource::<ResidentTextKeys>()
        .init_resource::<GlyphMetaCache>()
        .add_systems(
            bevy::render::ExtractSchedule,
            extract::extract_buiy_glyphs.after(crate::render::atlas::maintain_atlas),
        );
}
```

(also `pub use atlas_key::{FontKeyInterner, …}` from Task 1 stays; add `use` items as needed).

(b) Mechanical comment fixes (the producer is no longer "unbuilt"):
- `render/prepare.rs:39–51` (`ExtractedGlyphs` doc): replace the "text seam … unbuilt … tests play the producer" sentences with: produced by `text::extract_buiy_glyphs` in `ExtractSchedule` (T4); retained across steady frames — `is_changed()` is the § 6.2 damage signal the gate below reads.
- `render/mod.rs:248–252` (`init_resource::<prepare::ExtractedGlyphs>()` comment): now filled per frame by `text::extract_buiy_glyphs` (registered by `BuiyTextPlugin`); kept `init_resource`'d here so the prepare gate works even if the text plugin is absent.
- `render/node.rs:289–297` (TODO(text-seam)): drop the "(`glyph_count == 0` in v1)" clause — text lands in T4; glyphs inside an `EffectGroup` paint undimmed until the T8 partition (follow-ups.md entry). Keep the TODO itself.

- [ ] **Step 4: Build the adapterless extract harness**

Create `crates/buiy_core/tests/support/extract_harness.rs`:

```rust
//! The adapterless extract harness (text verification.md § 1.2): drives
//! `extract_buiy_glyphs` with NO wgpu adapter. The main `App` is built
//! WITHOUT `RenderPlugin`; the render side is a bare `World` carrying only
//! the CPU-side resources the producer touches (`BuiyAtlas` is device-free
//! by design). Each step swaps the main world in as `MainWorld`, runs a
//! manually built `ExtractSchedule`, and swaps it back — bevy_render's own
//! `extract()` dance (lib.rs:453–463), minus the renderer. Prepare/queue/
//! draw never exist, so nothing requests an adapter.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::CorePlugin;
use buiy_core::layout::LayoutPlugin;
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::atlas::{AtlasConfig, AtlasKey, BuiyAtlas, maintain_atlas};
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::{
    BuiySwashCache, BuiyTextPlugin, FontKeyInterner, GlyphMetaCache, ResidentTextKeys,
    SharedFontSystem, extract_buiy_glyphs,
};

/// Mirrors `prepare_buiy_instances`' glyph gate: counts frames on which
/// `ExtractedGlyphs` was rebuilt (`is_changed()` relative to this system's
/// last run — exactly the prepare semantics).
#[derive(Resource, Default)]
pub struct GlyphChangeLog {
    pub frames: usize,
    pub changed_frames: usize,
}

fn log_glyph_changes(glyphs: Res<ExtractedGlyphs>, mut log: ResMut<GlyphChangeLog>) {
    log.frames += 1;
    if glyphs.is_changed() {
        log.changed_frames += 1;
    }
}

pub struct TextExtractHarness {
    pub app: App,
    pub render: World,
    schedule: Schedule,
}

impl TextExtractHarness {
    pub fn new() -> Self {
        Self::with_atlas_config(AtlasConfig::default())
    }

    pub fn with_atlas_config(config: AtlasConfig) -> Self {
        let mut app = App::new();
        // BuiyRenderPlugin's MAIN-world half (clip rects, paint-skip,
        // effect groups, forced colors) registers headless — its render
        // half is guarded on a RenderApp that never exists here.
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(CorePlugin)
            .add_plugins(LayoutPlugin)
            .add_plugins(BuiyTextPlugin::default())
            .add_plugins(BuiyRenderPlugin);
        // Synthetic primary window (the layout_container_queries idiom):
        // component-only — the producer reads scale/presence via a Query.
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut render = World::new();
        render.insert_resource(BuiyAtlas::new(config));
        render.init_resource::<ExtractedGlyphs>();
        render.init_resource::<FontKeyInterner>();
        render.init_resource::<ResidentTextKeys>();
        render.init_resource::<GlyphMetaCache>();
        render.init_resource::<BuiySwashCache>();
        render.insert_resource(fonts);
        render.init_resource::<GlyphChangeLog>();
        // The slot the live main world is swapped into per extract step.
        render.init_resource::<MainWorld>();

        // Mirror the real chain: maintenance advances the frame clock, then
        // the producer (.after(maintain_atlas)), then the change probe.
        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems((maintain_atlas, extract_buiy_glyphs, log_glyph_changes).chain());

        Self { app, render, schedule }
    }

    /// One full frame: main-world Update (TextSync → measure → TextCommit),
    /// then the extract step against the live main world.
    pub fn frame(&mut self) {
        self.app.update();
        self.extract_only();
    }

    /// The extract step alone (no main-world Update).
    pub fn extract_only(&mut self) {
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
        self.schedule.run(&mut self.render);
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
    }

    /// Three frames: spawn-settle (TextSync insert + first commit + first
    /// extract rebuild all land within these).
    pub fn settle(&mut self) {
        for _ in 0..3 {
            self.frame();
        }
    }

    pub fn glyphs(&self) -> &ExtractedGlyphs {
        self.render.resource::<ExtractedGlyphs>()
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs().glyphs.len()
    }

    pub fn changed_frames(&self) -> usize {
        self.render.resource::<GlyphChangeLog>().changed_frames
    }

    pub fn resident_keys(&self) -> Vec<AtlasKey> {
        self.render.resource::<ResidentTextKeys>().keys.clone()
    }

    pub fn atlas(&self) -> &BuiyAtlas {
        self.render.resource::<BuiyAtlas>()
    }
}
```

In `crates/buiy_core/tests/support/mod.rs` add (after the inner attributes):

```rust
pub mod extract_harness;
```

- [ ] **Step 5: Write the failing damage-gate + emission tests**

Create `crates/buiy_core/tests/text_extract.rs`:

```rust
//! `extract_buiy_glyphs` — emission + THE § 6.2 damage gate, headless on
//! the adapterless extract harness (verification § 1.2; § 12 headless c).

mod support;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{ClipRect, CssVisibility, TextColor};
use buiy_core::text::{FontSize, Text};
use buiy_core::theme::Theme;
use std::borrow::Cow;
use support::extract_harness::TextExtractHarness;

/// "Hi!" under a sized column root: 3 non-whitespace glyphs.
fn spawn_text(h: &mut TextExtractHarness) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("Hi!")), FontSize(16.0)))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    text
}

fn set_cursor(h: &mut TextExtractHarness, pos: Vec2) {
    let mut q = h
        .app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>();
    q.single_mut(h.app.world_mut())
        .unwrap()
        .set_cursor_position(Some(pos));
}

fn set_scale(h: &mut TextExtractHarness, scale: f32) {
    let mut q = h
        .app
        .world_mut()
        .query_filtered::<&mut Window, With<PrimaryWindow>>();
    q.single_mut(h.app.world_mut())
        .unwrap()
        .resolution
        .set_scale_factor(scale);
}

#[test]
fn emits_one_instance_per_visible_glyph_with_resident_keys() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();

    assert_eq!(h.glyph_count(), 3, "one instance per non-whitespace glyph");
    assert_eq!(h.resident_keys().len(), 3);
    for key in h.resident_keys() {
        assert!(h.atlas().get(&key).is_some(), "every emitted key is resident");
    }
    // Geometry sanity: a 16px line near the content origin — the exact
    // numbers are pinned by text_glyph_math.rs; here we bound the fold.
    let first = h.glyphs().glyphs[0];
    assert!(first.rect[2] > 0.0 && first.rect[3] > 0.0, "non-degenerate rect");
    // Loose bound: baseline-folded y of a 16px line-1 glyph (bearings can
    // nudge a texel or two above the integer baseline truncation).
    assert!(first.rect[1] > -4.0 && first.rect[1] < 24.0, "baseline-folded y in line 1");
    // Unclipped fixture: the ±INF sentinel.
    assert_eq!(first.clip[0], f32::NEG_INFINITY);
    assert_eq!(first.clip[3], f32::INFINITY);
    assert_eq!(first.page, 0);
}

#[test]
fn steady_state_retains_extracted_glyphs_untouched() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    let settled = h.changed_frames();
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(
        h.changed_frames(),
        settled,
        "5 steady frames left ExtractedGlyphs untouched (is_changed stayed false — § 6.2 O(0))"
    );
}

#[test]
fn cursor_move_only_frame_does_not_rebuild() {
    // THE § 6.2 Changed<Window> regression pin: a Window component tick with
    // an unchanged scale_factor() must NOT fire the gate.
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    let settled = h.changed_frames();
    for i in 0..4 {
        set_cursor(&mut h, Vec2::new(i as f32 * 7.0, 5.0));
        h.frame();
    }
    assert_eq!(h.changed_frames(), settled, "mouse-move frames retained the carrier");
}

#[test]
fn each_union_member_fires_exactly_one_rebuild() {
    let mut h = TextExtractHarness::new();
    let text = spawn_text(&mut h);
    h.settle();
    let mut expect = h.changed_frames();

    // Text edit → ComputedTextLayout changes.
    h.app.world_mut().get_mut::<Text>(text).unwrap().0 = String::from("Hey!!");
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "text edit fired");
    assert_eq!(h.glyph_count(), 5);
    h.frame();
    assert_eq!(h.changed_frames(), expect, "…and settled");

    // TextColor (Added counts as Changed) + the resolved value lands.
    h.app
        .world_mut()
        .resource_mut::<Theme>()
        .colors
        .insert("test.text".into(), Color::srgb(0.2, 0.8, 0.4));
    h.app
        .world_mut()
        .entity_mut(text)
        .insert(TextColor(ColorToken::Token(Cow::Borrowed("test.text"))));
    h.frame();
    // theme.is_changed() and Changed<TextColor> may land on the same frame.
    assert!(h.changed_frames() > expect, "color/theme fired");
    expect = h.changed_frames();
    let lin = LinearRgba::from(Color::srgb(0.2, 0.8, 0.4));
    assert_eq!(
        h.glyphs().glyphs[0].color,
        [lin.red, lin.green, lin.blue, lin.alpha],
        "straight-alpha CPU-linearized token resolve (§ 7)"
    );
    h.frame();
    assert_eq!(h.changed_frames(), expect);

    // Scale flip: value compare fires; every key re-keys (§ 6.2 + § 6 arch).
    let keys_1x = h.resident_keys();
    set_scale(&mut h, 2.0);
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "scale flip fired");
    let keys_2x = h.resident_keys();
    assert_eq!(keys_2x.len(), keys_1x.len());
    assert!(
        keys_1x.iter().all(|k| !keys_2x.contains(k)),
        "a scale change re-keys every glyph (physical font size is in the key)"
    );
    h.frame();
    assert_eq!(h.changed_frames(), expect, "scale cache updated — no re-fire");

    // Hide (paint-skip ADD) → zero instances; show (REMOVE stream) → back.
    h.app.world_mut().entity_mut(text).insert(CssVisibility::Hidden);
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "paint-skip add fired");
    assert_eq!(h.glyph_count(), 0);
    h.app.world_mut().entity_mut(text).remove::<CssVisibility>();
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "hide→show rides the removal stream");
    assert_eq!(h.glyph_count(), 5);

    // Despawn → rebuild to empty, then steady.
    h.app.world_mut().entity_mut(text).despawn();
    h.frame();
    expect += 1;
    assert_eq!(h.changed_frames(), expect, "despawn rides RemovedComponents");
    assert_eq!(h.glyph_count(), 0);
    h.frame();
    assert_eq!(h.changed_frames(), expect, "empty steady state retains");
}

#[test]
fn clipped_text_packs_its_self_inclusive_clip() {
    // § 8: glyphs are CONTENT — clipped by the entity's OWN computed
    // ClipRect (own box ∩ ancestors), produced by write_clip_rects (the sole
    // producer — never inserted manually here, it would be overwritten).
    // The leaf's initial ClipRect insert also exercises the union's
    // Changed<ClipRect> member (Changed includes Added).
    use buiy_core::layout::OverflowMode;

    let mut h = TextExtractHarness::new();
    let text = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("clipped text wraps and overflows its tiny box")),
        ))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(60.0)
                .height_px(24.0)
                .overflow_x(OverflowMode::Hidden)
                .overflow_y(OverflowMode::Hidden),
        ))
        .add_child(text);
    h.settle();
    assert!(h.glyph_count() > 0);

    let clip = h
        .app
        .world()
        .get::<ClipRect>(text)
        .expect("write_clip_rects clipped the text leaf under the hidden-overflow box");
    let expected = [clip.min.x, clip.min.y, clip.max.x, clip.max.y];
    for inst in &h.glyphs().glyphs {
        assert_eq!(inst.clip, expected, "self-inclusive clip (§ 8) packed verbatim");
    }
}

#[test]
fn vanished_window_clears_once_then_retains() {
    let mut h = TextExtractHarness::new();
    spawn_text(&mut h);
    h.settle();
    assert!(h.glyph_count() > 0);

    let window = {
        let mut q = h
            .app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>();
        q.single(h.app.world()).unwrap()
    };
    h.app.world_mut().entity_mut(window).despawn();
    h.frame();
    assert_eq!(h.glyph_count(), 0, "no primary window ⇒ carrier cleared");
    let after_clear = h.changed_frames();
    h.frame();
    h.frame();
    assert_eq!(h.changed_frames(), after_clear, "the clear happens ONCE, not per frame");
}
```

- [ ] **Step 6: Run, iterate to green**

Run: `cargo test -p buiy_core --test text_extract`
Expected: 6 passed. (Common failure modes to debug, not paper over: a probe member missing from the union; the touch pass accidentally inside the dirty branch only; `settle()` needing one more frame because of command-flush timing — if so, fix the harness's `settle`, do not loosen assertions.)

Also run: `cargo test -p buiy_core --test text_commit --test text_measure --test text_sync --test layout_pipeline_order`
Expected: unchanged green (the producer must not disturb the main-world text path).

- [ ] **Step 7: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(text): T4 task 4 — extract_buiy_glyphs producer + § 6.2 damage gate

The render-world glyph producer in ExtractSchedule .after(maintain_atlas):
painters_z walk (shared with extract_buiy_nodes via context_tree_paint_order),
physical() quantization, structured keys, get_image_uncached-on-miss under a
lazy once-per-frame lock (site #3), straight-alpha TextColor resolve,
self-inclusive clip, retain-with-probe gate incl. the cached-f32 scale
compare (never Changed<Window> — test-pinned), GlyphMetaCache bearings
(AtlasEntry carries no bearings — recorded erratum). Adapterless extract
harness per verification § 1.2."
```

---

### Task 5: The un-gated touch pass — survival, drain, and the seam contract (headless)

The touch-pass code landed in Task 4 (it is part of the system); this task pins its § 6.3 contract and the seam-isolation contract with dedicated tests.

**Files:**
- Test: `crates/buiy_core/tests/text_touch_pass.rs`

- [ ] **Step 1: Write the tests**

Create `crates/buiy_core/tests/text_touch_pass.rs`:

```rust
//! § 6.3 — the eviction-under-retention hazard, CPU half (verification
//! § 12 headless d): against the real device-free `BuiyAtlas`, a retained
//! visible glyph survives > eviction_grace frames; an off-screen one
//! drains. Plus the § 12 (e) seam contract: no cosmic-text type crosses
//! into the render module.

mod support;

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::AtlasConfig;
use buiy_core::text::Text;
use support::extract_harness::TextExtractHarness;

const GRACE: u32 = 3;

fn harness() -> TextExtractHarness {
    TextExtractHarness::with_atlas_config(AtlasConfig {
        page_size: 1024,
        page_budget: 8,
        eviction_grace: GRACE,
    })
}

fn spawn_text(h: &mut TextExtractHarness, s: &str) -> Entity {
    let text = h
        .app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from(s))))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    text
}

#[test]
fn retained_visible_keys_survive_past_eviction_grace() {
    let mut h = harness();
    spawn_text(&mut h, "warm");
    h.settle();
    let keys = h.resident_keys();
    assert!(!keys.is_empty());

    // Steady frames ≫ grace: NO rebuild happens (Task 4 pins that), so
    // without the un-gated touch pass these keys would grace-evict while
    // the retained instances still sample them — the silent-corruption
    // hazard. The touch pass is the only per-frame text work.
    let settled = h.changed_frames();
    for _ in 0..(GRACE * 4) {
        h.frame();
    }
    assert_eq!(h.changed_frames(), settled, "no rebuild occurred (retention held)");
    for key in &keys {
        assert!(
            h.atlas().get(key).is_some(),
            "visible key survived idle > eviction_grace — the § 6.3 touch pass"
        );
    }
}

#[test]
fn offscreen_keys_drain_after_grace() {
    let mut h = harness();
    // "warm" and "cold" share no letters, so their glyph-key sets are
    // disjoint — the prune assertion below cannot be revived by re-inserts.
    let text = spawn_text(&mut h, "warm");
    h.settle();
    let keys = h.resident_keys();
    assert!(!keys.is_empty());

    // Despawn: the rebuild empties ResidentTextKeys, so nothing touches the
    // old keys — they must drain within the grace window (gate #15's
    // return-to-baseline depends on exactly this).
    h.app.world_mut().entity_mut(text).despawn();
    h.frame();
    assert!(h.resident_keys().is_empty());
    for _ in 0..(GRACE + 2) {
        h.frame();
    }
    for key in &keys {
        assert!(
            h.atlas().get(key).is_none(),
            "off-screen key drained after grace (no touch pass member keeps it warm)"
        );
    }
    // Decision 3 invariant: the bearing cache prunes to residency — the
    // prune runs on REBUILD frames, so force one (the disjoint "cold" text)
    // and observe the drained keys leave the cache.
    spawn_text(&mut h, "cold");
    h.frame();
    let meta = h.render.resource::<buiy_core::text::GlyphMetaCache>();
    for key in &keys {
        assert!(!meta.0.contains_key(key), "bearing cache pruned to residency");
    }
}

/// verification § 1.2's seam-contract row, half 1: the whole producer flow
/// is expressible against the render seam types alone — an `AtlasKey` is
/// opaque bytes, residency is `get_or_insert` with an `AtlasBitmap`, and
/// the output is a `GlyphAlphaInstance`. No cosmic-text type appears in
/// this function's signature or body.
#[test]
fn seam_speaks_only_render_types() {
    use buiy_core::render::atlas::{
        AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, BuiyAtlas, GlyphAlphaInstance,
    };
    use bevy::math::UVec2;

    fn stub_producer(atlas: &mut BuiyAtlas) -> Vec<GlyphAlphaInstance> {
        let key = AtlasKey::from_bytes(&[0u8, 1, 2, 3]); // opaque to the atlas
        let entry = atlas.get_or_insert(key, AtlasFormat::CoverageR8, || AtlasBitmap {
            size: UVec2::splat(4),
            format: AtlasFormat::CoverageR8,
            data: vec![0xFF; 16],
        });
        vec![GlyphAlphaInstance {
            rect: [0.0, 0.0, 4.0, 4.0],
            uv: [entry.uv.min.x, entry.uv.min.y, entry.uv.max.x, entry.uv.max.y],
            color: [1.0, 1.0, 1.0, 1.0],
            clip: [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY],
            page: entry.page as u32,
        }]
    }

    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    assert_eq!(stub_producer(&mut atlas).len(), 1);
}

/// Half 2 — the drift tripwire: the render atlas module never imports
/// cosmic-text (the construction of glyph keys is the text module's;
/// atlas/mod.rs's seam doc says "no cosmic-text type ever crosses").
#[test]
fn render_atlas_sources_never_name_cosmic_text() {
    for src in [
        include_str!("../src/render/atlas/mod.rs"),
        include_str!("../src/render/atlas/types.rs"),
        include_str!("../src/render/atlas/atlas.rs"),
        include_str!("../src/render/atlas/primitive.rs"),
        include_str!("../src/render/atlas/warmup.rs"),
        include_str!("../src/render/atlas/gpu.rs"),
    ] {
        assert!(
            !src.contains("cosmic_text"),
            "a cosmic-text type crossed into the render atlas seam"
        );
    }
}
```

- [ ] **Step 2: Run, verify green**

Run: `cargo test -p buiy_core --test text_touch_pass`
Expected: 4 passed (the production code already exists; these tests pin contracts — if `offscreen_keys_drain_after_grace` fails, the touch pass is touching stale keys: a real bug, fix the system).

- [ ] **Step 3: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "test(text): T4 task 5 — § 6.3 touch-pass survival/drain + seam contract

Headless against the device-free BuiyAtlas: visible keys survive > grace,
off-screen keys drain, bearing cache prunes with residency; the stub-atlas
seam test + a no-cosmic-in-render-atlas source tripwire (§ 12 d/e)."
```

---

### Task 6: `wait_for_fonts` + `warm_atlas`, realized (headless; enables the GPU fixtures)

**Files:**
- Modify: `crates/buiy_core/src/render/golden.rs`
- Modify: `crates/buiy_core/tests/support/mod.rs`
- Test: `crates/buiy_core/tests/render_golden_harness.rs` (extend)

- [ ] **Step 1: Write the failing test**

Append to `crates/buiy_core/tests/render_golden_harness.rs`:

```rust
/// verification § 3.2 — wait_for_fonts flips from declared flag to
/// implemented predicate: warmup queue drained AND every fixture key
/// resident, probed via the no-LRU-touch `BuiyAtlas::get`.
#[test]
fn fonts_ready_requires_drained_queue_and_resident_keys() {
    use bevy::math::UVec2;
    use buiy_core::render::atlas::{
        AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, AtlasWarmupQueue, AtlasWarmupRequest,
        BuiyAtlas,
    };
    use buiy_core::render::golden::fonts_ready;

    let bitmap = || AtlasBitmap {
        size: UVec2::splat(4),
        format: AtlasFormat::CoverageR8,
        data: vec![0xFF; 16],
    };
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let mut queue = AtlasWarmupQueue::default();
    let key = AtlasKey::from_bytes(b"ready-probe");

    assert!(!fonts_ready(&atlas, &queue, std::slice::from_ref(&key)), "missing key: not ready");

    atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, bitmap);
    assert!(fonts_ready(&atlas, &queue, std::slice::from_ref(&key)), "resident + drained: ready");

    queue.push(AtlasWarmupRequest {
        key: AtlasKey::from_bytes(b"pending"),
        format: AtlasFormat::CoverageR8,
        bitmap: bitmap(),
    });
    assert!(!fonts_ready(&atlas, &queue, std::slice::from_ref(&key)), "pending warmup: not ready");
    atlas.drain_warmup(&mut queue);
    assert!(fonts_ready(&atlas, &queue, std::slice::from_ref(&key)));
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p buiy_core --test render_golden_harness fonts_ready`
Expected: compile error — `fonts_ready` does not exist.

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/render/golden.rs`, add (with the import `use crate::render::atlas::{AtlasKey, AtlasWarmupQueue, BuiyAtlas};`):

```rust
/// verification § 3.2 — [`GoldenConfig::wait_for_fonts`], flipped from
/// declared flag to implemented predicate. With embedded deterministic
/// fonts, registration is synchronous at `FontSystem` construction (nothing
/// asynchronous exists to wait on), so "fonts ready" reduces to: the warmup
/// queue is drained AND every glyph key the fixture's producer emitted is
/// resident — probed via the **no-LRU-touch** [`BuiyAtlas::get`], so the
/// check never perturbs eviction order.
///
/// § 3.3 (`warm_atlas`) is satisfied STRUCTURALLY for text fixtures: the
/// producer inserts at extract, before Prepare's upload and the node's draw
/// (glyph-pipeline § 6.4), so by the time this predicate holds the atlas is
/// warm. `AtlasWarmupQueue` remains the seam for the optional production
/// ASCII pre-warm (deferred — text campaign T9) and T6's solid stamp.
pub fn fonts_ready(atlas: &BuiyAtlas, warmup: &AtlasWarmupQueue, visible_keys: &[AtlasKey]) -> bool {
    warmup.is_empty() && visible_keys.iter().all(|key| atlas.get(key).is_some())
}
```

In `crates/buiy_core/tests/support/mod.rs`, add the GPU-side poll helper (near `readback_rgba`):

```rust
/// Drive frames until the text fixture's `wait_for_fonts` predicate holds
/// (verification § 3.2): the producer has emitted (`ResidentTextKeys`
/// non-empty), the warmup queue is drained, and every emitted key is
/// resident. Returns frames driven; panics past `max_frames`.
pub fn wait_for_text_ready(app: &mut App, max_frames: usize) -> usize {
    use buiy_core::render::atlas::{AtlasWarmupQueue, BuiyAtlas};
    use buiy_core::render::golden::fonts_ready;
    use buiy_core::text::ResidentTextKeys;

    for frame in 0..max_frames {
        app.update();
        let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
        let world = render_app.world();
        let resident = world.resource::<ResidentTextKeys>();
        if !resident.keys.is_empty()
            && fonts_ready(
                world.resource::<BuiyAtlas>(),
                world.resource::<AtlasWarmupQueue>(),
                &resident.keys,
            )
        {
            return frame + 1;
        }
    }
    panic!("text never became atlas-resident within {max_frames} frames");
}
```

- [ ] **Step 4: Run, verify green**

Run: `cargo test -p buiy_core --test render_golden_harness`
Expected: all pass (the `#[ignore]` capture test is skipped headless).

- [ ] **Step 5: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(render): T4 task 6 — wait_for_fonts predicate + warm_atlas realized

verification § 3.2/3.3: fonts_ready(atlas, queue, keys) via the
no-LRU-touch get probe; warm_atlas is structural for text fixtures (the
producer inserts pre-paint). wait_for_text_ready poll helper for the GPU
harness. ASCII pre-warm stays out (T9)."
```

---

### Task 7: GPU lane — real-entity fixtures; delete the test-as-producer fill

**Files:**
- Modify: `crates/buiy_core/tests/support/mod.rs` (text plugin + shared pixel helpers)
- Create: `crates/buiy_core/tests/text_gpu.rs`
- Modify: `crates/buiy_core/tests/atlas_gpu.rs` (slim to gate-#15)

- [ ] **Step 1: Add `BuiyTextPlugin` + shared helpers to the support harness**

In `crates/buiy_core/tests/support/mod.rs`:

(a) In BOTH `gpu_test_app()` and `gpu_render_app()`, insert directly after `.add_plugins(CorePlugin)`:

```rust
        // The text engine + the T4 glyph producer (render half registers
        // against the live RenderApp created by RenderPlugin above).
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
```

(b) Move these two helpers here from `atlas_gpu.rs` as `pub fn`s (verbatim bodies):

```rust
/// Index one RGBA8 pixel out of an un-padded `w*h*4` readback buffer.
pub fn px(pixels: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The sRGB8 the target stores for a FULL-coverage texel of linear
/// straight-alpha `color` over the opaque-black clear: SrcOver in linear
/// (dst = 0), then the Rgba8UnormSrgb linear→sRGB encode.
pub fn expected_full_coverage_srgb(color: [f32; 4]) -> [u8; 4] {
    let a = color[3];
    let lin = LinearRgba::new(color[0] * a, color[1] * a, color[2] * a, 1.0);
    let s = Srgba::from(lin);
    [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255,
    ]
}
```

Run the existing GPU lane now: `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`
Expected: still green — the producer running inside the existing GPU tests is a no-op for textless fixtures (empty rebuild on the first theme tick, then retained). If anything regresses here, root-cause before continuing.

- [ ] **Step 2: Write the real-entity GPU tests**

Create `crates/buiy_core/tests/text_gpu.rs`:

```rust
//! GPU end-to-end TEXT tests (T4): real entities through TextSync →
//! measure → TextCommit → extract_buiy_glyphs → BuiyAtlas → the coverage
//! draw — the real-producer replacement for atlas_gpu.rs's deleted
//! test-as-producer fills (glyph-pipeline § 12 GPU a/b/c). All #[ignore]:
//! need a wgpu adapter (CLAUDE.md GPU lane).
//!
//! Run: cargo test -p buiy_core --test text_gpu -- --ignored --test-threads=1

mod support;

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::atlas::{
    AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, BuiyAtlas,
};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::TextColor;
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::{FontSize, ResidentTextKeys, Text};
use std::borrow::Cow;

const W: u32 = 128;
const H: u32 = 64;
const TOKEN: &str = "test.text";

/// One big themed line ("Hi", 40 px — thick stems guarantee full-coverage
/// interior texels) under a sized column root.
fn spawn_text_fixture(app: &mut App, color: Color) {
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TOKEN.into(), color);
    }
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TOKEN))),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(W as f32).height_px(H as f32),
        ))
        .add_child(text);
}

/// Build app → fixture → capture the first text-ready frame.
fn capture(color: Color) -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = support::gpu_render_app(W, H);
    spawn_text_fixture(&mut app, color);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    // wait_for_fonts, realized (Task 6): producer emitted + queue drained +
    // every key resident — warm_atlas is structural (§ 6.4).
    support::wait_for_text_ready(&mut app, 60);
    support::readback_rgba(&mut app, target)
}

/// Brightest painted pixel ≈ a full-coverage texel of the tint.
fn brightest(pixels: &[u8]) -> [u8; 4] {
    pixels
        .chunks_exact(4)
        .max_by_key(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .unwrap()
}

// --- (a) gate-#2 hello-text: first painted frame, deterministic. ----------
#[test]
#[ignore = "needs a wgpu adapter; gate-#2 hello-text inline golden (glyph-pipeline § 12 GPU a)"]
fn hello_text_first_frame_is_deterministic_and_tinted() {
    let tint = Color::srgba(0.10, 0.85, 0.30, 1.0);
    let frame_a = capture(tint);

    // Backdrop reads the opaque-black clear; something painted.
    assert_eq!(support::px(&frame_a, W, W - 2, H - 2), [0, 0, 0, 255]);
    assert!(
        frame_a.chunks_exact(4).any(|p| p != [0, 0, 0, 255]),
        "the glyphs painted at least one pixel"
    );

    // Alpha-as-color: a full-coverage stroke-interior texel reads exactly
    // the linearized instance tint (atlas stores coverage, never color).
    let lin = LinearRgba::from(tint);
    let expected = support::expected_full_coverage_srgb([lin.red, lin.green, lin.blue, lin.alpha]);
    let got = brightest(&frame_a);
    const TOL: i32 = 4;
    for ch in 0..3 {
        assert!(
            (got[ch] as i32 - expected[ch] as i32).abs() <= TOL,
            "brightest texel channel {ch}: got {} expected {} (±{TOL}) — full \
             pixel got={got:?} expected={expected:?}",
            got[ch],
            expected[ch],
        );
    }

    // gate-#2 determinism: an independent fresh capture matches (the
    // stored-PNG machinery stays deferred; the re-capture IS the golden).
    let frame_b = capture(tint);
    let diff = perceptual_diff(&frame_a, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: perceptual_diff = {diff}");
}

// --- (b) retint byte-identity with REAL text (§ 7's contract). ------------
#[test]
#[ignore = "needs a wgpu adapter; retint byte-identity with real text (glyph-pipeline § 12 GPU b)"]
fn retint_real_text_leaves_atlas_byte_identical() {
    let mut app = support::gpu_render_app(W, H);
    spawn_text_fixture(&mut app, Color::srgba(0.85, 0.10, 0.10, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_a = support::readback_rgba(&mut app, target.clone());
    let atlas_a = coverage_page0_bytes(&app);

    // Theme swap: theme.is_changed() re-fires the § 6.2 gate; instances
    // re-emit with the new color; the atlas must not move.
    app.world_mut()
        .resource_mut::<buiy_core::theme::Theme>()
        .colors
        .insert(TOKEN.into(), Color::srgba(0.10, 0.20, 0.90, 1.0));
    for _ in 0..3 {
        app.update();
    }
    let frame_b = support::readback_rgba(&mut app, target);
    let atlas_b = coverage_page0_bytes(&app);

    assert_eq!(
        atlas_a, atlas_b,
        "CoverageR8 page byte-identical across the retint — tint is \
         per-instance, never a key input (§ 5.1/§ 7)"
    );
    assert!(
        perceptual_diff(&frame_a, &frame_b) > 5e-4,
        "the retint is visible in the framebuffer (byte-identity is not vacuous)"
    );
}

fn coverage_page0_bytes(app: &App) -> Vec<u8> {
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<BuiyAtlas>()
        .page_pixels(AtlasFormat::CoverageR8, 0)
        .expect("a coverage page exists after the producer ran")
        .to_vec()
}

// --- (c) eviction-under-retention: the § 6.3 hazard, both halves. ---------
#[test]
#[ignore = "needs a wgpu adapter; eviction-under-retention regression (glyph-pipeline § 12 GPU c)"]
fn touch_pass_prevents_stale_uv_corruption() {
    let mut app = support::gpu_render_app(W, H);
    // Short grace so idling past it is cheap. Replace BEFORE any frame runs.
    {
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app.world_mut().insert_resource(BuiyAtlas::new(AtlasConfig {
            page_size: 1024,
            page_budget: 8,
            eviction_grace: 3,
        }));
    }
    spawn_text_fixture(&mut app, Color::srgba(0.90, 0.90, 0.20, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_a = support::readback_rgba(&mut app, target.clone());

    // Half 1 — touch pass ON (production shape): idle ≫ grace, keys stay
    // resident, pixels stay put.
    for _ in 0..12 {
        app.update();
    }
    let keys: Vec<AtlasKey> = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app.world().resource::<ResidentTextKeys>().keys.clone()
    };
    assert!(!keys.is_empty());
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        for key in &keys {
            assert!(atlas.get(key).is_some(), "touch pass kept the visible key resident");
        }
    }
    let frame_b = support::readback_rgba(&mut app, target.clone());
    assert!(perceptual_diff(&frame_a, &frame_b) < 1e-4, "retained frames render identically");

    // Half 2 — the hazard a DISABLED touch pass would allow, simulated
    // (decision 7: no prod flag — we force the eviction directly): evict a
    // still-referenced key, insert a same-size filler (guillotiere reuses
    // the freed cell — asserted), never damage the main world, and watch
    // the retained instances' stale UVs sample the filler.
    let victim = keys[0].clone();
    let old_px = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        render_app.world().resource::<BuiyAtlas>().get(&victim).unwrap().px
    };
    {
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        let mut atlas = render_app.world_mut().resource_mut::<BuiyAtlas>();
        atlas.evict_for_test(&victim);
        let size = old_px.size();
        let filler = atlas.get_or_insert(
            AtlasKey::from_bytes(b"eviction-hazard-filler"),
            AtlasFormat::CoverageR8,
            move || AtlasBitmap {
                size,
                format: AtlasFormat::CoverageR8,
                data: vec![0xFF; (size.x * size.y) as usize],
            },
        );
        assert_eq!(
            filler.px, old_px,
            "the filler reused the freed cell — the aliasing the hazard is made of"
        );
    }
    for _ in 0..2 {
        app.update();
    }
    // Guard: no rebuild re-rasterized the victim (retention really held).
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        assert!(
            render_app.world().resource::<BuiyAtlas>().get(&victim).is_none(),
            "no rebuild occurred during the hazard window"
        );
    }
    let frame_c = support::readback_rgba(&mut app, target);
    assert!(
        perceptual_diff(&frame_a, &frame_c) > 1e-4,
        "stale UVs sampled the filler — the silent corruption § 6.3's \
         un-gated touch pass exists to prevent"
    );
}
```

- [ ] **Step 3: Slim `atlas_gpu.rs` to the atlas-mechanics test**

In `crates/buiy_core/tests/atlas_gpu.rs`:
- Delete tests (1) `warmed_glyph_uploads_and_samples_with_tint`, (2) `retint_same_glyph_leaves_atlas_byte_identical`, (3) `warmup_makes_first_frame_match_golden`, and the helpers `warmup_coverage`, `set_glyphs`, `glyph`, `coverage_page0_bytes`, `px`, `expected_full_coverage_srgb`, `NEG_INF`/`POS_INF` (the real-entity equivalents live in `text_gpu.rs`; first-frame residency is now structural — § 6.4).
- Keep `full_coverage` and test (4) `gate15_atlas_entries_return_to_baseline_after_idle` unchanged.
- Rewrite the file header:

```rust
//! GPU atlas-mechanics tests: gate-#15 idle-settle on a real adapter.
//! The former test-as-producer fills (warmup_coverage/set_glyphs emitting
//! GlyphAlphaInstances directly) were REPLACED in T4 by real-entity text
//! fixtures in tests/text_gpu.rs — the in-crate producer
//! (text::extract_buiy_glyphs) now owns that seam. The warmup queue's GPU
//! consumer coverage returns with T6's solid-stamp push.
//!
//! Run: cargo test -p buiy_core --test atlas_gpu -- --ignored --test-threads=1
```
- Trim now-unused imports until clippy is clean.

- [ ] **Step 4: Run the GPU lane**

Run: `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`
Expected: all `#[ignore]` tests pass, including the three new text_gpu tests. Debug notes: if `hello_text_first_frame…`'s brightest pixel misses the tint, check the `content_offset` fold and the straight-alpha (NOT premultiplied) color first; if `filler.px != old_px` in (c), the allocator did not reuse the cell — try the exact evicted size (it is exact already) before touching the assertion; the assertion failing loudly is preferable to a vacuous pass.

- [ ] **Step 5: Run the headless gate, commit**

```bash
git add -A
git commit -m "test(text): T4 task 7 — real-entity GPU fixtures; delete test-as-producer fill

text_gpu.rs: gate-#2 hello-text inline golden (deterministic re-capture),
retint byte-identity with real text, eviction-under-retention pair (touch
pass keeps pixels; simulated disable corrupts — decision 7, no prod flag).
atlas_gpu.rs slimmed to the gate-#15 mechanics test; BuiyTextPlugin joins
both support app builders."
```

---

### Task 8: The `hello_text` example — the exit criterion

**Files:**
- Create: `examples/hello_text/Cargo.toml`
- Create: `examples/hello_text/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create the crate**

`examples/hello_text/Cargo.toml`:

```toml
[package]
name = "hello_text"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
bevy = { workspace = true, features = ["bevy_render", "bevy_winit", "x11", "wayland"] }
buiy = { path = "../../crates/buiy" }
```

`examples/hello_text/src/main.rs`:

```rust
//! Buiy text T4 hello-world: a themed paragraph through the full pipeline —
//! TextSync → Taffy measure → TextCommit → extract_buiy_glyphs →
//! BuiyAtlas (CoverageR8) → the alpha-as-color glyph draw.
//!
//! The automated twin of this scene is `tests/text_gpu.rs`'s gate-#2
//! fixture; this binary is the human-eyes smoke test
//! (`cargo run -p hello_text`).

use bevy::prelude::*;
use buiy::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // TextColor::default() == CurrentColor — the theme default foreground.
    let title = commands
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hello, Buiy text!")),
            FontSize(32.0),
        ))
        .id();
    let body = commands
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "The quick brown fox jumps over the lazy dog. Shaped by \
                 cosmic-text at the committed wrap width, rasterized once per \
                 (font, size, weight, subpixel-bin) into the shared coverage \
                 atlas, tinted per instance — a theme switch never touches \
                 the atlas.",
            )),
            FontSize(16.0),
        ))
        .id();
    commands
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(560.0)
                .padding(24.0)
                .gap_px(12.0),
        ))
        .add_children(&[title, body]);
}
```

Add `"examples/hello_text",` to the workspace `members` list in the root `Cargo.toml` (after `"examples/hello_button",`).

- [ ] **Step 2: Build headless, run on the GPU host**

Run: `cargo build -p hello_text`
Expected: clean build.

Run (manual, semantic verification — no "should work" claims): `cargo run -p hello_text`
Expected: a window with a 32 px title and a wrapped 16 px paragraph in the theme foreground color, crisp at the desktop scale factor. Close it. If text is invisible, check `TextColor` resolve (`color.text.primary` in the active theme) before anything else; if it is in the wrong place, check the `content_offset` fold (padding 24 must displace the lines).

- [ ] **Step 3: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(examples): T4 task 8 — hello_text (the § 1 exit criterion)

Themed paragraph end-to-end; automated twin is tests/text_gpu.rs's gate-#2
fixture. Verified by running it on the GPU host."
```

---

### Task 9: Docs flip + plan self-review

**Files:**
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Campaign plan**

In `docs/plans/2026-06-09-buiy-text-campaign.md`:

(a) Phase-status table: `| T4 | First pixels | proposed |` → `landed`.

(b) Append to the T4 section, mirroring the T1–T3 errata convention:

```markdown
- **T4 errata for the spec edit pass** (mechanical inaccuracies found while
  implementing — see the T4 plan's decisions 2–5; superseding context, not a
  silent contradiction):
  1. *glyph-pipeline § 2 step 0 / § 6.1's `TextBufferAccess` read-only form*
     — superseded by the T3 decision-12 deferral (already a T3 erratum): the
     producer binds `&TextBuffer` directly until the editing campaign lands
     `TextEditState`; the swap is mechanical.
  2. *glyph-pipeline § 5.2's "`AtlasEntry.px` exists precisely for this snap
     math"* — `px` carries the cell SIZE only; the bearings
     (`Placement.left/top`) are not recoverable from the seam on a cache
     hit. As built: a producer-owned `GlyphMetaCache(HashMap<AtlasKey,
     GlyphBearing>)` written on rasterize and pruned to atlas residency
     (bearings are a pure function of the `CacheKey`, so entries can never
     go stale). Runner-ups (widen `AtlasEntry`; re-rasterize per rebuild)
     rejected in the T4 plan.
  3. *glyph-pipeline § 2 step 4's literal one-closure shape* cannot encode
     the zero-coverage / `SwashContent::Color` skips (the closure must
     return a bitmap): as built, residency probe + prebuilt-bitmap closure;
     raster work and the lock still happen only on a miss (the lock is
     taken lazily once per frame, the `text_commit` guard pattern).
  4. *glyph-pipeline § 5.1's "content origin"* had no pinned source: as
     built, `ComputedTextLayout.content_offset` (border + padding), written
     idempotently by `TextCommit` — damage rides the existing
     `Changed<ComputedTextLayout>` probe.
  5. *glyph-pipeline Sources* say 0.19.0 resolves swash 0.2.7 — the lock
     resolves **0.2.8** (`Placement`/`SwashContent`/`SwashImage` shapes
     verified identical).
  6. *§ 6.2's "rebuild `ExtractedTextQuads` alongside"* and the
     `Changed<CaretVisual>`/`Changed<SelectionVisual>` union members are
     T6/T7 joins (the carriers do not exist yet) — ledger comments in
     `extract_buiy_glyphs` mark both seats.
```

- [ ] **Step 2: docs/README.md**

After the T3 plan line in the text-plans block, add:

```markdown
- [Buiy text T4 — First pixels](plans/2026-06-10-buiy-text-t4-first-pixels.md) — the glyph producer: `extract_buiy_glyphs` in `ExtractSchedule` `.after(maintain_atlas)` (`physical()` 4-bin quantization, 19 B structured `AtlasKey` + `FontKeyInterner`, `get_image_uncached`-on-miss = lock site #3, § 5.2 rect math + straight-alpha `TextColor` + self-inclusive clip), the § 6.2 retain-with-probe damage gate (cached-`f32` scale compare, never `Changed<Window>`) + the un-gated `ResidentTextKeys` touch pass, `GlyphMetaCache` bearings, color-emoji skip+warn (IconInstance seam named), real-entity GPU fixtures replacing atlas_gpu.rs's test-as-producer fill, `wait_for_fonts`/`warm_atlas` realized (`fonts_ready`), the `hello_text` example. Cross-layer quad/glyph interleave stays a render-spec dependency (z-artifacts on layered fixtures are expected until it lands). `[landed]`
```

(Adjust the trailing status marker to match reality at merge time.)

- [ ] **Step 3: CLAUDE.md**

In the GPU-lane paragraph, extend the parenthetical test list:
"(pipeline creation, the extract→prepare→node draw spine, render-to-texture + pixel readback, atlas, compositor)" → "(pipeline creation, the extract→prepare→node draw spine, render-to-texture + pixel readback, atlas, compositor, the text glyph producer)".

- [ ] **Step 4: Self-review (the implementer runs this checklist, fixes inline)**

1. **Charter coverage:** every T4 charter clause has a landed artifact — `.after(maintain_atlas)` (Task 4 registration), `physical()` 4-bin (Task 2 pins + Task 4 call), 19 B key + interner (Task 1), uncached-on-miss lock #3 (Task 4), rect/color/clip emission (Tasks 2–4), retain-with-probe + un-gated touch (Tasks 4–5), color-emoji skip+warn (Task 4), atlas_gpu.rs fill deleted for real entities (Task 7), hello_text (Task 8), wait_for_fonts/warm_atlas realized (Task 6), ASCII pre-warm absent (grep `AtlasWarmupQueue` in `src/text/` — must be zero hits), the interleave note present (plan header + node.rs comment).
2. **Lock-site ledger:** grep `fonts.lock()` / `\.lock()` over `crates/buiy_core/src` — exactly three sites (measure, commit, `resolve_glyph`). A fourth is a review reject.
3. **`#[ignore]` audit:** every test in `text_gpu.rs` carries it; no new test without it touches an adapter (the headless gate run proves this).
4. **Steady-state audit:** `text_extract.rs`'s steady/cursor tests green; `prepare.rs`'s glyph gate untouched.
5. **Type consistency:** `GlyphBearing`/`glyph_rect_logical(phys_x, phys_y, bearing, size, scale)`/`fonts_ready(atlas, warmup, keys)` signatures match across tasks 2/4/6/7.
6. Both gate lanes green at HEAD.

- [ ] **Step 5: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "docs(text): T4 docs flip — campaign status, errata, README catalog, CLAUDE.md

T4 landed: producer + damage gate + touch pass + GPU fixtures + golden
predicates + hello_text. Errata 1-6 recorded for the spec edit pass."
```
