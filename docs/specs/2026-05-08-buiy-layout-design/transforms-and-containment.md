# Transforms and containment

**Parent:** [README.md](README.md)

How an entity's box is visually transformed without affecting layout flow (`Transform`), and how layout/paint/size containment lets the engine skip work for off-screen or stable subtrees (`Containment`).

## 1. `Transform`

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Transform {
    pub matrix: TransformMatrix,
    pub origin: TransformOrigin,
    pub style:  TransformStyle,         // Flat | Preserve3d
    pub perspective: Option<Length>,
    pub backface_visibility: BackfaceVisibility,
}

pub enum TransformMatrix {
    None,                                                   // identity
    Translate(Length, Length, Length),                      // 3D translate
    Rotate(Quat),                                           // arbitrary 3D rotation
    Scale(f32, f32, f32),
    Skew(f32, f32),                                         // x, y in radians
    Matrix(Mat4),                                           // explicit 4×4
    Compose(Vec<TransformMatrix>),                          // applied right-to-left like CSS
}

pub struct TransformOrigin { pub x: Length, pub y: Length, pub z: Length }
pub enum TransformStyle { Flat, Preserve3d }
pub enum BackfaceVisibility { Visible, Hidden }
```

Defaults:

- `matrix` = `TransformMatrix::None`
- `origin` = `TransformOrigin { x: Length::Percent(50.0), y: Length::Percent(50.0), z: Length::ZERO }` (CSS default `50% 50% 0`)
- `style` = `Flat`
- `perspective` = `None`
- `backface_visibility` = `Visible`

### 1.1 Longhand components

CSS exposes `translate`, `rotate`, `scale` as separate properties applied independently of `transform`. Buiy mirrors:

```rust
#[derive(Component, Reflect, Clone, Default)]
pub struct Translate(pub Length, pub Length, pub Length);

#[derive(Component, Reflect, Clone, Default)]
pub struct Rotate(pub Quat);

#[derive(Component, Reflect, Clone, Default)]
pub struct Scale(pub f32, pub f32, pub f32);
```

When present, these compose with `Transform.matrix` in CSS order: `translate → rotate → scale → transform.matrix`. The composition runs in step 7 (`WriteResolvedLayout`); the composed matrix is written to a private `ResolvedTransform` component for render.

### 1.2 Layout impact

`Transform` does **not** affect Taffy compute. A transformed element occupies its un-transformed box for layout purposes; siblings ignore the transform.

Exceptions where transforms *do* affect layout:

- Stacking-context formation ([§ 3](#3-stacking-context-formation)).
- The transformed entity itself becomes a containing block for `Position::Fixed` descendants (CSS quirk).
- `transform-origin` and the resolved transform are read by `buiy-input-events-design`'s hit-test pass for transformed elements — picking is done in the entity's *transformed* space.

## 2. Mapping to Bevy `Transform`

Bevy already has a `bevy::prelude::Transform` and a `GlobalTransform`. Buiy's layout pipeline writes a private `ResolvedTransform` (containing the layout-derived 4×4) and *adds* it to Bevy's `GlobalTransform` during step 7. Author-set Bevy `Transform` (e.g., a parent's gameplay transform) composes naturally.

This means: a Buiy entity with `Position::Static` and `Transform::None` ends up with `GlobalTransform = parent.GlobalTransform * translation_to_resolved_position`. Authors don't need to touch Bevy's `Transform` to position UI; they touch Buiy's `Position` / `Transform` instead.

For 3D-anchored UI (`buiy-3d-anchored-ui-design`), the worldspace transform feeds back into Buiy's layout root sizing via that spec's render-target contract.

## 3. Stacking-context formation

A non-identity `Transform` forms a stacking context. Specifically:

- `TransformMatrix::None` and zero longhand `Translate`/`Rotate`/`Scale` → no stacking context.
- *Any* non-identity transform → forms one. (CSS-faithful: the spec is `transform: none` doesn't form, anything else does.)

This is one of the bullets in [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation). Detection happens during the stacking-context sub-pass.

## 4. `Perspective` and 3D

```rust
pub perspective: Option<Length>
```

Sets the 3D viewing distance for child elements with `Preserve3d`. Render-side concern; layout stores the value.

`TransformStyle::Preserve3d` means children's transforms are composed in 3D rather than flattened. Without `Preserve3d`, each child renders as a 2D layer (faster, but no 3D effects across siblings).

`BackfaceVisibility::Hidden` hides the entity when its rotated normal faces away from the viewer. Render-side; layout stores.

## 5. Containment

Tier-C. CSS Containment Module Level 3.

```rust
#[derive(Component, Reflect, Clone, Default)]
#[reflect(Component, Default)]
pub struct Containment {
    pub contain: ContainFlags,
    pub content_visibility: ContentVisibility,
    pub will_change: WillChange,
}

bitflags::bitflags! {
    #[derive(Reflect, Clone, Copy, Default)]
    pub struct ContainFlags: u8 {
        const LAYOUT     = 1 << 0;       // descendants don't affect ancestor layout
        const PAINT      = 1 << 1;       // descendants are clipped to box; opacity etc. doesn't bleed
        const SIZE       = 1 << 2;       // entity's own size is independent of descendants (must declare a size)
        const STYLE      = 1 << 3;       // counter-resets and certain style properties don't escape
        const INLINE_SIZE= 1 << 4;       // size containment for inline axis only
        const CONTENT    = 1 << 5;       // shorthand: LAYOUT | PAINT | STYLE
        const STRICT     = 1 << 6;       // shorthand: LAYOUT | PAINT | SIZE | STYLE
    }
}

pub enum ContentVisibility {
    Visible,                            // default
    Auto,                               // skips paint when off-screen, content participates in layout for size
    Hidden,                             // skips paint AND layout for descendants (treated as `Display::None` for layout)
}

pub enum WillChange {
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

Hint to the optimizer. Render uses it to promote the entity to its own composition layer. Layout uses it as a stacking-context trigger when its property list mentions an SC-forming property (e.g. `WillChangeProperty::Transform`).

```rust
pub enum WillChangeProperty {
    Transform, Opacity, Filter, ZIndex, ScrollPosition, /* ... */
}
```

Authors should use sparingly — `will-change` consumes memory by promoting layers eagerly.

## 6. Stacking-context formation triggers (full list)

Consolidating from this file and [stacking-and-top-layer.md § 2](stacking-and-top-layer.md#2-stacking-context-formation), the canonical layout-side trigger list:

1. Position is non-`Static` AND `z_index = Layer(_)`.
2. `Stacking::isolation = Isolate`.
3. `Transform` non-identity (this file).
4. `Containment::contain` includes `Paint` or `Strict` (this file).
5. `Containment::will_change` lists an SC-forming property (this file).
6. `TopLayer != None` (handled separately — top layer escapes the stacking system entirely).
7. Render-side triggers (`opacity < 1`, `filter != none`, `mix_blend_mode != normal`) — checked here for completeness; the components live in render-spec.

## 7. Test surface

- **Identity transform** — `Transform::None` produces no stacking context, no `ResolvedTransform`.
- **Non-identity transform forms SC** — `Transform { matrix: TransformMatrix::Rotate(Quat::from_rotation_z(0.1)), .. default() }` produces a `StackingContext`.
- **Transform doesn't affect layout flow** — fixture flex row with three children, middle child rotated 45°; assert siblings' positions match the un-rotated case.
- **Transform composes with Bevy `Transform`** — fixture with a Bevy parent transform + Buiy child transform; assert `GlobalTransform` is the composition.
- **Longhand `translate` composes with `Transform.matrix`** — fixture with both; assert composed matrix matches CSS spec order.
- **`contain: Size` with `Sizing::Auto` warns and zeros** — fixture asserts the `warn!` and the sized-zero box.
- **`content-visibility: auto` skips off-screen** — fixture with a tall scroll container, off-screen child has `ContentVisibility::Auto`; assert child is not in step 1's translation set when off-screen.
- **`will-change: transform` forms SC even with identity transform** — assert SC formed.

## 8. Coordination

- **`buiy-render-pipeline-design`** — owns the actual paint of `Transform`, `Containment` clip rects, `will-change` layer promotion. Reads `ResolvedTransform`, the SC list, and the containment flags.
- **`buiy-animation-design`** — owns interpolation of `Transform.matrix`, `Translate`/`Rotate`/`Scale` longhands. This spec defines their typed-value shape; animation tweens between values.
- **`buiy-input-events-design`** — applies inverse `ResolvedTransform` to pointer coordinates for transformed-element hit-testing.
