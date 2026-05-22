**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — text shaping via harfrust, complex scripts, font features, fallback

## Substrate correction

cosmic-text 0.19.0's shaper is **harfrust 0.5.0** (verified in `Cargo.toml`). harfrust started as a fork of rustybuzz and currently aligns with HarfBuzz v13.0.0; it is maintained by the official `harfbuzz` GitHub organization, not by the rustybuzz author. The migration from rustybuzz to harfrust landed in the 0.17.x/0.18.x line — anything older in the wild still says "rustybuzz." Anywhere in this corpus or in the Buiy text spec where "rustybuzz" appears should be read as "harfrust" for 0.19+.

Per harfrust's own README, the implementation is "less than 25% slower than HarfBuzz on most common fonts," with two explicit gaps: **no Arabic fallback shaper**, and "experimental HarfBuzz features like most of the boring-expansion-spec are not supported yet."

## How cosmic-text drives harfrust

The interesting work lives in `src/shape.rs`. Per-paragraph:

1. **BiDi resolve.** `unicode_bidi::BidiInfo::new(text, base_dir)` + `Paragraph::new(...)` produces level runs. Each contiguous run of equal embedding level becomes a `ShapeSpan` with a single `Direction` (LTR or RTL).
2. **Word-split.** Inside each span, `unicode-segmentation::UnicodeSegmentation::split_word_bound_indices` produces `ShapeWord`s. (Words are also the natural unit for wrap and double-click selection — see [editing.md, Agent B].)
3. **Script-tag per word.** `unicode_script::Script::from(c)` is queried per char to pick a script tag for harfrust's `ShapePlan`.
4. **Per-font shape.** For each (font candidate, word) pair, `shape_run` builds (or fetches from cache) a harfrust `ShapePlan`, fills a harfrust `Buffer`, calls `shape(...)`, and reads `GlyphInfo` + `GlyphPosition` into `ShapeGlyph`s. The plan cache is keyed by `(font_id, script, direction, language)` and lives on the `FontSystem`.
5. **Per-glyph fallback.** Any `glyph_id == 0` triggers a re-shape of the affected cluster range against the next font in `FontFallbackIter`. Fallback granularity is the *cluster*, not the run — so a single Devanagari word inside an English paragraph picks up a Devanagari font for those clusters and stays on the English font everywhere else.
6. **Cache.** With `shape-run-cache` enabled, the (text, font, attrs) → `Vec<ShapeGlyph>` mapping is memoized; rebuilding a `BufferLine` after a no-op width change reuses prior shaping.

## HarfBuzz feature parity (harfrust)

harfrust is a port of the shaping core, not a wrapper. Features inherited from HarfBuzz upstream (verified via harfrust's own claim of HarfBuzz v13.0.0 alignment):

- **GSUB** — glyph substitution: ligatures, contextual substitution, language-specific forms.
- **GPOS** — glyph positioning: kerning (pair, cursive, mark-to-base, mark-to-mark), anchor positioning.
- **Contextual shaping** — Arabic joining forms (isolated / initial / medial / final), Indic reordering, Hebrew final forms, etc.
- **Variation tables** — `fvar`, `gvar`, `HVAR`, `MVAR` for variable-font axis evaluation.
- **morx / kerx** — Apple Advanced Typography shaping (per upstream HarfBuzz parity; verify if a specific font hits issues).

Known gaps vs HarfBuzz (per harfrust README at time of writing):

- **No Arabic fallback shaper.** HarfBuzz's `fallback_shape` path that synthesizes basic Arabic shaping for fonts without proper OpenType tables is not yet implemented. Fonts must ship correct GSUB/GPOS to render Arabic correctly.
- **boring-expansion-spec** features (variable-width spaces, AAT-style justification) are not supported.

cosmic-text exposes a per-`Attrs` `font_features: FontFeatures` field (`src/attrs.rs`), which is the analog of CSS `font-feature-settings`. Buiy can map web `font-feature-settings: "liga" 1, "kern" 1` directly onto this.

## Complex script support

The script-tagging + per-cluster fallback mechanism is the same for every script; the per-script story is mostly "does a font with proper OpenType tables exist on the platform, and does harfrust's port of the relevant HarfBuzz shaper still pass conformance."

**Arabic (RTL, cursive joining).** Cursive joining works when the font has proper GSUB rules (Amiri, Noto Sans Arabic, Segoe UI). With a font that has only nominal glyphs, joined forms will not render correctly — there is no synthesized-joining fallback. This is the one gap most likely to surprise a Buiy app shipping with an incomplete bundled font.

**Hebrew (RTL).** No cursive joining concern; final forms (`ך ם ן ף ץ`) are codepoint-distinct. Works on any font with the relevant glyphs.

**Devanagari + Indic family (Bengali, Gurmukhi, Gujarati, Oriya, Tamil, Telugu, Kannada, Malayalam, Sinhala).** Indic syllable reordering, conjunct formation, vowel reorder around base consonants — all driven by GSUB. Works against Noto Sans Devanagari etc. The Indic shaper is one of the more complex HarfBuzz subsystems; bugs that exist upstream tend to mirror into harfrust.

**Thai / Lao / Khmer / Myanmar.** Shaping works against proper fonts. **Line-breaking** for these scripts (no inter-word spaces) is handled separately by `unicode-linebreak 0.1.5` (UAX #14), which implements dictionary-less line breaking. Dictionary-based Thai line-breaking (which the web platform uses via ICU's `BreakIterator`) is **not** present — Thai text will break at any allowed UAX #14 opportunity, not at lexical word boundaries. Buiy users targeting Thai-locale parity will need to layer a dictionary breaker (e.g., wrap `icu_segmenter`) above cosmic-text.

**CJK (Han, Hiragana, Katakana, Hangul).** Ideographs are simple to shape (one codepoint, one glyph, no contextual substitution typically), but **vertical writing modes** are not implemented in cosmic-text (see [capabilities.md](capabilities.md)). Half-width / full-width glyph variants come from the font and apply via standard GSUB if requested. No support for CJK-specific layout features like `text-emphasis` (ruby annotation marks) or `text-spacing-trim`.

**Emoji (UTS #51).** ZWJ sequences (`👨‍👩‍👧`), variation selectors (`☃︎` text vs `☃️` emoji), regional indicator pairs (`🇯🇵`), skin-tone modifiers — all rely on the font's GSUB rules being applied. The font ships the rules; harfrust applies them; swash rasterizes the resulting glyphs (COLR/CPAL/sbix/CBDT). Variation-selector-15 vs -16 presentation forms work *if* the font provides separate text + emoji glyphs and the GSUB rules to route them; many monochrome fonts do not.

## Per-run font fallback

`FontFallbackIter` (`src/font/fallback/mod.rs`) produces the candidate sequence:

1. The `Attrs::family` requested (or `FamilyOwned::SansSerif` etc.).
2. Script-specific fallback per `script_fallback(script, locale)` — the platform module (`macos.rs` / `windows.rs` / `unix.rs` / `other.rs`) defines the list.
3. Common locale-independent fallback (e.g., `"Noto Sans"` on Linux).
4. All remaining fonts in the `fontdb::Database`, sorted to prefer the requested weight and (for monospace requests) coverage.

For monospace requests, fonts are sorted by `(font_weight_diff, script_non_matches, font_weight)` — i.e. closest weight first, then most-matching-script, then weight as tiebreaker. For non-monospace, weight must match exactly *unless* the font is variable-weight (in which case the requested weight is interpolated on the `wght` axis).

This algorithm is curated for COSMIC desktop's font set; a Bevy game on a stripped-down embedded Linux without Noto installed may fall through all four steps and end up with whatever bitmap font happens to be in the bundle. Buiy needs to ship its own bundled-fonts story for asset-pipeline parity (see [capabilities.md § What Buiy must build above](capabilities.md)).

## Subpixel positioning + hinting

cosmic-text supports **horizontal subpixel positioning** via swash's "fractional positioning" mode. The `CacheKey` (in `glyph_cache.rs`) carries `x_bin` and `y_bin` of type `SubpixelBin`, which quantizes the sub-pixel offset to 4 bins per axis (0.0, 0.25, 0.5, 0.75). This bounds glyph-cache fan-out at 4× per axis and gives visually smooth positioning at common sizes.

**Hinting** is configurable per-buffer via the `Hinting` enum on `Buffer` (referenced in `buffer.rs`; full variant set lives in another module and isn't dumped here — verify against `src/`). Variable fonts get full grid hint suppression by default since variation interpolation and TrueType hinting interact poorly.

A 0.19.0 fix listed in `CHANGELOG.md` is "font matching for variable fonts" — variable-weight matching had been broken in 0.17/0.18 and was repaired. Buiy variable-font specs (text.md § Variable fonts, tier **C**) should pin against 0.19+.

## What `Shaping` enum controls

The `Shaping` enum (referenced as `Shaping::Advanced` in `set_text`) controls which path runs:

- `Shaping::Basic` — a cheap, non-OpenType path. No GSUB, no ligatures, no contextual forms. Acceptable for Latin-only Latin-script ASCII paths where shaping cost matters and complex script support is not needed.
- `Shaping::Advanced` — the full harfrust path described above. **This is what Buiy must use** for web-platform parity, since `Shaping::Basic` cannot render Arabic, Indic, or any contextual script.

A common cosmic-text user mistake is leaving `Shaping::Basic` (the default in some construction paths) and then reporting "Arabic doesn't work." Buiy's text-component wrapper should pin `Shaping::Advanced` as the default.

## Cross-links

- See [architecture.md](architecture.md) for where shape.rs sits in the pipeline.
- See [bidi.md](bidi.md) for the UAX #9 segmentation that precedes shaping.
- See [capabilities.md](capabilities.md) for what shaping cannot do (vertical writing, ruby, advanced justification variants).
- Editor-side text manipulation (cluster-correct delete, word-nav) is in Agent B's `editing.md`.

## Sources

- `src/shape.rs`, `src/attrs.rs`, `src/font/fallback/mod.rs` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text/tree/main/src
- harfrust README — https://github.com/harfbuzz/harfrust (HarfBuzz v13.0.0 alignment claim; Arabic fallback shaper gap; boring-expansion-spec gap).
- `unicode-script` — https://crates.io/crates/unicode-script
- `unicode-segmentation` — https://crates.io/crates/unicode-segmentation
- `unicode-linebreak` — https://crates.io/crates/unicode-linebreak (UAX #14, no Thai dictionary).
- HarfBuzz upstream features — https://harfbuzz.github.io
- `CHANGELOG.md` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text/blob/main/CHANGELOG.md (0.19.0 variable-font matching fix).
