**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — glossary of system-specific terms

# Glossary

System-specific terms used across the RmlUi corpus. Web-platform and CSS-spec terms are not re-defined here — see MDN or the relevant W3C spec.

| Term | Definition |
|---|---|
| **RmlUi** | The C++ user interface library that is the subject of this corpus. Acronym expansion: **Rocket Markup Language UI** (the "Rocket" is inherited from the libRocket predecessor; the project name has no current marketing tie to any other product called Rocket). |
| **libRocket** | The 2008–2014 predecessor to RmlUi. Maintained by CodePoint Ltd + Shift Technology Ltd. Repo: `libRocket/libRocket`. Dormant since ~2014. MIT-licensed. RmlUi is a fork. |
| **RML** | **R**ocket **M**arkup **L**anguage. The HTML-flavored markup language RmlUi parses. XHTML 1.0-flavored: tags close, attributes are quoted, strict parsing. Supports a deliberately small subset of HTML tags plus RmlUi-specific tags (`<panel>`, `<handle>`, `<tabset>`, `<datagrid>`). Includes RmlUi-specific attributes for data binding (`data-model`, `data-bind`, `data-for`, `data-event-*`). |
| **RCSS** | **R**ocket **C**ascading **S**tyle **S**heets. The CSS-flavored stylesheet language RmlUi parses. Based on CSS 2.1 + selective CSS 3 features. Adds RmlUi-specific properties (`decorator:`, `nav-*:`) and omits substantial portions of CSS (no Grid, no container queries, no anchor positioning, no logical properties). |
| **Embedder** | The C++ application or game engine that integrates RmlUi as a library. The embedder owns the main loop, GPU device, window, input source, asset I/O, and font bytes; RmlUi runs entirely as a result of calls into its API. |
| **Render Interface** (`RenderInterface`) | The single **mandatory** embedder interface. The embedder implements `RenderGeometry`, `EnableScissorRegion`, `LoadTexture` / `GenerateTexture` / `ReleaseTexture`, `SetTransform`, etc. RmlUi calls these to submit paint commands. Substantially redesigned in **6.0** (2024-08-26) to add filters / masks / render layers / custom shaders. |
| **System Interface** (`SystemInterface`) | Optional embedder interface (defaults to a stdlib-based implementation). Override for monotonic clock, log sink, string translation, mouse-cursor name → OS cursor, clipboard text, on-screen keyboard hints. |
| **File Interface** (`FileInterface`) | Optional embedder interface (defaults to `stdio.h`). Override to redirect asset I/O through an engine's packaged-asset system (Unreal `.pak`, Unity `Resources`, custom VFS). |
| **Font Engine Interface** (`FontEngineInterface`) | Optional embedder interface (defaults to FreeType). Override to ship a custom font engine. The HarfBuzz **sample** plugin is implemented as a custom `FontEngineInterface`. |
| **Text Input Handler** (`TextInputHandler`) | Optional embedder interface. Surfaces text input including IME composition. The Win32 backend ships an IME-capable implementation; other backends provide empty defaults. |
| **Context** (`Rml::Context`) | "An independent collection of documents." An RmlUi application can host many contexts (multi-window, multi-3D-anchored). Each context owns its document set, input state, focus state, RCSS cascade, and render pass. |
| **Document** (`ElementDocument`) | The root `Element` for an RML file. Loaded via `Context::LoadDocument(rml_path)`. Owns a styled element tree, can be shown / hidden / modal-shown. |
| **Element** (`Rml::Element`) | A node in the RML tree. Has a tag, attributes, computed RCSS style, layout output, event listeners, parent / children. Reference-counted via `UniquePtr<Element>` / parent borrow. |
| **Element Instancer** (`ElementInstancer`) | An RmlUi extension hook: a factory class that produces `Element` subclasses keyed by tag name. The built-in widget set (`<input>`, `<select>`, `<tabset>`, `<handle>`, `<datagrid>`) is implemented this way. Custom widgets register their own instancers. |
| **Decorator** | RmlUi-specific concept replacing CSS `background-image` / `background-position` / `background-repeat` / `border-image`. Applied via the RCSS `decorator:` property. Built-in decorators: `image`, `tiled-horizontal`, `tiled-vertical`, `tiled-box` (9-slice), `gradient`, `radial-gradient` (6.x), `conic-gradient` (6.x), `ninepatch`, `text` (6.1). Custom decorators register via `DecoratorInstancer`. |
| **Decorator Instancer** (`DecoratorInstancer`) | Extension hook: a factory class that produces custom decorator instances. The embedder uses this to extend RCSS with engine-specific visual primitives. |
| **Backend** | A reference implementation of the embedder interfaces shipped in `Backends/` for a specific GPU API + windowing combination (e.g., `Backends/RmlUi_Backend_GLFW_GL3.cpp`). Backends are **samples**, not first-class library components — the embedder copies and adapts. |
| **`Rml::LoadFontFace()`** | C++ API for loading a font file as an RmlUi font face. Font faces are referenced by `font-family` + style + weight from RCSS. There is no OS-system-font enumeration — fonts must be explicitly loaded. |
| **Data Model** (`DataModelConstructor`) | The C++ API for declaring observable data bound to RML elements. Models are constructed in C++ + referenced by `data-model="name"` attribute on RML elements; `data-bind`, `data-bind-value`, `data-for`, `data-event-*` resolve against the model. |
| **`Rml::Initialise()` / `Rml::Shutdown()`** | Global RmlUi initialization + teardown. Embedder calls `Initialise` after installing custom interfaces; calls `Shutdown` before tearing down its renderer. |
| **`mikke89`** | GitHub username of **Michael Ragazzon**, RmlUi's primary maintainer since the 2018 fork. |
| **Lloyd Weehuizen** | Lead figure most commonly cited from the libRocket era (2008–2014). Worked at CodePoint Ltd. Not currently active on RmlUi. |
| **CodePoint Ltd** | One of the two organizations holding original copyright on the libRocket codebase (the other is Shift Technology Ltd). |
| **KEX engine** | Nightdive Studios' internal game engine, used in their remasters (Quake, Doom 64, Shadow Man, Killing Time: Resurrected, The Thing: Remastered, etc.). Integrates RmlUi for HUD + menus. |
| **Cfx.re** | The team behind FiveM (Grand Theft Auto V multiplayer mod ecosystem). Their **Alchemist** asset-converter tool uses RmlUi. Acquired by Rockstar Games in August 2023. |
| **Bus factor** | Number of contributors whose simultaneous unavailability would halt the project. For RmlUi: **1** (mikke89). For libRocket post-2014: effectively zero (dormant). |
| **HarfBuzz sample** | The optional, sample-only HarfBuzz-backed `FontEngineInterface` implementation shipped in `Samples/basic/harfbuzz/`. Provides complex-script shaping (Arabic, Hindi, Thai, etc.) but is **not** a built-in feature; embedders must adopt + maintain it themselves. |
| **`<handle>`** | RmlUi-specific RML tag for a grab-handle child element that drags / resizes the parent. Used in window-like UIs. Edge-margin constraints added in 6.1. |
| **`<panel>`** | RmlUi-specific RML tag for a generic styled block container. Used internally by built-in widgets. |
| **`<tabset>` + `<tab>`** | RmlUi-specific RML tags for tab control. |
| **`<datagrid>`** | RmlUi-specific RML tag for data-bound table-like widget. |
| **Spatial navigation** | RmlUi's controller / D-pad / arrow-key directional UI navigation. Configured per-element via `nav-up`, `nav-down`, `nav-left`, `nav-right` attributes (explicit successor IDs) plus an auto mode (best-candidate-by-visible-geometry). Shipped since libRocket era. |
| **Render layers** (since 6.0) | Offscreen render targets in RmlUi 6.0+ that enable filters, masks, and custom shaders to operate on rendered subtrees rather than just per-element geometry. The architectural change that made the 6.0 effects suite possible. |

## Sources

- RmlUi documentation — https://mikke89.github.io/RmlUiDoc/
- RmlUi GitHub repository — https://github.com/mikke89/RmlUi
- RmlUi changelog — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- libRocket repository — https://github.com/libRocket/libRocket
- Sibling files in this corpus — [`README.md`](README.md), [`architecture.md`](architecture.md), [`rml-rcss-coverage.md`](rml-rcss-coverage.md), [`layout-and-styling.md`](layout-and-styling.md), [`text-and-input.md`](text-and-input.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`lessons.md`](lessons.md)
