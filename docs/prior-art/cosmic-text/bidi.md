**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — Unicode BiDi (UAX #9) implementation status and gaps

## Substrate

cosmic-text 0.19.0 depends on **`unicode-bidi 0.3.18`** with the `hardcoded-data` feature, so the UAX #9 character-property tables are compiled into the binary rather than loaded at runtime. `unicode-bidi` is the Servo-stewarded reference implementation of UAX #9 and tracks Unicode versions on the Servo cadence. cosmic-text re-exports `unicode_bidi::Level` through `LayoutGlyph::level`, so the BiDi level survives into the layout output where the embedder needs it for caret math.

The cosmic-text wrapper around `unicode-bidi` lives in `src/bidi_para.rs`. There is no cosmic-text-specific BiDi state machine — segmentation and reordering are delegated to `unicode-bidi`.

## Paragraph-level handling

Each `BufferLine` is one **logical paragraph** for BiDi purposes — `unicode_bidi::BidiInfo::new(text, base_dir)` is called per BufferLine in `shape.rs`. The output gives:

- `paragraphs[].level` — the resolved paragraph embedding level (0 = LTR base, 1 = RTL base).
- `levels[]` — the resolved embedding level per byte index.
- `original_classes[]` — the BiDi class per byte (L, R, AL, EN, ES, AT, ...).

cosmic-text then uses `Paragraph::level_runs(...)` (or equivalent) to walk runs of equal level; each run becomes a `ShapeSpan` with `level.is_rtl()` setting the per-span direction.

**Visual reordering** (UAX #9 rule L1–L4) happens at layout time: the `ShapeLine::layout` pass iterates spans in *visual* order, not logical order, when emitting `LayoutGlyph`s. Each glyph's logical (cluster) position is preserved on `LayoutGlyph::{start, end}`, so the embedder can map a screen point back to a byte offset.

## Base-direction selection

The base direction for a paragraph is picked by `unicode_bidi`'s first-strong-character heuristic by default (UAX #9 rule P2/P3): the first character of strong BiDi type (L, R, or AL) decides the base. If a paragraph contains no strong characters (e.g., only digits and punctuation), it defaults to LTR.

`unicode-bidi`'s API also supports passing an explicit `Direction`. cosmic-text exposes this on the editing layer (the `Buffer` honors a per-direction setting; the exact field name lives in `buffer.rs`'s constructor — verify before use). For Buiy's `dir="auto" | "ltr" | "rtl"` analog, the embedder maps to either the explicit direction or `None` (let the heuristic decide).

**Buiy implication.** The CSS `dir` attribute has three values: `ltr`, `rtl`, `auto`. cosmic-text natively supports `ltr` and `rtl` via explicit direction, and `auto` is the default `unicode-bidi` behavior. The CSS `unicode-bidi: bidi-override | isolate | isolate-override | plaintext` modes are *not* exposed as a single cosmic-text knob — they map to inserting the relevant Unicode formatting characters (`U+202D LRO`, `U+202E RLO`, `U+2068 FSI`, `U+2069 PDI`, etc.) into the text before passing it to cosmic-text. The same is true for `<bdi>` (isolation) and `<bdo>` (override).

## Mirroring (parentheses, brackets)

UAX #9 calls out **mirrored characters** in BiDi class ON (e.g., `(`, `)`, `[`, `]`, `<`, `>`, `«`, `»`) which must render as their Unicode-mirror glyph when the surrounding context is RTL. cosmic-text relies on **the font + harfrust** to do mirroring: most Arabic/Hebrew fonts ship mirrored glyphs and route them through GSUB based on the OpenType `rtlm` feature, which harfrust enables automatically for RTL runs.

If a font lacks proper `rtlm` rules, cosmic-text does **not** synthesize mirroring — the bracket renders unmirrored. This matches HarfBuzz's behavior; the web platform has the same dependency. Buiy can either ship a font that does proper mirroring or accept the small subset of fonts where it does not.

## Override and isolation characters

The Unicode bidi formatting characters fall into two groups, both handled by `unicode-bidi`:

- **Explicit-formatting characters:** `U+202A LRE`, `U+202B RLE`, `U+202D LRO`, `U+202E RLO`, `U+202C PDF` (deprecated style but still valid).
- **Isolates (Unicode 6.3+):** `U+2066 LRI`, `U+2067 RLI`, `U+2068 FSI`, `U+2069 PDI`.
- **Invisible marks:** `U+200E LRM`, `U+200F RLM`, `U+061C ALM`.

`unicode-bidi 0.3.18` implements both groups per UAX #9. cosmic-text passes them through transparently — they affect the BiDi resolution inside `unicode-bidi` and end up as zero-width glyphs in the shape output. No special editor-layer handling beyond cluster-correct delete (the marks are their own clusters).

**Verified absent:** cosmic-text does **not** ship a higher-level API to "insert an isolate around this span." Buiy's `<bdi>` analog must insert the literal isolate characters at span boundaries.

## BiDi-aware caret movement

Caret motion across a BiDi boundary is the editing layer's concern, not the shaping layer's. cosmic-text's `Cursor` carries `{ line, index, affinity }` where `affinity` (Before/After) disambiguates a position at a level boundary (the "split caret" case).

The `Editor`'s `Cursor` position is **logical** (byte index + affinity), but arrow-key motion (`Motion::Left/Right`) steps in **visual order** across BiDi runs. See editing.md for the canonical caret-movement semantics.

See Agent B's `editing.md` for the full caret model, selection rectangles for mixed-direction lines, and the affinity semantics.

## Known BiDi gaps and issues

From the cosmic-text issue tracker and pop-os/cosmic-text discussions (sampled at time of writing — verify with a live search before depending on any specific bug):

- **No visual caret movement out of the box.** The `Editor` is logical-only. RTL-first users on Windows-style platforms get unfamiliar caret behavior. Recurring issue category in the tracker.
- **No paragraph-direction override per BufferLine on the public Buffer API.** You can set a global default direction on the `FontSystem`/`Buffer`, but mixed-direction paragraphs in a single Buffer all share the same default; to force a different direction per paragraph, you embed the explicit-direction formatting characters in the text.
- **Mirroring depends on font cooperation.** Fonts without `rtlm` rules render brackets unmirrored.
- **Number handling in RTL paragraphs.** `unicode-bidi` follows UAX #9 strictly: Arabic-Indic digits in RTL paragraphs render in their natural order; European digits in RTL paragraphs flow LTR per BiDi class EN. Mixed-numeric strings (`"Phone: +1-555-1234"` inside Hebrew) sometimes confuse users; cosmic-text's behavior here is the *correct* UAX #9 behavior, not a bug.
- **Bracket-pair algorithm (UAX #9 N0, paired-bracket resolution).** `unicode-bidi 0.3.x` does implement N0; the cosmic-text level above does not override it. Buiy should not need to do anything special here, but be aware that some older third-party "BiDi reference" implementations skip N0 and produce different results in edge cases.

## Implications for Buiy

Buiy's text spec (`docs/specs/2026-05-07-buiy-foundation/text.md`) lists under **Bidirectional text**:

- **F:** Unicode BiDi (UAX #9), implicit — **inherited**, cosmic-text + unicode-bidi handle it.
- **F:** `dir` analogue per text-bearing component — **inheritable** via `unicode-bidi`'s explicit-direction API; Buiy wires it through the text component.
- **C:** `bdo` / `bdi` analogues, `unicode-bidi` — **build above:** Buiy must insert the Unicode formatting characters at span boundaries for these to work.
- **F:** BiDi caret traversal per UAX #9 (in text.md § 3.5 caret) — **partial inherit:** logical caret traversal is free; visual caret traversal needs a Buiy layer on top of `LayoutGlyph::level`.
- **F:** Visual selection rectangles (correct for mixed-direction lines) — **inherited:** layout output already has the per-glyph levels and positions; building a multi-rect selection from a logical range is a straightforward sweep, but the *helper* is not in cosmic-text — Buiy provides it.
- **E:** Vertical orientation (`text-orientation`) — **build above or out-of-scope:** see [capabilities.md](capabilities.md).

The decision Buiy faces: how much "visual mode" vs "logical mode" caret behavior to expose. The OS convention varies; the safest Buiy contract is logical-first with visual as a per-app toggle.

## Cross-links

- [architecture.md](architecture.md) — where `bidi_para.rs` sits in the pipeline.
- [shaping.md](shaping.md) — what happens to each BiDi-segmented run after `unicode-bidi`.
- [capabilities.md](capabilities.md) — what BiDi-related CSS features are not implemented.
- Agent B's `editing.md` — `Cursor` and `Editor` BiDi traversal.

## Sources

- `unicode-bidi 0.3.18` — https://github.com/servo/unicode-bidi
- `src/bidi_para.rs`, `src/shape.rs` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text/tree/main/src
- UAX #9 (Unicode Bidirectional Algorithm) — https://unicode.org/reports/tr9/
- `Cargo.toml` at `pop-os/cosmic-text` HEAD — https://github.com/pop-os/cosmic-text/blob/main/Cargo.toml
