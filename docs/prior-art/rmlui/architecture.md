**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — architecture: RML parser, RCSS parser, layout engine, embedder interface

# Architecture

RmlUi sits **above** an embedder's renderer + windowing + file-system, with five explicit C++ interfaces that the embedder implements (only one is mandatory). The library owns the parser → element tree → layout → paint-command pipeline; the embedder owns the actual GPU submission, input event source, font loading bytes, and asset I/O. The pattern is conceptually closer to **a UI runtime as a library** than to a self-contained framework.

## One-line summary

```
RML file ─► RML parser ─► Element tree (DOM) ─► RCSS parser + cascade ─► Style ─► Layout engine ─► Render commands ─► RenderInterface (embedder)
                                                                                                                            └─► SystemInterface (embedder)
                                                                                                                            └─► FileInterface (embedder)
                                                                                                                            └─► FontEngineInterface (embedder, optional)
                                                                                                                            └─► TextInputHandler (embedder, optional)
```

The embedder calls `Rml::Initialise()`, creates a `Context`, loads RML documents, then per frame:

1. Submit input events to the `Context` (mouse / keyboard / touch events).
2. Call `context->Update()` — RmlUi resolves properties + layout for all dirty elements.
3. Call `context->Render()` — RmlUi emits paint commands by calling back into the embedder's `RenderInterface`.

The library `"strictly runs as a result of calls to its API, never in the background"` (README). No background threads, no async work, no garbage collection, no event loop ownership.

## Embedder interfaces

Five interfaces. The embedder installs them via `Rml::Set*Interface()` *before* `Rml::Initialise()`. **RmlUi takes non-owning pointers** — embedder owns lifetime through `Rml::Shutdown()`.

### `RenderInterface` (mandatory)

The only required interface. The embedder must implement methods such as (paraphrased — exact signatures change per major version):

- `RenderGeometry(vertices, indices, texture, translation)` — submit a triangle list to the GPU.
- `CompileGeometry(...) / RenderCompiledGeometry(...) / ReleaseCompiledGeometry(...)` — optional path for retained geometry caching.
- `EnableScissorRegion(enable) / SetScissorRegion(x, y, w, h)` — clip rectangle.
- `LoadTexture(...) / GenerateTexture(...) / ReleaseTexture(...)` — texture lifecycle.
- `SetTransform(matrix)` — 4×4 transform stack for `transform:` property.
- **6.0 additions**: filters, masks, shaders, render layers — the render interface was substantially redesigned in 6.0 (2024-08-26) to support filters, gradients, box-shadows, masks. Pre-6.0 embedders must port to the new interface.

The reference repository ships sample implementations for **OpenGL 2, OpenGL 3, Vulkan, DirectX 12, SDL renderer, SDL GPU**. These are samples, not first-class library components — the embedder copies and adapts.

### `SystemInterface` (optional, default-implemented)

Defaults to a standard-library implementation. The embedder overrides for:

- `GetElapsedTime()` — monotonic clock.
- `LogMessage(type, message)` — log sink.
- `TranslateString(...)` — i18n string interception hook.
- `SetMouseCursor(name)` — propagate cursor name from RCSS `cursor:` property to the OS / game-engine cursor.
- `SetClipboardText() / GetClipboardText()` — clipboard bridge.
- `ActivateKeyboard() / DeactivateKeyboard()` — on-screen-keyboard hints for mobile / console.

### `FileInterface` (optional, default-implemented)

Defaults to `stdio.h`. The embedder overrides to redirect asset I/O through an engine's packaged-asset system (Unreal `.pak`, Unity `Resources`, custom VFS). Operations: `Open / Close / Read / Seek / Tell / Length`.

### `FontEngineInterface` (optional, default = FreeType)

Defaults to a FreeType-based implementation. The embedder overrides to ship its own font engine (this is how the HarfBuzz sample plugin works — it replaces the default FreeType-only engine with a HarfBuzz-shaped engine). Methods load font faces, query glyph metrics, and render glyph bitmaps that RmlUi caches into texture atlases.

### `TextInputHandler` (optional)

Backends supply a default. Surfaces text input (IME composition, complex-script entry) on platforms where the embedder owns the input source. The Win32 backend ships an IME-capable handler; other platforms get an empty default.

## The element tree

RmlUi parses RML into an **`Element` tree** rooted at an `ElementDocument`. Elements carry:

- **Tag** — `<div>`, `<p>`, `<button>`, plus a small RmlUi-specific tag set (`<handle>`, `<panel>`, `<tabset>`, etc.).
- **Attributes** — HTML-style `id`, `class`, plus RmlUi-specific (`data-bind`, `data-for`, `data-event`).
- **Computed style** — RCSS cascade result, ~150 properties.
- **Layout output** — resolved position + size after layout pass.
- **Event listeners** — attached via `AddEventListener` or RML `on*` attributes.

Elements are reference-counted via `UniquePtr<Element>` / raw pointer parent-borrow. No GC, no smart-pointer cycles in the public API (memory ownership is parent-child).

**Custom elements** are supported via `ElementInstancer` — the embedder registers a factory keyed by tag name to spawn `Element` subclasses. The widget set (`<input>`, `<select>`, `<tabset>`, `<handle>`) is implemented this way internally.

## The RCSS cascade

RCSS parsing produces a list of selector rules. The cascade pass walks every element, matches selectors, sorts by specificity (CSS spec rules), and resolves the ~150 computed properties. Specificity, inheritance, `!important`, custom properties (CSS variables in 6.0+) — all behave per CSS 2.1 + spec borrowings. **No `:has()`, no nesting (CSS Nesting Module), no container queries.**

Style resolution is **lazy** — elements track a dirty flag and re-resolve only on next `Context::Update()`.

## The layout engine

RmlUi ships its **own** layout engine, written in C++, owned end-to-end. It is **not** based on Yoga, Taffy, Flexbox.js, Stretch, or any external engine. Supported modes:

- **Block flow** — full CSS 2.1 block formatting context (block boxes, inline boxes, line boxes, float was added in libRocket but is sparsely tested; check before using).
- **Inline flow** — text + inline-block, line-breaking, basic vertical alignment.
- **Flexbox** — added in **5.0** (2022-12-11). Deliberately incomplete vs CSS spec: no `order`, no `flex-basis: content`, no `visibility: collapse`, no anonymous flex items from non-wrapped text, only approximate baseline alignment. See [`rml-rcss-coverage.md`](rml-rcss-coverage.md) § "Flexbox."
- **NO CSS Grid.** No tracking issue committing to it. Multi-column also absent.
- **`position: absolute / relative / fixed`** — supported. No `sticky` (verify; CHANGELOG-checked).
- **Tables** — `display: table` family supported for semantic table layout (data UI), not for legacy positioning hacks.

The engine is **single-pass-with-reflow** rather than constraint-solver-based. Performance bias: the engine recommends *avoiding* content-based sizing where possible (`flex: <number>` with definite dimensions over `flex: auto`), which surfaces the layout cost model explicitly to the author.

## The render pipeline

After layout, `Context::Render()` walks the element tree and emits paint commands. Each `Element` produces:

1. **Decorators** — background fills, gradients, image backgrounds, custom user decorators. RmlUi 6.0 introduced filters, box-shadows, masks as decorators / effects.
2. **Border edges** — `border-*` properties produce border geometry.
3. **Text runs** — glyph quads keyed by font atlas texture.
4. **Scissor / clip** — RmlUi calls `EnableScissorRegion` + `SetScissorRegion`. **Clipping is rectangular only** (matching the embedder-friendly bottom-of-pipeline constraint); rounded clipping is approximated via clip paths in 6.0+ but the embedder interface still ultimately fronts a scissor rect for the simple path.
5. **Transforms** — `transform:` property emits `SetTransform` calls.

The 6.0 render interface redesign adds:

- **Filters** — blur, drop-shadow, brightness, contrast, hue-rotate, opacity, etc.
- **Render layers** — offscreen render targets for effects.
- **Shaders** — embedder-provided shader stages.

**No** `backdrop-filter`, `mix-blend-mode`, `isolation`, or CSS top layer as named CSS features. The pipeline could express some of these via custom shaders + render layers but they are not surfaced as named RCSS properties.

## Context model

A `Context` is `"an independent collection of documents"` (docs § Contexts). One application can host many contexts (multi-window, multi-3D-anchored, multi-HUD). Each context:

- Owns its document set + element tree.
- Owns its input state + focus state.
- Drives its own `Update()` + `Render()` cycle.
- Resolves its own RCSS cascade.

Contexts do **not** share style assets implicitly — each loads its own fonts + RCSS. Asset sharing happens through the file interface's caching layer (embedder's responsibility).

## Data binding

RmlUi ships a **data-binding** subsystem (added in libRocket era, expanded in 4.x+). RML attributes `data-model`, `data-bind`, `data-for`, `data-event` connect element state to C++ data structures registered through `DataModelConstructor`. This is the equivalent of *Bevy observers + change detection*: a one-way pull from C++ state to UI, with event handlers writing back.

## What RmlUi does NOT own

- **GPU device / swapchain / queue.** Embedder.
- **Window / OS event loop.** Embedder.
- **Input source.** Embedder (RmlUi consumes synthetic events).
- **Font bytes on disk.** Embedder (FileInterface).
- **Clipboard.** Embedder (SystemInterface).
- **IME composition.** Embedder (TextInputHandler) — the Win32 backend ships one; others empty.

This is a clean separation. It is **the** pattern that makes RmlUi engine-portable — Cfx.re ships it in GTA-derived runtimes, Nightdive ships it in id Tech derivatives, Unreal plugins exist, Unity plugins exist. Embedder boundary is the single asset most worth studying for Buiy's render-to-texture / 3D-anchored UI surface (foundation `buiy_3d`, [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3).

## Implications for Buiy

- The embedder-interface pattern (RenderInterface + SystemInterface + FileInterface + FontEngineInterface + TextInputHandler) is **the right shape** for Buiy's render-to-texture surface API (foundation open question [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5, *"Render-to-texture surface API contract"*) — RmlUi has 15+ years of production validation that this decomposition holds up across rendering APIs (GL, DX, Vulkan, SDL_GPU, Metal) and platforms (Windows / macOS / Linux / Android / iOS / Switch).
- The **own-layout-engine** path is RmlUi's most-painful asymmetry vs Buiy: it built a C++ engine, ships ad-hoc CSS subsets (no Grid), and has to track CSS spec evolution itself. Buiy uses Taffy (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.2) and inherits Grid / subgrid / container-queries from upstream. Validates Buiy's substrate-borrow decision.
- The **6.0 render interface redesign** (2024-08-26) is a 15-year-late acknowledgment that the original libRocket interface didn't cover filters / masks / shadows. The redesign was breaking. Lesson: design render-interface surface for filters + masks + shadows + top-layer compositing **from day one** — Buiy's `buiy-render-pipeline-design` sub-spec must not leave these for later.
- The **single-pass-with-reflow** layout model is a load-bearing performance simplification but limits expressiveness (no full intrinsic-sizing roundtrip in flex). Buiy gets Taffy's full Flexbox + Grid for free; the architectural lesson is "watch what RmlUi *can't* express because its layout pass is single-shot" when reviewing Buiy's container-query plan ([`../../plans/2026-05-21-buiy-layout-container-queries.md`](../../plans/2026-05-21-buiy-layout-container-queries.md)) caps Buiy's re-layout at 2× Taffy.

## Sources

- RmlUi documentation, Interfaces — https://mikke89.github.io/RmlUiDoc/pages/cpp_manual/interfaces.html
- RmlUi documentation, Contexts — https://mikke89.github.io/RmlUiDoc/pages/cpp_manual/contexts.html
- RmlUi documentation, Main Loop — https://mikke89.github.io/RmlUiDoc/pages/cpp_manual/main_loop.html
- RmlUi documentation, Flexboxes — https://mikke89.github.io/RmlUiDoc/pages/rcss/flexboxes.html
- RmlUi 6.0 changelog (render interface redesign) — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- RmlUi Backends directory — https://github.com/mikke89/RmlUi/tree/master/Backends
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
