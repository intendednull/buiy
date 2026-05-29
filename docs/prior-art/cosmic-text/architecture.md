**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — module structure, data model, shaping/layout/cache pipeline

## Scope

This file documents the internal architecture of `cosmic-text` as of **0.19.0** (published 2026-04-22, MSRV 1.89, MIT OR Apache-2.0, stewarded by System76 / Jeremy Soller as part of the COSMIC desktop). It is the load-bearing text engine for Buiy; sibling files cover [shaping](shaping.md), [bidi](bidi.md), [capabilities](capabilities.md), and (Agent B) editing, integration, history, ecosystem, governance, critiques, open-problems.

## Brief correction up front

The Buiy-side verified-facts pre-amble called the substrate **rustybuzz + swash + unicode-bidi + fontdb**. Verified against `Cargo.toml` on `main` at HEAD:

- The shaper is **harfrust 0.5.0**, not rustybuzz. `harfrust` is a fork of rustybuzz, maintained by the official `harfbuzz` GitHub org, that ported the font backend from `ttf-parser` to `read-fonts`. The migration shipped in **cosmic-text 0.15.0** (PR #417, merged 2025-09-09); rustybuzz no longer appears in `Cargo.toml` as of 0.15.0.
- `swash 0.2.6` is still present but **only with `render` + `scale` features** — i.e. swash is used as the rasterizer / outline scaler, not as the shaper, font fallback engine, or font discovery layer. Color emoji rasterization (COLR/CPAL, sbix, CBDT/CBLC) lives in `swash::scale`.
- Font data parsing now goes through **skrifa 0.40.0** (Google Fonts' `read-fonts`-based crate), which replaced what was previously routed through swash's introspection side.
- BiDi is **unicode-bidi 0.3.18** with `hardcoded-data` (UAX #9 tables compiled in). Confirmed.
- Font discovery is **fontdb 0.23**, optionally with `fontconfig` and `memmap`. Confirmed.

Treat the rest of this folder as written against the harfrust/skrifa/swash substrate. The Buiy text spec (`docs/specs/2026-05-07-buiy-foundation/text.md`) should be updated to match.

## Module layout (`src/`)

Top-level files in `src/`:

```
attrs.rs           - Attrs, AttrsList, Family, Weight, Style, Color, TextDecoration, FontFeatures
bidi_para.rs       - paragraph-level UAX #9 wrapper around unicode-bidi
buffer.rs          - Buffer, Metrics, Scroll, DirtyFlags, set_text/set_size/set_metrics
buffer_line.rs     - BufferLine (one logical paragraph), per-line cached shape/layout
cached.rs          - generic cached-value helpers used by buffer/layout
cursor.rs          - Cursor (line + byte index + affinity), Selection
glyph_cache.rs     - CacheKey + subpixel binning for rasterized glyphs
layout.rs          - LayoutLine, LayoutGlyph, Align, Wrap, justify wiring
line_ending.rs     - LineEnding (None, CrLf, Lf, Cr, NEL, LS, PS)
math.rs            - libm/core_maths shims for no_std
render.rs          - Color-buffer rendering helpers (uses SwashCache)
shape.rs           - ShapeLine, ShapeSpan, ShapeWord, ShapeGlyph, harfrust driver
shape_run_cache.rs - optional cache of harfrust shape results per (text, font, attrs)
swash.rs           - SwashCache, SwashImage, SwashContent (rasterizer wrapper)
edit/              - Editor + Vi mode + syntect highlighting
font/              - FontSystem, font fallback (per-platform), FontFallbackIter
```

`lib.rs` re-exports every submodule via `pub use self::<module>::*`, so the public surface is flat — there is no `cosmic_text::layout::LayoutGlyph`, only `cosmic_text::LayoutGlyph`.

## Data model

```text
FontSystem  (owns fontdb::Database, locale, font cache, shape-plan cache)
  |
  +-- Buffer                                  (multi-line view: lines + wrap + scroll)
        Metrics { font_size, line_height }
        wrap: Wrap, ellipsize: Ellipsize, hinting: Hinting
        scroll: Scroll, width_opt, height_opt, monospace_width, tab_width
        |
        +-- Vec<BufferLine>                   (one entry per logical paragraph / "line")
              text: String
              attrs_list: AttrsList            (per-byte-range Attrs)
              align: Option<Align>
              line_ending: LineEnding
              ---- lazily-built caches ----
              shape: Option<ShapeLine>         (BiDi-segmented, harfrust-shaped)
              layout: Option<Vec<LayoutLine>>  (one ShapeLine -> N visual LayoutLines)

ShapeLine
  rtl: bool                                   (paragraph base direction)
  spans: Vec<ShapeSpan>                       (one per BiDi level run)
        words: Vec<ShapeWord>                 (segmented by unicode-segmentation)
              glyphs: Vec<ShapeGlyph>         (harfrust output, per-glyph cluster + offsets)

LayoutLine
  w, max_ascent, max_descent, line_height_opt
  glyphs: Vec<LayoutGlyph>
  decorations: Vec<DecorationSpan>            (added in 0.19.0)

LayoutGlyph
  start, end                                  (cluster byte range into BufferLine)
  font_id, glyph_id, font_size, font_weight
  x, y, w, x_offset, y_offset
  level: unicode_bidi::Level                  (for caret + selection math)
  color_opt, metadata, cache_key_flags

CacheKey (glyph_cache.rs)
  font_id, glyph_id, font_size_bits: u32
  x_bin, y_bin: SubpixelBin                   (quantized to 0.0/0.25/0.5/0.75)
  font_weight, flags: CacheKeyFlags
```

`Cursor` is `{ line: usize, index: usize, affinity: Affinity }` — `index` is a byte offset into the `BufferLine`'s `text` string, not a char or grapheme offset.

## Shaping pipeline (one BufferLine)

1. **Segment.** `unicode_bidi::BidiInfo::new(text, base_dir)` resolves a paragraph-level BiDi context. Level runs partition the text into `ShapeSpan`s of equal level.
2. **Word-split.** Inside each span, `unicode-segmentation` produces word boundaries (so wrap and word-nav have a unit to operate on).
3. **Script-tag.** Per cluster, `unicode-script` decides the script; the FontSystem's `FontFallbackIter` produces a candidate font sequence (default family → script-fallback list → common fallback → all remaining).
4. **Shape.** harfrust shapes each (span, font) pair into `ShapeGlyph`s with `glyph_id`, advance, x/y offset, cluster index. If any returned `glyph_id == 0` (the `.notdef` "tofu"), the run is re-shaped against the next fallback font (per-glyph fallback granularity inside `shape_run` in `shape.rs`). Optional `shape-run-cache` feature memoizes (text, font, attrs) → glyphs.
5. **Layout.** `ShapeLine::layout` walks the spans, applies `Wrap` (`None | Glyph | Word | WordOrGlyph`), `Align` (`Left | Right | Center | Justified | End`), `Ellipsize`, and emits a `Vec<LayoutLine>`.
6. **Rasterize-on-demand.** The renderer (or embedder) calls `SwashCache::get_image(cache_key)` per visible glyph; swash does the outline scaling + COLR/CPAL / sbix / CBDT compositing into a `SwashImage`. The image is stored in the `SwashCache` HashMap, keyed by `CacheKey`.

Steps 1–5 run on the CPU at edit-time and rebuild incrementally per `BufferLine`. Step 6 happens at draw-time and is the embedder's atlasing concern.

## Glyph cache and eviction

`SwashCache` (in `swash.rs`) holds two `HashMap`s:

- `image_cache: HashMap<CacheKey, Option<SwashImage>>` — rasterized images.
- `outline_command_cache: HashMap<CacheKey, Option<Vec<Command>>>` — vector path commands, for vector-target embedders.

**There is no LRU or size-cap eviction.** The cache grows monotonically for the lifetime of the `SwashCache`. The embedder is expected to either (a) drop and rebuild the cache, (b) call `SwashCache::trim` (added in a recent release — verify variant present in 0.19.0 by reading `swash.rs`), or (c) wrap the cache in its own LRU. Bevy's `bevy_text` builds its own atlas on top and lets the upstream `SwashCache` accumulate.

`CacheKey` quantizes subpixel offsets into four bins per axis (`SubpixelBin::Zero | One | Two | Three` = 0/0.25/0.5/0.75 of a pixel). This bounds horizontal cache fan-out at 4× per (font, size, glyph) pair, in exchange for visible "shimmer" only at extreme zoom levels.

The `shape_run_cache` feature adds a separate cache *before* shaping (keyed on the shaping input), independent of `SwashCache`.

## Font discovery (`font/`)

`FontSystem` owns:

- `db: fontdb::Database` — the font registry. `fontdb` does the platform-specific enumeration (Windows registry, CoreText, fontconfig on Linux when the `fontconfig` feature is on, plain directory scan otherwise). The `memmap` feature mmaps font files instead of reading them into RAM.
- `locale: String` — the system locale, queried from `sys-locale` if the `std` feature is on.
- A per-font cache of harfrust shape plans (one harfrust `ShapePlan` per (font, script, direction, language) tuple, expensive to build, cheap to reuse).

Asset fonts are registered via `db.load_font_data(Vec<u8>)` or `db.load_font_source(...)`. Bevy embedders wrap this in their `Assets<Font>` pipeline.

### Per-platform fallback

`font/fallback/` has `macos.rs`, `windows.rs`, `unix.rs`, and `other.rs`, each implementing a `PlatformFallback` trait. Order:

1. Default families (with monospace specially sorted by codepoint coverage).
2. Script-specific list (`script_fallback(Script, locale) -> &[&str]`).
3. Common locale-independent list.
4. All remaining fonts in the database.

Emoji are not treated as a separate axis at the algorithm level; on Windows the common list includes `"Segoe UI Emoji"`, on macOS `"Apple Color Emoji"`, on Linux `"Noto Color Emoji"`. If none are installed, emoji code points fall through to `.notdef`.

## Color emoji handling

Color emoji rasterization is **entirely inside swash**, not in cosmic-text itself. `swash::scale::Render` chooses among:

- **COLR / CPAL** — vector color glyphs (Microsoft, Twemoji COLRv1).
- **sbix** — Apple bitmap strikes.
- **CBDT / CBLC** — Google bitmap strikes (Noto Color Emoji legacy).

Per [swash's README](https://github.com/dfrg/swash), **SVG color glyphs (the OpenType `SVG ` table) are not listed**. Buiy must not rely on SVG-table color fonts; the COLR/CPAL + bitmap path is the supported one. Variation-selector handling (UTS #51, e.g. text-vs-emoji presentation, skin-tone modifiers, ZWJ sequences) depends on the font's GSUB rules being applied by harfrust upstream of swash.

## Memory layout

`Buffer.lines` is a flat `Vec<BufferLine>` — there is no rope, no piece table. Each `BufferLine` owns its `text: String` and `attrs_list: AttrsList`. Insertion at the start of a long document is O(line-length); insertion across a line split is O(N-lines) for the `Vec` shift.

Each `BufferLine` lazily caches:
- its `ShapeLine` (rebuilt on text or attrs change),
- its `Vec<LayoutLine>` (rebuilt on width / metrics / wrap / align change).

An edit dirties only the touched line(s); other lines keep their caches. This is the rebuild granularity that matters for typing latency.

`DirtyFlags` on the `Buffer` distinguishes "needs redraw" from "needs re-layout" so embedders can skip work.

## Optional features (`Cargo.toml`)

- `default = ["std", "swash", "fontconfig"]` — the standard target.
- `vi` — adds `modit` + `syntect` + `cosmic_undo_2` for the bundled vi-mode editor (used by COSMIC's terminal).
- `shape-run-cache` — adds the pre-shape cache. Memory cost; latency win on repeated content.
- `monospace_fallback` — a separate sort criterion for monospace requests.
- `no_std` — pulls in `hashbrown`, `libm`, `core_maths`. Drops fontdb's `memmap` + `std`, drops `sys-locale`, drops unicode-bidi's `std` feature. **No `std` means no fontconfig, no system locale, no mmap** — the embedder supplies fonts directly.
- `peniko` — emits color values as `peniko::Color` for Linebender-stack consumers.
- `wasm-web` — turns on `sys-locale/js` so locale detection works in browsers.
- `warn_on_missing_glyphs` — emits a `log::warn!` when a glyph falls all the way through fallback to `.notdef`.

## Cross-links

- See [shaping.md](shaping.md) for harfrust feature parity and per-script behavior.
- See [bidi.md](bidi.md) for the UAX #9 wiring and known gaps.
- See [capabilities.md](capabilities.md) for the can-do / can't-do gap table from Buiy's perspective.
- Editing primitives (`Editor`, `Cursor` motion semantics, undo) are in Agent B's `editing.md`.
- Bevy integration and Iced integration are in Agent B's `integration.md`.

## Sources

- `Cargo.toml` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text/blob/main/Cargo.toml
- `src/lib.rs`, `src/shape.rs`, `src/buffer.rs`, `src/layout.rs`, `src/attrs.rs`, `src/glyph_cache.rs`, `src/swash.rs`, `src/font/fallback/mod.rs` at `pop-os/cosmic-text` HEAD.
- `CHANGELOG.md` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text/blob/main/CHANGELOG.md
- crates.io metadata — https://crates.io/api/v1/crates/cosmic-text (latest 0.19.0, 2026-04-22, 4,731,411 total downloads, 1,299,778 recent).
- README — https://github.com/pop-os/cosmic-text/blob/main/README.md
- harfrust — https://github.com/harfbuzz/harfrust (HarfBuzz v13.0.0 alignment, v0.7.0 as of 2026-05-21; cosmic-text pins 0.5.0).
- swash — https://github.com/dfrg/swash, https://crates.io/crates/swash (0.2.7 on crates.io, 2026-03-27; cosmic-text pins 0.2.6).
- skrifa — https://crates.io/crates/skrifa (Google Fonts read-fonts wrapper).
- fontdb — https://crates.io/crates/fontdb
- unicode-bidi — https://github.com/servo/unicode-bidi
