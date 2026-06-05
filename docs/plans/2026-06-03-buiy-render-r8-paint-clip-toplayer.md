# Paint-order walk + clip scissor + top-layer composite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `BuiyNode` consume `StackingContext.painters_z` forward for paint, scissor each entity by its per-entity `ClipRect` (absent ⇒ full view), pin hit-test order as the exact reverse of paint order, and composite top-layer entries (already at the tail of the root `painters_z`) at the root in layout-decided tier order with no render-side re-sort.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [paint-order-and-top-layer.md](../specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md) (all of §1–§6) and [clip-and-transform.md § A](../specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md#a-the-writecliprects-render-prep-pass) (ClipRect **consumption** only — the `WriteClipRects` producer is a sibling phase).
**Architecture:** Render is a thin read-only consumer (README pillar 1). Layout's sub-pass 6f already wrote the immutable `StackingContext.painters_z` with top-layer members escaped to the root context's tail in tier order; this phase walks that order verbatim (no sort, no tree walk), reads each entity's `ClipRect` to derive a window-relative scissor rect, suppresses paint for `CssVisibility::Hidden` / `OffscreenAuto` subtrees, paints `Outline` against `AncestorClip` (not the own-box `ClipRect`), and composites the top-layer tail at the root with the window viewport as clip. The forward paint order and its exact reverse (hit-test order) are factored into pure functions so the §2 ordering identity is provable without a GPU.
**Tier/Test reality:** Mixed. The order-walk math, scissor-rect derivation, skip-rule consumption, and the paint/hit-test ordering identity are **HEADLESS** (pure fns + `App::new()+MinimalPlugins+CorePlugin+LayoutPlugin` integration, no wgpu adapter). The actual scissored draw and the top-layer ordering golden are **GPU** (real code, but `#[ignore]`-gated exactly like `render_smoke.rs` — CI has no wgpu adapter).

---

## Orientation for an engineer with zero codebase context

Read these before starting (absolute paths):

- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` — the Phase-0 extract (`extract_buiy_draws`) that today reads `(Visual, ResolvedLayout)` **unordered** and pushes `DrawData`. This phase replaces the unordered iteration with a `painters_z` walk.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/node.rs` — `BuiyNode::run` (the GPU draw). This phase adds the per-entity scissor.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/instance.rs` — `to_instance` (logical-px → clip-space). Reused as-is.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/components.rs` — `StackingContext { painters_z: Vec<Entity> }`, `ResolvedLayout { position, size }`, `Visual`.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/layout/components.rs` — `Stacking { z_index, isolation, top_layer }`.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/layout/types.rs` — `enum TopLayer { None, Modal, Popover, Tooltip, Fullscreen }`.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/layout_stacking.rs` — the **idiom** for driving the real layout plugin so a genuine `StackingContext.painters_z` exists to assert against.
- `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/render_smoke.rs` — the **idiom** for `#[ignore]`-gating GPU tests (the `#[ignore = "needs a wgpu adapter ..."]` attribute).

### Cross-phase dependencies (READ THIS — types this phase consumes but does NOT own)

This phase is the **consumer** of components owned by sibling render-pipeline phases. At plan-authoring time these do **not** exist in the tree (grep-confirmed: no `ClipRect`, `AncestorClip`, `CssVisibility`, `OffscreenAuto`, `Outline` anywhere under `crates/`). The dependency direction is fixed by the spec:

| Type | Canonical owner (sibling phase) | What this phase does with it |
|---|---|---|
| `ClipRect { min: Vec2, max: Vec2 }` | clip-and-transform.md § A.2 / component-model.md § 12 | **reads** (`Option<&ClipRect>`) → scissor rect; absent ⇒ no scissor |
| `AncestorClip { min: Vec2, max: Vec2 }` | clip-and-transform.md § A.2 | **reads** for `Outline` clip (not own-box) |
| `CssVisibility { Visible, Hidden, Collapse }` | component-model.md § 12 | **reads** `Hidden` → subtree paint-skip |
| `OffscreenAuto` (marker) | component-model.md § 12.2 (layout-emitted) | **reads** → off-screen `content-visibility:auto` subtree paint-skip |
| `Outline` | component-model.md § 7 | **reads** → outline primitive (clipped by `AncestorClip`) |

**How this plan stays buildable + gating without those phases landing first:** Task 1 defines the **minimal `ClipRect` and `AncestorClip` shapes** (the canonical definitions per clip-and-transform.md § A.2 — render does not invent them, it transcribes the spec's struct) in `render/clip.rs`, **only if a grep shows no `ClipRect` already exists in the crate** (a `// MOVED:` comment makes the hand-off explicit so the sibling phase deletes this copy and re-exports its own). `CssVisibility` / `OffscreenAuto` / `Outline` are consumed via **`Option<&T>` queries guarded behind their existence**; where a type is not yet in the tree, the task that needs it defines the **minimal render-read shape** in `render/skip.rs` / `render/clip.rs` with the same `// MOVED:` hand-off marker. Every such definition is `#[derive(Component, ...)]` matching the spec field-for-field, so when the owning phase lands, the consumer code is unchanged and the duplicate definition is deleted in favor of a re-export. **If, when you reach a task, the owning phase has already landed its type, skip the local definition and import the real one** — the test assertions are identical either way.

This is the single assumed cross-phase dependency set. It is recorded in the final structured output.

### THE GATE (every commit must keep this green — no xvfb, no wgpu adapter on this host or CI)

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

Run it before every commit. GPU tests are `#[ignore]`d so `cargo test --workspace` stays green with no adapter.

---

## Task 1 — `ClipRect` / `AncestorClip` consumption shapes + the pure scissor-rect derivation (HEADLESS)

Adds the per-entity clip rect type render reads, and the **pure function** that turns a `ClipRect` (logical-px, y-down, window-relative) into a wgpu scissor rect `(x, y, w, h)` in **physical** pixels, clamped to the view. This is the device-free half of "apply per-entity ClipRect as a scissor rect" — no GPU needed to prove the geometry.

**Files**
- Create: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/clip.rs`
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` (add `pub mod clip;`)
- Test: inline `#[cfg(test)] mod tests` in `clip.rs`

Steps:

- [ ] **Pre-check (cross-phase):** run `rg -n "struct ClipRect" crates/` from the worktree root. If it already exists (sibling phase landed), import it instead of defining it and **skip the struct definition below** (keep only `scissor_rect` + tests, importing `ClipRect` from its real home).
- [ ] Write the failing test first. Create `crates/buiy_core/src/render/clip.rs` with ONLY this content (the impl is stubbed to force a fail):

```rust
//! Render-side **consumption** of the per-entity clip rect (clip-and-transform.md § A).
//! Render reads `ClipRect`; it never re-derives it (the `WriteClipRects`
//! render-prep pass is the producer, owned by the clip-and-transform phase).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.2,
//!       docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 3.2.

use bevy::prelude::*;

// MOVED: canonical owner is clip-and-transform.md § A.2 (`WriteClipRects` pass).
// Defined here as the render-read shape until that sibling phase lands its
// producer; when it does, delete this and re-export the owner's `ClipRect`.
/// Per-entity computed clip AABB, logical px, y-down, window-relative.
/// Absent `ClipRect` ⇔ no ancestor clips this entity ⇒ render applies no scissor.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ClipRect {
    /// Top-left corner, logical px, window-relative (y-down).
    pub min: Vec2,
    /// Bottom-right corner, logical px, window-relative (y-down).
    pub max: Vec2,
}

// MOVED: canonical owner is clip-and-transform.md § A.2. Ancestor-only clip
// (no own-box step) — `Outline` reads this so a focus ring outside the border
// box is clipped by ancestors but not erased by the entity's own box.
/// Per-entity ancestor-only clip AABB, logical px, y-down, window-relative.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AncestorClip {
    /// Top-left corner, logical px, window-relative (y-down).
    pub min: Vec2,
    /// Bottom-right corner, logical px, window-relative (y-down).
    pub max: Vec2,
}

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

- [ ] **Pre-check (cross-phase):** `rg -n "enum CssVisibility|struct OffscreenAuto" crates/`. If `CssVisibility` / `OffscreenAuto` already exist (component-model phase landed), import them and skip the local definitions below.
- [ ] Write the failing test first. Create `crates/buiy_core/src/render/skip.rs`:

```rust
//! Render-owned paint-skip consumption (paint-order-and-top-layer.md § 5.3, § 5.4).
//! Render extract drops `CssVisibility::Hidden` (keep layout box, suppress
//! paint) and `OffscreenAuto` (off-screen `content-visibility:auto`) entities
//! AND their descendants. `Display::None` / `content-visibility:hidden` are
//! layout-owned — those subtrees are absent from `painters_z`, so render needs
//! no clause for them.
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 5.

use bevy::prelude::*;

// MOVED: canonical owner is component-model.md § 12. Defined here as the
// render-read shape until that phase lands; delete + re-export when it does.
/// CSS `visibility`. `Hidden` paints nothing for the entity + subtree but
/// keeps the layout box. `Collapse` is a deferred marker (v1 paints it as
/// `Visible`). Deliberately NOT `bevy::prelude::Visibility` (name collision).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CssVisibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

// MOVED: canonical owner is component-model.md § 12.2 (layout-emitted).
/// Marker: this entity's `content-visibility: auto` subtree is currently
/// off-screen (layout's hysteresis-expanded viewport test). Render skips
/// painting it + its descendants (§ 5.3).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct OffscreenAuto;

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

## Task 7 — Rewire `extract_buiy_draws` to walk `painters_z` in order + carry clip + apply skips (HEADLESS schedule, GPU draw deferred)

Replace the Phase-0 **unordered** `(Visual, ResolvedLayout)` iteration with the ordered consumption: extract from the root `StackingContext.painters_z` using the Task-2 flatten walk, attach each draw's `Option<ClipRect>` and a `top_layer` flag, and drop entities the Task-6 predicate skips (with subtree pruning). `DrawData` grows a `clip: Option<ClipRect>` field and an `is_top_layer: bool`. This system runs in `ExtractSchedule` (render world), so its **construction is headless** but it only does real work with extracted components — the per-frame schedule-membership is the headless gate; the actual pixels are Task 8 (GPU).

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/mod.rs` (extend `DrawData`, rewrite `extract_buiy_draws`)
- Test: inline `#[cfg(test)] mod tests` in `mod.rs` (pure: a free helper `build_draws` that does the walk given closures — testable with no render world)

Steps:

- [ ] **Refactor for testability first.** The current `extract_buiy_draws` does the work inline inside an `Extract<Query>` system, which is not headless-testable (needs the render world). Extract the **pure core** into a free function `build_draws` that takes the root `painters_z`, plus closures `resolved_of`, `visual_color_of`, `clip_of`, `top_layer_of`, `skips_of`, `painters_of`, and returns `Vec<DrawData>` in paint order. The `Extract` system becomes a thin adapter that builds those closures from queries and calls `build_draws`. This makes the order/skip/clip consumption provable headless.
- [ ] Write the failing test first — add to a new `#[cfg(test)] mod extract_tests` in `mod.rs`:

```rust
#[cfg(test)]
mod extract_tests {
    use super::*;
    use crate::render::clip::ClipRect;
    use crate::layout::TopLayer;
    use bevy::prelude::*;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    #[test]
    fn build_draws_emits_in_painters_z_order_and_skips_hidden() {
        let root = e(0);
        let (a, hidden, c) = (e(1), e(2), e(3));
        let order = vec![a, hidden, c];
        let resolved_of = |q: Entity| Some((Vec2::splat(q.index() as f32 * 10.0), Vec2::splat(10.0)));
        let color_of = |_q: Entity| Color::WHITE;
        let radius_of = |_q: Entity| 0.0;
        let clip_of = |q: Entity| {
            if q == c { Some(ClipRect { min: Vec2::ZERO, max: Vec2::splat(100.0) }) } else { None }
        };
        let top_layer_of = |_q: Entity| TopLayer::None;
        let skips_of = |q: Entity| q == hidden; // CssVisibility::Hidden on `hidden`
        let painters_of = |_q: Entity| None;

        let draws = build_draws(
            root, &order, &resolved_of, &color_of, &radius_of, &clip_of, &top_layer_of, &skips_of, &painters_of,
        );

        // `hidden` is dropped; order is [a, c] (painters_z order minus skip).
        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].position, Vec2::new(10.0, 10.0)); // a
        assert_eq!(draws[1].position, Vec2::new(30.0, 30.0)); // c
        // c carries its clip; a does not.
        assert!(draws[0].clip.is_none());
        assert_eq!(draws[1].clip, Some(ClipRect { min: Vec2::ZERO, max: Vec2::splat(100.0) }));
    }

    #[test]
    fn build_draws_marks_top_layer_entries() {
        let root = e(0);
        let (inflow, modal) = (e(1), e(2));
        let order = vec![inflow, modal];
        let resolved_of = |_q: Entity| Some((Vec2::ZERO, Vec2::splat(10.0)));
        let color_of = |_q: Entity| Color::WHITE;
        let radius_of = |_q: Entity| 0.0;
        let clip_of = |_q: Entity| None;
        let top_layer_of = |q: Entity| if q == modal { TopLayer::Modal } else { TopLayer::None };
        let skips_of = |_q: Entity| false;
        let painters_of = |_q: Entity| None;

        let draws = build_draws(
            root, &order, &resolved_of, &color_of, &radius_of, &clip_of, &top_layer_of, &skips_of, &painters_of,
        );
        assert!(!draws[0].is_top_layer);
        assert!(draws[1].is_top_layer, "modal draw flagged top-layer for the root composite");
    }
}
```

- [ ] Extend `DrawData` with the two new fields (keep `#[non_exhaustive]`; the `new` constructor keeps the Phase-0 signature and defaults the new fields, so `render_instance.rs` keeps compiling). Add the fields:

```rust
    /// The entity's computed clip; `None` ⇒ no scissor (full view). Read by
    /// `BuiyNode::run` to set the per-draw scissor rect (clip-and-transform.md § A).
    pub clip: Option<crate::render::clip::ClipRect>,
    /// True iff this draw is a top-layer member (already at the root
    /// `painters_z` tail). The node composites these at the root after the
    /// in-flow layers (paint-order-and-top-layer.md § 3).
    pub is_top_layer: bool,
```

- [ ] Add the pure `build_draws` free function (signature matches the test). It uses `paint_order::flatten_paint_order` for ordering and prunes skipped subtrees (a skipped entry's descendants are not emitted — implement by checking `skips_of` per emitted entity and, when skipping, **not descending** into its nested `painters_of`). Minimal real impl:

```rust
/// Pure core of `extract_buiy_draws`: walk the root `painters_z` forward
/// (atomic nested-SC descent), drop skipped subtrees, and emit one `DrawData`
/// per painted entity in paint order with its clip + top-layer flag.
///
/// Render NEVER re-sorts (paint-order-and-top-layer.md § 1.2): the order is
/// `flatten_paint_order`'s verbatim read of layout's list.
#[allow(clippy::too_many_arguments)]
pub fn build_draws<'a, R, C, Ra, Cl, Tl, Sk, Po>(
    root: Entity,
    root_painters: &'a [Entity],
    resolved_of: &R,
    color_of: &C,
    radius_of: &Ra,
    clip_of: &Cl,
    top_layer_of: &Tl,
    skips_of: &Sk,
    painters_of: &Po,
) -> Vec<DrawData>
where
    R: Fn(Entity) -> Option<(Vec2, Vec2)>,
    C: Fn(Entity) -> Color,
    Ra: Fn(Entity) -> f32,
    Cl: Fn(Entity) -> Option<crate::render::clip::ClipRect>,
    Tl: Fn(Entity) -> crate::layout::TopLayer,
    Sk: Fn(Entity) -> bool,
    Po: Fn(Entity) -> Option<&'a [Entity]>,
{
    let _ = root;
    let mut out = Vec::new();
    fn walk<'a, R, C, Ra, Cl, Tl, Sk, Po>(
        painters: &'a [Entity],
        resolved_of: &R, color_of: &C, radius_of: &Ra, clip_of: &Cl,
        top_layer_of: &Tl, skips_of: &Sk, painters_of: &Po, out: &mut Vec<DrawData>,
    )
    where
        R: Fn(Entity) -> Option<(Vec2, Vec2)>, C: Fn(Entity) -> Color, Ra: Fn(Entity) -> f32,
        Cl: Fn(Entity) -> Option<crate::render::clip::ClipRect>, Tl: Fn(Entity) -> crate::layout::TopLayer,
        Sk: Fn(Entity) -> bool, Po: Fn(Entity) -> Option<&'a [Entity]>,
    {
        for &p in painters {
            if skips_of(p) {
                continue; // skip the entity AND its subtree — do not descend.
            }
            if let Some((position, size)) = resolved_of(p) {
                let mut d = DrawData::new(position, size, color_of(p), radius_of(p));
                d.clip = clip_of(p);
                d.is_top_layer = top_layer_of(p) != crate::layout::TopLayer::None;
                out.push(d);
            }
            if let Some(nested) = painters_of(p) {
                walk(nested, resolved_of, color_of, radius_of, clip_of, top_layer_of, skips_of, painters_of, out);
            }
        }
    }
    walk(root_painters, resolved_of, color_of, radius_of, clip_of, top_layer_of, skips_of, painters_of, &mut out);
    out
}
```

- [ ] Rewrite `extract_buiy_draws` as the thin adapter: query the root entity(ies) carrying `StackingContext` (the root context — an entity `With<StackingContext>, Without<ChildOf>` or the layout-root definition), build the closures from `Extract<Query<(&Visual, &ResolvedLayout, Option<&ClipRect>, Option<&Stacking>, Option<&CssVisibility>, Option<&OffscreenAuto>)>>` plus a `Query<&StackingContext>` for `painters_of`, and call `build_draws`. Keep the existing theme-color resolution + window-size population. **Where `ClipRect` / `CssVisibility` / `OffscreenAuto` are not yet in the tree (cross-phase), use the Task-1/Task-6 local shapes via `crate::render::clip::ClipRect` etc.** The adapter does real work only at runtime with a render world; its correctness is proven by `build_draws` headless tests + Task 8 GPU.
- [ ] Run + FAIL first (before adding `build_draws`): `cargo test -p buiy_core --lib render::extract_tests` → compile error (no `build_draws`). Add the field + fn, re-run → PASS.
- [ ] Confirm `render_instance.rs` still compiles (the `DrawData::new` signature is unchanged; new fields default). Run `cargo test -p buiy_core --test render_instance` → PASS.
- [ ] Run the full GATE. Commit: `feat(render): extract walks painters_z in order, carrying clip + top-layer flag, pruning skips`.

---

## Task 8 — `BuiyNode::run`: per-entity scissor + top-layer-at-root composite (GPU, #[ignore] e2e)

Wire the consumption into the actual draw: in `BuiyNode::run`, walk the extracted draws (already in paint order from Task 7), set `pass.set_scissor_rect(...)` per draw from its `Option<ClipRect>` (absent ⇒ reset to full view; degenerate ⇒ skip the draw), and paint the top-layer-flagged draws **last** at the root with the **window viewport** as scissor (top-layer escapes ancestor clip — §3.2). Because this needs a real render pass / wgpu adapter, the end-to-end assertion is `#[ignore]`-gated exactly like `render_smoke.rs`; the device-free assertions already shipped in Tasks 1–7.

**Files**
- Modify: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/src/render/node.rs`
- Test: `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline/crates/buiy_core/tests/render_smoke.rs` (add `#[ignore]` GPU cases)

Steps:

- [ ] Write the failing GPU test first (mirrors the `#[ignore]` idiom already in `render_smoke.rs`). Append:

```rust
// GPU e2e: per-entity scissor + top-layer composite. Needs a wgpu adapter
// (real GPU or lavapipe); CI has none, so #[ignore] exactly like the other
// render-graph tests. The device-free consumption logic (paint order, scissor
// derivation, top-layer partition, skip rules) is proven headless in the
// `render::{clip,paint_order,top_layer,skip}` unit tests + `render_paint_order`.
//
// Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by the e2e golden harness"]
fn buiy_node_scissors_per_entity_and_composites_top_layer() {
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

- [ ] Run + confirm it is collected and **ignored** (no adapter): `cargo test -p buiy_core --test render_smoke` → shows `ignored`. The GATE stays green.
- [ ] Implement the `BuiyNode::run` changes. Two real behaviors:
  - **Per-draw scissor.** After building the instance buffer, instead of one `pass.draw(0..4, 0..n)`, iterate draws: for each draw compute `scissor_rect(&clip, scale_factor, view_physical)` (from `render::clip`); `None` (absent clip) ⇒ `pass.set_scissor_rect(0, 0, view_w, view_h)` (full view); `Some((x,y,w,h))` ⇒ `pass.set_scissor_rect(x,y,w,h)`; a degenerate clip (the `scissor_rect` returned `None` because `min>=max`, distinct from absent) ⇒ **skip that draw's instance**. Then issue the instanced draw for that one instance (or batch consecutive same-scissor draws — batching is a perf detail, correctness first). Obtain `scale_factor` + physical view size from the `ViewTarget` / extracted view (the `window_size` is logical; multiply, or read the view's physical size). Keep the existing early-returns (empty draws / zero window).
  - **Top-layer last at root.** Partition the extracted draws into in-flow vs `is_top_layer` (stable, preserving order — they are already tail-ordered). Draw in-flow first, then top-layer draws, **each top-layer draw scissored to the full window viewport** (ignore its `clip` — §3.2 clip escape). This is the "single ordered top-layer composite pass" (architecture § 2.3): for v1 it is a draw-order relocation, **no `::backdrop` dimming box** (§4 OPEN — paint top-layer over in-flow with no synthesized scrim).
- [ ] Add a code comment at the top-layer block: `// v1 ::backdrop = OPEN (paint-order § 4): no dimming backdrop. Top-layer paints over in-flow in layout's tier order; the golden asserts tier ORDER only.`
- [ ] Run + FAIL→handle: the new test stays `ignored` (green on CI). Locally on a GPU box, `cargo test -p buiy_core --test render_smoke -- --ignored` should pass once wired; if you cannot run on a GPU box, the headless tests + a careful read are the verification (note this in the commit body honestly — do NOT claim the GPU path passed if you did not run it).
- [ ] Run the full GATE (headless) → green. Commit: `feat(render): BuiyNode applies per-entity scissor + composites top-layer at root (GPU #[ignore])`.

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
- [ ] HEADLESS-gating tests (run on CI, no adapter): `render::clip` (scissor-rect derivation + outline-clip selector), `render::paint_order` (forward walk + atomicity + reverse identity), `render::top_layer` (tail partition, no re-sort), `render::skip` (paint-skip predicate), `render::extract_tests` (`build_draws` order + clip + top-layer flag + skip pruning), and integration `tests/render_paint_order.rs` (real 6f tail tier order + modal-first-hit).
- [ ] GPU `#[ignore]` tests (real code, no CI adapter): `tests/render_smoke.rs::buiy_node_scissors_per_entity_and_composites_top_layer` — the actual scissored draw + top-layer-at-root composite; the tier-order golden lives in the gate-#2 visual-regression harness, which this phase feeds but does not own.
- [ ] v1 `::backdrop` = no dimming backdrop (§4 OPEN); the top-layer composite paints over in-flow in layout's tier order, and the modal-over-popover golden asserts **tier ORDER only**.
- [ ] No render-side re-sort anywhere: paint order, hit-test order, and the top-layer tail are all verbatim reads of layout's `painters_z` (the §1.2 / §3.1 hard constraint), enforced by the `render_does_not_reorder_*` tests.
