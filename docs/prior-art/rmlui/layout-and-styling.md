**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — layout (block, inline, flexbox); animations + transitions; decorators + theming

# Layout and styling

RmlUi's layout engine is **its own** C++ implementation — no Yoga, no Stretch / Taffy, no third-party engine. The engine runs in `Context::Update()` after RCSS cascade and produces the resolved geometry the render pass consumes. The "own engine" choice is RmlUi's largest single architectural commitment and the asymmetry with Buiy that most shapes the comparative lessons.

## Block + inline flow

Inherited from libRocket (2008+) and refined over 15 years. Implements **CSS 2.1 block formatting context**: block boxes, inline boxes, line boxes, basic float positioning. The engine has all the recognisable CSS layout primitives — `padding`, `margin`, `border`, `width`/`height`, `min-`/`max-` constraints, `overflow`. Margin collapse follows the CSS 2.1 rules.

What is **thinly implemented** vs the CSS 2.1 spec:

- **Float / clear** — present but fragile in modern layouts; the docs nudge authors to Flexbox.
- **Table layout** — `display: table` family is present for semantic table data, but the full CSS table-layout algorithm (auto vs fixed, anonymous box generation) is sparse.
- **`vertical-align`** — basic baseline / middle / top / bottom; not the full CSS-spec lattice.

## Flexbox (added 5.0, 2022-12-11)

The biggest layout-feature addition in RmlUi's history. Activated with `display: flex` or `display: inline-flex`. Supports the **container** properties (`flex-direction`, `flex-wrap`, `justify-content`, `align-items`, `align-content`, `gap` / `row-gap` / `column-gap`) and the **item** properties (`flex` shorthand, `flex-grow`, `flex-shrink`, `flex-basis`, `align-self`, `margin: auto`).

**Deliberately omitted** (RmlUi docs § Flexbox):

- `order` property — items appear in source order only.
- `flex-basis: content` value.
- `visibility: collapse` for flex items.
- Anonymous flex items from non-wrapped text.
- Full baseline alignment (RmlUi approximates).
- Item reformatting when stretched.

Docs guidance: *"Avoid content-based sizing"*, prefer `flex: <number>` with definite dimensions. This surfaces the **single-pass layout** cost model — RmlUi's engine cannot afford the intrinsic-sizing roundtrip that CSS Flexbox formally requires, so it asks authors to side-step the cost.

Buiy gets full Flexbox via Taffy without these omissions; the gap is a Taffy quality-validation data point — Taffy has shipped what RmlUi declined to ship.

## NO CSS Grid

The single largest CSS layout gap. RmlUi 6.2 (2026-01-11) still has no `display: grid`, no `grid-template-*`, no `grid-area`, no subgrid, no masonry. There is **no public tracking issue** committing to Grid. After 15 years and a from-scratch layout engine, the Grid feature has not crossed the cost threshold for the project.

Buiy ships Grid via Taffy (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.2 tier **F**, see [`../../plans/2026-05-09-buiy-layout-grid.md`](../../plans/2026-05-09-buiy-layout-grid.md) for the implementation plan). The RmlUi data point is empirical evidence that **own-built engines don't catch up to CSS** — and validates Buiy's substrate-borrow decision (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.2).

## Positioning

`position: static | relative | absolute | fixed` — supported. `position: sticky` — verify; not in 6.2 changelog. No anchor positioning. Containing-block resolution follows CSS rules.

## Writing modes / direction

`direction: ltr | rtl` is parsed. **Vertical writing modes (`vertical-rl`, `vertical-lr`, `sideways-*`) are not supported** in practice. CJK / vertical-Japanese layouts are not a target. BiDi for inline text depends on the FontEngineInterface — FreeType default has no BiDi; the HarfBuzz sample plugin enables shaping but full BiDi paragraph-level resolution is the embedder's responsibility.

## Transforms

`transform`, `transform-origin`, 2D + partial 3D. RCSS `transform:` produces `SetTransform` calls into the RenderInterface; the embedder owns the matrix-stack semantics. Standalone `translate` / `rotate` / `scale` — verify per major release.

## Overflow + scrolling

`overflow: visible | hidden | scroll | auto` plus `-x` / `-y` axis variants. Scrollbars are **styleable child elements** — RmlUi exposes them as `<scrollbarvertical>` / `<scrollbarhorizontal>` sub-elements with pseudo-element selectors (`::scrollbar`, `::sliderbar`, `::sliderarrowinc`, etc.). This is a deviation from CSS `scrollbar-color` / `scrollbar-width` / `scrollbar-gutter` properties — RmlUi exposes scrollbars as DOM-visible, fully styleable.

Inertial scrolling and native touch input added in **6.2** (2026-01-11).

Scroll snap (CSS Scroll Snap Module) — not supported.

## Stacking & paint

Stacking contexts via `z-index`, `opacity < 1`, `transform`, `filter` (6.0+) — standard CSS behavior. `isolation` — not supported. **No CSS top layer** (no `<dialog>` modal that escapes z-index ordering, no popover top layer). Modals are positioned manually with `position: absolute` + high `z-index`.

## Filters, shadows, masks (added 6.0, 2024-08-26)

The **6.0 render-interface redesign** added a substantial visual-effects suite:

- **Filters**: `blur`, `brightness`, `contrast`, `drop-shadow`, `grayscale`, `hue-rotate`, `invert`, `opacity`, `saturate`, `sepia`.
- **`box-shadow`**, **`text-shadow`**.
- **Masks** via `mask-image` family.
- **Gradient decorators** for linear / radial / conic.
- **Render layers** — offscreen render targets enabling above effects.
- **Custom shaders** — embedder-provided GLSL/HLSL stages.

What is **still missing** as of 6.2: `backdrop-filter`, `mix-blend-mode`, `clip-path` (the rendering pipeline still operates on rectangular scissor for the simple path; non-rect masks are limited), `isolation`.

## Decorators — RmlUi's CSS-background replacement

RCSS does not implement CSS `background-image`, `background-position`, `background-repeat`, etc. Instead it exposes a **decorator** property whose value is one or more decorator instances:

```css
button {
    decorator: tiled-horizontal( header-l, header-c, header-r );
}
.icon {
    decorator: image( ui/icon.png );
}
.fade-bg {
    decorator: gradient( vertical #000 #fff );
}
```

Built-in decorators: `image`, `tiled-horizontal`, `tiled-vertical`, `tiled-box` (9-slice), `gradient`, `radial-gradient` (6.x), `conic-gradient` (6.x), `ninepatch`, `text` (6.1).

**Embedders register custom decorators** via the `DecoratorInstancer` interface. This is RmlUi's primary theming + visual-customization escape hatch. It is also the largest single semantic deviation from CSS in RCSS — every web developer expects `background-image: url(...);` to work; in RmlUi the equivalent is `decorator: image(...);`.

## Animations and transitions (added 3.x)

`@keyframes` + `animation: <name> ...` and `transition: <prop> <duration> <timing> <delay>` work per CSS Animations Level 1 + CSS Transitions Level 1. Timing functions include `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `cubic-bezier(...)`, `step-start`, `step-end`, `steps(N, jumpterm)`. Animatable properties cover transform, opacity, color, decorator parameters, and most numeric properties.

**Not supported:** `animation-timeline` / `scroll-timeline` / `view-timeline` (CSS Scroll-Driven Animations); reduced-motion gating (no built-in honoring of `prefers-reduced-motion`, no media-feature support for it).

## Theming approach

RmlUi has **no design-token system** built in. Theming is conventional CSS: a `.rcss` stylesheet defines the look, `<link rel="stylesheet">` includes it. The 6.0 addition of CSS custom properties (`--var`) enables variable-driven theming, but there is no semantic-token vocabulary, no light/dark variant binding, no OS-preference plumbing, no contrast linter.

A single application typically ships one stylesheet per "skin" and switches by toggling stylesheets in C++. No `prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency` media-query support.

## Custom properties (added 6.0)

`--my-var: 8px;` + `var(--my-var)` works as of 6.0. This is the closest RCSS gets to Buiy's token system — but there is no typing, no OS-pref binding, no hot-reload binding.

## Implications for Buiy

- **Buiy's choice to use Taffy** is empirically validated. RmlUi shipped its own engine for 15 years and still has no Grid, no subgrid, no container queries, no anchor positioning, no logical properties — exactly the gaps Buiy plans to close. Taffy gives Buiy these for free.
- **The single-pass layout cost model** (RmlUi's "avoid content-based sizing" guidance) is the engineering reality of a hand-rolled engine. Buiy gets Taffy's full intrinsic-sizing pass and is freed from that author-facing constraint.
- **Decorators as the `background-image` replacement** is RmlUi's most-visible CSS divergence. Buiy commits to CSS-spec `background-image` semantics (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.3 tier **C**). The lesson: replacing a CSS primitive with a custom one is a permanent ecosystem-divergence cost.
- **The 6.0 effects suite (filters, box-shadow, masks)** arriving in 2024, ~5 years into RmlUi's lifetime and ~16 years into the libRocket+RmlUi joint lifetime, demonstrates that **effects can be retrofitted** into an existing layout engine but only via a breaking render-interface redesign. Buiy's `buiy-render-pipeline-design` sub-spec must design effects + masks + top layer + filters in from day one.
- **Theming**: RmlUi has no token system, no OS-preference plumbing, no light/dark variant binding. Buiy's token-based theming (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5) is **explicitly more ambitious** than RmlUi's CSS-only approach — but RmlUi's lack-of-tokens is also evidence that a CSS-flavored library can ship for 15 years without one, which makes the foundation § 5 "CSS-flavored stylesheet" open question feel less urgent if the answer is "tokens are sufficient."
- **`prefers-reduced-motion` absence** in RmlUi is a concrete WCAG SC 2.3.3 gap. Buiy honors reduced-motion automatically from `UserPreferences` (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5).

## Sources

- RmlUi documentation, RCSS Flexboxes — https://mikke89.github.io/RmlUiDoc/pages/rcss/flexboxes.html
- RmlUi changelog (5.0 flexbox; 6.0 filters / masks / shadows / custom properties; 6.1 text decorator; 6.2 inertial scrolling) — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- RmlUi documentation, RCSS overview — https://mikke89.github.io/RmlUiDoc/pages/rcss.html
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Buiy layout plan — [`../../plans/2026-05-09-buiy-layout-grid.md`](../../plans/2026-05-09-buiy-layout-grid.md)
- Buiy container-queries plan — [`../../plans/2026-05-21-buiy-layout-container-queries.md`](../../plans/2026-05-21-buiy-layout-container-queries.md)
