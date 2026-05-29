**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — runtime architecture, render-device abstraction, integration surface

# Architecture

NoesisGUI's runtime is a native C++ library that loads XAML at runtime, builds a retained dependency tree, drives layout and rendering against an abstract `RenderDevice` interface that the host engine implements, and exposes a managed C# layer on top for Unity and any .NET host. The engine bindings (Unity package, Unreal plugin) are thin wrappers around the C++ core that provide a `RenderDevice` implementation for the host's graphics API, an asset importer for `.xaml` and font files, and an input-routing layer.

## Two-tier API: Framework vs Integration

NoesisGUI separates its API into two conceptually distinct layers, documented in the [WPF comparison docs](https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html):

- **Framework API** — high-level objects users author against. `Controls` (Button, ListBox, ItemsControl, ContentControl), `Panels` (Grid, StackPanel, DockPanel, Canvas), the dependency-property system, styles and templates, animation, the XAML loader. This mirrors WPF's `System.Windows.Controls` surface; all classes live in the single `Noesis` namespace rather than WPF's split namespaces.
- **Integration API** — low-level abstractions the host engine implements. `RenderDevice` (GPU command submission), texture providers, font providers, URI resolution, input event injection, custom shader effects. The Framework API is engine-agnostic; the Integration API is where engine-specific code lives.

The split is load-bearing: it is why Noesis can ship a single C++ core that targets Unity, Unreal, custom C++ engines, and managed .NET hosts without engine-specific forking of the framework code.

## Dependency tree

The core retained data structure is a tree of `DependencyObject` instances, each carrying a bag of `DependencyProperty` values (typed, can have default values, can inherit from parents, can be animated, can be bound). Visual elements (`UIElement`, `FrameworkElement`) are dependency objects with layout and rendering semantics layered on top. Property changes notify dependents; the dirty tree drives layout, render, and animation work each frame.

This is conceptually similar to bevy_ui's `Node` + `ComputedNode` split, but with two important differences:

1. **Properties are dictionary-backed, not statically typed components.** WPF-derived dependency properties carry the cost of a hash-table lookup per access; Buiy components carry zero overhead because they are typed ECS components.
2. **Inheritance is built into the system.** Setting `FontFamily` on a parent makes children inherit it without explicit propagation; Bevy + Buiy require explicit inheritance systems.

See [`xaml-paradigm.md`](xaml-paradigm.md) for the data binding and dependency-property model in detail.

## RenderDevice abstraction

`RenderDevice` is the load-bearing seam between Noesis and the host engine. It is an abstract C++ class with virtual methods for:

- Creating and updating textures (RGBA8 / R8 for glyph alpha).
- Creating render targets.
- Submitting batches of indexed primitives with bound shaders and uniform constants.
- Map/unmap for streaming vertex buffers.

Each host engine writes its own `RenderDevice` implementation. The official integrations ship reference implementations for D3D11, D3D12, Metal, Vulkan, OpenGL, GLES; the Unity plugin selects the appropriate one based on the active graphics API; the Unreal plugin uses RHI's per-platform backend. **wgpu is not a target backend** — Noesis predates wgpu and operates one layer below it. (Implication for Buiy in [`lessons.md`](lessons.md): Noesis lives directly on graphics APIs because it cannot assume an engine-provided abstraction; Buiy is one layer up because Bevy provides wgpu.)

The `RenderDevice` interface is designed for sub-millisecond rendering: it issues large indexed-triangle batches per frame, expects the host to manage GPU state correctly across `RenderDevice` calls, and assumes the host's render thread invokes it. Threading is the host's responsibility — Noesis does not own a render thread.

## Threading model

- **UI logic thread** — application owns; calls `View::Update(time)` once per frame to advance animations, layout, input handlers, and produce a render command list.
- **Render thread** — application owns; calls `View::Render()` on the render thread with a `RenderDevice` to submit the command list.

The two-phase split (update on logic thread, render on render thread) lets Unity and Unreal map Noesis cleanly onto their existing game-thread / render-thread separation. The framework explicitly does not own a thread or pump a runloop — it is library code, not an engine.

## Asset model: XAML, fonts, images

XAML files (`.xaml`) are the primary authoring asset. They are loaded at runtime by the framework loader; in shipping builds they can be precompiled to a binary format (`.bxml`) to skip XML parsing. Fonts are TrueType / OpenType (`.ttf`, `.otf`, `.ttc`); variable fonts are supported. Images are loaded as textures via the host's texture provider.

In Unity and Unreal the asset model is wrapped — `NoesisXaml` becomes a native engine asset that imports the XAML and tracks dependencies (referenced textures, fonts, sub-XAMLs). See [`engine-integration.md`](engine-integration.md) for the asset-import flow per engine.

## Exception model

The C++ core compiles with exceptions and RTTI disabled. Errors are reported via callbacks (`ErrorHandler`) rather than thrown. This is a game-engine-friendly choice (no exception-handling overhead in hot paths, no UB if an exception crosses an FFI boundary into Unity / Unreal C# or Blueprint code) but it forces an error-checking-style API on every call.

## Render passes & top-layer

Noesis owns its own render passes. Popups (`Popup`, `ToolTip`, `ContextMenu`, the visual root of a Window) escape their parent's clip rect by rendering into a top-layer-equivalent stack maintained per `View`. The host engine does not need to know about top-layer compositing — Noesis manages it internally and the host sees only the final pixel output.

This is structurally different from how bevy_ui handles popovers (`OverrideClip` opts a descendant out of inherited clipping, but there is no true top layer). Buiy commits to a true top layer in its foundation spec; the NoesisGUI precedent confirms top layer is achievable inside a UI-library-owned render path.

## Implication for Buiy

Buiy's parallel-stack architecture ([architecture § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) is *more* like Noesis than like bevy_ui in one specific way: it owns the render pipeline end-to-end. Noesis's `RenderDevice` is the "we own pixel submission" boundary; Buiy's render-graph nodes are the same boundary one layer up (wgpu does graphics-API abstraction so Buiy doesn't have to). The Framework/Integration API split is also worth studying — it lets Noesis ship one core against many engines, while Buiy commits to Bevy-only and so does not need that split. **Buiy's "Bevy-only" choice trades NoesisGUI's portability for a simpler architecture; the corpus's lessons file frames this as a deliberate scope reduction, not a missed opportunity.**

## Sources

- NoesisGUI docs index — https://www.noesisengine.com/docs/Gui.Core.Index.html
- WPF / UWP comparison — https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html
- Rendering tutorial — https://www.noesisengine.com/docs/Gui.Core.RenderingTutorial.html
- Unity tutorial — https://www.noesisengine.com/docs/Gui.Core.Unity3DTutorial.html
- Unreal tutorial — https://www.noesisengine.com/docs/Gui.Core.UnrealTutorial.html
- Technology and features — https://www.noesisengine.com/noesisgui/
