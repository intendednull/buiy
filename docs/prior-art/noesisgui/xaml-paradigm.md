**Date:** 2026-05-22
**Status:** active
**Subject:** NoesisGUI — XAML authoring, dependency properties, data binding, MVVM

# The XAML paradigm

XAML (eXtensible Application Markup Language) is the centerpiece of NoesisGUI. It is the original declarative-UI markup language from Microsoft, designed in the early 2000s for WPF (Windows Presentation Foundation, shipped with .NET 3.0 in November 2006). It later powered Silverlight, UWP, and the modern WinUI 3 stack. NoesisGUI is the **third-party reimplementation** that brings XAML to game engines.

This file documents XAML as an authoring paradigm and contrasts it with the web platform (HTML + CSS), with Bevy's authoring patterns (ECS spawning, BSN), and with what Buiy commits to.

## XAML in 60 seconds

A XAML document is XML; each element is a fully-qualified .NET / C++ type; each attribute is a property on that type; child elements are content / children:

```xml
<Grid>
    <Grid.RowDefinitions>
        <RowDefinition Height="Auto"/>
        <RowDefinition Height="*"/>
    </Grid.RowDefinitions>
    <TextBlock Grid.Row="0" Text="{Binding Title}" FontSize="24"/>
    <Button Grid.Row="1" Content="Save"
            Command="{Binding SaveCommand}"
            IsEnabled="{Binding IsDirty}"/>
</Grid>
```

The bracketed `{Binding ...}` syntax is the binding markup extension; it ties a property to a `DataContext` property at runtime, recomputing the value whenever the source notifies of a change. `Grid.Row="0"` is an **attached property** — `Row` is a property *of `Grid`* but *attached to* the `TextBlock` child. This is XAML's way of letting parent containers carry layout metadata about their children without putting layout fields on every element.

## Dependency properties

The dependency-property system is XAML's structural innovation. Every UI element extends `DependencyObject`; properties are not C# fields but registered `DependencyProperty` instances. The system gives every property:

- **Default values** (declared at registration, can be overridden per type).
- **Value precedence** (animated > local > template > style > inherited > default — value resolves at access time from the highest-priority source).
- **Inheritance** (children automatically resolve a parent's `FontFamily`, `Foreground`, `DataContext` unless overridden locally).
- **Change notification** (any change can drive a coercion callback, a validation callback, and a property-changed event that the UI tree listens to).
- **Animation** (a property can be the animation target without further plumbing).
- **Binding** (a property can be the bind target / source).

This is a powerful and a heavy primitive. Every property access goes through the dependency system; every property carries the overhead of a possibly-uncached lookup; every property is reflected for the binding / animation / styling layers to consume. The trade is: every property is uniformly addressable from XAML, Blend (the visual editor), the styling layer, and the binding layer, without per-property code.

## Data binding & MVVM

The XAML binding system pairs with the Model-View-ViewModel (MVVM) pattern: the View is the XAML; the ViewModel is a C# (or NoesisGUI-C++) object with properties and commands; the Model is the underlying domain data; bindings carry property values from ViewModel to View and command invocations from View to ViewModel.

The runtime contract:

- ViewModel implements `INotifyPropertyChanged` (raises `PropertyChanged` events when properties mutate).
- Collections implement `INotifyCollectionChanged` (raises events on add / remove / move).
- Commands implement `ICommand` (Execute + CanExecute + change notification).
- Bindings can be one-way, two-way, or one-time, with optional converters for type adaptation.

This pattern is the source of NoesisGUI's appeal to studios writing complex UIs: Larian's testimonial about Baldur's Gate 3 specifically calls out MVVM as the reason Noesis was chosen over alternatives — *"The MVVM pattern that Noesis uses is extremely flexible. It allows us to build large and complex interfaces that are easy to maintain."*

## XAML vs HTML + CSS

| Axis | XAML | HTML + CSS |
|---|---|---|
| Markup | XML, types as elements | HTML, fixed tag vocabulary + ARIA |
| Styling | Inline + Styles + Templates + Resources | Inline + classes + CSS rules + cascade |
| Layout | Grid + StackPanel + DockPanel + Canvas + Wrap | Flexbox + Grid + Block + Float + abs/sticky |
| Binding | First-class `{Binding}` markup | Frameworks add it (React, Vue, lit) |
| Animation | Storyboard + Triggers in XAML | CSS animations + transitions + Web Animations |
| Inheritance | Property inheritance built into framework | Cascading via CSS specificity |
| Standardisation | Microsoft, with NoesisGUI as a non-Microsoft impl | W3C, multiple browser implementations |
| Web parity? | No — XAML predates the web platform's UI maturity | Self |
| Tooling | Blend, Visual Studio designer, Noesis Studio | Browser devtools (universal) |

XAML is conceptually similar to a 2006-vintage React + CSS-in-XML hybrid: the declarative tree, the data binding, the styling-via-templates pattern all anticipate modern web frameworks by a decade. But the web platform has evolved since 2006 in ways XAML has not — container queries, anchor positioning, view transitions, the broader ARIA / WCAG conformance contract, complex script shaping in modern type engines. XAML's vocabulary has not grown to match.

## XAML vs Bevy ECS spawning

```rust
commands.spawn((
    Button,
    OnPress(submit),
    children![Text::new("Save")],
));
```

vs

```xml
<Button Click="OnClick">
    <TextBlock Text="Save"/>
</Button>
```

The shapes are similar but the semantics differ:

- **In XAML, `Button` is an instantiable type with a specific hierarchy of properties.** A `Button` is also a `ContentControl` is also a `Control` is also a `FrameworkElement` is also a `UIElement` is also a `DependencyObject`. Inheritance and reflected properties carry the whole chain.
- **In Bevy ECS, `Button` is a *component* on an entity.** The entity also carries `Node`, `ComputedNode`, `BackgroundColor`, etc. Composition by component aggregation, not by type-hierarchy inheritance.

Both are declarative-by-construction; both are reflection-friendly enough to support a markup format above them (XAML for Noesis, BSN for Bevy). The decomposition style — Bevy's many-small-components vs XAML's one-class-with-many-properties — is the deeper philosophical split.

## XAML vs Buiy's BSN

BSN (Bevy Scene Notation, [PR #20158](https://github.com/bevyengine/bevy/pull/20158)) is Bevy's emerging declarative markup format:

```
Button {
    on_press: Submit,
} [
    Text("Save"),
]
```

Conceptually XAML and BSN are kin (both declarative, both reflection-driven, both lay out a tree of typed elements with property attributes). But BSN inherits ECS semantics: every BSN node spawns an entity carrying a set of components, where each component is its own typed Bevy `Component`. Buiy's hard rule is *decomposed* components (per the foundation spec's [§ 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class)) — the XAML temptation of one-megaclass-per-element is structurally absent from BSN.

The XAML-style innovation Buiy *could* borrow:

- **Property inheritance.** XAML's free font / color inheritance is a real ergonomic win. Buiy could provide an opt-in inheritance system on theme tokens (already in scope via the token + theme-component approach) and on specific style properties (Font, Color) without recreating the full dependency-property machinery.
- **Markup extensions / binding syntax.** `{Binding ...}` is a clean syntax for reactive references. If BSN ever gains a similar markup-extension hook, Buiy could implement reactive bindings without a separate text DSL.
- **MVVM separation.** ViewModel-as-DataContext is good discipline. Buiy's reactivity layer (open question, [foundation § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) could be designed against an MVVM-style separation where component state is the View and a reactive resource is the ViewModel.

What Buiy should **not** borrow: the full dependency-property system. The runtime cost (dictionary-backed properties, reflected at every access) is the wrong fit for ECS, where typed components are free.

## Implication for Buiy

XAML's strength is that it has 20 years of design refinement behind it and ships in production AAA games today. Its weakness is that it predates the modern web platform (container queries, anchor positioning, modern a11y semantics) and carries a heavy runtime cost from its dependency-property system. Buiy's BSN path inherits the *ergonomic shape* of XAML (declarative, reflection-driven, attribute-property syntax) while skipping the runtime cost. The lessons file ([`lessons.md`](lessons.md)) is explicit about which patterns to borrow and which to avoid.

## Sources

- WPF / UWP comparison — https://www.noesisengine.com/docs/Gui.Core.WPFComparison.html
- Microsoft XAML overview — https://learn.microsoft.com/en-us/dotnet/desktop/xaml-services/
- WPF Dependency Properties — https://learn.microsoft.com/en-us/dotnet/desktop/wpf/properties/dependency-properties-overview
- WPF Data Binding — https://learn.microsoft.com/en-us/dotnet/desktop/wpf/data/data-binding-overview
- Larian / Baldur's Gate 3 testimonial — https://www.noesisengine.com/
- Bevy BSN PR #20158 — https://github.com/bevyengine/bevy/pull/20158
- Buiy foundation architecture § 2.4 — ../../specs/2026-05-07-buiy-foundation/architecture.md
