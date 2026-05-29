**Date:** 2026-05-22
**Status:** active
**Subject:** egui — the text pipeline, the 0.34.0 skrifa/vello_cpu switch, and the limits vs cosmic-text

# Text rendering

This file inventories egui's text pipeline — what it ships, what shifted in 0.34.0, what it doesn't do, and how it compares to cosmic-text (the text shaper Buiy commits to in [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)).

## The text stack as of 0.34.x

egui's text pipeline runs in `epaint` (the egui-internal 2D graphics + tessellation crate). The end-to-end flow:

1. **Font loading** — `egui::FontDefinitions` registers font files (TTF / OTF) and assigns them to font families (`FontFamily::Proportional`, `FontFamily::Monospace`, custom).
2. **Style mapping** — `Style::text_styles` maps named `TextStyle` keys (`Body`, `Heading`, `Monospace`, `Button`, `Small`, `Name(custom)`) to `FontId { size, family }`.
3. **Layout** — when a label is emitted, `epaint::text::Galley` is constructed by walking the input text, computing line breaks, justification, and per-glyph positioning.
4. **Glyph rasterization** — for each unique `(font, size, glyph_id)`, the rasterizer renders the glyph into the font atlas (an `Image` texture).
5. **Tessellation** — each glyph is emitted as a textured quad into the per-frame `Vec<ClippedShape>`.
6. **Render** — the host backend draws the tessellated triangles, sampling from the font-atlas texture.

This pipeline runs every frame, but glyph rasterization is cached in the atlas across frames — a glyph that's been rendered once isn't re-rasterized unless evicted.

## The 0.34.0 switch: `ab_glyph` → `skrifa` + `vello_cpu`

The most significant recent change. Through 0.33.x and earlier, egui used:

- **`ab_glyph`** — Alex Butler's pure-Rust font parser. Reads TTF/OTF, extracts glyph outlines.
- A custom rasterizer for converting outlines to alpha masks.

0.34.0 (2026-03-26) replaced this stack with:

- **`skrifa`** — Google Fonts' Rust font-parsing library, derived from the same internals as Chrome's text engine and the Vello renderer. More complete OpenType support than `ab_glyph`.
- **`vello_cpu`** — the CPU-side rasterizer from the Vello (GPU-accelerated 2D renderer) project. Higher-quality glyph rasterization with proper hinting.

The user-visible changes from the swap, per the 0.34.0 CHANGELOG:

- **Font hinting** — glyph stems align to pixel grid, improving small-text legibility on non-Retina displays.
- **Variable font support** — variable-axis fonts (weight, width, slant, optical-size axes) are now parsed correctly. Apps using variable fonts (Inter Variable, Roboto Flex) can pick axis values; previously only static weights worked.
- **Noticeably sharper text** — the README and release notes both call this out.

This is a real upgrade. egui's text quality at small sizes was historically a known weak point; 0.34.0 substantially closed the gap.

What did **not** change in 0.34.0:

- **Shaping primitives** — egui still does its own (limited) shaping, not HarfBuzz / harfrust / rustybuzz. See below.
- **BiDi handling** — still basic, not full UAX #9.
- **Complex script support** — Arabic shaping, Indic conjuncts, Mongolian, complex CJK still limited.

## Font loading

`egui::FontDefinitions` is the registry:

```rust
let mut fonts = egui::FontDefinitions::default();

fonts.font_data.insert(
    "Inter".into(),
    std::sync::Arc::new(egui::FontData::from_static(include_bytes!("Inter.ttf"))),
);

fonts.families
    .get_mut(&egui::FontFamily::Proportional)
    .unwrap()
    .insert(0, "Inter".into());

ctx.set_fonts(fonts);
```

`FontData` accepts TTF / OTF / TTC bytes. `from_static` for compile-time-included bytes, `from_owned` for runtime-loaded bytes. The font is parsed once at registration time.

Default fonts shipped with egui (when the `default_fonts` feature is on, which it is by default):

- `Hack` (monospace)
- `Ubuntu-Light` (proportional)
- `NotoEmoji-Regular` (emoji — color emoji)
- `emoji-icon-font` (UI icons + emoji line-drawn variants)

The bundled fonts give egui a recognizable look without external dependencies. They're a few hundred KB total — kept small by design.

## Font families and fallback

`FontFamily::Proportional` and `FontFamily::Monospace` are the built-in families. Each maps to an ordered list of font names; the first font is the primary, the rest are fallbacks. egui walks the list per-glyph: if the primary font doesn't have a glyph for the codepoint, the next is tried, etc.

User-defined families (`FontFamily::Name(Arc<str>)`) can be registered and referenced in `TextStyle::FontId`.

Fallback chain limitations:

- Fallback is per-codepoint, not per-grapheme. A complex grapheme cluster spanning multiple codepoints may render its components from different fonts, breaking the cluster.
- No system-font discovery — egui doesn't read OS-installed fonts. Apps that want system fonts plumb them manually via `FontData::from_owned`.

## Text shaping: what egui does and doesn't do

"Shaping" in text-rendering parlance means: given a string + font + size, compute the sequence of glyph IDs and positions to render. For Latin scripts this is mostly a 1:1 codepoint-to-glyph mapping plus kerning. For complex scripts it requires substantial OpenType engine logic.

egui's shaping (post-0.34.0, via `skrifa`):

- **Latin / Cyrillic / Greek** — fully supported, with kerning. 0.33.0 improved kerning specifically.
- **CJK (Han, Hiragana, Katakana, Hangul)** — basic support. Glyph positioning is correct for monospaced CJK; complex vertical layout and ruby (furigana) is not supported.
- **Arabic** — letters render but **shaping is limited**. Arabic letters change form based on position (initial/medial/final/isolated) and ligature combinations; egui does not run the full OpenType Arabic shaping pipeline. Users see Arabic letters in their isolated forms in many cases, which is incorrect.
- **Indic scripts (Devanagari, Bengali, Tamil, etc.)** — limited. Conjunct ligatures (e.g. क + ् + ष = क्ष), reordering of dependent vowel signs, and complex cluster formation require a full OpenType Indic shaper (Universal Shaping Engine). egui doesn't ship one.
- **Hebrew** — letters render; nikud (vowel diacritics) positioning is approximate.
- **Emoji** — color emoji via the bundled `NotoEmoji-Regular` font. ZWJ sequences (family emoji 👨‍👩‍👧‍👦, skin tone modifiers 👋🏽) are partially supported depending on which sequences `NotoEmoji-Regular` ships glyphs for.

The full OpenType shaping pipeline (HarfBuzz's job in most production text engines) is not what egui does. This is consistent with egui's "minimal dependencies" non-goal (HarfBuzz is large) and with its target audience (dev tools written by Latin-alphabet-using developers).

## BiDi (bidirectional text)

The Unicode Bidirectional Algorithm (UAX #9) governs how text containing both LTR scripts (Latin) and RTL scripts (Arabic, Hebrew) gets visually ordered for display.

egui's BiDi handling is **basic, not full UAX #9**:

- A run of pure-LTR or pure-RTL text within a label renders correctly.
- Mixed-direction text in a single label uses heuristics, not the full algorithm.
- BiDi controls (LRO, RLO, LRE, RLE, PDF, RLI, LRI, FSI, PDI) are not interpreted.
- Caret movement in `TextEdit` follows visual order, which for pure-LTR or pure-RTL is the same as logical order but for mixed text doesn't always match user expectation.

For an Arabic / Hebrew app this is a structural blocker. For a Latin-default app with occasional non-Latin text, it works but with rough edges.

## Text editing: `TextEdit`

`TextEdit::singleline` and `TextEdit::multiline` build on the same text pipeline:

- Shaping uses the same primitives as `Label`.
- Caret movement is character-by-character (not grapheme-cluster-aware for complex scripts).
- Selection is by character; complex script grapheme cluster boundaries are not respected.
- IME composition with preedit underlining is supported; the composition state lives in egui's `Memory`, not the host's IME plumbing directly.
- IME bug fixes shipped in 0.34.0 (macOS / Safari backspace, Linux arrow-key behavior).

For Latin-script editing this is solid. For complex script editing it has the same shaping limits as rendering.

## Comparison to cosmic-text (Buiy's choice)

Buiy commits to **cosmic-text** as its text-shaping primitive ([`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)). cosmic-text is the System76 / Pop!_OS text-shaping crate, built on top of `rustybuzz` (the pure-Rust HarfBuzz port).

What cosmic-text does that egui doesn't:

- **Full HarfBuzz shaping** via rustybuzz. Arabic, Indic, Hebrew, Mongolian, complex CJK — all the OpenType shaping logic the world's text engines use.
- **Full UAX #9 BiDi** via the `unicode-bidi` crate.
- **Grapheme-cluster-aware caret** via `unicode-segmentation`.
- **System font discovery** via `fontdb` (reads OS-installed fonts).
- **Per-span style runs** within a single layout (mixed font, size, weight, color in one paragraph).
- **Line-wrapping with proper word-boundary detection** for non-space-separated scripts (CJK, Thai).
- **Color emoji and emoji-cluster handling.**

What cosmic-text doesn't do that egui does:

- Render to a built-in atlas. cosmic-text produces glyph positions; the consumer wires up rasterization + atlas itself.
- Provide widgets. cosmic-text is a shaping library, not a UI framework.
- IME — cosmic-text doesn't handle IME composition; the consumer does.
- A complete editing surface — cosmic-text's `Editor` is lower-level than `TextEdit`. Buiy will build the editing surface on top.

The trade is real and intentional. egui packages a complete-but-limited text pipeline as part of its "easy to use" promise. Buiy assembles a more powerful text pipeline from cosmic-text + harfrust + (custom editor surface) + (cosmic-text → atlas integration) — more parts, more capability.

## Implications for Buiy

- **Validates cosmic-text choice.** egui's text limitations are exactly what cosmic-text fixes. If Buiy used egui-style text shaping, Buiy could not credibly claim WCAG-compliant complex-script support.
- **Validates harfrust adoption for shaping.** cosmic-text's rustybuzz is being upgraded to `harfrust` (a more actively maintained fork) in the broader ecosystem. Buiy can ride this evolution.
- **Validates IME as a dedicated subsystem.** egui's IME is competent but bolted into `TextEdit`. Buiy's foundation calls for IME as a first-class text-editing subsystem with its own design spec.
- **Notes the 0.34.0 skrifa adoption as a precedent.** Google Fonts' skrifa is becoming the standard Rust font parser; Buiy's text pipeline should consider skrifa for parsing if cosmic-text doesn't already use it (cosmic-text currently uses fontdb + rustybuzz for parsing and shaping; skrifa is a separate primitive).
- **Cautionary on default fonts.** egui's bundled fonts (Hack, Ubuntu Light, Noto Emoji) give it instant out-of-the-box working text. Buiy needs to decide whether to bundle defaults or require user-provided fonts.

## See also

- [`architecture.md`](architecture.md) — the broader text pipeline placement (epaint → atlas → tessellation).
- [`api-surface.md`](api-surface.md) — `TextEdit`, `Label`, `RichText` as the consumer-facing entry points.
- [`styling-and-theming.md`](styling-and-theming.md) — `TextStyle` and `FontId` from the theming side.
- [`../bevy-egui/`](../bevy-egui/) — how text rendering flows through the Bevy bridge specifically.
- [`../cosmic-text/`](../cosmic-text/) — the Buiy text-shaper deep dive (sibling prior-art folder).

## Sources

- egui CHANGELOG (0.34.0 entry for skrifa/vello_cpu) — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- `egui::FontDefinitions` rustdoc — https://docs.rs/egui/latest/egui/struct.FontDefinitions.html
- `epaint` text module rustdoc — https://docs.rs/epaint/latest/epaint/text/
- `skrifa` (Google Fonts Rust) — https://github.com/googlefonts/fontations
- `vello_cpu` — https://github.com/linebender/vello
- `ab_glyph` (pre-0.34 backend) — https://github.com/alexheretic/ab-glyph
- cosmic-text — https://github.com/pop-os/cosmic-text
- rustybuzz — https://github.com/RazrFalcon/rustybuzz
- UAX #9 (Unicode Bidirectional Algorithm) — https://unicode.org/reports/tr9/
- Buiy text foundation spec — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
