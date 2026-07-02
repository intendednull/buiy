**Date:** 2026-06-26
**Status:** active
**Subject:** Jetpack Compose — Android/Kotlin declarative UI as a COMPOSITION model (@Composable, Modifier chain, state hoisting, slot APIs, CompositionLocal)

# Architecture, runtime & distribution

See also: [composition-state-events.md](composition-state-events.md) (the
authoring DX), [styling-theming.md](styling-theming.md) (Modifier/tokens),
[lessons.md](lessons.md) (decisions). The semantics/automation lens lives at
[`../retained-mode-semantics-automation/`](../retained-mode-semantics-automation/).

## What it is

A declarative, Kotlin-native UI toolkit for Android (GA 2021-07-28) replacing
imperative `android.view`/XML. UI is a tree of `@Composable` functions that
*describe* target state rather than mutate views; a compiler plugin plus a
runtime re-invoke (recompose) only the functions whose inputs changed and
reconcile the result. The four conventions on top — state hoisting, the
`Modifier` chain, slot APIs, `CompositionLocal` — are the transferable subject;
the recomposition machinery underneath is not.

## Four-layer architecture

Each layer builds only on the public API of the one below, and is individually
replaceable
([layering](https://developer.android.com/develop/ui/compose/layering)):

1. **Runtime** — `@Composable`, `remember`, `mutableStateOf`, `SideEffect`.
   Pure tree/state management; no UI. (Build directly on this if you only need
   Compose's tree management, not its UI.)
2. **UI** (`ui`, `ui-text`, `ui-graphics`, `ui-tooling`) — `LayoutNode`,
   `Modifier`, input, custom layout, drawing.
3. **Foundation** — design-system-agnostic blocks: `Row`/`Column`/`Box`,
   `LazyColumn`, gestures. (Build on this to make your own design system.)
4. **Material / Material3** — a concrete design system: theming, styled
   components, ripple, icons.

## The runtime pipeline (the part Buiy CANNOT port)

The **Compose compiler plugin** rewrites every `@Composable` to thread an
implicit `$composer` and inject positional-memoization keys + stability /
skippability metadata. At runtime the composer writes the call tree into a
**slot table** (a gap-buffer of "remembered" slots keyed by source position);
a **snapshot state system** (`mutableStateOf`) records which composable read
which state; mutating that state invalidates exactly the enclosing
**recompose scopes**, which the runtime re-invokes — "calls only the functions
or lambdas that might have changed, and skips the rest." Composables are
required to be **idempotent, side-effect-free, fast, order-independent, and
potentially parallel**, because the runtime may re-run them every frame,
reorder them, or skip them
([mental-model](https://developer.android.com/jetpack/compose/mental-model)).
This automatic diff/skip engine is the retained-vs-immediate axis treated in
the [`../retained-mode-semantics-automation/`](../retained-mode-semantics-automation/)
sibling — see it for the semantics treatment.

## RUNTIME mechanism vs API-surface convention (what Buiy can port)

The recompose engine does not transfer; the API shapes layered on it largely
do.

| Aspect | Classification | Portable to Buiy (ECS + `bsn!`)? |
|---|---|---|
| Recomposition / slot table / positional memoization | **Runtime** | **No.** Buiy is retained ECS with explicit change-detection; no recompose engine to port. |
| Snapshot state (`mutableStateOf`), auto dependency tracking, `remember`/`derivedStateOf` | **Runtime** | No — replaced by ECS components / `Changed<T>` / observers. |
| Stability/skippability inference, `@Stable`/`@Immutable` | **Runtime** | No (and its footguns vanish — see [open-problems.md](open-problems.md)). |
| **Effect scheduling** (`SideEffect`/`LaunchedEffect`/`DisposableEffect`) | **Runtime** | **No** — "run X when a widget appears/leaves" maps to ECS systems/observers (`Added<T>` / `OnAdd` / `OnRemove`), not a composition-scoped effect; not treated further here. |
| **State hoisting** (`value` + `onValueChange`, stateless component) | **API-surface convention** | **Yes (shape).** See [lessons.md](lessons.md) B1. |
| **`Modifier`** as one ordered immutable value param | **Convention** (value shape; its draw/layout *effects* are runtime) | **Partial** — value-not-builder shape ports; ordered-chain semantics need ECS lowering. |
| **Slot APIs** (`content: @Composable () -> Unit`) | **Mostly convention** (named-slot shape; the deferred lambda is runtime) | **Partial** — named child slots port; the lambda does not. |
| **`CompositionLocal`** typed-ambient values | **Mixed**: typed token = convention; tree-scoped propagation = runtime | Typed tokens **yes**; ambient propagation **no** (Resource / inherited components). |
| **`compose-api-guidelines.md`** style guide | **Pure documentation artifact** | **Yes, in spirit verbatim.** See [lessons.md](lessons.md) B5. |

## Distribution / versioning / who-ships-it

- **Who ships it:** Google's AndroidX team. Source lives in **AOSP** under
  `frameworks/support/compose` on `android.googlesource.com` (Gerrit),
  mirrored to GitHub `androidx/androidx`. Released as Maven artifacts under the
  **`androidx.compose.*`** group.
- **Versioning is per-module, reconciled by a BOM.** Each layer (`compose-ui`,
  `foundation`, `runtime`, `material`, `material3`, `animation`) versions
  independently. The **Compose BOM** (`androidx.compose:compose-bom`, e.g.
  `2026.06.00`) is a date-named Bill of Materials pinning a mutually-compatible
  set, so apps declare one platform version instead of N. The **Material3
  track is decoupled**: BOM `2026.06.00` maps core libs to 1.11.3 but
  Material3 to **1.4.0**.
- **The compiler is no longer separately versioned.** Since **Kotlin 2.0**, the
  Compose compiler is merged into the Kotlin repo and ships with Kotlin; the
  Gradle plugin `org.jetbrains.kotlin.plugin.compose` carries the Kotlin
  version, removing the old Kotlin↔compiler compatibility-table coordination.
  **Distribution lesson for Buiy:** Compose's hardest packaging problem was a
  *runtime/compiler↔language* coupling — a problem Buiy structurally avoids
  because `bsn!` is **Bevy's** macro, versioned with Bevy, not a bespoke
  compiler plugin (a HARD CONSTRAINT, reinforced in [lessons.md](lessons.md) A4).
- **Compose Multiplatform** is a *separate distribution* by JetBrains (Gradle
  plugin `org.jetbrains.compose`, Apache 2.0) republishing the androidx
  runtime/UI/foundation/material for non-Android targets; its 1.11.x line
  tracks but lags androidx Compose.

## Flagged / unverified

- **License:** Apache 2.0 by AndroidX convention and AOSP hosting confirmed,
  but no explicit `LICENSE` file was surfaced this pass.
- **Material3 first-stable (1.0.0) date:** not re-verified; the current stable
  (1.4.0, released 2025-09-24) is verified.
- **Compose Multiplatform exact latest patch:** 1.11.0 (announced 2026-05)
  verified; a 1.11.1 patch appeared in search but was not opened.

## Sources

- https://developer.android.com/develop/ui/compose/layering
- https://developer.android.com/jetpack/compose/mental-model
- https://developer.android.com/develop/ui/compose/bom
- https://developer.android.com/develop/ui/compose/bom/bom-mapping
- https://developer.android.com/jetpack/androidx/releases/compose-material3
- https://kotlinlang.org/docs/compose-compiler-migration-guide.html
- https://android-developers.googleblog.com/2024/04/jetpack-compose-compiler-moving-to-kotlin-repository.html
- https://android.googlesource.com/platform/frameworks/support/+/androidx-main/compose/
- https://github.com/JetBrains/compose-multiplatform
- https://blog.jetbrains.com/kotlin/2026/05/compose-multiplatform-1-11-0/
