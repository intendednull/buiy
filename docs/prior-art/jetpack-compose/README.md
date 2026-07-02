**Date:** 2026-06-26
**Status:** active
**Subject:** Jetpack Compose — Android/Kotlin declarative UI as a COMPOSITION model (@Composable, Modifier chain, state hoisting, slot APIs, CompositionLocal)

# Jetpack Compose (composition lens)

Jetpack Compose is Google's declarative, Kotlin-native UI toolkit for Android
(GA 2021-07-28), the replacement for the imperative `android.view` / XML stack.
Its core idea is the **composition model**: UI is a tree of `@Composable`
functions that *describe* target state; a compiler plugin plus a runtime
re-invoke ("recompose") only the functions whose inputs changed. Around that
core sit four API conventions that are the real subject of this folder —
**state hoisting**, the **`Modifier`** chain, **slot APIs**, and
**`CompositionLocal`** — plus a *published, enforced* API style guide.

This folder reads Compose through one lens only: it is the load-bearing
**counter-example** to Buiy. Compose is reactive-recompute over a slot table;
Buiy is retained ECS + `bsn!`. That contrast is exactly what makes Compose's
*conventions* (not its runtime) transferable. The recomposition engine is a
deep compiler+runtime mechanism Buiy cannot and should not port; the four
conventions are largely API-surface shape that transfers independently — to
varying degrees, rated per lesson.

## Key facts

| Fact | Value | Note |
|---|---|---|
| First stable (1.0) | **2021-07-28** | Android Developers blog |
| Current Compose BOM (stable) | **2026.06.00** | June 2026 |
| BOM → core libs | `ui`/`foundation`/`runtime`/`animation` = **1.11.3** | bom-mapping |
| BOM → Material3 | **1.4.0** (decoupled track) | released 2025-09-24 (pinned by BOM 2026.06.00) |
| Material3 latest pre-release | **1.5.0-alpha22** (M3 Expressive) | 2026-06-17 |
| Language / compiler | Kotlin; compiler ships **inside Kotlin since 2.0** | plugin `org.jetbrains.kotlin.plugin.compose` |
| License | **Apache 2.0** (AndroidX convention; LICENSE file not surfaced this pass — flagged) | AOSP-hosted |
| Steward | **Google / Android (AndroidX team)**, developed in AOSP (Gerrit) | mirror `androidx/androidx` |
| Primary platform | **Android** | |
| Cross-platform sibling | **Compose Multiplatform** (JetBrains, Apache 2.0), 1.11.x line | tracks-but-lags androidx |

## Table of contents

- [architecture.md](architecture.md) — what it is, four-layer architecture,
  the recomposition runtime, distribution/versioning, and the central
  **RUNTIME-mechanism vs API-surface-convention** split (what Buiy can port).
- [composition-state-events.md](composition-state-events.md) — the core DX:
  composables emit (not return), slots, the state model
  (`remember`/`mutableStateOf`), state hoisting, the controlled
  `value`/`onValueChange` convention, event propagation, with real Kotlin.
- [styling-theming.md](styling-theming.md) — the `Modifier` chain (styling
  attachment, ordered-value semantics), `MaterialTheme` typed token
  subsystems, `CompositionLocal` cascade, the `*Defaults` copy-with-override
  pattern.
- [open-problems.md](open-problems.md) — what Compose structurally does NOT
  solve (state/a11y unification, no introspectable change bus, unprovable
  recomposition correctness, composition-is-code-not-data, relocated
  boilerplate).
- [lessons.md](lessons.md) — **the decision file**: Validates / Avoid /
  Borrow, each item tagged with a Buiy friction (F1–F8) + ECS+`bsn!`
  transferability.

Sibling folder (cross-linked, not duplicated here): the **semantics /
automation** lens at
[`../retained-mode-semantics-automation/`](../retained-mode-semantics-automation/)
— the `SemanticsNode` tree, `Modifier.semantics`, merged/unmerged tree,
`testTag`, `ComposeTestRule`. Claims about the a11y/agent tree are developed
there.

## How to use

These docs are written from Buiy's stance: **ECS-native (Bevy 0.19),
retained-mode, AccessKit-first, authored via Bevy's `bsn!` macro + bundle
constructors**. The "for Buiy" notes, the friction tags (F1–F8), and the
transferability ratings reflect that bias *by design* — this is prior-art
mined for one project's open questions, not a neutral Compose tutorial. Where
a Compose idea is attractive but rides the recomposition runtime, we say so
and split the portable *shape* from the non-portable *mechanism*. Read
[lessons.md](lessons.md) first if you want the decisions; read
[architecture.md](architecture.md) first if you want to understand why the
runtime does not transfer. Note: Compose's most-loved styling API (`Modifier`)
is the one shape `bsn!` cannot author — full treatment in
[styling-theming.md](styling-theming.md) §1 and [lessons.md](lessons.md) B2.

## Glossary (stub)

- **`@Composable`** — a function the compiler rewrites to emit UI into the
  current composition; emitters return `Unit`.
- **Recomposition** — re-invoking only the composables whose read state
  changed. The runtime mechanism Buiy does *not* port.
- **Slot table** — gap-buffer of "remembered" slots keyed by source position;
  the retained backing store the runtime diffs.
- **State hoisting** — moving state out of a widget to its caller, so the
  widget is stateless and *controlled* (`value` + `onValueChange`).
- **`Modifier`** — one ordered, immutable value carrying layout/draw/behavior,
  passed as the first optional param.
- **Slot API** — a `content: @Composable () -> Unit` parameter a parent places
  where it wants (single or named, e.g. `Scaffold(topBar = {…})`).
- **`CompositionLocal`** — typed ambient value propagated implicitly down a
  subtree (theming/DI); read as `Local…​.current`.
- **`*Defaults`** — per-component factory (`ButtonDefaults`, `CardDefaults`)
  that copies the full theme value and overrides named fields.
- **F1–F8** — Buiy's DX frictions this folder informs; defined in the
  [developer-experience audit](../../reports/2026-06-25-developer-experience-audit.md),
  summarized in [lessons.md](lessons.md).

## Sources

- https://android-developers.googleblog.com/2021/07/jetpack-compose-announcement.html
- https://developer.android.com/develop/ui/compose/bom
- https://developer.android.com/develop/ui/compose/bom/bom-mapping
- https://developer.android.com/jetpack/androidx/releases/compose-material3
- https://github.com/androidx/androidx/blob/androidx-main/compose/docs/compose-api-guidelines.md (read 2026-06-26)
- https://github.com/JetBrains/compose-multiplatform
