**Date:** 2026-06-26
**Status:** active
**Subject:** Jetpack Compose — Android/Kotlin declarative UI as a COMPOSITION model (@Composable, Modifier chain, state hoisting, slot APIs, CompositionLocal)

# Styling, theming & tokens

How Compose attaches styling (the `Modifier` chain) and theming (`MaterialTheme`
+ `CompositionLocal` + `*Defaults`). See [composition-state-events.md](composition-state-events.md)
for the authoring model, [architecture.md](architecture.md) for the runtime
split, [lessons.md](lessons.md) for verdicts.

## 1. Where styling lives: the `Modifier` chain (F4)

Compose has **no style-sheet, no style struct, no `class=` attribute**. Visual
+ layout + interaction decoration is a single value — a `Modifier` — passed as
an ordered, immutable parameter and built by chained calls:

```kotlin
Modifier
    .background(Color.Red)      // draw
    .padding(12.dp)             // layout
    .size(100.dp)               // layout
    .clickable { }              // interaction
    .border(2.dp, Color.Black)  // draw
    .clip(CircleShape)          // draw
```

A `Modifier` is "an ordered, immutable list of `Modifier.Element`"; each call
returns a new modifier wrapping the previous one. **Order is semantically
load-bearing** — the canonical footgun, straight from the docs:

```kotlin
Modifier.clickable(onClick = onClick).padding(16.dp)  // padded margin IS clickable
Modifier.padding(16.dp).clickable(onClick = onClick)  // padded margin is NOT clickable
```

The guideline rules are strict and RFC2119-graded: an element function **MUST**
accept a `Modifier`, **MUST** name it `modifier`, it **MUST** be the **first
optional parameter** with default `Modifier`; the received modifier is applied
**once** as the first modifier on the root layout; a component **MAY**
concatenate additional modifiers to the **end** but **MUST NOT** prepend.

**Buiy F4 signal.** Compose's styling is *one uniform value the authoring layer
passes positionally*, whereas Buiy's `Style` is a `Bundle` builder that `bsn!`
cannot author. Compose demonstrates the alternative — a single ordered styling
**value** as a normal parameter — but note the deep tension: a `Modifier` is a
runtime **builder chain of method calls**, and `bsn!` is a *literal scene
macro* with no call-site evaluation. So a `Modifier`-style fluent API would be
un-authorable in `bsn!` exactly like `Style`-the-`Bundle`. **The durable
lesson is cautionary:** Compose's most-loved styling API is the very shape
`bsn!` can't express. Buiy styling must be *data* (decomposed component
literals), not a builder; if Buiy ever exposes a chain, ordering must be
explicit and documented, never implicit. (Verdict: MED concept / LOW
mechanism — see [lessons.md](lessons.md) B2.)

## 2. The theming model: `MaterialTheme` + `CompositionLocal` (F6)

### 2.1 Three typed, closed token subsystems — no strings anywhere

| Subsystem | Type | Shape of the closed set |
|---|---|---|
| `colorScheme` | `ColorScheme` | ~29 **core** named roles (`primary`, `onPrimary`, `primaryContainer`, `surface`, `onSurface`, `error`, …) derived from 5 key colors over tonal palettes; the full scheme adds surface-container / outline / inverse / scrim tiers (40+ total). Each role: `Color`. |
| `typography` | `Typography` | 15 styles: `display/headline/title/body/label` × `Large/Medium/Small`. Each: `TextStyle`. |
| `shapes` | `Shapes` | 5 corner sizes `extraSmall…extraLarge` (increasing radius). Each: `Shape`. |

Reads are **strongly typed property accesses**, resolved at compile time:

```kotlin
Text(
    text = "Hello",
    color = MaterialTheme.colorScheme.primary,    // : Color
    style = MaterialTheme.typography.titleLarge,   // : TextStyle
)
Card(shape = MaterialTheme.shapes.medium)          // : Shape
```

### 2.2 Cascade via `CompositionLocal`

`MaterialTheme(colorScheme, typography, shapes) { content }` does not thread
tokens through parameters — it *provides* them into `CompositionLocal`s;
`MaterialTheme.colorScheme` is sugar for `LocalColorScheme.current`. Any
descendant reads the nearest ancestor's value; nesting re-scopes a subtree:

```kotlin
CompositionLocalProvider(LocalContentColor provides Color.Blue) {
    Text("Blue")
    CompositionLocalProvider(LocalContentColor provides Color.Red) {
        Text("Red")   // nearest provider wins
    }
}
```

Two flavors: `staticCompositionLocalOf` (changes invalidate the whole provided
subtree; for rarely-changing theme-like values) vs `compositionLocalOf`
(fine-grained, only readers recompose). Naming is guideline-governed: keys
**MAY** use `Local` as a *prefix* (`LocalTheme`), never as a noun suffix
(`ThemeLocal`).

### 2.3 Completeness and the `*Defaults` copy-with-override pattern (F3)

A theme is **total**: `lightColorScheme()`/`darkColorScheme()` fill *every*
role with a default, so you override a subset and the rest stay valid. Crucially,
per-component styling **never single-field-patches a component**. Each component
exposes a **`*Defaults` factory that copies the theme value and overrides named
fields**:

```kotlin
Card(
    colors = CardDefaults.cardColors(
        containerColor = MaterialTheme.colorScheme.primaryContainer,
        contentColor   = MaterialTheme.colorScheme.onPrimaryContainer,
        // disabled* etc. retain sensible defaults
    ),
)
```

This is the exact countermeasure to Buiy's **§4.1c suppression gotcha** ("a
single-field patch of a `#[require]`'d component drops the widget's other
defaults"). Compose never lets you set one field of a component struct
directly; you call a `*Defaults` factory that *starts from the full default*.
Buiy's widget styling helpers (`button("…")`, `text_input_*`) already follow
this spirit — the lesson is to make copy-with-override the **only** sanctioned
styling entry point, so the silent-default-drop footgun becomes unreachable.

## 3. Transferability summary (F4/F6/F3)

- **Typed closed token set (F6): HIGH.** Model tokens as a closed struct of
  *typed* roles (a `Theme` resource exposing `Color`/`TextStyle`/`Shape` fields,
  or a `#[derive]`'d token enum), **not** stringly-typed lookups. The
  compile-checked, role-named palette is the single biggest answer to F6.
- **Cascade via `CompositionLocal` (F6): MED.** ECS has no implicit
  tree-scoped ambient lookup; the natural Buiy analogue is a **`Theme`
  resource** (global) plus an **ancestor/context component** (subtree
  override). Compose's own docs warn `CompositionLocal` makes data flow
  *implicit and harder to test* — an argument for Buiy to prefer an explicit
  resource + scoped-override component over a hidden ambient.
- **`*Defaults` copy-with-override (F3): HIGH.** Make copy-with-override the
  only styling entry point; the default-drop bug becomes unreachable.
- **`Modifier` ordered value (F4): MED concept / LOW mechanism.** Borrow "one
  ordered, macro-authorable styling entry point"; reconcile it with component
  decomposition (a `bsn!`-authorable façade that expands to `BoxModel`,
  `Background`, …), *not* a literal opaque `Modifier` chain.

## 4. Sharp edge: order/stability footguns (the tax Buiy avoids)

Because composition *re-executes*, Compose's footguns are about *what re-runs*:
skipping depends on parameter **stability** (`List` is unstable → re-runs every
time); `@Stable`/`@Immutable` are *promises the compiler trusts, not checks* (a
wrong annotation is a **silent-wrong F3** bug — stale UI that never
recomposes); unkeyed loop items lose state on reorder. **Strong skipping** (on
by default since Kotlin 2.0.20) relaxes but does not erase this. Buiy has **no
recomposition** and therefore none of this stability surface; its change-track
analog is ECS `Changed<T>` keyed to real mutation. The takeaway is validating:
the convenience layer (hoisting, value/onChange, typed tokens) is separable
from — and desirable without — the recomputation engine. Full treatment in
[open-problems.md](open-problems.md) and [lessons.md](lessons.md).

## Cross-links

- Semantics / a11y lens (not duplicated):
  [`../retained-mode-semantics-automation/compose-semantics.md`](../retained-mode-semantics-automation/compose-semantics.md)
  — `Modifier.semantics`, merged/unmerged tree, `testTag`, `ComposeTestRule`.

## Sources

- https://developer.android.com/develop/ui/compose/modifiers
- https://developer.android.com/develop/ui/compose/designsystems/material3
- https://developer.android.com/develop/ui/compose/compositionlocal
- https://developer.android.com/develop/ui/compose/lifecycle
- https://developer.android.com/develop/ui/compose/performance/stability/strongskipping
- https://github.com/androidx/androidx/blob/androidx-main/compose/docs/compose-api-guidelines.md (read 2026-06-26)
- https://developer.android.com/jetpack/androidx/releases/compose-material3
