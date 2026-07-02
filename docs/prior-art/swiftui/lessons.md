**Date:** 2026-06-26
**Status:** active
**Subject:** SwiftUI — Apple's declarative value-typed UI framework (modifier chains, @State/@Binding/@Observable, @ViewBuilder)

# Lessons for Buiy — the decision file

Distilled from [architecture.md](./architecture.md),
[composition-state-events.md](./composition-state-events.md),
[styling-theming.md](./styling-theming.md), and
[open-problems.md](./open-problems.md). Every item is tagged with the Buiy friction
it informs and rated for **ECS + `bsn!` transferability**.

**The frictions (F1..F8):** F1 app state IS the a11y tree (no domain layer); F2 one
untyped `OnPress(Entity)`, no typed change/binding; F3 silent-wrong footguns; F4 the
`Style` builder is a `Bundle` so `bsn!` can't author it; F5 one widget has 4
spellings; F6 stringly-typed unchecked theme tokens; F7 retained-mode boilerplate
(marker sprawl, tree-walks, no dynamic content in scenes); F8 verbosity.

**Transferability lens:** SwiftUI re-evaluates `body` per change; `bsn!` authors
components **once** at spawn over an explicit `Entity`. Anything leaning on per-frame
re-evaluation transfers **low**; typed boundaries, derived outputs, and composition
ergonomics transfer **high**.

---

## Validates (Buiy's instinct is right; SwiftUI is proof)

**[V1] State separate from the a11y tree; a11y is a derived OUTPUT.** — **F1** ·
**HIGH (principle) / LOW (mechanism).** `@State`/`@Observable` is the source of
truth; `.accessibilityLabel/Value` *derive* the semantic tree. Strongest external
evidence on Buiy's open question: app state need **not** *be* the a11y tree. The
separation maps perfectly to ECS (state components → an `accesskit_node` projection
system). But SwiftUI re-derives for free by re-running `body`; Buiy is retained, so
the projection must be a **change-detected system**, not a re-run. Surface this
together with its cost (A4 / open-problem 4).

**[V2] Theme/contextual values are typed, key-based, compile-checked — never
stringly.** — **F6** · **HIGH.** `EnvironmentValues` are typed by a Swift type;
`@Entry` generates the key + default in one line. Direct rebuttal to F6: the *type*
is the token, the compiler checks it, a default is mandatory. ECS resources/
components are already typed — a typed-token newtype + theme resource maps 1:1.

**[V3] A result-builder / macro for children eliminates child-wiring boilerplate.**
— **F8** · **HIGH.** `@ViewBuilder` constructs views from closures without `return`
keywords; `body` itself is `@ViewBuilder`. Validates that declarative children belong
behind a builder macro — precisely what `bsn!`/`bsn_list!` give Buiy; build on
Bevy's macro, don't reinvent. Caveat: SwiftUI's builder is value-immediate; `bsn!`
is spawn-once, so dynamic child lists need an explicit `ForEach`-equivalent (F7).

**[V4] Collapsing many state "spellings" into one canonical mechanism is worth a
breaking migration.** — **F5** · **MED.** SwiftUI *had* F5's exact disease
(`@State`/`@StateObject`/`@ObservedObject`/`@EnvironmentObject`/`@Published`) and
Apple deliberately collapsed it onto `@Observable` + `@State` + `@Bindable`. Lesson
for "one widget has 4 spellings": pick one canonical spelling and migrate, even at
cost. The consolidation lesson transfers; the mechanism is SwiftUI-internal, so Buiy
does its own unification.

---

## Avoid (SwiftUI's sharp edges; do not import these)

**[A1] Don't make composition order load-bearing for semantics.** — **F4** ·
**MED.** `.padding().background()` ≠ `.background().padding()`, no error — identical
modifiers, different picture. ECS bundles are an *unordered set*, so Buiy is
naturally immune; the fix for F4 is **`bsn!`-authorable decomposed components** (an
unordered set), which structurally avoids A1. Borrow the chain's readability (B-items
below); reject order-as-meaning. (A future fluent style-builder could reintroduce
this — watch for it.)

**[A2] Don't rely on implicit/structural identity; lean on explicit identity.** —
**F3, F7** · **HIGH.** SwiftUI ties state to *implicit* identity (type + tree
position); changing either resets state. ECS `Entity` **is** explicit, stable
identity — keep state keyed to it and never derive identity from tree position,
sidestepping the entire "where did my state go" footgun class.

**[A3] Don't re-run widget construction as a side effect of redraw.** — **F3** ·
**MED.** Pre-Xcode-27 SwiftUI re-ran `@Observable` initializers on every rebuild
("non-deterministic last-write-wins"); Apple retrofitted retained, build-once
semantics via the `@State` macro. Buiy's spawn-once model already has this property;
the lesson is to keep widget-construction systems **idempotent** so a scene rebuild
can't re-run constructors with side effects.

**[A4] If state ≠ a11y, accessibility goes silently empty when authors forget the
modifiers.** — **F1, F3** · **HIGH.** The cost of V1: SwiftUI a11y is opt-in per
modifier, so a custom view with no `accessibility*` ships silently inaccessible. If
Buiy separates state from a11y, it must also **derive sensible a11y defaults from
widget components and lint for missing semantics** — get V1's clean separation
*without* the silent-empty failure mode. Directly shapes the F1 decision.

**[A5] Don't seal the declarative layer; an opaque framework that forces an external
escape hatch is a defect.** — **F3, F7** · **MED.** SwiftUI's escape hatch
(`UIViewRepresentable`) itself leaks (reused `UIView` stops tracking state). Buiy's
advantage: the imperative escape is *native* — add a system/query over the same ECS
world. Keep it first-class; never make a widget reachable only through sealed
internals.

---

## Borrow (adapt the mechanism into ECS + `bsn!`)

**[B1] The `$`-binding controlled convention: a typed, two-way handle into one
source of truth.** — **F2** · **MED.** `$value` projects a `Binding<T>`; mutating the
control writes back through it. Direct answer to F2's untyped `OnPress(Entity)`: a
slider hands back a typed `f32`, a toggle a typed `bool`, bound to a specific
component field. The *typed-ness* + *single-source-of-truth* transfer cleanly; the
closure-based get/set form does **not** (ECS has no closures-over-value-storage).
Adapt to ECS as **typed per-widget change events / Bevy observers**, or a
`Binding<T>`-shaped component (target entity + component-field accessor) plus a sync
system. Design the relationship; don't clone `Binding`. (ECS has a *better* identity
story — the entity is stable — but no `$` projection sugar, so the spelling won't
survive; the concept does.)

**[B2] An `@Entry`-style derive/macro generating a typed theme key + mandatory
default.** — **F6** · **HIGH.** `@Entry var primaryColor: Color = .black` generates
the key, get/set, and default in one line. Borrow the ergonomic: a Buiy derive that
turns a token type into a typed theme-resource accessor with a required default, so a
missing token is a compile error, not a runtime string miss. The per-subtree
*cascade* is the harder part (no implicit ECS hierarchy lookup) — implementable via
Bevy 0.19 inherited/propagated components; budget for the propagation system if
subtree theming is wanted.

**[B3] `accessibilityRepresentation` — a sanctioned way to make the a11y tree
deliberately diverge from the visual tree.** — **F1** · **HIGH.** It generates
accessibility behavior for custom views "separately," working with plain views and
complex hierarchies. For Buiy this validates an explicit divergence valve: a
custom-drawn widget can present *as* a slider to AccessKit without its render
entities dictating the semantic node — just an AccessKit-node component that need not
mirror the layout entity. Supports both V1 and AccessKit-first output.

**[B4] Fine-grained per-property observation, not coarse object-level
invalidation.** — **F7** · **HIGH.** `@Observable` redraws only if accessed
properties change, vs `ObservableObject` re-evaluating on *any* `@Published` change.
This **is** Bevy `Changed<T>` change detection — SwiftUI's `@Observable`-vs-
`ObservableObject` arc is cautionary proof that granularity matters. Buiy's answer to
F7's "no dynamic content / tree-walks" is to drive updates off fine-grained ECS
change detection plus a **keyed reconciler** (diff a data array against existing
child entities by stable id) for a `ForEach`-equivalent — the missing piece behind
F7. The `bsn!` surface can stay declarative on top.

**[B5] Move framework magic into macros deliberately; macros can back-deploy and
sidestep the type-check cliff.** — **F4, F5, F8** · **HIGH.** Apple's trajectory —
`@Observable` (2023), `@Entry` (2024), `@Animatable` (2025), `@State` (2026) — keeps
relocating ergonomics from runtime property wrappers into compile-time macros. This
aligns with the `bsn!`-on-Bevy constraint: prefer a macro that emits **concrete
components** (no giant inferred `some View` types → dodges the type-check blowup, a
real ECS+macro advantage) over fluent runtime builders. Make styling a uniform set of
composable scene-fns/combinators returning bundles/patches (one obvious authoring
surface) instead of an un-`bsn!`-able `Style` builder — borrow the chain's
*uniformity*, not its wrap-ordering (A1).

---

## Net takeaways (one line per friction)

- **F1:** State can be separate from a11y (V1, B3) — but only *with* a default-derive
  + lint, or you ship silently-inaccessible widgets (A4).
- **F2:** Typed single-source-of-truth binding (B1) replaces untyped `OnPress(Entity)`.
- **F3:** Explicit `Entity` + spawn-once already dodge SwiftUI's worst footguns (A2,
  A3) — preserve it; keep construction idempotent.
- **F4:** `bsn!`-authorable decomposed components (unordered set) — borrow readability,
  reject order-as-semantics (A1, B5).
- **F5:** Consolidate the 4 spellings to one canonical mechanism (V4); make wrong
  choices loud.
- **F6:** Typed theme keys with mandatory defaults (V2, B2), no stringly tokens.
- **F7:** Fine-grained ECS change detection + a keyed reconciler (B4), not tree-walks.
- **F8:** `bsn!`/`bsn_list!` is the validated answer to verbosity (V3, B5).

## Sources

- https://www.avanderlee.com/swiftui/accessibility-uikit-developers/ (a11y as output); https://swiftwithmajid.com/2021/09/01/the-power-of-accessibility-representation-view-modifier-in-swiftui/ (accessibilityRepresentation divergence)
- https://www.avanderlee.com/swiftui/entry-macro-custom-environment-values/ (@Entry typed tokens); https://www.avanderlee.com/swiftui/viewbuilder/ (@ViewBuilder)
- https://developer.apple.com/documentation/SwiftUI/Migrating-from-the-observable-object-protocol-to-the-observable-macro (spelling consolidation); https://www.avanderlee.com/swiftui/observable-macro-performance-increase-observableobject/ (per-property vs coarse invalidation)
- https://www.hackingwithswift.com/books/ios-swiftui/why-modifier-order-matters (order as load-bearing); https://www.hackingwithswift.com/quick-start/swiftui/two-way-bindings-in-swiftui ($ projected Binding<T>)
- https://dev.to/sebastienlato/swiftui-view-identity-lifecycle-why-views-recreate-state-resets-3afm (structural identity → state reset)
- https://www.jessesquires.com/blog/2024/09/09/swift-observable-macro/ (init-every-rebuild); https://nilcoalescing.com/blog/InitializingObservableClassesWithTheStateMacroInXcode27/ (@State macro retained-init retrofit)
