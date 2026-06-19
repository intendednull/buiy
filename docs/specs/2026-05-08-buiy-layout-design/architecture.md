# Layout architecture

**Parent:** [README.md](README.md)

This file defines the structural skeleton every other child relies on: the bridge between Buiy components and Taffy, the public API shape, the system pipeline order, the data lifecycle, and the error model. Every other file in this spec assumes the contracts here hold.

## 1. Bridge model: Buiy ↔ Taffy

Taffy is the layout engine. Buiy *translates* its decomposed component graph into a `taffy::TaffyTree`, calls `compute_layout` from the roots, then writes Taffy's results back to Bevy entities as `ResolvedLayout`.

The bridge is one-directional. Buiy never patches Taffy in place; features Taffy lacks are layered as Buiy passes wrapping Taffy (see [§ 3 System pipeline](#3-system-pipeline)).

### 1.1 `LayoutTree` — the bridge state

```rust
pub struct LayoutTree {
    tree: TaffyTree<Entity>,
    by_entity: HashMap<Entity, TaffyNodeId>,
}
```

The Taffy node context is `Entity`, not `()`: text leaves register their owning entity as the node context so Taffy's measure closure can resolve the entity behind a measured node. `TaffyTree<Entity>` is not itself `Default` (the `()` node-context form is), so `LayoutTree` carries a hand-written `impl Default` rather than `#[derive(Default)]`. (The `<Entity>` context was introduced by the text campaign when text leaves needed shrink-to-fit measurement.)

Stored as a `NonSendResource`. Lifetime: app-long. Reused frame-to-frame so Taffy's internal cache stays warm.

**Why `NonSendResource`?** Taffy 0.10 packs every `Dimension` into a tagged pointer (`*const ()`) regardless of whether the `calc` feature is enabled. `TaffyTree` is therefore `!Send + !Sync`. Layout is inherently a single-threaded pass over the tree, so `NonSendResource` is both correct and `unsafe`-free. (This is the same rationale documented in `crates/buiy_core/src/layout/tree.rs`. Layout is now a directory module: `layout/mod.rs` holds the plugin, with `tree.rs`, `systems.rs`, `pipeline.rs`, `style.rs`, `components.rs`, `translate.rs`, and `types.rs` alongside it.)

### 1.2 Translation layer

A free function `style_to_taffy(view: StyleView<'_>) -> taffy::Style` collects every layout-relevant decomposed component on an entity into a single `taffy::Style`. It runs every frame for entities whose layout components changed (per Bevy change detection); on unchanged entities it skips the rebuild. Per-entity translation cost is `O(properties)`; per-frame cost is `O(changed entities × properties)`.

Translation is a pure function. Taffy's compute step is the only thing that mutates the tree.

#### Change-detection trigger set

`SyncStyles` (step 1) re-translates an entity when *any* of the following changed since last frame:

```rust
Or<(
    Changed<Display>, Changed<BoxModel>, Changed<Position>,
    Changed<FlexParams>, Changed<FlexItem>, Changed<Overflow>, Changed<Scroll>,
    Changed<GridParams>, Changed<GridItem>, Changed<WritingMode>,
    Changed<WritingModeResolved>, Changed<Children>, Changed<ChildOf>,
    Changed<ResolvedLayout>,
    // Nested inner Or to stay under Bevy's 15-element outer-tuple cap.
    Or<(
        Changed<Container>, Changed<ContainerQuery>,
        Changed<ContainerQueryActive>, Changed<ContainerQueryInactive>,
        Changed<Anchor>, Changed<MultiColumn>, Changed<Containment>,
    )>,
)>
```

The `Children` / `ChildOf` triggers cover hierarchy mutations (re-parenting, sibling insertion, despawn). The `LayoutTree` GC (step 0) handles the despawn case via `RemovedComponents<Node>`; `SyncStyles`'s `Changed<Children>` covers the *parent-side* invalidation.

`Changed<ResolvedLayout>` feeds the container-unit cascade (Phase 5): an entity whose size shifts to track an ancestor's resolved size re-translates on the next frame. The four `Container` / `ContainerQuery*` triggers cover query-container changes and marker toggles; `Changed<Anchor>` keeps a freshly spawned anchored entity's Taffy node in sync for sub-pass 6d. `Changed<MultiColumn>` re-syncs an entity whose multi-column declaration changed so sub-pass 6c (`multicol_pack`, the Phase-13 position-only packer — see [flex-and-grid.md § 3.2](flex-and-grid.md#32-algorithm)) re-packs it; multicol still does not feed Taffy directly (the packer is a post-Taffy override), so the trigger keeps the entity's Taffy node current for the surrounding block-flow pass. `Changed<Containment>` joins the inner `Or` because containment landed in the shipped trigger set (Phase 8 — see below).

`WritingModeResolved` is a private cache component set by the `WritingModeInherit` pass (step 0b, before step 1); `Changed<WritingModeResolved>` re-triggers `SyncStyles` for an entity whose effective inherited writing-mode actually changed (the inherit pass skips writes when the value is unchanged, preserving the O(0) steady-state contract). `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` are intentionally **excluded** — they are runtime/snap state, not layout inputs.

`Containment` **joined the trigger set in Phase 8** (`Changed<Containment>` in the inner `Or` above): its layout-affecting bits (size containment, `content-visibility`) feed `style_to_taffy`, so a containment change must re-translate. `Stacking` and `UiTransform` remain **excluded**: neither feeds Taffy compute (stacking-context formation and the transform composition run as post-Taffy passes, sub-passes 6e/6f, reading `Changed` directly inside their own systems rather than gating `SyncStyles`).

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
| `MultiColumn` | [flex-and-grid.md](flex-and-grid.md) | column-count/width/gap/rule/span/fill, break-{inside,before,after} (tier-E; real position-only packer landed Phase 13 — true fragmentation still deferred, see [flex-and-grid.md § 3.2](flex-and-grid.md#32-algorithm)) |
| `Container` | [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) | container-type, container-name |
| `WritingMode` | [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) | writing-mode, direction, text-orientation, unicode-bidi |
| `Overflow` | [overflow-and-scrolling.md](overflow-and-scrolling.md) | overflow per axis, scrollbar-gutter, scroll-behavior, overscroll-behavior |
| `Scroll` | [overflow-and-scrolling.md](overflow-and-scrolling.md) | snap-type/align/stop, snap padding/margin |
| `Stacking` † | [stacking-and-top-layer.md](stacking-and-top-layer.md) | z-index, isolation, top-layer marker |
| `UiTransform` † | [transforms-and-containment.md](transforms-and-containment.md) | transform, translate/rotate/scale longhands, transform-origin, perspective (named `UiTransform`, not `Transform`, to avoid colliding with Bevy's prelude `Transform`) |
| `Containment` † | [transforms-and-containment.md](transforms-and-containment.md) | contain, content-visibility, will-change |

† **Landed (Phases 8/9).** `Stacking`, `UiTransform` (the CSS `transform` surface — named `UiTransform` to avoid Bevy's prelude `Transform`, see [transforms-and-containment.md § 1](transforms-and-containment.md#1-uitransform)), and `Containment` are all defined, exported, and registered in the layout plugin's `build` (`layout/mod.rs`): `Containment` and the transform components shipped in Phase 8, `Stacking` + top-layer in Phase 9. They are full author-set components like the rows above them; the `†` marks which phase introduced them, not a deferral.

Every component derives `Reflect + Default + Clone + Component`. Every component is registered in the layout plugin's `build` so reflection / BSN / inspectors find them.

Components are inserted independently. A user can spawn a `Display::Flex` without `FlexParams` (defaults apply); they can insert `Stacking` without `Position`.

### 2.2 `Style` builder — ergonomic authoring layer

`Style` is **not** a component. It's a `Bundle`-producing builder: a public-fielded struct *and* a fluent API over the same fields. On `commands.spawn(style)` (or `entity.insert(style)`) it expands into the relevant decomposed components via `Bundle`.

Two equally-valid forms. They write into the same fields:

```rust
// Struct-literal form — discoverable, IDE-autocomplete friendly.
let card = Style {
    display: Display::flex_column(),
    box_model: BoxModel { padding: Edges::all(16.0), ..default() },
    flex_params: FlexParams { gap: FlexGap { row: Length::Px(8.0), column: Length::Px(8.0) }, ..default() },
    overflow: Overflow { y: OverflowMode::Scroll, ..default() },
    ..default()
};

// Fluent form — compact, web-familiar.
let card = Style::default()
    .flex_column()
    .padding(16.0)
    .gap_px(8.0)
    .overflow_y_scroll();
```

The fluent methods are sugar; each one writes the same field the struct literal would. This means:

- A consumer can mix forms freely — set most fields fluently, then override one with `.box_model = ...`.
- Reflection sees the struct fields. Method names are not part of the reflected schema.
- Adding a new layout property is one place to edit (the field) plus one method (the fluent setter), not three.

### 2.3 Bundle expansion

`Style` is `#[derive(Bundle)]` over plain value fields (`style.rs` § struct def), so on insert it decomposes into its decomposed components — and **every** component it carries is **always** inserted, including ones left at their default value. A defaulted `Style` field produces a defaulted component, not an absent one.

> **Deferred (Phase 4 revisit).** Skip-on-default — only inserting a component when its `Style` field diverges from the default, to avoid polluting entities with empty components — is a target, not shipped behavior. `style.rs` § module docs records it as a Phase 4 (`LogicalBoxModel`) revisit; until then components are always inserted.

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
0.  RemovedNodesGc       — drop despawned entities from LayoutTree
0b. WritingModeInherit   — populate WritingModeResolved by walking the hierarchy
                           (inserted pre-pass, Phase 4; feeds step 1)
1.  SyncStyles           — translate changed Buiy components → taffy::Style
2.  CqActivate           — set/clear container-query marker components
3.  TaffyCompute         — call tree.compute_layout from each root
4.  CqFlipCheck          — re-evaluate queries against fresh sizes
5.  CqFlipReRun          — conditional re-run of 1+3 if any query flipped
6.  PostTaffyOverrides   — phase composed of sub-passes (in order):
                            6a. StickyOffset      — apply sticky displacement
                            6b. TableLayout       — Buiy-side table algorithm
                            6c. MulticolPack      — multi-column packing
                            6d. AnchorResolution  — anchor + position-try
                            6e. TransformCompose  — compose UiTransform/longhand → ResolvedTransform (Phase 8)
                            6f. StackingDetect    — stacking-context formation (Phase 9), reads 6e's matrix
7.  WriteResolvedLayout  — push positions+sizes to Bevy components
8.  CqDescendantInvalidate — collect descendants of resized query containers (Phase 14)
9.  CqDescendantReRun    — drain the dirty set; re-resolve Cq* descendants same-frame (Phase 14)
```

`WritingModeInherit` (step 0b) was inserted after `RemovedNodesGc` in Phase 4 so step 1 sees each entity's effective inherited writing-mode. `CqFlipReRun` (step 5) is a real sub-set, not an inline annotation: when step 4 signals a flip it re-runs the work of steps 1+3 (it shares their system code). The first link in step 6's chain is a `clear_post_taffy_overrides` system that empties the shared override map; the six sub-passes (6a-6f) then write into it (6e composes the transform matrix into `ResolvedTransform`, Phase 8; 6f detects stacking-context formation off 6e's composed matrix, Phase 9 — see [transforms-and-containment.md § 1.1](transforms-and-containment.md#11-longhand-components)).

Steps 8 (`CqDescendantInvalidate`) and 9 (`CqDescendantReRun`) were appended in **Phase 14** to extend the same-frame container-query settle to the *multi-level geometric* cascade: step 8 collects the descendants of any query container whose `ResolvedLayout` changed and step 9 (gated on a request flag, like step 5) re-resolves their `Length::Cq*` values and re-evaluates their rules the same frame, capped at one re-run. [container-queries-and-writing-modes.md § 1.3](container-queries-and-writing-modes.md#13-activation-same-frame-re-layout) details the algorithm.

**Layout owns steps 0–9. Text adds two more sub-sets.** The text-rendering subsystem registers `TextSync` and `TextCommit` inside `BuiySet::Layout` around the layout chain (shaping feeds layout's measure closure; the committed glyph geometry is read after `WriteResolvedLayout`), bringing the full count inside `BuiySet::Layout` to **thirteen sub-sets**: layout's eleven (steps 0, 0b, 1–9) plus text's two.

The geometric sub-passes of step 6 (6a-6d) mutate `ResolvedLayout` (via the shared override map) for entities matching their concern; they are independent (sticky doesn't read tables, multi-column doesn't read anchors), so the relative order is the order in which their writes get composed for entities that hit more than one. The two later sub-passes consume rather than displace geometry: 6e writes the composed matrix to `ResolvedTransform`, and 6f reads 6e's matrix to detect stacking-context formation — neither moves `ResolvedLayout`. Sub-passes with no matching entities (no sticky elements, no `Display::Table*`, no `MultiColumn`, no `Anchor`, no non-identity transform) are no-ops.

**Commands-flush boundary.** All eleven layout system sets (steps 0, 0b, 1–9) run as a single chained system set inside `BuiySet::Layout`; the sub-passes of step 6 (6a-6f) share one `Commands` buffer and one query state with the other steps. The buffer is applied at `BuiySet::Layout`'s end (after step 9 completes). This means a despawn issued by sub-pass 6c is **not visible** to sub-pass 6d's queries — both see the same world snapshot established at step 0. Authors must not depend on intra-pipeline despawn visibility; if a despawn must take effect mid-pipeline, schedule it in an earlier `BuiySet`.

The always-scheduled sets are steps 0, 0b, 1, 3, 6, 7. The container-query steps — 2 (`CqActivate`), 4 (`CqFlipCheck`), 5 (`CqFlipReRun`), 8 (`CqDescendantInvalidate`), 9 (`CqDescendantReRun`) — do work only when `Container` components exist on any entity.

### 3.1 Scheduling

All eleven layout system sets (steps 0, 0b, 1–9) live in `BuiySet::Layout` and are chained with `.before` / `.after` constraints. (Step 5, `CqFlipReRun`, is a conditional re-run of steps 1+3 when step 4 signals a flip; step 9, `CqDescendantReRun`, is the analogous Phase-14 re-run gated on `CqDescendantReRunRequested`; both are their own sub-sets but share the system code they re-run.) The chain is asserted by a test (see [foundation/verification.md § CI gates](../2026-05-07-buiy-foundation/verification.md)) — any reordering must update the test, which surfaces the change in code review.

The chain composes with the rest of `BuiySet`: the `CorePlugin` order is `Layout → Style → Input → Animate → Picking → A11yUpdate → Render`, so layout runs **before** `BuiySet::Animate` and `BuiySet::Render`. (Pinned by `tests/system_set_order.rs::layout_runs_before_animate` and `::layout_runs_before_render`.)

### 3.2 Container query re-layout

Step 4 (`CqFlipCheck`) evaluates each `@container` rule against the resolved size of its query container, computed in step 3. The size source is **`tree.layout(node_id)`** — Taffy's per-node layout result, which holds step 3's just-computed values; it is *not* the entity-side `ResolvedLayout` (that's written in step 7 and stale at this point in the chain). If any rule's *activation* state flipped (`@container (min-width: 600px)` was inactive last frame and is active now, or vice versa), the entities subject to that rule have a marker component toggled. Step 5 (`CqFlipReRun`) then re-runs the work of steps 1+3.

The re-layout fires **at most once per frame**. If a query flipped, ran steps 1+3 again, and a *transitive* query now also flips, the transitive flip applies on the *next* frame. This is the documented limit of the same-frame re-layout strategy ([README § 2 pillar 4](README.md#2-architectural-pillars-one-line-summaries)). [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md) details the algorithm.

### 3.3 Anchor resolution

Sub-pass 6d (`AnchorResolution`) walks every entity with an `Anchor` component, looks up the anchor target's `ResolvedLayout`, and overrides the anchored entity's `ResolvedLayout.position` per the `position-try` chain. Anchored elements participate in Taffy's pass first using their declared dimensions; the override applies post-Taffy. [display-and-positioning.md](display-and-positioning.md) details.

## 4. Lifecycle

### 4.1 Insert

When a Buiy `Node` is inserted (or any decomposed layout component on an entity that lacks `LayoutTree` mapping), the next frame's step 1 (`SyncStyles`) calls `tree.new_leaf(taffy_style)` and stores the mapping in `by_entity`.

### 4.2 Mutate

Bevy's change detection drives step 1 (`SyncStyles`). An entity with `Changed<BoxModel>` (or any other tracked layout component) gets `tree.set_style(node_id, taffy_style)` called this frame. Unchanged entities are skipped.

### 4.3 Despawn — the GC contract

Step 0 reads `RemovedComponents<Node>` and:

1. Removes the orphan from `by_entity`.
2. Calls `tree.remove(node_id)` on the inner `TaffyTree`.

`tree.remove` returning `Err(NotFound)` is **silently swallowed** — this absorbs the case where Taffy already detached the node as a side-effect of removing its parent earlier in the same step. `RemovedComponents<Node>` ordering is not guaranteed by Bevy across a parent/child despawn pair, so step 0 must tolerate either order: parent-first leaves children orphaned in Taffy (step 0 cleans them up by entity), child-first leaves the parent's `set_children` reference dangling (Taffy's `remove(parent)` cleans that up). Net: every despawn produces exactly one removal per affected entity, in arbitrary order.

> **Deferred — silent-swallow is the target, not yet shipped.** Today `gc_removed_nodes` (`systems.rs`) blanket-`warn!`s on **any** `tree.remove` error (including the benign `NotFound` above), carrying forward Phase 0's behavior. Narrowing the match so `NotFound` is swallowed while genuine errors still warn is deferred to a follow-up that audits Taffy 0.10's error enum (the variant for the already-detached case is uncertain enough to pin first).

Without step 0, both `by_entity` and the `TaffyTree` grow monotonically across despawns. GC is implemented as `gc_removed_nodes` in `crates/buiy_core/src/layout/systems.rs`.

### 4.4 Hierarchy changes

Bevy's `ChildOf` / `Children` are the source of truth for the entity hierarchy. Step 1 (`SyncStyles`) calls `tree.set_children(parent, &child_ids)` on every entity whose `Children` changed. Topology changes are cheap in Taffy; we don't try to defer or batch them.

## 5. Topological invariant

> **Parents resolve before children.** Document order = AccessKit tree order = default tab order.

Taffy's `compute_layout` enforces this within the tree it computes; Buiy guarantees it across the bridge by running step 3 (`TaffyCompute`) from each root entity, where a *root* is an entity with `Node` and either no `ChildOf`, or a `ChildOf` whose target lacks a `LayoutTree` entry.

A `Node` whose `ChildOf` points at a non-`Node` entity is a *root*. Mixing Buiy and non-Buiy parents is supported (e.g. a Buiy subtree inside a `bevy::prelude::Camera2dBundle` parent); the non-Buiy parent is invisible to layout.

The invariant is asserted by:

- `buiy-focus-model-design` consuming layout topological order for tab navigation.
- `buiy-accessibility-design` consuming the same order for AccessKit tree construction.
- A test in this spec's realizing crate that traverses a fixture and checks parent-before-child resolution.

## 6. Error model

Layout failures are *frame-local*. They never panic, never poison the tree, and never write a sentinel `ResolvedLayout`.

Failure modes split into two groups by dedup behavior.

**Taffy-bridge failures — `warn!` *every frame*, un-deduplicated.** These four paths emit a fresh `warn!` on every frame the error reproduces; there is no `HashSet` backing them today:

- `tree.set_style` returns `Err` (`systems.rs`) — `warn!` with `entity` + the underlying error each frame; entity uses last frame's style this frame.
- `tree.new_leaf` returns `Err` (`systems.rs`) — `warn!` each frame; entity is skipped this frame, retried next frame.
- `tree.compute_layout` returns `Err` (`systems.rs`) — `warn!` each frame; entire root subtree retains last frame's `ResolvedLayout`.
- `tree.set_children` returns `Err` (systems.rs) — `warn!` each frame; the parent's child list is left as Taffy last had it this frame.

**Sub-pass failures — `warn!` *deduplicated* via a `HashSet`.** Only the step-6 sub-pass error modes carry dedup state:

- Anchor target missing or absent from `LayoutTree` (sub-pass 6d) — `warn!`; the anchored element falls through its `position-try` chain. If every fallback fails, position defaults to `(0, 0)` and the entity gets a `LayoutAnchorBroken` marker for devtools.
- Sticky / table / multi-column sub-pass errors (sub-passes 6a-6c) — `warn!`; the affected entity falls back to its pre-override `ResolvedLayout`.

The sub-pass dedup uses a `HashSet` resource, but the scope differs by regime. Two regimes ship today:

- **Session-scoped** — `LayoutWarnedOnceSession` (a `HashSet<LayoutWarnOnceKey>`) backs the sticky / table / multi-column sub-passes (6a-6c). It is cleared only on `BuiyExit` via `clear_warned_once_on_exit`, so each key warns at most once per `App` lifetime. (That clear system is defined but not yet wired against an `OnExit` hook because `buiy_core` has no `BuiyState` / `BuiyExit` lifecycle enum yet — plan decision D7; `init_resource` constructing a fresh empty set on every `App::new()` satisfies the contract in the meantime.)
- **Per-frame** — `LayoutAnchorWarnedThisFrame` (a `HashSet<(Entity, AnchorErrorKind)>`) backs anchor resolution (sub-pass 6d). It is cleared and repopulated **each frame** at the top of `anchor_resolution`, so anchor warnings dedup within a frame but re-emit on a subsequent frame if the error persists. This per-frame scope is a deliberate divergence from the session-scoped model (it lets devtools surface a still-broken anchor every frame rather than once); the divergence is recorded in the Phase 6 changelog.

The dedup regimes avoid log spam from a sub-pass error reproducing every frame (session-scoped suppresses entirely; per-frame suppresses duplicates within the frame). The four Taffy-bridge paths above have no such suppression yet — they re-emit each frame.

Container queries are deliberately absent from both groups. A `ContainerQuery` rule whose container can't be resolved — no matching ancestor, or an ancestor that isn't a query container — is treated as `active = false` with **no `warn!`** at all: `cq_activate` / `cq_flip_check` return `false` for an unresolvable container exactly as they do for a genuinely-inactive one. This "silently inactive" outcome is the shipped contract, not an error path. CQ also runs as pipeline steps 2/4/5 rather than a step-6 sub-pass, so it carries no entry in either dedup `HashSet`.

The error model is **not** a panic budget. If a layout error reproduces every frame, that's a bug — the warn is the surface that lets the bug get fixed, not the response to it.

## 7. Crate placement

This spec assumes layout lives in **either**:

- `buiy_core` (Phase 0 location), or
- `buiy_layout` (a future split per [foundation README § 5](../2026-05-07-buiy-foundation/README.md#5-open-questions)).

The decision is independent of this spec — every type and system named here moves with whichever crate ends up holding layout. Plans choose the crate; this spec is silent on it.

## 8. Test surface

Tests live alongside the realizing code (Phase 0: `crates/buiy_core/tests/`; future: wherever layout splits to). Coverage required by this spec:

1. **System order** — assert the pipeline (eleven layout system sets: ten numbered steps 0–9 plus the `WritingModeInherit` pre-pass) runs in declared order; the conditional re-runs (step 5 `CqFlipReRun`, step 9 `CqDescendantReRun`) are exercised by separate fixtures.
2. **GC** — spawn Node, despawn, assert `LayoutTree` is empty.
3. **Topological invariant** — fixture with a 4-deep tree; assert parent resolves before children every frame.
4. **Hybrid API equivalence** — same logical layout produced via struct literal and fluent form yields identical decomposed components.
5. **CQ same-frame re-layout** — fixture with one `@container` rule; resize container, assert *this frame's* `ResolvedLayout` reflects the activated rule (not the previous frame's).
6. **Anchor resolution** — fixture with an anchored element + a moving anchor; assert anchored position tracks anchor each frame.
7. **Error path** — induce a Taffy `Err`; assert prior `ResolvedLayout` retained, no panic, exactly one `warn!`.

Tests for individual properties live in their owning child file's section.

## 9. Performance contract

This spec commits to the following invariants. Concrete budget *numbers* live in `buiy-verification-design` (foundation README § 5 — performance budgets is open).

- **Steady-state** (no layout component changed, no children changed): step 1 (`SyncStyles`) is `O(0)` work because change detection skips every entity. Step 3 (`TaffyCompute`) is `O(0)` because Taffy caches. Steps 0, 6, 7 are `O(roots + anchored)`. Total: sub-millisecond for ten-thousand-node trees.
- **Activation-flip frame**: step 3 runs at most twice. Worst case: `2× (steady-state Taffy cost)`.
- **Resize frame** (root size changes): step 3 invalidates and runs once. `1× Taffy cost`.
- **Mass-mutation frame** (e.g. theme switch invalidates every entity's `BoxModel`): step 1 walks every changed entity, step 3 recomputes every root. `O(changed × properties + tree size)`.

The pipeline never re-runs step 3 more than twice per frame. Fixed-point iteration is explicitly out (foundation README § 2 pillar 4).
