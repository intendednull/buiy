**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — text rendering (FreeType default, HarfBuzz sample), BiDi, complex scripts, input handling, IME

# Text and input

RmlUi's text and input subsystems sit at the embedder-interface boundary: text rendering is driven by a swappable `FontEngineInterface`; input enters through `Context::Process*` methods that the embedder calls when synthetic input events arrive. For Buiy this section is the most direct cross-substrate comparison — Buiy commits to cosmic-text + AccessKit-integrated input; RmlUi commits to FreeType + an embedder-driven input pipeline with no a11y interception layer.

## Text rendering

### FreeType is the default

The default `FontEngineInterface` is implemented on **FreeType** and ships in the RmlUi repo. It handles font face loading (`Rml::LoadFontFace()`), glyph rasterization at requested sizes, atlas packing, and the most basic shaping work (`(left-to-right, single-script, no contextual shaping`). What this means in practice:

- Western Latin / Greek / Cyrillic / monospace rendering: solid.
- Diacritics: positioned per-glyph from font metrics (no contextual / mark-cluster shaping).
- Ligatures: handled only if the font's `'cmap'` table already exposes them as single codepoints; OpenType ligature substitution (`GSUB`) is not applied.

### HarfBuzz is a *sample*, not a built-in

The single most important RmlUi-text fact to internalize: **HarfBuzz is shipped as a sample plugin in `Samples/basic/harfbuzz/`, not as a built-in font engine.** Authors who need complex-script shaping (Arabic, Hindi, Thai, Khmer, Burmese, complex-scripts where contextual shaping is required for legibility) must:

1. Copy / adapt the HarfBuzz sample into their codebase, OR
2. Implement their own `FontEngineInterface` from scratch.

The 6.x changelog shows ongoing work on the HarfBuzz sample (6.1: "HarfBuzz font engine now uses kerning from HarfBuzz instead of FreeType"; 6.2: "Fix rendering of unsupported glyph clusters, improve emoji rendering"), which indicates the project does maintain the sample but does not promote it to the core path.

### BiDi (right-to-left, paragraph-level)

**Not built in.** No UAX #9 BiDi algorithm in the core engine. RTL paragraph resolution, mixed-script paragraphs (Hebrew embedded in English, Arabic embedded in French), neutral-character resolution — all are the embedder's problem if the embedder needs them. The HarfBuzz sample plugin includes some shaping support but does not implement the full BiDi paragraph algorithm.

Compare: cosmic-text bundles unicode-bidi (UAX #9 implementation) and Buiy gets BiDi paragraph resolution for free (foundation [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md) tier **F**).

### Color emoji

Emoji rendering improved in 6.2 ("Improve emoji rendering"). Implementation detail: requires a font face with color emoji tables (Apple's `sbix`, Microsoft's `COLR/CPAL`, or Google's `CBDT/CBLC` formats). RmlUi 6.2's improvement targets glyph cluster handling — the previous behavior dropped or mis-rendered clusters whose primary codepoint was unsupported.

### Font fallback

RmlUi exposes font family declarations; if the primary face lacks a glyph, the engine can fall back to a configured fallback face. The fallback mechanism is per-application (registered through `Rml::LoadFontFace()` with an explicit fallback flag) — there is no OS-system-font-discovery cascade, no `font-family` value resolution against installed fonts (the embedder must explicitly load every face).

cosmic-text + fontdb + Buiy's text spec ([`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)) ship OS-system-font enumeration; RmlUi's lack is a concrete asymmetry.

### Text shaping in the core path

The default FreeType engine does the simplest possible shaping:

- Look up codepoint → glyph index in `'cmap'`.
- Look up glyph metrics + kerning pair from font.
- Position glyphs in run-direction (`ltr` only by default).

No `liga`, `clig`, `dlig`, `kern` table beyond the basic GPOS-pair, no contextual alternates, no positioning attachments. **This is the design.** Adding HarfBuzz integration is the embedder's choice, with all the build / dependency / licensing implications that brings.

### Atlas + glyph caching

RmlUi caches rasterized glyphs into texture atlases keyed by (font face, size, glyph index). Atlases are reference-counted; eviction happens when fonts are unloaded. Texture atlases are managed via `FontEngineInterface::GenerateString` (paraphrased) which returns the geometry + texture handle for a string.

## IME and complex input

The **`TextInputHandler` interface** abstracts platform-specific text composition. The repo's Win32 backend ships an IME-capable handler that wires into Windows IMM32 (paraphrased — see `Samples/basic/ime/`). Other backends (GLFW, SDL, SFML, X11) ship empty defaults — IME on Linux / macOS / Wayland / mobile is the embedder's problem.

cosmic-text + Buiy ship a unified IME-correct text-edit surface (foundation [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md), see also `buiy-text-editing-design` sub-spec). The platform matrix Buiy commits to (Windows / macOS / Linux / Android / iOS, with X11 + Wayland under Linux) is **substantially wider** than RmlUi's first-class IME story.

## Input handling

### How input enters

The embedder calls into the `Context`:

- `ProcessMouseMove(x, y, modifiers)`
- `ProcessMouseButtonDown(button, modifiers)`
- `ProcessMouseButtonUp(button, modifiers)`
- `ProcessMouseWheel(delta, modifiers)`
- `ProcessTouchDown / TouchUp / TouchMove` (added in 6.2 for native touch)
- `ProcessKeyDown(key, modifiers)` / `ProcessKeyUp(key, modifiers)`
- `ProcessTextInput(string)` — text input from IME or keyboard
- `ProcessMouseLeave()` — context boundary

The context routes events to the focused element + hover element, dispatches DOM-like events (`click`, `mousemove`, `keydown`, etc.) into the element tree, and runs the bubbling/capturing phases.

Native touch input + inertial scrolling were added in **6.2** (2026-01-11). Pre-6.2, mobile / touch surfaces had to synthesize mouse events.

### Hit-testing

Per-frame hit-testing walks the element tree top-down, respecting `position`, `z-index`, `transform`, scissor rectangles. `pointer-events: auto | none` is supported. The hit-testing is **internal** — there is no external picking-backend pattern (no equivalent to `bevy_picking`'s pluggable backends).

### Focus model

A single focused element per `Context`. `tabindex` attribute on RML elements controls tab order (analogous to HTML's `tabindex`). `Tab` / `Shift+Tab` cycle focus. Focus changes fire `focus` / `blur` events.

**What is missing vs Buiy's focus model:**

- No `:focus-visible` semantics (RmlUi has `:focus`, period).
- No focus traps or focus restoration.
- No `inert` subtree.
- No roving tabindex pattern as a built-in (composite widget authors implement manually).
- No `aria-activedescendant` (no ARIA at all).
- No sequential-focus-navigation-starting-point.

### Spatial navigation (controllers)

RmlUi ships **spatial navigation for controllers** — the README mentions it explicitly. RML elements can opt into directional navigation via attributes; the engine resolves `nav-up`, `nav-down`, `nav-left`, `nav-right` per element to specify explicit successor IDs, plus an auto mode that finds the best candidate by visible geometry. This is the closest analog to bevy_ui's `AutoDirectionalNavigation` (Bevy 0.18+) — RmlUi had it earlier, and it's the one accessibility-adjacent feature RmlUi consistently ships.

### Keymap / action dispatch

No first-class keymap / action / command system. The embedder writes event handlers in C++ that map raw key events to game-side actions. Compare: GPUI (Zed) ships a keymap-asset + typed-action + key-context dispatch pattern; Buiy plans a similar pattern in `buiy-input-events-design`.

## Implications for Buiy

- **HarfBuzz-as-a-sample** is RmlUi's largest single text feature gap. It means **every** RmlUi embedder targeting Arabic / Hindi / Thai / Khmer / complex scripts must individually port + maintain shaping integration. Buiy's commitment to cosmic-text (which includes harfrust + skrifa + unicode-bidi) makes complex scripts first-class out of the box. Validates Buiy foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.2 commitment to cosmic-text directly.
- **BiDi paragraph algorithm absence** matches a known pattern in game UI libraries — Coherent Gameface and NoesisGUI also under-deliver on BiDi vs browsers. Buiy's BiDi-via-cosmic-text is a concrete competitive advantage in the open-source design space.
- **IME story is platform-fragile** — Win32 backend has IME, others don't. Buiy's unified text-edit surface targeting Windows / macOS / Linux / Android / iOS / web (foundation [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md)) is genuinely more ambitious than RmlUi's posture and is one of the strongest data points for the **"productivity-app concerns are in scope"** goal (foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1 goal 6).
- **Focus model thinness** (`:focus` only, no `:focus-visible`, no traps, no inert, no roving tabindex pattern) is RmlUi's accessibility absence visible at the input layer. Buiy's `buiy-focus-model-design` sub-spec ([`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 4) ships all of these.
- **Spatial navigation IS shipped** by RmlUi (since libRocket era). Lesson: console / controller-driven UI navigation is a *solvable problem* in this design space — the bevy_ui community took until 0.18 (2026-01-13) to ship `AutoDirectionalNavigation`, but it's been a normal feature of game UI libraries since the 2000s. Buiy's foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1.6 "game and app, both" + spatial nav in [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) is aligned with industry practice, not novel.
- **No external picking-backend pattern** in RmlUi. The internal-only hit-testing is fine for a single-window embedder but contrasts with bevy_picking's pluggable design that Buiy borrows (lessons-list in [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Borrow entry 5).
- **No keymap-asset / action-dispatch system** in RmlUi. Buiy plans one (`buiy-input-events-design`); RmlUi's absence is one more reason game studios end up reinventing controller-rebinding UIs from scratch on top of RmlUi.

## Sources

- RmlUi documentation, Interfaces — https://mikke89.github.io/RmlUiDoc/pages/cpp_manual/interfaces.html (Font Engine Interface, Text Input Handler)
- RmlUi documentation, Fonts — https://mikke89.github.io/RmlUiDoc/pages/cpp_manual/fonts.html
- RmlUi Samples folder — https://github.com/mikke89/RmlUi/tree/master/Samples (harfbuzz, ime, bitmap_font samples)
- RmlUi changelog (6.1 HarfBuzz kerning; 6.2 native touch, inertial scrolling, emoji clusters) — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- Buiy foundation text — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- cosmic-text prior-art (sibling) — [`../cosmic-text/`](../cosmic-text/)
- AccessKit prior-art (sibling) — [`../accesskit/`](../accesskit/)
