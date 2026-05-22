**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — architectural model: producer/adapter split, push-based tree protocol, activation gate

## Two roles, one tree

AccessKit factors a UI toolkit's accessibility story into two roles around a single shared data structure (the tree):

- **Producer** (sometimes "provider" or "toolkit-side") — the UI toolkit. Builds nodes, owns identities, decides what is in the tree, pushes `TreeUpdate`s. Buiy is a producer. Bevy's `bevy_a11y` is a producer. egui, Slint, Xilem, and Freya are all producers; Iced is queueing integration (draft PRs only, see README.md § Adopters).
- **Adapter** — the per-platform code that takes the tree from the producer and exposes it through the local OS accessibility API (UIA on Windows, NSAccessibility on macOS, AT-SPI on Linux, etc.). Adapters live in `accesskit_windows`, `accesskit_macos`, `accesskit_unix`, `accesskit_android`, `accesskit_ios`, with `accesskit_winit` as a multiplexing helper. `accesskit_consumer` is the internal library *adapters* use to walk/diff/cache the tree — despite the name, it sits on the adapter side of the boundary. Application code never depends on `accesskit_consumer` directly.

The boundary is a data protocol (the `TreeUpdate` schema), not a Rust trait. That keeps the producer side language-agnostic: AccessKit ships C and Python bindings against the same schema, and the schema is reflectable to JSON Schema and Protocol Buffers.

> **Nomenclature warning.** The terms "consumer" (`accesskit_consumer` crate) and "producer" (toolkit) refer to *who consumes/produces the tree*, not to who consumes/produces accessibility information at the end user's ears. From the OS / AT side, the producer-of-the-tree (your toolkit) is the *source* of accessibility info; from the toolkit-author side, the OS adapter is the *consumer* of the tree. Buiy is on the producer/toolkit side throughout this corpus.

## The push-based protocol

AccessKit borrows Chromium's accessibility-tree model:

1. On activation, the producer builds a **full initial `TreeUpdate`** — every node in the tree, plus the `Tree` metadata struct (root NodeId, toolkit name and version).
2. Thereafter, the producer pushes **diff-style `TreeUpdate`s** containing only the nodes that changed since the last update. Unchanged nodes are not re-sent.
3. The adapter applies updates, caches the consolidated tree, and exposes views into it through the platform AT API on demand.

This is the inverse of the Cocoa / UIA "pull" model where the AT walks the producer's UI on every query. The push model assumes the producer can detect changes cheaply (true for retained-mode toolkits with change-tracked state) and lets the adapter answer AT queries from cached data with no toolkit involvement. For an immediate-mode toolkit, the producer re-derives the tree per frame and pushes the diff.

### TreeUpdate shape

From `common/src/lib.rs` at the 0.24 line:

```rust
pub struct TreeUpdate {
    pub nodes: Vec<(NodeId, Node)>,
    pub tree: Option<Tree>,
    pub tree_id: TreeId,
    pub focus: NodeId,
}
```

`nodes` is the per-update payload (changed nodes only after initial). `tree: Option<Tree>` is `Some` only on the initial update or when root-level metadata changes (toolkit name/version updates). `focus` carries the current focus on every update — the focused node is part of tree state, not a separate channel. `tree_id` identifies which tree the update belongs to (matters when one process hosts multiple windows / multiple trees).

A `Node` carries its role, name, value, state flags, geometry, relations, and supported actions. See [tree-model.md](tree-model.md) for the full field surface.

## The activation gate

Building and pushing the tree is not free — for a 500-widget UI a `TreeUpdate` is non-trivial work. AccessKit avoids paying that cost when nothing is observing.

Adapters notify the producer when an AT actually attaches. The mechanism varies per platform (UIA broker probe, NSAccessibility selector arrival, AT-SPI D-Bus listener registration). On the producer side, this manifests as the `ActivationHandler` callback being invoked, and `accesskit_winit` exposes the activated state through its `Adapter::update_if_active(...)` method:

- Before activation: `update_if_active` short-circuits, the closure that builds the `TreeUpdate` never runs, and the producer skips the cost.
- After activation: the closure runs and the result is forwarded to the platform adapter.

Buiy's spec calls this out explicitly: the AccessKit tree is "built lazily (gated on `AccessibilityRequested`)" — meaning Buiy's per-frame a11y-update system reads the activation state, and the `BuiySet::A11yUpdate` system set is essentially a no-op until an AT attaches. Once active, the system runs every frame the focus tree changes.

The flip side: if the producer pushes updates *before* activation, AccessKit accepts them and caches the latest tree — useful so the producer can publish a baseline early without coordinating "is it active yet."

## The handler triple

The producer registers three handler traits with the adapter:

- **`ActivationHandler`** — called when an AT attaches. Returns the *initial* `TreeUpdate` so the adapter has something to expose right away. Producer typically caches this so the next `update_if_active` call doesn't redo the work.
- **`ActionHandler`** — called when the AT asks to perform an action on the producer's behalf (`Click`, `Focus`, `SetValue`, `ScrollIntoView`, etc.). The handler receives an `ActionRequest { action, target: NodeId, data }` and routes it to whatever entity / widget owns that `NodeId`. See [tree-model.md § Actions](tree-model.md) for the full action vocabulary.
- **`DeactivationHandler`** — called when the AT detaches. The producer can drop the cached tree, stop pushing updates, and free associated resources until the next activation.

`accesskit_winit` provides `WinitActivationHandler`, `WinitActionHandler`, and `WinitDeactivationHandler` implementations that route through winit's `EventLoopProxy`. The `Adapter::with_event_loop_proxy` constructor uses these; `Adapter::with_direct_handlers` lets the producer supply its own implementations (useful when the producer's main loop is not winit's event loop — Bevy's main schedule, for example).

## Per-frame loop on the producer

The Buiy spec compresses the per-frame producer responsibilities into the `BuiySet::A11yUpdate` system, ordered after picking and before render in the `BuiyPlugin` sub-plugin order ([architecture.md §2.8](../../specs/2026-05-07-buiy-foundation/architecture.md#28-module-organization)). Inside that system:

1. Read activation state. If not active, return.
2. Walk Buiy's change-tracked entity components (`Role`, `Name`, `Value`, `Focused`, etc.) and collect dirty entities.
3. For each dirty entity, build the corresponding `accesskit::Node` from Buiy components.
4. Compose a `TreeUpdate { nodes, tree: None, tree_id, focus }`.
5. Hand the update to the per-window `accesskit_winit::Adapter::update_if_active(|| treeupdate)`.

Initial activation is the same path with `tree: Some(Tree { root, toolkit_name: Some("Buiy"), toolkit_version: Some(env!("CARGO_PKG_VERSION")) })` and every node in the world payloaded into `nodes`.

## Cross-platform tree invariant

The same `Node` struct expresses the producer's intent on every platform. AccessKit's per-platform adapters translate the unified `Node` into platform-native data structures:

- `Role::Button` becomes UIA `ControlType.Button`, NSAccessibility `kAXButtonRole`, AT-SPI `ROLE_PUSH_BUTTON`, Android `RoleDescription` plus an inferred control hint, iOS `UIAccessibilityTraitButton`.
- `TreeUpdate.focus: NodeId` becomes UIA `IsKeyboardFocusable` + focus event, NSAccessibility `kAXFocusedAttribute`, AT-SPI `STATE_FOCUSED`, etc.
- Geometry `set_bounds(Rect)` takes **window-relative logical coordinates**. The adapter applies the window's screen position + DPI scale transform when publishing to the platform AT. Producers must not bake screen position into the tree at push time — that would invalidate the entire tree on every window move. (See [tree-model.md § Coordinate spaces](tree-model.md).)

Because the producer-side `Node` shape is invariant, a producer that builds a correct tree gets correct AT behavior on every supported platform without per-platform conditionals in producer code. Per-platform divergences live entirely inside the adapter crates — which is what makes AccessKit useful as a "write once" target for toolkit authors.

## One tree per window per process

AccessKit enforces a **single tree per `accesskit_winit::Adapter` per window**. A toolkit cannot interleave two producers' subtrees into one window's tree without a coordinator; this is why Buiy's coexistence rule with `bevy_ui` is per-window ([cross-cutting.md §3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md#318-compatibility-and-coexistence)): one stack owns the adapter for a given window, the other stack is absent on that window.

Multi-window apps allocate one adapter per window, keyed by winit `WindowId`. `NodeId`s are scoped per-tree (per-window), not per-process — different windows may have overlapping `NodeId` numeric values without conflict.

## Sources

- AccessKit README: https://github.com/AccessKit/accesskit/blob/main/README.md
- `common/src/lib.rs` (TreeUpdate, Tree, NodeId, Role, Action): https://github.com/AccessKit/accesskit/tree/main/common/src
- `consumer/README.md` (adapter-side library, "not meant for direct application integration"): https://github.com/AccessKit/accesskit/blob/main/consumer/README.md
- `accesskit_winit` 0.33.0 docs: https://docs.rs/accesskit_winit/0.33.0/accesskit_winit/
- Buiy foundation spec — accessibility: `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md`
- Buiy foundation spec — architecture §2.6: `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`
- Buiy cross-cutting §3.18: `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/cross-cutting.md`
- Sibling: [tree-model.md](tree-model.md), [platform-adapters.md](platform-adapters.md), [api.md](api.md)
