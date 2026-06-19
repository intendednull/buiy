# Clip Render-Prep (WriteClipRects) Implementation Plan

**Date:** 2026-06-03
**Status:** landed

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** R1 (component model — owns `render/components.rs`, the sole home of `ClipRect` / `AncestorClip`). Execution order: R1 → **R2** → R3 → R4 → R5 → R6 → R7 → R8 → (R9, R10) → R11.

**Goal:** Add the `write_clip_rects` render-prep pass that walks the `Children` tree top-down and writes a per-entity `ClipRect` (own box ∩ ancestor clips) plus an ancestor-only `AncestorClip`, scheduled between `BuiySet::Animate` and `BuiySet::Picking`, with a scroll-only recompute that never re-runs layout.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [clip-and-transform.md](../specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md) § A (the `WriteClipRects` pass, `ClipRect`, `AncestorClip`).
**Architecture:** `ClipRect` / `AncestorClip` are render-owned computed `Component`s **defined by R1** in `crates/buiy_core/src/render/components.rs` — lean plain structs `{ pub min: Vec2, pub max: Vec2 }`, no `Reflect`, NOT `register_type`'d (spec clip-and-transform.md § A.2 + component-model.md § 13). They have no `Default` (an absent `ClipRect`/`AncestorClip` means "no clip"). **R2 does NOT define, re-export, or register these types — it imports them from `crate::render::components` and contributes only `write_clip_rects` plus the field *population* (the computed clip values).** A single render-prep ECS system `write_clip_rects` runs in `Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`, doing a top-down `Children` walk: it intersects each entity's own border box with each ancestor's clip box (overflow-clip padding box per-axis, scroll-container viewport padding box, nearest `contain: paint` border box), pruning `Display::None` / `ContentVisibility::Hidden` subtrees. `ClipRect` is the ancestor intersection ∩ own box; `AncestorClip` is the ancestor intersection alone (for `Outline`). Absent component ⇔ no ancestor clips. The walk is change-gated against the entity's previously-written `ClipRect` so a steady-state frame issues zero structural ops; `ScrollOffset` is **not** a clip-box input (scroll moves content, not the clip), so a scroll-only frame neither changes a clip box nor touches `ResolvedLayout`.
**Tier/Test reality:** HEADLESS. `write_clip_rects` is a plain `Update` ECS system — every gating test runs on this host / CI via `App::new() + MinimalPlugins + CorePlugin + LayoutPlugin` with **no** wgpu adapter and **no** xvfb. There is no GPU-`#[ignore]` test in this phase; the GATE below must stay green for every commit.

THE GATE (run before every commit; must be green — this host + CI have NO xvfb and NO wgpu adapter):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

Repo worktree root: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline`

---

## Conventions used by every task

- All paths are absolute from the worktree root above.
- Each task is RED → GREEN → COMMIT. Write the failing test first, run it, see it fail for the stated reason, then write the minimal impl, run it green, run THE GATE, commit.
- `ClipRect` / `AncestorClip` are **defined by R1** in `crates/buiy_core/src/render/components.rs` (the render-component home R1 creates) and are re-exported from `crate::render::components` / the crate root by R1. R2 imports them — it does NOT add a definition, a `pub mod`, a `pub use`, or a `register_type` for them.
- The pass + its public system fn live in a new module `crates/buiy_core/src/render/clip.rs`, re-exported from `crates/buiy_core/src/render/mod.rs`.
- Integration tests live in `crates/buiy_core/tests/render_clip_rects.rs` and mirror the layout snapshot idiom in `tests/layout_containment.rs` / `tests/layout_scroll_offset_no_invalidate.rs`: build `App::new()`, add `MinimalPlugins`, `CorePlugin`, `LayoutPlugin`, the `BuiyRenderPlugin` (its `build` is a clean no-op without a RenderApp — see `tests/render_smoke.rs`), `app.update()`, then read components off entities.
- `BuiyRenderPlugin` (in `render/mod.rs`) is where the **main-world** schedule wiring for `write_clip_rects` is added — its current `build` early-returns when there is no RenderApp, so a new **main-world** `add_systems(Update, …)` must be added *before* that early-return (Task 6 handles this precisely).
- Geometry helper: a clip box is an AABB `{ min: Vec2, max: Vec2 }`. Intersection of two AABBs is `min = a.min.max(b.min)`, `max = a.max.min(b.max)` (component-wise). Border-only inset of a border box `{pos, size}` by `BoxModel.border` edges (resolved to px) produces the **padding box**.
- `BoxModel.border` is `Edges { top, right, bottom, left }` of `Length`. v1 layout only emits `Length::Px` borders into Taffy; resolve each edge with the helper `px_edge(len) = match len { Length::Px(v) => v, _ => 0.0 }` (non-px border units are not produced by current layout; a `_ => 0.0` is correct and matches the spec absent-default "no border inset" intent — do NOT add unit resolution this phase, that is layout's job).

---

## Prerequisite (R1) — `ClipRect` / `AncestorClip` already exist; do NOT redefine them

`ClipRect` and `AncestorClip` are **created and owned by R1** in
`crates/buiy_core/src/render/components.rs`. Per the canonical-ownership
sheet they are lean computed structs, distinct types:

```rust
// Defined by R1 in render/components.rs — DO NOT re-add here.
pub struct ClipRect { pub min: Vec2, pub max: Vec2 }
pub struct AncestorClip { pub min: Vec2, pub max: Vec2 }
```

They derive `Clone, Copy, Debug, PartialEq` and have **no `Default`** (an
absent component means "no clip"). They are **not** `Reflect`, **not**
`register_type`'d, and **not** re-exported via a new `pub use` in this
phase — R1 does all of that. The registry test in R1 asserts these types
are ABSENT from the type registry; that is correct and intentional.

**R2's job for these types is import + population only.** Everywhere this
plan uses `ClipRect` / `AncestorClip`, import them with:

```rust
use crate::render::components::{AncestorClip, ClipRect};
```

(in integration tests: `use buiy_core::{AncestorClip, ClipRect, …};` —
R1's crate-root re-export). If R1 has not landed when you start, **do
not** create these types as a workaround; R1 is a hard precondition
(execution order R1 → R2). Confirm `crate::render::components::ClipRect`
resolves before writing any task below.

There is no separate "define the component" task in R2 — the first
behavioral task is the `clip.rs` geometry skeleton below.

---

## Task 1 — Clip geometry helpers (`clip.rs` module skeleton)

**Files**
- Create: `crates/buiy_core/src/render/clip.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (`pub mod clip;`)
- Test: `crates/buiy_core/src/render/clip.rs` (unit tests in a `#[cfg(test)] mod tests`)

Pure CPU geometry: AABB construction from `ResolvedLayout`, AABB intersection, and the border-only inset (border box → padding box). These have no ECS, no GPU — unit-test them directly. HEADLESS.

- [ ] RED — create `crates/buiy_core/src/render/clip.rs` with ONLY the test module and the function signatures stubbed to `unimplemented!()` so the test compiles and fails at runtime:

```rust
//! `write_clip_rects` render-prep pass: per-entity clip AABB computation.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.

use crate::layout::{Edges, Length}; // both re-exported from `crate::layout` (the `mod types` items are not pub-reachable as `crate::layout::types::` from a sibling module)
use bevy::math::Vec2;

/// An axis-aligned clip box in logical px (y-down, window-relative).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Aabb {
    min: Vec2,
    max: Vec2,
}

impl Aabb {
    /// The border box of an entity: top-left `position`, extent `size`.
    fn from_box(position: Vec2, size: Vec2) -> Self {
        unimplemented!()
    }

    /// Component-wise AABB intersection (may be degenerate if disjoint).
    fn intersect(self, other: Aabb) -> Aabb {
        unimplemented!()
    }

    /// Inset by border edges only (border box → padding box). `border`
    /// edges are resolved px-only (non-px units → 0.0, matching the
    /// spec absent-default: a missing/unsupported border contributes no
    /// inset).
    fn inset_border(self, border: &Edges) -> Aabb {
        unimplemented!()
    }
}

/// Resolve a border-edge `Length` to px. Layout emits only `Length::Px`
/// borders into Taffy; any other unit means "no border inset here".
fn px_edge(len: Length) -> f32 {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Edges, Length};

    #[test]
    fn from_box_makes_min_max_from_pos_size() {
        let a = Aabb::from_box(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        assert_eq!(a.min, Vec2::new(10.0, 20.0));
        assert_eq!(a.max, Vec2::new(110.0, 70.0));
    }

    #[test]
    fn intersect_takes_inner_overlap() {
        let a = Aabb { min: Vec2::new(0.0, 0.0), max: Vec2::new(100.0, 100.0) };
        let b = Aabb { min: Vec2::new(50.0, 25.0), max: Vec2::new(200.0, 75.0) };
        let i = a.intersect(b);
        assert_eq!(i.min, Vec2::new(50.0, 25.0));
        assert_eq!(i.max, Vec2::new(100.0, 75.0));
    }

    #[test]
    fn inset_border_makes_padding_box() {
        // border box (0,0)-(100,100), 10px border every side → (10,10)-(90,90).
        let bb = Aabb { min: Vec2::ZERO, max: Vec2::splat(100.0) };
        let padding = bb.inset_border(&Edges::all(10.0));
        assert_eq!(padding.min, Vec2::splat(10.0));
        assert_eq!(padding.max, Vec2::splat(90.0));
    }

    #[test]
    fn px_edge_resolves_px_only() {
        assert_eq!(px_edge(Length::Px(7.0)), 7.0);
        assert_eq!(px_edge(Length::Percent(50.0)), 0.0);
    }
}
```

- [ ] Add `pub mod clip;` to `crates/buiy_core/src/render/mod.rs` (next to `pub mod instance; pub mod node; pub mod pipeline;`).
- [ ] Run RED — `cargo test -p buiy_core --lib render::clip` → expect FAIL: each test panics on `unimplemented!()`.

- [ ] GREEN — fill in the four function bodies in `crates/buiy_core/src/render/clip.rs`:

```rust
impl Aabb {
    fn from_box(position: Vec2, size: Vec2) -> Self {
        Self { min: position, max: position + size }
    }

    fn intersect(self, other: Aabb) -> Aabb {
        Aabb {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        }
    }

    fn inset_border(self, border: &Edges) -> Aabb {
        Aabb {
            min: Vec2::new(self.min.x + px_edge(border.left), self.min.y + px_edge(border.top)),
            max: Vec2::new(self.max.x - px_edge(border.right), self.max.y - px_edge(border.bottom)),
        }
    }
}

fn px_edge(len: Length) -> f32 {
    match len {
        Length::Px(v) => v,
        _ => 0.0,
    }
}
```

(Remove the `unimplemented!()` stubs.)

- [ ] Run GREEN — `cargo test -p buiy_core --lib render::clip` → all four PASS.
- [ ] Run THE GATE → green. (If clippy flags `Aabb`/helpers as dead code because nothing outside `tests` uses them yet, that is expected only until Task 4 — to keep this commit green, add `#[allow(dead_code)]` on `struct Aabb`, its `impl`, and `px_edge`; Task 4 removes the allow when `write_clip_rects` consumes them. Note this in the commit body.)
- [ ] Commit: `feat(render): add clip AABB geometry helpers (intersect, border inset)`

---

## Task 2 — `write_clip_rects`: own-box-only `ClipRect` for a single unclipped node emits nothing

**Files**
- Modify: `crates/buiy_core/src/render/clip.rs` (add the system)
- Test: `crates/buiy_core/tests/render_clip_rects.rs` (create)

First behavioral slice: the top-down walk that, for a lone node with **no clipping ancestor**, emits **no** `ClipRect` (absent ⇔ no clip). This pins the absent-when-unclipped contract and stands up the `Query` shape + walk scaffold. HEADLESS.

- [ ] RED — create `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
//! Render-prep clip pass (`write_clip_rects`) — clip AABB geometry.
//!
//! HEADLESS: plain `Update` ECS system, no wgpu adapter. Build the app
//! with MinimalPlugins + CorePlugin + LayoutPlugin + BuiyRenderPlugin
//! (its build is a no-op without a RenderApp — see render_smoke.rs).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.

use bevy::prelude::*;
use buiy_core::{
    AncestorClip, ClipRect, CorePlugin, Node, Overflow, OverflowMode,
    layout::{LayoutPlugin, Style},
    render::BuiyRenderPlugin,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyRenderPlugin);
    app
}

#[test]
fn unclipped_node_emits_no_clip_rect() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();
    app.update();
    assert!(
        app.world().get::<ClipRect>(e).is_none(),
        "a node with no clipping ancestor must have NO ClipRect"
    );
    assert!(
        app.world().get::<AncestorClip>(e).is_none(),
        "a node with no clipping ancestor must have NO AncestorClip"
    );
}
```

- [ ] Run RED — `cargo test -p buiy_core --test render_clip_rects unclipped_node_emits_no_clip_rect`. Expected: the test **compiles** but **fails** because `write_clip_rects` does not exist / is not scheduled yet, OR fails to compile if the system isn't referenced. To make RED meaningful, this task adds the system AND its schedule wiring is deferred to Task 6 — so the system must be added here but NOT yet scheduled. Therefore RED here is: assert it compiles and the test passes *trivially* (no `ClipRect` written because nothing runs). That is a weak RED. Instead, fold the schedule wiring into THIS task so RED is real: see GREEN steps below; before GREEN the test fails to compile on the missing `write_clip_rects` import path used by Task 6. To get a clean RED for behavior, temporarily wire the system in this task (Task 6 only *re-confirms* placement).

  Concretely: run the command above now → expect FAIL (compile error: unresolved import for the not-yet-created system, or, once the empty system exists but is unscheduled, the test passes spuriously). The acceptance for RED is a **non-passing** run; if it passes spuriously, the implementer has wired GREEN already — proceed.

- [ ] GREEN — add the system to `crates/buiy_core/src/render/clip.rs` and schedule it in `BuiyRenderPlugin`:

In `clip.rs`, add the public system (remove the Task-3 `#[allow(dead_code)]` now that the helpers are used):

```rust
use crate::components::{Node, ResolvedLayout};
// ClipRect / AncestorClip are owned by R1 — import, never redefine.
use crate::render::components::{AncestorClip, ClipRect};
use crate::layout::{
    BoxModel, ContainFlags, Containment, ContentVisibility, Display, Edges, Overflow, OverflowMode,
};
use bevy::prelude::*;

/// Render-prep — computes each entity's `ClipRect` (own box ∩ ancestor
/// clips) and `AncestorClip` (ancestor clips only) by a top-down
/// `Children` walk. Reads only layout output + the per-node clip inputs;
/// writes/removes `ClipRect` / `AncestorClip`. Emits NO component when no
/// ancestor clips the entity.
///
/// Runs in `Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`
/// (architecture.md § 5.2). Reads no `ScrollOffset` — the clip box is
/// scroll-offset-independent (spec § A.4); scroll moves content via the
/// transform bridge, never the clip box. Prunes `Display::None` /
/// `ContentVisibility::Hidden` subtrees (spec § A.3, shared with
/// paint-order § 5).
///
/// Spec: clip-and-transform.md § A.3.
pub fn write_clip_rects(
    mut commands: Commands,
    roots: Query<Entity, (With<Node>, Without<ChildOf>)>,
    children: Query<&Children>,
    nodes: Query<(
        &ResolvedLayout,
        Option<&BoxModel>,
        Option<&Overflow>,
        Option<&Containment>,
        Option<&Display>,
    )>,
    existing: Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    for root in roots.iter() {
        walk(root, None, &mut commands, &children, &nodes, &existing);
    }
}

/// Carries the running ancestor-clip AABB (`None` = no ancestor clips yet)
/// down the tree, writing each entity's `ClipRect` / `AncestorClip`.
#[allow(clippy::too_many_arguments)]
fn walk(
    entity: Entity,
    ancestor: Option<Aabb>,
    commands: &mut Commands,
    children: &Query<&Children>,
    nodes: &Query<(
        &ResolvedLayout,
        Option<&BoxModel>,
        Option<&Overflow>,
        Option<&Containment>,
        Option<&Display>,
    )>,
    existing: &Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    let Ok((rl, box_model, overflow, containment, display)) = nodes.get(entity) else {
        return;
    };
    // Prune Display::None / ContentVisibility::Hidden subtrees (spec § A.3).
    if matches!(display, Some(Display::None)) {
        return;
    }
    if let Some(c) = containment {
        if c.content_visibility == ContentVisibility::Hidden {
            return;
        }
    }

    // (Own-box and ancestor-contribution math arrive in Tasks 5/7/8/9.
    //  This slice: a lone unclipped node has `ancestor == None`, so emit
    //  nothing and reconcile away any stale components.)
    let own = Aabb::from_box(rl.position, rl.size);
    let clip: Option<Aabb> = ancestor.map(|a| a.intersect(own));

    reconcile(entity, clip, ancestor, commands, existing);

    // This node's contribution to its descendants' ancestor clip is added
    // in later tasks; for now pass the running ancestor through unchanged.
    let child_ancestor = ancestor;
    if let Ok(kids) = children.get(entity) {
        for &child in kids.iter() {
            walk(child, child_ancestor, commands, children, nodes, existing);
        }
    }
}

/// Insert/remove `ClipRect` (from `clip`) and `AncestorClip` (from
/// `ancestor`) only when they differ from what is already stored — a
/// steady-state frame issues zero structural ops.
fn reconcile(
    entity: Entity,
    clip: Option<Aabb>,
    ancestor: Option<Aabb>,
    commands: &mut Commands,
    existing: &Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    let (prev_clip, prev_anc) = existing.get(entity).unwrap_or((None, None));

    match (clip, prev_clip) {
        (Some(a), prev) => {
            let next = ClipRect { min: a.min, max: a.max };
            if prev != Some(&next) {
                commands.entity(entity).insert(next);
            }
        }
        (None, Some(_)) => {
            commands.entity(entity).remove::<ClipRect>();
        }
        (None, None) => {}
    }

    match (ancestor, prev_anc) {
        (Some(a), prev) => {
            let next = AncestorClip { min: a.min, max: a.max };
            if prev != Some(&next) {
                commands.entity(entity).insert(next);
            }
        }
        (None, Some(_)) => {
            commands.entity(entity).remove::<AncestorClip>();
        }
        (None, None) => {}
    }
}
```

In `crates/buiy_core/src/render/mod.rs`, add the main-world schedule wiring at the **top** of `BuiyRenderPlugin::build`, *before* the `get_sub_app_mut(RenderApp)` early-return:

```rust
impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        // Main-world render-prep: clip computation runs between Animate and
        // Picking (architecture.md § 5.2), so picking + extract see settled
        // ClipRects. This runs on CI/headless — no RenderApp required.
        app.add_systems(
            Update,
            crate::render::clip::write_clip_rects
                .after(crate::BuiySet::Animate)
                .before(crate::BuiySet::Picking),
        );

        // ExtractedDraws is render-world only — the main world does not read it.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        // … existing render-app wiring unchanged …
```

Also re-export the system from `mod.rs`: `pub use clip::write_clip_rects;` (place near the top after `pub mod clip;`).

- [ ] Run GREEN — `cargo test -p buiy_core --test render_clip_rects unclipped_node_emits_no_clip_rect` → PASS.
- [ ] Run THE GATE → green. (Resolve any unused-import warnings in `clip.rs` — `ContainFlags` and `Overflow`/`BoxModel` reads land in Tasks 5/7/8; if clippy flags them unused now, scope the imports to the tasks that use them or add a focused `#[allow(unused_imports)]` with a `// used in Task 5/7/8` note and remove it there. Prefer adding imports task-by-task so the gate stays warning-clean.)
- [ ] Commit: `feat(render): write_clip_rects scaffold + absent-when-unclipped, scheduled after Animate before Picking`

---

## Task 3 — Single `overflow: hidden` ancestor clips its child to the padding box

**Files**
- Modify: `crates/buiy_core/src/render/clip.rs` (ancestor contribution: overflow clip)
- Test: `crates/buiy_core/tests/render_clip_rects.rs`

Now the walk must add a clipping ancestor's padding box to the running clip. A child of an `overflow: hidden` parent gets `ClipRect = child_box ∩ parent_padding_box` and `AncestorClip = parent_padding_box`. HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
#[test]
fn child_of_overflow_hidden_is_clipped_to_parent_padding_box() {
    let mut app = app();
    // Parent: 100x100 at origin, 10px border every side, overflow hidden.
    // Padding box = (10,10)-(90,90).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .border(10.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    // Child: 200x200 — overflows the parent on both axes.
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    // Child clip = child_box (its own pos/size) ∩ parent padding box.
    let clip = app.world().get::<ClipRect>(child).expect("child clipped");
    // Child position is laid out inside the parent's content box; assert the
    // clip is bounded by the parent padding box (the binding constraint).
    assert_eq!(clip.max, Vec2::new(90.0, 90.0), "clamped to parent padding max");
    assert!(clip.min.x >= 10.0 && clip.min.y >= 10.0, "clamped to parent padding min");

    let anc = app.world().get::<AncestorClip>(child).expect("child has ancestor clip");
    assert_eq!(anc.min, Vec2::new(10.0, 10.0), "ancestor clip = parent padding min");
    assert_eq!(anc.max, Vec2::new(90.0, 90.0), "ancestor clip = parent padding max");

    // The parent itself has no clipping ancestor → no ClipRect.
    assert!(app.world().get::<ClipRect>(parent).is_none(), "parent unclipped");
}
```

- [ ] Run RED — `cargo test -p buiy_core --test render_clip_rects child_of_overflow_hidden_is_clipped_to_parent_padding_box` → FAIL (child has no `ClipRect`: the walk currently passes `ancestor` through unchanged).

- [ ] GREEN — in `clip.rs`, replace the `let child_ancestor = ancestor;` line with the real contribution logic. Add a helper that computes the clip box THIS entity imposes on its descendants and folds it into the running ancestor AABB:

```rust
    // What clip box does THIS node impose on its descendants?
    let own_contribution = clip_contribution(own, box_model, overflow, containment);
    let child_ancestor = match (ancestor, own_contribution) {
        (Some(a), Some(c)) => Some(a.intersect(c)),
        (None, Some(c)) => Some(c),
        (some, None) => some,
    };
```

Add the helper at module scope:

```rust
/// The clip box `entity` imposes on its descendants, or `None` if it does
/// not clip. Overflow contributes the padding box per clipping axis;
/// `contain: paint` contributes the border box (Tasks 7/8 add scroll +
/// paint). A `Visible` overflow axis contributes no bound on that axis.
fn clip_contribution(
    own: Aabb,
    box_model: Option<&BoxModel>,
    overflow: Option<&Overflow>,
    _containment: Option<&Containment>,
) -> Option<Aabb> {
    let zero = Edges::default();
    let border = box_model.map(|b| &b.border).unwrap_or(&zero);
    let padding = own.inset_border(border);

    // Per-axis overflow: a Visible axis leaves that axis unbounded
    // (±infinity), only a clipping axis (not Visible) binds.
    let o = overflow.copied().unwrap_or_default();
    let x_clips = !matches!(o.x, OverflowMode::Visible);
    let y_clips = !matches!(o.y, OverflowMode::Visible);

    if !x_clips && !y_clips {
        return None;
    }
    Some(Aabb {
        min: Vec2::new(
            if x_clips { padding.min.x } else { f32::NEG_INFINITY },
            if y_clips { padding.min.y } else { f32::NEG_INFINITY },
        ),
        max: Vec2::new(
            if x_clips { padding.max.x } else { f32::INFINITY },
            if y_clips { padding.max.y } else { f32::INFINITY },
        ),
    })
}
```

`OverflowMode`, `Edges`, `ContainFlags`, and `ContentVisibility` are already in the Task-4 consolidated import block (`use crate::layout::{BoxModel, ContainFlags, Containment, ContentVisibility, Display, Edges, Overflow, OverflowMode};`); no new import line is needed. (The `clip.rs` `tests` module's `use crate::layout::{Edges, Length};` stays as added in Task 3.)

- [ ] Run GREEN — `cargo test -p buiy_core --test render_clip_rects child_of_overflow_hidden_is_clipped_to_parent_padding_box` → PASS. Also re-run `unclipped_node_emits_no_clip_rect` → still PASS.
- [ ] Run THE GATE → green.
- [ ] Commit: `feat(render): overflow-clip contribution (child clipped to ancestor padding box)`

---

## Task 6 — Confirm scheduling: `write_clip_rects` runs after `Animate`, before `Picking`

**Files**
- Test: `crates/buiy_core/tests/render_clip_rects.rs` (schedule-membership assertion)

The schedule edge was wired in Task 4; this task pins it with a dedicated ordering test mirroring `tests/system_set_order.rs`. The naive placement to catch is "inside `BuiySet::Render`" or "after `Picking`" — both would make picking read stale/absent `ClipRect`s. HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
use bevy::ecs::schedule::NodeId;

/// Assert `write_clip_rects` is ordered after BuiySet::Animate and before
/// BuiySet::Picking in the Update schedule's toposort.
#[test]
fn write_clip_rects_runs_between_animate_and_picking() {
    let mut app = app();
    app.update(); // force Schedule::initialize → toposort

    let schedules = app.world().resource::<Schedules>();
    let schedule = schedules.get(Update).expect("Update schedule");
    let graph = schedule.graph();
    let toposort = graph
        .dependency()
        .get_toposort()
        .expect("toposorted after initialize");

    let set_pos = |set: buiy_core::BuiySet| -> usize {
        let key = graph
            .system_sets
            .get_key(set.intern())
            .expect("set registered");
        toposort
            .iter()
            .position(|n| *n == NodeId::Set(key))
            .expect("set in toposort")
    };

    // Find write_clip_rects' system node by name.
    let sys_pos = toposort
        .iter()
        .filter_map(|n| match n {
            NodeId::System(_) => graph
                .systems()
                .find(|(id, _, _)| NodeId::System(*id) == *n)
                .map(|(_, sys, _)| sys.name().to_string())
                .map(|name| (n, name)),
            _ => None,
        })
        .find(|(_, name)| name.contains("write_clip_rects"))
        .map(|(n, _)| toposort.iter().position(|x| x == n).unwrap())
        .expect("write_clip_rects scheduled on Update");

    let animate = set_pos(buiy_core::BuiySet::Animate);
    let picking = set_pos(buiy_core::BuiySet::Picking);
    assert!(animate < sys_pos, "write_clip_rects must run after Animate ({animate} < {sys_pos})");
    assert!(sys_pos < picking, "write_clip_rects must run before Picking ({sys_pos} < {picking})");
}
```

> If `graph.systems()` / `sys.name()` API shape differs in this Bevy 0.18 build, fall back to the simpler robust assertion: introspect `set_pos` only and assert membership via running an ordering probe — i.e. add a tiny test-only resource and two marker systems, or read the toposort node names via `graph.dependency()`. Prefer the name-scan above; adapt the exact accessor names to whatever `system_set_order.rs` already uses successfully (it reads `graph.system_sets.get_key` + `graph.dependency().get_toposort()`, both confirmed present). The system-name scan is the only new accessor; verify it against `bevy_ecs` 0.18 before settling.

- [ ] Run RED — if the wiring from Task 4 is correct this test PASSES immediately (the edge already exists). To make this a meaningful RED→GREEN, temporarily comment out the `.after(Animate).before(Picking)` constraints in `mod.rs` (leave the bare `add_systems(Update, write_clip_rects)`), run the test → expect FAIL (`write_clip_rects must run before Picking` — without the edge the system can land anywhere, frequently after Picking). Then restore the constraints.

- [ ] GREEN — restore the `.after(crate::BuiySet::Animate).before(crate::BuiySet::Picking)` constraints (Task 4 already added them; this confirms they are load-bearing). Run the test → PASS.
- [ ] Run THE GATE → green.
- [ ] Commit: `test(render): pin write_clip_rects schedule between Animate and Picking`

---

## Task 7 — Scroll container clips to its viewport (padding box), independent of `ScrollOffset`

**Files**
- Modify: `crates/buiy_core/src/render/clip.rs` (treat scroll-container as a clipping ancestor)
- Test: `crates/buiy_core/tests/render_clip_rects.rs`

A scroll container (`Overflow::is_scroll_container()`) clips descendants to its **viewport = padding box**, and the clip box is **independent of `ScrollOffset`** (spec § A.4: scroll moves content via the bridge, not the clip box). `Overflow::Scroll` / `Auto` are already not `Visible`, so the per-axis `clip_contribution` from Task 5 *already* binds them — this task adds the explicit scroll-offset-independence test and confirms `Scroll`/`Auto` axes clip. HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
use buiy_core::ScrollOffset;

#[test]
fn scroll_container_clips_to_viewport_independent_of_offset() {
    let mut app = app();
    // Scroll container 100x100, no border → viewport = (0,0)-(100,100).
    let sc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Scroll, OverflowMode::Scroll),
            ScrollOffset::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(sc).add_child(child);
    app.update();

    let clip_before = *app.world().get::<ClipRect>(child).expect("child clipped by scroll viewport");
    assert_eq!(clip_before.max, Vec2::new(100.0, 100.0), "clamped to viewport max");

    // Mutate ScrollOffset — the clip BOX must not move (scroll moves content,
    // not the clip; spec § A.4).
    {
        let mut off = app.world_mut().get_mut::<ScrollOffset>(sc).unwrap();
        off.y = 80.0;
    }
    app.update();
    let clip_after = *app.world().get::<ClipRect>(child).expect("still clipped");
    assert_eq!(
        clip_before, clip_after,
        "scroll-container clip box is offset-independent (A.4)"
    );
}
```

- [ ] Run RED — `cargo test -p buiy_core --test render_clip_rects scroll_container_clips_to_viewport_independent_of_offset`. Expect PASS for the *clamp* (Task 5 already clips non-Visible axes) but this test also guards offset-independence. If it passes outright, that confirms `write_clip_rects` correctly reads **no** `ScrollOffset` (Task 4 query has none). To prove the guard is load-bearing, temporarily add `Option<&ScrollOffset>` to the `nodes` query and shift the contribution by the offset → test FAILS on `clip_after != clip_before`; then revert. Document this RED in the commit body.

- [ ] GREEN — the production code change is to ASSERT-by-construction that `write_clip_rects` reads no `ScrollOffset`. Add an explicit doc comment + a compile-fence: keep the `nodes` query free of `ScrollOffset` and add a `// SPEC § A.4: ScrollOffset is intentionally NOT a clip input.` comment above the `nodes:` query param. No logic change needed if Task 5 already clips `Scroll`/`Auto` axes (verify by running the test green).
- [ ] Run GREEN — test PASSES.
- [ ] Run THE GATE → green.
- [ ] Commit: `feat(render): scroll-container viewport clip is ScrollOffset-independent (spec A.4)`

---

## Task 8 — `contain: paint` ancestor clips descendants to its border box

**Files**
- Modify: `crates/buiy_core/src/render/clip.rs` (paint-containment contribution)
- Test: `crates/buiy_core/tests/render_clip_rects.rs`

A nearest `Containment.contain` including `ContainFlags::PAINT` (or `CONTENT` / `STRICT`, which subsume it) clips descendants to the ancestor's **border box** (not padding box — paint containment clips to the box itself; spec § A.3 rule 3). HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
use buiy_core::ContainFlags;

#[test]
fn contain_paint_ancestor_clips_to_border_box() {
    let mut app = app();
    // Parent: 100x100, 10px border, contain: paint, overflow VISIBLE.
    // Paint containment clips to the BORDER box (0,0)-(100,100), not the
    // padding box — distinguishing it from overflow:hidden.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .border(10.0)
                .contain(ContainFlags::PAINT),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    let anc = app.world().get::<AncestorClip>(child).expect("paint-contained");
    assert_eq!(anc.min, Vec2::ZERO, "paint clip = border box min (NOT padding)");
    assert_eq!(anc.max, Vec2::new(100.0, 100.0), "paint clip = border box max");
}
```

- [ ] Run RED — `cargo test -p buiy_core --test render_clip_rects contain_paint_ancestor_clips_to_border_box` → FAIL (child has no `AncestorClip`: overflow is `Visible`, and `clip_contribution` ignores `containment` — the `_containment` param is unused).

- [ ] GREEN — in `clip_contribution`, rename `_containment` to `containment` and fold the paint-containment border box into the contribution. The contribution is the **intersection** of the overflow padding-box bound and the paint border-box bound (a node can be both an overflow clipper and paint-contained):

```rust
fn clip_contribution(
    own: Aabb,
    box_model: Option<&BoxModel>,
    overflow: Option<&Overflow>,
    containment: Option<&Containment>,
) -> Option<Aabb> {
    let zero = Edges::default();
    let border = box_model.map(|b| &b.border).unwrap_or(&zero);
    let padding = own.inset_border(border);

    let o = overflow.copied().unwrap_or_default();
    let x_clips = !matches!(o.x, OverflowMode::Visible);
    let y_clips = !matches!(o.y, OverflowMode::Visible);

    let overflow_bound = (x_clips || y_clips).then(|| Aabb {
        min: Vec2::new(
            if x_clips { padding.min.x } else { f32::NEG_INFINITY },
            if y_clips { padding.min.y } else { f32::NEG_INFINITY },
        ),
        max: Vec2::new(
            if x_clips { padding.max.x } else { f32::INFINITY },
            if y_clips { padding.max.y } else { f32::INFINITY },
        ),
    });

    // contain: paint clips to the BORDER box (own box, no inset).
    let paint_bound = containment
        .filter(|c| c.contain.contains(ContainFlags::PAINT))
        .map(|_| own);

    match (overflow_bound, paint_bound) {
        (Some(a), Some(b)) => Some(a.intersect(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}
```

(`ContainFlags` is already in the Task-4 consolidated import block; ensure it is present and no longer behind an `#[allow(unused_imports)]`.)

- [ ] Run GREEN — test PASSES. Re-run the whole file: `cargo test -p buiy_core --test render_clip_rects` → all PASS.
- [ ] Run THE GATE → green.
- [ ] Commit: `feat(render): contain:paint contribution (descendants clipped to border box)`

---

## Task 9 — Nested clips intersect; per-axis overflow leaves the Visible axis unbounded

**Files**
- Test: `crates/buiy_core/tests/render_clip_rects.rs`
- Modify: `crates/buiy_core/src/render/clip.rs` only if a test exposes a gap (expected: none — the math already composes).

Two assertions the prior tasks' code should already satisfy; pin them explicitly because they are the load-bearing correctness properties of the walk: (a) nested `overflow:hidden` ancestors **intersect** (grandchild is bounded by the *tighter* of two ancestor boxes), and (b) `overflow-x: hidden; overflow-y: visible` constrains x and leaves y unbounded (no clip introduced on the visible axis). HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
#[test]
fn nested_overflow_hidden_intersects_to_tighter_box() {
    let mut app = app();
    // Outer 200x200 overflow hidden, no border → clip (0,0)-(200,200).
    let outer = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(200.0)
                .height_px(200.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    // Inner 50x50 overflow hidden, positioned at outer origin → clip
    // (0,0)-(50,50), tighter than outer.
    let inner = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(50.0)
                .height_px(50.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let grandchild = app
        .world_mut()
        .spawn((Node, Style::default().width_px(500.0).height_px(500.0)))
        .id();
    app.world_mut().entity_mut(outer).add_child(inner);
    app.world_mut().entity_mut(inner).add_child(grandchild);
    app.update();

    // Grandchild is bounded by BOTH ancestors ⇒ the tighter (inner) box.
    let anc = app.world().get::<AncestorClip>(grandchild).expect("clipped");
    assert_eq!(anc.max, Vec2::new(50.0, 50.0), "intersection = tighter inner box");
}

#[test]
fn per_axis_overflow_leaves_visible_axis_unbounded() {
    let mut app = app();
    // overflow-x: hidden, overflow-y: visible. x binds to padding box,
    // y stays unbounded (no finite y clamp introduced by this ancestor).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Visible),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    let anc = app.world().get::<AncestorClip>(child).expect("x-clipped");
    assert_eq!(anc.max.x, 100.0, "x axis clamped to parent padding");
    assert_eq!(anc.max.y, f32::INFINITY, "y axis (visible) is unbounded");
    assert_eq!(anc.min.y, f32::NEG_INFINITY, "y axis (visible) is unbounded");
}
```

- [ ] Run RED — `cargo test -p buiy_core --test render_clip_rects nested_overflow_hidden_intersects_to_tighter_box per_axis_overflow_leaves_visible_axis_unbounded`. If both PASS immediately, the Task-5/8 math is already correct — that is the expected outcome and these tests are regression locks (note in commit body that no production change was needed). If either FAILS, root-cause in `clip_contribution` / `walk` (do not patch the test).

- [ ] GREEN — only if a test failed: fix the math. Expected: no change.
- [ ] Run THE GATE → green.
- [ ] Commit: `test(render): pin nested-intersection + per-axis-overflow clip geometry`

---

## Task 10 — `AncestorClip` vs `ClipRect` differ exactly by the own-box step

**Files**
- Test: `crates/buiy_core/tests/render_clip_rects.rs`

Pin the § A.2 invariant: `ClipRect = AncestorClip ∩ own_box`, and they differ exactly when the own box is tighter than the ancestor clip on some edge. Construct a case where the child's own box is **smaller** than the ancestor clip so `ClipRect` (own ∩ ancestor) is bounded by the own box while `AncestorClip` is the (larger) ancestor box. HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
#[test]
fn clip_rect_is_ancestor_clip_intersected_with_own_box() {
    let mut app = app();
    // Parent overflow hidden 200x200 → ancestor clip (0,0)-(200,200).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(200.0)
                .height_px(200.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    // Child 40x40 — SMALLER than the ancestor clip. Its own box binds
    // ClipRect; AncestorClip stays the larger parent box.
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    let clip = *app.world().get::<ClipRect>(child).expect("ClipRect");
    let anc = *app.world().get::<AncestorClip>(child).expect("AncestorClip");

    // AncestorClip = parent padding box (no border) = full 200x200.
    assert_eq!(anc.min, Vec2::ZERO);
    assert_eq!(anc.max, Vec2::new(200.0, 200.0));
    // ClipRect is the own 40x40 box (tighter) ∩ ancestor → the own box.
    assert_eq!(clip.max, Vec2::new(40.0, 40.0), "ClipRect bounded by own box");
    // The two differ ⇒ own-box step is real.
    assert_ne!(clip.max, anc.max, "ClipRect ≠ AncestorClip when own box is tighter");
    // ClipRect == AncestorClip ∩ own_box, verified component-wise.
    assert_eq!(clip.min, anc.min.max(Vec2::ZERO));
    assert_eq!(clip.max, anc.max.min(Vec2::new(40.0, 40.0)));
}
```

- [ ] Run RED — run the test. Expected PASS if Tasks 4–8 are correct (the `walk` computes `clip = ancestor.intersect(own)` and `AncestorClip` from `ancestor`). If it passes outright, it is a regression lock. If it FAILS, the `walk`'s `ClipRect = ancestor ∩ own` vs `AncestorClip = ancestor` split is wrong — root-cause in `walk` / `reconcile`.

- [ ] GREEN — only if failed: ensure `reconcile` receives `clip = ancestor.map(|a| a.intersect(own))` and `ancestor` separately (Task 4 already does this). Expected: no change.
- [ ] Run THE GATE → green.
- [ ] Commit: `test(render): pin ClipRect = AncestorClip ∩ own-box invariant (spec A.2)`

---

## Task 11 — Pruned subtrees (`Display::None` / `ContentVisibility::Hidden`) get no `ClipRect`

**Files**
- Test: `crates/buiy_core/tests/render_clip_rects.rs`
- Modify: `crates/buiy_core/src/render/clip.rs` only if a gap is exposed (the prune guards landed in Task 4).

Pin spec § A.3 pruning: a `Display::None` subtree and a `ContentVisibility::Hidden` subtree are not descended into — their descendants receive **no** `ClipRect`/`AncestorClip` even when an ancestor clips. HEADLESS.

> Note on `Display::None`: layout upstream excludes `Display::None` from the Taffy tree (it may not receive a `ResolvedLayout`), and spec README § 3.1 says render normally never sees `Display::None`. The clip walk still guards defensively (spec § A.3 names `Display` as a pruning input). Test the **descendant** of a `Display::None` node — that descendant must be pruned regardless of whether the `None` node itself laid out.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
use buiy_core::layout::{Containment, ContentVisibility, Display};

#[test]
fn content_visibility_hidden_subtree_is_not_clipped() {
    let mut app = app();
    // Clipping root so descendants WOULD get a clip if walked.
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    // Hidden middle node prunes its subtree.
    let hidden = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0).containment(Containment {
                content_visibility: ContentVisibility::Hidden,
                ..Default::default()
            }),
        ))
        .id();
    let descendant = app
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0)))
        .id();
    app.world_mut().entity_mut(root).add_child(hidden);
    app.world_mut().entity_mut(hidden).add_child(descendant);
    app.update();

    assert!(
        app.world().get::<ClipRect>(descendant).is_none(),
        "descendant of content-visibility:hidden is pruned ⇒ no ClipRect"
    );
    assert!(
        app.world().get::<AncestorClip>(descendant).is_none(),
        "descendant of content-visibility:hidden is pruned ⇒ no AncestorClip"
    );
}
```

- [ ] Run RED — run the test. If the Task-4 prune guard (`content_visibility == Hidden ⇒ return`) is correct, this PASSES (the walk returns at `hidden` before descending, so `descendant` is never visited). If it FAILS, the guard returns *after* recursing or the `Display::None` guard shadows it — root-cause in `walk`.
- [ ] GREEN — only if failed: ensure both prune guards `return` **before** the `children.get(entity)` recursion in `walk`. Expected: no change (Task 4 placed them correctly).
- [ ] Run THE GATE → green.
- [ ] Commit: `test(render): pin Display::None / content-visibility:hidden subtree pruning`

---

## Task 12 — Scroll-only recompute does not touch `ResolvedLayout` (no relayout)

**Files**
- Test: `crates/buiy_core/tests/render_clip_rects.rs`

The decisive pillar-4 proof (spec § A.4 / § A.5 "Scroll-no-relayout"): mutating `ScrollOffset` across frames must (a) leave `ResolvedLayout` byte-stable, and (b) run **zero** new Taffy computes. `write_clip_rects` reads no `ScrollOffset`, so a scroll-only frame issues no clip writes either — but the load-bearing assertion this phase owns is that the scroll frame does not re-run layout. HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
use buiy_core::ResolvedLayout;
use buiy_core::layout::LayoutTaffyComputeCount; // re-exported under `layout`, not the crate root

#[test]
fn scroll_only_frame_does_not_relayout() {
    let mut app = app();
    let sc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Scroll, OverflowMode::Scroll),
            ScrollOffset::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(sc).add_child(child);

    app.update(); // frame 1: full layout
    app.update(); // frame 2: steady state

    let rl_before = app.world().get::<ResolvedLayout>(child).unwrap().position;
    let size_before = app.world().get::<ResolvedLayout>(child).unwrap().size;
    let taffy_before = app.world().resource::<LayoutTaffyComputeCount>().0;

    // Scroll-only mutation.
    {
        let mut off = app.world_mut().get_mut::<ScrollOffset>(sc).unwrap();
        off.y = 120.0;
    }
    app.update(); // frame 3: scroll-only

    let rl_after = app.world().get::<ResolvedLayout>(child).unwrap().position;
    let size_after = app.world().get::<ResolvedLayout>(child).unwrap().size;
    let taffy_after = app.world().resource::<LayoutTaffyComputeCount>().0;

    assert_eq!(rl_before, rl_after, "ResolvedLayout.position byte-stable across scroll");
    assert_eq!(size_before, size_after, "ResolvedLayout.size byte-stable across scroll");
    // LayoutTaffyComputeCount is reset to 0 at the top of each frame, then bumped
    // per root that runs Taffy. A scroll-only frame must run NO Taffy compute.
    assert_eq!(
        taffy_after, 0,
        "scroll-only frame runs zero Taffy compute (count resets per-frame; \
         scroll did not enter sync_styles' trigger set)"
    );
    let _ = taffy_before;
}
```

> `LayoutTaffyComputeCount` is reset to 0 at the start of every frame and bumped once per root that re-runs Taffy (see `crates/buiy_core/src/layout/systems.rs` ~L2589 reset, ~L2768/L3425 bump). On a scroll-only frame `sync_styles`' `Or<Changed<…>>` filter excludes `ScrollOffset` (proven by `tests/layout_scroll_offset_no_invalidate.rs`), so no root re-computes ⇒ end-of-frame count is 0. If a future change makes the reset land such that a steady-state frame ends non-zero, switch the assertion to `taffy_after == taffy_before` after capturing `taffy_before` on an equivalent steady frame. Verify the reset semantics by reading the cited lines before asserting.

- [ ] Run RED — run the test. Expected PASS (the invariant is already enforced by layout; this test just proves the clip phase did not perturb it). If it FAILS with `taffy_after != 0`, the scroll mutation leaked into layout's trigger set — root-cause there, not here (but it would be a layout regression, out of this phase's scope; stop and report).
- [ ] GREEN — no production change expected. This test is the phase's proof obligation.
- [ ] Run THE GATE → green.
- [ ] Commit: `test(render): prove scroll-only frame runs no relayout (pillar-4 ClipRect proof)`

---

## Task 13 — Steady-state frame issues zero structural clip ops (change-gate)

**Files**
- Test: `crates/buiy_core/tests/render_clip_rects.rs`

Pin the change-gate (spec § A.3 body): on a frame where no clip input changed, `write_clip_rects` issues no `insert`/`remove`. Prove it by asserting the `ClipRect`'s change tick does not advance across a steady-state frame (Bevy's `Ref::is_changed`). HEADLESS.

- [ ] RED — append to `crates/buiy_core/tests/render_clip_rects.rs`:

```rust
#[test]
fn steady_state_frame_does_not_rewrite_clip_rect() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);

    app.update(); // frame 1: writes ClipRect
    app.update(); // frame 2: should NOT rewrite

    // After a steady frame, the child's ClipRect must NOT be flagged changed.
    let mut q = app.world_mut().query::<Ref<ClipRect>>();
    let r = q.get(app.world(), child).expect("child has ClipRect");
    assert!(
        !r.is_changed(),
        "steady-state frame must not re-insert ClipRect (change-gate, spec § A.3)"
    );
}
```

> Bevy change-tick semantics: a `Commands::insert` of an equal-valued component still marks it `Changed`. The change-gate in `reconcile` (`if prev != Some(&next)`) skips the insert entirely when unchanged, so `is_changed()` is false on frame 2. This test fails if the gate is removed (e.g. inserting unconditionally), which is exactly the regression to catch.

- [ ] Run RED — to prove the test is load-bearing, temporarily make `reconcile` insert unconditionally (drop the `if prev != Some(&next)` guard) → test FAILS (`is_changed()` true). Restore the guard.
- [ ] GREEN — restore/confirm the change-gate in `reconcile`. Test PASSES.
- [ ] Run THE GATE → green.
- [ ] Commit: `test(render): pin steady-state change-gate (no ClipRect rewrite when unchanged)`

---

## Task 14 — Docs: mark the WriteClipRects pass landed in the spec child + docs index

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs` (module doc cross-ref, if not already added)
- Modify: `docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md` (status note on § A)
- Modify: `docs/README.md` (catalog entry under the render-pipeline plan area, if a plans section exists)

Docs ship with the change (user global guideline: "Update as part of the deliverable"). Keep edits surgical — the spec is the *target state*; add a small "Implemented by" pointer to the plan, do not rewrite the algorithm. HEADLESS (no code-test; THE GATE still runs for fmt/doc).

- [ ] Add an "Implemented by" line at the end of clip-and-transform.md § A.5 (Verification): a sentence pointing to this plan file `docs/plans/2026-06-03-buiy-render-r2-clip-rects.md` and noting `ClipRect` / `AncestorClip` + `write_clip_rects` landed (HEADLESS, scheduled `.after(Animate).before(Picking)`), with the rounded-clip (`ClipRadius`) and `clip-path` seams still reserved.
- [ ] In `docs/README.md`, add/extend the render-pipeline plan catalog entry referencing this plan (follow the existing catalog formatting; if there is no plans subsection for render yet, add one mirroring the layout plans entries). Use the `organizing-buiy-docs` skill conventions if unsure.
- [ ] Run THE GATE → green (fmt + doc must pass; no test change).
- [ ] Commit: `docs(render): mark WriteClipRects (ClipRect/AncestorClip) landed; index the clip-rects plan`

---

## Cross-phase dependencies (assumed)

- **Layout phases (already landed on `main`):** `ResolvedLayout`, `BoxModel.border` (px edges into Taffy), `Overflow` (+ `is_scroll_container()`), `Containment` / `ContainFlags::PAINT` / `ContentVisibility`, `Display::None`, `ScrollOffset`, and the `Style` builder are all present and re-exported from `buiy_core` (verified in `crates/buiy_core/src/layout/`). `LayoutTaffyComputeCount` is a `pub` resource re-exported from `buiy_core::layout` and reset per-frame.
- **Transform bridge (sibling render-pipeline phase, NOT this one):** scroll *content translation* (`base = position - accumulated_ancestor_scroll`) rides `write_buiy_transform` (spec § B.3), not `ClipRect`. This phase deliberately reads **no** `ScrollOffset` and writes only clip boxes. If the bridge phase has not landed, the scroll-content motion is simply not yet applied — `ClipRect` correctness (this phase) is independent of it, and Task 7/12 assert exactly that independence.
- **Picking (`BuiySet::Picking`):** consumes `ClipRect` to reject out-of-clip hits (spec § A.5 gate #10). This phase only guarantees `ClipRect` exists *before* `BuiySet::Picking` (Task 6); the picking-side consumption is owned by `buiy-input-events-design`, out of scope here.
- **Render extract:** reads `Option<&ClipRect>` to scissor (architecture § 3.1). Not wired in this phase; this phase produces the component the extract will read.
- **`WriteEffectGroups` (architecture § 5.2):** shares the same `.after(Animate).before(Picking)` prep window but is a separate sibling phase; this plan adds only `write_clip_rects` and does not order against it (they are independent).
