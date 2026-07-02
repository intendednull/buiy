**Date:** 2026-06-26
**Status:** active
**Subject:** Typed design-token systems — vanilla-extract, Panda CSS, Tailwind, Stripe (Sail/Apps), Style Dictionary / DTCG, cva/tailwind-variants (the typed/checked alternative to stringly-typed tokens)

# Typed design-token systems — survey overview

This is a **multi-subject survey folder** — the same topic-named (not product-named) shape
as other multi-subject surveys already in this corpus (`engine-devtools-protocols`,
`retained-mode-semantics-automation`): it does not document one product but partitions the
*typed design-token* landscape **by approach**, because each approach answers a different
slice of one Buiy friction.

The friction is **F6 — stringly-typed, unchecked theme tokens.** F6 is the gap where
Buiy currently lets a widget reference a theme color / spacing / typography token as an
unvalidated string (or a loose value) with no compile-time guarantee that the token
exists, that a theme is *complete*, or that a foreground/background pairing clears a WCAG
floor. This corpus is the **only place in Buiy's prior-art** with a *typed,
completeness-checked semantic-token vocabulary*, so it is the direct evidence base for
redesigning Buiy's token layer. It also bears secondarily on **F3 (silent-wrong
footguns)**, **F5 (one widget, many spellings)**, and Buiy's **non-negotiable AccessKit
output** (the WCAG-floor angle). The full friction catalog **F1–F8** is enumerated in
[`2026-06-25-ui-dx-composition-prior-art.md`](../../reports/2026-06-25-ui-dx-composition-prior-art.md)
§ 1 (companion to the [`2026-06-25-developer-experience-audit.md`](../../reports/2026-06-25-developer-experience-audit.md)
friction inventory).

## Key-facts table (verified 2026-06-26 via npm `registry.npmjs.org/<pkg>/latest`)

| System | Latest | License | Steward | The one portable idea it contributes to F6 |
|---|---|---|---|---|
| **vanilla-extract** (`@vanilla-extract/css`) | **1.21.0** | MIT | Mark Dalgleish / SEEK | Compiler-forced **total** themes: omitting any contract field is a *type error*. |
| **Panda CSS** (`@pandacss/dev`) | **1.11.3** | MIT | Chakra UI / Segun Adebayo | **Tiered** taxonomy: primitive → semantic → composite (textStyles/layerStyles). |
| **Tailwind CSS** (`tailwindcss`) | **4.3.1** | MIT | Tailwind Labs | **Constrained scales**: pick a step from a fixed enum ("constraint is strength"). |
| **cva** (`class-variance-authority`) | **0.7.1** | Apache-2.0 | Joe Bell (solo) | **Typed variant API**: variant selection is type-checked, not a string lookup. |
| **tailwind-variants** | **3.2.2** | MIT | HeroUI (Garcia/Pang) | cva superset: **slots** + compound variants + tw-merge. |
| **Style Dictionary** | **5.5.0** | Apache-2.0 | Amazon + Tokens Studio | A standard typed token schema usable as a **Rust-enum codegen source**. |
| **DTCG Format Module** | **2025.10** (first stable) | W3C CG open spec | W3C Design Tokens CG | `$type`/`$value`/aliases — closed token-kind enum; vendor-neutral interchange. |
| **Stripe** (Sail / Apps / Elements) | not publicly versioned | proprietary | Stripe, Inc. | **Un-overridable a11y presets**: the vocabulary *cannot express* a sub-WCAG pairing. |

Verification flags worth carrying forward: the six npm **versions are definitive** (pulled
from each `/latest` manifest, not memory). Exact **release dates** of the latest patches
are soft (GitHub tags render day/month without a year). DTCG 2025.10 = "Final Community
Group Report," **first stable version, announced 2025-10-28** (high confidence). Tailwind
v4.0 = Jan 22 2025 (verified). Stripe **Sail** internals are *not* publicly documented; the
un-overridable-a11y claim is grounded only in public Stripe Apps docs + the accessible-color
blog — treat "Sail" as background, not a verified product fact.

## Table of contents

- [architecture.md](./architecture.md) — what each system is, runtime-mechanism vs
  API-surface-convention, distribution/versioning, per-subject.
- [composition-state-events.md](./composition-state-events.md) — the core DX:
  composition/slots, state model (controlled `value`/`onChange`), the variant resolver as
  the one place tokens touch state, with real code.
- [styling-theming.md](./styling-theming.md) — how styling attaches + the theming/token
  model (typed? checked? complete? cascade?) across all six.
- [open-problems.md](./open-problems.md) — what these systems structurally do **not**
  solve (vocabulary, modes, contrast-as-correctness, ECS binding, F1/F2/F5).
- [lessons.md](./lessons.md) — **the decision file:** Validates / Avoid / Borrow, each item
  tagged with the friction (F1..F8) + ECS+`bsn!` transferability.

## How to use

These docs are written **from Buiy's stance** — an ECS-native (Bevy 0.19), retained-mode,
`bsn!`-authored, AccessKit-first Rust UI library. Every "for Buiy" note reflects that bias
*by design*: the survey reads each web/CSS system asking "which part is RUNTIME mechanism
(a JS/CSS compiler, a class-string concatenator, an iframe renderer — **not** portable) and
which part is API-surface CONVENTION (the token taxonomy, the totality rule, the
typed-variant shape, the WCAG-floor palette — portable to Rust enums + struct themes)."
Buiy can only port the convention; that split is called out per-subject. The corpus is the
F6 evidence base, *not* a recommendation to adopt any one product — start at
[lessons.md](./lessons.md) for the decisions, then drill into the facet files for the
grounding code and quotes. Honest boundary stated everywhere: none of these are UI
*frameworks*; they are styling/token layers inside a host, so they are silent on Buiy's
state-and-events questions except where the variant resolver touches appearance.

## Glossary (stub)

- **Token** — a named design decision (a color, a dimension, a font stack). The unit of the
  vocabulary.
- **Primitive / semantic / component token** — Panda's three tiers: raw value → meaning
  (`danger`, `bg.canvas`) that *references* a primitive → widget-scoped grouping.
- **Theme contract** — vanilla-extract's typed shape of token *names* with no values; a
  `createTheme` implementation must fill it **in full** or it is a type error (totality).
- **Variant resolver** — cva/tailwind-variants: a pure, total function from an enum-tuple of
  component state (`Intent`, `Size`, `Disabled`) to a style output. The one place tokens
  meet state.
- **Compound variant** — a style rule that fires only on a *combination* of variant values
  (the typed encoding of "when A ∧ B then …").
- **Slot** — one variant config that styles several sub-parts of a multi-part widget.
- **DTCG** — W3C Design Tokens Community Group; its Format Module is the `$type`/`$value`
  JSON interchange schema.
- **WCAG floor** — the question of whether the token *type* makes accessible-by-construction
  the only representable state (Buiy's hard constraint: a11y is an OUTPUT).
- **Runtime mechanism vs API-surface convention** — the non-portable engine vs the portable
  shape; the load-bearing distinction throughout this folder.

## Sources

- vanilla-extract npm: https://registry.npmjs.org/@vanilla-extract/css/latest
- Panda CSS npm: https://registry.npmjs.org/@pandacss/dev/latest
- Tailwind npm: https://registry.npmjs.org/tailwindcss/latest · v4 blog: https://tailwindcss.com/blog/tailwindcss-v4
- cva npm: https://registry.npmjs.org/class-variance-authority/latest
- tailwind-variants npm: https://registry.npmjs.org/tailwind-variants/latest
- Style Dictionary npm: https://registry.npmjs.org/style-dictionary/latest
- DTCG first-stable announcement: https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/
- DTCG 2025.10 format module: https://www.designtokens.org/tr/2025.10/format/
- Stripe accessible color systems: https://stripe.com/blog/accessible-color-systems
- Stripe Apps design guidance: https://docs.stripe.com/stripe-apps/design
