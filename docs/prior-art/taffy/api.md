**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — public API surface (types, traits, builder patterns)

# Taffy — public API

The shape Buiy talks to. Two API tiers (high-level `TaffyTree`, low-level traits), one `Style` struct, a small set of geometric value types, and a const-friendly construction pattern. See [architecture.md](architecture.md) for the structural rationale, [layout-algorithms.md](layout-algorithms.md) for what each algorithm covers.

## 1. High-level: `TaffyTree`

```rust
use taffy::prelude::*;

let mut tree: TaffyTree<()> = TaffyTree::new();
let leaf = tree.new_leaf(Style { size: Size { width: length(100.0), height: length(50.0) }, ..Default::default() })?;
let root = tree.new_with_children(Style::DEFAULT, &[leaf])?;
tree.compute_layout(root, Size::MAX_CONTENT)?;
let layout = tree.layout(root)?;       // &Layout { location, size, content_size, border, padding, scrollbar_size, order }
```

**Construction.**

- `TaffyTree::new() -> Self` — empty tree.
- `TaffyTree::with_capacity(n: usize) -> Self` — preallocate node storage.

**Node creation.**

- `new_leaf(style) -> TaffyResult<NodeId>` — a leaf node.
- `new_leaf_with_context(style, ctx: NodeContext) -> TaffyResult<NodeId>` — leaf carrying user data (used by the measure-function path for text/image leaves).
- `new_with_children(style, &[NodeId]) -> TaffyResult<NodeId>` — branch node with explicit children.

**Mutation.**

- `set_style(node, style) -> TaffyResult<()>` — replace style, mark dirty.
- `set_children(parent, &[NodeId]) -> TaffyResult<()>` — replace children, remove from prior parents, mark dirty (children-from-other-parent removal is 0.7+ behavior).
- `add_child(parent, child)`, `remove_child(parent, child)`, `replace_child_at_index(parent, idx, new)`, `remove_children_range(parent, range)` (0.7.7+).
- `set_node_context(node, Option<ctx>) -> TaffyResult<()>` — replace / clear context.
- `remove(node) -> TaffyResult<NodeId>` — detach + drop. **Returns `Err(InvalidInputNode)` if already removed** — this is the behavior Buiy's `LayoutTree` GC must tolerate ([Buiy architecture.md § 4.3](../../specs/2026-05-08-buiy-layout-design/architecture.md#43-despawn--the-gc-contract)).
- `clear()` — wipe the entire tree.
- `mark_dirty(node) -> TaffyResult<()>` — manually invalidate cache (rarely needed; `set_style`/`set_children` auto-dirty).

**Layout invocation.**

- `compute_layout(root, available_space: Size<AvailableSpace>) -> TaffyResult<()>` — the common entry.
- `compute_layout_with_measure(root, available, measure_fn) -> TaffyResult<()>` — measure-function path (text + image leaves). The closure has signature `FnMut(Size<Option<f32>>, Size<AvailableSpace>, NodeId, Option<&mut NodeContext>, &Style) -> Size<f32>`.

**Reading results.**

- `layout(node) -> TaffyResult<&Layout>` — final (rounded if `use_rounding` config is on, default true).
- `unrounded_layout(node) -> TaffyResult<&Layout>` (0.7.1+) — pre-round version, needed for parent-position composition.
- `enable_rounding()` / `disable_rounding()` — global config.
- `child_count`, `child_at_index`, `children(node)`, `parent(node)`, `total_node_count`.
- `print_tree(root)` (std-only) — debug dump.
- `write_tree(writer, root)` (0.9.3+) — same, to an arbitrary `core::fmt::Write`.
- `detailed_layout_info(node) -> Option<&DetailedLayoutInfo>` (0.7.2+) — Grid-specific computed track sizes.

## 2. Low-level: the trait approach

For embedders that own their own node storage (Servo, Blitz, Bevy, Slint, Dioxus, Buiy-eventually). Implement the traits against your tree; call standalone `compute_*` functions.

```rust
use taffy::*;

impl TraversePartialTree for MyTree {
    type ChildIter<'a> = std::slice::Iter<'a, NodeId> where Self: 'a;
    fn child_ids(&self, p: NodeId) -> Self::ChildIter<'_> { /*…*/ }
    fn child_count(&self, p: NodeId) -> usize             { /*…*/ }
    fn get_child_id(&self, p: NodeId, i: usize) -> NodeId { /*…*/ }
}

impl LayoutPartialTree for MyTree {
    type CoreContainerStyle<'a> = &'a Style where Self: 'a;
    type CustomIdent = std::sync::Arc<str>;
    fn get_core_container_style(&self, id: NodeId) -> &Style { /*…*/ }
    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) { /*…*/ }
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            let style = tree.get_core_container_style(id);
            match style.display() {
                Display::Flex  => compute_flexbox_layout(tree, id, inputs),
                Display::Grid  => compute_grid_layout(tree, id, inputs),
                Display::Block => compute_block_layout(tree, id, inputs),
                Display::None  => compute_hidden_layout(tree, id, inputs),
            }
        })
    }
}

// Implement LayoutFlexboxContainer, LayoutGridContainer, LayoutBlockContainer as needed.
// Implement CacheTree if you want Taffy's frame-to-frame caching.
// Implement TraverseTree + RoundTree + PrintTree if you want round_layout / print_tree.

compute_root_layout(&mut my_tree, root_id, Size::MAX_CONTENT);
round_layout(&mut my_tree, root_id);
```

**The full trait list:**

- `TraversePartialTree` — single-level child access. Required by everything.
- `LayoutPartialTree` — get style, set layout, recurse into children's layout. The hub.
- `LayoutFlexboxContainer` — provides `FlexboxContainerStyle` (per-node) and `FlexboxItemStyle` (per-child).
- `LayoutGridContainer` — `GridContainerStyle` + `GridItemStyle`. Plus optional `DetailedGridInfo` setter (feature `detailed_layout_info`).
- `LayoutBlockContainer` — `BlockContainerStyle` + `BlockItemStyle`.
- `CacheTree` — `cache_get` / `cache_store` / `cache_clear`. Split out of `LayoutPartialTree` in **0.7.0**.
- `TraverseTree` — marker; full-tree traversal.
- `RoundTree` — for `round_layout`.
- `PrintTree` — for `print_tree`.

The per-algorithm container traits each expose only the style fields *that algorithm cares about* — so an embedder using only Flexbox can implement `FlexboxContainerStyle` and ignore the Grid surface. This is the same pattern Servo uses to back Taffy with its own DOM-derived style structs.

## 3. The `Style` struct

```rust
pub struct Style<S: CheapCloneStr = DefaultCheapStr> {
    pub dummy: PhantomData<S>,
    pub display: Display,                                            // Block | Flex | Grid | None
    pub item_is_table: bool,
    pub item_is_replaced: bool,
    pub box_sizing: BoxSizing,                                       // BorderBox (default) | ContentBox
    pub direction: Direction,                                        // Ltr (default) | Rtl
    pub overflow: Point<Overflow>,                                   // {x, y} : Visible | Clip | Hidden | Scroll
    pub scrollbar_width: f32,

    // 0.10+ (feature = "float_layout")
    pub float: Float,                                                // None | Left | Right | InlineStart | InlineEnd
    pub clear: Clear,                                                // None | Left | Right | Both | InlineStart | InlineEnd

    pub position: Position,                                          // Relative (default) | Absolute  — ONLY two variants
    pub inset: Rect<LengthPercentageAuto>,

    pub size: Size<Dimension>,                                       // width, height
    pub min_size: Size<Dimension>,
    pub max_size: Size<Dimension>,
    pub aspect_ratio: Option<f32>,                                   // width / height

    pub margin: Rect<LengthPercentageAuto>,
    pub padding: Rect<LengthPercentage>,
    pub border: Rect<LengthPercentage>,

    // (feature = "flexbox" OR "grid")
    pub align_items:     Option<AlignItems>,
    pub align_self:      Option<AlignSelf>,
    pub align_content:   Option<AlignContent>,
    pub justify_content: Option<JustifyContent>,
    pub gap: Size<LengthPercentage>,

    // (feature = "grid")
    pub justify_items: Option<AlignItems>,
    pub justify_self:  Option<AlignSelf>,

    // (feature = "block_layout")
    pub text_align: TextAlign,                                       // Auto | Start | End | LegacyLeft | LegacyCenter | LegacyRight

    // (feature = "flexbox")
    pub flex_direction: FlexDirection,                               // Row (default) | Column | RowReverse | ColumnReverse
    pub flex_wrap:      FlexWrap,                                    // NoWrap (default) | Wrap | WrapReverse
    pub flex_basis:     Dimension,
    pub flex_grow:      f32,
    pub flex_shrink:    f32,                                         // default 1.0

    // (feature = "grid")
    pub grid_template_rows:        Vec<GridTemplateComponent<S>>,
    pub grid_template_columns:     Vec<GridTemplateComponent<S>>,
    pub grid_template_areas:       Vec<GridTemplateArea<S>>,         // 0.9+
    pub grid_template_row_names:   Vec<Vec<S>>,                      // 0.9+
    pub grid_template_column_names:Vec<Vec<S>>,                      // 0.9+
    pub grid_auto_rows:            Vec<TrackSizingFunction>,
    pub grid_auto_columns:         Vec<TrackSizingFunction>,
    pub grid_auto_flow:            GridAutoFlow,                     // Row | Column | RowDense | ColumnDense
    pub grid_row:                  Line<GridPlacement<S>>,
    pub grid_column:               Line<GridPlacement<S>>,
}
```

Source: [`src/style/mod.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/style/mod.rs).

**Mapping to CSS.** Every field maps to the CSS property of the obvious name (`flex_direction` → `flex-direction`, `grid_template_columns` → `grid-template-columns`, etc.). The exceptions:

- `Position::Relative` is Taffy's default, contrary to CSS where `static` is default. Static is essentially absent — `Relative` with zero inset behaves equivalently for everything Taffy models.
- `item_is_table` and `item_is_replaced` have no CSS-property equivalents — they are sizing-mode hints used because Taffy doesn't model real `display: table*` or replaced-element families.
- `Style.direction` is the CSS `direction` property (LTR/RTL). It is *not* `writing-mode`; writing-mode is unsupported.

## 4. Value types

```rust
// All length-shaped fields are CompactLength-backed tagged pointers (0.8+).

pub struct Length          (CompactLength);  // alias: a non-percent length helper

pub struct LengthPercentage(CompactLength);  // length(f32) | percent(f32 in 0..=1) | (calc-feature) calc(*const ())

pub struct LengthPercentageAuto(CompactLength);  // ↑ + auto

pub struct Dimension(CompactLength);  // ↑ + auto  (additional helpers planned for min/max-content per #751)

pub struct Size<T>  { pub width: T, pub height: T }
pub struct Point<T> { pub x: T,     pub y: T }
pub struct Rect<T>  { pub left: T, pub right: T, pub top: T, pub bottom: T }
pub struct Line<T>  { pub start: T, pub end: T }
```

**Construction helpers** (in `taffy::prelude::*`):

```rust
length(50.0)          // → LengthPercentage / LengthPercentageAuto / Dimension
percent(0.5)          // → 50% in [0..=1]
auto()                // → LengthPercentageAuto::auto / Dimension::auto
fr(1.0)               // → MaxTrackSizingFunction::fr  (Grid only)
min_content()         // → MinTrackSizingFunction (Grid intrinsic-track)
max_content()
fit_content(arg)
minmax(min, max)      // → TrackSizingFunction (TrackSizingFunction = (Min, Max))
zero()
```

**Percentages.** Are stored in `[0.0, 1.0]` not `[0.0, 100.0]`. `percent(0.5)` = 50%. The CHANGELOG comment is explicit and Buiy mirrors this convention.

**Grid-specific value types:**

- `TrackSizingFunction = (MinTrackSizingFunction, MaxTrackSizingFunction)` — the `minmax(min, max)` shape is canonical.
- `MinTrackSizingFunction`: `Auto | MinContent | MaxContent | Length(f32) | Percent(f32) | Calc(*const ())`.
- `MaxTrackSizingFunction`: same + `Fr(f32) | FitContent(LengthPercentage)`.
- `GridTemplateComponent<S>`: `Single(TrackSizingFunction) | Repeat(GridTemplateRepetition<S>)`.
- `RepetitionCount`: `AutoFill | AutoFit | Count(u16)`.
- `GridPlacement<S>`: `Auto | Line(i16) | Span(u16) | NamedLine(S, i16) | NamedSpan(S, u16)`.
- `GridAutoFlow`: `Row | Column | RowDense | ColumnDense`.
- `GridTemplateArea<S>` (0.9+): `{ name: S, row_start, row_end, column_start, column_end }`.

## 5. Enums

- `Display`: `Block` | `Flex` | `Grid` | `None`. **No inline / inline-block / inline-flex / inline-grid / table* / list-item / ruby / contents / flow-root**. The brief asked about an "inline variant story" — there is no inline variant.
- `Position`: `Relative` | `Absolute`. **Only two variants** — no `Static`, no `Fixed`, no `Sticky`. Embedders fake sticky in a post-Taffy pass (Buiy does this in sub-pass 6a).
- `Overflow`: `Visible` | `Clip` | `Hidden` | `Scroll`. Semantics:
  - `Visible`: layout's auto-min-size is content-based; overflowing content contributes to ancestor scroll regions.
  - `Clip`: auto-min-size still content-based; overflow does *not* contribute to ancestor scroll. Restricts `content_size` reported in `Layout`.
  - `Hidden`: auto-min-size becomes 0 (so flex/grid items can shrink past content); no contribution to ancestor scroll.
  - `Scroll`: auto-min-size becomes 0 AND reserves `scrollbar_width` pixels in the *opposite* axis for a scrollbar. `Scroll` with `scrollbar_width = 0.0` ≡ `Hidden`.
  - There is no `Overflow::Auto` ("scroll if needed"). Embedders fake `auto` by setting `Scroll` and rendering a scrollbar only if `content_size > size`. Buiy carries this distinction in `Overflow::y_auto()` and resolves it at write-time.
  - These describe *layout* effects only. Painting/clipping is the embedder's job.
- `BoxSizing`: `BorderBox` (default — contrary to CSS where `content-box` is default) | `ContentBox`. Affects `size`, `min_size`, `max_size`, `flex_basis`.
- `Direction`: `Ltr` (default) | `Rtl`. Inline-axis flow only; not `writing-mode`.
- `AlignItems`: `Start | End | FlexStart | FlexEnd | Center | Baseline | Stretch`.
- `AlignSelf` = `AlignItems` (alias).
- `AlignContent`: same + `SpaceBetween | SpaceAround | SpaceEvenly`.
- `JustifyContent` = `AlignContent` (alias).
- `JustifyItems` = `AlignItems` (alias).
- `JustifySelf` = `AlignItems` (alias).

## 6. Const construction

Taffy promotes a const-friendly construction pattern: `Style::DEFAULT` is `const`, and the value-helpers (`length`, `percent`, `auto`, `zero`) are all `const fn`. Static styles can therefore live as `static FOO: Style = Style { ... }` without runtime overhead. Example from the codebase:

```rust
const ROW_STYLE: Style = Style {
    display: Display::Flex,
    flex_direction: FlexDirection::Row,
    gap: Size { width: LengthPercentage::length(8.0), height: LengthPercentage::length(8.0) },
    ..Style::DEFAULT
};
```

This pattern is what Buiy's `Style` builder ([Buiy architecture.md § 2.2](../../specs/2026-05-08-buiy-layout-design/architecture.md#22-style--builder--ergonomic-authoring-layer)) compiles to once expanded. The compact-length tagged pointer representation (`CompactLength`, 0.8+) is itself `const`-constructible, which is why this works post-0.8.

## 7. Errors

`TaffyError`:

- `ChildIndexOutOfBounds { parent, child_index, child_count }`
- `InvalidParentNode(NodeId)`
- `InvalidChildNode(NodeId)`
- `InvalidInputNode(NodeId)`

Layout itself never returns `Err` from `compute_layout` once the tree is well-formed — errors are all topology errors (node-not-in-tree, child-index-OOB). Buiy's error model treats them as "warn once, retain prior frame's layout" ([Buiy architecture.md § 6](../../specs/2026-05-08-buiy-layout-design/architecture.md#6-error-model)).

## 8. The `parse` feature (0.10+)

With `feature = "parse"`, every style value type except the top-level `Style` struct implements `FromStr` for its CSS string form:

```rust
let lp: LengthPercentage = "30px".parse()?;
let lp: LengthPercentage = "50%".parse()?;
let a: LengthPercentageAuto = "auto".parse()?;
let d: Display = "flex".parse()?;
```

Full `Style` parsing from `;`-separated declarations is roadmapped but not shipped. `parse_faster` enables `cssparser`'s proc-macro fast path at the cost of pulling `syn` into the build graph.

## Sources

- `src/style/mod.rs` (Style, Display, Position, Overflow, Direction, BoxSizing): https://github.com/DioxusLabs/taffy/blob/main/src/style/mod.rs
- `src/style/dimension.rs` (Length value types): https://github.com/DioxusLabs/taffy/blob/main/src/style/dimension.rs
- `src/tree/traits.rs` (all eight traits): https://github.com/DioxusLabs/taffy/blob/main/src/tree/traits.rs
- `src/tree/taffy_tree.rs` (TaffyTree, TaffyError): https://github.com/DioxusLabs/taffy/blob/main/src/tree/taffy_tree.rs
- `src/style_helpers.rs` (length, percent, auto, fr helpers): https://github.com/DioxusLabs/taffy/blob/main/src/style_helpers.rs
- README usage example: https://github.com/DioxusLabs/taffy/blob/main/README.md
- docs.rs Style page: https://docs.rs/taffy/0.10.1/taffy/struct.Style.html
- CHANGELOG (CompactLength 0.8, named lines/areas 0.9, FromStr 0.10): https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Buiy layout architecture (Style-builder + decomposed components): [`docs/specs/2026-05-08-buiy-layout-design/architecture.md`](../../specs/2026-05-08-buiy-layout-design/architecture.md)
