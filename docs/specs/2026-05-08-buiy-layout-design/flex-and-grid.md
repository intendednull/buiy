# Flex, Grid, and multi-column

**Parent:** [README.md](README.md)

The two algorithms Taffy ships in full — Flexbox and CSS Grid — and the one Buiy adds on top: multi-column.

## 1. Flexbox

Tier-F. Full CSS Flexbox via Taffy. Buiy delegates the algorithm; the Buiy contract is the component shape.

### 1.1 `FlexParams` (on the flex container)

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct FlexParams {
    pub direction: FlexAxis,             // Row | Column | RowReverse | ColumnReverse
    pub wrap: FlexWrap,                  // NoWrap | Wrap | WrapReverse
    pub justify_content: JustifyContent, // FlexStart | FlexEnd | Center | SpaceBetween | SpaceAround | SpaceEvenly
    pub align_items: AlignItems,         // FlexStart | FlexEnd | Center | Baseline | Stretch
    pub align_content: AlignContent,     // FlexStart | FlexEnd | Center | SpaceBetween | SpaceAround | SpaceEvenly | Stretch
    pub gap: FlexGap,                    // { row, column }
}
```

`FlexParams` only takes effect when the entity's `Display` is `Display::Flex(_)` or `Display::InlineFlex(_)`. Otherwise it's ignored (no `warn!` — non-flex entities can carry `FlexParams` for future-display switches).

`Display::Flex(axis)` and `FlexParams.direction` carry redundant information. The canonical source is `FlexParams.direction`; `Display::Flex(axis)` writes the axis into `FlexParams` via the `Style` builder. If both are set explicitly and disagree, `FlexParams.direction` wins.

### 1.2 `FlexItem` (on flex children)

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct FlexItem {
    pub grow: f32,                      // CSS flex-grow
    pub shrink: f32,                    // CSS flex-shrink (default 1.0)
    pub basis: Sizing,                  // CSS flex-basis (default Auto)
    pub order: i32,                     // CSS order (default 0)
    pub align_self: Option<AlignItems>, // None = inherit from parent's align_items
}
```

### 1.3 Builder ergonomics

```rust
Style::default()
    .flex_row()                          // sets Display + FlexAxis::Row
    .justify_content(JustifyContent::SpaceBetween)
    .align_items(AlignItems::Center)
    .gap(Length::Rem(1.0))
```

Setting `.flex_row()` after `.flex_column()` overwrites the axis. The builder's fluent methods are commutative *only* within the same domain — the last call within a domain wins. Cross-domain order is irrelevant.

## 2. Grid

Tier-F. Full CSS Grid via Taffy.

### 2.1 `GridParams` (on the grid container)

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct GridParams {
    pub template_columns: Vec<TrackSize>,
    pub template_rows:    Vec<TrackSize>,
    pub template_areas:   Option<GridAreas>,
    pub auto_columns:     Vec<TrackSize>,
    pub auto_rows:        Vec<TrackSize>,
    pub auto_flow:        GridAutoFlow,         // Row | Column | RowDense | ColumnDense
    pub justify_items:    JustifyItems,
    pub align_items:      AlignItems,
    pub justify_content:  JustifyContent,
    pub align_content:    AlignContent,
    pub gap:              FlexGap,
}

pub enum TrackSize {
    Length(Length),                            // px, %, fr, etc.
    MinMax(Box<TrackSize>, Box<TrackSize>),    // CSS minmax()
    Repeat(RepeatCount, Vec<TrackSize>),       // CSS repeat()
    Auto, MinContent, MaxContent, FitContent(Length),
}

pub enum RepeatCount {
    AutoFill,
    AutoFit,
    Count(u32),
}
```

`Length::Fr(f32)` is only meaningful inside `TrackSize::Length(Length::Fr(_))`; using `Fr` outside grid is a `warn!` and falls back to `Auto`.

### 2.2 `GridItem` (on grid children)

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct GridItem {
    pub column: GridLine,                       // GridLine::span(2) | GridLine::start_end(1, 4) | GridLine::area("header") | GridLine::Auto
    pub row:    GridLine,
    pub justify_self: Option<JustifyItems>,
    pub align_self:   Option<AlignItems>,
}

pub enum GridLine {
    Auto,
    Start(i32),                                 // 1-indexed; negative counts from end
    Span(u32),
    StartEnd(i32, i32),
    Area(SmolStr),                              // resolved against parent's GridParams.template_areas
}
```

### 2.3 Subgrid

CSS `subgrid` value on `template-columns` / `template-rows`. Tracks Taffy upstream — Buiy ships subgrid when Taffy ships it. The API stub:

```rust
TrackSize::Subgrid       // future variant
```

is reserved. Until Taffy lands subgrid, `TrackSize::Subgrid` falls back to the parent's grid template by inheritance and emits a `warn!` once per session naming the limitation. (Plans coordinate the cutover.)

### 2.4 Masonry

Tier-E. CSS-WG flux. Not shipped. The `GridAutoFlow::Masonry` variant is reserved for forward compatibility but currently falls back to `GridAutoFlow::Row` with `warn!`.

## 3. Multi-column

Tier-E. CSS Multi-column Layout Module Level 1. Not in Taffy; Buiy-owned.

### 3.1 `MultiColumn` component

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct MultiColumn {
    pub column_count: ColumnCount,              // Auto | Count(u32)
    pub column_width: Option<Length>,
    pub column_gap:   Option<Length>,
    pub column_rule:  ColumnRule,               // width, style, color
    pub column_span:  ColumnSpan,               // None | All
    pub column_fill:  ColumnFill,               // Balance | Auto
    pub break_inside: BreakInside,              // Auto | Avoid | AvoidColumn
    pub break_before: BreakBefore,              // Auto | Always | Avoid | Column | AvoidColumn
    pub break_after:  BreakAfter,
}
```

### 3.2 Algorithm

A multi-column container's layout is computed in two stages:

1. **Determine column count** — from explicit `column_count`, or computed from `column_width` + container width + `column_gap`.
2. **Lay out children into columns** — Buiy walks children and packs them into columns, respecting `break-*` properties. Implementation detail: this runs as a Buiy pass between system pipeline steps 5 and 6 ([architecture.md § 3](architecture.md#3-system-pipeline)) — call it step 5c, after table layout (5b) and before anchor resolution. Children's `ResolvedLayout.position` is overwritten.

Multi-column is tier-E; v1 ships the API but the algorithm is a stub that produces single-column layout with `warn!` once per session. Prioritization waits on user demand.

## 4. Mixing display types

`Display::Flex` and `Display::Grid` containers are mutually exclusive at the container level — a single entity can't be both. A flex container's children can themselves be grid containers and vice versa; Taffy handles the nesting. This composes freely with `Position::Absolute` children, which escape both algorithms (their layout uses the absolute-positioning rules in [display-and-positioning.md § 2](display-and-positioning.md#2-position)).

## 5. Test surface

- **Flex direction** — `flex_row` lays children left-to-right; `flex_column` top-to-bottom; reverses reverse.
- **Flex grow/shrink** — three children with grow `[1, 2, 1]` in a 400px row distribute 100/200/100.
- **Flex wrap** — overflow forces wrap; `wrap_reverse` inverts cross-axis order.
- **Grid template** — `1fr 2fr 1fr` columns in a 400px row produce 100/200/100.
- **Grid named areas** — fixture with `template_areas` and child `GridItem.column = Area("header")`; assert correct cell.
- **Grid `repeat(auto-fill, ...)`** — fixture with `auto-fill` columns sized 100px in a 350px container; assert 3 columns + 50px slack.
- **Subgrid stub warns** — until Taffy lands subgrid, `TrackSize::Subgrid` produces inherited template + one `warn!`.
- **Multi-column stub warns** — `MultiColumn::column_count = Count(3)` produces single-column layout + one `warn!` (reverts once the algorithm ships).
- **Mixed flex-in-grid** — fixture nests a `Display::Flex(Row)` inside a `Display::Grid` cell; assert flex children are laid out within the cell's resolved box.
