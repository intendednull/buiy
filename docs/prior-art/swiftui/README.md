**Date:** 2026-06-26
**Status:** active
**Subject:** SwiftUI — Apple's declarative value-typed UI framework (modifier chains, @State/@Binding/@Observable, @ViewBuilder)

# SwiftUI — prior-art for Buiy

Apple's first-party declarative UI framework for all Apple platforms. You describe
UI as a tree of immutable **value-type `View` structs**; a private runtime (the
**AttributeGraph** dependency engine) tracks which state each view reads,
re-invokes only the affected `body` closures on change, diffs the resulting
ephemeral value tree against a retained shadow tree, and mutates the minimal set
of backing objects. The author-facing surface is four reusable conventions:
**modifier chains** (`.padding().background(…)`), **state property
wrappers/macros** (`@State`/`@Binding`/`@Observable`/`@Environment`),
**`$`-projected two-way bindings**, and **`@ViewBuilder` result-builder children**.

The single most load-bearing distinction for Buiy: **almost everything that makes
SwiftUI *feel* good is API-surface convention that can be ported; almost
everything that makes it *work* is a proprietary runtime you cannot port and must
re-implement against your own substrate** (ECS + Taffy + an AttributeGraph-
equivalent dirty-tracker). See [architecture.md](./architecture.md) for the split.

## Key facts

| Fact | Value |
|---|---|
| Initial release | June 3, 2019 (WWDC 2019, iOS 13 / macOS Catalina) |
| Latest cycle | iOS 27 line (WWDC 2026, June 8–12, 2026); prior iOS 26 / WWDC 2025 |
| Toolchain | Xcode 27 + Swift 6.4 (WWDC 2026); prior Xcode 26 + Swift 6.2 (WWDC 2025), then Swift 6.3 (Xcode 26.4, March 2026) |
| Official version number | **None.** Community informally numbers SwiftUI 1–6 ↔ iOS 13–18; no authoritative number past iOS 18 |
| License | Proprietary / closed-source |
| Steward | Apple Inc., single vendor, closed roadmap (the *Swift language* is open; the framework is not) |
| Platforms | iOS, iPadOS, macOS, watchOS, tvOS, visionOS |
| Distribution | Bundled in the platform SDKs shipped with Xcode; not a Swift Package, no `swiftui = "x.y"` line |
| Runtime engine | AttributeGraph (private C++ dependency graph) + value-type view diffing + shadow tree |
| State trajectory | `@Observable` (2023) → `@Entry` (2024) → `@Animatable` (2025) → `@State`-as-macro (2026): magic keeps moving from property wrappers into macros |

**Verification flags:** Apple markets by OS *year*, not framework version, so
"SwiftUI 7/8" for iOS 26/27 is informal. The 2024→2025 OS renumber (18 → 26)
makes version arithmetic discontinuous. Apple's `developer.apple.com/documentation`
pages are JS-rendered and did not return text to automated fetch, so primary-doc
shapes are corroborated through Apple WWDC pages + reputable third parties
(Hacking with Swift, SwiftLee, Swift with Majid, Donny Wals, Nil Coalescing).

## Contents

- [architecture.md](./architecture.md) — what it is, runtime vs API-surface split, distribution/versioning
- [composition-state-events.md](./composition-state-events.md) — the core DX: composition/slots, state model, change propagation, with code
- [styling-theming.md](./styling-theming.md) — modifier-chain styling, style protocols, `EnvironmentValues` + `@Entry` typed tokens
- [open-problems.md](./open-problems.md) — what SwiftUI structurally does NOT solve
- [lessons.md](./lessons.md) — **the decision file**: Validates / Avoid / Borrow, each tagged F1..F8 + ECS+`bsn!` transferability

## How to use

These docs are written from **Buiy's** stance: a Rust, ECS-native (Bevy 0.19),
retained-mode, AccessKit-first UI library authored via Bevy's `bsn!` macro +
bundle constructors. The "for Buiy" notes, the friction tags (F1..F8), and the
transferability ratings reflect that bias **by design** — they exist to feed
Buiy's DX-composition decisions, not to neutrally survey SwiftUI. SwiftUI is a
**value-typed, per-frame-re-evaluated** framework on a **proprietary runtime**;
Buiy is **retained, spawn-once, ECS**. So treat every lesson as: *does the
principle survive translation into "author components once at spawn over an
explicit `Entity` identity"?* Anything that leans on SwiftUI re-running `body`
each frame transfers **low**; anything about typed boundaries, derived outputs,
and composition ergonomics transfers **high**. Start with [lessons.md](./lessons.md)
if you want the decisions; read the others for the evidence behind them.

## Glossary (stub)

- **AttributeGraph** — SwiftUI's private C++ dependency graph; the reactive engine. Not portable; Buiy needs an ECS change-detection equivalent.
- **`body: some View`** — a view's one computed property returning an opaque concrete view type; rebuilt cheaply and diffed each evaluation.
- **Modifier** — a method `View -> View` that wraps its receiver in a new view value; order is semantic (§ styling-theming).
- **`@State` / `@Binding` / `@Observable` / `@Bindable` / `@Environment`** — the state-ownership vocabulary (own / borrow two-way / shared model / bind-into-shared / ambient).
- **`$value` / `Binding<T>`** — the projected two-way handle: child reads+writes, parent owns (the "controlled" convention).
- **`@ViewBuilder`** — a Swift result builder turning a `{ a; b; if c { d } }` closure into one composed `Content` value; how children/slots are passed.
- **`EnvironmentValues` + `@Entry`** — typed, key-path-indexed, defaulted, cascading ambient context = SwiftUI's design-token model.
- **F1..F8** — Buiy's DX frictions this corpus informs (defined in [lessons.md](./lessons.md)).

## Sources

- https://en.wikipedia.org/wiki/SwiftUI (license, initial release, platforms, distribution)
- https://developer.apple.com/videos/play/wwdc2019/204/ (WWDC 2019 SwiftUI introduction)
- https://swiftprogramming.com/swiftui-version-history/ (informal SwiftUI 1–6 ↔ iOS 13–18 mapping)
- https://developer.apple.com/wwdc26/guides/swiftui/ (WWDC 2026 SwiftUI guide)
- https://www.wokeey.com/events/apple-wwdc-event/ (WWDC 2026 dates June 8–12, iOS 27)
- https://byteiota.com/swift-64-wwdc-2026-upgrade/ (Swift 6.4 / Xcode 27 / WWDC 2026)
- https://rensbr.eu/blog/swiftui-attribute-graph/ (AttributeGraph runtime)
