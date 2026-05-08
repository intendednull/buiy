# Display and positioning

**Parent:** [README.md](README.md)

How an entity participates in layout (`Display`), how its box is placed relative to its containing block (`Position`), and how anchored elements override that placement (`Anchor`).

## 1. `Display`

```rust
#[derive(Component, Reflect, Clone, PartialEq, Default)]
#[reflect(Component, Default)]
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
    Contents,                           // children promoted to grandparent (tier-E)
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

impl Default for Display { fn default() -> Self { Self::Block } }

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
| `Flex(_)`, `InlineFlex(_)` | `Flex` (Buiy passes the axis through `FlexParams.flex_direction`) |
| `Grid`, `InlineGrid` | `Grid` |
| `FlowRoot` | `Block` with internal containment marker (Taffy doesn't have a distinct flow-root) |
| `Contents` | Skipped during tree build; children re-parented to grandparent |
| `Table*` | `Block` (Taffy lacks table layout; Buiy emits a Buiy-side table pass — see [§ 1.2](#12-table-layout-status)) |
| `ListItem` | `Block` with `::marker` pseudo-element handling (deferred to v1.x) |
| `Ruby` | `Block` (deferred — tracks Taffy + i18n) |
| `None` | Entity is removed from the Taffy tree entirely |

### 1.2 Table layout status

Taffy 0.10 doesn't ship table layout. v1 implements semantic table layout (rows, cells, captions) as a Buiy-side post-Taffy pass that:

1. Gathers entities by `Display::Table*` family.
2. Computes column widths via Taffy on a synthetic flex container per row group.
3. Writes corrected positions back to `ResolvedLayout`.

This is one of the larger v1 deliverables. The pass runs as sub-pass 6b ([architecture.md § 3](architecture.md#3-system-pipeline)) inside the post-Taffy-overrides phase. Until table layout ships, `Display::Table*` falls back to `Block` with a `warn!` once per session.

### 1.3 `Display::None` vs `Visibility::Hidden`

`Display::None` removes the entity from layout *and* render. `Visibility::Hidden` (a render-side concern, owned by `buiy-render-pipeline-design`) keeps the entity in layout but skips painting. Author guidance: use `Display::None` when the entity should not contribute to size; use `Visibility::Hidden` when it should reserve space.

`inert` (foundation 3.1) and `Display::None` interact: `inert` removes the entity from focus + AccessKit + hit-testing while leaving layout intact; `Display::None` removes from layout. They compose freely.

### 1.4 `Contents`

`Display::Contents` is tier-E. Children are *re-parented to the grandparent* during step 1's tree build — the entity itself is not added to Taffy. Useful for wrapper components that shouldn't form their own box. Caveat: `Contents` and absolute-positioned children interact — the absolute-positioned child uses the grandparent as containing block. Spec asserts this in tests.

## 2. `Position`

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Position {
    pub kind: PositionKind,
    pub inset: Inset,
}

#[derive(Reflect, Clone, Copy, Default, PartialEq)]
pub enum PositionKind {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

#[derive(Reflect, Clone, Copy, Default)]
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
| `Absolute` | Nearest ancestor with `PositionKind != Static`, OR the layout viewport if none |
| `Fixed` | The layout viewport (the root entity's containing block) |
| `Sticky` | Nearest scroll container; falls back to parent's content box outside the sticky range |

The "nearest ancestor with `PositionKind != Static`" lookup runs in `SyncStyles` (system pipeline step 1) and is cached on a `ContainingBlock` component (private — synced, not author-set).

`Display::Contents` is transparent to containing-block resolution — descend through it.

### 2.2 Mapping to Taffy

Taffy 0.10 supports `position: absolute` (and `relative` via offsets); `fixed` is modeled as `absolute` with the layout viewport as containing block; `sticky` is a Buiy post-Taffy pass.

| `PositionKind` | Taffy emission |
|---|---|
| `Static` | `taffy::Position::Relative` with zero offsets (Taffy's "in-flow"). |
| `Relative` | `taffy::Position::Relative` with `inset` as offset. |
| `Absolute` | `taffy::Position::Absolute` with `inset`; child of `ContainingBlock`. |
| `Fixed` | `taffy::Position::Absolute` with `inset`; child of layout root. |
| `Sticky` | `taffy::Position::Relative`; sticky offsets applied in step 6 (anchor resolution shares the pass). |

### 2.3 Sticky positioning

A sticky element behaves as `Relative` until its scroll container's scroll offset crosses the inset threshold, then it sticks to the threshold edge until the element's parent leaves the threshold range. The pass runs as sub-pass 6a ([architecture.md § 3](architecture.md#3-system-pipeline)) so it sees fresh `ResolvedLayout` from Taffy plus current scroll offsets from the entity's nearest scroll container.

Sticky offsets *do not* invalidate Taffy. The element's contribution to its parent's flow is computed as `Relative`; the sticky displacement is a render-time visual offset baked into `ResolvedLayout.position` after Taffy.

## 3. Anchor positioning

Tier-C. Buiy-owned, post-Taffy. CSS Anchor Positioning Module Level 1.

### 3.1 `Anchor` component

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Anchor {
    pub anchor_name: Option<AnchorName>,        // declares this entity AS an anchor
    pub position_anchor: Option<AnchorRef>,     // declares this entity is anchored TO another
    pub position_try: Vec<PositionTry>,         // ordered fallback chain
}

#[derive(Reflect, Clone)]
pub struct PositionTry {
    pub inset: Inset,                           // anchored offset relative to the anchor's box
    pub conditions: Vec<TryCondition>,          // when this fallback is "valid"
}

#[derive(Reflect, Clone)]
pub enum TryCondition {
    FitsInViewport,                             // anchored box must not overflow the viewport
    FitsInContainer(AnchorRef),                 // anchored box must fit inside <ref>'s box
    AnchorVisible,                              // anchor's resolved rect intersects viewport
}

#[derive(Reflect, Clone)]
pub enum AnchorName {
    Implicit,                                   // referenced by Entity ID alone
    Named(SmolStr),                             // CSS-style name lookup (registered in NameRegistry resource)
}

#[derive(Reflect, Clone)]
pub enum AnchorRef {
    Entity(Entity),
    Name(SmolStr),
}
```

### 3.2 Resolution

Sub-pass 6d of the pipeline ([architecture.md § 3](architecture.md#3-system-pipeline)) walks every entity with `Anchor.position_anchor.is_some()`:

1. **Resolve anchor target.** Look up the anchor's `Entity` (by reference or named lookup) and read its `ResolvedLayout`.
2. **Try fallbacks in order.** For each `PositionTry` in `position_try`, compute the anchored entity's would-be box (using `inset` relative to the anchor) and evaluate every condition. The first try whose conditions all pass wins.
3. **Apply.** Override `ResolvedLayout.position` with the chosen try's resolved coordinates.
4. **Fallback failure.** If every try fails, position defaults to `(0, 0)` and the entity gets a `LayoutAnchorBroken` marker for devtools. A `warn!` fires once per (entity, frame).

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

- An anchor target must resolve before its dependent. Step 6 is single-pass topological — anchors that point at other anchored entities form a DAG; cycles are detected and broken with `warn!` (the cyclic edge is dropped).
- Cost: `O(anchored entities × tries)`. Usually small — most anchored elements have 1-3 fallbacks.
- Anchor resolution does **not** trigger Taffy re-layout. The anchor's *size* is fixed by the time anchor resolution runs; only its position changes. (Anchor *size* affecting layout — `anchor-size()` in CSS — is a tier-C feature deferred to v1.x; see open questions in [README § 5](README.md#5-open-questions).)

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
