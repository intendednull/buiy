**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — HTML / CSS / JavaScript feature coverage vs the web platform

# HTML5 / CSS / JavaScript coverage

Coherent positions Gameface as a **standards-compliant HTML5 engine**: parses HTML, runs JS (via V8 where licensing permits), evaluates CSS, supports React / Preact / SolidJS / jQuery / Tailwind / TypeScript / Webpack as the authoring toolchain. The substrate is in-house (`Cohtml`), not Blink/WebKit/Gecko/Servo. **Standards-compliant means most-of-the-web-platform-but-not-all** — there are deliberate omissions and game-UI-specific extensions.

This file enumerates what is supported, what is partial, what is missing, and what is custom — sourced from Coherent's public documentation (Differences-to-traditional-browsers page, CSS Properties reference, all-features marketing page) and from search-result coverage. Treat the per-property granularity as approximate — the canonical source is Coherent's docs site, which gates some pages.

## HTML5 element coverage

The Differences-to-traditional-browsers docs reference a "Supported HTML elements" table. The shape (per Coherent's overview and product-page coverage):

| Category | Status | Notes |
|---|---|---|
| Structural (`<div>`, `<span>`, `<header>`, `<section>`, `<article>`, `<nav>`, `<footer>`, `<aside>`) | Supported | Standard HTML5 sectioning |
| Inline + block flow | Supported | Standard cascade |
| `<canvas>` (2D + WebGL) | Supported | 2D canvas always; WebGL on supported backends |
| `<svg>` | Supported | First-class for icons + vector UI |
| `<video>` | Supported | Including **transparent video** for particle-effect overlays — a Coherent-specific marketing point |
| `<audio>` | Partial | Coherent docs reference it; web-standard audio APIs are typically supplemented by the host engine's audio |
| `<input>` text/number/checkbox/radio/range | Supported | Standard form widgets |
| `<input type="file">` | **Not supported** | No browser-style file-picker chrome inside a game |
| `<input type="date">` / `time` / `datetime-local` / `color` | **Not supported** | Browser-native pickers not applicable |
| `<form>` submission to a URL | **Not supported** | No HTTP stack; gameplay-side networking is the host engine's concern |
| `<iframe>` | **Not supported** | No nested browsing contexts |
| `<a href>` navigation | **Not supported as URL navigation** — anchor tags work as styled elements but do not "navigate"; routing is the app's concern |
| `<select>` / `<option>` | Partial | Renders, but the native dropdown chrome is not the OS chrome (it's CSS-rendered) |
| `<textarea>` | Supported | Single-line + multi-line text input |
| `<button>` / `<label>` | Supported | Standard semantics |
| Drag-and-drop API | Supported | HTML5 D&D events |
| Gamepad API | Supported | `navigator.getGamepads()` works, fed by the host engine's input system (Unreal exposes its gamepad layer this way) |

(The exact element coverage matrix is in Coherent's docs; the table above is the user-facing shape derived from public coverage. AAA studios shipping Gameface have, in practice, the HTML elements they need for HUD / menu / inventory / minimap / chat / settings / store / lobby UIs.)

## CSS coverage

### Layout

| Feature | Status | Notes |
|---|---|---|
| Block layout | Supported | Standard |
| Inline layout | Supported | Standard |
| **Flexbox** | **Full support** | Coherent positions this as the primary layout primitive. Marketing material claims "full FlexBox support." |
| **CSS Grid** | **NOT native** | Coherent ships a **JavaScript custom-element grid** (`coherent-gameface-grid`, `coherent-gameface-automatic-grid`) as part of the open-source `GameUIComponents` library. Authoring via `<gameface-grid>` rather than `display: grid`. **Major divergence from CSS spec.** |
| **CSS Subgrid** | **Not supported** | Follows from no native Grid. |
| Float | Partial / minimal | Standard game UIs rarely need it; Coherent docs are sparse on float specifics |
| `position: relative / absolute / fixed` | Supported | Standard stacking |
| `position: sticky` | Status unclear | Not prominent in docs |
| **Container queries** (`@container`) | **Not supported** as of documentation surveyed | A web-platform feature ratified in 2023; modern Buiy targets this |
| **Anchor positioning** (`anchor-name`, `position-anchor`) | **Not supported** | Bleeding-edge web platform feature |
| **Logical properties** (`margin-inline-start`, etc.) | Status unclear | Not prominent |
| **Writing modes** (`writing-mode: vertical-rl`, etc.) | Status unclear | Not prominent; CJK vertical writing is the gap |

### Styling & effects

| Feature | Status |
|---|---|
| CSS variables (custom properties) | Supported |
| **CSS `calc()`** | Supported in declarations, **NOT inside `@keyframes`** (per Coherent docs); mixing `%` and `px` not supported inside `calc()` (e.g. `50% - 20px` fails) |
| **2D transforms** (`translate`, `scale`, `rotate`, `skew`, `matrix`) | Supported |
| **3D transforms** (`perspective`, `transform-style: preserve-3d`, `rotateX/Y/Z`) | Supported |
| **CSS animations** (`@keyframes`) | Supported; **C++-evaluated** — Coherent docs explicitly recommend preferring CSS animations over JS-driven animation for performance |
| **CSS transitions** | Supported |
| **CSS filters** (`blur`, `brightness`, `contrast`, `drop-shadow`, `grayscale`, `hue-rotate`, `invert`, `opacity`, `saturate`, `sepia`) | Supported |
| **CSS `backdrop-filter`** | Supported per all-features page |
| **CSS blend modes** (`mix-blend-mode`, `background-blend-mode`) | Supported |
| **CSS masks** (`mask`, `mask-image`, `clip-path`) | Supported |
| **CSS box-shadow** | Supported |
| **CSS gradients** (linear, radial, conic) | Supported |
| **9-slice** | Supported via standard `border-image`-style mechanisms |
| Custom fonts (`@font-face`) | Supported; loads from embedder's asset pipeline |
| **CSS color** (sRGB + named) | Supported |
| **Modern CSS color** (`oklch()`, `color-mix()`, `color()`, wide-gamut) | Status unclear; not prominent in docs |

### Selectors

| Feature | Status |
|---|---|
| Class, ID, tag, attribute selectors | Supported |
| Pseudo-classes `:hover`, `:active`, `:focus` | Supported |
| `:focus-visible` | Status unclear; not prominent |
| `:disabled`, `:checked`, `:enabled` | Supported (for form controls) |
| Combinators (`>`, `+`, `~`, descendant) | Supported |
| `:nth-child`, `:nth-of-type`, etc. | Supported |
| **`:has()`** | Status unclear; recent CSS feature |
| **CSS Nesting** | Status unclear; recent CSS feature |

### Media queries

| Feature | Status |
|---|---|
| `@media (width)`, `(orientation)`, `(aspect-ratio)` | Supported |
| `@media (resolution)` / DPR-driven queries | Supported |
| **`@media (prefers-color-scheme)`**, **`(prefers-reduced-motion)`**, **`(prefers-contrast)`**, **`(forced-colors)`** | **Not documented** — Coherent docs do not surface these accessibility-related media queries. In-game OS preference plumbing is largely a Buiy-tier concern not addressed by Coherent. |

## JavaScript

- **Engine**: **V8** on platforms where licensing + binary distribution permit it. The Cohtml docs explicitly note that `window.onerror` and `window.addEventListener("error", ...)` are **V8-only** APIs, confirming a **dual-VM strategy**: V8 on first-tier platforms, an alternate (smaller, possibly slower) VM on platforms where V8 cannot be shipped (some consoles historically). The alternate VM's identity is not publicly named in the docs surveyed.
- **Standard library**: most of the web's `Element`, `Document`, `Window`, `Event` APIs. `fetch` / `XMLHttpRequest` not surfaced as standard (no HTTP stack); asset loading is via Cohtml's `FileSystemReader` abstraction.
- **Framework support**: officially: **React + Redux**, **Preact**, **SolidJS**, **jQuery**, **anime.js**, **Webpack**, **TypeScript** compilation, **Tailwind CSS**. The "use any web library" pitch is real but bounded by what the JS engine subset supports.
- **Web APIs NOT in standard Cohtml**: Service Workers, Web Workers (status unclear — not prominent), IndexedDB, LocalStorage / SessionStorage (typically game saves go through engine), Notifications, WebRTC, WebSockets (status unclear — networking is host engine's), File / FileReader, History / URL API.

## Cohtml-specific extensions

Game-UI-specific additions over the web platform:

- **C++ ↔ JS native binding** — register C++ functions as callable from JS via `engine.call(...)`, register JS as callable from C++. This is the in-process equivalent of the postMessage / extensions API.
- **Declarative data binding** — Cohtml's C++-bound data-model API; bind a C++ struct to a DOM subtree, mutations on either side propagate. Sits below React-style frameworks.
- **Gamepad input** — Unreal / Unity gamepad data flows through Cohtml as the standard HTML5 Gamepad API.
- **Cohtml-specific custom elements** — `<gameface-grid>`, `<gameface-virtual-list>`, etc., shipped as the OSS `GameUIComponents` library.
- **TextToSpeech + ARIA plugin family** — Cohtml-specific JS plugins (`CohtmlARIAHoverReadPlugin`, `CohtmlARIAFocusChangePlugin`, `CohtmlARIALiveRegionsPlugin`) using a JS SpeechAPI library to speak hovered / focused / live-region content. **In-process TTS — not an OS-AT bridge.** See [`critiques-and-open-problems.md`](critiques-and-open-problems.md) for the a11y critique.

## Comparison: full Chromium → Cohtml deltas

What Chromium has and Cohtml doesn't (approximate, as of 2026):

- **Networking** — full HTTP/2, HTTP/3, fetch, WebSocket, WebRTC
- **Storage** — IndexedDB, localStorage, cookies, cache API
- **Service Workers / Web Workers**
- **History / URL navigation / iframes**
- **Sandbox security model** — site isolation, CORS, CSP enforcement
- **Modern CSS** — native Grid, container queries, anchor positioning, view transitions, scroll-driven animations, `:has()`, CSS Nesting at-rule, modern color spaces, variable fonts (status unclear), `font-palette`
- **`<dialog>` element + true top layer** — status unclear
- **DOM APIs** — `MutationObserver`, `IntersectionObserver`, `ResizeObserver` — status unclear, some likely supported
- **OS-AT bridge** — full ARIA tree exposed to screen readers via the OS accessibility API. Cohtml uses **in-process TTS** instead.

What Cohtml has and Chromium doesn't:

- **C++ ↔ JS native binding** at engine-arbitrary granularity
- **Game-engine-native rendering integration** — Unreal `FRHICommandList`, Unity `CommandBuffer`, custom-engine direct
- **Threading + memory model designed for in-process integration** — no IPC tax, host-allocator-routed
- **Transparent video as a render primitive** for particle overlays

## Implications for Buiy

- **Buiy's "feature parity with the web platform" goal is bounded by the spec, not by a single existing reference implementation.** Coherent demonstrates that a curated HTML5 subset is sufficient for AAA game UI; Buiy's foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) tier list intentionally targets a **superset** of what Coherent ships (Grid native, container queries, anchor positioning, modern color) because Buiy is also a productivity-app UI library (foundation [`README.md` goal 6](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product)).
- **CSS Grid as a custom JS element is the wrong direction.** Coherent's `<gameface-grid>` works but commits the same "divergence from CSS spec" pitfall RmlUi commits with its `decorator:` syntax (see [`rmlui/lessons.md`](../rmlui/lessons.md) Avoid table). Buiy commits to **native CSS Grid semantics via Taffy** (foundation [`architecture.md` § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly), `buiy-layout` already landed with Grid via plan `2026-05-09-buiy-layout-grid.md`).
- **Accessibility-by-TTS-only is not the floor.** Coherent's ARIA plugin family is a JS-side speech bridge; it does not expose an accessibility tree to OS-level assistive technologies (NVDA, VoiceOver, JAWS, TalkBack). Buiy commits to **AccessKit-first**, OS-AT-bridged a11y (foundation [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)).
- **Animations evaluated in C++ is the right pattern.** Coherent's recommendation to prefer CSS animations over JS-driven animations holds: declarative animations let the runtime advance them on a hot path. Buiy's animation sub-spec (foundation [§ 4 buiy-animation-design](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) commits to ECS-component-driven transitions + keyframes, evaluated in Rust on the `Update` schedule against `Time<Virtual>` (foundation [`architecture.md` § 2.8](../../specs/2026-05-07-buiy-foundation/architecture.md#28-module-organization)). Same shape.
- **V8 is not load-bearing for Buiy.** Buiy has no JS runtime; BSN is the authoring surface, Rust ECS is the logic surface. The dual-VM cost Coherent pays (V8 + an alternate VM for restricted platforms) is one of the cost reasons Buiy stays no-JS.
- **The `<dialog>` + top-layer story is a Buiy differentiator over Coherent.** Coherent's stacking story is `position: absolute` + `z-index` (per the documentation surveyed; not definitive). Buiy commits to true top layer in foundation [`visuals.md` § 3.2](../../specs/2026-05-07-buiy-foundation/visuals.md) (tier **F**).

## Sources

- Coherent Gameface all-features page — https://coherent-labs.com/all-features-gameface/
- Differences-to-traditional-browsers — https://docs.coherent-labs.com/cpp-gameface/what_is_gfp/htmlfeaturesupport/
- Gameface CSS Properties reference — https://docs.coherent-labs.com/cpp-gameface/content_development/supported_features_tables/cssproperties/
- Gameface Animations docs — https://docs.coherent-labs.com/unity-gameface/content_development/animations/
- Gameface Components page — https://coherent-labs.com/blog/components/ (per docs index)
- GameUIComponents OSS repo — https://github.com/CoherentLabs/GameUIComponents
- coherent-gameface-grid npm — https://www.npmjs.com/package/coherent-gameface-automatic-grid
- TextToSpeech / ARIA plugins docs — https://docs.coherent-labs.com/cpp-gameface/integration/optional_features/texttospeech/
- Coherent Gameface product page — https://coherent-labs.com/products/coherent-gameface/
