**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y / Buiy per-window coexistence — adapter-slot single-occupancy, the suppression rule, no shared-window coordinator

## The structural constraint

AccessKit allows **exactly one tree per `accesskit_winit::Adapter` per window**. The adapter slot is single-occupant by design — there is no upstream merge protocol where two producers push subtrees that get glued together. See [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md) for the long form on the AccessKit side; the relevant constraint for this file is: **two producers cannot share an adapter**.

Two consequences:

1. On any given window, exactly one stack (bevy_a11y or Buiy) owns the adapter.
2. There is no "Buiy and bevy_a11y coexist on the same window with some coordinator" mode in v1. Buiy's spec ([`../../specs/2026-05-07-buiy-foundation/cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) commits to per-window-only coexistence; a coordinator is a deferred follow-up sub-spec (`buiy-coexistence-design`) if demand arises.

## The supported model

Per the Buiy foundation spec:

- An app may have multiple windows. Each window is **owned by one stack** — either Buiy or bevy_ui (and by extension, bevy_a11y, which is bevy_ui's accessibility producer).
- On a Buiy-owned window: Buiy owns the `accesskit_winit::Adapter`, the render-graph nodes, the `bevy_picking` backend(s), the focus model, the IME consumer. `bevy_a11y` is suppressed for that window. bevy_ui's own systems do not render or interact on that window.
- On a bevy_ui-owned window: bevy_ui retains its current behavior. Buiy is absent.
- Window stack assignment is **fixed at window creation**. No runtime stack switching for an existing window in v1.

The key is **winit `WindowId` as the partition key**, not Bevy `Entity`. WindowId survives entity de-spawn / re-spawn cycles and is the identity AccessKit's adapter uses internally. (Bevy_winit's `AccessKitAdapters` thread-local is keyed by `Entity` today — see [`architecture.md`](architecture.md) — but the underlying identity is the winit window.)

## How Buiy suppresses bevy_a11y on its windows

Two mechanisms, with different scopes.

### Mechanism 1: disable the entire `AccessibilityPlugin`

For Buiy-only apps (no bevy_ui windows at all), the cleanest solution is to disable bevy_a11y's plugin outright:

```rust
App::new()
    .add_plugins(DefaultPlugins.build().disable::<AccessibilityPlugin>())
    .add_plugins(BuiyPlugin);
```

Effects:
- `AccessibilityRequested` and `ManageAccessibilityUpdates` resources are absent.
- `bevy_winit`'s accessibility systems are absent (they're added by `AccessibilityPlugin`'s registration, not directly).
- No `AccessKitAdapters` thread-local is populated by bevy.
- Buiy installs its own adapters per Buiy window.

This is the recommended path when no bevy_ui window exists.

### Mechanism 2: per-window suppression via `ManageAccessibilityUpdates`

For mixed-stack apps (some bevy_ui windows, some Buiy windows), the plugin must remain installed (it serves the bevy_ui windows). Buiy then suppresses updates on a per-window basis. The bevy_a11y `ManageAccessibilityUpdates` flag is a global toggle, so it doesn't help directly — it suppresses *all* windows.

The actual per-window suppression comes from **not registering a Buiy entity as an `AccessibilityNode` host on the Buiy window** and **not installing bevy_winit's accessibility system on the Buiy window's adapter**. The way bevy_winit's `update_adapter` system is structured, it iterates `AccessKitAdapters` (an entity → adapter map) and for each adapter walks `AccessibilityNode` components. If Buiy owns the window's adapter (registered in Buiy's own per-window adapter map, not bevy_winit's `AccessKitAdapters`), and the window's entity has no children with `AccessibilityNode` components (Buiy uses its own `A11y*` components instead), bevy_winit's system has nothing to push for that window.

This is structurally fragile — it relies on Buiy owning the adapter *outside* `AccessKitAdapters`. The cleaner long-term shape is `buiy-coexistence-design` formalising a registry of which crate owns which window's a11y adapter. Until then, the rule is "Buiy windows don't use `AccessibilityNode` components; Buiy registers its adapters in its own map; bevy_winit's accessibility iteration is empty for Buiy windows."

## Tree-conflict scenarios

If both bevy_a11y and Buiy tried to push `TreeUpdate`s to the same window's adapter, the outcome depends on order:

- The later call wins, but only for the nodes it pushed. AccessKit's `TreeUpdate` is a delta — it only mentions changed nodes. Nodes the later call didn't mention retain the earlier call's state, but with the wrong tree shape (because the earlier producer's `Tree.focus` and `Tree.root` were set with that producer's NodeId space, not the later one's).
- `NodeId` collisions: both producers using `Entity::to_bits()` would have overlapping NodeIds for unrelated entities. AccessKit would silently mis-link relations.
- Focus would oscillate between the two producers' chosen focused node every frame.
- ATs would receive incoherent updates and would either de-attach or expose a broken tree.

This is why "just push from both" is not a coexistence model — it produces a broken AT experience deterministically. The single-occupant adapter rule must be honored.

## What an app does to disable bevy_a11y for a Buiy window

The committed Buiy-side path is:

1. App adds `BuiyPlugin`, which installs its own per-window adapter manager.
2. For each Buiy window, Buiy installs an `accesskit_winit::Adapter` into Buiy's own per-window map (keyed by `WindowId`).
3. Buiy's `BuiySet::A11yUpdate` system reads Buiy's decomposed a11y components (`A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`), builds a `TreeUpdate`, calls `adapter.update_if_active` on the Buiy adapter.
4. Buiy's action-routing system polls Buiy's `WinitActionHandler` queue and dispatches `accesskit::ActionRequest`s to the target Buiy entity via Buiy's action plumbing — not via the `bevy_a11y::ActionRequest` event bus.

bevy_a11y's `AccessibilityPlugin` and bevy_winit's accessibility systems remain present in the app process but operate only on non-Buiy windows.

## App-level config matrix

| App shape | bevy_a11y plugin | Buiy plugin | Notes |
|---|---|---|---|
| Pure bevy_ui app | enabled | absent | Status quo. |
| Pure Buiy app | disabled | present | Cleanest config; recommended. |
| Mixed bevy_ui + Buiy, no shared window | enabled | present | Per-window stack assignment by app at window creation. |
| Mixed shared-window (single window, both stacks rendering) | **unsupported in v1** | — | Adapter slot is single-occupant. Future `buiy-coexistence-design` sub-spec if demand. |

## Migration

Per the Buiy spec, migration from a bevy_ui window to a Buiy window is by **replacement** of the window's UI tree, not by extending bevy_ui components. Practically:

1. Tear down the bevy_ui content on the window. The window's `AccessibilityNode` components vanish with the entities.
2. Buiy claims the window's adapter slot — replacing bevy_winit's `AccessKitAdapters` entry for that window with Buiy's own.
3. Spawn Buiy UI tree with Buiy a11y components.

The window's winit handle is preserved. The adapter is recreated. The AT may briefly see an empty tree during the swap; this is acceptable for the v1 contract (transient blank), and a future polish item to make the swap atomic via AccessKit's deactivation/reactivation.

## Open question

[`../../specs/2026-05-07-buiy-foundation/README.md` § 5](../../specs/2026-05-07-buiy-foundation/README.md) names two related open questions:

- **AccessKit-adapter ownership when both stacks coexist same-window.** Currently the spec rules this out. If demand arises, `buiy-coexistence-design` defines the coordinator.
- **Coexistence policy with `bevy_feathers` / `bevy_ui_widgets`.** Coexistence at the app level is committed; whether Buiy ships migration adapters from bevy_ui widgets is open. This applies symmetrically to bevy_a11y — Buiy ships no migration adapter from `AccessibilityNode` to Buiy's decomposed components.

Both remain open. The committed rule is per-window single-stack. Coordinator-merged is the deferred follow-up.

## Cross-references

- [`architecture.md`](architecture.md) — bevy_a11y's per-window adapter mechanics (via `bevy_winit`)
- [`component-model-incident.md`](component-model-incident.md) — why Buiy replaces rather than layers
- [`api.md`](api.md) — `disable::<AccessibilityPlugin>()` and `ManageAccessibilityUpdates` API surface
- [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md) — the single-occupant adapter constraint
- [`../accesskit/lessons.md`](../accesskit/lessons.md) — "Per-window adapter ownership keyed by winit WindowId"
- [`../../specs/2026-05-07-buiy-foundation/architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md) — Buiy's committed rule
- [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md` § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) — coexistence rules including the per-window-keyed state list

## Sources

- `accesskit_winit::Adapter` docs.rs (single-tree-per-adapter constraint): https://docs.rs/accesskit_winit/0.33.0/accesskit_winit/
- `bevy_winit/src/accessibility.rs` (AccessKitAdapters thread-local, the issue-#17667 workaround): https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs
- `bevy_a11y` API: https://docs.rs/bevy_a11y/0.18.1/bevy_a11y/
- Buiy foundation — cross-cutting § 3.18: [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- Buiy foundation — README § 5 open questions: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
