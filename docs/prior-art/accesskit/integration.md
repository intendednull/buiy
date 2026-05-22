**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — canonical integration pattern (adapter lifecycle, TreeUpdate cadence, ActionRequest routing) and how egui / Slint / Bevy / Freya / Xilem / Buiy each wire it up

## The canonical winit-based integration pattern

AccessKit deliberately separates the **data schema** (the `accesskit` core crate — `Node`, `Tree`, `TreeUpdate`, `NodeId`, `Role`, `Action`) from the **platform adapters** (`accesskit_windows`, `accesskit_macos`, `accesskit_unix`, `accesskit_android`, `accesskit_ios`). Most Rust GUI toolkits do not bind to those platform adapters directly; they use `accesskit_winit::Adapter`, which is the supported abstraction for any GUI built on top of `winit` ([upstream README](https://github.com/AccessKit/accesskit/blob/main/README.md)). The Buiy plan is the same — talk to `accesskit_winit`, not the per-platform adapters.

The three load-bearing pieces a host has to provide are:

1. **One `accesskit_winit::Adapter` per `winit::Window`,** constructed when the window is created. The adapter wraps the platform adapter and the consumer-side state for that window.
2. **A `TreeUpdate` push on every accessibility-relevant change.** The host calls `adapter.update_if_active(|| build_tree_update())`. The closure is only invoked if an assistive technology is actually listening on that window — the lazy gate that makes per-frame updates affordable on idle windows (see [capabilities.md](capabilities.md) and [critiques.md](critiques.md) for the cost story).
3. **An `ActivationHandler` + `ActionHandler` pair.** `ActivationHandler::request_initial_tree` is called by AccessKit the first time an AT activates on the window (this is when the host first builds and returns a complete tree). `ActionHandler::do_action(ActionRequest)` is called every time the AT requests an `Action` like `Click`, `Focus`, `ScrollIntoView`, or `SetValue`. The host routes that back into its widget state.

That triplet — *adapter ownership, push-on-change TreeUpdate, ActionRequest demux* — is the entire AccessKit integration contract. Everything else (role mapping, name computation, live regions, focus tracking) is host concern.

## The four key types a host touches

From [`accesskit_winit`](https://docs.rs/accesskit_winit/latest/accesskit_winit/) and [`accesskit`](https://docs.rs/accesskit/latest/accesskit/) at 0.24.0 / 0.33.0 respectively:

- **`accesskit_winit::Adapter`** — the per-window adapter handle.
- **`accesskit_winit::WindowEvent`** — variants the host has to translate from `winit::WindowEvent` (focus changes, etc.) into AccessKit-relevant events.
- **`accesskit::TreeUpdate`** — `{ nodes: Vec<(NodeId, Node)>, tree: Option<Tree>, focus: NodeId }`. The diff payload pushed to AccessKit on change.
- **`accesskit::Node`** — built by the host per accessibility-relevant entity (via setters like `set_role`, `set_label`, `set_bounds`, `set_children`, `set_labelled_by`, `set_active_descendant` — see [api.md](api.md)).

`accesskit_winit` is the load-bearing crate for the Buiy integration; the per-platform adapter crates are not direct dependencies of Buiy.

## How egui integrates

egui's AccessKit integration landed in [PR #2294](https://github.com/emilk/egui/pull/2294), merged 2022-12-04, authored by Matt Campbell (AccessKit's primary author). The integration is part of `eframe` (egui's standard wrapper) and is **enabled by default** in eframe. The integration is **lazy** — egui does not build an AccessKit tree until an assistive technology activates on the window, then it builds the tree per-frame from egui's immediate-mode widget output. The lazy gate is critical for egui because immediate-mode means there is no persistent widget tree to walk cheaply.

This is the canonical reference implementation for an immediate-mode GUI feeding AccessKit, and the AccessKit README's "stable IDs for each UI element" requirement (see [`README.md`](https://github.com/AccessKit/accesskit/blob/main/README.md)) was added specifically to make egui-style integration tractable.

## How Slint integrates

Slint added accessibility primitives in v0.2.5 (2022-07-06) — the `accessible-*` properties on declarative components ([Slint CHANGELOG](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md)). AccessKit is named explicitly in the changelog at v1.7.0 (2024-07-18), where the winit backend bumped to accesskit 0.16 alongside winit 0.30. Slint's a11y is property-driven at the `.slint` markup layer, which is closer to a static tree than egui's immediate-mode reshape. Slint maintains AccessKit upgrades on its own cadence; recent activity (e.g. Slint PR #9919, "Set toolkit name and version on AccessKit tree", merged 2025-11-03) shows ongoing investment.

## How Iced integrates (it doesn't yet)

Iced has accessibility-themed PRs but **no AccessKit integration on `master` as of 2026-05-22**. The active draft is [PR #3111 "draft: Accesskit integration"](https://github.com/iced-rs/iced/pulls?q=accesskit) (opened 2025-11-11). An earlier attempt [PR #1849 "WIP: Iced accessibility"](https://github.com/iced-rs/iced/pulls?q=accesskit) sat in draft from 2023-05-11 onward. PR #3281 "Accessibility support" was closed (not merged) on 2026-03-14. Iced 0.14.0 (2025-12-07) shipped without AccessKit in the release notes. The pre-amble for this folder cited Iced as a downstream adopter; that is **wrong as of 2026-05-22** — Iced is queueing AccessKit integration, not shipping it.

## How Bevy integrates today via `bevy_a11y` (the crate Buiy is replacing)

The original integration is [PR #6874](https://github.com/bevyengine/bevy/pull/6874), merged 2023-03-01, shipped in Bevy 0.10. It created the `bevy_a11y` crate and decoupled `bevy_ui` from `bevy_winit` to make AccessKit reachable. `bevy_a11y` exposes a single megacomponent — `AccessibilityNode(NodeBuilder)` historically, later `AccessNode(accesskit::Node)` — wrapping the entire AccessKit Node payload, plus a `Focus` resource and an `ActionRequest` event bridge. On the current `main` (per [bevy_a11y/Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/Cargo.toml)) it depends on accesskit 0.24.

The megacomponent design is **BSN-hostile** by Bevy's own analysis ([bevyengine/bevy#17644](https://github.com/bevyengine/bevy/issues/17644)). The issue calls out three concrete problems: (a) the component's properties are reachable only through method calls (`set_disabled()`, `clear_disabled()`) rather than direct fields, (b) inconsistent calling conventions per property type, and (c) monolithic shape that prevents BSN-style merge/patch composition. The issue's recommendation is "develop an idiomatic API for accessibility properties that translates into AccessKit structures — mirroring how `Node` abstracts Taffy's layout system." That recommendation is exactly what Buiy ships as the `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` split.

## How Buiy integrates

Per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) and [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md):

- **Per-window adapter ownership.** Buiy holds `Map<WindowId, accesskit_winit::Adapter>` keyed by the winit `WindowId` (not Bevy `Entity`). AccessKit allows exactly one tree per adapter per window; Buiy enforces this by being the sole adapter holder on any window it owns.
- **bevy_a11y is replaced, not layered.** Buiy does not coexist with `bevy_a11y` on the same window — Buiy suppresses it (see [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)). Multi-stack coexistence is per-window, not per-app-shared-window.
- **Decomposed components feed `TreeUpdate`.** `A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations` are small public-field components. A `BuiySet::A11yUpdate` system walks the changed entities each frame and builds the `TreeUpdate` from them. The composition follows the issue-17644 recommendation: an idiomatic API that translates to AccessKit Node, not a wrapper component.
- **ACCNAME 1.2 in `buiy_core`.** Name computation (label > labelledby > content > title precedence, hidden-subtree exclusion, locale-aware composition) runs in `buiy_core` and feeds the assembled Node's `set_label`. AccessKit itself does not compute accname; consumers must.
- **`ActionRequest` routing into Buiy entities.** Buiy owns the `ActionHandler::do_action` callback. Each incoming `ActionRequest` is routed to the Buiy entity whose `NodeId` matches the requested target (Buiy derives stable `NodeId`s from `Entity`). bevy_a11y's event bridge is not used.
- **Lazy gate via `AccessibilityRequested`.** `BuiySet::A11yUpdate` only does real work when AccessKit reports an AT is attached (the `adapter.update_if_active(...)` gate). Idle windows pay nothing.

## Coordinate-space gotcha

AccessKit `Node::set_bounds` takes an `accesskit::Rect` in **absolute screen coordinates** (window-relative coordinates are wrong — screen readers consume bounds via OS-level APIs that expect screen space). Embedders must translate from window-local layout coordinates to screen coordinates per push. Buiy's `BuiySet::A11yUpdate` pulls `winit::Window::inner_position()` once per frame and offsets every `Node`'s bounds; per-window adapter ownership makes that one-window-scoped, which is the simple case.

## Threading

`accesskit_winit::Adapter` calls must be made from the thread that owns the winit event loop (the main thread on macOS, the main thread or any thread on other platforms but in practice always the event-loop thread). AccessKit does not itself spawn threads, but the per-platform adapters interact with platform APIs (UIA, NSAccessibility, AT-SPI) that have main-thread expectations. In Bevy this means the AccessKit pump runs in the main world on `bevy_winit`'s event-loop thread, not in a parallel system.

## One adapter per window, period

The Buiy spec states: "AccessKit allows exactly one tree per `accesskit_winit::Adapter` per window." Verified — `accesskit_winit::Adapter::new` takes a `&winit::window::Window` and constructs one adapter bound to that window. There is no upstream API to attach two adapters to one window. The coexistence constraint in [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) (one stack per window) is the direct consequence of this AccessKit shape.

## Cross-links

- Architecture context for the per-window adapter: [`api.md`](api.md) (adapter handle API) and [`platform-adapters.md`](platform-adapters.md) (what `accesskit_winit` calls into per platform).
- Tree-model context for the Node payload: [`tree-model.md`](tree-model.md).
- The role / state / relation taxonomy: [`capabilities.md`](capabilities.md).
- bevy_a11y issue 17644 context: [`architecture.md § 2.4`](../../specs/2026-05-07-buiy-foundation/architecture.md) (BSN-friendly constraint) + this file.

## Sources

- https://github.com/AccessKit/accesskit/blob/main/README.md
- https://docs.rs/accesskit_winit/latest/accesskit_winit/
- https://docs.rs/accesskit/latest/accesskit/
- https://github.com/emilk/egui/pull/2294
- https://github.com/bevyengine/bevy/pull/6874
- https://github.com/bevyengine/bevy/issues/17644
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/Cargo.toml
- https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- https://github.com/iced-rs/iced/pulls?q=accesskit
- https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/cross-cutting.md
