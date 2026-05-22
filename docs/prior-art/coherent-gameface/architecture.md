**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — runtime architecture: Cohtml + Renoir, engine bindings, threading

# Architecture

## Two-product split

Coherent Labs ships two related products on a shared runtime:

- **Coherent Gameface** — the **code-first developer-facing product**. Author UI in HTML5/CSS3/JS (React, Preact, jQuery, Tailwind, TypeScript). The standard pipeline for programmer-authored AAA game UI.
- **Coherent Prysm** — the **artist-first product**. Author UI in Adobe Animate (via a Coherent-provided plugin); Prysm exports to the same Cohtml runtime. Marketed as the Scaleform successor for studios whose UI authoring lives in animator-driven asset pipelines. Released 1.0 alongside Gameface 1.0 on 2018-12-07.

Both products run on the same C++ runtime — the only difference is the authoring tool. SteamDB tags both together (`Coherent_Gameface_OR_Prysm`) because the runtime fingerprint is identical.

For the rest of this doc and for [`lessons.md`](lessons.md), "Gameface" refers to the runtime architecture shared by both products unless otherwise noted.

## The two-library substrate

The Gameface runtime is composed of two cooperating C++ libraries:

- **Cohtml** — the **HTML/CSS/JS engine**. Parses HTML, resolves CSS, owns the DOM, runs JavaScript (via V8 where licensing permits, an alternate VM otherwise), owns the layout pipeline, dispatches events. **Built in-house by Coherent Labs**; not derived from Blink, WebKit, Gecko, or Servo. Successor to the original WebKit-based Coherent UI runtime (deprecated 2017-12-05) and the WebKit-based Coherent GT runtime.
- **Renoir** — the **GPU rendering library**. Consumes the rendering command stream that Cohtml emits, translates to the target graphics API (DX11, DX12, Vulkan, Metal, OpenGL, GLES2/3, console-native). Built around "console-style graphics libraries" — DX12 / Vulkan / Metal first-class, command-list-style submission, multi-threaded command generation. Replaces the legacy single-threaded backend; Coherent claims **15–70% rendering-performance improvement** depending on UI complexity.

The two libraries are independently versioned. Cohtml is the entry point embedders typically touch; Renoir is exposed to embedders via the rendering backend abstraction.

Buiy's substrate equivalence: Cohtml's role is split across **Taffy** (layout) + **cosmic-text** (text shaping) + **Buiy components** (DOM equivalent) + **observers + change detection** (events). Renoir's role is filled by **wgpu via Bevy's render graph**. The decomposition is finer-grained than Coherent's two-library shape, because Buiy doesn't ship an HTML parser or a JavaScript engine.

## What Cohtml owns

Approximately:

- **HTML5 parsing** — standards-compliant subset (see [`html5-coverage.md`](html5-coverage.md) for the supported-element table).
- **CSS3 parsing + cascade + computed styles** — declarations, selectors, specificity, computed-value resolution.
- **DOM** — the live document tree with `Element`, `Node`, attribute mutation, event dispatch. JavaScript code manipulates the DOM through the standard browser APIs.
- **Layout** — Flexbox (full per Coherent docs), block / inline boxes. **Native CSS Grid is conspicuously absent** — Coherent ships a **JavaScript custom-element grid component** (`coherent-gameface-grid`, `coherent-gameface-automatic-grid`) as part of the open-source GameUIComponents library. See [`html5-coverage.md`](html5-coverage.md) and [`critiques-and-open-problems.md`](critiques-and-open-problems.md).
- **CSS animations + transitions** — evaluated in C++ for performance; Coherent docs explicitly recommend CSS3 animations over JS-driven animations for this reason.
- **CSS transforms (2D + 3D)**, **CSS filters**, **CSS blend modes**, **CSS masks/clipping**, **CSS box-shadow** — all first-class.
- **JavaScript runtime** — V8 on platforms where licensing + binary distribution permit it; an alternate VM elsewhere. `window.onerror` and `window.addEventListener("error", ...)` are documented as **V8-only**, confirming a dual-VM strategy.
- **Data binding** — Coherent's declarative C++ ↔ JS model-binding system. Bind a C++ data model to a DOM subtree; mutations on either side reflect on the other. Sits below React-style frameworks and below Vue-style two-way binding.
- **C++ ↔ JS native binding** — register C++ functions as callable from JS, register JS callbacks as triggerable from C++.

## What Renoir owns

- **Command-stream rendering** — Cohtml emits draw commands; Renoir builds GPU command lists.
- **Backend abstraction** — DX11, DX12, Vulkan, Metal, OpenGL, GLES2/3, console-native APIs.
- **Multi-threaded command generation** — data-oriented design; image decoding + texture compression offloaded to worker threads.
- **Resource management** — texture lifetime, atlas management, GPU memory tracking (`RenoirGPUMemoryInfo` API).
- **Resource-barrier helpers** — on DX12/Vulkan, explicit resource barriers are exposed to the embedder so the host engine's frame graph stays correct.

The Renoir API is the integration seam an embedder *can* drop into a custom graphics pipeline if their engine's frame graph needs full control. The Unity / Unreal bindings hide this seam by default; custom-engine embedders consume it directly.

Buiy substrate equivalence: Renoir's role is `wgpu` (single cross-platform graphics-API abstraction, accessed via Bevy's render graph). Where Renoir abstracts over many native graphics APIs, Buiy abstracts via Bevy abstracting via wgpu. The seam is one layer up. See foundation [`architecture.md` § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly).

## Engine-binding pattern

Three first-class bindings ship:

1. **Unreal Engine** plugin — supports UE 4 + UE 5; ships `CohtmlComponents` Blueprint nodes; integrates with Unreal's Slate input layer; render output goes through Unreal's `FRHICommandList`.
2. **Unity** plugin — supports Unity 2020.2+; ships C# wrappers over the Cohtml C API; integrates with Unity's input event system; render output via Unity's `CommandBuffer`.
3. **Custom C++ engine** — direct Cohtml + Renoir API consumption; embedder writes the render-backend bridge themselves. The `cpp-gameface` documentation track is for this audience.

Cohtml is engine-agnostic at the API level. The engine bindings are thin C/C++ wrappers over Cohtml + Renoir that conform to each engine's lifecycle, input system, asset pipeline, and render backend.

Buiy contrasts: **Bevy-only** (foundation [`README.md` non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)). Each Bevy minor release is one migration event for Buiy users. Coherent ships ~one binding-port per engine per major engine release — three bindings × ~quarterly engine cadences = approximately 12 engine-bump events per year.

## Threading

Cohtml is documented as multi-threaded internally:

- **UI thread** — runs JavaScript, dispatches events, advances animations, resolves layout.
- **Renderer thread(s)** — Renoir consumes Cohtml's draw commands; command-list generation is multi-threaded.
- **Worker threads** — image decoding, texture compression.

The host engine retains control over when each phase runs. Coherent's CEF critique post calls out CEF's IPC-driven cross-process model as adding "several milliseconds" of latency for game-relevant data flow; Gameface's in-process threaded model is positioned as the alternative.

## Memory model

Documented features:

- Custom memory allocators — the embedder supplies allocators; Cohtml + Renoir route every allocation through them. This matches AAA engines' arena / pool / tracking infrastructure.
- GPU memory tracking — `RenoirGPUMemoryInfo` reports texture / buffer / atlas usage to the embedder. Lets profilers attribute GPU memory to the UI subsystem specifically.
- Single-process — no out-of-process renderer (contrast with Chromium / CEF). Trade-off: one crash brings everything down, but data flow is direct in-memory, no IPC tax.

## Networking, files, security

The "differences to traditional browsers" docs (per the docs index, which we have partial access to) flag:

- **No general-purpose networking stack.** Cohtml does not bundle a browser-style HTTP stack. Embedders supply asset loading via Cohtml's `FileSystemReader` abstraction; gameplay-side networking goes through the host engine.
- **No persistent storage / cookies / IndexedDB / service workers** — out of scope for game UI.
- **No sandboxing** — the JS runtime executes in the game's process with whatever permissions the game has. The game is trusted; the UI authors are typically the same team. (This is the bargain Coherent + every game UI middleware accepts.)

Buiy is even further from a browser shape: no JavaScript runtime at all (BSN is a Bevy-asset format consumed by Bevy's asset system; logic lives in Rust ECS systems and observers, not JS). The "no networking, no sandboxing, no service workers" trade-off is built into BSN's design and reinforced as a non-goal in foundation [`README.md` non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals).

## Authoring tooling

- **Coherent Editor** (paid) — a visual editor for Coherent Prysm; Adobe Animate plugin. Targets artists with no-code animation + interaction setup.
- **GameUIComponents** (open-source, MIT-licensed, on GitHub at `CoherentLabs/GameUIComponents`) — a suite of Web-Components-style custom elements (`coherent-gameface-grid`, virtual lists, dropdowns, tooltips, etc.) usable both inside Gameface and inside Chrome for development. `coherent-guic-cli` is a scaffolding tool.
- **Standard web tooling** — VS Code, Chrome DevTools-style inspector (Coherent ships its own inspector), Webpack, React/Preact toolchains, TypeScript, Tailwind. **Authoring is cross-platform by default.** This is one place Coherent dramatically out-performs NoesisGUI (Blend for VS, Windows-only).

## Implications for Buiy

- **The two-library decomposition (HTML engine + GPU renderer) is the wrong shape for Buiy.** Buiy doesn't need an HTML parser because BSN is the authoring layer and BSN consumes through Bevy's asset system into reflection-driven component spawning. The "Cohtml-equivalent" surface for Buiy is split into Taffy (layout) + cosmic-text (text) + Bevy ECS (DOM equivalent) + observers (events). The "Renoir-equivalent" is wgpu via Bevy's render graph. **No equivalent of a "Coherent monolith" exists or is needed.** See foundation [`architecture.md` § 2.1](../../specs/2026-05-07-buiy-foundation/architecture.md#21-one-line-summary).
- **The engine-binding pattern doesn't apply.** Buiy is Bevy-only by design; Coherent's three-engine-binding overhead is the cost of being engine-agnostic, which Buiy explicitly opts out of.
- **The custom-memory-allocator + GPU-memory-tracking pattern is worth carrying forward.** Bevy already gives this for free (`MemoryUsage` plugin, render-graph instrumentation). Buiy should expose a per-UI-subsystem GPU memory breakdown in the devtools sub-spec (foundation [§ 4](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)).
- **The single-process model + threaded UI lane is correct.** Cohtml's critique of CEF's IPC is fair; the right shape for game UI is in-process, threaded, owned by the host engine's frame schedule. Buiy + Bevy ECS already operates this way (no out-of-process renderer, no IPC tax).

## Sources

- Coherent Gameface product page — https://coherent-labs.com/products/coherent-gameface/
- Coherent Gameface all-features page — https://coherent-labs.com/all-features-gameface/
- Coherent Prysm 1.0 release (2018-12-07) — https://coherent-labs.com/blog/releases/prysm-1-0-released/
- Gameface 1.0 release (2018-12-07) — https://coherent-labs.com/blog/releases/gameface-1-0-released/
- Renoir Graphics Library introduction — https://coherent-labs.com/posts/introducing-renoir-graphics-library/
- Vulkan-support announcement — https://coherent-labs.com/vulkan-support/
- Renoir API change notes — https://coherent-labs.com/incoming-renoir-backend-api-changes/
- CEF critique post — https://coherent-labs.com/posts/what-developers-should-consider-when-using-chromium-embedded-framework-cef-in-their-games/
- Gameface CSS Properties reference — https://docs.coherent-labs.com/cpp-gameface/content_development/supported_features_tables/cssproperties/
- Differences-to-traditional-browsers — https://docs.coherent-labs.com/cpp-gameface/what_is_gfp/htmlfeaturesupport/
- Resource-barriers docs — https://docs.coherent-labs.com/cpp-prysm/integration/optional_features/resourcebarriers_native/
- Rendering architecture (C++ Gameface) — https://docs.coherent-labs.com/cpp-gameface/integration/rendering/
- GameUIComponents OSS repo — https://github.com/CoherentLabs/GameUIComponents
- SteamDB tech detection — https://steamdb.info/tech/SDK/Coherent_Gameface_OR_Prysm/
