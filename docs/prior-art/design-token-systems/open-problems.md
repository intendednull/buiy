**Date:** 2026-06-26
**Status:** active
**Subject:** Typed design-token systems — vanilla-extract, Panda CSS, Tailwind, Stripe (Sail/Apps), Style Dictionary / DTCG, cva/tailwind-variants (the typed/checked alternative to stringly-typed tokens)

# Open problems — what these systems structurally do NOT solve

Stated honestly so this folder isn't over-applied. These systems answer F6 (typed,
completeness-checked tokens) well and inform F3; they are silent or weak on everything else. For
the decisions that *do* transfer see [lessons.md](./lessons.md); for the per-system grounding see
[styling-theming.md](./styling-theming.md) and [architecture.md](./architecture.md).

## OP-1 — DTCG standardizes the *format*, not the *vocabulary*

The 2025.10 module fixes the JSON shape and the type system but leaves naming, hierarchy, and
semantics to the implementer — "**Anything that isn't a token is a group**." It does **not**
define a standard semantic vocabulary (`surface.danger`, `background.muted`). Consequence for
Buiy: DTCG is a viable codegen *source* (see lessons B1) but Buiy must still design its **own**
semantic tier — the hard part (which names, what guarantees) is unsolved by the spec.

## OP-2 — DTCG's advanced layers are still unsettled

- **Modes** (light/dark/brand) have only a draft `$mode` proposal — Tokens Studio uses "sets,"
  Style Dictionary uses "themes"; no ratified mechanism.
- **Math expressions** (`space.4 * 2`) are not expressible.
- **Animation** beyond duration/easing (springs, keyframes) has no first-class types.
- **Cross-file aliasing** semantics aren't nailed down.

If Buiy leans on DTCG for theming-by-mode, it inherits an area the spec itself hasn't fixed.

## OP-3 — Completeness ≠ correctness; only Stripe makes the WCAG floor structural

vanilla-extract's totality proves every slot is *present*, not that the chosen colors *pass
contrast* — a `Theme` of all-grey-on-grey type-checks fine. Tailwind, Panda, and cva all leave
contrast as a separate lint/runtime concern. **Stripe alone** removes the knob so the floor can't
be breached. For Buiy this is the open design question behind lessons V5/B4: **presence-checking
is cheap** (free in Rust), but **value-checking** (the actual contrast ratio) needs either
bonded-pair tokens (compile-time) or a verification gate (runtime, the `buiy_verify` lane) — none
of these web systems gives Buiy that for free. The thresholds the check must enforce are **WCAG
2.2 SC 1.4.3 Contrast (Minimum)** — 4.5:1 for normal text, 3:1 for large text (≥ 18 pt, or ≥ 14 pt
bold) — and **SC 1.4.11 Non-text Contrast** — 3:1 for UI components and graphical objects (focus
rings, icons, control boundaries); both are Level AA, computed with the WCAG relative-luminance
ratio `(L1 + 0.05) / (L2 + 0.05)`.

## OP-4 — All six are web/CSS-targeted; none model ECS retained-mode token *binding*

They answer "what is the value" but not "how does a per-entity widget *subscribe* to a token and
re-paint when the theme resource swaps" — Buiy's **F7** problem. CSS gets this for free via the
cascade / custom-property indirection on a DOM ancestor; ECS has **no inheritance and no
cascade** — each entity holds concrete component values. Panda even fails the dynamic case
outright: runtime-computed values are "silently skipped" because extraction is build-time. So the
two honest options below are unaddressed by **the six web/CSS systems surveyed in this folder** —
but they are *not* unprecedented on Buiy's substrate. Option 2 is essentially what Bevy-native
prior art already ships: [`bevy_feathers`](../bevy-feathers/) resolves typed `ThemeToken` /
`UiTheme` / `ThemeBackgroundColor` → concrete color via change-detection observers (the companion
[UI-DX prior-art report](../../reports/2026-06-25-ui-dx-composition-prior-art.md) names it the
on-substrate *reference implementation*; its limits are a flat `SCREAMING_SNAKE` string namespace
and color-only scope), and [`bevy_flair`](../bevy-flair/) does the CSS-stylesheet variant. The
open question for Buiy is therefore *which* option to adopt, not whether token binding can be done
in ECS at all:

1. **Resolve at spawn / theme-build time** to concrete `Color`s — simple, but a runtime theme
   switch must re-resolve and re-write every themed entity.
2. **Keep a `TokenRef(SemanticToken)` component** resolved by a system each frame / on-change —
   an explicit analogue of CSS `var()`; enables live theme switching at the cost of a resolution
   pass + marker components (the F7 retained-mode tax).

This fork (token *references* are cheap — resolve once; token *conditions*, i.e. live light/dark,
force a resolution-indirection decision) is exactly the kind of two-approaches choice that belongs
in a `docs/specs` design note before coding.

## OP-5 — Tokens are orthogonal to Buiy's F1 / F2 / F5; expect no help there

None of these systems touch the app-state-vs-a11y-tree question (**F1**), typed change events /
binding (**F2**, except through the variant-resolver seam in
[composition-state-events.md](./composition-state-events.md)), or widget-spelling unification
(**F5**, except at the variant layer). Tokens are pure styling vocabulary: they cleanly answer F6
(and inform F3) and are silent on the rest. A useful boundary to state explicitly.

## OP-6 — Verbosity (F8) is not free

Panda's three-tier indirection and vanilla-extract's contract-then-implement both add ceremony.
cva exists *because* raw variant-switching is verbose — yet it adds its own DSL surface. Borrowing
the tiering (lessons V3/B1) buys F6 safety at an **F8 cost**; codegen from DTCG (B1) is the lever
that pays it back. Without codegen, a hand-maintained total theme is real boilerplate.

## Governance / liveness risks (not "open problems" of the design, but of relying on the tools)

- **cva is in maintenance stasis** — `0.7.1` since 2024-11-26; `cva@1.0` perpetually in beta. Its
  safety is a thin wrapper over Tailwind strings, so nothing compiler-level forces it to advance.
  Don't make Buiy's token correctness depend on an external, optional, string-layered tool — bake
  it into the type system where the compiler keeps it honest.
- **Stripe Sail is closed and unversioned** — the portable artifact is the *pattern* (un-
  overridable a11y presets), not any consumable code.
- **DTCG day-level dates and some patch dates are soft** — versions are registry-verified; exact
  release days are not (GitHub tags omit the year). Re-check before citing a date as fact.

## Third-party critiques (verbatim)

> "If the object shape can't be statically read (dynamic keys, computed values, spread from a
> function), Panda silently skips it and the rule never reaches the stylesheet."
> — FixDevs, *Panda CSS Not Working* — https://fixdevs.com/blog/panda-css-not-working/

> "When brand teams shift primary colors, the tokens update and cascade, but buttons with
> arbitrary color values don't move, making the brand look inconsistent across the product even
> though the tokens are right and the design is right."
> — Deslint, *The hidden cost of Tailwind arbitrary values* — https://deslint.com/blog/tailwind-arbitrary-values

> "Custom styling of UI elements is intentionally limited … to ensure a high accessibility bar.
> In particular, we limit the colors you can use for each element because color contrast is an
> important aspect of accessible UI." — Stripe Apps design — https://docs.stripe.com/stripe-apps/design

> "Anything that isn't a token is a group." … "when Claude Design sees a token called
> `background-muted`, it knows what to reach for in a way that 'neutral-50' never quite
> communicates." — Taste Profile — https://tasteprofile.io/blog/w3c-dtcg-design-tokens-practical-guide

Verification caveat: a *This Dot Labs* blog characterizes Stripe Apps as "you will never touch
HTML or CSS directly — in fact, you're forbidden to"; seen only in a search summary, not a direct
fetch — treat the exact wording as unverified. The directly-fetched Stripe-docs quote above is the
reliable one.

## Sources

- DTCG drafts index: https://www.designtokens.org/tr/drafts/ · Format Module 2025.10: https://www.designtokens.org/tr/2025.10/format/
- Taste Profile, W3C DTCG practical guide: https://tasteprofile.io/blog/w3c-dtcg-design-tokens-practical-guide
- FixDevs, Panda CSS Not Working: https://fixdevs.com/blog/panda-css-not-working/
- Deslint, Tailwind arbitrary values: https://deslint.com/blog/tailwind-arbitrary-values
- Stripe Apps design: https://docs.stripe.com/stripe-apps/design
- vanilla-extract createTheme (totality): https://vanilla-extract.style/documentation/api/create-theme/
- cva@1.0 beta discussion #205: https://github.com/joe-bell/cva/discussions/205
- npm registry (versions): https://registry.npmjs.org/
