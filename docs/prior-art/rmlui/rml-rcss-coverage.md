**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — RML vs HTML and RCSS vs CSS coverage; what is supported, altered, excluded

# RML and RCSS coverage

RmlUi is **not** an HTML/CSS conformance target. The project README is explicit: *"RmlUi is based around the XHTML1 and CSS2 standards while integrating features from HTML5 and CSS3. We do not aim to be fully compliant with CSS or HTML, in particular when it conflicts with lightness and performance."* The result is a *family* of HTML/CSS — recognizably web-shaped for any web developer, but with its own gaps, additions, and renames.

For Buiy this file is the **single most empirically relevant chapter** in the corpus: if Buiy ever ships a CSS-flavored stylesheet (foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5 open question), RmlUi is the longest-running real-world data point on *which CSS subset is feasible to ship and maintain*.

## RML vs HTML — element coverage

RML is **XHTML 1.0–flavored**: all tags close, attributes are quoted, parsing is strict. The element vocabulary is a deliberate subset.

### Web-equivalent tags (1:1)

- **Structural**: `<html>`, `<head>`, `<title>`, `<body>`, `<link>`, `<script>`, `<style>`.
- **Content**: `<div>`, `<span>`, `<p>`, `<h1>`–`<h6>`, `<a>`, `<img>`, `<br>`, `<hr>`.
- **Forms**: `<form>`, `<input>` (text, password, checkbox, radio, range, submit, button), `<textarea>`, `<select>` + `<option>`, `<button>`, `<label>`.

### RmlUi-specific tags (not in HTML)

- `<panel>` — generic styled block container (used internally by built-in widgets).
- `<handle>` — grab-handle for window-like draggable / resizable parents.
- `<tabset>` + `<tab>` — tab control.
- `<datagrid>`, `<dataselect>` — data-bound table-like and select widgets.
- `<progress>` — supported with a wider feature set than HTML5 `<progress>` (vertical, custom decorators).

### HTML tags **NOT** supported (relevant gaps)

- **Semantic content sectioning**: `<article>`, `<section>`, `<nav>`, `<header>`, `<footer>`, `<aside>`, `<main>`. RmlUi has no analogue and no ARIA-landmark substitute (since there's no a11y story).
- **Media**: `<video>`, `<audio>`, `<picture>`, `<source>`, `<track>`. RmlUi can host **`<img>`** with SVG (via lunasvg) and Lottie (via samples) but not video / audio elements.
- **Embedded**: `<iframe>`, `<canvas>` (no surface for arbitrary 2D draw), `<object>`, `<embed>`.
- **Interactive**: `<details>`, `<summary>`, `<dialog>` (no top-layer modal element), `<menu>`, `<popover>` (CSS `popover` attribute), `<datalist>`, `<output>`.
- **Inline semantic**: `<em>`, `<strong>`, `<mark>`, `<small>`, `<cite>`, `<q>`, `<abbr>`, `<dfn>`, `<time>`, `<code>`, `<kbd>`, `<samp>`, `<var>`. These can be approximated with `<span class="...">` but there is no built-in semantic role.
- **Tables**: HTML `<table>` family parses but full CSS table layout is sparse; data UIs use `<datagrid>` instead.
- **Lists**: `<ul>`, `<ol>`, `<li>`, `<dl>`, `<dt>`, `<dd>` are present; `list-style-*` properties exist but are limited.

### RML-specific attribute additions

- `data-model="modelName"` — declares the C++ data model the subtree binds to.
- `data-bind="property"`, `data-bind-value`, `data-bind-checked`, `data-bind-class`, `data-bind-style-*` — one-way + two-way binding.
- `data-for="item : items"` — list templating.
- `data-event-click="handler"`, `data-event-*` — event handler wiring.
- `data-if`, `data-show` — conditional render.

## RCSS vs CSS — property coverage

RCSS parses CSS 2.1 syntactically (selectors, declarations, at-rules) and supports a **subset of CSS 2.1 + selective CSS 3** properties. The list below is grouped by category; "supported" means RCSS will accept the property and render meaningful behavior, "altered" means semantics differ from CSS, "missing" means absent or no-op.

### Box model — supported

`width`, `height`, `min-width`, `max-width`, `min-height`, `max-height`, `padding(-*)`, `margin(-*)`, `border-width(-*)`, `border-color(-*)`, `border-style(-*)` (partial — `solid` and a few variants), `border-radius(-*)` (since 4.x), `box-sizing`.

**Gaps:** logical properties (`inline-size`, `block-size`, `padding-inline-*`, `margin-block-*`) are **not** supported. Buiy commits to logical properties (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.2 "Logical properties" tier **F**); RmlUi's absence is a contrast.

### Display modes — partially supported

- `display: block | inline | inline-block | flex | inline-flex | table | table-row | table-cell | none` — supported.
- `display: grid | inline-grid` — **NOT** supported. No CSS Grid in RmlUi as of 6.2. Buiy commits to Grid via Taffy (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.2 tier **F**); RmlUi's absence is the largest single feature gap.
- `display: contents` — not supported.
- `display: flow-root` — not supported.
- `display: list-item` — partially supported.

### Positioning — supported with gaps

- `position: static | relative | absolute | fixed` — supported.
- `position: sticky` — **NOT** supported (verify; not present in 6.2 changelog).
- `top`, `right`, `bottom`, `left` — supported.
- `inset` shorthand — verify per major version; not historically supported.
- **Anchor positioning** (`anchor-name`, `position-anchor`, `anchor()`, `position-try`) — **NOT** supported. Buiy plans to ship these (foundation tier **C**).
- `z-index` — supported.

### Flexbox — added 5.0, deliberately incomplete

Activated via `display: flex` / `inline-flex`. Supported:

- `flex-direction` (row, row-reverse, column, column-reverse).
- `flex-wrap` (nowrap, wrap, wrap-reverse).
- `flex` shorthand, `flex-grow`, `flex-shrink`, `flex-basis`.
- `justify-content`, `align-items`, `align-self`, `align-content`.
- `margin: auto` alignment.
- `row-gap`, `column-gap`, `gap` shorthand.

**Deliberately omitted** (RmlUi docs § Flexbox):

- `order` property.
- `flex-basis: content` value.
- `visibility: collapse`.
- Anonymous flex items from non-wrapped text.
- Full baseline alignment (only approximate).
- Item reformatting when stretched.

Performance guidance: docs recommend avoiding content-based sizing, prefer `flex: <number>` with definite dimensions. This is the explicit cost-model that surfaces RmlUi's single-pass layout.

### Grid — NOT SUPPORTED

No `display: grid`, no `grid-template-*`, no `grid-area`, no subgrid, no masonry. The single largest CSS feature gap vs both modern web and Buiy.

### Float / clear — supported, limited

Inherited from libRocket. `float: left | right` works but is fragile in modern layouts. Modern RmlUi authors use Flexbox.

### Multi-column — NOT SUPPORTED

No `column-count`, `column-width`, `column-gap`, `column-rule`, `column-span`, `break-*`. Listed as Buiy tier **E** (foundation § 3.2).

### Container queries — NOT SUPPORTED

No `@container`, no `container-type`, no `cqw/cqh/cqi/cqb` units. Buiy ships these (foundation tier **C**, see [`../../plans/2026-05-21-buiy-layout-container-queries.md`](../../plans/2026-05-21-buiy-layout-container-queries.md)).

### Writing modes & direction — limited

`direction: ltr | rtl` is parseable. `writing-mode: horizontal-tb | vertical-rl | vertical-lr | sideways-rl | sideways-lr` — vertical and sideways modes are **NOT** supported in practice (verify per major version). BiDi text rendering depends on FontEngine (FreeType default has none; HarfBuzz sample plugin enables it).

### Overflow & scrolling — supported

- `overflow: visible | hidden | scroll | auto`, `-x`, `-y` — supported.
- Scrollbar element (`<scrollbar>` as a styleable child of scrollable elements; pseudo-elements `::scrollbar`, `::sliderbar`, `::sliderarrowinc`, etc.) — RmlUi-specific shape, no `scrollbar-color` / `scrollbar-width` / `scrollbar-gutter`.
- **Scroll snap** (CSS Scroll Snap module) — **NOT** supported.
- **Smooth scroll** (`scroll-behavior: smooth`) — limited; inertial scrolling added in 6.2.

### Stacking & paint — partial

- Stacking contexts via `z-index`, `opacity`, `transform`, `filter` — supported.
- `isolation` — **NOT** supported.
- **CSS top layer** (for true `<dialog>` modal, popovers) — **NOT** supported.
- `mix-blend-mode`, `background-blend-mode` — **NOT** supported.

### Units — most CSS units supported

`px`, `em`, `rem`, `%`, `vw`, `vh`, `vmin`, `vmax` — supported. `dp` (density-independent, RmlUi-specific) — supported.

**NOT supported:** `ch`, `lh`, `rlh`, `cap`, `ic`, `ex`, container-query units (`cqw/cqh/cqi/cqb`), small/large/dynamic viewport variants (`svw`, `lvw`, `dvw`, etc.), `fr` (since no Grid).

### Transforms & containment — partial

- `transform`, `transform-origin`, 2D — supported.
- 3D transforms (`perspective`, `backface-visibility`, `transform-style`) — partially supported.
- Standalone `translate`, `rotate`, `scale` — verify per major version.
- `will-change` — limited.
- `contain` (layout / paint / size / style / strict) — **NOT** surfaced as a named property; some equivalent containment is implicit per stacking context.
- `content-visibility` — **NOT** supported.

### Color — partial

- Named colors, `transparent` — supported.
- `rgb()`, `rgba()`, `hsl()`, `hsla()` — supported.
- `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()` — **NOT** supported.
- `color()` profiles, `color-mix()` — **NOT** supported.
- `currentColor` — verify.
- System color keywords (`Canvas`, `CanvasText`, etc., used for forced-colors mode) — **NOT** supported.

### Backgrounds — limited via decorators

`background-color` — supported. Beyond that, RmlUi uses its own **decorators** system (`decorator: image(...);`, `decorator: tiled-horizontal(...);`, `decorator: gradient(...);`) rather than CSS `background-image` / `background-position` / `background-repeat`. Decorators are flexible (extensible via custom user decorators) but not CSS-compatible. **The single largest deviation from CSS semantics in RCSS.**

### Borders — supported

`border-*` family + `border-radius` (since 4.x). `border-image` — **NOT** supported.

### Shadows, filters, effects — added 6.0

- `box-shadow`, `text-shadow` — supported as of 6.0.
- `filter` (blur, brightness, contrast, drop-shadow, grayscale, hue-rotate, invert, opacity, saturate, sepia) — added 6.0.
- `backdrop-filter` — **NOT** supported.
- `clip-path` — **NOT** supported (rectangular clip only; `mask` is the limited substitute).
- `mask-image` family — partially supported as of 6.0.
- `opacity` — supported.
- `visibility: visible | hidden` — supported. `visibility: collapse` — not in flex.

### Animations & transitions — supported

- `transition: <prop> <duration> <timing> <delay>` — supported (since 3.x).
- `@keyframes` + `animation: <name> <duration> ...` — supported (since 3.x).
- Timing functions: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(...)`, `step-start`, `step-end`, `steps(...)`.
- `animation-timeline`, `scroll-timeline`, `view-timeline` (scroll-driven animations) — **NOT** supported.
- `prefers-reduced-motion` honoring — **NO** built-in; embedder would have to set a class.

### Custom properties + value functions — added 6.0

- `--var: value;` + `var(--var)` — supported as of 6.0.
- `calc(...)` — supported since 5.x.
- `min(...)`, `max(...)`, `clamp(...)` — verify per release; partially supported.
- `mod()`, `rem()`, `round()`, `abs()`, `sign()`, `pow()`, `sqrt()`, trig functions — **NOT** supported.
- `env()` — **NOT** supported (no UA-value namespace).

### Selectors — CSS 2.1 + partial CSS 3

- Type, class, id, descendant, child (`>`), adjacent sibling (`+`), general sibling (`~`) — supported.
- Attribute selectors (`[attr]`, `[attr=value]`, `[attr^=value]`, `[attr$=value]`, `[attr*=value]`) — supported.
- Pseudo-classes: `:hover`, `:active`, `:focus`, `:checked`, `:disabled`, `:not(...)`, `:first-child`, `:last-child`, `:nth-child(...)`, `:empty` — supported.
- `:focus-visible`, `:focus-within`, `:placeholder-shown`, `:user-invalid`, `:has(...)`, `:is(...)`, `:where(...)` — **NOT** supported.
- Pseudo-elements: `::before`, `::after` — verify (presence of generated-content varies). `::placeholder`, `::selection`, `::marker` — limited.

### At-rules — supported

- `@import` — supported.
- `@keyframes` — supported (since 3.x).
- `@media` — supported with limited media features (`width`, `height`, orientation-style queries).
- `@font-face` — verify; fonts are typically loaded via `Rml::LoadFontFace()` in C++.
- `@property` (registered custom properties), `@container`, `@layer`, `@scope` — **NOT** supported.

## Decorators — RmlUi-specific

RCSS extends CSS with a **decorator** property that handles what CSS would do via `background-image`, `border-image`, masks, and gradients. Built-in decorators:

- `image` — single image fill.
- `tiled-horizontal`, `tiled-vertical` — tiling.
- `tiled-box` — 9-slice (border-image analogue).
- `gradient` — linear gradient.
- `radial-gradient` (since 6.x), `conic-gradient` (since 6.x).
- `ninepatch` — explicit 9-slice.
- `text` (since 6.1) — text-shaped decorator.

The embedder can register **custom decorators** via the `DecoratorInstancer` interface. This is RmlUi's primary theming + visual-customization extension point — a fork in the road from CSS.

## Implications for Buiy

- **The "CSS subset that fits in a game engine" answer**, after 15+ years of empirical iteration, looks like *CSS 2.1 block/inline + Flexbox (since 5.0) + transitions/animations + transforms + filters/shadows (since 6.0)*. **Conspicuously missing**: CSS Grid, container queries, anchor positioning, logical properties, modern color spaces, mix-blend-mode, true top layer, `clip-path`, `:has()`, CSS Nesting, scroll-driven animations. Validates Buiy's foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) tier list: anything RmlUi ships is empirically feasible; anything RmlUi *omits* deserves scrutiny on whether Buiy genuinely needs it as **F** vs **C/E**.
- **Decorators as the CSS-`background` replacement** is the single most-visible RmlUi divergence from CSS spec. Lesson: when a game UI library *invents its own primitive* in place of a CSS feature, the cost is permanent — every author has to learn the custom shape, and the wider CSS ecosystem (Stylelint, designer tools, MDN reference) doesn't apply. Buiy commits to CSS-spec semantics (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.3 backgrounds tier **C**: `background-image` + URL + gradients, multiple layers, position/size/repeat) — **explicitly the choice RmlUi did not make**.
- **Selectors gap (`:has()`, `:is()`, `:where()`, `:focus-visible`)** is small in scope but disproportionately painful for component authors. Buiy's `:focus-visible` is **F** (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.3 "Outline & focus indicators"); RmlUi's lack is one reason its accessibility story is weaker than the CSS subset alone would suggest.
- **The `display: grid` absence** is the single biggest CSS layout gap. Buiy gets Grid for free via Taffy. The RmlUi data point: a own-built layout engine over 15 years still hasn't added Grid; the engineering cost is real.
- **The 6.0 redesign that finally added filters, custom properties, masks** is a 5-year-late acknowledgment that CSS effects had moved on. Lesson: if Buiy ever ships a CSS-flavored stylesheet (foundation § 5 open question), pin the spec target to a *snapshot date* (e.g. "CSS Snapshot 2024") and commit to the full snapshot rather than ratcheting features one by one — RmlUi's incremental ratchet has produced an inconsistent CSS dialect that takes years per gap to close.

## Sources

- RmlUi README — https://github.com/mikke89/RmlUi (specifically the "based around the XHTML1 and CSS2 standards" framing)
- RmlUi documentation, RCSS overview — https://mikke89.github.io/RmlUiDoc/pages/rcss.html
- RmlUi documentation, Flexboxes — https://mikke89.github.io/RmlUiDoc/pages/rcss/flexboxes.html
- RmlUi changelog (filter/mask/box-shadow added in 6.0, decorators evolution) — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Buiy foundation README (CSS stylesheet open question § 5) — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
