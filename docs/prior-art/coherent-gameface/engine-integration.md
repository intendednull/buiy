**Date:** 2026-05-22
**Status:** active
**Subject:** Coherent Gameface — Unreal, Unity, custom-engine integration patterns

# Engine integration

Cohtml + Renoir are the **engine-agnostic substrate**. Three first-class integrations ship on top: Unreal Engine, Unity, and custom C++ engines. Each binding is a relatively thin wrapper that conforms Cohtml's lifecycle, input, asset, and render hooks to the host engine's idioms.

## Unreal Engine binding

Public docs track: https://docs.coherent-labs.com/unreal-gameface/

**Versions supported**: Unreal Engine 4 + Unreal Engine 5 (UE 5.4+ at minimum based on documentation surveyed; the binding tracks current UE stable).

**Distribution shape**: Coherent ships a UE plugin (delivered via the per-customer license channel — not the public UE Marketplace, given the quote-based licensing model). Plugin installs into `Plugins/Coherent` and exposes:

- **`CohtmlComponents`** — Actor / Scene Component wrappers for placing a Gameface view in the level or in a Slate widget. Includes `CohtmlGameHUD` (full-screen game UI), `CohtmlComponent` (in-world UI), `CohtmlInputForward` (input bridge).
- **Blueprint nodes** — `Create View`, `Trigger Event`, `Bind Model`, `Load URL` (local file path), `Resize`, `Set Visible`, etc. Lets designers wire UI from Blueprint without C++.
- **Render integration** — Cohtml emits Renoir command lists; the UE plugin translates to `FRHICommandList` calls so UI composites into UE's frame graph correctly. Render thread + RHI thread separation is honored.
- **Slate integration** — `SCohtmlInputForward` widget so Slate's input routing (focus, navigation, keyboard, gamepad) reaches Gameface views.
- **Input mapping** — UE's gamepad input flows into Cohtml's HTML5 Gamepad API per Coherent's docs; UE keyboard / mouse events translate to DOM events.
- **Asset pipeline** — UE-specific asset references resolved through Cohtml's `FileSystemReader`; HTML / CSS / JS / images live in the UE content directory and can be hot-reloaded.

## Unity binding

Public docs track: https://docs.coherent-labs.com/unity-gameface/

**Versions supported**: Unity 2020.2 and later (per the Coherent product page).

**Distribution shape**: Unity plugin delivered through the per-customer channel. (Notable: Unity Asset Store distribution is not part of Coherent's go-to-market for Gameface, in contrast to NoesisGUI which does ship an Asset Store SKU with documented version-drift issues — see [`noesisgui/critiques-and-open-problems.md`](../noesisgui/critiques-and-open-problems.md)).

Surface includes:

- **C# wrapper classes** — `CohtmlView`, `CohtmlLiveView`, `CohtmlSystem`, exposing the C API.
- **Scene-object attachment** — attach a `CohtmlView` as a `MonoBehaviour` for a GameObject; configure HTML source, size, input mode.
- **Editor integration** — Unity Editor inspector for view configuration; runtime preview.
- **Render integration** — Cohtml emits Renoir command lists; Unity binding routes them through Unity's `CommandBuffer` API for compositing.
- **Input mapping** — Unity's `InputSystem` (or legacy Input Manager) events translate to DOM events; gamepad routes through HTML5 Gamepad API.
- **C# ↔ JS binding** — register C# methods callable from JS; register JS event handlers callable from C#. The same shape as the Unreal binding.

## Custom C++ engine binding

Public docs track: https://docs.coherent-labs.com/cpp-gameface/

For studios with proprietary engines (Bluehole's TERA / PUBG-era engine, Wargaming's BigWorld-descended engine, custom in-house engines), Coherent ships the Cohtml + Renoir libraries with a documented C++ API. The integration cost is higher than the Unity/Unreal plugins — embedders write:

- **`FileSystemReader`** implementation pointing at their asset pipeline.
- **Render backend** implementation against their RHI (or use one of Coherent's pre-built DX11/12, Vulkan, Metal, OpenGL, GLES backends).
- **Input bridge** mapping engine input events to Cohtml's DOM event injection API.
- **Frame schedule** — call Cohtml's per-frame `Advance` + Renoir's render-pass entry points at the right point in the host engine's frame.

The custom-engine path is what made Gameface the de-facto "engine-agnostic AAA UI middleware" — studios with proprietary engines (and there are many at AAA tier) can adopt Gameface without rewriting their renderer or input system.

## Render hooks: Renoir as the integration seam

Renoir is the layer that gets the most engine-specific attention. The Renoir public docs describe a command-list-style API:

- Cohtml emits **draw commands** (textured quads, vector paths, glyph runs, scissor rects, blend-mode changes, render-target switches).
- Renoir consumes commands and **builds GPU command lists** in the target API's idiom.
- The host engine **owns the GPU command queue** — the engine decides when to submit, where in the frame UI compositing happens.
- **Resource barriers** (DX12, Vulkan) are exposed so the host engine's frame graph stays correct.
- **Texture providers** — the host engine can supply textures to Cohtml (e.g., a render-to-texture surface for in-world UI) and Renoir composites them.

The conceptual shape parallels RmlUi's `RenderInterface` (see [`rmlui/architecture.md`](../rmlui/architecture.md) embedder-interfaces discussion) and NoesisGUI's `RenderDevice` (see [`noesisgui/architecture.md`](../noesisgui/architecture.md)). All three commercial / open-source HTML/CSS/XAML middleware projects converge on roughly the same primitive: **a command-stream rendering API the embedder backs with their engine's render system.**

## Threading

Across all three bindings:

- Cohtml's **UI thread** runs JS + advances animations + resolves layout. The host engine calls `Advance(deltaTime)` once per frame.
- Renoir's **command-generation** can be multi-threaded — the docs claim worker threads for image decoding, texture compression, and command-list build.
- The host engine's **render thread / RHI thread** consumes Renoir's command lists.

This matches AAA engines' standard frame model: game thread + render thread + RHI thread, with UI middleware living mostly on the game thread for logic + a portion of render-thread work for compositing.

## Comparison: per-engine binding overhead

Coherent maintains **three integrations** (Unreal + Unity + custom C++). Each is a separate plugin that must track its host engine's release cadence:

- Unreal Engine cadence: ~6-month minor cadence (UE 5.4 → 5.5 → 5.6 → 5.7 as of the Coherent docs).
- Unity cadence: rolling per-version (Unity 2020.2+ to Unity 6.x); LTS-and-tech-stream tracks.
- Custom C++ — no fixed cadence; per-customer migration support.

A back-of-the-envelope shows **~8–10 engine-bump events per year** that the Coherent team must absorb. This is the per-engine binding tax NoesisGUI also pays (see [`noesisgui/lessons.md`](../noesisgui/lessons.md) Avoid row "Per-engine binding tax"). Coherent's response, like Noesis's, is "we have engineers dedicated to that." This is feasible at commercial-pricing scale; it is not feasible at single-maintainer-open-source scale.

Buiy contrasts: **Bevy-only** (foundation [`README.md` non-goals](../../specs/2026-05-07-buiy-foundation/README.md#non-goals)). Each Bevy minor release is **one** migration event. Buiy users pay one migration cost per quarter; Buiy maintainers pay one migration cost per quarter. The cross-engine reach is sacrificed deliberately.

## Hot-reload

Coherent docs describe **live UI modification during gameplay**: changes to HTML / CSS / JS files reload automatically into the running view without restarting the game. This is a major productivity win for UI development and is part of the value proposition Coherent sells.

Buiy commits to BSN hot-reload via Bevy's asset system (foundation [`README.md` § 5 open question on component hot-reload](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)). The cross-pattern parallel — "edit the asset, see it reload without restart" — is shared. The difference is the authoring layer (HTML/CSS/JS vs BSN) and the substrate (parsed DOM tree vs reflection-instantiated Bevy components).

## Devtools

Coherent ships a **DevTools-style inspector** that hooks into running Cohtml views. Inspect DOM, view computed CSS, modify styles live, see layout boxes — the standard browser-devtools shape. Not Chrome DevTools itself (Cohtml is not Blink), but the UX is intentionally close.

Buiy commits to a devtools sub-spec (foundation [§ 4 buiy-devtools-design](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) — inspector, layout overlay, AccessKit tree viewer, contrast linter, focus-order visualizer. The shape is similar; the substrate is Bevy ECS rather than DOM.

## Implications for Buiy

- **The "engine-agnostic substrate with thin engine-specific bindings" pattern is what commercial UI middleware optimizes for.** Buiy explicitly opts out of this axis — the Bevy-only commitment is the cost. The gain is that Buiy's substrate **is** Bevy (ECS, observers, asset system, render graph, input, picking, AccessKit). The "thin binding" cost disappears because there's no binding to maintain.
- **The Renoir-style command-stream rendering API is the wrong fit for Buiy.** Bevy's render graph + wgpu already provides the integration seam at a finer granularity (per-render-pass, per-render-node). Buiy doesn't need to define a Renoir-equivalent because Bevy's render graph **is** that seam. See foundation [`architecture.md` § 2.3 "What Buiy owns"](../../specs/2026-05-07-buiy-foundation/architecture.md#23-what-buiy-owns).
- **The per-engine binding tax is the strongest argument *for* Buiy's Bevy-only commitment.** Coherent ships 3 bindings × ~quarterly engine cadences ≈ 12 engine-bump events per year. Buiy ships 1 substrate × ~quarterly Bevy cadence = 4 events per year. That's a 3× engineering cost reduction baked into the Bevy-only architectural decision.
- **Hot-reload + DevTools are foundation-tier expectations.** Coherent ships both as production-grade features; Buiy needs to as well. The devtools sub-spec is in the foundation roadmap; BSN hot-reload is in the open-questions list to be cemented in `buiy-bsn-integration-design`.

## Sources

- Coherent Gameface product page — https://coherent-labs.com/products/coherent-gameface/
- Gameface Unreal docs — https://docs.coherent-labs.com/unreal-gameface/
- Gameface Unity docs — https://docs.coherent-labs.com/unity-gameface/
- Gameface C++ docs — https://docs.coherent-labs.com/cpp-gameface/
- Gameface Unreal FAQ — https://docs.coherent-labs.com/unreal-gameface/faq/
- Gameface Unreal Quick Start — https://docs.coherent-labs.com/unity-gameface/quick_start/quickstartguide_unity/
- Gameface Components docs — https://docs.coherent-labs.com/unreal-gameface/integration/cohtmlcomponents/
- Renoir Graphics Library introduction — https://coherent-labs.com/posts/introducing-renoir-graphics-library/
- Vulkan-support announcement — https://coherent-labs.com/vulkan-support/
- Coherent Labs FAQ — https://coherent-labs.com/frequently-asked-questions/
