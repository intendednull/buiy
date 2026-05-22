**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — the decision file (inverted framing): what Buiy must implement, where Buiy diverges from the web-platform contract for game-engine reasons, and the implementation strategy that operationalises both

# Lessons for Buiy (inverted framing)

This is the consult-this-when-designing decision file. The other files in this folder are evidence; this file is decisions. Unlike most prior-art `lessons.md` files (which document a system Buiy **learns from**), APG is the **contract Buiy implements**. The framing inverts:

- **Implements** replaces "Validates" — these are Buiy commitments to APG, not the other way around
- **Diverge** replaces "Avoid" — these are places where Buiy's game-engine context exceeds APG's web-platform scope, requiring honest extension rather than pretending APG covers it
- **Implementation strategy** replaces "Borrow" — concrete subsystems Buiy ships to operationalise APG

## Implements

Every Buiy widget commits to the corresponding APG pattern's contract. The Buiy foundation specs pin this in writing:

- **Every interactive widget follows the APG keyboard contract.** Tab / Shift+Tab, arrow keys within composites, Home / End / PgUp / PgDn, Enter / Space (with the asymmetries documented in [`keyboard-contracts.md`](keyboard-contracts.md)), Escape, type-ahead. The foundation [`accessibility.md § Keyboard interaction patterns`](../../specs/2026-05-07-buiy-foundation/accessibility.md) uses verbatim "per APG" wording. The verification harness gate 7 ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) is the APG keyboard contract suite — every widget has a fixture that replays the contract.
- **Every widget emits the APG-prescribed ARIA role + states + properties via AccessKit.** The mapping is enumerated in [`roles-states-properties.md`](roles-states-properties.md); AccessKit's `Role` enum and node API ([`accesskit/tree-model.md`](../accesskit/tree-model.md)) is the substrate. The verification gate 3 (AccessKit tree snapshots) captures the emission per widget.
- **ACCNAME 1.2 lives in `buiy_core`.** The algorithm — `aria-labelledby > aria-label > host-language label > content > title` — is implemented in Buiy's core crate, recursively walked per the spec, memoised per-frame. See [`name-computation.md`](name-computation.md). The foundation [`accessibility.md § Accessible Name and Description Computation`](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to this.
- **WCAG 2.2 Level A + AA SCs gated in CI.** Per the foundation [`accessibility.md § WCAG 2.2 Success Criteria`](../../specs/2026-05-07-buiy-foundation/accessibility.md) table, every widget-implementable Level A and AA SC is enforced by the verification pipeline. AAA SCs are aspirational tier-C. See [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md).
- **Live regions follow `aria-live` / `aria-atomic` / `aria-relevant` / `aria-busy`.** The Buiy global announcer ([`live-regions.md`](live-regions.md)) implements all four; `aria-relevant` filtering lives Buiy-side because AccessKit doesn't carry the property ([`accesskit/lessons.md § Borrow`](../accesskit/lessons.md)).
- **Focus model honours roving tabindex + `aria-activedescendant` + sequential-focus-navigation-starting-point.** Both composite-widget focus patterns are foundation-tier per [`focus-management.md`](focus-management.md). The starting-point action is routed via AccessKit's action plumbing.
- **`inert` analogue is foundation-tier.** Modal dialogs and overlays use `inert` to exclude the rest of the window from focus + AT + hit-testing. See [`accessibility.md § Inert / hit testing`](../../specs/2026-05-07-buiy-foundation/accessibility.md).
- **`:focus-visible` semantics.** The focus ring renders only when focus was driven by keyboard / AT, not by pointer click. Foundation-tier; theme tokens reference the state.
- **WCAG 2.4.13 focus ring quality (AAA aspirational).** ≥2 px perimeter, ≥3:1 contrast vs unfocused — Buiy's default theme ships this, exceeding 2.4.7 AA.
- **WCAG 2.5.8 target size (AA).** ≥24×24 hit targets enforced by linter; bevy_feathers's `CHECKBOX_SIZE=18` is the canonical violation Buiy doesn't reproduce.
- **WCAG 2.5.7 drag alternatives (AA).** Every drag-driven widget exposes a keyboard alternative + AccessKit action + polite live-region announcements. `aria-grabbed` / `aria-dropeffect` (deprecated in ARIA 1.2) are NOT emitted.

## Diverge

These are places where Buiy's game-engine context exceeds APG's web-platform scope. Honest extension; don't pretend APG covers them.

| Divergence | Why | Buiy extension |
|---|---|---|
| **Gamepad navigation** | APG covers keyboard only. Game engines must accept D-pad, analog stick, face buttons, bumpers, triggers. | Gamepad-to-keyboard mapping (D-pad → arrows; A → Enter; B → Esc; etc.) plus spatial navigation fallback. Per-widget Tab/arrow contracts unchanged. Documented in [`focus-management.md § Spatial focus navigation`](focus-management.md). Cross-reference [`prior-art/unreal-slate-umg/`](../unreal-slate-umg/) CommonUI, [`prior-art/rmlui/`](../rmlui/) `nav-*` annotations. |
| **Spatial focus navigation** | APG focus order is linear/sequential. Game UI needs "focusable to the right of current" semantics for non-grid layouts. | Spatial-nav scoring function (distance + angular alignment) finds nearest focusable per direction. Authors can pin overrides via `SpatialNavOverride { up: Entity, ... }`. ADDITION to Tab + arrow nav; doesn't replace. |
| **3D-anchored / diegetic UI** | APG assumes 2D screen layouts. Buiy supports widgets in 3D world space against `Transform`. | Diegetic-UI focus contained within an "interaction context" (e.g. "while reading the console"); Tab order within the context determined by spatial layout. Spec'd in `buiy_3d`; APG has no precedent. Honest gap. |
| **Render-to-texture surfaces** | Custom procedural drawing surfaces (Canvas2D analogue) have no APG pattern. | Convention: every custom visualisation MUST provide an alternative accessible structure (data table for charts; list of POIs for maps; flattened sequence for timelines) reachable via an alternative-content slot. Documented in [`media-and-widgets.md § Programmatic rendering surfaces`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md). |
| **Game-specific widgets** | HUD, inventory grid, skill tree, dialogue choice tree, quest log have no APG patterns. | Each is a composition of APG patterns + custom extensions. Per-widget contracts in `buiy-widget-catalog-design` document the composition. |
| **Multi-input concurrency** | WCAG 2.5.6 (Concurrent Input Mechanisms, AAA) — Buiy must accept gamepad + keyboard + pointer simultaneously without input-mode flicker. APG doesn't address this. | Buiy's input pipeline accepts all three concurrently; focus state is independent of input source. `:focus-visible` is set from keyboard/AT/gamepad, cleared from pointer. |
| **`marquee` role** | APG / ARIA "deprecated-leaning" but still defined. | Buiy tier E; not foundation. AccessKit supports `Role::Marquee` if needed. |
| **Per-frame announcement batching** | Web apps update sparsely; game engines update at 60+ Hz. Naive live-region updates would deafen the user. | Buiy global announcer batches polite announcements per frame; debounces redundant; honours `aria-busy` windows. See [`live-regions.md`](live-regions.md). |
| **`aria-activedescendant` on AT-SPI / Orca** | APG specifies it; AT-SPI / Orca support is uneven. | Buiy emits per spec; verification gate 3 captures tree; real-AT verification is manual-release-gate, not CI ([`platform-bindings.md`](platform-bindings.md)). |
| **iOS / Android / Web AT** | APG-conformant emission via AccessKit, but the AccessKit adapters are pre-1.0 / not-yet-shipped. | Mobile + web are manual-release-gate only in v1. Buiy emits the conformant tree; the platform AT story catches up as AccessKit ships. |

## Implementation strategy

Concrete subsystems Buiy ships to operationalise the APG contract:

1. **`buiy_core::ACCNAME` module.** The full ACCNAME 1.2 algorithm in Rust, with memoisation per-frame, cycle protection, hidden-subtree inclusion rules, trimming and whitespace normalisation. Output written to AccessKit's `Node::set_label` / `set_description` per node. See [`name-computation.md`](name-computation.md).

2. **Per-widget APG-contract specs in `buiy-widget-catalog-design`.** Every foundation, core, and extended widget has a sub-spec naming: the APG pattern handle, the emitted ARIA role + states + properties, the keyboard contract, the name source, the live-region behaviour (if any), the WCAG SC anchors. The widget catalog ([`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) is the index; the sub-spec is the per-widget contract.

3. **AccessKit tree emission via decomposed components.** `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` (the decomposed shape from [`accesskit/lessons.md § Validates`](../accesskit/lessons.md), prescribed by Bevy issue #17644) carry the ARIA-shaped data; the `BuiySet::A11yUpdate` system materialises them into `TreeUpdate`s.

4. **Verification harness gates.**
   - **Gate 3** — AccessKit tree snapshot per widget. Captures role + name + description + states + relations. Per-platform expect-fail fixtures for known upstream bugs (macOS ListBox issue #520, etc.).
   - **Gate 4** — announcement output per fixture. Captures the live-region announcements emitted.
   - **Gate 7** — APG keyboard contract suite. Synthesised keyboard sequences replayed; widget end-state verified.
   - **Linters** — contrast linter (WCAG 1.4.3 / 1.4.11), hit-target linter (WCAG 2.5.8), label-in-name linter (WCAG 2.5.3), missing-alt linter (WCAG 1.1.1).

5. **Global announcer service.** A Bevy resource accepting `Announcer::announce(Politeness::Polite, "Saved.")` plus per-widget live-region containers. Materialises announcements into AccessKit tree updates with `aria-live` / `aria-atomic` / `aria-relevant` / `aria-busy` filtering on the Buiy side. See [`live-regions.md`](live-regions.md).

6. **Focus model.** Focus tree per window with `:focus-visible` flag, focus restoration on overlay close, focus trap via `inert` for modal contexts, roving tabindex + `aria-activedescendant` patterns for composites, sequential-focus-navigation-starting-point action plumbing. Plus the Buiy extension: spatial-nav scoring for gamepad. See [`focus-management.md`](focus-management.md).

7. **Manual-release-gate AT verification.** Real-AT testing (NVDA, JAWS, Narrator, VoiceOver, Orca, TalkBack, VoiceOver iOS) is a manual release-gate, not CI. Utterances drift; the AccessKit tree shape is the testable artifact ([`accesskit/lessons.md § Avoid`](../accesskit/lessons.md): "Coupling tests / fixtures to a specific AT utterance").

8. **Per-platform expect-fail fixtures.** When a known upstream bug exists (e.g. macOS ListBox issue #520), Buiy ships a fixture that passes on affected platforms with `expect_fail` and unsets the flag when upstream ships the fix.

9. **Buiy widget catalog as the APG-pattern index for the foundation tier.** The catalog ([`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) is structured by APG pattern groups (Foundational widgets / Selection & form / Navigation / Containers & overlays / Display & feedback / Tabular data). Tier F = ship day one; tier C = post-v1; tier E = future.

10. **WCAG 2.2 SC enforcement table.** Every Level A + AA SC has an enforcement strategy (CI / RT / LR / DC); AAA SCs are aspirational. See [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md) and [`accessibility.md § WCAG 2.2 Success Criteria`](../../specs/2026-05-07-buiy-foundation/accessibility.md).

## How to use this file

When designing a Buiy widget that maps to an APG pattern:

1. Look up the pattern row in [`patterns-catalog.md`](patterns-catalog.md). That's the contract.
2. Cross-reference [`keyboard-contracts.md`](keyboard-contracts.md) for the cross-cutting keyboard conventions, and [`roles-states-properties.md`](roles-states-properties.md) for the exact ARIA emission.
3. Check this file's **Implements** rows to find the Buiy commitment that anchors the contract.
4. Check **Diverge** rows for any game-engine-specific extension your widget needs to honour (gamepad nav, spatial nav, diegetic placement).
5. Check **Implementation strategy** rows for the concrete subsystem your widget plugs into (ACCNAME, announcer, focus model, verification gates).
6. Write the per-widget spec under `buiy-widget-catalog-design`. The spec MUST cite the APG pattern row + the WCAG SC anchors + the Buiy verification gates that cover it.

## What this folder is NOT

This folder is **not** an introduction to web accessibility. The W3C site is. This folder is **not** a tutorial on writing accessible web markup. <https://www.w3.org/WAI/tutorials/> is. This folder is **not** a competitor analysis — APG is not a competitor; it's the contract source.

This folder **is** the version-pinned reference future Buiy spec authors consult when authoring per-widget APG contracts, ACCNAME implementation work, focus-model decisions, and verification fixtures.

## Sources

- APG: <https://www.w3.org/WAI/ARIA/apg/>
- ARIA 1.2: <https://www.w3.org/TR/wai-aria-1.2/>
- ACCNAME 1.2: <https://www.w3.org/TR/accname-1.2/>
- WCAG 2.2: <https://www.w3.org/TR/WCAG22/>
- Buiy foundation accessibility: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation widget catalog: [`docs/specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy foundation architecture: [`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy verification harness: [`docs/specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
- AccessKit prior-art lessons: [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`keyboard-contracts.md`](keyboard-contracts.md), [`roles-states-properties.md`](roles-states-properties.md), [`name-computation.md`](name-computation.md), [`live-regions.md`](live-regions.md), [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md), [`focus-management.md`](focus-management.md), [`platform-bindings.md`](platform-bindings.md), [`evolution-and-gaps.md`](evolution-and-gaps.md), [`glossary.md`](glossary.md)
