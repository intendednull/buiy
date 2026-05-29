**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — unresolved questions and open problems that the integration leaves on the floor, organized by the area each problem touches in the Buiy foundation spec

# Open problems

This file enumerates structural questions `bevy_a11y` has not yet answered (or has answered partially and left work for downstream consumers). Each item names the problem, summarises what `bevy_a11y` currently does, points at the relevant evidence, and notes where the Buiy foundation spec stands on the same question. Items are deliberately framed as **problems**, not as `bevy_a11y` failures — most of these are areas where the producer-side primitive can only do so much without consumer cooperation, and where Buiy's foundation spec has to make a choice anyway.

## O1. Coordinate-space contract: producer pushes window-local or screen coords?

**The contract.** AccessKit's `Node::set_bounds` takes window-relative logical-pixel rectangles; the adapter applies the window's position + DPI scale to produce screen coordinates the AT consumes. The producer's job is **window-local-correct**. (See [`prior-art/accesskit/lessons.md`](../accesskit/lessons.md) for the canonical telling and the `integration.md` "loose phrasing" correction.)

**What `bevy_a11y` does.** Reads bevy_ui's computed layout rects (window-local logical pixels) and writes them to `set_bounds`. Correct per the contract.

**Open angle.** No documented test that the bounds-translation produces correct screen coords on Wayland (where the producer-side `winit::Window::inner_position()` returns `Err`). The contract is window-local for the producer, but the adapter's downstream translation depends on a position the OS may not surface. Failure mode: AT-reported click targets misaligned by the window's screen origin.

**Buiy stance.** Reads computed Buiy layout rects in window-local logical coords; verification harness exercises both X11 and Wayland sessions ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)). Open question in [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md).

## O2. Multi-window a11y: per-app or per-window?

**The contract.** AccessKit supports one `Adapter` per window. There is no "shared adapter across windows" mode.

**What `bevy_a11y` does.** `bevy_winit::AccessKitAdapters: EntityHashMap<Adapter>` — one adapter per window entity. The keying is by Bevy `Entity`. This is correct mechanically.

**Open angle.** `bevy_a11y` has no awareness of "this window is not mine." If an app integrates a non-Bevy a11y stack on the same process (e.g. an embedded Slint widget, a system-tray app), there's no coexistence rule on the `bevy_a11y` side. The integration assumes total ownership of every Bevy window.

**Buiy stance.** Per-window stack assignment, keyed by winit `WindowId`. Stack-ownership rule in [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md): one stack per window, fixed at window creation, no runtime switching. See [`coexistence.md`](coexistence.md).

## O3. WCAG 2.2 SC coverage — which SCs does the producer side handle?

**The contract.** WCAG 2.2 has 50+ Level A / AA Success Criteria; many are content-quality concerns (the consuming app owns), some are runtime-enforceable (focus visibility, reduced-motion honoring, contrast), some are CI-checkable (text-spacing, reflow at 320 CSS px).

**What `bevy_a11y` does.** Provides the AccessKit-tree substrate for SC 1.3.1 (Info and Relationships), 1.3.2 (Meaningful Sequence), 4.1.2 (Name, Role, Value), 4.1.3 (Status Messages) — i.e. the SCs that map directly to AT-tree shape. It does not provide focus-visible enforcement, contrast checking, text-spacing fixtures, reflow snapshots, or any of the runtime-honored / CI-checkable SCs.

**Open angle.** A Bevy app gets the AT-tree-substrate SCs "for free" and has to wire every other WCAG SC by hand. There is no Bevy-level WCAG conformance harness.

**Buiy stance.** Full WCAG 2.2 A + AA SC enumeration in [`accessibility.md` "WCAG 2.2 Success Criteria"](../../specs/2026-05-07-buiy-foundation/accessibility.md), each mapped to **CI** / **RT** / **LR** / **DC** / **OOS** strategy. The verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) enforces the CI subset.

## O4. Live regions — what's the status?

**The contract.** AccessKit models live-region politeness via `Live { Off, Polite, Assertive }` and the `is_live_atomic` / `is_busy` fields on `Node`. The producer is responsible for deciding **which mutations warrant an announcement** — `aria-relevant` (`additions` / `removals` / `text` / `all`) is not modeled by AccessKit; it has to be filtered producer-side.

**What `bevy_a11y` does.** Exposes the `Live` enum via the inner `Node`'s setter. No global announcer service. No `aria-relevant` filtering. No `role=status` / `role=alert` / `role=log` / `role=timer` widgets in the upstream catalog.

**Open angle.** Apps needing WCAG SC 4.1.3 (Status Messages, AA) have to implement the announcer service themselves. The upstream surface is field-level; the *behaviour* is missing.

**Buiy stance.** Global announcer service as a Buiy resource ([`accessibility.md` "Live regions and announcements"](../../specs/2026-05-07-buiy-foundation/accessibility.md)); full `aria-relevant` filtering producer-side; `role=status` / `role=alert` / `role=log` / `role=timer` widgets in the v1 catalog.

## O5. Real screen-reader testing in CI

**The contract.** AT utterances (what NVDA / VoiceOver / Orca actually *say*) drift across AT versions, language packs, verbosity settings. The stable signal is the AccessKit tree shape (property names + relations), not the verbalisation.

**What `bevy_a11y` does.** Unit tests for plumbing; no AccessKit-tree snapshot harness; no real-AT scripted testing in CI.

**Open angle.** When an AT change breaks a Bevy widget's accessibility, the breakage is discovered only by user reports — there's no Bevy-side gate. (The same gap exists in egui / Slint / Iced, per [`prior-art/accesskit/critiques.md`](../accesskit/critiques.md) and [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md).)

**Buiy stance.** AccessKit-tree snapshot tests in CI (the stable layer); manual-release-gate for real-AT cross-check (the drift-prone layer). Split spec'd in [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).

## O6. AT-SPI Wayland-vs-X11 differences

**The contract.** AT-SPI runs over D-Bus on Linux. X11 and Wayland session managers differ in window-position-reporting (Wayland intentionally hides absolute positions) and in some session-bus arrangements.

**What `bevy_a11y` does.** Integrates `accesskit_unix` opaquely via the meta-crate's `accesskit_unix` feature; no session-type-aware documentation or test fixtures.

**Open angle.** Behaviour gaps on Wayland are not currently surfaced as `bevy_a11y` issues; the AT-SPI bus arrangement, Orca-on-Wayland quirks, and the `winit::Window::inner_position()` `Err` case on Wayland are all real and unaddressed.

**Buiy stance.** Verification harness exercises both X11 and Wayland sessions; the divergence is documented as an open question in [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md).

## O7. Web target — when (if ever) does the adapter ship?

**The contract.** AccessKit-as-producer schema is platform-agnostic, but the **web adapter does not exist on crates.io** as of 2026-05-22. No active WIP PR visible. The web a11y model is architecturally different (DOM-aligned ARIA, not a parallel tree).

**What `bevy_a11y` does.** Compiles to WASM (the data types are platform-neutral), but the `accesskit_winit` adapter has no web backend, so the tree is built and immediately discarded. Bevy WASM apps have effectively no a11y.

**Open angle.** No timeline. The web case requires either an `accesskit_web` adapter (which would translate the tree into DOM `aria-*` attributes on a hidden mirror DOM) or a Bevy-side "render the AccessKit tree into DOM mirror" implementation. Neither exists.

**Buiy stance.** Web is manual-release-gate per [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md). Buiy does not promise web a11y in v1; the gap is documented.

## O8. AccessKit cadence vs Bevy cadence

**The contract.** AccessKit releases on its own (irregular) cadence — the recent verified rhythm is ~6 months between 0.21 → 0.22 and ~2 weeks between 0.22 → 0.23 → 0.24. Bevy minor releases land roughly quarterly. AccessKit majors often land between Bevy minors.

**What `bevy_a11y` does.** Absorbs AccessKit version bumps via `ndarilek`-authored PRs (#8655, #16234, etc.); the bumps go in with whatever Bevy minor is next. There is no policy for "AccessKit major lands mid-Bevy-cycle" — the bump waits for the next minor.

**Open angle.** Downstream Bevy apps that need an AccessKit fix shipped between Bevy minors have no path other than git-dependency overrides.

**Buiy stance.** Open question in [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md): the proposed policy is "AccessKit major release between Bevy minors triggers a Buiy patch release with a documented migration note." Not yet committed. See [`prior-art/accesskit/governance.md`](../accesskit/governance.md) for the AccessKit-side cadence picture.

## O9. ACCNAME 1.2 conformance: who computes the accessible name?

**The contract.** ACCNAME 1.2 specifies how an AT-perceived name is composed from `aria-labelledby` chains, `aria-label`, host-language label, content, and `title`, with hidden-subtree exclusion rules.

**What `bevy_a11y` does.** Does not implement the algorithm. Each widget sets a pre-computed label string on the inner `Node`. The `aria-labelledby` chain walk, content fallback, and hidden-subtree rules are not implemented in `bevy_a11y`, `bevy_ui`, or `bevy_feathers`.

**Open angle.** Apps with composite widgets where the accessible name depends on referenced sibling text (the canonical `aria-labelledby` chain case) have to walk the references themselves.

**Buiy stance.** Full algorithm in `buiy_core` ([`accessibility.md` "ACCNAME 1.2 Implementation"](../../specs/2026-05-07-buiy-foundation/accessibility.md)). This is one of the places Buiy explicitly does work `bevy_a11y` does not.

## O10. Focus restoration, focus traps, inert subtrees

**The contract.** Modal dialogs need focus traps (Tab cycles within the modal) and focus restoration on close. Inert subtrees (background content when a modal is open) need to be excluded from focus, hit-testing, and AccessKit.

**What `bevy_a11y` does.** None of the above. The focus tree is fragmented (`bevy_ui::focus`, `bevy_input_focus`, `bevy_ui::auto_directional_navigation` — see [`critiques.md`](critiques.md) §6). No focus-trap primitive in `bevy_ui` or `bevy_feathers`. No `inert` analogue.

**Open angle.** Modal-heavy Bevy apps (editor preview, productivity-style UIs) have to roll their own.

**Buiy stance.** Foundation commitment: focus traps for `Dialog` / `AlertDialog`, focus restoration on overlay close, inert subtrees ([`accessibility.md` "Focus management"](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`focus-model.md`](focus-model.md)).

## O11. `aria-activedescendant` semantics on AT-SPI

**The contract.** Composite widgets (combobox, listbox, grid, tree) often manage an "active descendant" pointed at by `aria-activedescendant` while keyboard focus stays on the container. AT-SPI's translation has historical sharp edges — some screen readers announce the active descendant, some announce the container, some need explicit focus moves.

**What `bevy_a11y` does.** AccessKit's `Node` has `active_descendant: Option<NodeId>` and the relation is preserved through the tree. No higher-level "manage the active-descendant correctly" widget pattern; each composite widget rolls its own.

**Open angle.** Bevy widgets that don't get the pattern right ship subtle Orca / NVDA inconsistencies. There's no upstream test corpus for the case.

**Buiy stance.** Foundation commits to the pattern at the focus-model level ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)); the verification harness includes Orca-specific manual-release-gate fixtures for composite widgets.

## O12. Coexistence with non-Bevy a11y stacks

**The contract.** AccessKit's per-window adapter slot is single-occupant; multiple a11y stacks cannot share a window.

**What `bevy_a11y` does.** Assumes total ownership of every Bevy window. There's no opt-out per-window.

**Open angle.** Apps with mixed UI stacks (e.g. Bevy + Slint, Bevy + native widgets) have no documented coexistence path on the `bevy_a11y` side. The single-occupant rule is a hard constraint, but the failure mode (whose adapter wins?) is not specified.

**Buiy stance.** Per-window stack assignment, fixed at window creation. Buiy suppresses `bevy_a11y` on Buiy-owned windows. See [`coexistence.md`](coexistence.md), [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).

## O13. 3D-anchored UI a11y

**The contract.** Bevy supports UI panels anchored in 3D space (billboards, on curved surfaces, render-to-texture for diegetic UI). The AccessKit tree is fundamentally 2D — `Node::set_bounds` is a 2D rectangle in window-local coordinates.

**What `bevy_a11y` does.** Nothing. There is no documented or implemented story for emitting accessible representations of 3D-anchored UI. A 3D-floating Bevy UI panel is invisible to AT.

**Open angle.** Diegetic UI (a terminal screen in a game world; a hologram interface) cannot be made accessible. The fallback is "expose a flat 2D mirror of the same widget hierarchy to AT," but neither the mirror nor the rule is implemented.

**Buiy stance.** Foundation includes 3D-anchored / diegetic UI as a deferred subsystem ([`cross-cutting.md § 3.17`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), `buiy_3d` crate). The a11y story for 3D-anchored UI is itself an open sub-question in that sub-spec; the foundation does not yet commit to a solution beyond "flat 2D mirror of the same hierarchy" as the placeholder approach.

## O14. Performance at 1000+ accessible nodes

**The contract.** Lazy activation gating amortises per-frame `TreeUpdate` cost to zero on idle windows. On AT-attached windows, the cost is "rebuild the diff against the previous frame's tree."

**What `bevy_a11y` does.** Activation gate works. Per-frame diff cost at 1000+ nodes is **not benchmarked**. The change-detection-based diff path has no published numbers.

**Open angle.** Productivity-app-sized hierarchies (tree views with 10k items, spreadsheet grids, code-editor gutters) are unmeasured territory for `bevy_a11y`. AccessKit's adapter side has similar gaps (per [`prior-art/accesskit/critiques.md`](../accesskit/critiques.md)).

**Buiy stance.** Verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) includes productivity-app fixtures at 1000+ nodes. Buiy commits to publishing the benchmark.

## O15. The decomposition completion timeline

**The contract.** PR #24308's `AccessibleLabel` is the first concrete step toward decomposing the megacomponent surface. Issue [#20524](https://github.com/bevyengine/bevy/issues/20524) is the broader follow-up but does not yet name a target Bevy version.

**What `bevy_a11y` does.** Decomposing one property at a time as PRs land. The current cadence is "one property in Bevy 0.19."

**Open angle.** At the current cadence, full decomposition across the ~30+ ARIA properties + relations + states is multiple years of work. During that interval, BSN-authoring of any non-label a11y property is broken.

**Buiy stance.** Decomposed up front: `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` ship together in `buiy_core` ([`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md)). Buiy does not wait for `bevy_a11y`'s convergence.

## Sources

- Issue [#17644](https://github.com/bevyengine/bevy/issues/17644) — BSN-incompatibility, viridia, 2025-02-02.
- PR [#24308](https://github.com/bevyengine/bevy/pull/24308) — `AccessibleLabel` partial fix, viridia, merged 2026-05-21.
- Issue [#20524](https://github.com/bevyengine/bevy/issues/20524) — broader decomposition follow-up.
- Issue [#16312](https://github.com/bevyengine/bevy/issues/16312) — feature-gating a11y in bevy_ui (Niashi24, 2024-11-09).
- `bevy_a11y` source (HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs.
- `bevy_winit::accessibility` adapter ownership: https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs.
- AccessKit folder open-problems + lessons: [`prior-art/accesskit/lessons.md`](../accesskit/lessons.md), [`prior-art/accesskit/critiques.md`](../accesskit/critiques.md), [`prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md), [`prior-art/accesskit/capabilities.md`](../accesskit/capabilities.md), [`prior-art/accesskit/governance.md`](../accesskit/governance.md).
- bevy-ui folder open-problems: [`prior-art/bevy-ui/open-problems.md`](../bevy-ui/open-problems.md), [`prior-art/bevy-ui/critiques.md`](../bevy-ui/critiques.md), [`prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md).
- Buiy foundation: [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
- Sibling files: [`distribution.md`](distribution.md), [`history.md`](history.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`comparisons.md`](comparisons.md), [`component-model-incident.md`](component-model-incident.md), [`api.md`](api.md), [`coexistence.md`](coexistence.md), [`focus-model.md`](focus-model.md).
