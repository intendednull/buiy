# Flex, Grid, and multi-column

**Parent:** [README.md](README.md)

The two algorithms Taffy ships in full — Flexbox and CSS Grid — and the one Buiy adds on top: multi-column.

## 1. Flexbox

Tier-F. Full CSS Flexbox via Taffy. Buiy delegates the algorithm; the Buiy contract is the component shape.

### 1.1 `FlexParams` (on the flex container)

`FlexParams` is the only `Component` here; the inner enums and `FlexGap` are
plain value types and carry no `#[reflect(Component, Default)]` attribute.

```rust
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct FlexParams {
    pub direction: FlexAxis,             // Row | Column | RowReverse | ColumnReverse
    pub wrap: FlexWrap,                  // NoWrap | Wrap | WrapReverse
    pub justify_content: JustifyContent, // FlexStart | FlexEnd | Center | SpaceBetween | SpaceAround | SpaceEvenly
    pub align_items: AlignItems,         // Stretch | FlexStart | FlexEnd | Center | Baseline
    pub align_content: AlignContent,     // Stretch | FlexStart | FlexEnd | Center | SpaceBetween | SpaceAround | SpaceEvenly
    pub gap: FlexGap,                    // { row, column }
}

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexAxis { #[default] Row, Column, RowReverse, ColumnReverse }

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexWrap { #[default] NoWrap, Wrap, WrapReverse }

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyContent { #[default] FlexStart, FlexEnd, Center, SpaceBetween, SpaceAround, SpaceEvenly }

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems { #[default] Stretch, FlexStart, FlexEnd, Center, Baseline }

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignContent { #[default] Stretch, FlexStart, FlexEnd, Center, SpaceBetween, SpaceAround, SpaceEvenly }

#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct FlexGap { pub row: Length, pub column: Length }
```

`FlexParams` only takes effect when the entity's `Display` is `Display::Flex(_)` or `Display::InlineFlex(_)`. Otherwise it's ignored (no `warn!` — non-flex entities can carry `FlexParams` for future-display switches).

`Display::Flex(axis)` and `FlexParams.direction` carry redundant information. The canonical source is `FlexParams.direction`; `Display::Flex(axis)` writes the axis into `FlexParams` via the `Style` builder. If both are set explicitly and disagree, `FlexParams.direction` wins.

### 1.2 `FlexItem` (on flex children)

```rust
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct FlexItem {
    pub grow: f32,                      // CSS flex-grow
    pub shrink: f32,                    // CSS flex-shrink (default 1.0)
    pub basis: Sizing,                  // CSS flex-basis (default Auto)
    pub order: i32,                     // CSS order (default 0)
    pub align_self: Option<AlignItems>, // None = inherit from parent's align_items
}
```

`Default` is a hand-written `impl` rather than `#[derive(Default)]` — `flex-shrink`
defaults to `1.0` (not the field's natural `0.0`), matching the CSS initial value. For
the same reason `#[reflect(Component)]` carries no `Default` (no `#[reflect(Default)]`).

### 1.3 Builder ergonomics

```rust
Style::default()
    .flex_row()                          // sets Display + FlexAxis::Row
    .justify_content(JustifyContent::SpaceBetween)
    .align_items(AlignItems::Center)
    .gap_px(16.0)
```

Setting `.flex_row()` after `.flex_column()` overwrites the axis. The builder's fluent methods are commutative *only* within the same domain — the last call within a domain wins. Cross-domain order is irrelevant.

## 2. Grid

Tier-F. Full CSS Grid via Taffy.

### 2.1 `GridParams` (on the grid container)

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
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
    MinMax(Vec<TrackSize>),                    // CSS minmax(); expected len() == 2 ([min, max])
    Repeat(RepeatCount, Vec<TrackSize>),       // CSS repeat()
    Auto, MinContent, MaxContent, FitContent(Length),
}

pub enum RepeatCount {
    AutoFill,
    AutoFit,
    Count(u16),
}
```

`MinMax` stores its two arguments as a `Vec<TrackSize>` (expected `len() == 2`, `[min, max]`) rather than the more natural `(Box<TrackSize>, Box<TrackSize>)` because `bevy_reflect` 0.18 has no `Reflect` impl for `Box<T>`. Translation validates the arity and emits a warn-once, falling back to `Auto`, if it isn't exactly 2. `RepeatCount::Count` is `u16` to match Taffy 0.10's `RepetitionCount::Count(u16)` directly (65 535 repetitions is well above any realistic UI grid).

`Length::Fr(f32)` is only meaningful inside `TrackSize::Length(Length::Fr(_))`; using `Fr` outside grid warns once and falls back to `Auto` (or `0`px where the Taffy target type has no `Auto` variant — gap/padding/border, which translate through `length_to_lp`).

### 2.2 `GridItem` (on grid children)

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct GridItem {
    pub column: GridLine,                       // GridLine::span(2) | GridLine::start_end(1, 4) | GridLine::area("header") | GridLine::Auto
    pub row:    GridLine,
    pub justify_self: Option<JustifyItems>,
    pub align_self:   Option<AlignItems>,
}

pub enum GridLine {
    Auto,
    Start(i16),                                 // 1-indexed; negative counts from end
    Span(u16),
    StartEnd(i16, i16),
    Area(String),                               // resolved against parent's GridParams.template_areas
}
```

`Start` / `Span` / `StartEnd` use `i16` / `u16` to match Taffy 0.10's `GridLine` / `Span` underlying types directly. `Area` uses `String` rather than `SmolStr` to avoid a new direct dependency — area names are set once per spawn and never touched on a hot path.

### 2.3 Subgrid

CSS `subgrid` value on `template-columns` / `template-rows`. Tracks Taffy upstream — Buiy ships full subgrid behavior when Taffy ships it. The API variant:

```rust
TrackSize::Subgrid
```

ships today as a present stub. Taffy 0.10 has no subgrid support, so until it lands, `TrackSize::Subgrid` falls back to Taffy `Auto` (`track_to_*` in `translate.rs`) and emits a `warn!` once per session naming the limitation (`warn_once_subgrid`). The fallback is plain `Auto`, *not* inheritance of the parent's grid template — that richer behavior arrives with the Taffy cutover. (Plans coordinate the cutover.)

### 2.4 Masonry

Tier-E. CSS-WG flux. Not shipped. The `GridAutoFlow::Masonry` variant is reserved for forward compatibility but currently falls back to `GridAutoFlow::Row` with `warn!`.

## 3. Multi-column

Tier-E. CSS Multi-column Layout Module Level 1. Not in Taffy; Buiy-owned.

### 3.1 `MultiColumn` component

```rust
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug, Default)]
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

`Eq` is intentionally *not* derived: `column_width` / `column_gap` carry `Length` (`f32`)
and `column_rule.color` is a `bevy::color::Color`, neither of which is `Eq`. `PartialEq`
is sufficient for tests and authoring ergonomics.

### 3.2 Algorithm

A multi-column container's layout is computed in two stages:

1. **Determine column count** — from explicit `column_count`, or computed from `column_width` + container width + `column_gap`.
2. **Lay out children into columns** — Buiy walks children and packs them into columns, respecting `break-*` properties, overwriting children's `ResolvedLayout.position`. This runs as sub-pass 6c of the post-Taffy-overrides phase ([architecture.md § 3](architecture.md#3-system-pipeline)), after table layout (6b) and before anchor resolution (6d).

**Implemented (Phase 13).** Sub-pass 6c (`multicol_pack` in `systems.rs`) is a real position-only overlay, structurally mirroring the table sub-pass (6b): Taffy lays the container + children out in block flow first, then 6c reads the container's Taffy content box + each child's Taffy size, resolves the used column count, packs the in-flow children into columns, and writes corrected **parent-relative** child positions into the shared `PostTaffyPositionOverrides` map. Sizes are never touched — only positions.

- **Column-count resolution** is the pure `resolve_column_count` helper implementing the CSS Multicol L1 § 7.3 used-value algorithm (count-only / width-only / both / neither). When both `column_count` and `column_width` are set, `column-count` is treated as a *maximum*; columns expand to fill the content box.
- **Packing** is the pure `pack_columns` greedy whole-child packer: children flow into columns in document order, top-to-bottom, until the next child would exceed the column block-size, then start the next column. Forced breaks (`break-before` / `break-after` = `Column` / `Always`) start a new column at the child boundary. In-flow children are the container's direct `Children` minus `Display::None` (no box) and `Position::Absolute` / `Fixed` (out of flow — they keep their Taffy position).
- `column_width` / `column_gap` resolve `Px` only in v1 (percent / cq column metrics deferred); the gap fallback for CSS `normal` is `0.0` (pre-font-metrics).

**Still deferred (tier-E).** True content *fragmentation* — splitting a single child's box across a column boundary — is not implemented: Buiy produces one rect per entity, so a child taller than its column is placed whole and overflows. Break-*avoidance* (`break-inside: avoid`, `break-before/after: avoid`) is likewise a best-effort no-op (whole-child packing trivially satisfies `break-inside: avoid`; avoidance breaks that need backtracking/balancing are not honored). Both gaps are reported by a session-wide `LayoutWarnOnceKey::MulticolFragmentationDeferred` warn-once, which fires when a child whose block-size exceeds the resolved column block-size is encountered under `column_fill: Balance` (balanced fill assumes divisible content). That key carries **no `Entity` payload**, so it fires exactly once per session *in total*.

The Phase-7 stub's blanket `LayoutWarnOnceKey::MulticolUnsupported` warn is **retired** — it is kept as a `Reflect`-stable variant that no code emits (the same retire pattern as `TableUnsupported`), since the packing algorithm now ships and the blanket "unsupported" warn would be wrong for the common, fully-supported case.

## 4. Mixing display types

`Display::Flex` and `Display::Grid` containers are mutually exclusive at the container level — a single entity can't be both. A flex container's children can themselves be grid containers and vice versa; Taffy handles the nesting. This composes freely with `Position::Absolute` children, which escape both algorithms (their layout uses the absolute-positioning rules in [display-and-positioning.md § 2](display-and-positioning.md#2-position)).

## 5. Test surface

- **Flex direction** — `flex_row` lays children left-to-right; `flex_column` top-to-bottom; reverses reverse.
- **Flex grow/shrink** — three children with grow `[1, 2, 1]` in a 400px row distribute 100/200/100.
- **Flex wrap** — overflow forces wrap; `wrap_reverse` inverts cross-axis order.
- **Grid template** — `1fr 2fr 1fr` columns in a 400px row produce 100/200/100.
- **Grid named areas** — fixture with `template_areas` and child `GridItem.column = Area("header")`; assert correct cell.
- **Grid `repeat(auto-fill, ...)`** — fixture with `auto-fill` columns sized 100px in a 350px container; assert 3 columns + 50px slack.
- **Subgrid stub warns** — until Taffy lands subgrid, `TrackSize::Subgrid` falls back to Taffy `Auto` + one `warn!`.
- **Multi-column packing** (Phase 13) — a `column_count = Count(2)` container with three fixed-size children packs them greedily into two columns (col 0 fills top-to-bottom, overflow spills to col 1); `resolve_column_count` unit-tests the four CSS used-value cases; `pack_columns` unit-tests greedy fill + forced `break-before` / `break-after` starting a new column; a child taller than the column under `column_fill: Balance` produces whole-child overflow + exactly one session-wide `MulticolFragmentationDeferred` `warn!` (the residual fragmentation gap).
- **Mixed flex-in-grid** — fixture nests a `Display::Flex(Row)` inside a `Display::Grid` cell; assert flex children are laid out within the cell's resolved box.
