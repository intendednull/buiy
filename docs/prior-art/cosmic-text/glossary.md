**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — glossary of system-specific terms

Cross-link target for every other file in the folder. Terms are listed in rough dependency order (data types first, then companion crates, then standards).

## cosmic-text data types

- **`FontSystem`** — Process-wide handle owning the `fontdb::Database`, the locale, and the per-font cache of harfrust `ShapePlan`s. Non-`Sync`, non-`Clone`; every shape / layout / render call requires `&mut FontSystem`. Embedders keep one per process. See [`architecture.md`](architecture.md), [`integration.md`](integration.md).
- **`Buffer`** — The per-text-node owning type. Holds source text (as `Vec<BufferLine>`), `Metrics { font_size, line_height }`, `Wrap`, `Align`, `Ellipsize`, scroll state, optional width/height bounds. Re-laid-out lazily on text / attrs / width / metrics change. See [`architecture.md` § Data model](architecture.md#data-model).
- **`BufferLine`** — One logical paragraph. Owns `text: String`, `attrs_list: AttrsList`, optional `Align`, `LineEnding`, and lazily-built `ShapeLine` + `Vec<LayoutLine>` caches. An edit dirties only the touched line(s). See [`architecture.md`](architecture.md).
- **`Cursor`** — `{ line: usize, index: usize, affinity: Affinity }`. `line` is the BufferLine index; `index` is a **byte offset** into the line's UTF-8 string (not char or grapheme); `affinity` (Before/After) resolves visual ambiguity at level boundaries and soft-wrap boundaries. See [`editing.md`](editing.md), [`bidi.md`](bidi.md).
- **`ShapeLine`** — The intermediate shaping result for one BufferLine. Contains paragraph base direction (`rtl: bool`), `Vec<ShapeSpan>` (one per BiDi level run), each with `Vec<ShapeWord>` (segmented by `unicode-segmentation`), each with `Vec<ShapeGlyph>` (harfrust output: glyph_id, advance, x/y offset, cluster). See [`architecture.md`](architecture.md), [`shaping.md`](shaping.md).
- **`LayoutLine`** — One visual line. The output of layout: `Vec<LayoutGlyph>` in visual order, `w`, `max_ascent`, `max_descent`, optional `line_height_opt`, and (since 0.19.0) `Vec<DecorationSpan>`. One `ShapeLine` produces N `LayoutLine`s after wrap. See [`architecture.md`](architecture.md).
- **`LayoutGlyph`** — A single positioned glyph: `start, end` (cluster byte range into BufferLine), `font_id, glyph_id, font_size, font_weight`, `x, y, w, x_offset, y_offset`, `level: unicode_bidi::Level` (for caret math), `color_opt`, `metadata`, `cache_key_flags`. See [`architecture.md`](architecture.md).
- **`Attrs<'a>`** — Per-span styling: `family, weight, style, stretch, color, font_features, letter_spacing_opt, text_decoration, metadata`. Borrows its font-name string (lifetime tax — see [`critiques.md` § Attrs lifetimes](critiques.md#attrs-lifetimes)).
- **`AttrsList`** — Interval-map (`rangemap`) of byte-range → `Attrs` for a single `BufferLine`. Where rich-text-style spans live.
- **`Wrap`** — Layout wrap policy: `None | Glyph | Word | WordOrGlyph`. See [`capabilities.md` § Wrapping](capabilities.md#wrapping--breaking).
- **`Align`** — Horizontal text alignment: `Left | Right | Center | Justified | End`. `Justified` is inter-word only. See [`capabilities.md`](capabilities.md).
- **`Ellipsize`** — Truncation policy (added 0.18.0): `Start | Middle | End` (plus `None`). See [`critiques.md` § text-overflow](critiques.md#text-overflow-ellipsis).
- **`Editor<'buffer>`** — Wraps a mutable borrow of `Buffer`, tracks `Cursor`, `Selection`, in-progress `Change`. **Does not own undo history** in plain form. See [`editing.md`](editing.md).
- **`ViEditor`** — `Editor` + `modit` (vi-keys parser) + `syntect` (syntax highlighting) + `cosmic_undo_2::Commands<Change>` (the actual undo stack). Gated behind the `vi` feature. The only built-in editor with an undo stack. See [`editing.md`](editing.md).
- **`Action`** — Editor command verb set: `Motion(Motion)`, `Escape`, `Insert(char)`, `Enter`, `Backspace`, `Delete`, `Indent`, `Unindent`, `Click { x, y }`, `DoubleClick { x, y }`, `TripleClick { x, y }`, `Drag { x, y }`, `Scroll { lines }`. Embedders translate OS input → `Action`. See [`editing.md` § The Action enum](editing.md#the-action-enum).
- **`Change`** — A recordable edit. `Change { items: Vec<ChangeItem> }`, where each `ChangeItem { start: Cursor, end: Cursor, text: String, insert: bool }`. Reversing `insert` gives the inverse. The substrate the optional undo stack consumes; plain `Editor` emits these for embedders to push on their own stack. See [`editing.md` § Undo / redo](editing.md#undo--redo-the-split).
- **`SwashCache`** — `HashMap<CacheKey, Option<SwashImage>>` glyph-rasterization cache. Monotonically growing — no built-in LRU. Lives alongside `FontSystem` in the embedder. See [`architecture.md` § Glyph cache](architecture.md#glyph-cache-and-eviction).

## Companion crates (substrate)

- **`harfrust`** — Pure-Rust HarfBuzz port (aligned with HarfBuzz v13.0.0). **The shaper since cosmic-text 0.15.0** (PR #417, 2025-09-09). Maintained by the official `harfbuzz` GitHub organization. Started as a fork of `rustybuzz`, ported its font backend from `ttf-parser` to `read-fonts`. Per its own README, "less than 25% slower than HarfBuzz on most common fonts"; gaps include no Arabic fallback shaper and no boring-expansion-spec support. See [`shaping.md`](shaping.md).
- **`rustybuzz`** — The previous shaper. **Replaced in cosmic-text 0.15.0.** Maintained by `RazrFalcon`; HarfBuzz port. Still in the wider Rust text ecosystem but no longer in cosmic-text. Anywhere "rustybuzz" appears in cosmic-text discussions older than 0.15.0 should be read as "harfrust" for 0.15+. See [`history.md` § Version timeline](history.md).
- **`swash`** — Pure-Rust font format reader and rasterizer by `dfrg`. Used by cosmic-text **only for rasterization** (outlines + COLR/CPAL + sbix + CBDT/CBLC). Not for shaping, font fallback, or font discovery. Color emoji rasterization lives in `swash::scale`. Upstream development appears inactive per critiques.md analysis of the COLRv1 issue. See [`architecture.md`](architecture.md), [`critiques.md` § COLRv1](critiques.md#colrv1-color-fonts).
- **`skrifa`** — Google Fonts' pure-Rust font-data crate. Wraps `read-fonts` (also from `googlefonts/fontations`). Replaces what was previously routed through swash's introspection side. See [`architecture.md`](architecture.md).
- **`fontdb`** — Pure-Rust font discovery, by `RazrFalcon`. Platform-specific enumeration (Windows registry, CoreText, fontconfig on Linux when `fontconfig` feature is on, plain directory scan otherwise). Optional `memmap` feature for read-via-mmap. See [`architecture.md` § Font discovery](architecture.md#font-discovery-font).
- **`unicode-bidi`** — Servo's reference UAX #9 implementation. cosmic-text uses `0.3.18` with the `hardcoded-data` feature (UAX #9 tables compiled in). Re-exports `Level` through `LayoutGlyph::level`. See [`bidi.md`](bidi.md).
- **`unicode-linebreak`** — UAX #14 line-break opportunity finder. `0.1.5`. Dictionary-less; no Thai / Lao / Khmer / Burmese word dictionary. See [`shaping.md`](shaping.md), [`capabilities.md` § Wrapping](capabilities.md#wrapping--breaking).
- **`unicode-script`** — Script-property lookup. `0.5.8`.
- **`unicode-segmentation`** — Grapheme / word / sentence iterators (Servo). `1.12.0`. cosmic-text uses it for word boundaries inside shape spans and for grapheme-cluster-correct delete.
- **`cosmic_undo_2`** — System76's undo crate. `0.2.0`. ~651 LOC. Used only by `ViEditor` (behind the `vi` feature). See [`editing.md` § Undo / redo](editing.md#undo--redo-the-split).
- **`modit`** — System76's vi-keys parser. `0.1.5`. Used only by `ViEditor`.
- **`syntect`** — Syntax highlighting (Sublime Text grammars). `5.3.0`. Used only by `ViEditor`. Optionally pulls in `onig` C bindings; cosmic-text's default config does not.

## Standards and formats

- **BiDi** — Bidirectional algorithm per **UAX #9**. The Unicode Standard Annex that defines how mixed LTR/RTL text resolves into level runs and reorders for display. https://unicode.org/reports/tr9/.
- **UAX #9** — Unicode Standard Annex #9, the BiDi algorithm spec.
- **UAX #14** — Unicode Standard Annex #14, line-breaking algorithm. Used via `unicode-linebreak`.
- **UAX #29** — Unicode Standard Annex #29, grapheme / word / sentence segmentation. Used via `unicode-segmentation`.
- **UTS #51** — Unicode Technical Standard #51, Unicode Emoji. Defines ZWJ sequences, variation selectors, regional-indicator pairs, skin-tone modifiers. Handled by font GSUB rules applied in `harfrust`.
- **COLR / CPAL** — OpenType color-glyph tables. **COLRv0** (Microsoft, original Twemoji): per-glyph layered colored glyphs. **Supported** by swash. **COLRv1**: gradients, transformations, sub-glyph composition. **NOT supported** by swash, breaks Fedora 43+ Noto Color Emoji. See [`critiques.md` § COLRv1](critiques.md#colrv1-color-fonts), issue [#446](https://github.com/pop-os/cosmic-text/issues/446).
- **COLRv1** — The newer COLR variant. **The cosmic-text gap.** Upstream-blocked on swash.
- **sbix** — Apple's bitmap glyph table (Apple Color Emoji). Supported by swash.
- **CBDT / CBLC** — Google's bitmap glyph tables (legacy Noto Color Emoji, Android emoji). Supported by swash.
- **GSUB** — OpenType glyph substitution table. Drives ligatures, contextual forms (Arabic joining, Indic reordering), variation selectors. Applied by `harfrust`.
- **GPOS** — OpenType glyph positioning table. Drives kerning (pair, cursive, mark-to-base, mark-to-mark), anchor positioning. Applied by `harfrust`.

## Adjacent / contrast

- **Parley** — Linebender's competing modern Rust text layout crate. Current 0.7.0. Uses `swash` for rasterization but routes color emoji through `vello` (which supports COLRv1). Has its own editor surface (`PlainEditor`). **Bevy 0.19-dev migrated `bevy_text` from cosmic-text to Parley + swash** (issue [#21765](https://github.com/bevyengine/bevy/issues/21765), 2025-11-06). Used by Floem 0.7.0; Buiy chose cosmic-text over Parley per the foundation spec, diverging from bevy_ui post-0.19. See [`ecosystem.md` § Parley](ecosystem.md#parley-linebender).
- **`ab_glyph` / `glyph_brush`** — `alexheretic`'s legacy glyph-cache + bitmap rasterizer. **No shaping. No BiDi. No fallback.** Bevy used `ab_glyph` until 0.14 (when it migrated to cosmic-text). Still adequate for simple Latin-only games; capped at the simple-script ceiling. See [`history.md`](history.md), [`ecosystem.md`](ecosystem.md).

## Sources

- cosmic-text Cargo.toml — https://github.com/pop-os/cosmic-text/blob/main/Cargo.toml
- cosmic-text source — https://github.com/pop-os/cosmic-text/tree/main/src
- All sibling files in this folder.
