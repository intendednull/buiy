# Container queries and writing modes

**Parent:** [README.md](README.md)

Two distinct features grouped here because they share the same *cross-cutting* property — both require Buiy to know more about an entity's surroundings than Taffy alone tracks. Container queries care about a container's resolved size; writing modes care about which axis is "inline" vs "block."

## 1. Container queries

Tier-C. CSS Containment Module Level 3. Buiy-owned implementation; Taffy doesn't ship container queries.

### 1.1 `Container` component

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Container {
    pub container_type: ContainerType,
    pub container_name: Option<SmolStr>,
}

pub enum ContainerType {
    Normal,             // not a query container
    Size,               // both axes queryable
    InlineSize,         // only inline axis queryable
}
```

An entity with `Container { container_type: ContainerType::InlineSize, .. }` becomes a query container for descendants; its inline-axis resolved size is what `cqi` / `cqw` units and `@container (min-width: ..)` rules resolve against.

### 1.2 `ContainerQuery` — the rule

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct ContainerQuery {
    pub container: Option<SmolStr>,             // None = nearest queried ancestor; Some = named lookup
    pub conditions: Vec<QueryCondition>,        // ALL must hold for activation
    pub when_active:   Option<Entity>,          // Entity holding the "active" component bundle to apply
    pub when_inactive: Option<Entity>,          // Entity holding the "inactive" component bundle to apply
}

pub enum QueryCondition {
    MinWidth(Length),
    MaxWidth(Length),
    MinHeight(Length),
    MaxHeight(Length),
    MinAspectRatio(f32),
    MaxAspectRatio(f32),
    Orientation(Orientation),                   // Portrait | Landscape
}
```

The query is *applied* by toggling marker components on the queried entity (one of `ContainerQueryActive(rule_id)` or `ContainerQueryInactive(rule_id)`). Style-bundle application is the consumer's responsibility — typically a separate observer / system reads the marker and (un)inserts a corresponding component bundle.

This decoupling is intentional: the spec doesn't bake any one component-application strategy into the query system. Themes, BSN authors, and ad-hoc systems all consume the marker the same way.

### 1.3 Activation: same-frame re-layout

The activation algorithm runs in two pipeline steps ([architecture.md § 3](architecture.md#3-system-pipeline)):

**Step 2 — `CqActivate`** (before Taffy compute):

1. For each `ContainerQuery` rule, find its target query container (by name or nearest-ancestor-with-`Container::Size`).
2. Read the container's `ResolvedLayout` from the *previous frame*.
3. Evaluate every `QueryCondition` against that prior size.
4. Toggle the rule's `ContainerQueryActive` / `ContainerQueryInactive` markers if the activation flipped.

**Step 4 — `CqFlipCheck`** (after Taffy compute):

5. Re-evaluate every rule against the *current frame's* fresh `ResolvedLayout`.
6. If any rule's activation differs from what step 2 computed, toggle the markers and signal "re-layout needed."

If step 4 signals re-layout, steps 1 (`SyncStyles`) and 3 (`TaffyCompute`) re-run **once**. Step 4 does not re-run; transitive flips wait until next frame.

This is the same-frame re-layout strategy ([README § 2 pillar 4](README.md#2-architectural-pillars-one-line-summaries)). Cost ceiling: 2× Taffy on activation-flip frames, 1× otherwise.

### 1.4 Container units

`Length::Cqw / Cqh / Cqi / Cqb / Cqmin / Cqmax` resolve against the entity's nearest queried ancestor's previous-frame resolved size. The resolution rule:

| Unit | Resolves against |
|---|---|
| `Cqw(p)` | `p%` of nearest queried ancestor's `width` |
| `Cqh(p)` | `p%` of nearest queried ancestor's `height` |
| `Cqi(p)` | `p%` of nearest queried ancestor's *inline* axis (depends on writing-mode) |
| `Cqb(p)` | `p%` of nearest queried ancestor's *block* axis |
| `Cqmin(p)` | `p%` of `min(cqi, cqb)` |
| `Cqmax(p)` | `p%` of `max(cqi, cqb)` |

If no queried ancestor exists, container units fall back to viewport units (`cqw → vw`, `cqh → vh`) with a `warn!` once per session per entity.

`cqi` / `cqb` against a container with `ContainerType::InlineSize` resolve only on the inline axis; querying the block axis falls back to the same warn-and-degrade path. (See [README § 5 — open questions](README.md#5-open-questions) for nested-container subtleties.)

### 1.5 Test surface

- **Activation flip** — fixture with one `@container (min-width: 600px)` rule; resize container from 500 → 700 in two frames; assert this frame's `ResolvedLayout` reflects the activated rule.
- **Same-frame re-layout cap** — fixture where activating a rule flips the rule's container's size enough to *de*activate it; assert exactly 2× Taffy passes and the result is the second pass's output (not oscillation).
- **Transitive cascade is one-frame stale** — fixture A→B→C where activation of A's rule changes B's size (which would flip B's rule); assert frame N applies A's activation, frame N+1 applies B's.
- **Container-unit resolution** — fixture with a 800px-wide container and a child `width: Cqw(50)`; assert child width = 400px.
- **Fallback to viewport units** — fixture with no queried ancestor; child `width: Cqw(50)` resolves to `Vw(50)` with one `warn!`.

## 2. Writing modes

Tier-F (direction) / tier-C (writing-mode + sideways).

### 2.1 `WritingMode` component

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct WritingMode {
    pub mode: WritingModeKind,
    pub direction: Direction,
    pub text_orientation: TextOrientation,
    pub unicode_bidi: UnicodeBidi,
}

pub enum WritingModeKind {
    HorizontalTb,        // CSS `horizontal-tb` (default)
    VerticalRl,          // `vertical-rl` — Japanese, Chinese vertical
    VerticalLr,          // `vertical-lr` — Mongolian
    SidewaysRl,          // `sideways-rl` — tier-C polish
    SidewaysLr,          // `sideways-lr`
}

pub enum Direction { Ltr, Rtl }
pub enum TextOrientation { Mixed, Upright, Sideways }
pub enum UnicodeBidi { Normal, Embed, Isolate, BidiOverride, IsolateOverride, Plaintext }
```

`text_orientation` is foundation tier-E ([visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout)) — the value is stored on `WritingMode` for forward compatibility, but the glyph-rotation that consumes it lives in `buiy-text-rendering-design` and is not shipped in v1. v1 layout treats every `TextOrientation` as `Mixed` for glyph orientation; vertical layout itself (the part this spec owns) honors `mode` regardless.

### 2.2 Inheritance

`WritingMode` *inherits down the entity hierarchy*. The effective writing-mode for an entity is its own `WritingMode` if set, else the nearest ancestor's. A `WritingModeResolved` private component is synced by an inheritance pass that runs *before* step 1 ([architecture.md § 1.2](architecture.md#change-detection-trigger-set)) so step 1's logical→physical translation is `O(1)` per entity.

Changing `WritingMode` on a parent invalidates `WritingModeResolved` on every descendant via Bevy change detection. The walking is `O(subtree size)`; mass theme switches are absorbed because writing-mode changes are rare relative to other layout mutations.

### 2.3 Logical → physical translation

The bridge between logical and physical edges/axes happens during step 1. Specifically:

1. Physical `BoxModel` and `Position::Inset` are passed to Taffy unchanged.
2. Logical insert helpers (`LogicalBoxModel`, `LogicalInset` — see [box-model.md § 4.1](box-model.md#41-api-shape)) translate at insert time into physical fields, using the entity's `WritingModeResolved`.

Mapping:

| Effective writing-mode + direction | Logical → physical |
|---|---|
| `horizontal-tb` + `ltr` | `inline-start` = `left`, `block-start` = `top` |
| `horizontal-tb` + `rtl` | `inline-start` = `right`, `block-start` = `top` |
| `vertical-rl` + `ltr` | `inline-start` = `top`, `block-start` = `right` |
| `vertical-rl` + `rtl` | `inline-start` = `bottom`, `block-start` = `right` |
| `vertical-lr` + `ltr` | `inline-start` = `top`, `block-start` = `left` |
| `vertical-lr` + `rtl` | `inline-start` = `bottom`, `block-start` = `left` |

`sideways-rl` and `sideways-lr` are tier-C polish modes that rotate text glyphs but otherwise behave like `vertical-rl` / `vertical-lr` for layout. Glyph rotation is `buiy-text-rendering-design`'s concern; layout treats them as their non-sideways equivalents.

### 2.4 Taffy integration

Taffy 0.10 has logical-property awareness on its `Style` (e.g. `inset.start` / `inset.end`); we route logical insets through it directly when the writing-mode is one of `horizontal-tb` / `vertical-rl` / `vertical-lr`. For `sideways-*` we pass the corresponding non-sideways mode and rely on glyph rotation downstream.

Taffy doesn't natively know about `direction: rtl` for *inline-flow* purposes (text directionality lives in the shaper). For block-level mirroring (e.g. flex `flex-start` becoming the right edge under RTL), Taffy honors the `rtl` flag when set.

### 2.5 Open question

Whether to ship a Buiy-side rotation pass for `sideways-*` to deliver true vertical-text layout, or wait on Taffy upstream. Tracked in [README § 5](README.md#5-open-questions). v1 ships the `sideways-*` API surface; the rotation pass is deferred.

### 2.6 `unicode-bidi`

Layout-relevant for nested bidi contexts. Detailed semantics (BiDi resolution algorithm, paragraph boundary handling) live in `buiy-i18n-design`. This spec stores the value on `WritingMode.unicode_bidi`; the i18n spec consumes it.

### 2.7 Test surface

- **`direction: rtl` flips flex** — `Display::Flex(Row)` + `Direction::Rtl` lays children right-to-left; assert first child's `position.x` is greater than last child's.
- **`writing-mode: vertical-rl`** — fixture with a 200×300 container; assert `inline-size: 100` resolves to `height: 100`.
- **Inheritance** — set `WritingMode::VerticalRl` on a parent; assert descendant's `WritingModeResolved` is `VerticalRl`.
- **Logical → physical edge** — `LogicalEdges { inline_start: Length::px(8), .. }` under `vertical-rl` produces `Edges { top: 8.0, .. }`.
- **`sideways-rl` falls back to `vertical-rl` layout + warn** — assert layout matches `VerticalRl`; one `warn!` per session.

## 3. Coordination

Container queries and writing modes share a *change-detection* surface — both invalidate downstream layout when their ancestor state changes. They run in distinct pipeline steps and don't otherwise interact.
