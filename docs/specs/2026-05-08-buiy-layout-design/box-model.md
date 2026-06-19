# Box model and units

**Parent:** [README.md](README.md)

Sizing fundamentals: how a Buiy entity computes its width, height, padding, border, margin, and the unit system every other dimension references.

## 1. The box

Each laid-out entity has four nested boxes: **content**, **padding**, **border**, **margin**. CSS-faithful semantics; Taffy delivers them.

```
┌─────────────────────── margin box ───────────────────────┐
│                                                          │
│   ┌──────────────── border box ─────────────────────┐    │
│   │                                                 │    │
│   │   ┌──────────── padding box ──────────────┐     │    │
│   │   │                                       │     │    │
│   │   │   ┌────────── content box ────────┐   │     │    │
│   │   │   │                               │   │     │    │
│   │   │   └───────────────────────────────┘   │     │    │
│   │   │                                       │     │    │
│   │   └───────────────────────────────────────┘     │    │
│   │                                                 │    │
│   └─────────────────────────────────────────────────┘    │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

`ResolvedLayout` reports the **border box** (`position`, `size`). Padding and margin are queryable by reading the `BoxModel` component. Render reads `ResolvedLayout`; styling needs (e.g. drawing the border) read both.

## 2. `BoxModel`

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct BoxModel {
    pub width:    Sizing,
    pub height:   Sizing,
    pub min_width:  Sizing,
    pub min_height: Sizing,
    pub max_width:  Sizing,
    pub max_height: Sizing,
    pub padding:  Edges,
    pub margin:   Edges,
    pub border:   Edges,
    pub box_sizing: BoxSizing,
    pub aspect_ratio: Option<AspectRatio>,
}
```

Flex/grid gap is carried by `FlexParams.gap` / `GridParams.gap` (the `FlexGap` type — see [flex-and-grid.md](flex-and-grid.md)), not by `BoxModel`. A block-layout `gap` shorthand on `BoxModel` is deferred to a follow-up phase that wires block-axis gap to Taffy.

Defaults (derived `Default`):

- `width` / `height` = `Sizing::Auto`
- `min_*` / `max_*` = `Sizing::Auto` — `Sizing`'s derived default. This is more CSS-faithful than `Px(0)` / `None`: CSS's initial value for `min-width` / `min-height` is `auto`, resolved contextually (Taffy applies the automatic minimum-size rule).
- `padding` / `margin` / `border` = `Edges::ZERO`
- `box_sizing` = `BoxSizing::ContentBox` (CSS default)
- `aspect_ratio` = `None`

### 2.1 `Edges` — physical-or-logical edge values

```rust
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: Length::ZERO,
        right: Length::ZERO,
        bottom: Length::ZERO,
        left: Length::ZERO,
    };
    pub fn all(v: f32) -> Self;
    pub fn axis(x: f32, y: f32) -> Self;
    // ... etc
}
```

For logical-property authoring, use the separate `LogicalEdges` type (see [§ 4](#4-logical-properties)) — it carries the four logical edges and translates to physical `Edges` via `to_edges(mode, direction)` at construct time.

### 2.2 `BoxSizing`

```rust
pub enum BoxSizing {
    ContentBox,    // CSS default: width/height set the content box
    BorderBox,     // width/height set the border box (padding+border subtracted from content)
}
```

Most app UIs prefer `BorderBox`. There is no global override (no `* { box-sizing: border-box }` analogue): the component default stays `ContentBox` to match CSS, and authors opt into `BorderBox` per entity.

### 2.3 `AspectRatio`

```rust
pub struct AspectRatio {
    pub ratio: f32,             // width / height; 16/9 = 1.777..
}
```

`BoxModel.aspect_ratio = Some(AspectRatio { ratio: 16.0/9.0 })` matches CSS `aspect-ratio: 16/9`. CSS `aspect-ratio: auto` (intrinsic dimensions take precedence) is represented by *not setting* the ratio — `BoxModel.aspect_ratio == None`, the field being `Option<AspectRatio>`.

Replaced elements (image, video, canvas — see foundation 3.1) feed their intrinsic ratio through this component automatically. Authors override by setting `aspect_ratio` to an explicit `Some(_)`.

## 3. `Sizing` — the size value type

```rust
pub enum Sizing {
    Auto,
    None,                       // valid only on max-*
    Length(Length),
    MinContent,                 // CSS `min-content`
    MaxContent,                 // CSS `max-content`
    FitContent(Length),         // CSS `fit-content(<length>)`
    Stretch,                    // CSS `stretch`
}
```

### 3.1 Intrinsic sizing

`MinContent` / `MaxContent` / `FitContent` query the entity's content for its preferred size:

- **Containers** propagate to children; result = sum (block axis) or max (inline axis).
- **Replaced elements**: intrinsic dimensions or aspect-ratio.
- **Text** (target state): the text shaper computes shrink-to-fit width. The query interface is owned by this spec; the text-shaper implementation lives in `buiy-text-rendering-design`. v1 falls back to `Auto` until text-rendering integrates.

Phase 0 ships only `Auto` and `Length`; intrinsic keywords resolve to `Auto` until text-rendering lands. Plans coordinate the cutover.

### 3.2 `Stretch`

Fills the parent's free space along the affected axis. CSS-WG `stretch` keyword.

## 4. Logical properties

Every physical-axis property has a logical-axis sibling:

| Physical | Logical |
|---|---|
| `width` | `inline-size` |
| `height` | `block-size` |
| `padding-top` | `padding-block-start` |
| `padding-right` | `padding-inline-end` |
| `padding-bottom` | `padding-block-end` |
| `padding-left` | `padding-inline-start` |
| `margin-*`, `border-*`, `inset-*` | `*-block-start`, `*-block-end`, `*-inline-start`, `*-inline-end` |
| `min-width` / `max-width` | `min-inline-size` / `max-inline-size` |
| `min-height` / `max-height` | `min-block-size` / `max-block-size` |

### 4.1 API shape

`BoxModel` stores **physical** values (`width`, `height`, `padding.top`, etc.) — the canonical form Taffy consumes. Authors who want logical authoring use the `LogicalBoxModel` *builder* — a plain struct (not a `Component`) whose `to_box_model(&WritingMode)` they call explicitly to produce a `BoxModel`:

```rust
let bm = LogicalBoxModel {
    inline_size: Sizing::Length(Length::Px(320.0)),
    block_size:  Sizing::Auto,
    padding: LogicalEdges {
        block_start:  Length::Px(16.0),
        block_end:    Length::Px(16.0),
        inline_start: Length::Px(24.0),
        inline_end:   Length::Px(24.0),
    },
    .. default()
}
.to_box_model(&writing_mode);
```

`to_box_model` resolves against the passed `WritingMode` (its `mode` + `direction`): vertical modes swap inline ↔ block onto width ↔ height, and physical edges follow the `LogicalEdges` 6-row table. The author then passes the returned `BoxModel` into `Style`. `LogicalBoxModel` is never inserted as a component and there is no on-insert auto-resolution — translation happens at construct time, on the author's side. (The same builder pattern applies to `Position` insets via `LogicalInset` — see [display-and-positioning.md](display-and-positioning.md).)

This keeps Taffy talking only physical values while letting authors think in logical ones. Reflection / BSN / inspectors see `BoxModel`; the logical builder is a Rust ergonomic.

### 4.2 Why not store logical?

Storing logical and translating per-frame would require knowing the writing-mode at every read. A theme switch that flips writing-mode would invalidate every cached translation. Storing physical means the writing-mode is consulted exactly once, when the author calls `to_box_model`; a later writing-mode change requires re-running the builder, but there is no per-frame re-translation pass and no `Changed<WritingMode>` tracking. Cost is paid on the boundary, not on every read.

## 5. Units

The variants and constructors marked **units/calc()** below are *target state* — they are not yet shipped. The current `Length` (Phases 1/3/5) ships only `Px`, `Percent`, `Fr`, and the container-query family (`Cqw`/`Cqh`/`Cqi`/`Cqb`/`Cqmin`/`Cqmax`), with `px()` / `percent()` constructors. Font-relative, viewport, and `Calc` variants are the unbuilt `buiy-layout-units-calc` follow-up — an unscheduled phase, tracked in `follow-ups.md`. (It is *not* the landed Phase 10, which shipped `position: fixed`; the original "Phase 10" planning number was reused for position-fixed, so this work is referenced by its plan slug, not a number.)

```rust
pub enum Length {
    // --- shipped ---
    Px(f32),
    Percent(f32),               // relative to containing block
    Fr(f32),                    // grid fractional unit (only valid in GridParams)
    Cqw(f32), Cqh(f32),         // container-query units
    Cqi(f32), Cqb(f32),
    Cqmin(f32), Cqmax(f32),
    // --- buiy-layout-units-calc follow-up (unscheduled), not yet shipped ---
    Em(f32),                    // relative to current font-size
    Rem(f32),                   // relative to root font-size
    Vw(f32), Vh(f32),           // viewport units
    Vmin(f32), Vmax(f32),
    Svw(f32), Svh(f32),         // small viewport (mobile UA bars retracted)
    Lvw(f32), Lvh(f32),         // large viewport (mobile UA bars expanded)
    Dvw(f32), Dvh(f32),         // dynamic viewport (live)
    Calc(Box<CalcExpr>),        // calc()/min()/max()/clamp() tree
}

impl Length {
    pub const ZERO: Self = Self::Px(0.0);
    pub fn px(v: f32) -> Self;
    pub fn percent(v: f32) -> Self;
    // --- buiy-layout-units-calc follow-up (unscheduled), not yet shipped ---
    pub fn rem(v: f32) -> Self;
    pub fn calc(expr: CalcExpr) -> Self;
    // ... etc
}
```

### 5.1 Resolution

Each unit resolves at one of four points:

1. **`Px`** — already absolute; no resolution.
2. **`Em` / `Rem` / viewport / container** — resolved by `style_to_taffy` against:
   - `Em`: current font-size (resolved via `buiy-text-rendering-design`'s font cascade; falls back to `16px` until text-rendering integrates).
   - `Rem`: root font-size resource (`RootFontSize`, default `16px`).
   - Viewport: `bevy::window::Window` size.
   - Container (`Cq*`): nearest queried ancestor (see [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md)).
3. **`Percent` / `Fr`** — passed through to Taffy as a percent / fractional dimension; `style_to_taffy` does *not* pre-resolve them. `Percent` resolves against the containing block (axis-dependent) inside Taffy; `Fr` is resolved only by Taffy's grid algorithm.
4. **`Calc`** — recursively resolves operands, then evaluates the expression (`+ - * /`, `min()`, `max()`, `clamp()`). Resolution happens *before* Taffy sees the value.

### 5.2 `Calc`

**`buiy-layout-units-calc` follow-up (unscheduled) — not implemented.** `CalcExpr`, the `Length::Calc` variant, and the `Length::calc` constructor ship together in that follow-up; nothing in this section exists in the current code.

```rust
pub enum CalcExpr {
    Length(Length),
    Add(Box<CalcExpr>, Box<CalcExpr>),
    Sub(Box<CalcExpr>, Box<CalcExpr>),
    Mul(Box<CalcExpr>, f32),
    Div(Box<CalcExpr>, f32),
    Min(Vec<CalcExpr>),
    Max(Vec<CalcExpr>),
    Clamp(Box<CalcExpr>, Box<CalcExpr>, Box<CalcExpr>),
}
```

`Length::calc(min![percent(100.0), px(800.0)])` resolves to `min(containing_block_inline, 800px)`.

CSS `calc()` arithmetic rules apply: `Length + Length` is `Length`; `Length * f32` is `Length`; type errors panic in debug, silently use `Length::ZERO` in release with a `warn!`.

### 5.3 Resolution timing

Unit resolution happens during `SyncStyles` (system pipeline step 1) — *before* Taffy compute. Container-unit resolution is special: the container's resolved size from *step 3 of the previous frame* drives this frame's container-unit math. (Same-frame container-unit refresh would require a cycle: container size → child size → container size. The same-frame re-layout strategy ([architecture.md § 3.2](architecture.md#32-container-query-re-layout)) doesn't change this — re-layout flips activation, not unit values.)

This is documented behavior, not a bug. The lag is one frame and matches container-query activation lag for transitive cases.

## 6. Test surface

- **`BoxSizing::ContentBox` vs `BorderBox`** — fixture asserting `width: 100px, padding: 10px` produces 100px content box (`ContentBox`) vs 80px content box (`BorderBox`).
- **Aspect ratio** (target test, not yet present) — fixture with `width: auto, height: 100px, aspect_ratio: 16/9` produces `width: 177.7..px`. The `aspect_ratio → taffy` wiring exists (`translate.rs` populates `s.aspect_ratio`) but is not asserted by a box-model fixture; `layout_box_sizing.rs` covers only the `ContentBox`/`BorderBox` cases.
- **Intrinsic sizing fall-through** (target test, not yet present) — until text-rendering integrates, `MinContent` produces `Auto` semantics; a test should assert this so the cutover is visible. The behavior exists (`translate.rs`'s `sizing_to_dim` maps `MinContent` / `MaxContent` / `FitContent` to Taffy `auto`) but no test currently asserts it.
- **Logical → physical translation** — a test constructs `LogicalBoxModel` with `inline_size: 100px` and calls `to_box_model(&wm)` for a `VerticalRl` writing mode; assert the resulting `BoxModel.height == 100px` (`LogicalBoxModel` is a builder, not a `Component` — see [§ 4.1](#41-api-shape)).
- **Unit resolution** — percentage resolution against the containing block is exercised in `layout_container_queries.rs` (`Cqw(50)` of an 800px container resolves to `400px`); the exact `100%` → `800px` box-model fixture is not present. (`2rem` resolving to `2 × root_font_size` is a **`buiy-layout-units-calc`** target test, not yet present.) `Cqw(50)` resolution lives under container-queries, not a box-model fixture — see [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md).
- **`Calc` evaluation** (**`buiy-layout-units-calc`** target test, not yet present) — `min(100%, 800px)` of a 600px container = 600px; of a 1000px container = 800px.
