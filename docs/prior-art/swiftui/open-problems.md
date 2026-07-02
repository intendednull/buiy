**Date:** 2026-06-26
**Status:** active
**Subject:** SwiftUI — Apple's declarative value-typed UI framework (modifier chains, @State/@Binding/@Observable, @ViewBuilder)

# What SwiftUI structurally does NOT solve

The honest counterweight to the reference-design framing in
[lessons.md](./lessons.md). These are not bugs Apple will patch — they are
consequences of the value-typed, per-frame-re-evaluated, opaque-runtime design.
Each notes the Buiy-relevant analog.

## 1. Re-render causality is not observable

There is no first-class answer to *"why did this re-evaluate?"* — only the private
`Self._printChanges()`, and even `print()` won't compile inside `body`. The
dependency graph that decides what to recompute is opaque to the author.
**Buiy analog:** ECS change-detection (`Changed<T>`) has the same opacity risk; an
introspection/debug overlay for "what invalidated this widget" is unsolved prior
art Buiy must build itself.

## 2. Implicit identity → silent state loss

Identity is **structural** (type + position in the tree). `.id()` is a manual
override, but there is no ergonomic *default* for explicit identity, so conditional
branches "accidentally reset state": *"If your view doesn't have a stable identity,
SwiftUI will: reuse wrong cells, mix animations, skip updates, corrupt binding
states"* (Rahul Nimje, 2025). `AnyView` is "the evil nemesis of structural
identity," hiding structure from the compiler (WWDC21).
**Buiy analog/advantage:** ECS entities carry explicit, stable `Entity` ids — Buiy
starts on the right side of this *if it never derives identity from tree position*.

## 3. Compile-time blowups are intrinsic to the design

Deep modifier chains + opaque `some View` + overload-heavy inference trigger
*"unable to type-check this expression in reasonable time."* Daniel Hooper, 2024:
*"Swift 6 spends 42 seconds on these 12 lines on an M1 Pro … In the same amount of
time, Clang can perform a clean build of my 59,000 line C project 38 times."* The
constraint solver "can behave super-linearly or even exponentially."
**Buiy mitigant:** Rust's inference is HM-style too, but `bsn!`/bundles avoid
SwiftUI's `some View` type-accretion. Keep widget composition out of deep
generic-wrapper chains and prefer a macro that emits concrete components to dodge
this class — a genuine ECS+macro advantage, not a guarantee.

## 4. Accessibility is opt-in, never derived from state

The cost of the clean state/a11y separation: a11y is **opt-in per modifier**,
derived by walking the view tree. A custom view with no `accessibility*` modifiers
ships **silently inaccessible** — the framework does not derive semantics from your
state model. The separation is clean but can produce a *wrong/empty* tree.
**Buiy lesson:** if Buiy separates state from a11y (the F1 decision), it must also
**derive sensible a11y defaults from widget components and lint for missing
semantics** — separation *without* a default-derive ships silently-inaccessible
widgets. Decide F1 *with this failure mode in view.*

## 5. No sanctioned imperative escape *within* the model

To do something the declarative layer can't express, you must exit to UIKit/AppKit
via `UIViewRepresentable` — and that bridge leaks: "SwiftUI will reuse underlying
`UIView` instances … meaning any properties assigned in `makeUIView` won't be
continuously updated." Verbatim (John, via Tsai): *"There is something wrong with
declarative frameworks that are opaque and don't let you escape into imperative
mode when needed."*
**Buiy advantage:** the imperative escape is *native* — any user can add a
system/query over the same ECS world the widgets use. Keep it that way; never make a
widget reachable only through sealed internals.

## 6. First-party / third-party asymmetry

Apple's own components use private API third parties can't reach. Verbatim (Thomas
Clement, 2020): *"It is bothering that third-party developers are not able to write
the same kind of views that Apple provides in SwiftUI."*
**Buiy lesson:** as an open-source ECS framework, ensure widget authors build on the
**same public primitives** as built-in widgets — no private fast-path.

## 7. The declarative layer historically under-exposed layout control

Verbatim (Clement, 2020): SwiftUI is *"missing … the notion of the intrinsic
content size"* and *"compression resistance priorities and content hugging
priorities."* Mechanically moot for Buiy (it owns Taffy), but the meta-lesson holds:
a declarative authoring layer must not hide enough of the layout engine that authors
can't express real constraints.

## 8. The ownership-wrapper zoo is self-inflicted complexity

`@State`/`@Binding`/`@StateObject`/`@ObservedObject`/`@EnvironmentObject`/
`@Observable`/`@Bindable`/`@Environment` — many near-synonyms with **silent-wrong**
lifetime semantics if you pick wrong. *"80% of SwiftUI bugs come from: Wrong state
ownership, Wrong state scope, Duplicated state, State passed too deep, State mutated
from wrong layer"* (Rahul Nimje, 2025). Observation was partly an effort to
*collapse* this zoo; even the reference design fights its own F3/F5.
**Buiy lesson:** keep the count of ways-to-spell-a-thing small and orthogonal; where
two spellings differ in lifetime, make the wrong one a **loud** failure.

## Sources

- https://www.hackingwithswift.com/quick-start/swiftui/how-to-find-which-data-change-is-causing-a-swiftui-view-to-update (`Self._printChanges()`)
- https://dev.to/sebastienlato/swiftui-view-identity-lifecycle-why-views-recreate-state-resets-3afm (structural identity → state reset)
- https://developer.apple.com/videos/play/wwdc2021/10022/ (Demystify SwiftUI; AnyView)
- https://swiftwithmajid.com/2021/12/09/structural-identity-in-swiftui/ (structural identity)
- https://danielchasehooper.com/posts/why-swift-is-slow/ (type-checker exponential blowup; 42s/12 lines)
- https://www.avanderlee.com/swiftui/accessibility-uikit-developers/ (a11y as opt-in output)
- https://swiftwithmajid.com/2021/09/01/the-power-of-accessibility-representation-view-modifier-in-swiftui/ (accessibilityRepresentation)
- https://mjtsai.com/blog/2020/11/30/what-is-not-so-great-about-swiftui/ (Clement, Streza, "John"; opacity, layout, asymmetry)
- https://www.swiftbysundell.com/articles/swiftui-and-uikit-interoperability-part-1/ (UIViewRepresentable leakage)
- https://medium.com/@rahulnimje94/swiftui-in-production-25-hard-lessons-i-learned-the-painful-way-2a72261abcae (state-ownership / identity production lessons)
- https://www.jessesquires.com/blog/2024/09/09/swift-observable-macro/ (init-every-rebuild footgun)
