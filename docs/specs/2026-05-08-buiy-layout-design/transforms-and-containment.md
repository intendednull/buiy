# Transforms and containment

**Parent:** [README.md](README.md)

How an entity's box is visually transformed without affecting layout flow (`UiTransform`), and how layout/paint/size containment lets the engine skip work for off-screen or stable subtrees (`Containment`).

## 1. `UiTransform`

The component is named `UiTransform`, **not** `Transform`: `bevy::prelude::Transform` is glob-imported across `buiy_core` (`layout/mod.rs` does `use bevy::prelude::*`), so a Buiy `Transform` component would collide with Bevy's. The longhands keep their CSS names (`Translate` / `Rotate` / `Scale`); Bevy 0.18 has no prelude components of those names, so they do not collide.

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct UiTransform {
    pub matrix: TransformMatrix,
    pub origin: TransformOrigin,
    pub style:  TransformStyle,         // Flat | Preserve3d
    pub perspective: Option<Length>,
    pub backface_visibility: BackfaceVisibility,
}

#[derive(Reflect, Clone, Default, PartialEq)]
pub enum TransformMatrix {
    #[default]
    None,                                                   // identity
    Translate(Length, Length, Length),                      // 3D translate
    Rotate(Quat),                                           // arbitrary 3D rotation
    Scale(f32, f32, f32),
    Skew(f32, f32),                                         // x, y in radians
    Matrix(Mat4),                                           // explicit 4×4
    Compose(Vec<TransformMatrix>),                          // matrix product A · B · … (see § 1, composition order)
}

#[derive(Reflect, Clone, Copy, PartialEq)]
pub struct TransformOrigin { pub x: Length, pub y: Length, pub z: Length }

// CSS default is `50% 50% 0`, which `#[derive(Default)]` (all-zero
// `Length`s) would not produce — so `Default` is hand-written.
impl Default for TransformOrigin {
    fn default() -> Self {
        Self { x: Length::Percent(50.0), y: Length::Percent(50.0), z: Length::ZERO }
    }
}

#[derive(Reflect, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransformStyle {
    #[default]
    Flat,
    Preserve3d,
}

#[derive(Reflect, Clone, Copy, Default, PartialEq, Eq)]
pub enum BackfaceVisibility {
    #[default]
    Visible,
    Hidden,
}
```

Defaults:

- `matrix` = `TransformMatrix::None`
- `origin` = `TransformOrigin { x: Length::Percent(50.0), y: Length::Percent(50.0), z: Length::ZERO }` (CSS default `50% 50% 0`)
- `style` = `Flat`
- `perspective` = `None`
- `backface_visibility` = `Visible`

**Composition order (single convention for the whole spec).** The final transform matrix is the inner product conjugated by the resolved **transform-origin** `O` (so rotate/scale/matrix pivot about `O`, default the box center):

```
M = T(O) · ( T_translate · R_rotate · S_scale · M_transform ) · T(-O)
```

where `O = (resolve(origin.x, box.w), resolve(origin.y, box.h), 0)` in px (default `50% 50%` → box center). The inner product is written left-to-right in outermost-to-innermost order: `T_translate` is the outermost inner factor and `M_transform` the innermost, so a child point `p` is transformed as `M · p` and therefore feels the *rightmost* (innermost) factor first. The `T(O)·…·T(-O)` conjugation is the CSS `transform-origin` pivot; it is baked once at 6e so every consumer of `ResolvedTransform`/`GlobalTransform` (render box/quad, the coverage glyph/icon producers, picking) agrees. A pure translate is origin-invariant, so 6e skips the conjugation for the translate-only case (bit-identical). Identity inner ⇒ `T(O)·I·T(-O) == I` (the identity fast-path is preserved exactly). *(Landed 2026-07-01; before then 6e ignored `origin` and the pivot was the box top-left — see `docs/specs/2026-07-01-glyph-affine-transform-design.md`.)* For `TransformMatrix::Compose([A, B, …])` the list is written outermost-first and applied as the matrix product `A · B · …` — same rule: the rightmost/innermost entry transforms a child point first. This is exactly the convention CSS uses for a `transform:` list written left-to-right. Phase 8 implemented this product; there is no separate "right-to-left" rule to reconcile — "rightmost-applied-first" is just what reading the matrix product `M · p` means.

**Translate length units.** A translate term carries a [`Length`](container-queries-and-writing-modes.md) per axis. `Px` resolves to its magnitude. **`Percent` resolves against the entity's OWN border box** per CSS Transforms — `translateX(p%)` = `p%` of border-box **width**, `translateY(p%)` = `p%` of **height** (each axis against its own dimension); `translateZ` percentages are invalid in CSS and resolve to **0**. This resolution happens in sub-pass **6e** (`transform_composition`), which reads the entity's **current-frame** Taffy border box (`tree.tree.layout(node).size`, the same box `anchor_resolution` (6d) reads — *not* `ResolvedLayout`, which is still last-frame at 6e time since `WriteResolvedLayout` runs later). `Cq*` translate (`cqw/cqh/cqi/cqb/cqmin/cqmax`) is a **residual deferral**: it needs the entity's nearest CQ-ancestor container frame (the sticky-L4 / `resolve_cq_unit_px` machinery), which 6e does not gather, so it resolves to **0.0** and fires a one-shot warn. `Fr`/`anchor-size()` are meaningless on a transform and likewise resolve to 0.

### 1.1 Longhand components

CSS exposes `translate`, `rotate`, `scale` as separate properties applied independently of `transform`. Buiy mirrors:

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Translate(pub Length, pub Length, pub Length);

#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Rotate(pub Quat);

#[derive(Component, Reflect, Clone)]
#[reflect(Component, Default)]
pub struct Scale(pub f32, pub f32, pub f32);

// CSS default scale is identity `(1, 1, 1)`, not derived zeros.
impl Default for Scale {
    fn default() -> Self {
        Scale(1.0, 1.0, 1.0)
    }
}
```

When present, these compose with `UiTransform.matrix` per the single convention in [§ 1](#1-transform): `M = T_translate · R_rotate · S_scale · M_transform` (the written order `translate → rotate → scale → transform.matrix` is outermost-to-innermost, so `M_transform` transforms a child point first). The composition runs as `PostTaffyOverrides` sub-pass **6e** (landed Phase 8, extending the shipped sub-pass chain 6a sticky → 6b table → 6c multicol → 6d anchor → 6e transform; per [architecture.md § 3](architecture.md#3-system-pipeline)); the composed matrix is written to the private `ResolvedTransform` component, which the render bridge consumes (`write_buiy_transform`, [§ 2](#2-mapping-to-bevy-transform)). Translate percent terms resolve against the entity's own current-frame border box at this sub-pass (see [§ 1](#1-transform), "Translate length units"); `Cq*` translate is a residual `0.0` deferral. Transform-triggered stacking-context detection (landed Phase 9, [stacking-and-top-layer.md](stacking-and-top-layer.md)) runs as sub-pass **6f**, *after* 6e, since it reads the composed matrix produced by 6e.

### 1.2 Layout impact

`UiTransform` does **not** affect Taffy compute. A transformed element occupies its un-transformed box for layout purposes; siblings ignore the transform.

Exceptions where transforms *do* affect layout:

- Stacking-context formation ([§ 3](#3-stacking-context-formation)).
- The transformed entity itself becomes a containing block for `Position.kind = PositionKind::Fixed` descendants (CSS quirk).
- `transform-origin` and the resolved transform are read by `buiy-input-events-design`'s hit-test pass for transformed elements — picking is done in the entity's *transformed* space.

## 2. Mapping to Bevy `Transform`

Bevy already has a `bevy::prelude::Transform` and a `GlobalTransform`. The layout pipeline must cooperate with Bevy's transform ownership: `TransformSystems::Propagate` *recomposes `GlobalTransform` from `Transform` on any frame `Transform` is changed or added (and the subtree isn't statically optimized out)* — both the root path (`bevy_transform-0.18.1/src/systems.rs:201`, `*global_transform = GlobalTransform::from(*transform)`) and the child-propagation path (same file, `:376`, `*parent_transform = GlobalTransform::from(*transform)`) recompose via `GlobalTransform::from(*transform)`. Writing `GlobalTransform` directly would be clobbered by that propagation.

**Approach (a) — chosen and implemented.** Buiy composes the layout-derived position + the resolved `UiTransform`/longhand matrix (the matrix is exposed via the private `ResolvedTransform` component, written by sub-pass 6e) and writes the result into the entity's Bevy `Transform`. Bevy's `TransformSystems::Propagate` then composes that `Transform` into `GlobalTransform` the normal way, so Buiy lives inside Bevy's ownership model instead of fighting it. Buiy owns the `Transform` of every entity it lays out.

> **Landed — relocated to the render bridge (render Phase R3).** The Bevy `Transform` write is **not** done by `write_resolved_layout` (which still writes only `ResolvedLayout` — position + size). It shipped one stage later, in the render-prep bridge: `write_buiy_transform` (`render/bridge.rs`) folds `ResolvedLayout.position` + `ScrollOffset` + `ResolvedTransform.matrix` into the entity's Bevy `Transform`; Bevy's propagation then derives `GlobalTransform`, and render's `extract` reads `GlobalTransform` for position (no longer `ResolvedLayout` directly). The composition and coordinate contract are pinned in [render/clip-and-transform.md § B](../2026-06-03-buiy-render-pipeline-design/clip-and-transform.md#b-the-transform--globaltransform-bridge). This is approach (a) realized: layout produces `ResolvedLayout` + `ResolvedTransform`, the render bridge owns the single `Transform` write so it can also fold scroll translation in the same pass.

**Approach (b) — alternative.** Keep writing the composed 4×4 straight into `GlobalTransform`, but explicitly order the Buiy write *after* `TransformSystems::Propagate` (e.g. in `PostUpdate` after the transform set) so the recomposition cannot clobber it. Rejected as the primary path because it requires Buiy entities to opt out of normal `Transform`-based propagation for their descendants and complicates composition with author/gameplay transforms; (a) is cleaner.

Under (a), a Buiy entity with `Position.kind = PositionKind::Static` and `UiTransform { matrix: TransformMatrix::None, .. }` ends up with `GlobalTransform = parent.GlobalTransform * translation_to_resolved_position` after propagation. Authors don't need to touch Bevy's `Transform` to position UI; they touch Buiy's `Position` / `UiTransform` instead, and Buiy owns the resulting Bevy `Transform`.

For 3D-anchored UI (`buiy-3d-anchored-ui-design`), the worldspace transform feeds back into Buiy's layout root sizing via that spec's render-target contract.

## 3. Stacking-context formation

A non-identity `UiTransform` forms a stacking context. Specifically:

- `TransformMatrix::None`, `Scale(1,1,1)`, and zero `Translate`/`Rotate` → no stacking context; any non-identity transform forms one.
- *Any* non-identity transform → forms one. (CSS-faithful: the spec is `transform: none` doesn't form, anything else does.)

This is one of the bullets in [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation). Detection happens during the stacking-context sub-pass **6f** (landed Phase 9), which runs *after* the transform-composition sub-pass 6e ([§ 1.1](#11-longhand-components)) so it sees the composed matrix.

## 4. `Perspective` and 3D

```rust
pub perspective: Option<Length>
```

Sets the 3D viewing distance for child elements with `Preserve3d`. Render-side concern; layout stores the value.

`TransformStyle::Preserve3d` means children's transforms are composed in 3D rather than flattened. Without `Preserve3d`, each child renders as a 2D layer (faster, but no 3D effects across siblings).

`BackfaceVisibility::Hidden` hides the entity when its rotated normal faces away from the viewer. Render-side; layout stores.

## 5. Containment

`contain` and `content-visibility` are tier-C ([visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout)); `will_change` is tier-E (see [§ 5.3](#53-will-change)). CSS Containment Module Level 3.

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Containment {
    pub contain: ContainFlags,
    pub content_visibility: ContentVisibility,
    pub will_change: WillChange,            // tier-E — see § 5.3
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub struct ContainFlags: u8 {
        const LAYOUT     = 1 << 0;       // descendants don't affect ancestor layout
        const PAINT      = 1 << 1;       // descendants are clipped to box; opacity etc. doesn't bleed
        const SIZE       = 1 << 2;       // entity's own size is independent of descendants (must declare a size)
        const STYLE      = 1 << 3;       // counter-resets and certain style properties don't escape
        const INLINE_SIZE= 1 << 4;       // size containment for inline axis only
        // Shorthands are unions of the primitive bits (not standalone bits), so
        // `.contains(PAINT)` is true for a CONTENT- or STRICT-contained entity.
        const CONTENT    = Self::LAYOUT.bits() | Self::PAINT.bits() | Self::STYLE.bits();
        const STRICT     = Self::LAYOUT.bits() | Self::PAINT.bits() | Self::SIZE.bits() | Self::STYLE.bits();
    }
}

// `bitflags!` doesn't compose with `#[derive(Reflect)]` — register the
// opaque type manually (`impl_reflect_value!` was renamed to
// `impl_reflect_opaque!` in bevy_reflect 0.18).
impl_reflect_opaque!(ContainFlags(Default, PartialEq));

#[derive(Reflect, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContentVisibility {
    #[default]
    Visible,                            // default
    Auto,                               // skips paint when off-screen, content participates in layout for size
    Hidden,                             // skips paint AND layout for descendants (treated as `Display::None` for layout)
}

#[derive(Reflect, Clone, Default, PartialEq)]
pub enum WillChange {
    #[default]
    Auto,
    Properties(Vec<WillChangeProperty>), // hint: optimizer should expect these to change
}
```

### 5.1 Effect of `contain`

| Flag | Effect on layout |
|---|---|
| `LAYOUT` | The entity's content does not affect ancestor layout (Taffy already gets close to this for block formatting contexts; Buiy uses the flag to opt into the strict version). |
| `PAINT` | The entity establishes a clip rect at its border box; descendants don't paint outside. Render-side primarily; layout records. |
| `SIZE` | The entity's size *must be* explicit (no intrinsic sizing from descendants). If size containment is enabled and width/height are `Sizing::Auto`, treat as `Sizing::Length(Length::px(0.0))` and `warn!`. |
| `STYLE` | Counter resets + similar style scopes don't escape. Mostly render-side. |
| `INLINE_SIZE` | Inline-axis variant of `SIZE`. |
| `CONTENT` | Shorthand for `LAYOUT \| PAINT \| STYLE`. |
| `STRICT` | Shorthand for `LAYOUT \| PAINT \| SIZE \| STYLE`. |

Containment is a *performance opt-in*. An entity with `Containment::contain = ContainFlags::CONTENT` lets the engine skip recomputing its descendants when properties outside the container change. Buiy honors this for change-detection scope: a Bevy `Changed<X>` query on an entity inside a `CONTENT`-contained subtree doesn't invalidate the container's siblings.

### 5.2 `content-visibility: auto` / `hidden`

The big perf win. **Shipped in `Phase 11`** ([`plans/2026-05-29-buiy-layout-content-visibility.md`](../../plans/2026-05-29-buiy-layout-content-visibility.md)). A `ContentVisibility::Auto` entity:

- *Skips Taffy compute* on its descendants when both off-screen AND its `contain-intrinsic-size` (an opt-in size hint, the `ContainIntrinsicSize` component) is set. Without `contain-intrinsic-size`, the engine would have to lay out to determine size — defeats the purpose — so without the hint the entity lays out normally (the layout skip does not run; it warns once instead, see below).
- Snaps back to full layout when the entity comes on-screen.

The skip runs during step 1 (`sync_styles`): a pure `content_visibility_skip(...)` helper classifies each entity from its `Containment.content_visibility`, its optional `ContainIntrinsicSize`, and an off-screen test. "Off-screen" is the entity's **last-frame** `ResolvedLayout` border box failing to intersect the primary-window viewport expanded outward by a `ContentVisibilityMargin` (default 200 logical px) on all sides — a single symmetric expanded rect, so the margin doubles as a stateless hysteresis dead-band that prevents per-frame skip-state thrash at the edge. Reading only last-frame geometry keeps the "layout writes, render reads" no-re-entrancy contract (same bounded feedback edge as `Length::Cq*`). An entity with no last-frame `ResolvedLayout` (first frame) is treated as on-screen.

When the Auto skip fires, the entity's own Taffy node receives the `ContainIntrinsicSize` hint as a sentinel size (per-axis, unset axis → `0`), and its descendants are detached from the Taffy child list (their nodes are kept alive in the `LayoutTree`, so snap-back is a cheap `set_children` re-attach rather than a subtree rebuild). The skip is computed identically in `sync_styles` and `cq_flip_rerun`, so a container-query flip frame does not transiently re-lay-out the skipped subtree.

`ContentVisibility::Hidden` is harsher — its descendants are skipped exactly like `Display::None`, geometry-independent (no off-screen test, no hint needed), and it doesn't snap back unless toggled. Per CSS, only the *descendants* are skipped: the Hidden entity itself still lays out and resolves its own box.

**Still deferred (render-side / v1 limitation):**

- **Auto's off-screen *paint* skip.** Phase 11 owns layout, not paint. An off-screen Auto entity *without* a `contain-intrinsic-size` hint lays out normally; suppressing its paint while off-screen is a render-pipeline concern (see [§ 8](#8-coordination)) and is not yet wired. When the Auto layout skip cannot run because the hint is missing, the engine warns once per entity (`LayoutWarnOnceKey::ContentVisibilityDeferred`, repurposed from the old blanket deferral warn) to surface the actionable fix ("ship `contain-intrinsic-size` first").
- **`contain-intrinsic-size: auto` (remembered size).** The browser behavior where a once-laid-out skipped subtree caches its measured size as the placeholder is deferred; v1 gates the skip on an *explicit* `ContainIntrinsicSize` hint only.

### 5.3 `will-change`

`will-change` is foundation tier-E ([visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout)) — v1 ships the `WillChange` API surface for forward compatibility. The **SC-forming behavior is realized** (layout reads `will-change` as a stacking-context trigger, below); the **layer-promotion hint remains deferred** until a composition-layer concept exists in render/.

Hint to the optimizer. Layout uses it as a stacking-context trigger when its property list mentions an SC-forming property (e.g. `WillChangeProperty::Transform`): `forms_stacking_context` forms an SC when `Containment.will_change` names a property in the SC-forming subset. (The render-side use — promoting the entity to its own composition layer — is the deferred half; there is no `RenderLayers`/composition-layer mechanism yet.)

```rust
pub enum WillChangeProperty {
    Transform, Opacity, Filter, ZIndex, ScrollPosition, /* ... */
}
```

The SC-forming subset (CSS: a property named in `will-change` forms an SC iff it would at a non-initial value) is `Transform` / `Opacity` / `Filter`, encoded once in `WillChangeProperty::forms_stacking_context`. `ZIndex` and `ScrollPosition` are excluded (`will-change: z-index` does not form an SC — z-index needs positioning).

Authors should use sparingly — `will-change` consumes memory by promoting layers eagerly.

## 6. Stacking-context formation triggers (full list)

Consolidating from this file and [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation), the canonical layout-side trigger list:

1. `Position.kind` is non-`PositionKind::Static` AND `z_index = Layer(_)`.
2. `Stacking::isolation = Isolate`.
3. `UiTransform` non-identity (this file).
4. `Containment::contain` includes `ContainFlags::PAINT` or `ContainFlags::STRICT` (this file).
5. `Containment::will_change` lists an SC-forming property (this file) — realized (see [§ 5.3](#53-will-change); subset = Transform/Opacity/Filter). Only the layer-promotion hint stays deferred.
6. `TopLayer != None` (handled separately — top layer escapes the stacking system entirely).
7. Render-side triggers (`opacity < 1`, `filter != none`, `mix_blend_mode != normal`) — checked here for completeness; the components live in render-spec.
8. The root entity always forms a stacking context (matches [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation) trigger 6).

## 7. Test surface

- **Identity transform** — `UiTransform { matrix: TransformMatrix::None, .. }` produces no stacking context, no `ResolvedTransform`.
- **Non-identity transform forms SC** — `UiTransform { matrix: TransformMatrix::Rotate(Quat::from_rotation_z(0.1)), .. default() }` produces a `StackingContext`.
- **Transform doesn't affect layout flow** — fixture flex row with three children, middle child rotated 45°; assert siblings' positions match the un-rotated case.
- **`UiTransform` composes into Bevy `Transform`** — fixture with a Bevy parent transform + Buiy child `UiTransform`; assert the child's Bevy `Transform` (and resulting `GlobalTransform` after propagation) is the composition (approach (a), [§ 2](#2-mapping-to-bevy-transform)).
- **Longhand `translate` composes with `UiTransform.matrix`** — fixture with both; assert composed matrix matches CSS spec order.
- **`contain: Size` with `Sizing::Auto` warns and zeros** — fixture asserts the `warn!` and the sized-zero box.
- **`content-visibility: auto` skips off-screen** — fixture with a tall scroll container, off-screen child has `ContentVisibility::Auto`; assert child is not in step 1's translation set when off-screen.
- **`will-change: transform` forms SC even with identity transform** — assert SC formed.

## 8. Coordination

- **`buiy-render-pipeline-design`** — owns the actual paint of `UiTransform`, `Containment` clip rects, `will-change` layer promotion. Reads `ResolvedTransform`, the SC list, and the containment flags.
- **`buiy-animation-design`** — owns interpolation of `UiTransform.matrix`, `Translate`/`Rotate`/`Scale` longhands. This spec defines their typed-value shape; animation tweens between values.
- **`buiy-input-events-design`** — applies inverse `ResolvedTransform` to pointer coordinates for transformed-element hit-testing.
