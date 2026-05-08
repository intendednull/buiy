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
#[derive(Component, Reflect, Clone, Default)]
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
    pub gap: Option<Length>,            // shorthand; flex/grid override
    pub row_gap: Option<Length>,
    pub column_gap: Option<Length>,
}
```

Defaults:

- `width` / `height` = `Sizing::Auto`
- `min_*` = `Sizing::Length(Length::Px(0.0))`
- `max_*` = `Sizing::None`
- `padding` / `margin` / `border` = `Edges::ZERO`
- `box_sizing` = `BoxSizing::ContentBox` (CSS default)
- `aspect_ratio` = `None`
- `gap` / `row_gap` / `column_gap` = `None`

### 2.1 `Edges` — physical-or-logical edge values

```rust
#[derive(Reflect, Clone, Copy, Default, PartialEq)]
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
    pub fn logical(start: f32, end: f32, block_start: f32, block_end: f32) -> LogicalEdges;
    // ... etc
}
```

For logical-property authoring, use `LogicalEdges` (see [§ 4](#4-logical-properties)). Translation to physical edges happens during `style_to_taffy` based on the entity's effective writing-mode + direction.

### 2.2 `BoxSizing`

```rust
pub enum BoxSizing {
    ContentBox,    // CSS default: width/height set the content box
    BorderBox,     // width/height set the border box (padding+border subtracted from content)
}
```

Most app UIs prefer `BorderBox`. Buiy's default theme sets `BorderBox` globally via a `BuiyDefaults` plugin override (analogous to `* { box-sizing: border-box }`). The component default stays `ContentBox` to match CSS.

### 2.3 `AspectRatio`

```rust
pub struct AspectRatio {
    pub ratio: f32,             // width / height; 16/9 = 1.777..
    pub auto:  bool,            // true = auto; intrinsic dimensions take precedence
}
```

`AspectRatio { ratio: 16.0/9.0, auto: false }` matches CSS `aspect-ratio: 16/9`. `auto: true` matches CSS `aspect-ratio: auto`.

Replaced elements (image, video, canvas — see foundation 3.1) feed their intrinsic ratio through this component automatically. Authors override by setting `auto: false`.

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

`BoxModel` stores **physical** values (`width`, `height`, `padding.top`, etc.) — the canonical form Taffy consumes. Authors who want logical authoring use a `LogicalBoxModel` *insert helper*:

```rust
LogicalBoxModel {
    inline_size: Sizing::Length(Length::Rem(20.0)),
    block_size:  Sizing::Auto,
    padding: LogicalEdges {
        block_start:  Length::Rem(1.0),
        block_end:    Length::Rem(1.0),
        inline_start: Length::Rem(1.5),
        inline_end:   Length::Rem(1.5),
    },
    .. default()
}
```

On insert, `LogicalBoxModel` resolves against the entity's effective `WritingMode` + `direction` and writes a `BoxModel` with the corresponding physical fields. The `LogicalBoxModel` component is *not* stored — it's an insert-time transform. (The same pattern applies to `Position` insets — see [display-and-positioning.md](display-and-positioning.md).)

This keeps Taffy talking only physical values while letting authors think in logical ones. Reflection / BSN / inspectors see `BoxModel`; the logical helper is a Rust ergonomic.

### 4.2 Why not store logical?

Storing logical and translating per-frame would require knowing the writing-mode at every read. A theme switch that flips writing-mode would invalidate every cached translation. Storing physical means a writing-mode switch invalidates only entities whose `LogicalBoxModel` was the source — which `Style`'s `Bundle` insertion already tracks via `Changed<WritingMode>` propagating to a re-translation pass. Cost is paid on the boundary, not on every read.

## 5. Units

```rust
pub enum Length {
    Px(f32),
    Percent(f32),               // relative to containing block
    Em(f32),                    // relative to current font-size
    Rem(f32),                   // relative to root font-size
    Vw(f32), Vh(f32),           // viewport units
    Vmin(f32), Vmax(f32),
    Svw(f32), Svh(f32),         // small viewport (mobile UA bars retracted)
    Lvw(f32), Lvh(f32),         // large viewport (mobile UA bars expanded)
    Dvw(f32), Dvh(f32),         // dynamic viewport (live)
    Cqw(f32), Cqh(f32),         // container-query units
    Cqi(f32), Cqb(f32),
    Cqmin(f32), Cqmax(f32),
    Fr(f32),                    // grid fractional unit (only valid in GridParams)
    Calc(Box<CalcExpr>),        // calc()/min()/max()/clamp() tree
}

impl Length {
    pub const ZERO: Self = Self::Px(0.0);
    pub fn px(v: f32) -> Self;
    pub fn percent(v: f32) -> Self;
    pub fn rem(v: f32) -> Self;
    pub fn calc(expr: CalcExpr) -> Self;
    // ... etc
}
```

### 5.1 Resolution

Each unit resolves at one of three points:

1. **`Px`** — already absolute; no resolution.
2. **`Percent` / `Em` / `Rem` / viewport / container** — resolved by `style_to_taffy` against:
   - `Percent`: containing block dimension (axis-dependent).
   - `Em`: current font-size (resolved via `buiy-text-rendering-design`'s font cascade; falls back to `16px` until text-rendering integrates).
   - `Rem`: root font-size resource (`RootFontSize`, default `16px`).
   - Viewport: `bevy::window::Window` size.
   - Container: nearest queried ancestor (see [container-queries-and-writing-modes.md](container-queries-and-writing-modes.md)).
3. **`Fr`** — passed through to Taffy untouched; only Taffy's grid algorithm resolves it.
4. **`Calc`** — recursively resolves operands, then evaluates the expression (`+ - * /`, `min()`, `max()`, `clamp()`). Resolution happens *before* Taffy sees the value.

### 5.2 `Calc`

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
- **Aspect ratio** — fixture with `width: auto, height: 100px, aspect_ratio: 16/9` produces `width: 177.7..px`.
- **Intrinsic sizing fall-through** — until text-rendering integrates, `MinContent` produces `Auto` semantics; this is asserted so the cutover is visible.
- **Logical → physical translation** — fixture inserts `LogicalBoxModel` with `inline_size: 100px` under `WritingMode::VerticalRl`; assert resulting `BoxModel.height == 100px`.
- **Unit resolution** — `100%` of a 800px container resolves to `800px`; `2rem` resolves to `2 × root_font_size`; `Cqw(50)` resolves to half the queried container's inline axis.
- **`Calc` evaluation** — `min(100%, 800px)` of a 600px container = 600px; of a 1000px container = 800px.
