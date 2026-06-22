**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — capability matrix and gap analysis from Buiy's perspective

## Reading guide

This is the can-do / can't-do file. Each row maps a Buiy text-spec feature (from `docs/specs/2026-05-07-buiy-foundation/text.md`) to cosmic-text 0.19.0's status and to Buiy's plausible response — **inherit** (use as-is), **build above** (wrap or extend), **fork+patch** (upstream PR), or **out** (defer or drop). Stance is provisional; the per-subsystem spec (`buiy-text-rendering-design`) is the place to commit.

Verified against `Cargo.toml`, `CHANGELOG.md`, and the `src/` tree on `pop-os/cosmic-text` main HEAD.

## What cosmic-text CAN do

| Capability | Status | Notes |
|---|---|---|
| Multi-line layout with wrap | yes | `Wrap::None / Glyph / Word / WordOrGlyph` (`src/layout.rs`). |
| Text alignment | yes | `Align::Left / Right / Center / Justified / End` (`src/layout.rs`). |
| Justify | yes (basic) | `Align::Justified` distributes inter-word space; no `text-justify` variant control (see gap below). |
| BiDi + RTL | yes | UAX #9 via `unicode-bidi 0.3.18` (see [bidi.md](bidi.md)). |
| Arabic / Hebrew / Indic / Thai shaping | yes | Via harfrust 0.5.0, font-dependent (see [shaping.md](shaping.md)). |
| CJK shaping (horizontal only) | yes | Simple per-codepoint shaping; vertical not supported. |
| Color emoji | partial | COLR/CPAL + sbix + CBDT/CBLC via swash. **SVG color glyphs not listed in swash README — verified absent.** |
| Subpixel positioning (horizontal) | yes | 4 sub-pixel bins per axis (`SubpixelBin::Zero/One/Two/Three` in `glyph_cache.rs`). |
| Font hinting | yes | `Hinting` enum on `Buffer`; variable fonts get hinting suppression. |
| Font fallback (per-cluster) | yes | `FontFallbackIter`, script + locale + platform-specific (`font/fallback/{macos,unix,windows,other}.rs`). |
| Per-span attributes | yes | `AttrsList` carries `Attrs` (family, weight, style, stretch, color, font_features, letter_spacing, text_decoration, metadata) per byte range. |
| Per-glyph color override | yes | `LayoutGlyph::color_opt`. |
| Text decorations | yes | `TextDecoration { underline: UnderlineStyle, underline_color_opt, strikethrough, strikethrough_color_opt, overline, overline_color_opt }` added/expanded in 0.19.0 per `CHANGELOG`. |
| Cursor positioning + selection (multi-line) | yes | `Cursor { line, index, affinity }`, `Selection`. |
| Ellipsization | yes | `Ellipsize` enum; start/middle/end variants added in 0.18.0. |
| Tab handling | yes | `Buffer::tab_width: u16`. |
| Variable-font axis evaluation | yes | Via harfrust's `fvar`/`gvar` support; matching repaired in 0.19.0. |
| Font features (CSS `font-feature-settings`) | yes | `Attrs::font_features: FontFeatures` maps to OpenType feature tags. |
| Letter spacing | yes | `Attrs::letter_spacing_opt`. |
| Line height override | yes | Per-`Metrics` and per-`LayoutLine::line_height_opt`. |
| Monospace alignment | yes | `Buffer::monospace_width: Option<f32>` forces all glyphs to a column. |
| no_std support | partial | `no_std` feature pulls in `hashbrown`/`libm`/`core_maths`; loses fontconfig + mmap + sys-locale. |
| Vi-mode editor + syntect highlighting | yes (opt-in) | `vi` feature; used by COSMIC terminal. Not generally relevant to Buiy. |

## What cosmic-text CANNOT do (from Buiy's perspective)

Each row cites which Buiy `text.md` line it maps to where applicable. The Buiy text spec lives at `docs/specs/2026-05-07-buiy-foundation/text.md`.

### Layout / writing modes

| Feature | text.md tier | Status | Buiy stance |
|---|---|---|---|
| Vertical writing modes (`vertical-rl`, `vertical-lr`, `sideways-rl`, `sideways-lr`) | **E** | **Not implemented.** No `WritingMode` enum in `src/`; all layout assumes horizontal-LTR/RTL. | **Build above or out.** True vertical writing requires the layout pass to walk a y-major axis; this is a large rewrite, not a wrapping. If Buiy needs vertical-CJK or sideways labels, the realistic options are (a) defer to vertical-text being **E** in text.md, (b) glue in a separate engine for vertical contexts, or (c) upstream a writing-mode axis to cosmic-text. |
| `text-orientation: mixed | upright | sideways` | **E** | Not implemented (depends on writing-mode). | **Defer with vertical-writing.** |
| Ruby / furigana | **E** | Not implemented. No ruby primitives in `src/`. | **Build above.** A Buiy-layer composes two cosmic-text `Buffer`s (base + ruby) and aligns them; this is workable for short annotations, painful for long ones. |
| `text-align-last` | **C** | Not implemented as a separate setting; the last line of a justified block aligns per `Align::Justified`'s defaults. | **Build above.** |
| `text-justify` variants (`inter-word | inter-character | distribute`) | **E** | Not implemented. `Align::Justified` is the only knob; the distribution algorithm is inter-word. | **Build above or out.** Inter-character justify is required for proper CJK; if Buiy ships CJK as **C**, this graduates to required. |
| `text-indent` | **C** | Not implemented. | **Build above.** Insert a leading horizontal offset at layout time per BufferLine; straightforward wrapper. |
| `text-overflow: ellipsis` | **C** | **Yes** (via `Ellipsize`), including custom `<string>` per 0.18.0 if the API permits — verify exact API shape. | **Inherit.** |
| Multi-line clamp (`line-clamp`) | **C** | Not as a dedicated API. cosmic-text exposes `height_opt` on `Buffer`; clipping happens at render. | **Build above.** Compute max-lines, slice the layout output, add ellipsis to the last visible line. |
| `letter-spacing` | **C** | **Yes** (`Attrs::letter_spacing_opt: Option<LetterSpacing>`). | **Inherit.** |
| `word-spacing` | **C** | Not a dedicated knob; the closest analog is increasing the advance of `U+0020` glyphs, which is not exposed. | **Build above.** Walk LayoutGlyph stream and adjust word-break advance; or fork+patch to add `Attrs::word_spacing_opt`. |
| `tab-size` | **C** | **Yes** (`Buffer::tab_width: u16`). Note: width in *glyphs*, not in `ch` units like CSS — verify the unit semantics. | **Inherit.** |
| `vertical-align` | **C** | Partially — line metrics expose ascent/descent; per-glyph baseline alignment for super/subscript is the embedder's job. | **Build above.** |

### Wrapping & breaking

| Feature | text.md tier | Status | Buiy stance |
|---|---|---|---|
| `white-space` variants (`pre`, `pre-wrap`, `pre-line`, `nowrap`, `break-spaces`) | **F** | Partial: `Wrap::None` ≈ `nowrap`. Pre-wrap (preserving runs of spaces) requires the embedder to *not* pre-collapse whitespace before calling `set_text`. | **Build above.** Buiy's text component normalizes input per the `white-space` mode, then sets the appropriate `Wrap`. |
| `word-break: break-all` | **C** | Closest match: `Wrap::Glyph`. Acceptable mapping. | **Inherit.** |
| `overflow-wrap: anywhere | break-word` | **C** | Closest match: `Wrap::WordOrGlyph` (prefer word boundaries, fall back to glyph boundaries when needed). Acceptable. | **Inherit.** |
| `hyphens: auto` | **C** | **Not implemented.** `unicode-linebreak` provides UAX #14 break opportunities, not hyphenation dictionaries. | **Build above.** Wrap a hyphenation crate (e.g., `hyphenation`) and inject soft-hyphens at break candidates before `set_text`. |
| `line-break: strict | normal | loose` (CJK) | **C** | Not implemented. | **Build above or out.** |
| Thai dictionary line-breaking | (Thai-as-**C**) | **Not implemented.** `unicode-linebreak` does dictionary-less UAX #14 only. | **Build above.** Layer `icu_segmenter` for Thai/Lao/Khmer/Burmese paragraphs. |
| `hyphenate-character`, `hyphenate-limit-chars` | **E** | Not implemented (no hyphenation in the first place). | **Defer with hyphens.** |

### Decoration

| Feature | text.md tier | Status | Buiy stance |
|---|---|---|---|
| `text-decoration-line` (underline, overline, line-through) | **F** | **Yes** (`TextDecoration` struct in `Attrs`, decorations carried on `LayoutLine::decorations: Vec<DecorationSpan>` per 0.19.0). | **Inherit.** |
| `text-decoration-color` (per-decoration) | **F** | **Yes** (`underline_color_opt`, `strikethrough_color_opt`, `overline_color_opt`). | **Inherit.** |
| `text-decoration-style: wavy | dotted | dashed | double` | **C** | Partial — `UnderlineStyle` enum exists; variant list needs verification against 0.19.0 source. | **Inherit or build above** depending on which variants land. |
| `text-decoration-thickness` | **C** | Not exposed as a per-decoration knob. | **Build above.** The renderer (embedder) draws decoration lines; Buiy can compute thickness from a Buiy-level token. |
| `text-underline-offset`, `text-underline-position` | **C** | Not exposed. | **Build above.** Same — drawing is at the embedder. |
| `text-decoration-skip-ink` | **C** | Not implemented (would need per-glyph ink-box query for skipping). | **Build above** (later) or **out** at v1. |
| `text-emphasis-*` (CJK) | **E** | Not implemented. | **Out at v1; defer with CJK polish.** |
| `text-transform` | **C** | **Not implemented.** Casing transforms are the embedder's job — preprocess the string before `set_text`. | **Build above.** Locale-aware uppercase/lowercase needs ICU (Turkish dotted-I etc.). |
| `hanging-punctuation` | **E** | Not implemented. | **Out at v1.** |
| `text-box-trim` / `text-box-edge` (leading trim) | **E** | Not implemented (line metrics are font-leading-default). | **Build above.** Adjust the line box top/bottom from ascent/descent metrics on `LayoutLine`. |
| `text-spacing-trim` (CJK) | **E** | Not implemented. | **Out at v1.** |

### Selection and editing

| Feature | text.md tier | Status | Buiy stance |
|---|---|---|---|
| Selection rendering (visual rects for mixed-direction) | **F** | **Geometries only.** cosmic-text gives `LayoutGlyph::level` and positions; the multi-rect selection algorithm is the embedder's. | **Build above.** Standard sweep — walk visible LayoutGlyphs inside the logical range, union rects per level run. **Correction (text campaign T9, 2026-06-11):** the build-the-sweep-yourself stance is superseded — 0.19 ships per-run `LayoutRun::highlight(start, end)` + `Editor::selection_bounds()`, and Buiy landed on `highlight` ([decoration-and-paint.md § 5.1](../../specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md#51-rectangles-via-layoutrunhighlight)); see [text verification.md § 5](../../specs/2026-06-09-buiy-text-rendering-design/verification.md#5-prior-art-errata-ledger). |
| Caret blink with reduced-motion | **F** | Out of scope for cosmic-text. | **Build above.** Buiy timing layer respects `prefers-reduced-motion`. |
| `caret-color` | **F** | Out of scope. | **Build above.** Renderer draws the caret. |
| BiDi visual caret traversal | **F** | **Logical only** at editor layer. | **Build above.** See [bidi.md](bidi.md) and Agent B's `editing.md`. |
| IME composition (preedit underline, popup positioning) | **F** | **Embedder's responsibility.** cosmic-text exposes editing primitives; winit / Bevy IME plumbing carries the composition events. | **Build above.** This is `buiy-text-editing-design`'s core problem. |
| Word-segmented navigation per locale | **C** | Word boundaries via `unicode-segmentation` (UAX #29); locale-aware variants (Japanese morphology, etc.) require ICU. | **Build above.** Wrap `icu_segmenter` if Buiy commits to locale-aware word-nav. |
| Cut / copy / paste, OS clipboard | **F** | Out of scope for cosmic-text. | **Build above.** `buiy-clipboard-and-os-integration-design` owns it. |
| Undo/redo with composition grouping | **F** | `cosmic_undo_2` is available behind the `vi` feature, but the generic `Editor` does not bundle undo. | **Build above.** |
| Spellcheck integration | **C** | Out of scope. | **Build above.** OS bridge per `buiy-clipboard-and-os-integration-design`. |

### Font model

| Feature | text.md tier | Status | Buiy stance |
|---|---|---|---|
| `font-family` stack with fallback | **F** | **Yes.** | **Inherit.** |
| Generic families (`serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`) | **C** | **Yes** via `FamilyOwned` enum. | **Inherit.** |
| Extra generic families (`system-ui`, `ui-serif`, `ui-sans-serif`, `ui-monospace`, `ui-rounded`, `emoji`, `math`) | **C** | **Not in `FamilyOwned`.** | **Fork+patch or build above.** Map `system-ui` etc. to platform-specific concrete families in Buiy's font-resolution layer. |
| `font-size` | **F** | **Yes** (`Metrics::font_size`). | **Inherit.** |
| `font-weight` | **F** | **Yes** (`Attrs::weight: Weight`). | **Inherit.** |
| `font-style: italic | oblique <angle>` | **C** | `Style::Italic` works; `oblique <angle>` requires variable-font `slnt` axis manipulation. | **Inherit (italic); build above (oblique-angle).** |
| `font-stretch` / `font-width` | **C** | `Attrs::stretch: Stretch`. | **Inherit.** |
| `font-variant-*` | **C** | Partially via `font_features` (CSS feature settings are the underlying mechanism for most `font-variant-*` longhands). | **Build above.** Buiy's CSS-shaped surface translates `font-variant-numeric: tabular-nums` etc. to feature-tag arrays. |
| `font-feature-settings` (raw OpenType) | **C** | **Yes** (`Attrs::font_features`). | **Inherit.** |
| `font-variation-settings` (variable-font axes) | **C** | Variable fonts work via harfrust; explicit axis-value settings on `Attrs` — **verify exact API**; may need a small wrapper. | **Build above.** |
| `font-optical-sizing` | **E** | Inherited via variable-font `opsz` axis evaluation; no explicit on/off knob. | **Build above** (optional). |
| `font-kerning` | **C** | Kerning is on by default via GPOS; opt-out requires disabling the `kern` feature via `font_features`. | **Inherit.** |
| `font-synthesis` | **C** | **Not implemented.** No synthetic bold or oblique. | **Out or fork+patch.** Synthetic styles are a small wrapper over swash's transform matrix; doable above. |
| `font-language-override` | **E** | Not exposed as a per-attrs field; the locale is global on `FontSystem`. | **Build above.** Wrap per-span attrs to pass language tag through to harfrust's `ShapePlan`. |
| `font-size-adjust` | **E** | Not implemented. | **Build above.** Trivial: scale based on font's x-height query. |
| `font-palette` + `@font-palette-values` | **E** | Not implemented as a top-level API; swash supports COLR palette indexing. | **Fork+patch** to expose palette selection per-attrs. |
| Metric overrides (`size-adjust`, `ascent-override`, `descent-override`, `line-gap-override`) | **C** | Not implemented. | **Build above.** Apply at the Metrics computation. |
| Web-font async loading (FOUT / FOIT semantics) | n/a | Not applicable directly (no network). cosmic-text reads sync from `fontdb`. | **Build above.** Buiy's asset pipeline loads fonts via Bevy's `AssetServer`; while-loading state needs a Buiy-level fallback. |

### Pseudo-elements

| Feature | text.md tier | Status | Buiy stance |
|---|---|---|---|
| `::selection` styling | **F** | Out of scope (selection rendering is the embedder's). | **Build above.** |
| `::placeholder` | **F** | Out of scope. | **Build above.** |
| `::first-letter` / `::first-line` | **E** | Out of scope. | **Build above** (later) or **out**. |
| `::spelling-error` / `::grammar-error` | **E** | Out of scope. | **Build above** with spellchecker integration or **out**. |

## Summary

cosmic-text gives Buiy: full shaping (harfrust = HarfBuzz v13), BiDi (unicode-bidi UAX #9), per-cluster font fallback, color emoji (COLR/CPAL + bitmap formats), subpixel positioning, glyph caching, multi-line layout with wrap and basic justify, text decorations, cursor/selection geometry, and lazy per-line cache rebuild.

cosmic-text does **not** give Buiy: vertical writing, ruby, hyphenation, locale-aware line break, `text-transform`, `font-synthesis`, `font-palette`, decoration thickness/offset, selection painting, IME composition wiring, undo/redo bundling, clipboard, or spellcheck. Most of these are correctly out of cosmic-text's scope and belong to Buiy's text-component layer.

The one row that should sting most is **vertical writing modes** — the layout pass assumes horizontal axis end-to-end, and adding vertical-CJK as more than a paper feature is a real engineering investment. text.md tiers it **E** today, which is the right call.

## Cross-links

- [architecture.md](architecture.md), [shaping.md](shaping.md), [bidi.md](bidi.md).
- Agent B's `editing.md` (Cursor / Editor model), `integration.md` (Bevy + Iced wiring), `open-problems.md` (where the gaps above get queued).
- Buiy text spec — `docs/specs/2026-05-07-buiy-foundation/text.md`.
- Buiy text-rendering sub-spec (planned) — `buiy-text-rendering-design`.
- Buiy text-editing sub-spec (planned) — `buiy-text-editing-design`.

## Sources

- `Cargo.toml`, `CHANGELOG.md`, and all of `src/` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text
- crates.io — https://crates.io/crates/cosmic-text (0.19.0, 2026-04-22)
- harfrust — https://github.com/harfbuzz/harfrust
- swash — https://github.com/dfrg/swash, https://crates.io/crates/swash
- unicode-bidi — https://github.com/servo/unicode-bidi
- unicode-linebreak — https://crates.io/crates/unicode-linebreak
- Buiy text spec — `docs/specs/2026-05-07-buiy-foundation/text.md`
