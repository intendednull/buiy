**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — producer-side API surface and integration shape

This file documents the Rust API surface a producer (toolkit / app) interacts with. Versions: `accesskit` 0.24.0 (2026-02-01), `accesskit_winit` 0.33.0 (2026-05-11). For data-model semantics see [tree-model.md](tree-model.md); for the protocol mechanics see [architecture.md](architecture.md); for per-platform adapter shape see [platform-adapters.md](platform-adapters.md).

## Top-level item inventory (`accesskit` 0.24.0)

From `docs.rs/accesskit/0.24.0`:

**Structs:** `ActionRequest`, `Affine`, `Color`, `CustomAction`, `Node`, `NodeId`, `Point`, `Rect`, `Size`, `TextDecoration`, `TextPosition`, `TextSelection`, `Tree`, `TreeId`, `TreeUpdate`, `Uuid`, `Vec2`.

**Enums:** `Action`, `ActionData`, `AriaCurrent`, `AutoComplete`, `HasPopup`, `Invalid`, `ListStyle`, `Live`, `Orientation`, `Role`, `ScrollHint`, `ScrollUnit`, `SortDirection`, `TextAlign`, `TextDecorationStyle`, `TextDirection`, `Toggled`, `VerticalOffset`.

**Traits:** `ActionHandler`, `ActivationHandler`, `DeactivationHandler`.

**Type aliases:** `NodeIdContent = u64`.

The crate exposes only what producers need on the boundary. Internal types live in `accesskit_consumer` (adapter-side) and per-platform crates.

## Building a `Node`

The 0.24 API is **direct setter style on a mutable `Node`** — there is no `NodeBuilder`, no `NodeClass`, no `NodeClassSet`. (Earlier AccessKit versions had a builder pattern with class-based sharing; that was removed before the 0.24 line. Bevy-side examples and external write-ups predating the removal may still mention `NodeBuilder` — those are outdated.)

```rust
use accesskit::{Node, NodeId, Rect, Role, Action};

let mut node = Node::new(Role::Button);
node.set_label("Submit");
node.set_bounds(Rect { x0: 10.0, y0: 100.0, x1: 90.0, y1: 130.0 });
node.add_action(Action::Click);
node.add_action(Action::Focus);
// node is ready to be paired with its NodeId in a TreeUpdate.nodes vec
```

`Node::new(role: Role)` is the only constructor. Every property has a setter (`set_label`, `set_value`, `set_bounds`, `set_description`, `set_children`, etc.) and a clearer (`clear_label`, etc.). Relations have list-mutation helpers (`push_child(NodeId)`, `set_controls(Vec<NodeId>)`, `clear_owns()`). Actions have `add_action(Action)`, `remove_action(Action)`, `clear_actions()`, `supports_action(Action) -> bool`.

Per docs.rs, the `Node` API has on the order of 200+ accessor methods covering the full field surface. Documentation coverage on docs.rs is at ~16.7% for `accesskit` 0.24 — the source remains the authoritative reference for nuanced field semantics.

## `NodeId` semantics

```rust
pub type NodeIdContent = u64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct NodeId(pub NodeIdContent);
```

A `NodeId` is just a `u64` wrapper, opaque to AccessKit — the producer chooses the value space. Common producer strategies:

- **Bevy ECS:** derive from `Entity` — `Entity::to_bits() -> u64` is the obvious mapping. Buiy uses this strategy: "Stable `NodeId`s derived from Bevy `Entity`" ([architecture.md §2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)). Entity de-spawn means the NodeId vanishes, which matches AT expectations.
- **Slotmap-backed toolkits:** derive from slot key.
- **DOM-shaped toolkits:** UUID hashing or a sequential counter.

The producer is responsible for ensuring `NodeId`s are **stable across `TreeUpdate`s** — the adapter's diff model requires that the same logical node carry the same `NodeId` over time. A widget that changes identity between frames (e.g. due to remount) is a different node from AccessKit's view.

`NodeId`s are **per-tree-scoped**. Two adapters / two windows can use overlapping numeric values without conflict; the `TreeUpdate.tree_id` carries the tree's identity.

## `TreeUpdate` construction

```rust
let update = TreeUpdate {
    nodes: vec![(node_id_a, node_a), (node_id_b, node_b)],
    tree: Some(Tree {
        root: root_id,
        toolkit_name: Some("Buiy".into()),
        toolkit_version: Some(env!("CARGO_PKG_VERSION").into()),
    }),
    tree_id: TreeId::default(),
    focus: focused_id,
};
adapter.update_if_active(|| update);
```

`nodes` carries the changed nodes; the adapter merges them into its cached tree. `tree: Some(_)` is required only on initial activation or when toolkit metadata changes — `None` for routine diff updates. `focus` is required on every update; if focus didn't change, repeat the previous frame's focused `NodeId`.

`TreeId` identifies which tree the update belongs to. For single-window apps the default is fine; multi-window apps allocate one `TreeId` per window.

## `accesskit_winit::Adapter` API

The adapter struct, its constructors, and the `Event` / `WindowEvent` types:

```rust
pub struct Adapter { /* opaque */ }

pub struct Event { pub window_id: WindowId, pub window_event: WindowEvent }

pub enum WindowEvent {
    InitialTreeRequested,
    ActionRequested(ActionRequest),
    AccessibilityDeactivated,
}

impl Adapter {
    pub fn with_event_loop_proxy<T: 'static>(
        window: &Window,
        event_loop_proxy: EventLoopProxy<T>,
    ) -> Self { ... }

    pub fn with_direct_handlers(
        window: &Window,
        activation_handler: impl ActivationHandler + Send + 'static,
        action_handler: impl ActionHandler + Send + 'static,
        deactivation_handler: impl DeactivationHandler + Send + 'static,
    ) -> Self { ... }

    pub fn with_mixed_handlers<T: 'static>(
        window: &Window,
        activation_handler: impl ActivationHandler + Send + 'static,
        event_loop_proxy: EventLoopProxy<T>,
    ) -> Self { ... }

    pub fn process_event(&mut self, window: &Window, event: &WinitWindowEvent) { ... }

    pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) { ... }
}
```

All three constructors **panic if the window is already visible** — register the adapter before the window's first show.

**`update_if_active` is the producer's hot path.** The closure is only invoked when an AT is attached; otherwise the call returns immediately and the producer pays nothing. Buiy's `BuiySet::A11yUpdate` system wraps the per-frame tree-building in this closure to gate the cost.

**`process_event` is mandatory** — the adapter learns about window focus, resize, and close from winit events. Skip this and the adapter desyncs from window state.

## Handler traits

```rust
pub trait ActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate>;
}

pub trait ActionHandler {
    fn do_action(&mut self, request: ActionRequest);
}

pub trait DeactivationHandler {
    fn deactivate_accessibility(&mut self);
}
```

`ActivationHandler::request_initial_tree` is called the first time an AT attaches. The producer must return a complete `TreeUpdate` with `tree: Some(...)` and every node in the tree in `nodes`. Returning `None` defers — the AT sees an empty tree until the next push.

`ActionHandler::do_action` receives an `ActionRequest { action, target, data }` and is expected to dispatch the action synchronously or queue it for the producer's main loop. Bevy / Buiy will queue: the request crosses a thread boundary into the main schedule, where a system reads the queue and emits the corresponding Buiy interaction event.

`DeactivationHandler::deactivate_accessibility` is the cleanup hook — the producer can drop cached state until reactivation.

## `accesskit_winit` async runtime requirement (Unix only)

On Linux, `accesskit_unix` is async-runtime-backed (D-Bus operations). `accesskit_winit` propagates the requirement: the producer must enable either the `tokio` or `async-io` Cargo feature on `accesskit_winit`. This forces a runtime dependency that may not otherwise exist in the producer's tree.

Buiy will inherit this. The `async-io` feature is the lighter option (smaller dependency closure, no async-runtime startup cost in non-async producers). Pinning the choice is an open question worth a sub-spec entry.

## Threading model

`accesskit::Node`, `accesskit::TreeUpdate`, and `accesskit::Tree` are plain `Send + Sync` data — safe to construct on any thread and ship to the adapter from any thread.

The `Adapter::update_if_active` call itself may be made from the main thread (Bevy main schedule). On Linux the adapter's internal D-Bus work happens on the chosen async runtime. On Windows and macOS the adapter's internal work is synchronous; UIA / NSAccessibility queries are answered on the WndProc / `NSView` thread, which is the same thread the producer drives the window from when using winit.

The crate-level documentation on `accesskit_winit` characterizes the API as "purely blocking" from the producer's perspective despite the internal async tasks — the producer does not write `async fn` code to interact with AccessKit.

## Lifetime / ownership

- The `accesskit_winit::Adapter` owns the per-platform adapter and the cached tree. The producer holds the `Adapter` instance.
- One `Adapter` per `winit::Window`. Buiy keys this by winit `WindowId` — see [cross-cutting.md §3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md#318-compatibility-and-coexistence).
- The `Tree` metadata struct lives inside the consolidated tree owned by the adapter; the producer constructs `Tree` values on each `TreeUpdate.tree = Some(...)` but does not retain them.
- `Node` instances are short-lived: the producer builds a fresh `Node` for each entity-update in a frame, ships it via `TreeUpdate.nodes`, then drops the local copy. The adapter takes ownership when the update is applied.

## Versioning posture

AccessKit's crate set co-releases on a roughly-monthly cadence (release batches 2026-01-03, 2026-01-15, 2026-01-18, 2026-02-01, 2026-02-25, 2026-03-04, 2026-05-11 in the recent window). Minor-version bumps regularly carry breaking changes — the project does not commit to back-compatible minors on the producer-side `accesskit` crate. The 0.24.0 release notes called out the workspace move to Rust 2024 edition and rust-version 1.85; previous releases bumped through similar inflection points.

**Implication for Buiy.** Buiy's "AccessKit major release between Bevy minors triggers a Buiy patch release" policy ([architecture.md §2.9](../../specs/2026-05-07-buiy-foundation/architecture.md#29-compatibility--policy)) needs to read "minor or major" given the pre-1.0 versioning. A breaking change in `accesskit_consumer` 0.36.0 (May 2026) cascaded to every platform adapter on the same release day — a producer pinning AccessKit needs to update all `accesskit_*` crates together, not piecemeal.

## What Buiy uses from this surface

The minimum producer-side imports for Buiy's a11y plumbing:

```rust
use accesskit::{
    Node, NodeId, Role, Action, ActionRequest, ActionHandler,
    ActivationHandler, DeactivationHandler,
    Tree, TreeUpdate, TreeId, Rect, Live, Toggled, Invalid,
};
use accesskit_winit::{Adapter, Event as A11yEvent, WindowEvent as A11yWindowEvent};
```

Everything else (Color, TextSelection, custom-action machinery, ListStyle, AriaCurrent, AutoComplete, etc.) is reached as Buiy implements the corresponding widget contracts.

## Sources

- `accesskit` 0.24.0 docs.rs: https://docs.rs/accesskit/0.24.0/accesskit/
- `accesskit::Node` accessor catalogue: https://docs.rs/accesskit/0.24.0/accesskit/struct.Node.html
- `accesskit_winit` 0.33.0 docs.rs: https://docs.rs/accesskit_winit/0.33.0/accesskit_winit/
- `common/src/lib.rs` (NodeId, TreeUpdate, Tree, Live, Toggled, Invalid definitions): https://github.com/AccessKit/accesskit/tree/main/common/src
- crates.io release history (versions, dates, co-release pattern): https://crates.io/crates/accesskit
- Buiy spec — accessibility integration: `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md`
- Buiy spec — architecture §2.6, §2.9: `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`
- Sibling: [architecture.md](architecture.md), [tree-model.md](tree-model.md), [platform-adapters.md](platform-adapters.md)
