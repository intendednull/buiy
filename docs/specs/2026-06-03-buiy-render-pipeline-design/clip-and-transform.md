# Buiy render — clip and transform

**Parent:** [README.md](README.md)

Two layout-owned geometry seams that render only *reads*: the per-entity
**clip rectangle** (pillar 4) and the **transform bridge** (pillar 5). Both
exist so that render never walks the entity tree, never re-derives geometry,
and never fights Bevy's transform ownership. Layout (or a layout-adjacent
render-prep pass that reads *only* layout output) computes; render consumes a
flat per-entity component.

This file owns:

- **(A)** the `WriteClipRects` render-prep pass — the ancestor-clip
  intersection algorithm, the `ClipRect` output shape, and the proof that the
  `Changed<ScrollOffset>` recompute does **not** re-run layout.
- **(B)** the `Transform` / `GlobalTransform` bridge — how `ResolvedLayout` +
  `ResolvedTransform` fold into one Bevy `Transform`, why **Bevy's** propagation
  systems (`mark_dirty_trees` / `propagate_parent_transforms` /
  `sync_simple_transforms`), not Buiy, own `GlobalTransform`, **how Buiy
  schedules those three in `Update`** so `GlobalTransform` is final before
  picking + extract (§ B.2.1), the logical-pixel / y-down coordinate contract,
  and the consumption of the Phase-8 stored-but-unconsumed `perspective` /
  `transform-style` / `backface-visibility` data.

The component names used here are fixed by [README § 3](README.md#3-the-component-contract);
this file elaborates the two it introduces (`ClipRect`) and the bridge logic,
and cross-references the others rather than redefining them.

---

## A. The `WriteClipRects` render-prep pass

### A.1 Why a pass, not an extract-time walk (pillar 4)

Every input to an entity's clip rectangle is layout-owned:

- the entity's own border box — [`ResolvedLayout`](../../../crates/buiy_core/src/components.rs) `{ position, size }`;
- each ancestor whose [`Overflow`](../2026-05-08-buiy-layout-design/overflow-and-scrolling.md) is `Hidden` / `Clip` / `Scroll` / `Auto` — a clip box at that ancestor's border box;
- each scroll-container ancestor's visible **viewport**, translated by its [`ScrollOffset`](../2026-05-08-buiy-layout-design/overflow-and-scrolling.md#2-scroll-state) (runtime state, also layout-owned);
- the nearest [`Containment`](../2026-05-08-buiy-layout-design/transforms-and-containment.md#51-effect-of-contain) ancestor that includes `ContainFlags::PAINT` (or `STRICT`/`CONTENT`, which subsume `PAINT`) — a paint-containment clip at that ancestor's border box.

Because the inputs are all layout output, the ancestor walk belongs on the
layout side of the boundary, not inside an `Extract<Query>`. Pushing the walk
into render extract was the rejected runner-up (README pillar 4): it would
drift from layout's `ChildOf` / `Overflow` / `Containment` semantics silently
and be invisible to the layout-snapshot gate (#5). Instead a dedicated
**render-prep system** reads layout components and writes a flat per-entity
`ClipRect`, so render extract reads one component and never traverses.

`WriteClipRects` is **F-tier** (it gates correct clipping for every
`Overflow != Visible` container, the foundation's most-used visual-correctness
row). It is not a render-graph node — it is a normal ECS system in the main
(render-prep) world, so it is fully testable headless with no wgpu adapter.

### A.2 The `ClipRect` output shape

`ClipRect` is **owned and defined here** — this is its single canonical
definition across the render-pipeline spec; every other file references this
section rather than redefining the type.

```rust
/// Per-entity computed clip, written by `write_clip_rects` (render-prep)
/// and read by render extract + picking. Render reads it; render never
/// re-derives it. The accumulated clip AABB, in logical px.
///
/// Geometry is in the same logical-pixel, y-down, window-relative space
/// as `ResolvedLayout.position` (see § B.4), so render extract can
/// intersect it against an instance's box without a coordinate change.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    /// Top-left corner, logical px, window-relative (y-down).
    pub min: Vec2,
    /// Bottom-right corner, logical px, window-relative (y-down).
    pub max: Vec2,
}
// NO `Default`: an absent `ClipRect` is meaningful (it means "no clip"),
// so the type is never default-constructed into a zero rect.
```

**Absent-semantics (canonical, quoted identically in component-model.md § 9):**
"Absent ClipRect ⇔ no ancestor clips this entity ⇒ render applies no scissor."

Alongside `ClipRect`, `write_clip_rects` emits a companion **`AncestorClip`**
— the canonical owner of which is **this section** — for every entity that
paints an [`Outline`](component-model.md) (emitting it always is permissible if
that is simpler to implement; the cost is one extra component on Outline-bearing
entities only):

```rust
/// Per-entity *ancestor-only* clip, written by `write_clip_rects`
/// (render-prep). This is the intersection of the entity's **ancestor**
/// clip boxes ONLY — the own-box step (§ A.3 rule 1) is NOT applied — so
/// it bounds geometry an entity paints *outside* its own border box.
///
/// `Outline` reads this (not the self-intersected `ClipRect`), so an
/// outline drawn outside the border box is clipped only by ancestors,
/// never by the entity's own box (which would erase it). component-model.md
/// § 7 (Outline) consumes this; § A.2 here is its canonical definition.
///
/// Geometry is in the same logical-pixel, y-down, window-relative space as
/// `ClipRect` (§ B.4). Absent `AncestorClip` ⇔ no ancestor clips this
/// entity ⇒ the outline is unclipped.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.2.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AncestorClip {
    /// Top-left corner, logical px, window-relative (y-down).
    pub min: Vec2,
    /// Bottom-right corner, logical px, window-relative (y-down).
    pub max: Vec2,
}
```

`ClipRect` (= ancestor clips **intersected with** the own box) and
`AncestorClip` (= the intersection of ancestor clip boxes **only**, without the
own-box step) differ exactly by rule 1 of the § A.3 walk: `AncestorClip` is the
running ancestor intersection *before* the own-box intersection, `ClipRect` is
that same value *after* it. The pass already computes both as a byproduct of
the single top-down fold, so `AncestorClip` adds no extra walk.

The rect is an axis-aligned bounding box (AABB) in logical pixels. v1 stores a
**rectangular** clip only — non-rectangular clips (`border-radius` rounding of
the clip edge, `clip-path`) are deferred:

- **Rounded-rect clip** (the `Border.radius` of a clipping ancestor) is
  deferred to a **separate sibling component `ClipRadius`** — a C-tier,
  reserved carrier that is **not built in v1**. `ClipRect` carries only the
  AABB; `ClipRadius` (per-corner elliptical radii, mirroring
  [`Border`](component-model.md)) is the reserved rounded-clip carrier that
  render's SDF quad shader already understands. This is a render-side C-tier
  fast-follow tracked in [verification.md](verification.md)'s gate map, not a
  cross-spec open question.
- **`clip-path`** (arbitrary path clip) is **C-tier** (matching
  [foundation/visuals.md](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering)),
  a **reserved seam** carried by the **path primitive** (architecture § 1.4
  already tiers it C). Named only in v1, no component built — it is a reserved
  C-tier seam, not an E-tier deviation.

A degenerate clip — `min.x >= max.x` or `min.y >= max.y` — is the empty rect;
render skips painting the entity entirely (it is fully clipped away). This is
the natural output when an entity scrolls completely out of an ancestor's
viewport.

### A.3 The ancestor walk

`write_clip_rects` runs once per entity that carries `ResolvedLayout`. For
entity `E` it computes the **intersection** of:

1. **`E`'s own box** — `Rect::from(resolved.position, resolved.size)`. (Self-box
   is included so a `ClipRect` is always ⊆ the entity; render can use it as the
   instance's scissor without a separate box read.)

   > **Superseded by C1 (2026-06-22, widget-catalog campaign):** the producer now
   > computes the own box from `GlobalTransform.translation().truncate()`, not
   > `ResolvedLayout.position`, consistent with pillar 5 § B.5 (render/picking read
   > `GlobalTransform`). `ResolvedLayout.position` is parent-local; the absolute
   > basis is `GlobalTransform`. See
   > docs/specs/2026-06-22-buiy-widget-catalog-design/coordinate-space-correctness.md.
2. For each ancestor `A` walked via `ChildOf` from `E` to the layout root, in
   order:
   - if `A.Overflow` clips on an axis (`Hidden` / `Clip` / `Scroll` / `Auto` —
     i.e. *not* `Visible`), intersect with `A`'s **padding box** on that axis
     (the border box inset by **border only** — *not* padding; scrollable /
     overflowing content shows *under* the padding, so the clip edge is the
     inner border edge, not the content edge). `ResolvedLayout` reports the
     border box, so the pass insets it using `A`'s `BoxModel` **border** edges
     only. `Overflow` is **per-axis** (`{ x, y }`): a `Visible` axis contributes
     **no** clip even when the other axis clips — only the clipping axis is
     intersected, so a node with `overflow-x: hidden; overflow-y: visible`
     constrains `x` and leaves `y` unbounded;
   - if `A` is a **scroll container** (`A.Overflow.is_scroll_container()`),
     the viewport is `A`'s **padding box** (same border-only inset — content
     scrolls under the padding), and the *child-facing* clip is that padding
     box — `ScrollOffset` does **not** move the clip box itself (the visible
     window is fixed in `A`'s frame); it moves the *content* (see § A.4 for
     where the offset is applied);
   - if `A.Containment.contain` includes `ContainFlags::PAINT`, intersect with
     `A`'s border box (paint containment clips descendants to the box —
     [transforms-and-containment.md § 5.1](../2026-05-08-buiy-layout-design/transforms-and-containment.md#51-effect-of-contain)).

The walk stops at the layout root (an entity with no `ChildOf`, or whose
`ChildOf` target lacks a `Node` — the same root definition layout uses,
[layout architecture § 5](../2026-05-08-buiy-layout-design/architecture.md#5-topological-invariant)).
The intersection is computed incrementally; an ancestor that does not clip
contributes nothing (its `Overflow` is `Visible` and it is neither a
scroll container nor paint-contained), so an entity with no clipping ancestor
emits **no `ClipRect`** (cheaper than writing a window-sized sentinel and lets
render branch on `Option<&ClipRect>`).

The signature below is **illustrative** (it sketches the inputs and the
write path; the real system also carries the change-gating resource of § A.4):

```rust
/// Render-prep — computes per-entity `ClipRect` by intersecting the
/// entity box with every ancestor clip / scroll-viewport / PAINT
/// boundary. Reads only layout output; writes `ClipRect`. Runs in the
/// render-prep stage, after layout's `BuiySet::Layout`, before picking
/// and before render `ExtractSchedule`.
///
/// Spec: clip-and-transform.md § A.3.
fn write_clip_rects(
    mut commands: Commands,
    // A clip root is a `Node` with no `ChildOf`, OR whose `ChildOf` parent is
    // not a `Node` — the same two-disjunct root predicate § A.3 defines (and
    // layout uses). Seeding `Without<ChildOf>` alone would silently drop the
    // walk for a Buiy subtree parented under a non-`Node` Bevy entity, so the
    // sketch iterates *all* Nodes and tests both disjuncts (see the body): for
    // each `entity`, it is a root when it has no `ChildOf`, or
    // `node_marker.get(parent.parent()).is_err()`.
    all_nodes: Query<Entity, With<Node>>,
    child_of: Query<&ChildOf>,
    node_marker: Query<(), With<Node>>,
    // No `ScrollOffset` here: the clip *box* is offset-independent (§ A.4) —
    // scroll moves content via the bridge (§ B.3), never the clip box — so
    // this fold reads only the box-geometry inputs.
    //
    // Every per-node component except `ResolvedLayout` is `Option<&T>`,
    // because `Node` carries no `#[require]` for `BoxModel` / `Overflow` /
    // `Containment` (they are inserted only via the `Style` bundle, never
    // guaranteed on a bare `Node`). A non-`Option` `&BoxModel` / `&Overflow`
    // query would silently *drop* bare Nodes from the walk — those entities
    // would never receive a `ClipRect`. The fold therefore reads each via
    // `Option` and applies the absent-default below.
    nodes: Query<(
        &ResolvedLayout,
        Option<&BoxModel>,
        Option<&Overflow>,
        Option<&Containment>,
        // Pruning inputs (§ A.3): the same skip predicates paint-order § 5
        // owns. `Display::None` and `ContentVisibility::Hidden` subtrees are
        // not descended into. `Containment.content_visibility` lives on the
        // `Containment` above; `Display` is read here. Both are `Option` for
        // the same bare-`Node` reason — absent `Display` ⇒ not `None`.
        Option<&Display>,
    )>,
    children: Query<&Children>,
    // `existing`: the entity's previously-written `ClipRect` (if any). The
    // fold is change-gated against it — see the body below — so a frame that
    // recomputes the same rect issues no `Commands` op.
    existing: Query<Option<&ClipRect>>,
) {
    // Select roots by the § A.3 two-disjunct predicate, then walk each:
    //
    //   for entity in all_nodes.iter() {
    //       let is_root = match child_of.get(entity) {
    //           Ok(parent) => node_marker.get(parent.parent()).is_err(),
    //           Err(_)     => true, // no `ChildOf` ⇒ root
    //       };
    //       if is_root { /* walk this subtree top-down */ }
    //   }
    //
    // Top-down from each root: push the running clip down through `Children`,
    // intersecting each ancestor's clip box (§ A.3). At each entity:
    //
    //   let computed: Option<ClipRect> = /* running intersection, None if
    //                                       no ancestor clips this entity */;
    //   match (computed, existing.get(e).ok().flatten()) {
    //       (Some(c), prev) if prev != Some(&c) => { commands.entity(e).insert(c); }
    //       (None,    Some(_))                  => { commands.entity(e).remove::<ClipRect>(); }
    //       _ => { /* unchanged — no write */ }
    //   }
    //
    // The `insert` / `remove` is what actually writes the output; the
    // change-gate against `existing` keeps a steady-state frame at zero
    // structural ops.
}
```

The system is written **top-down from each root** (push the running clip down
through `Children`) rather than bottom-up per entity — it needs the running
ancestor context anyway — so each ancestor box is computed once and the running
intersection is `O(depth)` amortized to `O(1)` per entity — the same top-down
shape `inherit_writing_mode`
([layout systems.rs](../../../crates/buiy_core/src/layout/systems.rs)) uses.

**Per-node data is read with absent-defaults, never as a flat non-`Option`
query.** A bare `Node` is not guaranteed to carry `BoxModel` / `Overflow` /
`Containment` (they arrive only via the `Style` bundle), so the walk fetches
each via `Option<&T>` (the query above) and applies a default that makes a
missing component a *no-op* for that node's clip contribution:

- **no `BoxModel`** ⇒ no border inset (the border-only inset of § A.3 rule 2
  uses a zero border edge, so the clip box is the bare border box);
- **no `Overflow`** ⇒ `Overflow::Visible` ⇒ the node contributes **no** clip
  (it is treated as non-clipping on both axes);
- **no `Containment`** ⇒ no `PAINT` clip (the node imposes no paint-containment
  box).

A non-`Option` `(&ResolvedLayout, &BoxModel, &Overflow)` query would instead
silently drop every bare `Node` from the walk and emit no `ClipRect` for it —
the same bare-`Node` reasoning architecture § 1.2 applies to the render
extract's `Option<&ClipRect>`.

**Pruning has real inputs.** The top-down walk does **not** seed `Display::None`
or `ContentVisibility::Hidden` subtrees: it applies the *same* skip predicates
[paint-order-and-top-layer.md § 5](paint-order-and-top-layer.md) owns — reading
the `Display` enum and `Containment.content_visibility` at each node and not
descending into a pruned subtree (a pruned entity's descendants are never
painted, so they need no `ClipRect`). Because the predicates are shared with
paint-order, a pruned-here / painted-there divergence is impossible by
construction.

### A.4 `ScrollOffset` moves content, not the clip — and never re-runs layout

The decisive invariant (README pillar 4; [layout components.rs `ScrollOffset`](../../../crates/buiy_core/src/layout/components.rs)):

> **Mutating `ScrollOffset` must NOT invalidate `ResolvedLayout`.**

Layout enforces this by *excluding* `Changed<ScrollOffset>` from the
`sync_styles` change-detection trigger set (the `Or<(Changed<…>)>` filter in
[layout systems.rs](../../../crates/buiy_core/src/layout/systems.rs); asserted by
`tests/layout_scroll_offset_no_invalidate.rs` and pinned in
[overflow-and-scrolling.md § 2.1](../2026-05-08-buiy-layout-design/overflow-and-scrolling.md#21-effect-on-resolvedlayout)).
`ResolvedLayout` reports content position *before* scroll is applied, so it is
byte-stable across scroll-only frames.

The render side honors the same split. A scroll container's clip box is its
**viewport** (padding box, § A.3) and is independent of `ScrollOffset` —
scrolling does not move the visible window. What scroll moves is the **content
translation** applied to the scrolled descendants. That translation is folded
into the transform bridge (§ B.3), **not** into `ClipRect`: render draws each
descendant at `GlobalTransform`-derived position shifted by the accumulated
ancestor `ScrollOffset`, then scissors against the (offset-independent)
`ClipRect`.

Because of this split, `WriteClipRects` has two cost regimes:

- **Layout-changed frame** (any clip input — `ResolvedLayout`, `Overflow`,
  `Containment`, hierarchy — changed): the affected root subtrees re-fold.
  The trigger is the union of `Changed<ResolvedLayout>`, `Changed<Overflow>`,
  `Changed<Containment>`, `Changed<Children>`, `Changed<ChildOf>` (and the
  pass re-walks any subtree containing a change, since a parent box change
  shifts every descendant clip).
- **Scroll-only frame** (`Changed<ScrollOffset>` and nothing else): a
  *separate, narrower* recompute runs. Because the clip **box** geometry is
  offset-independent (§ A.4), a scroll-only frame does **not** need to re-fold
  `ClipRect` at all for the offset-independent boxes — the only thing that
  changes is the content translation the bridge applies (§ B.3). The
  `Changed<ScrollOffset>` recompute therefore touches the *transform* of the
  scrolled descendants, not their clip rects, and **never** touches
  `sync_styles` / `taffy_compute`. This is the proof the README pillar 4 asks
  for: scroll responsiveness rides ECS change-detection on `ScrollOffset`
  alone, and the heavyweight layout pipeline (steps 0–9) is gated off it by
  construction.

> **Where it runs.** `write_clip_rects` runs in the **render-prep stage** —
> after `BuiySet::Layout` (so `ResolvedLayout` / `ScrollOffset` / `Containment`
> are final for the frame) and after the bridge's `write_buiy_transform`
> (§ B.2, so `Transform` is composed) but **before** `BuiySet::Picking` and
> before the render world's `ExtractSchedule`. Picking reads `ClipRect` to
> reject pointer hits outside the clip; render extract reads it to scissor.
> Render-prep is scheduled `.after(BuiySet::Animate).before(BuiySet::Picking)`
> — it runs **between** `Animate` and `Picking`, **not** inside
> `BuiySet::Render`'s preamble (`BuiySet::Render` is the last set, after
> `Picking`, so render-prep cannot live there). [architecture.md](architecture.md)
> pins the render-prep stage's exact set ordering (matching architecture § 5.2);
> this file requires only the ordering relation
> `Layout → bridge → WriteClipRects → Picking → Extract`.

### A.5 Verification (clip)

- **Gate #5 (layout-snapshot).** `ClipRect` geometry is asserted by the same
  snapshot harness that pins `ResolvedLayout`: a fixture tree with nested
  `Overflow::hidden()`, a scroll container with a non-zero `ScrollOffset`, and
  a `contain: paint` ancestor; the snapshot records each entity's `ClipRect`
  `{ min, max }`. Because the pass is a plain ECS system, this runs headless
  with no GPU. (This is why pillar 4 routes clip through a render-prep
  component rather than the GPU: it makes clip geometry a *layout-snapshot*
  property, not a pixel property.)
- **Scroll-no-relayout.** A fixture mutates `ScrollOffset` across frames and
  asserts (a) `ResolvedLayout` is byte-equal (the existing layout test), and
  (b) `LayoutTaffyComputeCount` did not increase — i.e. the scroll frame ran
  no Taffy compute. The clip box is byte-equal across the scroll; only the
  bridge's content translation moves.
- **Gate #10 (hit-target).** Picking's rejection of out-of-clip hits is proven
  in [verification.md](verification.md); it consumes the same `ClipRect`, so
  the paint-clip and hit-clip cannot diverge.

---

## B. The `Transform` / `GlobalTransform` bridge

### B.1 The ownership problem (pillar 5)

Bevy already owns a `bevy::prelude::Transform` and `GlobalTransform`.
`TransformPlugin` registers a `TransformSystems::Propagate` set that recomposes
`GlobalTransform` from `Transform` on every frame a `Transform` changed —
`*global_transform = GlobalTransform::from(*transform)` in both
`sync_simple_transforms` (the root / parentless path) and
`propagate_parent_transforms` (the child-propagation path)
([`bevy_transform-0.18.1/src/systems.rs`](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)).
Writing `GlobalTransform` directly would be clobbered by that propagation.

So Buiy lives **inside** Bevy's ownership model (the recommended "approach (a)"
in [transforms-and-containment.md § 2](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)):
**layout composes into `Transform`; Bevy propagates into `GlobalTransform`;
render reads `GlobalTransform`.** Render reads neither `ResolvedLayout` nor
`ResolvedTransform` for positioning — it reads `GlobalTransform`, the one value
Bevy guarantees is consistent across the whole hierarchy. The rejected
runner-up (render reads `ResolvedTransform` directly) re-implements the
hierarchical propagation Bevy already ships and diverges from what picking and
3D-anchored UI expect.

This bridge is exactly the work the Phase-8 follow-up
[*"Bevy `Transform` ownership bridge (`GlobalTransform` write)"*](../../plans/follow-ups.md)
deferred: Phase 8 produced `ResolvedTransform { matrix: Mat4 }` via sub-pass 6e
but wrote no Bevy `Transform`, because `buiy_core`'s layout test harness used
`MinimalPlugins` (no `TransformPlugin`), so a `Transform` write would have been
dead code. This spec pulls `TransformPlugin` into the app and lights up the
write.

### B.2 The composition: `ResolvedLayout` + `ResolvedTransform` → one `Transform`

A single render-prep system, `write_buiy_transform`, folds the two layout
outputs **and the accumulated ancestor scroll** into the entity's Bevy
`Transform`. It is the **only writer** of `Transform.translation` for a
laid-out entity (architecture § 5.2): there is no second per-entity writer and
no scroll-specific writer that could race it — "which write wins" is moot
because there is exactly one write per entity per frame.

Because each entity's translation depends on the *accumulated* `ScrollOffset`
of its scroll-container **ancestors** (§ B.3), the system is a **top-down
accumulation walk** (`Children`), not a flat per-entity query: it carries the
running ancestor scroll sum down the tree and composes it into each node's
base, exactly mirroring the `write_clip_rects` top-down shape (§ A.3):

```rust
/// Render-prep — folds layout's `ResolvedLayout.position`, the accumulated
/// ancestor scroll, and the optional composed `ResolvedTransform.matrix`
/// into the entity's Bevy `Transform`. The SOLE writer of a laid-out
/// entity's `Transform`. Buiy's own `Update`-scheduled propagation chain
/// (§ B.2.1) then owns the resulting `GlobalTransform`.
///
/// Runs in `Update`, after `BuiySet::Layout` (so both inputs are final)
/// and before the propagation chain + `BuiySet::Picking` (§ B.2.1).
/// Render reads the propagated `GlobalTransform`, never
/// `ResolvedLayout`/`ResolvedTransform`.
///
/// Top-down walk: per entity, compose
///   `base = position - accumulated_ancestor_scroll`
/// (lifted to a translation `Mat4`), then `base * ResolvedTransform.matrix`,
/// into ONE `Transform`. The walk re-runs a subtree (the `ScrollDirty`
/// top-down set, § B.3) iff its `ResolvedLayout` OR `ResolvedTransform`
/// changed OR any ancestor scroll-container's `ScrollOffset` changed.
///
/// Spec: clip-and-transform.md § B.2 / § B.3.
fn write_buiy_transform(
    // Roots are selected by the § A.3 two-disjunct predicate (a Node is a
    // root iff it has no `ChildOf` OR its `ChildOf` parent is not a Node),
    // identical to `write_clip_rects` — `Without<ChildOf>` alone would
    // silently drop the walk for a Buiy subtree parented under a non-Node
    // Bevy entity, leaving it with no `Transform`.
    all_nodes: Query<Entity, With<Node>>,
    child_of: Query<&ChildOf>,
    node_marker: Query<(), With<Node>>,
    layout: Query<(&ResolvedLayout, Option<&ResolvedTransform>, Option<&ScrollOffset>)>,
    children: Query<&Children>,
    mut transforms: Query<&mut Transform>,
    // `ScrollDirty` (§ B.3): the top-down set seeded by `Changed<ScrollOffset>`
    // (a scroll container re-translates its whole subtree) unioned with the
    // entities whose `ResolvedLayout`/`ResolvedTransform` changed, plus the
    // entities whose `ResolvedTransform` was *removed* this frame (6e drops it
    // on a return to identity — a removal does not match `Changed`, so the
    // seed reads `RemovedComponents<ResolvedTransform>` to recompose the node
    // back to its position-only translation). The walk only descends into
    // seeded subtrees; a steady-state frame visits none.
    dirty: Res<ScrollDirty>,
) {
    // Top-down from each seeded root, carrying the running ancestor scroll
    // sum `acc: Vec2` (starts at zero). At each entity `e`:
    //
    //   let base = Mat4::from_translation((layout.position - acc).extend(0.0));
    //   let composed = match resolved_transform {
    //       // 6e writes `ResolvedTransform` only when non-identity, and
    //       // removes a stale one otherwise (layout components.rs), so the
    //       // `None` arm is the identity fast path.
    //       Some(rt) => base * rt.matrix,
    //       None     => base,
    //   };
    //   *transforms.get_mut(e)? = Transform::from_matrix(composed);
    //   // Push down: a scroll-container child sees its parent's `acc` plus
    //   // this node's own `ScrollOffset` (if it is a scroll container).
    //   let child_acc = acc + this_node_scroll_offset;
    //
    // The y-down → y-up flip lives in the view uniform (§ B.4), NOT here —
    // the bridge stays in logical-px, y-down space so picking and `ClipRect`
    // share one coordinate frame.
}
```

Key points:

- **`ResolvedTransform` is optional by design.** Sub-pass 6e inserts it only
  when the composed transform is non-identity and removes a stale one otherwise
  ([components.rs `ResolvedTransform`](../../../crates/buiy_core/src/components.rs)).
  The `None` arm is the steady-state fast path (most entities have identity
  transforms). The walk keeps the system `O(0)` on frames where nothing moved by
  descending only into the `ScrollDirty` set (§ B.3) — seeded by
  `Changed<ResolvedLayout>`, `Changed<ResolvedTransform>`, **and**
  `Changed<ScrollOffset>` on a scroll-container ancestor — which is empty on a
  steady-state frame, matching the layout pipeline's steady-state contract. This
  is one re-run trigger feeding one writer, not two competing filters.
- **The transform origin** (`UiTransform.origin`, default `50% 50% 0`) is
  *intended to be baked into* `ResolvedTransform.matrix` by sub-pass 6e (it would
  compose `M = T·R·S·M_transform` around the resolved origin), so the bridge does
  a flat `base * matrix` and never re-derives origin. **As of R1 (transform-paint
  landed), this is the TARGET state, not current:** sub-pass 6e's
  `compose_transform` does NOT yet read `ui.origin`, so the composed matrix
  rotates/scales about the box-local TOP-LEFT, not the 50%/50% center (a
  layout-side residual surfaced by R1 — see the
  [follow-up](../../plans/follow-ups.md) residual A). The contract that matters
  for render holds regardless: render applies the affine EXACTLY as
  `GlobalTransform` encodes it (it does NOT independently re-apply an origin), so
  render and the bridge cannot diverge from picking, which applies the inverse of
  the same matrix
  ([transforms-and-containment.md § 1.2](../2026-05-08-buiy-layout-design/transforms-and-containment.md#12-layout-impact)).
- **Buiy owns the whole `Transform`.** An author positions UI via Buiy's
  `Position` / `UiTransform`, never Bevy's `Transform`; the bridge owns the
  resulting `Transform` for every laid-out entity, so author/gameplay
  transforms do not race the UI layout for the same component
  ([transforms-and-containment.md § 2](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)).
- **The bridge carries the *affine* part only — true perspective cannot
  survive it.** Bevy's `Transform` is affine (TRS) and `GlobalTransform` wraps
  an `Affine3A`; `Transform::from_matrix`
  ([`bevy_transform-0.18.1` `components/transform.rs`](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform))
  calls `Mat4::to_scale_rotation_translation()`, which **decomposes** the `Mat4`
  to TRS and **drops any projective (non-`(0,0,0,1)`) row**. So even though `ResolvedTransform.matrix` is a full `Mat4`, only its
  affine part survives the `from_matrix` bridge — a true perspective term placed
  in that row would be silently lost. v1 ships only the **`Flat` fast path**
  (no perspective in the bridge); perspective (`Preserve3d`, C-tier deferred,
  § B.5) rides a **separate render-side channel** applied at the view / primitive
  stage, *not* through `Transform`/`GlobalTransform`.

### B.2.1 Scheduling the propagation (mechanism owned here)

Bevy's `TransformPlugin` schedules propagation in **`PostUpdate`**. Buiy cannot
wait until `PostUpdate`: picking (`BuiySet::Picking`) and the render world's
`ExtractSchedule` both read `GlobalTransform` and both run *within / right after*
`Update`. So Buiy schedules the **three real public propagation systems**

```rust
bevy_transform::systems::{ mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms }
```

itself, in **`Update`**, **chained in that exact order**, **after**
`write_buiy_transform` and **before** `BuiySet::Picking`:

```rust
app.add_systems(
    Update,
    (
        write_buiy_transform,
        // Bevy's own propagation systems, chained in dependency order:
        mark_dirty_trees,            // marks subtrees whose Transform changed
        propagate_parent_transforms, // recomposes GlobalTransform down the tree
        sync_simple_transforms,      // root + parentless fast path
    )
        .chain()
        .before(BuiySet::Picking),
);
```

This guarantees `GlobalTransform` is **final** before picking and before
`ExtractSchedule` read it — the ordering constraint
[architecture.md § 5.3](architecture.md) restates (it owns only the ordering;
the *mechanism* — these three systems, this chain — lives here).

**Accepted re-propagation cost, and the engine-global heuristic it perturbs
(corrects the "PostUpdate re-run is bounded by changed-entity count,
idempotent" claim).** Change ticks clear **once per `World::update`** (at frame
end), **not** between schedules. So if the app *also* runs Bevy's default
`PostUpdate` `TransformSystems::Propagate` (e.g. a 3D scene that keeps the
stock `TransformPlugin` schedule), every Buiy subtree Buiy touched this frame
still has its `TransformTreeChanged` flagged `Changed` when `PostUpdate` runs,
so those subtrees **re-propagate a second time** in `PostUpdate`. That second
pass is **not** free. Two facts pin the bound exactly:

- **Buiy's subtrees re-propagate in `PostUpdate` either way — but for two
  different source reasons depending on the `enabled` flag.**
  `propagate_parent_transforms`'s early-exit is
  `if static_optimizations.enabled && !transform_tree.is_changed()`
  (`bevy_transform::systems::propagate_parent_transforms`,
  [`bevy_transform-0.18.1/src/systems.rs`](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)).
  The early-exit short-circuits on **either** conjunct, giving two paths:
  - **`enabled == true`** (≤30% of all transforms moved this frame): the
    `TransformTreeChanged` marks that Buiy's `Update` `mark_dirty_trees` set on
    every subtree Buiy moved **survive** the unclearable tick into `PostUpdate`
    (change ticks clear only at frame end, below), so `!is_changed()` is `false`
    for those subtrees and the early-exit **cannot** fire — the `PostUpdate`
    re-walk of Buiy's touched subtrees is forced by the surviving marks. The flag
    still skips Buiy's *untouched* static subtrees and the rest of the world.
  - **`enabled == false`** (>30% of all transforms moved): Buiy's `Update`
    `mark_dirty_trees` set **no** marks at all — it does
    `if !static_optimizations.enabled { return; }`
    (`bevy_transform-0.18.1/src/systems.rs`, in `mark_dirty_trees`) **before** the
    `set_changed()` loop, so the >30%-moving frame leaves `TransformTreeChanged`
    untouched. But `propagate_parent_transforms` re-propagates **every** root
    anyway, because its early-exit's first conjunct (`enabled`) is `false`, so the
    whole `enabled && !is_changed()` short-circuits to `false` for every subtree —
    Buiy's included. Either path re-propagates Buiy's subtrees in `PostUpdate`;
    the surviving-marks path (`enabled == true`) and the disabled-early-exit path
    (`enabled == false`) are two different source reasons for the same outcome,
    not the single "marks survive regardless of `enabled`" mechanism.
- **The flag is the global `StaticTransformOptimizations` resource**
  (`bevy_transform::systems::StaticTransformOptimizations`,
  [`bevy_transform-0.18.1/src/systems.rs`](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)),
  whose `enabled` field `mark_dirty_trees` **recomputes every call** from the
  **whole-world** moving-entity ratio (`changed_transforms.count() /
  transforms.count()`, default threshold **0.30**).

Two consequences follow, both world-global rather than Buiy-scoped:

1. **A co-existing dynamic 3D scene can disable the optimization globally.** If
   more than 30% of *all* transformed entities in the world moved this frame,
   `mark_dirty_trees` sets `enabled = false`, switching off the
   `enabled && !is_changed()` early-exit's first conjunct for **every** subtree.
   On that frame Buiy's `Update` `mark_dirty_trees` sets no marks (the
   `enabled == false` path above), so the `PostUpdate` re-walk is driven purely by
   the disabled early-exit and covers Buiy's *static* (untouched) subtrees too —
   the extra `Update`/`PostUpdate` propagation degrades to **O(Buiy-tree size)**
   rather than O(changed Buiy subtrees). When `enabled == true` (the common UI
   case) only Buiy's mark-bearing touched subtrees re-walk (previous bullet,
   first path).
2. **Buiy's extra `mark_dirty_trees` perturbs the shared heuristic.** Because
   `mark_dirty_trees` takes `ResMut<StaticTransformOptimizations>` and runs a
   whole-world `count()` over every `TransformTreeChanged`, Buiy running it an
   **extra** time in `Update` both **mutates this engine-global resource** (the
   `enabled` flag the app's own `PostUpdate` pass then reads) and **adds a
   whole-world `count()` pass** unrelated to Buiy's tree size. The two
   `StaticTransformOptimizations` writers (Buiy's `Update` `mark_dirty_trees`
   and the app's `PostUpdate` one) are ordered **only** by the fact that `Update`
   precedes `PostUpdate` — there is no explicit set ordering between them, since
   Buiy's `Update` propagation copies are a **distinct schedule**, **not**
   members of `TransformSystems::Propagate` (that set lives in `PostUpdate`).

**Escape hatch (adopt only if measured).** If this becomes a measured
**gate #14** problem (the per-frame budget), the fix is a **Buiy-scoped
propagation** — a private propagation chain over only Buiy's subtree that does
**not** touch the shared `StaticTransformOptimizations` resource (no `ResMut`,
no whole-world `count()`) — adopted at the cost of re-implementing Bevy's
change-gating locally. We do **not** take it pre-emptively: a **UI-only** app
that does **not** add propagation to `PostUpdate` (no `TransformPlugin`
`PostUpdate` schedule) pays the propagation exactly **once** (the `Update` run
only) and never co-exists with a >30%-moving dynamic scene, so for that app the
double-pass and the shared-resource perturbation simply do not arise.

### B.3 Scroll content translation rides the bridge

Scroll is **not** a second transform writer — it is folded into the *same*
top-down `write_buiy_transform` walk (§ B.2), applied here rather than in
`ClipRect`. The walk's `base` for a scrolled descendant is
`position - accumulated_ancestor_scroll`: it subtracts each scroll-container
ancestor's `ScrollOffset` accumulated down the tree (content scrolls up as the
offset grows). Because `ScrollOffset` is layout output that does *not*
invalidate `ResolvedLayout`, a scroll-only frame re-runs only the
translation-composition walk, never a re-layout. This is the single place a
scroll-only frame does measurable work, and it is bounded by the count of
scrolled descendants, not the tree size.

The re-run trigger is the **`ScrollDirty` top-down set** (the one trigger
feeding the one writer, § B.2): an ancestor scroll-container whose
`ScrollOffset` changed seeds its whole subtree into the set, just as a node
whose `ResolvedLayout`/`ResolvedTransform` changed seeds itself. There is no
separate scroll filter and no flat `Or<…>` query — the union lives in
`ScrollDirty`, and the walk composes base − scroll × matrix once per seeded
entity.

> The exact mechanism for "an ancestor's `ScrollOffset` changed, re-translate
> my descendants" is the same multi-level-descendant problem layout solved in
> Phase 14 with a dirty-set resource (`ContainerSizeDirty`) — the bridge
> reuses that pattern with a `ScrollDirty(HashSet<Entity>)` render-prep
> resource seeded by `Changed<ScrollOffset>` (plus the
> `Changed<ResolvedLayout>` / `Changed<ResolvedTransform>` seeds, § B.2) and
> drained top-down. This keeps the bridge a thin consumer and is the
> render-prep analogue of the layout cascade, not a new invalidation
> philosophy.

### B.4 The coordinate contract (pinned)

This is the contract the bridge fixes once, for the whole render pipeline:

| Space | Convention |
|---|---|
| **Buiy logical-pixel** | `ResolvedLayout.position` is the **top-left** corner, **window-relative**, **y-down** (y grows downward, the CSS/screen convention), in logical (DPI-independent) pixels. `ClipRect` shares this exactly. |
| **Bevy `Transform` translation** | The bridge lifts the logical-px top-left straight into `Transform.translation` (`z = 0`), **keeping y-down** at the ECS level. No flip happens in the bridge. |
| **View uniform (GPU)** | The y-down → Bevy's y-up screen convention flip is folded into the **view-projection uniform** for Buiy's render node, so every primitive inherits it once. Logical-px → physical-px scaling (the window `scale_factor`) is folded into the same uniform. |

**`scale_factor` lives in two places, by design (reconciles
[effect-compositor.md § 2.1](effect-compositor.md)).** `GlobalTransform`,
`ClipRect`, and the bridge all stay in **logical px** — `scale_factor` is *not*
baked into them. Instead the window `scale_factor` is:

1. folded into the **GPU view-projection uniform** (above), so every primitive
   renders at physical-pixel resolution; **and**
2. read **explicitly CPU-side** (from the extracted view / window) by the
   [effect compositor](effect-compositor.md), which multiplies the
   logical-px painted bounds by `scale_factor` to size its off-screen
   `TextureDescriptor` in physical pixels.

These are the *same* scalar applied at two layers; because `GlobalTransform`
stays logical-px (this section), the compositor must re-apply `scale_factor`
CPU-side rather than reading it back out of the transform.

The decisive move: **the Phase-0 per-instance y-flip migrates into the view
uniform.** Phase 0 (and the temporary `Visual` extract) flipped y per instance
on the CPU as a hack; the hybrid-handoff upgrade (README pillar 3) retires that
along with the per-instance radius hack. With one view-projection matrix
carrying `(y-flip · logical→physical scale · projection)`, the entire pipeline
— bridge, `ClipRect`, picking — operates in **one** logical-px, y-down,
window-relative frame, and only the GPU sees y-up. This is why neither the
bridge nor `ClipRect` flips y: a flip on either would double-apply against the
view uniform.

`GlobalTransform` therefore carries logical-px, y-down, window-relative world
coordinates (the parent chain of a Buiy subtree is all Buiy entities composed
the same way; a non-Buiy Bevy parent is invisible to layout per the
[topological invariant](../2026-05-08-buiy-layout-design/architecture.md#5-topological-invariant),
so it does not perturb the contract). Picking applies the inverse
`GlobalTransform` to pointer coordinates and lands in the same frame
`ResolvedLayout` and `ClipRect` live in — the three cannot diverge.

### B.5 Perspective / `transform-style` / `backface-visibility` consumption

**Status (R1, transform-paint landed):** the **2D affine** half of the
transform-paint follow-up now LANDS via the GPU vertex stage. Extract reads the
`GlobalTransform` 2D linear part (`global_transform.affine().matrix3` xy columns
— NOT a re-read of `ResolvedTransform`, per the pillar-5 contract in § B.2) and
the quad + shadow shaders transform each box-local corner by it before the
logical→clip view map (`PackedInstance` grew 52 B → 68 B by appending the 2x2
basis after the clip fields; vertex attrs `@location(8)/(9)`). The
**PAINT-clip half** was already done (§ A.3 rule 3 / `clip_for_primitive`). The
**perspective channel / `Preserve3d` / `backface-visibility`** stay C-tier
deferred (the bullets below).

**Fidelity bound (R1):** render faithfully reproduces **rotation + non-uniform
scale**, but **skew (`TransformMatrix::Skew`) and general
`TransformMatrix::Matrix`** are BOUNDED by the bridge's TRS-only
`Transform::from_matrix` decompose (§ B.2 — a Bevy `Transform` cannot represent
a general shear, lossy by the same decompose that drops the projective row).
Faithful skew is a separate residual; it needs the bridge to stop round-tripping
through TRS (or render to read a non-TRS source). Not covered by R1.

Phase 8 *stored* three `UiTransform` fields with no consumer
([components.rs `UiTransform`](../../../crates/buiy_core/src/layout/components.rs);
[transforms-and-containment.md § 4](../2026-05-08-buiy-layout-design/transforms-and-containment.md#4-perspective-and-3d)).
The remaining (C-tier) consumption — resolving the
[*"`UiTransform` paint + `Containment` PAINT clip + perspective/backface"*](../../plans/follow-ups.md)
follow-up's perspective/3D half:

- **`perspective: Option<Length>`** — the 3D viewing distance for `Preserve3d`
  children. Resolved to logical px and folded into the **perspective matrix
  factor** of the *parent's* contribution to a child's composed transform.
  `ResolvedTransform.matrix` is a full `Mat4` (Phase 8 chose `Mat4`, not
  `Affine2` —[components.rs](../../../crates/buiy_core/src/components.rs)), so it
  *can hold* a projective perspective row. **But Bevy's transform is affine and
  cannot transport that row** (§ B.2): `Transform::from_matrix` decomposes to TRS
  and `GlobalTransform` wraps `Affine3A`, so the projective term is dropped at the
  bridge. The perspective therefore does **not** flow through
  `Transform`/`GlobalTransform`; it is a **render-side channel** applied at the
  view/primitive stage (the same place the view uniform lives, § B.4), reading
  the parent's resolved `perspective` directly. v1 ships only the `Flat` fast path
  (no perspective in the bridge); the perspective channel is the C-tier
  `Preserve3d` extension. No new *layout* component (the data is already on
  `UiTransform`).
- **`transform-style: Flat | Preserve3d`** — selects whether a subtree
  composes in 3D (`Preserve3d`: children's matrices multiply in 3D before
  projection) or is **flattened** to a 2D layer at this boundary (`Flat`, the
  default and the common UI case). `Flat` is the trigger that turns the subtree
  into one composited 2D layer — which is exactly an
  [`EffectGroup`](component-model.md) boundary. So `Preserve3d` is consumed by
  *composition* (matrices stay 3D through the bridge into `GlobalTransform`);
  `Flat` at a 3D boundary is consumed by the
  [effect compositor](effect-compositor.md) (the subtree renders to an
  off-screen target, then composites flat). v1 ships the `Flat` fast path
  (no 3D children → no off-screen flatten); `Preserve3d` cross-sibling 3D is a
  C-tier extension that rides the already-built compositor.
- **`backface-visibility: Visible | Hidden`** — `Hidden` culls the entity when
  its transformed normal faces away from the viewer. This is a per-primitive
  **render** decision (the sign of the transformed z-basis of
  `GlobalTransform`), so render reads `GlobalTransform` + a one-bit
  `backface_visibility` flag extracted alongside it. Because the data is one
  bit and already on `UiTransform`, no new component is introduced — render
  extract reads `UiTransform.backface_visibility` directly (it is a layout-owned
  component render already reads for the SC trigger).

### B.6 What the bridge does *not* do

- It does **not** introduce a property tree (README pillar 7). `Transform` /
  `GlobalTransform` *is* Bevy's transform tree; the bridge reuses it. The
  revisit trigger (animating transform without re-running layout) is named in
  the README and not taken here.
- It does **not** write `GlobalTransform` directly (that is the rejected
  approach (b) — clobbered by propagation).
- It does **not** flip y, scale logical→physical, or apply projection — those
  live in the view uniform (§ B.4), owned by [architecture.md](architecture.md).

### B.7 Verification (transform)

- **Bridge composition** — a fixture with a Bevy-parented Buiy subtree, a child
  carrying a non-identity `UiTransform`; assert the child's `Transform` equals
  `from_translation(ResolvedLayout.position) * ResolvedTransform.matrix`, and —
  after running propagation — its `GlobalTransform` equals the parent-composed
  product. **The test must add `TransformPlugin`** (or chain the three
  `bevy_transform::systems::{ mark_dirty_trees, propagate_parent_transforms,
  sync_simple_transforms }` directly, the same chain § B.2.1 schedules) before
  reading `GlobalTransform`: the `buiy_core` harness runs `MinimalPlugins`,
  which does **not** include `TransformPlugin`, so a bare "manual
  `TransformSystems::Propagate` run" is ambiguous — that set is empty unless
  some plugin populated it. Headless, no GPU.
- **Identity fast path** — an entity with identity transform and no scrolled
  ancestor has **no** `ResolvedTransform` and its `Transform.translation` equals
  the resolved position with `z = 0`; the walk visits **no** entities on a
  steady-state frame (the `ScrollDirty` set is empty, § B.3), asserted via a
  per-frame visited-count probe analogous to `SyncStylesIterCount`.
- **Coordinate contract** — a golden-image fixture (gate #2) renders a box at a
  known logical-px top-left and asserts the pixel lands y-down-correct, proving
  the y-flip is in the view uniform exactly once (a double-flip or missing flip
  is a visible regression).
- **Perspective / backface** — a fixture with a `Preserve3d` parent + rotated
  child asserts the **affine** part of the composed transform survives into
  `GlobalTransform`, and asserts the **projective perspective row does NOT**
  (it is dropped by `Transform::from_matrix`'s TRS decomposition, § B.2) —
  perspective is verified instead on the render-side channel, not on
  `GlobalTransform`. A `backface-visibility: hidden` entity rotated 180° about y
  is culled by render (gate #2 golden shows it absent). v1 exercises only the
  `Flat` fast path; the perspective-channel assertion is a C-tier follow-up.

---

## Cross-file dependencies

- **[architecture.md](architecture.md)** owns the render-prep stage's exact
  system-set ordering — render-prep is scheduled
  `.after(BuiySet::Animate).before(BuiySet::Picking)` (between `Animate` and
  `Picking`, **not** inside `BuiySet::Render`, which is the last set after
  `Picking`; matching architecture § 5.2) — and the **view-uniform**
  (y-flip + logical→physical scale + projection) this file relies on (§ B.4).
  This file requires only `Layout → write_buiy_transform → write_clip_rects →
  Picking → Extract` and that the view uniform carries the y-flip.
- **[component-model.md](component-model.md)** owns `Border` (whose radius
  feeds the deferred rounded-clip seam, § A.2) and `EffectGroup` (the
  `Flat`-boundary target, § B.5).
- **[effect-compositor.md](effect-compositor.md)** consumes the `Flat` /
  `Preserve3d` flatten boundary (§ B.5).
- **[paint-order-and-top-layer.md](paint-order-and-top-layer.md)** owns the
  `Display::None` / `ContentVisibility` skip rules the clip walk prunes on
  (§ A.3).
- **Layout** (`buiy-layout-design`) owns every input: `ResolvedLayout`,
  `ResolvedTransform`, `Overflow`, `ScrollOffset`, `Containment`, `UiTransform`,
  and the `ScrollOffset`-does-not-invalidate-`ResolvedLayout` invariant this
  file's pillar-4 proof rests on.

## Open items

- **Rounded-rect / `clip-path` clipping** (§ A.2) is a render-side **C-tier**
  reserved seam (rounded-rect via the `ClipRadius` carrier, `clip-path` via the
  path primitive), tracked in [verification.md](verification.md)'s gate map —
  not a README § 5 open question. Flagged here so the AABB-only `ClipRect` is
  not mistaken for the final clip shape.
- **`Preserve3d` cross-sibling 3D compositing** (§ B.5) is C-tier; v1 ships the
  `Flat` fast path and the reserved compositor seam.
