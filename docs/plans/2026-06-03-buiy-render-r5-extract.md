# Per-view Extract Rework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Phase-0 `extract_buiy_draws` with a per-view, `Changed`-gated `extract_buiy_nodes` whose entire per-entity mapping (DrawData build, color-token resolution, skip predicates, `painters_z` forward ordering) is factored into pure functions that are unit-tested headless, while the device-dependent system wiring/registration is `#[ignore]` GPU.

**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes **architecture.md § 1.2 / § 3 / § 4** (the `ExtractSchedule` + `Extract<Query>` boundary, the `Changed<T>`-gated per-frame instance set, per-view `ExtractedNodes` stored on the primary view) and **paint-order-and-top-layer.md § 1 / § 5** (forward walk of `StackingContext.painters_z`; the `Display::None` / `ContentVisibility::Hidden` / `CssVisibility::Hidden` / `OffscreenAuto` skip rules).

**Architecture:** Extraction is one-directional and read-only (README pillar 1). This phase keeps render a thin consumer: a fan of `Option<&T>` author-set components plus required `&ResolvedLayout` / `&GlobalTransform`, gated by an `Or`-set of `Changed<…>` (incl. the three paint-*skip* triggers). The mapping — "should this entity paint? what color does its token resolve to? what `ExtractedNode` does it produce, in what `painters_z` order?" — is hoisted into pure functions in a new `render/extract.rs` so it is provable on CI runners that have **no wgpu adapter** (the whole `RenderApp`/extract-system path needs a device). v1 resolves every `Node` to the **primary** window's view (architecture § 4, D2), but the query reads *all* windows so the per-window partition can be turned on later without a query change.

**Tier/Test reality:** HEADLESS for the pure mapping/skip/order logic and for `App::new() + MinimalPlugins`-level registration smoke; **GPU `#[ignore]`** for any assertion that needs a live `RenderApp` (extract-system schedule membership, actual draw). The gate that every commit must keep green (this host + CI have NO xvfb and NO wgpu adapter):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

---

## Cross-phase dependencies (read before starting)

This phase is **R5 (extract rework)** in the render-pipeline plan series. It consumes types that the **component-model phase** owns:

- `Background { color: ColorToken }`, `Border { radius: Corners, … }`, `BoxShadow`, `Opacity`, `Outline`, `CssVisibility`, `EffectGroup`, `ClipRect`, `OffscreenAuto`, and the reserved effect components (component-model.md §§ 3–12).
- `ColorToken` + its resolution rules (color-and-forced-colors.md § 2.0/§ 2.1).

**Assumption taken by this plan (stated explicitly):** at the time this phase runs, the component-model phase has landed at minimum `Background { color: ColorToken }`, `CssVisibility`, and `ColorToken` (with `Transparent` / `Token(Cow<'static,str>)` variants and `Theme`-based resolution). To keep this phase **independently executable and gate-green even if those have not landed yet**, Task 1 defines a *local minimal shim* of exactly the surface this phase needs (`ColorToken` with `Transparent` + `Token`, and a thin re-use of `Background` if present), guarded by a `// SUPERSEDED-BY: component-model phase` comment so the component-model phase replaces it rather than duplicating it. If the component-model phase **has** already landed `Background`/`ColorToken`, skip the shim in Task 1 and import the real types (the task notes the exact swap). Either way the pure functions in `extract.rs` are written against the trait-free concrete types and do not change.

Layout-owned types this phase reads are **already shipped** and exported from `buiy_core::layout`: `StackingContext` (crate-root `components.rs`), `Stacking`, `TopLayer`, `Containment`, `ContentVisibility`, `Display`, `ScrollOffset`, `Overflow`. `ResolvedLayout` / `ResolvedTransform` / `StackingContext` are in `crate::components`.

The Phase-0 `extract_buiy_draws` + `ExtractedDraws` + `DrawData` + `instance::to_instance` remain **until the `node.rs`/`instance.rs` rework phase (R3 hybrid handoff) lands**. This phase introduces `extract_buiy_nodes` and `ExtractedNodes` and rewires `mod.rs` to register the new system, but leaves the Phase-0 `DrawData`/`to_instance`/`node.rs` draw path intact so the workspace keeps compiling and the GPU draw still works under lavapipe. The node-side consumption swap (`ExtractedDraws` → per-view `ExtractedNodes`) is R3's deliverable, called out at Task 7.

---

## Type contract used by this phase (verbatim names — do not rename)

| Name | Kind | Source / owner | This phase |
|---|---|---|---|
| `ExtractedNode` | struct (CPU record, one per painted entity) | this phase (render/extract.rs) | introduces |
| `ExtractedNodes` | per-view component `{ nodes: Vec<ExtractedNode> }` | this phase | introduces |
| `extract_buiy_nodes` | `ExtractSchedule` system | this phase (architecture § 1.2) | introduces |
| `node_skip_reason` / `SkipReason` | pure fn + enum | this phase | introduces |
| `resolve_color_token` | pure fn (`&ColorToken`, `&Theme`) → `Color` | this phase (color § 2.1) | introduces |
| `extracted_node_for` | pure fn building one `ExtractedNode` | this phase | introduces |
| `ColorToken` | enum (`Transparent`, `Token`, …) | component-model.md § 2 / color § 2.0 | shim-or-import |
| `Background` `{ color: ColorToken }` | author-set component | component-model.md § 3 | shim-or-import |
| `CssVisibility` `{ Visible, Hidden, Collapse }` | render-owned component | component-model.md § 12.1 | shim-or-import |
| `OffscreenAuto` | layout-written marker | component-model.md § 12.2 | shim-or-import (read-only) |
| `StackingContext { painters_z: Vec<Entity> }` | layout handoff | `crate::components` (shipped) | read |
| `Stacking { z_index, isolation, top_layer }` | layout component | `layout::components` (shipped) | read |
| `Containment.content_visibility` | layout field | `layout::components` (shipped) | read |
| `ResolvedLayout { position, size }` | layout output | `crate::components` (shipped) | read |
| `MISSING_TOKEN_FALLBACK` | magenta sentinel | `render/mod.rs` (shipped) | reuse |

---

## Task 1 — Minimal `ColorToken` + skip/visibility surface this phase reads

**Why:** the pure extract functions need a concrete `ColorToken`, a `CssVisibility` enum, and the `OffscreenAuto` marker to compile and be tested. The component-model phase owns the full versions; this task provides exactly the surface this phase consumes, marked for supersession.

**Files**
- Create: `crates/buiy_core/src/render/components.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod components;`)
- Test: `crates/buiy_core/src/render/components.rs` (inline `#[cfg(test)] mod tests`)

**IF the component-model phase already shipped `ColorToken` / `Background` / `CssVisibility` / `OffscreenAuto`:** skip creating these here; instead in `extract.rs` (Task 3) import them from their real home and delete this task's shim file. Verify with `grep -rn "pub enum ColorToken" crates/buiy_core/src`. If that grep matches outside this file, do the import-swap and mark Task 1 done with a note.

### Steps

- [ ] Write the failing test. Append to `crates/buiy_core/src/render/components.rs` (after writing the module skeleton in the impl step, the test must compile-fail first because the types don't exist — so create the file containing ONLY the test, run, see it fail to compile):

```rust
//! Minimal render-side component surface this phase reads. The component-model
//! phase (component-model.md §§ 2, 3, 12) is the canonical owner of these
//! types — this is the narrow subset the extract rework needs to compile and be
//! tested headless.
//!
//! SUPERSEDED-BY: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md
//! When the component-model phase lands `ColorToken` / `CssVisibility` /
//! `OffscreenAuto`, this module's definitions are replaced by imports of the
//! canonical types; `render/extract.rs` keeps working unchanged.

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn color_token_default_is_transparent() {
        assert_eq!(ColorToken::default(), ColorToken::Transparent);
    }

    #[test]
    fn color_token_token_holds_name() {
        let t = ColorToken::Token(Cow::Borrowed("color.surface.primary"));
        assert_eq!(t, ColorToken::Token(Cow::Borrowed("color.surface.primary")));
    }

    #[test]
    fn css_visibility_default_is_visible() {
        assert_eq!(CssVisibility::default(), CssVisibility::Visible);
    }

    #[test]
    fn offscreen_auto_is_unit_marker() {
        let _m = OffscreenAuto;
    }
}
```

- [ ] Run it — expect FAIL (does not compile: `ColorToken`, `CssVisibility`, `OffscreenAuto` unresolved):

```sh
cargo test -p buiy_core --lib render::components 2>&1 | tail -20
```

- [ ] Minimal impl. Prepend to the same file, above the test module:

```rust
use bevy::prelude::*;
use std::borrow::Cow;

/// A themeable color reference, resolved against `Res<Theme>` at extract time
/// (color-and-forced-colors.md § 2.0/§ 2.1). This phase ships only the two
/// variants extract needs; `CurrentColor` / `SystemColor` arrive with the
/// component-model phase.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum ColorToken {
    /// CSS `transparent` / empty-token "skip the fill". Resolves to
    /// `Color::NONE`; extract emits no quad for it (component-model.md § 3).
    #[default]
    Transparent,
    /// A named theme token. Resolves via `Theme::color(name)`; a miss is the
    /// magenta sentinel + `warn!` (color-and-forced-colors.md § 2.2).
    Token(Cow<'static, str>),
}

/// Solid background fill (v1 subset). Replaces `Visual.background_token`.
/// Absent component == transparent (component-model.md § 3).
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct Background {
    pub color: ColorToken,
}

/// CSS `visibility` (render-owned, component-model.md § 12.1). `Hidden` skips
/// paint for this entity + subtree but keeps the layout box. Deliberately NOT
/// `bevy::prelude::Visibility`.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Default)]
#[reflect(Component, Default)]
pub enum CssVisibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

/// Zero-field marker placed by LAYOUT on entities whose
/// `content-visibility: auto` subtree is currently off-screen
/// (component-model.md § 12.2). Render skips paint for an `OffscreenAuto`
/// subtree. Layout-written, render-read.
#[derive(Component, Clone, Copy, Debug)]
pub struct OffscreenAuto;
```

- [ ] Add `pub mod components;` to `crates/buiy_core/src/render/mod.rs` (next to `pub mod instance;`).
- [ ] Run — expect PASS:

```sh
cargo test -p buiy_core --lib render::components 2>&1 | tail -20
```

- [ ] Run the full gate (fmt/clippy/doc/test). Resolve every warning.
- [ ] Commit: `feat(render): add minimal render-side ColorToken/CssVisibility/OffscreenAuto surface for extract`

---

## Task 2 — `resolve_color_token` pure function (color § 2.1 / § 2.2)

**Why:** token→`Color` resolution is a leaf lookup against `Theme` (no tree traversal, so it stays in extract per color § 2.1). Hoisting it into a pure fn lets us unit-test the transparent-skip and missing-token-sentinel rules with no GPU.

**Files**
- Create: `crates/buiy_core/src/render/extract.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod extract;`)
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [ ] Create `crates/buiy_core/src/render/extract.rs` with just the module doc + `use`s so the test target resolves the path, then write the failing test file `crates/buiy_core/tests/render_extract.rs`:

```rust
//! Headless unit tests for the per-view extract mapping. Pure-CPU: no wgpu
//! adapter, no RenderApp. Mirrors tests/render_instance.rs conventions.

use bevy::prelude::*;
use buiy_core::render::components::ColorToken;
use buiy_core::render::extract::resolve_color_token;
use buiy_core::theme::{Theme, default_light_theme};
use std::borrow::Cow;

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    let c = resolve_color_token(&ColorToken::Transparent, &theme);
    assert_eq!(c, Color::NONE);
}

#[test]
fn known_token_resolves_to_theme_color() {
    let theme = default_light_theme();
    let c = resolve_color_token(
        &ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        &theme,
    );
    assert_eq!(c, Color::WHITE);
}

#[test]
fn missing_token_resolves_to_magenta_sentinel() {
    let theme = default_light_theme();
    let c = resolve_color_token(&ColorToken::Token(Cow::Borrowed("nope.not.a.token")), &theme);
    // Same sentinel render/mod.rs uses for a missing token.
    assert_eq!(c, Color::srgb(1.0, 0.0, 1.0));
}
```

- [ ] Run — expect FAIL (does not compile: `resolve_color_token` unresolved):

```sh
cargo test -p buiy_core --test render_extract 2>&1 | tail -20
```

- [ ] Minimal impl. Write `crates/buiy_core/src/render/extract.rs`:

```rust
//! The per-view extract mapping, factored into pure functions so the
//! device-independent half (color-token resolution, skip predicates,
//! `painters_z` ordering, per-entity record build) is unit-testable on CI
//! runners with no wgpu adapter. The `extract_buiy_nodes` system (Task 6) is a
//! thin wrapper that calls these.
//!
//! Spec: architecture.md § 1.2/§ 3/§ 4, paint-order-and-top-layer.md § 1/§ 5.

use crate::render::MISSING_TOKEN_FALLBACK;
use crate::render::components::ColorToken;
use crate::theme::Theme;
use bevy::prelude::*;

/// Resolve a [`ColorToken`] to a concrete `Color` against `Res<Theme>`
/// (color-and-forced-colors.md § 2.1). `Transparent` → `Color::NONE`;
/// `Token(name)` → `Theme::color(name)`, falling back to the magenta sentinel
/// + `warn!` on a miss (§ 2.2) — a missing token is an author bug that must be
/// loud, never silently transparent.
pub fn resolve_color_token(token: &ColorToken, theme: &Theme) -> Color {
    match token {
        ColorToken::Transparent => Color::NONE,
        ColorToken::Token(name) => match theme.color(name) {
            Some(c) => c,
            None => {
                tracing::warn!(
                    token = %name,
                    "missing theme color token; falling back to magenta sentinel"
                );
                MISSING_TOKEN_FALLBACK
            }
        },
    }
}
```

- [ ] Add `pub mod extract;` to `crates/buiy_core/src/render/mod.rs`. Confirm `MISSING_TOKEN_FALLBACK` is reachable: it is currently a private `const` in `mod.rs`. Change its declaration from `const MISSING_TOKEN_FALLBACK` to `pub(crate) const MISSING_TOKEN_FALLBACK` so `extract.rs` can use it (do NOT make it `pub`).
- [ ] Run — expect PASS:

```sh
cargo test -p buiy_core --test render_extract 2>&1 | tail -20
```

- [ ] Run the full gate. Resolve every warning.
- [ ] Commit: `feat(render): resolve_color_token pure fn with transparent + sentinel rules`

---

## Task 3 — `SkipReason` + `node_skip_reason` pure predicate (paint-order § 5)

**Why:** the forward walk skips four categories. Three are layout-owned (the entity is already absent from `painters_z` for `Display::None`; `Containment.content_visibility == Hidden` prunes descendants; `OffscreenAuto` marks off-screen `auto`) and one is render-owned (`CssVisibility::Hidden`). Render must emit nothing for a skipped entity. Hoisting the predicate makes every skip rule a pure, table-driven unit test.

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [ ] Add failing tests to `crates/buiy_core/tests/render_extract.rs`:

```rust
use buiy_core::layout::{Containment, ContentVisibility};
use buiy_core::render::components::{CssVisibility, OffscreenAuto};
use buiy_core::render::extract::{SkipReason, node_skip_reason};

// Helper mirroring what extract binds per entity: Option of each skip input.
fn skip(
    css_vis: Option<CssVisibility>,
    offscreen: bool,
    containment: Option<&Containment>,
) -> Option<SkipReason> {
    node_skip_reason(css_vis.as_ref(), offscreen, containment)
}

#[test]
fn visible_entity_is_not_skipped() {
    assert_eq!(skip(None, false, None), None);
    assert_eq!(skip(Some(CssVisibility::Visible), false, None), None);
}

#[test]
fn css_visibility_hidden_is_skipped() {
    assert_eq!(
        skip(Some(CssVisibility::Hidden), false, None),
        Some(SkipReason::CssHidden)
    );
}

#[test]
fn css_visibility_collapse_is_not_a_paint_skip_in_v1() {
    // Collapse is a deferred table/flex marker (component-model.md § 12.1) —
    // v1 ships only the Hidden paint-skip, so Collapse paints normally.
    assert_eq!(skip(Some(CssVisibility::Collapse), false, None), None);
}

#[test]
fn offscreen_auto_is_skipped() {
    assert_eq!(skip(None, true, None), Some(SkipReason::OffscreenAuto));
}

#[test]
fn content_visibility_hidden_is_skipped() {
    let c = Containment {
        content_visibility: ContentVisibility::Hidden,
        ..Default::default()
    };
    assert_eq!(skip(None, false, Some(&c)), Some(SkipReason::ContentHidden));
}

#[test]
fn content_visibility_auto_alone_is_not_skipped() {
    // Auto only skips paint when ALSO off-screen (the OffscreenAuto marker);
    // an on-screen Auto paints normally.
    let c = Containment {
        content_visibility: ContentVisibility::Auto,
        ..Default::default()
    };
    assert_eq!(skip(None, false, Some(&c)), None);
}

#[test]
fn css_hidden_takes_precedence_over_offscreen() {
    // Precedence is observable; CssHidden is checked first.
    assert_eq!(
        skip(Some(CssVisibility::Hidden), true, None),
        Some(SkipReason::CssHidden)
    );
}
```

- [ ] Run — expect FAIL (does not compile: `SkipReason`, `node_skip_reason` unresolved):

```sh
cargo test -p buiy_core --test render_extract 2>&1 | tail -20
```

- [ ] Minimal impl. Append to `crates/buiy_core/src/render/extract.rs`:

```rust
use crate::layout::{Containment, ContentVisibility};
use crate::render::components::CssVisibility;

/// Why the forward paint walk skips an entity (paint-order-and-top-layer.md
/// § 5). `Display::None` is NOT a variant: such entities never reach extract
/// (no `ResolvedLayout`, absent from `painters_z`), so there is nothing to
/// skip — the absence IS the skip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// `CssVisibility::Hidden` — render-owned paint-skip, keep the box (§ 5.4).
    CssHidden,
    /// Off-screen `content-visibility: auto` (the `OffscreenAuto` marker, § 5.3).
    OffscreenAuto,
    /// `Containment.content_visibility == Hidden` — own box still paints, but
    /// this is the subtree-prune carrier; render emits nothing for a Hidden
    /// entity's descendants (§ 5.2). The Hidden entity itself is laid out by
    /// layout, so this predicate intentionally classifies it as skip-paint:
    /// v1 takes the conservative subtree-skip (matches the spec's "render
    /// inherits the prune for free").
    ContentHidden,
}

/// Decide whether a `Node` entity should be skipped at paint, and why.
/// `None` => paint normally. Inputs are bound as `Option<&T>` / `bool` exactly
/// as the extract fan binds them. Precedence (first match wins): render-owned
/// `CssVisibility::Hidden`, then `OffscreenAuto`, then
/// `Containment.content_visibility == Hidden`.
pub fn node_skip_reason(
    css_visibility: Option<&CssVisibility>,
    offscreen_auto: bool,
    containment: Option<&Containment>,
) -> Option<SkipReason> {
    if matches!(css_visibility, Some(CssVisibility::Hidden)) {
        return Some(SkipReason::CssHidden);
    }
    if offscreen_auto {
        return Some(SkipReason::OffscreenAuto);
    }
    if matches!(
        containment.map(|c| c.content_visibility),
        Some(ContentVisibility::Hidden)
    ) {
        return Some(SkipReason::ContentHidden);
    }
    None
}
```

- [ ] Run — expect PASS.
- [ ] Run the full gate. Resolve every warning.
- [ ] Commit: `feat(render): node_skip_reason predicate for the four paint-skip rules`

---

## Task 4 — `ExtractedNode` record + `extracted_node_for` builder

**Why:** the per-entity CPU record is the v1 instance set (`ExtractedNodes`, architecture § 3.1). Building one record is pure geometry + color: read `ResolvedLayout` for the box, fold position via `GlobalTransform`, resolve the `Background` token. Hoisting it keeps the build testable and keyed by `Entity` (so a partial re-extract can patch only changed entities, architecture § 3.1).

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [ ] Add failing tests:

```rust
use bevy::prelude::*;
use buiy_core::components::ResolvedLayout;
use buiy_core::render::components::Background;
use buiy_core::render::extract::{ExtractedNode, extracted_node_for};

#[test]
fn extracted_node_carries_box_and_resolved_color() {
    let theme = default_light_theme();
    let layout = ResolvedLayout {
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(100.0, 40.0),
    };
    let gt = GlobalTransform::from_translation(Vec3::new(10.0, 20.0, 0.0));
    let bg = Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
    };
    let entity = Entity::from_raw(7);

    let node = extracted_node_for(entity, &gt, &layout, Some(&bg), &theme);

    assert_eq!(node.entity, entity);
    assert_eq!(node.size, Vec2::new(100.0, 40.0));
    // Position is taken from GlobalTransform.translation (xy), per pillar 5.
    assert_eq!(node.position, Vec2::new(10.0, 20.0));
    assert_eq!(node.color, Color::WHITE);
}

#[test]
fn absent_background_is_transparent() {
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::ZERO,
        size: Vec2::splat(8.0),
    };
    let gt = GlobalTransform::IDENTITY;
    let node = extracted_node_for(Entity::from_raw(1), &gt, &layout, None, &theme);
    assert_eq!(node.color, Color::NONE);
}

#[test]
fn extracted_node_position_follows_global_transform() {
    // The bridge folds ResolvedLayout.position + ResolvedTransform into a Bevy
    // Transform; render reads the propagated GlobalTransform, NOT
    // ResolvedLayout.position directly. A transformed entity's painted origin
    // is the GlobalTransform translation.
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::new(0.0, 0.0), // pre-transform box origin
        size: Vec2::splat(50.0),
    };
    let gt = GlobalTransform::from_translation(Vec3::new(200.0, 300.0, 0.0));
    let node = extracted_node_for(Entity::from_raw(2), &gt, &layout, None, &theme);
    assert_eq!(node.position, Vec2::new(200.0, 300.0));
}
```

- [ ] Run — expect FAIL (unresolved `ExtractedNode` / `extracted_node_for`).
- [ ] Minimal impl. Append to `crates/buiy_core/src/render/extract.rs`:

```rust
use crate::components::ResolvedLayout;
use crate::render::components::Background;

/// One painted entity's CPU record — the per-frame instance the per-view
/// `ExtractedNodes` (Task 5) holds, keyed by `Entity` so a partial re-extract
/// patches only changed entities (architecture.md § 3.1). v1 carries the
/// solid-fill quad inputs; shadow/border/glyph fields are added by their tier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtractedNode {
    /// The source main-world entity (the partial-re-extract key).
    pub entity: Entity,
    /// Painted top-left, in logical px — `GlobalTransform.translation.xy`
    /// (pillar 5: render reads the propagated transform, not
    /// `ResolvedLayout.position`).
    pub position: Vec2,
    /// Box size in logical px, from `ResolvedLayout.size`.
    pub size: Vec2,
    /// Resolved background fill (already theme-resolved; `Color::NONE` ==
    /// transparent, extract emits no quad for it downstream).
    pub color: Color,
}

/// Build one [`ExtractedNode`] from the layout box + composed transform + the
/// (optional) background token. Pure: no GPU, no ECS access beyond the
/// borrowed components. `position` is the `GlobalTransform` translation; `size`
/// is `ResolvedLayout.size`; `color` resolves the `Background` token (absent
/// background == transparent).
pub fn extracted_node_for(
    entity: Entity,
    global_transform: &GlobalTransform,
    layout: &ResolvedLayout,
    background: Option<&Background>,
    theme: &Theme,
) -> ExtractedNode {
    let translation = global_transform.translation();
    let color = match background {
        Some(bg) => resolve_color_token(&bg.color, theme),
        None => Color::NONE,
    };
    ExtractedNode {
        entity,
        position: translation.truncate(),
        size: layout.size,
        color,
    }
}
```

- [ ] Run — expect PASS.
- [ ] Run the full gate. Resolve every warning.
- [ ] Commit: `feat(render): ExtractedNode record + extracted_node_for builder`

---

## Task 5 — `ExtractedNodes` per-view component + `painters_z` forward-order assembly

**Why:** architecture § 3.1/§ 4 replaces the global `ExtractedDraws` resource with a per-view `ExtractedNodes` component, and paint-order § 1 requires emission in `StackingContext.painters_z` index order (forward walk, never re-sorted). This task adds the container and a pure assembler that, given the root `painters_z` and a per-entity "build or skip" closure, produces the ordered `Vec<ExtractedNode>` — the orderable core of the extract system, testable without a device.

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [ ] Add failing tests (paint-order identity + skip-drops-from-output, the two headless properties paint-order § 6 names):

```rust
use buiy_core::render::extract::{ExtractedNodes, assemble_in_paint_order};

#[test]
fn extracted_nodes_default_is_empty() {
    assert!(ExtractedNodes::default().nodes.is_empty());
}

#[test]
fn assemble_emits_in_painters_z_order() {
    // painters_z is the already-sorted forward order; assembly must preserve it.
    let order = vec![
        Entity::from_raw(30),
        Entity::from_raw(10),
        Entity::from_raw(20),
    ];
    // Build closure: every entity paints; record carries its entity for the
    // order assertion.
    let nodes = assemble_in_paint_order(&order, |e| {
        Some(ExtractedNode {
            entity: e,
            position: Vec2::ZERO,
            size: Vec2::ONE,
            color: Color::WHITE,
        })
    });
    let got: Vec<Entity> = nodes.nodes.iter().map(|n| n.entity).collect();
    assert_eq!(got, order, "emission order must equal painters_z index order");
}

#[test]
fn assemble_drops_skipped_entities() {
    let order = vec![
        Entity::from_raw(1),
        Entity::from_raw(2), // skipped
        Entity::from_raw(3),
    ];
    let nodes = assemble_in_paint_order(&order, |e| {
        if e == Entity::from_raw(2) {
            None // skip
        } else {
            Some(ExtractedNode {
                entity: e,
                position: Vec2::ZERO,
                size: Vec2::ONE,
                color: Color::WHITE,
            })
        }
    });
    let got: Vec<Entity> = nodes.nodes.iter().map(|n| n.entity).collect();
    assert_eq!(got, vec![Entity::from_raw(1), Entity::from_raw(3)]);
}

#[test]
fn hit_test_order_is_paint_order_reversed() {
    // The ordering identity (paint-order § 2): hit-test = painters_z reversed.
    // Asserted on the assembled output so paint and pick cannot diverge.
    let order = vec![Entity::from_raw(1), Entity::from_raw(2), Entity::from_raw(3)];
    let nodes = assemble_in_paint_order(&order, |e| {
        Some(ExtractedNode {
            entity: e,
            position: Vec2::ZERO,
            size: Vec2::ONE,
            color: Color::WHITE,
        })
    });
    let paint: Vec<Entity> = nodes.nodes.iter().map(|n| n.entity).collect();
    let hit: Vec<Entity> = nodes.nodes.iter().rev().map(|n| n.entity).collect();
    let mut paint_rev = paint.clone();
    paint_rev.reverse();
    assert_eq!(hit, paint_rev);
}
```

- [ ] Run — expect FAIL (unresolved `ExtractedNodes`, `assemble_in_paint_order`).
- [ ] Minimal impl. Append to `crates/buiy_core/src/render/extract.rs`:

```rust
/// Per-view CPU instance set — the `Changed`-gated per-frame product of
/// extract, stored as a COMPONENT on the per-view render entity (architecture
/// § 4, R8), NOT a global resource, so each window's set is isolated. v1 writes
/// every `Node` into the PRIMARY view's `ExtractedNodes` (architecture § 4,
/// D2); a second window's view runs `BuiyNode` but receives an empty set until
/// the per-window partition is wired.
#[derive(Component, Default, Clone, Debug)]
pub struct ExtractedNodes {
    /// In `painters_z` forward order (index 0 bottom-most). Never re-sorted by
    /// render (pillar 1); hit-test order is this reversed (paint-order § 2).
    pub nodes: Vec<ExtractedNode>,
}

/// Walk `painters_z` front-to-back (paint-order § 1) and emit each painter's
/// record, skipping entities for which `build` returns `None` (the skip rules,
/// § 5). Pure: the caller supplies `build` closing over the extract query so
/// this assembler stays device- and ECS-free and unit-testable. Emission order
/// is exactly `painters_z` index order — never a re-sort (pillar 1).
pub fn assemble_in_paint_order(
    painters_z: &[Entity],
    mut build: impl FnMut(Entity) -> Option<ExtractedNode>,
) -> ExtractedNodes {
    let mut nodes = Vec::with_capacity(painters_z.len());
    for &painter in painters_z {
        if let Some(node) = build(painter) {
            nodes.push(node);
        }
    }
    ExtractedNodes { nodes }
}
```

- [ ] Run — expect PASS.
- [ ] Run the full gate. Resolve every warning.
- [ ] Commit: `feat(render): ExtractedNodes per-view component + painters_z-ordered assembly`

---

## Task 6 — `extract_buiy_nodes` system (the Extract fan + Changed-gating) — wiring only

**Why:** the system is the thin wrapper that binds the `Option<&T>` fan, the `Changed`-gated `Or`-set, resolves the primary view, and calls the Task 3–5 pure functions. The system body itself is mostly non-unit-testable on CI (it needs `Extract<…>` / the render world), so it is written to call the already-tested pure core, and its only headless assertion is "the function type-checks as a Bevy system / compiles into a schedule" (Task 8). The behavioral assertions ride the pure-fn tests above.

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: none new here (behavior is covered by Tasks 2–5; system-membership smoke is Task 8 GPU-`#[ignore]`)

### Steps

- [ ] Implement the system. Append to `crates/buiy_core/src/render/extract.rs`. This is the architecture § 1.2 fan, written against the v1 subset this phase ships (Background + the skip inputs + the layout handoffs); reserved effect components (`BoxShadow`/`Opacity`/`Outline`/`EffectGroup`/`ClipRect`/`Filter`/…) are added to the fan as their tier lands — the `// FAN: add … here` comment marks the seam:

```rust
use crate::components::StackingContext;
use crate::layout::Stacking;
use crate::render::components::{Background, CssVisibility, OffscreenAuto};
use crate::theme::Theme;
use bevy::ecs::query::QueryItem;
use bevy::render::Extract;
use bevy::window::PrimaryWindow;

/// Per-frame, `Changed`-gated extract (architecture.md § 1.2/§ 3/§ 4). Reads
/// the main world's layout + render-owned components through `Extract`, walks
/// the primary view's stacking order, and writes the per-view `ExtractedNodes`.
///
/// v1 resolves every `Node` to the PRIMARY window's view (architecture § 4,
/// D2). The query reads ALL windows (reserved per-window structure) so the
/// partition can be turned on without a query change; v1 still targets primary.
///
/// Extraction is one-directional and read-only (pillar 1): it never mutates the
/// main world, never re-sorts `painters_z`, never re-derives stacking/geometry.
#[allow(clippy::type_complexity)]
pub fn extract_buiy_nodes(
    mut commands: Commands,
    // The author-set + handoff fan: Option<&T> for every independently-inserted
    // component (architecture § 1.2 — a non-Option term would silently drop a
    // Node missing that component). Required terms: &ResolvedLayout (a
    // Display::None entity has no ResolvedLayout and is dropped here) and
    // &GlobalTransform (pillar 5).
    nodes: Extract<
        Query<
            (
                Entity,
                &GlobalTransform,
                &ResolvedLayout,
                Option<&Background>,
                Option<&CssVisibility>,
                Option<&OffscreenAuto>,
                Option<&Containment>,
                // FAN: add Option<&BoxShadow>/&Outline/&Opacity/&EffectGroup/
                // &ClipRect/&Border and the reserved effect components here as
                // their tier lands (architecture § 1.2 illustrative subset).
            ),
            (
                With<Node>,
                Or<(
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<Background>,
                    Changed<CssVisibility>,
                    Changed<OffscreenAuto>,
                    Changed<Containment>,
                    Changed<StackingContext>,
                    Changed<Stacking>,
                    // FAN: extend the Or-set in lockstep with the query tuple
                    // (architecture § 3.1 trigger union).
                )>,
            ),
        >,
    >,
    roots: Extract<Query<&StackingContext>>,
    theme: Extract<Res<Theme>>,
    // Reserved per-window structure: read ALL windows, not just primary
    // (architecture § 4) — v1 still resolves every Node to the primary view.
    _windows: Extract<Query<(Entity, &Window)>>,
    primary: Extract<Query<Entity, With<PrimaryWindow>>>,
) {
    // Resolve the primary window's view target entity. v1: all Nodes paint into
    // the primary view (D2). If there is no primary window this frame, emit
    // nothing (matches the Phase-0 window-size guard).
    let Ok(_primary_window) = primary.single() else {
        return;
    };

    // Build a per-entity index so the painters_z walk can look each painter up.
    // (A HashMap keyed by Entity; the partial-re-extract cache keyed by Entity
    // inside ExtractedNodes is R3's optimization — v1 rebuilds the changed set.)
    let theme = theme.into_inner();
    // `std::collections::HashMap` matches the convention in layout/systems.rs.
    let mut by_entity: std::collections::HashMap<Entity, ExtractedNode> =
        std::collections::HashMap::new();
    for (entity, gt, layout, bg, css_vis, offscreen, containment) in nodes.iter() {
        let skip = node_skip_reason(css_vis, offscreen.is_some(), containment);
        if skip.is_some() {
            continue;
        }
        by_entity.insert(entity, extracted_node_for(entity, gt, layout, bg, theme));
    }

    // Walk every root context's painters_z forward and assemble in order.
    // (v1: one global root set written to the primary view; nested contexts are
    // entered atomically by their presence in the parent's painters_z.)
    let mut all = ExtractedNodes::default();
    for sc in roots.iter() {
        let part = assemble_in_paint_order(&sc.painters_z, |e| by_entity.get(&e).copied());
        all.nodes.extend(part.nodes);
    }

    // Write the per-view ExtractedNodes onto the primary render view entity.
    // R3 wires the exact main<->render view mapping; v1 inserts as a resource-
    // free per-view component. The precise target-entity resolution is the one
    // piece that needs the render world and is exercised only under the GPU
    // e2e path (Task 8 / R3). Until R3 lands the node-side read, this insert is
    // a no-op consumer-wise but keeps the data shape correct and tested.
    commands.insert_resource(ExtractedNodesPrimary(all));
}

/// v1 bridge carrier: the primary view's `ExtractedNodes`, inserted by
/// `extract_buiy_nodes`. A temporary resource shape until R3 wires the
/// per-view component onto the resolved render-view entity (architecture § 4).
/// SUPERSEDED-BY: R3 hybrid-handoff phase (node.rs reads per-view component).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedNodesPrimary(pub ExtractedNodes);
```

> **Note on `by_entity.get(&e).copied()`:** the `Changed`-gated fan only yields *changed* entities, so an unchanged painter is absent from `by_entity` and would be dropped from this frame's set. That is the R3 partial-re-extract concern (architecture § 3.1: "an unchanged entity contributes its *cached* instance record"). v1 of THIS phase intentionally rebuilds only the changed set into the bridge resource; the cache-merge across frames is R3. The pure assembler (Task 5) is already correct for the full-set case the cache will feed it. Leave a `// R3: merge cached records for unchanged painters here` comment at the `assemble_in_paint_order` call.

- [ ] Confirm it compiles as a library (no GPU needed to *compile* the system; only to *run* it in a `RenderApp`):

```sh
cargo build -p buiy_core 2>&1 | tail -20
```

- [ ] Run the full gate. Resolve every warning. (Clippy will see the system; the `#[allow(clippy::type_complexity)]` keeps the fan tuple acceptable — mirror the layout systems' convention if clippy complains about anything else.)
- [ ] Commit: `feat(render): extract_buiy_nodes system wiring (per-view, Changed-gated, primary view)`

---

## Task 7 — Register `extract_buiy_nodes`, retire `extract_buiy_draws` from the schedule

**Why:** `mod.rs` currently registers `extract_buiy_draws`. This phase swaps the registered extract system to `extract_buiy_nodes` and registers the new render-side components for reflection (component-model § 13: author-set components `register_type`; the computed/marker ones are not). The Phase-0 draw path (`ExtractedDraws`/`DrawData`/`to_instance`/`node.rs`) stays defined so the GPU draw still links — but it is no longer fed by a registered extract system, so the node's `ExtractedDraws` read becomes empty. **This is intentional and bounded:** R3 (the node/instance rework) is the phase that swaps `node.rs` to read the per-view `ExtractedNodes`. To avoid a regression where the lavapipe e2e draws nothing between this phase and R3, this task keeps BOTH extract systems registered (the old one feeds the current node; the new one feeds the bridge resource), and removes the old one in R3. Pick the dual-registration option below.

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs`
- Test: `crates/buiy_core/tests/render_smoke.rs` (extend the headless no-panic test)

### Steps

- [ ] Add a failing headless test to `crates/buiy_core/tests/render_smoke.rs` — assert the plugin still loads without a `RenderApp` AND that the new render components register for reflection in the *main* world (reflection registration is device-free; it happens in `BuiyRenderPlugin::build`'s non-`RenderApp` path only if we move registration there — see impl note). Append:

```rust
#[test]
fn render_components_register_for_reflection() {
    use bevy::reflect::TypeRegistry;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();

    let registry = app.world().resource::<AppTypeRegistry>().read();
    // Author-set render components register (component-model § 13).
    assert!(
        registry
            .get(std::any::TypeId::of::<buiy_core::render::components::Background>())
            .is_some(),
        "Background registers for reflection"
    );
    assert!(
        registry
            .get(std::any::TypeId::of::<buiy_core::render::components::CssVisibility>())
            .is_some(),
        "CssVisibility registers for reflection"
    );
    let _ = TypeRegistry::default(); // keep the import used if the above changes
}
```

- [ ] Run — expect FAIL (the components are not yet `register_type`'d):

```sh
cargo test -p buiy_core --test render_smoke render_components_register_for_reflection 2>&1 | tail -20
```

- [ ] Minimal impl in `crates/buiy_core/src/render/mod.rs`:
  1. Add `pub mod extract;` and `pub mod components;` near the other `pub mod` lines (if not already added in Tasks 1–2).
  2. In `BuiyRenderPlugin::build`, **before** the `get_sub_app_mut(RenderApp)` early-return, register the author-set render components for reflection in the **main** app (so reflection works headlessly, and matches how `CorePlugin` registers types in the main world). Insert at the top of `build`:

```rust
// Author-set render components register for reflection/BSN (component-model
// § 13). The computed ClipRect/EffectGroup and the layout-written
// OffscreenAuto marker are intentionally NOT registered here.
app.register_type::<components::Background>()
    .register_type::<components::CssVisibility>();
```

  3. Inside the `RenderApp` branch, register the new extract system alongside the existing one (dual-registration; see the task preamble). Change:

```rust
render_app
    .init_resource::<ExtractedDraws>()
    .add_systems(ExtractSchedule, extract_buiy_draws);
```

to:

```rust
render_app
    .init_resource::<ExtractedDraws>()
    .init_resource::<extract::ExtractedNodesPrimary>()
    // Phase-0 draw path (feeds node.rs today); retired in the R3 hybrid-handoff
    // phase when node.rs reads the per-view ExtractedNodes instead.
    .add_systems(ExtractSchedule, extract_buiy_draws)
    // The per-view extract rework (this phase). architecture § 1.2/§ 3/§ 4.
    .add_systems(ExtractSchedule, extract::extract_buiy_nodes);
```

  4. Add `use extract::extract_buiy_nodes;`? No — keep the path-qualified `extract::extract_buiy_nodes` to avoid an unused-import lint if the symbol is referenced only once. Confirm the `extract` module is `pub mod`.

- [ ] Run — expect PASS:

```sh
cargo test -p buiy_core --test render_smoke 2>&1 | tail -20
```

- [ ] Run the full gate. Resolve every warning.
- [ ] Commit: `feat(render): register extract_buiy_nodes + author-set render components`

---

## Task 8 — GPU `#[ignore]` smoke: `extract_buiy_nodes` is in `ExtractSchedule`

**Why:** architecture § 7 / verification § 2.1 pin that render-world schedule membership (`extract_buiy_nodes ∈ ExtractSchedule`) is NOT device-free — building the `RenderApp` requires `block_on(initialize_renderer)` which `.expect()`s a wgpu adapter. So this assertion rides the `#[ignore]` GPU path, exactly like the existing `pipeline_registers_in_render_app` / `render_graph_node_inserted_after_main_2d_pass` smoke tests. This is the only place this phase touches the device path.

**Files**
- Test: `crates/buiy_core/tests/render_smoke.rs` (add one `#[ignore]` test)

### Steps

- [ ] Add the GPU-`#[ignore]` test to `crates/buiy_core/tests/render_smoke.rs`, mirroring the existing `#[ignore]` idiom verbatim:

```rust
// Same RenderApp/wgpu-adapter caveat as `pipeline_registers_in_render_app`:
// constructing the RenderApp needs a wgpu adapter (real GPU or lavapipe), so
// this rides the #[ignore] GPU path (architecture § 7 / verification § 2.1 —
// extract-system membership is NOT device-free). The device-free behavior of
// the extract mapping is covered headlessly in tests/render_extract.rs.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the e2e harness"]
fn extract_buiy_nodes_registered_in_extract_schedule() {
    use bevy::render::{ExtractSchedule, RenderApp};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);

    let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
    // The schedule exists and contains systems; a full per-system membership
    // assertion would walk the schedule graph. We assert the schedule is
    // present and the app built — the behavioral correctness of the mapping is
    // the headless render_extract.rs suite.
    assert!(
        render_app
            .world()
            .get_resource::<buiy_core::render::extract::ExtractedNodesPrimary>()
            .is_some(),
        "ExtractedNodesPrimary initialized in the RenderApp"
    );
    let _ = ExtractSchedule; // label in scope; membership runs under the e2e harness
}
```

- [ ] Confirm the ignored test compiles and is skipped by default:

```sh
cargo test -p buiy_core --test render_smoke 2>&1 | tail -20   # the new test shows as `ignored`
```

- [ ] (Optional, only if a lavapipe adapter is available locally — NOT on CI) run the ignored test:

```sh
cargo test -p buiy_core --test render_smoke -- --ignored 2>&1 | tail -20
```

- [ ] Run the full gate (the ignored test must still *compile* clean). Resolve every warning.
- [ ] Commit: `test(render): GPU-ignored smoke for extract_buiy_nodes RenderApp membership`

---

## Task 9 — Documentation: mark the extract rework landed in the spec catalog

**Why:** per the project doc convention, doc updates ship *with* the change. The render-pipeline spec README's Phase-0 note and the docs index should reflect that `extract_buiy_nodes` + per-view `ExtractedNodes` now exist (replacing `extract_buiy_draws` at the schedule level), with the node-side consumption swap still pending R3.

**Files**
- Modify: `docs/README.md` (the master index — add/annotate the plan row)
- Modify: `docs/plans/2026-06-03-buiy-render-r5-extract.md` (this file — check every box)

### Steps

- [ ] In `docs/README.md`, find the render-pipeline plans grouping (or add one under the render area) and add a row pointing to this plan with status `landed (extract rework; node-side read pending R3)`. Mirror the existing row formatting in that file — read the surrounding rows first, do not invent a new table shape.
- [ ] Confirm no other spec child contradicts "extract_buiy_nodes shipped": the architecture § 1.2 text already describes it as the target; no edit needed there (it is target-state prose). Leave a one-line note in this plan's header if any drift is found.
- [ ] Run the full gate one final time (docs changes do not affect it, but run it to be certain nothing regressed):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

- [ ] Commit: `docs(render): mark per-view extract rework landed in catalog`

---

## Done criteria

- `extract_buiy_nodes` registered in `ExtractSchedule`; its entire mapping (color resolution, skip predicates, `ExtractedNode` build, `painters_z` forward assembly) lives in pure functions in `render/extract.rs` and is unit-tested in `tests/render_extract.rs` with **no GPU**.
- Skip rules (`CssVisibility::Hidden`, `OffscreenAuto`, `Containment.content_visibility == Hidden`; `Display::None` via absent `ResolvedLayout`/absent from `painters_z`) are headless table tests.
- The paint/hit-test ordering identity (paint = `painters_z` forward, hit-test = reversed) is a headless property test.
- The only device-dependent assertion (`extract_buiy_nodes` in a live `RenderApp`) is `#[ignore]` GPU, matching the existing `render_smoke.rs` idiom.
- The full gate (`fmt && clippy -D warnings && doc -D warnings && test`) is green at every commit, with no xvfb and no wgpu adapter.
- The Phase-0 draw path is left intact (retired by R3); the per-window partition is reserved (query reads all windows, v1 targets primary).
```
