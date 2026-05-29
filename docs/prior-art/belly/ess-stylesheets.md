**Date:** 2026-05-22
**Status:** active
**Subject:** belly's `.ess` stylesheets — CSS-like styling above bevy_ui, with selector/property coverage and a direct comparison to bevy_flair

# The `.ess` stylesheet

`.ess` (Element Style Sheet) is belly's CSS-like asset format. Files load via `commands.add(StyleSheet::load("path.ess"))` and parse into a cascade asset that writes to bevy_ui style components at runtime. The `.ess` filename extension is belly's choice; the format is a CSS subset with belly-specific extensions.

`.ess` is the closest direct competitor to bevy_flair's `.css` loader — but belly does not use Servo's `cssparser`. The parser is hand-rolled inside `belly_core`, and the resulting subset is narrower than bevy_flair's coverage. Cross-link: [`../bevy-flair/css-coverage.md`](../bevy-flair/css-coverage.md) for the published-alternative's property + selector inventory.

## Canonical example

From `assets/stylesheet.ess` shipped with the `style-sheet.rs` example:

```css
/* stylesheet.ess */

body {
  padding: 5px;
  flex-direction: column;
}

span {
  padding: 25px;
  margin: 5px;
  background-color: black;
}

div {
  font: bold;
  color: black;
  padding: 3px;
  background-color: white;
  margin-left: 10%;
}
```

Three tag selectors, eight properties between them, no pseudo-classes in this minimal example. The bigger `selectors.rs` example layers class selectors (via `c:`-prefix), pseudo-class selectors (`:hover`), and descendant combinators (` `) on top.

## Selectors

belly's selector language — observed across the examples, the parser source, and the docs:

| Selector | Form | Notes |
|---|---|---|
| Tag | `button { … }` | Matches by belly widget name (the same identifier used in `eml!`) |
| Class | `.red { … }` | Matches classes assigned via `c:red` attribute in `eml!` |
| ID | `#submit { … }` | Matches by `id="submit"` attribute |
| Descendant | `button .content { … }` | Whitespace combinator |
| Pseudo-class | `:hover` | Plus `:active`, `:focus` (per source) — pseudo-class set is limited |
| State class | (via class manipulation) | belly toggles classes for state, partly substituting for richer pseudo-classes |

**Not supported** (compared to CSS):

- Child combinator `>` — not documented or visible in examples
- Sibling combinators `+` / `~` — absent
- Attribute selectors `[name=value]` — absent
- Pseudo-elements `::before` / `::after` — absent
- `:focus-visible` — absent (would need belly's own focus model, which it doesn't have)
- `:nth-child(…)` family — absent
- `:not(…)`, `:is(…)`, `:where(…)` — absent
- `@media` queries — absent (no OS-pref support)
- `@container` queries — absent
- Cascade layers `@layer` — absent
- `!important` — not documented; behavior on encountering it is undefined

The pseudo-class story is the most consequential gap. WCAG 2.4.7 *focus visible* relies on `:focus-visible` semantics. belly's lack of it means a belly-styled UI cannot ship the WCAG-conformant focus-ring contract without bypassing the stylesheet for focus styling.

## Properties

Per `docs/style-properties.md` at v0.5.0, the supported property list maps directly to bevy_ui's `Style` struct fields. Verbatim from the doc:

**Layout (Style component):**
- `display`, `position-type`, `direction`
- `width`, `height`, `min-width`, `max-width`, `min-height`, `max-height`
- `aspect-ratio`
- `top`, `right`, `bottom`, `left` (or `position` as a `$rect`)
- `margin` (longhand + `margin-{top,right,bottom,left}`)
- `padding` (longhand + `padding-{top,right,bottom,left}`)
- `border-width` (longhand + per-side)
- `flex-direction`, `flex-wrap`, `flex-basis`, `flex-grow`, `flex-shrink`
- `align-content`, `align-items`, `align-self`
- `justify-content`
- `overflow`

**Paint:**
- `background-color`, `color`
- `z-index` (with `auto` / `$local` / `$global`)

**Text:**
- `font` (with values `regular` / `bold` / `italic` / `bold-italic` / `$string` for a font path)
- `font-size`

**Stylebox (belly-specific):**
- `stylebox-source` (path to nine-slice image)
- `stylebox-slice` (slice rect)
- `stylebox-region` (sub-region rect)
- `stylebox-width` (border widths)
- `stylebox-modulate` (tint color)

**Not supported** (relative to bevy_flair / CSS):

- `border-radius` — belly's bevy_ui target (0.13) had a single `BorderRadius` but belly didn't expose it as a property
- `border-color` (per side or aggregate)
- `box-shadow`, `text-shadow`
- `filter`, `backdrop-filter`, `mix-blend-mode`, `isolation`
- `clip-path`, `mask`, `opacity`
- `outline-*`, `accent-color`, `caret-color`
- `cursor`, `pointer-events`, `user-select`, `touch-action`
- `transform`, `translate`, `rotate`, `scale`
- `transition-*`, `animation-*` (the README marks transitions as "coming soon" — they never arrived)
- Linear/radial/conic gradients
- CSS custom properties (`--name`) and `var()` — belly has no equivalent
- `calc()`, `clamp()`, `min()`, `max()`
- Container queries / `@media` / `@container`
- Logical properties (`inline-size`, `padding-inline-*`, etc.)
- `oklab()`, `oklch()`, `display-p3`, `color-mix()`

The property coverage is markedly narrower than bevy_flair's (cf. [`../bevy-flair/css-coverage.md`](../bevy-flair/css-coverage.md)). bevy_flair has shipped `var()` + `calc()` + Oklab transitions + `@media (prefers-color-scheme)` since its 0.3 release; belly stopped at the bevy_ui 0.13 `Style` field set.

## Cascade behavior

belly's cascade is **undocumented in detail**. From source-code observation:

- **Specificity** roughly follows CSS: `#id` > `.class` > `tag`, longer compound > shorter.
- **Order** within equal specificity: later rule wins (declaration order).
- **Inline `s:` styles** appear to override stylesheet rules — there is no documented `!important` escape from this.
- **Inheritance**: text-related properties (`color`, `font`, `font-size`) appear to inherit; layout properties don't. The exact inherited set is not enumerated anywhere.
- **`!important`**: parser may accept it or may not — undocumented; the absence of any test or example referencing it means the behavior should be treated as undefined.

bevy_flair documents its cascade explicitly down to the eleven-stage system pipeline; belly does not. This is one place belly's lower-decomposition pipeline cost shows up as a documentation gap.

## Hot-reload

`.ess` files reload via Bevy's asset system. Edit a file in `assets/`, save, and the running app picks up the change without restart. This was a marquee feature in early belly releases and **works** in v0.5.0 — at least to the extent the example apps demonstrate. Cf. bevy_flair's hot-reload story, which is similarly first-class ([`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) § Validates).

The hot-reload story is one of the strongest validates for "the cascade-engine pattern is the right primitive on Bevy" — both belly and bevy_flair independently arrived at the same answer.

## Direct comparison to bevy_flair

| Dimension | belly `.ess` | bevy_flair `.css` |
|---|---|---|
| Parser | hand-rolled (`belly_core`) | Servo `cssparser` |
| Property count | ~30, narrow | ~60+, broader |
| `var()` / `calc()` | absent | present (F-tier coverage) |
| `@media` queries | absent | `prefers-color-scheme` only |
| `:focus-visible` | absent | absent (bevy_ui doesn't model it) |
| Hot-reload | works | works |
| `!important` | undocumented (avoid) | parsed but silently ignored (avoid) |
| Cascade docs | sparse | thorough |
| Crates.io | not published | published |
| Bevy version | 0.13 (April 2024) | tracks current Bevy |
| Adoption | 436 stars, 0 production users verifiable | 27 stars at last count, but published + tracking-current |

The blunt summary: bevy_flair is the smaller-scope, better-engineered, currently-maintained published alternative. belly is the broader-scope, less-engineered, abandoned-on-Bevy-0.13 prototype. Neither has production-scale adoption.

## Implications for Buiy

The `.ess` precedent contributes the following to Buiy's open question on a stylesheet layer ([foundation README § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)):

1. **Two independent attempts arrived at "cascade engine that writes to decomposed style components."** belly and bevy_flair didn't coordinate; they converged on the same shape. This is meaningful evidence that the pattern is the right primitive on Bevy.
2. **A narrow property subset is acceptable for a v1.** belly shipped with ~30 properties and was usable for the demo set; coverage can grow. Buiy doesn't need to ship every CSS property on day 1 if it ships a stylesheet at all.
3. **Hand-rolling the parser is a maintenance debt.** belly stalled after one developer. bevy_flair's choice to lease Servo `cssparser` saved months and produced a better-conforming parser. Buiy must use the Servo crates if it ships a stylesheet layer — see [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md) § Borrow.
4. **Cascade docs are not optional.** belly's undocumented specificity / inheritance / `!important` handling is the file's biggest debugging hazard. A Buiy stylesheet spec must explicitly enumerate inherited properties, specificity rules, precedence among stylesheet / inline / programmatic / BSN, and `!important` handling.
5. **OS-pref support is a hard requirement, not an extension.** Neither belly nor bevy_flair covers `prefers-contrast`, `prefers-reduced-motion`, `forced-colors`. Buiy foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system) requires all five OS prefs flow through the theme system; a stylesheet layer must extend `@media` to cover all of them, not just `prefers-color-scheme`.

## Sources

- belly v0.5.0 `docs/style-properties.md` — https://github.com/jkb0o/belly/blob/v0.5.0/docs/style-properties.md
- belly v0.5.0 `assets/stylesheet.ess` — https://github.com/jkb0o/belly/blob/v0.5.0/assets/stylesheet.ess
- example `selectors.rs` (pseudo-class + class usage) — https://github.com/jkb0o/belly/blob/v0.5.0/examples/selectors.rs
- example `style-sheet.rs` (asset loading) — https://github.com/jkb0o/belly/blob/v0.5.0/examples/style-sheet.rs
- belly_core source (parser + cascade) — https://github.com/jkb0o/belly/tree/v0.5.0/crates/belly_core
- bevy_flair prior-art coverage doc — [`../bevy-flair/css-coverage.md`](../bevy-flair/css-coverage.md)
- bevy_flair lessons — [`../bevy-flair/lessons.md`](../bevy-flair/lessons.md)
- Buiy foundation visuals (custom properties + value functions) — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) § 3.3
- Buiy foundation theming section — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.5
