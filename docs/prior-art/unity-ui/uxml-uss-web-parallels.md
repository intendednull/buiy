**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — UXML vs HTML and USS vs CSS feature-coverage audit; Unity's deviations from web spec

# UXML/USS ↔ HTML/CSS audit

This file audits, feature-by-feature, what Unity's UXML/USS pair covers from the web platform and where it deviates. Buiy's foundation §5 leaves "CSS-flavored stylesheet — never, or future layer?" as an open question; this audit is the comparative evidence for that decision. Pair with [`docs/prior-art/bevy-flair/`](../bevy-flair/) (the Bevy precedent for CSS-on-ECS).

## UXML vs HTML — markup mapping

| HTML construct | UXML construct | Notes |
|---|---|---|
| `<div>` | `<VisualElement>` | The catch-all container. |
| `<button>` | `<Button>` | Built-in selectable. |
| `<input type="text">` | `<TextField>` | IME support via Unity's input system; one-line by default; `multiline` attribute. |
| `<input type="checkbox">` | `<Toggle>` | Includes built-in label. |
| `<input type="radio">` + form | `<RadioButton>` + `<RadioButtonGroup>` | Group provides exclusivity. |
| `<input type="range">` | `<Slider>` / `<SliderInt>` | Both axes via `direction` attribute. |
| `<select>` | `<DropdownField>` | Custom popup; not OS-native. |
| `<label>` | `<Label>` | Plain text element. |
| `<p>` | `<Label>` | No separate paragraph element; text always inside `<Label>` or `<TextElement>`. |
| `<ul>`/`<ol>`/`<li>` | `<ListView>` | Virtualized list (not a markup element per-item — items are templated). |
| `<details>`/`<summary>` | `<Foldout>` | Built-in collapsible. |
| `<table>` | `<MultiColumnListView>` | Spreadsheet-style; no `<th>`/`<tr>` equivalent. |
| `<dialog>` / `<dialog popover>` | No direct equivalent | Panel-stacking workaround; no CSS top layer. |
| `<a>` | No equivalent | UI Toolkit has no link semantics. |
| `<img>` | `style="background-image: url(...)"` on `<VisualElement>` | No standalone `<Image>` element in UXML. |
| `<script>` | C# code-behind | UXML loads via `UIDocument`; the `MonoBehaviour` references the asset. |
| `<style>` / `<link>` | `<Style src="…"/>` | USS asset reference. |
| `<template>` / Web Components | `<Instance template="..."/>` + custom `VisualElement` subclass | Subclass + UXML factory + USS-stylable. |
| `id="..."` | `name="..."` | UQuery selects by name. |
| `class="..."` | `class="..."` | Same semantics; USS class selectors work identically. |
| `data-*` attributes | `userData` C# field | Single-slot, not per-key. |
| `aria-*` attributes | **None** | No ARIA model exposed in UXML. Closest: Accessibility module C# API for screen-reader labels (Unity 2023.2+). |

## USS vs CSS — property/feature coverage

**Legend:** ✅ supported · ⚠️ partial · ❌ not supported · ➕ Unity-specific addition

### Layout

| CSS feature | USS status | Notes |
|---|---|---|
| Flexbox (`flex-*`, `align-*`, `justify-content`, `flex-direction`, `flex-wrap`, `flex-basis`, `flex-grow`, `flex-shrink`) | ✅ | Implemented via Yoga (subset). |
| `display: block` / `inline` / `inline-block` / `grid` / `contents` | ❌ | `display` accepts only `flex` and `none`. |
| `position: relative` / `absolute` | ✅ | |
| `position: fixed` / `sticky` | ❌ | |
| `top` / `right` / `bottom` / `left` | ✅ | |
| `width` / `height` / `min-` / `max-` / `aspect-ratio` | ✅ / ❌ on `aspect-ratio` | |
| `margin` / `padding` / `border-width` | ✅ | |
| `box-sizing` | ❌ | Unity uses border-box semantics implicitly. |
| **CSS Grid** | ❌ | Yoga does not implement Grid. |
| **Subgrid** | ❌ | |
| **Container queries** (`@container`) | ❌ | |
| **Anchor positioning** (`anchor()`, `inset-area`) | ❌ | |
| `gap` / `row-gap` / `column-gap` | ⚠️ | Limited; Yoga added `gap` later. |
| Writing modes (`writing-mode: vertical-rl`) | ❌ | No vertical text layout. |
| Logical properties (`margin-inline-start`, etc.) | ❌ | Only physical (left/right/top/bottom). |

### Color, background, decoration

| CSS feature | USS status | Notes |
|---|---|---|
| `color` | ✅ | Hex / `rgb()` / `rgba()` / `hsl()` / Unity color name. |
| `background-color` / `background-image` | ✅ | |
| `background-position` / `background-size` / `background-repeat` | ⚠️ | `-unity-background-scale-mode` covers stretch/fit/crop; not full CSS shorthand. |
| **Multiple backgrounds** | ❌ | One background per element. |
| `border` / `border-color` / `border-radius` | ✅ | `border-radius` per-corner; **no elliptical second-radius shorthand**. |
| `outline` | ❌ | No outline-with-offset; use border on a wrapper. |
| `box-shadow` | ❌ | No native box-shadow USS property; effects require shader or post-processing filter. |
| **`mix-blend-mode` / `isolation`** | ❌ | |
| **`backdrop-filter`** | ❌ | |
| **`filter`** | ⚠️ | Per-element filter via post-processing API (separate, not a USS property). |
| **`clip-path`** | ❌ | Rectangular only via `overflow: hidden`. Rounded via `border-radius` + overflow. |
| **9-slice background** | ➕ | `-unity-slice-*` Unity-specific; web has none. |

### Text

| CSS feature | USS status | Notes |
|---|---|---|
| `font-family` | ⚠️ | `-unity-font` / `-unity-font-definition` (TMP font asset reference). Not arbitrary CSS font-family. |
| `font-size` / `font-weight` / `font-style` | ⚠️ | `font-size` ✅; weight/style via `-unity-font-style` enum (normal/bold/italic/bold-and-italic). No 100..900 weight scale. |
| `line-height` | ❌ | No direct property; controlled by font asset + `-unity-paragraph-spacing`. |
| `text-align` | ⚠️ | `-unity-text-align` enum (upper-left, middle-center, etc.) — not standard CSS keywords. |
| `text-decoration` (underline / strikethrough) | ⚠️ | Via TMP rich-text inline markup, not USS property. |
| `text-shadow` | ❌ | `-unity-text-outline-*` for outline; no shadow. |
| `text-transform` | ❌ | |
| `letter-spacing` / `word-spacing` | ❌ | |
| `white-space` | ⚠️ | `white-space` accepts `normal` and `nowrap` only. |
| `overflow-wrap` / `word-break` | ❌ | |
| **BiDi** / `direction` | ⚠️ | TMP handles BiDi rendering; USS has no `direction` property. |
| **Custom fonts via `@font-face`** | ❌ | Fonts are project assets (`.ttf`/`.otf` → font asset). |

### Layout flow + interaction

| CSS feature | USS status | Notes |
|---|---|---|
| `overflow: hidden` / `scroll` / `auto` | ⚠️ | `overflow: hidden / visible` only. Scrolling is via `<ScrollView>` element, not the `overflow` property. |
| `cursor` | ✅ | Single-cursor (no fallback chain). |
| `pointer-events` | ⚠️ | `pickingMode` C# property; not a USS property per se. |
| `visibility` | ✅ | |
| `opacity` | ✅ | |
| `z-index` | ⚠️ | Sibling order determines stacking by default; `style.depth` available via C#. No CSS stacking-context model. |
| **CSS top layer** (`<dialog>`, `:popover-open`) | ❌ | Panel layering only. |

### Color/units/functions

| CSS feature | USS status | Notes |
|---|---|---|
| `px`, `%` | ✅ | |
| `em`, `rem`, `vh`, `vw`, `vmin`, `vmax`, `ch` | ❌ | Only `px` and `%` for size units. |
| `calc()` | ❌ | No expression support. |
| `var(--token)` / custom properties | ✅ | Variables work as in CSS. |
| `attr()` | ❌ | |
| `env()` | ❌ | |
| `color-mix()` / `color-contrast()` | ❌ | |
| Color spaces (`lch()`, `oklch()`, `display-p3`) | ❌ | sRGB only. |

### Transitions, animations, transforms

| CSS feature | USS status | Notes |
|---|---|---|
| `transition` / `transition-property` / `transition-duration` / `transition-delay` / `transition-timing-function` | ✅ | USS Transitions (Unity 2021+). |
| **`@keyframes`** / `animation-*` family | ❌ | No keyframed animation. Per Unity's own UI comparison table. |
| `transform: translate/rotate/scale` | ✅ | |
| `transform: matrix()` / 3D transforms | ⚠️ | Limited; preserve-3d not exposed. |
| `transform-origin` | ✅ | |
| `perspective` | ❌ | |
| `will-change` | ❌ | |

### Selectors

| CSS feature | USS status | Notes |
|---|---|---|
| Type / class / id / descendant / child / universal | ✅ | |
| `:hover` / `:active` / `:focus` / `:disabled` / `:checked` / `:root` | ✅ | |
| `:focus-visible` / `:focus-within` | ❌ | |
| `:nth-child()` / `:nth-of-type()` / `:first-child` / `:last-child` | ❌ | |
| `:not(...)` | ⚠️ | Supported in some Unity versions; verify per release. |
| `::before` / `::after` | ❌ | |
| `[attr=val]` attribute selector | ❌ | |
| `@media` (any) | ❌ | No media queries. |
| `@supports` | ❌ | |

### At-rules / structural

| CSS feature | USS status | Notes |
|---|---|---|
| `@import` | ⚠️ | USS can `@import` other USS in some configurations; `<Style src="..."/>` in UXML is more common. |
| `@layer` (cascade layers) | ❌ | |
| `@font-face` | ❌ | |
| `@property` (custom property registration) | ❌ | |

## Net assessment

- **Web parity score (informal):** UI Toolkit covers roughly **the Flexbox-subset core of modern web platform** plus a handful of Unity-specific extensions (9-slice backgrounds, TMP integration, scheduled callbacks). It does **not** cover Grid, container queries, anchor positioning, calc(), keyframes, media queries, top layer, color spaces, complex selectors. The gap is meaningful.
- **Stylesheet-language familiarity is real but partial.** A web dev reads USS without ceremony; they hit "no Grid" within hours of trying to build a settings page and "no calc()" within minutes of trying to express derived sizes.
- **The Unity-specific additions are useful.** `-unity-slice-*` (9-slice backgrounds for stretchy borders/panels), `-unity-text-outline-*` (text outlines as a first-class property), `-unity-background-scale-mode` (image fit modes) all solve real game-UI needs CSS doesn't address.

## Implications for Buiy

1. **The CSS-flavored-stylesheet open question** (foundation README §5) — USS demonstrates that a CSS-subset *plus* engine-specific extensions is shippable at scale. If Buiy ever adds a stylesheet layer, the architectural call is whether to:
   - (a) Match CSS strictly where Buiy supports the feature, and **omit** anything Buiy doesn't (the bevy_flair direction), or
   - (b) Match USS's "subset + prefixed extensions" approach.
   - **The corpus's read:** option (a) reduces onboarding friction. USS's `display: flex|none` (no `block`) and `-unity-font-definition` are exactly the moments where USS feels gameengine-y rather than web-y, and they accumulate.
2. **Anchor positioning + container queries are Taffy-owned.** Buiy's foundation already commits to Taffy as the layout substrate (architecture.md §2.2). Taffy ships these features ahead of Yoga and ahead of Unity's USS surface; Buiy gets them for free without USS-style negotiation.
3. **Keyframes are mandatory.** USS's transitions-only choice is the *single most-cited* runtime UI Toolkit gap in 2024-2026 community discussions. `buiy-animation-design` must commit to keyframes day-one.
4. **Top layer and clipping are render-pipeline concerns, not stylesheet concerns.** Buiy's foundation §2.3 already commits to true top layer and `clip-path` shapes in the render pipeline; even if Buiy never ships a stylesheet, the *visual capability* must be present. UI Toolkit's gap here is a renderer gap, not a USS gap.
5. **Decomposed components vs USS class theming.** USS class theming is the megacomponent-style approach to theming (one selector hits many properties); Buiy's token-based theming (foundation §2.5) is the decomposed-component-style approach (semantic tokens per property). The trade-off is real: USS is more expressive per-element-tweak; tokens are more disciplined and more accessible. Buiy stays with tokens; USS classes can be a future layer atop tokens, not a replacement.

## Sources

- USS supported properties — https://docs.unity3d.com/Manual/UIE-USS-SupportedProperties.html
- USS overview — https://docs.unity3d.com/Manual/UIE-USS.html
- USS Transitions — https://docs.unity3d.com/6000.2/Documentation/Manual/UIE-Transitions.html
- USS properties reference — https://docs.unity3d.com/Manual/UIE-USS-Properties-Reference.html
- UXML VisualElement reference — https://docs.unity3d.com/Manual/UIE-uxml-element-VisualElement.html
- UXML Elements Reference (community mirror) — https://docs.pumachen.xyz/unity-doc/Manual/UIE-ElementRef.html
- UI system comparison (Unity 6.3) — https://docs.unity3d.com/6000.3/Documentation/Manual/UI-system-compare.html
- bevy_flair prior art (CSS on Bevy precedent) — [`../bevy-flair/`](../bevy-flair/)
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) (open question §5)
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
