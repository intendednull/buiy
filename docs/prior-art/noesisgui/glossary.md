**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — terminology glossary

# Glossary

NoesisGUI-specific and XAML-derived terminology used across this corpus.

| Term | Definition |
|---|---|
| **AccessKit** | Cross-platform Rust accessibility-tree bridge. NoesisGUI does *not* use it. Buiy commits to it. See [`accesskit/`](../accesskit/). |
| **APG (ARIA Authoring Practices Guide)** | W3C document describing keyboard interaction patterns for common widgets. NoesisGUI does not document widget conformance against APG. |
| **Attached property** | XAML pattern where a property is declared on a *parent* type but attached to a *child* element instance. Example: `Grid.Row="0"` on a child of a `Grid` — `Row` belongs to `Grid`, attached to the child. |
| **Binding (`{Binding ...}`)** | XAML markup extension that ties a property value to a `DataContext` property. One-way / two-way / one-time directions; runs through the dependency-property system. |
| **Blend (Microsoft Blend for Visual Studio)** | The de-facto WYSIWYG XAML editor. Windows-only. Used by most professional Noesis customers. |
| **BSN (Bevy Scene Notation)** | Bevy's emerging declarative markup format ([PR #20158](https://github.com/bevyengine/bevy/pull/20158)). Reflection-driven, similar declarative shape to XAML, but ECS-native. Still in draft. |
| **C++ SDK** | The native C++ build of NoesisGUI. Used for custom-engine integrations. |
| **`DataContext`** | The object whose properties bindings resolve against. In Unity, can be a MonoBehaviour; in Unreal, a UObject; in C++, any binding-target object. |
| **`DependencyObject`** | Base class for all XAML elements. Provides the dependency-property system. |
| **`DependencyProperty`** | A registered property type with default value, value-precedence, change-notification, animation-target, and binding-target capabilities. The structural primitive of XAML. |
| **Framework API** | NoesisGUI's high-level engine-agnostic API: Controls, Panels, dependency system, XAML loader. |
| **Hot-reload** | Live update of XAML files (and as of 3.2.11, templates with instance-state preservation) without restarting the application. |
| **Integration API** | NoesisGUI's low-level engine-specific API: `RenderDevice`, texture providers, font providers, URI resolution. Host engines implement this. |
| **Lottie** | After Effects-derived JSON animation format. Noesis supports via the `Lottie-Noesis` Python tool that converts to XAML. |
| **Managed (Noesis.GUI)** | The .NET C# SDK of NoesisGUI, published as a NuGet package. Used in Unity and standalone .NET hosts. |
| **MeshGeometry** | A pre-tessellated path geometry type added in 3.1+ for performance — skip the runtime tessellation step. |
| **MVVM (Model-View-ViewModel)** | UI architecture pattern. View = XAML, ViewModel = C# object with properties + commands, Model = domain data. Larian's testimonial calls this out as the BG3 architecture reason. |
| **`NoesisView`** | The integration class that hosts a XAML tree. In Unity it's a MonoBehaviour; in Unreal it's a UMG UWidget; in C++ it's a renderable view object. |
| **`NoesisXaml`** | An asset type representing an imported XAML file. Tracks dependencies on referenced textures, fonts, sub-XAMLs. |
| **Noesis Studio** | The next-generation visual XAML editor. "Coming 2024" beta as of corpus date; not yet GA. Cross-platform. |
| **`RenderDevice`** | The abstract C++ interface that the host engine implements to provide GPU command submission. The seam between Noesis and the host's graphics API. |
| **Rive** | A modern vector-animation file format (`.riv`). Noesis ships `RiveControl` (since 3.2) to embed Rive animations as UI elements. |
| **Single-pass stereo rendering** | Render UI to both VR eye buffers simultaneously rather than twice. Added in 3.2. |
| **Tessellation** | Conversion of vector paths into triangulated meshes for GPU rendering. Noesis tessellates per frame; `MeshGeometry` skips this step. |
| **UIA (UI Automation)** | Microsoft's Windows accessibility framework. WPF integrates with UIA natively; NoesisGUI does not produce a UIA tree. |
| **UMG (Unreal Motion Graphics)** | Unreal's native UI framework. NoesisGUI `NoesisView` is a UMG `UWidget`, so Noesis content can be embedded anywhere a UMG widget is used. |
| **`UIElement` / `FrameworkElement`** | Base XAML visual element classes. Inherit dependency-object behaviour, add layout + render semantics. |
| **VirtualizingWrapPanel** | A panel that virtualizes children for large collections (only renders visible items). Added in 3.2.9. |
| **World Space UI** | XAML UI rendered directly in 3D world space (no render-to-texture). Added in 3.2. |
| **WPF (Windows Presentation Foundation)** | Microsoft's XAML-based UI framework for .NET, shipped 2006. NoesisGUI's spiritual ancestor; Noesis is a from-scratch C++ reimplementation of WPF-style XAML for game engines. |
| **XAML (.xaml)** | eXtensible Application Markup Language. The XML-based declarative UI language designed by Microsoft. NoesisGUI's authoring layer. |
| **`.bxml`** | Pre-parsed binary XAML format used in shipping builds to skip XML parsing at load time. |

## Sources

- NoesisGUI docs index — https://www.noesisengine.com/docs/Gui.Core.Index.html
- WPF / UWP comparison — https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html
- Bevy BSN PR #20158 — https://github.com/bevyengine/bevy/pull/20158
- Microsoft XAML overview — https://learn.microsoft.com/en-us/dotnet/desktop/xaml-services/
