**Date:** 2026-05-08
**Status:** draft

# Buiy layout design

## Purpose

Define the target shape of Buiy's layout subsystem: how layout properties live on entities, how they are resolved, and how the system stages [Taffy](https://github.com/DioxusLabs/taffy) computations alongside Buiy-owned features that Taffy does not provide.

This spec graduates the `buiy-layout-design` slot from the foundation roadmap (`docs/specs/2026-05-07-buiy-foundation/README.md` § 4). It refines the layout topics catalogued in foundation `visuals.md` § 3.2 — box model, display modes, positioning, container queries, writing-mode integration, anchor positioning, units, and overflow basics.

## Scope

**In scope (medium tier):**
- Decomposed component model (`Display`, `Size`, `BoxSpace`, `Position`, `FlexLayout`, `GridLayout`, `Overflow`, `WritingMode`, `AnchorName`/`AnchorTo`, `ContainerContext`).
- Single `Length` value type covering `px`, `%`, `em`, `rem`, `vw`/`vh`/`vmin`/`vmax`, `cqw`/`cqh`/`cqi`/`cqb`, `auto`, `fr`, `fit-content`/`min-content`/`max-content`, and `calc()`.
- Pipeline stages: resolution → Taffy sync → compute → container-query re-evaluation → anchor resolution → writeback.
- Container queries (Buiy-defined typed-effect API; not parsed CSS).
- Anchor positioning with fallback chains (`position-try`).
- Writing-mode component (forwarded to Taffy where supported; vertical modes deferred).
- Migration from Phase 0's monolithic `Style` (in `crates/buiy_core/src/components.rs`) to the decomposed components.
- LayoutTree GC via `RemovedComponents<LayoutNode>`.

**Out of scope (handled by sibling specs):**
- Stacking-context computation, `z-index` paint order, top-layer compositing — `buiy-render-pipeline-design`.
- `transform`, `clip-path`, `mask`, `filter`, `backdrop-filter`, `mix-blend-mode` — they affect paint, not layout — `buiy-render-pipeline-design`.
- Scroll offset, scroll-snap behavior, scrollbar styling, `overflow: scroll` UX — `buiy-overflow-and-scrolling-design` (future spec; this spec defines the `Overflow` component shape only).
- Scroll-driven animations — `buiy-animation-design`.
- Multi-column layout — out for v1 (E tier in foundation).
- `float`/`clear` — out (legacy).
- Subgrid, masonry, ruby — gated on Taffy roadmap; absorbed when Taffy ships.

## 1. Component model

Layout properties are split across small, public-fielded, change-detectable components. Aligns with foundation goal #3 (BSN-native, decomposed by concern). Replaces Phase 0's monolithic `Style`.

### 1.1 Always-present components

A `LayoutNode` marker component opts an entity into layout. Children of a layout node also need `LayoutNode` to participate.

```rust
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct LayoutNode;

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Display { pub kind: DisplayKind }

pub enum DisplayKind {
    Block, Inline, InlineBlock,
    Flex, InlineFlex,
    Grid, InlineGrid,
    FlowRoot,
    None,        // removes entity + subtree from layout, focus, picking, AccessKit.
    Contents,    // children participate in parent's layout as if this entity didn't exist.
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Size {
    pub width: Length, pub height: Length,
    pub min_width: Length, pub min_height: Length,
    pub max_width: Length, pub max_height: Length,
    pub aspect_ratio: Option<f32>,
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct BoxSpace {
    pub padding: Edges<Length>,
    pub margin:  Edges<Length>,
    pub border:  Edges<Length>,
    pub box_sizing: BoxSizing,    // ContentBox | BorderBox; default BorderBox.
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Position {
    pub kind: PositionKind,       // Static | Relative | Absolute | Fixed | Sticky
    pub inset: Edges<Length>,     // top/right/bottom/left, with logical aliases.
}
```

`Sticky` is supported via a Buiy-owned post-Taffy adjustment pass (Stage E.5) — see § 3 and open question § 10.4.

`Edges<T>` is `{ top, right, bottom, left }` plus logical accessors that resolve via `WritingMode`:

```rust
pub struct Edges<T> { pub top: T, pub right: T, pub bottom: T, pub left: T }
impl<T: Copy> Edges<T> {
    pub fn block_start(&self, wm: &WritingMode) -> T { ... }
    pub fn inline_start(&self, wm: &WritingMode) -> T { ... }
    // etc.
}
```

For LTR horizontal-tb (the only mode Taffy ships today) the logical aliases are direct passthroughs. Vertical modes light up later.

### 1.2 Opt-in components

Present only when the entity actually uses the feature. Layout system queries `Option<&FlexLayout>` etc. so absent components are zero-cost.

```rust
#[derive(Component, Reflect)]
pub struct FlexLayout {
    pub direction: FlexDirection, // Row | Column | RowReverse | ColumnReverse
    pub wrap: FlexWrap,           // NoWrap | Wrap | WrapReverse
    pub justify: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub gap: Size2D<Length>,      // {row, column}
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub align_self: Option<AlignSelf>,
}

#[derive(Component, Reflect)]
pub struct GridLayout {
    pub template_rows: GridTemplate,    // Vec<TrackSize>
    pub template_columns: GridTemplate,
    pub auto_rows: TrackSize,
    pub auto_columns: TrackSize,
    pub auto_flow: GridAutoFlow,
    pub gap: Size2D<Length>,
    pub areas: GridAreas,               // optional named-area grid
    // grid item placement on this entity:
    pub row: GridLine,                  // Auto | Span(n) | Line(i32) | Named
    pub column: GridLine,
    pub row_span: u16,
    pub column_span: u16,
}

#[derive(Component, Reflect)]
pub struct Overflow {
    pub x: OverflowMode,    // Visible | Hidden | Clip | Scroll | Auto
    pub y: OverflowMode,
}

#[derive(Component, Reflect)]
pub struct WritingMode {
    pub mode: WritingModeKind,    // HorizontalTb (only working variant in v1)
    pub direction: Direction,     // Ltr | Rtl
}

// An entity without `WritingMode` is treated as { HorizontalTb, Ltr }. Layout
// system uses `WritingMode::default()` for resolution when the component is absent.

#[derive(Component, Reflect)]
pub struct AnchorName(pub SmolStr);

#[derive(Component, Reflect)]
pub struct AnchorTo {
    pub anchor: SmolStr,
    pub side: AnchorSide,         // Top | Bottom | Start | End | Center | etc.
    pub align: AnchorAlign,
    pub inset_offset: Length,
    pub fallbacks: Vec<PositionTry>,
}

#[derive(Component, Reflect)]
pub struct ContainerContext {
    pub kind: ContainerType,      // Normal | InlineSize | Size
    pub name: Option<SmolStr>,
}

#[derive(Component, Reflect)]
pub struct ContainerQueries { pub queries: Vec<ContainerQuery> }
```

### 1.3 Output

```rust
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct ResolvedLayout {
    pub position: Vec2,    // top-left, in containing-block coordinates.
    pub size: Vec2,
}

#[derive(Component, Reflect)]
pub struct AnchorResolved {
    pub fallback_index: usize,    // which `position-try` won.
    pub absolute_rect: Rect,
}
```

`ResolvedLayout` is unchanged from Phase 0 to keep the render pipeline stable across the migration. `AnchorResolved` is new; render reads it instead of re-running the fallback chain.

## 2. Length values

A single enum covers every CSS-like length kind:

```rust
pub enum Length {
    Px(f32),
    Percent(f32),
    Em(f32), Rem(f32),
    Vw(f32), Vh(f32), Vmin(f32), Vmax(f32),
    Cqw(f32), Cqh(f32), Cqi(f32), Cqb(f32),
    Auto,
    Fr(f32),
    FitContent, MinContent, MaxContent,
    FitContentFn(Box<Length>),    // fit-content(<length>)
    Token(TokenId),               // theme token reference
    Calc(Box<LengthExpr>),
}

pub enum LengthExpr {
    Lit(Length),
    Add(Box<LengthExpr>, Box<LengthExpr>),
    Sub(Box<LengthExpr>, Box<LengthExpr>),
    Mul(Box<LengthExpr>, f32),    // typed: length * number
    Div(Box<LengthExpr>, f32),
    Min(Vec<LengthExpr>),
    Max(Vec<LengthExpr>),
    Clamp(Box<LengthExpr>, Box<LengthExpr>, Box<LengthExpr>),
}
```

**Tradeoff (recorded).** A single `Length` enum trades compile-time correctness for ergonomics. Taffy's three-type split (`Dimension` / `LengthPercentage` / `LengthPercentageAuto`) prevents `padding: auto` from typechecking. Buiy accepts that as a runtime check (Stage A coerces invalid context to `Px(0.0)` and emits a `warn!`), in exchange for one type that flows uniformly through builders, BSN, and `calc()` trees. Re-evaluate if instrumentation shows misuse is common.

**Construction ergonomics.**

```rust
let w: Length = 200.0.px();
let h: Length = 50.0.percent();
let p: Length = Length::calc(
    LengthExpr::lit(100.0.percent())
        .sub(LengthExpr::lit(16.0.px())),
);
```

A `length!()` macro that parses CSS-ish syntax (`length!("100% - 16px")`) is **deferred**; this spec calls it out as a follow-up but does not specify it.

## 3. Layout pipeline

Runs in `BuiySet::Layout`. Multi-stage because container queries can require re-resolution.

### Stage A — Length resolution (layout-independent)

For every `LayoutNode` entity, resolve every `Length` whose value does not depend on the layout pass itself: `Px`, `Em` (parent-resolved font size — read from theme), `Rem` (root font size), `Vw`/`Vh`/`Vmin`/`Vmax` (window), `Auto`, `FitContent`/`MinContent`/`MaxContent`, `Token` (theme lookup).

`Percent` and `Fr` are **passed verbatim to Taffy** — Taffy resolves percent against the containing block during compute and `fr` during grid track sizing. Stage A does not pre-evaluate them.

`Calc` resolution is split: a calc tree whose leaves are all context-free (`Px`, `Em`, `Rem`, `Vw`/`Vh`/`Vmin`/`Vmax`, `Token`) is fully evaluated to a single `Px` in Stage A. A calc tree containing `Percent`, `Fr`, or `Cq*` is evaluated through Taffy's compact-length representation (`taffy::Dimension::calc` when the `calc` feature is enabled) — Stage A flattens the tree into Taffy's representation but defers numeric evaluation to Taffy's compute step. If Taffy cannot represent the expression (e.g., `min`/`max`/`clamp` mixing percent and px in v1), Stage A coerces to the first context-free term and emits `warn!(target: "buiy::layout::calc_unsupported", ...)`.

Defer: `Cqw`/`Cqh`/`Cqi`/`Cqb` — container-query units need the parent container's resolved size, and the inline-/block-axis swap depends on the entity's `WritingMode`. Marked "deferred" in an internal `LengthResolutionState` and revisited in Stage D.

### Stage B — Sync to Taffy

For each `LayoutNode`:
- If no Taffy node exists in `LayoutTree.by_entity`, create one.
- If any of `Display`, `Size`, `BoxSpace`, `Position`, `FlexLayout`, `GridLayout`, `Overflow`, `WritingMode` changed (`Changed<T>` query), rebuild the `taffy::Style` and call `set_style`.
- Sync child relationships from the entity's `Children` to Taffy's `set_children`.

### Stage C — Compute

For each root layout node (entity with `LayoutNode` and either no `ChildOf` or a `ChildOf` whose parent lacks `LayoutNode`):

```rust
taffy.compute_layout(root, taffy::Size {
    width: AvailableSpace::Definite(window.width()),
    height: AvailableSpace::Definite(window.height()),
});
```

### Stage D — Container-query re-evaluation

Skip if no entity carries `ContainerContext`. Otherwise:

1. Walk the tree top-down. For each `ContainerContext` ancestor, compute its resolved inline-size + block-size from the just-computed Taffy output.
2. For each `ContainerQueries` component in the subtree, evaluate every query's `ContainerCondition` against the nearest matching container's size.
3. For queries that became active (or inactive) since the last frame, apply their `ContainerQueryEffect` (see § 4) — typically inserting/removing components.
4. Resolve any `Cqw`/`Cqh`/`Cqi`/`Cqb` lengths that depended on a now-known container size.
5. If any effect changed an entity's layout-relevant components OR any deferred `cq*` length now has a value, re-run Stages A–C.
6. Cap iterations at **3**. On overflow, emit `warn!(target: "buiy::layout::container_query_iteration", entity = ?)`, freeze the next iteration's state, and continue.

### Stage E — Anchor resolution

For each `AnchorTo`:
1. Resolve `anchor` (the name) via the `AnchorRegistry` resource (see § 5).
2. Compute the candidate rect for the default placement (`side` + `align` + `inset_offset`) relative to the anchor's `ResolvedLayout`.
3. Containing-block check: `Position::Fixed` → window; otherwise nearest positioned ancestor.
4. If the candidate rect overflows its containing block, walk `fallbacks` until one fits. If none fit, use the last fallback and emit `debug!(target: "buiy::layout::anchor_fallback_exhausted", ...)`.
5. Compute the entity's containing-block-relative position from the absolute rect and the containing-block origin.
6. Insert/update `AnchorResolved { fallback_index, absolute_rect }` on the entity (`absolute_rect` is in window coordinates, for renderer/picking convenience).

Anchor resolution does **not** trigger another Stage C — anchored elements are positioned out-of-flow; their absolute rect does not feed back into sibling layout.

### Stage E.5 — Sticky offset

For each entity with `Position::kind == Sticky`, compute the offset to keep the box inside its scroll container's visible region (clamped by `inset` thresholds). Adjusts `ResolvedLayout::position` directly; runs after anchor resolution because anchored elements never use sticky.

Skipped entirely if no entity carries `Position::kind == Sticky` and skipped per-entity if the entity has no scrollable ancestor (in which case Sticky degrades to Relative). The full sticky algorithm lives in `buiy-overflow-and-scrolling-design`; this spec commits to running the offset adjustment in this stage.

### Stage F — Writeback

Walk `LayoutTree.by_entity`. For each entity:
- If the entity has `AnchorResolved`, the layout system already wrote `ResolvedLayout` in Stage E — skip Taffy writeback for that entity (Taffy's output for an anchored element is meaningless because anchor placement is out-of-flow).
- Otherwise read the Taffy layout and insert/update `ResolvedLayout` (CB-relative coordinates per § 1.3).

### Stage G — Garbage collection

A separate system (`gc_layout_tree`) reads `RemovedComponents<LayoutNode>` and removes the matching `TaffyNodeId` entries. Closes the Phase 0 TODO.

## 4. Container queries — typed-effect API

No CSS to parse. Queries and their effects are typed Rust:

```rust
pub struct ContainerQuery {
    pub container: ContainerSelector,    // Nearest | Named(SmolStr) | Type(ContainerType)
    pub condition: ContainerCondition,
    pub effect: ContainerQueryEffect,
}

pub enum ContainerCondition {
    MinWidth(Length), MaxWidth(Length), Width(LengthRange),
    MinHeight(Length), MaxHeight(Length), Height(LengthRange),
    AspectRatio(F32Range),
    And(Vec<ContainerCondition>),
    Or(Vec<ContainerCondition>),
    Not(Box<ContainerCondition>),
}

pub enum ContainerQueryEffect {
    InsertReflected(Box<dyn PartialReflect + Send + Sync>),  // type must have ReflectComponent in the type registry
    RemoveComponent(ComponentId),
    SetClass(SmolStr),
    Compound(Vec<ContainerQueryEffect>),
}
```

**Tradeoff.** A `Box<dyn Fn(&mut EntityCommands)>` effect would be maximally expressive but unreflectable, hostile to BSN, and unserializable. The typed-effect form is BSN-friendly and serializable, at the cost of expressiveness — apps that need arbitrary code in response to container size run a normal Bevy system gated on `Changed<ResolvedLayout>` instead.

**Insertion mechanism.** `InsertReflected` requires the component's type to have `ReflectComponent` registered in the world's `AppTypeRegistry`. Stage D looks up the registration by `TypeId` and uses `ReflectComponent::insert` to apply the value. Components that don't register `ReflectComponent` cannot be inserted via container queries — emit `warn!(target: "buiy::layout::container_query_unregistered", ...)` and skip.

**Selector resolution.** `ContainerSelector::Type(t)` matches the **nearest** ancestor with `ContainerContext { kind: t, .. }`. `ContainerSelector::Named(n)` matches the nearest ancestor with `ContainerContext { name: Some(n), .. }`. `ContainerSelector::Nearest` matches any ancestor with a `ContainerContext`.

`SetClass` is Buiy's analogue of CSS class toggling; a `Class` component (defined in `buiy-theme-tokens-design`) holds the active class set, and theme resolution reads it.

## 5. Anchor positioning

```rust
#[derive(Resource, Default)]
pub struct AnchorRegistry {
    by_name: HashMap<SmolStr, Entity>,
}
```

Maintained by an observer on `OnInsert<AnchorName>` / `OnRemove<AnchorName>`. Last-spawned wins on duplicate names; debug-builds warn. When the winning entity is despawned and another entity still carries the same name, the registry must restore one of the survivors — the simplest correct implementation stores `Vec<Entity>` per name and pops the tail; the spec commits to that behavior so anchor names degrade predictably under entity churn.

`PositionTry` is a small struct describing one fallback placement:

```rust
pub struct PositionTry {
    pub side: AnchorSide,
    pub align: AnchorAlign,
    pub inset_offset: Length,
}
```

Common preset chains (`flip-block`, `flip-inline`, `flip-block flip-inline`) are exposed as `PositionTry::flip_block(...)` constructors.

**Not in v1:** `@position-try` named blocks (CSS feature for reusable named fallback chains). Users build the `Vec<PositionTry>` directly. Deferred to a future minor.

## 6. Writing modes

`WritingMode { mode: WritingModeKind, direction: Direction }`.

Working variants in v1:
- `mode: HorizontalTb`, `direction: Ltr` — default, fully supported.
- `mode: HorizontalTb`, `direction: Rtl` — supported (Taffy implements RTL flip).

Reserved variants (warn-and-fallback to HorizontalTb):
- `VerticalRl`, `VerticalLr`, `SidewaysRl`, `SidewaysLr`.

Logical edges (`block_start`/`block_end`/`inline_start`/`inline_end` on `Edges<T>`) resolve correctly for LTR + RTL. They are passthrough (`block_start == top`) for horizontal modes today; the resolution table grows when Taffy adds vertical writing-mode support.

**Why defer.** Taffy 0.10 does not implement vertical writing modes. Buiy can either reimplement layout for vertical modes (large) or wait. The spec waits.

## 7. Integration with other subsystems

### Render pipeline (`buiy-render-pipeline-design`)
- Reads `ResolvedLayout` only; never `Display`/`Size`/etc. directly.
- Stacking-context computation, top-layer compositing, paint-order rules belong to render.
- Layout publishes geometry; render decides what gets painted and in what order.

### Focus / a11y (`buiy-focus-model-design`, `buiy-accessibility-design`)
- Read `ResolvedLayout` for tab-order tie-breaks and `inert` ancestor checks.
- `Display::None` excludes the subtree from layout, focus, picking, and AccessKit. The kill switch.
- `Visibility::Hidden` (defined in render-pipeline spec) keeps the layout box, removes paint + focus + picking.

### Picking (`buiy-input-events-design`)
- `bevy_picking` backend hit-tests against `ResolvedLayout` rects + `Overflow` clip rects.
- Scroll-offset hit-testing requires a `Scroll` component (deferred to overflow-and-scrolling spec).

### Theme / tokens (`buiy-theme-tokens-design`)
- `Length::Token(TokenId)` resolves at Stage A by reading the active `ResolvedTokens`.
- Theme resolution must commit to publishing `ResolvedTokens` before `BuiySet::Layout` runs. This becomes a documented `BuiySet::Theme.before(BuiySet::Layout)` ordering constraint.

### Animation (`buiy-animation-design`)
- The layout system emits `Changed<ResolvedLayout>` so animation can interpolate rect changes.
- Layout transitions (animating between two computed layouts) are owned by animation; this spec only commits to making the change-detection reliable.

## 8. Migration from Phase 0

Phase 0 (`crates/buiy_core/src/components.rs`) ships:

```rust
pub struct Style { pub width: f32, pub height: f32, pub flex_direction: FlexDirection }
```

The migration:

1. Rename Phase 0's `Node` marker to `LayoutNode` everywhere (`crates/buiy_core/src/components.rs`, `layout.rs`, the focus and a11y queries that filter `With<Node>`, tests). The new name avoids collision with `taffy::Node` and is clearer about what the marker actually marks. Phase 0's `Node` was an internal placeholder, no public-API deprecation.
2. Delete `Style`. Introduce the components in § 1.1 and § 1.2.
3. Update `crates/buiy_core/src/layout.rs` to query the decomposed components instead of `Style`. The single-pass `sync_and_compute_layout` becomes the multi-stage pipeline of § 3.
4. Update `tests/hello_button_e2e.rs` and any other internal call site to insert `LayoutNode + Display + Size + (FlexLayout)` instead of `Style`.
5. Add the `gc_layout_tree` system (§ 3 Stage G).
6. Register reflection for every new component (`CorePlugin::build` already calls `register_type::<FlexDirection>()`; extend the list).

Pre-0.1, no public-API deprecation surface to manage. The plan that realizes this spec breaks the change into reviewable chunks.

## 9. Testing

- **Length conversion unit tests.** Round-trip every `Length` variant through `length_to_taffy_dimension` / `length_to_taffy_lp` / `length_to_taffy_lpa` with a synthetic resolution context. Asserts the coercion behavior on invalid context (e.g., `Auto` in `LengthPercentage` → `Px(0.0)` + warn).
- **Calc proptest.** `proptest!` over small `LengthExpr` trees in pure `Px` (so the resolution context is trivial). Asserts: `(a + b) - b ≈ a`, `min(a, b) ≤ a`, `clamp(x, lo, hi) ∈ [lo, hi]`.
- **Pipeline integration tests.** A Bevy `App` with one root + a few children. Insert decomposed components, run `app.update()`, assert `ResolvedLayout` rects.
- **Container-query iteration cap.** Construct a deliberately oscillating query (effect changes a sibling's size in a way that flips the parent container's size class). Assert: `warn!` emitted, deterministic frozen state, no infinite loop.
- **Anchor fallback exhaustion.** Anchor in a containing block too small to fit any fallback. Assert: last fallback used, `debug!` emitted, `AnchorResolved::fallback_index == fallbacks.len() - 1`.
- **Visual diff fixtures.** Through the existing `buiy_verify` harness; one fixture per representative layout (flex row, flex column wrap, grid 3x3 with named areas, anchor + fallback, container-query breakpoint).

## 10. Open questions

1. **`length!()` macro.** Ship a `length!("100% - 16px")` parser, or builder-only? Default: builder-only; revisit with user demand.
2. **Container-query effect type.** Reflection-driven typed bundle (current proposal) vs. `Box<dyn Fn>` for max expressiveness. Default: typed.
3. **Anchor name vs entity reference.** Some apps will want to anchor to an `Entity` known at construction time (faster, no string lookup). Whether to add `AnchorTo::Entity(Entity)` as a parallel variant in v1 or wait. Default: name-only for v1.
4. **Sticky positioning.** Taffy's sticky support is incomplete (last surveyed). Whether Buiy ships its own sticky pass after Taffy compute, or waits. Default: ship our own (small offset adjustment).
5. **Logical edge accessor cost.** `Edges::block_start(&wm)` is a cheap match today but threads `WritingMode` through every site. Whether to publish a resolved `LogicalEdges<T>` alongside the physical `Edges<T>` after Stage A. Default: accessor-only.
6. **Token-driven calc.** `Length::Calc` containing `Length::Token` resolves by looking up the token then evaluating. Caching the resolved calc result across frames (tokens rarely change) is a perf win. Default: no caching in v1.
7. **`Overflow: Clip` vs `Hidden` semantics.** `Hidden` allows scrolling via JS in CSS; we have no JS, so the distinction is whether the subtree is programmatically scrollable via the future `Scroll` component. Default: `Clip` is non-scrollable, `Hidden` is scrollable-but-no-scrollbar; `Scroll` always shows scrollbars; `Auto` shows scrollbars only when needed.
8. **Subgrid / masonry / vertical writing modes.** Pinned to Taffy's roadmap; absorbed on the Buiy minor that follows Taffy's release. No spec changes here; tracking in a follow-up.

## References

- Foundation spec: `docs/specs/2026-05-07-buiy-foundation/README.md` § 4 (sub-spec roadmap).
- Foundation `visuals.md` § 3.2 (layout feature inventory + tier tags).
- Foundation `architecture.md` § 2.3 (Taffy in the architectural foundation).
- Phase 0 layout: `crates/buiy_core/src/layout.rs`, `crates/buiy_core/src/components.rs`.
- Taffy: <https://github.com/DioxusLabs/taffy>
- CSS Containment Module L3 (container queries): <https://www.w3.org/TR/css-contain-3/>
- CSS Anchor Positioning: <https://www.w3.org/TR/css-anchor-position-1/>
- CSS Writing Modes Module L4: <https://www.w3.org/TR/css-writing-modes-4/>
- CSS Values and Units Module L4 (length, calc, units): <https://www.w3.org/TR/css-values-4/>
