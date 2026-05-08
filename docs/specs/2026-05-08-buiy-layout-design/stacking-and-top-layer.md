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

pub enum ZIndex {
    Auto,                // CSS `z-index: auto` — does not form a stacking context on its own
    Layer(i32),          // explicit; forms a stacking context
}

pub enum Isolation {
    Auto, Isolate,       // `Isolate` forces a stacking context
}

pub enum TopLayer {
    None,
    Modal,               // <dialog open> equivalent — escapes containing-block stacking
    Popover,             // popover-attribute equivalent — same escape
    Tooltip,             // tooltip — also escapes, but ordered below modal/popover
    Fullscreen,          // fullscreen — top of the top layer
}
```

## 2. Stacking-context formation

An entity forms a *stacking context* — a sub-tree painted as one unit, ordered against siblings by `z_index` — when **any** of:

1. `Position::Static` AND `Stacking::z_index = Layer(_)` → CSS quirk: positioned-with-z-index forms a stacking context, but pure `z_index` on `Static` does *not*. So this rule actually requires `Position::Kind != Static` AND `z_index = Layer(_)`.
2. `Stacking::isolation = Isolate`.
3. `Transform` is non-identity. (Detailed in [transforms-and-containment.md § 3](transforms-and-containment.md#3-stacking-context-formation).)
4. `Containment::contain` includes `Paint` or `Strict`. (Detailed in [transforms-and-containment.md § 5](transforms-and-containment.md#5-containment).)
5. Render-side properties form one too: `opacity < 1.0`, `filter != none`, `mix_blend_mode != normal`, `will_change` mentions an SC-forming property. These live on render-side components but are *checked* during this spec's stacking-context detection so layout can hand a correct list to render.
6. The root entity always forms one.

The rule set is deliberately union — any single trigger is sufficient. The CSS spec is the source of truth; the foundation visuals.md § 3.2 enumeration anchors the trigger list.

### 2.1 `StackingContext` private component

A private `StackingContext { painters_z: Vec<Entity>, .. }` component is synced by the layout-pipeline's `WriteResolvedLayout` step (or a sub-pass thereof) onto every entity that forms a stacking context. The component is private (not author-set) but reflectable so devtools can inspect it.

`StackingContext.painters_z` is the *paint order* of every descendant within this context, sorted by:

1. Negative `z_index` first (lowest first).
2. In-flow non-positioned descendants (document order).
3. Floats (none in Buiy — floats are tier-O — so always empty).
4. In-flow positioned with `z_index: Auto` (document order).
5. Positive `z_index` (lowest first).

This list is what render walks at paint time. Resolving paint order at layout time avoids rendering having to re-walk the tree.

### 2.2 Performance

Stacking-context detection runs as a sub-pass of step 7 (`WriteResolvedLayout`). Cost: `O(entities)`. Most entities don't form a stacking context, so the inner sort is `O(stacking-context count × children-per-context log)` in practice.

The detect-eagerly-vs-lazily question ([README § 5](README.md#5-open-questions)) is open: lazy detection during paint would amortize, but break the rule of "render reads finished data."

## 3. `z_index`

`ZIndex::Layer(i32)` orders siblings within the same stacking context. CSS-faithful semantics: 0 is default for explicit, negative integers paint behind, positive in front. There is no upper or lower bound; render handles the full `i32` range.

`ZIndex::Auto` does *not* form a stacking context on its own (per the rule above) and orders strictly by document order.

Mixing absolute-positioned siblings with and without explicit `z_index`: the explicit ones layer per their `z_index`, the auto ones interleave per document order. CSS has the same semantics; tests assert it.

## 4. Top layer

The top layer is a parallel render layer that escapes all containing-block stacking. Modals, popovers, fullscreen surfaces, and tooltips paint on top of the entire window regardless of where their entity sits in the layout tree.

### 4.1 `TopLayer` activation

```rust
pub enum TopLayer {
    None,        // default — entity participates in normal stacking
    Modal,       // <dialog open>
    Popover,     // popover-attribute
    Tooltip,     // tooltip pattern
    Fullscreen,  // fullscreen API equivalent
}
```

Setting `TopLayer != None` *removes* the entity from its parent's stacking context for paint purposes. Layout still treats it normally — its containing block, size, and position resolve as if it were in-flow. (Authoring guidance: top-layer elements are typically `Position::Fixed` or use anchor positioning to attach to a trigger.)

### 4.2 Top-layer ordering

Within the top layer, order is:

1. **Fullscreen** — bottom of the top-layer stack (one entity wins; the rest fall back to their normal stacking).
2. **Tooltip**.
3. **Popover** (CSS: nested popovers stack in popover-open order).
4. **Modal** — top.

Within each tier, order is by *activation order* — the entity activated most recently paints on top. The activation order is tracked by a `TopLayerActivation` resource (a `VecDeque<Entity>`) updated whenever `TopLayer` changes from `None` → non-`None`.

### 4.3 Escape from clip

Top-layer entities are not clipped by ancestor `Overflow::Hidden` / `Overflow::Clip`. Their effective clip rect is the window viewport (or per-window viewport in multi-window setups; see `buiy-window-and-surface-design`).

### 4.4 Per-window scope

Each window has its own top layer. A modal in window A doesn't paint over window B. Cross-window top-layer ordering is out of scope ([README § 5](README.md#5-open-questions)).

### 4.5 Authoring example

```rust
// Modal dialog escaping its layout parent's stacking context.
commands.spawn((
    Style::default()
        .position(PositionKind::Fixed)
        .inset(Inset { top: Length::px(50.0), right: Length::px(50.0), .. default() })
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
- **`Position::Static` ignores `z_index`** — fixture with a static element + z-index 5; assert it paints in document order, not lifted.
- **Isolation forms stacking context** — fixture with `Isolation::Isolate`; assert a `StackingContext` component appears.
- **Top-layer escapes parent overflow** — fixture parent `Overflow::Hidden`, child `TopLayer::Modal` with `Position::Fixed` extending past the parent; assert the modal's `StackingContext` membership is the window root, not the parent.
- **Top-layer activation order** — open three popovers in sequence; assert the activation deque has them in order; assert the most-recent paints last (on top).
- **Mixed top-layer tiers** — Modal + Tooltip simultaneously open; assert paint order is Tooltip below Modal regardless of activation order.
- **Per-window top layer** — multi-window fixture; modal in window A doesn't appear in window B's `painters_z`.
