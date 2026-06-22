# Container queries and writing modes

**Parent:** [README.md](README.md)

Two distinct features grouped here because they share the same *cross-cutting* property — both require Buiy to know more about an entity's surroundings than Taffy alone tracks. Container queries care about a container's resolved size; writing modes care about which axis is "inline" vs "block."

## 1. Container queries

Tier-C. CSS Containment Module Level 3. Buiy-owned implementation; Taffy doesn't ship container queries.

### 1.1 `Container` component

```rust
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct Container {
    pub container_type: ContainerType,
    pub container_name: Option<String>,
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
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ContainerQuery {
    pub container: Option<String>,              // None = nearest queried ancestor; Some = named lookup
    pub conditions: Vec<QueryCondition>,        // ALL must hold for activation
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

The query is *applied* by toggling zero-field marker components on the queried entity (one of `ContainerQueryActive` or `ContainerQueryInactive`). There is at most one `ContainerQuery` per entity (Bevy `Component` is single-instance), so the markers carry no rule id. Style-bundle application is the consumer's responsibility — typically a separate observer / system reads the marker and (un)inserts a corresponding component bundle.

This decoupling is intentional: the spec doesn't bake any one component-application strategy into the query system. Themes, BSN authors, and ad-hoc systems all consume the marker the same way.

### 1.3 Activation: same-frame re-layout

The activation algorithm runs in two pipeline steps ([architecture.md § 3](architecture.md#3-system-pipeline)):

**Step 2 — `CqActivate`** (before Taffy compute):

1. For each `ContainerQuery` rule, find its target query container (by name or nearest ancestor whose `container_type` is `Size` or `InlineSize`).
2. Read the container's `ResolvedLayout` from the *previous frame*.
3. Evaluate every `QueryCondition` against that prior size.
4. Toggle the rule's `ContainerQueryActive` / `ContainerQueryInactive` markers if the activation flipped.

**Step 4 — `CqFlipCheck`** (after Taffy compute):

5. Re-evaluate every rule against this frame's fresh Taffy output, read directly from `tree.layout(node_id)` — NOT the entity-side `ResolvedLayout`, which step 7 has not written yet and still holds the previous frame's value.
6. If any rule's activation differs from what step 2 computed, toggle the markers and signal "re-layout needed."

**Step 5 — `CqFlipReRun`:**

7. If step 4 signaled re-layout, a dedicated step-5 system (`cq_flip_rerun`) re-runs the *inner work* of `SyncStyles` + `TaffyCompute` **once**, gated on `CqReRunRequested`. This is a single system holding the union of those two steps' params — not a literal re-invocation of steps 1 and 3 in place. Step 4 does not re-run.

This is the same-frame re-layout strategy ([README § 2 pillar 4](README.md#2-architectural-pillars-one-line-summaries)). Cost ceiling: 2× Taffy on activation-flip frames, 1× otherwise.

**Step 8 — `CqDescendantInvalidate`** (after `WriteResolvedLayout`, step 7) and **Step 9 — `CqDescendantReRun`** extend the same-frame settle to the *multi-level geometric* cascade. When a query container `A`'s `ResolvedLayout` changes, a `Cqw`-sized intermediate `B` between `A` and a rule-bearing descendant `C` has no `Changed<*>` bit of its own, so steps 1–5 alone never re-translate `B` and `C`'s rule never re-evaluates. Phase 14 closes this:

8. `cq_descendant_invalidate` (step 8) reads `Changed<ResolvedLayout>` on query containers (`Container { container_type != Normal }`), walks each changed container's `Children` subtree, and collects the descendants into a private `ContainerSizeDirty(HashSet<Entity>)` resource — setting `CqDescendantReRunRequested(true)` when non-empty. (Bevy ships no "ancestor's `T` changed" filter, so the cascade is found by reading the container that *did* change and walking **down**.)
9. `cq_descendant_rerun` (step 9, gated on `CqDescendantReRunRequested`, analogous to `cq_flip_rerun`) drains the dirty set: it re-translates exactly those descendants so their `Length::Cq*` re-resolves against `A`'s new size, recomputes Taffy, re-writes `ResolvedLayout`, and re-evaluates every `ContainerQuery` inline (reusing the same `resolve_nearest_container` / `evaluate_conditions` pieces `cq_activate` uses) — so `B` re-resolves and `C` flips its `ContainerQueryActive` / `ContainerQueryInactive` marker the **same frame** `A` resizes.

The descendant re-run is likewise capped at **one re-run per frame** (`CqDescendantReRunRequested` is cleared at the top), preserving the 2×-Taffy ceiling. A deeper `A`→`B`→`C`→`D` chain therefore settles one further level per frame: the *direct* intermediate is same-frame; levels beyond it remain eventually-consistent over subsequent frames (next frame's step 8 sees the now-changed `ResolvedLayout` of the next level and seeds it). Per-fixpoint looping within one frame is intentionally **not** done — bounded per-frame re-layout is the loop-breaker.

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

If no queried ancestor exists, container units fall back to viewport units (`cqw → vw`, `cqh → vh`) with a `warn!` once per session (a single global `AtomicBool` gate, not per entity).

`cqi` / `cqb` against a container with `ContainerType::InlineSize` resolve only on the inline axis; querying the block axis falls back to the same warn-and-degrade path. (See [README § 5 — open questions](README.md#5-open-questions) for nested-container subtleties.)

**Consumers.** The shared `resolve_cq_unit_px` resolver is used by sizing, grid tracks, edge translation, *and* sticky positioning insets — a sticky `Length::Cq*` inset resolves against the sticky entity's own nearest queried ancestor (size read current-frame from Taffy). See [display-and-positioning.md § 2.3](display-and-positioning.md).

### 1.5 Test surface

- **Activation flip** — fixture with one `@container (min-width: 600px)` rule; resize container from 500 → 700 in two frames; assert this frame's `ResolvedLayout` reflects the activated rule.
- **Same-frame re-layout cap** — fixture where activating a rule flips the rule's container's size enough to *de*activate it; assert exactly 2× Taffy passes and the result is the second pass's output (not oscillation).
- **Transitive cascade catches up in-frame (direct intermediate)** — fixture A→B→C where A is a query container, B is `Cqw`-sized off A, and C bears a `ContainerQuery` against B: widening A makes B re-resolve its `Cqw` *and* C flip its marker in the **same frame** A resizes, via the step-8/step-9 descendant re-run (Phase 14). The regression test `cq_transitive_cascade_is_one_frame_stale` was flipped to the positive `cq_transitive_cascade_catches_up_in_frame`. Still bounded to one level per frame: in a deeper A→B→C→D chain, the level *beyond* the direct intermediate settles on the following frame (next-frame step 8 seeds it) — the eventual-consistency contract holds past the first hop.
- **Container-unit resolution** — fixture with a 800px-wide container and a child `width: Cqw(50)`; assert child width = 400px.
- **Fallback to viewport units** — fixture with no queried ancestor; child `width: Cqw(50)` resolves against the viewport (window) width — e.g. 50% of a 1000px window = 500px — with one `warn!`.

## 2. Writing modes

Tier-F (direction) / tier-C (writing-mode + sideways).

### 2.1 `WritingMode` component

```rust
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
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

`WritingMode` *inherits down the entity hierarchy*. The effective writing-mode for an entity is its own `WritingMode` if set, else the nearest ancestor's. A `WritingModeResolved` private component is synced by an inheritance pass that runs *before* step 1 ([architecture.md § 1.2](architecture.md#change-detection-trigger-set)) so step 1's `direction`-wiring and container-unit (`Cq*`) axis resolution are `O(1)` per entity. (Step 1 does *not* perform logical→physical edge translation — that happens at author-construct time in the `Logical*` builders; see § 2.4.) `WritingModeResolved` carries the same derives as `WritingMode` (`#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]`).

Changing `WritingMode` on a parent invalidates `WritingModeResolved` on every descendant via Bevy change detection. The walking is `O(subtree size)`; mass theme switches are absorbed because writing-mode changes are rare relative to other layout mutations.

### 2.3 Logical → physical translation

The bridge between logical and physical edges/axes happens at *author-construct time* in the `Logical*` builders, **not** during step 1 (consistent with § 2.4). Specifically:

1. Physical `BoxModel` and `Position::Inset` are passed to Taffy unchanged. Step 1 (`style_to_taffy`) only forwards the already-physical fields to Taffy and wires `WritingModeResolved.direction` → `taffy::Style.direction`; it performs no logical→physical edge translation.
2. The logical builders (`LogicalInset::to_inset` in `style.rs`; `LogicalEdges::to_edges` in `types.rs`; `LogicalBoxModel::to_box_model` in `style.rs`; see [box-model.md § 4.1](box-model.md#41-api-shape)) translate at author-construct time into physical fields, using the author-supplied `WritingMode` value.

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

Taffy 0.10 has *no* writing-mode awareness at the layout-engine level — it deals exclusively in physical `inset` / `padding` / `margin` rects. Buiy therefore does **not** route logical properties through Taffy. Instead, logical → physical translation happens at author-construct time: `LogicalInset::to_inset` (`style.rs`) and `LogicalEdges::to_edges` (`types.rs`) apply the 6-row mode/direction mapping (§ 2.3) and produce a physical `Inset` / `Edges`, which step 1 feeds to Taffy via the physical `inset_to_lpa` / `edges_to_*` helpers (`translate.rs`). For `sideways-*` the builders normalize to the corresponding non-sideways vertical mode and rely on glyph rotation downstream.

The *only* writing-mode field wired into `taffy::Style` is `direction`: `WritingModeResolved.direction` (`Ltr` / `Rtl`) maps to `taffy::Style.direction`. Taffy honors that flag for block-level mirroring (e.g. flex `flex-start` becoming the right edge under RTL). Inline-flow text directionality is not Taffy's concern — it lives in the shaper.

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
