**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — how embedders integrate it (Bevy, Dioxus/Blitz, Slint, GPUI, Buiy)

# Taffy — integration

Taffy is *only* a layout engine. It computes rectangles. An embedder owns: node storage, dirty propagation policy, the translation from its style model to `taffy::Style`, the measure functions for text/image leaves, and the consumer of `Layout` output. This file enumerates the integration surface and how the major shipping embedders use it.

For trait definitions and the high-vs-low-level API split see [architecture.md](architecture.md). For algorithm coverage see [layout-algorithms.md](layout-algorithms.md). For the Buiy-specific bridge design see [Buiy architecture.md](../../specs/2026-05-08-buiy-layout-design/architecture.md).

## 1. Two embedding shapes

Both are first-class; choice is about who owns the node tree.

**Wrap `TaffyTree`.** Construct a long-lived `TaffyTree<NodeContext>`, keep an `EmbedderId -> taffy::NodeId` map, call `set_style` / `set_children` / `new_leaf` / `remove` as the embedder's tree mutates, then `compute_layout(root, available_space)` and `tree.layout(node_id)` to read results. This is the path Bevy UI takes today and the path Buiy takes in Phase 0.

**Implement the traits.** Implement `TraversePartialTree` (child enumeration) + `LayoutPartialTree` (style + child recursion) against the embedder's existing node arena, and call the free `compute_*_layout` functions. Add `CacheTree` if you want Taffy's caching against your storage. This is what Servo, Blitz, GPUI, and Slint do — none of them keep two copies of their node tree.

The trait approach trades implementation cost for ownership: no parallel storage, no `Entity -> NodeId` map, no GC contract. Buiy's design ([Buiy architecture.md § 1.1](../../specs/2026-05-08-buiy-layout-design/architecture.md#11-layouttree--the-bridge-state)) wraps `TaffyTree` because Bevy ECS storage doesn't fit Taffy's recursive child traversal cleanly — `ChildOf` / `Children` are queryable but not arena-indexable from inside a Taffy callback. The cost is the `LayoutTree` GC pass (step 0) and the `HashMap<Entity, TaffyNodeId>`.

## 2. Bevy UI's integration

Pinned to `taffy = "0.10"` in current main; `taffy = "0.9"` in `bevy_ui 0.18.1` ([Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml)). Bevy bumps Taffy every minor; the upgrades regularly carry breaking changes (e.g. [PR #15844](https://github.com/bevyengine/bevy/pull/15844) upgrade to 0.6 added `box_sizing`).

Layout source lives at `crates/bevy_ui/src/layout/{mod.rs, ui_surface.rs, convert.rs, debug.rs}`:

- **`UiSurface`** — the `ResMut<UiSurface>` resource that owns the `TaffyTree` and the `Entity -> TaffyNodeId` map. Methods: `upsert_node`, `try_update_measure`, `set_camera_children`, `compute_layout`, `get_layout`.
- **`convert.rs`** — `from_node(node: &Node, context: &LayoutContext) -> taffy::Style`. Maps Bevy's `Node` (one mega-struct of layout fields) to Taffy's `Style`. `LayoutContext` carries `scale_factor` and `physical_size` so `Val::Vw` / `Val::Vh` resolve here, not in Taffy.
- **`ui_layout_system`** — one giant `pub fn` that (1) syncs `Node` changes to `UiSurface`, (2) reconciles parent/child links, (3) calls `compute_layout`, (4) walks results into `ComputedNode` + `UiGlobalTransform` via `update_uinode_geometry_recursive`.

Bevy stores layout *output* in `ComputedNode` (rounded `Vec2` size, computed inset, computed border, calculated size, content size). Stacking + clipping consume `ComputedNode` in downstream systems.

What Bevy does *not* do:

- Implement Taffy's low-level traits. `UiSurface` wraps `TaffyTree` and never exposes its internals.
- Surface every Taffy feature on `Node`. `float` and `clear` (shipped in Taffy 0.10) are not on `Node` as of 0.18.1; `direction` is exposed but no `writing-mode`.
- Use container queries or anchor positioning. Both unimplemented in Taffy, both unimplemented in Bevy; apps roll their own with `ComputedNode` reads.

See [bevy-ui/layout.md](../bevy-ui/layout.md) for the full bevy_ui surface.

## 3. Dioxus / Blitz integration

Dioxus the framework does not depend on Taffy directly — `dioxus-core`'s virtualdom is layout-agnostic. The layout-bearing crates are:

- **`dioxus-native-core`** ([Cargo.toml](https://github.com/DioxusLabs/dioxus/blob/main/packages/native-core/Cargo.toml)) — at `taffy ^0.3.12` historically; this crate is mostly maintenance-mode now that Blitz is the recommended native path.
- **`blitz-dom`** ([Blitz Cargo.toml](https://github.com/DioxusLabs/blitz/blob/main/Cargo.toml)) — at `taffy = "=0.11.0-experimental-cache-fix.3"`. Blitz pins an experimental version because it implements the trait surface (not the wrapper) and benefits from cache-correctness work landing on the experimental branch.

Blitz is the load-bearing Taffy consumer in the Dioxus ecosystem. From its README: *"`blitz-dom`: The core DOM abstraction that includes style resolution, layout and event handling [...] Uses: Stylo (CSS parsing/resolution), Taffy (box-level layout)."* Blitz implements the low-level traits against its own DOM arena (`blitz-dom`'s node tree, not a `TaffyTree`), then layers Stylo for CSS parsing and Vello for paint. The `dioxus-native` crate is a thin shim over Blitz that renders Dioxus VirtualDoms with Blitz.

`stylo_taffy` (`taffy = "^0.9"` on crates.io) is the Servo Stylo → Taffy bridge crate. Blitz uses it; servo-layout uses it (Servo replaced its own layout engine with Taffy via `servo-layout 0.10`).

## 4. Iced does NOT use Taffy

The Taffy README's "Iced uses Taffy" line and several third-party comparison posts are stale or wrong. Verifying:

- `iced/Cargo.toml` at master has no `taffy` dependency ([master Cargo.toml](https://github.com/iced-rs/iced/blob/master/Cargo.toml)).
- `iced_core/src/layout.rs` defines its own `Node` + `Layout` types ([layout.rs](https://github.com/iced-rs/iced/blob/master/core/src/layout.rs)).
- Iced ships its own `iced::widget::Widget::layout` trait — each widget computes its own size given `Limits` (min/max box).

This is one of the brief-corrections for the pre-amble: Iced has a per-widget retained-mode layout protocol, not a Taffy-style global recompute. The mental model is different (widget-defined `Layout` returns vs Taffy's tree-walking algorithms) and the gap matters when comparing approaches.

## 5. Other shipping integrations

Top reverse-dependencies on crates.io (verified [taffy/reverse_dependencies](https://crates.io/crates/taffy/reverse_dependencies)):

- **`i-slint-core ^0.9`** — Slint's declarative GUI toolkit. Uses Taffy for `Display::Flex` items; native layout primitives ("Grid", "Column", "Row" markup) compile to Taffy.
- **`gpui =0.9.0`** — Zed's UI framework. Pinned exact-version. Uses the trait surface against GPUI's own element tree.
- **`floem ^0.4`** — UI framework powering Lapce. Uses Taffy via the trait surface.
- **`takumi ^0.10`** — React-to-image server-side renderer.
- **`iocraft ^0.5.2`** — terminal UI; uses Taffy in character cells.
- **`servo-layout ^0.10`** — Servo's layout engine post-replacement.
- **`blitz-dom ^0.9`**, **`blitz-paint ^0.9`**, **`blitz-renderer-vello ^0.8`** — see § 3 above.
- **`stylo_taffy ^0.9`** — Stylo style → Taffy style bridge.
- **`azul-layout ^0.9.1`** — Azul's layout module.
- **`inlyne ^0.3.19`** — markdown viewer; uses Taffy 0.3 era.
- **`egui_taffy ^0.9.2`** — community crate marrying egui's immediate-mode widgets to a Taffy-laid-out outer tree.

This is a meaningful list: web (Servo, Blitz), one production game engine (Bevy), two production editors (Zed via GPUI, Lapce via Floem), several declarative toolkits (Slint, Azul, Dioxus/Blitz), and a long tail of niche renderers (terminal, image, markdown).

## 6. Embedder choices the API forces

**Storage.** `TaffyTree` uses `slotmap` internally. Embedders wrapping it pay for one map. Embedders implementing the traits choose: vec-arena (Slint), hash-map (some prototypes), pointer-to-owned-children (Blitz). Taffy is unopinionated.

**Dirty propagation.** Two valid stances:

1. *Manual `mark_dirty`* — the embedder tracks what changed and calls `tree.mark_dirty(node)` precisely. `TaffyTree::set_style` and `set_children` call `mark_dirty` internally, so the typical path is "stop calling them when nothing changed." Bevy does this via change detection on `Node`.
2. *Full-rebuild* — the embedder reconstructs the entire `TaffyTree` every frame. Simpler, defeats Taffy's cache. Used by some prototype embedders; production embedders all do (1).

Buiy uses (1) via Bevy's `Changed<Component>` on each of 15 decomposed layout components ([Buiy architecture.md § 1.2](../../specs/2026-05-08-buiy-layout-design/architecture.md#12-translation-layer)).

**Measure functions.** Text and image leaves need to ask the embedder "how wide are you given this available space?" Taffy passes `Size<Option<f32>>` (known dims) + `Size<AvailableSpace>` to a `MeasureFunc` per node. The embedder returns `Size<f32>`. For Buiy this is the cosmic-text bridge (a separate spec); for Bevy this is text + image; for Blitz this is paragraph shaping via Skrifa.

**Round vs unrounded.** Taffy stores both `unrounded_layout` and `final_layout` per node ([architecture.md § 3](architecture.md#3-storage-model-taffytree)). Embedders that want stable rounding under animation read `final_layout`; embedders that need fractional pixels (e.g. for animation interpolation midway) read `unrounded_layout`. Bevy uses `final_layout` exclusively; Buiy will follow.

## 7. Buiy's integration shape

Per [Buiy architecture.md](../../specs/2026-05-08-buiy-layout-design/architecture.md), in order of execution per frame:

1. **`RemovedNodesGc`** — drop despawned entities from the `LayoutTree`'s `Entity → NodeId` map; call `tree.remove(node_id)` and tolerate `Err(NotFound)` for parent-then-child despawn ordering.
2. **`SyncStyles`** — for entities with `Changed<BoxModel | Display | Position | Anchor | FlexParams | FlexItem | GridParams | GridItem | Container | WritingMode | Overflow | Scroll | Stacking | Transform | Containment | MultiColumn | Children | ChildOf>`, build a `taffy::Style` from the decomposed components via `style_to_taffy` and call `tree.set_style(node_id, style)`. Same pass calls `tree.set_children(node_id, &child_ids)` for parents whose hierarchy changed.
3. **`CqActivate`** — set or clear container-query marker components from last frame's resolved sizes (initial pass).
4. **`TaffyCompute`** — call `tree.compute_layout(root, available_space)` for each root.
5. **`CqFlipCheck`** — re-evaluate `@container` rules against the freshly-computed sizes; if any flipped, re-run 1 + 4 once more (capped 2×).
6. **`PostTaffyOverrides`** — composed of four sub-passes that share a `Commands` buffer (sticky, table, multi-column, anchor); these run after Taffy because Taffy doesn't model them ([Buiy architecture.md § 3](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline)).
7. **`WriteResolvedLayout`** — copy `tree.layout(node_id)` into `ResolvedLayout` on each entity.

The decomposed component graph — `BoxModel`, `Display`, `Position`, `Anchor`, `FlexParams`, `FlexItem`, `GridParams`, `GridItem`, `Container`, `WritingMode`, `Overflow`, `Scroll`, `Stacking`, `Transform`, `Containment`, `MultiColumn` — is Buiy's, not Taffy's. `style_to_taffy` collapses them into the single `taffy::Style` Taffy expects. This is the "Buiy stores `Style` on entities, syncs to `TaffyTree` per frame on change" pattern: one `taffy::Style` per entity, rebuilt only when one of the decomposed components carries `Changed<_>`.

The `LayoutTree` lives as a `NonSendResource` ([Buiy architecture.md § 1.1](../../specs/2026-05-08-buiy-layout-design/architecture.md#11-layouttree--the-bridge-state)) because `Style` is `!Send + !Sync` since Taffy 0.8 (see [architecture.md § 8](architecture.md#8-concurrency)). Layout is sequential anyway, so the restriction is free.

## 8. Hazards visible only after integration

Things that bite embedders, distilled from issue tracker churn:

- **Stale cache after measure-function context changes.** If the embedder's measure function depends on state outside Taffy (font sets, image decode), Taffy can't know to invalidate. Embedders must `mark_dirty` themselves when measure-function inputs change. Buiy hits this when cosmic-text shapes a font that wasn't available last frame.
- **`Style.size = Dimension::Length(0.0)` vs `Dimension::Auto`.** `Auto` lets the algorithm pick; `Length(0.0)` collapses the node. Bug source on ports — the brief "set width to zero" usually means `Auto` and the embedder must default accordingly.
- **`children` ordering.** Taffy's `set_children(parent, &[children])` is order-significant (laid out in slice order). Bevy's `Children` component is ordered; the bridge must preserve the order, not sort.
- **`NodeId` is not `Entity`.** They're different types and the embedder must own the map. `tree.remove` only frees one direction; the map needs explicit cleanup. This is the `LayoutTree` GC contract.
- **Experimental versions are pinned hard.** `=0.11.0-experimental-cache-fix.3` (exactly) — semver-loose pins on experimental versions are a footgun. Blitz pins exact; production embedders pin to `0.10.1` until experimental graduates.

## Sources

- Taffy README (integration users list): https://github.com/DioxusLabs/taffy/blob/main/README.md
- Taffy reverse-dependencies on crates.io: https://crates.io/crates/taffy/reverse_dependencies
- Bevy UI Cargo.toml (taffy 0.10 pin): https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml
- Bevy UI layout source: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui/src/layout
- Bevy UI v0.18.1 Cargo.toml (taffy 0.9 pin): https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_ui/Cargo.toml
- Bevy PR #15844 (upgrade to Taffy 0.6, box_sizing): https://github.com/bevyengine/bevy/pull/15844
- Bevy PR #6743 (upgrade to Taffy 0.2): https://github.com/bevyengine/bevy/pull/6743
- Blitz Cargo.toml (taffy =0.11.0-experimental-cache-fix.3): https://github.com/DioxusLabs/blitz/blob/main/Cargo.toml
- Blitz README (Taffy as box-level layout): https://github.com/DioxusLabs/blitz
- Iced Cargo.toml (no taffy dependency): https://github.com/iced-rs/iced/blob/master/Cargo.toml
- Iced layout primitive: https://github.com/iced-rs/iced/blob/master/core/src/layout.rs
- Buiy layout architecture: [`docs/specs/2026-05-08-buiy-layout-design/architecture.md`](../../specs/2026-05-08-buiy-layout-design/architecture.md)
- Buiy bevy-ui prior-art layout: [`docs/prior-art/bevy-ui/layout.md`](../bevy-ui/layout.md)
- Sibling: [architecture.md](architecture.md), [layout-algorithms.md](layout-algorithms.md), [open-problems.md](open-problems.md)
