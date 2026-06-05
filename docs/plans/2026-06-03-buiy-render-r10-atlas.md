# Texture Atlas Infrastructure + Glyph/Icon Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the shared render-world `BuiyAtlas` resource (guillotiere allocation, LRU eviction, page-budget pressure, warmup, pooling) plus the two F-tier atlas-sampling primitive shapes (`GlyphAlphaInstance`, `IconInstance`) and the `get_or_insert`/`AtlasEntry` seam that `buiy-text-rendering-design` plugs into.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [atlas-and-text-seam.md](../specs/2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md) in full (§ 2 resource, § 3 insert API, § 4 primitives, § 6 reserved gradient/mask entry kinds, § 7 verification).
**Architecture:** One render-world `BuiyAtlas` resource owns a `guillotiere::AtlasAllocator` per page per format (`CoverageR8`/`ColorRgba8`), a content-addressed `entries` map keyed by an opaque `AtlasKey`, an LRU recency ring, and a free-page pool. `get_or_insert(key, format, closure)` is idempotent: a hit touches LRU and returns the resident `AtlasEntry`; a miss allocates (evicting LRU under page-budget pressure), blits the closure's `AtlasBitmap`, and records the entry. Text rendering produces coverage + primitives; this spec is the warehouse and the primitive *kind* they speak.
**Tier/Test reality:** HEADLESS for everything except actual GPU upload+sampling. The allocator (guillotiere), `entries`/`lru` maps, page-budget eviction, pooling, warmup-drain, and the primitive `bytemuck::Pod` layouts are **pure CPU** — they need no wgpu adapter and are the real gating tests (`cargo test --workspace` on this xvfb-less, adapter-less host). The atlas texture upload + the actual sampling draw + the gate-#2 warmup golden + the gate-#15 atlas-entries-return-to-baseline e2e fixture need a wgpu adapter and are written as `#[ignore]` exactly like `render_smoke.rs`.

---

## Conventions for every task

**The gate (must stay green at every commit — this host + CI have NO xvfb and NO wgpu adapter):**

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

- **HEADLESS** tasks add gating tests that run on CI. **GPU** assertions are `#[ignore]`d with a comment naming why (no wgpu adapter), mirroring `crates/buiy_core/tests/render_smoke.rs`.
- All new atlas code lives in a new module tree `crates/buiy_core/src/render/atlas/`. Public re-exports go through `crates/buiy_core/src/render/mod.rs`.
- Type/field names are taken **verbatim** from spec § 2–§ 4. Do not rename `BuiyAtlas`, `AtlasKey`, `AtlasEntry`, `AtlasBitmap`, `AtlasFormat`, `AtlasConfig`, `AtlasPage`, `GlyphAlphaInstance`, `IconInstance`, `get_or_insert`, `get`, `warmup_atlas`, `AtlasWarmupQueue`.
- `guillotiere` is a **direct, version-pinned** dependency (spec § 2.1): `bevy_image 0.18.1` resolves `guillotiere 0.6.2` (verified via `cargo tree -p bevy_image -i guillotiere`). Pin to exactly `0.6.2`.
- `smallvec` (for `AtlasKey(SmallVec<[u8; 24]>)`) is a direct dependency: `1.15.1` is already in the lockfile transitively; pin `1`.
- guillotiere API facts (verified against the 0.6.2 source):
  - `guillotiere::AtlasAllocator::new(size: guillotiere::Size) -> Self`
  - `allocate(&mut self, size: guillotiere::Size) -> Option<guillotiere::Allocation>`
  - `deallocate(&mut self, id: guillotiere::AllocId)`
  - `is_empty(&self) -> bool`
  - `guillotiere::Allocation { id: AllocId, rectangle: Rectangle }`
  - `guillotiere::Rectangle = euclid::default::Box2D<i32>` with public `.min` / `.max` `Point2D<i32>` fields (`.min.x`, `.min.y`, `.max.x`, `.max.y` are `i32`).
  - `guillotiere::size2(w: i32, h: i32) -> guillotiere::Size` builds a `Size`.

---

## Task 1 — Add `guillotiere` + `smallvec` direct deps, scaffold the atlas module (HEADLESS)

Pins the allocator crate so a `bevy_image` patch bump cannot drop it (spec § 2.1 callout), and stands up an empty `render::atlas` module that compiles and is reachable.

**Files**
- Modify: `Cargo.toml` (workspace `[workspace.dependencies]`)
- Modify: `crates/buiy_core/Cargo.toml`
- Create: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Create (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Write the failing scaffolding test first. Create `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  //! Headless unit tests for the BuiyAtlas allocator + LRU + pooling logic.
  //! Pure-CPU (guillotiere is CPU); no wgpu adapter required, so these gate on CI.
  use buiy_core::render::atlas::AtlasFormat;

  #[test]
  fn atlas_module_is_reachable() {
      // Compile-time proof the module + a public type are wired through
      // render::mod. Real allocator tests land in Task 4+.
      let f = AtlasFormat::CoverageR8;
      assert_ne!(f, AtlasFormat::ColorRgba8);
  }
  ```
- [ ] Run it, expect a **compile FAIL** (`render::atlas` does not exist):
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Add deps. In root `Cargo.toml` under `[workspace.dependencies]` add:
  ```toml
  # guillotiere is only a *transitive* dep of bevy_image (not re-exported);
  # pin it directly to the version bevy_image 0.18.1 resolves so a bevy patch
  # bump cannot drop the atlas allocator. Spec atlas-and-text-seam.md § 2.1.
  guillotiere = "=0.6.2"
  smallvec = "1"
  ```
- [ ] In `crates/buiy_core/Cargo.toml` `[dependencies]` add:
  ```toml
  guillotiere = { workspace = true }
  smallvec = { workspace = true }
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/mod.rs`:
  ```rust
  //! Buiy's shared texture atlas: the warehouse all coverage-and-image
  //! primitives (glyph / icon / gradient / mask) sample. One render-world
  //! `BuiyAtlas` resource; allocation via `guillotiere`, content-addressed
  //! entries, LRU eviction, page-budget pressure, warmup, and pooling.
  //!
  //! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md.
  //! This spec owns the atlas + the glyph-alpha / icon primitive *shapes*;
  //! `buiy-text-rendering-design` owns shaping and produces coverage bitmaps,
  //! plugging in through the § 3 `get_or_insert` API only.

  /// The two backing-texture formats. Glyph/mask coverage is single-channel
  /// `R8`; icon/sprite and baked gradient stops are full-color `Rgba8`. A
  /// `guillotiere` page is one format — the two never share a page
  /// (spec § 2.2).
  #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
  pub enum AtlasFormat {
      /// `TextureFormat::R8Unorm` — alpha-as-color coverage (spec § 2.2, § 4.1).
      CoverageR8,
      /// `TextureFormat::Rgba8UnormSrgb` — full color; hardware sRGB→linear
      /// decode on sample keeps the all-linear shading invariant (spec § 2.2).
      ColorRgba8,
  }
  ```
- [ ] In `crates/buiy_core/src/render/mod.rs`, register the module under the existing `pub mod instance;` etc. block:
  ```rust
  pub mod atlas;
  ```
- [ ] Re-run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Run the full gate, expect green. Then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): pin guillotiere + smallvec, scaffold atlas module"
  ```

---

## Task 2 — `AtlasKey`, `AtlasBitmap`, `AtlasEntry` value types (HEADLESS)

The three seam value types. `AtlasKey` is the opaque content-addressed handle; `AtlasBitmap` is the CPU coverage/color bitmap handed in on a miss; `AtlasEntry` is the resident handle read back (spec § 2.1, § 3).

**Files**
- Create: `crates/buiy_core/src/render/atlas/types.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add a failing test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::{AtlasBitmap, AtlasEntry, AtlasKey};
  use bevy::math::{URect, UVec2, Vec2, Rect};

  #[test]
  fn atlas_key_is_eq_and_hash_by_content() {
      use std::collections::HashMap;
      let a = AtlasKey::from_bytes(&[1, 2, 3]);
      let b = AtlasKey::from_bytes(&[1, 2, 3]);
      let c = AtlasKey::from_bytes(&[1, 2, 4]);
      assert_eq!(a, b, "equal bytes -> equal key");
      assert_ne!(a, c);
      let mut m: HashMap<AtlasKey, u32> = HashMap::new();
      m.insert(a, 7);
      assert_eq!(m.get(&b), Some(&7), "equal key hashes to same slot");
  }

  #[test]
  fn atlas_bitmap_carries_size_format_and_data() {
      let bmp = AtlasBitmap {
          size: UVec2::new(2, 1),
          format: AtlasFormat::CoverageR8,
          data: vec![0xAB, 0xCD],
      };
      assert_eq!(bmp.size, UVec2::new(2, 1));
      assert_eq!(bmp.data.len(), 2);
  }

  #[test]
  fn atlas_entry_is_copy_and_carries_uv_and_px() {
      let e = AtlasEntry {
          page: 0,
          format: AtlasFormat::CoverageR8,
          uv: Rect::new(0.0, 0.0, 0.5, 0.5),
          px: URect::new(0, 0, 16, 16),
      };
      let e2 = e; // Copy
      assert_eq!(e2.page, 0);
      assert_eq!(e2.px, URect::new(0, 0, 16, 16));
      let _ = Vec2::ZERO; // keep the Vec2 import honest
  }
  ```
- [ ] Run, expect **compile FAIL** (types absent):
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/types.rs`:
  ```rust
  //! Seam value types: the opaque content-addressed key, the CPU bitmap
  //! handed in on a miss, and the resident-handle entry read back.
  //! Spec atlas-and-text-seam.md § 2.1, § 3.

  use bevy::math::{Rect, URect, UVec2};
  use smallvec::SmallVec;

  use super::AtlasFormat;

  /// Content-addressed, **opaque to the atlas**. The producer (text) defines
  /// what the bytes mean; the atlas treats it as an `Eq + Hash` identity for
  /// dedup + eviction. For glyphs, `buiy-text-rendering-design` builds it from
  /// `(FontId, subpixel_bucket, glyph_id, px_size)` — that construction is the
  /// text spec's concern, not this one. Spec § 3.
  #[derive(Clone, PartialEq, Eq, Hash, Debug)]
  pub struct AtlasKey(pub SmallVec<[u8; 24]>);

  impl AtlasKey {
      /// Build a key from a byte slice (the common producer-side path).
      pub fn from_bytes(bytes: &[u8]) -> Self {
          Self(SmallVec::from_slice(bytes))
      }
  }

  /// A CPU coverage/color bitmap handed to the atlas on a miss. `R8` for
  /// glyph/mask, `Rgba8` for icon/gradient. The atlas wraps it as a Bevy
  /// `Image` for the blit and never interprets it. Spec § 3.
  pub struct AtlasBitmap {
      pub size: UVec2,
      pub format: AtlasFormat,
      pub data: Vec<u8>,
  }

  /// The value the seam reads back after an insert (or a `get` probe).
  /// Spec § 2.1.
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct AtlasEntry {
      /// Index into `pages[format]`.
      pub page: u16,
      pub format: AtlasFormat,
      /// Normalized `[0,1]` UV rect into that page.
      pub uv: Rect,
      /// Pixel rect, for the subpixel-snap math text needs (spec § 4.3).
      pub px: URect,
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` add below the `AtlasFormat` enum:
  ```rust
  mod types;
  pub use types::{AtlasBitmap, AtlasEntry, AtlasKey};
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): AtlasKey/AtlasBitmap/AtlasEntry seam value types"
  ```

---

## Task 3 — `AtlasConfig` (page size + page budget + eviction grace) (HEADLESS)

The tunable knobs. Units are pinned by spec § 2.4: `page_budget` is a **page count** (1024×1024 pages), v1 default `8`; `eviction_grace` is a frame count; page size default 1024 (spec § 2.2). The *tuned* numbers are deferred to `buiy-verification-design`; this task pins units + v1 defaults.

**Files**
- Modify: `crates/buiy_core/src/render/atlas/types.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add a failing test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::AtlasConfig;

  #[test]
  fn atlas_config_v1_defaults_match_spec() {
      let c = AtlasConfig::default();
      // Spec § 2.2: default page size 1024x1024.
      assert_eq!(c.page_size, 1024, "default page size 1024 (spec § 2.2)");
      // Spec § 2.4: v1 default page_budget = 8 pages.
      assert_eq!(c.page_budget, 8, "v1 default page_budget = 8 (spec § 2.4)");
      // eviction_grace is a frame count; v1 picks a small nonzero default so
      // idle transient entries drain (spec § 2.4 step 3). Tuned value deferred.
      assert!(c.eviction_grace >= 1, "grace is at least one frame");
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Append to `crates/buiy_core/src/render/atlas/types.rs`:
  ```rust
  /// Tunable atlas knobs. **Units are pinned here; tuned numbers are deferred**
  /// to `buiy-verification-design` (spec § 2.4 "Open — tuned budget numbers").
  #[derive(Clone, Copy, Debug)]
  pub struct AtlasConfig {
      /// Edge length of each square page, in texels. Spec § 2.2 default: 1024.
      pub page_size: u32,
      /// **Maximum page count** per format (a count of `page_size`² pages, not
      /// a byte figure — pages are uniform-sized so a count *is* the memory
      /// cap). When an allocation would push a format's page set past this,
      /// eviction runs first; only if the LRU queue is exhausted and the entry
      /// still does not fit does a page append exceed the budget (the budget
      /// bounds steady state, never correctness). Spec § 2.4 v1 default: 8.
      pub page_budget: u16,
      /// An entry untouched for this many frames is eviction-eligible even
      /// without pressure, so an idle fixture's transient entries drain back
      /// out (spec § 2.4 step 3 — the clause that makes "return to baseline"
      /// hold). Tuned value deferred.
      pub eviction_grace: u32,
  }

  impl Default for AtlasConfig {
      fn default() -> Self {
          Self {
              page_size: 1024,
              page_budget: 8,
              eviction_grace: 60,
          }
      }
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` extend the re-export:
  ```rust
  pub use types::{AtlasBitmap, AtlasConfig, AtlasEntry, AtlasKey};
  ```
- [ ] Run, expect **PASS**. Full gate green, then commit:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  git add -A && git commit -m "feat(render/atlas): AtlasConfig with pinned units + v1 defaults"
  ```

---

## Task 4 — `AtlasPage` wrapping `guillotiere::AtlasAllocator` (HEADLESS)

The per-page allocator wrapper. Owns the raw `guillotiere::AtlasAllocator`, the page's CPU `Image` handle, and the `AtlasKey -> (AllocId, URect)` live map eviction uses to `deallocate(id)` (spec § 2.1). This task wires only the allocation half (allocate→rect); blit and entry recording land in Task 6.

**Files**
- Create: `crates/buiy_core/src/render/atlas/page.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add a failing test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::AtlasPage;
  use bevy::math::URect;

  #[test]
  fn page_allocates_a_rect_of_requested_size() {
      let mut page = AtlasPage::new(1024);
      let r: URect = page.try_alloc(URect::new(0, 0, 16, 32).size()).expect("fits");
      assert_eq!(r.width(), 16);
      assert_eq!(r.height(), 32);
      assert!(r.max.x <= 1024 && r.max.y <= 1024, "stays inside the page");
  }

  #[test]
  fn page_alloc_returns_none_when_request_exceeds_page() {
      let mut page = AtlasPage::new(64);
      assert!(
          page.try_alloc(bevy::math::UVec2::new(128, 128)).is_none(),
          "a request larger than the page cannot fit"
      );
  }

  #[test]
  fn page_deallocate_frees_space_for_reuse() {
      // Fill the page with one big alloc, free it, re-alloc the same size.
      let mut page = AtlasPage::new(64);
      let id = page.alloc_id(bevy::math::UVec2::new(64, 64)).expect("first fits");
      assert!(
          page.try_alloc(bevy::math::UVec2::new(64, 64)).is_none(),
          "page is full after the 64x64 alloc"
      );
      page.free(id);
      assert!(
          page.try_alloc(bevy::math::UVec2::new(64, 64)).is_some(),
          "after free the space is reusable (guillotiere deallocate)"
      );
  }

  #[test]
  fn page_is_empty_reports_residency() {
      let mut page = AtlasPage::new(64);
      assert!(page.is_empty());
      let id = page.alloc_id(bevy::math::UVec2::new(8, 8)).unwrap();
      assert!(!page.is_empty());
      page.free(id);
      assert!(page.is_empty(), "page empty again after the only alloc frees");
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/page.rs`:
  ```rust
  //! One backing page: the raw `guillotiere::AtlasAllocator`, its CPU `Image`
  //! handle, and the live `AtlasKey -> (AllocId, URect)` map eviction reads.
  //! Spec atlas-and-text-seam.md § 2.1.
  //!
  //! Buiy drives the **raw** allocator (not bevy_image's
  //! `DynamicTextureAtlasBuilder`, which allocates internally and discards the
  //! `Allocation`, hiding the `AllocId`). Owning the `AllocId` ourselves is
  //! what makes LRU eviction (§ 2.4) buildable rather than clear-the-world.

  use std::collections::HashMap;

  use bevy::math::{URect, UVec2};
  use bevy::prelude::{Handle, Image};
  use guillotiere::{size2, AllocId, AtlasAllocator};

  use super::AtlasKey;

  /// A single atlas page of one format. Square, `size × size` texels.
  pub struct AtlasPage {
      /// guillotine/shelf allocator — exposes the `allocate`/`deallocate` pair
      /// eviction needs.
      allocator: AtlasAllocator,
      /// Edge length, texels (uniform across all pages — see `AtlasConfig`).
      size: u32,
      /// CPU-side `Image`; its `GpuImage` is uploaded the frames it changes.
      /// `None` until the GPU-side wiring lands (Task 8); the headless
      /// allocator logic never touches it.
      texture: Option<Handle<Image>>,
      /// guillotiere `AllocId` + owned pixel rect per resident key, so eviction
      /// can `deallocate(id)` and free the rect. Spec § 2.1 `live`.
      live: HashMap<AtlasKey, (AllocId, URect)>,
  }

  /// Convert a guillotiere `Rectangle` (`euclid Box2D<i32>`) to a Bevy `URect`.
  /// guillotiere never returns negative coordinates for in-bounds allocations,
  /// so the `as u32` casts are lossless.
  fn rect_to_urect(r: guillotiere::Rectangle) -> URect {
      URect::new(
          r.min.x as u32,
          r.min.y as u32,
          r.max.x as u32,
          r.max.y as u32,
      )
  }

  impl AtlasPage {
      /// Fresh empty page of `size × size` texels.
      pub fn new(size: u32) -> Self {
          Self {
              allocator: AtlasAllocator::new(size2(size as i32, size as i32)),
              size,
              texture: None,
              live: HashMap::new(),
          }
      }

      /// Page edge length in texels.
      pub fn size(&self) -> u32 {
          self.size
      }

      /// Try to allocate a `req`-sized cell; `None` if it does not fit.
      /// Returns only the rect (no `AllocId`) — for callers that do not need
      /// to free it (tests, fit probes).
      pub fn try_alloc(&mut self, req: UVec2) -> Option<URect> {
          self.alloc(req).map(|(_, r)| r)
      }

      /// Try to allocate, returning just the `AllocId` for later `free`.
      pub fn alloc_id(&mut self, req: UVec2) -> Option<AllocId> {
          self.alloc(req).map(|(id, _)| id)
      }

      /// Core allocate: returns the `(AllocId, URect)` pair on success.
      pub fn alloc(&mut self, req: UVec2) -> Option<(AllocId, URect)> {
          let alloc = self
              .allocator
              .allocate(size2(req.x as i32, req.y as i32))?;
          Some((alloc.id, rect_to_urect(alloc.rectangle)))
      }

      /// Free a previously-allocated cell, coalescing its space.
      pub fn free(&mut self, id: AllocId) {
          self.allocator.deallocate(id);
      }

      /// No live allocations remain (eligible for the page pool, Task 7).
      pub fn is_empty(&self) -> bool {
          self.allocator.is_empty()
      }
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` add:
  ```rust
  mod page;
  pub use page::AtlasPage;
  ```
- [ ] Run, expect **PASS** (this exercises the real guillotiere allocate/deallocate path, CPU-only):
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): AtlasPage wrapping guillotiere AtlasAllocator"
  ```

---

## Task 5 — `LruQueue` recency ring (HEADLESS)

The LRU bookkeeping the eviction policy walks: `touch(key)` moves a key to most-recently-used; `pop_lru()` removes and returns the least-recently-used; per-key last-touched frame supports the `eviction_grace` clause (spec § 2.4 steps 1–3).

**Files**
- Create: `crates/buiy_core/src/render/atlas/lru.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add a failing test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::{AtlasKey, LruQueue};

  fn k(b: u8) -> AtlasKey {
      AtlasKey::from_bytes(&[b])
  }

  #[test]
  fn lru_pops_least_recently_touched_first() {
      let mut lru = LruQueue::default();
      lru.touch(k(1), 0);
      lru.touch(k(2), 0);
      lru.touch(k(3), 0);
      // Re-touch k(1): now order from LRU is k(2), k(3), k(1).
      lru.touch(k(1), 1);
      assert_eq!(lru.pop_lru(), Some(k(2)));
      assert_eq!(lru.pop_lru(), Some(k(3)));
      assert_eq!(lru.pop_lru(), Some(k(1)));
      assert_eq!(lru.pop_lru(), None);
  }

  #[test]
  fn lru_touch_is_idempotent_on_membership() {
      let mut lru = LruQueue::default();
      lru.touch(k(7), 0);
      lru.touch(k(7), 1);
      lru.touch(k(7), 2);
      assert_eq!(lru.len(), 1, "re-touching does not duplicate the entry");
      assert_eq!(lru.pop_lru(), Some(k(7)));
      assert_eq!(lru.pop_lru(), None);
  }

  #[test]
  fn lru_grace_expired_lists_keys_untouched_past_grace() {
      let mut lru = LruQueue::default();
      lru.touch(k(1), 10); // last touched frame 10
      lru.touch(k(2), 50); // last touched frame 50
      // At frame 100 with grace 60: k(1) (idle 90) expired; k(2) (idle 50) not.
      let expired = lru.grace_expired(100, 60);
      assert_eq!(expired, vec![k(1)]);
  }

  #[test]
  fn lru_remove_drops_a_specific_key() {
      let mut lru = LruQueue::default();
      lru.touch(k(1), 0);
      lru.touch(k(2), 0);
      lru.remove(&k(1));
      assert_eq!(lru.len(), 1);
      assert_eq!(lru.pop_lru(), Some(k(2)));
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/lru.rs`:
  ```rust
  //! LRU recency ring keyed by `AtlasKey`, with per-key last-touched frame.
  //! Drives eviction order (spec § 2.4 step 1–2) and the `eviction_grace`
  //! idle-drain clause (step 3). A `VecDeque` ordered LRU→MRU plus a frame map;
  //! atlas entry counts are small, so the O(n) `touch`-reorder is fine and
  //! keeps the structure trivially correct.

  use std::collections::VecDeque;

  use super::AtlasKey;

  /// LRU→MRU recency ring. Front is least-recently-used.
  #[derive(Default)]
  pub struct LruQueue {
      order: VecDeque<AtlasKey>,
      /// Per-key frame index of the most recent touch.
      last_touched: std::collections::HashMap<AtlasKey, u64>,
  }

  impl LruQueue {
      /// Mark `key` most-recently-used at `frame`. Idempotent on membership:
      /// re-touching moves the existing entry, never duplicates it.
      pub fn touch(&mut self, key: AtlasKey, frame: u64) {
          if let Some(pos) = self.order.iter().position(|k| *k == key) {
              self.order.remove(pos);
          }
          self.last_touched.insert(key.clone(), frame);
          self.order.push_back(key);
      }

      /// Remove and return the least-recently-used key, if any.
      pub fn pop_lru(&mut self) -> Option<AtlasKey> {
          let key = self.order.pop_front()?;
          self.last_touched.remove(&key);
          Some(key)
      }

      /// Drop a specific key (e.g. when evicted under grace).
      pub fn remove(&mut self, key: &AtlasKey) {
          if let Some(pos) = self.order.iter().position(|k| k == key) {
              self.order.remove(pos);
          }
          self.last_touched.remove(key);
      }

      /// Keys untouched for more than `grace` frames as of `now` (spec § 2.4
      /// step 3). Order is unspecified; callers evict all of them.
      pub fn grace_expired(&self, now: u64, grace: u32) -> Vec<AtlasKey> {
          self.last_touched
              .iter()
              .filter(|(_, &t)| now.saturating_sub(t) > grace as u64)
              .map(|(k, _)| k.clone())
              .collect()
      }

      /// Number of tracked entries.
      pub fn len(&self) -> usize {
          self.order.len()
      }

      /// No tracked entries.
      pub fn is_empty(&self) -> bool {
          self.order.is_empty()
      }
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` add:
  ```rust
  mod lru;
  pub use lru::LruQueue;
  ```
- [ ] Run, expect **PASS**. Full gate green, then commit:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  git add -A && git commit -m "feat(render/atlas): LruQueue recency ring with grace-expiry"
  ```

---

## Task 6 — `BuiyAtlas::get_or_insert` / `get` — idempotent insert, no eviction yet (HEADLESS)

The seam's only public methods on the happy path. A hit touches LRU and returns the entry **without re-blitting** (the closure is not called); a miss allocates on the format's last page (appending a page when the current one is full), records the entry + the live `(AllocId, URect)`, and returns it. Eviction-under-pressure is **deferred to Task 7** — here a full page set simply appends (still bounded by nothing yet; budget enforced next task). The blit is recorded as a pending CPU upload (no GPU here); the actual texture write lands in Task 8.

**Files**
- Create: `crates/buiy_core/src/render/atlas/atlas.rs`
- Modify: `crates/buiy_core/src/render/atlas/page.rs` (expose `insert_live` / `entry_for` helpers)
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add the failing idempotency test (the **headless half of gate #15 (a)**, spec § 7) to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::{AtlasBitmap, AtlasConfig, AtlasFormat, AtlasKey, BuiyAtlas};
  use bevy::math::UVec2;
  use std::cell::Cell;

  fn cov(w: u32, h: u32) -> AtlasBitmap {
      AtlasBitmap {
          size: UVec2::new(w, h),
          format: AtlasFormat::CoverageR8,
          data: vec![0xFF; (w * h) as usize],
      }
  }

  #[test]
  fn get_or_insert_is_idempotent_no_reblit_on_hit() {
      let mut atlas = BuiyAtlas::new(AtlasConfig::default());
      let key = AtlasKey::from_bytes(b"glyph-A");
      let calls = Cell::new(0);

      let e1 = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, || {
          calls.set(calls.get() + 1);
          cov(16, 16)
      });
      // Second call with an equal key: closure must NOT run (no rasterize, no
      // blit), and the returned entry is identical. Spec § 3, § 7 (a).
      let e2 = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, || {
          calls.set(calls.get() + 1);
          cov(16, 16)
      });
      assert_eq!(calls.get(), 1, "closure runs exactly once across two inserts");
      assert_eq!(e1, e2, "equal key -> identical AtlasEntry");
      assert_eq!(e1.px.width(), 16);
      assert_eq!(e1.px.height(), 16);
  }

  #[test]
  fn get_probe_does_not_touch_lru_and_sees_residency() {
      let mut atlas = BuiyAtlas::new(AtlasConfig::default());
      let key = AtlasKey::from_bytes(b"glyph-B");
      assert!(atlas.get(&key).is_none(), "absent before insert");
      let e = atlas.get_or_insert(key.clone(), AtlasFormat::CoverageR8, || cov(8, 8));
      assert_eq!(atlas.get(&key), Some(e), "resident after insert");
      assert_eq!(atlas.live_entry_count(), 1);
  }

  #[test]
  fn uv_rect_is_normalized_against_page_size() {
      let mut atlas = BuiyAtlas::new(AtlasConfig::default()); // 1024 page
      let e = atlas.get_or_insert(
          AtlasKey::from_bytes(b"g"),
          AtlasFormat::CoverageR8,
          || cov(512, 256),
      );
      // px (0,0)-(512,256) over a 1024 page -> uv (0,0)-(0.5,0.25).
      assert!((e.uv.max.x - 0.5).abs() < 1e-6);
      assert!((e.uv.max.y - 0.25).abs() < 1e-6);
  }

  #[test]
  fn formats_do_not_share_a_page() {
      let mut atlas = BuiyAtlas::new(AtlasConfig::default());
      atlas.get_or_insert(AtlasKey::from_bytes(b"cov"), AtlasFormat::CoverageR8, || cov(8, 8));
      atlas.get_or_insert(AtlasKey::from_bytes(b"col"), AtlasFormat::ColorRgba8, || AtlasBitmap {
          size: UVec2::new(8, 8),
          format: AtlasFormat::ColorRgba8,
          data: vec![0xFF; 8 * 8 * 4],
      });
      assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 1);
      assert_eq!(atlas.page_count(AtlasFormat::ColorRgba8), 1);
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Add to `crates/buiy_core/src/render/atlas/page.rs` (inside `impl AtlasPage`) the live-map + entry helpers:
  ```rust
      /// Record a resident cell so eviction can later free it.
      pub fn insert_live(&mut self, key: AtlasKey, id: AllocId, rect: URect) {
          self.live.insert(key, (id, rect));
      }

      /// The `(AllocId, URect)` of a resident key, if present.
      pub fn live_of(&self, key: &AtlasKey) -> Option<(AllocId, URect)> {
          self.live.get(key).copied()
      }

      /// Remove a resident cell from the live map (after `free`).
      pub fn remove_live(&mut self, key: &AtlasKey) {
          self.live.remove(key);
      }

      /// Number of resident cells (for tests / baseline assertions).
      pub fn live_len(&self) -> usize {
          self.live.len()
      }
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/atlas.rs`:
  ```rust
  //! The one render-world `BuiyAtlas` resource. Owns the per-format page
  //! lists, the content-addressed `entries` map, the LRU ring, and the config.
  //! Spec atlas-and-text-seam.md § 2–§ 3.
  //!
  //! Atlas mutation is single-threaded **by design**: there is exactly one
  //! `BuiyAtlas`, so every insert/evict serializes through one `ResMut`. This
  //! is a performance, not a correctness, coupling — entries are
  //! content-addressed, so a producer that loses a frame's mutation simply
  //! re-inserts on the next miss (spec § 2.1).

  use std::collections::HashMap;

  use bevy::math::{Rect, UVec2};
  use bevy::prelude::Resource;

  use super::{AtlasBitmap, AtlasConfig, AtlasEntry, AtlasFormat, AtlasKey, AtlasPage, LruQueue};

  /// One render-world resource shared by every coverage-and-image primitive.
  #[derive(Resource)]
  pub struct BuiyAtlas {
      /// One backing-page list per format. Distinct formats never share a page.
      pages: HashMap<AtlasFormat, Vec<AtlasPage>>,
      /// Key -> where it lives. The seam's only handle (spec § 3).
      entries: HashMap<AtlasKey, AtlasEntry>,
      /// LRU recency ring; evicted oldest-first under pressure (spec § 2.4).
      lru: LruQueue,
      config: AtlasConfig,
      /// Monotonic frame counter advanced by `begin_frame` (Task 9). Drives
      /// LRU touch timestamps + grace expiry.
      frame: u64,
  }

  impl BuiyAtlas {
      /// Empty atlas with the given config.
      pub fn new(config: AtlasConfig) -> Self {
          Self {
              pages: HashMap::new(),
              entries: HashMap::new(),
              lru: LruQueue::default(),
              config,
              frame: 0,
          }
      }

      /// Residency probe; does **not** touch LRU (spec § 3 `get`).
      pub fn get(&self, key: &AtlasKey) -> Option<AtlasEntry> {
          self.entries.get(key).copied()
      }

      /// Idempotent insert (spec § 3 `get_or_insert`). On a hit, touch LRU and
      /// return the resident entry — the closure is **not** called, so no
      /// rasterize and no blit. On a miss, allocate (Task 7 adds eviction
      /// under pressure), record the entry + live cell, and return it.
      pub fn get_or_insert(
          &mut self,
          key: AtlasKey,
          format: AtlasFormat,
          coverage: impl FnOnce() -> AtlasBitmap,
      ) -> AtlasEntry {
          if let Some(entry) = self.entries.get(&key).copied() {
              self.lru.touch(key, self.frame);
              return entry;
          }
          let bitmap = coverage();
          debug_assert_eq!(bitmap.format, format, "bitmap format must match the key's format");
          let entry = self.allocate_and_record(key.clone(), format, bitmap);
          self.lru.touch(key, self.frame);
          entry
      }

      /// Allocate the bitmap on the format's page set, appending a page if the
      /// existing ones are full, and record the entry + live cell. (Eviction
      /// under page-budget pressure is layered on in Task 7.)
      fn allocate_and_record(
          &mut self,
          key: AtlasKey,
          format: AtlasFormat,
          bitmap: AtlasBitmap,
      ) -> AtlasEntry {
          let page_size = self.config.page_size;
          let req = bitmap.size;
          let list = self.pages.entry(format).or_default();

          // Try existing pages, oldest-first.
          for (idx, page) in list.iter_mut().enumerate() {
              if let Some((id, px)) = page.alloc(req) {
                  page.insert_live(key.clone(), id, px);
                  let entry = entry_from(idx as u16, format, px, page_size);
                  self.entries.insert(key, entry);
                  return entry;
              }
          }

          // No page fit: append a fresh one. (Budget enforcement: Task 7.)
          let mut page = AtlasPage::new(page_size);
          let (id, px) = page
              .alloc(req)
              .expect("a fresh page must fit a sub-page-sized request");
          page.insert_live(key.clone(), id, px);
          let idx = list.len();
          list.push(page);
          let entry = entry_from(idx as u16, format, px, page_size);
          self.entries.insert(key, entry);
          entry
      }

      /// Number of resident entries (baseline assertions, gate #15 headless).
      pub fn live_entry_count(&self) -> usize {
          self.entries.len()
      }

      /// Number of pages for a format (the lever gate #15 watches, spec § 2.2).
      pub fn page_count(&self, format: AtlasFormat) -> usize {
          self.pages.get(&format).map(|p| p.len()).unwrap_or(0)
      }
  }

  /// Build an `AtlasEntry` from a placed pixel rect + page geometry.
  fn entry_from(
      page: u16,
      format: AtlasFormat,
      px: bevy::math::URect,
      page_size: u32,
  ) -> AtlasEntry {
      let inv = 1.0 / page_size as f32;
      let uv = Rect {
          min: bevy::math::Vec2::new(px.min.x as f32 * inv, px.min.y as f32 * inv),
          max: bevy::math::Vec2::new(px.max.x as f32 * inv, px.max.y as f32 * inv),
      };
      let _ = UVec2::ZERO; // keep import honest for future page-grid math
      AtlasEntry {
          page,
          format,
          uv,
          px,
      }
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` add:
  ```rust
  mod atlas;
  pub use atlas::BuiyAtlas;
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): BuiyAtlas get_or_insert/get idempotent seam"
  ```

---

## Task 7 — LRU eviction under page-budget pressure + grace drain (HEADLESS — gate #15 (b))

The gate-#15 contract (spec § 2.4). On a miss where the format's page set is **at budget** and no existing page fits, evict least-recently-used entries (`page.free(alloc_id)`, drop from `entries`/`live`/`lru`) until the new entry fits; only if the LRU queue is exhausted does a page append exceed budget (budget bounds steady state, never correctness). A separate per-frame `drain_grace_expired` removes entries untouched past `eviction_grace` even without pressure (step 3) — the clause that makes "return to baseline" hold.

**Files**
- Modify: `crates/buiy_core/src/render/atlas/atlas.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add the failing eviction tests to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  // A tiny-budget config that forces pressure quickly: one 64x64 page per
  // format, grace 2 frames.
  fn pressure_config() -> AtlasConfig {
      AtlasConfig {
          page_size: 64,
          page_budget: 1,
          eviction_grace: 2,
      }
  }

  #[test]
  fn eviction_drops_least_recently_used_under_budget_pressure() {
      let mut atlas = BuiyAtlas::new(pressure_config());
      // Four 32x32 cells exactly tile a 64x64 page (budget = 1 page).
      let keys: Vec<AtlasKey> = (0..4).map(|i| AtlasKey::from_bytes(&[i])).collect();
      for k in &keys {
          atlas.get_or_insert(k.clone(), AtlasFormat::CoverageR8, || cov(32, 32));
      }
      assert_eq!(atlas.live_entry_count(), 4, "page is exactly full");
      assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 1, "still one page");

      // Touch keys 1,2,3 so key 0 is the LRU victim.
      for k in &keys[1..] {
          atlas.touch_existing(k);
      }
      // A fifth cell forces eviction of the LRU (key 0), NOT a budget-busting
      // 2nd page. Spec § 2.4 step 2.
      let k4 = AtlasKey::from_bytes(&[4]);
      atlas.get_or_insert(k4.clone(), AtlasFormat::CoverageR8, || cov(32, 32));
      assert_eq!(
          atlas.page_count(AtlasFormat::CoverageR8),
          1,
          "eviction kept us at budget, no new page"
      );
      assert!(atlas.get(&keys[0]).is_none(), "LRU victim evicted");
      assert!(atlas.get(&k4).is_some(), "new entry resident");
      assert_eq!(atlas.live_entry_count(), 4);
  }

  #[test]
  fn budget_exceeded_only_when_lru_exhausted() {
      // One cell that fills the whole page; a second of the same size cannot
      // fit even after evicting the first IF the first is the one being
      // re-requested. Here we insert a 64x64 (fills page), then a *new* 64x64:
      // eviction frees the first, the second fits -> still one page.
      let mut atlas = BuiyAtlas::new(pressure_config());
      let k0 = AtlasKey::from_bytes(b"big0");
      let k1 = AtlasKey::from_bytes(b"big1");
      atlas.get_or_insert(k0.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
      atlas.get_or_insert(k1.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
      assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 1);
      assert!(atlas.get(&k0).is_none(), "first evicted to make room");
      assert!(atlas.get(&k1).is_some());
  }

  #[test]
  fn grace_drain_returns_idle_entries_to_baseline() {
      // The headless half of gate #15: after a scripted insert -> idle cycle,
      // live-entry count returns to baseline and page count does not grow
      // monotonically. Spec § 2.4 step 3, § 7.
      let mut atlas = BuiyAtlas::new(pressure_config());
      let baseline = atlas.live_entry_count(); // 0
      atlas.get_or_insert(AtlasKey::from_bytes(b"transient"), AtlasFormat::CoverageR8, || cov(16, 16));
      assert_eq!(atlas.live_entry_count(), 1);

      // Idle: advance frames past the grace window without touching the entry,
      // draining each frame.
      for _ in 0..(pressure_config().eviction_grace + 1) {
          atlas.begin_frame();
          atlas.drain_grace_expired();
      }
      assert_eq!(
          atlas.live_entry_count(),
          baseline,
          "transient entry drained back to baseline after idle"
      );
      assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 1, "page count did not grow monotonically");
  }
  ```
- [ ] Run, expect **compile FAIL** (`touch_existing`, `begin_frame`, `drain_grace_expired`, and eviction logic absent):
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] In `crates/buiy_core/src/render/atlas/atlas.rs`, replace the body of `allocate_and_record`'s "append a fresh page" branch with budget-aware eviction, and add the new methods. Specifically:
  - Add a frame-advance + grace-drain + test helper to `impl BuiyAtlas`:
  ```rust
      /// Advance the atlas frame counter (call once per render frame before
      /// inserts). Drives LRU timestamps + grace expiry.
      pub fn begin_frame(&mut self) {
          self.frame = self.frame.wrapping_add(1);
      }

      /// Touch an already-resident key, moving it to most-recently-used.
      /// (Each primitive that samples an entry calls this per frame — spec
      /// § 2.4 step 1. Exposed for tests as `touch_existing`.)
      pub fn touch_existing(&mut self, key: &AtlasKey) {
          if self.entries.contains_key(key) {
              self.lru.touch(key.clone(), self.frame);
          }
      }

      /// Evict every entry untouched for more than `eviction_grace` frames
      /// (spec § 2.4 step 3). The clause that makes "return to baseline" hold.
      pub fn drain_grace_expired(&mut self) {
          let grace = self.config.eviction_grace;
          for key in self.lru.grace_expired(self.frame, grace) {
              self.evict_entry(&key);
          }
      }

      /// Free one entry everywhere: page allocator, live map, entries, LRU.
      fn evict_entry(&mut self, key: &AtlasKey) {
          let Some(entry) = self.entries.remove(key) else {
              return;
          };
          if let Some(list) = self.pages.get_mut(&entry.format) {
              if let Some(page) = list.get_mut(entry.page as usize) {
                  if let Some((id, _)) = page.live_of(key) {
                      page.free(id);
                      page.remove_live(key);
                  }
              }
          }
          self.lru.remove(key);
      }
  ```
  - Replace the fresh-page branch of `allocate_and_record` so it evicts before appending past budget:
  ```rust
          // No existing page fit. If the format's page set is at budget, evict
          // LRU entries (of this format) until either a page fits the request
          // or the LRU queue is exhausted; only then append a fresh page
          // (exceeding budget rather than failing — budget bounds steady
          // state, never correctness). Spec § 2.4 step 2.
          loop {
              let list = self.pages.entry(format).or_default();
              if (list.len() as u16) < self.config.page_budget {
                  break; // under budget: appending a page is allowed.
              }
              // At budget: try to free room by evicting the LRU entry of this
              // format. If none can be evicted, fall through to append.
              let Some(victim) = self.next_lru_of_format(format) else {
                  break;
              };
              self.evict_entry(&victim);
              // Retry existing pages now that a cell freed.
              let list = self.pages.entry(format).or_default();
              for (idx, page) in list.iter_mut().enumerate() {
                  if let Some((id, px)) = page.alloc(req) {
                      page.insert_live(key.clone(), id, px);
                      let entry = entry_from(idx as u16, format, px, page_size);
                      self.entries.insert(key, entry);
                      return entry;
                  }
              }
          }

          // Append a fresh page (under budget, or budget exceeded because the
          // LRU was exhausted and the entry still did not fit).
          let mut page = AtlasPage::new(page_size);
  ```
  (Keep the existing fresh-page `alloc`/`insert_live`/`push`/`entries.insert`/`return` tail that follows.)
  - Add the LRU-of-format helper:
  ```rust
      /// The least-recently-used resident key of `format`, if any. Walks LRU
      /// order front-to-back and returns the first entry of the right format.
      fn next_lru_of_format(&self, format: AtlasFormat) -> Option<AtlasKey> {
          self.lru
              .iter_lru_to_mru()
              .find(|k| self.entries.get(*k).map(|e| e.format) == Some(format))
              .cloned()
      }
  ```
- [ ] Add the LRU iterator to `crates/buiy_core/src/render/atlas/lru.rs` (`impl LruQueue`):
  ```rust
      /// Iterate keys least-recently-used first.
      pub fn iter_lru_to_mru(&self) -> impl Iterator<Item = &AtlasKey> {
          self.order.iter()
      }
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): LRU eviction under page-budget pressure + grace drain"
  ```

---

## Task 8 — Page pooling: emptied pages reset + reused, not freed (HEADLESS)

Spec § 2.5: when eviction empties a page entirely, reset its allocator (`AtlasAllocator::new(size)`) and return it (with its texture handle) to a free list for the next page-growth request, rather than dropping it — keeps RSS flat across alloc/free cycles. The headless assertion is that an emptied page's *identity* (texture handle slot) is reused, not reallocated (spec § 7 "Pooling is asserted by checking an emptied page's texture handle is reused").

**Files**
- Modify: `crates/buiy_core/src/render/atlas/page.rs` (add `reset` + texture-handle accessor)
- Modify: `crates/buiy_core/src/render/atlas/atlas.rs` (free-list + reuse path)
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add the failing pooling test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  #[test]
  fn emptied_page_is_pooled_and_reused_not_reallocated() {
      let mut atlas = BuiyAtlas::new(AtlasConfig {
          page_size: 64,
          page_budget: 8,
          eviction_grace: 0, // drain immediately on idle
      });
      // Fill page 0 fully, then add one cell that needs a 2nd page.
      let k0 = AtlasKey::from_bytes(b"fills-page-0");
      atlas.get_or_insert(k0.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
      let k1 = AtlasKey::from_bytes(b"needs-page-1");
      atlas.get_or_insert(k1.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
      assert_eq!(atlas.page_count(AtlasFormat::CoverageR8), 2);

      // The pooled-page identity token before any recycling.
      let pool_before = atlas.pooled_page_count(AtlasFormat::CoverageR8);
      assert_eq!(pool_before, 0, "nothing pooled yet");

      // Evict everything on page 1 -> it empties -> pooled, not dropped.
      atlas.begin_frame();
      atlas.evict_for_test(&k1);
      atlas.collect_emptied_pages();
      assert_eq!(
          atlas.pooled_page_count(AtlasFormat::CoverageR8),
          1,
          "emptied page returned to the pool, not freed"
      );

      // A new page-growth request reuses the pooled page instead of allocating.
      let k2 = AtlasKey::from_bytes(b"reuses-pool");
      atlas.get_or_insert(k2.clone(), AtlasFormat::CoverageR8, || cov(64, 64));
      assert_eq!(
          atlas.pooled_page_count(AtlasFormat::CoverageR8),
          0,
          "pooled page taken back into service (reused, not reallocated)"
      );
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] In `crates/buiy_core/src/render/atlas/page.rs` add to `impl AtlasPage`:
  ```rust
      /// Reset the allocator to empty and clear the live map, **keeping the
      /// texture handle** — the expensive GPU object is reused, not realloc'd
      /// (spec § 2.5 pooling).
      pub fn reset(&mut self) {
          self.allocator = AtlasAllocator::new(size2(self.size as i32, self.size as i32));
          self.live.clear();
      }

      /// The page's texture handle (pooling reuses this slot rather than
      /// dropping it). `None` until GPU wiring lands.
      pub fn texture(&self) -> Option<&Handle<Image>> {
          self.texture.as_ref()
      }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/atlas.rs`:
  - Add a per-format free list field to `BuiyAtlas`:
  ```rust
      /// Emptied pages, reset and held for reuse instead of dropped (spec
      /// § 2.5). Keyed by format because formats never share a page.
      pooled: HashMap<AtlasFormat, Vec<AtlasPage>>,
  ```
  (Initialize `pooled: HashMap::new()` in `new`.)
  - Add the pooling API + test seams:
  ```rust
      /// After eviction, move every now-empty page into the per-format pool
      /// (reset, texture handle retained). Call once per frame after drains.
      pub fn collect_emptied_pages(&mut self) {
          for (format, list) in self.pages.iter_mut() {
              let mut i = 0;
              while i < list.len() {
                  if list[i].live_len() == 0 && list[i].is_empty() {
                      let mut page = list.remove(i);
                      page.reset();
                      self.pooled.entry(*format).or_default().push(page);
                  } else {
                      i += 1;
                  }
              }
          }
      }

      /// Number of pooled (recyclable) pages for a format.
      pub fn pooled_page_count(&self, format: AtlasFormat) -> usize {
          self.pooled.get(&format).map(|p| p.len()).unwrap_or(0)
      }

      /// Test seam: evict a specific key (mirrors the per-frame eviction path).
      pub fn evict_for_test(&mut self, key: &AtlasKey) {
          self.evict_entry(key);
      }
  ```
  - In `allocate_and_record`'s fresh-page branch, take a pooled page when available instead of `AtlasPage::new`:
  ```rust
          // Reuse a pooled (emptied) page if one exists; else allocate fresh.
          let mut page = self
              .pooled
              .get_mut(&format)
              .and_then(|pool| pool.pop())
              .unwrap_or_else(|| AtlasPage::new(page_size));
  ```
  (When taking the pooled branch the page is already `reset`, so it is empty and the subsequent `alloc` succeeds. **Note for the implementer:** because page indices shift when `collect_emptied_pages` removes pages, the `AtlasEntry.page` of surviving entries can go stale. For the headless milestone, re-derive indices is out of scope — instead `collect_emptied_pages` must only pool pages that are **trailing** empties OR the atlas must reindex. Simplest correct rule: only pool a page when it is the **last** page of the list; assert this with a `debug_assert`. Replace the `while`-loop body condition with: pool only if `i == list.len() - 1 && list[i].live_len() == 0`. Document this as the v1 pooling restriction; full mid-list compaction with entry reindex is a follow-up.)
- [ ] Apply the trailing-empty restriction so no live `AtlasEntry.page` index goes stale. Final `collect_emptied_pages`:
  ```rust
      pub fn collect_emptied_pages(&mut self) {
          for (format, list) in self.pages.iter_mut() {
              // Only pool *trailing* empty pages: popping from the end never
              // shifts a surviving entry's page index. Mid-list compaction
              // (with entry reindex) is a v1 follow-up. Spec § 2.5.
              while let Some(last) = list.last() {
                  if last.live_len() == 0 && last.is_empty() {
                      let mut page = list.pop().expect("checked last");
                      page.reset();
                      self.pooled.entry(*format).or_default().push(page);
                  } else {
                      break;
                  }
              }
          }
      }
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): pool emptied trailing pages for reuse (spec § 2.5)"
  ```

---

## Task 9 — `GlyphAlphaInstance` + `IconInstance` primitive shapes (HEADLESS)

The two F-tier atlas-sampling primitive instance layouts (spec § 4.1, § 4.2). Pure POD structs — `bytemuck::Pod`, `#[repr(C)]`, fields verbatim from the spec. The headless test pins the byte layout (size/offsets) the same way `render_instance.rs` pins `InstanceData` against the pipeline descriptor stride.

**Files**
- Create: `crates/buiy_core/src/render/atlas/primitive.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Create (test): `crates/buiy_core/tests/atlas_primitive.rs`

Steps:

- [ ] Create the failing test `crates/buiy_core/tests/atlas_primitive.rs`:
  ```rust
  //! Headless layout tests for the two atlas-sampling primitive shapes.
  //! Pure-CPU POD layout; no GPU adapter. Spec atlas-and-text-seam.md § 4.
  use buiy_core::render::atlas::{GlyphAlphaInstance, IconInstance};

  #[test]
  fn glyph_alpha_instance_layout() {
      // rect[4] + uv[4] + color[4] + clip[4] = 16 f32 = 64 B, + page u32 = 4 B,
      // total 68 B before alignment. repr(C) with f32 fields aligns to 4, so
      // size = 68. Lock it so a field reorder/addition is caught.
      assert_eq!(std::mem::size_of::<GlyphAlphaInstance>(), 68);
      assert_eq!(std::mem::align_of::<GlyphAlphaInstance>(), 4);
      // Construct one; proves the public field set matches the spec.
      let g = GlyphAlphaInstance {
          rect: [0.0; 4],
          uv: [0.0; 4],
          color: [1.0, 1.0, 1.0, 1.0],
          clip: [0.0; 4],
          page: 0,
      };
      let _bytes: &[u8] = bytemuck::bytes_of(&g); // Pod
  }

  #[test]
  fn icon_instance_layout() {
      assert_eq!(std::mem::size_of::<IconInstance>(), 68);
      assert_eq!(std::mem::align_of::<IconInstance>(), 4);
      let i = IconInstance {
          rect: [0.0; 4],
          uv: [0.0; 4],
          tint: [1.0, 1.0, 1.0, 1.0],
          clip: [0.0; 4],
          page: 0,
      };
      let _bytes: &[u8] = bytemuck::bytes_of(&i);
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_primitive
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/primitive.rs`:
  ```rust
  //! The two F-tier atlas-sampling primitive shapes. The shapes are owned here
  //! so text and render cannot drift (spec § 4). `buiy-text-rendering-design`
  //! *emits* them (one per visible glyph); the batched node *consumes* them.

  use bytemuck::{Pod, Zeroable};

  /// One instance per visible glyph (or any single-channel coverage quad, e.g.
  /// a generated mask stamp). The **alpha-as-color** primitive: the atlas
  /// stores `R8` coverage and color is applied per-instance, so one resident
  /// copy serves any tint and a theme color change never touches the atlas
  /// (spec § 4.1). Not text-specific — any coverage stamp uses it.
  #[repr(C)]
  #[derive(Clone, Copy, Pod, Zeroable)]
  pub struct GlyphAlphaInstance {
      /// Screen-space x, y, w, h (post-bridge `GlobalTransform`-resolved).
      pub rect: [f32; 4],
      /// `CoverageR8` page UV from `AtlasEntry.uv`.
      pub uv: [f32; 4],
      /// Linear-light premultiplied tint — the "alpha as color" value.
      pub color: [f32; 4],
      /// `ClipRect`, the per-instance clip (clip-and-transform.md).
      pub clip: [f32; 4],
      /// Which `CoverageR8` page → selects the bind slot.
      pub page: u32,
  }

  /// One instance per full-color stamp — themed raster icons, color-emoji
  /// glyph bitmaps the text spec produces as `Rgba8`. **No recolor trick**:
  /// the atlas stores the color and the primitive samples it straight, with an
  /// optional multiplied tint (spec § 4.2). Mirrors GPUI's `PolychromeSprite`.
  #[repr(C)]
  #[derive(Clone, Copy, Pod, Zeroable)]
  pub struct IconInstance {
      pub rect: [f32; 4],
      /// `ColorRgba8` page UV.
      pub uv: [f32; 4],
      /// Multiplied over the sampled color (`[1,1,1,1]` = no tint).
      pub tint: [f32; 4],
      pub clip: [f32; 4],
      pub page: u32,
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` add:
  ```rust
  mod primitive;
  pub use primitive::{GlyphAlphaInstance, IconInstance};
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_primitive
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): GlyphAlphaInstance + IconInstance primitive shapes"
  ```

---

## Task 10 — Warmup queue + `warmup_atlas` drain (HEADLESS for the drain mechanism)

Spec § 2.3: a pre-paint queue of "insert this now" requests, drained before the first paint so steady-state and golden frames never race a cold atlas (gate #2 flake mitigation). This spec owns the **mechanism** (the drain that forces requested entries resident); it does **not** own *what* to warm — the text/icon owners push requests. The drain is pure CPU and gates on CI; the determinism *proof* (golden-image) is GPU `#[ignore]` (Task 12).

**Files**
- Create: `crates/buiy_core/src/render/atlas/warmup.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add the failing warmup test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::{AtlasWarmupQueue, AtlasWarmupRequest};

  #[test]
  fn warmup_drain_forces_requested_entries_resident_before_paint() {
      let mut atlas = BuiyAtlas::new(AtlasConfig::default());
      let mut queue = AtlasWarmupQueue::default();
      let key = AtlasKey::from_bytes(b"ascii-A");
      queue.push(AtlasWarmupRequest {
          key: key.clone(),
          format: AtlasFormat::CoverageR8,
          bitmap: cov(16, 16),
      });
      assert!(atlas.get(&key).is_none(), "cold before warmup");
      assert_eq!(queue.len(), 1);

      // The drain (mechanism this spec owns) forces residency and empties the
      // queue. In-app this runs pre-paint via `warmup_atlas`. Spec § 2.3.
      atlas.drain_warmup(&mut queue);
      assert!(atlas.get(&key).is_some(), "resident after warmup drain");
      assert_eq!(queue.len(), 0, "queue drained");
  }

  #[test]
  fn warmup_drain_is_idempotent_for_duplicate_requests() {
      let mut atlas = BuiyAtlas::new(AtlasConfig::default());
      let mut queue = AtlasWarmupQueue::default();
      let key = AtlasKey::from_bytes(b"dup");
      for _ in 0..3 {
          queue.push(AtlasWarmupRequest {
              key: key.clone(),
              format: AtlasFormat::CoverageR8,
              bitmap: cov(16, 16),
          });
      }
      atlas.drain_warmup(&mut queue);
      assert_eq!(atlas.live_entry_count(), 1, "duplicate warmups dedup to one entry");
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Create `crates/buiy_core/src/render/atlas/warmup.rs`:
  ```rust
  //! Warmup: a pre-paint queue of "insert this now" requests, drained before
  //! the first paint so golden frames never race a cold atlas (spec § 2.3,
  //! gate #2). This spec owns the *mechanism* (the drain); producers
  //! (text/icon owners) decide *what* to warm and push requests.

  use bevy::prelude::Resource;

  use super::{AtlasBitmap, AtlasFormat, AtlasKey};

  /// One pre-paint residency request.
  pub struct AtlasWarmupRequest {
      pub key: AtlasKey,
      pub format: AtlasFormat,
      pub bitmap: AtlasBitmap,
  }

  /// Render-world queue of warmup requests, drained pre-paint by
  /// `warmup_atlas`. Producers push; the atlas drains.
  #[derive(Resource, Default)]
  pub struct AtlasWarmupQueue {
      requests: Vec<AtlasWarmupRequest>,
  }

  impl AtlasWarmupQueue {
      /// Enqueue a residency request.
      pub fn push(&mut self, req: AtlasWarmupRequest) {
          self.requests.push(req);
      }

      /// Pending request count.
      pub fn len(&self) -> usize {
          self.requests.len()
      }

      /// No pending requests.
      pub fn is_empty(&self) -> bool {
          self.requests.is_empty()
      }

      /// Take all pending requests, emptying the queue.
      pub(crate) fn take(&mut self) -> Vec<AtlasWarmupRequest> {
          std::mem::take(&mut self.requests)
      }
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/atlas.rs` add to `impl BuiyAtlas`:
  ```rust
      /// Drain a warmup queue: force every requested entry resident (idempotent
      /// — a request whose key is already resident is a no-op insert). Spec
      /// § 2.3. The in-app `warmup_atlas` system calls this pre-paint.
      pub fn drain_warmup(&mut self, queue: &mut super::AtlasWarmupQueue) {
          for req in queue.take() {
              let bitmap = req.bitmap;
              self.get_or_insert(req.key, req.format, move || bitmap);
          }
      }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` add:
  ```rust
  mod warmup;
  pub use warmup::{AtlasWarmupQueue, AtlasWarmupRequest};
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): warmup queue + idempotent pre-paint drain"
  ```

---

## Task 11 — Reserved gradient/mask entry kinds + key constructors (HEADLESS)

Spec § 6: gradients and generated masks ride the **same** atlas (no new resource, no new allocator, no new eviction policy). What ships now is the *reservation*: an `AtlasEntryKind` tag + key-constructor seam so the C-tier shaders add only a *producer* + a *key constructor* later, never atlas machinery. **No baking/mask shader in v1.** This task only proves the reservation compiles and routes through the existing `get_or_insert`.

**Files**
- Modify: `crates/buiy_core/src/render/atlas/types.rs`
- Modify: `crates/buiy_core/src/render/atlas/mod.rs`
- Modify (test): `crates/buiy_core/tests/atlas_alloc.rs`

Steps:

- [ ] Add the failing reservation test to `crates/buiy_core/tests/atlas_alloc.rs`:
  ```rust
  use buiy_core::render::atlas::AtlasEntryKind;

  #[test]
  fn entry_kind_maps_to_format_per_spec() {
      // Glyph + mask are CoverageR8; icon + gradient are ColorRgba8 (spec § 6).
      assert_eq!(AtlasEntryKind::Glyph.format(), AtlasFormat::CoverageR8);
      assert_eq!(AtlasEntryKind::Mask.format(), AtlasFormat::CoverageR8);
      assert_eq!(AtlasEntryKind::Icon.format(), AtlasFormat::ColorRgba8);
      assert_eq!(AtlasEntryKind::Gradient.format(), AtlasFormat::ColorRgba8);
  }

  #[test]
  fn reserved_mask_entry_uses_the_glyph_alpha_path() {
      // A generated mask *is* a CoverageR8 entry sampled like a glyph — same
      // get_or_insert, same primitive (spec § 6 / § 4.1). Proves the reserved
      // kind needs no new atlas machinery.
      let mut atlas = BuiyAtlas::new(AtlasConfig::default());
      let key = AtlasKey::from_bytes(b"mask-1");
      let e = atlas.get_or_insert(key, AtlasEntryKind::Mask.format(), || cov(8, 8));
      assert_eq!(e.format, AtlasFormat::CoverageR8);
  }
  ```
- [ ] Run, expect **compile FAIL**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Append to `crates/buiy_core/src/render/atlas/types.rs`:
  ```rust
  /// What an atlas entry represents. Glyph + Icon ship F-tier (spec § 4);
  /// Gradient + Mask are **reserved C-tier** entry *kinds* (spec § 6) — they
  /// ride the same atlas, allocator, eviction policy, and bind group, so the
  /// deferred shaders add only a producer + a key constructor, never new atlas
  /// machinery. **No baking/mask shader in v1.**
  #[derive(Clone, Copy, PartialEq, Eq, Debug)]
  pub enum AtlasEntryKind {
      /// Single-channel coverage glyph (F-tier, alpha-as-color, § 4.1).
      Glyph,
      /// Full-color icon / color-emoji bitmap (F-tier, § 4.2).
      Icon,
      /// Baked gradient color strip (C-tier reserved, § 6).
      Gradient,
      /// Generated `clip-path`/`mask-image` coverage (C-tier reserved, § 6) —
      /// sampled exactly like a glyph (a `GlyphAlphaInstance` with a mask key).
      Mask,
  }

  impl AtlasEntryKind {
      /// The page format this kind lives in (spec § 2.2, § 6). Coverage kinds
      /// are `R8`; color kinds are `Rgba8`.
      pub fn format(self) -> AtlasFormat {
          match self {
              AtlasEntryKind::Glyph | AtlasEntryKind::Mask => AtlasFormat::CoverageR8,
              AtlasEntryKind::Icon | AtlasEntryKind::Gradient => AtlasFormat::ColorRgba8,
          }
      }
  }
  ```
- [ ] In `crates/buiy_core/src/render/atlas/mod.rs` extend the `types` re-export to include `AtlasEntryKind`:
  ```rust
  pub use types::{AtlasBitmap, AtlasConfig, AtlasEntry, AtlasEntryKind, AtlasFormat, AtlasKey};
  ```
  (Note: `AtlasFormat` itself stays declared in `mod.rs`; export `AtlasEntryKind` from `types`, leave `AtlasFormat` where it is.)
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_alloc
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): reserve gradient/mask entry kinds (C-tier, spec § 6)"
  ```

---

## Task 12 — Register the atlas resources in `BuiyRenderPlugin` + GPU e2e seams (HEADLESS reg test + GPU #[ignore])

Wire `BuiyAtlas` and `AtlasWarmupQueue` as render-world resources, and add the `warmup_atlas` system to `ExtractSchedule` so producers' warmup requests drain pre-paint. The **registration presence** is a headless test that runs `BuiyRenderPlugin::build` *without* a RenderApp (it early-returns, like `render_smoke.rs`) — so the real gating assertion is the plugin still builds clean. The resource-actually-inserted-into-RenderApp and the atlas upload/sampling draw are GPU `#[ignore]` (need a wgpu adapter — there is none on CI/this host).

**Files**
- Modify: `crates/buiy_core/src/render/atlas/mod.rs` (add a `register` fn)
- Modify: `crates/buiy_core/src/render/mod.rs` (call it from `BuiyRenderPlugin::build`)
- Create (test): `crates/buiy_core/tests/atlas_register.rs`

Steps:

- [ ] Create `crates/buiy_core/tests/atlas_register.rs`:
  ```rust
  //! Atlas registration. The headless half asserts the plugin still builds
  //! clean with the atlas wiring added (no RenderApp -> early return, mirrors
  //! render_smoke.rs). The RenderApp-resource-presence + GPU draw are #[ignore]
  //! (need a wgpu adapter; none on CI/this host).
  use bevy::prelude::*;
  use buiy_core::{CorePlugin, render::BuiyRenderPlugin};

  #[test]
  fn render_plugin_with_atlas_builds_without_panic() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(BuiyRenderPlugin);
      app.update();
  }

  // Needs a wgpu adapter: RenderPlugin::build block_on(initialize_renderer)
  // expect()s one; headless CI without a GPU/lavapipe panics before our code
  // runs. Same caveat as render_smoke.rs. Run locally with:
  //   cargo test -p buiy_core --test atlas_register -- --ignored
  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the gate-#15 e2e harness"]
  fn atlas_resources_registered_in_render_app() {
      use buiy_core::render::atlas::{AtlasWarmupQueue, BuiyAtlas};
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(BuiyRenderPlugin);

      let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
      assert!(
          render_app.world().get_resource::<BuiyAtlas>().is_some(),
          "BuiyAtlas registered in the render world"
      );
      assert!(
          render_app.world().get_resource::<AtlasWarmupQueue>().is_some(),
          "AtlasWarmupQueue registered in the render world"
      );
  }
  ```
- [ ] Run the headless test, expect **PASS already** for the build-clean test only if `register` exists; since it does not yet, the `#[ignore]` test references `BuiyAtlas`/`AtlasWarmupQueue` (already public) so the file compiles, but the headless test passes trivially. To make this a real RED first, temporarily assert resource presence in the **headless** test is impossible (no RenderApp). Instead, drive RED via the GPU test compiling against a not-yet-existing `register`. Simplest: skip RED ceremony here and confirm the headless build test passes, then add wiring and re-run:
  ```sh
  cargo test -p buiy_core --test atlas_register
  ```
  Expected: **PASS** (build-clean). The `#[ignore]`d GPU test does not run on CI.
- [ ] Add a `register` fn to `crates/buiy_core/src/render/atlas/mod.rs`:
  ```rust
  use bevy::prelude::*;
  use bevy::render::{ExtractSchedule, RenderApp};

  /// Insert the shared atlas resources into the render world and schedule the
  /// pre-paint warmup drain. Called from `BuiyRenderPlugin::build` inside the
  /// `RenderApp` branch. Spec § 2.1 (one resource per `RenderApp`), § 2.3.
  pub(crate) fn register(render_app: &mut SubApp) {
      render_app
          .insert_resource(BuiyAtlas::new(AtlasConfig::default()))
          .init_resource::<AtlasWarmupQueue>()
          .add_systems(ExtractSchedule, warmup_atlas);
  }

  /// Pre-paint warmup drain (spec § 2.3): force every queued residency request
  /// resident before the first paint, so golden frames never race a cold atlas
  /// (gate #2). Producers (text/icon owners) push to the queue; this drains it.
  fn warmup_atlas(mut atlas: ResMut<BuiyAtlas>, mut queue: ResMut<AtlasWarmupQueue>) {
      if queue.is_empty() {
          return;
      }
      atlas.drain_warmup(&mut queue);
  }

  // Avoid an unused-import warning when RenderApp is not constructed (the
  // headless build path) — `RenderApp` is referenced only by `register`'s
  // caller in render::mod, so re-export the symbol the caller needs.
  #[allow(unused_imports)]
  use RenderApp as _RenderAppMarker;
  ```
  **Implementer note:** if the `_RenderAppMarker` shim trips clippy, drop it and instead `use` `RenderApp` only where needed in `render::mod`. The clean form: keep `register(render_app: &mut SubApp)` here and import `RenderApp` in `render::mod` where `get_sub_app_mut` already uses it.
- [ ] In `crates/buiy_core/src/render/mod.rs`, inside `BuiyRenderPlugin::build`'s `RenderApp` branch (after `pipeline::register(render_app);`), add:
  ```rust
          atlas::register(render_app);
  ```
- [ ] Run, expect **PASS**:
  ```sh
  cargo test -p buiy_core --test atlas_register
  ```
- [ ] Full gate green (the `#[ignore]`d GPU test is skipped), then commit:
  ```sh
  git add -A && git commit -m "feat(render/atlas): register BuiyAtlas + warmup_atlas in RenderApp"
  ```

---

## Task 13 — GPU e2e: atlas upload + sampling draw + gate-#15 return-to-baseline fixture (GPU #[ignore])

The device-bound proofs (spec § 7 "On GPU"). All `#[ignore]`d — they need a wgpu adapter, which CI and this host lack. These document the end-to-end contract so a GPU runner (lavapipe) can validate it, mirroring `render_smoke.rs`'s `#[ignore]` style. **No headless assertions are added here** — the headless contract was fully covered in Tasks 4–11.

**Files**
- Create (test): `crates/buiy_core/tests/atlas_gpu.rs`

Steps:

- [ ] Create `crates/buiy_core/tests/atlas_gpu.rs` (all `#[ignore]`):
  ```rust
  //! GPU-only end-to-end atlas tests. Every test here needs a wgpu adapter
  //! (real GPU or lavapipe) — CI and this host have none, so all are #[ignore]
  //! exactly like render_smoke.rs. The headless allocator/LRU/pooling/warmup
  //! contract is covered adapter-free in atlas_alloc.rs.
  //!
  //! Run locally with a GPU/lavapipe:
  //!   cargo test -p buiy_core --test atlas_gpu -- --ignored

  // --- (1) Upload + sample: a warmed coverage entry paints with its tint. ---
  #[test]
  #[ignore = "needs a wgpu adapter; GPU upload + sampling draw (spec § 7 'On GPU')"]
  fn warmed_glyph_uploads_and_samples_with_tint() {
      // Build a RenderApp, push a CoverageR8 warmup request for a known 16x16
      // coverage bitmap, run one frame, and read back the target: the entry's
      // pixels must equal `color * coverage` (alpha-as-color, spec § 4.1).
      // Implementation deferred to the GPU runner; the headless residency +
      // primitive layout it depends on are proven in atlas_alloc.rs /
      // atlas_primitive.rs.
  }

  // --- (2) Alpha-as-color: re-tinting a glyph never regenerates the atlas. ---
  #[test]
  #[ignore = "needs a wgpu adapter; atlas byte-identity across two themes (spec § 7)"]
  fn retint_same_glyph_leaves_atlas_byte_identical() {
      // Insert glyph G once. Emit a GlyphAlphaInstance with theme-A color, then
      // theme-B color. Assert the CoverageR8 page texture is byte-identical
      // between the two frames (only the instance `color` differs) — the
      // alpha-as-color trick (spec § 4.1, § 7).
  }

  // --- (3) Warmup determinism: first painted frame matches golden. ---
  #[test]
  #[ignore = "needs a wgpu adapter; gate #2 warmup-determinism golden (spec § 2.3, § 7)"]
  fn warmup_makes_first_frame_match_golden() {
      // With warmup_atlas draining the queue pre-paint, the fixture's FIRST
      // painted frame matches its golden (no glyph lands a frame late).
  }

  // --- (4) Gate #15: atlas entries return within ε of baseline after idle. ---
  #[test]
  #[ignore = "needs a wgpu adapter; gate #15 atlas-entries-return-to-baseline fixture"]
  fn gate15_atlas_entries_return_to_baseline_after_idle() {
      // Drive a fixture that exercises many transient glyphs/icons, then go
      // idle. The idle-settle window must exceed
      // max(config.eviction_grace, RT-pool 3 frames) (spec § 2.4 "Consequence
      // for the gate-#15 fixture's idle-settle window"). After settling, the
      // live-entry count returns within ε of baseline and page count does not
      // grow monotonically. The headless half of this (entry count + page
      // count, adapter-free) is `grace_drain_returns_idle_entries_to_baseline`
      // in atlas_alloc.rs; this is the on-GPU RSS half.
  }
  ```
- [ ] Run (CI mode skips ignored), expect **PASS (0 run, 4 ignored)**:
  ```sh
  cargo test -p buiy_core --test atlas_gpu
  ```
- [ ] Confirm they compile and are visible under `--ignored` locally (will be `ignored`/no-adapter-skipped — bodies are empty so they "pass" if an adapter exists):
  ```sh
  cargo test -p buiy_core --test atlas_gpu -- --ignored --list
  ```
- [ ] Full gate green, then commit:
  ```sh
  git add -A && git commit -m "test(render/atlas): GPU e2e #[ignore] seams (upload, alpha-as-color, warmup, gate #15)"
  ```

---

## Task 14 — Module rustdoc + docs/README catalog entry (HEADLESS)

Closeout: a crate-level doc note on the seam boundary (so a reader lands on "text produces, atlas warehouses") and a catalog line for this plan in `docs/README.md` under the render-pipeline area, per the project's docs discipline.

**Files**
- Modify: `crates/buiy_core/src/render/atlas/mod.rs` (expand the module doc with the seam table)
- Modify: `docs/README.md`

Steps:

- [ ] Expand the top-of-file doc comment in `crates/buiy_core/src/render/atlas/mod.rs` to state the seam in one place (mirroring spec § 1), e.g. add:
  ```rust
  //! ## The seam (spec § 1)
  //! - **This module owns:** the shared atlas (allocation, warmup, eviction,
  //!   pooling), the glyph-alpha + icon primitive *shapes*, and the reserved
  //!   gradient/mask entry kinds.
  //! - **`buiy-text-rendering-design` owns:** glyph shaping, line layout, font
  //!   fallback, BiDi, coverage-bitmap rasterization, and *emitting* primitives.
  //!   It plugs in through `get_or_insert`/`AtlasEntry` (inbound) and
  //!   `GlyphAlphaInstance`/`IconInstance` (outbound) only — no cosmic-text type
  //!   ever crosses into this module.
  ```
- [ ] Verify rustdoc is warning-clean (the gate runs this, but check the crate directly):
  ```sh
  RUSTDOCFLAGS="-D warnings" cargo doc -p buiy_core --no-deps
  ```
- [ ] Add a catalog line under the render-pipeline area's **Plans** subsection in `docs/README.md` (match the existing one-line entry format, e.g.):
  ```markdown
  - [Render R10: texture atlas + glyph/icon seam](plans/2026-06-03-buiy-render-r10-atlas.md) — the shared `BuiyAtlas` render resource (guillotiere allocation, LRU eviction, page-budget pressure, warmup, pooling), the glyph-alpha + icon primitive shapes, and the reserved gradient/mask entry kinds. Realizes atlas-and-text-seam.md.
  ```
- [ ] Run the full gate green, then commit:
  ```sh
  git add -A && git commit -m "docs(render/atlas): seam-boundary module doc + plan catalog entry"
  ```

---

## Done criteria

- [ ] `cargo test --workspace` green on this adapter-less host (all HEADLESS atlas tests pass; all GPU tests `#[ignore]`d/skipped).
- [ ] `crates/buiy_core/tests/atlas_alloc.rs` proves spec § 7's headless contract: (a) `get_or_insert` idempotent — closure runs once, no re-blit on hit; (b) LRU eviction under budget pressure frees the least-recently-used and `deallocate`s its `AllocId`; (c) the insert→idle cycle returns live-entry count to baseline with no monotonic page growth; pooling reuses an emptied page's texture handle.
- [ ] `crates/buiy_core/tests/atlas_primitive.rs` pins both primitive byte layouts.
- [ ] `guillotiere = "=0.6.2"` and `smallvec = "1"` are direct deps (spec § 2.1 pin).
- [ ] The GPU `#[ignore]` tests in `atlas_gpu.rs` document upload/sampling, alpha-as-color byte-identity, warmup determinism, and the gate-#15 return-to-baseline fixture for a future lavapipe runner.
- [ ] `docs/README.md` catalogs the plan.

## Cross-phase dependencies assumed

- **Consumed by `buiy-text-rendering-design` (downstream).** That spec builds `AtlasKey` from `(FontId, subpixel_bucket, glyph_id, px_size)`, rasterizes coverage on a miss, and emits `GlyphAlphaInstance`/`IconInstance`. This plan delivers exactly the § 3 insert API + § 4 primitive shapes it depends on; no cosmic-text type is referenced here.
- **The batched node (architecture.md phase, sibling).** The actual instanced draw that binds the atlas and consumes `GlyphAlphaInstance`/`IconInstance` lives in the typed-primitive-batched-node phase (architecture.md § "typed-primitive batched node"). This plan ships the primitive *shapes* and the atlas *resource*; the draw call that batches per (type, page, layer) and binds the atlas bind group is that phase's deliverable. The GPU `#[ignore]` upload/sampling test in Task 13 is the seam where the two phases meet.
- **`ClipRect` (clip-and-transform.md phase, sibling).** `GlyphAlphaInstance.clip`/`IconInstance.clip` carry a `ClipRect`-shaped `[f32; 4]`; the per-instance clip *semantics* are owned by the clip-and-transform phase. This plan only reserves the field.
- **Tuned numbers (`buiy-verification-design`).** `page_budget`/`eviction_grace`/ε are calibration owned downstream; this plan pins units + v1 defaults (`page_budget = 8`, `page_size = 1024`) only, exactly as spec § 2.4 commits.
