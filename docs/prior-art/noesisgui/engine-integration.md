**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — Unity, Unreal, and custom-engine integration patterns

# Engine integration

NoesisGUI's value proposition rests on shipping **one** UI runtime that integrates cleanly with multiple game engines. The integration pattern is consistent: each engine gets a thin plugin that wraps the C++ core, provides a `RenderDevice` implementation for the host's graphics API, exposes XAML files as native engine assets, and routes input + data binding through engine-idiomatic channels (Blueprint in Unreal, MonoBehaviour in Unity).

This file documents the three primary integration paths.

## Unity integration

The Unity package is published as an asset-store entry and a downloadable Unity package; users install it via the Unity Package Manager from a local folder. Minimum Unity version is **2020.2**, with the latest 3.2.13 supporting Unity 6.3 and 6.4 (April 2026).

### Architecture

The core integration point is the **`NoesisView` MonoBehaviour**: attach it to a GameObject, set its `Xaml` field to a XAML asset, and the GameObject's camera renders the XAML at runtime. `NoesisView` handles render-thread integration, input forwarding, and data binding.

The C# wrapper around the C++ core exposes `DependencyObject`, `FrameworkElement`, and the Framework API to Unity's managed code. Calls cross the C++ / C# boundary via P/Invoke; the managed layer's API mirrors the C++ API name-for-name.

### Render pipeline integration

Noesis supports all three Unity render pipelines explicitly:

- **Built-in Render Pipeline.** Post-processing affects UI by default. Users add a second camera with higher depth and "Don't Clear" flags to layer UI after post-effects.
- **URP (Universal Render Pipeline).** Camera-stacking: the UI camera is set to `Overlay` mode, post-processing disabled, and placed last in the stack. (Latest URP support added in 3.2.12 / 3.2.13.)
- **HDRP (High Definition Render Pipeline).** UI is unaffected by post-processing by default; an `Injection Point` property modifies this.

Each pipeline gets its own `RenderDevice` selection — on built-in pipelines this is the legacy GfxDevice path, on URP/HDRP this is the SRP-aware path.

### DataContext binding

The recommended Unity authoring pattern: a **MonoBehaviour acts as the DataContext** for a `NoesisView`. Properties on the MonoBehaviour become bindable from XAML; methods can be wired to XAML commands via `NoesisEventCommand` (which bridges UnityEvents and `ICommand`).

```csharp
public class MyViewModel : MonoBehaviour, INotifyPropertyChanged {
    private string _title;
    public string Title {
        get => _title;
        set { _title = value; NotifyPropertyChanged(nameof(Title)); }
    }
    public event PropertyChangedEventHandler PropertyChanged;
    private void NotifyPropertyChanged(string name) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
```

```xml
<TextBlock Text="{Binding Title}"/>
```

The MonoBehaviour-as-DataContext binding is the closest Unity gets to a true MVVM pattern; it works because Unity's Inspector can serialise MonoBehaviour fields, which then become live bindable properties at runtime.

### Asset import

XAML files placed in the project's Assets folder are auto-imported as `NoesisXaml` ScriptedImporter assets. Font files (`.ttf`, `.otf`, `.ttc`) get dedicated importers with preview windows. Dependencies are auto-detected by **naming convention** (e.g., `<local:MyUserControl>` in XAML maps to `MyUserControl.xaml` on disk), so the package build process collects referenced sub-XAMLs, textures, fonts, audio clips, video clips, shaders, and Rive assets automatically.

### Input integration

Noesis uses Unity's **Input Actions Assets** (newer Input System Package) for gamepad mapping. Keyboard, mouse, and touch use the legacy input system; this hybrid posture reflects Unity's own input-system fragmentation.

## Unreal integration

The Unreal plugin (`UnrealPlugin`, [github.com/Noesis/UnrealPlugin](https://github.com/Noesis/UnrealPlugin), C++, 353 stars, last updated 2026-04-27) ships as a Game Plugin or Engine Plugin. The latest 3.2.13 supports **UE 5.7** (5.7 support added in 3.2.11, February 2026).

### Architecture

The integration point is **`NoesisView`**, which is itself an Unreal Engine **UMG `UWidget`**. This is significant: a `NoesisView` can be embedded anywhere a native UMG widget is used. The docs framing — *"NoesisGUI entirely replaces Unreal UI offering a much more convenient and efficient approach"* — overstates the case in practice; the typical Noesis-Unreal app uses Noesis for the main UI surfaces *inside* a UMG host widget that handles whole-screen layout, with UMG itself still running for editor / debug overlays.

### Asset import

Files in the project's Content folder are imported as native Unreal assets:

- `.xaml` → `NoesisXaml` asset.
- Image files → `Texture2D` asset.
- Font files → `FontFace` asset.

URI references in XAML resolve relative-or-absolute paths against the Content folder. Hot-reload of XAML works against the editor's normal asset-reimport flow.

### Blueprint data binding

Noesis exposes a Blueprint binding model that does not require C++:

- **Variables** declared `BlueprintReadWrite` or `BlueprintReadOnly` on a `UObject` can be bound to XAML properties.
- **Functions** declared `BlueprintCallable` can be invoked as XAML commands.
- The instantiated `NoesisView` is the default `DataContext`; users can override with `Set Data Context` Blueprint node.
- The plugin ships custom Blueprint nodes — *"Set w/ NotifyChanged"* — that replace standard `Set` operations and emit the property-change notification XAML bindings require.
- Since UE 5.1, Noesis also supports Unreal's native `INotifyFieldValueChanged` interface (UMG ViewModel support).

### Render thread integration

Noesis renders inside Unreal's standard render thread via the RHI abstraction. The `stat Noesis` console command exposes Noesis-specific timing categories (input processing, view updates, render-command generation). GPU rendering is profiled normally via `stat GPU`.

### Input integration

The plugin provides default Input Action Assets for gamepad input and a default Enhanced Input mapping context the player controller can adopt. Keyboard / mouse follow Unreal's standard input pipeline.

## Custom C++ engine integration

For studios with their own engines (or proprietary forks), Noesis exposes the C++ SDK directly. The integration contract is:

1. **Implement `RenderDevice`** against the engine's graphics API. Reference implementations exist for D3D11, D3D12, Metal, Vulkan, OpenGL, GLES.
2. **Provide asset loaders** for XAML, fonts, textures via the integration API (texture provider, font provider, XAML provider).
3. **Call `View::Update(time)` on the logic thread** each frame.
4. **Call `View::Render()` on the render thread** with the `RenderDevice` each frame.
5. **Inject input events** via `View::MouseMove`, `View::KeyDown`, etc.
6. **Implement `IComponentInitializer`** if your engine needs to initialise Noesis types at engine boot.

The custom-engine path is what large in-house studios use (the customer list suggests several do). It is also what enables platforms like **iRacing**, **DLSS-class simulation tooling**, and the industrial / simulation sector applications listed on Noesis's customer page.

## Managed (.NET) integration without an engine

Beyond engine plugins, NoesisGUI publishes a **managed C# SDK** (`Noesis.GUI`, [github.com/Noesis/Managed](https://github.com/Noesis/Managed), 107 stars, updated 2026-04-27, published to NuGet as `Noesis.GUI 3.2.13`). This is for .NET applications that want WPF-comparable behaviour outside Windows — Avalonia is the closest free analogue, but Noesis can host XAML inside non-Windows .NET hosts without depending on Windows-only APIs.

This path is less commonly used in games (almost always a game host implies an engine) but matters for industrial simulation tooling where a .NET host runs without an engine wrapper.

## Comparison: per-engine binding overhead

| Engine | Plugin LOC (approx) | Maintenance burden owner |
|---|---|---|
| Unity | ~25K C# + glue | Noesis Technologies |
| Unreal | ~30K C++ + glue | Noesis Technologies |
| C++ direct | minimal (user-owned) | Customer |
| C# managed | minimal | Noesis Technologies (NuGet) |

The **per-engine binding overhead is the structural cost of Noesis's cross-engine model**: every supported engine adds a long-tail maintenance commitment. Each Unity / Unreal minor release is a migration event; the 3.2.x patch releases (3.2.8 added UE 5.6 + Unity 6.1; 3.2.9 added Switch 2; 3.2.10 fixed cross-platform issues; 3.2.11 added UE 5.7; 3.2.12 added VS 2026 / TimeSpan; 3.2.13 added Unity 6.3-6.4 + Xcode 26 + DirectComposition) are dominated by engine-version-bump work, not core-framework work.

## Implication for Buiy

Buiy commits to **Bevy-only** ([foundation README § 1 goals](../../specs/2026-05-07-buiy-foundation/README.md#buiys-goals-the-product), specifically *non-goal: non-Bevy frontends*). This trades NoesisGUI's portability advantage (one runtime, three+ engines) for a structural simplification (no per-engine plugin, no per-engine release cadence, no version-pin trap when Unity 6.5 ships).

The cost: a Bevy game that switches engines cannot bring Buiy with it. The trade is correct for Buiy's scope (open-source library tracking Bevy) and incorrect for Noesis's scope (commercial middleware selling to engine-agnostic AAA studios). Each project's architecture is right for its own goals; the lesson is: **don't try to be both a Bevy-native library and engine-portable middleware.** They are different products.

Specific Noesis patterns Buiy still wants:

- **Per-engine idiomatic data binding** — MonoBehaviour-as-DataContext in Unity, Blueprint-property-bindings in Unreal. Buiy's analog is "Bevy ECS components are the DataContext" — Buiy doesn't need anything else because Bevy's reflection + change-detection + observers already provide it.
- **Asset-import-by-convention** — Buiy's asset pipeline ([open question: asset pipeline sub-spec](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) should treat `.bsn` and theme assets like Noesis treats `.xaml`: automatic dependency detection, hot-reload, dedicated importer.

## Sources

- Unity tutorial — https://www.noesisengine.com/docs/Gui.Core.Unity3DTutorial.html
- Unreal tutorial — https://www.noesisengine.com/docs/Gui.Core.UnrealTutorial.html
- Noesis Unreal plugin repo — https://github.com/Noesis/UnrealPlugin
- Noesis Managed SDK repo — https://github.com/Noesis/Managed
- Noesis Tutorials repo — https://github.com/Noesis/Tutorials
- 3.2 changelog — https://www.noesisengine.com/docs/Gui.Core.Changelog.html
- Buiy foundation goals — ../../specs/2026-05-07-buiy-foundation/README.md
