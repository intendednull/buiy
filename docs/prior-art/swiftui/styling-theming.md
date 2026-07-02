**Date:** 2026-06-26
**Status:** active
**Subject:** SwiftUI — Apple's declarative value-typed UI framework (modifier chains, @State/@Binding/@Observable, @ViewBuilder)

# Styling, theming & design tokens

How appearance attaches and how theme/contextual values cascade. See
[composition-state-events.md](./composition-state-events.md) for the state model
this builds on, and [lessons.md](./lessons.md) for the Buiy decisions.

## 1. Styling attaches as a modifier chain

Styling is **method chaining on an opaque view value**. Every modifier is
`func foo(...) -> some View` that *wraps* its receiver in a new view value. There
is no separate "style object" you construct and hand to the view — **the style
*is* the chain.**

```swift
Text("Save")
    .font(.headline)
    .foregroundStyle(.white)            // ShapeStyle, not Color-only
    .padding(.horizontal, 24)
    .background(.tint, in: .capsule)    // fill + clip shape in one call
```

Two consequences for Buiy:

- **Order is load-bearing and silent.** `.padding().background(.red)` colors the
  padded area; `.background(.red).padding()` colors only the text. No error — just a
  different picture. The chain encodes a **wrapping order** an unordered property set
  cannot. (Hacking with Swift, *Why modifier order matters*)
- **Uniform shape.** Every modifier has the identical `View -> View` signature, so
  they compose without per-property ceremony. This uniformity is exactly what Buiy's
  `Style`-is-a-`Bundle` problem (F4) lacks.

`foregroundStyle`/`background`/`tint` all take **`ShapeStyle`**, so "a color", "a
material", and "a gradient" are one interchangeable type at the call site.

## 2. Style protocols + cascade through the environment

For *control* appearance (not ad-hoc layout) SwiftUI uses a **style-protocol
pattern**: `ButtonStyle`, `ToggleStyle`, `LabelStyle`, `PickerStyle`. You implement
`makeBody(configuration:)`, receiving a typed `Configuration` (the control's
`label`, `isPressed`, `role`, …), and return a styled view.

```swift
struct PrimaryButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding().foregroundStyle(.white)
            .background(configuration.isPressed ? .blue.opacity(0.7) : .blue,
                        in: .capsule)
    }
}
// applied — and it CASCADES down the hierarchy through the environment:
ContentView().buttonStyle(PrimaryButton())   // every Button in the subtree
```

Mechanically `.buttonStyle(_:)` writes the style into an `EnvironmentKey`; relevant
descendants read it back out — "like `font(_:)` and `tint(_:)`… applying that style
to all relevant views within that view hierarchy" (Moving Parts). A `.automatic`
style resolves per-platform. **The cost:** because a style protocol isn't itself a
`View`, a style that needs environment values must route through a `DynamicProperty`
wrapper view — extra indirection is the price of "styles live in the environment."

## 3. Theming / design tokens: `EnvironmentValues` + `@Entry`

This is SwiftUI's design-token model, and it is **typed, compiler-checked,
defaulted, and cascading** — the antithesis of stringly-typed tokens (F6). Before
`@Entry` you hand-wrote an `EnvironmentKey` + getter/setter; the `@Entry` macro
(Xcode 16, 2024; generated code back-ports to iOS 13) collapses it to one line:

```swift
extension EnvironmentValues {
    @Entry var brandTint: Color = .accentColor   // typed key + default, no boilerplate
}
ContentView().environment(\.brandTint, .indigo)  // set for a subtree (cascades)

struct Badge: View {
    @Environment(\.brandTint) private var brandTint   // read; default applies if unset
    var body: some View { Text("New").foregroundStyle(brandTint) }
}
```

Properties worth stealing:

- **Typed key paths, not strings.** `\.brandTint` is a `KeyPath`; a typo is a
  compile error, the value type is fixed, autocomplete works.
- **Mandatory default ⇒ total.** Every key has a default, so *every* read resolves —
  no "missing token" runtime hole.
- **Per-subtree override (cascade).** Setting `.environment(\.brandTint, …)`
  re-themes only that subtree; descendants inherit the nearest binding.
- **Same machinery as built-ins.** Custom tokens sit beside `\.colorScheme`,
  `\.layoutDirection`, `\.dynamicTypeSize`; even Liquid Glass (iOS 26,
  `glassEffect(_:in:)`) delivers system theming through this same environment
  cascade — your view "responds to the system Liquid Glass slider without any
  changes on your part" (arshtechpro).

**Honest completeness caveat:** there is no *enumerated, exhaustively-checked* token
palette — `@Entry var` lets anyone add any key with any default. The guarantee is
"every *declared* key resolves," **not** "the design system is closed."

## 4. The controlled `$binding` convention (cross-ref)

Controls take a **typed two-way `Binding<T>`** minted with the `$` projected value
(`TextField("Name", text: $user.name)`), not untyped events. Forgetting the `$`
(passing the value instead of the binding) is a canonical trigger of the §6
type-check blowup — it usually fails to compile rather than silently mis-binding.
Full treatment in [composition-state-events.md](./composition-state-events.md) § 3.

## 5. Accessibility as **output modifiers** (the F1 evidence)

The unique point for *this* file: accessibility attaches through the **same
modifier-chain mechanism as visual styling** (§1) — `.accessibilityLabel/Value/Hint`
are `View -> View` output modifiers that *describe* `@State`/`@Observable` (the
source of truth) for the platform a11y tree, re-projected each `body`, never a
separate tree the author hand-maintains. That a11y rides the identical machinery as
appearance is the direct evidence that "app STATE separate from the a11y tree" is
workable and clean. Full treatment + the `Slider` example in
[composition-state-events.md](./composition-state-events.md) § 4.

## 6. Sharp edges

The opaque-type compile-blowup and "where did my state go" edges are shared with the
core DX and **owned** in [composition-state-events.md](./composition-state-events.md)
§ 5 (a missing `$` is a canonical blowup trigger, as in §4 above). Two edges specific
to styling / theming:

- **Silent modifier order** (§1) — `.padding().background()` ≠
  `.background().padding()`, a visual-only divergence that produces no diagnostic, so
  it is found only by eye.
- **WWDC26 `ContentBuilder`.** A unified result-builder said (per third-party
  roundups — **not** independently confirmed in primary Apple material) to replace the
  per-container `@ViewBuilder` overloads, introduced precisely to claw back the
  compile time — and, if real, to lift the 10-child `buildBlock` ceiling noted in
  [composition-state-events.md](./composition-state-events.md) § 5.

## Sources

- https://www.hackingwithswift.com/books/ios-swiftui/why-modifier-order-matters (modifier order is load-bearing)
- https://www.hackingwithswift.com/quick-start/swiftui/customizing-button-with-buttonstyle (ButtonStyle)
- https://movingparts.io/styling-components-in-swiftui (style protocol + environment cascade)
- https://www.avanderlee.com/swiftui/entry-macro-custom-environment-values/ (@Entry typed tokens)
- https://useyourloaf.com/blog/entry-macro-for-custom-swiftui-environment-values/ (@Entry boilerplate elimination)
- https://www.hackingwithswift.com/quick-start/swiftui/how-to-create-and-use-custom-environment-values (custom EnvironmentValues, back to iOS 13)
- https://www.apple.com/newsroom/2025/06/apple-introduces-a-delightful-and-elegant-new-software-design/ (Liquid Glass, iOS 26)
- https://dev.to/arshtechpro/wwdc26-whats-new-in-swiftui-a-developers-breakdown-1333 (WWDC26 @State macro, ContentBuilder claim)
- https://sarunw.com/posts/how-to-fix-the-compiler-is-unable-to-type-check-this-expression-in-reasonable-time/ (type-check blowup)
- https://developer.apple.com/documentation/swiftui/buttonstyle (ButtonStyle docs — JS-rendered)
