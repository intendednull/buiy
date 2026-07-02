**Date:** 2026-06-26
**Status:** active
**Subject:** SwiftUI — Apple's declarative value-typed UI framework (modifier chains, @State/@Binding/@Observable, @ViewBuilder)

# Architecture, runtime & distribution

See [README.md](./README.md) for the key-facts table and framing. This file
separates the **runtime mechanism** (not portable — Buiy must supply its own
equivalent on ECS) from the **API-surface convention** (portable — what Buiy can
adopt around `bsn!`).

## What it is

SwiftUI describes UI as a tree of immutable **value-type `View` structs** whose
`body: some View` computed property returns more views. Composition is value
composition: the body is rebuilt cheaply and diffed. A private runtime tracks
which state each `body` reads, re-invokes only the affected closures on change,
diffs the new ephemeral value tree against a retained shadow tree, and mutates the
minimal set of backing objects. It ships only inside the OS SDKs bundled with
Xcode, versioned in lockstep with the annual OS releases.

## RUNTIME mechanism — NOT portable (Buiy must build its own on ECS)

- **AttributeGraph** — a private, undocumented C++ dependency graph. Each property
  a `body` reads becomes an attribute node; writing state invalidates downstream
  nodes and schedules recomputation. This is the reactive engine ("Effect
  Graph"/"Dependency Graph" in talks are nicknames). It is reverse-engineered, not
  spec'd — prior-art on the runtime is inference, not ground truth.
  (rensbr.eu, kyleye.top)
- **View identity & lifecycle** — SwiftUI maps ephemeral value-type structs to
  stable backing "shadow" objects keyed by **structural position + explicit
  `.id()`**. A type/identity mismatch destroys and rebuilds a subtree (and resets
  its state). (DoorDash eng, rensbr.eu)
- **Diffing engine** — `body` runs to produce a new value tree that exists only
  long enough to be compared against the previous one; cheap because views are
  value types. (Apple "What's new in SwiftUI", malcolmhall.com)
- **`@State` storage lifecycle** — the backing store **outlives the struct**;
  recreating the struct does not recreate the state. As of WWDC 2026 / Xcode 27,
  `@State` became a **macro** so an `@Observable` stored in it initializes
  **lazily, once per view lifetime** (back-deployed to iOS 17). (arshtechpro,
  nilcoalescing, byteiota)

The whole runtime turns on **re-running `body` per change**. Buiy is **retained,
spawn-once**, so none of this re-derives for free — Buiy's equivalent of every
"derived output" (render tree, a11y tree, dependent values) must be an explicit
**change-detected system** over ECS components, not a re-run.

## API-SURFACE convention — PORTABLE (adopt around `bsn!`)

Each item is tagged with the Buiy friction it informs (**F1..F8 defined in
[lessons.md](./lessons.md)**):

- **Modifier chain** — `.modifier()` methods that each return a wrapping view;
  ordering is semantic. Naming/composition convention over the graph. → **F4**
- **State separation triad** — `@State` (owned local), `@Binding` (borrowed
  two-way), `@Observable` (shared reference model; Observation/SE-0395, iOS 17+).
  The *vocabulary of ownership* is portable; the auto-tracking is runtime.
  → **F1** (state lives *outside* the view/a11y tree — the precondition for
  treating a11y as a derived output; the consolidate-the-spellings angle is **F5**,
  the two-way handle **F2**).
- **`$` projected binding** — `$value` exposes a `Binding<T>`; parent stays source
  of truth, child mutates through it. A controlled-component convention. → **F2**
- **`EnvironmentValues` + `@Entry`** — typed, inherited ambient context; `@Entry`
  (Xcode 16 / iOS 18, back to iOS 13) collapses custom-key boilerplate. → **F6**
- **`@ViewBuilder`** — a Swift *language* result builder (not SwiftUI-private)
  whose `buildBlock` overloads turn a multi-statement closure into a `TupleView`;
  children-as-closure with inline `if`/`switch`. → **F8**
- **Accessibility-as-output-modifiers** — `.accessibilityLabel`/`Value`/… layer
  semantics onto the *same* view tree; a11y is a **projection** of the view, never
  a separate tree the developer maintains. → **F1**

## Distribution / versioning / governance

- **Single vendor, no community governance.** Apple defines, implements, and ships
  SwiftUI; there is no public proposal/review process for the framework (contrast
  the Swift *language*, governed openly on swiftlang/swift-evolution — that is
  where SE-0395 Observation lives, but the framework binding is Apple-internal).
- **Versioned with the OS SDK, not independently.** You get a feature set by
  targeting a deployment OS + matching Xcode/Swift toolchain. No dependency line;
  back-deployment is per-API and inconsistent (`@Entry` back to iOS 13; the
  `@State`-macro lazy-init back to iOS 17; many APIs gated to their ship OS).
- **Closed-source.** No source to read; behavior is reverse-engineered (rensbr.eu,
  objc.io Swift Talk on AttributeGraph). Only the public API conventions are
  documented ground truth.

**Buiy contrast:** open-source, ECS-native, retained. Distribution and governance
are inverted (open RFC-able vs single-vendor), but the *technical* lesson is the
runtime/convention split above: copy conventions, rebuild the engine.

## Sources

- https://en.wikipedia.org/wiki/SwiftUI (license, release, platforms, distribution)
- https://rensbr.eu/blog/swiftui-attribute-graph/ (AttributeGraph runtime)
- https://rensbr.eu/blog/swiftui-diffing/ (diffing algorithm)
- https://kyleye.top/posts/demystify-attributegraph-1/ (AttributeGraph internals)
- https://careersatdoordash.com/blog/how-the-swiftui-view-lifecycle-and-identity-work/ (view identity / shadow tree)
- https://www.malcolmhall.com/2023/03/23/learn-swiftuis-view-struct-value-semantics-diffing-and-dependency-tracking/ (value-type views, dependency tracking)
- https://nilcoalescing.com/blog/InitializingObservableClassesWithTheStateMacroInXcode27/ (@State macro, Xcode 27)
- https://dev.to/arshtechpro/wwdc26-whats-new-in-swiftui-a-developers-breakdown-1333 (@State-as-macro, back-deploy iOS 17)
- https://github.com/swiftlang/swift-evolution/blob/main/proposals/0395-observability.md (SE-0395 Observation; language governed openly)
- https://developer.apple.com/documentation/swiftui/viewbuilder (ViewBuilder result builder)
- https://developer.apple.com/documentation/updates/swiftui (Apple SwiftUI updates index)
