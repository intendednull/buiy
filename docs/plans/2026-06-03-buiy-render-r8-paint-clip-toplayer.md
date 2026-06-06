# Paint-order walk + clip scissor + top-layer composite Implementation Plan

> **STATUS (2026-06-06): partially landed + partially superseded.** Executed
> against the landed R5/R6 state, several tasks turned out to be already shipped by
> R5: the forward `painters_z` walk (Task 2 `flatten_paint_order`) duplicates R5's
> live `assemble_context_tree`, the skip predicate (Task 6 `skips_paint`) duplicates
> R5's `node_skip_reason`, and the Task 7 "rewire `extract_buiy_draws`" targets a
> path R5/R6 already retired (`extract_buiy_nodes` is the live extract). Those
> duplicate drafts were dropped. **Landed:** the genuinely-new pure consumer helpers
> `scissor_rect` + `clip_for_primitive` (`render/clip.rs`), `partition_top_layer`
> (`render/top_layer.rs`), and the `render_paint_order` integration test (reuses R5's
> walk). **Deferred (Task 8):** the GPU consumer — per-entity scissored draw +
> top-layer composite in `BuiyNode::run` — blocked on the node-draw-model design
> decision (per-entity clip + composite passes on R6's single-buffer draw), shared
> with R9. See
> [2026-06-06-render-node-draw-model-design.md](../specs/2026-06-03-buiy-render-pipeline-design/2026-06-06-render-node-draw-model-design.md)
> and the follow-ups.md entry. Tasks below are the original plan, kept for context.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** R5 (`render/extract.rs` — owns `ExtractedNode` / `ExtractedNodes`, the per-view CPU instance set this phase extends with the paint-order walk + `clip` / `is_top_layer` fields) and R6 (the view-uniform `BuiyNode::run` rework + persistent prepare-phase buffers — where this phase's per-entity scissor + top-layer composite land). Execution order is **R1 → R2 → R3 → R4 → R5 → R6 → R7 → R8 → (R9, R10) → R11**; R8 rebases onto the R5 + R6 target state and must land after both.

**Goal:** Make the view-uniform `BuiyNode::run` (R6) consume `StackingContext.painters_z` forward for paint, scissor each entity by its per-entity `ClipRect` (absent ⇒ full view), pin hit-test order as the exact reverse of paint order, and composite top-layer entries (already at the tail of the root `painters_z`) at the root in layout-decided tier order with no render-side re-sort.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [paint-order-and-top-layer.md](../specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md) (all of §1–§6) and [clip-and-transform.md § A](../specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md#a-the-writecliprects-render-prep-pass) (ClipRect **consumption** only — the `WriteClipRects` producer is a sibling phase).
**Architecture:** Render is a thin read-only consumer (README pillar 1). Layout's sub-pass 6f already wrote the immutable `StackingContext.painters_z` with top-layer members escaped to the root context's tail in tier order; this phase walks that order verbatim (no sort, no tree walk), reads each entity's `ClipRect` to derive a window-relative scissor rect, suppresses paint for `CssVisibility::Hidden` / `OffscreenAuto` subtrees, paints `Outline` against `AncestorClip` (not the own-box `ClipRect`), and composites the top-layer tail at the root with the window viewport as clip. The paint-order walk and the per-entity `clip` / `is_top_layer` carriers attach to **R5's `ExtractedNode`** (the per-view CPU instance record), not the retired Phase-0 `DrawData`; the actual scissored draw + top-layer composite happen in **R6's view-uniform `BuiyNode::run`**, not the Phase-0 node. The forward paint order and its exact reverse (hit-test order) are factored into pure functions so the §2 ordering identity is provable without a GPU.
**Tier/Test reality:** Mixed. The order-walk math, scissor-rect derivation, skip-rule consumption, and the paint/hit-test ordering identity are **HEADLESS** (pure fns + `App::new()+MinimalPlugins+CorePlugin+LayoutPlugin` integration, no wgpu adapter). The actual scissored draw and the top-layer ordering golden are **GPU** (real code, but `#[ignore]`-gated exactly like `render_smoke.rs` — CI has no wgpu adapter).

---

## Orientation for an engineer with zero codebase context

Read these before starting (absolute paths):

- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/extract.rs` — **R5's** `extract_buiy_nodes`, the per-view `ExtractedNode` / `ExtractedNodes` carrier that walks `painters_z` (it already calls `assemble_context_tree` for the atomic nested-SC descent). This phase extends `ExtractedNode` with the `clip` / `is_top_layer` fields and populates them in `extracted_node_for` / `extract_buiy_nodes`; it does **not** touch the Phase-0 `extract_buiy_draws` / `DrawData`. Those are still present in `render/mod.rs` (labeled "retired by R6/R8") but are **dead** — the live node reads `ExtractedNodes` via `prepare_buiy_instances` (`render/prepare.rs`) → `BuiyInstanceBuffers`, never `DrawData`. R8 must NOT revive the `DrawData` path; leave it untouched (a later cleanup removes it).
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/node.rs` — **R6's** view-uniform `BuiyNode::run` (the GPU draw, rebuilt onto `ExtractedNodes` + the persistent prepare-phase buffers). This phase adds the per-entity scissor + the top-layer-at-root composite here.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/components.rs` — **R1's** sole home of the shared author-set + computed render components, including `ClipRect` / `AncestorClip` (`{ pub min: Vec2, pub max: Vec2 }`). This phase **imports** them; it never defines them.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/components.rs` — `StackingContext { painters_z: Vec<Entity> }`, `ResolvedLayout { position, size }`.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/layout/components.rs` — `Stacking { z_index, isolation, top_layer }`.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/layout/types.rs` — `enum TopLayer { None, Modal, Popover, Tooltip, Fullscreen }`.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/layout_stacking.rs` — the **idiom** for driving the real layout plugin so a genuine `StackingContext.painters_z` exists to assert against.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/render_smoke.rs` — the **idiom** for `#[ignore]`-gating GPU tests (the `#[ignore = "needs a wgpu adapter ..."]` attribute).

### Cross-phase dependencies (READ THIS — types this phase consumes but does NOT own)

This phase is the **consumer** of components owned by **R1** (`render/components.rs` — the sole creator/definer of every shared render type) and the per-view carrier owned by **R5** (`render/extract.rs`). R8 lands after R1, R5, and R6, so by the time this phase runs every type below already exists in the tree — **R8 imports them, it never defines them**. The dependency direction is fixed by the spec:

| Type | Canonical owner | Where R8 imports it from | What this phase does with it |
|---|---|---|---|
| `ClipRect { pub min: Vec2, pub max: Vec2 }` | R1 (plain computed struct, no Reflect, not registered — clip-and-transform.md § A.2) | `crate::render::components` | **reads** (`Option<&ClipRect>`) → scissor rect; absent ⇒ no scissor |
| `AncestorClip { pub min: Vec2, pub max: Vec2 }` | R1 (plain computed struct, no Reflect, not registered — clip-and-transform.md § A.2) | `crate::render::components` | **reads** for `Outline` clip (not own-box) |
| `CssVisibility { Visible, Hidden, Collapse }` | R1 (author-set component — component-model.md § 12) | `crate::render::components` | **reads** `Hidden` → subtree paint-skip |
| `OffscreenAuto` (marker) | R1 (layout-emitted marker — component-model.md § 12.2) | `crate::render::components` | **reads** → off-screen `content-visibility:auto` subtree paint-skip |
| `Outline` | R1 (author-set component — component-model.md § 7) | `crate::render::components` | **reads** → outline primitive (clipped by `AncestorClip`) |
| `ExtractedNode` / `ExtractedNodes` | R5 (per-view CPU instance set — architecture.md § 3.1) | `crate::render::extract` | **extends** `ExtractedNode` with `clip` / `is_top_layer`; the walk emits into `ExtractedNodes` |

**Guarded-import rule (no duplicate definitions).** Every shared type R8 touches is owned by R1 (or, for the per-view carrier, R5). When a task below reaches a type, **assume it already exists** (it does — R1/R5 landed first) and `use crate::render::components::{ClipRect, AncestorClip, CssVisibility, OffscreenAuto, Outline};` / `use crate::render::extract::{ExtractedNode, ExtractedNodes};`. **Do NOT define, re-export, register_type, or `pub mod` any of them.** No `render/clip.rs` / `render/skip.rs` definition of `ClipRect` / `AncestorClip` / `CssVisibility` / `OffscreenAuto` — those are R1's. The pure helpers this phase adds (`scissor_rect`, `flatten_paint_order`, `clip_for_primitive`, the skip predicate, the top-layer partition) live in render modules but operate over the **imported** types.

This cross-phase dependency set is recorded in the final structured output.

### THE GATE (every commit must keep this green — no xvfb, no wgpu adapter on this host or CI)

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

Run it before every commit. GPU tests are `#[ignore]`d so `cargo test --workspace` stays green with no adapter.

---

## Task 1 — Import `ClipRect` / `AncestorClip` (R1) + the pure scissor-rect derivation (HEADLESS)

Adds the **pure function** that turns a `ClipRect` (logical-px, y-down, window-relative) into a wgpu scissor rect `(x, y, w, h)` in **physical** pixels, clamped to the view. This is the device-free half of "apply per-entity ClipRect as a scissor rect" — no GPU needed to prove the geometry. `ClipRect` / `AncestorClip` are **imported from R1's `render::components`** (the sole owner of every shared render type); this task does NOT define them.

**Files**
- Create: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/clip.rs` (the `scissor_rect` helper only — NO type definitions)
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` (add `pub mod clip;`)
- Test: inline `#[cfg(test)] mod tests` in `clip.rs`

Steps:

- [ ] **Guarded import (cross-phase):** `ClipRect` and `AncestorClip` already exist — they are owned by **R1** in `render/components.rs` (plain computed structs `{ pub min: Vec2, pub max: Vec2 }`, no `Reflect`, not registered, per clip-and-transform.md § A.2 + component-model.md § 13). Import them with `use crate::render::components::{ClipRect, AncestorClip};`. Do **NOT** define them here, do **NOT** add a `// MOVED:` shape, do **NOT** re-export or `register_type` them. R8 lands after R1, so a `rg -n "struct ClipRect" crates/` confirms the type is already present; if it is somehow absent, STOP — R1 has not landed and the execution order (R1 → … → R8) was violated.
- [ ] Write the failing test first. Create `crates/buiy_core/src/render/clip.rs` with ONLY this content (the impl is stubbed to force a fail):

```rust
//! Render-side **consumption** of the per-entity clip rect (clip-and-transform.md § A).
//! Render reads `ClipRect` (owned by R1, `render::components`); it never
//! re-derives it (the `WriteClipRects` render-prep pass is the producer, owned
//! by R2). This module holds only the pure scissor-rect derivation.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.2,
//!       docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 3.2.

use crate::render::components::{AncestorClip, ClipRect};
use bevy::prelude::*;

/// A wgpu scissor rect in **physical** pixels: `(x, y, width, height)`.
/// `None` ⇒ the clip is degenerate (empty) ⇒ render must skip the entity.
pub type ScissorRect = Option<(u32, u32, u32, u32)>;

/// Derive a physical-pixel wgpu scissor rect from a logical-px `ClipRect`.
///
/// `scale_factor` converts logical → physical px (the same scalar the view
/// uniform folds in, clip-and-transform.md § B.4). The result is clamped to
/// `[0, view_physical]` on both axes. A degenerate clip (`min.x >= max.x` or
/// `min.y >= max.y`, clip-and-transform.md § A.2) returns `None` — the entity
/// is fully clipped away. The clip is already y-down window-relative, the same
/// space wgpu's scissor expects, so NO y-flip happens here.
pub fn scissor_rect(clip: &ClipRect, scale_factor: f32, view_physical: UVec2) -> ScissorRect {
    todo!("implemented in the GREEN step")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_box_maps_to_full_physical_rect() {
        let clip = ClipRect { min: Vec2::ZERO, max: Vec2::new(800.0, 600.0) };
        let s = scissor_rect(&clip, 1.0, UVec2::new(800, 600));
        assert_eq!(s, Some((0, 0, 800, 600)));
    }

    #[test]
    fn scale_factor_scales_to_physical_px() {
        let clip = ClipRect { min: Vec2::new(10.0, 20.0), max: Vec2::new(110.0, 220.0) };
        let s = scissor_rect(&clip, 2.0, UVec2::new(1600, 1200));
        // (10,20)..(110,220) logical → (20,40) origin, 200x400 physical.
        assert_eq!(s, Some((20, 40, 200, 400)));
    }

    #[test]
    fn clip_is_clamped_to_view() {
        // A clip wider than the view clamps to the view bounds (no overflow).
        let clip = ClipRect { min: Vec2::new(-50.0, -50.0), max: Vec2::new(2000.0, 2000.0) };
        let s = scissor_rect(&clip, 1.0, UVec2::new(800, 600));
        assert_eq!(s, Some((0, 0, 800, 600)));
    }

    #[test]
    fn degenerate_clip_returns_none() {
        // min.x >= max.x ⇒ empty rect ⇒ skip (clip-and-transform.md § A.2).
        let clip = ClipRect { min: Vec2::new(100.0, 0.0), max: Vec2::new(100.0, 50.0) };
        assert_eq!(scissor_rect(&clip, 1.0, UVec2::new(800, 600)), None);
        let clip2 = ClipRect { min: Vec2::new(0.0, 80.0), max: Vec2::new(50.0, 40.0) };
        assert_eq!(scissor_rect(&clip2, 1.0, UVec2::new(800, 600)), None);
    }
}
```

- [ ] Add `pub mod clip;` to `crates/buiy_core/src/render/mod.rs` (next to `pub mod instance;`).
- [ ] Run it and watch it FAIL: `cargo test -p buiy_core --lib render::clip` → expect a panic from `todo!()` (compiles, fails at runtime). Confirm the four test names appear and panic.
- [ ] Make it pass — replace the `todo!()` body with the minimal real impl:

```rust
    if clip.min.x >= clip.max.x || clip.min.y >= clip.max.y {
        return None;
    }
    let min = (clip.min * scale_factor).max(Vec2::ZERO);
    let max = (clip.max * scale_factor)
        .min(Vec2::new(view_physical.x as f32, view_physical.y as f32))
        .max(Vec2::ZERO);
    if min.x >= max.x || min.y >= max.y {
        return None; // clamped away entirely (off-screen)
    }
    Some((
        min.x as u32,
        min.y as u32,
        (max.x - min.x) as u32,
        (max.y - min.y) as u32,
    ))
```

- [ ] Run again → PASS. Run the full GATE. Commit: `feat(render): ClipRect consumption shape + pure scissor-rect derivation`.

---

## Task 2 — Forward paint-order walk as a pure function (HEADLESS, §1 + §1.1 atomicity)

Factor "walk `painters_z` forward, descending nested stacking contexts atomically" into a pure function that returns the flat paint order **without any sort or comparison** (the spec's hard constraint: render trusts the layout-decided order). This is the device-free core of the extract walk.

**Files**
- Create: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/paint_order.rs`
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` (add `pub mod paint_order;`)
- Test: inline `#[cfg(test)] mod tests`

Steps:

- [ ] Write the failing test first. Create `crates/buiy_core/src/render/paint_order.rs`:

```rust
//! Pure consumption of `StackingContext.painters_z` (paint-order-and-top-layer.md § 1).
//! Render walks the layout-sorted order FORWARD for paint; it never sorts,
//! compares, or re-derives. Nested stacking contexts are entered atomically
//! (§ 1.1): a nested SC root appears as a single entry in its parent's list,
//! and the walk descends that entity's own `painters_z` as a unit.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 1, § 1.1.

use bevy::prelude::Entity;

/// Resolve the flat front-to-back paint order rooted at `root`, descending
/// nested stacking contexts atomically. `painters_of(e)` returns the
/// `painters_z` slice for entity `e` if it forms a stacking context, else
/// `None` (a non-SC painter contributes only itself). The walk is a verbatim
/// read of layout's order — NO sort, NO `Entity`-id tiebreak, NO
/// `GlobalTransform.z` disambiguation (the § 1.2 hard constraint).
///
/// Terminates because the SC tree is a finite DAG over distinct entities and
/// an entity never appears in its own `painters_z` (sub-pass 6f builds a
/// context's list from its descendants).
pub fn flatten_paint_order<'a, F>(root: Entity, root_painters: &'a [Entity], painters_of: &F) -> Vec<Entity>
where
    F: Fn(Entity) -> Option<&'a [Entity]>,
{
    todo!("implemented in the GREEN step")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Entity;

    fn e(i: u64) -> Entity {
        Entity::from_raw_u32(i as u32).unwrap()
    }

    #[test]
    fn flat_context_is_returned_verbatim() {
        let root = e(0);
        let order = vec![e(1), e(2), e(3)];
        let none = |_q: Entity| None;
        assert_eq!(flatten_paint_order(root, &order, &none), vec![e(1), e(2), e(3)]);
    }

    #[test]
    fn nested_context_entered_atomically_no_interleave() {
        // root.painters_z = [a, NESTED, b]; NESTED.painters_z = [n1, n2].
        // Result must be [a, NESTED, n1, n2, b] — NESTED's children are NOT
        // interleaved with the parent's b; the whole nested context paints
        // before returning to b (§ 1.1).
        let root = e(0);
        let (a, nested, b, n1, n2) = (e(1), e(2), e(3), e(4), e(5));
        let root_order = vec![a, nested, b];
        let nested_order = vec![n1, n2];
        let painters_of = move |q: Entity| -> Option<&[Entity]> {
            if q == nested { Some(nested_order.as_slice()) } else { None }
        };
        assert_eq!(
            flatten_paint_order(root, &root_order, &painters_of),
            vec![a, nested, n1, n2, b]
        );
    }

    #[test]
    fn never_reorders_equal_looking_entries() {
        // The function must preserve list order exactly even when entity ids
        // are descending — proves NO id-order tiebreak sneaks in (§ 1.2).
        let root = e(0);
        let order = vec![e(9), e(3), e(7), e(1)];
        let none = |_q: Entity| None;
        assert_eq!(flatten_paint_order(root, &order, &none), vec![e(9), e(3), e(7), e(1)]);
    }
}
```

- [ ] Add `pub mod paint_order;` to `crates/buiy_core/src/render/mod.rs`.
- [ ] Run + FAIL: `cargo test -p buiy_core --lib render::paint_order` → `todo!()` panic, three named tests visible.
- [ ] Make it pass — replace the body:

```rust
    let mut out = Vec::new();
    let _ = root; // root identity is the caller's; its own box is emitted by the SC walk, not here.
    fn descend<'a, F>(painters: &'a [Entity], painters_of: &F, out: &mut Vec<Entity>)
    where
        F: Fn(Entity) -> Option<&'a [Entity]>,
    {
        for &p in painters {
            out.push(p);
            if let Some(nested) = painters_of(p) {
                descend(nested, painters_of, out);
            }
        }
    }
    descend(root_painters, painters_of, &mut out);
    out
```

- [ ] Run → PASS. Run the full GATE. Commit: `feat(render): pure forward paint-order walk with atomic nested-SC descent`.

---

## Task 3 — Paint/hit-test ordering identity helper (HEADLESS, §2)

Pin the single cross-subsystem invariant this spec fixes: **hit-test order = `painters_z` reversed**. Provide the reverse helper and a test that asserts forward-then-reverse is the identity, so paint and pick can never diverge.

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/paint_order.rs` (add `hit_test_order` + tests)

Steps:

- [ ] Add the failing test + stub to `paint_order.rs` (append inside the module, the fn above `mod tests`, the tests inside it):

```rust
/// Hit-test order = the flattened paint order **reversed** (§ 2): the
/// top-most painter (painted last) is the first hit-test candidate. This is
/// the ordering identity the spec fixes — paint walks forward, picking walks
/// the same list backward, so they cannot drift. Pickability (`pointer-events`)
/// is applied by the picking backend (`buiy-input-events-design`), NOT here;
/// this is order only.
pub fn hit_test_order(paint_order: &[Entity]) -> Vec<Entity> {
    todo!("implemented in the GREEN step")
}
```

```rust
    #[test]
    fn hit_test_order_is_paint_order_reversed() {
        let root = e(0);
        let (a, nested, b, n1, n2) = (e(1), e(2), e(3), e(4), e(5));
        let root_order = vec![a, nested, b];
        let nested_order = vec![n1, n2];
        let painters_of = move |q: Entity| -> Option<&[Entity]> {
            if q == nested { Some(nested_order.as_slice()) } else { None }
        };
        let paint = flatten_paint_order(root, &root_order, &painters_of);
        let hit = hit_test_order(&paint);
        // Exact reverse — the § 2 identity.
        let mut rev = paint.clone();
        rev.reverse();
        assert_eq!(hit, rev);
        // And specifically: last-painted (b) is first hit candidate.
        assert_eq!(hit.first(), Some(&b));
    }

    #[test]
    fn double_reverse_is_identity() {
        let order = vec![e(1), e(2), e(3), e(4)];
        assert_eq!(hit_test_order(&hit_test_order(&order)), order);
    }
```

- [ ] Run + FAIL: `cargo test -p buiy_core --lib render::paint_order::tests::hit_test` → `todo!()` panic.
- [ ] Make it pass:

```rust
    let mut v = paint_order.to_vec();
    v.reverse();
    v
```

- [ ] Run → PASS. Run the full GATE. Commit: `feat(render): hit-test order = reversed paint order (the §2 ordering identity)`.

---

## Task 4 — Top-layer tail recognition without re-sort (HEADLESS, §3 + §3.1)

Layout already appended top-layer members to the **tail** of the root `painters_z` in tier order (Fullscreen < Tooltip < Popover < Modal, then within-tier recency). Render must paint them at the root **in that order, verbatim** — no render-side re-sort. Provide a pure helper that partitions a root `painters_z` into `(in_flow, top_layer_tail)` by reading each entry's `TopLayer` membership, and assert the tail order matches layout's tier order without render re-sorting.

**Files**
- Create: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/top_layer.rs`
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` (add `pub mod top_layer;`)
- Test: inline `#[cfg(test)] mod tests` (pure) **plus** an integration test against the real layout plugin in a new test file (next task pairs them; this task keeps the pure split here).

Steps:

- [ ] Write the failing test first. Create `crates/buiy_core/src/render/top_layer.rs`:

```rust
//! Top-layer composite consumption (paint-order-and-top-layer.md § 3).
//! Top-layer members are ALREADY at the tail of the root context's
//! `painters_z`, appended by layout sub-pass 6f in tier order. Render
//! partitions the root list into (in-flow, top-layer-tail) by reading each
//! entry's `TopLayer` membership and paints the tail at the root — it NEVER
//! re-sorts (§ 3.1: "Render's only ordering input is the `painters_z` tail").
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 3, § 3.1.

use crate::layout::TopLayer;
use bevy::prelude::Entity;

/// Split a root context's `painters_z` into `(in_flow, top_layer_tail)` by
/// reading each entry's top-layer membership via `top_layer_of`. The relative
/// order of BOTH partitions is preserved verbatim from the input — this is a
/// stable partition, NOT a sort. The tail is whatever layout already ordered.
pub fn partition_top_layer<F>(root_painters: &[Entity], top_layer_of: F) -> (Vec<Entity>, Vec<Entity>)
where
    F: Fn(Entity) -> TopLayer,
{
    todo!("implemented in the GREEN step")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::TopLayer;
    use bevy::prelude::Entity;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    #[test]
    fn partitions_tail_preserving_order() {
        // Layout produced: [inflow0, inflow1, FULLSCREEN, POPOVER, MODAL].
        // partition keeps in-flow first (in order) and the tail in the exact
        // order layout emitted (tier order) — render does NOT re-sort it.
        let (i0, i1, fs, pop, modal) = (e(1), e(2), e(3), e(4), e(5));
        let root = vec![i0, i1, fs, pop, modal];
        let tl = move |q: Entity| {
            if q == fs { TopLayer::Fullscreen }
            else if q == pop { TopLayer::Popover }
            else if q == modal { TopLayer::Modal }
            else { TopLayer::None }
        };
        let (in_flow, tail) = partition_top_layer(&root, tl);
        assert_eq!(in_flow, vec![i0, i1]);
        // Verbatim tail — Fullscreen < Popover < Modal as layout ordered it.
        assert_eq!(tail, vec![fs, pop, modal]);
    }

    #[test]
    fn no_top_layer_means_empty_tail() {
        let root = vec![e(1), e(2)];
        let (in_flow, tail) = partition_top_layer(&root, |_| TopLayer::None);
        assert_eq!(in_flow, vec![e(1), e(2)]);
        assert!(tail.is_empty());
    }

    #[test]
    fn render_does_not_reorder_an_out_of_tier_tail() {
        // Hostile fixture: a tail layout (hypothetically) emitted as
        // [MODAL, POPOVER] — render MUST keep that exact order, proving it
        // does not impose its own tier sort (§ 3.1 hard constraint). If render
        // sorted, this would come back [POPOVER, MODAL].
        let (modal, pop) = (e(1), e(2));
        let root = vec![modal, pop];
        let tl = move |q: Entity| {
            if q == modal { TopLayer::Modal } else { TopLayer::Popover }
        };
        let (_in_flow, tail) = partition_top_layer(&root, tl);
        assert_eq!(tail, vec![modal, pop], "render must not re-sort the tail");
    }
}
```

- [ ] Add `pub mod top_layer;` to `crates/buiy_core/src/render/mod.rs`.
- [ ] Run + FAIL: `cargo test -p buiy_core --lib render::top_layer` → `todo!()` panic.
- [ ] Make it pass:

```rust
    let mut in_flow = Vec::new();
    let mut tail = Vec::new();
    for &p in root_painters {
        if top_layer_of(p) == TopLayer::None {
            in_flow.push(p);
        } else {
            tail.push(p);
        }
    }
    (in_flow, tail)
```

- [ ] Run → PASS. Run the full GATE. Commit: `feat(render): partition root painters_z into in-flow + top-layer tail (no re-sort)`.

---

## Task 5 — Integration: real layout 6f → render tail ordering (HEADLESS, §3.1 end-to-end)

Prove the consumption helpers against a **real** `StackingContext.painters_z` produced by the layout plugin, not a synthetic list. This is the headless half of the top-layer ordering golden: it asserts the **tier ORDER only** (Fullscreen < Tooltip < Popover < Modal, plus within-tier recency) the way the spec mandates for v1 (no `::backdrop` — §4 OPEN).

**Files**
- Create: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/render_paint_order.rs`

Steps:

- [ ] Write the failing test first (it fails to compile until the helpers from Tasks 2/4 are `pub`-reachable from the integration crate — they are, via `buiy_core::render::{paint_order, top_layer}`). Create the file mirroring the `layout_stacking.rs` idiom:

```rust
//! Integration: render consumes the REAL `StackingContext.painters_z` that
//! layout sub-pass 6f produces. Asserts the top-layer tier ORDER (v1 ships no
//! `::backdrop`, so this is order only — paint-order-and-top-layer.md § 3.1, § 4).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md.

use bevy::prelude::*;
use buiy_core::components::StackingContext;
use buiy_core::layout::{LayoutPlugin, Stacking, Style, TopLayer};
use buiy_core::render::top_layer::partition_top_layer;
use buiy_core::{CorePlugin, Node};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

fn top_layer_of(world: &World, e: Entity) -> TopLayer {
    world.get::<Stacking>(e).map(|s| s.top_layer).unwrap_or(TopLayer::None)
}

#[test]
fn top_layer_tail_is_tier_ordered_fullscreen_to_modal() {
    let mut app = app();
    // Spawn one of each non-None tier as children of a single root. Layout 6f
    // escapes them to the root context's tail, tier-sorted.
    let modal = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Modal))).id();
    let tooltip = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Tooltip))).id();
    let popover = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Popover))).id();
    let fullscreen = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Fullscreen))).id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[modal, tooltip, popover, fullscreen])
        .id();
    app.update();

    let sc = app.world().get::<StackingContext>(root).expect("root forms a context").clone();
    let world = app.world();
    let (_in_flow, tail) = partition_top_layer(&sc.painters_z, |e| top_layer_of(world, e));

    // Render reads the tail verbatim; layout pinned the tier order. Assert it.
    assert_eq!(
        tail,
        vec![fullscreen, tooltip, popover, modal],
        "top-layer tail paints Fullscreen < Tooltip < Popover < Modal (paint-order § 3.1)"
    );
}

#[test]
fn modal_is_first_hit_candidate_over_popover() {
    // The § 2 / § 3 identity at the integration level: the modal paints last,
    // so it is the FIRST hit-test candidate (why a modal is modal).
    use buiy_core::render::paint_order::{flatten_paint_order, hit_test_order};
    let mut app = app();
    let popover = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Popover))).id();
    let modal = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Modal))).id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[popover, modal])
        .id();
    app.update();

    let sc = app.world().get::<StackingContext>(root).unwrap().clone();
    // Flat order of the root context (no nested SCs among these leaves).
    let paint = flatten_paint_order(root, &sc.painters_z, &|_e| None);
    let hit = hit_test_order(&paint);
    assert_eq!(hit.first(), Some(&modal), "modal is the first hit-test candidate");
    // And popover is below it in paint order (modal painted later).
    let modal_idx = paint.iter().position(|&x| x == modal).unwrap();
    let pop_idx = paint.iter().position(|&x| x == popover).unwrap();
    assert!(modal_idx > pop_idx, "modal paints after (above) popover");
}
```

- [ ] Run + FAIL: `cargo test -p buiy_core --test render_paint_order`. Expect a real assertion FAIL or compile error only if a helper signature drifted — if both helpers landed in Tasks 2/4 the test should compile and the assertions should already pass (they exercise shipped behavior). **If it passes immediately, that is acceptable** — this task is a characterization/integration test pinning the cross-system contract; note it in the commit. If it fails, root-cause against the layout 6f tier order (do NOT "fix" by re-sorting render-side — that violates §3.1).
- [ ] Verify the `add_children` API name compiles (Bevy 0.18). If `add_children` is not the method, use `.add_child(x)` chained per child, matching `layout_stacking.rs`.
- [ ] Run → PASS. Run the full GATE. Commit: `test(render): integration — real 6f painters_z tail is tier-ordered (headless)`.

---

## Task 6 — Skip-rule consumption: `CssVisibility::Hidden` + `OffscreenAuto` (HEADLESS, §5.3 + §5.4)

Render's forward walk drops `CssVisibility::Hidden` and `OffscreenAuto` entities **and their descendants** from primitive emission, keyed on the marker (§5.4) / off-screen flag (§5.3). `Display::None` and `content-visibility:hidden` are layout-owned skips (absent from `painters_z` — no render clause). Provide the pure subtree-skip predicate render extract uses, and prove it drops a subtree while painting an on-screen sibling.

**Files**
- Create: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/skip.rs`
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` (add `pub mod skip;`)
- Test: inline `#[cfg(test)] mod tests`

Steps:

- [ ] **Guarded import (cross-phase):** `CssVisibility` (`{ Visible, Hidden, Collapse }`) and `OffscreenAuto` (marker) already exist — they are author-set / layout-emitted components owned by **R1** in `render/components.rs` (component-model.md § 12 / § 12.2). Import them with `use crate::render::components::{CssVisibility, OffscreenAuto};`. Do **NOT** define them here, do **NOT** add a `// MOVED:` shape, do **NOT** re-export or `register_type` them. R8 lands after R1; if `rg -n "enum CssVisibility|struct OffscreenAuto" crates/` finds nothing, STOP — the execution order was violated.
- [ ] Write the failing test first. Create `crates/buiy_core/src/render/skip.rs`:

```rust
//! Render-owned paint-skip consumption (paint-order-and-top-layer.md § 5.3, § 5.4).
//! Render extract drops `CssVisibility::Hidden` (keep layout box, suppress
//! paint) and `OffscreenAuto` (off-screen `content-visibility:auto`) entities
//! AND their descendants. `Display::None` / `content-visibility:hidden` are
//! layout-owned — those subtrees are absent from `painters_z`, so render needs
//! no clause for them. `CssVisibility` / `OffscreenAuto` are owned by R1
//! (`render::components`); this module holds only the pure skip predicate.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 5.

use crate::render::components::CssVisibility;

/// True iff this entity (and therefore its subtree) must be skipped for paint:
/// `CssVisibility::Hidden` (§ 5.4) or `OffscreenAuto` present (§ 5.3).
/// `Collapse` and `Visible` paint in v1. Inputs are `Option` because a bare
/// `Node` carries neither component.
pub fn skips_paint(css_visibility: Option<&CssVisibility>, offscreen_auto: bool) -> bool {
    todo!("implemented in the GREEN step")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_components_paint() {
        assert!(!skips_paint(None, false));
    }

    #[test]
    fn css_hidden_skips() {
        assert!(skips_paint(Some(&CssVisibility::Hidden), false));
    }

    #[test]
    fn collapse_and_visible_paint_in_v1() {
        assert!(!skips_paint(Some(&CssVisibility::Visible), false));
        assert!(!skips_paint(Some(&CssVisibility::Collapse), false));
    }

    #[test]
    fn offscreen_auto_skips() {
        assert!(skips_paint(None, true));
    }
}
```

- [ ] Add `pub mod skip;` to `crates/buiy_core/src/render/mod.rs`.
- [ ] Run + FAIL: `cargo test -p buiy_core --lib render::skip` → `todo!()` panic.
- [ ] Make it pass:

```rust
    matches!(css_visibility, Some(CssVisibility::Hidden)) || offscreen_auto
```

- [ ] Run → PASS. Run the full GATE. Commit: `feat(render): paint-skip predicate for CssVisibility::Hidden + OffscreenAuto`.

---

## Task 7 — Carry `clip` + `is_top_layer` on `ExtractedNode`; populate them in the landed `extract_buiy_nodes` walk (HEADLESS)

> **RE-PLANNED against the landed R5/R6 architecture.** The original Task 7 grew the Phase-0 `DrawData` and rewrote `extract_buiy_draws`. That path is **dead**: the live render reads R5's per-view `ExtractedNodes` (`render/extract.rs`) via R6's `prepare_buiy_instances` → `BuiyInstanceBuffers` (`render/prepare.rs`) → `BuiyNode::run`. `DrawData` / `extract_buiy_draws` / `ExtractedDraws` still exist in `render/mod.rs` (labeled "retired by R6/R8") but nothing reads them, so wiring clip/top-layer into them would attach scissor logic to a path the node never paints from. Architecture.md § 3.1 is explicit: "The per-view `ExtractedNodes` component (**replacing** the global `ExtractedDraws` resource)". So this task extends `ExtractedNode`, not `DrawData`, and edits the **already-landed** `extract_buiy_nodes` (which already does the atomic `painters_z` walk via `assemble_context_tree` and already applies the `node_skip_reason` skips), not the Phase-0 `extract_buiy_draws`. **Do NOT touch `DrawData` / `extract_buiy_draws`** — a later cleanup deletes the dead path; reviving it here is out of scope and wrong.

Extend the per-entity CPU record `ExtractedNode` (`render/extract.rs`) with `clip: Option<ClipRect>` and `is_top_layer: bool`, and populate them in `extracted_node_for` (the pure record builder) and `extract_buiy_nodes` (the system that already walks `painters_z`). The walk, the atomic nested-SC descent (`assemble_context_tree`), and the `CssVisibility::Hidden` / `OffscreenAuto` skips already landed in R5 — this task only adds the two carriers, populated from the extract fan (`Option<&ClipRect>`, `Option<&Stacking>`). The `painters_z` order and skips are NOT re-implemented here.

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/extract.rs` (extend `ExtractedNode`; extend `extracted_node_for`; add `Option<&ClipRect>` + `Option<&Stacking>` to the `extract_buiy_nodes` query fan + its `Changed<…>` Or-set; populate the two fields)
- Test: inline `#[cfg(test)] mod tests` in `extract.rs` (pure: `extracted_node_for` carries the clip + top-layer flag; the ordering/skip behavior is already covered by R5's `assemble_*` tests, do NOT duplicate it)

Steps:

- [ ] **Guarded import (cross-phase).** `ClipRect` is R1's (`crate::render::components`), already imported by `clip.rs`; `Stacking` / `TopLayer` are layout's (`crate::layout`). Import what `extract.rs` is missing (`use crate::render::components::ClipRect;`, `use crate::layout::{Stacking, TopLayer};` — `Stacking` may already be imported for the `Changed<Stacking>` trigger). Define **nothing** new.
- [ ] Extend `ExtractedNode` (currently `{ entity, position, size, color }`) with two fields:

```rust
    /// The entity's computed clip rect (R2's `WriteClipRects` output), in
    /// logical px. `None` ⇒ no ancestor clips this node ⇒ no scissor (full
    /// view). Read by `BuiyNode::run` (Task 8) to set the per-batch scissor.
    pub clip: Option<ClipRect>,
    /// True iff this node is a top-layer member (already at the tail of the
    /// root context's `painters_z`, appended by layout sub-pass 6f in tier
    /// order). The node composites these at the root AFTER the in-flow layers,
    /// scissored to the full viewport — never re-sorting (paint-order § 3).
    pub is_top_layer: bool,
```

  `ExtractedNode` derives `Clone, Copy, Debug, PartialEq`; `ClipRect` is `Copy + PartialEq`, so the derives still hold. Every constructor of `ExtractedNode` (only `extracted_node_for`) must set the new fields — there is no `Default`/`..` spread, so the compiler enforces completeness.
- [ ] Write the failing test first — add to `extract.rs`'s `#[cfg(test)] mod tests` (the pure record builder carries the new fields):

```rust
    #[test]
    fn extracted_node_carries_clip_and_top_layer_flag() {
        let theme = Theme::default();
        let gt = GlobalTransform::from_translation(Vec3::new(5.0, 6.0, 0.0));
        let layout = ResolvedLayout { position: Vec2::ZERO, size: Vec2::splat(10.0) };
        let clip = ClipRect { min: Vec2::ZERO, max: Vec2::splat(100.0) };
        let e = Entity::from_raw_u32(1).unwrap();

        // Clipped, top-layer (Modal).
        let node = extracted_node_for(e, &gt, &layout, None, Some(&clip), TopLayer::Modal, &theme);
        assert_eq!(node.clip, Some(clip));
        assert!(node.is_top_layer);

        // Unclipped, in-flow.
        let node2 = extracted_node_for(e, &gt, &layout, None, None, TopLayer::None, &theme);
        assert_eq!(node2.clip, None);
        assert!(!node2.is_top_layer);
    }
```

- [ ] Make it pass — extend `extracted_node_for`'s signature with `clip: Option<&ClipRect>` and `top_layer: TopLayer`, and set the fields (`clip: clip.copied()`, `is_top_layer: top_layer != TopLayer::None`). Run `cargo test -p buiy_core --lib render::extract::tests::extracted_node_carries` → PASS.
- [ ] Thread the inputs through the **landed** `extract_buiy_nodes` system (do NOT add a new walk):
  - Add `Option<&ClipRect>` and `Option<&Stacking>` to the query tuple's author/handoff fan (architecture § 1.2 lists both as `Option<&_>`); `Stacking` carries `top_layer` (a bare `Node` lacks it ⇒ `TopLayer::None`).
  - Add `Changed<ClipRect>` to the `Or<(…)>` trigger set in lockstep (architecture § 3.1 lists it; `Changed<Stacking>` is already present).
  - In the per-entity loop, pass `clip` and `stacking.map(|s| s.top_layer).unwrap_or(TopLayer::None)` into `extracted_node_for` when building each `by_entity` record.
- [ ] Confirm `pack_extracted` (`render/instance.rs`) and `pack_view` (`render/buckets.rs`) still compile unchanged — they read only `position`/`size`/`color` off `ExtractedNode`, so the two new fields are inert until Task 8 consumes them. `prepare_buiy_instances` is untouched this task. Run `cargo test -p buiy_core --test render_instance` (if present) and `--lib render::buckets render::instance` → PASS.
- [ ] Run the full GATE. Commit: `feat(render): ExtractedNode carries clip + top-layer flag, populated in the painters_z walk`.

---

## Task 8 — Per-batch scissor side-table + top-layer tail batch → `BuiyNode::run` set_scissor + top-layer-last (HEADLESS prepare seam + GPU #[ignore] e2e)

> **RE-PLANNED against the landed R6 prepare/node architecture.** The original Task 8 said to "walk the extracted draws" and "partition the extracted draws into in-flow vs `is_top_layer`" inside `BuiyNode::run`. That does not exist in the landed code: `prepare_buiy_instances` (`render/prepare.rs`) packs `ExtractedNodes` into a **flat `[f32; 9]` blob** (`BuiyInstanceBuffers.quad`) via `pack_view`/`packed_to_raw` (`render/buckets.rs`), and `BuiyNode::run` (`render/node.rs`) issues exactly **one** `pass.draw(0..4, 0..quad_count)`. The `[f32; 9]` instance (pos2+size2+color4+radius1, `PACKED_INSTANCE_STRIDE_BYTES = 36`) has **no slot** for a clip rect or a top-layer flag, and the node has no per-draw loop to set a scissor in. So the clip/top-layer carriers added to `ExtractedNode` in Task 7 must be threaded through the **bucket/prepare layer as side-data**, NOT into the instance blob, and `BuiyNode::run` must grow a per-batch draw loop. This is the architecture-§2.2 shape ("one instanced draw per `(primitive, layer)` batch") that R6 deferred to "layer 0 only" — R8 turns the deferred per-batch loop on.

Thread per-entity clip + top-layer from `ExtractedNode` into `InstanceBuckets` as a **per-batch scissor side-table** plus a **top-layer tail batch**, extend `BuiyInstanceBuffers` to carry the side-table, and rewrite `BuiyNode::run` to iterate batches: `set_scissor_rect` per batch, draw the slice, then draw the top-layer tail last scissored to the full viewport. The device-free half (bucketing by `(scissor, is_top_layer)`, the flat-buffer offset/len side-table, the top-layer-last order) is **HEADLESS** and unit-tested; the actual GPU pass is `#[ignore]`-gated like `render_smoke.rs`.

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/buckets.rs` (`pack_view` reads `ExtractedNode.clip` / `.is_top_layer`; emit a per-batch scissor + an `is_top_layer` flag on the batch key, so the natural `BTreeMap` order is in-flow batches then the top-layer tail)
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/prepare.rs` (`pack_extracted_nodes` returns the flat blob **plus** a `Vec<BatchDraw>` side-table of `{ offset, len, scissor: Option<ClipRect>, is_top_layer }`; `BuiyInstanceBuffers` carries that `Vec<BatchDraw>` alongside `quad`/`quad_count`)
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/node.rs` (replace the single `pass.draw(0..4, 0..quad_count)` with the per-batch `set_scissor_rect` + draw loop, top-layer tail scissored to the full viewport)
- Test: inline `#[cfg(test)] mod tests` in `buckets.rs` + `prepare.rs` (HEADLESS: side-table offsets/lens, scissor per batch, top-layer batches sort last) and `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/render_smoke.rs` (`#[ignore]` GPU wiring smoke)

Steps:

- [ ] **Bucket key carries scissor + top-layer (HEADLESS, the real R8 work).** `PrimitiveBatchKey` (`buckets.rs`) is currently `{ primitive, layer }`. A scissor rect cannot be a `BTreeMap` key directly (`ClipRect` is `f32`-based, no `Ord`), so do **not** key on the rect. Instead: in `pack_view`, group nodes into batches by **consecutive runs of equal `(scissor, is_top_layer)`** along the already-paint-ordered `nodes` slice (the order is `painters_z`; never re-sort — pillar 1), and record each run as a `BatchDraw { offset, len, scissor: Option<ClipRect>, is_top_layer }` against the flat instance vec. Top-layer nodes are already at the `painters_z` tail (layout 6f), so the `is_top_layer` runs naturally fall last — assert this rather than re-sorting. Keep transparent-skip (`color == Color::NONE`) exactly as `pack_view` does today.
- [ ] Write the failing test first (HEADLESS) in `buckets.rs` / a new `pack` helper test:

```rust
    #[test]
    fn pack_view_emits_per_clip_batches_then_top_layer_tail() {
        use crate::render::components::ClipRect;
        let clip = ClipRect { min: Vec2::ZERO, max: Vec2::splat(50.0) };
        // paint order: [inflow-unclipped, inflow-clipped, top-layer-clipped].
        let nodes = vec![
            node(Vec2::ZERO, Color::WHITE, None, false),
            node(Vec2::splat(10.0), Color::WHITE, Some(clip), false),
            node(Vec2::splat(20.0), Color::WHITE, Some(clip), true),
        ];
        let (instances, batches) = pack_view_batched(&nodes);
        assert_eq!(instances.len(), 3);
        // three runs: unclipped in-flow, clipped in-flow, clipped top-layer.
        assert_eq!(batches.len(), 3);
        assert_eq!((batches[0].scissor, batches[0].is_top_layer), (None, false));
        assert_eq!((batches[1].scissor, batches[1].is_top_layer), (Some(clip), false));
        // top-layer batch is LAST (it was at the painters_z tail; not re-sorted).
        assert!(batches[2].is_top_layer);
        // offsets/lens partition the flat instance vec contiguously.
        assert_eq!(batches[0].offset, 0);
        assert_eq!(batches.iter().map(|b| b.len).sum::<usize>(), instances.len());
    }
```

  (`node(..)` / `pack_view_batched(..)` are the test fixture + the batched packer this task introduces; `pack_view`'s current single-batch behavior is replaced by it. Keep the existing `pack_view` tests passing or migrate them.)
- [ ] Make it pass — implement `pack_view_batched` (run-length over `(scissor, is_top_layer)`) returning `(Vec<[f32;9]>, Vec<BatchDraw>)`; have `prepare::pack_extracted_nodes` return the `Vec<BatchDraw>` alongside the flat blob and the uniform; store it on `BuiyInstanceBuffers`. Run `cargo test -p buiy_core --lib render::buckets render::prepare` → PASS.
- [ ] **`BuiyNode::run` per-batch loop (GPU behavior).** Replace the single `pass.draw(0..4, 0..buffers.quad_count)` with: for each `BatchDraw` in `buffers.batches` in order — compute the scissor (`b.is_top_layer` ⇒ full viewport, escaping ancestor clip per §3.2; else `b.scissor.map(|c| scissor_rect(&c, scale_factor, view_physical))`); on a degenerate `None`-from-`scissor_rect` (clip `min>=max`), **skip the batch**; on absent clip, `set_scissor_rect` to the full view; then `pass.draw(0..4, b.offset as u32 .. (b.offset + b.len) as u32)`. The top-layer batches already sit last in `buffers.batches`, so iterating in order paints them after in-flow (architecture § 2.3: one ordered composite pass) — **no render-side re-sort**. Obtain `scale_factor` + physical view size from the view (`ExtractedNodes.scale_factor` × `logical_size`, carried through prepare, or the `ViewTarget`'s physical size). Keep the existing early-returns (no buffers / `quad_count == 0` / no view binding).
- [ ] Add a code comment at the top-layer batches: `// v1 ::backdrop = OPEN (paint-order § 4): no dimming backdrop. Top-layer batches already sit last in painters_z (layout 6f); we draw them in order, scissored to the full viewport — never re-sorted (§3.1).`
- [ ] Write the GPU `#[ignore]` wiring smoke test (mirrors the `#[ignore]` idiom already in `render_smoke.rs`). Append:

```rust
// GPU e2e: per-batch scissor + top-layer-last composite. Needs a wgpu adapter
// (real GPU or lavapipe); CI has none, so #[ignore] exactly like the other
// render-graph tests. The device-free consumption logic (paint order, scissor
// derivation, per-batch side-table, top-layer-last) is proven headless in the
// `render::{clip,paint_order,top_layer,skip,buckets,prepare}` unit tests +
// `render_paint_order`.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the e2e golden harness"]
fn buiy_node_scissors_per_batch_and_composites_top_layer() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::render::RenderPlugin::default());
    app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
    app.add_plugins(buiy_core::render::BuiyRenderPlugin);
    // A real golden-image assertion lives in the visual-regression harness
    // (gate #2). Here we only assert the node + pipeline registered so the
    // ignored run smoke-tests the wiring on a GPU box.
    let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
    assert!(
        render_app
            .world()
            .get_resource::<buiy_core::render::pipeline::BuiyPipeline>()
            .is_some(),
        "BuiyPipeline registered for the scissor/top-layer node"
    );
}
```

- [ ] Run + confirm the GPU test is collected and **ignored** (no adapter): `cargo test -p buiy_core --test render_smoke` → shows `ignored`. The GATE stays green.
- [ ] Run + FAIL→handle: the GPU test stays `ignored` (green on CI). Locally on a GPU box, `cargo test -p buiy_core --test render_smoke -- --ignored` should pass once wired; if you cannot run on a GPU box, the headless `buckets`/`prepare` side-table tests + a careful read are the verification (note this in the commit body honestly — do NOT claim the GPU path passed if you did not run it).
- [ ] Run the full GATE (headless) → green. Commit: `feat(render): per-batch scissor side-table + top-layer-last in BuiyNode::run (GPU #[ignore])`.

---

## Task 9 — `Outline` clips against `AncestorClip`, never own-box (HEADLESS predicate, §3.2 / component-model § 7)

The spec pins one subtle clip rule render owns: an `Outline` is painted **outside** the border box and must be clipped by **ancestor** clips only (`AncestorClip`), never the entity's own `ClipRect` (which would erase a ring drawn outside the box). Provide the pure selector that, given an entity's draw kind, returns which clip the scissor uses, and prove an outline picks `AncestorClip` while a fill picks `ClipRect`. (The `Outline` **primitive** is owned by the component-model phase; this task fixes only the consumption *rule* this phase is responsible for.)

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/clip.rs` (add `clip_for_outline` selector + tests)

Steps:

- [ ] Write the failing test first. Append to `clip.rs`:

```rust
/// Which clip a primitive scissors against. A fill / background / border uses
/// the entity's own-box-intersected `ClipRect`. An `Outline` (painted outside
/// the border box) uses `AncestorClip` — the ancestor intersection WITHOUT the
/// own-box step — so a focus ring outside the box is cropped by ancestors but
/// not erased by the entity's own box (clip-and-transform.md § A.2; § 3.2).
///
/// Returns the AABB to scissor against, or `None` ⇒ no scissor (unclipped).
pub fn clip_for_primitive(
    is_outline: bool,
    own_clip: Option<&ClipRect>,
    ancestor_clip: Option<&AncestorClip>,
) -> Option<ClipRect> {
    todo!("implemented in the GREEN step")
}

#[cfg(test)]
mod outline_clip_tests {
    use super::*;

    #[test]
    fn fill_uses_own_clip() {
        let own = ClipRect { min: Vec2::ZERO, max: Vec2::splat(50.0) };
        let anc = AncestorClip { min: Vec2::ZERO, max: Vec2::splat(200.0) };
        assert_eq!(clip_for_primitive(false, Some(&own), Some(&anc)), Some(own));
    }

    #[test]
    fn outline_uses_ancestor_clip_not_own() {
        let own = ClipRect { min: Vec2::ZERO, max: Vec2::splat(50.0) };
        let anc = AncestorClip { min: Vec2::ZERO, max: Vec2::splat(200.0) };
        // Outline must NOT be clipped to the 50x50 own box — it uses the 200x200
        // ancestor clip, so the ring outside the border box survives.
        let got = clip_for_primitive(true, Some(&own), Some(&anc));
        assert_eq!(got, Some(ClipRect { min: anc.min, max: anc.max }));
    }

    #[test]
    fn absent_clips_are_unclipped() {
        assert_eq!(clip_for_primitive(false, None, None), None);
        assert_eq!(clip_for_primitive(true, None, None), None);
    }
}
```

- [ ] Run + FAIL: `cargo test -p buiy_core --lib render::clip::outline_clip_tests` → `todo!()` panic.
- [ ] Make it pass:

```rust
    if is_outline {
        ancestor_clip.map(|a| ClipRect { min: a.min, max: a.max })
    } else {
        own_clip.copied()
    }
```

- [ ] Run → PASS. Run the full GATE. Commit: `feat(render): Outline scissors against AncestorClip, fills against own ClipRect (§3.2)`.

---

## Task 10 — Docs: mark the consumed seams + update the spec/plan catalog (HOUSEKEEPING)

Reflect the landed consumer-side behavior in the spec children and the docs index, per the project's "docs ship with the change" rule. Do **not** silently contradict the OPEN `::backdrop` question — record what v1 ships (tier order, no dimming) and leave §4 OPEN.

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md` (annotate §6 verification rows that now have landed headless tests; do NOT remove the OPEN §4 marker)
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/docs/README.md` (add this plan to the render-pipeline plans list, if a plans catalog section exists)

Steps:

- [ ] In `paint-order-and-top-layer.md` § 6, append a one-line "**Landed (headless):**" note under the *Paint-order identity*, *Nested-context atomicity*, and *Skip rules* bullets pointing at `crates/buiy_core/tests/render_paint_order.rs` and the `render::{paint_order,top_layer,skip,clip}` unit tests. Leave the *Top-layer compositing* (gate #2 GPU golden) and the OPEN §4 backdrop text exactly as-is.
- [ ] Confirm the OPEN markers (`§ 4 — OPEN`, README § 5 #3) are untouched — grep `OPEN` in the file and verify the count is unchanged from before this task.
- [ ] If `docs/README.md` has a render-pipeline plans subsection, add a link to `docs/plans/2026-06-03-buiy-render-r8-paint-clip-toplayer.md`. If not, add a one-line entry under the closest plans grouping; otherwise skip (do not invent structure — follow `organizing-buiy-docs`).
- [ ] Run the full GATE (docs-only change still must pass `cargo doc` + everything else). Commit: `docs(render): mark landed paint-order/clip/skip consumption seams [headless]`.

---

## Done criteria

- [ ] `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace` is green.
- [ ] HEADLESS-gating tests (run on CI, no adapter): `render::clip` (scissor-rect derivation + outline-clip selector), `render::paint_order` (forward walk + atomicity + reverse identity), `render::top_layer` (tail partition, no re-sort), `render::skip` (paint-skip predicate), `render::extract` (`extracted_node_for` carries clip + top-layer flag; the ordering/skip walk is already covered by R5's `assemble_*` tests), `render::buckets` / `render::prepare` (per-batch scissor side-table + top-layer tail sorts last, flat-blob offsets/lens), and integration `tests/render_paint_order.rs` (real 6f tail tier order + modal-first-hit).
- [ ] GPU `#[ignore]` tests (real code, no CI adapter): `tests/render_smoke.rs::buiy_node_scissors_per_batch_and_composites_top_layer` — the actual per-batch scissored draw + top-layer-last composite; the tier-order golden lives in the gate-#2 visual-regression harness, which this phase feeds but does not own.
- [ ] v1 `::backdrop` = no dimming backdrop (§4 OPEN); the top-layer composite paints over in-flow in layout's tier order, and the modal-over-popover golden asserts **tier ORDER only**.
- [ ] No render-side re-sort anywhere: paint order, hit-test order, and the top-layer tail are all verbatim reads of layout's `painters_z` (the §1.2 / §3.1 hard constraint), enforced by the `render_does_not_reorder_*` tests.
