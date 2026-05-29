**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — honest critiques: the megacomponent legacy, post-#24308 reality, coverage gaps, multi-window concerns, performance unknowns, Wayland/X11 divergence, the deployment gap

# Critiques

`bevy_a11y` is structurally sound on its load-bearing axes — the AccessKit-as-substrate choice is right, the activation gate is right, the per-window-entity adapter keying is right — and has shipped reliably for three years on Windows, macOS, and (with opt-in) Linux. The critiques below are about the **component-model surface** the producer-side a11y story exposes to authors, the **coverage** the integration provides today versus what WCAG 2.2 + ARIA APG demand, and the **deployment reality** behind the ~4.2M download count. This is the Buiy-relevant honest assessment, with cross-links to the specific Buiy decisions that respond to each gap.

## 1. The BSN-unfriendly megacomponent — the canonical lesson

The single most influential `bevy_a11y` critique is the one viridia framed in [issue #17644](https://github.com/bevyengine/bevy/issues/17644) (2025-02-02): `AccessibilityNode(pub Node)` is a newtype wrapper around `accesskit::Node` whose properties are reachable only through AccessKit's method-style setters (`set_disabled()` / `clear_disabled()` / `set_role(Role)` / `set_label(&str)` / …). For a code-first author who knows the API this is workable. For BSN's reflection-driven authoring model where multiple templates contribute partial property bags that get merged at instantiation, it is broken: BSN cannot patch a method-call, only a public field. The full quote from viridia (verified):

> "Because of this, I can well imagine wanting to merge together multiple BSN templates, each of which has opinions about various accessibility attributes."

This is the canonical demonstration that **the AccessKit-side API shape (a flat property bag reachable through setter methods on a single `Node`) does not survive the trip into ECS-component-model surface intact**. AccessKit's own `Node` is fine — every property is reachable independently, and the method-call shape is just AccessKit's idiom. The problem is `bevy_a11y` shipped a single ECS component wrapping the entire `Node`, which means every BSN template that wants to set any a11y property is contending for the same one-component slot.

For Buiy this is the load-bearing lesson cited in [`architecture.md § 2.4`](../../specs/2026-05-07-buiy-foundation/architecture.md) (the BSN-friendliness constraint on every Buiy component) and again in [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md) (the decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` choice). The full incident telling is in the sibling [`component-model-incident.md`](component-model-incident.md).

## 2. Post-#24308 — is the new shape actually BSN-friendly enough?

PR [#24308](https://github.com/bevyengine/bevy/pull/24308) (merged 2026-05-21, targeting Bevy 0.19) introduces `AccessibleLabel(pub String)` as a companion component that synchronises its string into the underlying `AccessibilityNode` via hooks. The PR closes issue #17644.

> *Correction vs preamble:* the preamble described #24308 as "the fix" that decomposed the megacomponent. Verified content: **#24308 is a single-property additive change, not a decomposition.** The megacomponent `AccessibilityNode(pub Node)` is unchanged.

The honest read of #24308 against the issue's framing:

- **Solves** the label-overlay case viridia named in his concrete example. A BSN template that wants to set only the label can now insert `AccessibleLabel("Save")` and BSN's merge model handles it.
- **Does not solve** any other ARIA property. Role, description, states (`disabled`, `expanded`, `selected`, `checked`, `pressed`, `busy`, …), relations (`labelled_by`, `described_by`, `controls`, …), value, live region, and the ~190 other AccessKit `Node` setters all remain in the same method-call shape. PR #24308's description acknowledges this is partial; issue [#20524](https://github.com/bevyengine/bevy/issues/20524) is the broader follow-up.
- **Implies a long migration**. If `AccessibleLabel` is the template for decomposition, full coverage is on the order of dozens of small PRs spread over several Bevy minors. At the current cadence (one property in Bevy 0.19) that's years of work.

This is the structural reason Buiy commits to its own decomposed component set on day one rather than waiting for `bevy_a11y` to converge. The convergence direction is correct; the timeline is incompatible with shipping a complete v1 widget catalog.

## 3. Coverage gaps — WAI-ARIA APG patterns

Buiy's [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) enumerates the full ARIA 1.2 role taxonomy and APG keyboard contracts. The `bevy_a11y` integration *can* express all of these because AccessKit's `Role` enum (182 variants) covers them; the question is whether Bevy ships widgets that actually emit them.

As of Bevy 0.18.1 / 0.19.0-rc.2:

- **`bevy_ui` itself** ships no APG-aligned widget catalog. The crate provides `Node`, `Button`, `Text`, `Image`, and primitive containers — not `Combobox`, `Listbox`, `Tablist`, `Treeview`, `Slider`, `Menu`, etc. with their APG keyboard contracts.
- **`bevy_ui_widgets` + `bevy_feathers`** (Bevy 0.17+) cover a meaningful subset (`Button`, `Checkbox`, `RadioGroup`, `Slider`, `TextInput`, container types) but not the full APG roster (no `Combobox`, no `Treeview`, no `Grid`, no `Menubar`).
- **Composite widget patterns** (the 9-pattern set: `combobox`, `grid`, `listbox`, `menu`, `menubar`, `radiogroup`, `tablist`, `tree`, `treegrid`) are largely unimplemented in the upstream widget catalog.

This is not a `bevy_a11y` defect per se — `bevy_a11y` is the producer-side a11y *primitive*; widget-shape coverage is `bevy_ui` / `bevy_feathers`'s responsibility. But the practical effect is that a Bevy app pulling `DefaultPlugins` and using bevy_ui gets a11y data on the widgets it has, and the widgets it has are a small subset of WAI-ARIA APG. Buiy's foundation `widget-catalog` sub-spec commits to the full APG roster as a v1 requirement.

## 4. Live region support — status

AccessKit models live regions via the `Live { Off, Polite, Assertive }` enum and `is_live_atomic` / `is_busy` fields on `Node`. `bevy_a11y` exposes these via the underlying `Node`'s setters — meaning the data structures exist, but there is no Bevy-level live-region announcer service that an app can call with "announce this string politely." `bevy_feathers` does not appear to ship one either.

The `aria-relevant` property (which mutations of a live region warrant an announcement: `additions` / `removals` / `text` / `all`) is **not modeled** by AccessKit at all; it has to be implemented producer-side. Neither `bevy_a11y` nor `bevy_feathers` implements it. Apps that need WCAG 2.2 SC 4.1.3 (Status Messages, AA) compliance have to roll their own.

Buiy's [`accessibility.md` § "Live regions and announcements"](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to a global announcer service and full `aria-relevant` filtering on the Buiy side. This is one of the load-bearing additions Buiy ships that `bevy_a11y` does not.

## 5. ACCNAME 1.2 — who computes the accessible name?

The Accessible Name and Description Computation algorithm (ACCNAME 1.2) walks the accessibility tree to compose a name from `aria-labelledby` chains, `aria-label`, host-language labels, content text, and `title` attributes, with hidden-subtree exclusion rules. The full algorithm is non-trivial.

AccessKit **does not implement ACCNAME 1.2**. It holds the references (`set_labelled_by`, `set_described_by`, `set_label`, `set_description`) and trusts the producing toolkit to compute the final name per the spec. See [`prior-art/accesskit/lessons.md`](../accesskit/lessons.md) and [`prior-art/accesskit/capabilities.md`](../accesskit/capabilities.md).

`bevy_a11y` **also does not implement ACCNAME 1.2**. There is no visible name-composition algorithm in the crate's source. In practice, Bevy widgets that emit `AccessibilityNode` set the `label` field directly on the inner `Node` to a single pre-computed string and rely on the platform AT to read what's there — which works for simple buttons with literal text labels but does not implement the full `aria-labelledby` chain walk, the content-fallback rules, or the hidden-subtree exclusion.

Buiy's commitment ([`accessibility.md` "ACCNAME 1.2 Implementation"](../../specs/2026-05-07-buiy-foundation/accessibility.md)) is full algorithm implementation in `buiy_core`. This is one of the places Buiy explicitly does work `bevy_a11y` does not.

## 6. Focus model integration with `bevy_picking` — status

The bevy_ui focus story is fragmented across `bevy_ui::focus` (mouse-driven), `bevy_input_focus` (keyboard-driven, Bevy 0.16+), and `bevy_ui::auto_directional_navigation` (spatial / gamepad, Bevy 0.18+). `bevy_a11y` itself does not own a focus tree; it consumes whatever focused entity is signalled and sets the AccessKit `Tree.focus` field per update.

This means there is no single coherent Bevy-level focus model with `:focus-visible` semantics, focus traps, focus restoration on overlay close, `inert` subtree handling, roving tabindex, or `aria-activedescendant` semantics. Each widget has to wire its own. The fragmentation is the source of much of the per-widget a11y overhead.

Buiy's commitment is a single focus tree with all of the above ([`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`focus-model.md`](focus-model.md)). The fragmentation is one of the load-bearing motivations for the parallel-stack choice.

## 7. The deployment gap — high downloads, low actual a11y use

`bevy_a11y` is at ~4.2M total downloads, with ~925k in the recent window. By every plausible measure of "Bevy apps that have shipped accessible UIs" the number of actual deployments is **at most a small handful** — feathers-driven editor preview, a few `this-week-in-bevy`-mentioned community apps, the example projects in the repository. There is no flagship Bevy commercial title verified as a real-AT-tested accessible deployment. See [`ecosystem.md`](ecosystem.md) for the disconnect's structural drivers.

The criticism is not that `bevy_a11y` is broken; it is that the download number is misleading as an adoption signal. The crate is on every Bevy app's manifest because it's a default-plugin dependency; the integration is actually exercised on a small fraction of those. Buiy should not assume `bevy_a11y`'s download volume validates the underlying design — production deployment, not download count, is the signal.

## 8. Multi-window adapter management

`bevy_winit::AccessKitAdapters` is a `RefCell<EntityHashMap<Adapter>>` thread-local: one adapter per window entity. The implementation is conceptually right (AccessKit requires per-window adapters; there's no multi-window single-adapter mode). Two concerns:

- **Keying:** The map is keyed by Bevy `Entity` (the window entity), not by winit `WindowId`. For Buiy this is mostly equivalent because Bevy creates a 1:1 mapping at window-creation time, but the Buiy spec ([`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) explicitly keys by `WindowId` because that's the AccessKit-side identifier and survives Bevy entity re-spawning if it ever happens. The Bevy choice is reasonable for a single-stack world; Buiy's choice is more defensive.
- **Coexistence with non-Bevy a11y stacks:** if a Buiy window and a bevy_ui window are in the same app, both stacks need to know which window is whose. `bevy_a11y` has no concept of "this window is not mine" — the integration assumes total ownership of every Bevy window. The Buiy `coexistence.md` spec is built around the per-window stack assignment rule; `bevy_a11y` does not need to be aware because Buiy's plugin order suppresses the `bevy_a11y` path on Buiy windows.

## 9. Performance at 1000+ accessible nodes

There are **no published benchmarks** for `bevy_a11y` at 1000+ node trees. The activation gate (`update_if_active`) ensures idle windows pay nothing, but for an active AT-attached window with a productivity-app-sized hierarchy (a tree view with 10k items expanded, a spreadsheet grid, a code editor's gutter) the per-frame `TreeUpdate` cost is unknown.

This is not unique to `bevy_a11y` — the AccessKit folder's `critiques.md` notes the same gap on the AccessKit side ([`prior-art/accesskit/critiques.md`](../accesskit/critiques.md)). The Buiy verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) commits to productivity-app fixtures at 1000+ nodes specifically because nobody else in the Bevy ecosystem has published a benchmark there.

## 10. Wayland-vs-X11 AT-SPI quirks

AT-SPI runs over D-Bus on Linux; behaviour diverges between X11 and Wayland sessions:

- **Window-position reporting:** Wayland intentionally hides absolute window positions for sandbox reasons. `winit::Window::inner_position()` returns `Err` on most Wayland desktops. The bounds reported to AT-SPI for `Node::set_bounds` are window-local logical pixels, which is the correct producer-side contract — but the adapter's translation to screen coords for AT consumption depends on a window-position the OS may not surface.
- **AT-SPI bus availability:** Wayland session managers may run AT-SPI on a different bus than X11; the `accesskit_unix` backend assumes a session bus is reachable.
- **Orca's interaction with Wayland** has historically been less polished than its interaction with X11; behaviour gaps exist that are neither AccessKit's nor `bevy_a11y`'s defect but show up at the seams.

`bevy_a11y` does not document Wayland-vs-X11 expectations; the integration assumes "AT-SPI is AT-SPI." Buiy's `architecture.md § 2.9` flags this as an open question that the verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) must exercise on both session types.

## 11. `aria-activedescendant` semantics

`aria-activedescendant` is the "active item in a composite widget" pattern (combobox, listbox, grid, tree). AccessKit's `Node` has `active_descendant: Option<NodeId>` and the relation is well-defined; AT-SPI's translation has historical sharp edges (some screen readers announce the active descendant correctly, some announce the container, some need explicit focus moves). This is mostly an AT-side problem.

`bevy_a11y` does not provide a higher-level "composite widget with active descendant" pattern — applications wiring this up have to manage it manually via the underlying `Node` setters. Buiy commits to the pattern at the focus-model level ([`focus-model.md`](focus-model.md), [`accessibility.md` § "Focus management"](../../specs/2026-05-07-buiy-foundation/accessibility.md)).

## 12. Real screen-reader testing — not in Bevy CI

`bevy_a11y` has unit tests for resource initialization and basic plumbing but does not run NVDA / VoiceOver / Orca against the AccessKit tree in CI. (This is consistent with the broader AccessKit ecosystem — Iced, egui, Slint also don't do this in CI. See [`prior-art/accesskit/critiques.md`](../accesskit/critiques.md).) Real-AT testing happens manually, when it happens at all.

Buiy's [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md) commits to AccessKit-tree snapshot tests in CI (the property-name-stable layer) and a manual-release-gate AT cross-check (the utterance-drift-prone layer). The split matches `bevy_a11y`'s practical reality but makes the manual gate explicit rather than implicit.

## Sources

- Issue [#17644](https://github.com/bevyengine/bevy/issues/17644) — BSN-unfriendly framing, viridia quote.
- PR [#24308](https://github.com/bevyengine/bevy/pull/24308) — additive `AccessibleLabel` partial fix.
- Issue [#20524](https://github.com/bevyengine/bevy/issues/20524) — broader decomposition follow-up.
- Issue [#16312](https://github.com/bevyengine/bevy/issues/16312) — `no_std` motivation for feature-gating bevy_ui a11y.
- `bevy_a11y` source (HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs.
- `bevy_winit::accessibility` (adapter ownership): https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs.
- AccessKit folder cross-references: [`prior-art/accesskit/lessons.md`](../accesskit/lessons.md), [`prior-art/accesskit/critiques.md`](../accesskit/critiques.md), [`prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md), [`prior-art/accesskit/capabilities.md`](../accesskit/capabilities.md).
- bevy-ui folder cross-references: [`prior-art/bevy-ui/critiques.md`](../bevy-ui/critiques.md), [`prior-art/bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md), [`prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md).
- Buiy foundation: [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
- Sibling files: [`history.md`](history.md), [`component-model-incident.md`](component-model-incident.md), [`api.md`](api.md), [`coexistence.md`](coexistence.md), [`focus-model.md`](focus-model.md), [`ecosystem.md`](ecosystem.md), [`open-problems.md`](open-problems.md).
