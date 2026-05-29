**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — accessibility absence; the 15-year case study for what HTML/CSS-flavored UI without an a11y story looks like

# Accessibility

**Short version: RmlUi has no accessibility story.** No AccessKit integration, no screen-reader bridge, no ARIA role / state / property vocabulary, no accessible-name / description computation, no live regions, no focus-visible distinction. The single accessibility-adjacent feature it ships is **spatial navigation for controllers** (which exists for the gameplay use case, not the assistive-technology use case). After 15+ years of cumulative libRocket + RmlUi shipping history, the situation is essentially unchanged.

This file is the **longest-running negative data point** in the corpus: it shows what an HTML/CSS-flavored open-source UI library looks like 15 years after launch if accessibility is never on the roadmap. For Buiy, which commits to AccessKit-first + WCAG 2.2 AA as **foundation tier** (foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1.2), RmlUi is the cautionary tale — the opposite stance, sustained over multiple software-generations, with predictable consequences.

## What RmlUi does NOT have

A complete enumeration of features Buiy's [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) tiers as **F** (foundation) or **C** (core) that RmlUi does not implement:

- **AT bridge**: no AccessKit, no NSAccessibility, no UIA, no AT-SPI, no TalkBack, no UIAccessibility integration. Screen readers see RmlUi UIs as **opaque windows** with no semantic content.
- **ARIA**: no `role`, no `aria-*` attribute family. No way to declare a button as `role="button"`, a tab list as `role="tablist"`, a live region as `aria-live="polite"`.
- **Accessible name / description computation (ACCNAME 1.2)**: no algorithm, no `aria-label`, no `aria-labelledby`, no `aria-describedby` chain.
- **Focus visibility**: no `:focus-visible` (only `:focus`). No focus traps. No focus restoration on dialog close. No `inert` subtree.
- **Roving tabindex pattern**: composite widget authors must implement manually; no built-in.
- **Live regions**: no polite/assertive announcement mechanism.
- **Forced colors / high-contrast mode**: no `forced-colors` media query, no system-color keywords (`Canvas`, `CanvasText`, `LinkText`, etc.), no `forced-color-adjust` opt-out.
- **`prefers-reduced-motion`**: no automatic gating of animations / transitions on the OS preference.
- **`prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-transparency`, `inverted-colors`**: no media-query support.
- **Caption / subtitle containers**: no equivalent of HTML `<track>` / WebVTT.
- **Drag-and-drop a11y replacement**: drag-and-drop is supported as a pointer interaction; there is no keyboard-replacement contract (which WCAG 2.5.7 requires).
- **Touch target size**: no enforcement of WCAG SC 2.5.8's 24×24 minimum target size; widgets render at any author-supplied size.
- **Color-contrast linting**: no contrast checker, no APCA support, no enforcement of WCAG SC 1.4.3 / 1.4.11.
- **Reading-order independence from visual order**: not enforced; flex `order` is omitted (which incidentally protects against the SC 1.3.2 violation, but that's accidental).

## The single accessibility-adjacent feature RmlUi ships

**Spatial navigation for controllers.** The libRocket era added explicit `nav-up`, `nav-down`, `nav-left`, `nav-right` attributes on RML elements, plus an auto mode that resolves the best directional candidate by visible geometry. This is **a gameplay feature** (Xbox / PlayStation controller D-pad navigation through menus) — it does not replace screen-reader access, it does not satisfy WCAG SC 2.1.1 keyboard requirement (controller D-pad is not a keyboard equivalent for assistive-technology users), and it does not provide accessible-name computation.

It is the *one* place RmlUi consistently shipped what other game UI libraries struggled with — controller nav has been part of the project since at least libRocket 1.x (~2010). The lesson is **gameplay accessibility ≠ WCAG accessibility** — a library can ship strong controller nav and still fail every screen-reader SC.

## Why this matters for Buiy

Buiy's foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1.2 commits to *"WCAG 2.2 AA is the floor. Every interactive widget ships with its APG keyboard contract, accessible name/role/value, focus management, AccessKit tree wiring. Forced-colors, reduced-motion, prefers-contrast, prefers-color-scheme are honored automatically from OS preferences."* This is the maximally-different stance from RmlUi's.

The lesson is not "RmlUi is bad" — RmlUi is a small open-source project with a single primary maintainer serving a specific game-UI-embedder need where the customer base apparently has not driven accessibility into the roadmap. The lesson is **structural**: an HTML/CSS-flavored game UI library shipping for 15+ years can stay accessibility-free if the design center doesn't include accessibility from day one. Once that's the design center, *adding* accessibility requires:

- A new tree representation (AccessKit) with its own NodeId space.
- ARIA role / state / property modeling per widget.
- ACCNAME 1.2 name computation walking the tree.
- Focus model redesign (`:focus-visible`, traps, restoration, inert).
- OS-preference plumbing (forced-colors, reduced-motion, etc.).
- Live-region announcer.
- Per-widget APG keyboard contract.
- Contrast linter.
- Verification harness (AccessKit tree snapshots, contract conformance tests).
- Per-platform AT-adapter integration (Windows UIA, macOS NSAccessibility, Linux AT-SPI, Android TalkBack, iOS UIAccessibility, web ARIA).

Every one of these is a multi-quarter project. The Bevy ecosystem's experience with `bevy_a11y` (see [`../bevy-a11y/`](../bevy-a11y/) and the `AccessibilityNode` megacomponent issue [#17644](https://github.com/bevyengine/bevy/issues/17644)) demonstrates that even when accessibility is on the roadmap, **the component-surface decomposition has to be right from the start** — retrofitting accessibility into a megacomponent that wasn't designed for it produces years of cleanup work.

## What Buiy commits to that RmlUi does not

Direct contrast against the lesson:

- **AccessKit-first** (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6): every Buiy widget has an AccessKit producer.
- **Decomposed a11y components** (`A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations`) — public-fielded, observable, BSN-authorable.
- **ACCNAME 1.2** name computation in `buiy_core`.
- **WCAG 2.2 AA conformance table** (foundation [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) tier table per SC).
- **APG widget catalog** — every widget ships its keyboard contract, accessible name/role/value, focus management.
- **Per-window AccessKit adapter ownership** (foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6).
- **Verification harness** that gates CI on AccessKit tree snapshots + APG conformance tests (foundation [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)).

## Implications for Buiy

- **The "AccessKit-first" choice is not optional.** RmlUi is the data point that demonstrates how an open-source HTML/CSS-flavored UI library looks when accessibility is left out of the foundation — it stays out. Retrofitting is years of work that may never happen. Buiy's commitment to start with accessibility is structurally load-bearing.
- **Spatial controller navigation is a game-UI feature, not an a11y feature.** Buiy ships both: spatial gamepad nav for console / game UIs *and* AccessKit-driven screen-reader access for productivity / accessibility users. Foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1.6 "game and app, both" requires both; RmlUi has only one.
- **Open-source library + no AAA-foundation funding ≠ no accessibility.** The argument that "small open-source projects can't afford accessibility" is undermined by AccessKit itself — AccessKit is open-source (Pneuma Solutions stewardship) and provides the substrate. The cost is **architectural commitment**, not money. RmlUi could integrate AccessKit; it chooses not to.
- **Cautionary tale for the foundation § 5 open question** on a CSS-flavored stylesheet. If Buiy ships a CSS-flavored stylesheet layer, the temptation will be to copy RmlUi's design (style-only, no semantic-role / a11y plumbing baked in). The lesson: any future Buiy CSS layer must compose with the a11y component surface, not replace it.

## Sources

- RmlUi project README (no a11y feature claim) — https://github.com/mikke89/RmlUi
- RmlUi documentation (no a11y section in docs sitemap) — https://mikke89.github.io/RmlUiDoc/
- RmlUi changelog (no a11y entries 2.0 → 6.2) — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- AccessKit project — https://accesskit.dev
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- ARIA Authoring Practices Guide — https://www.w3.org/WAI/ARIA/apg/
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation architecture (AccessKit-first) — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_a11y prior-art (megacomponent retrofitting case study) — [`../bevy-a11y/`](../bevy-a11y/)
- AccessKit prior-art — [`../accesskit/`](../accesskit/)
