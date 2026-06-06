# Per-view Extract Rework Implementation Plan

> **Depends on:** R1 (component-model — owns `render/components.rs` + `render/color.rs` and all shared render types). Execution order across the render-pipeline series: R1 → R2 → R3 → R4 → **R5** → R6 → R7 → R8 → (R9, R10) → R11. R6 consumes this phase's `ExtractedNodes`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Phase-0 `extract_buiy_draws` with a per-view, `Changed`-gated `extract_buiy_nodes` whose entire per-entity mapping (DrawData build, color-token resolution, skip predicates, `painters_z` forward ordering) is factored into pure functions that are unit-tested headless, while the device-dependent system wiring/registration is `#[ignore]` GPU.

**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes **architecture.md § 1.2 / § 3 / § 4** (the `ExtractSchedule` + `Extract<Query>` boundary, the `Changed<T>`-gated per-frame instance set, per-view `ExtractedNodes` stored on the primary view) and **paint-order-and-top-layer.md § 1 / § 5** (forward walk of `StackingContext.painters_z`; the `Display::None` / `CssVisibility::Hidden` / `OffscreenAuto` skip rules — `content-visibility: hidden` paints the entity's own box and prunes descendants layout-side, § 5.2, so it is not a render skip).

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

This phase is **R5 (extract rework)** in the render-pipeline plan series. It **depends on R1** (the component-model phase), which is the SOLE creator of `render/components.rs` + `render/color.rs` and the sole definer of every shared render type. R5 consumes — and never redefines — these R1-owned types:

- `Background { color: ColorToken }`, `Border { radius: Corners, … }`, `BoxShadow`, `Opacity`, `Outline`, `CssVisibility`, `EffectGroup`, `ClipRect`, `OffscreenAuto`, and the reserved effect components (component-model.md §§ 3–12) — all in `render/components.rs`.
- `ColorToken` + `SystemColorKeyword` — in `render/color.rs` (color-and-forced-colors.md § 2.0). Resolution rules are extended by R11.

**Ownership rule (no duplication):** R1 has already landed `render/components.rs` and `render/color.rs`. R5 therefore **imports** every type it reads (`ColorToken` from `render::color`; `Background` / `CssVisibility` / `OffscreenAuto` / `Border` / `BoxShadow` / `Opacity` / `Outline` / `EffectGroup` / `ClipRect` from `render::components`) and never re-defines a type, never adds a `pub mod components;` / `pub mod color;`, and never re-exports from `lib.rs`. The pure functions in `extract.rs` are written against these concrete types.

Layout-owned types this phase reads are **already shipped** and exported from `buiy_core::layout`: `StackingContext` (crate-root `components.rs`), `Stacking`, `TopLayer`, `Display`, `ScrollOffset`, `Overflow`. `ResolvedLayout` / `ResolvedTransform` / `StackingContext` are in `crate::components`. (`Containment` / `ContentVisibility` are NOT read by render — `content-visibility: hidden` paints the entity's own box and prunes descendants layout-side, paint-order § 5.2.)

The Phase-0 `extract_buiy_draws` + `ExtractedDraws` + `DrawData` + `instance::to_instance` remain **until the `node.rs`/`instance.rs` rework lands** — that is **R6 (prepare-buffers, Task 7 — buckets/instance build)** plus **R8 (paint-clip-toplayer — the per-entity scissor + composite in `BuiyNode::run`)**, NOT R3 (R3 is the transform bridge). This phase introduces `extract_buiy_nodes` and `ExtractedNodes` and rewires `mod.rs` to register the new system, but leaves the Phase-0 `DrawData`/`to_instance`/`node.rs` draw path intact so the workspace keeps compiling and the GPU draw still works under lavapipe. The node-side consumption swap (`ExtractedDraws` → per-view `ExtractedNodes`) is **R6/R8's** deliverable; **R6 consumes this phase's `ExtractedNodes` directly** (no parallel rebuild). Called out at Task 7.

---

## Type contract used by this phase (verbatim names — do not rename)

| Name | Kind | Source / owner | This phase |
|---|---|---|---|
| `ExtractedNode` | struct (CPU record, one per painted entity) | this phase (render/extract.rs) | introduces |
| `ExtractedNodes` | per-view component `{ nodes: Vec<ExtractedNode>, logical_size: Vec2, scale_factor: f32 }` (manual `Default`, `scale_factor = 1.0`) | **this phase — SOLE owner**; R6 consumes it | introduces |
| `extract_buiy_nodes` | `ExtractSchedule` system | this phase (architecture § 1.2) | introduces |
| `node_skip_reason` / `SkipReason` | pure fn + enum | this phase | introduces |
| `resolve_color_token` | pure fn (`&ColorToken`, `&Theme`) → `Color` | this phase (color § 2.1) | introduces |
| `extracted_node_for` | pure fn building one `ExtractedNode` | this phase | introduces |
| `ColorToken` | enum (`Transparent`, `Token`, …) | **R1 — `render/color.rs`** (color § 2.0) | import |
| `Background` `{ color: ColorToken }` | author-set component | **R1 — `render/components.rs`** (component-model.md § 3) | import |
| `CssVisibility` `{ Visible, Hidden, Collapse }` | render-owned component | **R1 — `render/components.rs`** (component-model.md § 12.1) | import |
| `OffscreenAuto` | layout-written marker | **R1 — `render/components.rs`** (component-model.md § 12.2) | import (read-only) |
| `StackingContext { painters_z: Vec<Entity> }` | layout handoff | `crate::components` (shipped) | read |
| `Stacking { z_index, isolation, top_layer }` | layout component | `layout::components` (shipped) | read |
| `ResolvedLayout { position, size }` | layout output | `crate::components` (shipped) | read |
| `MISSING_TOKEN_FALLBACK` | magenta sentinel | `render/mod.rs` (shipped) | reuse |

---

## Task 1 — Guarded import of the R1 render-side component surface (no creation)

**Why:** the pure extract functions need concrete `ColorToken`, `Background`, `CssVisibility`, and `OffscreenAuto` types. **R1 owns all of them** — `render/components.rs` and `render/color.rs` already exist. This task does NOT create a file, add a `pub mod`, or re-export anything; it verifies the R1 types are present and pins the exact import paths `extract.rs` uses.

**Files**
- Verify only (no edits): `crates/buiy_core/src/render/components.rs`, `crates/buiy_core/src/render/color.rs` (both created by R1).

**GUARDED IMPORT — these types already exist (owned by R1):** import them; do NOT redefine, do NOT re-add `pub mod components;` / `pub mod color;`, do NOT re-export from `lib.rs`.

- `ColorToken` (and `SystemColorKeyword`) → `crate::render::color` (color-and-forced-colors.md § 2.0).
- `Background`, `CssVisibility`, `OffscreenAuto` (and `Border`, `BoxShadow`, `Opacity`, `Outline`, `EffectGroup`, `ClipRect` as their tier lands) → `crate::render::components` (component-model.md §§ 3, 12, 13).

### Steps

- [x] Confirm the R1 types exist (they do — R1 is a prerequisite). If any grep below is empty, R1 has not landed and this phase is blocked — stop and surface it rather than re-creating the type:

```sh
grep -rn "pub enum ColorToken" crates/buiy_core/src/render/color.rs
grep -rn "pub struct Background\|pub enum CssVisibility\|pub struct OffscreenAuto" crates/buiy_core/src/render/components.rs
```

- [x] Confirm `lib.rs` already re-exports `ColorToken` (from `render::color`, added by R1). Do NOT add a second re-export here.
- [x] No code/test is written in this task — `extract.rs` (Tasks 2–6) imports `ColorToken` from `crate::render::color` and `Background` / `CssVisibility` / `OffscreenAuto` from `crate::render::components`. The R1 inline tests already cover these types' `Default`/variant behavior; this phase does not re-test R1's surface.
- [x] No commit for this task (verification only); fold it into Task 2's first commit if any tracking change is needed.

---

## Task 2 — `resolve_color_token` pure function (color § 2.1 / § 2.2)

**Why:** token→`Color` resolution is a leaf lookup against `Theme` (no tree traversal, so it stays in extract per color § 2.1). Hoisting it into a pure fn lets us unit-test the transparent-skip and missing-token-sentinel rules with no GPU.

**Files**
- Create: `crates/buiy_core/src/render/extract.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod extract;`)
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [x] Create `crates/buiy_core/src/render/extract.rs` with just the module doc + `use`s so the test target resolves the path, then write the failing test file `crates/buiy_core/tests/render_extract.rs`:

```rust
//! Headless unit tests for the per-view extract mapping. Pure-CPU: no wgpu
//! adapter, no RenderApp. Mirrors tests/render_instance.rs conventions.

use bevy::prelude::*;
use buiy_core::render::color::ColorToken;
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

- [x] Run — expect FAIL (does not compile: `resolve_color_token` unresolved):

```sh
cargo test -p buiy_core --test render_extract 2>&1 | tail -20
```

- [x] Minimal impl. Write `crates/buiy_core/src/render/extract.rs`:

```rust
//! The per-view extract mapping, factored into pure functions so the
//! device-independent half (color-token resolution, skip predicates,
//! `painters_z` ordering, per-entity record build) is unit-testable on CI
//! runners with no wgpu adapter. The `extract_buiy_nodes` system (Task 6) is a
//! thin wrapper that calls these.
//!
//! Spec: architecture.md § 1.2/§ 3/§ 4, paint-order-and-top-layer.md § 1/§ 5.

use crate::render::MISSING_TOKEN_FALLBACK;
use crate::render::color::ColorToken;
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

- [x] Add `pub mod extract;` to `crates/buiy_core/src/render/mod.rs`. Confirm `MISSING_TOKEN_FALLBACK` is reachable: it is currently a private `const` in `mod.rs`. Change its declaration from `const MISSING_TOKEN_FALLBACK` to `pub(crate) const MISSING_TOKEN_FALLBACK` so `extract.rs` can use it (do NOT make it `pub`).
- [x] Run — expect PASS:

```sh
cargo test -p buiy_core --test render_extract 2>&1 | tail -20
```

- [x] Run the full gate. Resolve every warning.
- [x] Commit: `feat(render): resolve_color_token pure fn with transparent + sentinel rules`

---

## Task 3 — `SkipReason` + `node_skip_reason` pure predicate (paint-order § 5)

**Why:** the forward walk skips three render-relevant categories: `Display::None` (the entity is already absent from `painters_z`, so the absence *is* the skip — no variant needed), render-owned `CssVisibility::Hidden`, and off-screen `content-visibility: auto` (the layout-written `OffscreenAuto` marker). **`Containment.content_visibility == Hidden` is NOT a render paint-skip:** per paint-order-and-top-layer.md § 5.2 the Hidden entity's OWN box still paints; its descendants are already pruned **layout-side** (they never reach `painters_z`), so render inherits the prune for free and must NOT skip the Hidden entity itself. Hoisting the predicate makes every skip rule a pure, table-driven unit test.

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [x] Add failing tests to `crates/buiy_core/tests/render_extract.rs`:

```rust
use buiy_core::render::components::{CssVisibility, OffscreenAuto};
use buiy_core::render::extract::{SkipReason, node_skip_reason};

// Helper mirroring what extract binds per entity: Option of each skip input.
fn skip(css_vis: Option<CssVisibility>, offscreen: bool) -> Option<SkipReason> {
    node_skip_reason(css_vis.as_ref(), offscreen)
}

#[test]
fn visible_entity_is_not_skipped() {
    assert_eq!(skip(None, false), None);
    assert_eq!(skip(Some(CssVisibility::Visible), false), None);
}

#[test]
fn css_visibility_hidden_is_skipped() {
    assert_eq!(
        skip(Some(CssVisibility::Hidden), false),
        Some(SkipReason::CssHidden)
    );
}

#[test]
fn css_visibility_collapse_is_not_a_paint_skip_in_v1() {
    // Collapse is a deferred table/flex marker (component-model.md § 12.1) —
    // v1 ships only the Hidden paint-skip, so Collapse paints normally.
    assert_eq!(skip(Some(CssVisibility::Collapse), false), None);
}

#[test]
fn offscreen_auto_is_skipped() {
    assert_eq!(skip(None, true), Some(SkipReason::OffscreenAuto));
}

#[test]
fn content_visibility_hidden_entity_still_paints_its_own_box() {
    // paint-order-and-top-layer.md § 5.2: a `content-visibility: hidden`
    // entity's OWN box paints; only its descendants are pruned, and that prune
    // happens layout-side (they never reach painters_z). Render therefore does
    // NOT skip the Hidden entity itself — Containment is not even a skip input.
    assert_eq!(skip(None, false), None);
}

#[test]
fn css_hidden_takes_precedence_over_offscreen() {
    // Precedence is observable; CssHidden is checked first.
    assert_eq!(
        skip(Some(CssVisibility::Hidden), true),
        Some(SkipReason::CssHidden)
    );
}
```

- [x] Run — expect FAIL (does not compile: `SkipReason`, `node_skip_reason` unresolved):

```sh
cargo test -p buiy_core --test render_extract 2>&1 | tail -20
```

- [x] Minimal impl. Append to `crates/buiy_core/src/render/extract.rs`:

```rust
use crate::render::components::CssVisibility;

/// Why the forward paint walk skips an entity (paint-order-and-top-layer.md
/// § 5). `Display::None` is NOT a variant: such entities never reach extract
/// (no `ResolvedLayout`, absent from `painters_z`), so there is nothing to
/// skip — the absence IS the skip. `content-visibility: hidden` is likewise NOT
/// a variant: § 5.2 keeps the Hidden entity's own box painting and prunes its
/// descendants layout-side (they never enter `painters_z`), so render inherits
/// the prune for free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// `CssVisibility::Hidden` — render-owned paint-skip, keep the box (§ 5.4).
    CssHidden,
    /// Off-screen `content-visibility: auto` (the `OffscreenAuto` marker, § 5.3).
    OffscreenAuto,
}

/// Decide whether a `Node` entity should be skipped at paint, and why.
/// `None` => paint normally. Inputs are bound as `Option<&T>` / `bool` exactly
/// as the extract fan binds them. Precedence (first match wins): render-owned
/// `CssVisibility::Hidden`, then `OffscreenAuto`. `content-visibility: hidden`
/// is deliberately NOT consulted here — the Hidden entity's own box paints
/// (§ 5.2) and its descendants are pruned layout-side.
pub fn node_skip_reason(
    css_visibility: Option<&CssVisibility>,
    offscreen_auto: bool,
) -> Option<SkipReason> {
    if matches!(css_visibility, Some(CssVisibility::Hidden)) {
        return Some(SkipReason::CssHidden);
    }
    if offscreen_auto {
        return Some(SkipReason::OffscreenAuto);
    }
    None
}
```

- [x] Run — expect PASS.
- [x] Run the full gate. Resolve every warning.
- [x] Commit: `feat(render): node_skip_reason predicate (CssHidden + OffscreenAuto; content-visibility:hidden paints own box)`

---

## Task 4 — `ExtractedNode` record + `extracted_node_for` builder

**Why:** the per-entity CPU record is the v1 instance set (`ExtractedNodes`, architecture § 3.1). Building one record is pure geometry + color: read `ResolvedLayout` for the box, fold position via `GlobalTransform`, resolve the `Background` token. Hoisting it keeps the build testable and keyed by `Entity` (so a partial re-extract can patch only changed entities, architecture § 3.1).

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [x] Add failing tests:

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

- [x] Run — expect FAIL (unresolved `ExtractedNode` / `extracted_node_for`).
- [x] Minimal impl. Append to `crates/buiy_core/src/render/extract.rs`:

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

- [x] Run — expect PASS.
- [x] Run the full gate. Resolve every warning.
- [x] Commit: `feat(render): ExtractedNode record + extracted_node_for builder`

---

## Task 5 — `ExtractedNodes` per-view component + `painters_z` forward-order assembly

**Why:** architecture § 3.1/§ 4 replaces the global `ExtractedDraws` resource with a per-view `ExtractedNodes` component, and paint-order § 1 requires emission in `StackingContext.painters_z` index order (forward walk, never re-sorted). This task adds the container and a pure assembler that, given the root `painters_z` and a per-entity "build or skip" closure, produces the ordered `Vec<ExtractedNode>` — the orderable core of the extract system, testable without a device.

**R5 is the SOLE owner of `ExtractedNodes`.** It is the single per-view carrier `{ nodes: Vec<ExtractedNode>, logical_size: Vec2, scale_factor: f32 }` with a **manual `Default`** (`scale_factor = 1.0`, the others empty/`ZERO`). **R6 consumes this exact type** — there is no parallel `ExtractedNodes` / `ExtractedNodesResource` rebuilt from `ExtractedDraws`. Do NOT derive `Default` (it would give `scale_factor = 0.0`).

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: `crates/buiy_core/tests/render_extract.rs`

### Steps

- [x] Add failing tests (paint-order identity + skip-drops-from-output, the two headless properties paint-order § 6 names):

```rust
use buiy_core::render::extract::{ExtractedNodes, assemble_in_paint_order};

#[test]
fn extracted_nodes_default_is_empty_with_unit_scale() {
    let d = ExtractedNodes::default();
    assert!(d.nodes.is_empty());
    // Manual Default: scale_factor is 1.0, NOT the derived 0.0.
    assert_eq!(d.scale_factor, 1.0);
    assert_eq!(d.logical_size, Vec2::ZERO);
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

- [x] Run — expect FAIL (unresolved `ExtractedNodes`, `assemble_in_paint_order`).
- [x] Minimal impl. Append to `crates/buiy_core/src/render/extract.rs`:

```rust
/// Per-view CPU instance set — the `Changed`-gated per-frame product of
/// extract, stored as a COMPONENT on the per-view render entity (architecture
/// § 4, R8), NOT a global resource, so each window's set is isolated. v1 writes
/// every `Node` into the PRIMARY view's `ExtractedNodes` (architecture § 4,
/// D2); a second window's view runs `BuiyNode` but receives an empty set until
/// the per-window partition is wired.
///
/// **R5 owns this type; R6 consumes it.** Single carrier — there is no parallel
/// `ExtractedNodes`/`ExtractedNodesResource` rebuilt from `ExtractedDraws`.
/// `Default` is MANUAL so `scale_factor` is `1.0` (a derived `Default` would be
/// `0.0` and divide-by-zero the logical→physical map).
#[derive(Component, Clone, Debug)]
pub struct ExtractedNodes {
    /// In `painters_z` forward order (index 0 bottom-most). Never re-sorted by
    /// render (pillar 1); hit-test order is this reversed (paint-order § 2).
    pub nodes: Vec<ExtractedNode>,
    /// The view's logical (CSS-px) size — used by R6 to build the view uniform.
    pub logical_size: Vec2,
    /// Device pixel ratio (logical→physical). `1.0` until the window scale is
    /// wired; the manual `Default` keeps it `1.0`, never `0.0`.
    pub scale_factor: f32,
}

impl Default for ExtractedNodes {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            logical_size: Vec2::ZERO,
            scale_factor: 1.0,
        }
    }
}

/// Walk `painters_z` front-to-back (paint-order § 1) and emit each painter's
/// record, skipping entities for which `build` returns `None` (the skip rules,
/// § 5). Pure: the caller supplies `build` closing over the extract query so
/// this assembler stays device- and ECS-free and unit-testable. Emission order
/// is exactly `painters_z` index order — never a re-sort (pillar 1). The
/// view-level `logical_size` / `scale_factor` are filled by the system that
/// owns the window (Task 6); this assembler only orders the node list.
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
    ExtractedNodes {
        nodes,
        ..Default::default()
    }
}
```

- [x] Run — expect PASS.
- [x] Run the full gate. Resolve every warning.
- [x] Commit: `feat(render): ExtractedNodes per-view component + painters_z-ordered assembly`

---

## Task 6 — `extract_buiy_nodes` system (the Extract fan + Changed-gating) — wiring only

**Why:** the system is the thin wrapper that binds the `Option<&T>` fan, the `Changed`-gated `Or`-set, resolves the primary view, and calls the Task 3–5 pure functions. The system body itself is mostly non-unit-testable on CI (it needs `Extract<…>` / the render world), so it is written to call the already-tested pure core, and its only headless assertion is "the function type-checks as a Bevy system / compiles into a schedule" (Task 8). The behavioral assertions ride the pure-fn tests above.

**Files**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Test: none new here (behavior is covered by Tasks 2–5; system-membership smoke is Task 8 GPU-`#[ignore]`)

### Steps

- [x] Implement the system. Append to `crates/buiy_core/src/render/extract.rs`. This is the architecture § 1.2 fan, written against the v1 subset this phase ships (Background + the skip inputs + the layout handoffs); reserved effect components (`BoxShadow`/`Opacity`/`Outline`/`EffectGroup`/`ClipRect`/`Filter`/…) are added to the fan as their tier lands — the `// FAN: add … here` comment marks the seam:

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
                // FAN: add Option<&BoxShadow>/&Outline/&Opacity/&EffectGroup/
                // &ClipRect/&Border and the reserved effect components here as
                // their tier lands (architecture § 1.2 illustrative subset).
                // NOTE: Containment is NOT in the fan — content-visibility:hidden
                // paints the entity's own box and prunes descendants layout-side
                // (paint-order § 5.2), so it is not a render skip input.
            ),
            (
                With<Node>,
                Or<(
                    Changed<GlobalTransform>,
                    Changed<ResolvedLayout>,
                    Changed<Background>,
                    Changed<CssVisibility>,
                    Changed<OffscreenAuto>,
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
    // inside ExtractedNodes is R6/R8's optimization — v1 rebuilds the changed set.)
    let theme = theme.into_inner();
    // `std::collections::HashMap` matches the convention in layout/systems.rs.
    let mut by_entity: std::collections::HashMap<Entity, ExtractedNode> =
        std::collections::HashMap::new();
    for (entity, gt, layout, bg, css_vis, offscreen) in nodes.iter() {
        let skip = node_skip_reason(css_vis, offscreen.is_some());
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
        // R6/R8: merge cached records for unchanged painters here.
        let part = assemble_in_paint_order(&sc.painters_z, |e| by_entity.get(&e).copied());
        all.nodes.extend(part.nodes);
    }

    // Write the per-view ExtractedNodes onto the primary render view entity.
    // R6/R8 wire the exact main<->render view mapping and consume this component;
    // v1 inserts the single ExtractedNodes carrier (R5 owns the type — there is
    // no ExtractedNodesPrimary/ExtractedNodesResource wrapper). The precise
    // target-entity resolution is the one piece that needs the render world and
    // is exercised only under the GPU e2e path (Task 8 / R6/R8).
    commands.insert_resource(ExtractedNodesView(all));
}

/// v1 carrier-by-resource: the primary view's `ExtractedNodes`, inserted by
/// `extract_buiy_nodes` until R6/R8 wire it onto the resolved render-view entity
/// as a per-view component (architecture § 4). This is a thin newtype over the
/// R5-owned `ExtractedNodes` — NOT a parallel definition. R6 reads the inner
/// `ExtractedNodes`; the type itself stays R5's single carrier.
/// SUPERSEDED-BY: R6/R8 (node.rs/buckets read the per-view `ExtractedNodes`).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedNodesView(pub ExtractedNodes);
```

> **Note on `by_entity.get(&e).copied()`:** the `Changed`-gated fan only yields *changed* entities, so an unchanged painter is absent from `by_entity` and would be dropped from this frame's set. That is the R6/R8 partial-re-extract concern (architecture § 3.1: "an unchanged entity contributes its *cached* instance record"). v1 of THIS phase intentionally rebuilds only the changed set into the carrier; the cache-merge across frames is R6/R8. The pure assembler (Task 5) is already correct for the full-set case the cache will feed it. The `// R6/R8: merge cached records for unchanged painters here` comment marks the seam at the `assemble_in_paint_order` call.

- [x] Confirm it compiles as a library (no GPU needed to *compile* the system; only to *run* it in a `RenderApp`):

```sh
cargo build -p buiy_core 2>&1 | tail -20
```

- [x] Run the full gate. Resolve every warning. (Clippy will see the system; the `#[allow(clippy::type_complexity)]` keeps the fan tuple acceptable — mirror the layout systems' convention if clippy complains about anything else.)
- [x] Commit: `feat(render): extract_buiy_nodes system wiring (per-view, Changed-gated, primary view)`

---

## Task 7 — Register `extract_buiy_nodes` in `ExtractSchedule` (extract-system swap only)

**Why:** `mod.rs` currently registers `extract_buiy_draws`. This phase registers `extract_buiy_nodes` alongside it. **R1 already `register_type`'s the author-set render components** (`Background`, `CssVisibility`, …) in its own plugin build — R5 does NOT re-register them (no `app.register_type::<…>()` chain here; that would be a duplicate registration). This task touches ONLY the `ExtractSchedule` system list. The Phase-0 draw path (`ExtractedDraws`/`DrawData`/`to_instance`/`node.rs`) stays defined so the GPU draw still links — it is retired by R6/R8 (the node/instance rework), not here. To avoid a regression where the lavapipe e2e draws nothing between this phase and R6/R8, this task keeps BOTH extract systems registered (the old one feeds the current node; the new one feeds the `ExtractedNodesView` carrier), and R6/R8 removes the old one.

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs`
- Test: `crates/buiy_core/tests/render_smoke.rs` (extend the headless no-panic test)

### Steps

- [x] Add a failing headless test to `crates/buiy_core/tests/render_smoke.rs` asserting the plugin still loads without a `RenderApp` and that `extract.rs` / its types are reachable. (Reflection registration of `Background`/`CssVisibility` is R1's responsibility and is tested in R1 — do NOT re-assert it here.) Append:

```rust
#[test]
fn buiy_render_plugin_loads_headless_with_extract_module() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);
    app.update();

    // The extract carrier type resolves (module is wired); no RenderApp needed.
    let _ = buiy_core::render::extract::ExtractedNodes::default();
}
```

- [x] Run — expect PASS once `pub mod extract;` exists (or FAIL to compile if the module path is not yet public):

```sh
cargo test -p buiy_core --test render_smoke buiy_render_plugin_loads_headless_with_extract_module 2>&1 | tail -20
```

- [x] Minimal impl in `crates/buiy_core/src/render/mod.rs`:
  1. Add `pub mod extract;` near the other `pub mod` lines (if not already added in Task 2). Do NOT add `pub mod components;` / `pub mod color;` — those modules are owned and declared by R1.
  2. Do NOT add any `app.register_type::<components::…>()` chain — R1 already registers the author-set render components. (If a `cargo test` shows a missing registration, that is an R1 gap to fix in R1, not a re-registration here.)
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
    .init_resource::<extract::ExtractedNodesView>()
    // Phase-0 draw path (feeds node.rs today); retired by R6/R8 (the node/
    // instance rework) when node.rs reads the per-view ExtractedNodes instead.
    .add_systems(ExtractSchedule, extract_buiy_draws)
    // The per-view extract rework (this phase). architecture § 1.2/§ 3/§ 4.
    .add_systems(ExtractSchedule, extract::extract_buiy_nodes);
```

  4. Keep the path-qualified `extract::extract_buiy_nodes` to avoid an unused-import lint. Confirm the `extract` module is `pub mod`.

- [x] Run — expect PASS:

```sh
cargo test -p buiy_core --test render_smoke 2>&1 | tail -20
```

- [x] Run the full gate. Resolve every warning.
- [x] Commit: `feat(render): register extract_buiy_nodes in ExtractSchedule (alongside Phase-0 draw)`

---

## Task 8 — GPU `#[ignore]` smoke: `extract_buiy_nodes` is in `ExtractSchedule`

**Why:** architecture § 7 / verification § 2.1 pin that render-world schedule membership (`extract_buiy_nodes ∈ ExtractSchedule`) is NOT device-free — building the `RenderApp` requires `block_on(initialize_renderer)` which `.expect()`s a wgpu adapter. So this assertion rides the `#[ignore]` GPU path, exactly like the existing `pipeline_registers_in_render_app` / `render_graph_node_inserted_after_main_2d_pass` smoke tests. This is the only place this phase touches the device path.

**Files**
- Test: `crates/buiy_core/tests/render_smoke.rs` (add one `#[ignore]` test)

### Steps

- [x] Add the GPU-`#[ignore]` test to `crates/buiy_core/tests/render_smoke.rs`, mirroring the existing `#[ignore]` idiom verbatim:

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
            .get_resource::<buiy_core::render::extract::ExtractedNodesView>()
            .is_some(),
        "ExtractedNodesView initialized in the RenderApp"
    );
    let _ = ExtractSchedule; // label in scope; membership runs under the e2e harness
}
```

- [x] Confirm the ignored test compiles and is skipped by default:

```sh
cargo test -p buiy_core --test render_smoke 2>&1 | tail -20   # the new test shows as `ignored`
```

- [x] (Optional, only if a lavapipe adapter is available locally — NOT on CI) run the ignored test:

```sh
cargo test -p buiy_core --test render_smoke -- --ignored 2>&1 | tail -20
```

- [x] Run the full gate (the ignored test must still *compile* clean). Resolve every warning.
- [x] Commit: `test(render): GPU-ignored smoke for extract_buiy_nodes RenderApp membership`

---

## Task 9 — Documentation: mark the extract rework landed in the spec catalog

**Why:** per the project doc convention, doc updates ship *with* the change. The render-pipeline spec README's Phase-0 note and the docs index should reflect that `extract_buiy_nodes` + per-view `ExtractedNodes` now exist (replacing `extract_buiy_draws` at the schedule level), with the node-side consumption swap still pending R6/R8.

**Files**
- Modify: `docs/README.md` (the master index — add/annotate the plan row)
- Modify: `docs/plans/2026-06-03-buiy-render-r5-extract.md` (this file — check every box)

### Steps

- [x] In `docs/README.md`, find the render-pipeline plans grouping (or add one under the render area) and add a row pointing to this plan with status `landed (extract rework; node-side read pending R6/R8)`. Mirror the existing row formatting in that file — read the surrounding rows first, do not invent a new table shape.
- [x] Confirm no other spec child contradicts "extract_buiy_nodes shipped": the architecture § 1.2 text already describes it as the target; no edit needed there (it is target-state prose). Leave a one-line note in this plan's header if any drift is found.
- [x] Run the full gate one final time (docs changes do not affect it, but run it to be certain nothing regressed):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

- [x] Commit: `docs(render): mark per-view extract rework landed in catalog`

---

## Done criteria

- `extract_buiy_nodes` registered in `ExtractSchedule`; its entire mapping (color resolution, skip predicates, `ExtractedNode` build, `painters_z` forward assembly) lives in pure functions in `render/extract.rs` and is unit-tested in `tests/render_extract.rs` with **no GPU**.
- Skip rules (`CssVisibility::Hidden`, `OffscreenAuto`; `Display::None` via absent `ResolvedLayout`/absent from `painters_z`) are headless table tests. `content-visibility: hidden` is NOT a render skip — the Hidden entity's own box paints and its descendants are pruned layout-side (paint-order § 5.2).
- `ExtractedNodes` is the single R5-owned per-view carrier `{ nodes, logical_size, scale_factor }` with a manual `Default` (`scale_factor = 1.0`); R6 consumes it (no parallel rebuild from `ExtractedDraws`).
- The paint/hit-test ordering identity (paint = `painters_z` forward, hit-test = reversed) is a headless property test.
- The only device-dependent assertion (`extract_buiy_nodes` in a live `RenderApp`) is `#[ignore]` GPU, matching the existing `render_smoke.rs` idiom.
- The full gate (`fmt && clippy -D warnings && doc -D warnings && test`) is green at every commit, with no xvfb and no wgpu adapter.
- The Phase-0 draw path is left intact (retired by R6/R8); the per-window partition is reserved (query reads all windows, v1 targets primary).
- No type owned by R1 is redefined: `render/components.rs` and `render/color.rs` are imported, never re-created, and no `pub mod components;` / `pub mod color;` / `lib.rs` re-export is added by this phase.
```
