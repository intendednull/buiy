**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — side-by-side comparisons with Buiy's planned model and with peer AccessKit producers (egui, Slint, Freya, Xilem/Masonry, Godot) plus the non-AccessKit game-engine a11y stacks (Unity, Unreal)

# Comparisons

This file does direct side-by-sides. Each row identifies how the other toolkit / engine wires accessibility, the key design difference vs `bevy_a11y`, and (where relevant) what Buiy borrows or rejects from that comparison. The honest finding: `bevy_a11y` is on the AccessKit-producer side of the divide with several Rust peers, and its main differentiator within that group is **the megacomponent surface** — the design choice that issue [#17644](https://github.com/bevyengine/bevy/issues/17644) named as the BSN-incompatibility.

For the AccessKit-side comparison-matrix detail, see [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md) and [`prior-art/accesskit/comparisons.md`](../accesskit/comparisons.md). This file adds the producer-side ergonomics layer.

## vs Buiy's planned a11y model

| Axis | `bevy_a11y` (0.18.1 / 0.19.0-rc.2) | Buiy (planned, v1) |
|---|---|---|
| Component surface | Single `AccessibilityNode(pub Node)` newtype around `accesskit::Node`. Properties via AccessKit method-style setters. Post-#24308: `AccessibleLabel(pub String)` companion for the label property only. | Decomposed: `A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations` — each a small public-fielded `Component` with `Reflect + FromReflect + Default + Clone`. BSN-overlayable on every property. |
| Tree update path | `bevy_ui` widgets mutate the inner `Node`; `bevy_a11y` plumbs the updates into `accesskit_winit` via `bevy_winit::accessibility`. | Buiy components → `BuiySet::A11yUpdate` system → `TreeUpdate` builder → `accesskit_winit::Adapter::update_if_active`. No `bevy_a11y` in the path. |
| Adapter ownership | `bevy_winit::AccessKitAdapters: EntityHashMap<Adapter>` (one per window entity). Thread-local `RefCell`. | Per-window resource keyed by winit `WindowId` (more defensive against Bevy entity churn). Owned by Buiy on Buiy windows; `bevy_a11y` is suppressed for those windows. |
| ACCNAME 1.2 | Not implemented. Widgets pre-compute a label string. | Full algorithm implemented in `buiy_core` (covers `aria-labelledby` chains, content fallback, hidden-subtree exclusion). |
| Focus model | Fragmented: `bevy_ui::focus` (mouse), `bevy_input_focus` (keyboard, 0.16+), `bevy_ui::auto_directional_navigation` (spatial, 0.18+). No `:focus-visible`, no traps, no inert, no roving tabindex, no `aria-activedescendant`. | Single focus tree with `:focus-visible`, traps, restoration, inert, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point, spatial gamepad nav. |
| APG widget coverage | Subset via `bevy_feathers` (Button, Checkbox, RadioGroup, Slider, TextInput, containers). No Combobox, Treeview, Grid, Menubar. | Full APG widget catalog as v1 commitment (`buiy_widgets`). |
| Live regions / announcer | Field-level support (`Live` enum on `Node`), no global announcer service, no `aria-relevant` filtering. | Global announcer service (Buiy resource), full `aria-relevant` filtering producer-side. |
| Linux AT-SPI | Opt-in via meta-crate's `accesskit_unix` feature; not on by default. | On for Buiy-owned windows by default. |
| Multi-window | Per-window-entity adapter; no awareness of "this window is not mine." | Per-window stack assignment; coexistence rules in `cross-cutting.md § 3.18`. |
| Verification | Unit tests for plumbing; no AccessKit-tree snapshot harness in CI. | AccessKit-tree snapshot tests in CI; manual-release-gate for real-AT cross-check. |

**Key design difference:** Buiy decomposes the producer-side surface up front; `bevy_a11y` is decomposing one property at a time over multiple Bevy minors. Both end at the same place on the AccessKit-tree side (a correct `TreeUpdate`); the path there differs by ~two years. Buiy is also the only design in this comparison set committing to ACCNAME 1.2 implementation, full APG widget coverage, and the global-announcer + `aria-relevant` filter on the producer side.

## vs egui's AccessKit integration

[egui](https://github.com/emilk/egui) ships AccessKit directly inside `eframe` (its windowing layer). There is no separate "a11y crate" — accessibility is integrated into the immediate-mode update loop. Each frame, the egui context emits an AccessKit `TreeUpdate` reflecting the widgets drawn that frame; the `eframe` wrapper owns the `accesskit_winit::Adapter`.

| Axis | egui | `bevy_a11y` | Buiy |
|---|---|---|---|
| Crate split | Inside eframe; no separate a11y crate | Separate `bevy_a11y` crate | Separate `buiy_core` + sub-crates |
| Mode | Immediate; per-frame full tree | Retained; per-frame diff against ECS state | Retained; per-frame diff against ECS state |
| Component surface | n/a — widgets emit a11y data inline as they draw | Megacomponent ECS-component | Decomposed ECS-components |
| Default for app authors | On by default in eframe | On by default but feature-gated on Linux | On by default for Buiy windows |

**Key design difference:** egui's immediate-mode model means there is no component surface to be BSN-friendly about — the a11y data is built inline as each widget is drawn each frame. The producer-side ergonomics question (decomposed vs megacomponent) doesn't arise for egui. AccessKit-side, egui and Bevy push the same `TreeUpdate` shape; the producer-side authoring story is structurally different. Buiy borrows nothing from egui on the component-model axis (retained-mode requires component surfaces) but borrows the "AccessKit-first, no intermediate a11y crate to layer through" posture.

## vs Slint's AccessKit integration

[Slint](https://slint.dev) ships AccessKit as a direct integration in its `winit` backend. Slint's component model is its own (`.slint` declarative language → compiled runtime); a11y properties are declared on widget definitions (`accessible-role`, `accessible-label`, `accessible-description`, …) and the runtime builds the AccessKit tree at materialization time.

| Axis | Slint | `bevy_a11y` | Buiy |
|---|---|---|---|
| Authoring | Declarative `.slint` files with `accessible-*` properties | ECS-component (`AccessibilityNode`) | ECS-components (decomposed) or BSN (`.bsn` files) when BSN lands |
| Adapter ownership | Slint runtime, per-window | `bevy_winit`, per-window-entity | Buiy, per-`WindowId` |
| Decomposition | Each `accessible-*` property is independent in the source language | Single component, properties via methods | Independent ECS-components per property |
| BSN-style overlay | n/a (Slint has its own composition model) | Broken for non-label properties | Works for every property by construction |

**Key design difference:** Slint's `accessible-role` and `accessible-label` declared on a widget are conceptually identical to Buiy's `A11yRole` and `A11yLabel` ECS-components. The Slint approach validates that **per-property a11y declarations are the right authoring surface** — Slint shipped this in 2023 with no equivalent of issue #17644 because the surface was decomposed from day one. `bevy_a11y` is the outlier in the Rust producer-side ecosystem on this axis; Buiy joins Slint and egui (in their respective idioms) on the decomposed side.

## vs Freya's AccessKit integration

[Freya](https://freyaui.dev) is a React-style Rust UI library built on Dioxus's hooks model, with rendering through Skia. Its AccessKit integration is direct (no intermediate a11y crate). As of the current Freya cut, the manifest pins `accesskit 0.24` + `accesskit_winit 0.32` (lagging the latest `accesskit_winit 0.33` by one minor — a typical downstream-toolkit pin pattern; see [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md)).

Component-model-wise Freya is closer to egui (declarative component tree built per render pass) than to `bevy_a11y`. The a11y attributes on Freya elements (`role="button"`, `aria-label="Save"`) map to AccessKit's vocabulary at render time.

**Key design difference:** Freya validates that React-style declarative attribute syntax is a workable producer-side surface. For ECS-component land that idiom translates directly to "small ECS-component per property" — which is what Buiy commits to.

## vs Xilem / Masonry's AccessKit integration

[Xilem](https://github.com/linebender/xilem) and [Masonry](https://github.com/linebender/xilem/tree/main/masonry) (Linebender, post-Druid) use AccessKit directly. The Masonry widget trait carries an `accessibility(&mut self, ctx: &mut AccessCtx)` method where each widget builds its `accesskit::Node` directly; Xilem's reactive layer above Masonry retains the same shape.

| Axis | Xilem / Masonry | `bevy_a11y` | Buiy |
|---|---|---|---|
| Surface | Widget trait method per widget building its `Node` | Megacomponent ECS-component | Decomposed ECS-components |
| Authoring | Imperative Rust + Xilem reactive | ECS authoring | ECS authoring + BSN (when it lands) |

**Key design difference:** Masonry's `accessibility()` widget-method is closer to AccessKit's underlying API shape than ECS-component land naturally is. The translation `Buiy widget → ECS-component → TreeUpdate` is the bridge `bevy_a11y` got wrong by collapsing the middle layer onto AccessKit's API directly. Buiy's decomposition is the structural answer.

## vs Unity's Accessibility module

Unity introduced the `UnityEngine.Accessibility` module in Unity 2023.2 (Tech Stream); mobile-screen-reader bridge (TalkBack + VoiceOver) is the headline feature. Desktop screen-reader support landed in Unity 6.3 (announced 2025).

The model is `AssistiveSupport` → `AccessibilityHierarchy` → individual `AccessibilityNode` entries (Unity's own type, unrelated to `bevy_a11y::AccessibilityNode`). The hierarchy is built imperatively from the app; Unity's UI Toolkit and uGUI integrate it at the widget level.

**Key design difference:** Unity is *not* an AccessKit producer; it implements its own cross-platform abstraction. The development is platform-team-driven inside Unity rather than community-driven, which means:
- Faster iteration on platform support (desktop landed in Unity 6.3 because Unity invested directly).
- No open-schema benefit (third-party tooling cannot consume Unity's a11y tree the way it can AccessKit's).
- Larger surface area to maintain (Unity owns every platform adapter).

Bevy / Godot / egui / Slint / Freya / Masonry **share** AccessKit; Unity does not. This is the largest engine-level architectural divide in game-engine a11y.

## vs Unreal's Slate accessibility

Unreal's Slate Screen Reader plugin (opt-in) plumbs the Slate widget hierarchy through a `IAccessibleWidget` interface to platform screen readers (NVDA / JAWS on Windows, VoiceOver on iOS). Non-Slate UI in Unreal (UMG, etc.) gets it only insofar as UMG wraps Slate.

| Axis | Unreal Slate | `bevy_a11y` | Buiy |
|---|---|---|---|
| Substrate | Platform-direct via `IAccessibleWidget` | AccessKit | AccessKit |
| Default? | Opt-in plugin | Default-plugins dep but Linux opt-in | Default on Buiy windows |
| Scope | Slate widgets only | Bevy widgets only | Buiy widgets only |

**Key design difference:** Unreal's a11y is widget-substrate-coupled (Slate) and platform-direct (no AccessKit-style schema layer). It is older than `bevy_a11y` and has more shipping commercial-title evidence, but the architecture is heavier — Epic maintains the platform translation themselves. `bevy_a11y`'s AccessKit-based approach is structurally lighter and benefits from shared-with-other-Rust-toolkits adapter maintenance.

## vs Godot's Control-node a11y

Godot 4.5 (2025-09) added AccessKit-based screen-reader support via PR [#76829](https://github.com/godotengine/godot/pull/76829) (`bruvzg`). Godot Control nodes carry a11y attributes; the engine builds an AccessKit tree.

| Axis | Godot 4.5 | `bevy_a11y` | Buiy |
|---|---|---|---|
| Substrate | AccessKit | AccessKit | AccessKit |
| Component surface | Per-property declarative on Control nodes | Megacomponent ECS | Decomposed ECS |
| Status | Experimental, partial editor coverage | Production, default-plugins | Production target for v1 |
| Production deployments | Just shipped (data thin) | Many downloads, few real deployments | n/a (Buiy is pre-v1) |

**Key design difference:** Godot and Buiy are the two AccessKit producers in the "general-purpose game engine" niche. Godot's integration is fresher (just shipped) and starts from a per-property surface; `bevy_a11y` is older but ships the megacomponent shape. If Godot's integration matures faster than `bevy_a11y`'s decomposition completes, the game-engine a11y comparison flips — Godot becomes the more BSN-equivalent declarative target while Bevy is still on the migration path. Buiy's parallel-stack choice positions it to leapfrog this question for Bevy specifically.

## Summary

`bevy_a11y` sits in the middle of the Rust AccessKit-producer ecosystem on the substrate side (everyone uses the same `Node` / `Action` / `Role` vocabulary) and at the back of the pack on the component-model surface (the megacomponent newtype is the only mainstream Rust AccessKit producer that did not decompose at design time). The two-year gap between this surface and where the BSN era expects it to be is the structural opening for Buiy's parallel-stack a11y model: by decomposing on day one, computing ACCNAME 1.2 in `buiy_core`, owning the focus model, and bundling the full APG widget catalog, Buiy is positioning the same AccessKit-based substrate to ship a production-grade a11y story without waiting for `bevy_a11y`'s piecemeal decomposition to complete.

## Sources

- egui AccessKit integration: https://github.com/emilk/egui/blob/master/crates/egui_glow/, [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md).
- Slint AccessKit integration: https://slint.dev, [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md).
- Freya AccessKit integration: https://freyaui.dev, [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md).
- Xilem / Masonry: https://github.com/linebender/xilem.
- Unity Accessibility module: https://docs.unity3d.com/Manual/com.unity.modules.accessibility.html, https://unity.com/blog/engine-platform/mobile-screen-reader-support-in-unity, Unity 6.3 native desktop screen reader: https://discussions.unity.com/t/native-desktop-screen-reader-support-now-available-in-unity-6-3/1681788.
- Unreal Slate Screen Reader: https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine.
- Godot AccessKit PR #76829: https://github.com/godotengine/godot/pull/76829, Godot 4.5 release: https://godotengine.org/releases/4.5/.
- AccessKit folder ecosystem + comparisons: [`prior-art/accesskit/ecosystem.md`](../accesskit/ecosystem.md), [`prior-art/accesskit/comparisons.md`](../accesskit/comparisons.md).
- Buiy foundation: [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).
- Sibling files: [`distribution.md`](distribution.md), [`history.md`](history.md), [`component-model-incident.md`](component-model-incident.md), [`api.md`](api.md), [`coexistence.md`](coexistence.md), [`focus-model.md`](focus-model.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md).
