**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA Authoring Practices Guide (APG) — the W3C contract source Buiy implements for every widget keyboard interaction, ARIA role mapping, and accessible name and description computation

# WAI-ARIA Authoring Practices Guide (APG)

The **WAI-ARIA Authoring Practices Guide** is a W3C document maintained by the ARIA Working Group at <https://www.w3.org/WAI/ARIA/apg/>. It is **not a specification** — the normative specs are WAI-ARIA 1.2 (the role / state / property vocabulary, W3C Recommendation 6 June 2023), ACCNAME 1.2 (the accessible name and description computation, W3C Working Draft 20 May 2026), and WCAG 2.2 (the success criteria, W3C Recommendation 5 October 2023). APG is the **non-normative companion** that tells widget authors *how to combine* those primitives into the keyboard contract, ARIA emission, and name/description sourcing that screen readers and assistive technologies expect.

APG enumerates **30 widget design patterns**, each pinning a keyboard contract, an ARIA role + state + property mapping, and accessible-name + description sourcing rules. The patterns are the **lookup reference** Buiy spec authors reach for whenever they author a new widget contract — because every Buiy widget MUST conform to the relevant APG pattern, and the AccessKit tree Buiy emits MUST be readable in a way that drives the AT verbalisations APG users expect.

## Framing disclosure — this is a CONTRACT, not a lesson source

Most prior-art folders in `docs/prior-art/` document an external **system** that Buiy *learns from* — the lessons file says "validates this Buiy choice / avoid this pitfall / borrow this primitive." **APG is different.** APG is **the contract** every Buiy widget must implement. The framing inverts:

- **Validates** becomes **Implements** — every Buiy widget keyboard contract, every AccessKit role emission, every accessible-name computation MUST follow APG. The Buiy foundation spec ([accessibility.md § 3.11](../../specs/2026-05-07-buiy-foundation/accessibility.md), [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) commits to this in writing.
- **Avoid** becomes **Diverge** — where Buiy's game-engine context exceeds APG's web-platform scope (gamepad spatial navigation, in-world diegetic UI, render-to-texture surfaces), Buiy must extend the contract honestly, not pretend APG covers it. APG **does not cover** these cases.
- **Borrow** becomes **Implementation strategy** — Buiy's verification harness, ACCNAME 1.2 implementation in `buiy_core`, global announcer service, focus model, and per-widget sub-specs all derive from APG. The strategy file ([`lessons.md`](lessons.md)) documents how.

Future Buiy spec authors auditing whether AccessKit-first + APG-conformant is itself the right primitive should weigh the corpus accordingly: it's a **what Buiy must implement** artifact, not a neutral catalog of design alternatives. APG is not optional and Buiy does not get to design around it — the only authoring degrees of freedom are *which* patterns to ship and *how* to extend the contract for game-specific surfaces APG doesn't cover.

## Why APG matters to Buiy

The Buiy foundation makes **three load-bearing commitments** that anchor in APG:

1. **§ 3.11 keyboard interaction patterns** — Tab / Shift+Tab, arrow keys, Home / End / PgUp / PgDn, Enter / Space, Escape, type-ahead. Verbatim "per APG" wording. The list of widget keyboard contracts lives in [`buiy-widget-catalog-design`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md#310-widget-catalog-apg-patterns).
2. **§ 3.10 widget catalog** — every widget on the foundation roster (Button, Combobox, Dialog, Listbox, Menu, Slider, Tabs, Tree, Treegrid, ...) maps **directly** to an APG pattern. The list is APG's, not Buiy's. Buiy ships the foundation tier (F) widgets day one; core (C) and extended (E) tiers are deferred but the contract is fixed.
3. **§ 3.11 ACCNAME 1.2** — the foundation pins ACCNAME 1.2 computation in `buiy_core`, with the precedence rules `aria-labelledby > aria-label > host-language label > content > title`. This algorithm is the spec, not a Buiy invention; Buiy must conform.

The **verification harness** ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) operationalises this — gate 3 is AccessKit tree snapshots, gate 7 is the APG keyboard contract suite. Every widget in the catalog gets a fixture per APG-pattern row.

## How to use this folder

When designing a Buiy widget spec, **start with `patterns-catalog.md`** to look up the APG row for your widget. Then:

- Read `keyboard-contracts.md` for the cross-cutting keyboard conventions (Tab traversal, arrow-key conventions per widget family, type-ahead behaviour) the widget inherits.
- Read `roles-states-properties.md` for the exact ARIA vocabulary the widget emits (role, states, properties), and how each maps to AccessKit's `Role` / state / relation API.
- Read `name-computation.md` if the widget has non-trivial labelling chains (`aria-labelledby` cycles, `aria-describedby` for help text, hidden-subtree exclusion).
- Read `live-regions.md` if the widget emits announcements (alert, status, log, timer; or polite live updates).
- Read `focus-management.md` if the widget is composite (roving tabindex vs `aria-activedescendant`, focus trap, focus restoration).
- Read `wcag-22-aa-mapping.md` to confirm which Level A / AA success criteria the widget gates on; the verification harness fixture roster lives here.
- Read `platform-bindings.md` if you need to understand why an ARIA role surfaces a particular way under UIA / NSAccessibility / AT-SPI / TalkBack / VoiceOver.
- Consult `lessons.md` last — it's the **implements / diverge / implementation strategy** decision file.

## Sibling project files

The Buiy foundation specs that hard-bind to APG:

- [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) — the canonical Buiy-side commitment surface
- [`docs/specs/2026-05-07-buiy-foundation/media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) — widget catalog rooted in APG patterns
- [`docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md) — AccessKit-first
- [`docs/specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md) — APG conformance gates

Cross-link prior-art:

- [`docs/prior-art/accesskit/`](../accesskit/) — the producer-side bridge Buiy uses to surface its ARIA-shaped tree to platform AT. AccessKit's `Role` enum and node model implement ARIA-1.2 (with deliberate divergences); APG is the upstream contract AccessKit serves.
- [`docs/prior-art/bevy-a11y/`](../bevy-a11y/) — Bevy's existing a11y substrate. Cannot meet APG cleanly (megacomponent shape, no ACCNAME, sparse keyboard contracts); Buiy replaces it per-window.
- [`docs/prior-art/bevy-ui-widgets/`](../bevy-ui-widgets/) — official Bevy headless widgets. Implements partial APG contracts (Button, Checkbox, Radio, Slider, Scrollbar, Menu, Popover, EditableText) but not the full catalog or full keyboard suite.
- [`docs/prior-art/bevy-feathers/`](../bevy-feathers/) — Bevy's styled widget kit. `CHECKBOX_SIZE=18` is below WCAG 2.5.8 — the canonical case study for why Buiy ships its own widget catalog with the contract enforced from day one.

## Key facts

| Fact | Value | Source |
|---|---|---|
| APG home | <https://www.w3.org/WAI/ARIA/apg/> | W3C |
| APG patterns library | <https://www.w3.org/WAI/ARIA/apg/patterns/> | W3C |
| Pattern count | **~30** widget design patterns (see [`patterns-catalog.md`](patterns-catalog.md)) | APG patterns index |
| WAI-ARIA 1.2 spec | W3C Recommendation, **6 June 2023** | <https://www.w3.org/TR/wai-aria-1.2/> |
| WAI-ARIA 1.3 spec | W3C Working Draft (in progress) | <https://www.w3.org/TR/wai-aria-1.3/> |
| ACCNAME 1.2 spec | W3C Working Draft, **20 May 2026** | <https://www.w3.org/TR/accname-1.2/> |
| WCAG 2.2 spec | W3C Recommendation, **5 October 2023** | <https://www.w3.org/TR/WCAG22/> |
| Maintainer | W3C ARIA Working Group (under Web Accessibility Initiative, WAI) | <https://www.w3.org/WAI/ARIA/> |
| License | W3C Document License (free use, attribution, no modification) | W3C Document License |
| Companion specs | ARIA in HTML, HTML Accessibility API Mappings (HTML-AAM), Core Accessibility API Mappings (Core-AAM), Graphics ARIA, DPUB ARIA | <https://www.w3.org/WAI/standards-guidelines/aria/> |
| Implementations | Every major browser (Chrome / Edge / Firefox / Safari), every major screen reader (NVDA / JAWS / VoiceOver / TalkBack / Orca / Narrator), every OS a11y API (UIA / NSAccessibility / AT-SPI / Android `AccessibilityNodeInfo` / iOS UIAccessibility) | platform docs |
| Buiy foundation tier | F (foundation widgets), C (core), E (extended) — all bind to APG patterns | [accessibility.md § 3.11](../../specs/2026-05-07-buiy-foundation/accessibility.md) |

## Files in this folder

- **`README.md`** (this file) — overview, framing disclosure, key facts
- [`patterns-catalog.md`](patterns-catalog.md) — full catalog of the 32 APG patterns; keyboard contract summary, ARIA roles/states/properties, name+description requirements. The **lookup reference**.
- [`keyboard-contracts.md`](keyboard-contracts.md) — cross-cutting keyboard conventions: Tab traversal, arrow-key family conventions, Home/End/PgUp/PgDn, Enter/Space, Escape, type-ahead. Per-widget overrides.
- [`roles-states-properties.md`](roles-states-properties.md) — ARIA vocabulary: roles, states, properties; how AccessKit maps them.
- [`name-computation.md`](name-computation.md) — ACCNAME 1.2 algorithm: precedence rules, recursive descent, labelling chain, hidden-subtree exclusion.
- [`live-regions.md`](live-regions.md) — `aria-live`, `aria-atomic`, `aria-relevant`, `aria-busy`; how Buiy's global announcer implements this.
- [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md) — widget-implementable WCAG 2.2 success criteria; per-SC verification strategy.
- [`focus-management.md`](focus-management.md) — tab order, `:focus-visible`, focus traps, focus restoration, inert subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point.
- [`platform-bindings.md`](platform-bindings.md) — ARIA → platform a11y API mappings (UIA, NSAccessibility, AT-SPI, TalkBack, VoiceOver); gotchas.
- [`evolution-and-gaps.md`](evolution-and-gaps.md) — APG history: ARIA 1.0 → 1.1 → 1.2 → 1.3 draft; what's coming; documented gaps (game UI, gamepad, 3D-anchored, complex visualisations).
- [`lessons.md`](lessons.md) — **the decision file (inverted framing)**: implements / diverge / implementation strategy.
- [`glossary.md`](glossary.md) — ARIA, APG, ACCNAME, WCAG, role, state, property, live region, focus management, AT, etc.

## Sources

- APG home: <https://www.w3.org/WAI/ARIA/apg/>
- APG patterns library: <https://www.w3.org/WAI/ARIA/apg/patterns/>
- WAI-ARIA 1.2 Recommendation: <https://www.w3.org/TR/wai-aria-1.2/>
- WAI-ARIA 1.3 Working Draft: <https://www.w3.org/TR/wai-aria-1.3/>
- ACCNAME 1.2 Working Draft: <https://www.w3.org/TR/accname-1.2/>
- WCAG 2.2 Recommendation: <https://www.w3.org/TR/WCAG22/>
- WAI ARIA standards index: <https://www.w3.org/WAI/standards-guidelines/aria/>
- Buiy foundation accessibility spec: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation widget catalog: [`docs/specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
