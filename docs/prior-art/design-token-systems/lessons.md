**Date:** 2026-06-26
**Status:** active
**Subject:** Typed design-token systems — vanilla-extract, Panda CSS, Tailwind, Stripe (Sail/Apps), Style Dictionary / DTCG, cva/tailwind-variants (the typed/checked alternative to stringly-typed tokens)

# Lessons for Buiy — the decision file

This is the file to act on. It maps the survey onto Buiy's frictions (primarily **F6**
stringly-typed unchecked theme tokens, secondarily **F3** silent-wrong footguns, **F5** one
widget many spellings, **F7** retained-mode boilerplate, **F8** verbosity, and the WCAG-floor /
AccessKit angle). Every item is tagged with the friction(s) and an **ECS+`bsn!` transferability**
rating (HIGH/MED/LOW + why). Grounding lives in [styling-theming.md](./styling-theming.md),
[composition-state-events.md](./composition-state-events.md), [architecture.md](./architecture.md);
the limits in [open-problems.md](./open-problems.md).

The net for Buiy's open question (should app STATE be separate from the a11y tree?): these
systems separate the *style* contract from data flow, and Stripe shows the a11y guarantee living
in the **token layer**, not the data layer. That argues the WCAG floor is best enforced as a
property of a **typed token vocabulary** (Rust enum + contrast-checked scale, DTCG-sourced),
independent of wherever app state lives — you don't need to fuse state and a11y to guarantee the
floor; you need the *token enum* to make sub-floor combinations unrepresentable.

## Validates — confirms Buiy's instinct to make tokens a typed, total vocabulary

**V1 — A closed token type with a totality check is the right shape for F6.** [F6, F3] —
**HIGH.** vanilla-extract's `createThemeContract` defines slot names once; every `createTheme`
must fill every slot — "all theme values must be provided or it's a type error." A Rust `Theme`
struct (all fields `: Color`/`: Px`, no `Option`) gives the identical guarantee *for free*; a
theme that forgets `surface.muted` is a build error, not a runtime fallback. `bsn!` authors
struct literals natively.

**V2 — Constraining to a closed scale prevents magic numbers and yields consistency.** [F6] —
**HIGH.** Tailwind: "with utilities, you're choosing styles from a predefined design system,
which makes it much easier to build visually consistent UIs." Tokens should be `Spacing::S4`, not
`f32`. A Rust enum is *strictly stronger* than Tailwind because an enum has no escape hatch (see
A1).

**V3 — A *layered* vocabulary (primitive → semantic → component) is the unit widgets reference.**
[F6, F7] — **MED-HIGH.** Panda separates raw primitives (`blue.500`), semantic tokens that
reference primitives and vary by condition (`bg.default`), and component tokens. Widgets must
reference the **semantic** tier (`surface.danger`), never raw hex, so a re-theme is a one-line
primitive swap. Each tier is a separate Rust newtype/enum — encodable, but the indirection adds
`bsn!` ceremony (F8).

**V4 — Semantic names are what tools/agents reason over; the raw tier is opaque to them.** [F6,
F1] — **HIGH.** "When Claude Design sees a token called `background-muted`, it knows what to reach
for in a way that 'neutral-50' never quite communicates." Buiy's agent-interface campaign wants
the app legible to LLM drivers: a typed *semantic* token enum (`Surface::Danger`) is simultaneously
the styling key and a self-describing identifier in tools/tests/agents/the AccessKit tree. A
stringly hex value is neither. Rust enum variant names are first-class identifiers.

**V5 — The WCAG floor can be made structural by removing the override knob.** [F6, F1] — **HIGH
(conceptually).** Stripe Apps is the only system here that makes accessibility non-negotiable by
design: styling is "intentionally limited" to hold a high accessibility bar, constraining the
colors usable per element because contrast is load-bearing (verbatim kept once in
[open-problems.md](./open-problems.md) § "Third-party critiques"). Validates Buiy's hard
constraint directly: if a `Surface` token *carries its own
accessible foreground* (a bonded `(bg, fg)` pair) rather than exposing independent fg/bg knobs,
the type system forbids the low-contrast combination at authoring time — accessible-by-
construction, not accessible-if-you-remember. (Note: this is *conceptual* validation; the contrast
math still has to be implemented — see OP-3.)

## Avoid — sharp edges to design out

**A1 — Do NOT ship a stringly/arbitrary escape hatch alongside the typed scale.** [F6, F3] —
**HIGH.** Tailwind's constraint is *advisory*, not enforced: `bg-[#1da1f2]`, `p-[17px]` bypass the
scale, and the cost is a textbook F3 silent-wrong — "buttons with arbitrary color values don't
move [on a re-theme], making the brand look inconsistent … even though the tokens are right." A
Rust token enum with no escape variant is strictly stronger. If Buiy ever needs raw values, gate
them behind a visibly-named `Color::raw(...)` so they're greppable, never the default path.

**A2 — "Typed token system" ≠ token *references* are typed; undefined tokens can still silently
no-op.** [F3, F6] — **HIGH.** Even Panda fails open: `color: 'primary'` only works if `primary` is
defined, and for any shape it can't statically read "Panda silently skips it and the rule never
reaches the stylesheet." That's the F3/F6 footgun in a system *marketed* as type-safe. Buiy's
mandate: token references are enum variants / typed paths, so a typo or undefined token is a
`rustc` error, never a silent no-paint. This is Buiy's natural advantage — take it.

**A3 — Typing the *selector* without typing the *vocabulary* leaves F6 unsolved.** [F6, F2] —
**MED.** cva/tailwind-variants give typed *variant props* (`size: 'sm' | 'lg'`) but the class
*bodies* they switch between are unchecked Tailwind strings. The "typed" veneer stops at the
keys. Buiy should type **both** the selector (an enum of variants) *and* the vocabulary (enum
tokens in the body). A variant DSL that types only the dial is half a solution.

**A4 — A safety layer bolted onto strings has no forcing function to evolve.** [F6] — **LOW
(governance, not a code pattern).** cva has sat at `0.7.1` since 2024-11, with `cva@1.0` in
perpetual beta, because its safety is a thin wrapper over Tailwind strings — nothing compiler-level
depends on it advancing. The caution: don't make Buiy's token correctness depend on an external,
optional, string-layered tool; bake it into the type system where the compiler keeps it honest.

## Borrow — concrete mechanisms to adapt

**B1 — DTCG JSON as the single source, Rust types as codegen output.** [F6, F8] — **MED (net-new
build infra).** The Design Tokens Format Module 2025.10 is now a *stable* vendor-neutral JSON
format (`$value`/`$type`/`$description`); Style Dictionary (5.5.0) emits *any* target via custom
formats and is DTCG-compatible. Borrow: author tokens once in DTCG JSON, codegen a Rust `Theme`
struct + semantic-token enums via `build.rs`/proc-macro. Totality is then enforced by the
generator (every theme JSON must cover the contract) *and* by `rustc` (every enum variant must be
matched); the structured-color form makes a **contrast check at codegen time** tractable, failing
the build on a bad pairing. Cost: a reference resolver + emitter + validator — likely a later
phase. Caveat (OP-1): DTCG gives the *shape*, not the *vocabulary*.

**B2 — vanilla-extract's contract/implementation split → a `ThemeContract` separate from `Theme`
instances.** [F6, F7] — **HIGH.** `createThemeContract` defines slot names with no values,
decoupled from any implementation. Borrow the separation: the *type* (which slots exist) is fixed
once; each `Theme` is a value the compiler proves total against it. This is what lets Buiy
hot-swap themes (a Bevy resource swap) with a static guarantee that every theme fills every
semantic slot — no `Option`, no fallback-to-default surprise.

**B3 — Panda's semantic-token *conditions* → resolve a semantic token as a function of mode at
access, not at authoring.** [F7, F6] — **MED.** A Panda semantic token resolves differently under
light/dark via conditions. Borrow the *late binding*: `bsn!` authors `Surface::Default` (a stable
identifier); the concrete `Color` is looked up against the active-mode `Theme` resource at
extract/paint time. Keeps mode out of the retained scene (helps F7 — no per-mode marker sprawl, no
re-spawning on theme change) and keeps the token reference legible (V4). Note the ECS binding fork
in OP-4: references are cheap (resolve once), conditions force a resolution-indirection decision —
the resolve-on-change option already ships on Buiy's substrate in [`bevy_feathers`](../bevy-feathers/)
(typed `ThemeToken`/`UiTheme` + change-detection observers), so the fork has an in-corpus precedent.

**B4 — Stripe's "bond the token to the guarantee" → `SurfaceToken` variants that fix an accessible
(bg, fg) pair.** [F6, F1] — **HIGH.** Stripe makes contrast structural by removing the per-element
color knob (V5). Borrow the data shape: expose surfaces as bonded pairs (`Surface::Danger => { bg,
on_bg }`) rather than independent `bg` and `text_color` knobs an author can mismatch. The single
most direct way for Buiy's token type to make the WCAG floor *unrepresentable to violate* rather
than merely *checkable*. Aligns with the existing §4.1c "don't single-field-patch a `#[require]`'d
component" gotcha — patching one color of a pair is the same footgun.

**B5 — Variants are enums + exhaustive `match`, not stringly-typed class soup.** [F6, F5, F2] —
**HIGH.** cva/tailwind-variants are a TS *approximation* of what Rust enums + exhaustiveness do
natively. `button("primary")`-style scene-fns should take `Intent`/`Size` enums; compound behavior
is `match (intent, state)`; defaults are `Default`. Kills F6 *and* F5 at the variant layer.
*a11y angle:* disabled/selected/pressed are *states* that must flow to AccessKit; modeling them as
typed variants (not strings) keeps the visual variant and the semantic state from drifting. This
is also an ECS system: read `(Intent, Size, Disabled)` state components → write style components,
no callbacks/signals to port, slots = child entities — a direct structural map onto ECS.

**B6 — The decoupling itself: state→appearance (typed, total, pure) separate from state→behavior.**
[F2, F6] — **HIGH.** All six keep the style contract separate from data flow. In ECS+`bsn!` that's
two systems over shared typed state components, the **token enum as the shared, compile-checked
vocabulary** between them — native to ECS, and it dissolves the untyped-`OnPress` (F2) and
stringly-token (F6) problems together.

## The one place the web model does NOT transfer

**The cascade/conditions mechanism (Panda `_dark`, vanilla-extract scoped vars).** [F7] —
**LOW (mechanism), HIGH (the warning).** CSS resolves semantic→primitive and light/dark **lazily
via the cascade / CSS custom properties** on a DOM ancestor. ECS has **no inheritance and no
cascade** — each entity holds concrete values. The cascade does not port; the *value* is knowing
upfront that token *references* are cheap (resolve once) but token *conditions* (live light/dark)
force the resolution-indirection decision in OP-4 — a two-approaches fork that belongs in a
`docs/specs` design note before coding.

## Sources

- vanilla-extract createTheme (totality) / createThemeContract: https://vanilla-extract.style/documentation/api/create-theme/ · https://vanilla-extract.style/documentation/api/create-theme-contract/
- Panda CSS tokens / multi-theme: https://panda-css.com/docs/theming/tokens · https://panda-css.com/docs/guides/multiple-themes
- FixDevs, Panda CSS Not Working (silent-skip / token-must-exist): https://fixdevs.com/blog/panda-css-not-working/
- Tailwind utility-first ("designing with constraints"): https://v3.tailwindcss.com/docs/utility-first · arbitrary-value cost: https://deslint.com/blog/tailwind-arbitrary-values
- Stripe Apps design (un-overridable a11y presets): https://docs.stripe.com/stripe-apps/design · accessible color systems: https://stripe.com/blog/accessible-color-systems
- DTCG first stable (2025-10-28): https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/ · Format Module: https://www.designtokens.org/tr/2025.10/format/
- Style Dictionary formats / DTCG: https://styledictionary.com/reference/hooks/formats/ · https://styledictionary.com/info/dtcg/
- Taste Profile, W3C DTCG practical guide (semantic names for agents): https://tasteprofile.io/blog/w3c-dtcg-design-tokens-practical-guide
- cva docs / @1.0 beta discussion: https://cva.style/docs · https://github.com/joe-bell/cva/discussions/205
- tailwind-variants intro: https://www.tailwind-variants.org/docs/introduction
- npm registry (versions): https://registry.npmjs.org/
