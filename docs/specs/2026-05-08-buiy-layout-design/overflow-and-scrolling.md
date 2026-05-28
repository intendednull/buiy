# Overflow and scrolling

**Parent:** [README.md](README.md)

How an entity handles content that exceeds its box, and how scrolling — including snap, smooth-scroll, and overscroll — is exposed.

## 1. `Overflow`

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Overflow {
    pub x: OverflowMode,
    pub y: OverflowMode,
    pub scrollbar_gutter:  ScrollbarGutter,
    pub scrollbar_width:   ScrollbarWidth,
    pub scrollbar_color:   ScrollbarColor,
    pub scroll_behavior:   ScrollBehavior,
    pub overscroll_x:      OverscrollBehavior,
    pub overscroll_y:      OverscrollBehavior,
}

pub enum OverflowMode {
    Visible,    // default — children render outside the box, no clipping
    Hidden,     // clip; no scrolling, no scrollbar
    Clip,       // clip; like Hidden but creates no scroll container, ignores `scroll-padding` etc.
    Scroll,     // always show scrollbar (per axis)
    Auto,       // scrollbar shown only when content exceeds the box
}

pub enum ScrollbarGutter {
    Auto,       // gutter only when scroll is active
    Stable,     // gutter always reserved (avoids layout jump when scrollbar appears)
    StableBothEdges,
}

pub enum ScrollbarWidth {
    Auto, Thin, None,
}
```

**Planned (not yet shipped).** Logical aliases — `overflow-block` and `overflow-inline` — that translate to `x` / `y` based on the entity's `WritingModeResolved` ([container-queries-and-writing-modes.md § 2.3](container-queries-and-writing-modes.md#23-logical--physical-translation)) are future work; `map_overflow_mode` does no axis swap today. v1 ships only the physical `x` / `y` fields.

### 1.1 Mapping to Taffy

Taffy 0.10 has `overflow` awareness sufficient for sizing decisions (an `overflow: hidden` element doesn't expand its parent). The actual *clip rect* and scroll viewport are Buiy-rendering / Buiy-input-events concerns; this spec defines the data, not the rendering.

| `OverflowMode` | Taffy `overflow` field |
|---|---|
| `Visible` | `Visible` |
| `Hidden`, `Clip` | `Hidden` |
| `Scroll`, `Auto` | `Scroll` |

The distinction between `Scroll` and `Auto` (always-vs-conditional scrollbar) is rendering-side. Layout sees both as scrollable — content can exceed the container's box.

### 1.2 Scroll container

An entity is a *scroll container* if either axis's `OverflowMode` is `Scroll` or `Auto`. Scroll containers establish:

- A scroll viewport (the visible portion of children).
- A scroll position (`ScrollOffset` component, runtime state — see [§ 2](#2-scroll-state)).
- A containing block (the sticky-positioning reference frame) for descendants with `Position::Sticky`, resolved at runtime.

`Hidden` / `Clip` clip but do not scroll.

## 2. Scroll state

```rust
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollOffset {
    pub x: f32,
    pub y: f32,
}
```

Author-mutable. The scroll system in `buiy-input-events-design` writes to it in response to scroll events; the layout system *reads* it during sub-pass 6a (sticky offset) and step 7 (writing the displayed positions).

### 2.1 Effect on `ResolvedLayout`

`ResolvedLayout` reports the **content's** position relative to the entity's content box, *before* scroll offset is applied. Render and picking apply `ScrollOffset` separately when drawing/hit-testing. This separation:

- Keeps `ResolvedLayout` cacheable across frames where only scroll changed.
- Lets sticky positioning ([display-and-positioning.md § 2.3](display-and-positioning.md#23-sticky-positioning)) compute against the un-scrolled position then add the sticky displacement.

### 2.2 `scroll-behavior`

```rust
pub enum ScrollBehavior { Auto, Smooth }
```

Lives on `Overflow.scroll_behavior` (see § 1 struct definition). Programmatic scroll APIs (e.g. `entity.scroll_to(...)`) honor `Smooth` by interpolating `ScrollOffset` over a configurable duration. The interpolation system runs in `BuiySet::Animate`; layout doesn't care.

### 2.3 `overscroll-behavior`

```rust
pub enum OverscrollBehavior { Auto, Contain, None }
```

Per-axis. Lives on `Overflow.overscroll_x` / `overscroll_y` (see § 1 struct definition). `Contain` prevents scroll-chaining to ancestors; `None` additionally disables overscroll glow / bounce. Honored by `buiy-input-events-design`'s scroll handler. Layout stores it; doesn't act on it.

## 3. Scroll snap

Tier-C. CSS Scroll Snap Module Level 1.

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Scroll {
    pub snap_type:   SnapType,
    pub snap_padding: Edges,
    pub snap_margin:  Edges,
}

pub enum SnapType {
    None,
    XMandatory, XProximity,
    YMandatory, YProximity,
    BothMandatory, BothProximity,
}

#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollSnapItem {
    pub align: SnapAlign,
    pub stop:  SnapStop,
}

pub enum SnapAlign { None, Start, End, Center }
pub enum SnapStop { Normal, Always }
```

`Scroll` lives on the scroll container; `ScrollSnapItem` lives on each child that participates in snap. Snap point resolution runs in `buiy-input-events-design`'s scroll handler — it reads `ResolvedLayout` for snap candidates, computes the nearest snap point, and writes the target `ScrollOffset`.

Layout's role here is to provide accurate `ResolvedLayout` for snap math and to honor `snap_padding` (insets the snap viewport) and `snap_margin` (insets each snap item's snap rect).

## 4. Scrollbar styling

```rust
pub enum ScrollbarColor {
    Auto,
    Custom { thumb: Color, track: Color },
}
```

Render-side concern; layout stores the value. `buiy-render-pipeline-design` consumes.

## 5. Test surface

- **`OverflowMode::Visible` doesn't clip** — fixture parent 100×100 with a 200×100 child; assert child's `ResolvedLayout` extends beyond parent.
- **`OverflowMode::Hidden` clips** — same fixture with `Overflow::hidden()`; child's `ResolvedLayout` unchanged but render-side clip rect = parent box. (Render concern; this spec verifies that `Overflow` is correctly stored.)
- **Scroll container detection** — fixture with `OverflowMode::Auto` on x-axis only; assert entity is treated as scroll container in containing-block resolution for sticky descendants.
- **Deferred / not yet shipped: `ScrollbarGutter::Stable` reserves space** — fixture with `Stable` gutter on a non-scrolling container; assert content box is inset by scrollbar width regardless. (`Stable` does not yet reserve space — see `components.rs` / `types.rs`.)
- **Scroll offset doesn't invalidate layout** — fixture with content; modify `ScrollOffset`; assert `ResolvedLayout` is byte-equal across frames.
- **Deferred / not yet shipped: `overflow-block` / `overflow-inline` translate** — under `WritingMode::VerticalRl`, `overflow-block: hidden` translates to `x: Hidden` (block axis = x in vertical-rl). (The logical aliases are unbuilt — see § 1.)

## 6. Open: virtual scrolling

CSS doesn't define a "virtual scroll" primitive; it's implemented above layout. `buiy-widget-catalog-design` covers a virtual-list widget. This spec is only concerned with the scroll-container primitive.

## 7. Scroll-driven animations — deferred

CSS scroll-driven animations (`animation-timeline`, `scroll-timeline`, `view-timeline`) are foundation tier-E ([visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout)). They consume `ScrollOffset` (defined in § 2 above) but don't add layout-side state — the timeline plumbing lives in `buiy-animation-design`. This spec exposes the data scroll-driven animations need (`ScrollOffset` per scroll container, scroll bounds derivable from `ResolvedLayout`); the timeline machinery is deferred.
