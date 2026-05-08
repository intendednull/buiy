# Layout architecture

**Parent:** [README.md](README.md)

This file defines the structural skeleton every other child relies on: the bridge between Buiy components and Taffy, the public API shape, the system pipeline order, the data lifecycle, and the error model. Every other file in this spec assumes the contracts here hold.

## 1. Bridge model: Buiy ↔ Taffy

Taffy is the layout engine. Buiy *translates* its decomposed component graph into a `taffy::TaffyTree`, calls `compute_layout` from the roots, then writes Taffy's results back to Bevy entities as `ResolvedLayout`.

The bridge is one-directional. Buiy never patches Taffy in place; features Taffy lacks are layered as Buiy passes wrapping Taffy (see [§ 3 System pipeline](#3-system-pipeline)).

### 1.1 `LayoutTree` — the bridge state

```rust
#[derive(Default)]
pub struct LayoutTree {
    tree: TaffyTree<()>,
    by_entity: HashMap<Entity, TaffyNodeId>,
}
```

Stored as a `NonSendResource`. Lifetime: app-long. Reused frame-to-frame so Taffy's internal cache stays warm.

**Why `NonSendResource`?** Taffy 0.10 packs every `Dimension` into a tagged pointer (`*const ()`) regardless of whether the `calc` feature is enabled. `TaffyTree` is therefore `!Send + !Sync`. Layout is inherently a single-threaded pass over the tree, so `NonSendResource` is both correct and `unsafe`-free. (This is the same rationale documented in `crates/buiy_core/src/layout.rs`.)

### 1.2 Translation layer

A free function `style_to_taffy(components: BundleView) -> TaffyStyle` collects every layout-relevant decomposed component on an entity into a single `taffy::Style`. It runs every frame for entities whose layout components changed (per Bevy change detection); on unchanged entities it skips the rebuild. Per-entity translation cost is `O(properties)`; per-frame cost is `O(changed entities × properties)`.

Translation is a pure function. Taffy's compute step is the only thing that mutates the tree.

#### Change-detection trigger set

`SyncStyles` (step 1) re-translates an entity when *any* of the following changed since last frame:

```rust
Or<(
    Changed<BoxModel>, Changed<Display>, Changed<Position>, Changed<Anchor>,
    Changed<FlexParams>, Changed<FlexItem>, Changed<GridParams>, Changed<GridItem>,
    Changed<Container>, Changed<WritingMode>, Changed<Overflow>, Changed<Scroll>,
    Changed<Stacking>, Changed<Transform>, Changed<Containment>, Changed<MultiColumn>,
    Changed<Children>, Changed<ChildOf>,
)>
```

The `Children` / `ChildOf` triggers cover hierarchy mutations (re-parenting, sibling insertion, despawn). The `LayoutTree` GC (step 0) handles the despawn case via `RemovedComponents<Node>`; `SyncStyles`'s `Changed<Children>` covers the *parent-side* invalidation.

Two private cache components — `WritingModeResolved` (set by an inheritance pass before step 1) and `ContainingBlock` (set by `SyncStyles` itself) — are themselves invalidated when their feeders change: `WritingModeResolved` recomputes when an ancestor's `WritingMode` changed; `ContainingBlock` recomputes when an ancestor's `Position` flipped between `Static` and non-`Static`. Both invalidations re-trigger `SyncStyles` for the affected subtree.

## 2. Public API: hybrid builder + decomposed

Two layers, distinct roles.

### 2.1 Decomposed components — canonical storage

Per the project convention (foundation goal §1.3, `buiy-bsn-integration-design` issue #19), each layout property lives in a small public-fielded `Component`. The table below lists the *author-set styling* components — the surface a Bundle expansion or BSN file writes. Rule-carriers (`ContainerQuery`), runtime state (`ScrollOffset`), longhand transforms (`Translate` / `Rotate` / `Scale`), and per-item child-side styling (`ScrollSnapItem`) live in their owning files but are not on this table.

| Component | Owner file | Concerns |
|---|---|---|
| `BoxModel` | [box-model.md](box-model.md) | width/height + min/max, padding, margin, border, box-sizing, aspect-ratio, logical aliases |
| `Display` | [display-and-positioning.md](display-and-positioning.md) | Display enum (Block, Inline, Flex, Grid, Table*, FlowRoot, Contents, ListItem, Ruby, None) |
| `Position` | [display-and-positioning.md](display-and-positioning.md) | static/relative/absolute/fixed/sticky + inset (logical+physical) |
| `Anchor` | [display-and-positioning.md](display-and-positioning.md) | anchor-name, position-anchor, position-try chain (anchor-size() deferred to v1.x) |
| `FlexParams` | [flex-and-grid.md](flex-and-grid.md) | flex-direction, wrap, justify, align, gap |
| `FlexItem` | [flex-and-grid.md](flex-and-grid.md) | flex-grow/shrink/basis, order, align-self |
| `GridParams` | [flex-and-grid.md](flex-and-grid.md) | grid-template-{columns,rows,areas}, auto-flow, gap |
| `GridItem` | [flex-and-grid.md](flex-and-grid.md) | grid-{column,row,area}, justify-self, align-self |
| `MultiColumn` | [flex-and-grid.md](flex-and-grid.md) | column-count/width/gap/rule/span/fill, break-{inside,before,after} (tier-E, stub in v1) |
| `Container` | [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) | container-type, container-name |
| `WritingMode` | [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) | writing-mode, direction, text-orientation, unicode-bidi |
| `Overflow` | [overflow-and-scrolling.md](overflow-and-scrolling.md) | overflow per axis, scrollbar-gutter, scroll-behavior, overscroll-behavior |
| `Scroll` | [overflow-and-scrolling.md](overflow-and-scrolling.md) | snap-type/align/stop, snap padding/margin |
| `Stacking` | [stacking-and-top-layer.md](stacking-and-top-layer.md) | z-index, isolation, top-layer marker |
| `Transform` | [transforms-and-containment.md](transforms-and-containment.md) | transform, translate/rotate/scale longhands, transform-origin, perspective |
| `Containment` | [transforms-and-containment.md](transforms-and-containment.md) | contain, content-visibility, will-change |

Every component derives `Reflect + Default + Clone + Component`. Every component is registered in the layout plugin's `build` so reflection / BSN / inspectors find them.

Components are inserted independently. A user can spawn a `Display::Flex` without `FlexParams` (defaults apply); they can insert `Stacking` without `Position`.

### 2.2 `Style` builder — ergonomic authoring layer

`Style` is **not** a component. It's a `Bundle`-producing builder: a public-fielded struct *and* a fluent API over the same fields. On `commands.spawn(style)` (or `entity.insert(style)`) it expands into the relevant decomposed components via `Bundle`.

Two equally-valid forms. They write into the same fields:

```rust
// Struct-literal form — discoverable, IDE-autocomplete friendly.
let card = Style {
    display: Display::flex_column(),
    box_model: BoxModel { padding: Edges::all(16.0), gap: Some(8.0), ..default() },
    overflow: Overflow::y_scroll(),
    ..default()
};

// Fluent form — compact, web-familiar.
let card = Style::default()
    .flex_column()
    .padding(16.0)
    .gap(8.0)
    .overflow_y_scroll();
```

The fluent methods are sugar; each one writes the same field the struct literal would. This means:

- A consumer can mix forms freely — set most fields fluently, then override one with `.box_model = ...`.
- Reflection sees the struct fields. Method names are not part of the reflected schema.
- Adding a new layout property is one place to edit (the field) plus one method (the fluent setter), not three.

### 2.3 Bundle expansion

`impl Bundle for Style` decomposes on insert. If a `Style` field is `None` or default, the corresponding component is *not* inserted (so we don't pollute entities with empty components). If a field is set, the component is inserted with the field's value.

Re-inserting `Style` replaces every component it would produce. To partially update layout, insert the decomposed component directly — `commands.entity(e).insert(BoxModel { padding: Edges::all(8.0), ..default() })`.

### 2.4 Child-side components: decomposed-only

`Style` covers an entity's *self-styling* — properties that describe the entity's own box (`BoxModel`, `Display`, `Position`, `Overflow`, etc.) and the *container side* of layout algorithms it participates in (`FlexParams` when it's a flex container, `GridParams` when it's a grid container, `Container` when it's a query container).

The *child side* — properties that only make sense on a child of a particular container (`FlexItem`, `GridItem`, `ScrollSnapItem`) — and `Anchor` (which describes a relationship to another entity) live as decomposed components only. They are spawned alongside `Style` rather than nested inside it:

```rust
commands.spawn((
    Style::default().flex_row().justify_content(JustifyContent::SpaceBetween),
    /* container's own self-styling above */,
)).with_children(|p| {
    p.spawn((Style::default(), FlexItem { grow: 1.0, ..default() }));
    p.spawn((Style::default(), FlexItem { grow: 2.0, ..default() }));
    p.spawn((Style::default(), FlexItem { grow: 1.0, ..default() }));
});
```

Rationale: `Style`'s field set is bounded by an entity's self-shape, not by the cross-product with every algorithm an ancestor might run. Folding `FlexItem` / `GridItem` into `Style` would either explode `Style`'s schema or require it to know which container algorithm is active in scope (which it can't at insert time). Keeping these decomposed sidesteps the question.

For `Anchor` specifically: anchored elements are typically rare (tooltips, popovers, dropdowns) and each carries a non-trivial `position_try` chain. The decomposed-only convention keeps `Style`'s authoring surface focused on the 95% case.

### 2.5 BSN authoring

BSN files reference decomposed components by name, not the `Style` builder. The builder is a Rust-API convenience; BSN is the portable serialization layer.

## 3. System pipeline

One ordered chain runs in `BuiySet::Layout`:

```
0. RemovedNodesGc       — drop despawned entities from LayoutTree
1. SyncStyles           — translate changed Buiy components → taffy::Style
2. CqActivate           — set/clear container-query marker components
3. TaffyCompute         — call tree.compute_layout from each root
4. CqFlipCheck          — re-evaluate queries against fresh sizes
5. (conditional) re-run 1+3 if any query flipped
6. PostTaffyOverrides   — phase composed of sub-passes (in order):
                            6a. StickyOffset      — apply sticky displacement
                            6b. TableLayout       — Buiy-side table algorithm
                            6c. MulticolPack      — multi-column packing
                            6d. AnchorResolution  — anchor + position-try
7. WriteResolvedLayout  — push positions+sizes to Bevy components
```

Each sub-pass of step 6 mutates `ResolvedLayout` for entities matching its concern; sub-passes are independent (sticky doesn't read tables, multi-column doesn't read anchors), so the relative order is the order in which their writes get composed for entities that hit more than one. Sub-passes that have no work (no sticky elements, no `Display::Table*`, no `MultiColumn`, no `Anchor`) are no-ops.

**Commands-flush boundary.** All eight steps run as a single chained system set inside `BuiySet::Layout`; the sub-passes of step 6 (6a-6d) share one `Commands` buffer and one query state with steps 1, 2, 3, 4, 5, 7. The buffer is applied at `BuiySet::Layout`'s end (step 7's completion). This means a despawn issued by sub-pass 6c is **not visible** to sub-pass 6d's queries — both see the same world snapshot established at step 0. Authors must not depend on intra-pipeline despawn visibility; if a despawn must take effect mid-pipeline, schedule it in an earlier `BuiySet`.

Steps 0, 1, 2, 3, 6, 7 always run. Steps 4-5 run only when `Container` components exist on any entity.

### 3.1 Scheduling

All eight steps live in `BuiySet::Layout` and are chained with `.before` / `.after` constraints. (Step 5 is a conditional re-run of steps 1+3 when step 4 signals flip; the chain visualizes it as a discrete step but it shares system code.) The chain is asserted by a test (see [foundation/verification.md § CI gates](../2026-05-07-buiy-foundation/verification.md)) — any reordering must update the test, which surfaces the change in code review.

The chain composes with the rest of `BuiySet`: layout runs after `BuiySet::Animate` (so animated property values are up-to-date) and before `BuiySet::Render`.

### 3.2 Container query re-layout

Step 4 evaluates each `@container` rule against the resolved size of its query container, computed in step 3. The size source is **`tree.layout(node_id)`** — Taffy's per-node layout result, which holds step 3's just-computed values; it is *not* the entity-side `ResolvedLayout` (that's written in step 7 and stale at this point in the chain). If any rule's *activation* state flipped (`@container (min-width: 600px)` was inactive last frame and is active now, or vice versa), the entities subject to that rule have a marker component toggled. Step 1 and step 3 then re-run.

The re-layout fires **at most once per frame**. If a query flipped, ran steps 1+3 again, and a *transitive* query now also flips, the transitive flip applies on the *next* frame. This is the documented limit of the same-frame re-layout strategy ([README § 2 pillar 4](README.md#2-architectural-pillars-one-line-summaries)). [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) details the algorithm.

### 3.3 Anchor resolution

Step 6 walks every entity with an `Anchor` component, looks up the anchor target's `ResolvedLayout`, and overrides the anchored entity's `ResolvedLayout.position` per the `position-try` chain. Anchored elements participate in Taffy's pass first using their declared dimensions; the override applies post-Taffy. [display-and-positioning.md](display-and-positioning.md) details.

## 4. Lifecycle

### 4.1 Insert

When a Buiy `Node` is inserted (or any decomposed layout component on an entity that lacks `LayoutTree` mapping), the next frame's step 1 calls `tree.new_leaf(taffy_style)` and stores the mapping in `by_entity`.

### 4.2 Mutate

Bevy's change detection drives step 1. An entity with `Changed<BoxModel>` (or any other tracked layout component) gets `tree.set_style(node_id, taffy_style)` called this frame. Unchanged entities are skipped.

### 4.3 Despawn — the GC contract

Step 0 reads `RemovedComponents<Node>` and:

1. Removes the orphan from `by_entity`.
2. Calls `tree.remove(node_id)` on the inner `TaffyTree`.

`tree.remove` returning `Err(NotFound)` is **silently swallowed** — this absorbs the case where Taffy already detached the node as a side-effect of removing its parent earlier in the same step. `RemovedComponents<Node>` ordering is not guaranteed by Bevy across a parent/child despawn pair, so step 0 must tolerate either order: parent-first leaves children orphaned in Taffy (step 0 cleans them up by entity), child-first leaves the parent's `set_children` reference dangling (Taffy's `remove(parent)` cleans that up). Net: every despawn produces exactly one removal per affected entity, in arbitrary order.

Without step 0, both `by_entity` and the `TaffyTree` grow monotonically across despawns. (This is the gap described in the `TODO(buiy-layout-design)` block on `LayoutTree` in `crates/buiy_core/src/layout.rs`; the v0.1 backlog implements it.)

### 4.4 Hierarchy changes

Bevy's `ChildOf` / `Children` are the source of truth for the entity hierarchy. Step 1 calls `tree.set_children(parent, &child_ids)` on every entity whose `Children` changed. Topology changes are cheap in Taffy; we don't try to defer or batch them.

## 5. Topological invariant

> **Parents resolve before children.** Document order = AccessKit tree order = default tab order.

Taffy's `compute_layout` enforces this within the tree it computes; Buiy guarantees it across the bridge by running step 3 from each root entity, where a *root* is an entity with `Node` and either no `ChildOf`, or a `ChildOf` whose target lacks a `LayoutTree` entry.

A `Node` whose `ChildOf` points at a non-`Node` entity is a *root*. Mixing Buiy and non-Buiy parents is supported (e.g. a Buiy subtree inside a `bevy::prelude::Camera2dBundle` parent); the non-Buiy parent is invisible to layout.

The invariant is asserted by:

- `buiy-focus-model-design` consuming layout topological order for tab navigation.
- `buiy-accessibility-design` consuming the same order for AccessKit tree construction.
- A test in this spec's realizing crate that traverses a fixture and checks parent-before-child resolution.

## 6. Error model

Layout failures are *frame-local*. They never panic, never poison the tree, and never write a sentinel `ResolvedLayout`.

Failure modes:

- `tree.set_style` returns `Err` — `warn!` once with `entity` + the underlying error; entity uses last frame's style this frame.
- `tree.new_leaf` returns `Err` — `warn!` once; entity is skipped this frame, retried next frame.
- `tree.compute_layout` returns `Err` — `warn!` once; entire root subtree retains last frame's `ResolvedLayout`.
- Anchor target missing or absent from `LayoutTree` — `warn!` once; the anchored element falls through its `position-try` chain. If every fallback fails, position defaults to `(0, 0)` and the entity gets a `LayoutAnchorBroken` marker for devtools.
- Container query references an entity that's not a query container — `warn!` once; the rule is skipped.

The "warn once" semantics are per-(entity, error-kind) pair, deduplicated via a `HashSet` resource cleared on `BuiyExit`. This avoids log spam when an error reproduces every frame.

The error model is **not** a panic budget. If a layout error reproduces every frame, that's a bug — the warn is the surface that lets the bug get fixed, not the response to it.

## 7. Crate placement

This spec assumes layout lives in **either**:

- `buiy_core` (Phase 0 location), or
- `buiy_layout` (a future split per [foundation README § 5](../2026-05-07-buiy-foundation/README.md#5-open-questions)).

The decision is independent of this spec — every type and system named here moves with whichever crate ends up holding layout. Plans choose the crate; this spec is silent on it.

## 8. Test surface

Tests live alongside the realizing code (Phase 0: `crates/buiy_core/tests/`; future: wherever layout splits to). Coverage required by this spec:

1. **System order** — assert the eight-step pipeline runs in declared order; step 5 (conditional re-run) is exercised by a separate fixture.
2. **GC** — spawn Node, despawn, assert `LayoutTree` is empty.
3. **Topological invariant** — fixture with a 4-deep tree; assert parent resolves before children every frame.
4. **Hybrid API equivalence** — same logical layout produced via struct literal and fluent form yields identical decomposed components.
5. **CQ same-frame re-layout** — fixture with one `@container` rule; resize container, assert *this frame's* `ResolvedLayout` reflects the activated rule (not the previous frame's).
6. **Anchor resolution** — fixture with an anchored element + a moving anchor; assert anchored position tracks anchor each frame.
7. **Error path** — induce a Taffy `Err`; assert prior `ResolvedLayout` retained, no panic, exactly one `warn!`.

Tests for individual properties live in their owning child file's section.

## 9. Performance contract

This spec commits to the following invariants. Concrete budget *numbers* live in `buiy-verification-design` (foundation README § 5 — performance budgets is open).

- **Steady-state** (no layout component changed, no children changed): step 1 is `O(0)` work because change detection skips every entity. Step 3 is `O(0)` because Taffy caches. Steps 0, 6, 7 are `O(roots + anchored)`. Total: sub-millisecond for ten-thousand-node trees.
- **Activation-flip frame**: step 3 runs at most twice. Worst case: `2× (steady-state Taffy cost)`.
- **Resize frame** (root size changes): step 3 invalidates and runs once. `1× Taffy cost`.
- **Mass-mutation frame** (e.g. theme switch invalidates every entity's `BoxModel`): step 1 walks every changed entity, step 3 recomputes every root. `O(changed × properties + tree size)`.

The pipeline never re-runs step 3 more than twice per frame. Fixed-point iteration is explicitly out (foundation README § 2 pillar 4).
