**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — text rendering across UGUI (TextMeshPro) and UI Toolkit (TextCore); BiDi, complex shaping, IME, CJK / Arabic / Indic

# Text rendering

Unity's text rendering story is shaped by one event: the **2017 acquisition of TextMesh Pro** from Stephan Bouchard (creator since 2011). TMP became the default text renderer for both UGUI (`TextMeshProUGUI`) and, through its evolution into **TextCore**, the text engine inside UI Toolkit's `<TextElement>` / `<Label>` / `<TextField>`. The acquisition foreclosed the "use system text" path and committed Unity to a custom SDF-based pipeline. This is the closest Unity has to Buiy's cosmic-text commitment.

## Two text stacks (sharing one underlying engine)

| Stack | Text component | Underlying engine |
|---|---|---|
| UGUI | Legacy `Text` (deprecated), `TextMeshProUGUI` (recommended) | TextMesh Pro (TMP) |
| UI Toolkit | `<Label>`, `<TextElement>`, `<TextField>`, `<Button>` (label child) | TextCore (TMP-derived; same SDF rendering, same font asset format) |

Both stacks share **TMP font assets**: a Unity-specific asset baked from a `.ttf`/`.otf` font, containing the SDF (Signed Distance Field) atlas, glyph metrics, kerning, and fallback chain. Font assets are project-scoped; there is no `@font-face`-style runtime registration of OS fonts as the primary path.

## SDF rendering — the TMP innovation

- Glyphs are pre-baked into **Signed Distance Field** atlases. A single low-res SDF atlas can render text at any point size with crisp edges, because the shader reconstructs the glyph silhouette from the SDF gradient.
- Text scales arbitrarily — useful for game UIs that zoom, rotate, or render in world space.
- Effects (outline, drop shadow, glow, dilation) come from shader uniforms applied to the same SDF data; no per-effect glyph regeneration.
- Font asset generation is an **edit-time** step. Adding a new glyph (e.g. an emoji or a Korean character not in the initial atlas) requires regenerating the asset or using **Dynamic SDF Atlas** mode (Unity 2020+) that rasterises new glyphs on demand at runtime.

## BiDi, complex shaping, RTL

- **BiDi** — TMP/TextCore implements Unicode Bidirectional Algorithm (UAX #9) so Arabic and Hebrew flow right-to-left correctly when mixed with LTR runs. This was historically incomplete; Unity has improved BiDi over multiple releases.
- **Arabic shaping** — TMP supports Arabic contextual forms (isolated / initial / medial / final) via a built-in shaping pass. Quality has been the subject of community criticism (some letters mis-join, ligatures are imperfect); third-party Arabic-text packages exist on the Asset Store for projects with high-quality Arabic requirements.
- **CJK** — supported via Dynamic SDF Atlas mode (otherwise the static atlas would need every CJK character pre-baked, exploding asset size). Performance characteristic: first appearance of a CJK character is a small rasterisation hit.
- **Indic** (Devanagari, Tamil, etc.) — **partially supported**. Complex Indic shaping (consonant conjuncts, vowel reordering, reph positioning) is the long-standing weak spot. Per community reports, Indic renders but conjuncts often fail; serious Indic localisation typically uses third-party HarfBuzz-based packages.
- **Emoji** — supported via Sprite Asset fallback chains. Color emoji is project-asset-driven, not OS-native.

## IME (Input Method Editor) support

- UGUI's `InputField` (legacy) had passable IME support on Windows (Mono runtime), buggy on macOS, varied on mobile.
- UGUI's `TMP_InputField` improved IME integration substantially.
- UI Toolkit's `<TextField>` carries IME composition support via Unity's input system layer. Composition strings render inline; candidate windows are OS-provided (Windows IME bar, macOS input candidate window, mobile IME overlays).
- **Mobile IME** — Android (IME via on-screen keyboard) and iOS (UITextField bridge) work; quality is OS-version-dependent.
- **Limitations** — multi-line IME composition with line-wrap is historically fragile; vertical writing modes are not supported (no `writing-mode: vertical-rl` in USS, no equivalent in UGUI).

## Font fallback

- TMP font assets define a **fallback chain**: if a glyph isn't in the primary atlas, walk the fallback list until found, then fall back to a "missing glyph" character (commonly `□`).
- Fallback works at the glyph level, so a label can mix Latin (primary asset) + CJK (fallback) + emoji (sprite asset fallback) within a single text run.
- Sprite Asset fallback — pre-2017 emoji approach; replaced largely by Dynamic OS Font Asset fallback in modern Unity.

## Comparison to cosmic-text (Buiy's substrate)

| Axis | TMP / TextCore | cosmic-text |
|---|---|---|
| Glyph rendering | SDF atlas (Unity-specific) | Glyph atlas (raster, fontdb + swash) |
| BiDi (UAX #9) | ✅ | ✅ |
| Arabic shaping | ⚠️ partial; community-criticised | ✅ via rustybuzz (HarfBuzz port) |
| Indic complex shaping | ❌ poor | ✅ via rustybuzz |
| CJK | ✅ via dynamic atlas | ✅ native |
| Color emoji | ✅ via sprite assets | ✅ COLR/CPAL + bitmap |
| Font fallback chain | ✅ per font asset | ✅ via fontdb |
| `@font-face` / runtime fonts | ⚠️ project-asset-baked | ✅ runtime font registration |
| OS font discovery | ⚠️ Dynamic OS Font (limited) | ✅ via fontdb |
| IME | ✅ basic | Buiy's responsibility (cosmic-text + Bevy input) |
| Vertical writing | ❌ | ✅ |
| Effects (outline / glow) | ✅ via SDF shader | Buiy's render pipeline |

## What Unity does well (text)

- **Crisp text at any scale.** SDF wins decisively here vs raster atlases.
- **TMP rich-text inline markup.** `<color=red>`, `<size=20>`, `<b>`, `<i>`, `<sprite=0>`, `<link=foo>` — inline formatting without span-level USS. This is *better* than HTML/CSS for game-UI scenarios where text changes shape mid-line.
- **CJK via dynamic atlases.** Modern TMP handles hundreds of thousands of glyphs without exploding asset size.
- **Sprite Asset inline emoji / icons.** Inline graphics in text streams — common game-UI pattern (button-prompt glyphs, item icons in chat) — is first-class.

## What Unity does badly (text)

- **Complex Indic shaping.** Historically poor; multiple community packages exist to fill the gap.
- **Arabic shaping quality.** Better than nothing but not best-in-class; HarfBuzz-quality results typically require third-party packages.
- **No `writing-mode: vertical-rl`.** Vertical text (traditional Japanese/Chinese typesetting) is not supported in any Unity text path.
- **`@font-face`-style runtime font registration is awkward.** Font assets are project-baked; dynamic OS font discovery is limited compared to fontdb's capability.
- **No SVG fonts / color font formats beyond emoji.** OpenType `COLRv1`, COLRv0 multi-color, SVG-in-OpenType are not supported in TMP atlases.

## Implications for Buiy

1. **SDF is not Buiy's choice and that is OK.** SDF wins for arbitrary-scale text; cosmic-text + Buiy's atlas pipeline (foundation `buiy-text-rendering-design`) bakes glyphs at draw-time-size + caches. The trade-off is that re-rendering at a much different size hits the atlas again; the win is full HarfBuzz shaping quality. For productivity-app UI (Buiy's stated goal 6 — "Game and app, both") this is the right trade.
2. **The complex-script weak spot is the standard outcome of an in-house shaper.** TMP's Indic / Arabic limitations are the same shape as any non-HarfBuzz shaping engine. Buiy's commitment to cosmic-text + rustybuzz directly avoids this class of bug. Foundation text.md §3.4 commits to UAX #9, UAX #14 (line breaking), UAX #29 (grapheme/word/sentence boundaries), Unicode-correct shaping — these are HarfBuzz-via-rustybuzz capabilities that TMP partially lacks.
3. **Inline rich-text markup in text strings is a UX win Buiy should learn from.** `<color=red>` inline is more ergonomic than nested `<span style="color:red">` in some game-UI contexts. Buiy's `buiy-text-rendering-design` sub-spec should consider an analog (probably TMP-tag-compatible or a small Buiy-defined inline tag set) for in-game scenarios.
4. **IME quality is a Bevy-substrate concern, not Buiy's alone.** Unity's IME quality varies by platform because winit / OS bridges vary by platform. Buiy inherits the same variability via Bevy's input layer; foundation text.md §3.5 already flags IME-correctness as a manual release-gate item.
5. **Sprite asset fallback for emoji/icons in text** is a borrowable pattern. Buiy's `buiy-text-rendering-design` should consider per-glyph fallback to image / SVG resources within a text run for the game-UI button-prompt-glyph use case.

## Sources

- TextMesh Pro Joins Unity (Unity Blog, 2017-03-20) — https://blog.unity.com/games/textmesh-pro-joins-unity
- TextMesh Pro for Unity overview — https://viscircle.de/what-you-should-know-about-textmesh-pro-for-unity/?lang=en
- Kodeco TMP introduction — https://www.kodeco.com/22175776-introduction-to-textmesh-pro-in-unity
- Stephan Bouchard profile — https://www.crunchbase.com/person/stephan-bouchard
- Unity UI Toolkit text systems (manual chain) — https://docs.unity3d.com/Manual/UIE-USS-SupportedProperties.html
- Buiy foundation text — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
- cosmic-text prior art (cross-link) — [`../cosmic-text/`](../cosmic-text/)
- bevy_ui text-and-input — [`../bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md)
