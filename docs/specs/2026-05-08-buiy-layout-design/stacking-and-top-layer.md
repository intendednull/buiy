# Stacking and top layer

**Parent:** [README.md](README.md)

How entities order themselves visually along the depth axis: stacking-context formation, `z_index`, and the *top layer* — the escape hatch for modals, popovers, dialogs, and fullscreen.

This file's contract is to define the **layout-side** facts: which entities form stacking contexts, what their z-index is, which are on the top layer. Compositing them — actually drawing in the right order, applying clip/opacity/blend correctly — lives in [`buiy-render-pipeline-design`](../2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap). The boundary is: layout decides *position in the depth ordering*; render decides *how to paint that order*.

## 1. `Stacking` component

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Stacking {
    pub z_index: ZIndex,
    pub isolation: Isolation,
    pub top_layer: TopLayer,
}

#[derive(Reflect, Clone, Copy, Default)]
pub enum ZIndex {
    #[default]
    Auto,                // CSS `z-index: auto` — does not form a stacking context
    Layer(i32),          // explicit; forms a stacking context iff `Position.kind != PositionKind::Static` (CSS rule)
}

#[derive(Reflect, Clone, Copy, Default)]
pub enum Isolation {
    #[default]
    Auto,
    Isolate,             // `Isolate` forces a stacking context
}

#[derive(Reflect, Clone, Copy, Default)]
pub enum TopLayer {
    #[default]
    None,
    Modal,               // <dialog open> equivalent — escapes containing-block stacking
    Popover,             // popover-attribute equivalent — same escape
    Tooltip,             // tooltip — also escapes, but ordered below modal/popover
    Fullscreen,          // fullscreen — top of the top layer
}
```

## 2. Stacking-context formation

An entity forms a *stacking context* — a sub-tree painted as one unit, ordered against siblings by `z_index` — when **any** of:

1. `Position.kind != PositionKind::Static` AND `Stacking::z_index = Layer(_)`. (CSS quirk: positioned-with-explicit-z-index forms a stacking context; `z_index` on a `PositionKind::Static` entity does *not*.)
2. `Stacking::isolation = Isolate`.
3. `Transform` is non-identity. (Detailed in [transforms-and-containment.md § 3](transforms-and-containment.md#3-stacking-context-formation).)
4. `Containment::contain` includes `Paint` or `Strict`. (Detailed in [transforms-and-containment.md § 5](transforms-and-containment.md#5-containment).)
5. Render-side properties form one too: `opacity < 1.0`, `filter != none`, `mix_blend_mode != normal`, `will_change` mentions an SC-forming property (will-change portion: tier-E, deferred — see [transforms-and-containment.md § 5.3](transforms-and-containment.md#53-will-change)). These live on render-side components but are *checked* during this spec's stacking-context detection so layout can hand a correct list to render.
6. The root entity always forms one.

The rule set is deliberately union — any single trigger is sufficient. The CSS spec is the source of truth; the foundation visuals.md § 3.2 enumeration anchors the trigger list.

### 2.1 `StackingContext` private component

A private `StackingContext { painters_z: Vec<Entity>, .. }` component is synced by a new step-6 `BuiyLayoutStep::PostTaffyOverrides` sub-pass — **sub-pass 6f**, chained after the shipped sticky (6a) / table (6b) / multicol (6c) / anchor (6d) sub-passes and after the Phase 8 transform-composition sub-pass (6e) — onto every entity that forms a stacking context. It must run after 6e: stacking-context detection needs the composed transform to detect transform-triggered stacking contexts (§2 trigger 3). Placing it in `PostTaffyOverrides` rather than the single-system `WriteResolvedLayout` write-back keeps it alongside the other post-Taffy override sub-passes. The component is private (not author-set) but reflectable so devtools can inspect it.

`StackingContext.painters_z` is the *paint order* of every descendant within this context, sorted by:

1. Negative `z_index` first (lowest first).
2. In-flow non-positioned descendants (document order).
3. Floats (none in Buiy — floats are tier-O — so always empty).
4. In-flow positioned with `z_index: Auto` (document order).
5. Positive `z_index` (lowest first).

This list is what render walks at paint time. Resolving paint order at layout time avoids rendering having to re-walk the tree.

### 2.2 Performance

Stacking-context detection runs as a step-6 (`PostTaffyOverrides`) sub-pass (6f, after the 6e transform-composition sub-pass). Cost: `O(entities)`. Most entities don't form a stacking context, so the inner sort is `O(stacking-context count × children-per-context log)` in practice.

The detect-eagerly-vs-lazily question ([README § 5](README.md#5-open-questions)) is open: lazy detection during paint would amortize, but break the rule of "render reads finished data."

## 3. `z_index`

`ZIndex::Layer(i32)` orders siblings within the same stacking context. CSS-faithful semantics: 0 is default for explicit, negative integers paint behind, positive in front. There is no upper or lower bound; render handles the full `i32` range.

`ZIndex::Auto` does *not* form a stacking context on its own (per the rule above) and orders strictly by document order.

Mixing absolute-positioned siblings with and without explicit `z_index`: the explicit ones layer per their `z_index`, the auto ones interleave per document order. CSS has the same semantics; tests assert it.

## 4. Top layer

The top layer is a parallel render layer that escapes all containing-block stacking. Modals, popovers, fullscreen surfaces, and tooltips paint on top of the entire window regardless of where their entity sits in the layout tree.

### 4.1 `TopLayer` activation

```rust
#[derive(Reflect, Clone, Copy, Default)]
pub enum TopLayer {
    #[default]
    None,        // default — entity participates in normal stacking
    Modal,       // <dialog open>
    Popover,     // popover-attribute
    Tooltip,     // tooltip pattern
    Fullscreen,  // fullscreen API equivalent
}
```

Setting `TopLayer != None` *removes* the entity from its parent's stacking context for paint purposes. Layout still treats it normally — its containing block, size, and position resolve as if it were in-flow. (Authoring guidance: top-layer elements are typically `PositionKind::Fixed` or use anchor positioning to attach to a trigger.)

### 4.2 Top-layer ordering

Within the top layer, order is:

1. **Fullscreen** — bottom of the top-layer stack (one entity wins; the rest fall back to their normal stacking).
2. **Tooltip**.
3. **Popover** (CSS: nested popovers stack in popover-open order).
4. **Modal** — top.

Within each tier, order is by *activation order* — the entity activated most recently paints on top. The activation order is tracked by a `TopLayerActivation` resource (a `VecDeque<Entity>`) updated whenever `TopLayer` changes from `None` → non-`None`.

### 4.3 Escape from clip

Top-layer entities are not clipped by an ancestor whose `Overflow.x` / `Overflow.y` is set to `OverflowMode::Hidden` or `OverflowMode::Clip`. Their effective clip rect is the window viewport (or per-window viewport in multi-window setups; see `buiy-window-and-surface-design`).

### 4.4 Per-window scope

Each window has its own top layer. A modal in window A doesn't paint over window B. Cross-window top-layer ordering is out of scope ([README § 5](README.md#5-open-questions)).

### 4.5 Authoring example

```rust
// Modal dialog escaping its layout parent's stacking context.
commands.spawn((
    Style::default()
        .position(PositionKind::Fixed)
        .inset(Inset {
            top: Sizing::Length(Length::px(50.0)),
            right: Sizing::Length(Length::px(50.0)),
            ..default()
        })
        .top_layer(TopLayer::Modal),
    /* dialog contents */,
));
```

The fluent `.top_layer(TopLayer::Modal)` writes `Stacking.top_layer = Modal`.

## 5. Mapping to render

`buiy-render-pipeline-design` consumes:

- `StackingContext.painters_z` to schedule draws within each context.
- `Stacking.z_index` to order sibling stacking contexts (already pre-sorted into `painters_z` of the parent).
- `Stacking.top_layer` to dispatch to the per-window top-layer pass.
- `TopLayerActivation` for top-layer ordering within each tier.

The contract: render reads, layout writes. Render does *not* compute stacking contexts, paint order, or top-layer membership — those are done here.

## 6. Test surface

- **`z_index` ordering** — fixture with three positioned siblings, z-index `[2, -1, 0]`; assert `painters_z` orders them `[-1, 0, 2]`.
- **`PositionKind::Static` ignores `z_index`** — fixture with a static element + z-index 5; assert it paints in document order, not lifted.
- **Isolation forms stacking context** — fixture with `Isolation::Isolate`; assert a `StackingContext` component appears.
- **Top-layer escapes parent overflow** — fixture parent with `Overflow { x: OverflowMode::Hidden, y: OverflowMode::Hidden }`, child `TopLayer::Modal` with `PositionKind::Fixed` extending past the parent; assert the modal's `StackingContext` membership is the window root, not the parent.
- **Top-layer activation order** — open three popovers in sequence; assert the activation deque has them in order; assert the most-recent paints last (on top).
- **Mixed top-layer tiers** — Modal + Tooltip simultaneously open; assert paint order is Tooltip below Modal regardless of activation order.
- **Per-window top layer** — multi-window fixture; modal in window A doesn't appear in window B's `painters_z`. *(Deferred — see § 7; `buiy_core` has no per-window layout yet.)*

## 7. v1 implementation status (Phase 9 scope)

This section records the seam between the canonical target above and what the
v1 phase (Phase 9, sub-pass 6f) actually realizes. The target is unchanged;
the deferrals are forced by features that do not yet exist in `buiy_core`, and
each is tracked in [`../../plans/follow-ups.md`](../../plans/follow-ups.md).

**Realized in Phase 9 (sub-pass 6f):**

- Stacking-context formation triggers **1** (positioned + explicit `z_index`),
  **2** (`Isolation::Isolate`), **3** (non-identity transform — read from the
  Phase-8 `ResolvedTransform`), **4** (`Containment.contain` ⊇ `PAINT` / `STRICT`),
  and **6** (root).
- `StackingContext.painters_z` paint-order sort (§ 2.1, all five tiers; floats
  always empty).
- `z_index` sibling ordering within a context (§ 3).
- Top-layer escape from parent stacking + from ancestor `Overflow` clip (§ 4.1,
  § 4.3), tier ordering (§ 4.2), and `TopLayerActivation` activation-order
  tracking — all within a **single global top layer**.

**Deferred (target stands; not in Phase 9):**

- **Trigger 5 — render-side SC formers** (`opacity < 1`, `filter`,
  `mix_blend_mode`). These live on render-side components that do not exist in
  `buiy_core` yet; 6f cannot check what is not present. When the render
  components land, 6f's trigger predicate extends to read them.
- **Trigger 5 — `will-change` SC former.** `WillChange` is stored by Phase 8
  (tier-E, no behavior); its SC-forming behavior is deferred with the rest of
  `will-change` layer promotion.
- **§ 4.4 per-window scope.** `buiy_core` has a single global `LayoutTree` and
  uses the primary window only (no per-window layout segregation). Phase 9
  ships one global top layer; per-window top layers depend on
  `buiy-window-and-surface-design`. The per-window test in § 6 is deferred with
  it.
