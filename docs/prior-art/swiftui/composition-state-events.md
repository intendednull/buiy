**Date:** 2026-06-26
**Status:** active
**Subject:** SwiftUI — Apple's declarative value-typed UI framework (modifier chains, @State/@Binding/@Observable, @ViewBuilder)

# Composition · State · Events (the core DX)

How SwiftUI composes UI, models state, and propagates change. See
[architecture.md](./architecture.md) for the runtime vs convention split,
[styling-theming.md](./styling-theming.md) for the modifier-chain/theming detail,
and [lessons.md](./lessons.md) for the Buiy decisions distilled from this.

## Version provenance (these get hallucinated)

- **iOS 17 / WWDC 2023** — Observation: `@Observable` **macro** (replaces
  `ObservableObject`/`@Published`), `@Bindable`, per-property tracking; classes only.
- **iOS 18 / WWDC 2024** — `@Entry` macro: removes `EnvironmentKey` boilerplate
  (back-deploys to iOS 13).
- **iOS 26 / WWDC 2025** — year-based OS naming; Swift 6.2 `Observations`
  (`AsyncSequence` of transactional changes), `@Animatable`, rich-text `TextEditor`.
- **WWDC 2026 / Xcode 27** — **`@State` becomes a macro**; initial value evaluated
  **lazily, once per view lifetime** (back-deployed to iOS 17), fixing the
  "model re-constructed on every parent re-eval" footgun.

Net trajectory: Apple keeps **moving the magic from property wrappers into macros**.

## 1. Composition

**Views are values; `body: some View`.** A view is a `struct` with one computed
`body` returning an **opaque** `some View` (one concrete, often enormous generic
type, e.g. `VStack<TupleView<(Text, …)>>`) — the seed of the §5 compile hazard.

**The modifier chain (the F4 reference design).** Styling/layout is a fluent chain;
each modifier takes a view and returns a **new wrapping view value** — no mutable
style object. Order is value-semantic (`.padding().background()` ≠
`.background().padding()`). The `ViewModifier` + `.modifier(_:)` escape hatch makes
"a reusable bundle of styling" a first-class **value**, not a special builder type.
Detail in [styling-theming.md](./styling-theming.md).

**Children / slots via `@ViewBuilder` (the F8 reference design).** Containers take
children as a result-builder closure that turns `{ a; b; if c { d } }` into one
`Content` value — no `return`, commas, or arrays. Reuse the exact mechanism for
your own containers; multiple named slots = multiple `@ViewBuilder` params
(`label:`/`content:`, as in `Toggle(isOn:label:)`, `Section(header:footer:content:)`).

```swift
struct Card<Content: View>: View {
    @ViewBuilder var content: () -> Content        // the "slot"
    var body: some View {
        VStack(alignment: .leading, spacing: 8) { content() }
            .padding().background(.thinMaterial, in: .rect(cornerRadius: 12))
    }
}
Card { Text("Title").font(.title3); if showBody { Text("Body copy") } }
```

**Dynamic / data-driven lists.** `ForEach`/`List` keyed by **identity** (`Identifiable`
or explicit `id:`), not index. On change the body re-runs, SwiftUI diffs by `id`,
and inserts/moves/removes the minimum. **Dynamic content is data → identity-diff,
not imperative tree mutation** — precisely the capability Buiy's retained scenes
lack today (F7: "no dynamic content in scenes").

## 2. State

Central decision: **app state is value-typed and lives entirely outside the
render/accessibility tree**; the body is a pure-ish `state → view`. Three ownership
tiers plus ambient context:

- **`@State`** — local, view-owned source of truth; `$count` projects a
  `Binding<Int>`; the backing store outlives the struct.
- **`@Binding`** — delegated two-way reference for a child that must read+write state
  it doesn't own; parent passes `$value`, no copy is made.
- **`@Observable` + `@Bindable`** — shared reference models. `@Observable`
  auto-tracks per property (SwiftUI subscribes only to properties a view **actually
  reads**); `@Bindable` mints `$model.property` bindings into a model you don't own.

```swift
@Observable class AppSettings { var hidesTitles = false }

struct RootView: View {
    @State private var settings = AppSettings()      // owns the model
    var body: some View { SettingsForm(settings: settings) }
}
struct SettingsForm: View {
    @Bindable var settings: AppSettings
    var body: some View { Toggle("Hide titles", isOn: $settings.hidesTitles) }
}
```

Ownership rule: `@State` *creates/owns*; `let`/`@Bindable` *borrows*; `@Environment`
*receives* ambiently. The fourth tier, **`@Environment` / `@Entry`** (ambient
theming, the F6 reference design), is a typed, key-path-indexed cascade with
**compiler-checked keys, never strings** — full detail in
[styling-theming.md](./styling-theming.md).

## 3. Events — how change propagates

For downward and local change SwiftUI has **three** mechanisms, in order of
preference, and **no global event bus and no per-widget `ChangedEvent` type** — plus
one sanctioned **upward** (child → ancestor) channel, `PreferenceKey` (item 4):

1. **Two-way `Binding<T>`** (the F2 reference design) — the default. Controls take a
   `Binding<T>` *typed by their value* (`TextField(text:)`, `Toggle(isOn:)`,
   `Slider(value:)`); the binding **is** the bidirectional change channel. Writing
   through `$value` mutates the owner's storage and invalidates exactly the readers.
   For side-effects use `.onChange(of:) { old, new in … }` (the supported hook;
   `didSet` on `@State` is explicitly *not* reliable). For transform/validation,
   build a custom `Binding(get:set:)` — a binding is just a get/set pair.
2. **Imperative action closures** — momentary, valueless events: `Button { … }`,
   `.onSubmit { … }`, `.onTapGesture { … }`. Callbacks, not bus events.
3. **Observation invalidation** — mutating an `@Observable` property re-runs the
   bodies that read *that* property, or feeds `onChange`/`Observations`.
4. **Upward via `PreferenceKey`** (child → ancestor) — the sanctioned channel for a
   child to *report a value up* the tree (measured size, a title, a scroll offset,
   badge counts) without a shared binding. A descendant sets `.preference(key:value:)`;
   an ancestor reads the reduced result with `.onPreferenceChange { … }` (or, for
   geometry, anchor preferences via `.anchorPreference`/`.transformAnchorPreference`
   resolved through a `GeometryReader`). The runtime folds sibling values through the
   key's `reduce` while diffing, so reporting-up stays **declarative**, not an
   imperative callback walked up the stack — the upward dual of the downward
   `Binding<T>`. A Buiy DX decision on widget→parent reporting (size, badges, scroll
   offset) should weigh this datapoint.

```swift
TextField("Name", text: $name)         // $name : Binding<String>
Slider(value: $volume, in: 0...1)      // Binding<Double>
    .onChange(of: volume) { old, new in persist(new) }
```

*Deliberately out of scope here:* the `Layout` protocol (custom layout containers)
and the `Animatable`/animation system — Buiy owns layout via Taffy
([open-problems.md](./open-problems.md) § 7) and drives animation on its own ECS
clock, so SwiftUI's mechanisms there don't transfer and are not surveyed.

**The value flow *is* the event flow** — the deliberate counter-design to Buiy's
"one untyped `OnPress(Entity)`." `Observations { … }` (Swift 6.2 / iOS 26)
additionally exposes the same observation graph as a **pull-based `AsyncSequence`**,
coalescing synchronous changes into one transactional update — the "observe a
derived value, get a coalesced signal" primitive an ECS change-detection system
would emit.

## 4. Accessibility as a derived OUTPUT (the F1 reference design)

Buiy's open question: *"should app STATE be separate from the AccessKit tree?"*
**SwiftUI's answer is an emphatic yes, structurally:**

- App state (`@State` values, `@Observable` models) is the **single source of
  truth** and knows nothing about accessibility.
- The body computes a render tree; accessibility is a **projection of that tree via
  output modifiers** — `.accessibilityLabel/Value/Hint`, `.accessibilityElement`,
  `.accessibilityAddTraits`.
- Standard controls **synthesize a11y from their own value/role for free**; you
  annotate only to override/enrich.

```swift
Slider(value: $volume, in: 0...1)
    .accessibilityLabel("Volume")
    .accessibilityValue("\(Int(volume * 100)) percent")
```

The a11y tree is a **third projection of state, peer to the visual render** —
derived each `body`, never hand-maintained. No "domain layer == a11y tree" collapse.
Its honest cost (silent-empty a11y when authors forget the modifiers) is in
[open-problems.md](./open-problems.md).

## 5. Sharp edges

- **Opaque-type compile blowups.** Long chains/deep nesting encode the subtree into
  one giant inferred generic behind `some View`; past a threshold the type-checker
  emits *"unable to type-check this expression in reasonable time."* Mundane
  triggers: a missing `$` (value where a `Binding` is expected), too many modifiers.
- **"Where did my state go" footguns.** Wrong ownership wrapper silently
  destroys/recreates state on re-render — no error, just a reset (pre-Xcode-27, an
  `@Observable` in `@State` re-ran its initializer on every parent re-eval).
- **10-child `buildBlock` ceiling** — `@ViewBuilder` overloads cover only 1–10
  children; exceeding forces `Group`/refactor. (WWDC26's unconfirmed `ContentBuilder`
  may lift this — see [styling-theming.md](./styling-theming.md) § 6.)

See [open-problems.md](./open-problems.md) for the structural (not-fixable) edges.

## Sources

- https://developer.apple.com/documentation/swiftui/migrating-from-the-observable-object-protocol-to-the-observable-macro (Observable migration)
- https://developer.apple.com/tutorials/swiftui-concepts/driving-changes-in-your-ui-with-state-and-bindings (state & bindings)
- https://developer.apple.com/documentation/swiftui/viewbuilder (ViewBuilder); https://www.avanderlee.com/swiftui/viewbuilder/ (→ TupleView)
- https://www.donnywals.com/observable-in-swiftui-explained/ (@Observable per-property tracking)
- https://swiftwithmajid.com/2024/09/24/mastering-container-views-in-swiftui-basics/ (container views / slots)
- https://swiftwithmajid.com/2025/07/30/streaming-changes-with-observations/ (Observations AsyncSequence)
- https://nilcoalescing.com/blog/InitializingObservableClassesWithTheStateMacroInXcode27/ (@State macro, Xcode 27); https://fatbobman.com/en/posts/lazy-initialization-state-in-swiftui/ (lazy-init footgun)
- https://www.hackingwithswift.com/books/ios-swiftui/responding-to-state-changes-using-onchange (onChange); https://www.hackingwithswift.com/quick-start/swiftui/how-to-create-custom-bindings (custom Binding)
- https://www.hackingwithswift.com/quick-start/swiftui/two-way-bindings-in-swiftui ($ projected Binding)
- https://sarunw.com/posts/how-to-fix-the-compiler-is-unable-to-type-check-this-expression-in-reasonable-time/ (type-check blowup)
