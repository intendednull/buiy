**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — architectural shape: the tiny crate, the megacomponent, the per-window adapter that lives elsewhere

## What bevy_a11y is, structurally

`bevy_a11y` is a **single-file crate** (`crates/bevy_a11y/src/lib.rs` on main HEAD; verified against `0.19.0-dev` Cargo.toml). It exposes a handful of plugin-level primitives — a resource, a system set, a component, an event wrapper, a plugin — and almost no logic. The real per-window adapter ownership, tree-build, and `ActionRequest` plumbing live in [`bevy_winit/src/accessibility.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs). The producer-side per-widget logic (which AccessKit role / label / value to set per UI node) lives in [`bevy_ui/src/accessibility.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/accessibility.rs). The `bevy_a11y` crate itself is glue.

This is worth naming up front because the brief's per-file plan implies more code lives in `bevy_a11y` than actually does. The pre-#17644 and post-#24308 component model both span three crates (`bevy_a11y`, `bevy_winit`, `bevy_ui`). When this folder refers to "bevy_a11y" architecturally, it means **the producer-side accessibility plugin surface owned across those three crates**, of which `bevy_a11y` is the smallest member but the API anchor.

## The plugin

```rust
#[derive(Default)]
pub struct AccessibilityPlugin;

impl Plugin for AccessibilityPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<AccessibilityRequested>()
            .init_resource::<ManageAccessibilityUpdates>()
            .allow_ambiguous_component::<AccessibilityNode>();
    }
}
```

Three things: install the activation gate (`AccessibilityRequested`), install the "is anyone managing this?" flag (`ManageAccessibilityUpdates`), and disable ambiguity errors for the megacomponent (`AccessibilityNode`). No systems added here. The plugin is a **resource installer**, not a system-scheduler.

The `AccessibilityPlugin` is a default sub-plugin in `DefaultPlugins` since Bevy 0.10. Apps disabling it do so via `disable::<AccessibilityPlugin>()` on the `DefaultPluginsBuilder`.

## The `AccessibilitySystems` set (singleton)

```rust
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum AccessibilitySystems {
    Update,
}
```

A one-variant enum. All accessibility systems hang off `AccessibilitySystems::Update`, run in `PostUpdate`, installed by `bevy_winit`'s `AccessKitPlugin` (yes, there are two accessibility-related plugins — one in `bevy_a11y`, one in `bevy_winit`). The systems are:

- `window_closed` — adapter cleanup on window close
- `poll_receivers` — drains the `WinitActionRequestHandlers` queues, converts `accesskit::ActionRequest` → Bevy `ActionRequest` message
- `update_accessibility_nodes` — rebuilds the tree from `AccessibilityNode` components and pushes to the adapter via `update_if_active`

Order is roughly `window_closed` then `poll_receivers` then `update_accessibility_nodes`, all conditional on activation flags. This is *the* system set Buiy's `BuiySet::A11yUpdate` parallels; the activation-gating pattern is sound and Buiy borrows it (see [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — "AccessibilityRequested activation gate").

## The activation gate — `AccessibilityRequested`

```rust
/// Tracks whether an assistive technology has requested accessibility information.
```

A resource with `get() -> bool` and `set(value: bool)` over atomic ops (so it's `Sync`). Set to `true` when the platform AT activates (e.g. NVDA attaches, VoiceOver fires `request_initial_tree`); systems short-circuit when it's `false`. This is the cost-amortisation lever — building a 500-node tree per frame for an idle window is real cost; `AccessibilityRequested` makes it free until something is listening.

The companion `ManageAccessibilityUpdates` is a separate flag that *another* system can lower to suppress all updates (useful when an app wants to temporarily own the tree from outside the plugin). Both are bools; activation gating is conjunctive — updates run when **both** are true.

## The megacomponent — `AccessibilityNode`

```rust
#[derive(Component, Clone, Deref, DerefMut, Default)]
pub struct AccessibilityNode(pub Node);
```

Where `Node` is `accesskit::Node` — the entire AccessKit node-builder struct (role, name, value, bounds, transform, label-relations, state flags, ~200+ setter methods). Per-entity ECS storage of one `AccessibilityNode` carries every a11y property for that entity. Updates happen by calling setter methods on the `Node` inside (`accessible.set_label(text)`, `accessible.set_role(Role::Button)`, etc.) via `Deref`/`DerefMut`.

This is the megacomponent that issue [#17644](https://github.com/bevyengine/bevy/issues/17644) names as BSN-unfriendly. The full incident analysis is in [`component-model-incident.md`](component-model-incident.md). **The component still exists in this exact shape on main HEAD as of 2026-05-22.** PR [#24308](https://github.com/bevyengine/bevy/pull/24308) did not decompose it; it added a single sibling component (`AccessibleLabel`) that mirrors into `AccessibilityNode` via component hooks. The wrapper is still the wrapper.

The `allow_ambiguous_component::<AccessibilityNode>()` call in the plugin is a workaround for Bevy's system-ambiguity detector — multiple systems mutate the same `AccessibilityNode` per frame (the button system sets role, the label system sets label, the layout system sets bounds), and Bevy would normally warn. The flag suppresses the warning. It's a structural sign that the megacomponent shape forces the producer-side code into a multi-system-mutates-one-component pattern that Bevy's safety tooling doesn't love.

## The `ActionRequest` newtype

```rust
/// Wrapper struct for `accesskit::ActionRequest`. This newtype is required to use
/// `ActionRequest` as a Bevy `Event`.
```

A thin newtype around `accesskit::ActionRequest`. Required because Bevy's `Event` (now `Message`) trait can't be derived on a foreign type. The newtype is created in `bevy_winit`'s `poll_receivers` system from queued action requests (see "Action handler routing" below) and surfaced as a Bevy event for apps to read. Buiy bypasses this newtype — Buiy's own action plumbing routes `accesskit::ActionRequest` directly to Buiy entities by `NodeId` (= `Entity::to_bits()`), no event-bus indirection (see [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6).

## Per-window adapter ownership (lives in `bevy_winit`)

```rust
struct AccessKitState {
    name: String,
    entity: Entity,
    requested: AccessibilityRequested,
}
```

The `accesskit_winit::Adapter` is stored in a thread-local `AccessKitAdapters` keyed by `Entity` (the window entity), not as a normal ECS resource. The comment in source says this is "until issue #17667 is complete" — the workaround exists because the adapter is `!Send`. Each `Adapter` is paired with a `WinitActivationHandler` (returns the initial tree via `build_initial_tree`) and a `WinitActionHandler` (pushes incoming `accesskit::ActionRequest` into a per-window `VecDeque`).

```rust
impl ActivationHandler for WinitActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.0.lock().unwrap().build_initial_tree())
    }
}

impl ActionHandler for WinitActionHandler {
    fn do_action(&mut self, request: ActionRequest) {
        let mut requests = self.0.lock().unwrap();
        requests.push_back(request);
    }
}
```

Multi-window support is therefore real and structural: each window gets its own adapter, its own activation handler, its own action queue, keyed by the window entity. This is the structural pattern Buiy adopts — per-window adapter ownership keyed by winit `WindowId` (Buiy uses the platform window id rather than `Entity` to make the boundary survive entity de-spawn / re-spawn cycles, but the shape is the same).

## Tree-update push mechanism

`update_accessibility_nodes` walks all entities with an `AccessibilityNode` component, builds a `TreeUpdate` containing the changed nodes plus a `Tree { root, focus, ... }` block, and calls `adapter.update_if_active(|| tree_update)`. The closure form means the build is **skipped entirely** when no AT is attached — the adapter checks its active flag before invoking the callback.

The producer-side code mutates `accesskit::Node` fields by calling setter methods on `AccessibilityNode` via `Deref::deref_mut`. Change-detection on `AccessibilityNode` (the component) is coarse — any setter touch flags the component as changed, so any mutation rebuilds the entire `Node` representation for that entity in the next tree update. Fine-grained per-field change-detection isn't possible because the fields aren't separate components. (This is precisely the BSN-hostility complaint in #17644.)

## How the post-#24308 shape diverges from "decomposed"

PR #24308 (merged 2026-05-21, milestone 0.19) introduces one new component:

```rust
#[derive(Component, Debug, Default, Clone, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
#[require(AccessibilityNode)]
#[component(immutable, on_insert = on_label_inserted, on_remove = on_label_removed)]
pub struct AccessibleLabel(pub String);
```

with hooks:

```rust
fn on_label_inserted(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    if let Some(label) = world.get::<AccessibleLabel>(entity) {
        let label_text = label.0.clone().into_boxed_str();
        if let Some(mut accessible) = world.get_mut::<AccessibilityNode>(entity) {
            accessible.set_label(label_text);
        }
    }
}
```

Net effect: **one** field of `AccessibilityNode` (the label) is now mirror-authored from a separate immutable component. The other ~200 fields still flow through the megacomponent's setters. PR author viridia notes in the PR description: *"It may not be a 100% fix, but it's good enough to close the ticket I think."* The fix is incremental, single-property, and intentionally stops short of full decomposition.

This matters for Buiy: the upstream decomposition trajectory is "split one field at a time as need arises." Buiy commits to the opposite — full decomposition (`A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`) from day one. The two decomposition shapes will not converge; the rationale for full-replacement (not layer-over) is structural, not merely about #17644 being unfixed. See [`component-model-incident.md`](component-model-incident.md) for the long form.

## Cross-references

- [`component-model-incident.md`](component-model-incident.md) — the #17644 / #24308 case study in depth
- [`api.md`](api.md) — public API surface
- [`coexistence.md`](coexistence.md) — per-window coexistence with Buiy
- [`focus-model.md`](focus-model.md) — focus tracking (which mostly lives in `bevy_input_focus`, not `bevy_a11y`)
- [`/home/user/buiy/docs/prior-art/accesskit/architecture.md`](../accesskit/architecture.md) — the underlying AccessKit protocol
- [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — Buiy's borrow / avoid table for the AccessKit shape

## Sources

- `bevy_a11y` lib.rs (main HEAD, 0.19.0-dev): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs
- `bevy_a11y` Cargo.toml (main HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/Cargo.toml
- `bevy_a11y` lib.rs (v0.17.3): https://github.com/bevyengine/bevy/blob/v0.17.3/crates/bevy_a11y/src/lib.rs
- `bevy_a11y` Cargo.toml (v0.18.1): https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_a11y/Cargo.toml (accesskit = "0.21")
- `bevy_winit/src/accessibility.rs` (main HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs
- `bevy_ui/src/accessibility.rs` (main HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/accessibility.rs
- PR #6874 (AccessKit integration, Bevy 0.10, 2023-03): https://github.com/bevyengine/bevy/pull/6874
- Issue #17644 (BSN-unfriendly, opened 2025-02-02 by viridia): https://github.com/bevyengine/bevy/issues/17644
- PR #24308 (Introduce AccessibleLabel, merged 2026-05-21, milestone 0.19): https://github.com/bevyengine/bevy/pull/24308
- Buiy foundation — architecture §2.6: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
