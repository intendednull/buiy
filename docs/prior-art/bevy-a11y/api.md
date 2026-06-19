**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — public API surface (resources, components, events, system set, plugin) as of v0.18.1 stable + main HEAD (0.19.0-dev)

This file inventories what an app can reach when it depends on `bevy_a11y`. For architectural context see [`architecture.md`](architecture.md); for the BSN-unfriendliness story see [`component-model-incident.md`](component-model-incident.md); for how Buiy bypasses this API see [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6.

## Crate item inventory

From `crates/bevy_a11y/src/lib.rs` on main HEAD (verified 2026-05-22; the v0.17.3 inventory matches except for the accesskit dep version pin):

**Plugin:** `AccessibilityPlugin`

**Resources:**
- `AccessibilityRequested` — activation gate (atomic bool)
- `ManageAccessibilityUpdates` — secondary suppress flag (bool)

**Component:** `AccessibilityNode(pub accesskit::Node)` — the megacomponent

**Event (Message wrapper):** `ActionRequest` — newtype around `accesskit::ActionRequest`

**SystemSet:** `AccessibilitySystems::Update` (one-variant enum)

**Re-exports:** none. Note: "As of Bevy version 0.15, the `accesskit` crate is no longer re-exported from this crate" — apps depend on `accesskit` separately.

That's the entire bevy_a11y public API. Five items plus a plugin. The crate is small by design — the per-window adapter and per-widget property authoring live elsewhere (`bevy_winit`, `bevy_ui`).

## `AccessibilityPlugin`

```rust
#[derive(Default)]
pub struct AccessibilityPlugin;
```

Added to `DefaultPlugins`. To opt out:

```rust
App::new().add_plugins(DefaultPlugins.build().disable::<AccessibilityPlugin>())
```

Disabling means `AccessibilityRequested` / `ManageAccessibilityUpdates` resources are absent, downstream systems that probe them via `Option<Res<_>>` short-circuit, and the AccessKit tree is never built. Use this on Buiy-owned windows in an app that mixes Buiy and bevy_ui windows (see [`coexistence.md`](coexistence.md)).

## `AccessibilityRequested`

Atomic-backed bool resource. API surface (per main HEAD):

```rust
impl AccessibilityRequested {
    pub fn get(&self) -> bool;
    pub fn set(&self, value: bool);
}
```

`set` takes `&self`, not `&mut self` — internally an `AtomicBool`. Multiple systems can write without exclusive access. The flag is raised by `bevy_winit`'s `WinitActivationHandler` when AccessKit reports an AT activation; raised back to `false` is uncommon (ATs typically stay attached for the session lifetime).

Apps reading the flag for cost-amortisation:

```rust
fn my_a11y_system(requested: Res<AccessibilityRequested>) {
    if !requested.get() { return; }
    // build expensive a11y data
}
```

This is the pattern Buiy's `BuiySet::A11yUpdate` system inherits via condition-based scheduling.

## `ManageAccessibilityUpdates`

A separate bool resource for **external systems** that want to suppress the plugin's updates. Set to `false` to make `update_accessibility_nodes` no-op even when `AccessibilityRequested` is true. Used when an app wants to drive its own tree updates outside the plugin. The conjunctive gate is `requested && manage_updates`.

For Buiy: when Buiy is present in a window, Buiy sets `ManageAccessibilityUpdates = false` to stop bevy_a11y's tree from clashing with Buiy's. But the more durable solution is to disable `AccessibilityPlugin` outright on Buiy-only apps. The flag is a tactical override; plugin-disable is the strategic one.

## `AccessibilityNode`

```rust
#[derive(Component, Clone, Deref, DerefMut, Default)]
pub struct AccessibilityNode(pub accesskit::Node);
```

The megacomponent. App authors who use bevy_ui get `AccessibilityNode` auto-inserted on `Button` / `ImageNode` / `Label` widgets by the systems in `bevy_ui/src/accessibility.rs`. App authors building bespoke widgets insert it manually:

```rust
let mut node = accesskit::Node::new(accesskit::Role::Button);
node.set_label("Save");
commands.spawn(AccessibilityNode(node));
```

To mutate after spawn (the inconsistent-setter pain point from #17644):

```rust
fn toggle_disabled(mut q: Query<&mut AccessibilityNode, With<MyWidget>>) {
    for mut accessible in &mut q {
        if accessible.is_disabled() {
            accessible.clear_disabled();
        } else {
            accessible.set_disabled();
        }
    }
}
```

The `Deref<Target = accesskit::Node>` + `DerefMut` impl is why this works — the component is a thin wrapper. The setter-zoo lives on `accesskit::Node`, not on `AccessibilityNode`. See [`../accesskit/api.md`](../accesskit/api.md) for the full setter inventory.

Change detection is on the `AccessibilityNode` component, so any method-call mutation flags the entire wrapper as changed. Per-field change-detection is not available.

## `AccessibleLabel` (post-PR #24308, in `bevy_ui` not `bevy_a11y`)

```rust
#[derive(Component, Debug, Default, Clone, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
#[require(AccessibilityNode)]
#[component(immutable, on_insert = on_label_inserted, on_remove = on_label_removed)]
pub struct AccessibleLabel(pub String);

impl AccessibleLabel {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}
```

Lives in `bevy_ui/src/accessibility.rs`, not in `bevy_a11y`. **This is important for downstream API stability** — depending on `bevy_a11y` does not give you `AccessibleLabel`; you need `bevy_ui` (or its re-export). The split is awkward: `bevy_a11y` owns the megacomponent it requires, but the decomposed sibling lives in a different crate. Future decompositions may follow the same pattern (in `bevy_ui`) or migrate to `bevy_a11y`; this is unsettled.

Mutation pattern (since the component is `immutable`): replace the entity's `AccessibleLabel` to change the label. The `on_remove` + `on_insert` hooks fire and mirror into `AccessibilityNode.set_label` / `clear_label`.

## `ActionRequest` (Message wrapper)

```rust
/// Wrapper struct for `accesskit::ActionRequest`. This newtype is required to use
/// `ActionRequest` as a Bevy `Event` / `Message`.
```

A Bevy `Message`. Apps read it from `MessageReader<ActionRequest>`:

```rust
fn handle_actions(mut events: MessageReader<bevy_a11y::ActionRequest>) {
    for event in events.read() {
        let req = &event.0; // unwrap newtype to accesskit::ActionRequest
        match req.action {
            accesskit::Action::Click => { /* … */ }
            accesskit::Action::Focus => { /* … */ }
            accesskit::Action::SetValue => { /* … */ }
            _ => {}
        }
        // req.target is accesskit::NodeId — apps map back to Entity themselves
    }
}
```

Lookup from `NodeId` → `Entity` is the app's responsibility. Bevy uses `Entity::to_bits()` as the producer's NodeId convention, so `Entity::from_bits(node_id.0)` is the typical inverse (though the producer can choose any NodeId scheme). The brittleness — no canonical lookup map, no resolver — is one reason Buiy routes `ActionRequest` directly to Buiy entities via its own action plumbing, bypassing this event bus (see [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6).

`accesskit::ActionRequest` carries:
- `action: accesskit::Action` (22-variant enum; see [`../accesskit/tree-model.md`](../accesskit/tree-model.md))
- `target: accesskit::NodeId`
- `data: Option<ActionData>` (carries `SetTextSelection` `TextSelection`, `SetValue` value, scroll target, custom action data, etc.)

The newtype adds nothing semantic — it's purely a Bevy-`Message`-trait workaround.

## How an app pushes accessibility changes

There's no "push" API — the contract is **mutate the `AccessibilityNode` component, let the system pick it up**:

1. Mutate `AccessibilityNode` (or, for label, insert/replace `AccessibleLabel`).
2. `bevy_winit::accessibility::update_accessibility_nodes` (in `AccessibilitySystems::Update`) runs in `PostUpdate`, walks changed `AccessibilityNode`s, builds a `TreeUpdate`, calls `adapter.update_if_active(|| tree_update)`.
3. AccessKit pushes through `accesskit_winit` to the platform adapter (UIA / NSAccessibility / AT-SPI).
4. AT receives the change.

The producer never directly calls `adapter.update_if_active` from app code. The plugin owns the adapter and the call site. This is fine for the common case (per-widget systems in `bevy_ui/src/accessibility.rs` mutate `AccessibilityNode` per change-detected widget); it's friction for any consumer that wants per-frame control (e.g. a custom widget library with its own update lifecycle).

Buiy inverts this: Buiy's `BuiySet::A11yUpdate` system owns the `update_if_active` call directly for Buiy-owned windows; bevy_a11y's plugin is disabled or its `ManageAccessibilityUpdates` is false on those windows.

## How an app reads action requests

`MessageReader<bevy_a11y::ActionRequest>`, as shown above. Apps that need fine-grained per-widget action routing have to:

1. Read all events.
2. For each event, look up the target entity from the `NodeId`.
3. Dispatch to the widget's action handler.

There's no built-in action-dispatch layer; per-widget plumbing is what `bevy_ui/src/accessibility.rs` provides for the standard widgets, and what apps re-implement for bespoke widgets. Buiy's own action plumbing (foundation spec) handles this routing as a first-class concept rather than leaving it to consumers.

## API stability across versions

| Version | accesskit pin | API change |
|---|---|---|
| 0.10 (2023-03) | 0.8 | Initial: `AccessibilityNode(NodeBuilder)`, `AccessibilityRequested`, `AccessibilitySystem` (singular). |
| 0.13 (2024-02) | — | `AccessibilitySystem` → `AccessibilitySystems::Update` (set, then enum). |
| 0.15 (2024-12) | 0.17 | `accesskit` no longer re-exported. App depends on it directly. `AccessibilityNode` shifted from `NodeBuilder` to direct `Node` wrapping with the upstream API change. |
| 0.17.3 (2026) | 0.21 | Stable shape; `AccessibilityNode(pub Node)`. |
| 0.18.1 (2026-03) | 0.21 | Same shape. |
| 0.19.0-dev (main) | 0.24 | Same `bevy_a11y` shape. PR #24308 adds `AccessibleLabel` to `bevy_ui` (not `bevy_a11y`). |

The `bevy_a11y`-crate itself has been stable in shape for the last several releases; the underlying `accesskit` API changes drove the upgrade churn (NodeBuilder → Node was the 0.14/0.15 migration upstream; the recent 0.21 → 0.24 jump tracks AccessKit's faster cadence — see [`../accesskit/history.md`](../accesskit/history.md)).

## Cross-references

- [`architecture.md`](architecture.md) — the plugin shape and system flow
- [`component-model-incident.md`](component-model-incident.md) — why the API has the inconsistent-setter shape
- [`focus-model.md`](focus-model.md) — `bevy_input_focus` exposes `InputFocus` / `InputFocusVisible` separately (not via `bevy_a11y`)
- [`coexistence.md`](coexistence.md) — `disable::<AccessibilityPlugin>()` vs `ManageAccessibilityUpdates = false` tradeoff
- [`../accesskit/api.md`](../accesskit/api.md) — the underlying AccessKit setter inventory the megacomponent wraps
- [`../accesskit/tree-model.md`](../accesskit/tree-model.md) — `Role`, `Action`, `ActionData`, tri-state enums

## Sources

- `bevy_a11y/src/lib.rs` (main, 0.19.0-dev): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs
- `bevy_a11y/src/lib.rs` (v0.17.3): https://github.com/bevyengine/bevy/blob/v0.17.3/crates/bevy_a11y/src/lib.rs
- `bevy_ui/src/accessibility.rs` (post-#24308): https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/accessibility.rs
- `bevy_winit/src/accessibility.rs` (where the adapter lives): https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs
- docs.rs `bevy_a11y` 0.18.1: https://docs.rs/bevy_a11y/0.18.1/bevy_a11y/
