**Date:** 2026-06-26
**Status:** active
**Subject:** Typed design-token systems — vanilla-extract, Panda CSS, Tailwind, Stripe (Sail/Apps), Style Dictionary / DTCG, cva/tailwind-variants (the typed/checked alternative to stringly-typed tokens)

# Composition, state & events — the core DX

**Honest boundary up front:** none of these six are UI *frameworks*. They are styling/token
layers that live *inside* a host (React/Vue/Solid; for Style Dictionary/DTCG, any compiler).
So "composition" here means *how tokens compose into styles into components*, and "state/
events" means *the host's controlled `value`/`onChange` convention plus the one place tokens
touch state: the **variant resolver** (component state → appearance as a pure function)*. That
decoupling — **state→appearance is a total pure function, separate from state→behavior** — is
itself the transferable insight: in ECS it maps onto one system that reads state components and
writes style components, alongside a separate system driving behavior. See
[architecture.md](./architecture.md) for runtime-vs-convention; [lessons.md](./lessons.md) for
decisions.

> The **cva stagnation is itself a finding:** the canonical "typed variant" library hasn't
> shipped since 2024-11 (`0.7.1`); momentum moved to tailwind-variants (a cva-compatible
> superset) and Panda's built-in `cva`/`sva`. Don't anchor a Buiy design on cva's exact API as
> "the live standard."

## Approach 1 — Compiler-forced *total* themes: vanilla-extract

Tokens are a TS value (`vars`) consumed by `style()` calls; components apply the resulting
class names. The key primitive is `createThemeContract` — declare the *shape* with `null`
leaves, emit **no CSS**, only a typed tree of CSS-var references:

```typescript
// contract.css.ts — the vocabulary, no values
export const vars = createThemeContract({ color: { brand: null }, font: { body: null } });

// themeA.css.ts — an implementation that MUST be total
export const themeA = createTheme(vars, { color: { brand: 'blue' }, font: { body: 'arial' } });
```

The second argument is typed to the contract's exact shape, so a missing/misspelled leaf is a
**compile error** — you "never end up with declarations like `font-size: undefined`." The
**sprinkles** sub-system (`defineProperties` + `createSprinkles`) builds a finite, type-checked
atomic API — "your own zero-runtime, type-safe Tailwind"; `sprinkles({ display: 'invalid' })`
fails to compile. **State/events:** none of its own — build-time only; state→appearance is
*which class you apply*, chosen by the host from its own state.

## Approach 2 — Tiered token pipeline + typed recipes: Panda CSS

Three tiers: **primitives** (raw, each value wrapped `{ value }`), **semantic tokens** that are
*references* to primitives and vary by condition, and a **recipe** component tier:

```typescript
tokens: { colors: { red: { value: '#EE0F0F' } } }
semanticTokens: {
  colors: {
    danger: { value: '{colors.red}' },
    text:   { value: { base: '{colors.gray.900}', _dark: '{colors.gray.50}' } }, // light/dark in the token
  },
}
```

The component tier is Panda's own `cva` (atomic recipe) and `sva` (**slot recipe**) bound to
those tokens — so variants are type-checked against the vocabulary, and `sva` maps each **slot**
to a part (a button's `icon`/`label`/`loading`). **State/events:** the host's. The interesting
seam is **virtual color** (`colorPalette`): a recipe defers its concrete palette to a prop, so
the consumer's runtime state can re-point a whole token family — as close as a build-time system
gets to data-driven token binding.

## Approach 3 — Constrained utility scales: Tailwind CSS v4

v4 declares tokens in **CSS** via `@theme`; each namespaced variable *generates* its utility
classes — tokens and utilities are the same source of truth:

```css
@theme { --color-mint-500: oklch(0.72 0.11 178); } /* → bg-mint-500, text-mint-500, … + the CSS var */
```

The quotable F6 argument: *"Too many tokens create cognitive overload… When everything is
available nothing feels standard. **Constraint is strength.**"* A small named scale *is* the
vocabulary; arbitrary values (`mt-[7px]`) are conspicuous escapes. **State/events:** none
native; variant *resolution* (state→class) is delegated to cva / tailwind-variants (Approach 6),
the failure mode to avoid being the `[arbitrary]` door re-opening F6.

## Approach 4 — Un-overridable a11y presets: Stripe (Sail + Apps SDK)

The **WCAG-floor-in-the-type-system** exemplar, most relevant to Buiy's "AccessKit is a
non-negotiable output." Stripe Apps forbid arbitrary HTML/CSS; you compose only Stripe
components, and even the `Box` escape hatch accepts a `css` prop whose values are **enumerated
unions, not free strings** (verbatim from the v9 reference):

```ts
padding:        number | "xxsmall" | "xsmall" | "small" | "medium" | "large" | "xlarge" | "xxlarge"
backgroundColor:"container" | "surface"
fontWeight:     "regular" | "semibold" | "bold"
boxShadow:      "none" | "base" | "top" | "hover" | "focus"
```

These TS unions *are* the Rust-enum analogy. Rationale stated as accessibility — colors are
constrained per element because contrast is load-bearing (verbatim kept once in
[open-problems.md](./open-problems.md) § "Third-party critiques"). The `className` prop was
**removed** from `Button`/`Link`; SDK v9 adds prop validation. The scale
itself encodes the floor (Sail, Lab space: steps ≥ 500 apart guaranteed AA 4.5:1). **State/
events:** Stripe Apps *do* have a real controlled model (`value` + `onChange`); the point is the
styling layer is locked down *without* locking down the data/event layer. **Headline:** Stripe
makes the **invalid (sub-WCAG) state unrepresentable** — for Buiy this argues for theme tokens
whose *type* (enum + contrast-checked pairings) makes an inaccessible combination a compile
error, not a runtime audit finding.

## Approach 5 — Tool-agnostic token IR as codegen source: DTCG + Style Dictionary

DTCG (Format Module 2025.10, first stable) is a JSON interchange where every token is
`{$value, $type}`; groups are nodes *without* `$value`. The stable spec uses **structured**
values, not strings:

```json
{ "Button background": { "$type": "color",
    "$value": { "colorSpace": "srgb", "components": [0.467,0.467,0.467] } },
  "spacing-stack-1": { "$type": "dimension", "$value": { "value": 0.5, "unit": "rem" } } }
```

The closed `$type` set is essentially **an enum of token kinds**. Style Dictionary consumes the
JSON via **parse → transform → filter → format** and can emit *any* output via custom formats.
**State/events:** none — a build-artifact layer, upstream of any runtime. **For Buiy:** the
closed `$type` enum + structured `$value` is a language-neutral schema a build step could codegen
into a **Rust `enum Token { Color(..), Dimension(..), … }`** — a checked semantic vocabulary
without inventing the schema, with alias resolution + a contrast check at codegen time.

## Approach 6 — Typed variant resolvers (the load-bearing transfer): cva / tailwind-variants

This is the cleanest "state→appearance is a total pure function" example. `cva` declares base +
named `variants` + `compoundVariants` + `defaultVariants` and returns a resolver:

```typescript
const button = cva(["font-semibold","border","rounded"], {
  variants: {
    intent:   { primary: ["bg-blue-500","text-white"], secondary: ["bg-white","text-gray-800"] },
    size:     { small: ["text-sm","py-1","px-2"], medium: ["text-base","py-2","px-4"] },
    disabled: { false: null, true: ["opacity-50","cursor-not-allowed"] },
  },
  compoundVariants: [{ intent: "primary", size: "medium", class: "uppercase" }],
  defaultVariants: { intent: "primary", size: "medium", disabled: false },
});
export type ButtonProps = VariantProps<typeof button>; // invalid variant value = compile error
```

`tailwind-variants` is the cva-compatible superset adding **slots** (multi-part components),
responsive variants, and automatic `tailwind-merge` conflict resolution. **State/events binding
— the transfer:** the variant prop *is* the binding from component state → style, and resolution
is a *pure, total function of an enum-tuple* — no callback, no signal, no subscription. Compound
variants encode "when (state A ∧ state B) then style"; `defaultVariants` guarantees totality. For
Buiy: a `Button` whose visual state is `(Intent, Size, Disabled)` enums fed to a pure resolver
*is* an ECS system reading marker/state components and writing style components — no marker
sprawl, no tree-walk. Slots map to child entities.

## The shared host convention — and what it means for Buiy

Because all six are styling/token layers, the *actual* app-state model is the host's, and across
React/Vue/Solid that is **controlled `value` + `onChange`**: the parent owns state, passes it down
as `value`, receives changes via callback; the component renders appearance as a pure function of
`value` (the variant resolver). The token systems intersect this only at appearance, never at data
flow. The clean separation worth importing: **state→appearance** (total, pure, typed) decoupled
from **state→behavior** (the host's `onChange`). In ECS+`bsn!` that is two systems over the same
state components — one writing style, one driving behavior — with the **token enum as the shared,
compile-checked vocabulary**. That answers F2 (one untyped `OnPress`) and F6 at once: typed enums
on both the event and the style side. (See [open-problems.md](./open-problems.md): tokens are
silent on F1/F2/F5 *except* through this resolver seam.)

## Sources

- vanilla-extract createThemeContract: https://vanilla-extract.style/documentation/api/create-theme-contract/ · Sprinkles: https://vanilla-extract.style/documentation/packages/sprinkles/
- Panda CSS tokens: https://panda-css.com/docs/theming/tokens · recipes: https://panda-css.com/docs/concepts/recipes · slot recipes: https://panda-css.com/docs/concepts/slot-recipes · virtual color: https://panda-css.com/docs/concepts/virtual-color
- Tailwind theme (`@theme`): https://tailwindcss.com/docs/theme · v4 announcement: https://tailwindcss.com/blog/tailwindcss-v4
- Stripe Apps Box (enumerated `css`): https://docs.stripe.com/stripe-apps/components/box.md?app-sdk-version=9 · how UI extensions work: https://docs.stripe.com/stripe-apps/how-ui-extensions-work · accessible color systems: https://stripe.com/blog/accessible-color-systems
- DTCG first stable (2025-10-28): https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/ · Format Module 2025.10: https://www.designtokens.org/tr/2025.10/format/
- Style Dictionary DTCG / pipeline: https://styledictionary.com/info/dtcg/
- cva variants: https://cva.style/docs/getting-started/variants · TypeScript (`VariantProps`): https://cva.style/docs/getting-started/typescript
- tailwind-variants variants/slots: https://www.tailwind-variants.org/docs/variants
- npm registry (versions/dates): https://registry.npmjs.org/
