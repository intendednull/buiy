**Date:** 2026-06-26
**Status:** active
**Subject:** Typed design-token systems — vanilla-extract, Panda CSS, Tailwind, Stripe (Sail/Apps), Style Dictionary / DTCG, cva/tailwind-variants (the typed/checked alternative to stringly-typed tokens)

# Styling attachment + theming / tokens

The animating tension across all six: **a token is a *constraint*, and a constraint is only
worth having if the toolchain enforces it.** Each subject answers "typed? checked? complete?
cascading?" differently; the spread is what's instructive for Buiy's F6 redesign. For
runtime-vs-convention see [architecture.md](./architecture.md); for the DX seam see
[composition-state-events.md](./composition-state-events.md); for decisions see
[lessons.md](./lessons.md).

## A. vanilla-extract — compiler-forced *total* themes

A *theme contract* is "a typed data-structure of CSS variables, matching the shape of the
provided theme implementation." Declare the contract once with placeholder (`null`) values;
every concrete theme must fill it. The load-bearing property is **completeness**: passing the
contract to `createTheme` means vanilla-extract "knows the type of the existing theme contract
and **requires you implement it completely and correctly**" — a theme missing a token is a
**compile-time type error**, not a runtime `undefined`. **Attach:** *wrapper + cascade* — the
theme is a generated class on a DOM ancestor; descendants inherit the scoped CSS custom
properties. Runtime/unknown values use `assignInlineVars` (typed, no runtime CSS injection).
**For F6:** the lesson is not "copy the API," it's "**make a theme a total record, not a
bag**" — and in Rust `struct Theme { brand: Color, body: FontStack, … }` *cannot* be
constructed with a field missing. vanilla-extract works hard for what Rust gives for free.

## B. Panda CSS — tiered primitive → semantic → component

Two named tiers in config, "largely influenced by the W3C Token Format":

```typescript
tokens:         { colors: { primary: { value: '#0FEE0F' } } }                 // primitives (raw)
semanticTokens: { colors: { danger:  { value: '{colors.red}' },               // references via {}
                            text:    { value: { base: '{colors.gray.900}', _dark: '{colors.gray.50}' } } } }
```

Semantic tokens carry **conditions** — the same name resolves differently by context (light/
dark) without the consumer knowing; "conditions must be an at-rule or parent-selector
condition." Panda recognizes **25+ token categories** (colors, gradients, sizes, spacing, fonts,
fontSizes/Weights, letterSpacings, lineHeights, radii, borders/Widths, shadows, easings,
opacity, zIndex, assets, durations, animations, aspectRatios, cursor). Codegen emits typed
accessors, so a typo is a TS error, not a dead CSS variable; a third **component-token** tier
scopes to a UI element. **Attach:** *style-props* whose *values are constrained to token names*
(`css({ color: 'danger' })`). **For F6:** Panda is the reference for **the tier you actually
want** — primitives are the palette; the **semantic layer is the contract the rest of the app
codes against** (`fg.default`, `bg.subtle`, `border.danger`) and the *only* layer where light/
dark/contrast logic lives. App code never names a primitive — exactly the indirection that lets
a WCAG floor be enforced in one place.

## C. Tailwind CSS v4 — constrained scales, one source of truth (`@theme`)

v4 collapses tokens into a CSS-first `@theme` block; a single declaration does *double duty* —
it is a CSS variable **and** the thing that brings a utility class into existence:

```css
@theme { --color-mint-500: oklch(0.72 0.11 178); }
/* now BOTH var(--color-mint-500) AND bg-mint-500 / text-mint-500 / fill-mint-500 exist */
```

Tokens live in **namespaces** mapping 1:1 to utility families (`--color-*`, `--font-*`,
`--text-*`, `--radius-*`, `--shadow-*`, `--breakpoint-*`, `--spacing-*`, …). **Philosophy —
"constraint is strength":** "you can usually build the bulk of a well-crafted design using a
constrained set of design tokens." The escape hatch is deliberately *ugly* and *visible*:
arbitrary values require square brackets (`bg-[#1da1f2]`, `mt-[3px]`), so going off-scale is a
syntactic flag, not a silent override. **Attach:** *atomic utility classes* (a constrained
vocabulary) + bracketed arbitrary escape. **For F6:** the value of a token system is the values
it makes *hard to express* — a small enumerated scale keeps a UI consistent. Most ECS-
transferable idea in the survey: a **Rust enum of scale steps** *is* a constrained scale, with
the escape hatch (`Px(f32)`) as a separate, visible variant.

## D. Stripe — un-overridable accessibility presets (Sail / Apps)

**Sourcing caveat:** Stripe's internal design system "Sail" is **not publicly documented**; the
verifiable, public manifestation is the **Stripe Apps UI toolkit**, which is what the lesson is
grounded in. Stripe's "Design your app" guidance states that custom styling is "intentionally
limited" to maintain platform consistency and hold a high accessibility bar — in particular
constraining the colors usable per element because contrast is load-bearing (the full verbatim
is kept once, in [open-problems.md](./open-problems.md) § "Third-party critiques"). Extensions
do not support arbitrary HTML and compose exclusively from Stripe-provided UI components; the
only styling escape is the ornamental app-indicator color, which **cannot affect any
contrast-bearing surface.** **Attach:** *no style-attach surface at
all for contrast-bearing properties* — you pick a component and a small set of safe enumerated
props. **For F6 + WCAG floor:** the purest statement that **accessibility is an *output*, not a
setting** — the exact stance Buiy takes for AccessKit. Consequence: the fg/bg *pairings* that
determine contrast should be **semantic tokens the widget owns, not raw colors the caller
patches.** If a caller can set `fg` and `bg` independently to arbitrary values, the floor is
unenforceable; expose the *intent* (`danger`, `primary`), resolve the safe pair internally.

## E. DTCG 2025.10 + Style Dictionary — interchange format as a Rust-codegen source

First **stable** version (announced 2025-10-28; 20+ editors incl. Adobe, Google, Microsoft,
Salesforce, Figma, Shopify). JSON, media type `application/design-tokens+json`, `.tokens` /
`.tokens.json`. Every token is `{ "$value", "$type", "$description?" }` with **structured**
values:

```json
{ "shadow-token": { "$type": "shadow", "$value": {
    "color": { "colorSpace": "srgb", "components": [0,0,0], "alpha": 0.5 },
    "offsetX": { "value": 0.5, "unit": "rem" }, "blur": { "value": 1.5, "unit": "rem" } } } }
```

**Closed type vocabulary** (the part that makes codegen tractable): single-value `color, dimension, fontFamily,
fontWeight, duration, cubicBezier, number`; composite `strokeStyle, border, transition, shadow,
gradient, typography`. **References** use `{group.token}` (resolves to the target's whole
`$value`, may chain, **must not be circular**). Style Dictionary (v4+, first-class DTCG) consumes
it and emits CSS/iOS/Android — or, via custom formats, *any* language. **For F6:** DTCG's `$type`
set is **almost exactly a Rust `enum TokenValue { Color(..), Dimension(..), … }`** — a
`.tokens.json` → `build.rs`/proc-macro → a generated, total Rust `Tokens` struct with reference
resolution done at codegen. The bridge between Buiy's typed internal model and the external
design-tooling ecosystem; Buiy adopts a now-stable industry vocabulary instead of inventing one.

## F. cva / tailwind-variants — the *typed, checked* alternative to stringly variants

The direct typed alternative to "a widget variant is a string." A component declares a closed set
of `variants`, `compoundVariants`, `defaultVariants`, and `VariantProps<typeof button>` extracts
the prop interface *from the config* — autocomplete on variant names, **TS error on an invalid
value**, types stay in sync with styles automatically. **tailwind-variants** (3.2.2) is the
cva-inspired successor adding **slots**, responsive variants, and built-in `tailwind-merge`.
**Attach:** *typed variant props → class string*, raw `class` passthrough as the escape. **For F6
+ F5:** these libraries exist *because* TS users wanted variants to be a checked enum, not a
freeform `className` — their reason for existence is the F6/F5 complaint. **Everything they
approximate in TS, Rust does natively and better:** `enum Intent { Primary, Secondary }` + `match`
is `VariantProps` without the `typeof`-inference machinery; `compoundVariants` is a `match
(intent, disabled)` arm; `defaultVariants` is `Default`; exhaustiveness is a *compiler guarantee*,
not a lint.

## Cross-cutting comparison

**How styling attaches:**

| Approach | Attach mechanism | Escape hatch |
|---|---|---|
| vanilla-extract | wrapper class + scoped-var **cascade** | `assignInlineVars` (typed runtime) |
| Panda | **style-props** constrained to token names | arbitrary CSS value |
| Tailwind v4 | atomic **utility classes** from a constrained scale | `[arbitrary]` bracket (visible) |
| Stripe Apps | **none** for contrast props (component + safe props only) | app-indicator color only |
| DTCG / Style Dictionary | n/a (interchange) — feeds the others | `{alias}` references |
| cva / tw-variants | **typed variant props** → class string | raw `class` passthrough |

**Theming / token model — typed? checked? complete? cascade?**

| Approach | Typed | Checked (typo = error) | Completeness enforced | Cascade |
|---|---|---|---|---|
| vanilla-extract | yes | yes (compile-time) | **yes — every theme total** | yes (CSS vars) |
| Panda | yes (codegen) | yes | tier-shaped, not total | yes (conditions) |
| Tailwind v4 | string utilities | lint / IntelliSense only | no (constrained, not total) | yes (CSS vars) |
| Stripe Apps | yes (closed component API) | yes | yes (no missing-token path) | platform |
| DTCG | yes (`$type`) | tool-dependent | no (format only) | `{alias}` |
| cva / tw-variants | yes | yes (compile-time) | variants total, values not | n/a |

The frontier Buiy should occupy is the **top-left corner of both tables for the parts it cares
about**: vanilla-extract's *completeness*, Panda's *semantic tier*, Tailwind's *constrained
scale*, Stripe's *un-overridable contrast pairs* — all of which Rust's type system enforces more
cheaply than any of these libraries can in TS/CSS.

## Sources

- vanilla-extract createThemeContract: https://vanilla-extract.style/documentation/api/create-theme-contract/ · createTheme: https://vanilla-extract.style/documentation/api/create-theme/ · theming: https://vanilla-extract.style/documentation/theming/
- Panda CSS tokens: https://panda-css.com/docs/theming/tokens · usage: https://panda-css.com/docs/theming/usage · multi-theme: https://panda-css.com/docs/guides/multiple-themes
- Tailwind theme (`@theme`): https://tailwindcss.com/docs/theme · custom styles / bracket escape: https://tailwindcss.com/docs/adding-custom-styles · v4: https://tailwindcss.com/blog/tailwindcss-v4
- Stripe Apps design: https://docs.stripe.com/stripe-apps/design · how UI extensions work: https://docs.stripe.com/stripe-apps/how-ui-extensions-work
- DTCG first stable announcement: https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/ · Format Module: https://www.designtokens.org/tr/2025.10/format/
- Style Dictionary DTCG: https://styledictionary.com/info/dtcg/
- cva variants: https://cva.style/docs/getting-started/variants · TypeScript: https://cva.style/docs/getting-started/typescript
- tailwind-variants intro: https://www.tailwind-variants.org/docs/introduction
- npm registry (versions): https://registry.npmjs.org/
