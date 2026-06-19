# Render component model (R1) Implementation Plan

**Date:** 2026-06-03
**Status:** landed

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** nothing. **Execution order:** R1 → R2 → R3 → R4 → R5 → R6 → R7 → R8 → (R9, R10) → R11. R1 is the FIRST phase and the **sole creator** of `crates/buiy_core/src/render/components.rs` and `crates/buiy_core/src/render/color.rs`, and the **sole definer** of every shared render type: `Background`, `Border` (+ `BorderSide`, `Radius`, `Corners`, `LineStyle`), `BoxShadow` (+ `Shadow`), `Opacity` (manual `Default 1.0`), `Outline`, `CssVisibility`, `OffscreenAuto`, `ClipRect`, `AncestorClip`, `EffectGroup` (+ `EffectReason`), `Filter` (+ full 10-variant `FilterFn`), `MixBlendMode`, `BackdropFilter`, `Angle`, `ClipRadius`, and `ColorToken` (+ `SystemColorKeyword`, in `render/color.rs`). Every later phase imports these from `render::components` / `render::color` and MUST NOT redefine them, re-`pub mod`, or re-`register_type` them.

**Goal:** Replace the temporary `Visual` carrier with the render-side component model from the render-pipeline spec (`Background`/`Border`/`BoxShadow`/`Opacity`/`Outline`/`CssVisibility`/`EffectGroup`/`ClipRect`/the reserved effect components), register the author-set ones, and migrate the Phase-0 render extract + button + example onto `Background`/`Border` with zero pixel-behavior change.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [component-model.md](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md) (all sections) and [color-and-forced-colors.md § 2.0](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md#20-the-colortoken-type) (`ColorToken`).
**Architecture:** A new `crate::render::components` module holds every render-owned component as a small public-fielded decomposed component (foundation §1.3 convention), each deriving `Reflect + Default + Clone + Component` with the computed exceptions (`ClipRect`, `AncestorClip`, `EffectGroup`) carrying leaner derives. `ColorToken` + `SystemColorKeyword` live in a sibling new module `crate::render::color` (spec color-and-forced-colors.md § 2.0 names color.rs as the canonical owner); `render/components.rs` imports `ColorToken` from `render::color`. Author-set types are `register_type`'d in `BuiyRenderPlugin::build` **before** the RenderApp branch (so registration runs in the main world and is headless-testable). The extract path resolves a `ColorToken` against `Res<Theme>` exactly as Phase-0 resolved a `background_token` string, so the rounded-rect drawing behaviour and `render_smoke`/`render_instance` stay green.
**Tier/Test reality:** HEADLESS (unit/integration on CI). Every gating test in this plan runs under `App::new()` + `MinimalPlugins` (+ `CorePlugin`/`BuiyRenderPlugin` where a type registry is needed) with **no** wgpu adapter — the component model, `Default`/field math, `ColorToken` resolution, and `register_type` coverage need no GPU. The two pre-existing GPU tests in `render_smoke.rs` stay `#[ignore]`d (no wgpu adapter on CI/this host); this plan adds **no** new `#[ignore]` tests because nothing here constructs a RenderApp or compiles a pipeline.

THE GATE — every commit must keep this green (this host + CI have NO xvfb and NO wgpu adapter):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

Repo worktree root: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline`

---

## Orientation for an engineer with zero context

- **Where things live.** `buiy_core` is the only crate touched for types. The Phase-0 render lives in `crates/buiy_core/src/render/{mod,node,pipeline,instance}.rs` + `shader.wgsl`. The temporary `Visual` lives in `crates/buiy_core/src/components.rs` (crate root, alongside `Node`/`ResolvedLayout`/`ResolvedTransform`/`StackingContext`). Layout value types (`Length`, `ColumnRuleStyle`, `Isolation`) live in `crates/buiy_core/src/layout/types.rs`; layout components in `crates/buiy_core/src/layout/components.rs`.
- **Decomposed-component convention** (mirror it exactly): each author-set component is `#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]` + `#[reflect(Component, Default)]`, all fields public. Types that hold a `bevy::color::Color` (none here, but `ColorToken` holds f32 indirectly via resolution only — the tokens themselves are `PartialEq`-able) derive `PartialEq` not `Eq`. See `BoxModel` / `Overflow` / `Stacking` in `layout/components.rs` and `ColumnRule` / `ScrollbarColor` in `layout/types.rs` for the established idiom.
- **Manual-`Default` exceptions** (do NOT derive `Default`): `Opacity` (must be `1.0`, derived would be `0.0`), `ClipRect` (computed, no `Default`/`Reflect`), `EffectGroup` (computed, only exists when a reason holds — no `Default`/`Reflect`). `Scale` in `layout/components.rs` is the precedent for a hand-written `Default` on a tuple struct with `#[reflect(Default)]`.
- **`register_type` placement.** `BuiyRenderPlugin::build(app)` receives the **main** `App` and early-returns when `get_sub_app_mut(RenderApp)` is `None`. Put every `app.register_type::<T>()` call **before** that early-return so reflection registration happens in the main world even on headless CI (where `render_smoke::render_plugin_loads_without_panic` adds `BuiyRenderPlugin` under `MinimalPlugins` with no RenderApp). The computed `ClipRect`/`EffectGroup` and the layout-owned `OffscreenAuto` are NOT registered here (§ 13 of the spec).
- **Test idioms to mirror.** `crates/buiy_core/tests/render_instance.rs` is the pure-CPU test style (no app). `crates/buiy_core/tests/render_smoke.rs` shows the `App::new()+MinimalPlugins+CorePlugin+BuiyRenderPlugin` headless style and the `#[ignore]` GPU convention. Component `Default`/field unit tests live in `#[cfg(test)] mod tests` inside the module (see the bottom of `layout/components.rs`). Registration coverage is an integration test under `crates/buiy_core/tests/`.
- **Cross-phase dependency assumed:** `ClipRect`/`AncestorClip` are *defined as types here* (this phase) but *written* by the `WriteClipRects` render-prep pass in a later phase (R2 / clip-and-transform). This plan only mints the type + its `min`/`max` geometry fields and a CPU intersection helper used by the registration/field tests; it does NOT wire any system that writes it. Likewise `OffscreenAuto` is layout-written — this plan defines its shape (so render extract can later read it) but does not register or emit it.

Naming is **fixed by the spec**; do not invent names that contradict it. The exact set this phase introduces: `ColorToken` (+ `SystemColorKeyword`), `Background`, `Border` (+ `BorderSide`, `Corners`, `Radius`, `LineStyle`), `BoxShadow` (+ `Shadow`), `Opacity`, `Outline`, `Filter`/`BackdropFilter`/`MixBlendMode` (+ `FilterFn`, `Angle`), `CssVisibility`, `OffscreenAuto`, `ClipRect`, `AncestorClip`, `ClipRadius`, `EffectGroup` (+ `EffectReason`).

---

## Module layout decision (read once)

All new component types go into a single new file `crates/buiy_core/src/render/components.rs`, declared `pub mod components;` in `render/mod.rs`. `ColorToken` + `SystemColorKeyword` live in a **sibling** new file `crates/buiy_core/src/render/color.rs`, declared `pub mod color;` in `render/mod.rs`; `render/components.rs` does `use crate::render::color::ColorToken;`. Rationale: the spec calls the components "render-owned"; co-locating them under `render/` mirrors how layout types sit under `layout/`, and color-and-forced-colors.md § 2.0 names `render/color.rs` as the canonical owner of `ColorToken` so the R11 forced-colors phase can EXTEND that same file with resolution logic (it must not redefine the enum). Re-export the author-set public types from the crate root (`lib.rs`) and from `render/mod.rs` so `buiy::*` users and the migrated `button.rs`/`hello_button` reach them; `ColorToken` re-exports from `render::color`. `Visual` is deleted from the crate-root `components.rs` and dropped from every re-export in the final migration task.

`SystemColorKeyword` (in `render/color.rs`) and `Angle` (in `render/components.rs`) are **v1 unit prerequisites defined here** (the spec assigns ownership of `SystemColorKeyword`'s *resolution* to color-and-forced-colors.md and `buiy-theme-tokens-design`, but the *enum* must exist for `ColorToken::SystemColor(_)` to compile, exactly like `Angle` for `FilterFn::HueRotate`). Define them with the 16-keyword / radian-scalar shapes the spec names; do not implement resolution here.

---

## Task 1 — `ColorToken` + `SystemColorKeyword` (new `render/color.rs`)

The themeable color reference every paint field holds. Default is `Transparent` (CSS-initial "no fill", matching `Visual.background_token == ""`). These two types live in their own new module `render/color.rs` — color-and-forced-colors.md § 2.0 names `render/color.rs` as the canonical owner so the R11 forced-colors phase can EXTEND this file with resolution logic. `render/components.rs` (created in Task 2 alongside `Background`) imports `ColorToken` from `render::color`.

**Files**
- Create: `crates/buiy_core/src/render/color.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod color;`)

Steps:

- [ ] Add `pub mod color;` to `crates/buiy_core/src/render/mod.rs` (next to the existing `pub mod instance;` etc.). (`pub mod components;` is added in Task 2.)
- [ ] Write the failing test first. Create `crates/buiy_core/src/render/color.rs` with only the test module and a stub so the crate compiles to a failing assertion:

```rust
//! The `ColorToken` themeable color reference (the layout↔render paint
//! boundary's color seam) + the CSS `SystemColorKeyword` set. Canonical
//! owner per color-and-forced-colors.md § 2.0; the R11 forced-colors phase
//! EXTENDS this file with resolution logic (it must not redefine these
//! enums). `render/components.rs` imports `ColorToken` from here.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.

use bevy::prelude::*;
use std::borrow::Cow;

/// CSS system-color keyword. Foundation-**F** set (16 keywords). Defined
/// here as a v1 unit prerequisite so `ColorToken::SystemColor(_)` compiles;
/// its *resolution* against the active theme's system-color map is owned by
/// color-and-forced-colors.md § 3 / `buiy-theme-tokens-design`.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SystemColorKeyword {
    #[default]
    Canvas,
    CanvasText,
    LinkText,
    ButtonText,
    ButtonBorder,
    GrayText,
    Highlight,
    HighlightText,
    Field,
    FieldText,
    Mark,
    MarkText,
    SelectedItem,
    SelectedItemText,
    AccentColor,
    AccentColorText,
}

/// A themeable color reference, resolved against `Res<Theme>` at extract
/// time (color-and-forced-colors.md § 2.1). Default is `Transparent`,
/// matching `Visual.background_token == ""` and the CSS-initial "no fill"
/// semantics (component-model.md § 2 / § 3).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md § 2.0.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum ColorToken {
    /// CSS `transparent` (and the empty-token "skip the fill" case). The
    /// default; resolves to `Color::NONE` (alpha 0). Extract skips emitting
    /// a quad for a transparent fill.
    #[default]
    Transparent,
    /// A named theme token, e.g. `Token("color.surface.secondary")`.
    /// Resolves via `Theme::color(name)`; a miss is the magenta sentinel.
    Token(Cow<'static, str>),
    /// CSS `currentColor`: resolves to the inherited text color (v1 fallback
    /// = theme default foreground token). Carrier owned by
    /// `buiy-text-rendering-design`.
    CurrentColor,
    /// A CSS system-color keyword; under forced-colors the only set that
    /// resolves.
    SystemColor(SystemColorKeyword),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_token_default_is_transparent() {
        assert_eq!(ColorToken::default(), ColorToken::Transparent);
    }

    #[test]
    fn color_token_from_static_token_round_trips() {
        let t = ColorToken::Token(Cow::Borrowed("color.surface.secondary"));
        assert_eq!(t, ColorToken::Token(Cow::Borrowed("color.surface.secondary")));
        assert_ne!(t, ColorToken::Transparent);
    }

    #[test]
    fn system_color_keyword_default_is_canvas() {
        assert_eq!(SystemColorKeyword::default(), SystemColorKeyword::Canvas);
    }
}
```

- [ ] Run it and watch it FAIL to compile/build first if the module wiring is wrong, then pass. Command: `cargo test -p buiy_core --lib render::color` — expect the three `tests` to PASS once the file above compiles (this task's code *is* the implementation; the test is co-written, which is acceptable for pure type-shape tasks — the RED here is "module does not compile / not declared" before you add the `pub mod` line). If you want a strict RED first, add the `pub mod color;` line and the test module *before* the type definitions; `cargo test -p buiy_core --lib render::color` then fails with "cannot find type `ColorToken`".
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add ColorToken + SystemColorKeyword in render/color.rs (R1 component model)`.

---

## Task 2 — `Background` (create `render/components.rs`)

Solid color token (v1). Replaces `Visual.background_token`. Absent component == transparent. This task creates `render/components.rs` (the home of every render component except `ColorToken`/`SystemColorKeyword`, which live in `render/color.rs` from Task 1) and wires its module header, importing `ColorToken` from `render::color`.

**Files**
- Create: `crates/buiy_core/src/render/components.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod components;`)

Steps:

- [ ] Add `pub mod components;` to `crates/buiy_core/src/render/mod.rs` (next to the `pub mod color;` from Task 1).
- [ ] Create `crates/buiy_core/src/render/components.rs` with this header (it imports `ColorToken` from `render::color`; do NOT redefine `ColorToken`/`SystemColorKeyword` here):

```rust
//! Render-owned components (the layout↔render paint boundary).
//!
//! Replaces the temporary `crate::components::Visual`. Each author-set
//! component is a small public-fielded decomposed component deriving
//! `Reflect + Default + Clone + Component`; the computed components
//! (`ClipRect`, `AncestorClip`, `EffectGroup`) carry leaner derives (no
//! `Reflect`/`Default`) because they are render-prep outputs, never authored
//! or serialized. `ColorToken`/`SystemColorKeyword` live in the sibling
//! `render/color.rs` (color-and-forced-colors.md § 2.0 owns them).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md.

use bevy::prelude::*;
use crate::render::color::ColorToken;
```

- [ ] Add the failing test to the (new) `tests` module:

```rust
    #[test]
    fn background_default_is_transparent() {
        assert_eq!(Background::default().color, ColorToken::Transparent);
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL (`cannot find type Background`).
- [ ] Add the impl (above the `tests` module):

```rust
/// Solid background fill (v1). Replaces `Visual.background_token`.
/// Absent component == transparent.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 3.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Background {
    /// F: solid fill. `ColorToken::default()` (transparent) == no fill,
    /// matching `Visual.background_token == ""`. The reserved layered/
    /// gradient surface (`layers: Vec<BackgroundLayer>`, C-tier) lands with
    /// the gradient/image fast-follow — not a v1 field.
    pub color: ColorToken,
}
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add Background component (R1)`.

---

## Task 3 — `LineStyle`, `Radius`, `Corners`, `BorderSide`, `Border`

Per-side border paint + elliptical per-corner radius. Replaces `Visual.border_radius`. `LineStyle` reuses the shape of `ColumnRuleStyle` (in `layout/types.rs`) extended with the remaining CSS keywords; `Radius` holds two `Length` radii; `Corners` holds four `Radius`. Width is NOT here (it lives in `BoxModel.border` because it affects layout).

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn line_style_default_is_none() {
        assert_eq!(LineStyle::default(), LineStyle::None);
    }

    #[test]
    fn radius_default_is_zero_zero() {
        let r = Radius::default();
        assert_eq!(r.x, Length::ZERO);
        assert_eq!(r.y, Length::ZERO);
    }

    #[test]
    fn corners_zero_is_all_zero() {
        let c = Corners::ZERO;
        assert_eq!(c.top_left, Radius::default());
        assert_eq!(c.bottom_right, Radius::default());
        assert_eq!(Corners::default(), Corners::ZERO);
    }

    #[test]
    fn corners_all_sets_every_corner() {
        let r = Radius { x: Length::px(6.0), y: Length::px(6.0) };
        let c = Corners::all(r);
        assert_eq!(c.top_left, r);
        assert_eq!(c.top_right, r);
        assert_eq!(c.bottom_right, r);
        assert_eq!(c.bottom_left, r);
    }

    #[test]
    fn radius_circular_sets_x_and_y_equal() {
        let r = Radius::circular(6.0);
        assert_eq!(r.x, Length::px(6.0));
        assert_eq!(r.y, Length::px(6.0));
    }

    #[test]
    fn border_side_default_is_transparent_none() {
        let s = BorderSide::default();
        assert_eq!(s.color, ColorToken::Transparent);
        assert_eq!(s.style, LineStyle::None);
    }

    #[test]
    fn border_default_is_square_no_stroke() {
        let b = Border::default();
        assert_eq!(b.radius, Corners::ZERO);
        assert_eq!(b.top, BorderSide::default());
        assert_eq!(b.left.style, LineStyle::None);
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL (`cannot find type LineStyle`).
- [ ] Add `use crate::layout::types::Length;` to the module's imports (top of file).
- [ ] Add the impl:

```rust
/// Border / outline line style. Reuses the shape of `ColumnRuleStyle`
/// (layout/types.rs) extended with the remaining CSS keywords.
/// `Groove`/`Ridge`/`Inset`/`Outset` are C-tier and render as `Solid`
/// until the bevel shader lands.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

/// One corner's elliptical radii (CSS `border-radius` allows `rx / ry`).
/// Paint `Length`: only the resolvable subset (`Px`/`Percent`/`Cq*`)
/// applies; grid-only `Fr` is warned-and-resolved to `0`px downstream.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Radius {
    pub x: Length,
    pub y: Length,
}

impl Radius {
    /// A circular (`x == y`) radius of `px` logical pixels.
    pub const fn circular(px: f32) -> Self {
        Self { x: Length::px(px), y: Length::px(px) }
    }
}

/// Elliptical per-corner radius (x and y radii per corner).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct Corners {
    pub top_left: Radius,
    pub top_right: Radius,
    pub bottom_right: Radius,
    pub bottom_left: Radius,
}

impl Corners {
    /// All corners square (the default — matches `Visual.border_radius == 0`).
    pub const ZERO: Self = Self {
        top_left: Radius { x: Length::ZERO, y: Length::ZERO },
        top_right: Radius { x: Length::ZERO, y: Length::ZERO },
        bottom_right: Radius { x: Length::ZERO, y: Length::ZERO },
        bottom_left: Radius { x: Length::ZERO, y: Length::ZERO },
    };

    /// A uniform radius on all four corners.
    pub const fn all(r: Radius) -> Self {
        Self { top_left: r, top_right: r, bottom_right: r, bottom_left: r }
    }
}

/// One side's paint description. Width is NOT here — it lives in
/// `BoxModel.border` because it affects layout.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Debug)]
pub struct BorderSide {
    pub color: ColorToken,
    pub style: LineStyle,
}

/// Per-side border paint + elliptical per-corner radius. Replaces
/// `Visual.border_radius`. The border *band's* thickness is the layout
/// input `BoxModel.border`; this component paints into that band.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 4.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
    /// Elliptical per-corner radius. `Corners::ZERO` (default) == square
    /// corners, matching `Visual.border_radius == 0.0`. A uniform radius is
    /// `Corners::all(Radius::circular(px))`.
    pub radius: Corners,
}
```

Note: `BorderSide` is `Copy` but holds `ColorToken` which is NOT `Copy` (it holds a `Cow`/enum). Drop `Copy` from `BorderSide`, `Corners`, and `Radius` if `ColorToken` makes them non-`Copy` — verify by compile. `Radius`/`Corners` contain only `Length` (which is `Copy`) so they stay `Copy`; `BorderSide` holds `ColorToken` so it must be `Clone` not `Copy`. **Adjust the derives accordingly when the compiler rejects `Copy` on `BorderSide`** (expected): make `BorderSide` and `Border` `Clone` only. Keep `Radius`/`Corners` `Copy`.

- [ ] Run `cargo test -p buiy_core --lib render::components` — fix the `Copy` derive per the compiler, then expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add Border + BorderSide/Corners/Radius/LineStyle (R1)`.

---

## Task 4 — `Shadow` + `BoxShadow`

Ordered box-shadow list (multiple, inset, spread, blur, color). Index 0 paints on top. Empty/absent == no shadow.

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn box_shadow_default_is_empty() {
        assert!(BoxShadow::default().0.is_empty());
    }

    #[test]
    fn shadow_default_is_transparent_zero_outset() {
        let s = Shadow::default();
        assert_eq!(s.color, ColorToken::Transparent);
        assert_eq!(s.offset_x, Length::ZERO);
        assert_eq!(s.offset_y, Length::ZERO);
        assert_eq!(s.blur, Length::ZERO);
        assert_eq!(s.spread, Length::ZERO);
        assert!(!s.inset);
    }

    #[test]
    fn box_shadow_preserves_list_order() {
        let front = Shadow { offset_x: Length::px(1.0), ..Default::default() };
        let back = Shadow { offset_x: Length::px(2.0), ..Default::default() };
        let bs = BoxShadow(vec![front, back]);
        assert_eq!(bs.0[0].offset_x, Length::px(1.0));
        assert_eq!(bs.0[1].offset_x, Length::px(2.0));
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
/// One shadow term. Painted by the shadow primitive (architecture pillar 2).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 5.
#[derive(Reflect, Default, Clone, PartialEq, Debug)]
pub struct Shadow {
    pub color: ColorToken,
    pub offset_x: Length,
    pub offset_y: Length,
    /// CSS blur radius (>= 0).
    pub blur: Length,
    /// Grows (+) / shrinks (-) the shadow shape.
    pub spread: Length,
    /// `false` = outset (default), `true` = inner shadow.
    pub inset: bool,
}

/// Ordered box-shadow list. Index 0 paints on top (CSS paint order: first
/// shadow is frontmost). Empty / absent == no shadow.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 5.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BoxShadow(pub Vec<Shadow>);
```

(`Shadow` holds `ColorToken` so it is not `Copy` — leave it `Clone`.)

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add BoxShadow + Shadow (R1)`.

---

## Task 5 — `Opacity` (manual `Default = 1.0`)

Group opacity in `[0.0, 1.0]`. `1.0` (default) is a no-op. Manual `Default` because a derived `Default` over `f32` gives `0.0` (fully transparent) — the wrong CSS-initial value.

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn opacity_default_is_one_not_zero() {
        // The whole reason Opacity has a manual Default: a derived Default
        // over f32 would be 0.0 (fully transparent), the wrong CSS initial.
        assert_eq!(Opacity::default().0, 1.0);
    }

    #[test]
    fn opacity_is_copy() {
        let a = Opacity(0.5);
        let b = a;
        assert_eq!(a.0, b.0);
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
/// Group opacity in `[0.0, 1.0]`. `1.0` (default) is a no-op. A value
/// `< 1.0` forms an `EffectGroup` (off-screen composite boundary) and is a
/// future SC-trigger layout sub-pass 6f will read once the trigger-5 clause
/// lands. Absent == opaque.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 6.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Opacity(pub f32);

impl Default for Opacity {
    fn default() -> Self {
        Opacity(1.0)
    }
}
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add Opacity with manual Default 1.0 (R1)`.

---

## Task 6 — `Outline`

Focus / selection outline, painted outside the border box and never clipped by the element's own `ClipRect`. Absent == no outline.

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing test:

```rust
    #[test]
    fn outline_default_is_transparent_none_zero() {
        let o = Outline::default();
        assert_eq!(o.color, ColorToken::Transparent);
        assert_eq!(o.style, LineStyle::None);
        assert_eq!(o.width, Length::ZERO);
        assert_eq!(o.offset, Length::ZERO);
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
/// Focus / selection outline, painted OUTSIDE the border box and never
/// clipped by the element's own `ClipRect` (the render pass uses the
/// companion `AncestorClip`). Absent == no outline.
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

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add Outline (R1)`.

---

## Task 7 — `Angle` + `FilterFn` + reserved `Filter` / `BackdropFilter` / `MixBlendMode`

C-tier reserved effect components — components ship v1 (registered, reflectable, public-fielded) so layout 6f can wire the complete SC-trigger union later; their shaders are deferred. `Angle` and `FilterFn` are v1 unit prerequisites so the shipped `FilterFn` enum compiles.

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn angle_holds_radians() {
        assert_eq!(Angle(std::f32::consts::PI).0, std::f32::consts::PI);
    }

    #[test]
    fn filter_default_is_empty() {
        assert!(Filter::default().0.is_empty());
    }

    #[test]
    fn backdrop_filter_default_is_empty() {
        assert!(BackdropFilter::default().0.is_empty());
    }

    #[test]
    fn mix_blend_mode_default_is_normal() {
        assert_eq!(MixBlendMode::default(), MixBlendMode::Normal);
    }

    #[test]
    fn filter_fn_blur_carries_length() {
        let f = FilterFn::Blur(Length::px(4.0));
        assert_eq!(f, FilterFn::Blur(Length::px(4.0)));
        assert_ne!(f, FilterFn::Brightness(0.5));
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
/// Angle in radians. Minimal v1 stub so `FilterFn::HueRotate(Angle)`
/// compiles. The full CSS angle-unit family (deg/grad/turn, C-tier) lands
/// with the units fast-follow, which may re-home this type.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Reflect, Clone, Copy, PartialEq, Debug)]
pub struct Angle(pub f32);

/// Reserved filter-function value. Shapes ship now so authors can write
/// filter-aware code that compiles against v1; evaluation is deferred.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Reflect, Clone, PartialEq, Debug)]
pub enum FilterFn {
    Blur(Length),
    Brightness(f32),
    Contrast(f32),
    Grayscale(f32),
    Invert(f32),
    Opacity(f32),
    Saturate(f32),
    Sepia(f32),
    HueRotate(Angle),
    DropShadow(Shadow),
}

/// C (reserved). Filter function list. `EffectGroup` former in v1 and a
/// future SC-trigger (layout 6f reads it once its trigger-5 clause lands);
/// the filter shaders are deferred. Non-empty == forms an `EffectGroup`.
/// Empty / absent == no filter.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Filter(pub Vec<FilterFn>);

/// C (reserved). Backdrop filter list — samples what is BEHIND the element.
/// Forms an `EffectGroup` (compositor holds a backdrop copy). Buiy treats
/// `backdrop-filter` as an effect-group former ONLY (it does NOT form a
/// stacking context, so layout 6f does not read it). Backdrop-sampling
/// shader deferred.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct BackdropFilter(pub Vec<FilterFn>);

/// C (reserved). Blend mode against the backdrop. Any value other than
/// `Normal` forms an `EffectGroup` in v1 and is a future SC-trigger; the
/// blend shader is deferred. `Normal` (default) is a no-op.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum MixBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add reserved Filter/BackdropFilter/MixBlendMode + FilterFn/Angle (R1)`.

---

## Task 8 — `CssVisibility` + `OffscreenAuto`

`CssVisibility` is render-owned F-tier (`Visible`/`Hidden`/`Collapse`), deliberately NOT `bevy::prelude::Visibility`. `OffscreenAuto` is a zero-field layout-written marker whose shape render extract will read — defined here but NOT registered (layout owns its registration).

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn css_visibility_default_is_visible() {
        assert_eq!(CssVisibility::default(), CssVisibility::Visible);
    }

    #[test]
    fn css_visibility_has_hidden_and_collapse() {
        assert_ne!(CssVisibility::Hidden, CssVisibility::Collapse);
        assert_ne!(CssVisibility::Hidden, CssVisibility::Visible);
    }

    #[test]
    fn offscreen_auto_is_zero_field_marker() {
        let _m = OffscreenAuto;
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
/// CSS `visibility`. `Hidden` skips paint for this entity's subtree but
/// keeps its layout box and a11y presence (unlike `Display::None`).
/// `Collapse` is a deferred marker (table-row / flex-item collapse) —
/// named only in v1. Deliberately NOT `bevy::prelude::Visibility` (which
/// has different variants/semantics).
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 12.1.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Component, Default)]
pub enum CssVisibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

/// Zero-field marker placed by LAYOUT on entities whose
/// `Containment.content_visibility == Auto` subtree is currently off-screen.
/// Render's extract skips paint for an `OffscreenAuto` subtree. Layout-
/// written, render-read; NOT registered by this spec's render plugin
/// (layout owns its registration — README § 3.1). Defined here only so
/// render extract has the type to read.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 12.2.
#[derive(Component, Clone, Copy, Debug)]
pub struct OffscreenAuto;
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add CssVisibility + OffscreenAuto marker shape (R1)`.

---

## Task 9 — `ClipRect` + `AncestorClip` + reserved `ClipRadius` (computed; no `Reflect`/`Default`)

Computed clip AABBs `{ min: Vec2, max: Vec2 }` (logical px), written by `WriteClipRects` in a later phase (R2). This task only mints the type + a CPU `intersect` helper and a constructor; it wires no system. Leaner derives — no `Reflect`/`Default` (computed, never authored/serialized). `ClipRadius` is the reserved rounded-clip sibling (C-tier, not built).

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn clip_rect_intersect_is_aabb_overlap() {
        let a = ClipRect { min: Vec2::new(0.0, 0.0), max: Vec2::new(100.0, 100.0) };
        let b = ClipRect { min: Vec2::new(50.0, 25.0), max: Vec2::new(150.0, 75.0) };
        let c = a.intersect(&b);
        assert_eq!(c.min, Vec2::new(50.0, 25.0));
        assert_eq!(c.max, Vec2::new(100.0, 75.0));
    }

    #[test]
    fn clip_rect_intersect_is_commutative_on_overlap() {
        let a = ClipRect { min: Vec2::new(10.0, 10.0), max: Vec2::new(40.0, 40.0) };
        let b = ClipRect { min: Vec2::new(20.0, 0.0), max: Vec2::new(60.0, 30.0) };
        assert_eq!(a.intersect(&b).min, b.intersect(&a).min);
        assert_eq!(a.intersect(&b).max, b.intersect(&a).max);
    }

    #[test]
    fn ancestor_clip_holds_min_max() {
        let ac = AncestorClip { min: Vec2::ZERO, max: Vec2::splat(10.0) };
        assert_eq!(ac.max, Vec2::splat(10.0));
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
/// Computed clip AABB in logical px. Written by the `WriteClipRects`
/// render-prep pass (a later phase) and read by render (scissor) and
/// picking. NOT author-set or serialized — hence the leaner derives (no
/// `Reflect`/`Default`). Absent ClipRect ⇔ no ancestor clips this entity ⇒
/// render applies no scissor.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 9
/// (type fields + accumulation algorithm owned by clip-and-transform.md § A.2).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl ClipRect {
    /// AABB intersection of two clip rects (component-wise max of mins,
    /// min of maxes). Used by the accumulation pass to fold an ancestor
    /// clip into this entity's box.
    pub fn intersect(&self, other: &ClipRect) -> ClipRect {
        ClipRect {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        }
    }
}

/// Companion clip AABB holding the intersection of **ancestor** clip boxes
/// only (without the own-box step). Read by render for `Outline` painting
/// so a focus ring is cropped by ancestor clips but not by the element's
/// own clip. Written by `WriteClipRects` (a later phase). A plain `min`/`max`
/// struct (a DISTINCT type from `ClipRect`, NOT a newtype wrapper) per spec
/// clip-and-transform.md § A.2 + component-model.md § 13. NOT author-set.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 7 / § 9.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AncestorClip {
    pub min: Vec2,
    pub max: Vec2,
}

/// C (reserved). Rounded-clip corners — the sibling carrier for
/// rounded-corner clipping, not built in v1. The rounded-rect / `clip-path`
/// cases live here, NOT as a field on `ClipRect`.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 9.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct ClipRadius {
    pub corners: Corners,
}
```

Note: `ClipRadius` IS author-reserved per § 3.2 (C-tier reserved, listed as a render-owned row) — give it the standard author-set derives + register it in Task 11. `ClipRect`/`AncestorClip` are the computed exceptions and are NOT registered.

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add ClipRect/AncestorClip (computed) + reserved ClipRadius (R1)`.

---

## Task 10 — `EffectReason` bitflags + `EffectGroup` (computed; no `Reflect`/`Default`)

`EffectGroup` carries the OR of every reason that formed an off-screen compositing boundary. NO `Default` — it only exists when at least one reason holds. `EffectReason` is a `u8` bitflags (mirror the layout `ContainFlags` pattern, but `EffectReason` needs no `Reflect` since `EffectGroup` is unregistered).

**Files**
- Modify: `crates/buiy_core/src/render/components.rs`

Steps:

- [ ] Add the failing tests:

```rust
    #[test]
    fn effect_reason_bits_are_distinct() {
        assert_ne!(EffectReason::OPACITY, EffectReason::ISOLATION);
        assert_ne!(EffectReason::FILTER, EffectReason::BACKDROP_FILTER);
        assert_ne!(EffectReason::MIX_BLEND, EffectReason::OPACITY);
    }

    #[test]
    fn effect_reason_ors_combine() {
        let r = EffectReason::OPACITY | EffectReason::ISOLATION;
        assert!(r.contains(EffectReason::OPACITY));
        assert!(r.contains(EffectReason::ISOLATION));
        assert!(!r.contains(EffectReason::FILTER));
    }

    #[test]
    fn effect_group_carries_reason() {
        let g = EffectGroup { reason: EffectReason::OPACITY | EffectReason::FILTER };
        assert!(g.reason.contains(EffectReason::OPACITY));
        assert!(g.reason.contains(EffectReason::FILTER));
    }
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect FAIL.
- [ ] Add the impl:

```rust
bitflags::bitflags! {
    /// Which effect(s) caused an entity to form an off-screen compositing
    /// boundary. One entity can carry several at once (opacity<1 AND isolate).
    ///
    /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct EffectReason: u8 {
        /// v1: carried (group opacity).
        const OPACITY         = 1;
        /// v1: carried (`isolation: isolate`).
        const ISOLATION       = 2;
        /// Reserved: marks the group, no shader in v1.
        const FILTER          = 4;
        /// Reserved: marks the group, needs backdrop sample.
        const BACKDROP_FILTER = 8;
        /// Reserved: marks the group, no shader in v1.
        const MIX_BLEND       = 16;
    }
}

/// This entity establishes an off-screen compositing boundary. Written by
/// the render-prep pass that detects an effect-group former (a later phase;
/// canonical predicate owned by effect-compositor.md § 1), removed when none
/// holds. Read by the compositor to choose the composite op without
/// re-querying the five effect components. NOT author-set; NO `Default` (an
/// `EffectGroup` only exists when at least one reason holds). Absence == no
/// group.
///
/// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
#[derive(Component, Clone, Copy, Debug)]
pub struct EffectGroup {
    /// The OR of every reason that formed this group.
    pub reason: EffectReason,
}
```

- [ ] Run `cargo test -p buiy_core --lib render::components` — expect PASS.
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): add EffectReason bitflags + EffectGroup (computed) (R1)`.

---

## Task 11 — Register the author-set types + re-export; registration-coverage test

Register every author-set render component in `BuiyRenderPlugin::build` **before** the RenderApp early-return (so it runs in the main world, headless). Do NOT register the computed `ClipRect`/`AncestorClip`/`EffectGroup`/`EffectReason` or the layout-owned `OffscreenAuto`. Re-export the public types from `render/mod.rs` and `lib.rs`. Add an integration test asserting registry membership.

The author-set set to register (every component carrying `#[reflect(Component, Default)]`): `Background`, `Border`, `BoxShadow`, `Opacity`, `Outline`, `Filter`, `BackdropFilter`, `MixBlendMode`, `CssVisibility`, `ClipRadius`. Also register the **value types** that author-set components contain so reflection/BSN can resolve their fields, matching how `layout/mod.rs` registers `Length`/`Edges`/etc.: `ColorToken`, `SystemColorKeyword`, `LineStyle`, `Radius`, `Corners`, `BorderSide`, `Shadow`, `FilterFn`, `Angle`.

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs` (registration + re-exports)
- Modify: `crates/buiy_core/src/lib.rs` (crate-root re-exports)
- Create: `crates/buiy_core/tests/render_components_registry.rs`

Steps:

- [ ] Write the failing integration test `crates/buiy_core/tests/render_components_registry.rs`:

```rust
//! Headless (no GPU): asserts every author-set render component is
//! `register_type`'d by `BuiyRenderPlugin::build` (the registration runs in
//! the main world, before the RenderApp branch, so this works under
//! MinimalPlugins with no wgpu adapter). The computed components
//! (`ClipRect`, `AncestorClip`, `EffectGroup`) and the layout-owned
//! `OffscreenAuto` are deliberately NOT registered and are asserted absent.

use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use buiy_core::CorePlugin;
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::components::{
    AncestorClip, Background, BackdropFilter, Border, BoxShadow, ClipRadius, ClipRect, ColorToken,
    CssVisibility, EffectGroup, Filter, MixBlendMode, OffscreenAuto, Opacity, Outline,
};

fn registry(app: &App) -> &TypeRegistry {
    // AppTypeRegistry wraps a read-write TypeRegistry; read it once for the
    // assertions below.
    app.world().resource::<AppTypeRegistry>().read_arc_handle();
    unreachable!()
}

#[test]
fn author_set_render_components_are_registered() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let reg = type_registry.read();

    assert!(reg.get(std::any::TypeId::of::<Background>()).is_some(), "Background");
    assert!(reg.get(std::any::TypeId::of::<Border>()).is_some(), "Border");
    assert!(reg.get(std::any::TypeId::of::<BoxShadow>()).is_some(), "BoxShadow");
    assert!(reg.get(std::any::TypeId::of::<Opacity>()).is_some(), "Opacity");
    assert!(reg.get(std::any::TypeId::of::<Outline>()).is_some(), "Outline");
    assert!(reg.get(std::any::TypeId::of::<Filter>()).is_some(), "Filter");
    assert!(reg.get(std::any::TypeId::of::<BackdropFilter>()).is_some(), "BackdropFilter");
    assert!(reg.get(std::any::TypeId::of::<MixBlendMode>()).is_some(), "MixBlendMode");
    assert!(reg.get(std::any::TypeId::of::<CssVisibility>()).is_some(), "CssVisibility");
    assert!(reg.get(std::any::TypeId::of::<ClipRadius>()).is_some(), "ClipRadius");
    assert!(reg.get(std::any::TypeId::of::<ColorToken>()).is_some(), "ColorToken");
}

#[test]
fn computed_and_layout_owned_components_are_not_registered_here() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let reg = type_registry.read();

    // ClipRect / AncestorClip / EffectGroup are computed (no Reflect derive,
    // so they cannot be in the registry); OffscreenAuto is layout-owned.
    assert!(reg.get(std::any::TypeId::of::<ClipRect>()).is_none(), "ClipRect must not be registered here");
    assert!(reg.get(std::any::TypeId::of::<AncestorClip>()).is_none(), "AncestorClip must not be registered here");
    assert!(reg.get(std::any::TypeId::of::<EffectGroup>()).is_none(), "EffectGroup must not be registered here");
    assert!(reg.get(std::any::TypeId::of::<OffscreenAuto>()).is_none(), "OffscreenAuto is layout-owned");
}
```

Remove the bogus `registry` helper / `read_arc_handle` placeholder — it is a deliberate stub to force you to use the real API. The working pattern is the `let type_registry = app.world().resource::<AppTypeRegistry>().clone(); let reg = type_registry.read();` two-liner shown in the test bodies. Verify against the layout registry test if one exists (`rg "AppTypeRegistry" crates/buiy_core/tests`); if the read API differs in this Bevy version, mirror whatever the layout side uses. Keep `bevy::reflect::TypeRegistry` import only if needed.

- [ ] Run `cargo test -p buiy_core --test render_components_registry` — expect FAIL (`cannot find ... components::Background` until re-exported, and registration absent).
- [ ] Add `pub mod components;` re-exports to `render/mod.rs` — make the new module's public types reachable. At the top of `render/mod.rs`, the existing `pub mod components;` (from Task 1) already exposes `buiy_core::render::components::*`. Add a convenience re-export block in `render/mod.rs`:

```rust
pub use components::{
    AncestorClip, Angle, Background, BackdropFilter, Border, BorderSide, BoxShadow, ClipRadius,
    ClipRect, ColorToken, Corners, CssVisibility, EffectGroup, EffectReason, Filter, FilterFn,
    LineStyle, MixBlendMode, OffscreenAuto, Opacity, Outline, Radius, Shadow, SystemColorKeyword,
};
```

- [ ] Register the author-set types in `BuiyRenderPlugin::build`, BEFORE the `let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return; };` line:

```rust
impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        // Register author-set render components (reflection / BSN / inspectors)
        // in the MAIN world, before the RenderApp branch, so registration
        // happens even on headless hosts with no RenderApp (component-model.md
        // § 13). The computed ClipRect/AncestorClip/EffectGroup and the
        // layout-owned OffscreenAuto are deliberately NOT registered here.
        app.register_type::<components::Background>()
            .register_type::<components::Border>()
            .register_type::<components::BorderSide>()
            .register_type::<components::Corners>()
            .register_type::<components::Radius>()
            .register_type::<components::LineStyle>()
            .register_type::<components::BoxShadow>()
            .register_type::<components::Shadow>()
            .register_type::<components::Opacity>()
            .register_type::<components::Outline>()
            .register_type::<components::Filter>()
            .register_type::<components::BackdropFilter>()
            .register_type::<components::MixBlendMode>()
            .register_type::<components::FilterFn>()
            .register_type::<components::Angle>()
            .register_type::<components::CssVisibility>()
            .register_type::<components::ClipRadius>()
            .register_type::<components::ColorToken>()
            .register_type::<components::SystemColorKeyword>();

        // ExtractedDraws is render-world only — the main world does not read it.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<ExtractedDraws>()
            .add_systems(ExtractSchedule, extract_buiy_draws);
        node::register(render_app);
        pipeline::register(render_app);
    }
}
```

- [ ] Run `cargo test -p buiy_core --test render_components_registry` — expect PASS. If `register_type` on a value type that itself contains an unregistered generic complains, register the contained type too; do not silence with `#[reflect(no_field_bounds)]` unless the compiler demands it for a `Vec`/recursive type (the layout side used it only for `TrackSize`).
- [ ] Add crate-root re-exports in `crates/buiy_core/src/lib.rs`. Extend the existing `pub use components::{...}` is for crate-root `components.rs`; the render components are a different module — add a new line:

```rust
pub use render::components::{
    Background, BackdropFilter, Border, BorderSide, BoxShadow, ClipRadius, ClipRect, ColorToken,
    Corners, CssVisibility, EffectGroup, EffectReason, Filter, FilterFn, LineStyle, MixBlendMode,
    Opacity, Outline, Radius, Shadow, SystemColorKeyword,
};
```

(Do not crate-root-export `AncestorClip`/`OffscreenAuto`/`Angle` unless a later phase needs them publicly — keep the root surface minimal; they remain reachable via `render::components`.)

- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `feat(render): register author-set render components + re-export (R1)`.

---

## Task 12 — Migrate the render extract onto `Background` + `Border`

Replace the `Visual` read in `extract_buiy_draws` with a `Background` + `Border` read, resolving `Background.color` (a `ColorToken`) through the same `Theme::color` path and the same magenta-sentinel-on-miss behaviour, and deriving the Phase-0 uniform `radius: f32` from `Border.radius`. Behaviour must be byte-identical to Phase 0 for the button fixture (`color.surface.secondary` token, radius 6.0).

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs` (`extract_buiy_draws` query + body)
- Create: `crates/buiy_core/tests/render_extract_background.rs`

Steps:

- [ ] Write a failing headless test that drives the extract path via a `ColorToken` resolution helper. Because `extract_buiy_draws` only runs inside `ExtractSchedule` (render world), test the **token→Color resolution** seam as a pure function so it is GPU-free. Add a small `pub(crate)` helper `resolve_token` to `render/mod.rs` and test it. Create `crates/buiy_core/tests/render_extract_background.rs`:

```rust
//! Headless (no GPU): the Background `ColorToken` resolves to the same
//! `Color` the Phase-0 `Visual.background_token` string did, and a miss
//! still yields the magenta sentinel. Pure resolution; no RenderApp.

use bevy::prelude::*;
use buiy_core::render::components::ColorToken;
use buiy_core::render::resolve_token;
use buiy_core::theme::Theme;
use std::borrow::Cow;

fn theme_with(token: &str, color: Color) -> Theme {
    let mut t = Theme::default();
    t.colors.insert(token.to_string(), color);
    t
}

#[test]
fn token_resolves_to_theme_color() {
    let theme = theme_with("color.surface.secondary", Color::srgb(0.2, 0.3, 0.4));
    let tok = ColorToken::Token(Cow::Borrowed("color.surface.secondary"));
    let (color, missed) = resolve_token(&tok, &theme);
    assert_eq!(color, Color::srgb(0.2, 0.3, 0.4));
    assert!(!missed);
}

#[test]
fn missing_token_falls_back_to_magenta_sentinel() {
    let theme = Theme::default();
    let tok = ColorToken::Token(Cow::Borrowed("nope.not.here"));
    let (color, missed) = resolve_token(&tok, &theme);
    assert_eq!(color, Color::srgb(1.0, 0.0, 1.0));
    assert!(missed);
}

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    let (color, missed) = resolve_token(&ColorToken::Transparent, &theme);
    assert_eq!(color, Color::NONE);
    assert!(!missed);
}
```

- [ ] Run `cargo test -p buiy_core --test render_extract_background` — expect FAIL (`cannot find resolve_token`).
- [ ] In `render/mod.rs`, add the resolver and rewrite `extract_buiy_draws`. Add `use crate::render::components::{Background, Border, ColorToken};` to the imports and remove `Visual` from the `use crate::components::{...}` import. Implement:

```rust
/// Resolve a [`ColorToken`] against the active theme. Returns the resolved
/// `Color` and whether a named token *missed* (so the caller can emit one
/// `warn!`). Mirrors the Phase-0 `Visual.background_token` resolution:
/// `Transparent` → `Color::NONE`; `Token(name)` → `Theme::color(name)` or
/// the magenta sentinel on miss; `CurrentColor` / `SystemColor(_)` use the
/// v1 fallback (theme default foreground / system-color map — owned by
/// color-and-forced-colors.md; here they route through `Theme::color` of
/// the fallback token and sentinel-on-miss like any other token).
pub(crate) fn resolve_token(token: &ColorToken, theme: &Theme) -> (Color, bool) {
    match token {
        ColorToken::Transparent => (Color::NONE, false),
        ColorToken::Token(name) => match theme.color(name) {
            Some(c) => (c, false),
            None => (MISSING_TOKEN_FALLBACK, true),
        },
        // v1 fallback: currentColor → theme default foreground token
        // (color-and-forced-colors.md § 2.0); a miss is sentinel + warn.
        ColorToken::CurrentColor => match theme.color("color.text.primary") {
            Some(c) => (c, false),
            None => (MISSING_TOKEN_FALLBACK, true),
        },
        // v1 fallback: system-color keywords resolve via the active theme's
        // system-color map (owned by buiy-theme-tokens-design); until that
        // map lands a lookup misses → sentinel + warn.
        ColorToken::SystemColor(_) => (MISSING_TOKEN_FALLBACK, true),
    }
}
```

Then rewrite `extract_buiy_draws`:

```rust
fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<Query<(&Background, Option<&Border>, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
    main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    let mut draws = ExtractedDraws::default();
    if let Ok(window) = main_world_windows.single() {
        let res = window.resolution.size();
        draws.window_size = Vec2::new(res.x, res.y);
    }
    for (background, border, layout) in main_world_q.iter() {
        let (color, missed) = resolve_token(&background.color, &main_world_theme);
        if missed {
            tracing::warn!(
                token = ?background.color,
                "missing theme color token; falling back to magenta sentinel"
            );
        }
        // Phase-0 parity: skip emitting a quad for a transparent fill
        // (Background absent OR ColorToken::Transparent), matching the
        // old empty-string skip.
        if color == Color::NONE {
            continue;
        }
        // Phase-0 used a single f32 radius. Derive it from the uniform
        // top-left corner's x radius for parity; the full elliptical/per-
        // corner radius lands when the rounded-rect primitive gains it
        // (a later R-phase). A `Border`-less entity is square (radius 0).
        let radius = border
            .and_then(|b| uniform_radius_px(&b.radius))
            .unwrap_or(0.0);
        draws.draws.push(DrawData::new(layout.position, layout.size, color, radius));
    }
    commands.insert_resource(draws);
}

/// Phase-0 parity helper: the uniform corner radius in logical px, read from
/// the top-left corner's x radius when every corner is the same circular
/// radius. Returns `None` if the radii are not a single uniform circular
/// value (the elliptical / per-corner cases land with the rounded-rect
/// primitive upgrade). Only `Length::Px` resolves here; other units resolve
/// to 0 for now (paint-`Length` resolution is a later-phase concern).
fn uniform_radius_px(corners: &crate::render::components::Corners) -> Option<f32> {
    use crate::layout::types::Length;
    let r = corners.top_left;
    let px = match r.x {
        Length::Px(v) => v,
        _ => return Some(0.0),
    };
    let _ = r.y; // x==y for a circular radius; ignore the y term in Phase-0 parity.
    Some(px)
}
```

Drop `MISSING_TOKEN_FALLBACK`'s now-unused direct use only if the compiler flags it — it is still used inside `resolve_token`, so keep it. Keep the `commands.insert_resource(draws)` line and the per-frame TODO comment.

- [ ] Run `cargo test -p buiy_core --test render_extract_background` — expect PASS.
- [ ] Run `cargo test -p buiy_core --test render_instance` and `--test render_smoke` — expect PASS (no behavioural change to `to_instance`; `render_plugin_loads_without_panic` still loads the plugin under `MinimalPlugins`; the two GPU tests stay `#[ignore]`d).
- [ ] Run the full gate. Expect PASS.
- [ ] Commit: `refactor(render): extract reads Background + Border instead of Visual (R1)`.

---

## Task 13 — Migrate `button.rs` and `hello_button` off `Visual`; delete `Visual`

Replace the button's `Visual { background_token, foreground_token, border_radius }` with `Background { color: ColorToken::Token("color.surface.secondary") }` + `Border { radius: Corners::all(Radius::circular(6.0)), .. }`. Then delete `Visual` from `components.rs` and drop it from every re-export (`lib.rs`, `buiy/src/lib.rs`) and the `CorePlugin` `register_type::<Visual>()`. `foreground_token` is dropped (moves to `buiy-text-rendering-design` per spec § 11); the Phase-0 `Stroke` placeholder is subsumed by `Border` (no new type).

**Files**
- Modify: `crates/buiy_widgets/src/button.rs`
- Modify: `examples/hello_button/src/main.rs` (only if it constructs `Visual` directly — it does not; it uses `Button::new`, so likely no change beyond a compile check)
- Modify: `crates/buiy_core/src/components.rs` (delete `Visual` + its test references)
- Modify: `crates/buiy_core/tests/components.rs` (delete the `Visual`-registered assertion + rename the test — see step below; verified: line 24 asserts `registry.get::<Visual>().is_some()` and line 6 names the test `..._and_visual_...`, both go red the moment `Visual` is deleted)
- Modify: `crates/buiy_core/src/lib.rs` (drop `Visual` from `pub use components::{...}` and from `register_type::<Visual>()`)
- Modify: `crates/buiy/src/lib.rs` (drop `Visual` from the re-export)

Steps:

- [ ] Update `button.rs`. Change the import `use buiy_core::{ ... components::{Node, Visual}, ... }` to `use buiy_core::{ ... components::Node, render::components::{Background, Border, ColorToken, Corners, Radius}, ... }` (or pull from the crate root if re-exported there: `Background, Border, ColorToken, Corners, Radius`). Replace the bundle element:

```rust
            Background {
                color: ColorToken::Token(std::borrow::Cow::Borrowed("color.surface.secondary")),
            },
            Border {
                radius: Corners::all(Radius::circular(6.0)), // matches "radius.md"
                ..Default::default()
            },
```

Remove the `foreground_token` concern entirely (the text color is owned by the text spec). Keep the `// TODO(buiy-widget-catalog-design)` size comment.

- [ ] Run `cargo test -p buiy_widgets` — the existing button tests must still pass. If a button test asserted on `Visual` fields, retarget it to `Background`/`Border`. Run `rg "Visual" crates/buiy_widgets` to find any.
- [ ] Delete `Visual` from `crates/buiy_core/src/components.rs`: remove the `Visual` struct + its doc comment. Update the module doc-comment's "the temporary `Visual` component" sentence to past tense ("…replaced by render-side `Background`/`Border` in the render-pipeline spec"). There are no `Visual`-specific tests in that file's `#[cfg(test)]` block (verify with `rg "Visual" crates/buiy_core/src/components.rs`).
- [ ] In `crates/buiy_core/src/lib.rs`: remove `Visual` from `pub use components::{Node, ResolvedLayout, ResolvedTransform, StackingContext, Visual};` and remove the `.register_type::<Visual>()` line from `CorePlugin::build`.
- [ ] In `crates/buiy/src/lib.rs`: remove `Visual` from the `components::{Node, ResolvedLayout, ResolvedTransform, StackingContext, Visual}` re-export.
- [ ] Run `rg "Visual" crates examples` — expect ZERO remaining references (other than the spec's migration prose in `docs/`). Fix any stragglers.
- [ ] Run `cargo test -p buiy_core --test render_smoke` and the full workspace test — `render_plugin_loads_without_panic` and the existing button / hello_button tests must stay green. The `hello_button` e2e (if `#[ignore]`d for GPU) is unaffected.
- [ ] Run the full gate. Expect PASS. (Watch for `RUSTDOCFLAGS="-D warnings" cargo doc` failing on a dangling intra-doc link to `Visual` — grep doc comments for `[\`Visual\`]` / `Visual` links and retarget to `Background`/`Border`.)
- [ ] Commit: `refactor!(render): delete Visual; button + extract use Background/Border (R1)`.

---

## Task 14 — Docs: mark the plan landed + update the docs index

Reflect the completed migration in the docs system (per the "update docs as part of the deliverable" rule). The spec children are already drafted; this task only records that the component-model phase has an executed plan.

**Files**
- Modify: `docs/README.md` (add the plan to the catalog under the render area / plans grouping, with `[landed]` once green)

Steps:

- [ ] Add a catalog entry for this plan near the render spec row in `docs/README.md`, mirroring the existing plan-entry format:

```
- [Buiy render R1 — component model](plans/2026-06-03-buiy-render-r1-component-model.md) — replaces the temporary `Visual` with the render-side component model (`Background`/`Border`/`BoxShadow`/`Opacity`/`Outline`/`CssVisibility`), the reserved effect components (`Filter`/`BackdropFilter`/`MixBlendMode`), the computed `ClipRect`/`AncestorClip`/`EffectGroup`, and `ColorToken`; registers the author-set types; migrates the Phase-0 extract + button onto `Background`/`Border`. Unblocks the layout 6f SC-former follow-up (the reserved `Opacity`/`Filter`/`MixBlendMode` now exist). `[landed]`
```

Use the `organizing-buiy-docs` skill conventions for placement (the render spec sits in the layout/render area of the catalog; put the plan alongside it).

- [ ] Run the full gate one final time. Expect PASS.
- [ ] Commit: `docs(render): record R1 component-model plan as landed`.

---

## Done-criteria (verify before claiming complete)

- [ ] `rg "Visual" crates examples` returns nothing (spec prose in `docs/` may still mention it as the migrated-from type — that is correct).
- [ ] `cargo test -p buiy_core --test render_components_registry` proves every author-set type is registered and the computed/layout-owned ones are not.
- [ ] `render_smoke::render_plugin_loads_without_panic`, `render_instance`, and the button/hello_button tests are green; the two GPU tests in `render_smoke.rs` remain `#[ignore]`d (unchanged).
- [ ] `Opacity::default().0 == 1.0`, `Background::default()` is transparent, `Border::default()` is square + no stroke, `BoxShadow::default()` is empty, `MixBlendMode::default() == Normal`, `CssVisibility::default() == Visible` — all asserted by the in-module tests.
- [ ] The full gate is green at every commit (the host has no xvfb / no wgpu adapter; every test added here is headless).

## Cross-phase dependencies assumed (state explicitly for the executor)

1. **`ClipRect` / `AncestorClip` are written by a later phase (R2 / `WriteClipRects`).** This plan mints the type + a CPU `intersect` helper only; it wires no system that writes them. Do not add a `WriteClipRects` pass here.
2. **`OffscreenAuto` is layout-written.** Defined here only so render extract can later read it; NOT registered, NOT emitted by this plan.
3. **`SystemColorKeyword` + `Angle` are minted here as v1 unit prerequisites** (the spec assigns their *resolution* / full unit surface to color-and-forced-colors.md / the units fast-follow; the enums must exist for `ColorToken` / `FilterFn` to compile).
4. **This phase unblocks the layout 6f SC-former follow-up** (the reserved `Opacity`/`Filter`/`MixBlendMode` now exist for layout to read) but does **not** wire layout 6f — that is a separate layout follow-up.
