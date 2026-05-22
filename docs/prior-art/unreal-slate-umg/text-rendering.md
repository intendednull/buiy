**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — text shaping, BiDi, CJK, IME

# Text rendering

Slate owns its text pipeline end-to-end. The Slate text stack is one of the most complete in any game engine — far more complete than Bevy's pre-0.19 stack, comparable to Unity UI Toolkit's HarfBuzz integration, and weaker than browser text in only a few specific corners (justification quality, hyphenation, vertical-writing modes).

## Pipeline overview

```
Text source (FText)
   → ICU break iterator (line / word / grapheme breaks)
   → HarfBuzz shaper (per font run, after BiDi resolution)
   → FreeType rasterizer (per glyph) → FSlateFontCache atlas
   → FSlateDrawElement::MakeText → batched RHI draw
```

The pipeline runs **every frame for changed text** (UMG `Tick` re-shapes when `Text` changes); cached glyphs in the atlas are reused across frames.

## `FSlateFontInfo` — the font descriptor

`FSlateFontInfo` is Slate's font handle:

- **`FontFamily`** (`UObject*` reference to a `UFont` asset).
- **`TypefaceFontName`** (named typeface within the family — "Bold", "Italic", "Regular").
- **`Size`** (in Slate units; not pixels — Slate is DPI-aware via `SDPIScaler`).
- **`OutlineSettings`** (font outline color + thickness, for shaded text).
- **`LetterSpacing`** + **`SkewAmount`** (UE5+).
- **`MonospacedWidth`** (UE5+; emulated monospaced rendering of a proportional font).
- **`FontFallback`** — runtime fallback level. **Warning** baked into the API docs: this is mutation-unsafe on shared `FSlateFontInfo`s.

## `UFont` assets

A `UFont` asset is the disk-side font. Two flavors:

- **Runtime fonts** (the default) — wrap a `UFontFace` asset, which embeds the TTF/OTF bytes. The shaping path is dynamic via HarfBuzz at runtime. Supports the whole Unicode plane.
- **Offline-cached fonts** (legacy UE3-era; "bitmap fonts") — pre-rendered into a glyph atlas at import time. Used for fixed-codepoint UI (early UE4 game UI). Largely deprecated in UE5; runtime fonts are the modern path.

Font families compose multiple `UFontFace`s into named typefaces, plus a **typeface fallback list** (per-typeface). Fallback walks the list in order until a glyph is found, then renders from that face's glyphs. This is how CJK + Latin co-existence works without a single huge unified font.

## BiDi and complex scripts

Slate's text-shaping is **HarfBuzz-backed**, which inherits HarfBuzz's full feature set:

- Right-to-left scripts (Arabic, Hebrew, Persian, Urdu) render correctly with proper joining/initial/medial/final forms.
- Complex Indic scripts (Devanagari, Tamil, Bengali, Tibetan) shape with correct conjunct/cluster behavior.
- Arabic shaping uses HarfBuzz's `arab` shaper; cursive joining works.
- ICU drives line breaking, word breaking, and Unicode BiDi resolution (UAX #9). Slate calls `IBreakIterator` (its `ICUBreakIterator` impl) for line breaks and word breaks.

**Caveats:** Bevy-style "complete BiDi caret movement (logical vs visual cursor)" support exists but the editing surface for RTL text in `SEditableText` / `UEditableTextBox` is widely considered the weakest part of the stack. Long-form RTL editing typically goes through OS-native widgets or third-party plugins for shipped games.

## CJK

Full CJK support since UE4:

- Multi-byte UTF-16 (`FText` / `FString` internally stores as `TCHAR`, which is wide-char on Windows + UTF-16 elsewhere; Slate APIs accept `FText` consistently).
- Vertical and horizontal CJK fonts are loaded as separate typefaces.
- **No vertical-writing-mode** support — text always lays out left-to-right horizontally. Vertical CJK (manga-style) is not a first-class layout; authors usually rotate the widget with `RenderTransform`.
- CJK font fallback works via the typeface fallback list.

## Color emoji

Color emoji is supported when the loaded font includes a color glyph table:

- `EmojiOne`-style PNG-in-OTF (Apple Color Emoji, Segoe UI Emoji): rendered via the FreeType color bitmap path.
- COLRv1 vector color emoji: limited support; the FreeType version Unreal ships gates this.

The default Unreal-shipped fonts do not include color emoji — game UI authors load `NotoColorEmoji.ttf` or `seguiemj.ttf` to enable.

## IME (Input Method Editor) composition

Slate has cross-platform IME support routed through OS IME APIs:

- **Windows** — TSF (Text Services Framework) bridge through `WindowsApplication` + `FWindowsTextInputMethodSystem`.
- **macOS** — `NSTextInputClient` bridge.
- **Linux** — minimal; X11 XIM bridge exists but the gaps are well-known.
- **iOS / Android** — OS-native IME via UMG's mobile input plumbing.

Composing characters (Chinese, Japanese, Korean) shows the composition string inline in `SEditableText`/`UEditableTextBox`. Slate exposes `ITextInputMethodContext` for custom editing surfaces (the rich-text editors in the engine implement it directly).

## Localization (`FText` and namespaces)

Slate text is always typed as **`FText`** — a localized-string type that wraps a string with a localization key and namespace:

```cpp
SNew(STextBlock).Text(LOCTEXT("Save", "Save"))
SNew(STextBlock).Text(NSLOCTEXT("MainMenu", "Title", "Welcome"))
```

`LOCTEXT` / `NSLOCTEXT` macros generate localization-table entries. The text loader resolves at runtime against the current culture. Format strings (`FText::Format`) support gender, plurals, ordinals, dates, numbers via ICU MessageFormat.

This is one of the strongest points of the entire stack — text is **never** a raw `FString` in shippable UI. Every shipping AAA UE title gets localization "for free" because the Slate API rejects raw strings.

## Text-edit surface — strengths and gaps

`SMultiLineEditableText` and its `UMultiLineEditableTextBox` UMG wrapper are the workhorse multi-line editor. Features:

- Undo / redo via `FUndoableEditableText`.
- Text selection (mouse, keyboard with shift-arrow, double/triple-click for word/paragraph).
- Clipboard cut/copy/paste.
- Per-run text decorations via `FTextRunRenderer` (used for syntax highlighting in the Blueprint editor's comments).
- Marker text / placeholder.

Gaps relative to a productivity-app text editor:

- No collaborative-editing primitives (no CRDT, no per-character version vector).
- No grammar/spell-check OS bridge.
- BiDi editing (visual cursor in RTL context) is partial.
- No web-platform `contenteditable` semantics (no DOM range, no selection events with detail).

For a **game** UI these gaps are invisible. For Buiy's "Game and app, both" goal they would be load-bearing — which is why Buiy commits to cosmic-text + its own editing surface end-to-end (see [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)).

## What's borrowable

- **The `FText`-everywhere discipline.** Buiy types its text vocabulary as a single localized-text type at the API boundary, never raw strings. This is verifiable in CI (lint that no public API takes `&str` for user-visible text).
- **The typeface-fallback-list pattern.** Buiy's font-fallback story (cosmic-text-driven) is per-family fallback in order; Slate's `UFont` family with typeface fallback is the same shape.
- **`SInvalidationPanel` for text-heavy subtrees** — large static-text panels (item descriptions, lore text) get walled off and only re-paint on change. Buiy's render pipeline (`buiy_core`) has the analog (see foundation README).

## Sources

- FSlateFontInfo API — https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/SlateCore/Fonts/FSlateFontInfo
- Loading Fonts in UE5's Slate (Parade of Rain) — https://paradeofrain.com/posts/loading-fonts-in-slate/
- Fonts API listing — https://docs.unrealengine.com/5.2/en-US/API/Runtime/SlateCore/Fonts/
- Slate, Edit Text Widget, Custom Rendering & Any TrueTypeFont — https://nerivec.github.io/old-ue4-wiki/pages/slate-edit-text-widget-custom-rendering-any-truetypefont.html
- UMG-Slate-Compendium — https://github.com/YawLighthouse/UMG-Slate-Compendium
