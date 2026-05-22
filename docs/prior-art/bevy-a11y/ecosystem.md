**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — ecosystem context: who actually depends on it, the download-vs-deployment disconnect, adjacent Bevy crates, comparison to other game-engine a11y stacks

# Ecosystem

The ecosystem story for `bevy_a11y` has a clean headline number — ~4,236,097 total downloads as of 2026-05-22 — and a much messier underlying reality. The download volume is high because the crate is a transitive dependency of `bevy_app`, `bevy_ui`, and `bevy_winit`, all pulled in by `DefaultPlugins`. Actual production deployment of Bevy-driven accessible UIs is rare. This file pins both numbers, explains the gap, and surveys the adjacent crates that share or compete with the same niche.

## Download volume vs actual a11y deployment

**Download volume (verified):** ~4,236,097 total, ~925,016 in the recent window. Top published versions at the time of writing: 0.18.1 (211k since 2026-03-04), 0.18.0 (190k since 2026-01-13), 0.17.3 (172k since 2025-11-17), 0.17.2 (137k since 2025-10-04). The volume scales with Bevy's overall download volume because the dependency is transitive and unavoidable for default-plugins users.

**Actual production a11y deployment:** the publicly verifiable list of Bevy-driven applications that have shipped accessible UIs — meaning a real user with a screen reader could complete the app's primary task — is **very short**. No flagship commercial Bevy game (Tiny Glade — the most cited Bevy-shipped commercial title — wrote its own UI renderer) is on that list. The Bevy editor preview itself is in-progress, and its a11y story is the one viridia and the feathers cluster are actively shaping (issue [#17644](https://github.com/bevyengine/bevy/issues/17644), PR [#24308](https://github.com/bevyengine/bevy/pull/24308)).

This is the honest disconnect: every Bevy app *ships with* `bevy_a11y`, almost none *uses* it. The reasons cluster around three structural facts:

1. **The AT-SPI backend is opt-in.** The Linux integration requires the meta-crate's `accesskit_unix` feature, which the release-notes phrase as "currently only works with experimental screen readers and forks." Default-plugins Linux apps do not engage Orca. See [`distribution.md`](distribution.md).
2. **The component-model surface is BSN-hostile** (issue #17644), which is the same set of properties that makes the integration hard to drive from author-side code. The integration "works" but is awkward enough that no widget catalog exposes it ergonomically yet outside `bevy_feathers`. See [`component-model-incident.md`](component-model-incident.md) and [`critiques.md`](critiques.md).
3. **`bevy_ui` does not yet ship a widget catalog with APG-compliant keyboard contracts.** Even a determined app author has to write the focus model, the role mapping, and the `ActionRequest` handling per widget. The widget surface to make a11y *easy* is what `bevy_feathers` + `bevy_ui_widgets` (Bevy 0.17+) are trying to provide. See [`prior-art/bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md) on the focus-model fragmentation.

Buiy's response is to bundle the whole stack: per-widget APG contracts, decomposed a11y components, focus model, picking backend, and the AccessKit adapter all in one crate set ([`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md)). The download volume of `bevy_a11y` is not a benchmark for what Buiy needs to ship.

## Adjacent crates inside the Bevy workspace

`bevy_a11y` is the producer-side primitive; several other Bevy crates touch it directly or indirectly:

- **`bevy_ui`** — depends on `bevy_a11y`. Inserts `AccessibilityNode` on widget entities; the integration mechanic was added in 0.10 and remains current. The `bevy_ui::accessibility` module is where PR #24308's `AccessibleLabel` lives (not in `bevy_a11y` itself).
- **`bevy_winit`** — owns the `AccessKitAdapters` resource (`pub struct AccessKitAdapters(pub EntityHashMap<Adapter>)`) — a thread-local `RefCell`-wrapped map from Bevy `Entity` (the window entity) to `accesskit_winit::Adapter`. This is the operative adapter ownership; `bevy_a11y` itself does not hold the adapters. See [`api.md`](api.md) for the full architecture.
- **`bevy_feathers`** (Bevy 0.17+) — tooling-focused widget set built on `bevy_ui_widgets`. Each feathers widget wires its accessible label, role, and state through the `bevy_a11y` + `bevy_ui::accessibility` API surface. This is where most production-grade Bevy a11y exercise actually lives today.
- **`bevy_ui_widgets`** (Bevy 0.17+) — headless widget primitives below `bevy_feathers`. Defines the APG-aligned widget contracts that feathers (and future Bevy widget sets) implement.
- **`bevy_input_focus`** (Bevy 0.16+) — keyboard focus tracking. Not formally part of `bevy_a11y` but the focus signal flows through to AccessKit's `Tree.focus` per-update field. The fragmentation across `bevy_ui::focus` (mouse), `bevy_input_focus` (keyboard), and `bevy_ui::auto_directional_navigation` (spatial, 0.18+) is the focus-model story the Buiy spec consolidates into a single tree.
- **`bevy_app`** — depends on `bevy_a11y` for plugin-registration scaffolding; this is why the download volume tracks Bevy's overall volume rather than the UI-using subset.

## Adjacent crates outside the Bevy workspace

A small ecosystem of community crates uses or layers over `bevy_a11y`:

- **`bevy-egui-kbgp`** — keyboard / gamepad navigation for `bevy_egui`. Re-exports `bevy_a11y` types from older Bevy versions for downstream `Focus` tracking. The crate predates Bevy 0.15's dropping of the `accesskit` re-export and is a representative example of a transitively-coupled consumer.
- **`bevy_quill_obsidian`** — UI library for the Quill reactive layer; exposes its own widgets but feeds `bevy_a11y` for AT integration.
- **`bevy_gauge`**, **`bevy_audio`**, **`bevy_android`** — depend on `bevy_a11y` transitively as part of `DefaultPlugins` consumption; not direct integrators.

None of these provide a decomposed-component layer over `bevy_a11y`'s megacomponent. The decomposition work is happening upstream piecemeal (PR #24308's `AccessibleLabel` is the first concrete step), not in a community crate.

## Buiy's parallel a11y model — direct replacement for Buiy windows

Buiy does not consume `bevy_a11y` or layer over it. The integration shape on a Buiy-owned window:

- Buiy components (`A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations`) drive an AccessKit `TreeUpdate` directly via `BuiySet::A11yUpdate`. ACCNAME 1.2 name computation lives in `buiy_core` (not pulled from AccessKit; see [`prior-art/accesskit/lessons.md`](../accesskit/lessons.md)).
- Buiy owns the `accesskit_winit::Adapter` for the window via its own per-window resource keyed by winit `WindowId` (Buiy's spec) — distinct from `bevy_winit::AccessKitAdapters` keyed by Bevy window-entity. See [`focus-model.md`](focus-model.md), [`coexistence.md`](coexistence.md).
- `bevy_a11y`'s `AccessibilityRequested` activation gate semantics are mirrored in Buiy's own gate. The Buiy gate is informed by `accesskit_winit`'s `update_if_active` callback, not by the `bevy_a11y` resource.
- On a Buiy-owned window `bevy_a11y` is **structurally suppressed**: even though the crate is in the dependency graph (transitively, unavoidable in a Bevy app), its `AccessibilityNode`-driven path is not the one writing `TreeUpdate`s for that window. This is enforceable because AccessKit's adapter slot is single-occupant per window — there is no second tree to push.

For multi-window apps where one window is bevy_ui and one is Buiy, both stacks coexist at the app level but neither stack's a11y machinery touches the other's window. See [`coexistence.md`](coexistence.md) for the full rule set.

## Comparison to other game-engine a11y stacks

| Engine | Built-in a11y | Approach | Production reach |
|---|---|---|---|
| **Bevy (`bevy_a11y`)** | Yes, since 0.10 (2023-03) | AccessKit producer; default-plugin dependency; opt-in AT-SPI on Linux | High download count, low deployment count |
| **Unity** | Yes, since 2023.2 (Unity Accessibility module) | Unity-native API; mobile-first (iOS / Android screen readers), desktop expanded in Unity 6.3 (2025+) | Many shipping mobile titles use it (Forza Customs, Microsoft Flight Simulator menus, others) |
| **Unreal** | Yes, via Slate Screen Reader plugin | Slate-coupled; supports NVDA / JAWS on Windows and VoiceOver on iOS; opt-in plugin | Used in commercial CVAA-compliance work; modest list of named titles |
| **Godot** | Yes, since 4.5 (2025-09, experimental) | AccessKit producer (the integration is bruvzg's PR [#76829](https://github.com/godotengine/godot/pull/76829), merged ~2025-Q2 into 4.5) | Just shipped; production deployment data not yet visible |
| **Construct 3** | In-progress | Browser-native ARIA via the engine's HTML5 output | Some shipping titles; depends on browser AT |
| **GameMaker** | Limited | No native screen-reader support; third-party plugins | Niche |

The two-line summary: **Bevy is structurally on par with Godot** (both AccessKit producers; both still in the "the integration exists but production deployment is sparse" phase) and **trails Unity for mobile a11y** (Unity's mobile-screen-reader integration has more shipping-game evidence). Unreal's a11y is older but Slate-coupled, meaning a non-Slate UI in Unreal (which is common) doesn't get it for free. Bevy's choice to make `bevy_a11y` a default dependency, even if widely unused, at least keeps the architectural slot occupied — Unity took until 2023 to add the module, Unreal's screen-reader plugin is opt-in. Bevy is mid-pack: not the leader, not absent.

Buiy's commitment to per-widget APG contracts, ACCNAME 1.2 in `buiy_core`, full WCAG 2.2 SC enumeration ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)), and a verification harness with AccessKit tree snapshots ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) is the choice to lead this comparison rather than stay mid-pack. The decomposed-component model is one piece of that; the verification infrastructure is the other.

## Sources

- crates.io `bevy_a11y` page: https://crates.io/crates/bevy_a11y.
- Bevy 0.10 release notes: https://bevy.org/news/bevy-0-10/.
- Bevy 0.17 release notes (feathers + a11y): https://bevy.org/news/bevy-0-17/.
- Tiny Glade UI renderer (custom): [`prior-art/bevy-ui/ecosystem.md`](../bevy-ui/ecosystem.md).
- Issue [#17644](https://github.com/bevyengine/bevy/issues/17644), PR [#24308](https://github.com/bevyengine/bevy/pull/24308), issue [#16312](https://github.com/bevyengine/bevy/issues/16312).
- Unity Accessibility module: https://docs.unity3d.com/Manual/com.unity.modules.accessibility.html, https://unity.com/blog/engine-platform/mobile-screen-reader-support-in-unity.
- Unreal Slate Screen Reader plugin: https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine.
- Godot 4.5 + AccessKit (PR #76829, bruvzg): https://github.com/godotengine/godot/pull/76829, https://godotengine.org/releases/4.5/.
- AccessKit folder ecosystem context: [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md).
- Buiy foundation accessibility commitments: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`docs/specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
- Sibling files: [`distribution.md`](distribution.md), [`history.md`](history.md), [`component-model-incident.md`](component-model-incident.md), [`api.md`](api.md), [`coexistence.md`](coexistence.md), [`focus-model.md`](focus-model.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md).
