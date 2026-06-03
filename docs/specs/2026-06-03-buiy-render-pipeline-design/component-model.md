# Buiy render — component model

**Parent:** [README.md](README.md)

This file defines every render-owned component on the layout↔render boundary —
the typed surface that replaces the temporary `Visual` and the seam the layout
subsystem reaches back across for stacking-context trigger 5. The names and
ownership are fixed by [README § 3.2](README.md#32-render-owned-this-spec-introduces);
this file elaborates each component's fields, tier, and the reasoning that
shapes its shape. Field-level clip *geometry* lives in
[clip-and-transform.md](clip-and-transform.md); atlas sampling for tokenized
fills and glyph alpha lives in
[atlas-and-text-seam.md](atlas-and-text-seam.md); how the paint passes *consume*
these components is [paint-order-and-top-layer.md](paint-order-and-top-layer.md)
and [effect-compositor.md](effect-compositor.md).

These components follow the foundation's decomposed-component convention
(foundation goal §1.3): each is a small public-fielded `Component` deriving
`Reflect + Default + Clone + Component`, registered in the render plugin's
`build` so reflection / BSN / inspectors find them. They are the render-side
analogue of the layout decomposed components in
[`layout/components.rs`](../../../crates/buiy_core/src/layout/components.rs) and
mirror the layout-spec authoring model
([layout/architecture.md § 2.1](../2026-05-08-buiy-layout-design/architecture.md#21-decomposed-components--canonical-storage)).

## 1. The decomposed-component convention (why this shape)

Every component below is the render twin of the layout-side rule the foundation
already commits to: **one property family per component, all fields public, no
hidden builder state, every field reflectable.** This is the same rule that
governs `BoxModel` / `Display` / `Position`
([box-model.md § 2](../2026-05-08-buiy-layout-design/box-model.md#2-boxmodel)),
and it exists for the same three reasons:

- **A `Style`-builder analogue can expand into them.** Just as the layout
  `Style` builder
  ([layout/architecture.md § 2.2](../2026-05-08-buiy-layout-design/architecture.md#22-style--ergonomic-authoring-layer))
  is a `Bundle`-producing convenience over public-fielded layout components, a
  render-side authoring surface (`Appearance`, defined alongside `Style` in the
  authoring crate — out of scope here) expands into `Background` / `Border` /
  `BoxShadow` / `Outline` / `Opacity`. This file defines the *components*, not
  the builder; the builder is a Rust ergonomic that writes the same fields.
- **BSN references them by name.** BSN files (the portable serialization layer,
  per
  [layout/architecture.md § 2.5](../2026-05-08-buiy-layout-design/architecture.md#25-bsn-authoring))
  write `Background { .. }`, not the builder. Reflection sees the struct fields;
  that is the schema BSN and inspectors bind to.
- **Change detection is the damage signal.** Render's `Extract<Query>` is gated
  on `Changed<Background>`, `Changed<Border>`, etc. (pillar 3). Keeping one
  property family per component makes the change signal precise — mutating a
  background does not re-extract a border. This is the "damage tracking for
  free" the architecture relies on.

Components are inserted independently. An entity can carry `Background` without
`Border`; `Outline` without `Opacity`. A missing component means "render the
CSS-initial value of that property family" (transparent background, zero-width
border, no shadow, opacity 1.0, no outline) — never a panic, never a sentinel.

Where a field carries a `bevy::color::Color` (which contains `f32` and so is
`Reflect` but not `Eq`), the containing type derives `PartialEq` but **not**
`Eq`, matching the `ScrollbarColor` / `ColumnRule` convention already in
[`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs).

## 2. Color tokens vs. resolved colors

Render components store **color tokens**, not resolved `Color`s, wherever a
value is themeable — exactly as `Visual.background_token` did. A `ColorToken` is
a typed reference into `Res<Theme>` (a token name plus the `currentColor` /
`transparent` / forced-colors system-keyword cases); its resolution against the
active theme, the linear-light pipeline, and the forced-colors contract are
owned by [color-and-forced-colors.md](color-and-forced-colors.md). This file
treats `ColorToken` as an opaque value type and only states *which fields hold
one*.

```rust
/// A themeable color reference. Resolved against `Res<Theme>` at extract
/// time by the color pass; see color-and-forced-colors.md. Carries the
/// `currentColor` / `transparent` / system-color-keyword cases so a render
/// component never stores a pre-resolved `Color` for a themeable slot.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum ColorToken { /* defined in color-and-forced-colors.md */ }
```

Non-themeable literal colors (rare; e.g. a debug overlay) may store a raw
`bevy::color::Color` directly, but the F-tier authoring path is always a token.

## 3. `Background` — F

Replaces `Visual.background_token`. v1 ships a single solid color token; the
layered / gradient surface (foundation §3.3 *Backgrounds*, all **C**) is
reserved as fields that default to "absent" so the v1 → C migration adds
behavior without changing the component's identity.

```rust
/// Solid background fill (v1) with reserved layered/gradient fields (C).
/// Replaces `Visual.background_token`. Absent component == transparent.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 3.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Background {
    /// F: solid fill. `ColorToken::default()` (transparent) == no fill,
    /// matching `Visual.background_token == ""`.
    pub color: ColorToken,

    /// C (reserved): ordered background layers (gradients, images,
    /// multiple stacked fills) painted over `color`, bottom-to-top.
    /// Empty in v1; `BackgroundLayer` is a reserved type the gradient /
    /// image fast-follow defines. Gradient stops sample the atlas
    /// (atlas-and-text-seam.md).
    pub layers: Vec<BackgroundLayer>,
}
```

`color` resolving to transparent makes the entity background-free without
removing the component — the render extract skips emitting a quad primitive for
a transparent fill, preserving `Visual`'s "empty string → skip the fill"
behavior. The painted shape is the **border box** from `ResolvedLayout`, inset
to the padding/border edges per `Border` (the `background-clip` C-tier knob that
chooses which box to fill lives in the reserved `layers` work).

## 4. `Border` — F

Replaces `Visual.border_radius` and absorbs the full F-tier border surface
(foundation §3.3 *Borders*: per-side width/style/color longhands, logical, plus
elliptical per-corner radius). Border *width* is a layout input owned by
`BoxModel.border`
([box-model.md § 2](../2026-05-08-buiy-layout-design/box-model.md#2-boxmodel)) —
it participates in box sizing — so `Border` here is the **paint** description:
how the already-reserved border band is stroked, and the corner radii that round
both the border and the background fill.

```rust
/// Per-side border paint + elliptical per-corner radius. Replaces
/// `Visual.border_radius`. The border *band's* thickness is the layout
/// input `BoxModel.border`; this component paints into that band.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Border {
    /// Per-side paint: color token + line style, one per physical side.
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
    /// Elliptical per-corner radius. `Corners::ZERO` (default) == square
    /// corners, matching `Visual.border_radius == 0.0`. A uniform radius
    /// is `Corners::all(Radius::circular(px))`.
    pub radius: Corners,
}

/// One side's paint description. Width is NOT here — it lives in
/// `BoxModel.border` because it affects layout.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct BorderSide {
    pub color: ColorToken,
    pub style: LineStyle, // None | Solid | Dashed | Dotted | Double (...)
}

/// Elliptical per-corner radius (x and y radii per corner).
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Corners {
    pub top_left: Radius,
    pub top_right: Radius,
    pub bottom_right: Radius,
    pub bottom_left: Radius,
}

/// One corner's elliptical radii (CSS `border-radius` allows `rx / ry`).
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Radius {
    pub x: Length,
    pub y: Length,
}
```

`Length` is the layout unit type from
[`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs) (`Px` /
`Percent` / the container-query family ship today; `Em` / viewport / `Calc`
arrive with Phase 10). Percentage radii resolve against the border box per CSS;
the resolution timing matches the unit-resolution contract in
[box-model.md § 5.1](../2026-05-08-buiy-layout-design/box-model.md#51-resolution).

The radius is read by **both** render (to round the fill + border) and the clip
pass — a rounded border box clips its rounded-rect descendants. That coupling is
why the radius lives on `Border` (a render component) yet feeds `ClipRect`
geometry; the seam is described in
[clip-and-transform.md](clip-and-transform.md).

`LineStyle` reuses the shape of `ColumnRuleStyle`
([`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs)) — the
existing border-line-style enum — extended with the remaining CSS keywords
(`Groove`/`Ridge`/`Inset`/`Outset` are C-tier and render as `Solid` until the
bevel shader lands).

## 5. `BoxShadow` — F

Full F-tier box-shadow: an **ordered list** of shadows, each independently
inset/outset, with spread, blur, offset, and color (foundation §3.3
*Shadows…*: "`box-shadow`, multiple, inset, spread" is **F**). Painted by a
dedicated shadow primitive (architecture pillar 2's `shadow` typed primitive),
behind the background for outset shadows and above the fill (clipped to the
padding box) for inset shadows.

```rust
/// Ordered box-shadow list. Index 0 paints on top (CSS paint order:
/// first shadow is frontmost). Empty / absent == no shadow.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 5.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BoxShadow(pub Vec<Shadow>);

#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Shadow {
    pub color: ColorToken,
    pub offset_x: Length,
    pub offset_y: Length,
    pub blur: Length,   // CSS blur radius (>= 0)
    pub spread: Length, // grows (+) / shrinks (-) the shadow shape
    pub inset: bool,    // false = outset (default), true = inner shadow
}
```

The shadow shape is the border box expanded by `spread` and rounded by
`Border.radius`, blurred by a Gaussian of `blur`. The blur is a per-shadow SDF
parameter on the shadow primitive — no off-screen pass and no atlas entry is
needed for rectangular/rounded-rect shadows (the common case); arbitrary
`clip-path` shadow shapes are C-tier and defer with `clip-path`.

## 6. `Opacity` — F (and an `EffectGroup` former)

```rust
/// Group opacity in `[0.0, 1.0]`. `1.0` (default) is a no-op. A value
/// `< 1.0` forms an `EffectGroup` (off-screen composite boundary) AND is
/// a stacking-context trigger that layout sub-pass 6f reads back
/// (pillar 6). Absent == opaque.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 6.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Opacity(pub f32);

impl Default for Opacity {
    fn default() -> Self { Opacity(1.0) }
}
```

`Opacity` is the one v1 effect that is *both* fully shipped paint **and** a
compositing boundary. When `0.0 ≤ value < 1.0` the entity is a
**group-opacity** case: its whole subtree is composited to an off-screen target
and that target is blended at `value`, so overlapping children don't
double-blend (the correctness point the user insisted on — pillar 6, runner-up
"forward-pass approximation" was rejected). Forming the boundary is the job of
[effect-compositor.md](effect-compositor.md); this component is the trigger.

`Opacity < 1` participates in stacking-context formation. Because layout sub-pass
6f (`stacking_context`) is the SC former and lives across the layout↔render
boundary, the trigger components it reads must be visible to layout — the
crate-placement constraint in
[README § 3.2](README.md#32-render-owned-this-spec-introduces) and pinned by
[architecture.md](architecture.md). `Opacity` is in the SC-trigger union
alongside `Filter` / `MixBlendMode` (below) and the layout-owned `Stacking`
(`isolation`) and `UiTransform`.

> **Distinct from `visibility` and `Display::None`.** `opacity: 0` still
> participates in layout, paint traversal, and hit-testing (it forms an
> invisible-but-present group); `Display::None` (layout-owned) skips everything;
> `visibility: hidden` (foundation §3.3, **F**, render-owned, not yet a
> component here — see [§ 11 open](#11-reserved-vs-deferred-and-open-items))
> paints nothing but keeps layout + a11y. They are three different mechanisms.

## 7. `Outline` — F

```rust
/// Focus / selection outline, painted OUTSIDE the border box and never
/// clipped by the element's own `ClipRect`. Absent == no outline.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 7.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Outline {
    pub color: ColorToken,
    pub style: LineStyle,
    pub width: Length,
    /// Gap between the border box and the outline. Positive pushes the
    /// outline outward (CSS `outline-offset`); negative draws it inset.
    pub offset: Length,
}
```

Two properties make `Outline` its own component rather than a `Border` field, in
faithful CSS semantics:

1. **It does not affect layout** — an outline never changes box geometry
   (`BoxModel.border` does), so it has no layout-side counterpart and lives
   purely render-side.
2. **It is not clipped by the element's own clip.** A focus ring on a
   `overflow: hidden` element must remain visible. The render pass paints
   `Outline` using the entity's *ancestor* `ClipRect` (the clip it inherits),
   **not** the entity's own `ClipRect` — the one place a render component
   deliberately reads a different clip than the box it decorates. This rule is
   stated here and enforced by the clip pass
   ([clip-and-transform.md](clip-and-transform.md)); the
   gate-#10 hit-target / gate-#2 visual-regression fixtures assert a focus ring
   survives an `overflow: hidden` ancestor.

The outline shape is the border box expanded by `width + offset`, rounded by
`Border.radius` grown by the same amount (CSS-faithful outline corner rounding).

## 8. The reserved effect components — C (ship now, shaders deferred)

`Filter`, `BackdropFilter`, and `MixBlendMode` are **C-tier** (foundation §3.3:
`filter`, `backdrop-filter`, `mix-blend-mode` are all **C**) — their *shaders*
are deferred. But their **components ship in v1**, for one decisive reason:

> **They unblock the layout 6f SC-former follow-up.** The render-gated layout
> follow-up "Render-side SC formers (`opacity`/`filter`/`mix-blend-mode`)"
> ([README § 4](README.md#4-what-this-spec-unblocks) item 2) needs layout
> sub-pass 6f to read the trigger-5 components. 6f cannot gain its trigger-5
> clause against components that do not exist. Shipping the components now —
> even with no shader — lets layout wire the *complete* SC-trigger union in one
> change, so the C-tier shader fast-follow is purely additive (a render pass,
> no layout edit). Deferring the components would force layout to re-open 6f
> later, exactly the relitigation the immutable-output contract forbids.

So these three ship as registered, reflectable, public-fielded components that
participate in **two** v1 behaviors — SC-trigger union (read by layout 6f) and
`EffectGroup` formation (read by the compositor to allocate an off-screen
target, [effect-compositor.md](effect-compositor.md)) — and contribute **no**
pixels until their shader lands. An entity carrying `Filter([Blur(4px)])` in v1
forms a stacking context and an effect group; the group composites with an
identity pass (the reserved seam) and the blur is a no-op until the filter
shader ships. The deferral is a missing shader, never a missing trigger.

```rust
/// C (reserved). Filter function list. SC-trigger + EffectGroup former in
/// v1; the filter shaders (blur/brightness/.../drop-shadow) are deferred
/// (foundation §3.3). A non-empty list forms a stacking context (read by
/// layout 6f) and an `EffectGroup`. Empty / absent == no filter.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Filter(pub Vec<FilterFn>);

/// C (reserved). Backdrop filter list — samples what is BEHIND the
/// element. Component + the off-screen boundary ship v1; the
/// backdrop-sampling shader is deferred (foundation §3.3 "Backdrop
/// sampling for `backdrop-filter`"). Forms an `EffectGroup` (the
/// compositor must hold a backdrop copy) but is NOT a stacking-context
/// trigger on its own in CSS — see note below.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BackdropFilter(pub Vec<FilterFn>);

/// C (reserved). Blend mode against the backdrop. Any value other than
/// `Normal` forms a stacking context (read by layout 6f) and an
/// `EffectGroup`; the blend shader is deferred (foundation §3.3).
/// `Normal` (default) is a no-op.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub enum MixBlendMode {
    #[default]
    Normal,
    Multiply, Screen, Overlay, Darken, Lighten,
    ColorDodge, ColorBurn, HardLight, SoftLight,
    Difference, Exclusion, Hue, Saturation, Color, Luminosity,
    // CSS `mix-blend-mode` full set; all render as `Normal` until the
    // blend shader lands.
}

/// Reserved filter-function value. Shapes (blur radius, percentages, the
/// drop-shadow tuple) ship now so authors can write filter-aware code
/// that compiles against v1; evaluation is deferred.
#[derive(Reflect, Clone, PartialEq, Debug)]
pub enum FilterFn {
    Blur(Length),
    Brightness(f32), Contrast(f32), Grayscale(f32),
    Invert(f32), Opacity(f32), Saturate(f32), Sepia(f32),
    HueRotate(Angle),
    DropShadow(Shadow),
}
```

**`MixBlendMode` is a stacking-context trigger; `BackdropFilter` is not** — this
matches CSS (a non-`Normal` blend mode and a non-empty `filter` form stacking
contexts; `backdrop-filter` forms an `EffectGroup`/containing block but the
trigger-5 union layout 6f reads is the SC-forming set: `opacity < 1`, non-empty
`filter`, non-`Normal` `mix-blend-mode`, `isolation: isolate`). The exact
trigger predicate is owned by layout's
[stacking-and-top-layer.md](../2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
`forms_stacking_context`; this file fixes *which render components feed it*.
`will-change` of an SC-forming property is the fifth wedge (README § 4 item 3,
the `WillChange` hint already in
[`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs)); a render-side
`WillChange` layer-promotion hint and layout's reading of the
SC-forming-property clause land as the one paired change README § 4 item 3
describes.

`Angle` (for `HueRotate`) is a reserved angle type (foundation §3.3 *Units*:
angles are **C**); it is named here and defined by the units fast-follow, not
this file.

## 9. `ClipRect` — F (computed, not authored)

`ClipRect` is the **only** render component on this list that is *not*
author-set. It is written by the `WriteClipRects` render-prep pass (pillar 4)
from layout-owned inputs (`Overflow`, `ScrollOffset`, `Containment` PAINT,
`Border.radius`, the box) and read by **both** render (to clip primitives) and
picking (so a hit-test cannot fall outside the visible region — the
ordering/clip identity README § 1 guards). Because it is computed by a pass that
reads only layout output, it stays testable under the layout-snapshot gate (#5)
without a GPU.

```rust
/// Per-entity computed clip region. Written by `WriteClipRects`
/// (render-prep), read by render and picking. NOT author-set. Absent ==
/// unclipped (inherits the ancestor clip only).
///
/// The stored rect is the intersection of this entity's own clip box
/// (overflow/containment-paint, offset by scroll) with every ancestor
/// clip — so a single component read gives the final scissor region. The
/// rounded-corner and non-rectangular (`clip-path`, C) cases live in the
/// `shape` field; the geometry algorithm is in clip-and-transform.md.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 9.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct ClipRect {
    /// Axis-aligned clip rectangle (logical px, window-relative — same
    /// frame as `ResolvedLayout.position`), already intersected with all
    /// ancestor clips.
    pub rect: Rect, // bevy::math::Rect
    /// F: rounded-rect corners (from the clipping ancestor's
    /// `Border.radius`). C (reserved): arbitrary `clip-path` shapes.
    pub shape: ClipShape,
}

/// F = `RoundedRect`; the non-rectangular cases are reserved for the
/// C-tier `clip-path` work.
#[derive(Reflect, Default, Clone, PartialEq, Debug)]
pub enum ClipShape {
    #[default]
    Rect,
    RoundedRect(Corners),
    // C (reserved): Path / Circle / Ellipse / Polygon (clip-path).
}
```

The **fields here are deliberately the storage shape only** — how `rect` is
computed (ancestor intersection, the `Changed<ScrollOffset>` fast recompute, the
PAINT-containment boundary, the transformed-clip interaction) is owned by
[clip-and-transform.md](clip-and-transform.md). This file fixes the component's
name, ownership (render-prep writes, render + picking read), and that a single
read yields the final region.

## 10. `EffectGroup` — F (compositing-boundary marker)

```rust
/// Marker: this entity establishes an off-screen compositing boundary.
/// Written by the render-prep pass that detects an effect-group former
/// (`Opacity < 1`, `isolation: isolate`, non-empty `Filter`, non-`Normal`
/// `MixBlendMode`, non-empty `BackdropFilter`), removed when none holds.
/// Read by the compositor to allocate / pool a render target per group.
/// NOT author-set. Absent == this entity paints into its parent's target.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct EffectGroup;
```

`EffectGroup` is a pure marker — it carries no data because the *which-effect*
information already lives on the typed effect components (`Opacity` / `Filter` /
`MixBlendMode` / `BackdropFilter`) and the *isolation* bit on the layout-owned
`Stacking`. The marker exists so the compositor can query `With<EffectGroup>`
without re-deriving the former predicate every frame, and so its presence is a
single change-detection signal for "this subtree needs an off-screen target."
Its detection rule is the union of the SC-trigger set **plus** `BackdropFilter`
(which forms a compositing boundary without forming a stacking context). Render
target allocation/pooling, the v1 set (group `opacity` + `isolation` only), and
the reserved filter/blend seams are owned by
[effect-compositor.md](effect-compositor.md).

> The effect-group former set is a **superset** of the SC-trigger set
> (`BackdropFilter` is in one but not the other). The two predicates are kept
> distinct on purpose: layout 6f reads the SC set; the compositor reads the
> `EffectGroup` set. Conflating them would either make `backdrop-filter` form a
> spurious stacking context or make an isolated group skip its off-screen
> target. The membership table is owned by
> [effect-compositor.md](effect-compositor.md).

## 11. The `Visual` migration

`Visual`
([`crates/buiy_core/src/components.rs`](../../../crates/buiy_core/src/components.rs))
is the Phase-0 temporary carrier — `background_token`, `foreground_token`,
`border_radius`. It is fully replaced:

| `Visual` field | Target home | Notes |
|---|---|---|
| `background_token: String` | `Background.color: ColorToken` | Empty string → transparent maps to `ColorToken::default()`. |
| `border_radius: f32` | `Border.radius: Corners` | Uniform `f32` → `Corners::all(Radius::circular(px))`. |
| `foreground_token: String` | **moves to `buiy-text-rendering-design`** | Reserved/unused in Phase 1 render; it is a *text* color, so it belongs to the text spec, not here. |

Once `Background` + `Border` ship and the text spec owns the foreground token,
`Visual` is deleted from
[`components.rs`](../../../crates/buiy_core/src/components.rs) and its
render-extract usage retargets to the new components. **The migration mechanics
(when `Visual` is removed, how the Phase-0 extract retargets) are a plan
concern** — this spec defines only the target: `Visual` gone, its three concerns
re-homed as above. The Phase-0 closeout note in `Visual`'s own doc-comment
already names `Background` / `Border` as the successors, so this is the
fulfilment of a commitment the code already records, not a new decision.

## 12. Reserved vs. deferred, and open items

- **`visibility: visible | hidden | collapse`** (foundation §3.3, **F**) has no
  component on README § 3.2's list. It is render-owned (it paints nothing but
  keeps layout + a11y, unlike `Display::None`). Whether it becomes its own
  `Visibility` component or a field — and how `collapse` interacts with table
  layout — is **open**; see [README § 5](README.md#5-open-questions). It is
  flagged here so it is not silently dropped; it does not block the trigger-5
  follow-up and can land with the paint-order child.
- **`WillChange` (render-side promotion hint).** The render half of README § 4
  item 3 is a `WillChange`-driven layer-promotion hint that pairs with layout 6f
  reading the SC-forming-property clause. The *layout* `WillChange` type already
  exists in
  [`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs); whether the
  render promotion hint is a new component or a read of the existing one is a
  one-change-both-sides decision tracked in README § 4, not resolved here.
- **`Angle`, `BackgroundLayer`, `currentColor` resolution** are named-but-not-
  owned here: `Angle` and the gradient `BackgroundLayer` type belong to the
  units and gradient fast-follows; `ColorToken` (incl. `currentColor` /
  forced-colors) is owned by
  [color-and-forced-colors.md](color-and-forced-colors.md).

## 13. Registration and crate placement

Every component above is registered in the render plugin's `build` via
`app.register_type::<T>()` so reflection, BSN, and inspectors resolve them —
the same contract the layout plugin honors for its decomposed components
([layout/architecture.md § 2.1](../2026-05-08-buiy-layout-design/architecture.md#21-decomposed-components--canonical-storage)).

The SC-trigger components (`Opacity`, `Filter`, `BackdropFilter`,
`MixBlendMode`) carry the **crate-placement constraint** from
[README § 3.2](README.md#32-render-owned-this-spec-introduces): they must live
where both layout sub-pass 6f and render can read them, and the dependency edge
points layout → these components → render (never inverted). If the workspace
later splits `buiy_render` out of `buiy_core`
([README § 5](README.md#5-open-questions) #5), these components stay reachable
from layout. [architecture.md](architecture.md) pins the concrete crate home;
this file only restates the invariant that constrains it.

## Verification

How these claims are proven (gate IDs from
[verification.md](verification.md)):

- **Reflection/registration** — a headless test asserts every render component
  `register_type`s (no GPU needed), mirroring the layout plugin's
  registration test. Defaults match CSS initial values (`Opacity(1.0)`,
  `Background::default()` transparent, `Border::default()` square + no stroke,
  empty `BoxShadow`, `MixBlendMode::Normal`).
- **`Visual` migration** — a test (or the absence of `Visual` from the type
  registry once migration lands) pins that `Background.color` reproduces
  `Visual.background_token` and `Border.radius` reproduces `border_radius` on a
  fixture; gate #2 (visual-regression golden image) catches any pixel drift.
- **SC-trigger participation** — a headless layout-side test asserts an entity
  with `Opacity(0.5)` / non-empty `Filter` / non-`Normal` `MixBlendMode` forms a
  stacking context via 6f's `forms_stacking_context`, proving the reserved
  components unblock the follow-up *before* any shader exists.
- **`ClipRect` geometry** — proven under the layout-snapshot gate (#5), since the
  clip pass reads only layout output (the assertion lives in
  [clip-and-transform.md](clip-and-transform.md)).
- **`Outline` not-self-clipped** — gate #2 + gate #10 fixtures assert a focus
  ring survives an `overflow: hidden` ancestor.
