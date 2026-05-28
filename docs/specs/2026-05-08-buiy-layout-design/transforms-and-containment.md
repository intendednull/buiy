# Transforms and containment

**Parent:** [README.md](README.md)

How an entity's box is visually transformed without affecting layout flow (`UiTransform`), and how layout/paint/size containment lets the engine skip work for off-screen or stable subtrees (`Containment`).

## 1. `UiTransform`

The component is named `UiTransform`, **not** `Transform`: `bevy::prelude::Transform` is glob-imported across `buiy_core` (`layout/mod.rs:36` does `use bevy::prelude::*`), so a Buiy `Transform` component would collide with Bevy's. The longhands keep their CSS names (`Translate` / `Rotate` / `Scale`); Bevy 0.18 has no prelude components of those names, so they do not collide.

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

**Composition order (single convention for the whole spec).** The final transform matrix is

```
M = T_translate · R_rotate · S_scale · M_transform
```

a matrix product written left-to-right in outermost-to-innermost order: `T_translate` is the outermost factor and `M_transform` the innermost, so a child point `p` is transformed as `M · p` and therefore feels the *rightmost* (innermost) factor first. For `TransformMatrix::Compose([A, B, …])` the list is written outermost-first and applied as the matrix product `A · B · …` — same rule: the rightmost/innermost entry transforms a child point first. This is exactly the convention CSS uses for a `transform:` list written left-to-right. Phase 8 implements this product; there is no separate "right-to-left" rule to reconcile — "rightmost-applied-first" is just what reading the matrix product `M · p` means.

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

When present, these compose with `UiTransform.matrix` per the single convention in [§ 1](#1-transform): `M = T_translate · R_rotate · S_scale · M_transform` (the written order `translate → rotate → scale → transform.matrix` is outermost-to-innermost, so `M_transform` transforms a child point first). The composition runs as `PostTaffyOverrides` sub-pass **6e** (`Phase 8` — extending the shipped sub-pass chain: 6a sticky, 6b table, 6c multicol, 6d anchor; per [architecture.md § 3](architecture.md#3-system-pipeline)); the composed matrix is written to a private `ResolvedTransform` component for render and consumed by step 7 (`WriteResolvedLayout`). Transform-triggered stacking-context detection (`Phase 9`, [stacking-and-top-layer.md](stacking-and-top-layer.md)) runs as sub-pass **6f**, *after* 6e, since it reads the composed matrix produced by 6e.

### 1.2 Layout impact

`UiTransform` does **not** affect Taffy compute. A transformed element occupies its un-transformed box for layout purposes; siblings ignore the transform.

Exceptions where transforms *do* affect layout:

- Stacking-context formation ([§ 3](#3-stacking-context-formation)).
- The transformed entity itself becomes a containing block for `Position.kind = PositionKind::Fixed` descendants (CSS quirk).
- `transform-origin` and the resolved transform are read by `buiy-input-events-design`'s hit-test pass for transformed elements — picking is done in the entity's *transformed* space.

## 2. Mapping to Bevy `Transform`

Bevy already has a `bevy::prelude::Transform` and a `GlobalTransform`. The layout pipeline must cooperate with Bevy's transform ownership: `TransformSystems::Propagate` *recomposes `GlobalTransform` from `Transform` on any frame `Transform` is changed or added (and the subtree isn't statically optimized out)* — both the root path (`bevy_transform-0.18.1/src/systems.rs:201`, `*global_transform = GlobalTransform::from(*transform)`) and the child-propagation path (same file, `:376`, `*parent_transform = GlobalTransform::from(*transform)`) recompose via `GlobalTransform::from(*transform)`. Writing `GlobalTransform` directly would be clobbered by that propagation.

**Approach (a) — recommended (`Phase 8` target).** Step 7 (`WriteResolvedLayout`) composes the layout-derived position + the resolved `UiTransform`/longhand matrix (also exposed via the private `ResolvedTransform` component for render) and writes the result into the entity's Bevy `Transform`. Bevy's `TransformSystems::Propagate` then composes that `Transform` into `GlobalTransform` the normal way, so Buiy lives inside Bevy's ownership model instead of fighting it. Buiy owns the `Transform` of every entity it lays out.

> **Not yet in effect (as of Phases 0–7).** Today `write_resolved_layout` (`layout/systems.rs:1662`) writes **only** `ResolvedLayout` (position + size), never Bevy `Transform`; `ResolvedLayout` is consumed directly by render. The "Buiy owns the `Transform`" ownership story and the `Transform`/`ResolvedTransform` write described above land in `Phase 8` alongside transform-matrix composition (sub-pass 6e, [§ 1.1](#11-longhand-components)).

**Approach (b) — alternative.** Keep writing the composed 4×4 straight into `GlobalTransform`, but explicitly order the Buiy write *after* `TransformSystems::Propagate` (e.g. in `PostUpdate` after the transform set) so the recomposition cannot clobber it. Rejected as the primary path because it requires Buiy entities to opt out of normal `Transform`-based propagation for their descendants and complicates composition with author/gameplay transforms; (a) is cleaner.

Under (a), a Buiy entity with `Position.kind = PositionKind::Static` and `UiTransform { matrix: TransformMatrix::None, .. }` ends up with `GlobalTransform = parent.GlobalTransform * translation_to_resolved_position` after propagation. Authors don't need to touch Bevy's `Transform` to position UI; they touch Buiy's `Position` / `UiTransform` instead, and Buiy owns the resulting Bevy `Transform`.

For 3D-anchored UI (`buiy-3d-anchored-ui-design`), the worldspace transform feeds back into Buiy's layout root sizing via that spec's render-target contract.

## 3. Stacking-context formation

A non-identity `UiTransform` forms a stacking context. Specifically:

- `TransformMatrix::None`, `Scale(1,1,1)`, and zero `Translate`/`Rotate` → no stacking context; any non-identity transform forms one.
- *Any* non-identity transform → forms one. (CSS-faithful: the spec is `transform: none` doesn't form, anything else does.)

This is one of the bullets in [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation). Detection happens during the stacking-context sub-pass **6f** (`Phase 9`), which runs *after* the transform-composition sub-pass 6e ([§ 1.1](#11-longhand-components)) so it sees the composed matrix.

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

### 5.2 `content-visibility: auto`

The big perf win. A `ContentVisibility::Auto` entity:

- Skips paint when the entity is fully outside the viewport.
- *Skips Taffy compute* on its descendants when both off-screen AND its `contain-intrinsic-size` (an opt-in size hint) is set. Without `contain-intrinsic-size`, the engine has to lay out to determine size — defeats the purpose.
- Snaps back to full layout + paint when the entity comes on-screen.

The skip is implemented as: during step 1, check if the entity is `ContentVisibility::Auto` and currently off-screen (using last frame's `ResolvedLayout`); if so, mark the subtree for skip — Taffy receives a sentinel size and the descendants' style sync is no-op.

`ContentVisibility::Hidden` is harsher — equivalent to `Display::None` for descendants, doesn't snap back unless toggled.

### 5.3 `will-change`

`will-change` is foundation tier-E ([visuals.md § 3.2](../2026-05-07-buiy-foundation/visuals.md#32-layout)) — v1 ships the `WillChange` API surface for forward compatibility, but the layer-promotion hint and the SC-forming behavior below are deferred until user demand. Prioritization waits on user demand.

Hint to the optimizer. Render uses it to promote the entity to its own composition layer. Layout uses it as a stacking-context trigger when its property list mentions an SC-forming property (e.g. `WillChangeProperty::Transform`).

```rust
pub enum WillChangeProperty {
    Transform, Opacity, Filter, ZIndex, ScrollPosition, /* ... */
}
```

Authors should use sparingly — `will-change` consumes memory by promoting layers eagerly.

## 6. Stacking-context formation triggers (full list)

Consolidating from this file and [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation), the canonical layout-side trigger list:

1. `Position.kind` is non-`PositionKind::Static` AND `z_index = Layer(_)`.
2. `Stacking::isolation = Isolate`.
3. `UiTransform` non-identity (this file).
4. `Containment::contain` includes `ContainFlags::PAINT` or `ContainFlags::STRICT` (this file).
5. `Containment::will_change` lists an SC-forming property (this file) — tier-E, deferred (see [§ 5.3](#53-will-change)).
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
