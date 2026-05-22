**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — internal architecture, tree model, traits, and caching strategy

# Taffy — architecture

Taffy is a layout engine (not a UI framework, not a renderer). It takes a tree of styled nodes and produces a tree of `Layout` rects: position + size in float pixels. It owns no rendering, no text shaping, no input — those are the embedder's job. The internal architecture is shaped by two design pressures: (a) it must be reusable across embedders with their own node trees (Servo, Blitz, Bevy, Slint, Zed/GPUI, Floem, Dioxus/Blitz), and (b) it must be cache-friendly because real UI trees re-layout every frame.

This file is the structural skeleton. See [api.md](api.md) for the public surface, [layout-algorithms.md](layout-algorithms.md) for what each algorithm covers, and [capabilities.md](capabilities.md) for the CSS-parity gap. Siblings on integration / governance / history / ecosystem / critiques / open-problems are written by Agent B.

## 1. Two APIs, one engine

Taffy exposes its functionality at two levels.

**High-level API: `TaffyTree<NodeContext>`.** A ready-to-use tree implementation that owns node storage (`slotmap::SlotMap<DefaultKey, NodeData>`), per-node `Cache`, per-node `Style`, and the parent/child topology (a `ChildrenVec` of `NodeId`s per node). Construct with `TaffyTree::new()`, add nodes via `new_leaf` / `new_with_children`, then call `compute_layout(root, available_space)`. Source: [`src/tree/taffy_tree.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/tree/taffy_tree.rs).

**Low-level API: a set of traits.** An embedder implements `TraversePartialTree` + `LayoutPartialTree` (and the algorithm-specific container traits `LayoutFlexboxContainer` / `LayoutGridContainer` / `LayoutBlockContainer` it cares about) against its own tree storage. Taffy then provides free functions — `compute_flexbox_layout`, `compute_grid_layout`, `compute_block_layout`, `compute_leaf_layout`, `compute_root_layout`, `compute_cached_layout`, `compute_hidden_layout`, `round_layout` — that act on a single node given that trait surface. Source: [`src/tree/traits.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/tree/traits.rs).

The high-level API is itself implemented in terms of the low-level traits — `TaffyTree` impls `LayoutPartialTree`, `LayoutFlexboxContainer`, etc. So there is exactly one algorithm implementation, callable two ways. Servo, Blitz, and Dioxus all use the trait approach to keep Taffy out of their node-storage decisions; this is Buiy's path too (see Buiy [architecture.md § 1](../../specs/2026-05-08-buiy-layout-design/architecture.md#1-bridge-model-buiy--taffy)), though Buiy currently wraps `TaffyTree` directly rather than implementing the traits.

## 2. The trait stack

The five core traits, from least to most capability:

```
TraversePartialTree           — child_ids, child_count, get_child_id (one level)
├── LayoutPartialTree         — get_style + compute_child_layout + set_unrounded_layout
│   └── (algorithm subtraits) — LayoutFlexboxContainer, LayoutGridContainer, LayoutBlockContainer
├── TraverseTree              — marker: child traversal works recursively
│   ├── RoundTree             — get_unrounded_layout + set_final_layout (for round_layout)
│   └── PrintTree             — get_debug_label + get_final_layout (for print_tree)
└── CacheTree                 — cache_get / cache_store / cache_clear (decoupled cache backend)
```

`LayoutPartialTree` is the trait Taffy's algorithms call back into for *children* during the recursive descent. An embedder implementing it gets all four layout algorithms for free, given they implement the per-algorithm container subtraits as needed. The algorithms only require `TraversePartialTree` (single-level child access) — they never recurse outside Taffy itself; they call `compute_child_layout(child_id, inputs)` and Taffy handles the recursion. This is why an embedder's tree can be arbitrarily shaped (vec-arena, owned-children, raw-pointer index) and still plug in.

`CacheTree` was split out of `LayoutPartialTree` in **0.7.0** (was `cache_mut` before) so that callers who don't want Taffy's caching (or want their own) can opt out. The default `TaffyTree` implements it; embedders who do not implement it lose Taffy's frame-to-frame caching.

## 3. Storage model (`TaffyTree`)

```rust
pub struct TaffyTree<NodeContext = ()> {
    nodes: SlotMap<DefaultKey, NodeData>,
    children: SlotMap<DefaultKey, ChildrenVec<NodeId>>,
    parents: SlotMap<DefaultKey, Option<NodeId>>,
    node_context_data: SecondaryMap<DefaultKey, NodeContext>,
    config: TaffyConfig,                // {use_rounding: bool}
}

struct NodeData {
    style: Style,
    unrounded_layout: Layout,
    final_layout: Layout,
    has_context: bool,
    cache: Cache,
    detailed_layout_info: DetailedLayoutInfo,  // feature-gated
}
```

Source: [`src/tree/taffy_tree.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/tree/taffy_tree.rs).

**Notes:**

- `NodeId` is a `u64`-wrapping type. It is *not* `slotmap::DefaultKey`; the `From` conversions handle the boundary. This was reworked in 0.4 specifically to make the low-level API easier to implement against non-slotmap storage.
- `unrounded_layout` and `final_layout` are stored separately. The reason is rounding-stability: rounding an already-rounded value when an ancestor moves a fractional pixel produces visible jitter; see [issue #501](https://github.com/DioxusLabs/taffy/issues/501).
- `NodeContext` is the per-node payload an embedder attaches (used by measure functions for text/image leaves). For a no-context tree this is `()`.

## 4. Cache strategy

Each node carries its own `Cache` (in `NodeData.cache` for `TaffyTree`, externalized via `CacheTree` for embedders). The cache stores `(LayoutInput, LayoutOutput)` pairs keyed by the input — known dimensions, available space, run mode (PerformLayout vs ComputeSize), sizing mode, vertical-margins-collapse flag. On `compute_child_layout`, Taffy first asks `cache_get` for a matching entry; on hit, it returns the cached `LayoutOutput` without descending. On miss, it computes, then `cache_store`s.

**Dirty propagation.** `TaffyTree::mark_dirty(node_id)` walks up the parent chain calling `NodeData::mark_dirty()` (which clears the cache). `set_style` and `set_children` call `mark_dirty` on the affected node implicitly. This is why an embedder reusing the same `TaffyTree` across frames pays only for the entities whose style or children changed — and why Buiy keeps the `TaffyTree` as a long-lived `NonSendResource` ([Buiy architecture.md § 1.1](../../specs/2026-05-08-buiy-layout-design/architecture.md#11-layouttree--the-bridge-state)).

**Practical performance.** Per the project's README benchmarks (M1 Pro, criterion), Taffy 0.4 (commit `71027a8`) sits within 10–30% of Yoga (commit `ba27f9d`) on most workloads, faster on some, slower on others. Specific numbers: 100k-node `huge nested` at depth 5 = **38.6 ms** Taffy vs **45.8 ms** Yoga; 100k-node `wide` at depth 1 = **247 ms** Taffy vs **136 ms** Yoga (Taffy is slower at extreme width). See README "Benchmarks" section. The numbers measure layout only; they exclude tree construction and text measurement. For the much harder full-WPT correctness comparison, Taffy implements Block + Flexbox + Grid; Yoga implements Flexbox only.

## 5. Available space and the size-resolution model

Taffy resolves sizes against an `AvailableSpace` value that itself can be `Definite(f32)`, `MinContent`, or `MaxContent`. Compute calls pass `available_space: Size<AvailableSpace>` (width and height independently); leaf nodes resolve content-sized dimensions by querying this against the measure function. `Size::MAX_CONTENT` (both axes `MaxContent`) is the typical root call when the container has no fixed size; passing `Definite(viewport_width)` is the typical browser-style call. Source: [`src/style/available_space.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/style/available_space.rs).

Intrinsic-sizing keywords (`min-content` / `max-content` / `fit-content` as `Dimension` values) are **not yet shipped** — see [issue #751](https://github.com/DioxusLabs/taffy/issues/751). `AvailableSpace` exists as an algorithmic input, but you cannot set `style.size.width = Dimension::MinContent`. This is the gap [capabilities.md](capabilities.md) covers.

## 6. The four algorithms

Each layout algorithm is its own module under `src/compute/`:

- **Flexbox** (`flexbox.rs`) — gated by `feature = "flexbox"`. The default `Display::Flex` when the `flexbox` feature is enabled (which it is by default).
- **CSS Grid** (`grid/`) — gated by `feature = "grid"`. Full CSS Grid Level 1 + a slice of Level 2 (named lines, areas). Pulls in the `grid` external crate dep.
- **Block** (`block.rs`) — gated by `feature = "block_layout"`. Added in 0.4 (closes [issue #405](https://github.com/DioxusLabs/taffy/issues/405)). Block containers with block-level children only; no inline layout, no inline-block.
- **Float** (`float.rs`, `block.rs` integration) — gated by `feature = "float_layout"`, a sub-feature of block. Added in **0.10.0**. Provides `Float::Left` / `Float::Right` / `Float::None` and `Clear::Left` / `Right` / `Both` / `None`. Implemented via a `FloatContext` shared across a `BlockContext` (i.e. one block formatting context's floats). This is a genuine CSS-spec-tracking implementation, not a stub.

All algorithms are also exposed as standalone functions (`compute_flexbox_layout(tree, node, inputs)` etc.) so an embedder using the low-level API can dispatch on `Display` themselves.

Algorithm dispatch in `TaffyTree::compute_child_layout` is by `style.display`:

```
Display::Flex   → compute_flexbox_layout
Display::Grid   → compute_grid_layout
Display::Block  → compute_block_layout
Display::None   → compute_hidden_layout
(leaf, any)     → compute_leaf_layout
```

No `Inline`, `InlineBlock`, `InlineFlex`, `InlineGrid`, `Table`, `TableRow`, `ListItem`, `Ruby`, `Contents`, or `FlowRoot` variants exist on the `Display` enum. Embedders that need them (Buiy needs `Table`, `ListItem`, `Contents`, `FlowRoot` per its [display-and-positioning](../../specs/2026-05-08-buiy-layout-design/architecture.md) spec) must either layer them above Taffy or set `Display::Block` and accept the difference.

## 7. What Taffy does *not* model

These are absent from the internal architecture by design, not omitted from this writeup:

- **Painting / drawing.** No colors, no borders-as-pixels, no shadows. `Layout` is rect-only.
- **Text shaping.** Leaf measure functions are the embedder's responsibility; Taffy hands them `Size<Option<f32>>` (known dims) + `Size<AvailableSpace>` and trusts the return.
- **Hit testing.** The embedder takes `Layout` and does its own picking.
- **Inline formatting context.** No line breaking, no `vertical-align`, no inline boxes. A leaf is a leaf; if it's text, the measure function returns one rect.
- **Containing-block computation for absolute positioning.** Taffy handles abspos children within their formatting context, but the "nearest positioned ancestor" walk for `Position::Absolute` is mostly local (it uses the parent formatting context's box as the containing block, not a CSS-spec-conformant walk past static ancestors).
- **Stacking contexts.** No z-index, no paint order.
- **Compositing / clipping.** `Overflow` affects *layout* (automatic minimum size, scrollbar reservation) but not rendering.
- **Float painting.** `Float` affects layout (line-box shortening of following inline content); it does not produce a "this rect was painted as a float" annotation.

Buiy fills several of these gaps as post-Taffy passes ([Buiy architecture.md § 3](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline) sub-passes 6a–6d: sticky, table, multi-column, anchor). Container queries are an above-Taffy concern (capped 2× re-layout).

## 8. Concurrency

`Style` and `TaffyTree` are `!Send + !Sync` since **0.8.0** because `Dimension`, `LengthPercentage`, `LengthPercentageAuto`, `MinTrackSizingFunction`, and `MaxTrackSizingFunction` switched from enums to tagged-pointer `CompactLength` (this is what enables `calc()` to carry an opaque pointer-sized handle). The `calc` feature is on by default; even with `calc` off, the representation is still pointer-shaped. This is the documented reason Buiy stores `LayoutTree` as a `NonSendResource` rather than a `Resource` — see [Buiy architecture.md § 1.1](../../specs/2026-05-08-buiy-layout-design/architecture.md#11-layouttree--the-bridge-state). Layout is inherently a sequential pass, so the `!Send` restriction is not a perf problem.

## 9. Versioning posture

`taffy` is pre-1.0. The 0.x cadence is roughly quarterly. Each minor bump has shipped meaningful API breakage (the 0.4 measure-function rework, the 0.6 trait split, the 0.7 `CacheTree` extraction, the 0.8 `CompactLength` tagged-pointer migration, the 0.9 `CheapCloneStr` generic on `Style`, the 0.10 float + direction additions). Embedders are expected to read the CHANGELOG. The MSRV as of 0.10 is **1.71**. License is **MIT** (see workspace `Cargo.toml`).

Three experimental versions are currently in flight on crates.io: `0.11.0-experimental-cache-fix.3`, `0.10.2-experimental-cache-fix.2`, `0.10.1-experimental-cache-fix.1`. These are scratch-cache-correctness work; production embedders pin to `0.10.1`. See [history.md](history.md) (sibling, Agent B) for chronology.

## Sources

- Taffy repo: https://github.com/DioxusLabs/taffy
- Workspace `Cargo.toml` (verified version 0.10.1, license MIT, MSRV 1.71): https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml
- `src/tree/taffy_tree.rs` (NodeData, TaffyTree, TaffyError): https://github.com/DioxusLabs/taffy/blob/main/src/tree/taffy_tree.rs
- `src/tree/traits.rs` (LayoutPartialTree, TraversePartialTree, CacheTree): https://github.com/DioxusLabs/taffy/blob/main/src/tree/traits.rs
- `src/lib.rs` (module structure, prelude, feature flags): https://github.com/DioxusLabs/taffy/blob/main/src/lib.rs
- `src/style/mod.rs` (Style struct, Display, Overflow, Position, Direction): https://github.com/DioxusLabs/taffy/blob/main/src/style/mod.rs
- CHANGELOG (verified Block in 0.4, CacheTree split in 0.7, calc/`CompactLength` in 0.8, named grid lines/areas in 0.9, float + direction in 0.10): https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Issue #501 (rounded-vs-unrounded layout separation): https://github.com/DioxusLabs/taffy/issues/501
- Issue #405 (Block layout): https://github.com/DioxusLabs/taffy/issues/405 (closed)
- Issue #751 (intrinsic sizing keywords): https://github.com/DioxusLabs/taffy/issues/751
- Issue #639 (WPT test suite): https://github.com/DioxusLabs/taffy/issues/639 (closed)
- Roadmap issue #345: https://github.com/DioxusLabs/taffy/issues/345
- Buiy layout architecture: [`docs/specs/2026-05-08-buiy-layout-design/architecture.md`](../../specs/2026-05-08-buiy-layout-design/architecture.md)
