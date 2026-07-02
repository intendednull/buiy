**Date:** 2026-06-26
**Status:** active
**Subject:** Jetpack Compose — Android/Kotlin declarative UI as a COMPOSITION model (@Composable, Modifier chain, state hoisting, slot APIs, CompositionLocal)

# Lessons for Buiy — Validates / Avoid / Borrow

The decision file. Each item is tagged with the Buiy DX friction it informs and
rated for **ECS+`bsn!` transferability** (high/med/low + why). Evidence lives in
[architecture.md](architecture.md), [composition-state-events.md](composition-state-events.md),
[styling-theming.md](styling-theming.md); gaps in [open-problems.md](open-problems.md).
The a11y/agent-tree lessons are in the sibling
[`../retained-mode-semantics-automation/lessons.md`](../retained-mode-semantics-automation/lessons.md).

**The frictions (F1–F8)** — defined in the [developer-experience
audit](../../reports/2026-06-25-developer-experience-audit.md); summarized here:
F1 app state IS the a11y tree (no domain layer); F2 one untyped `OnPress(Entity)`,
no typed per-widget change events/binding; F3 silent-wrong footguns; F4 the
`Style` builder is a `Bundle` so `bsn!` can't author it; F5 one widget has 4
spellings; F6 stringly-typed theme tokens, no compile-time check, half-wired
(colors resolve, spacing/radius hardcoded); F7 retained-mode boilerplate (marker
sprawl, tree-walks, no dynamic content in scenes); F8 verbosity.

**HARD CONSTRAINTS:** Buiy stays ECS + retained; AccessKit is a non-negotiable
OUTPUT (the open question is whether app STATE should be separate from it);
`bsn!` is Bevy's macro (build on/around it, don't fork).

---

## Validates — Buiy bets Compose confirms at scale

- **V1 — Declarative front over a retained/diffing back. [F7/F8 · transfer: HIGH]**
  Compose is a *declarative* `@Composable` surface compiled into mutations of a
  **retained** slot table + node tree the runtime diffs. Structurally Buiy's bet
  — `bsn!` authoring over a retained ECS world — proven at shipping-platform
  scale. Compose is the existence proof that "declarative author, retained diff
  back-end" is not a compromise.
- **V2 — Composition over configuration. [F7 · transfer: MED]** Compose pushes
  *passing child content* over fat config bundles (slot APIs, B3). Buiy already
  composes by child entities / scene-fns; Compose canonizes the instinct. ECS
  composes via child entities, not captured closures — principle transfers,
  mechanism differs.
- **V3 — Keep app state explicit and separate from presentation. [F1 · transfer:
  HIGH]** Compose's discipline is *hoisted, caller-owned* state with a
  *separately declared* semantics layer; it never makes the rendered/semantic
  tree the source of truth for app state. This is **evidence for** separating
  app STATE (ECS components) from the AccessKit OUTPUT rather than collapsing
  them — the most successful declarative toolkit keeps them distinct. (It does
  *not* answer the unification question; see [open-problems.md](open-problems.md) §1.)

## Borrow — concrete patterns to lift

- **B1 — State hoisting + the controlled `value`/`onValueChange` convention.
  [F1/F2 · transfer: MED — HIGH contract, LOW mechanism]** Stateless by default:
  hoist state to the lowest common ancestor; expose `value: T` +
  `onValueChange: (T) -> Unit`; `MutableState<T>` params are *discouraged*
  ("promotes joint ownership"). The typed, per-widget change signal is exactly
  what Buiy's single untyped `OnPress(Entity)` lacks. **Adopt the contract
  shape** (typed value in, typed change out, single owner). The *mechanism* does
  NOT transfer: there is no recomposing parent to hoist out of — state already
  lives in ECS components. Buiy's idiomatic encoding is a typed
  `Changed<Value<T>>`-style component + a typed change **event/observer**, not a
  captured `onValueChange` lambda.
- **B2 — `Modifier` as one uniform, ordered, value-typed presentation param.
  [F4 · transfer: MED — concept HIGH, mechanism LOW]** A component has exactly
  one `modifier: Modifier = Modifier`, first optional param, applied once to the
  root. The transferable insight: **presentation should be one uniform
  authorable VALUE, not a builder/Bundle** — the direct F4 fix. The
  *fluent-chain syntax* does NOT transfer: `bsn!` authors decomposed component
  *data* (`BoxModel{…} Background(…)`), and a runtime builder chain is exactly
  what `bsn!` can't author (the same wall as `Style`-the-`Bundle`). Port the
  shape via a `bsn!`-authorable façade that expands to components; redesign the
  ordering/lowering (ECS component sets are unordered).
- **B3 — Slot APIs (`content: @Composable () -> Unit`) + named/scoped slots.
  [F7 · transfer: MED/LOW]** Dynamic/composed content via a trailing
  `@Composable` lambda, preferred over twenty config knobs; multi-slot layouters
  use named lambdas (`Scaffold(topBar = {…})`). A slot is a *captured closure
  composed lazily* — a `bsn!` scene is *data* and cannot hold a closure-slot.
  Borrow the **named child-slot intent** (header/body/footer as named
  child-entity anchors / scene-fn params); genuinely dynamic content stays a
  *systems* concern (spawn/despawn child entities, `bsn_list!` over a
  collection), NOT a re-run lambda.
- **B4 — `CompositionLocal` for typed, tree-scoped theming/DI. [F6 · transfer:
  SPLIT — typed token HIGH, propagation LOW]** Typed `CompositionLocal` keys
  with a good default; `MaterialTheme.colorScheme.primary` is a **typed,
  compiler-checked** token, not a string. **Replace Buiy's unchecked string
  theme tokens with typed handles + a guaranteed default — the clean F6 fix
  (HIGH).** The *implicit tree-scoped provide* mechanism is LOW for flat ECS:
  Buiy has no cheap "nearest ancestor provides" lookup and would ancestor-walk
  (the F7 tree-walk pain). Take the type-safety; use a `Theme` resource (global)
  + scoped-override component (subtree), and import the guardrails (A3).
- **B5 — A PUBLISHED widget-API-guidelines doc — the direct F5 fix. [F5 ·
  transfer: HIGH — top-priority artifact to lift]** Compose ships two
  versioned, RFC2119-graded, lint-enforced docs (`compose-api-guidelines.md`,
  `compose-component-api-guidelines.md`). Lift nearly verbatim into a Buiy
  `widget-api-guidelines.md`:
  - **Naming:** a `@Composable`/widget is a `PascalCase` **noun**; ONE canonical
    unprefixed name for the common variant; **variant prefixes** for the rest
    (`OutlinedButton`); **`Basic*`** for the undecorated primitive; no
    company/module prefixes (`GoogleButton`). This *is* the cure for "4
    spellings."
  - **Parameter order (MUST):** required → single style entry (`modifier`/
    `Style`) → optional (defaulted) → trailing content.
  - **Backward-compat (MUST):** never remove params; new params have defaults
    and append last; deprecate-and-forward, don't break.
  Map "framework/library/app" conformance tiers onto Buiy's
  **core/widgets/user-code**. Low effort, high leverage. Caveat:
  a guideline *bounds* drift but does not eliminate it ([open-problems.md](open-problems.md) §7) — pair the doc with lint and a decided deprecation policy.
- **B6 — `*Defaults` copy-with-override factories. [F3 / §4.1c · transfer:
  HIGH]** Per-component `*Defaults.xColors(field = …)` always *start from the
  full default* and override named fields; Compose never lets you single-field-
  patch a component struct. Exact countermeasure to Buiy's §4.1c suppression
  gotcha (a single-field patch of a `#[require]`'d component drops the other
  defaults). Make copy-with-override the **only** sanctioned styling entry point
  so the silent-default-drop footgun becomes unreachable.

## Avoid — sharp edges Buiy must not import

- **A1 — Implicit recompute ordering/cardinality. [F3 · transfer: HIGH
  cautionary]** Per the mental-model doc, composables may run **in any order**,
  **in parallel**, are **optimistic and may be cancelled** (a cancelled
  composition can still apply a side effect → "inconsistent app state"), and run
  **as often as every frame**. The "fast, idempotent, side-effect-free" contract
  is convention-enforced only. Buiy's ECS schedule is explicit, ordered, and
  controlled — *that is the antidote*. Do not regress toward a recompute model
  with implicit order/cardinality.
- **A2 — Correctness/perf riding on author annotations the system can't verify.
  [F3 · transfer: HIGH cautionary]** Skipping rides **stability**, asserted via
  `@Stable`/`@Immutable` that the compiler **trusts without checking**;
  `List`/`Set`/`Map` are unstable. Mislabel → missed-recomposition stale-UI
  bugs, invisible until production. Buiy's `Changed<T>` is *explicit and
  verifiable* (keyed to real component writes) — keep it that way; never gate
  correctness on a "trust-me, this is stable" annotation.
- **A3 — `CompositionLocal` overuse = implicit, un-checked-at-call-site
  dependencies. [F6 · transfer: MED]** Google's own docs warn it makes behavior
  "harder to reason about," creates implicit dependencies, and has "no clear
  source of truth … debugging … more challenging." Named anti-pattern: a
  `CompositionLocal` holding a screen's `ViewModel`. When borrowing B4, import
  the guardrails: require a good default, restrict to genuinely tree-scoped
  concepts (theme/locale/density), never as generic DI.
- **A4 — A magic compiler plugin that rewrites authored functions. [F8/F3 ·
  transfer: HIGH — reinforces a HARD CONSTRAINT]** Compose's positional
  memoization / slot table / skipping require a Kotlin compiler plugin that
  rewrites every `@Composable`, hiding control flow and coupling the toolkit to
  a Kotlin↔compiler↔BOM version triangle. `bsn!` is **Bevy's** macro — keep it a
  transparent *declarative data builder*; do **not** grow it into a
  Compose-style rewriting plugin that injects hidden memoization/recompute
  semantics.

---

## At-a-glance transfer table

| # | Lesson | Friction | Transfer |
|---|---|---|---|
| V1 | Declarative front over retained/diffing back | F7/F8 | HIGH |
| V3 | App state separate from presentation/semantics | F1 | HIGH (insight) |
| B1 | Controlled `value`/`onValueChange` per widget | F1/F2 | MED (HIGH contract / LOW mechanism) |
| B2 | `Modifier` = one uniform ordered styling value | F4 | MED (concept HIGH / mechanism LOW) |
| B3 | Slot APIs / named child slots | F7 | MED–LOW |
| B4 | `CompositionLocal` typed theming/DI | F6 | SPLIT (token HIGH / propagation LOW) |
| B5 | Published, enforced API-guidelines doc | F5 | HIGH |
| B6 | `*Defaults` copy-with-override | F3 | HIGH |
| A1 | Implicit recompute order/cardinality | F3 | HIGH (avoid) |
| A2 | Unverified stability annotations | F3 | HIGH (avoid) |
| A3 | `CompositionLocal` overuse | F6 | MED (avoid) |
| A4 | Compiler plugin rewriting authored fns | F8/F3 | HIGH (avoid) |

## Sources

- https://developer.android.com/develop/ui/compose/state-hoisting
- https://github.com/androidx/androidx/blob/androidx-main/compose/docs/compose-component-api-guidelines.md (read 2026-06-26)
- https://github.com/androidx/androidx/blob/androidx-main/compose/docs/compose-api-guidelines.md (read 2026-06-26)
- https://developer.android.com/develop/ui/compose/modifiers
- https://developer.android.com/develop/ui/compose/compositionlocal
- https://developer.android.com/develop/ui/compose/designsystems/material3
- https://developer.android.com/develop/ui/compose/performance/stability
- https://developer.android.com/develop/ui/compose/mental-model
- https://ahmednmahran.medium.com/the-invisible-performance-killer-a-deep-dive-into-jetpack-compose-stability-236a810c16fb
- https://mvpfactory.io/blog/profiling-jetpack-compose-recomposition-in-production-composition-tracing/
