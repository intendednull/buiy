# Buiy Text T5: Fonts, Fallback, and BiDi Correctness — Implementation Plan

**Date:** 2026-06-10
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/font-assets.md](../specs/2026-06-09-buiy-text-rendering-design/font-assets.md) §§ 2–3, 6–8 + [measure-and-layout.md](../specs/2026-06-09-buiy-text-rendering-design/measure-and-layout.md) § 5.4 + [verification.md](../specs/2026-06-09-buiy-text-rendering-design/verification.md) § 2.2 + [architecture.md](../specs/2026-06-09-buiy-text-rendering-design/architecture.md) §§ 1.2, 2.2, 5.1
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T5 (depends on T4; the implementer starts from a branch with T1–T4 merged — T4 landed @ `8d32649`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fonts become an authoring surface and correctness becomes provable.
Land the `BuiyFont` asset + `BuiyFontLoader` (sfnt invariant, `Modified` =
remove+re-add); `FontRegistry` (strong handles, in-place `load_font_source`
add, rebuild-on-remove via `into_locale_and_db`, the AtlasKey-never-persisted
rule made mechanical via `FontDbLineage` + the hardened `FontKeyInterner`);
the Buiy-owned `FontStack` resolver (fontdb `Query` matching against a
lock-free `FontMatchIndex` snapshot, coverage span-splitting, `unicode-range`
filtering, cosmic-text's `FontFallbackIter` as the implicit per-glyph last
resort); `font-display` Swap (default) + Block (zero-alpha emission with the
3 s timeout); per-node `TextDirection` via the § 5.4 strong-mark prepend; and
the curated multi-script shaping-snapshot corpus over per-script OFL fixture
fonts.

**Architecture:** Everything new lives in `buiy_core::text` (the cosmic-text
boundary). The registry is a main-world resource; ONE system
(`apply_font_registry`, before `BuiySet::Layout`, after
`apply_system_font_scan`) drains staged ops + `AssetEvent<BuiyFont>` messages,
mutates the engine under a single lock hold per batch, rebuilds the
`FontMatchIndex` snapshot, and bumps `FontsGeneration` exactly once per batch
— so a frame never measures against a half-registered family (font-assets
§ 3). The resolver runs inside `TextSync`, **lock-free**, against the
snapshot: fontdb's real `query()` on a cloned `Database` (same-lineage IDs are
valid for the live engine) plus lazily-extracted per-face coverage sets
(skrifa charmap via `with_face_data` — no `FontSystem`, no lock). Resolution
lowers to `set_text` (single span) / `set_rich_text` (coverage splits) through
the existing lazy-setter path, so architecture § 1.2's steady-frame lock
discipline survives intact. Direction is lowered entirely in the `TextSync`
pre-pass (collapse → strong-mark prepend → resolve → set). The render-side
producer grows two value-compare probes (`FontsGeneration`,
`FontDbLineage`) and the Block zero-alpha arm; the atlas/consumer seam stays
frozen.

**v1 slice (font-assets §§ 2–8, verbatim):** ttf/otf/ttc/otc only (the sfnt
invariant IS the woff2 seam); strong-handle registry keyed by declared family
name; weight matching end-to-end (`Attrs.weight` → `Query.weight` →
`get_font(id, weight)` — variable-font weight already works); style/stretch
pass through `Query` as-is (no carriers yet — `Normal` defaults);
`font-display` Swap + Block only; programmatic `UnicodeRanges` (no CSS string
parsing); entity-level Block granularity; `TextDirection { Ltr, Rtl, Auto }`
with Auto = absent = cosmic's first-strong default.

**Where T5 ends (honesty pins — named seams, not built):**

- **Rich-text spans** (per-span `FontFamily`/stacks, inline `dir`
  isolates `<bdi>`) — the C-tier rich-text tier (measure § 5.4's isolate
  paragraph; font-assets § 8). The resolver's span machinery is
  *coverage*-driven only; authored spans are the successor seam.
- **Variable-font axes beyond weight** (`font-variation-settings`) — blocked
  on upstream #406 (font-assets § 9). Weight-only is committed and works.
- **Theme font tokens** (token → `FontStack` indirection, generic-family
  rebinding) — `buiy-theme-tokens-design` (font-assets § 9). This plan defines
  the resolver entry point that work will target.
- **woff2** — the loader's sfnt-magic validation is the named decompression
  seam (font-assets § 2). Rejected bytes get an error naming it.
- **`font-display` Fallback/Optional** — enum variants exist (the spec's
  descriptor shape), degrade to Swap with a warn-once. Per-run Block
  granularity and a configurable timeout are named seams; v1 is entity-level
  + a 3 s const.
- **Family-alias overrides** (`push_face_info` renaming) — C-tier (§ 9). A
  declared-name/internal-name mismatch warns loudly and will not match (the
  alias seam is the fix).
- **CSS `unicode-range` string parsing** — descriptors take
  `RangeInclusive<u32>` programmatically; the string syntax is styling-tier.
- **Editing byte-offset mapping** — the strong mark shifts line byte offsets
  by 3 (its UTF-8 length); hit-testing/cursor↔source mapping maps through the
  same pre-pass offset table as the collapse transform — the successor
  `buiy-text-editing` campaign's seam (measure § 5.4, noted in code).

**Tech stack:** cosmic-text 0.19.0 (default features only — load-bearing:
`monospace_fallback` is NOT default, see Orientation), fontdb 0.23.0 (via the
crate-root re-export), skrifa (via cosmic-text's crate-root `pub use skrifa`),
Bevy 0.18.1 (`bevy_asset` already enabled, workspace `Cargo.toml:46`).
**No new dependencies** — if a task appears to need one, STOP: that
contradicts the charter. (`cargo deny check` is not required: no dep changes.)

**Test reality:** the registry, loader, resolver, direction, font-display,
and the entire snapshot corpus are headless (shaping is pure CPU). The GPU
lane carries two pixels-only additions: the multi-script golden and the
rebuild-storm bound. Every GPU test keeps `#[ignore]` and builds on
`tests/support/mod.rs`.

---

## The gate (run BOTH lanes at every task boundary)

T5 ships render-world probe changes and two GPU tests, so the per-task gate is
the headless gate **plus the GPU lane** (this host has the RX 6700 XT / RADV;
Vulkan render-to-texture needs no display):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace -j 2
```

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

Expected: both green. The headless gate must stay green **independently** (CI
has no adapter); the GPU lane is additive and must pass on this host before
the phase merges. A GPU-needing test without `#[ignore]` panics the headless
run at adapter init — self-policing.

---

## Orientation: verified facts this plan builds on

Source-verified against the vendored crates
(`~/.cargo/registry/src/*/cosmic-text-0.19.0/`, `fontdb-0.23.0/`,
`bevy_asset-0.18.1/`). Re-verify file/line refs before editing — they drift.

**THE two questions the charter ordered verified, answered:**

1. **fontdb `ID` stability across remove/re-add — the registry's correctness
   hinge.** `ID` is a `slotmap` key (`fontdb` lib.rs:75, 93–97:
   `faces: SlotMap<InnerId, FaceInfo>`; slot index + 32-bit version).
   Verified semantics:
   - **Within one `Database` value ("lineage"):** `remove_face(id)`
     (lib.rs:601) removes only that slot; **every surviving face keeps its
     ID**. A removed slot reused by a later insert gets a **bumped version**,
     so a dead ID never aliases a new face — lookups with it return `None`
     forever. Distinct faces always have distinct IDs over a lineage's
     lifetime.
   - **`into_locale_and_db` carries the SAME `Database` by value**
     (cosmic-text system.rs:297–299: `(self.locale, self.db)`), so the
     font-assets § 3.1 unregister rebuild does **NOT** reissue surviving IDs.
     The spec's § 3.2 claim ("every rebuild issues fresh IDs for every face")
     is **wrong for that path** — recorded as erratum 1 (Task 11).
   - **Across different `Database` instances** (the § 5 system-scan swap
     builds a fresh db): a fresh `SlotMap` hands out the same `(slot, v1)`
     key values in insertion order, so **two different faces in two
     databases can carry EQUAL ID values**. The as-built `FontKeyInterner`
     (`HashMap<fontdb::ID, u32>`, never evicted) would then return the OLD
     `u32` for a NEW face → `AtlasKey` aliasing → the atlas silently serves
     the wrong font's bitmaps. **This is the hazard the
     AtlasKey-never-persisted rule exists for**, and rebuilding keys from
     live IDs every emission does *not* close it (the fresh key aliases the
     resident old key). Task 1 closes it mechanically: `FontDbLineage`
     bumped only on fresh-db swaps; the interner clears its map per lineage
     while its `u32` counter stays **monotonic** (the as-built
     `len()`-as-next allocation would re-issue seat 0 after a clear —
     aliasing with grace-resident entries).

2. **`FontFallbackIter` in 0.19's public API — verified PUBLIC, and Buiy
   never needs to construct one.** `pub struct FontFallbackIter<'a>` with a
   `pub fn new(...)` lives in `pub mod fallback` (font/mod.rs:21), glob
   re-exported through the crate root (`lib.rs:118 pub use self::font::*`),
   so it is reachable at `cosmic_text::fallback::FontFallbackIter` (and its
   `FontMatchKey` input is `pub`, system.rs:21, with
   `pub fn get_font_matches`, system.rs:359). But its load-bearing role is
   **engine-internal**: shaping invokes it per run (shape.rs:307, 489, 985)
   whenever the concrete `Attrs.family` face misses a glyph — script lists
   from the injected `BuiyFallback`, then the whole-db sweep. The spec's
   "remains the last-resort safety net … runs only when the author's entire
   stack missed" is accurate as an *implicit* mechanism; the resolver leans
   on it by handing cosmic-text concrete families and nothing else. No
   erratum needed; a clarifying note rides erratum 4.

| Fact | Verified shape |
|---|---|
| `Database::load_font_source(&mut self, Source) -> TinyVec<[ID; 8]>` | fontdb lib.rs:203 — collections register every face; **no dedup** (re-loading the same source yields new faces with new IDs) |
| `Database::remove_face(&mut self, id)` / `push_face_info` | lib.rs:601 / :587 (`push_face_info` = the C-tier alias seam, not used here) |
| `Database` is `Clone` | lib.rs:150–152 (`#[derive(Clone, Debug)]`) — the `FontMatchIndex` snapshot is a clone; **cloned slotmap keys are identical**, so snapshot IDs are valid for the live engine within a lineage |
| `Database::query(&self, &Query) -> Option<ID>` | lib.rs:661–679 — walks `families` in priority order, name-matches `FaceInfo.families`, then CSS `find_best_match` (weight/stretch/style). `Query { families: &[Family], weight, stretch, style }` (lib.rs:929) |
| `Database::with_face_data(&self, id, FnOnce(&[u8], u32) -> T) -> Option<T>` | lib.rs:721 — `&self`, works for Binary/SharedFile/File sources (File re-maps per call) — the lock-free coverage route |
| `Font::unicode_codepoints()` is **feature-gated empty** | font/mod.rs:46, 83–86, 145–195: populated only under `monospace_fallback`, which is **NOT** in `default = ["std", "swash", "fontconfig"]` (Cargo.toml). Under Buiy's default-features pin it returns `&[]` — the spec-era assumption that cosmic exposes coverage is dead; Task 5 extracts coverage Buiy-side |
| `Font::new(db: &Database, id, weight) -> Option<Font>` returns `None` for `Source::File` | font/mod.rs:122–131 ("Unsupported fontdb Source::File") — another reason Font-based coverage is out |
| skrifa is re-exported | font/mod.rs:8 `pub use skrifa` → `cosmic_text::skrifa::{prelude::*, raw::FontRef}`; `FontRef::from_index(data, index)` + `.charmap().mappings()` yields `(u32 codepoint, GlyphId)` — exactly what `Font::new`'s gated block iterates (font/mod.rs:182–187) |
| `Buffer::set_rich_text(spans, &default_attrs, Shaping, Option<Align>)` | buffer.rs:1102–1111 — `I: IntoIterator<Item = (&'s str, Attrs<'r>)>`; **lazy** (no `FontSystem`), sets `DirtyFlags::TEXT_SET` like `set_text` |
| `Attrs<'a>` fields | attrs.rs:286–300 — `family: Family<'a>` (single), `weight`, `style`, `stretch`, … — per-node stacks cannot live inside cosmic-text (the spec's § 6 premise, re-verified) |
| `LayoutGlyph.font_id: fontdb::ID` | layout.rs:43 — pub; the end-to-end span-assignment assertion route |
| `FontSystem::get_font(&mut self, ID, Weight) -> Option<Arc<Font>>` | system.rs:302 — caches in the unpurgeable `font_cache`; **the § 3.1 rebuild is what keeps it honest** (a fresh `FontSystem` = a fresh cache); post-rebuild `get_font(dead_id, w)` returns `None` — the staleness assertion |
| `db_mut()` clears only `font_matches_cache` | system.rs (`pub fn db_mut`) — in-place **addition** is safe (a new face can never make a match-cache entry stale); removal is not (the § 3.1 rationale, re-verified) |
| bevy 0.18 `AssetLoader` | bevy_asset loader.rs:32 — `type Asset/Settings/Error`; `async fn load(&self, reader: &mut dyn Reader, settings: &S, load_context: &mut LoadContext<'_>) -> Result<A, E>`; `fn extensions() -> &[&str]`. `ImageLoader` (bevy_image image_loader.rs:169) is the canonical impl shape |
| `init_asset`/`register_asset_loader` **panic without `AssetServer`** | bevy_asset lib.rs:590–596, 637–641 (`self.world().resource::<AssetServer>()`) — the plugin's asset half must be gated on `contains_resource::<AssetServer>()` (headless `text_app()` fixtures add no `AssetPlugin`) |
| `AssetEvent<A>` is a **Message** | bevy_asset event.rs:49–60 — `Added/Modified/Removed/Unused/LoadedWithDependencies { id: AssetId<A> }`; consumed via `MessageReader`, registered via `add_message::<AssetEvent<A>>()` (lib.rs:656; idempotent — safe to call without `AssetPlugin`) |
| `unicode_script` classification | already a direct dep (T1); `use unicode_script::UnicodeScript; char::script()` → `Script::Common`/`Script::Inherited` = the never-split classes (the HarfBuzz itemization rule) |

As-built code consumed (read before editing, confirm current — **extend,
never redefine**):

- `crates/buiy_core/src/text/font_system.rs` — `SharedFontSystem` (+ the
  poison-panic `lock()`), `BuiyFallback` (`script_fallback` deliberately
  empty "until T5 pins deterministic per-script lists" — Task 8 fills it),
  `FontsGeneration` (doc: "runtime asset registration joins in T5"),
  `registered_fonts_db()` (doc: "T5's `FontRegistry` extends it"),
  `build_font_system`, `buiy_locale`.
- `crates/buiy_core/src/text/system_scan.rs` — `PendingSystemFontScan`,
  `spawn_system_font_scan` (doc: "T5's `FontRegistry` adds every registered
  `Source::Binary`"), `apply_system_font_scan` (bumps generation exactly
  once), `swap_font_db` (the § 3.1 mem::replace dance — Task 5 extends it
  with the registry re-add), `placeholder_font_system` (private — Task 4
  moves it to `font_system.rs` as `pub(crate)`; the registry rebuild needs
  the same dance).
- `crates/buiy_core/src/text/sync.rs` — `TextSyncTriggers` (10-member `Or`;
  ledger names `TextDirection` as the T5 join), `SyncedText` (9-tuple),
  `AuthoredStyle` (+ `attrs()`'s "T2 INTERIM: the stack's FIRST entry only —
  the Buiy-owned resolver … is T5's"), `apply_authored` (doc: "The § 5.4
  direction strong-mark prepend (T5) slots between the collapse transform
  and `set_text`, AFTER the trim"), the `FontsGeneration` full-sweep arm.
- `crates/buiy_core/src/text/whitespace.rs` — `collapse_whitespace` +
  `CollapseMode`; module doc reserves the T5 prepend slot.
- `crates/buiy_core/src/text/atlas_key.rs` — `FontKeyInterner` (the
  `len()`-as-next allocation Task 1 hardens), `glyph_atlas_key` (19 B,
  unchanged), `GLYPH_KEY_LEN`.
- `crates/buiy_core/src/text/extract.rs` — `extract_buiy_glyphs` (the § 6.2
  probe union + `ResidentTextKeys { keys, last_scale_factor }` value-compare
  idiom Task 5 extends; the `removed.read()` drain-first discipline; the
  `color[3] == 0.0 → continue` skip Task 7 conditions; `theme:
  Extract<Res<Theme>>` is the main-world-resource extraction precedent),
  `resolve_glyph`, `GlyphMetaCache`.
- `crates/buiy_core/src/text/components.rs` — the carriers + `TextBuffer` +
  `ComputedTextLayout`/`ComputedTextLine` (`rtl` flag — the direction
  assertion route) + `TextStyleDefaults`; the warn-once statics precedent.
- `crates/buiy_core/src/text/mod.rs` — `BuiyTextPlugin` (resource init,
  reflect registration, `apply_system_font_scan.before(BuiySet::Layout)`
  precedent), `register_render_world`.
- `crates/buiy_core/src/text/commit.rs` + `measure.rs` — **unchanged by T5**
  (the resolver changes what attrs the buffer holds; shaping sites are
  agnostic). Read for the lazy-`Option<MutexGuard>` pattern Task 4 copies.
- `tools/fonts/subset_default_font.sh` — THE provenance-script precedent
  Task 8 mirrors per fixture font: pinned upstream URL + sha256, pinned
  fonttools (4.56.0), `pyftsubset` flags keeping name records (OFL) +
  `--notdef-outline`, output committed, never hand-edited.
- `crates/buiy_core/tests/support/mod.rs` — `gpu_render_app`,
  `render_to_image`, `spawn_capture_camera`, `finish_and_run`,
  `wait_for_text_ready`, `readback_rgba`, `px` (Task 8 adds the
  fixture-font helpers); `tests/support/extract_harness.rs` —
  `TextExtractHarness` (main app WITHOUT RenderPlugin + manual render
  `World` + manual `ExtractSchedule`; `GlyphChangeLog` mirrors the prepare
  gate) — Tasks 5/7 drive the producer through it headless.
- `crates/buiy_core/tests/text_gpu.rs` — `spawn_text_fixture` /
  `capture` / `brightest`; the `GoldenConfig::deterministic()` +
  `wait_for_text_ready` + `perceptual_diff` shape Task 10 extends.
- `crates/buiy_core/tests/text_sync.rs` — `text_app()` (MinimalPlugins +
  CorePlugin + LayoutPlugin + BuiyTextPlugin, **no AssetPlugin**) + `settle`
  + trigger-count assertions — the headless fixture shape Tasks 2/6/7 grow.
- `tests/atlas_register.rs:27` — `app.add_plugins(bevy::asset::AssetPlugin::default())`
  — the headless-AssetPlugin precedent Task 3's loader tests follow.

## Decisions (with runner-ups) — read before implementing

1. **fontdb ID instability is split into two regimes, and only fresh-db swaps
   invalidate the interner.** `FontDbLineage(u64)` (main world, extracted) is
   bumped ONLY when a fresh `Database` replaces the engine's (the system-scan
   swap; any future restore-from-scratch). In-lineage ops — registration
   add, unregister rebuild (`into_locale_and_db` carries the db), hot-reload
   remove+re-add — never touch it: surviving IDs stay valid and dead IDs
   never alias (slotmap versioning, Orientation fact 1). On a lineage change
   the `FontKeyInterner` clears its map but keeps a **monotonic** `next`
   counter, so `u32` seats are never reused while old atlas entries are still
   grace-resident. *Runner-up rejected: clear on every `FontsGeneration`
   bump* — registering one new font would storm every glyph of every other
   font for nothing. *Runner-up rejected: content-stable interning (postscript
   name / data hash)* — name-stable seats would serve **stale bitmaps** across
   a hot-reload edit of the same family (silent corruption beats a bounded
   storm — wrong way around).
2. **The resolver runs lock-free in `TextSync` against a `FontMatchIndex`
   snapshot.** The index holds a `fontdb::Database` **clone** (rare-event
   rebuild, under the same lock holds that mutate the engine) + a lazily
   filled `HashMap<ID, CoverageSet>`. `query()` on the clone is fontdb's real
   CSS matcher; same-lineage IDs are valid against the live engine; coverage
   extraction uses `with_face_data` + the re-exported skrifa charmap — zero
   locks, zero new deps. *Runner-up rejected: lock in `TextSync`* — breaks
   architecture § 1.2's lazy-setter F-pin and adds a per-keystroke lock site.
   *Runner-up rejected: resolve at the measure closure / `TextCommit`* —
   intrinsics would be measured against unresolved attrs (wrong widths on the
   first frame, the worst kind of wrong). *Runner-up rejected: re-implement
   CSS matching Buiy-side* — fontdb's `find_best_match` already is the
   CSS-mapped matcher; duplicating it invites drift.
3. **Coverage = skrifa charmap through `with_face_data`, cached per face
   ID.** `Font::unicode_codepoints()` is empty under the default-features pin
   (`monospace_fallback` gate — Orientation) and `Font::new` rejects
   `Source::File`; `with_face_data` handles every source kind from `&self`.
   Sets are sorted+deduped `Vec<u32>` + binary search. *Runner-up rejected:
   enabling `monospace_fallback`* — the charter pins default features
   (campaign pre-phase decision 1's posture), and it pays full `Font`
   construction (harfrust shaper) per probe. *Runner-up rejected: a
   `ttf-parser` dep* — new dependency for what the re-exported skrifa does.
4. **Registration requires the declared family name** (the CSS `@font-face`
   model): `register_asset(family, handle, descriptors)` /
   `register_bytes(family, bytes, descriptors)`. The declared name keys the
   record, carries the Loading state (it exists before the file's internal
   names are knowable — without it `font-display: Block` is unimplementable),
   and is what stacks reference. After load, the faces' internal family names
   are validated: a mismatch warns loudly (per family, once) — the resolver
   queries by name, so a mismatched registration will not match until the
   C-tier alias seam (`push_face_info`, font-assets § 9) lands. *Runner-up
   rejected: infer the family post-load* — loses Loading-state tracking and
   silently diverges from `@font-face`. **Erratum 3:** the spec's `FontKey`
   return is dropped — family-name identity is the registry's whole public
   surface and `FontKey` is defined nowhere else.
5. **Registry methods stage ops; ONE system applies them.**
   `register_*`/`unregister_family` push `RegistryOp`s; `apply_font_registry`
   (Update, `.after(apply_system_font_scan).before(BuiySet::Layout)`) drains
   ops + `AssetEvent<BuiyFont>` messages, takes the `SharedFontSystem` lock
   **lazily once per batch**, performs in-place adds
   (`db_mut().load_font_source` — safe, Orientation) and rebuild-removals
   (the § 3.1 `into_locale_and_db` dance; `Modified` = remove+re-add under
   the same hold; `Removed` = forced unregister), re-snapshots the
   `FontMatchIndex`, and bumps `FontsGeneration` once. This is a **rare-event
   lock site** — architecture § 1.2's "exactly three" table is steady-frame
   scoped; `swap_font_db` has been a fourth (rare) site since T1. Recorded as
   erratum 2 for the spec edit pass, not silently widened. *Runner-up
   rejected: registry methods mutating the engine directly* — scatters lock
   acquisition across arbitrary user call sites and makes the
   one-bump-per-batch contract unenforceable.
6. **The system-scan swap re-adds registry sources on the main thread at
   apply time, under the swap's own lock hold.** The scan task stays
   registered-baseline + `load_system_fonts()`; `apply_system_font_scan`
   then re-adds every registered record's bytes into the fresh db, re-records
   the fresh face IDs into the registry, re-snapshots the index, and bumps
   `FontsGeneration` + `FontDbLineage` together. *Runner-up rejected: re-add
   inside the task* — a font registered **during** the scan would vanish from
   the swapped db (a real race), and the task would have to plumb
   per-source ID vectors back. Parsing dozens of registered faces on the
   main thread at a rare event is noise next to the reshape the swap forces
   anyway.
7. **Resolver semantics (the § 6 walk, pinned):** chars whose
   `unicode_script::Script` is `Common`/`Inherited`/`Unknown` never force a
   span boundary (they join the current span; leading ones attach to the
   first resolved span — the HarfBuzz itemization rule; this also keeps the
   prepended LRM/RLM and ZWJ/VS16 from fragmenting spans). For each other
   char, walk the stack: a **Generic entry is terminal** (lowered as the
   cosmic generic — the `registered_fonts_db` pins make it deterministic; no
   coverage check); a **Named entry** consults the registry (Loading →
   skip [+ Block marking, decision 9]; Failed → skip; `unicode_range` declared
   and char outside → skip) then `query()` by name + weight (+ default
   style/stretch) and wins iff the matched face **covers** the char. A char no
   entry covers resolves to the **first** stack entry — cosmic's
   `FontFallbackIter` then patches per-glyph (the spec's last-resort
   contract). Adjacent same-resolution chars merge into one span. *Runner-up
   rejected: coverage-checking generics* — the generic IS the author's
   catch-all; splitting below it duplicates what `FontFallbackIter` already
   does per-glyph. *Runner-up rejected: whole-run-or-nothing matching* — the
   spec pins per-codepoint splitting (font-assets § 6.1's interim
   "whole-run" posture applied only until span-splitting landed — it lands
   here).
8. **Span lowering: `set_text` for ≤ 1 span, `set_rich_text` for splits**
   (both lazy, verified). Resolved span families are **owned `String`s**
   (cloned from registry/FaceInfo names at resolve time) — resolution only
   runs on damage-gated syncs; interning/`SmolStr` is a named perf seam, not
   built. *Runner-up rejected: borrowing names from the index* — ties
   `Attrs<'a>` lifetimes through a `ResMut` split-borrow for a hot path that
   does not exist (sync is trigger-gated).
9. **`font-display` Block is entity-level zero-alpha with a 3 s const
   deadline.** The resolver reports `blocked` when any char's walk passed a
   `Loading`+`Block` family still inside its window; `TextSync`
   inserts/removes `PendingFontBlock { until }` (deadline =
   the record's `loading_since` + `FONT_BLOCK_TIMEOUT_SECS = 3.0`, the web
   default); the producer forces `color[3] = 0.0` for blocked entities
   (bypassing the zero-alpha skip — instances ARE emitted, the spec's § 7
   shape, keeping fallback glyphs warm); `expire_font_block` removes expired
   components; the producer's probe union gains
   `Changed<PendingFontBlock>` + its removal stream. Layout is the fallback
   family's both before and after — geometry never jumps twice (§ 7
   verbatim). *Runner-up rejected: skip emission instead of zero-alpha* —
   pixel-identical but diverges from the spec's named mechanism for no win.
   *Runner-up rejected: per-run granularity* — needs per-span damage
   plumbing; rich-text tier.
10. **Strong marks are prepended per NON-EMPTY line, after collapse.**
    `Ltr` → LRM (U+200E), `Rtl` → RLM (U+200F), `Auto` → nothing
    (absent component = `Auto` — cosmic's per-paragraph first-strong default
    IS `dir=auto`, measure § 5.4). Per buffer line because cosmic treats
    each line as a UAX #9 paragraph (P2 runs per line). Empty lines stay
    unmarked: a shaped mark could grow a phantom glyph and flip T3's
    glyphs-keyed `ResolvedBaseline` semantics for `Text("")`; the editing
    campaign revisits empty-line caret direction with the offset table.
    The direction value joins the intrinsics content version for free — T2's
    wholesale `invalidate_intrinsics()` on every sync covers it (noted in
    code, no new mechanism).
11. **Shaping snapshots are committed text fixtures with an accept env.**
    `tests/fixtures/shaping/<name>.snap` — header + one line per glyph:
    `line_i glyph_id font_seat x y` (x/y at 0.01 precision, `font_seat` =
    per-fixture face index so the file is stable across fontdb ID churn);
    mismatch prints a unified diff; `BUIY_ACCEPT_SHAPING=1` regenerates
    (the golden `--accept` analogue, human-curated). *Runner-up rejected:
    `insta`* — a new dev-dep for ~50 lines of harness. *Runner-up rejected:
    inline expected constants* — multi-script corpora are unreviewable as
    Rust literals and unmaintainable on a curated font bump.
12. **Five OFL fixture subsets: Arabic, Hebrew, Devanagari, CJK (SC), emoji
    (monochrome).** The spec's mixed-BiDi string is Hebrew
    (`"hello עולם world"`, verification § 2.2) — Hebrew stays so the corpus
    pins **non-joining** RTL separately from Arabic's joining RTL.
    Monochrome Noto Emoji (outline glyphs → `SwashContent::Mask`) because the
    v1 producer skips `Color` content (T4 § 9) — a color-emoji fixture would
    pin nothing paintable. All five ride the T1 provenance-script pattern
    (pinned upstream + sha256 + pinned fonttools), subset to the corpus
    codepoints, committed under `tests/fixtures/fonts/` (repo test fixtures,
    NOT embedded in the library — font-assets § 4). *Runner-up rejected:
    Arabic-only mixed-BiDi (4 fonts)* — loses the spec's literal string and
    the non-joining-RTL axis for a ~30 KB saving.
13. **The plugin's asset half is gated on `AssetServer` presence;
    `add_message::<AssetEvent<BuiyFont>>()` is unconditional.** Headless
    fixtures (`text_app()`) carry no `AssetPlugin`; `init_asset` /
    `register_asset_loader` panic without the server (Orientation). The
    unconditional message registration (idempotent) lets
    `apply_font_registry` keep a plain `MessageReader` in every world; the
    bytes path (`register_bytes`) needs no asset machinery at all.
14. **`BuiyFallback::script_fallback` gets deterministic per-script entries**
    naming the fixture families (Task 8) — the T1 placeholder's documented
    fill: CI-deterministic last-resort lists for the corpus scripts, never
    platform-dependent. Apps that register none of those families lose
    nothing (absent names are skipped — the T1 `common_fallback` comment's
    contract).

## File structure

| File | Action | Responsibility |
|---|---|---|
| `crates/buiy_core/src/text/atlas_key.rs` | modify | interner hardening: monotonic `next`, `begin_lineage` |
| `crates/buiy_core/src/text/font_system.rs` | modify | `FontDbLineage`; `placeholder_font_system` moves here `pub(crate)` |
| `crates/buiy_core/src/text/components.rs` | modify | `TextDirection` carrier |
| `crates/buiy_core/src/text/direction.rs` | create | `prepend_strong_marks` (the § 5.4 pre-pass) |
| `crates/buiy_core/src/text/font_asset.rs` | create | `BuiyFont`, `BuiyFontLoader`, sfnt sniff, loader error |
| `crates/buiy_core/src/text/registry.rs` | create | `FontRegistry`, `FamilyRecord`, `FontFaceDescriptors`, `FontDisplay`, `FontLoadState`, `UnicodeRanges`, `PendingFontBlock`, `apply_font_registry`, `expire_font_block` |
| `crates/buiy_core/src/text/match_index.rs` | create | `FontMatchIndex` (db snapshot + lazy `CoverageSet`s) |
| `crates/buiy_core/src/text/resolver.rs` | create | `resolve_spans` (stack walk, span split, block flag) |
| `crates/buiy_core/src/text/sync.rs` | modify | direction + resolver integration; `TextDirection` trigger |
| `crates/buiy_core/src/text/system_scan.rs` | modify | swap re-adds registry sources, re-records IDs; lineage bump |
| `crates/buiy_core/src/text/extract.rs` | modify | generation/lineage probes; Block zero-alpha arm |
| `crates/buiy_core/src/text/mod.rs` | modify | modules, exports, plugin wiring (asset gating, new systems) |
| `tools/fonts/subset_fixture_fonts.sh` | create | per-script OFL subset provenance script |
| `crates/buiy_core/tests/fixtures/fonts/` | create | 5 subset ttf/otf + OFL licenses (script output, committed) |
| `crates/buiy_core/tests/fixtures/shaping/` | create | committed `.snap` corpus files |
| `crates/buiy_core/tests/text_fontdb_semantics.rs` | create | Task 1's ID-semantics pins + interner tests |
| `crates/buiy_core/tests/text_direction.rs` | create | § 5.4 pure-fn + end-to-end rtl tests |
| `crates/buiy_core/tests/text_font_asset.rs` | create | loader tests (with `AssetPlugin`) |
| `crates/buiy_core/tests/text_registry.rs` | create | register/unregister/hot-reload/leak tests |
| `crates/buiy_core/tests/text_resolver.rs` | create | match-index, resolver, span-split, unicode-range tests |
| `crates/buiy_core/tests/text_font_display.rs` | create | Swap/Block transitions (extract harness for alpha) |
| `crates/buiy_core/tests/text_shaping_snapshots.rs` | create | the multi-script corpus harness |
| `crates/buiy_core/tests/text_extract.rs` | modify | generation/lineage probe tests (extract harness) |
| `crates/buiy_core/tests/text_gpu.rs` | modify | multi-script golden + rebuild-storm bound (`#[ignore]`) |
| `crates/buiy_core/tests/support/mod.rs` | modify | fixture-font loading/registration helpers |
| `docs/plans/2026-06-09-buiy-text-campaign.md`, `docs/README.md` | modify | docs flip + errata (Task 11) |

---

### Task 1: Pin fontdb ID semantics + harden the `FontKeyInterner` (headless)

The charter's mandated early verification step, executable form: the
Orientation-fact-1 findings become drift tripwires (they re-prove themselves
on every fontdb bump), and the interner gains the lineage mechanics those
findings force.

**Files:**
- Create: `crates/buiy_core/tests/text_fontdb_semantics.rs`
- Modify: `crates/buiy_core/src/text/atlas_key.rs`
- Modify: `crates/buiy_core/src/text/font_system.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_fontdb_semantics.rs`:

```rust
//! THE fontdb ID-stability pins (font-assets § 3.2, corrected — see the T5
//! plan's Orientation fact 1 and erratum 1): fontdb `ID`s are slotmap keys.
//! Within one `Database` value ("lineage") surviving faces keep their IDs
//! across `remove_face` and dead IDs never alias; across DIFFERENT
//! `Database` instances equal ID values name different faces. These tests
//! are drift tripwires for any fontdb bump — if one fails after an upgrade,
//! the FontKeyInterner lineage mechanics below it are what's at stake.

use std::sync::Arc;

use buiy_core::text::{FontKeyInterner, registered_fonts_db};
use cosmic_text::fontdb;

/// The embedded default font bytes, loaded twice to get two distinct faces
/// in one db (fontdb does NOT dedup sources — verified, Orientation table).
fn two_face_db() -> (fontdb::Database, fontdb::ID, fontdb::ID) {
    let mut db = registered_fonts_db();
    let first = db.faces().next().expect("embedded face").id;
    let bytes: Arc<Vec<u8>> = Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .expect("embedded font artifact exists"),
    );
    let ids = db.load_font_source(fontdb::Source::Binary(bytes));
    assert_eq!(ids.len(), 1);
    (db, first, ids[0])
}

#[test]
fn surviving_ids_are_stable_across_remove_face() {
    // The § 3.1 unregister path: into_locale_and_db carries the SAME
    // Database, so removal of one face must leave every other ID valid.
    let (mut db, keep, remove) = two_face_db();
    assert!(db.face(keep).is_some() && db.face(remove).is_some());
    db.remove_face(remove);
    assert!(db.face(keep).is_some(), "surviving face keeps its ID");
    assert!(db.face(remove).is_none(), "removed ID is dead");
}

#[test]
fn dead_ids_never_alias_within_a_lineage() {
    // Slot reuse bumps the slotmap version: a re-added face gets a NEW id,
    // and the dead id keeps returning None forever — in-lineage interner
    // entries can never serve the wrong face.
    let (mut db, _keep, remove) = two_face_db();
    db.remove_face(remove);
    let bytes: Arc<Vec<u8>> = Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .unwrap(),
    );
    let readded = db.load_font_source(fontdb::Source::Binary(bytes))[0];
    assert_ne!(readded, remove, "re-add issues a fresh ID");
    assert!(db.face(remove).is_none(), "the dead ID stays dead");
    assert!(db.face(readded).is_some());
}

#[test]
fn fresh_databases_reissue_equal_id_values_for_different_faces() {
    // THE aliasing hazard (Orientation fact 1, erratum 1): two fresh
    // databases hand out the same (slot, version) key values in insertion
    // order, so ID equality is meaningless across lineages. This is the
    // fact FontDbLineage + the interner clear exist for.
    let db_a = registered_fonts_db();
    let db_b = registered_fonts_db();
    let a = db_a.faces().next().unwrap().id;
    let b = db_b.faces().next().unwrap().id;
    assert_eq!(a, b, "equal ID values across independent databases");
    // (Same bytes here, but nothing about the VALUE says so — a system
    // scan puts arbitrary faces in these slots.)
}

#[test]
fn interner_clears_per_lineage_but_never_reuses_seats() {
    let (db, first, second) = two_face_db();
    drop(db);
    let mut interner = FontKeyInterner::default();
    assert_eq!(interner.intern(first), 0);
    assert_eq!(interner.intern(second), 1);
    assert_eq!(interner.intern(first), 0, "idempotent within a lineage");

    // Lineage 1 → 2: the map clears (old IDs are meaningless now)…
    assert!(interner.begin_lineage(2));
    assert!(!interner.begin_lineage(2), "same lineage = no-op");
    assert_eq!(interner.len(), 0, "map cleared");
    // …but seats stay monotonic: the same ID VALUE re-interned after the
    // clear gets a FRESH u32 — never seat 0/1, which may still name
    // grace-resident atlas entries of the OLD faces. (The as-built
    // len()-as-next allocation would hand back 0 here — the aliasing bug
    // this test exists to prevent.)
    assert_eq!(interner.intern(first), 2);
    assert_eq!(interner.intern(second), 3);
}

#[test]
fn lineage_resource_defaults_to_zero() {
    use buiy_core::text::FontDbLineage;
    assert_eq!(FontDbLineage::default().0, 0);
}
```

Run: `cargo test -p buiy_core --test text_fontdb_semantics`
Expected: FAILS — `begin_lineage` and `FontDbLineage` do not exist; the
seat-reuse assertion fails against the `len()`-based interner.

- [ ] **Step 2: Implement**

In `crates/buiy_core/src/text/font_system.rs`, after `FontsGeneration`:

```rust
/// The font-database LINEAGE counter (T5; font-assets § 3.2 corrected —
/// see the T5 plan's erratum 1): bumped ONLY when a FRESH `fontdb::Database`
/// replaces the engine's (the system-scan swap). In-lineage mutations —
/// registry adds, the § 3.1 unregister rebuild, hot-reload remove+re-add —
/// keep surviving IDs valid (`into_locale_and_db` carries the same
/// `Database` by value; slotmap keys of untouched faces never change) and
/// MUST NOT bump this. Consumers: the render-world `FontKeyInterner` clears
/// its ID map per lineage (equal fontdb ID values name DIFFERENT faces
/// across databases — the AtlasKey-aliasing hazard § 3.2's
/// never-persist rule exists for). Every lineage bump is accompanied by a
/// `FontsGeneration` bump (the reshape + producer-rebuild trigger).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FontDbLineage(pub u64);
```

Also move `placeholder_font_system` from `system_scan.rs` here as
`pub(crate)` (Task 4's registry rebuild needs the same parked-placeholder
dance; one definition):

```rust
/// Briefly parked in the mutex during the § 3.1 mem::replace rebuild dance
/// (`swap_font_db`, `apply_font_registry`); never observable — every swap
/// completes under one lock hold.
pub(crate) fn placeholder_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db(String::from("en-US"), fontdb::Database::new())
}
```

(Adjust `system_scan.rs` to import it; delete its private copy.)

In `crates/buiy_core/src/text/atlas_key.rs`, rework the interner:

```rust
/// Render-world interner: `fontdb::ID` → stable `u32` seat (glyph-pipeline
/// § 4). Seats are MONOTONIC for the life of the resource — never reused —
/// because old atlas entries keyed under old seats stay grace-resident
/// after any font-set change; a reused seat would alias them (stale-bitmap
/// corruption, the § 3.2 hazard). The ID map is valid only within one
/// database lineage (fontdb IDs are slotmap keys — equal VALUES name
/// different faces across databases), so `begin_lineage` clears it whenever
/// `FontDbLineage` advances; `next` survives the clear.
#[derive(Resource, Default)]
pub struct FontKeyInterner {
    ids: HashMap<fontdb::ID, u32>,
    next: u32,
    lineage: u64,
}

impl FontKeyInterner {
    /// The stable `u32` seat for `font` — allocated on first sight within
    /// the current lineage, identical until the next lineage change.
    pub fn intern(&mut self, font: fontdb::ID) -> u32 {
        match self.ids.entry(font) {
            Entry::Occupied(seat) => *seat.get(),
            Entry::Vacant(slot) => {
                let seat = self.next;
                self.next += 1;
                *slot.insert(seat)
            }
        }
    }

    /// Synchronize with the main world's `FontDbLineage`: on a change,
    /// clear the ID map (every fontdb ID was reissued — the § 3.2 storm)
    /// while keeping `next` monotonic. Returns true when a clear happened.
    pub fn begin_lineage(&mut self, lineage: u64) -> bool {
        if self.lineage == lineage {
            return false;
        }
        self.ids.clear();
        self.lineage = lineage;
        true
    }

    /// Number of fonts interned in the current lineage.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when no font has been interned in the current lineage.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}
```

(`use std::collections::hash_map::Entry;`. `glyph_atlas_key` is unchanged.)

In `mod.rs`, export `FontDbLineage` and `init_resource::<FontDbLineage>()`
in the plugin (main world).

- [ ] **Step 3: Run the gate, commit**

Run both gate lanes. The existing `text_atlas_key.rs` interner tests must
stay green unmodified (in-lineage behavior is identical).

```bash
git add -A
git commit -m "feat(text): T5 task 1 — fontdb ID-semantics pins + lineage-aware FontKeyInterner

fontdb IDs are slotmap keys: stable for survivors within one Database
lineage (into_locale_and_db carries the db — font-assets § 3.2 erratum),
never-aliasing in-lineage, but REISSUED by fresh databases (equal values,
different faces). FontDbLineage names the fresh-db events; the interner
clears its map per lineage with a monotonic seat counter (the len()-as-next
allocation would re-issue seat 0 into grace-resident atlas entries)."
```

---

### Task 2: `TextDirection` + the § 5.4 strong-mark prepend (headless)

Independent of the font work — lands first so the corpus task can use it.

**Files:**
- Create: `crates/buiy_core/src/text/direction.rs`
- Create: `crates/buiy_core/tests/text_direction.rs`
- Modify: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/sync.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_direction.rs`:

```rust
//! Per-node direction — the § 5.4 strong-mark prepend (measure-and-layout
//! § 5.4): LRM/RLM after collapse forces the UAX #9 P2 paragraph level, so
//! base direction drives reordering, the unaligned `start` default, and
//! the `LayoutRun.rtl` flag. Headless: the rtl flag needs no font coverage
//! (bidi levels come from unicode-bidi, not the font).

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, ComputedTextLayout, Text, TextDirection, prepend_strong_marks,
};
use std::borrow::Cow;

// --- the pure pre-pass --------------------------------------------------

#[test]
fn ltr_prepends_lrm_per_non_empty_line() {
    assert_eq!(
        prepend_strong_marks("ab\ncd", TextDirection::Ltr),
        "\u{200E}ab\n\u{200E}cd"
    );
}

#[test]
fn rtl_prepends_rlm_per_non_empty_line() {
    assert_eq!(
        prepend_strong_marks("ab\ncd", TextDirection::Rtl),
        "\u{200F}ab\n\u{200F}cd"
    );
}

#[test]
fn auto_is_borrowed_passthrough() {
    // Auto = cosmic's first-strong default IS CSS dir=auto (§ 5.4); the
    // steady path allocates nothing.
    assert!(matches!(
        prepend_strong_marks("hello", TextDirection::Auto),
        Cow::Borrowed(_)
    ));
}

#[test]
fn empty_lines_stay_unmarked() {
    // Decision 10: a mark on an empty line could shape into a phantom
    // glyph and flip T3's glyphs-keyed ResolvedBaseline for Text("").
    assert_eq!(prepend_strong_marks("", TextDirection::Rtl), "");
    assert_eq!(
        prepend_strong_marks("a\n\nb", TextDirection::Rtl),
        "\u{200F}a\n\n\u{200F}b"
    );
}

// --- end-to-end through TextSync → measure → TextCommit ------------------

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

fn spawn_line(app: &mut App, text: &str, dir: Option<TextDirection>) -> Entity {
    let mut e = app.world_mut().spawn((
        buiy_core::Node,
        Style::default().width_px(300.0).height_px(60.0),
        Text(String::from(text)),
    ));
    if let Some(dir) = dir {
        e.insert(dir);
    }
    e.id()
}

#[test]
fn rtl_component_forces_rtl_on_latin_text() {
    // The crisp § 5.4 effect: pure-LTR content under dir=rtl must resolve
    // an RTL paragraph level — only the prepended RLM can do that (an
    // isolate wrap cannot set P2, the rejected runner-up).
    let mut app = text_app();
    let e = spawn_line(&mut app, "hello", Some(TextDirection::Rtl));
    settle(&mut app);
    let layout = app.world().get::<ComputedTextLayout>(e).unwrap();
    assert!(layout.lines[0].rtl, "RLM forced the paragraph level");
}

#[test]
fn ltr_component_forces_ltr_on_hebrew_text() {
    let mut app = text_app();
    let e = spawn_line(&mut app, "עולם", Some(TextDirection::Ltr));
    settle(&mut app);
    let layout = app.world().get::<ComputedTextLayout>(e).unwrap();
    assert!(!layout.lines[0].rtl, "LRM forced LTR over RTL content");
}

#[test]
fn auto_follows_first_strong() {
    let mut app = text_app();
    let heb = spawn_line(&mut app, "עולם", None);
    let lat = spawn_line(&mut app, "hello", None);
    settle(&mut app);
    assert!(app.world().get::<ComputedTextLayout>(heb).unwrap().lines[0].rtl);
    assert!(!app.world().get::<ComputedTextLayout>(lat).unwrap().lines[0].rtl);
}

#[test]
fn direction_change_retriggers_sync() {
    // TextDirection joins the § 5.1 trigger union: flipping it must reshape
    // (rtl flips) without touching Text.
    let mut app = text_app();
    let e = spawn_line(&mut app, "hello", Some(TextDirection::Rtl));
    settle(&mut app);
    assert!(app.world().get::<ComputedTextLayout>(e).unwrap().lines[0].rtl);
    app.world_mut().entity_mut(e).insert(TextDirection::Ltr);
    settle(&mut app);
    assert!(!app.world().get::<ComputedTextLayout>(e).unwrap().lines[0].rtl);
}
```

Run: `cargo test -p buiy_core --test text_direction` — FAILS (no
`TextDirection`, no `prepend_strong_marks`).

- [ ] **Step 2: Implement**

In `components.rs` (with the other carriers):

```rust
/// CSS `dir` analogue (measure § 5.4, F). Lowered ENTIRELY in the TextSync
/// pre-pass as a strong direction mark prepended per non-empty buffer line
/// AFTER the § 5.2 collapse: UAX #9 P2 finds the mark as the line's first
/// strong character and forces the paragraph level — base direction then
/// drives reordering, the unaligned `start` default, `Align::End`, and
/// `ComputedTextLine.rtl`. Absent component = `Auto` (cosmic's
/// first-strong default IS `dir=auto`). Inline span direction (`<bdi>`,
/// isolates) is the rich-text seam — an isolate wrap can never set P2
/// (the § 5.4 rejected runner-up).
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum TextDirection {
    Ltr,
    Rtl,
    #[default]
    Auto,
}
```

Create `crates/buiy_core/src/text/direction.rs`:

```rust
//! The § 5.4 direction pre-pass: strong-mark prepend (measure-and-layout
//! § 5.4). Runs AFTER `collapse_whitespace` (the trim must see authored
//! edges, never the mark) and BEFORE the resolver/`set_text`.
//!
//! **Editing consequence (successor campaign, § 5.4):** the mark shifts
//! every marked line's byte offsets by its UTF-8 length (3 bytes) —
//! hit-testing and cursor↔source mapping must map through the same
//! pre-pass offset table as the collapse transform.

use std::borrow::Cow;

use super::components::TextDirection;

/// U+200E LEFT-TO-RIGHT MARK / U+200F RIGHT-TO-LEFT MARK.
const LRM: char = '\u{200E}';
const RLM: char = '\u{200F}';

/// Prepend the strong mark per NON-EMPTY line (cosmic treats each buffer
/// line as a UAX #9 paragraph, so P2 runs per line). Empty lines stay
/// unmarked — a shaped mark could grow a phantom glyph and flip the
/// glyphs-keyed `ResolvedBaseline` semantics for empty text (T5 plan
/// decision 10). `Auto` borrows through: the steady path allocates nothing.
pub fn prepend_strong_marks(text: &str, dir: TextDirection) -> Cow<'_, str> {
    let mark = match dir {
        TextDirection::Auto => return Cow::Borrowed(text),
        TextDirection::Ltr => LRM,
        TextDirection::Rtl => RLM,
    };
    let lines = text.split('\n');
    let mut out = String::with_capacity(text.len() + 4 * (text.matches('\n').count() + 1));
    for (i, line) in lines.enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !line.is_empty() {
            out.push(mark);
        }
        out.push_str(line);
    }
    Cow::Owned(out)
}
```

In `sync.rs`:
- `TextSyncTriggers` gains `Changed<TextDirection>` (the ledger comment's T5
  join — update the module-doc ledger: `TextDirection` moves from "joins in
  T5" to listed carrier; the theme font-token swap stays the named future
  member).
- `SyncedText`/`SyncedTextItem` and the `unsynced` query gain
  `Option<&TextDirection>`; `AuthoredStyle` gains
  `direction: TextDirection` (`copied().unwrap_or_default()`).
- `apply_authored` composes the pre-passes:

```rust
let collapsed = collapse_whitespace(&text.0, style.white_space.collapse_mode());
// § 5.4: AFTER collapse (the trim sees authored edges, never the mark).
// Direction joins the intrinsics content version for free — the wholesale
// invalidate_intrinsics() below covers every content change.
let directed = prepend_strong_marks(&collapsed, style.direction);
...
buffer.buffer.set_text(&directed, &style.attrs(), TEXT_SHAPING, None);
```

- Carrier-removal note: like every other carrier (T2 erratum 1),
  `TextDirection` removal resyncs on the next other trigger — documented in
  the module ledger, not special-cased.

In `mod.rs`: `mod direction;`, export `TextDirection` (from components) +
`prepend_strong_marks`, `register_type::<TextDirection>()`.

- [ ] **Step 3: Run the gate, commit**

Both lanes. Also confirm `tests/text_sync.rs` steady-state counts are
untouched (the new trigger member fires nothing on unchanged entities).

```bash
git add -A
git commit -m "feat(text): T5 task 2 — TextDirection + the § 5.4 strong-mark prepend

LRM/RLM per non-empty line, after collapse, before set_text: UAX #9 P2
takes the mark as the first strong char and forces the paragraph level
(an isolate wrap can never set P2 — the spec's rejected runner-up). Auto =
absent = cosmic's first-strong default. Joins the TextSync trigger union;
intrinsics invalidation rides the existing wholesale invalidate."
```

---

### Task 3: `BuiyFont` asset + `BuiyFontLoader` — the sfnt invariant (headless)

**Files:**
- Create: `crates/buiy_core/src/text/font_asset.rs`
- Create: `crates/buiy_core/tests/text_font_asset.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_font_asset.rs`:

```rust
//! The BuiyFont asset + loader (font-assets § 2): ttf/otf/ttc/otc only —
//! fontdb's native set — with the loader-output-is-always-sfnt invariant
//! (the named woff2 seam: adding woff2 later means a decompression
//! pre-pass HERE, touching neither registry nor FontSystem). Headless with
//! AssetPlugin (the atlas_register.rs precedent).

use buiy_core::text::{BuiyFont, BuiyFontLoader, sniff_sfnt};

const EMBEDDED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/FiraSans-Regular-latin.ttf"
);

#[test]
fn extensions_are_fontdbs_native_set() {
    use bevy::asset::AssetLoader;
    assert_eq!(BuiyFontLoader.extensions(), &["ttf", "otf", "ttc", "otc"]);
}

#[test]
fn sfnt_sniff_accepts_the_four_magics_and_rejects_the_rest() {
    // 0x00010000 (TrueType), OTTO (CFF), ttcf (collection), true (legacy).
    assert!(sniff_sfnt(&[0x00, 0x01, 0x00, 0x00, 0, 0]));
    assert!(sniff_sfnt(b"OTTO----"));
    assert!(sniff_sfnt(b"ttcf----"));
    assert!(sniff_sfnt(b"true----"));
    assert!(!sniff_sfnt(b"wOF2----"), "woff2 is the NAMED seam, not sfnt");
    assert!(!sniff_sfnt(b"<svg"));
    assert!(!sniff_sfnt(&[]));
    let real = std::fs::read(EMBEDDED).unwrap();
    assert!(sniff_sfnt(&real));
}

#[test]
fn loader_loads_a_real_ttf_through_the_asset_server() {
    // End-to-end through bevy_asset: the loader registers for the
    // extensions and produces Arc'd bytes (zero-copy into
    // Source::Binary later).
    use bevy::asset::{AssetApp, AssetPlugin, AssetServer, Assets};
    use bevy::prelude::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin {
        // Serve the crate's assets dir so the embedded artifact doubles as
        // the load fixture (no second font committed for this test).
        file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
        ..Default::default()
    });
    app.init_asset::<BuiyFont>()
        .register_asset_loader(BuiyFontLoader);

    let handle: Handle<BuiyFont> = app
        .world()
        .resource::<AssetServer>()
        .load("fonts/FiraSans-Regular-latin.ttf");
    // Drive the async load to completion (bounded poll loop — the
    // condition-based-waiting discipline, no sleeps).
    for _ in 0..200 {
        app.update();
        if app.world().resource::<Assets<BuiyFont>>().get(&handle).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let font = app
        .world()
        .resource::<Assets<BuiyFont>>()
        .get(&handle)
        .expect("loaded within the poll budget");
    assert!(sniff_sfnt(&font.data), "loader output is always sfnt");
    assert_eq!(font.data.len(), std::fs::read(EMBEDDED).unwrap().len());
}
```

Run: FAILS — `BuiyFont`/`BuiyFontLoader`/`sniff_sfnt` do not exist.

- [ ] **Step 2: Implement**

Create `crates/buiy_core/src/text/font_asset.rs`:

```rust
//! The `@font-face` byte source: `BuiyFont` + `BuiyFontLoader`
//! (font-assets § 2). The loader's invariant is
//! **loader-output-is-always-sfnt** — whatever it accepts, the bytes
//! handed to fontdb are sfnt. That invariant IS the named woff2 seam:
//! adding woff2 later means a magic sniff + decompression pre-pass inside
//! `load`, touching neither the registry nor the `FontSystem`. **C (seam
//! named, font-assets § 9.)**

use std::sync::Arc;

use bevy::asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy::reflect::TypePath;

/// Raw sfnt bytes (ttf/otf/ttc/otc). `Arc` so registration hands fontdb a
/// zero-copy `Source::Binary(Arc<dyn AsRef<[u8]> + Send + Sync>)`
/// (font-assets § 2; `Arc<Vec<u8>>` satisfies the bound).
#[derive(Asset, TypePath)]
pub struct BuiyFont {
    /// The validated sfnt bytes.
    pub data: Arc<Vec<u8>>,
}

/// `AssetLoader` for fontdb's native formats (verified: fontdb 0.23 "Will
/// load ttf, otf, ttc and otc fonts"; no WOFF/WOFF2).
#[derive(Default)]
pub struct BuiyFontLoader;

/// Loader failure: IO, or bytes that are not sfnt (the woff2 seam's
/// honest error).
#[derive(Debug, thiserror::Error)]
pub enum BuiyFontLoaderError { /* see note below */ }
```

**Error-type note:** `thiserror` is NOT a buiy_core dependency — do not add
one (charter: no new deps). Hand-write the error enum + `Display` +
`std::error::Error` impls instead (two variants: `Io(std::io::Error)`,
`NotSfnt`; `NotSfnt`'s message names the woff2 seam verbatim: `"not an sfnt
font (ttf/otf/ttc/otc); woff2 needs the font-assets § 9 decompression
seam"`). `type Error` must satisfy `Into<BevyError>` — any
`std::error::Error + Send + Sync + 'static` does.

```rust
impl AssetLoader for BuiyFontLoader {
    type Asset = BuiyFont;
    type Settings = ();
    type Error = BuiyFontLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<BuiyFont, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        if !sniff_sfnt(&bytes) {
            return Err(BuiyFontLoaderError::NotSfnt);
        }
        Ok(BuiyFont { data: Arc::new(bytes) })
    }

    fn extensions(&self) -> &[&str] {
        &["ttf", "otf", "ttc", "otc"]
    }
}

/// The sfnt magic sniff — the loader-output-is-always-sfnt gate: TrueType
/// (0x00010000), CFF (`OTTO`), collection (`ttcf`), legacy Apple TrueType
/// (`true`). Everything else (wOF2 included) is rejected with the seam
/// named in the error.
pub fn sniff_sfnt(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0x00, 0x01, 0x00, 0x00] | b"OTTO" | b"ttcf" | b"true")
    )
}
```

In `mod.rs`: `mod font_asset;` + exports; plugin wiring (decision 13):

```rust
// The asset half is gated: init_asset/register_asset_loader PANIC without
// an AssetServer (bevy_asset lib.rs:590/637), and headless text fixtures
// carry no AssetPlugin. The bytes registration path (T5 registry) needs no
// asset machinery at all.
if app.world().contains_resource::<bevy::asset::AssetServer>() {
    use bevy::asset::AssetApp;
    app.init_asset::<BuiyFont>()
        .register_asset_loader(BuiyFontLoader);
}
```

- [ ] **Step 3: Run the gate, commit**

```bash
git add -A
git commit -m "feat(text): T5 task 3 — BuiyFont asset + BuiyFontLoader (sfnt invariant)

ttf/otf/ttc/otc (fontdb's native set); loader-output-is-always-sfnt — the
magic sniff IS the named woff2 seam (decompression pre-pass later, registry
and FontSystem untouched). Asset machinery gated on AssetServer presence;
Arc'd bytes feed Source::Binary zero-copy."
```

---

### Task 4: `FontRegistry` — register, unregister-rebuild, hot-reload (headless)

**Files:**
- Create: `crates/buiy_core/src/text/registry.rs`
- Create: `crates/buiy_core/tests/text_registry.rs`
- Modify: `crates/buiy_core/src/text/font_system.rs` (use of placeholder)
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_registry.rs`:

```rust
//! FontRegistry (font-assets § 3): the FontFaceSet model — strong handles,
//! explicit register/unregister, in-place add, REBUILD on remove (the
//! unpurgeable font_cache: a fresh FontSystem is the only purge), Modified
//! = remove+re-add. The leak/staleness contract: across N hot-reload
//! cycles, dead IDs never resolve (`get_font` None), the db never grows,
//! and FontsGeneration bumps exactly once per cycle.

use std::sync::Arc;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::LayoutPlugin;
use buiy_core::text::{
    BuiyFont, BuiyTextPlugin, FontDbLineage, FontFaceDescriptors, FontLoadState, FontRegistry,
    FontsGeneration, SharedFontSystem,
};
use cosmic_text::fontdb;

fn fira_bytes() -> Arc<Vec<u8>> {
    Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .unwrap(),
    )
}

/// MinimalPlugins + text, NO AssetPlugin: the bytes path must work
/// asset-machinery-free (decision 13).
fn registry_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn generation(app: &App) -> u64 {
    app.world().resource::<FontsGeneration>().0
}

fn lineage(app: &App) -> u64 {
    app.world().resource::<FontDbLineage>().0
}

fn db_face_count(app: &App) -> usize {
    app.world().resource::<SharedFontSystem>().lock().db().len()
}

#[test]
fn register_bytes_adds_faces_in_place_and_bumps_generation_once() {
    let mut app = registry_app();
    app.update(); // settle plugin init (generation is_added frame)
    let gen0 = generation(&app);
    let faces0 = db_face_count(&app);

    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();

    assert_eq!(generation(&app), gen0 + 1, "exactly one bump per batch");
    assert_eq!(lineage(&app), 0, "in-place add never bumps lineage");
    assert_eq!(db_face_count(&app), faces0 + 1);
    let registry = app.world().resource::<FontRegistry>();
    assert_eq!(registry.load_state("Fira Sans"), Some(FontLoadState::Loaded));
    assert_eq!(registry.faces("Fira Sans").len(), 1);
}

#[test]
fn unregister_rebuilds_no_stale_get_font_no_growth() {
    // The § 3.1 contract: after unregister, the dead ID must not resolve
    // (the rebuild swapped in a FRESH FontSystem — fresh font_cache), the
    // db face count returns to baseline, and surviving faces still resolve.
    let mut app = registry_app();
    app.update();
    let faces0 = db_face_count(&app);
    let surviving = app
        .world()
        .resource::<SharedFontSystem>()
        .lock()
        .db()
        .faces()
        .next()
        .unwrap()
        .id;

    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();
    let dead = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    // Warm the cache so staleness is a real assertion, not vacuous.
    assert!(
        app.world()
            .resource::<SharedFontSystem>()
            .lock()
            .get_font(dead, fontdb::Weight(400))
            .is_some()
    );

    let gen_before = generation(&app);
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .unregister_family("Fira Sans");
    app.update();

    assert_eq!(generation(&app), gen_before + 1);
    assert_eq!(lineage(&app), 0, "into_locale_and_db carries the db — in-lineage");
    assert_eq!(db_face_count(&app), faces0, "no growth");
    let fonts = app.world().resource::<SharedFontSystem>();
    let mut guard = fonts.lock();
    assert!(
        guard.get_font(dead, fontdb::Weight(400)).is_none(),
        "no stale get_font hit — the rebuilt FontSystem has a fresh font_cache"
    );
    assert!(
        guard.get_font(surviving, fontdb::Weight(400)).is_some(),
        "surviving IDs stayed valid (Orientation fact 1)"
    );
    drop(guard);
    assert!(app.world().resource::<FontRegistry>().load_state("Fira Sans").is_none());
}

#[test]
fn hot_reload_cycles_leak_nothing_and_stay_fresh() {
    // Modified = remove+re-add under ONE lock hold + ONE bump (font-assets
    // § 2). N cycles via the bytes path's re-register (same composed
    // mechanics; the AssetEvent::Modified arm is exercised in the
    // asset-driven test below).
    let mut app = registry_app();
    app.update();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();
    let faces_registered = db_face_count(&app);

    let mut dead_ids = Vec::new();
    for _cycle in 0..8 {
        let old = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
        let gen_before = generation(&app);
        app.world_mut()
            .resource_mut::<FontRegistry>()
            .reregister_bytes("Fira Sans", fira_bytes());
        app.update();
        assert_eq!(generation(&app), gen_before + 1, "one bump per cycle");
        assert_eq!(db_face_count(&app), faces_registered, "no growth across cycles");
        let new = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
        assert_ne!(new, old, "re-add issues a fresh ID");
        dead_ids.push(old);
    }
    let fonts = app.world().resource::<SharedFontSystem>();
    let mut guard = fonts.lock();
    for dead in dead_ids {
        assert!(
            guard.get_font(dead, fontdb::Weight(400)).is_none(),
            "every prior cycle's ID is dead — no staleness, no leak"
        );
    }
}

#[test]
fn asset_registration_loading_to_loaded_with_strong_pinning() {
    // The asset path: register against a not-yet-loaded handle → Loading;
    // asset arrives → Loaded + one bump. The registry's strong handle pins
    // the asset even when the caller drops theirs.
    use bevy::asset::AssetPlugin;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();

    let handle = app
        .world()
        .resource::<bevy::asset::Assets<BuiyFont>>()
        .reserve_handle();
    app.world_mut().resource_mut::<FontRegistry>().register_asset(
        "Fira Sans",
        handle.clone(),
        FontFaceDescriptors::default(),
    );
    app.update();
    assert_eq!(
        app.world().resource::<FontRegistry>().load_state("Fira Sans"),
        Some(FontLoadState::Loading)
    );

    let gen_before = generation(&app);
    let id = handle.id();
    app.world_mut()
        .resource_mut::<bevy::asset::Assets<BuiyFont>>()
        .insert(id, BuiyFont { data: fira_bytes() });
    drop(handle); // caller drops; the registry's strong handle must pin
    // Asset events land next frame; settle two.
    app.update();
    app.update();

    let registry = app.world().resource::<FontRegistry>();
    assert_eq!(registry.load_state("Fira Sans"), Some(FontLoadState::Loaded));
    assert_eq!(generation(&app), gen_before + 1);
    assert!(
        app.world()
            .resource::<bevy::asset::Assets<BuiyFont>>()
            .get(id)
            .is_some(),
        "strong registry handle pins the asset (the weak-registry footgun, § 3)"
    );
}

#[test]
fn asset_modified_is_remove_plus_readd() {
    use bevy::asset::AssetPlugin;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();

    let id = {
        let mut assets = app.world_mut().resource_mut::<bevy::asset::Assets<BuiyFont>>();
        assets.add(BuiyFont { data: fira_bytes() }).id()
    };
    let handle = bevy::asset::Handle::Weak(id); // registry will hold strong via record? see note
    let _ = handle; // — register via the typed API instead:
    let strong = app
        .world()
        .resource::<bevy::asset::Assets<BuiyFont>>()
        .get_strong_handle(id)
        .unwrap();
    app.world_mut().resource_mut::<FontRegistry>().register_asset(
        "Fira Sans",
        strong,
        FontFaceDescriptors::default(),
    );
    app.update();
    app.update();
    let old = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];

    // Hot-reload: mutate the asset → AssetEvent::Modified.
    let gen_before = generation(&app);
    app.world_mut()
        .resource_mut::<bevy::asset::Assets<BuiyFont>>()
        .get_mut(id)
        .unwrap()
        .data = fira_bytes();
    app.update();
    app.update();

    let new = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    assert_ne!(new, old, "Modified = remove + re-add (fresh ID)");
    assert_eq!(generation(&app), gen_before + 1, "composed under one bump");
    let fonts = app.world().resource::<SharedFontSystem>();
    assert!(fonts.lock().get_font(old, fontdb::Weight(400)).is_none());
}
```

Run: FAILS — the registry does not exist. (Verify `reserve_handle` /
`get_strong_handle` exist on `Assets<A>` in 0.18 before relying on them —
both are present in bevy_asset 0.18.1's `Assets` impl; if `get_strong_handle`
moved, fall back to keeping the original strong handle in the test.)

- [ ] **Step 2: Implement**

Create `crates/buiy_core/src/text/registry.rs`. The shape (REAL code —
trim/adjust mechanically while implementing):

```rust
//! `FontRegistry` — the `FontFaceSet` analogue (font-assets § 3): strong
//! handles, declared family names, explicit register/unregister. Face
//! ADDITION is in-place (`db_mut().load_font_source` — safe: db_mut clears
//! only font_matches_cache, and a new face can never make a match-cache
//! entry stale). Face REMOVAL rebuilds the FontSystem via
//! `into_locale_and_db` (§ 3.1): the font_cache has no purge API — after an
//! in-place remove it would leak the Arc<Font> forever AND serve the dead
//! face from get_font. The rebuild carries the SAME Database, so surviving
//! IDs stay valid (in-lineage — FontDbLineage untouched; T5 plan
//! Orientation fact 1).
//!
//! Registry methods STAGE ops; `apply_font_registry` (one system, before
//! BuiySet::Layout) applies them + the AssetEvent stream under ONE lazy
//! lock hold and ONE FontsGeneration bump per batch — a frame never
//! measures against a half-registered family (§ 3). This is a RARE-EVENT
//! lock site (architecture § 1.2's table is steady-frame scoped — T5
//! erratum 2).

use std::ops::RangeInclusive;
use std::sync::Arc;

use bevy::asset::{AssetEvent, AssetId, Assets, Handle};
use bevy::ecs::message::MessageReader;
use bevy::platform::collections::HashMap; // or std HashMap — match crate convention
use bevy::prelude::*;
use cosmic_text::fontdb;

use super::font_asset::BuiyFont;
use super::font_system::{
    BuiyFallback, FontsGeneration, SharedFontSystem, placeholder_font_system,
};
use super::match_index::FontMatchIndex;

/// `font-display` (font-assets § 7): v1 implements Swap (default) + Block;
/// Fallback/Optional parse and degrade to Swap with a warn-once (C-tier
/// reserved — the descriptor shape is the spec's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontDisplay {
    Block,
    #[default]
    Swap,
    Fallback,
    Optional,
}

/// Declared `unicode-range` (font-assets § 6.1): a per-codepoint face
/// filter enforced by the resolver (fontdb has no range concept —
/// verified). Programmatic ranges only; the CSS string syntax is
/// styling-tier (named seam).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnicodeRanges(Vec<RangeInclusive<u32>>);

impl UnicodeRanges {
    pub fn new(ranges: Vec<RangeInclusive<u32>>) -> Self {
        Self(ranges)
    }
    pub fn contains(&self, c: char) -> bool {
        let cp = c as u32;
        self.0.iter().any(|r| r.contains(&cp))
    }
}

/// Registration descriptors (font-assets § 3). Families with NO declared
/// range skip the resolver's range check entirely (§ 6.1's cost gate).
#[derive(Clone, Debug, Default)]
pub struct FontFaceDescriptors {
    pub unicode_range: Option<UnicodeRanges>,
    pub font_display: FontDisplay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontLoadState {
    Loading,
    Loaded,
    Failed,
}

/// How the bytes entered: Bevy asset (STRONG handle — pins the asset for
/// the registration's lifetime, the anti-silent-refallback decision § 3)
/// or the include_bytes! escape hatch (font-assets § 2's
/// register_font_bytes, method form).
#[derive(Clone)]
enum RegisteredSource {
    Asset(Handle<BuiyFont>),
    Bytes,
}

struct FamilyRecord {
    /// fontdb face IDs, valid for the CURRENT lineage. Re-recorded by the
    /// system-scan swap (fresh db = every ID reissued).
    faces: Vec<fontdb::ID>,
    descriptors: FontFaceDescriptors,
    load_state: FontLoadState,
    source: RegisteredSource,
    /// Loaded bytes (Arc clone of BuiyFont.data / the bytes argument):
    /// the scan-swap re-add source. None until loaded.
    data: Option<Arc<Vec<u8>>>,
    /// Time::elapsed_secs_f64 at registration — the Block deadline base
    /// (CSS block period starts at load start).
    loading_since: f64,
}

enum RegistryOp {
    RegisterAsset { family: String, handle: Handle<BuiyFont>, descriptors: FontFaceDescriptors },
    RegisterBytes { family: String, bytes: Arc<Vec<u8>>, descriptors: FontFaceDescriptors },
    /// Hot-reload via the bytes path (tests; advanced apps).
    ReregisterBytes { family: String, bytes: Arc<Vec<u8>> },
    Unregister { family: String },
}

#[derive(Resource, Default)]
pub struct FontRegistry {
    families: HashMap<String, FamilyRecord>,
    ops: Vec<RegistryOp>,
}

impl FontRegistry {
    pub fn register_asset(&mut self, family: impl Into<String>, handle: Handle<BuiyFont>, descriptors: FontFaceDescriptors) { /* push op */ }
    pub fn register_bytes(&mut self, family: impl Into<String>, bytes: Arc<Vec<u8>>, descriptors: FontFaceDescriptors) { /* push op */ }
    pub fn reregister_bytes(&mut self, family: impl Into<String>, bytes: Arc<Vec<u8>>) { /* push op */ }
    pub fn unregister_family(&mut self, family: impl Into<String>) { /* push op */ }

    pub fn load_state(&self, family: &str) -> Option<FontLoadState> { /* … */ }
    pub fn faces(&self, family: &str) -> &[fontdb::ID] { /* … (empty slice when absent) */ }
    pub fn descriptors(&self, family: &str) -> Option<&FontFaceDescriptors> { /* … */ }
    /// Block deadline for a Loading family (resolver + expiry consume).
    pub fn block_deadline(&self, family: &str) -> Option<f64> { /* loading_since + FONT_BLOCK_TIMEOUT_SECS */ }
    /// Iterate loaded (family, bytes) pairs — the scan-swap re-add source.
    pub(crate) fn loaded_sources(&self) -> impl Iterator<Item = (&str, &Arc<Vec<u8>>)> { /* … */ }
    /// Re-record a family's faces after a fresh-db swap (system_scan).
    pub(crate) fn record_faces(&mut self, family: &str, faces: Vec<fontdb::ID>) { /* … */ }
}

/// CSS `font-display` block period (web default 3 s). Configurability is a
/// named seam (T5 plan honesty pins), not a resource knob.
pub const FONT_BLOCK_TIMEOUT_SECS: f64 = 3.0;

/// `font-display: block`'s paint-side marker (Task 7 consumes): inserted by
/// TextSync while a blocking family is loading inside its window; the
/// producer emits the entity's glyphs with zero alpha (identical fallback
/// LAYOUT, invisible paint — § 7). `until` = loading_since + 3 s.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct PendingFontBlock {
    pub until: f64,
}
```

`apply_font_registry` — the one applier:

```rust
/// Apply staged registry ops + AssetEvent<BuiyFont> messages: in-place adds,
/// § 3.1 rebuild-removals (Modified = remove+re-add composed; Removed =
/// forced unregister), then ONE FontMatchIndex re-snapshot + ONE
/// FontsGeneration bump if anything changed. Lock taken LAZILY once per
/// batch (the text_commit guard pattern); zero steady-state cost.
#[allow(clippy::too_many_arguments)]
pub fn apply_font_registry(
    mut registry: ResMut<FontRegistry>,
    fonts: Res<SharedFontSystem>,
    mut index: ResMut<FontMatchIndex>,
    mut generation: ResMut<FontsGeneration>,
    assets: Option<Res<Assets<BuiyFont>>>,
    mut events: MessageReader<AssetEvent<BuiyFont>>,
    time: Res<Time>,
) {
    // 1. Fold AssetEvents into ops: Added/LoadedWithDependencies for a
    //    pending Asset record → complete-load; Modified → remove+re-add
    //    with the asset's current bytes; Removed/Unused-of-registered →
    //    forced unregister (warn — the deliberate-unload § 3 arm).
    //    Match records by `handle.id() == *id`.
    // 2. Drain ops. Removals collect dead IDs; ONE rebuild covers them all:
    //       let mut guard = fonts.lock();
    //       let old = mem::replace(&mut *guard, placeholder_font_system());
    //       let (locale, mut db) = old.into_locale_and_db();
    //       for id in &dead { db.remove_face(*id); }
    //       *guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
    //    Additions: guard.db_mut().load_font_source(Source::Binary(arc))
    //    → record IDs; validate the declared family against the loaded
    //    faces' internal names (FaceInfo.families) — mismatch warns loudly
    //    once per family (decision 4; the § 9 alias seam is the fix);
    //    flip load_state; stash the Arc for the scan re-add.
    // 3. If anything changed: index.reset(guard.db().clone());
    //    generation.0 += 1;  // exactly once per batch
    //    (FontDbLineage is NOT touched — in-lineage by construction.)
}
```

Ordering in `mod.rs` (plugin):

```rust
app.init_resource::<FontRegistry>();
app.insert_resource(FontMatchIndex::new(fonts.lock().db().clone())); // Task 5 creates the type
app.add_message::<bevy::asset::AssetEvent<BuiyFont>>(); // idempotent; reader works without AssetPlugin
app.add_systems(
    Update,
    apply_font_registry
        .after(apply_system_font_scan)
        .before(crate::BuiySet::Layout),
);
```

(If Task 5 hasn't landed yet, stub `FontMatchIndex` as an empty resource
with `new`/`reset` no-ops so this task compiles standalone, OR reorder the
two `insert_resource` lines into Task 5 — implementer's choice; the tests
here don't touch the index. Keep the seam obvious either way.)

- [ ] **Step 3: Run the gate, commit**

```bash
git add -A
git commit -m "feat(text): T5 task 4 — FontRegistry: strong handles, in-place add, rebuild-on-remove

The FontFaceSet model (font-assets § 3): declared family names, staged ops,
one applier system before Layout (rare-event lock site — erratum 2), one
FontsGeneration bump per batch. Removal rebuilds via into_locale_and_db
(unpurgeable font_cache: fresh FontSystem is the only purge) — surviving
IDs stable, dead IDs never resolve, N hot-reload cycles leak nothing.
Modified = remove+re-add composed; Removed = forced unregister."
```

---

### Task 5: `FontMatchIndex` + the scan-swap integration + producer probes (headless)

**Files:**
- Create: `crates/buiy_core/src/text/match_index.rs`
- Create: `crates/buiy_core/tests/text_resolver.rs` (index half; Task 6 grows it)
- Modify: `crates/buiy_core/src/text/system_scan.rs`
- Modify: `crates/buiy_core/src/text/extract.rs`
- Modify: `crates/buiy_core/tests/text_extract.rs`
- Modify: `crates/buiy_core/tests/text_system_scan.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

In a new `crates/buiy_core/tests/text_resolver.rs` (index section):

```rust
//! FontMatchIndex: the lock-free resolver substrate (T5 plan decision 2) —
//! a fontdb::Database CLONE (same-lineage IDs valid for the live engine)
//! plus lazily extracted per-face coverage (skrifa charmap via
//! with_face_data; Font::unicode_codepoints is feature-gated EMPTY under
//! the default-features pin — Orientation).

use buiy_core::text::{FontMatchIndex, registered_fonts_db};
use cosmic_text::fontdb::{self, Family, Query};

#[test]
fn query_on_the_snapshot_is_fontdbs_real_matcher() {
    let db = registered_fonts_db();
    let expected = db.faces().next().unwrap().id;
    let index = FontMatchIndex::new(db);
    let hit = index.query(&Query {
        families: &[Family::Name("Fira Sans")],
        weight: fontdb::Weight(400),
        ..Default::default()
    });
    assert_eq!(hit, Some(expected));
    assert_eq!(
        index.query(&Query {
            families: &[Family::SansSerif],
            ..Default::default()
        }),
        Some(expected),
        "generic pins resolve through the snapshot too"
    );
    assert_eq!(
        index.query(&Query {
            families: &[Family::Name("No Such Family")],
            ..Default::default()
        }),
        None
    );
}

#[test]
fn coverage_is_lazily_extracted_and_cached() {
    let db = registered_fonts_db();
    let face = db.faces().next().unwrap().id;
    let mut index = FontMatchIndex::new(db);
    // The latin subset covers ASCII…
    assert!(index.covers(face, 'A'));
    assert!(index.covers(face, 'é'), "U+00E9 is in the latin-1 range");
    // …and not Hebrew/Arabic/CJK.
    assert!(!index.covers(face, 'ע'));
    assert!(!index.covers(face, 'م'));
    assert!(!index.covers(face, '你'));
    // Second probe = pure cache hit (no observable side effect to assert
    // beyond not panicking; the laziness contract is the with_face_data
    // call count, which has no public counter — documented, not asserted).
    assert!(index.covers(face, 'A'));
}

#[test]
fn reset_prunes_dead_coverage_and_swaps_the_snapshot() {
    let mut db = registered_fonts_db();
    let face = db.faces().next().unwrap().id;
    let mut index = FontMatchIndex::new(db.clone());
    assert!(index.covers(face, 'A'));
    db.remove_face(face);
    index.reset(db);
    assert!(!index.covers(face, 'A'), "dead ID: no face, no coverage");
    assert_eq!(
        index.query(&Query {
            families: &[Family::Name("Fira Sans")],
            ..Default::default()
        }),
        None
    );
}
```

In `crates/buiy_core/tests/text_system_scan.rs`, add (extending the existing
injected-scan test fixture):

```rust
#[test]
fn scan_swap_readds_registry_sources_rerecords_ids_and_bumps_lineage() {
    // font-assets § 5: the swap re-adds every registered Source::Binary —
    // on the MAIN THREAD at apply time (T5 plan decision 6: an in-task
    // re-add would lose fonts registered DURING the scan). Fresh db ⇒
    // FontDbLineage advances together with FontsGeneration, and the
    // registry's recorded face IDs are re-recorded against the new db.
    let mut app = scan_app(); // the file's existing fixture
    app.update();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();
    let old_face = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    let gen_before = app.world().resource::<FontsGeneration>().0;
    let lineage_before = app.world().resource::<FontDbLineage>().0;

    inject_completed_scan(&mut app); // existing helper: PendingSystemFontScan with a ready task
    app.update();

    assert_eq!(app.world().resource::<FontsGeneration>().0, gen_before + 1);
    assert_eq!(
        app.world().resource::<FontDbLineage>().0,
        lineage_before + 1,
        "fresh db = lineage advance (every fontdb ID reissued)"
    );
    let new_face = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    // Equal VALUES are possible across lineages (Orientation fact 1) — the
    // real assertion is liveness against the swapped db:
    let fonts = app.world().resource::<SharedFontSystem>();
    assert!(fonts.lock().db().face(new_face).is_some(), "re-recorded ID is live");
    let _ = old_face; // may or may not equal new_face by value — see above
    // And the registered family still RESOLVES post-swap:
    assert!(
        fonts
            .lock()
            .db()
            .faces()
            .any(|f| f.families.iter().any(|(n, _)| n == "Fira Sans")),
        "registered face survived the swap into the fresh db"
    );
}
```

In `crates/buiy_core/tests/text_extract.rs`, add (through
`TextExtractHarness`):

```rust
#[test]
fn generation_bump_rebuilds_and_lineage_bump_reseats_the_interner() {
    // § 6.2 grows two value-compare probes: FontsGeneration (rebuild) and
    // FontDbLineage (interner clear + monotonic reseat). A steady frame
    // after the storm settles back to zero-change.
    let mut harness = TextExtractHarness::new();
    // …spawn a text entity, settle, record ExtractedGlyphs + interner len…
    let seat_count_before = harness.render.resource::<FontKeyInterner>().len();
    assert!(seat_count_before > 0);

    // Simulate the swap's main-world face: bump both counters (the
    // apply_system_font_scan contract — every lineage bump rides a
    // generation bump).
    harness.app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    harness.app.world_mut().resource_mut::<FontDbLineage>().0 += 1;
    harness.step();

    let glyphs_changed = /* GlyphChangeLog on this frame */;
    assert!(glyphs_changed, "generation probe forced a rebuild");
    // The interner cleared and re-seated MONOTONICALLY: same face count,
    // but the keys embed fresh u32 seats — assert via ResidentTextKeys:
    // every key differs from the pre-bump set (the font-u32 bytes moved).
    // …
    harness.step();
    // Steady again: no rebuild on the very next frame.
}
```

(Concretize against the harness's existing accessors while implementing;
the assertions that matter: dirty-on-generation-change, keys-differ-on-
lineage-change, steady-after.)

Run: all three FAIL.

- [ ] **Step 2: Implement `FontMatchIndex`**

Create `crates/buiy_core/src/text/match_index.rs`:

```rust
//! The resolver's lock-free substrate (T5 plan decision 2): a
//! `fontdb::Database` CLONE snapshot + lazily extracted per-face coverage.
//! Same-lineage slotmap keys are identical across clones, so snapshot IDs
//! are valid against the live engine until the next index reset (every
//! engine mutation site resets the snapshot under its own lock hold).
//!
//! Coverage comes from the face's cmap via the RE-EXPORTED skrifa
//! (`cosmic_text::skrifa`) through `Database::with_face_data` (&self —
//! handles Binary/SharedFile/File sources): `Font::unicode_codepoints()`
//! is gated behind the non-default `monospace_fallback` feature (returns
//! `&[]` under Buiy's default-features pin — T5 erratum 5), and
//! `Font::new` rejects `Source::File`. No new dependency.

use bevy::prelude::*;
use cosmic_text::fontdb;
use cosmic_text::skrifa::{prelude::*, raw::FontRef};
use std::collections::HashMap;

/// Sorted, deduped codepoint set — binary-search membership.
struct CoverageSet(Vec<u32>);

impl CoverageSet {
    fn contains(&self, c: char) -> bool {
        self.0.binary_search(&(c as u32)).is_ok()
    }
}

#[derive(Resource)]
pub struct FontMatchIndex {
    db: fontdb::Database,
    coverage: HashMap<fontdb::ID, CoverageSet>,
}

impl FontMatchIndex {
    pub fn new(db: fontdb::Database) -> Self {
        Self { db, coverage: HashMap::new() }
    }

    /// Re-snapshot after an engine mutation: swap the db clone in, prune
    /// coverage of dead IDs (in-lineage resets keep survivors' sets — they
    /// are content-addressed by face), drop everything on a fresh-lineage
    /// reset for free (every old ID is dead against the new db).
    pub fn reset(&mut self, db: fontdb::Database) {
        self.coverage.retain(|id, _| db.face(*id).is_some());
        self.db = db;
    }

    /// fontdb's real CSS matcher, on the snapshot.
    pub fn query(&self, query: &fontdb::Query) -> Option<fontdb::ID> {
        self.db.query(query)
    }

    /// Does `id` cover `c`? Extracts the face's cmap on first probe
    /// (with_face_data + skrifa charmap), cached for the face's lifetime
    /// in this index. A face that fails to parse covers nothing.
    pub fn covers(&mut self, id: fontdb::ID, c: char) -> bool {
        if !self.coverage.contains_key(&id) {
            let set = self
                .db
                .with_face_data(id, |data, face_index| {
                    let font = FontRef::from_index(data, face_index).ok()?;
                    let mut cps: Vec<u32> =
                        font.charmap().mappings().map(|(cp, _)| cp).collect();
                    cps.sort_unstable();
                    cps.dedup();
                    Some(cps)
                })
                .flatten()
                .unwrap_or_default();
            self.coverage.insert(id, CoverageSet(set));
        }
        self.coverage[&id].contains(c)
    }
}
```

(Verify the exact skrifa charmap call shape against the vendored
`cosmic-text-0.19.0/src/font/mod.rs:182–187` — `charmap.mappings()` yielding
`(code_point, glyph_id)` is the upstream's own usage. If `prelude::*` does
not bring `MetadataProvider` in this skrifa version, import it explicitly.)

- [ ] **Step 3: Implement the scan-swap integration**

In `system_scan.rs`:

```rust
/// The font-assets § 3.1/§ 5 swap, extended (T5): under ONE lock hold —
/// park the placeholder, drop the old FontSystem (its db and font_cache go
/// with it), install the fresh db, then RE-ADD every loaded registry
/// source on this thread and re-record the fresh face IDs (decision 6: an
/// in-task re-add would lose fonts registered DURING the scan). Returns a
/// db clone for the FontMatchIndex reset. Every fontdb ID is reissued by
/// the FRESH db — the caller must bump FontsGeneration AND FontDbLineage
/// together (the producer's two probes).
pub fn swap_font_db(
    fonts: &SharedFontSystem,
    db: fontdb::Database,
    registry: &mut FontRegistry,
) -> fontdb::Database {
    let mut guard = fonts.lock();
    let old = mem::replace(&mut *guard, placeholder_font_system());
    let (locale, _discarded_db) = old.into_locale_and_db();
    *guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
    let readds: Vec<(String, Arc<Vec<u8>>)> = registry
        .loaded_sources()
        .map(|(family, bytes)| (family.to_owned(), bytes.clone()))
        .collect();
    for (family, bytes) in readds {
        let ids = guard
            .db_mut()
            .load_font_source(fontdb::Source::Binary(bytes));
        registry.record_faces(&family, ids.to_vec());
    }
    guard.db().clone()
}
```

`apply_system_font_scan` gains `registry: ResMut<FontRegistry>`,
`index: ResMut<FontMatchIndex>`, `lineage: ResMut<FontDbLineage>`:

```rust
let snapshot = swap_font_db(&fonts, db, &mut registry);
index.reset(snapshot);
generation.0 += 1; // exactly one bump per completed scan
lineage.0 += 1;    // fresh db: every fontdb ID reissued (§ 3.2)
```

Also update `spawn_system_font_scan`'s doc (the "T5's FontRegistry adds
every registered Source::Binary" ledger comment now describes the apply-time
re-add) — the task body stays `registered_fonts_db()` + `load_system_fonts()`.

Existing `swap_font_db` callers/tests: update signatures (the function is
pub; `text_system_scan.rs` and any direct users pass the registry now).

- [ ] **Step 4: Implement the producer probes**

In `extract.rs`:
- `ResidentTextKeys` gains `pub last_generation: Option<u64>`.
- `extract_buiy_glyphs` gains
  `generation: Extract<Res<FontsGeneration>>` and
  `lineage: Extract<Res<FontDbLineage>>` (the `theme` extraction precedent);
  the dirty fold gains
  `let fonts_changed = resident.last_generation != Some(generation.0);`
  (value compare — the § 6.2 idiom, never `is_changed` across the
  extract boundary); on rebuild, `resident.last_generation = Some(generation.0);`
  and **first thing in the rebuild path**:
  `interner.begin_lineage(lineage.0);` (the § 3.2 storm: keys re-seat
  monotonically; old entries grace-evict; `GlyphMetaCache` prunes via the
  existing residency `retain`).
- Update the § 6.2 ledger comment: the generation/lineage probes are now
  members; `ExtractedTextQuads` (T6) and the caret/selection members (T7)
  remain the named future joins.

- [ ] **Step 5: Run the gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(text): T5 task 5 — FontMatchIndex snapshot + scan-swap registry re-add + producer probes

Lock-free resolver substrate: fontdb::Database clone (same-lineage IDs
valid) + lazy per-face coverage via with_face_data + re-exported skrifa
charmap (Font::unicode_codepoints is empty under default features —
erratum 5). Scan swap re-adds registry sources main-thread under the swap
lock (in-task re-add loses mid-scan registrations), re-records IDs, bumps
FontsGeneration + FontDbLineage together; the producer rebuilds on the
generation value-compare and reseats the interner per lineage."
```

---

### Task 6: The `FontStack` resolver — Query matching, coverage splits, unicode-range (headless)

**Files:**
- Create: `crates/buiy_core/src/text/resolver.rs`
- Modify: `crates/buiy_core/tests/text_resolver.rs`
- Modify: `crates/buiy_core/src/text/sync.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

Grow `crates/buiy_core/tests/text_resolver.rs`. Pure-resolver tests first
(no app — registry + index constructed directly), then end-to-end:

```rust
// --- the resolver walk (decision 7) ---------------------------------------

use buiy_core::text::{
    FamilyEntry, FontFaceDescriptors, FontRegistry, FontStack, GenericFamily, ResolvedFamily,
    UnicodeRanges, resolve_spans,
};

fn latin_index_and_registry_with(extra: &[(&str, &[u8])]) -> (FontMatchIndex, FontRegistry) {
    // registered_fonts_db + register_bytes-equivalent records for `extra`
    // (drive FontRegistry's public API through a minimal App, or expose a
    // test-only constructor — PREFER the App route: production code gets
    // no test-only methods, testing-anti-patterns).
    /* … shared fixture builder using registry_app() from text_registry.rs … */
}

#[test]
fn named_entry_wins_for_covered_codepoints() {
    // Stack ["Fira Sans", sans-serif] over pure ASCII: one span, Named.
    /* spans == [(0..len, Named("Fira Sans"))] */
}

#[test]
fn generic_entry_is_terminal() {
    // Stack [sans-serif] over ANY content: one span, Generic — no coverage
    // probe, no split (the generic is the author's catch-all; per-glyph
    // gaps are FontFallbackIter's job below the stack).
}

#[test]
fn coverage_miss_splits_spans() {
    // Stack ["Hebrew Fixture", "Fira Sans"] over "abc עברית xyz" (Hebrew
    // fixture covers Hebrew only): Latin → Fira (second entry), Hebrew →
    // the fixture (first entry) — THREE spans, adjacent same-winner merged.
    // NOTE: runs after Task 8 lands the fixture fonts; until then use a
    // two-face latin db split via unicode_range instead (below) — keep this
    // test #[ignore = "needs Task 8 fixture fonts"] and flip it in Task 8.
}

#[test]
fn unicode_range_filters_per_codepoint() {
    // "Fira Sans" registered with unicode_range = U+0041..=U+005A
    // (uppercase only); stack ["Fira Sans", sans-serif] over "aAbB":
    // lowercase → Generic(sans-serif), uppercase → Named — 4 spans merged
    // to 4 alternating… exactly the § 6.1 per-character CSS semantics via
    // the same span machinery.
}

#[test]
fn common_and_inherited_never_split() {
    // Spaces, digits punctuation (Common) and combining marks (Inherited)
    // join the current span: "ab cd!" resolves as ONE span; the prepended
    // LRM/RLM never fragments either.
}

#[test]
fn stack_missed_falls_to_first_entry() {
    // No entry covers '∰' (math op, not in the latin subset): the span
    // resolves to the FIRST entry and shaping's FontFallbackIter takes
    // over per-glyph (BuiyFallback's deterministic lists) — asserted here
    // only as span assignment; the shaping side is the corpus's job.
}

// --- end-to-end: set_rich_text through TextSync ---------------------------

#[test]
fn registered_family_wins_over_generic_end_to_end() {
    // The font-assets § 10 round-trip: register a second face under its
    // own family, stack [that family, sans-serif], settle, then assert
    // every LayoutGlyph.font_id == the registered face's ID (the committed
    // glyph carries its face — layout.rs:43).
    // Until Task 8, "second face" = the embedded Fira bytes re-registered
    // under their real family name — assert font_id == the REGISTERED
    // record's face id (not the embedded baseline's).
    /* …text_app + registry + spawn + settle + inspect
       world.get::<TextBuffer>(e).buffer.layout_runs() glyphs… */
}

#[test]
fn single_span_resolution_uses_set_text_path() {
    // Behavioral pin via TextSyncAppliedCount + ComputedTextLayout
    // idempotency: a plain ASCII entity resolves to one span and the sync
    // path stays the T2 shape (no observable difference — this test pins
    // steady-state counts unchanged from T4's baselines).
}
```

Run: FAILS — `resolve_spans`/`ResolvedFamily` do not exist.

- [ ] **Step 2: Implement the resolver**

Create `crates/buiy_core/src/text/resolver.rs`:

```rust
//! The Buiy-owned font-family stack resolver (font-assets § 6): per text
//! run, BEFORE Attrs construction, entirely lock-free (decision 2 —
//! FontRegistry + FontMatchIndex snapshot only). Two verified API facts
//! force Buiy ownership: `Attrs.family` is a SINGLE `Family` and the
//! `Fallback` trait is constructor-injected + 'static — per-node stacks
//! cannot live inside cosmic-text. Below the stack, cosmic-text's
//! per-glyph `FontFallbackIter` (engine-internal — shape.rs:307/489/985;
//! public at cosmic_text::fallback but never constructed by Buiy) remains
//! the last-resort safety net: it runs only when the resolved face misses
//! a glyph, i.e. when the author's entire stack missed (T5 erratum 4).

use std::ops::Range;

use unicode_script::{Script, UnicodeScript};

use super::components::{FamilyEntry, FontStack, GenericFamily};
use super::match_index::FontMatchIndex;
use super::registry::{FontDisplay, FontLoadState, FontRegistry};

/// One resolved span's family target. Named carries an OWNED String
/// (resolution only runs on damage-gated syncs; interning is a named perf
/// seam — T5 plan decision 8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResolvedFamily {
    Named(String),
    Generic(GenericFamily),
}

/// A resolved byte-range of the (collapsed + direction-marked) string.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedSpan {
    pub range: Range<usize>,
    pub family: ResolvedFamily,
}

/// Resolution output: spans in source order + the font-display Block flag
/// (entity-level v1 — decision 9).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Resolution {
    pub spans: Vec<ResolvedSpan>,
    pub blocked: bool,
}

/// Walk the stack per codepoint (decision 7):
/// - `Common`/`Inherited`/`Unknown` script chars never force a boundary
///   (they join the current span; leading ones attach to the first
///   resolved span) — the HarfBuzz itemization rule, and what keeps the
///   § 5.4 marks + ZWJ/VS16 from fragmenting spans.
/// - Generic entries are TERMINAL (the deterministic § 4 pins resolve
///   them; no coverage probe).
/// - Named entries: registry gate (Loading → skip [Block marking inside
///   its window], Failed → skip, declared unicode-range filter), then
///   fontdb Query by name+weight on the snapshot, win iff the matched
///   face covers the char.
/// - No winner → the FIRST entry (FontFallbackIter patches per-glyph).
/// Adjacent equal resolutions merge.
pub fn resolve_spans(
    text: &str,
    stack: &FontStack,
    weight: u16,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
) -> Resolution {
    /* char_indices walk; current: Option<(start, ResolvedFamily)>;
       resolve_char for boundary-forcing chars; flush on change; tail
       flush; empty stack → single Generic(SansSerif) span (the T2
       degrade, sync.rs first_family precedent). */
}
```

`resolve_char` (private) implements the decision-7 walk; the Loading+Block
arm sets `blocked = true` only while `now < registry.block_deadline(name)`
(past the deadline Block degrades to Swap — the § 7 timeout).
`FontDisplay::Fallback | Optional` hit the warn-once degrade
(`components.rs` statics precedent) and behave as Swap.

- [ ] **Step 3: Wire into `TextSync`**

In `sync.rs`:
- `text_sync_buffers` gains `registry: Res<FontRegistry>`,
  `index: ResMut<FontMatchIndex>`, `time: Res<Time>`; thread
  `&FontRegistry`/`&mut FontMatchIndex`/`now` through `SyncContext` into
  `apply_authored`.
- Replace the T2-interim single-family lowering:

```rust
fn apply_authored(
    buffer: &mut TextBuffer,
    text: &Text,
    style: &AuthoredStyle<'_>,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
) -> bool {
    let collapsed = collapse_whitespace(&text.0, style.white_space.collapse_mode());
    let directed = prepend_strong_marks(&collapsed, style.direction);
    let resolution = resolve_spans(&directed, style.family, style.weight, registry, index, now);
    buffer.buffer.set_metrics(style.metrics());
    buffer.buffer.set_wrap(resolve_wrap(style.white_space, style.text_wrap));
    buffer.buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    match resolution.spans.as_slice() {
        // ≤1 span: the T2 set_text path (decision 8) — identical buffer
        // state, no AttrsList churn.
        [] => buffer.buffer.set_text(&directed, &style.attrs(), TEXT_SHAPING, None),
        [only] => buffer
            .buffer
            .set_text(&directed, &span_attrs(style, &only.family), TEXT_SHAPING, None),
        spans => buffer.buffer.set_rich_text(
            spans.iter().map(|s| (&directed[s.range.clone()], span_attrs(style, &s.family))),
            &style.attrs(),
            TEXT_SHAPING,
            None,
        ),
    }
    buffer.invalidate_intrinsics();
    resolution.blocked
}

/// Base attrs + the span's resolved family. Weight rides the committed
/// surface (Attrs.weight → Query.weight → get_font(id, weight) — variable
/// weight works, font-assets § 6); style/stretch stay Normal (no carriers
/// — C-tier).
fn span_attrs<'a>(style: &AuthoredStyle<'_>, family: &'a ResolvedFamily) -> Attrs<'a> {
    let base = Attrs::new().weight(Weight(style.weight));
    match family {
        ResolvedFamily::Named(name) => base.family(Family::Name(name)),
        ResolvedFamily::Generic(generic) => base.family(generic.to_cosmic()),
    }
}
```

- `AuthoredStyle::attrs()` keeps its first-entry shape as the
  `default_attrs`/empty fallback; update its "T2 INTERIM" doc — the resolver
  is now the lowering, `attrs()` is the rich-text default + empty-text path.
- The returned `blocked` flag is consumed in Task 7 — until then,
  `let _blocked = …;` with a `// Task 7 (font-display Block) consumes this`
  marker, OR land Tasks 6+7 in one PR-internal sequence (implementer's
  choice; the gate must be green at every commit either way).

- [ ] **Step 4: Run the gate (both lanes), commit**

Steady-state guard: `tests/text_sync.rs` + `tests/text_measure.rs` counts
must be byte-identical to pre-task values (the resolver runs only inside
trigger-gated syncs; a hit on these counters means the gating broke).

```bash
git add -A
git commit -m "feat(text): T5 task 6 — the Buiy-owned FontStack resolver

Per-codepoint stack walk above cosmic-text (Attrs.family is single; the
Fallback trait is global — verified): fontdb Query matching on the
FontMatchIndex snapshot, coverage span-splitting (Common/Inherited never
split — the itemization rule), unicode-range per-codepoint filtering,
generic entries terminal, stack-miss → first entry with FontFallbackIter
as the engine-internal per-glyph last resort. Lowered via set_text (≤1
span) / set_rich_text (splits), both lazy — TextSync stays lock-free."
```

---

### Task 7: `font-display` — Swap default, Block zero-alpha + timeout (headless)

**Files:**
- Create: `crates/buiy_core/tests/text_font_display.rs`
- Modify: `crates/buiy_core/src/text/registry.rs` (`expire_font_block`)
- Modify: `crates/buiy_core/src/text/sync.rs` (PendingFontBlock writes)
- Modify: `crates/buiy_core/src/text/extract.rs` (zero-alpha arm + probes)
- Modify: `crates/buiy_core/src/text/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_font_display.rs`:

```rust
//! font-display (font-assets § 7): Swap renders the next resolved family
//! immediately (FOUT — load completion reshapes via the generation bump);
//! Block keeps the IDENTICAL fallback layout but paints zero-alpha until
//! load or the 3 s timeout. Loading states are driven headless via
//! reserve_handle (no async IO); the alpha side runs through the
//! adapterless extract harness.

/* fixtures: AssetPlugin app (the text_registry.rs shape) + the
   TextExtractHarness for instance-alpha assertions. */

#[test]
fn swap_renders_fallback_while_loading_and_reshapes_once_on_load() {
    // register_asset("Pending Sans", reserved_handle, Swap default);
    // stack ["Pending Sans", sans-serif]; settle.
    // 1. Loading: glyphs shape with the EMBEDDED face (resolver skipped
    //    the Loading family) — assert font_id == embedded.
    // 2. Insert the asset (Fira bytes under "Pending Sans"? NO — decision
    //    4: declared name must match the internal family; this test
    //    registers the DECLARED name "Fira Sans" instead and stacks
    //    ["Fira Sans", serif]… adjust: register under "Fira Sans" with a
    //    reserved handle; baseline = the generic serif pin.)
    // 3. Loaded: exactly one FontsGeneration bump; entities reshape; the
    //    layout's geometry now reflects the registered face (font_id ==
    //    the record's face).
}

#[test]
fn block_layout_is_the_fallback_layout_and_paint_is_zero_alpha() {
    // Same Loading registration with FontDisplay::Block:
    // 1. ComputedTextLayout (size, line metrics) == the Swap fixture's —
    //    IDENTICAL fallback layout, geometry never jumps twice (§ 7).
    // 2. PendingFontBlock present on the entity.
    // 3. Through the extract harness: every emitted GlyphAlphaInstance for
    //    the entity has color[3] == 0.0 — and instances ARE emitted (the
    //    zero-alpha skip is bypassed for blocked entities; the atlas stays
    //    warm with the fallback's glyphs).
}

#[test]
fn block_clears_on_load_with_one_bump() {
    // Loading+Block → insert asset → settle: PendingFontBlock removed,
    // alpha restored (instances carry the real color), ONE generation
    // bump, glyphs now the loaded face's.
}

#[test]
fn block_times_out_to_swap_after_three_seconds() {
    // Loading+Block, never load. Advance Time<Virtual> past
    // FONT_BLOCK_TIMEOUT_SECS (the stepped-clock discipline — no sleeps):
    //   app.world_mut().resource_mut::<Time<Virtual>>()
    //      .advance_by(Duration::from_secs_f64(3.5));
    // settle → expire_font_block removed the component; alpha restored
    // (fallback now PAINTS — the § 7 "then swap" arm); a LATER load still
    // swaps the face via the normal generation path.
}

#[test]
fn fallback_and_optional_degrade_to_swap_with_warn_once() {
    // FontDisplay::Fallback / ::Optional registrations behave exactly as
    // Swap (the C-tier reserve) — assert no PendingFontBlock, fallback
    // paints with full alpha.
}
```

Run: FAILS.

- [ ] **Step 2: Implement**

`sync.rs` — consume the Task 6 `blocked` flag (entity-level, decision 9):

```rust
// In sync_one / the creation arm, after apply_authored:
let blocked_until = blocked
    .then(|| ctx.registry.earliest_block_deadline(style.family))
    .flatten();
match (blocked_until, has_pending /* Option<&PendingFontBlock> from the query */) {
    (Some(until), existing) if existing.map(|p| p.until) != Some(until) => {
        commands.entity(entity).insert(PendingFontBlock { until });
    }
    (None, Some(_)) => {
        commands.entity(entity).remove::<PendingFontBlock>();
    }
    _ => {} // idempotent: no tick churn on steady re-syncs
}
```

(`SyncedText` gains `Option<&PendingFontBlock>` — tuple count 11, within
Bevy's cap. `earliest_block_deadline(stack)` on the registry: min deadline
across the stack's Loading+Block families — the resolver already computed
per-family deadlines; expose what it used.)

`registry.rs` — the expiry system:

```rust
/// font-display Block's timeout (§ 7: "until load or a configurable
/// timeout (web default: 3 s), then swap"): removing the marker IS the
/// swap-to-visible — the producer's Changed/Removed probes repaint.
pub fn expire_font_block(
    mut commands: Commands,
    time: Res<Time>,
    pending: Query<(Entity, &PendingFontBlock)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, block) in &pending {
        if now >= block.until {
            commands.entity(entity).remove::<PendingFontBlock>();
        }
    }
}
```

Registered `.before(crate::BuiySet::Layout)` next to `apply_font_registry`.

`extract.rs`:
- texts query gains `Option<&PendingFontBlock>`; the probe union gains
  `Changed<PendingFontBlock>`; a third removal stream
  `removed_block: Extract<RemovedComponents<PendingFontBlock>>` joins the
  drain-first block (`let block_lifted = removed_block.read().count() > 0;`
  folded into `dirty`).
- Emission (the decision-9 shape):

```rust
let blocked = pending_block.is_some();
// § 7 Block: identical fallback layout, ZERO-alpha paint — instances ARE
// emitted (atlas + buffer stay warm; the skip below is bypassed).
let entity_color = if blocked {
    [entity_color[0], entity_color[1], entity_color[2], 0.0]
} else {
    entity_color
};
...
let color = glyph.color_opt.map(span_color).unwrap_or(entity_color);
if !blocked && color[3] == 0.0 {
    continue; // fully transparent (NOT block-pending): nothing to paint
}
```

- [ ] **Step 3: Run the gate (both lanes), commit**

```bash
git add -A
git commit -m "feat(text): T5 task 7 — font-display Swap (default) + Block (zero-alpha, 3 s timeout)

Swap: Loading families skip in the resolver, fallback renders immediately,
load completion = one generation bump = one reshape (FOUT). Block:
identical fallback LAYOUT, instances emitted with alpha 0 (PendingFontBlock
marker; producer probe union + removal stream grow); expire_font_block
flips to Swap at the 3 s web default. Fallback/Optional reserved — degrade
to Swap with warn-once."
```

---

### Task 8: Per-script OFL fixture fonts — the provenance script (network)

The T1 pattern (`tools/fonts/subset_default_font.sh`), once per script.
**This task downloads from the network** — pinned URLs + sha256, fonttools
via the same pinned pip install T1 used
(`python3 -m pip install fonttools==4.56.0`, venv recommended).

**Files:**
- Create: `tools/fonts/subset_fixture_fonts.sh`
- Create: `crates/buiy_core/tests/fixtures/fonts/` (script output, committed)
- Modify: `crates/buiy_core/tests/support/mod.rs`
- Modify: `crates/buiy_core/src/text/font_system.rs` (`BuiyFallback` lists)
- Modify: `crates/buiy_core/tests/text_resolver.rs` (flip the `#[ignore]`)

- [ ] **Step 1: Write the provenance script**

Create `tools/fonts/subset_fixture_fonts.sh`, mirroring
`subset_default_font.sh` exactly (pins block, preflight, fetch+verify,
subset, license copy). One entry per fixture font:

| Fixture family | Upstream (pin a tag/commit + sha256) | Subset ranges |
|---|---|---|
| Noto Sans Arabic | `notofonts/notofonts.github.io` raw at a PINNED commit: `fonts/NotoSansArabic/hinted/ttf/NotoSansArabic-Regular.ttf` | `U+0600-06FF,U+0750-077F,U+0020,U+200C-200F` |
| Noto Sans Hebrew | same repo: `fonts/NotoSansHebrew/hinted/ttf/NotoSansHebrew-Regular.ttf` | `U+0590-05FF,U+0020,U+200E-200F` |
| Noto Sans Devanagari | same repo: `fonts/NotoSansDevanagari/hinted/ttf/NotoSansDevanagari-Regular.ttf` | `U+0900-097F,U+0020,U+200C-200D` |
| Noto Sans SC | `notofonts/noto-cjk` raw at a PINNED tag (e.g. `Sans2.004`): `Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf` | `--text='你好，世界'` (the corpus string only — keeps the artifact tiny) |
| Noto Emoji | `googlefonts/noto-emoji` raw at a PINNED commit: the monochrome `fonts/NotoEmoji-Regular.ttf` (verify the path at the pinned commit; the repo has reshuffled — if only the variable `NotoEmoji[wght].ttf` exists, subset that; swash renders the default instance) | the corpus ZWJ sequence's codepoints: `U+1F468,U+1F469,U+1F467,U+1F466,U+200D,U+FE0F` + `U+0020` |

Execution rule for the pins (the T1 precedent had its sha verified before
commit): **fetch once, record the printed sha256, paste it into the script,
re-run end-to-end so `sha256sum -c` passes** — the committed script must be
self-verifying. Subset flags per font: the T1 set
(`--layout-features=ccmp,kern,liga,clig,calt,locl,mark,mkmk` — for Arabic
add `init,medi,fina,isol,rlig` [joining]; for Devanagari add the Indic set
`nukt,akhn,rphf,blwf,half,vatu,pres,abvs,blws,psts,haln,abvm,blwm` [conjunct
reordering]; for emoji keep `ccmp,liga` [ZWJ ligatures]), plus
`--name-IDs='*' --name-languages='*' --name-legacy` (OFL name records) and
`--notdef-outline`. Outputs:
`crates/buiy_core/tests/fixtures/fonts/<Name>-<script>.ttf` (the CJK one
stays `.otf`) + one `OFL-<Name>.txt` per upstream. Run the script; commit
script + artifacts + licenses together. Sizes: assert each artifact is
< 200 KB except CJK (< 1 MB) — a bloated subset means the ranges are wrong.

- [ ] **Step 2: Support helpers + fallback lists**

In `tests/support/mod.rs`:

```rust
/// A committed per-script fixture font (verification § 2.2; produced ONLY
/// by tools/fonts/subset_fixture_fonts.sh).
pub fn fixture_font_bytes(file_name: &str) -> std::sync::Arc<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fonts")
        .join(file_name);
    std::sync::Arc::new(std::fs::read(&path).unwrap_or_else(|e| {
        panic!("fixture font {file_name} missing ({e}); run tools/fonts/subset_fixture_fonts.sh")
    }))
}

/// Register a fixture font through the production bytes path and settle.
pub fn register_fixture_font(app: &mut App, family: &str, file_name: &str) { /* registry.register_bytes + one update */ }
```

In `font_system.rs`, fill `BuiyFallback::script_fallback` (decision 14 —
the T1 placeholder's documented fill): deterministic per-script lists naming
the fixture families (`Arab` → `["Noto Sans Arabic"]`, `Hebr` →
`["Noto Sans Hebrew"]`, `Deva` → `["Noto Sans Devanagari"]`, `Han` →
`["Noto Sans SC"]`, else `[]`), with the module-doc note that absent
families are skipped harmlessly (the `common_fallback` contract) — apps
that register their own per-script fonts under these generic roles get
deterministic CI resolution; nothing platform-varying ever enters.
**Check the exact family names the subsets carry** (`fontdb` reads the name
table — print `FaceInfo.families` in a throwaway test) and use those
strings verbatim in both the fallback lists and the test registrations.

Flip `coverage_miss_splits_spans`'s `#[ignore]` (Task 6) to live, using the
Hebrew fixture.

- [ ] **Step 3: Run the gate (both lanes), commit**

```bash
git add -A
git commit -m "test(text): T5 task 8 — per-script OFL fixture fonts + provenance script

Arabic/Hebrew/Devanagari/CJK-SC/emoji(monochrome) subsets via
tools/fonts/subset_fixture_fonts.sh (the T1 pattern: pinned upstreams +
sha256 + fonttools 4.56.0; OFL name records kept; artifacts committed,
never hand-edited). BuiyFallback grows the deterministic per-script lists
naming the fixture families (the T1 placeholder's documented fill).
Monochrome emoji because the v1 producer skips SwashContent::Color."
```

---

### Task 9: The multi-script shaping-snapshot corpus (headless)

**Files:**
- Create: `crates/buiy_core/tests/text_shaping_snapshots.rs`
- Create: `crates/buiy_core/tests/fixtures/shaping/*.snap` (harness output, curated + committed)

- [ ] **Step 1: Write the harness + fixtures (failing: no snapshots yet)**

Create `crates/buiy_core/tests/text_shaping_snapshots.rs`:

```rust
//! The multi-script shaping-snapshot corpus (verification §§ 1.2, 2.2):
//! one fixture per F-tier shaping claim — Latin, Arabic (joining/RTL),
//! Devanagari (reordering), CJK, emoji-ZWJ, mixed-BiDi — each pinning
//! `(line_i, glyph_id, font_seat, x, y)` per glyph against the committed
//! per-script OFL fixture fonts. The text analogue of the gate-#5 layout
//! snapshots; breadth beyond one-behavior-per-fixture is upstream's job.
//!
//! Update workflow (the golden `--accept` analogue, human-curated):
//!   BUIY_ACCEPT_SHAPING=1 cargo test -p buiy_core --test text_shaping_snapshots
//! then REVIEW THE DIFF before committing — a snapshot change is a shaping
//! change.

mod support;

/* fixture table: (name, fixture fonts to register, stack, text, dir) */
const CORPUS: &[Fixture] = &[
    Fixture { name: "latin",      fonts: &[], stack: SANS, text: "Sphinx of black quartz, judge my vow.", dir: None },
    Fixture { name: "arabic",     fonts: &[ARABIC], stack: ARABIC_STACK, text: "السلام عليكم", dir: None },
    Fixture { name: "devanagari", fonts: &[DEVANAGARI], stack: DEVA_STACK, text: "नमस्ते क्षत्रिय", dir: None },
    Fixture { name: "cjk",        fonts: &[CJK], stack: CJK_STACK, text: "你好，世界", dir: None },
    Fixture { name: "emoji_zwj",  fonts: &[EMOJI], stack: EMOJI_STACK, text: "👨\u{200D}👩\u{200D}👧\u{200D}👦", dir: None },
    Fixture { name: "mixed_bidi", fonts: &[HEBREW], stack: BIDI_STACK, text: "hello עולם world", dir: None },
];

/* per fixture:
   1. text_app() + register fixture fonts (support::register_fixture_font,
      the production bytes path — NOT a hand-built FontSystem: the corpus
      proves the whole resolver+registry+engine stack);
   2. spawn a 400 px-wide text leaf with the stack + 20 px FontSize;
      settle;
   3. fold buffer.layout_runs() into the snapshot text:
        line_i glyph_id font_seat x y      (x/y as {:.2})
      where font_seat = index of glyph.font_id in the fixture's
      first-seen-order face list (fontdb IDs are NOT stable across
      processes — seats are; the file header lists seat → family name);
   4. compare against tests/fixtures/shaping/<name>.snap; on mismatch
      print a labeled unified diff and panic — unless BUIY_ACCEPT_SHAPING
      is set, in which case (re)write the file and pass. */

/* PLUS per-fixture structural assertions that hold regardless of the
   snapshot bytes (the hand-derivable invariants):
   - arabic: every run rtl == true; glyph count < char count (joining
     ligation happened); all font_seats == the Arabic face's seat.
   - devanagari: at least one glyph cluster reorders (a glyph whose
     `start` byte > a later glyph's — the reordering witness).
   - cjk: every codepoint produced exactly one glyph; seats == CJK face.
   - emoji_zwj: glyph count < scalar-value count (ZWJ ligation) — and NOT
     zero (the monochrome fixture rasterizes as Mask, decision 12).
   - mixed_bidi: ≥ 2 seats used (Hebrew face + embedded latin); the line's
     rtl == false (first-strong = "hello"); the Hebrew glyphs' x positions
     are CONTIGUOUS and reverse-ordered relative to logical order (the
     visual-reorder witness).
   - latin: single seat, x strictly increasing (no reorder). */
```

First run: the structural assertions pass or fail honestly; the snapshot
compare fails with "no snapshot committed". Run once with
`BUIY_ACCEPT_SHAPING=1`, **manually review** every generated `.snap`
(plausible glyph counts, seats, monotonic y), commit them.

- [ ] **Step 2: Determinism double-run**

Run the suite twice back-to-back (no `--accept`): both green, byte-stable.
Then run the FULL headless gate — the corpus now rides every PR.

```bash
git add -A
git commit -m "test(text): T5 task 9 — multi-script shaping-snapshot corpus

Latin/Arabic/Devanagari/CJK/emoji-ZWJ/mixed-BiDi fixtures through the full
registry+resolver+engine stack against the committed OFL subsets; snapshots
pin (line, glyph_id, font_seat, x, y) per glyph (seats, not raw fontdb IDs
— process-stable); BUIY_ACCEPT_SHAPING=1 is the curated update gate.
Structural invariants (joining, reordering, ZWJ ligation, BiDi reorder,
seat assignment) assert independently of the snapshot bytes."
```

---

### Task 10: GPU lane — the multi-script golden + the rebuild-storm bound (`#[ignore]`)

**Files:**
- Modify: `crates/buiy_core/tests/text_gpu.rs`

- [ ] **Step 1: Write the two tests**

Append to `tests/text_gpu.rs` (the existing `capture`/`brightest`/
`perceptual_diff` idioms; both `#[ignore]`):

```rust
// --- (d) T5: the multi-script golden (campaign: "1–2 goldens"). ----------
#[test]
#[ignore = "needs a wgpu adapter; T5 multi-script golden (verification § 1.3 pixels row)"]
fn multi_script_text_renders_deterministically() {
    // Two RTL lines through the fixture fonts — Arabic (joining) and the
    // mixed-BiDi string (Hebrew + Latin) — registered via the production
    // bytes path. Inline-golden discipline (the T4 stored-PNG deferral
    // stands): capture twice in two independent app instances, assert
    // byte-stability + non-emptiness. Glyph correctness lives headless in
    // the corpus; THIS test proves the pixels lane end-to-end (resolver →
    // set_rich_text → rasterize → atlas → draw) with non-Latin faces.
    fn capture_bidi() -> Vec<u8> {
        let mut app = support::gpu_render_app(W, H);
        support::register_fixture_font(&mut app, "Noto Sans Arabic", "NotoSansArabic-arabic.ttf");
        support::register_fixture_font(&mut app, "Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");
        /* spawn two 20 px lines: "السلام عليكم" (stack [Noto Sans Arabic,
           sans-serif]) and "hello עולם world" (stack [Noto Sans Hebrew?
           — NO: latin must hit the embedded face, so stack
           [Noto Sans Hebrew, sans-serif] and let coverage split]) under a
           sized column; TextColor token as in spawn_text_fixture. */
        /* render_to_image + spawn_capture_camera + finish_and_run +
           wait_for_text_ready + readback */
    }
    let a = capture_bidi();
    let b = capture_bidi();
    assert!(!a.chunks_exact(4).all(|p| p == &a[0..4]), "something painted");
    assert!(
        perceptual_diff(&a, &b) < 1e-4,
        "two independent captures are byte-stable (deterministic fonts + resolver)"
    );
}

// --- (e) T5: THE rebuild-storm bound (font-assets §§ 3.2, 10). ------------
#[test]
#[ignore = "needs a wgpu adapter; T5 rebuild-storm bound (one frame of misses, baseline restored)"]
fn font_db_rebuild_storm_is_bounded() {
    // A fresh-db swap reissues EVERY fontdb ID: every AtlasKey goes stale
    // at once. Bounded, not broken: one frame of misses re-rasterizes,
    // old entries grace-evict, page count and entry count return to
    // baseline, pixels never change (same font bytes, same shaping).
    let mut app = support::gpu_render_app(W, H);
    {
        // Tight grace so the settle window is test-sized (the T4 fixture's
        // AtlasConfig override pattern).
        let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
        render_app.world_mut().insert_resource(BuiyAtlas::new(AtlasConfig {
            page_size: 1024,
            page_budget: 8,
            eviction_grace: 3,
        }));
    }
    spawn_text_fixture(&mut app, Color::srgba(0.9, 0.9, 0.2, 1.0));
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);
    let frame_before = support::readback_rgba(&mut app, target.clone());
    let (entries_before, pages_before, keys_before) = {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        (
            atlas.live_entry_count(),
            atlas.page_count(AtlasFormat::CoverageR8),
            render_app.world().resource::<ResidentTextKeys>().keys.clone(),
        )
    };

    // Trigger the swap through the production path: a completed scan task
    // carrying a FRESH registered-baseline db (same bytes — pixels must
    // not move; only the IDs do).
    let task = bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async move { buiy_core::text::registered_fonts_db() });
    app.world_mut()
        .insert_resource(buiy_core::text::PendingSystemFontScan(Some(task)));

    // The storm frame(s): swap applies → generation+lineage bump → sweep
    // reshape → producer rebuild, interner reseat, full re-rasterize.
    app.update();
    app.update();
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        let keys_after = &render_app.world().resource::<ResidentTextKeys>().keys;
        assert!(!keys_after.is_empty());
        assert!(
            keys_after.iter().all(|k| !keys_before.contains(k)),
            "every key re-seated (fresh lineage = fresh font u32s)"
        );
        assert!(
            atlas.live_entry_count() > entries_before,
            "old entries still grace-resident mid-storm (the double-resident window)"
        );
    }

    // Settle past the grace window: baseline restored, pixels identical.
    for _ in 0..8 {
        app.update();
    }
    {
        let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
        let atlas = render_app.world().resource::<BuiyAtlas>();
        assert_eq!(atlas.live_entry_count(), entries_before, "entry count returned to baseline");
        assert_eq!(
            atlas.page_count(AtlasFormat::CoverageR8),
            pages_before,
            "page count returned to baseline (the campaign's bound)"
        );
    }
    let frame_after = support::readback_rgba(&mut app, target);
    assert!(
        perceptual_diff(&frame_before, &frame_after) < 1e-4,
        "the storm is invisible: same bytes, same shaping, same pixels"
    );
}
```

- [ ] **Step 2: Run the GPU lane, then the headless gate, commit**

Run: `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`
Debug notes: if `keys_after` intersects `keys_before`, `begin_lineage` is
not being called before keying (or the lineage extract is missing); if
entry count never returns, the touch pass is touching dead keys (it must
touch `resident.keys` — the NEW set — only); if pixels move, the swap
re-shaped against a *different* db than the baseline (the task must build
`registered_fonts_db()`, nothing else).

```bash
git add -A
git commit -m "test(text): T5 task 10 — GPU multi-script golden + rebuild-storm bound

Arabic + mixed-BiDi pixels through the fixture-font resolver path,
byte-stable across independent captures; the fresh-db swap storm is
bounded — every key re-seats, old entries grace-evict, entry/page counts
return to baseline, pixels never move."
```

---

### Task 11: Docs flip + errata + self-review

**Files:**
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/README.md`
- Modify: `docs/plans/2026-06-10-buiy-text-t5-fonts-fallback-bidi.md` (Status)

- [ ] **Step 1: Campaign plan**

In `docs/plans/2026-06-09-buiy-text-campaign.md`:

(a) Phase-status table: `| T5 | Fonts, fallback, and BiDi correctness | proposed |` → `landed`.

(b) Append to the T5 section, mirroring the T1–T4 errata convention:

```markdown
- **T5 errata for the spec edit pass** (mechanical inaccuracies found while
  implementing — see the T5 plan's Orientation + decisions 1–9; superseding
  context, not a silent contradiction):
  1. *font-assets § 3.2's "every rebuild issues fresh IDs for every face"*
     is wrong for the § 3.1 path: `into_locale_and_db` returns the SAME
     `Database` by value (system.rs:297–299), and fontdb IDs are slotmap
     keys — surviving faces keep their IDs across `remove_face`, and dead
     IDs never alias in-lineage (version bump on slot reuse). The claim
     holds only for FRESH-database rebuilds (the § 5 scan swap) — where the
     real hazard is the opposite one: fresh databases REISSUE equal ID
     values for different faces, so the render-side `FontKeyInterner` must
     clear per database lineage with a monotonic seat counter
     (`FontDbLineage`, bumped only by fresh-db swaps, always alongside
     `FontsGeneration`). The AtlasKey-never-persisted rule alone does not
     close it — a key rebuilt from a live ID can alias a grace-resident old
     entry.
  2. *architecture § 1.2's "exactly three lock sites"* is steady-frame
     scoped, not absolute: `swap_font_db` has been a rare-event fourth
     since T1, and T5 adds `apply_font_registry` (registration /
     unregistration / hot-reload, event-driven, pre-Layout, one lazy hold
     per batch). The spec edit should rescope the table to steady-frame
     sites and list the rare-event sites beside it.
  3. *font-assets §§ 2–3's `FontKey`* (the `register_font_bytes` return) is
     dropped as-built: the registry's public identity is the declared
     family name (the FontFaceSet model); `FontKey` is defined nowhere
     else. Corollary: registration REQUIRES the declared family name (the
     Loading state exists before the file's internal names are knowable —
     `font-display: Block` is unimplementable without it); a declared/
     internal mismatch warns loudly and cannot match until the § 9
     family-alias seam lands.
  4. *font-assets § 6's `FontFallbackIter`* — verified public-but-internal
     in 0.19: reachable at `cosmic_text::fallback::FontFallbackIter`
     (`pub mod fallback`, glob re-exported), but Buiy never constructs one;
     it is the engine-internal per-glyph last resort inside shaping
     (shape.rs:307/489/985). The spec's framing is accurate; the as-built
     resolver leans on it implicitly by emitting concrete families only.
  5. *coverage extraction*: `cosmic_text::Font::unicode_codepoints()` is
     feature-gated behind the non-default `monospace_fallback` (returns
     `&[]` under the default-features pin) and `Font::new` rejects
     `Source::File` faces. As built, coverage is extracted Buiy-side via
     `fontdb::Database::with_face_data` + the crate-root-re-exported
     `skrifa` charmap — no new dependency, no feature flip.
  6. *measure § 5.4*: marks are prepended per NON-EMPTY line only — a
     shaped mark on an empty line could grow a phantom glyph and flip T3's
     glyphs-keyed `ResolvedBaseline` for `Text("")`. Empty-line caret
     direction is the editing campaign's offset-table seam.
```

- [ ] **Step 2: docs/README.md**

After the T4 plan line in the text-plans block, add:

```markdown
- [Buiy text T5 — Fonts, fallback, and BiDi](plans/2026-06-10-buiy-text-t5-fonts-fallback-bidi.md) — `BuiyFont` asset + `BuiyFontLoader` (sfnt invariant = the named woff2 seam; Modified = remove+re-add), `FontRegistry` (strong handles, declared family names, in-place `load_font_source` add, rebuild-on-remove via `into_locale_and_db` — surviving fontdb IDs verified stable in-lineage; `FontDbLineage` + monotonic `FontKeyInterner` reseat close the fresh-db AtlasKey-aliasing hazard), the lock-free `FontStack` resolver (`FontMatchIndex` db snapshot + skrifa-charmap coverage, fontdb `Query` matching, coverage span-splitting via `set_rich_text`, `unicode-range`, `FontFallbackIter` as the implicit last resort), `font-display` Swap/Block (zero-alpha + 3 s timeout), `TextDirection` strong-mark prepend (§ 5.4), per-script OFL fixture subsets + the multi-script shaping-snapshot corpus, GPU multi-script golden + rebuild-storm bound. `[landed]`
```

(Adjust the trailing status marker to match reality at merge time.)

- [ ] **Step 3: This plan's Status header**

Flip `**Status:** proposed` → `**Status:** landed` in this file.
(CLAUDE.md's GPU-lane parenthetical already covers text; no edit needed —
verify and leave it if so. `follow-ups.md` carries no T5 entries — verify
with a grep for `font`/`bidi` and leave untouched if clean.)

- [ ] **Step 4: Self-review (the implementer runs this checklist, fixes inline)**

1. **Charter coverage**, clause by clause against the campaign T5 entry:
   `BuiyFont` + loader w/ sfnt invariant + Modified=remove+re-add (Tasks
   3–4); `FontRegistry` strong handles + in-place add + rebuild-on-remove +
   the AtlasKey rule made mechanical (Tasks 1, 4, 5); the resolver — Query
   matching, coverage splits, unicode-range, `FontFallbackIter` last resort
   (Task 6); font-display Swap default + Block zero-alpha (Task 7);
   per-node direction via the strong-mark prepend (Task 2); the corpus +
   per-script OFL fixtures (Tasks 8–9); GPU goldens incl. the rebuild-storm
   bound (Task 10).
2. **Lock-site ledger:** grep `\.lock()` over `crates/buiy_core/src` — the
   steady-frame three (measure, commit, `resolve_glyph`) plus exactly two
   rare-event sites (`swap_font_db`, `apply_font_registry`). `TextSync`
   takes NO lock (the resolver is snapshot-only) — a lock in `sync.rs` or
   `resolver.rs`/`match_index.rs` is a review reject.
3. **`#[ignore]` audit:** both new GPU tests carry it; the headless gate
   run proves no new test touches an adapter.
4. **Steady-state audit:** `text_sync.rs`/`text_measure.rs`/
   `text_extract.rs` steady-frame counts unchanged from the T4 baseline;
   the resolver runs only inside trigger-gated syncs.
5. **Seam honesty:** grep for the named seams — rich-text spans, variable
   axes, theme tokens, woff2, alias overrides, per-run Block — each appears
   as a comment/doc seam, none as code.
6. **Type consistency:** `resolve_spans(text, stack, weight, registry,
   index, now) -> Resolution`, `FontMatchIndex::{new, reset, query,
   covers}`, `FontKeyInterner::{intern, begin_lineage}`,
   `swap_font_db(fonts, db, registry) -> Database` — signatures match
   across tasks 1/4/5/6/7/10.
7. Both gate lanes green at HEAD.

- [ ] **Step 5: Run the full gate (both lanes), commit**

```bash
git add -A
git commit -m "docs(text): T5 docs flip — campaign status, errata 1-6, README catalog

T5 landed: font assets + registry + lock-free resolver + font-display +
TextDirection + fixture corpus + GPU goldens. The fontdb-ID findings
(in-lineage stability, fresh-db reissue) and the FontFallbackIter /
unicode_codepoints verifications recorded for the spec edit pass."
```
