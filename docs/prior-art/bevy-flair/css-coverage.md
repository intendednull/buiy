**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — CSS feature coverage matrix as of 0.7.0

# CSS coverage

What subset of CSS bevy_flair actually supports vs the public CSS spec, verified against the README, CHANGELOG (0.1 → 0.7), and source. The framing matters: bevy_flair is **explicitly not** trying to be CSS-spec-compliant ("we do not aim for complete CSS specification compliance" — README non-goals). It supports the subset that maps cleanly to bevy_ui's component model + a few high-leverage extras.

## Selectors

| Category | Supported | Notes |
|---|---|---|
| Type selectors | **Yes** | Match against Bevy component type via `TypeName` component (0.4+) or built-in component types. |
| Class (`.foo`) | **Yes** | Class set lives on the entity (component lookup); no global class store. |
| ID (`#foo`) | **Yes** | One ID per entity. |
| `:root` | **Yes** | Matches the root entity of a `Styled` subtree (not the bevy `World` root). |
| Pseudo-class `:hover` / `:active` / `:focus` / `:disabled` / `:checked` | **Yes** | Sourced from `NodePseudoState`; pseudo-state is set by `bevy_ui` input systems before the `PostUpdate` cascade. |
| `:nth-child()`, `:first-child`, `:last-child` | **Yes** | Recalculated when siblings are added (0.2 fix). |
| `:not(...)`, `:has(...)`, `:is(...)`, `:where(...)` | **Yes** | Added in 0.2; uses the Servo `selectors` 0.32 implementation. |
| Pseudo-element `::before` / `::after` | **Yes** (0.4+) | Via `PseudoElementsSupport`. Generates synthetic child entities. |
| Attribute selectors `[foo]`, `[foo=bar]`, `[foo~=bar]`, `[foo\|=bar]`, `[foo^=bar]`, `[foo$=bar]`, `[foo*=bar]` | **Yes** (0.4+) | All Servo `selectors`-supplied. |
| Descendant ` ` | **Yes** | |
| Child `>` | **Yes** | |
| Adjacent sibling `+` | **Yes** | Per CHANGELOG, the README mentions `+` and `~` combinators. |
| General sibling `~` | **Yes** | |
| Nested selectors (`&`) | **Yes** | CSS Nesting Level 1 syntax. |
| `:focus-visible` | **Not mentioned** in README / CHANGELOG. Probably **NO** — would require Buiy-style "keyboard-vs-pointer focus source" tracking that bevy_ui doesn't expose yet. |
| `:placeholder-shown`, `:user-valid`, `:user-invalid`, form-validation pseudo-classes | **Not mentioned**. Probably **NO**. |
| `:nth-of-type()`, `:nth-last-child()`, `:nth-last-of-type()` | Inherited from `selectors` crate; **probably yes** but not explicitly tested. |
| `:dir(ltr/rtl)` | **Not mentioned**. |
| Logical pseudo-classes (`:lang()`, `:scope`) | Inherited from `selectors`; status not verified. |

## At-rules

| At-rule | Status |
|---|---|
| `@import url(...)` | **Yes** (0.2+) — inlines another stylesheet. Font-face imports in nested stylesheets fixed in 0.6. |
| `@font-face` | **Yes** — loads a font file as a Bevy `Font` asset. **Single fonts only — no local fonts, no fallback families** (README limitation). |
| `@keyframes name { ... }` | **Yes** — defines a named keyframe sequence. `var()` in `@keyframes` works (0.6+). |
| `@media (prefers-color-scheme: ...)` / `(width: ...)` / `(height: ...)` / `(resolution: ...)` / `(aspect-ratio: ...)` | **Yes** (0.3+) — multiple properties per query. |
| `@media (prefers-reduced-motion)`, `(prefers-contrast)`, `(forced-colors)` | **Not mentioned** in CHANGELOG; status unverified. |
| `@layer name { ... }` | **Yes** (0.4+) — cascade layers, affects cascade order before specificity. |
| `@container` (container queries) | **NO** — not in CHANGELOG, no source references. |
| `@supports` (feature queries) | **Not mentioned**. Probably **NO**. |
| `@scope` (scoped styling) | **Not mentioned**. Probably **NO**. |
| `@property` (registered custom properties) | **Not mentioned**. Probably **NO**. |
| `@page` | Print-only; **NO**. |

## Property coverage

bevy_flair maps a CSS property name to one or more fields on bevy_ui component types via `PropertyRegistry`. Coverage as of 0.7:

### Layout (via `Node` component)
All `Node` fields: `display`, `position`, `flex-*`, `grid-*`, `gap`, `row-gap`, `column-gap`, `padding-*`, `margin-*`, `width`, `height`, `min-*`, `max-*`, `aspect-ratio`, `top`, `right`, `bottom`, `left`, `inset-*`, `overflow`, `align-*`, `justify-*`, `line-height`. All supported (CHANGELOG 0.3 added grid/gap/aspect-ratio/line-height). Logical properties (`inline-size`, `block-size`, `padding-inline-*`) **not mentioned**; presumed **NO**.

### Color + appearance
- `background-color` → `BackgroundColor` (auto-inserted).
- `background-image` → `BackgroundGradient` or `ImageNode` (gradient parsing in 0.5+).
- `color` → text color (inherited).
- `border-color` → `BorderColor` (auto-inserted).
- `outline-color`, `outline-width`, `outline-offset` → `Outline`.
- Color formats: named colors, hex, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `oklch()`, `oklab()` (via `cssparser-color`).
- **NOT supported:** `color-mix()` (README limitation), `lab()` / `lch()` (not in CHANGELOG), `color()` with custom profiles, relative color syntax.

### Borders
- `border-width`, `border-style`, `border-color`, longhands per side (0.7+ added individual `border-left`, `border-right`, etc.), shorthand `border: 1px solid red`.
- `border-radius` + per-corner longhands → `BorderRadius` (auto-inserted).

### Effects
- `box-shadow` → `BoxShadow` (auto-inserted, interpolatable per 0.7).
- `text-shadow` → `TextShadow` (added 0.3, interpolatable per 0.7).
- `transform` → `UiTransform` (via `translate`, `scale`, `rotate` shorthands per 0.5; full `transform` matrix? unverified).
- `opacity` → presumed via `BackgroundColor` alpha or a dedicated component; not explicitly listed.
- `z-index` → `ZIndex`.
- **NOT supported:** `filter`, `backdrop-filter`, `mix-blend-mode`, `clip-path`, `mask-image`, `isolation` (none referenced in CHANGELOG or README).

### Gradients
- `linear-gradient(...)`, `radial-gradient(...)`, `conic-gradient(...)` — all supported as of 0.5.
- Repeating variants not explicitly mentioned.

### Typography
- `font-family` → loads via `@font-face`. **Inherited.** Single font only.
- `font-size` → text size. **Inherited.**
- `font-weight` → text weight if supported by the loaded font; bevy_ui's text system has limited weight support.
- `line-height` → 0.3+.
- `text-shadow` → 0.3+.
- **NOT supported:** `text-decoration` (presumed — not in CHANGELOG), `text-transform`, `letter-spacing`, `word-spacing`, `text-overflow`, `white-space` (unverified).

### Cursor + interaction
Status of `cursor`, `pointer-events`, `user-select`, `touch-action` — **not in CHANGELOG**. Presumed **NO**.

### Custom Bevy extensions (`-bevy-*` vendor prefix, 0.3+)
- `-bevy-image-mode` → `ImageNode` mode (stretch / tile / 9-slice).
- `-bevy-image-rect` → 9-slice insets (0.6).
- Custom properties without a CSS equivalent live under the `-bevy-` prefix by convention.

### Value functions
| Function | Supported | Notes |
|---|---|---|
| `var(--name)` | **Yes** (0.2+) — custom properties / CSS variables. |
| `calc()` | **Yes** (0.2+) — basic arithmetic, parsed via `CalcAdd` / `CalcMul` / `parse_calc_value`. |
| `min()`, `max()`, `clamp()` | Not explicitly mentioned. Status unverified — probably partial via `calc()` substrate. |
| `env()` | Not mentioned. **NO.** |
| `color-mix()` | **NO** (explicit README limitation). |
| `attr()`, `counter()`, `url(...)` | `url(...)` works for `@font-face` and `background-image`; `attr()` and `counter()` not mentioned. |
| Math functions (`mod()`, `round()`, `sin()`, `pow()`, `sqrt()`) | Not mentioned. Presumed **NO**. |

### Inheritance
- 0.3 fix: all properties now support the `inherit` value.
- Inherited-by-default properties: `color`, `font-family`, `font-size`, possibly others (README mentions "color, font-family, and others inherit to children"). The exact inherited-by-default set is **not enumerated in the public docs** — a real coverage gap if Buiy wants to model this precisely.

### Animations / transitions
- `transition: <property> <duration> <timing-function> <delay>` — **Yes**. Individual-property support overhauled in 0.7; `var()` in `transition` properties added in 0.7.
- `@keyframes` — **Yes**.
- `animation: name duration timing iteration-count direction fill-mode delay` — **Yes**.
- Color interpolation in Oklab space (0.3+).
- Box-shadow + text-shadow interpolation (0.7+).
- Custom time source (0.5+) — animations can run on `Time<Virtual>` instead of `Time<Real>`.
- Transition events (`AnimationEvent`, 0.5.1+).

### Explicit non-goals + limitations
From the README directly:
- **No global stylesheets** — must be attached per-entity-tree via `Styled`.
- **Single stylesheet per entity** — a `Styled` component holds one `Handle<StyleSheet>`. Cascading multiple stylesheets onto the same root is not first-class (must `@import` from one).
- **Single fonts only** via `@font-face` — no fallback families, no local fonts, no font-format negotiation.
- **`!important` ignored** — parsed and detected, logged as a warning, but does not affect cascade order. Cascade order is `@layer` → specificity → source order.
- **No `color-mix()`**.
- **"We don't aim for complete CSS specification compliance."**
- **"We don't implement missing Bevy UI features"** — if `bevy_ui` doesn't support a property (e.g. `filter`), bevy_flair doesn't either.

## Coverage summary vs Buiy's foundation [visuals.md § 3.3](../../specs/2026-05-07-buiy-foundation/visuals.md)

A rough mapping (F = foundation, C = core, E = extended in Buiy's tier system):

| Buiy capability | bevy_flair coverage |
|---|---|
| F-tier `var()` / `calc()` / `min` / `max` / `clamp` | `var()` + `calc()` yes; `min/max/clamp` partial via `calc()` |
| F-tier color formats (sRGB, hex, named, rgba, hsla, currentColor) | Yes minus `currentColor` (unverified) |
| F-tier rounded clipping, top-layer, stacking | Inherited from bevy_ui — none of these are bevy_flair's job |
| C-tier `lab()`, `lch()`, `oklab()`, `oklch()` | `oklch()`/`oklab()` yes; `lab()`/`lch()` no |
| C-tier `color-mix()` | **NO** — explicit non-goal |
| C-tier gradients (linear, radial, conic) | Yes |
| C-tier transitions + `@keyframes` | Yes (0.7 overhaul) |
| C-tier container queries | **NO** |
| C-tier `filter`, `backdrop-filter`, `clip-path`, `mask`, `mix-blend-mode` | **NO** (bevy_ui doesn't support; bevy_flair doesn't either) |
| C-tier logical properties (`inline-size`, `padding-inline-*`) | **Not mentioned** — presumed NO |
| C-tier `transform` (full 2D/3D) | Partial (`translate`, `scale`, `rotate` per 0.5; full `transform` matrix unverified) |
| C-tier anchor positioning | **NO** (Taffy doesn't support yet, bevy_ui doesn't, bevy_flair doesn't) |

Net: bevy_flair covers the cosmetic + layout-property + animation surface that bevy_ui already implements. It does **not** add capabilities bevy_ui lacks. The Buiy "comprehensive web parity" goal ([README.md § 1](../../specs/2026-05-07-buiy-foundation/README.md) goal 1) is therefore **not solved** by adopting bevy_flair — it would still need Buiy's own renderer to ship `backdrop-filter`, `clip-path`, etc. and a separate token / theming layer for forced-colors and `prefers-contrast`.

## Sources

- bevy_flair README — https://github.com/eckz/bevy_flair/blob/main/README.md
- bevy_flair CHANGELOG (0.1 through 0.7) — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- Servo `selectors` crate — https://crates.io/crates/selectors
- Buiy visuals tier list — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Sibling: [`architecture.md`](architecture.md), [`api.md`](api.md), [`lessons.md`](lessons.md)
