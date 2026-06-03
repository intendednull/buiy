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
  `ResolvedTransform` fold into one Bevy `Transform`, why
  `TransformSystems::Propagate` (not Buiy) owns `GlobalTransform`, the
  logical-pixel / y-down coordinate contract, and the consumption of the
  Phase-8 stored-but-unconsumed `perspective` / `transform-style` /
  `backface-visibility` data.

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

```rust
/// Per-entity computed clip, written by `write_clip_rects` (render-prep)
/// and read by render extract + picking. Render reads it; render never
/// re-derives it. Absent on entities that are not clipped by any
/// ancestor (the unbounded case) — an absent `ClipRect` means "no clip".
///
/// Geometry is in the same logical-pixel, y-down, window-relative space
/// as `ResolvedLayout.position` (see § B.4), so render extract can
/// intersect it against an instance's box without a coordinate change.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct ClipRect {
    /// Top-left corner, logical px, window-relative (y-down).
    pub min: Vec2,
    /// Bottom-right corner, logical px, window-relative (y-down).
    pub max: Vec2,
}
```

The rect is an axis-aligned bounding box (AABB) in logical pixels. v1 stores a
**rectangular** clip only — non-rectangular clips (`border-radius` rounding of
the clip edge, `clip-path`) are deferred:

- **Rounded-rect clip** (the `Border.radius` of a clipping ancestor) is the
  first C-tier extension. `ClipRect` reserves the seam by carrying only the
  AABB now; the rounded variant adds a sibling `ClipRadius` component (per-corner
  elliptical radii, mirroring [`Border`](component-model.md)) that render's SDF
  quad shader already understands. Marked open where the AABB is insufficient;
  see [README § 5](README.md#5-open-questions) is **not** the owner — this is a
  render-side C-tier fast-follow tracked in [verification.md](verification.md)'s
  gate map, not a cross-spec open question.
- **`clip-path`** (arbitrary path clip) is E-tier, named only, no component.

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
2. For each ancestor `A` walked via `ChildOf` from `E` to the layout root, in
   order:
   - if `A.Overflow` clips on an axis (`Hidden` / `Clip` / `Scroll` / `Auto` —
     i.e. *not* `Visible`), intersect with `A`'s **content box** on that axis
     (the border box inset by border + padding — the same content rect Taffy
     resolved; `ResolvedLayout` reports the border box, so the pass insets it
     using `A`'s `BoxModel` border + padding edges);
   - if `A` is a **scroll container** (`A.Overflow.is_scroll_container()`),
     the viewport is `A`'s content box, and the *child-facing* clip is that
     content box — `ScrollOffset` does **not** move the clip box itself (the
     visible window is fixed in `A`'s frame); it moves the *content* (see § A.4
     for where the offset is applied);
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
    roots: Query<Entity, (With<Node>, Without<ChildOf>)>,
    nodes: Query<(&ResolvedLayout, &BoxModel, &Overflow, Option<&Containment>, Option<&ScrollOffset>)>,
    children: Query<&Children>,
    existing: Query<&ClipRect>,
) { /* top-down ancestor-clip fold; see § A.3 */ }
```

The system is written **top-down from each root** (push the running clip down
through `Children`) rather than bottom-up per entity, so each ancestor box is
computed once and the running intersection is `O(depth)` amortized to `O(1)`
per entity — the same top-down shape `inherit_writing_mode`
([layout systems.rs](../../../crates/buiy_core/src/layout/systems.rs)) uses.
`Display::None` and `ContentVisibility::Hidden` subtrees are pruned from the
walk (their descendants are not painted, so they get no `ClipRect`); this
mirrors the skip rules detailed in [paint-order-and-top-layer.md](paint-order-and-top-layer.md).

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
**viewport** (content box) and is independent of `ScrollOffset` — scrolling
does not move the visible window. What scroll moves is the **content
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
> [architecture.md](architecture.md) pins the render-prep stage's exact set
> ordering inside `BuiySet::Render`'s preamble; this file requires only the
> ordering relation `Layout → bridge → WriteClipRects → Picking → Extract`.

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
`*global_transform = GlobalTransform::from(*transform)` at both the root path
and the child-propagation path
([`bevy_transform-0.18.1/src/systems.rs:201` / `:376`](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)).
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
outputs into the entity's Bevy `Transform`:

```rust
/// Render-prep — folds layout's `ResolvedLayout.position` and the
/// optional composed `ResolvedTransform.matrix` into the entity's Bevy
/// `Transform`. Bevy's `TransformSystems::Propagate` then owns the
/// resulting `GlobalTransform`. Buiy owns the `Transform` of every
/// entity it lays out.
///
/// Runs after `BuiySet::Layout` (so both inputs are final) and before
/// `TransformSystems::Propagate` (PostUpdate). Render reads the
/// propagated `GlobalTransform`, never `ResolvedLayout`/`ResolvedTransform`.
///
/// Spec: clip-and-transform.md § B.2.
fn write_buiy_transform(
    mut q: Query<
        (&ResolvedLayout, Option<&ResolvedTransform>, &mut Transform),
        Or<(Changed<ResolvedLayout>, Changed<ResolvedTransform>)>,
    >,
) {
    for (layout, xform, mut transform) in &mut q {
        // Position: layout's window-relative top-left, lifted into
        // Bevy's translation. The y-down → y-up flip lives in the view
        // uniform (§ B.4), NOT here — the bridge stays in logical-px,
        // y-down space so picking and ClipRect share one coordinate frame.
        let base = Mat4::from_translation(layout.position.extend(0.0));
        let composed = match xform {
            // 6e writes `ResolvedTransform` only when non-identity, and
            // removes a stale one otherwise (layout components.rs), so the
            // `None` arm is the identity fast path.
            Some(rt) => base * rt.matrix,
            None => base,
        };
        *transform = Transform::from_matrix(composed);
    }
}
```

Key points:

- **`ResolvedTransform` is optional by design.** Sub-pass 6e inserts it only
  when the composed transform is non-identity and removes a stale one otherwise
  ([components.rs `ResolvedTransform`](../../../crates/buiy_core/src/components.rs)).
  The `None` arm is the steady-state fast path (most entities have identity
  transforms), and the `Or<(Changed<ResolvedLayout>, Changed<ResolvedTransform>)>`
  filter keeps the system `O(0)` on frames where nothing moved — matching the
  layout pipeline's steady-state contract.
- **The transform origin** (`UiTransform.origin`, default `50% 50% 0`) is
  *already baked into* `ResolvedTransform.matrix` by sub-pass 6e (it composes
  `M = T·R·S·M_transform` around the resolved origin), so the bridge does a
  flat `base * matrix` and never re-derives origin. Render and the bridge thus
  agree with picking, which applies the inverse of the same matrix
  ([transforms-and-containment.md § 1.2](../2026-05-08-buiy-layout-design/transforms-and-containment.md#12-layout-impact)).
- **Buiy owns the whole `Transform`.** An author positions UI via Buiy's
  `Position` / `UiTransform`, never Bevy's `Transform`; the bridge owns the
  resulting `Transform` for every laid-out entity, so author/gameplay
  transforms do not race the UI layout for the same component
  ([transforms-and-containment.md § 2](../2026-05-08-buiy-layout-design/transforms-and-containment.md#2-mapping-to-bevy-transform)).

### B.3 Scroll content translation rides the bridge

The content translation from accumulated ancestor `ScrollOffset` (§ A.4) is
applied here, not in `ClipRect`. The bridge's `base` translation for a
scrolled descendant subtracts each scroll-container ancestor's `ScrollOffset`
(content scrolls up as the offset grows). Because `ScrollOffset` is layout
output that does *not* invalidate `ResolvedLayout`, the bridge's
change-detection filter gains `Changed<ScrollOffset>` on scroll-container
*descendants* — but the recompute is a pure translation update, never a
re-layout. This is the single place a scroll-only frame does measurable work,
and it is bounded by the count of scrolled descendants, not the tree size.

> The exact mechanism for "an ancestor's `ScrollOffset` changed, re-translate
> my descendants" is the same multi-level-descendant problem layout solved in
> Phase 14 with a dirty-set resource (`ContainerSizeDirty`) — the bridge
> reuses that pattern with a `ScrollDirty(HashSet<Entity>)` render-prep
> resource seeded by `Changed<ScrollOffset>` and drained top-down. This keeps
> the bridge a thin consumer and is the render-prep analogue of the layout
> cascade, not a new invalidation philosophy.

### B.4 The coordinate contract (pinned)

This is the contract the bridge fixes once, for the whole render pipeline:

| Space | Convention |
|---|---|
| **Buiy logical-pixel** | `ResolvedLayout.position` is the **top-left** corner, **window-relative**, **y-down** (y grows downward, the CSS/screen convention), in logical (DPI-independent) pixels. `ClipRect` shares this exactly. |
| **Bevy `Transform` translation** | The bridge lifts the logical-px top-left straight into `Transform.translation` (`z = 0`), **keeping y-down** at the ECS level. No flip happens in the bridge. |
| **View uniform (GPU)** | The y-down → Bevy's y-up screen convention flip is folded into the **view-projection uniform** for Buiy's render node, so every primitive inherits it once. Logical-px → physical-px scaling (the window `scale_factor`) is folded into the same uniform. |

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

Phase 8 *stored* three `UiTransform` fields with no consumer
([components.rs `UiTransform`](../../../crates/buiy_core/src/layout/components.rs);
[transforms-and-containment.md § 4](../2026-05-08-buiy-layout-design/transforms-and-containment.md#4-perspective-and-3d)).
This bridge consumes them — resolving the
[*"`UiTransform` paint + `Containment` PAINT clip + perspective/backface"*](../../plans/follow-ups.md)
follow-up's transform half (the PAINT-clip half is § A.3):

- **`perspective: Option<Length>`** — the 3D viewing distance for `Preserve3d`
  children. Resolved to logical px and folded into the **perspective matrix
  factor** of the *parent's* contribution to a child's composed transform.
  Because `ResolvedTransform.matrix` is already a full `Mat4` (Phase 8 chose
  `Mat4`, not `Affine2`, expressly to carry perspective —
  [components.rs](../../../crates/buiy_core/src/components.rs)), the perspective
  term composes into the matrix and flows through the bridge unchanged; render
  reads the perspective-bearing `GlobalTransform`. No new component.
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
  `from_translation(ResolvedLayout.position) * ResolvedTransform.matrix`, and
  (after a manual `TransformSystems::Propagate` run) its `GlobalTransform`
  equals the parent-composed product. Headless, no GPU.
- **Identity fast path** — an entity with identity transform has **no**
  `ResolvedTransform` and its `Transform.translation` equals the resolved
  position with `z = 0`; the `Or<Changed<…>>` filter does no work on a
  steady-state frame (assert via a per-frame iter-count probe analogous to
  `SyncStylesIterCount`).
- **Coordinate contract** — a golden-image fixture (gate #2) renders a box at a
  known logical-px top-left and asserts the pixel lands y-down-correct, proving
  the y-flip is in the view uniform exactly once (a double-flip or missing flip
  is a visible regression).
- **Perspective / backface** — a fixture with a `Preserve3d` parent + rotated
  child asserts the perspective term survives into `GlobalTransform`; a
  `backface-visibility: hidden` entity rotated 180° about y is culled by render
  (gate #2 golden shows it absent).

---

## Cross-file dependencies

- **[architecture.md](architecture.md)** owns the render-prep stage's exact
  system-set ordering inside `BuiySet::Render` and the **view-uniform**
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

- **Rounded-rect / `clip-path` clipping** (§ A.2) is a render-side C/E-tier
  fast-follow, tracked in [verification.md](verification.md)'s gate map — not a
  README § 5 open question. Flagged here so the AABB-only `ClipRect` is not
  mistaken for the final clip shape.
- **`Preserve3d` cross-sibling 3D compositing** (§ B.5) is C-tier; v1 ships the
  `Flat` fast path and the reserved compositor seam.
