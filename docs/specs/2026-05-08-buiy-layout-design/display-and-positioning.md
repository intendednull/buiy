# Display and positioning

**Parent:** [README.md](README.md)

How an entity participates in layout (`Display`), how its box is placed relative to its containing block (`Position`), and how anchored elements override that placement (`Anchor`).

## 1. `Display`

```rust
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)] // NOTE: no `Default` in the reflect attr, unlike `Position`/`PositionKind`.
                      // Follow-up: decide whether `Display` should expose `Default` through
                      // reflection for parity; if yes, add it in code, otherwise this is the spec.
pub enum Display {
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex(FlexAxis),                     // Row | Column | RowReverse | ColumnReverse
    InlineFlex(FlexAxis),
    Grid,
    InlineGrid,
    FlowRoot,                           // CSS `display: flow-root`; establishes BFC
    Contents,                           // children promoted to grandparent (tier-E, not yet shipped)
    Table,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableRow,
    TableCell,
    TableCaption,
    TableColumnGroup,
    TableColumn,
    ListItem,                           // tier-E
    Ruby,                               // tier-E (CJK furigana)
    None,
}

impl Display {
    pub fn flex_row()    -> Self { Self::Flex(FlexAxis::Row) }
    pub fn flex_column() -> Self { Self::Flex(FlexAxis::Column) }
    // ... etc
}
```

### 1.1 Mapping to Taffy

| `Display` variant | `taffy::Display` |
|---|---|
| `Block` | `Block` |
| `Inline`, `InlineBlock` | `Block` (Taffy 0.10 doesn't model inline-flow; Buiy text shaper handles inline-level participation) |
| `Flex(_)`, `InlineFlex(_)` | `Flex` (Buiy passes the axis through `FlexParams.direction`) |
| `Grid`, `InlineGrid` | `Grid` |
| `FlowRoot` | `Block` (Taffy doesn't have a distinct flow-root; v1 maps it to plain `Block` with no BFC / containment marker — deferred) |
| `Contents` | `Block` (v1 fallback; re-parenting children to the grandparent is **deferred — tier-E, not yet shipped**, like `ListItem`/`Ruby`) |
| `Table*` | `Block` (Taffy lacks table layout; Buiy emits a Buiy-side table pass — see [§ 1.2](#12-table-layout-status)) |
| `ListItem` | `Block` with `::marker` pseudo-element handling (deferred to v1.x) |
| `Ruby` | `Block` (deferred — tracks Taffy + i18n) |
| `None` | Entity is removed from the Taffy tree entirely |

### 1.2 Table layout status

Taffy 0.10 doesn't ship table layout. **v1 ships only the API surface and the fallback path; the full algorithm is deferred to a v1.x point release.** The deferred algorithm is described here so the API stays stable across the cutover; the fallback path is what actually runs in v1.

When the algorithm lands, sub-pass 6b ([architecture.md § 3](architecture.md#3-system-pipeline)) will:

1. Gather entities by `Display::Table*` family.
2. Compute column widths via Taffy on a synthetic flex container per row group.
3. Write corrected positions back to `ResolvedLayout`.

**Fallback behavior in v1** (the path actually shipping): `Display::Table*` translates to `Display::Block` for Taffy purposes; sub-pass 6b is a no-op. A `warn!` fires once per (entity, session) the first time each `Display::Table*` value is encountered, naming the entity. Authors who need correct table layout in v1 should use `Display::Grid` with row/column templates instead.

Tier per [foundation/visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout): tier-C (the algorithm). The v1 fallback path covers the API stability commitment; the algorithm itself is tracked as a v1.x deliverable in the migration plan.

### 1.3 `Display::None` vs `Visibility::Hidden`

`Display::None` removes the entity from layout *and* render. `Visibility::Hidden` (a render-side concern, owned by `buiy-render-pipeline-design`) keeps the entity in layout but skips painting. Author guidance: use `Display::None` when the entity should not contribute to size; use `Visibility::Hidden` when it should reserve space.

`inert` (foundation 3.1) and `Display::None` interact: `inert` removes the entity from focus + AccessKit + hit-testing while leaving layout intact; `Display::None` removes from layout. They compose freely.

### 1.4 `Contents`

`Display::Contents` is tier-E and its re-parenting behavior is **deferred — not yet shipped** (like `ListItem`/`Ruby`). In v1, `map_display` ([translate.rs](../../../crates/buiy_core/src/layout/translate.rs)) maps `Contents` → `taffy::Display::Block`, so the entity stays in the Taffy tree and forms its own box. The target behavior described below applies once re-parenting lands:

When shipped, children are *re-parented to the grandparent* during step 1's tree build — the entity itself is not added to Taffy. Useful for wrapper components that shouldn't form their own box. Caveat: `Contents` and absolute-positioned children interact — the absolute-positioned child uses the grandparent as containing block. Spec asserts this in tests.

## 2. `Position`

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Position {
    pub kind: PositionKind,
    pub inset: Inset,
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionKind {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct Inset {
    pub top:    Sizing,
    pub right:  Sizing,
    pub bottom: Sizing,
    pub left:   Sizing,
}
```

For logical authoring, a `LogicalInset` insert helper (analogous to [box-model.md § 4.1 `LogicalBoxModel`](box-model.md#41-api-shape)) translates `inset_block_start` / `inset_inline_end` etc. to physical edges based on the entity's `WritingMode` + `direction`.

### 2.1 Containing block resolution

| `PositionKind` | Containing block |
|---|---|
| `Static`, `Relative` | Parent's content box |
| `Absolute` | Taffy's nearest positioned ancestor (see note), OR the layout viewport if none |
| `Fixed` | The **layout root** (Phase 10 — implemented; see § 2.2 + Known gap below). |
| `Sticky` | Nearest scroll container; falls back to parent's content box outside the sticky range |

> **Note — "positioned ancestor" at the Taffy layer.** Don't read the `Absolute` row as "nearest Buiy non-`Static` ancestor." `map_position_kind` ([translate.rs](../../../crates/buiy_core/src/layout/translate.rs)) emits Buiy `Static` (and `Sticky`) as `taffy::Position::Relative` (see [§ 2.2](#22-mapping-to-taffy)), and in Taffy a `Relative` box *is* a positioned containing block. So at the Taffy layer **every** ancestor box — including Buiy-`Static` ones — is a positioned containing block, and Taffy resolves an `Absolute` child against its immediate parent's content box in practice. This is the shipped behavior; true CSS "nearest non-static ancestor" semantics are the known gap noted below.

Buiy does **not** resolve containing blocks itself: `Absolute` entities stay children of their real parent in the Taffy tree, and Taffy's native absolute-positioning algorithm walks up to the nearest positioned ancestor on its own. `sync_styles` (system pipeline step 1) performs no containing-block lookup, and there is no `ContainingBlock` component — `map_position_kind` ([translate.rs](../../../crates/buiy_core/src/layout/translate.rs)) simply maps `PositionKind` onto `taffy::Position` and the real parent edge carries the relationship.

`Display::Contents` is transparent to containing-block resolution — descend through it (Taffy never sees the re-parented wrapper, so its native lookup also skips it).

> **Shipped — `Fixed` resolves against the layout root.** Phase 10 implements `Position::Fixed`: a `Fixed` entity's Taffy node is re-parented onto the layout root's child list in `sync_children_for_entity` ([systems.rs](../../../crates/buiy_core/src/layout/systems.rs)), so Taffy's native absolute algorithm resolves it (including percentage insets) against the root's content box rather than its nearest positioned ancestor. This is the sole behavioral difference from `Absolute`. The entity remains a Bevy child of its real parent, so writing-mode inheritance, the changed-set filter, and the `ResolvedLayout` write are unchanged.
>
> **Still deferred.** Taffy-native positioning does not model the *full* CSS containing-block rules: a transformed element must become the containing block for `Position::Fixed` descendants. That transformed-ancestor case is **not yet implemented** — `Fixed` always resolves against the layout root regardless of an intervening transformed ancestor. Per-window / multi-root `Fixed` targeting is likewise gated on `buiy-window-and-surface-design` (single global root in v1; the first root in `Node`-query order wins).

### 2.2 Mapping to Taffy

Taffy 0.10 supports `position: absolute` (resolving against the nearest positioned ancestor) and `relative` via offsets; `sticky` is a Buiy post-Taffy pass. `fixed` has no distinct Taffy modeling, so it emits `taffy::Position::Absolute` like `Absolute`; the difference is the *containing block*, which Buiy effects by re-parenting the `Fixed` node onto the layout root in the children-sync pass (so the emitted `Position` is identical but the Taffy parent edge — and therefore the resolution box — is the root). Implemented in Phase 10.

| `PositionKind` | Taffy emission |
|---|---|
| `Static` | `taffy::Position::Relative` with zero offsets (Taffy's "in-flow"). |
| `Relative` | `taffy::Position::Relative` with `inset` as offset. |
| `Absolute` | `taffy::Position::Absolute` with `inset`; resolved by Taffy against the real parent / nearest positioned ancestor. |
| `Fixed` | `taffy::Position::Absolute`; resolved against the **layout root** because the node is re-parented onto the root's child list in `sync_children_for_entity` (Phase 10). Transformed-ancestor-as-containing-block still deferred (see Known gap, § 2.1). |
| `Sticky` | `taffy::Position::Relative`; sticky offsets applied in sub-pass 6a ([architecture.md § 3](architecture.md#3-system-pipeline)). |

### 2.3 Sticky positioning

A sticky element behaves as `Relative` until its scroll container's scroll offset crosses the inset threshold, then it sticks to the threshold edge until the element's parent leaves the threshold range. The pass runs as sub-pass 6a ([architecture.md § 3](architecture.md#3-system-pipeline)) so it sees fresh `ResolvedLayout` from Taffy plus current scroll offsets from the entity's nearest scroll container.

Sticky offsets *do not* invalidate Taffy. The element's contribution to its parent's flow is computed as `Relative`; the sticky displacement is a render-time visual offset baked into `ResolvedLayout.position` after Taffy.

## 3. Anchor positioning

Tier-C. Buiy-owned, post-Taffy. CSS Anchor Positioning Module Level 1.

### 3.1 `Anchor` component

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Anchor {
    pub anchor_name: Option<AnchorName>,        // declares this entity AS an anchor
    pub position_anchor: Option<AnchorRef>,     // declares this entity is anchored TO another
    pub position_try: Vec<PositionTry>,         // ordered fallback chain
}

#[derive(Reflect, Clone, Debug, PartialEq, Default)]
pub struct PositionTry {
    pub inset: Inset,                           // anchored offset relative to the anchor's box
    pub conditions: Vec<TryCondition>,          // when this fallback is "valid"
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum TryCondition {
    FitsInViewport,                             // anchored box must not overflow the viewport
    FitsInContainer(AnchorRef),                 // anchored box must fit inside <ref>'s box
    AnchorVisible,                              // anchor's resolved rect intersects viewport
}

// `Named`/`Name` use `String` (not `SmolStr`): Phase 6 follows the Phase 3
// `GridAreas` precedent and avoids a new direct dep (see the String-choice
// note on `GridLine`/`AnchorName` in types.rs, ~types.rs:787-789).
#[derive(Reflect, Clone, Debug, PartialEq, Eq, Default)]
pub enum AnchorName {
    #[default]
    Implicit,                                   // referenced by Entity ID alone
    Named(String),                              // CSS-style name lookup (registered in AnchorNameRegistry)
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum AnchorRef {
    Entity(Entity),
    Name(String),
}

/// Resource: maps `AnchorName::Named` strings to the entities that declared
/// that name. Maintained by an observer on `Anchor` insert/remove —
/// authors do not write to it directly. Multiple entities declaring the
/// same name produce a `warn!` once per (late-inserter entity, frame)
/// (keyed `(Entity, AnchorErrorKind)`); the most-recently-inserted entity
/// wins (each bucket stores `(Entity, epoch)` pairs, last wins).
///
/// `Default` is hand-written (NOT derived) to seed `next_epoch = 1`, so
/// `entity_epoch(e) > 0` is a faithful "tracked" predicate (epoch 0 is the
/// `unwrap_or(0)` "no entry" fallback). The `(Entity, epoch)` pairs and
/// `entity_epochs` feed the Kahn cycle-edge-drop tiebreaker (§ 3.4).
#[derive(Resource, Debug)]
pub struct AnchorNameRegistry {
    by_name: HashMap<String, Vec<(Entity, u64)>>,
    entity_epochs: HashMap<Entity, u64>,
    next_epoch: u64,
}
```

### 3.2 Resolution

Sub-pass 6d of the pipeline ([architecture.md § 3](architecture.md#3-system-pipeline)) walks every entity with `Anchor.position_anchor.is_some()`:

1. **Resolve anchor target.** Look up the anchor's `Entity` — either directly (`AnchorRef::Entity`) or via `AnchorNameRegistry` (`AnchorRef::Name`); read its `ResolvedLayout`. If the target is missing, despawned, or carries `Display::None`, the lookup fails and falls through to step 4 below. The `Display::None`-as-missing rule is a **live query check** (Decision D9), not a stored-state mechanism: `resolve_target` reads the target's `Display` from a `display_query` each frame and returns `None` when it is `Display::None`; `try_conditions_pass` applies the same check to `FitsInContainer` containers. `write_resolved_layout` (step 7) does **not** clear `ResolvedLayout` for `Display::None` entities — the stale-position concern is moot because the live `Display` lookup short-circuits before the stored layout is ever consulted.
2. **Try fallbacks in order.** For each `PositionTry` in `position_try`, compute the anchored entity's would-be box (using `inset` relative to the anchor) and evaluate every condition. The first try whose conditions all pass wins.
3. **Apply.** Override `ResolvedLayout.position` with the chosen try's resolved coordinates.
4. **Fallback failure.** If every try fails (or the anchor target was missing), position defaults to `(0, 0)` and the entity gets a `LayoutAnchorBroken` marker for devtools. A `warn!` fires once per (entity, frame).

### 3.3 Authoring example

```rust
// A tooltip anchored to a button, preferring above, falling back to below.
commands.spawn((
    /* button bundle */,
    Anchor { anchor_name: Some(AnchorName::Named("submit-btn".into())), .. default() },
));
commands.spawn((
    /* tooltip bundle */,
    Anchor {
        position_anchor: Some(AnchorRef::Name("submit-btn".into())),
        position_try: vec![
            PositionTry { inset: Inset::above(Length::px(8.0)), conditions: vec![TryCondition::FitsInViewport] },
            PositionTry { inset: Inset::below(Length::px(8.0)), conditions: vec![TryCondition::FitsInViewport] },
        ],
        .. default()
    },
));
```

### 3.4 Performance and ordering

- An anchor target must resolve before its dependent. Sub-pass 6d builds a Kahn topological sort over the (anchored → anchor) DAG: `O(V + E)` where V = anchored entities, E = anchor edges (= V, since each anchored entity has exactly one anchor target).
- **Cycle handling.** Edges that would close a cycle are dropped — the dropped edge is `(child anchored, anchor)` for the *most-recently-inserted* anchored entity in the cycle (tracked by the `AnchorNameRegistry` insertion epoch and an analogous epoch on `AnchorRef::Entity`). Both endpoints get `LayoutAnchorBroken` markers; one `warn!` fires per cycle per frame naming the dropped edge. Result: every cycle resolves deterministically; tests can assert exact membership.
- Cost: `O(anchored entities × tries + V + E)`. Usually small — most anchored elements have 1-3 fallbacks.
- Anchor resolution does **not** trigger Taffy re-layout. The anchor's *size* is fixed by the time anchor resolution runs; only its position changes. (Anchor *size* affecting layout — `anchor-size()` in CSS — is a tier-C feature **fully deferred to v1.x with no API surface in v1**: there is no `Length`/`Sizing`/`Inset` variant that can express `anchor-size()`, so authors cannot exercise it. The `AnchorErrorKind::AnchorSizeUsed` warn arm exists ([systems.rs](../../../crates/buiy_core/src/layout/systems.rs)) but is currently **unreachable** — nothing pushes that kind into the warn set, because no inset term can request an anchor size. When the API lands in v1.x, the warn arm will fire once per (entity, frame) and the size term resolves to zero. See open questions in [README § 5](README.md#5-open-questions).)

### 3.5 Open question: `position-try` chain depth

CSS spec allows arbitrarily many fallbacks. v1 supports any chain length but evaluates linearly; if performance becomes an issue with deeply nested fallbacks, we add a `position_try_max_depth` resource cap. Tracked in [README § 5](README.md#5-open-questions).

## 4. Test surface

- **Containing block resolution** — fixture with nested `Position::Static / Relative / Absolute` ancestors; assert each child's containing block resolves correctly.
- **Sticky behavior** — fixture with a scrolling container and a sticky child; scroll the container, assert sticky offset clamps within the threshold range.
- **`Display::Contents` re-parenting** — fixture parent → contents-wrapper → grandchild; assert grandchild's box matches what it would if wrapper were absent.
- **`Display::None` vs `Visibility::Hidden`** — assert `Display::None` produces zero `ResolvedLayout.size`, `Visibility::Hidden` produces non-zero.
- **Anchor basic** — fixture with an anchor + one anchored element; assert the anchored entity's `position` tracks the anchor every frame.
- **Anchor fallback chain** — fixture with two fallbacks; force the first to fail (move the anchor near the viewport edge); assert the second activates.
- **Anchor cycle detection** — fixture where A anchors to B, B anchors to A; assert one resolves, one gets `LayoutAnchorBroken`, exactly one `warn!`.
