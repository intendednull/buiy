# Buiy render — component model

**Parent:** [README.md](README.md)

This file defines every render-owned component on the layout↔render boundary —
the typed surface that replaced the temporary `Visual` and the seam the layout
subsystem reaches back across for stacking-context trigger 5. The names and
ownership are fixed by [README § 3.2](README.md#32-render-owned-this-spec-introduces);
this file elaborates each component's fields, tier, and the reasoning that
shapes its shape. Field-level clip *geometry* lives in
[clip-and-transform.md](clip-and-transform.md); atlas sampling for tokenized
fills and glyph alpha lives in
[atlas-and-text-seam.md](atlas-and-text-seam.md); how the paint passes *consume*
these components is [paint-order-and-top-layer.md](paint-order-and-top-layer.md)
and [effect-compositor.md](effect-compositor.md).

The **author-set** components follow the foundation's decomposed-component
convention (foundation goal §1.3): each is a small public-fielded `Component`
deriving `Reflect + Default + Clone + Component`, registered in the render
plugin's `build` so reflection / BSN / inspectors find them. The three
**computed** components are deliberate exceptions — `ClipRect` (§ 9),
`AncestorClip` (clip-and-transform.md § A.2), and `EffectGroup` (§ 10) are render-prep-written, never
authored or serialized, so they carry leaner derives (no `Reflect` / `Default`)
per their owning specs. They are the
render-side analogue of the layout decomposed components in
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
  free" the architecture relies on. The same gate must also cover the
  **paint-skip** signals: the extract Or-set includes `Changed<CssVisibility>`
  (§ 12.1), `Changed<OffscreenAuto>` (§ 12.2), and `Changed<Containment>`
  (`content_visibility` toggle) — all three flip whether a subtree paints at all
  (`visibility: hidden`, off-screen-`Auto` add/remove, `content-visibility`
  change), so without them in the trigger set a paint-skip flip would never
  re-extract and paint would go stale (architecture.md § 3.1).

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
a typed reference into `Res<Theme>` with four variants — `Transparent`,
`Token(name)`, `CurrentColor`, and the forced-colors `SystemColor(keyword)`
(see [color-and-forced-colors.md § 2.0](color-and-forced-colors.md#20-the-colortoken-type)
for the variant set); the **enum and its
variants are defined and owned** by
[color-and-forced-colors.md § 2](color-and-forced-colors.md#2-theme-token-resolution-at-extract-time),
which also owns each variant's resolution against the active theme, the
linear-light pipeline, and the forced-colors contract. This file treats
`ColorToken` as an opaque value type and only states *which fields hold one*.

```rust
/// A themeable color reference. Resolved against `Res<Theme>` at extract
/// time by the color pass; see color-and-forced-colors.md § 2.0 for the
/// variant set. Carries all four variants — `Transparent`, `Token(name)`,
/// `CurrentColor`, and the forced-colors `SystemColor(keyword)` — so a
/// render component never stores a pre-resolved `Color` for a themeable
/// slot.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum ColorToken { /* variants defined in color-and-forced-colors.md § 2 */ }
```

Non-themeable literal colors (rare; e.g. a debug overlay) may store a raw
`bevy::color::Color` directly, but the F-tier authoring path is always a token.

## 3. `Background` — F

Replaces `Visual.background_token`. v1 ships a **single solid color token** and
nothing else — the v1 struct is `Background { color: ColorToken }`.

```rust
/// Solid background fill (v1). Replaces `Visual.background_token`.
/// Absent component == transparent.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 3.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Background {
    /// F: solid fill. `ColorToken::default()` (transparent) == no fill,
    /// matching `Visual.background_token == ""`.
    pub color: ColorToken,
}
```

The layered / gradient surface (foundation §3.3 *Backgrounds*, all **C**) is
**reserved in prose, not as a v1 field**: a future `layers: Vec<BackgroundLayer>`
will carry ordered background layers (gradients, images, multiple stacked fills)
painted over `color`, bottom-to-top, with gradient stops sampling the atlas
([atlas-and-text-seam.md](atlas-and-text-seam.md)). That `layers` field and the
`BackgroundLayer` type both land **with the gradient / image fast-follow** — the
v1 struct intentionally does **not** name an undefined `BackgroundLayer` type.
Adding the field later is a purely additive C-tier change that does not alter the
component's identity or the v1 → C migration of the solid `color` it already
ships.

`color` resolving to transparent makes the entity background-free without
removing the component — the render extract skips emitting a quad primitive for
a transparent fill, preserving `Visual`'s "empty string → skip the fill"
behavior. The v1 painted shape is just the **border box** from `ResolvedLayout`
(the `background-clip` box-selection knob that chooses which box to fill — padding
box, content box — is the deferred **C-tier** clause and lands with the reserved
`layers` work).

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
A paint radius accepts only the resolvable subset (`Px` / `Percent` /
container-query `Cq*`, all of which resolve to px); the grid-only `Fr` variant is
**not applicable** to a corner radius and is **warned-and-resolved to `0`px**,
mirroring how layout warns-once-and-falls-back when `Fr` appears outside a grid
track ([`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs)
`Length::Fr`). The same warn-and-resolve rule applies to every paint `Length`
field in this file (`Border.radius`, `BoxShadow` terms, `Outline.width` /
`Outline.offset`).

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

The `BoxShadow` list-index order (which shadow within *this* element's list
paints over which — index 0 frontmost) and the shadow-under-quad paint order
(every outset shadow paints behind this element's own background/border quad,
every inset shadow above its fill) are **orthogonal**: list index orders the
shadows *among themselves*, while the under-/over-quad rule fixes where the whole
shadow group sits relative to the element's box. Neither order changes the
other.

## 6. `Opacity` — F (and an `EffectGroup` former)

```rust
/// Group opacity in `[0.0, 1.0]`. `1.0` (default) is a no-op. A value
/// `< 1.0` forms an `EffectGroup` (off-screen composite boundary) AND is
/// a stacking-context trigger that layout sub-pass 6f *will* read back
/// once the trigger-5 clause lands (pillar 6). Absent == opaque.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 6.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Opacity(pub f32);

impl Default for Opacity {
    fn default() -> Self { Opacity(1.0) }
}
```

`Opacity` carries a **manual `Default`** (initial `1.0`), not the derived one: a
derived `Default` on a tuple struct over `f32` would give `Opacity(0.0)` — fully
transparent — which is the wrong CSS initial value. The `#[reflect(Default)]`
attribute binds this manual impl, so reflection / BSN / inspectors construct
`Opacity(1.0)` too. This is a deliberate divergence from the derive-`Default`
convention every other author-set component in this file follows, justified by the
CSS initial value `opacity: 1`.

`Opacity` is the one v1 effect that is *both* fully shipped paint **and** a
compositing boundary. When `0.0 ≤ value < 1.0` the entity is a
**group-opacity** case: its whole subtree is composited to an off-screen target
and that target is blended at `value`, so overlapping children don't
double-blend (the correctness point the user insisted on — pillar 6, runner-up
"forward-pass approximation" was rejected). Forming the boundary is the job of
[effect-compositor.md](effect-compositor.md); this component is the trigger.

`Opacity < 1` *will* participate in stacking-context formation once layout
sub-pass 6f gains its trigger-5 clause. Today `forms_stacking_context`
([`layout/systems.rs`](../../../crates/buiy_core/src/layout/systems.rs)) takes no
`Opacity` parameter — the render-side trigger components do not yet exist for it
to read. Because 6f (`stacking_context`) is the SC former and lives across the
layout↔render boundary, the trigger components it *will* read must be visible to
layout — the crate-placement constraint in
[README § 3.2](README.md#32-render-owned-this-spec-introduces) and pinned by
[architecture.md](architecture.md). `Opacity` is in the future SC-trigger union
alongside `Filter` / `MixBlendMode` (below) and the layout-owned `Stacking`
(`isolation`) and `UiTransform`.

> **Distinct from `visibility` and `Display::None`.** `opacity: 0` still
> participates in layout, paint traversal, and hit-testing (it forms an
> invisible-but-present group); `Display::None` (layout-owned) skips everything;
> `visibility: hidden` (foundation §3.3, **F**, render-owned, the `CssVisibility`
> component defined in [§ 12.1](#121-cssvisibility--f-render-owned))
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
   `Outline` using the entity's companion **`AncestorClip`** (the intersection of
   its *ancestor* clip boxes only, without the own-box intersection step), **not**
   the entity's own `ClipRect` (which is already intersected with the element's
   border box and would crop the ring back to the box). `AncestorClip` is emitted
   alongside `ClipRect` by `WriteClipRects` and is **owned and defined** by
   [clip-and-transform.md § A.2](clip-and-transform.md#a2-the-cliprect-output-shape);
   this is the one place a render component deliberately reads a different clip
   than the box it decorates. The rule is stated here and enforced by the clip
   pass; the gate-#10 hit-target / gate-#2 visual-regression fixtures assert a
   focus ring survives an `overflow: hidden` ancestor.

The outline shape is the border box expanded by `width + offset`, rounded by
`Border.radius` grown by the same amount (CSS-faithful outline corner rounding).

`Outline.width` and `Outline.offset` are paint `Length`s and follow the same
applicable-subset rule as `Border.radius` (§ 4): only the resolvable variants
(`Px` / `Percent` / `Cq*`) apply; the grid-only `Fr` variant is
warned-and-resolved to `0`px, mirroring layout's `Length::Fr` fallback.

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

So these three ship as registered, reflectable, public-fielded components.
`Filter` and `MixBlendMode` are slated for **two** behaviors — `EffectGroup`
formation (active in v1) and the SC-trigger union (which layout 6f *will* read
once its trigger-5 clause lands). `BackdropFilter` participates in **only**
`EffectGroup` formation: it is the "EffectGroup-but-not-SC" case (it forms a
compositing boundary without forming a stacking context, so layout 6f does
**not** read it). `EffectGroup` formation is read by the compositor to allocate
an off-screen target ([effect-compositor.md](effect-compositor.md)). All three contribute **no**
pixels until their shader lands. An entity carrying `Filter([Blur(4px)])` forms
an effect group in v1 and *will* form a stacking context once layout 6f gains
its trigger-5 clause (today `forms_stacking_context` takes no `Filter`
parameter); the group composites with an identity pass (the reserved seam) and
the blur is a no-op until the filter shader ships. The deferral is a missing
shader, never a missing trigger.

```rust
/// C (reserved). Filter function list. `EffectGroup` former in v1 and a
/// future SC-trigger (layout 6f will read it once its trigger-5 clause
/// lands); the filter shaders (blur/brightness/.../drop-shadow) are
/// deferred (foundation §3.3). A non-empty list forms an `EffectGroup`.
/// Empty / absent == no filter.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Filter(pub Vec<FilterFn>);

/// C (reserved). Backdrop filter list — samples what is BEHIND the
/// element. Component + the off-screen boundary ship v1; the
/// backdrop-sampling shader is deferred ([effect-compositor.md § 6](effect-compositor.md)). Forms an `EffectGroup` (the
/// compositor must hold a backdrop copy). Buiy treats `backdrop-filter`
/// as an effect-group former ONLY; CSS additionally makes it form a
/// stacking context, which Buiy does not rely on because `EffectGroup`
/// membership is derived render-side — see note below.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BackdropFilter(pub Vec<FilterFn>);

/// C (reserved). Blend mode against the backdrop. Any value other than
/// `Normal` forms an `EffectGroup` in v1 and is a future SC-trigger
/// (layout 6f will read it once its trigger-5 clause lands); the blend
/// shader is deferred (foundation §3.3). `Normal` (default) is a no-op.
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

**`MixBlendMode` will be a stacking-context trigger; `BackdropFilter` will not**
— a non-`Normal` blend mode and a non-empty `filter` form stacking contexts,
matching CSS. `BackdropFilter` is the deliberate divergence: Buiy treats
`backdrop-filter` as an effect-group former ONLY; CSS additionally makes it form
a stacking context, which Buiy does not rely on because `EffectGroup` membership
is derived render-side. So the trigger-5 union layout 6f *will* read is the
SC-forming set — `Opacity < 1`, non-empty `Filter`, non-`Normal` `MixBlendMode`,
`Stacking.isolation == Isolation::Isolate` — and `BackdropFilter` is excluded
from it. The canonical effect-group-former predicate (the superset that *does*
include `BackdropFilter`) is owned by
[effect-compositor.md § 1](effect-compositor.md#1-the-identity-effect-group-set--effect-forming-stacking-context-set); the exact
SC-trigger predicate is owned by layout's
[stacking-and-top-layer.md](../2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
`forms_stacking_context`; this file fixes *which render components feed it*.
`will-change` of an SC-forming property is the fifth SC wedge (README § 4 item 3,
the `WillChange` hint already in
[`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs)). It is on a
**different timeline** from the three C-tier trigger-5 components above and must
not be conflated with them: the `will-change` SC-former is **tier-E / deferred**
per the layout spec — named-only, not shipped in v1 — whereas `Filter` /
`MixBlendMode` / `BackdropFilter` are the **near-term C-tier** trigger-5 components
that ship their component model in v1 (the `opacity` / `filter` /
`mix-blend-mode` clause). A render-side `WillChange` layer-promotion hint is
correspondingly **reserved/open**, not a contract component listed today (see the
reserved row in [README § 3.2](README.md#32-render-owned-this-spec-introduces));
whether it is a new render component or a read of the existing layout type, and
layout's reading of the SC-forming-property clause, land as the one paired change
README § 4 item 3 describes — strictly after the C-tier trigger-5 components.

`Angle` (for `HueRotate`) is a **v1 unit prerequisite defined here** as a minimal
stub so `FilterFn::HueRotate(Angle)` compiles against v1:

```rust
/// Angle in radians. Minimal v1 stub so `FilterFn::HueRotate(Angle)` compiles.
/// The full CSS angle unit family (deg/grad/turn parsing, the rest of foundation
/// §3.3 *Units* angle surface, which is **C**) lands with the units fast-follow,
/// which is expected to absorb or re-home this type; v1 carries only the radian
/// scalar `HueRotate` needs.
#[derive(Reflect, Clone, Copy, PartialEq, Debug)]
pub struct Angle(pub f32); // radians
```

This is a v1 unit prerequisite, not a deferred fast-follow type — the stub must
exist now because `FilterFn` is a shipped (registered, reflectable) v1 enum even
though `HueRotate`'s evaluation is deferred (foundation §3.3 *Units*: the broader
angle surface is **C**).

## 9. `ClipRect` — F (computed, not authored)

`ClipRect` is one of the **three computed** render components that are *not*
author-set (alongside `AncestorClip` (clip-and-transform.md § A.2) and `EffectGroup` § 10); what distinguishes it is that it is read by **both** render and picking. Its type definition (fields + the accumulation algorithm) is owned
by [clip-and-transform.md § A.2](clip-and-transform.md#a2-the-cliprect-output-shape)
— it is `ClipRect { min: Vec2, max: Vec2 }` (logical px, the accumulated clip
AABB),
written by the `WriteClipRects` render-prep pass (pillar 4) and read by **both**
render (to clip primitives) and picking. This file does **not** redefine it.

Absent-semantics: "Absent ClipRect ⇔ no ancestor clips this entity ⇒ render
applies no scissor."

The reserved rounded-corner clip is carried by a **separate** sibling component
`ClipRadius` (C-tier, reserved, not built v1) — the rounded-rect / `clip-path`
cases live there, not as a `shape` field on `ClipRect`. Both components, and the
geometry algorithm, are owned by
[clip-and-transform.md](clip-and-transform.md).

## 10. `EffectGroup` — F (compositing-boundary, carries the reason)

```rust
/// Which effect(s) caused this entity to form an off-screen compositing
/// boundary. The compositor reads `reason` to choose the composite op
/// without re-querying the five effect components.
bitflags::bitflags! {
    /// One entity can carry several reasons at once (opacity<1 AND isolate).
    pub struct EffectReason: u8 {
        const OPACITY         = 1;  // v1: carried
        const ISOLATION       = 2;  // v1: carried
        const FILTER          = 4;  // reserved: marks the group, no shader in v1
        const BACKDROP_FILTER = 8;  // reserved: marks the group, needs backdrop sample ([effect-compositor.md § 6](effect-compositor.md))
        const MIX_BLEND       = 16; // reserved: marks the group, no shader in v1
    }
}

/// This entity establishes an off-screen compositing boundary. Written by
/// the render-prep pass that detects an effect-group former — the canonical
/// predicate is owned by effect-compositor.md § 1 (`Opacity < 1`,
/// `Stacking.isolation == Isolation::Isolate`, non-empty `Filter`,
/// non-`Normal` `MixBlendMode`, non-empty `BackdropFilter`), removed when
/// none holds. Read by the
/// compositor to allocate / pool a render target per group. NOT author-set.
/// Absence of the component == no group (this entity paints into its
/// parent's target).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
#[derive(Component, Clone, Copy, Debug)]
pub struct EffectGroup {
    /// The OR of every reason that formed this group. NO `Default`:
    /// an `EffectGroup` only exists when at least one reason holds.
    pub reason: EffectReason,
}
```

`EffectGroup` **carries data** — `reason` records *which* of the formers (one or
more, OR-ed) caused the boundary. The compositor reads `reason` to pick the
composite op (alpha-blend for `OPACITY`, plain copy for `ISOLATION`, the
filter/backdrop/blend seams for the others) **without re-querying the five
underlying components** (`Opacity` / `Filter` / `MixBlendMode` / `BackdropFilter`
and the layout-owned `Stacking.isolation` bit, spelled `Isolation::Isolate`).
Its detection rule — the canonical effect-group-former predicate owned by
[effect-compositor.md § 1](effect-compositor.md#1-the-identity-effect-group-set--effect-forming-stacking-context-set)
— is the union of the *effect-forming* SC triggers **plus** `BackdropFilter` (which forms a
compositing boundary without forming a stacking context). Render target allocation/pooling,
the v1 set (group `opacity` + `isolation` only), and the reserved filter/blend
seams are owned by [effect-compositor.md](effect-compositor.md), which uses this
exact struct.

> The effect-group former set is a **superset** of the SC-trigger set
> (`BackdropFilter` is in one but not the other). The two predicates are kept
> distinct on purpose: layout 6f reads the SC set; the compositor reads the
> `EffectGroup` set. Conflating them would either make `backdrop-filter` form a
> spurious stacking context or make an isolated group skip its off-screen
> target. The membership table is owned by
> [effect-compositor.md](effect-compositor.md).

## 11. The `Visual` migration

`Visual` was the Phase-0 temporary carrier — `background_token`,
`foreground_token`, `border_radius`. It has been **deleted** from
[`crates/buiy_core/src/components.rs`](../../../crates/buiy_core/src/components.rs)
and fully replaced by the render-side components in
[`crates/buiy_core/src/render/components.rs`](../../../crates/buiy_core/src/render/components.rs):

| `Visual` field | Target home | Notes |
|---|---|---|
| `background_token: String` | `Background.color: ColorToken` | Empty string → transparent maps to `ColorToken::default()`. |
| `border_radius: f32` | `Border.radius: Corners` | Uniform `f32` → `Corners::all(Radius::circular(px))`. |
| `foreground_token: String` | **moves to `buiy-text-rendering-design`** | Reserved/unused in Phase 1 render; it is a *text* color, so it belongs to the text spec, not here. |

`Background` + `Border` shipped, the text spec owns the foreground token, and
`Visual` was deleted from
[`components.rs`](../../../crates/buiy_core/src/components.rs); its
render-extract usage retargeted to the new components. **The migration mechanics
(when `Visual` was removed, how the Phase-0 extract retargeted) were a plan
concern** — this spec defines only the target: `Visual` gone, its three concerns
re-homed as above. This section remains as a historical migration record; the
landed successors live in
[`render/components.rs`](../../../crates/buiy_core/src/render/components.rs).

The Phase-0 `Visual` design had also contemplated a third successor,
**`Stroke`** — which this spec deliberately did **not** mint as a separate
component. That `Stroke` placeholder is **subsumed by `Border` (§ 4)**: per-side
line style is `BorderSide.style` folded into `Border`, not a standalone stroke
type. So the `Stroke` concern resolved to "the `style` longhand on `Border`" in
the landed
[`render/components.rs`](../../../crates/buiy_core/src/render/components.rs) — the
placeholder was fulfilled, not dropped.

## 12. Defined-here F-tier rows, reserved items, and open items

`CssVisibility` is the one README § 3.2 **render-owned** F-tier row this file
defines concretely (rather than leaving it dangling as "reserved"). The
`OffscreenAuto` marker defined alongside it in § 12.2 is **not** a § 3.2
render-owned component — it is a [README § 3.1](README.md#31-layout-owned-render-reads-already-exist)
**layout-written, render-read** marker; this file pins its shape because render
extract consumes it, but layout owns and writes it.

### 12.1 `CssVisibility` — F (render-owned)

```rust
/// CSS `visibility`. `Hidden` skips paint for this entity's subtree but keeps
/// its layout box and a11y presence (unlike `Display::None`). `Collapse` is a
/// deferred marker (table-row / flex-item collapse) — named only in v1.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 12.1.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Default)]
#[reflect(Component, Default)]
pub enum CssVisibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}
```

This component is named `CssVisibility`, **not** `Visibility`, and deliberately
does **not** reuse `bevy::prelude::Visibility`. Bevy already ships a
`Visibility { Inherited, Hidden, Visible }` enum (bevy_camera `visibility/mod.rs`,
re-exported in `bevy::prelude`) with different variants and its own visibility
systems; a render-owned CSS-visibility enum named `Visibility` would collide with
it on import and inherit Bevy's inherit/cull semantics rather than CSS's. This is
the same name-collision rationale that keeps the layout transform bridge writing
Bevy's `Transform` rather than minting a Buiy `Transform`
([clip-and-transform.md § B.1](clip-and-transform.md#b1-the-ownership-problem-pillar-5)) —
where a Bevy type with established semantics owns the name, Buiy renames its
property carrier to avoid the collision.

`CssVisibility` is render-owned and F-tier (foundation §3.3 *visibility* is **F**).
**v1 ships the `Hidden` paint-skip:** the subtree is not painted but keeps its
layout box (the box still occupies space and still contributes to layout — the
distinction from `Display::None`, which skips everything). Per CSS
`visibility: hidden`, the subtree is *also* not hit-tested; that picking
interaction is **owned by [`buiy-input-events-design`](../2026-05-07-buiy-foundation/README.md)**,
not committed here. The paint-skip rule itself is stated in
[paint-order-and-top-layer.md § 5](paint-order-and-top-layer.md#5-skip-rules).

`Collapse` (table-row / flex-item collapse) is a **deferred marker** — the enum
variant exists so authors can write `visibility`-aware code that compiles against
v1, but its collapse-the-box behavior (how a collapsed row removes its track,
how a collapsed flex item is treated) is **named-only in v1** and lands with the
table / flex-collapse fast-follow.

### 12.2 `OffscreenAuto` — F (layout-written marker)

```rust
/// Zero-field marker placed by LAYOUT on entities whose
/// `Containment.content_visibility == Auto` subtree is currently off-screen
/// (the `is_off_screen` test against the `ContentVisibilityMargin`-expanded
/// viewport). Render's extract skips paint for an `OffscreenAuto` subtree
/// exactly as it skips a `Containment.content_visibility == Hidden` subtree.
/// (`ContentVisibility` is the enum *type* of that field —
/// `Visible`/`Auto`/`Hidden`; the field carrier is `Containment`.)
/// Layout-written, render-read. NOT registered by this spec's render plugin:
/// it is layout-owned (README § 3.1), so layout's plugin owns whether and how
/// it is reflected/registered (matching the § 13 "layout-written markers are
/// not reflected here" convention) — render only reads it.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 12.2.
#[derive(Component, Clone, Copy, Debug)]
pub struct OffscreenAuto;
```

`OffscreenAuto` is the carrier for the render half of layout's Phase 11 (the
off-screen `content-visibility: auto` *paint* skip — see
[paint-order-and-top-layer.md § 5.3](paint-order-and-top-layer.md#53-contentvisibilityauto-off-screen--skip-paint-render-owned-half)).
Render reuses layout's already-computed off-screen determination rather than
recomputing visibility, keeping render a thin consumer (README § 2 pillar 1).

> **This marker does not exist in layout today (grep-confirmed).** Layout's
> off-screen test runs inline in `sync_styles`
> ([`layout/systems.rs`](../../../crates/buiy_core/src/layout/systems.rs)) and
> persists nothing render can read. Emitting `OffscreenAuto` from that pass is the
> **layout-side deliverable** of the render half of Phase 11 — a **tracked
> cross-spec dependency** (README § 5), not code this spec can assume present.
> [paint-order-and-top-layer.md § 5.3](paint-order-and-top-layer.md#53-contentvisibilityauto-off-screen--skip-paint-render-owned-half)
> references the marker by name; this section defines its shape.

### 12.3 Reserved / open

- **`WillChange` (render-side promotion hint).** The render half of README § 4
  item 3 is a `WillChange`-driven layer-promotion hint that pairs with layout 6f
  reading the SC-forming-property clause. It is on the **tier-E / deferred**
  timeline (the `will-change` SC-former; see [§ 8](#8-the-reserved-effect-components--c-ship-now-shaders-deferred)),
  **distinct from** the near-term C-tier trigger-5 components — do not conflate
  the two. It is carried as a **reserved/open** row in
  [README § 3.2](README.md#32-render-owned-this-spec-introduces), not a contract
  component listed today. The *layout* `WillChange` type already exists in
  [`layout/types.rs`](../../../crates/buiy_core/src/layout/types.rs); whether the
  render promotion hint is a new component or a read of the existing one is a
  one-change-both-sides decision tracked in README § 4, not resolved here.
- **`BackgroundLayer`, `currentColor` resolution** are named-but-not-owned
  here: the gradient `BackgroundLayer` type lands with the gradient / image
  fast-follow (§ 3); `ColorToken` (incl. `currentColor` / forced-colors) is owned
  by [color-and-forced-colors.md](color-and-forced-colors.md). (`Angle`, by
  contrast, **is** defined here as a v1 unit prerequisite — see [§ 8](#8-the-reserved-effect-components--c-ship-now-shaders-deferred) —
  because the shipped `FilterFn` enum needs it to compile.)

## 13. Registration and crate placement

Every **author-set** component above is registered in the render plugin's
`build` via `app.register_type::<T>()` so reflection, BSN, and inspectors resolve
them — the same contract the layout plugin honors for its decomposed components
([layout/architecture.md § 2.1](../2026-05-08-buiy-layout-design/architecture.md#21-decomposed-components--canonical-storage)).
The three computed components (`ClipRect`, `AncestorClip`, `EffectGroup`) are not author-set or
serialized and so are not `register_type`'d; they exist only as render-prep
outputs. The layout-written `OffscreenAuto` marker (§ 12.2) is likewise **not**
registered by this spec's render plugin — it is layout-owned (README § 3.1), so
layout's plugin owns its registration; render only reads it.

The three SC-trigger components (`Opacity`, `Filter`, `MixBlendMode`) — plus
`BackdropFilter`, which is **not** read by layout 6f but shares the crate home
for `EffectGroup` reasons — carry the **crate-placement constraint** from
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

- **Reflection/registration** — a headless test asserts every author-set render
  component `register_type`s (no GPU needed), mirroring the layout plugin's
  registration test (the computed `ClipRect` / `AncestorClip` / `EffectGroup` are exempt — they
  are not reflected). Defaults match CSS initial values (`Opacity(1.0)`,
  `Background::default()` transparent, `Border::default()` square + no stroke,
  empty `BoxShadow`, `MixBlendMode::Normal`).
- **`Visual` migration** — a test (or the absence of `Visual` from the type
  registry once migration lands) pins that `Background.color` reproduces
  `Visual.background_token` and `Border.radius` reproduces `border_radius` on a
  fixture; gate #2 (visual-regression golden image) catches any pixel drift.
- **SC-trigger participation** — once layout 6f gains its trigger-5 clause, a
  headless layout-side test asserts an entity with `Opacity(0.5)` / non-empty
  `Filter` / non-`Normal` `MixBlendMode` forms a stacking context via 6f's
  `forms_stacking_context`, proving the reserved components unblock the follow-up
  *before* any shader exists. (Today `forms_stacking_context` takes no such
  parameter; the components ship first so the clause can be added in one change.)
- **`ClipRect` geometry** — proven under the layout-snapshot gate (#5), since the
  clip pass reads only layout output (the assertion lives in
  [clip-and-transform.md](clip-and-transform.md)).
- **`Outline` not-self-clipped** — gate #2 + gate #10 fixtures assert a focus
  ring survives an `overflow: hidden` ancestor.
