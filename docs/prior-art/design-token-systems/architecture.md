**Date:** 2026-06-26
**Status:** active
**Subject:** Typed design-token systems — vanilla-extract, Panda CSS, Tailwind, Stripe (Sail/Apps), Style Dictionary / DTCG, cva/tailwind-variants (the typed/checked alternative to stringly-typed tokens)

# Architecture — what each system is, runtime vs convention, distribution

Cross-cutting rule for this whole folder: separate **RUNTIME mechanism** (a JS/CSS
compiler, a class-string concatenator, an iframe renderer — *not* portable to an ECS Rust
library) from **API-surface CONVENTION** (the token taxonomy, the totality rule, the
typed-variant shape, the WCAG-floor palette — portable). Buiy can only port the convention.
See [composition-state-events.md](./composition-state-events.md) and
[styling-theming.md](./styling-theming.md) for the DX detail; [lessons.md](./lessons.md) for
the decisions.

## 1. vanilla-extract — `createThemeContract` (compiler-forced total themes)

**What it is.** A "zero-runtime stylesheets-in-TypeScript" library: you author styles in
`.css.ts` files using TS, and a build-time compiler emits static `.css` plus CSS custom
properties. The load-bearing idea: `createThemeContract({...})` declares a *contract* (a
typed nested shape of token names) once, generating CSS-variable names but **no values**;
each concrete theme implements it via `createTheme(contract, {...})`, and a theme missing any
leaf **does not type-check**.

- **RUNTIME (not portable):** the `.css.ts` compiler that statically extracts CSS and emits
  CSS custom-property declarations; the contract emits var *names* with no values so themes
  can be CSS-code-split into separate bundles.
- **CONVENTION (portable):** contract = one typed shape, **every theme implements it in
  full, partial implementation is a compile error** — exhaustiveness on themes. Maps to a
  Rust `Theme` struct / `ThemeContract` trait where a missing token cannot compile (no silent
  `Default` fallback dropping tokens — an F3 footgun as well as F6).

**Distribution.** npm, semver, monorepo of scoped packages (`/css`, `/recipes`, `/sprinkles`,
`/dynamic`). Community OSS, no foundation; corporate sponsor SEEK. `@vanilla-extract/css`
**1.21.0**, MIT, created by Mark Dalgleish + Michael Taranto at SEEK.

## 2. Panda CSS — tiered primitive → semantic → component tokens

**What it is.** A build-time, type-safe CSS-in-JS framework (spiritual successor to Chakra's
styling engine). It statically extracts styles and **generates typed token accessors** from a
`panda.config` `theme` block. Token model is explicitly tiered: `tokens` (primitives,
`colors.red.500`), `semanticTokens` (contextual, *reference* primitives and resolve per
condition, e.g. `colors.danger` → `red.500` light / `red.400` `_dark`), plus composite
`textStyles`/`layerStyles`. Format is "largely influenced by the W3C DTCG."

- **RUNTIME (not portable):** the source-scanning codegen that emits CSS (cascade `@layer`s,
  CSS vars) + a typed JS token API; conditional resolution of semantic tokens into CSS custom
  properties.
- **CONVENTION (portable):** the **three-tier taxonomy** — primitives carry raw values,
  semantic tokens carry **references (aliases)** and own meaning, composites group properties.
  Widgets bind to *semantic* tokens, not primitives. Reshapes Buiy's flat token bag (F6) into
  a `Palette` layer + a `SemanticTokens` layer that aliases into it; "semantic references a
  primitive, never a raw value" is the portable discipline.

**Distribution.** npm, semver, very high release cadence; OSS under Chakra,
no foundation. `@pandacss/dev` **1.11.3** (`engines.node` ≥ 20), MIT © Segun Adebayo.

## 3. Tailwind CSS — constrained scales ("constraint is strength")

**What it is.** A utility-first CSS framework whose token contribution is *philosophical*:
instead of arbitrary values, it exposes a **small, fixed, opinionated scale** per axis
(spacing `0,1,2,…,96`; color families `50…950`; a bounded type scale). The constraint is the
feature. v4 made the *theme itself* CSS-native (`@theme` with CSS custom properties) and
replaced the JS config + PostCSS pipeline with a Rust/Lightning-CSS engine.

- **RUNTIME (not portable):** the JIT/compiler that scans markup for utility classes and
  generates exactly the CSS used; `@theme`-to-CSS-variable emission.
- **CONVENTION (portable):** **bounded scales as the token vocabulary** — each axis is a
  finite enumerated ladder; authoring = choosing a step, not a free number. Argues for **Rust
  enums (or newtype-bounded indices)** — `Spacing::S4`, `FontSize::Lg`, color step `500` —
  instead of free `f32`/`Color`. Turns "any value, unchecked" (F6) into "a value from the
  known ladder, checked by the type system." A low-cost structural lever (a bounded enum, no
  new build infrastructure).

**Distribution.** npm + standalone CLI, semver, corporate-stewarded OSS; commercial add-on
(Tailwind UI/Catalyst) funds it. `tailwindcss` **4.3.1**, MIT, Tailwind Labs, Inc. (a company,
not a foundation). v4.0 landed Jan 22 2025; 4.3.1 ~Jun 12 2026 (day soft).

## 4. cva / tailwind-variants — the typed/checked variant API

**What it is.** Two small libraries that turn a component's style *variants* into a **typed
function** instead of ad-hoc string concatenation. `cva` declares `variant → option →
classes` maps with a typed config; calling the returned function with an invalid variant value
is a type error. `tailwind-variants` builds on the same idea, adding **slots** (multi-part
components), **compound variants**, composition, and class-conflict resolution via
`tailwind-merge`.

- **RUNTIME (not portable):** the actual work both do is **string concatenation of CSS class
  names** (cva) + `tailwind-merge` conflict resolution (tw-variants). Buiy emits no class
  strings, so this engine is irrelevant.
- **CONVENTION (portable, high value):** the **typed variant shape** — declare variant axes +
  options once, *selecting* is type-checked (`ButtonVariant::Primary`, `Size::Sm`), with
  compound variants (rules firing on a combination) + slots (one config drives several
  sub-parts). The output target differs (Buiy resolves to component bundles / token
  references, not classes) but the typed-selection API ports cleanly.

**Distribution.** Both npm, semver. cva **0.7.1**, Apache-2.0, Joe Bell (single-maintainer —
governance risk; a next-gen `cva` 1.0.0-beta sits under a separate package, perpetually beta).
tailwind-variants **3.2.2**, MIT, HeroUI org (ex-NextUI), tracks Tailwind releases.

## 5. Style Dictionary / DTCG — the typed token schema as codegen source

**What it is.** **DTCG** (W3C Design Tokens Community Group) defines a vendor-neutral **JSON
schema for tokens**: every token is `{$value (required), $type, $description?}`; the closed
`$type` set is `color, dimension, fontFamily, fontWeight, duration, cubicBezier, number,
strokeStyle, border, transition, shadow, gradient, typography`; references use `{group.token}`
aliases (must not be circular). **Style Dictionary** is the canonical *build tool* that
consumes such files and runs **parse → transform → filter → format** to emit platform outputs
(CSS vars, iOS, Android, Compose, Flutter, …).

- **RUNTIME (not portable):** Style Dictionary's transform/format pipeline (a Node build
  system) — Buiy won't run it at runtime.
- **CONVENTION (portable, structural keystone for F6):** **DTCG is a typed interchange schema,
  so it's a codegen *source*.** Buiy can treat a `.tokens.json` file as the single source of
  truth and **generate Rust enums/structs from it** (a `build.rs` / proc-macro), getting (a) a
  closed set of token names, (b) `$type`-checked values, (c) alias resolution at codegen time.
  The structured color form (not hex) makes a contrast check at codegen time tractable.

**Distribution.** DTCG: dated versioned reports (**2025.10**, first stable, announced
2025-10-28), open W3C-CG spec, no patent/royalty, 20+ cross-vendor editors. Style Dictionary
**5.5.0**, Apache-2.0, originally Amazon (Danny Banks), **co-maintained by Tokens Studio since
Aug 2023** (led v4/v5). v5 requires Node ≥ 22, so v5.0.0 is 2025-or-later (exact date
unverified).

## 6. Stripe — Sail / Apps / Elements (un-overridable a11y presets)

**What it is.** The a11y/WCAG-floor angle: ship components whose style surface is
**constrained so an inaccessible result cannot be authored.** Two public manifestations: (1)
**Stripe Apps UI toolkit** — apps build from prebuilt components with curated props (even
`Box`'s `css` prop takes **enumerated unions, not free strings**); styling is "intentionally
limited" to hold a high accessibility bar, constraining the colors usable per element because
contrast is load-bearing (full verbatim kept once in [open-problems.md](./open-problems.md)
§ "Third-party critiques"); (2) Stripe's **accessible color system** — built in CIELAB so
contrast is uniform across hues, any two scale steps ≥ 500 apart guaranteed AA (4.5:1 small
text). "Sail" is Stripe's *internal* React design system; specifics are **not publicly
verifiable**, so this entry is grounded in the public Apps + color-system sources.

- **RUNTIME (not portable):** Stripe's proprietary rendering — Elements run in **iframes**
  Stripe controls; the Apps SDK renders Stripe-owned components. None ships to third parties as
  code.
- **CONVENTION (portable, the F6 + AccessKit tie-in):** the **vocabulary itself encodes the
  WCAG floor** — the API exposes only pre-vetted constrained choices; you *can't* express an
  inaccessible pairing. For Buiy: semantic color tokens should come in **contrast-checked fg/bg
  pairs**, and the public theming API should not expose raw arbitrary colors for a11y-load-
  bearing slots. Accessibility becomes a property of the token vocabulary, not a later lint.

**Distribution.** Not distributed as a package; consumed only via Stripe's hosted SDKs
(`@stripe/ui-extension-sdk` exists on npm; SDK v9 adds component prop validation). Proprietary,
Stripe, Inc. Included purely for the un-overridable-a11y pattern.

## Sources

- vanilla-extract createThemeContract: https://vanilla-extract.style/documentation/api/create-theme-contract/
- vanilla-extract repo: https://github.com/vanilla-extract-css/vanilla-extract
- Panda CSS tokens: https://panda-css.com/docs/theming/tokens · repo: https://github.com/chakra-ui/panda
- Tailwind v4 blog: https://tailwindcss.com/blog/tailwindcss-v4 · releases: https://github.com/tailwindlabs/tailwindcss/releases
- cva docs: https://cva.style/docs · repo: https://github.com/joe-bell/cva
- tailwind-variants repo: https://github.com/heroui-inc/tailwind-variants
- DTCG 2025.10 format: https://www.designtokens.org/tr/2025.10/format/ · announcement: https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/
- Style Dictionary DTCG support: https://styledictionary.com/info/dtcg/ · v4 statement: https://styledictionary.com/versions/v4/statement/
- Stripe Apps components: https://docs.stripe.com/stripe-apps/components · design: https://docs.stripe.com/stripe-apps/design
- Stripe accessible color systems: https://stripe.com/blog/accessible-color-systems
- npm registry (versions): https://registry.npmjs.org/
