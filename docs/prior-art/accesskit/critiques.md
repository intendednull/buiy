**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — honest critiques (half-shipped iOS/Android adapters, no web adapter, per-frame cost, one-tree-per-window constraint, cadence mismatch) and the open-problem list Buiy will inherit

This file consolidates critiques.md and open-problems.md per the brief. The first half is critiques of AccessKit *as it stands today*; the second half is the open-problem list Buiy will inherit by adopting it.

## Critiques

### 1. iOS adapter just shipped at v0.1.0 — production-readiness story is unclear

`accesskit_ios` v0.1.0 landed **2026-05-11** (11 days before this folder's date). The release notes simply read "Basic iOS adapter" ([releases page](https://github.com/AccessKit/accesskit/releases)). The semver implies pre-stable, and pre-stable means:

- Coverage of UIAccessibility protocols is incomplete (the AccessKit README explicitly says all adapters "don't yet support all types of UI elements").
- API surface may break in 0.x bumps.
- No production application is reported as shipping on the iOS adapter as of writing.

For Buiy's "iOS deferred to manual-release-gate" stance in [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md), this is the right posture: iOS is not a CI-coverable target until the adapter matures. The spec's wording should be updated from "currently in-progress upstream" (true before 2026-05-11) to "v0.1.0 just shipped, treat as alpha".

### 2. Android adapter ships but coverage is incomplete

`accesskit_android` is at v0.7.3 (2026-05-11). It is more mature than iOS but still under the upstream caveat "doesn't yet support all types of UI elements." Concrete production-app references are not pinned in this folder; recommend a follow-up sweep on next refresh. The Buiy "Android deferred" posture is correct.

### 3. No shipping web adapter

The web adapter is listed as **planned**, with no shipping crate. This is the single largest open AccessKit platform gap. Critically, the web case is **architecturally different** from the desktop / mobile case:

- On Windows / macOS / Linux / Android / iOS, AccessKit pushes a *parallel* accessibility tree to the OS. The OS already has accessibility infrastructure (UIA / NSAccessibility / AT-SPI / Android-a11y / UIAccessibility) and AccessKit feeds it.
- On the web, accessibility is **DOM-aligned ARIA** — the screen reader reads `aria-*` attributes off the actual DOM nodes, not from a parallel tree. To make AccessKit-on-web work, the consuming toolkit either renders to a real DOM with ARIA attributes (defeating the cross-platform pitch) or renders to canvas and synthesises a shadow DOM for the AT.

The Chromium-derived schema lineage means AccessKit *could* land a synthesised-shadow-DOM web adapter, but doing so is materially harder than the desktop adapters. As of 2026-05-22, the work has not visibly started. Buiy's "web deferred" posture in [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md) is correct; there is no concrete timeline.

### 4. Per-frame TreeUpdate cost in large trees

AccessKit's lazy gate (`update_if_active`) means idle windows pay nothing. But once an AT activates, the host must push a `TreeUpdate` on every accessibility-relevant change. For an immediate-mode GUI rebuilding the tree each frame (egui's model) or for a retained-mode GUI with high-frequency layout churn, the cost is:

- Building each affected `Node` (allocations for label, description, children, relations).
- Diffing against the previous tree to compute the `TreeUpdate.nodes` payload.
- IPC-equivalent cost into the platform adapter (which then crosses into UIA / NSAccessibility / AT-SPI).

The NodeClass / shared-style optimisation (an earlier AccessKit architecture revision; see [`history.md`](history.md)) reduces per-node allocation when many nodes share style. But for trees of thousands of nodes — large data grids, virtualised lists with naive non-virtualised AT exposure — the per-frame cost is non-trivial. Buiy's [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) handles this with the change-detection-driven `BuiySet::A11yUpdate` system that only walks changed entities, but the cost characterisation should be validated under load.

### 5. One-tree-per-window constraint is structural

AccessKit's API allows exactly one `accesskit_winit::Adapter` per `winit::Window`. This is not a Buiy quirk — it's the AccessKit shape. The Buiy spec accepts this and applies it as the per-window coexistence rule between Buiy and `bevy_ui` ([`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). The downside: any app that wants to mix two UI stacks *in the same window* has to write a merge coordinator. The Buiy spec explicitly defers this as an open question (`buiy-coexistence-design` would be the follow-up sub-spec).

This is architectural debt for the multi-stack-coexistence case, not a bug — it's the cost of the cross-platform abstraction.

### 6. AccessKit major-release cadence does not align with Bevy or winit

AccessKit releases on its own schedule (see cadence data in [`governance.md`](governance.md)). Bevy releases on a quarterly minor cycle. winit releases on its own cycle. The three together produce frequent dependency-misalignment windows where:

- A Bevy minor pins to a particular accesskit version.
- AccessKit ships a new major (or breaking minor) between Bevy releases.
- Buiy would be stuck on the old accesskit until either Bevy bumps or Buiy applies a patch with an unsupported accesskit-version pin.

The Buiy [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md) flags this as **the open question**: "AccessKit major release between Bevy minors triggers a Buiy patch release with a documented migration note." Not yet committed; this folder confirms the constraint is real.

### 7. Bus factor and stewardship informality

AccessKit is heavily Matt-Campbell-centric with Arnold Loubriat as the main co-maintainer on Linux/AT-SPI. Pneuma Solutions (Campbell's company) has no contractual relationship with the project — Campbell maintains it in his open-source capacity, with implicit company support (see [`governance.md`](governance.md)). If Campbell's posture toward AccessKit changed (Pneuma sale, refocus, departure), there is no formal continuity. Adopting AccessKit as a load-bearing dependency in Buiy means inheriting this concentration risk.

### 8. The "rough feature parity" caveat across adapters is not version-pinned

The README's status statement — "the current released platform adapters are all at rough feature parity. They don't yet support all types of UI elements or all of the properties in the schema" — is **not version-tagged**. There is no per-Role / per-platform support matrix in the upstream docs. Consumers cannot tell from the AccessKit docs alone "does AT-SPI honor `Role::TreeGrid` correctly today?" — they have to test against Orca and discover by experiment. This is a documentation-debt issue, not a code-quality issue, but it directly affects Buiy's verification harness scope.

### 9. Rich text / hypertext explicitly unsupported

"They don't yet support rich text or hypertext" — per the README. For Buiy, this means the cosmic-text-shaped multi-run paragraphs that the text pipeline produces are exposed to AccessKit as flat strings, with structure conveyed via tree shape only (parent paragraph → `Heading`/`Emphasis`/`Strong` child nodes). Rich-text editing controls (the `text_editor` widget catalog spec) inherit this limitation.

---

## Open problems

### O1. iOS adapter production readiness — timeline?

`accesskit_ios` v0.1.0 shipped 2026-05-11. There is no public roadmap pinning a v1.0. Buiy's "iOS deferred" stance buys time, but the open question for Buiy is **at what AccessKit iOS version does Buiy promote iOS from manual-release-gate to CI-covered platform?** Suggested guard: when at least one major adopter (egui's eframe iOS path, Slint mobile, Bevy mobile) reports production deployment + AccessKit iOS supports the Buiy widget catalog's full Role set.

### O2. Web adapter — not yet started; biggest open question

No active development visible. Buiy's "web deferred until AccessKit web adapter ships" stance is necessary but means web is **architecturally blocked** on upstream work that may take years. If Buiy ever wants web, it may need to either:

(a) Ship its own web adapter (writing a synthesised-shadow-DOM ARIA bridge) — large scope, likely outside Buiy's footprint.
(b) Render through a different abstraction on web (a Bevy-on-WebGPU stack with separate ARIA emission).
(c) Drop web as a target.

This decision deserves its own sub-spec when the Buiy web question becomes real.

### O3. Real screen-reader testing in CI

AccessKit upstream does not have public CI that exercises NVDA / VoiceOver / Orca / TalkBack utterances against test fixtures. Manual testing happens at release. The Buiy [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md) ambition to test "NVDA, JAWS, Narrator, VoiceOver (mac/iOS), Orca, TalkBack" is **harder than the upstream baseline**. Concrete approaches:

- Tree-snapshot-only verification (no AT in the loop) — what Buiy does today, validates Name / Role / Value but not utterance.
- Headless NVDA via [`nvdaTester`](https://github.com/nvaccess/nvda) or community tooling — partially possible on Windows in a VM.
- Headless Orca — feasible via D-Bus snooping (`busctl monitor org.a11y.atspi`) but verbalisation depends on speech-synthesiser version.
- VoiceOver — no headless mode; manual only.
- TalkBack — Android emulator supports TalkBack; automation is possible via UI Automator but fragile.

The honest answer: full screen-reader-in-CI is **not achievable** today across all six ATs. Buiy's verification harness must rely on tree-snapshot equivalence + a manual-release-gate AT pass.

### O4. Wayland vs X11 AT-SPI divergence

AT-SPI runs over D-Bus on both X11 and Wayland sessions, but several behaviours differ:

- Window-position queries: X11 exposes screen coordinates via XCB; Wayland intentionally does not expose absolute window positions (sandbox boundary). `accesskit_unix` has to handle the "bounds in screen space but Wayland won't tell me where the window is" case — `winit::Window::inner_position()` returns `Err` on Wayland on most desktops. Bounds reported to AT-SPI may be wrong / relative on Wayland.
- Focus-tracking: differs between session types.
- Modal dialog handling: differs across desktop environments (GNOME, KDE, Sway, Hyprland) each running its own AT-SPI registry tweaks.

Buiy must verify on **both** X11 and Wayland session types and document divergence. This is a verification-harness scope item.

### O5. AT-SPI quirks for specific roles

Specific Roles have known divergent AT-SPI behaviour:

- `combobox` — AT-SPI's combobox is implemented as a container with a dropdown popup; ARIA's `combobox` has multiple flavours (read-only vs editable, with various `aria-autocomplete` values). The mapping is not 1:1; Orca's verbalisation can mis-classify.
- `tree` — AT-SPI's tree role + `TreeItem` children may not handle deeply-nested trees with `aria-activedescendant` cleanly (known sharp edge — see next item).
- `grid` — AT-SPI has `Table` + `TableCell`; ARIA `grid` has interaction expectations (cell-level focus) that don't map cleanly.

These are platform-side quirks, not AccessKit bugs, but consumers feel them.

### O6. `aria-activedescendant` semantics on AT-SPI

`aria-activedescendant` is the "focus stays on the container, but this child is the current item" pattern (used by listbox, combobox dropdown, tree). On AT-SPI, the equivalent is fuzzy — Orca expects focus events on the active descendant in many cases. Bug reports across egui / Slint / Bevy a11y issues mention activedescendant-related verbalisation glitches on Linux. AccessKit exposes `set_active_descendant(NodeId)`; whether Orca says the right thing depends on Orca's version + the user's verbosity settings.

### O7. TalkBack vs VoiceOver divergence

The same AccessKit tree may verbalise differently on Android TalkBack vs iOS VoiceOver. Specific divergences (hedge — these need verification against current AT versions):

- TalkBack tends to read role names more eagerly than VoiceOver.
- VoiceOver iOS has rotor-based navigation that requires careful `aria-roledescription` tagging.
- Both ATs apply locale-specific phrasing that AccessKit does not control.

The Buiy verification harness should snapshot the tree, not the utterance.

### O8. AccessKit's Role-set vs ARIA's role-set

The 182-variant `Role` enum is **closed**. ARIA evolves (1.0 → 1.1 → 1.2 → 1.3 drafts) and may introduce roles AccessKit does not yet have. Recent ARIA draft additions (e.g. `mark`, `meter`, more granular widget roles) may or may not have AccessKit equivalents at any given version. The Buiy widget catalog spec must pin **which Role each Buiy widget emits** and verify that the chosen Role is in AccessKit 0.24. If ARIA 1.3 adds a Role Buiy wants, AccessKit must catch up.

### O9. Multi-window same-app a11y trees

AccessKit supports multiple adapters in one app (one per window — see [`integration.md`](integration.md)). Coordination overhead is on the host: each window has its own focus, its own NodeId space (per AccessKit's tree-per-adapter model), and the host has to route ATs across them when the user Alt-Tabs. Buiy keys per-window state by winit `WindowId` ([`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) which handles this cleanly, but the verification harness needs multi-window fixtures.

### O10. APCA / WCAG 3 — out of scope

WCAG 3 (in working-draft as of 2026) and APCA (Advanced Perceptual Contrast Algorithm, a candidate contrast formula for WCAG 3) are not AccessKit concerns — AccessKit models the tree, not the visual rendering. Buiy's contrast verification runs in the theme / verification layer ([`accessibility.md § "Visual a11y"`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)). Worth noting here only because consumers occasionally expect "the a11y crate" to handle contrast — it does not.

### O11. Schema-version compatibility across AccessKit majors

When AccessKit bumps a major version, the data schema (`Node`, `Role`, `Action` shapes) may change incompatibly. AccessKit does not currently ship a "translate old tree to new tree" shim, so the consuming toolkit must update its build-side at the same time as the platform-adapter side. For Buiy this is manageable (Buiy controls both sides) but it interacts with the cadence problem (item 6 above).

## Cross-links

- The cadence / open-question framing: [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md).
- The verification-harness scope: [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
- ACCNAME 1.2 vs AccessKit split: [`capabilities.md`](capabilities.md), [`ecosystem.md`](ecosystem.md).
- Integration mechanics that surface these limits: [`integration.md`](integration.md).

## Sources

- https://github.com/AccessKit/accesskit/blob/main/README.md
- https://github.com/AccessKit/accesskit/releases
- https://crates.io/crates/accesskit
- https://github.com/bevyengine/bevy/issues/17644
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/cross-cutting.md
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/verification.md
