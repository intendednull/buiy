# Coordinate-Space + Picking/Clip Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Make every absolute-coordinate consumer (picking AABB + the clip producer) read window-absolute position from a **non-optional** `GlobalTransform` instead of the parent-local `ResolvedLayout.position`, fixing Bug 1 (picking, live) and its latent twin in `render/clip.rs`, while preserving the `bridge.rs:138` `position − acc` invariant.

**Architecture:** `ResolvedLayout.position` stays parent-local (Taffy's per-node `location`); the transform bridge (`render/bridge.rs`) remains the sole accumulator that folds `position − ancestor_scroll` into `Transform` → `GlobalTransform`. C1 routes `point_in_aabb` / `hit_test` / `emit_picks` and `write_clip_rects`'s `walk` through `GlobalTransform.translation().truncate()`, adds the load-bearing `write_clip_rects.after(sync_simple_transforms)` scheduling edge, and corrects the lying `components.rs:65` doc comment. No `ResolvedLayout` value, no paint, and no `PackedInstance` byte layout change — so **no snapshot/golden re-bless**.

**Tech Stack:** Rust, Bevy 0.19.0-rc.3 (`bevy::picking` backend + `PointerHits`/`HitData`, `bevy::transform::systems::sync_simple_transforms`), Taffy layout, `buiy_core` (`picking`, `render/clip`, `render/bridge`, `layout`), `buiy_verify` (C7 `PointerHarness`).

**Wave / dependencies:** Wave 1. Depends on **C0** (umbrella decisions) only. Co-delivered with **C7** — C7 **owns** the shared Wave-1 test infrastructure: `buiy_verify::pointer::PointerHarness` (`crates/buiy_verify/src/pointer.rs`) and the offset-picking RED proof `synthetic_pointer_hits_offset_widget_at_its_global_position` (`crates/buiy_verify/tests/pointer_offset_regression.rs`), landed by C7 as a committed `#[ignore = "RED until C1 …"]`. **C1 does NOT create the harness or that test** — C1's job is to land the coordinate fix and then **un-ignore** C7's offset test (delete its `#[ignore]` attribute) so it goes GREEN. That committed RED→GREEN transition IS C1's coordinate-fix regression proof (umbrella §9.5; the old `picking_backend.rs`/`picking.rs` hand-write `ResolvedLayout` and are structurally blind to Bug 1). C1 strictly **precedes** C3 (input model rewrites `emit_picks` body around the AABB seam C1 leaves), C5 (overlays/scroll are the first offset clippers), and C6 (outline survives `overflow:hidden` via the now-correct `AncestorClip`). C1 also lands its OWN buiy_core RED-first tests the harness does not cover — the nested+offset overflow-clip test (Task 5), the clip-after-bridge ordering test (Task 6), and the no-fallback assertions (Task 7) — each written RED against pre-fix code in the same task, then GREEN after the fix (correct RED-first, not a hand-revert).

---

## PHASE 0 (Task 0): Rebase + re-confirm anchors

**This plan's code blocks were written against `507855f`. They MUST be re-confirmed against the then-current `origin/main` before any edit.** Implementation is gated on the inspection-tools merge + a fresh rebase (umbrella §8). The prototype diffs are stale-base and are re-derived here, never cherry-picked.

**Files**
- Read: `/mnt/storage/projects/buiy/CLAUDE.md` (Build & Test), `crates/buiy_core/src/picking/mod.rs`, `crates/buiy_core/src/picking/backend.rs`, `crates/buiy_core/src/render/clip.rs`, `crates/buiy_core/src/render/mod.rs`, `crates/buiy_core/src/render/bridge.rs`, `crates/buiy_core/src/lib.rs`, `crates/buiy_core/src/components.rs`, `crates/buiy_core/tests/picking.rs`, `crates/buiy_core/tests/picking_backend.rs`, `crates/buiy_core/tests/render_clip_rects.rs`
- Read (C7-owned, do NOT edit beyond un-ignoring): `crates/buiy_verify/src/pointer.rs` (the `PointerHarness`), `crates/buiy_verify/tests/pointer_offset_regression.rs` (the offset RED proof C1 un-ignores). Confirm both are present on the rebased tree (C7 lands in the same wave, before C1).

**Steps**
- [ ] **Fetch + branch fresh from upstream.** `git fetch --all --prune`, then `git log --oneline -1 origin/main` (confirm it is *not* `507855f`). Create the work branch from the remote ref: `git switch -c c1-coordinate-space origin/main`. Do **not** branch from a stale local `main`/HEAD. (This picks up any merged inspection-tools / #77 testing-audit / #78 CI-hardening.)
- [ ] **Re-grep every anchor cited below and fix drift.** Run each grep; if a line moved, update the Task that cites it. Anchors to re-confirm:
  - `grep -n "fn point_in_aabb" crates/buiy_core/src/picking/mod.rs` → expect `point_in_aabb(point: Vec2, layout: &ResolvedLayout)` (was `mod.rs:51`).
  - `grep -n "fn hit_test" crates/buiy_core/src/picking/mod.rs` → expect `QueryState::<(Entity, &ResolvedLayout)>` body (was `mod.rs:37-49`).
  - `grep -n "fn emit_picks" crates/buiy_core/src/picking/backend.rs` and `grep -n "nodes: Query" crates/buiy_core/src/picking/backend.rs` → expect `Query<(Entity, &ResolvedLayout)>` (was `backend.rs:29`); `point_in_aabb(cursor, layout)` (was `backend.rs:42`).
  - `grep -n "Aabb::from_box(rl.position" crates/buiy_core/src/render/clip.rs` → was `clip.rs:286`; `grep -n "type ClipNodeData" crates/buiy_core/src/render/clip.rs` → was `clip.rs:215`.
  - `grep -n "write_clip_rects" crates/buiy_core/src/render/mod.rs` → the `.after(crate::BuiySet::Animate).before(crate::BuiySet::Picking)` block (was `render/mod.rs:119-124`).
  - `grep -n "sync_simple_transforms" crates/buiy_core/src/lib.rs` → import at top (was `lib.rs:7-9`); bridge chain `.chain().after(BuiySet::Animate).before(BuiySet::Picking)` (was `lib.rs:108-129`).
  - `grep -n "window-relative" crates/buiy_core/src/components.rs` → the `ResolvedLayout.position` doc lie (was `components.rs:65`).
  - `grep -n "resolved.position - acc" crates/buiy_core/src/render/bridge.rs` → the invariant to PRESERVE (was `bridge.rs:138`).
- [ ] **Confirm the C7-owned harness + RED proof are present (do NOT recreate them).** C7 lands first in this wave. Verify:
  - `grep -n "pub fn spawn_offset_tree\|pub fn top_hit\|pub fn global_center\|pub fn move_to\|pub fn world_mut\|pub fn captured" crates/buiy_verify/src/pointer.rs` → expect the contract API surface: `spawn_offset_tree(offset: Vec2, scene: impl Bundle) -> Entity`, `move_to(pos: Vec2)`, `press/release/click(button: PointerButton)`, `top_hit() -> Option<Entity>`, `global_center(entity) -> Vec2`, `world_mut() -> &mut World`, `captured() -> &CapturedEvents`. **Use exactly this surface in Task 4** — do not invent helpers, do not recreate the harness inline.
  - `grep -n "#\[ignore" crates/buiy_verify/tests/pointer_offset_regression.rs` → expect `synthetic_pointer_hits_offset_widget_at_its_global_position` carries `#[ignore = "RED until C1 …"]`. **This is the line C1 deletes in Task 4** — note its exact text. If C7's offset-tree shape differs from what Task 4 assumes (offset value, target size), align Task 4's expected numbers to C7's actual fixture.
  - If, against the wave ordering, C7 is **not** yet merged when C1 starts, STOP and resolve ordering with the wave coordinator — C1 must not recreate the C7-owned file, harness, or test (contract: C7 is the sole creator). C1's own buiy_core tests (Tasks 5–7) are independent of the harness and can proceed.
- [ ] **Integrate merged inspection tools.** If the inspection-tools merge added helpers/lints touching picking or clip, note them; they do not change C1's seams but may change adjacent imports.
- [ ] **Baseline gate green.** Run the headless gate on the two touched crates to confirm a clean base before any edit:
  ```sh
  cargo test -p buiy_core --test picking --test picking_backend --test render_clip_rects
  ```
  Expected: all existing picking + clip tests PASS. Record the count — Task 5/6 must not regress it.
- [ ] **Confirm the RED-first seam reproduces.** This is the geometry every later task relies on. Temporarily add this throwaway test to `crates/buiy_core/tests/render_clip_rects.rs`, run it, confirm the printed values, then delete it:
  ```rust
  #[test]
  fn phase0_probe_offset_seam() {
      let mut app = app();
      let outer = app.world_mut().spawn((
          Node,
          Style::default().width_px(400.0).height_px(400.0).translate_px(70.0, 90.0),
      )).id();
      let clipper = app.world_mut().spawn((
          Node,
          Style::default().width_px(100.0).height_px(100.0)
              .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
      )).id();
      let child = app.world_mut().spawn((Node, Style::default().width_px(300.0).height_px(300.0))).id();
      app.world_mut().entity_mut(outer).add_child(clipper);
      app.world_mut().entity_mut(clipper).add_child(child);
      app.update();
      app.update();
      let rl = app.world().get::<ResolvedLayout>(clipper).cloned().unwrap();
      let gt = app.world().get::<GlobalTransform>(clipper).map(|g| g.translation().truncate());
      let ac = app.world().get::<AncestorClip>(child).cloned();
      panic!("clipper rl.position={:?} gt={:?} child AncestorClip={:?}", rl.position, gt, ac);
  }
  ```
  Expected panic message on **pre-fix** code (confirmed against `507855f`): `clipper rl.position=Vec2(0.0, 0.0) gt=Some(Vec2(70.0, 90.0)) child AncestorClip=Some(AncestorClip { min: Vec2(0.0, 0.0), max: Vec2(100.0, 100.0) })`. The divergence `rl.position (0,0)` vs `gt (70,90)` is the Bug-1 seam; the wrong `AncestorClip` at the origin (instead of `(70,90)..(170,190)`) is what Task 5 turns into a permanent RED-first test. **If the values differ after rebase** (e.g. `translate_px` lowering changed), re-derive the expected numbers here and propagate to Task 5. Delete the probe before committing.
- [ ] **Commit (docs/branch only — no code change yet):** nothing to commit; Phase 0 produces no diff. Proceed to Task 1.

---

## Task 1: Fix the `components.rs:65` doc lie (rule written before code obeys it)

The doc comment claims `ResolvedLayout.position` is "window-relative". It is parent-**local**. Fix it first so the absolute-coordinate rule is documented before any consumer is changed (spec §2.4, §5 step 1). Doc-only; no behavior change, no test.

**Files**
- Modify: `crates/buiy_core/src/components.rs:64-66` (the `ResolvedLayout.position` doc comment)

**Steps**
- [ ] **Re-read the current comment** (Phase 0 confirmed the line; re-read to get the exact text for the edit). It is:
  ```rust
      /// Top-left position in logical pixels (window-relative).
      pub position: Vec2,
  ```
- [ ] **Replace the comment** with the parent-local text (spec §2.4 verbatim):
  ```rust
      /// Top-left position in logical pixels, **parent-relative** (Taffy's
      /// per-node `location`; only `PostTaffyPositionOverrides` substitutes it —
      /// sticky/table/multicol/anchor — never a general accumulation). This is NOT
      /// an absolute coordinate: the transform bridge (`render/bridge.rs`) is the
      /// sole accumulator (`position − ancestor_scroll` → `Transform` →
      /// `GlobalTransform`). Absolute consumers (picking, clip, render extract,
      /// overlays) MUST read `GlobalTransform.translation().truncate()`, never this
      /// field. See docs/specs/2026-06-22-buiy-widget-catalog-design/coordinate-space-correctness.md.
      pub position: Vec2,
  ```
- [ ] **Confirm it still builds** (rustdoc intra-doc links / no warnings):
  ```sh
  RUSTDOCFLAGS="-D warnings" cargo doc -p buiy_core --no-deps
  ```
  Expected: builds with no warnings.
- [ ] **Commit:** `docs(picking): correct ResolvedLayout.position doc to parent-relative (C1)`

---

## Task 2: Picking coordinate basis — `point_in_aabb` / `hit_test` read `GlobalTransform`

Change `point_in_aabb` to take an absolute top-left + size (pure geometry), and make `hit_test` query `&GlobalTransform` **non-optionally** (spec §2.1, §5 step 2). C1 changes *only the coordinate the AABB is computed in*; it touches neither the smallest-area tiebreak nor `emit_picks`'s depth/camera/no-hit (those are C3). The existing `tests/picking.rs` spawns a bare `ResolvedLayout` with no bridge, so after this change its node has no `GlobalTransform` and drops out of the query — it is **re-homed in Task 4** onto the C7-owned `PointerHarness` (real layout→bridge→`GlobalTransform`).

**Files**
- Modify: `crates/buiy_core/src/picking/mod.rs` (`point_in_aabb` signature + `hit_test` query/call)
- Test: `crates/buiy_core/tests/picking.rs` (re-homed in Task 4 — left RED at the end of this task)

**Steps**
- [ ] **Write the failing unit test first.** `point_in_aabb` is `pub(crate)`, so test it inside `picking/mod.rs`. Add a `#[cfg(test)]` module at the end of `crates/buiy_core/src/picking/mod.rs` proving the new signature operates on an absolute top-left (this is the unit-level RED for the new geometry):
  ```rust
  #[cfg(test)]
  mod aabb_tests {
      use super::*;

      #[test]
      fn point_in_aabb_uses_absolute_top_left() {
          // A widget whose absolute top-left is (70,90), size 100x100. A point at
          // (120,140) is inside the ABSOLUTE box but OUTSIDE a box anchored at the
          // origin — so this only passes once point_in_aabb takes an absolute pos.
          let abs_pos = Vec2::new(70.0, 90.0);
          let size = Vec2::new(100.0, 100.0);
          assert!(point_in_aabb(Vec2::new(120.0, 140.0), abs_pos, size));
          assert!(!point_in_aabb(Vec2::new(20.0, 20.0), abs_pos, size));
          // boundary inclusive on both edges
          assert!(point_in_aabb(abs_pos, abs_pos, size));
          assert!(point_in_aabb(abs_pos + size, abs_pos, size));
      }
  }
  ```
- [ ] **Run it — expect a COMPILE failure (RED).** The current `point_in_aabb(point: Vec2, layout: &ResolvedLayout)` does not accept `(Vec2, Vec2, Vec2)`:
  ```sh
  cargo test -p buiy_core --lib picking::aabb_tests 2>&1 | tail -20
  ```
  Expected: `error[E0061]: this function takes 2 arguments but 3 arguments were supplied` (or arg-type mismatch) — the RED.
- [ ] **Change `point_in_aabb` to pure geometry** (spec §2.1). Replace `crates/buiy_core/src/picking/mod.rs:51-57`:
  ```rust
  pub(crate) fn point_in_aabb(point: Vec2, abs_pos: Vec2, size: Vec2) -> bool {
      let max = abs_pos + size;
      point.x >= abs_pos.x && point.x <= max.x && point.y >= abs_pos.y && point.y <= max.y
  }
  ```
- [ ] **Update `hit_test` to query `&GlobalTransform` non-optionally** (spec §2.1; no fallback — D2). Replace the body of `hit_test` (`mod.rs:37-49`):
  ```rust
  pub fn hit_test(world: &World, point: Vec2) -> Option<Entity> {
      let mut state =
          QueryState::<(Entity, &ResolvedLayout, &GlobalTransform)>::try_new(world)?;
      let mut best: Option<(Entity, f32)> = None; // entity, area (smallest wins for top-most)
      for (entity, layout, gt) in state.iter(world) {
          let abs_pos = gt.translation().truncate();
          if point_in_aabb(point, abs_pos, layout.size) {
              let area = layout.size.x * layout.size.y;
              if best.map(|(_, a)| area < a).unwrap_or(true) {
                  best = Some((entity, area));
              }
          }
      }
      best.map(|(e, _)| e)
  }
  ```
  (`GlobalTransform` is in `bevy::prelude::*`, already imported at `mod.rs:8`.) Leave the `hit_test` doc comment's "non-optional query" intent intact; add one sentence: `A node without a GlobalTransform (never bridged) is absent from the query — the same drop render accepts; no fallback.`
- [ ] **Run the unit test — expect PASS (GREEN):**
  ```sh
  cargo test -p buiy_core --lib picking::aabb_tests 2>&1 | tail -5
  ```
  Expected: `test result: ok. 1 passed`.
- [ ] **Confirm `tests/picking.rs` is now RED** (it spawns no `GlobalTransform`; this is expected and fixed in Task 4):
  ```sh
  cargo test -p buiy_core --test picking 2>&1 | tail -15
  ```
  Expected: `hit_test_returns_entity_under_point` FAILS at `assert_eq!(hit, Some(entity))` — the node dropped out of the `&GlobalTransform` query. **Do not fix it here**; Task 4 re-homes it onto the bridge path. (`backend.rs` still references the old call shape — it will not compile yet; that is fixed in Task 3, which is part of the same logical change. Run the `--lib` test, not the full crate, until Task 3 lands.)
- [ ] **Commit:** `feat(picking): hit_test reads absolute pos via non-optional GlobalTransform (C1)`

---

## Task 3: Picking backend — `emit_picks` widens its query to `&GlobalTransform`

Widen `emit_picks`'s `nodes` query and hit-test in absolute space (spec §2.1, §5 step 2). **Touch only the AABB call** — leave the smallest-area sort (`backend.rs:50-53`), `Entity::PLACEHOLDER` camera (`backend.rs:65`), `order = 0.0` (`backend.rs:74`), depth-by-rank, and no-hit emission to C3 (spec §2.1 boundary, §4.1). `tests/picking_backend.rs` spawns a bare `ResolvedLayout` and is re-homed onto the C7-owned `PointerHarness` in Task 4.

**Files**
- Modify: `crates/buiy_core/src/picking/backend.rs` (`emit_picks` `nodes` query + the `point_in_aabb` call)
- Test: `crates/buiy_core/tests/picking_backend.rs` (re-homed in Task 4 — left RED at the end of this task)

**Steps**
- [ ] **Confirm the RED.** With Task 2 landed, `backend.rs:42` still calls `point_in_aabb(cursor, layout)` (two args) against the new three-arg signature, so the crate test build is broken:
  ```sh
  cargo build -p buiy_core --tests 2>&1 | tail -15
  ```
  Expected: `error[E0061]` at `backend.rs:42` — the compile RED that Task 3 resolves.
- [ ] **Widen the `nodes` query.** Replace `backend.rs:29`:
  ```rust
      nodes: Query<(Entity, &ResolvedLayout, &GlobalTransform)>,
  ```
- [ ] **Hit-test in absolute space.** Replace the collection loop (`backend.rs:41-46`) — the `for` over `nodes.iter()`:
  ```rust
          for (entity, layout, gt) in nodes.iter() {
              let abs_pos = gt.translation().truncate();
              if point_in_aabb(cursor, abs_pos, layout.size) {
                  let area = layout.size.x * layout.size.y;
                  hits.push((entity, area));
              }
          }
  ```
  Leave everything else in `emit_picks` byte-for-byte: the `hits.is_empty()` early-continue, the `hits.sort_by(...)` smallest-area sort, the `HitData::new(Entity::PLACEHOLDER, i as f32, None, None)` mapping, and `output.write(PointerHits::new(*pointer, picks, 0.0))`. Add a one-line comment above the loop: `// C1: absolute basis = GlobalTransform; C3 owns depth/camera/no-hit.`
- [ ] **Build — expect it to compile (GREEN at the build level):**
  ```sh
  cargo build -p buiy_core --tests 2>&1 | tail -5
  ```
  Expected: compiles (the integration tests `picking.rs` / `picking_backend.rs` will still *fail at runtime* — fixed in Task 4).
- [ ] **Confirm `tests/picking_backend.rs` is RED at runtime** (no `GlobalTransform` on its hand-spawned node → dropped from the query → no hit emitted):
  ```sh
  cargo test -p buiy_core --test picking_backend 2>&1 | tail -15
  ```
  Expected: `pointer_over_buiy_node_emits_hit` FAILS at the `assert!(any_hit, ...)` — the node had no `GlobalTransform`. **Do not fix here**; Task 4 re-homes it.
- [ ] **Commit:** `feat(picking): emit_picks hit-tests in absolute space via GlobalTransform (C1)`

---

## Task 4: Un-ignore C7's offset RED proof + re-home `picking.rs` / `picking_backend.rs` onto the C7 `PointerHarness`

The load-bearing RED→GREEN proof for C1 is **C7-owned**: C7 committed `synthetic_pointer_hits_offset_widget_at_its_global_position` (in `crates/buiy_verify/tests/pointer_offset_regression.rs`) as `#[ignore = "RED until C1 routes picking through GlobalTransform …"]`, driving the real layout→bridge→`GlobalTransform` chain through the C7 `PointerHarness`. **C1 does not write that test or the harness, and uses no manual hand-revert demonstration** — C1 simply **deletes the `#[ignore]`** and asserts the test is now GREEN. The committed un-ignore IS the RED→GREEN coordinate-fix proof (umbrella §9.5; the contract: C7 is the sole creator of the harness + this test).

Then re-home the two stale unit tests (`tests/picking.rs` + `tests/picking_backend.rs`) — which hand-write `ResolvedLayout` with no `GlobalTransform` and are structurally blind to Bug 1 — onto the **C7-owned `PointerHarness`** so they exercise the real bridge path through the contract API. No inline `laid_out_app`, no recreated harness: the harness exists and is owned by C7. `buiy_verify` is already a `[dev-dependencies]` of `buiy_core` (the existing dev-only `buiy_core → buiy_verify → buiy_core` cycle, `crates/buiy_core/Cargo.toml`), so these tests can `use buiy_verify::pointer::PointerHarness;` with no manifest change.

**C7-owned `PointerHarness` API (contract — use exactly this surface):**
- `PointerHarness::new() -> Self`
- `spawn_offset_tree(offset: Vec2, scene: impl Bundle) -> Entity` — `offset` is an EXPLICIT arg (the root is placed at this window offset); `scene` is the bundle spawned as the offset root's content; returns the entity under test.
- `move_to(pos: Vec2)`, `press(button: PointerButton)`, `release(button: PointerButton)`, `click(button: PointerButton)`
- `top_hit() -> Option<Entity>`, `global_center(entity: Entity) -> Vec2`
- `world_mut() -> &mut World` (app/world access for assertions), `captured() -> &CapturedEvents`

**Files**
- Modify: `crates/buiy_verify/tests/pointer_offset_regression.rs` (delete the `#[ignore]` on `synthetic_pointer_hits_offset_widget_at_its_global_position` — C1's only edit to a C7-owned file)
- Modify: `crates/buiy_core/tests/picking.rs` (re-home onto `PointerHarness` for the `hit_test` integration assertion)
- Modify: `crates/buiy_core/tests/picking_backend.rs` (re-home onto `PointerHarness` for the backend `top_hit` assertion)

**Steps**
- [ ] **Un-ignore C7's offset RED proof.** Open `crates/buiy_verify/tests/pointer_offset_regression.rs` and DELETE the `#[ignore = "RED until C1 routes picking through GlobalTransform …"]` attribute on `synthetic_pointer_hits_offset_widget_at_its_global_position` (the exact text was captured in Phase 0). Do not touch the test body, the harness, or any other C7 file — un-ignoring is the whole edit.
- [ ] **Run it — expect GREEN (the C1 coordinate fix is what flips it):**
  ```sh
  cargo test -p buiy_verify --test pointer_offset_regression 2>&1 | tail -8
  ```
  Expected: `test result: ok. 1 passed` — the synthetic pointer over the offset widget's absolute center now hits the target. (On pre-C1 code this same test was the captured RED; the un-ignore turning it GREEN is C1's coordinate-fix proof — no hand-revert needed, the git history of C7's committed `#[ignore]` → C1's deletion is the RED→GREEN record.)
- [ ] **Re-home `tests/picking.rs` onto the `PointerHarness` (offset integration assertion for the free `hit_test` fn).** Replace the whole file. The harness builds the real layout→bridge→`GlobalTransform` stack and offsets the tree via the explicit `offset` arg, so `hit_test` is exercised against an absolute box:
  ```rust
  //! C1: the free `hit_test` fn reads absolute position via GlobalTransform.
  //! Re-homed onto the C7-owned PointerHarness (crates/buiy_verify/src/pointer.rs),
  //! which drives the real layout → bridge → GlobalTransform chain with the root
  //! placed at an explicit window offset — so this OBSERVES Bug 1, unlike the
  //! prior hand-written single-node ResolvedLayout test. The harness is the SOLE
  //! coordinate-correctness gate (C7 owns it; do not recreate it here).
  use bevy::prelude::*;
  use buiy_core::{Node, ResolvedLayout, layout::Style, picking::hit_test};
  use buiy_verify::pointer::PointerHarness;

  #[test]
  fn hit_test_returns_entity_under_offset_widget() {
      // The harness places the root at window offset (70,90); the target is the
      // offset root's content, so its ResolvedLayout.position is parent-local but
      // its GlobalTransform.translation is the accumulated absolute. A pre-C1
      // hit_test (reading ResolvedLayout.position) would look at the origin box
      // and MISS the absolute box; the fixed one HITS it.
      let mut h = PointerHarness::new();
      let target = h.spawn_offset_tree(
          Vec2::new(70.0, 90.0),
          (Node, Style::default().width_px(100.0).height_px(50.0)),
      );

      // Sanity: the target is genuinely offset (parent-local position != absolute).
      let rl = h.world_mut().get::<ResolvedLayout>(target).cloned().unwrap();
      let gt = h
          .world_mut()
          .get::<GlobalTransform>(target)
          .unwrap()
          .translation()
          .truncate();
      assert_ne!(gt, rl.position, "absolute != parent-local (the offset is real)");

      // hit_test lands at the ABSOLUTE box (the target's global center), not the
      // origin box where the buggy code looked.
      let center = h.global_center(target);
      assert_eq!(
          hit_test(h.world_mut(), center),
          Some(target),
          "a point at the target's GLOBAL center hits it"
      );
      assert_eq!(
          hit_test(h.world_mut(), Vec2::new(10.0, 10.0)),
          None,
          "a point in the ORIGIN box (where pre-C1 code looked) misses"
      );
  }
  ```
  > Note on the world accessor: `hit_test(&World, Vec2)` takes `&World`; `world_mut()` yields `&mut World`, which coerces to `&World` at the call site (a shared reborrow). If the harness also exposes a shared `world()` accessor on the rebased tree, prefer it for these read-only calls and note the swap; the contract guarantees `world_mut()`, so this plan targets that.
- [ ] **Run it — expect GREEN:**
  ```sh
  cargo test -p buiy_core --test picking 2>&1 | tail -8
  ```
  Expected: `test result: ok. 1 passed`. (No revert demonstration here — the C7-owned `pointer_offset_regression` un-ignore above is the RED→GREEN proof; this test is the re-homed integration coverage for the free `hit_test` fn.)
- [ ] **Re-home `tests/picking_backend.rs` onto the `PointerHarness` (offset backend assertion via `top_hit`).** Replace the file body. The harness owns the synthetic-pointer injection (the sanctioned path); C1 just drives `move_to(center)` + `top_hit()`:
  ```rust
  //! C1: the bevy_picking backend (emit_picks) hit-tests in absolute space.
  //! Re-homed onto the C7-owned PointerHarness, which spawns the target off the
  //! origin via the real layout → bridge chain and injects a synthetic pointer
  //! through the sanctioned bevy_picking path — so this observes Bug 1 (a
  //! hand-written single-node ResolvedLayout cannot — spec §1). The harness is
  //! C7-owned; do not recreate the injection machinery here.
  use bevy::prelude::*;
  use buiy_core::{Node, layout::Style};
  use buiy_verify::pointer::PointerHarness;

  #[test]
  fn pointer_over_offset_buiy_node_emits_hit() {
      let mut h = PointerHarness::new();
      // Target placed at window offset (70,90): absolute box (70,90)..(170,140).
      let target = h.spawn_offset_tree(
          Vec2::new(70.0, 90.0),
          (Node, Style::default().width_px(100.0).height_px(50.0)),
      );
      // Aim the synthetic pointer at the target's GLOBAL center; the backend must
      // emit a hit for it. On pre-C1 code the origin-anchored rect is at
      // (0,0)..(100,50), the global center is outside it, and no hit fires.
      let center = h.global_center(target);
      h.move_to(center);
      assert_eq!(
          h.top_hit(),
          Some(target),
          "the backend must emit a hit for the OFFSET target at its absolute box \
           (Bug-1 regression; the harness drives the real layout→bridge chain)"
      );
  }
  ```
  > The harness's `move_to` runs the update so the backend re-emits `PointerHits`, and `top_hit` reads the top-most pick (the contract API). Reading `Messages<PointerHits>` directly is NOT needed here — `top_hit()` is the harness's purpose-built accessor and is simpler than re-deriving the cursor read. (The one-frame `PreUpdate`/`Update` staleness D4 documents is absorbed inside the harness's bounded settle + `move_to` update.)
- [ ] **Run it — expect GREEN:**
  ```sh
  cargo test -p buiy_core --test picking_backend 2>&1 | tail -8
  ```
  Expected: `test result: ok. 1 passed`.
- [ ] **Commit:** `test(picking): un-ignore C7 offset gate + re-home picking tests onto PointerHarness (C1)`

---

## Task 5: Clip coordinate basis — `walk` reads `GlobalTransform`; add the OFFSET overflow-clip test (RED-first)

Make `write_clip_rects`'s `walk` compute the own box from the absolute position, and add the new nested + **offset** overflow-clip test that the audit (§1 MISSED #1) says is missing — none today offsets a clipper (spec §2.2, §5 steps 3–4, §6). `ResolvedLayout` stays **optional** in `ClipNodeData` (for the box-less "clear stale clip" arm, clip.rs:294-297); the *position read* is now gated on `GlobalTransform` being present (D2).

**Files**
- Modify: `crates/buiy_core/src/render/clip.rs` (`ClipNodeData` adds `Option<&GlobalTransform>`; the `walk` match arm reads absolute pos)
- Test: `crates/buiy_core/tests/render_clip_rects.rs` (add the offset-clip test)

**Steps**
- [ ] **Write the failing offset-clip test FIRST.** Append to `crates/buiy_core/tests/render_clip_rects.rs` (the `Style`, `Node`, `AncestorClip`, `ClipRect`, `OverflowMode` imports are already present at the top of the file):
  ```rust
  #[test]
  fn offset_clipper_clips_in_absolute_space() {
      // The C1-specific gate (audit §1 MISSED #1): no existing clip test offsets
      // a clipper. Outer is translated (70,90); the clipper is its child at
      // parent-local (0,0), so its ABSOLUTE box is (70,90)..(170,190). A child
      // overflowing it must be clipped to that ABSOLUTE box. Pre-C1 (own box from
      // ResolvedLayout.position) the clip is wrongly anchored at the origin
      // (0,0)..(100,100); post-C1 it is the absolute (70,90)..(170,190).
      let mut app = app();
      let outer = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(400.0).height_px(400.0).translate_px(70.0, 90.0),
          ))
          .id();
      let clipper = app
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
          .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
          .id();
      app.world_mut().entity_mut(outer).add_child(clipper);
      app.world_mut().entity_mut(clipper).add_child(child);
      app.update();
      app.update();

      let anc = *app
          .world()
          .get::<AncestorClip>(child)
          .expect("child clipped by the offset clipper");
      assert_eq!(
          anc.min,
          Vec2::new(70.0, 90.0),
          "ancestor clip min = clipper ABSOLUTE top-left, not the origin"
      );
      assert_eq!(
          anc.max,
          Vec2::new(170.0, 190.0),
          "ancestor clip max = clipper ABSOLUTE bottom-right"
      );
      let clip = *app.world().get::<ClipRect>(child).expect("child has clip rect");
      assert_eq!(clip.min, Vec2::new(70.0, 90.0), "ClipRect min = absolute");
      assert_eq!(clip.max, Vec2::new(170.0, 190.0), "ClipRect max = absolute");
  }
  ```
- [ ] **Run it — expect RED:**
  ```sh
  cargo test -p buiy_core --test render_clip_rects offset_clipper_clips_in_absolute_space 2>&1 | tail -15
  ```
  Expected (pre-fix): FAIL — `assertion failed: anc.min == (70,90)` with `left: Vec2(0.0, 0.0)` (the clipper's `ResolvedLayout.position` is `(0,0)`, so the old code anchored the clip at the origin). This is the latent Bug-1 instance firing for the first time.
- [ ] **Add `GlobalTransform` to `ClipNodeData`.** Replace `clip.rs:215-221`:
  ```rust
  type ClipNodeData<'w> = (
      Option<&'w ResolvedLayout>,
      Option<&'w BoxModel>,
      Option<&'w Overflow>,
      Option<&'w Containment>,
      Option<&'w Display>,
      Option<&'w GlobalTransform>,
  );
  ```
  (`GlobalTransform` is in `bevy::prelude::*`, already imported at `clip.rs:26`.)
- [ ] **Destructure the new term in `walk`.** Replace the `let Ok(...) = nodes.get(entity)` line (`clip.rs:268`):
  ```rust
      let Ok((rl, box_model, overflow, containment, display, gt)) = nodes.get(entity) else {
          return;
      };
  ```
- [ ] **Read the absolute position in the `Some(rl)` arm.** Replace the match arm (`clip.rs:284-293`):
  ```rust
      let child_ancestor = match (rl, gt) {
          (Some(rl), Some(gt)) => {
              let abs = gt.translation().truncate(); // C1: absolute basis, not rl.position
              let own = Aabb::from_box(abs, rl.size);
              let clip = ancestor.map(|a| a.intersect(own));
              reconcile(entity, clip, ancestor, commands, existing);
              // The clip box THIS node imposes on its descendants, folded into
              // the running ancestor AABB.
              let contribution = clip_contribution(own, box_model, overflow, containment);
              intersect_opt(ancestor, contribution)
          }
          // No resolved box OR no GlobalTransform (never bridged): cannot be
          // clipped or contribute a clip — clear any stale own clip, keep walking
          // descendants with the unchanged ancestor (D2: no fallback to rl.position).
          _ => {
              reconcile(entity, None, None, commands, existing);
              ancestor
          }
      };
  ```
- [ ] **Run the offset test — expect GREEN:**
  ```sh
  cargo test -p buiy_core --test render_clip_rects offset_clipper_clips_in_absolute_space 2>&1 | tail -8
  ```
  Expected: `test result: ok. 1 passed` — `anc` is now `(70,90)..(170,190)`.
- [ ] **Run the WHOLE clip suite — expect every existing test still GREEN** (all existing clippers sit at the window origin, so `GlobalTransform`-derived == `ResolvedLayout.position` there; spec §5 step 3):
  ```sh
  cargo test -p buiy_core --test render_clip_rects 2>&1 | tail -8
  ```
  Expected: all pass, including `child_of_overflow_hidden_is_clipped_to_parent_padding_box`, `nested_overflow_hidden_intersects_to_tighter_box`, `scroll_container_clips_to_viewport_independent_of_offset`. **If `scroll_container_clips_to_viewport_independent_of_offset` regresses:** that is the §A.4 invariant — the scroll container's OWN `GlobalTransform` is unaffected by its OWN `ScrollOffset` (scroll only shifts descendants via `acc` in `bridge.rs:161`), so its clip box must stay constant; a failure means an ordering bug, addressed in Task 6.
- [ ] **Commit:** `feat(render): write_clip_rects computes own box in absolute space (C1)`

---

## Task 6: Clip-after-bridge scheduling edge + the not-stale ordering test

`write_clip_rects` now reads `GlobalTransform`, so it MUST run **after** the propagation chain (`sync_simple_transforms`, the last propagation system). Today there is no ordering edge between them — harmless while clip read `ResolvedLayout`, but now a correctness bug (spec §2.2 "load-bearing new constraint", D3, §4.2). Add `.after(sync_simple_transforms)` and a regression test proving clip reads the post-bridge transform.

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs:119-124` (add the `.after` edge to the `write_clip_rects` registration)
- Test: `crates/buiy_core/tests/render_clip_rects.rs` (ordering / not-stale test)

**Steps**
- [ ] **Write the failing ordering test FIRST.** This asserts clip reflects an ancestor scroll *after* one frame (clip is not one-frame-stale relative to the bridge). A scroll-offset ancestor shifts a deeper clipper's absolute position via the bridge; clip must see the post-bridge value in the same frame. Append to `crates/buiy_core/tests/render_clip_rects.rs`:
  ```rust
  #[test]
  fn clip_reflects_post_bridge_transform_same_frame() {
      // A scroll container scrolls its content; a CLIPPER nested inside the
      // scrolled content moves in absolute space by the scroll delta. Clip must
      // read the POST-bridge GlobalTransform, so the deeper child's clip tracks
      // the clipper's new absolute position within the same settled frame —
      // proving write_clip_rects runs .after(sync_simple_transforms) (D3).
      let mut app = app();
      let scroller = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width_px(400.0)
                  .height_px(400.0)
                  .overflow(OverflowMode::Scroll, OverflowMode::Scroll),
              ScrollOffset::default(),
          ))
          .id();
      let clipper = app
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
          .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
          .id();
      app.world_mut().entity_mut(scroller).add_child(clipper);
      app.world_mut().entity_mut(clipper).add_child(child);
      app.update();
      app.update();

      // Before scroll: clipper absolute top-left = (0,0); child clip = (0,0)..(100,100).
      let before = *app.world().get::<AncestorClip>(child).expect("clipped");
      assert_eq!(before.min, Vec2::ZERO);
      assert_eq!(before.max, Vec2::new(100.0, 100.0));

      // Scroll the OUTER container by (0,-50): content (incl. clipper) shifts up
      // by 50px in absolute space => clipper absolute top-left = (0,-50).
      {
          let mut off = app.world_mut().get_mut::<ScrollOffset>(scroller).unwrap();
          off.y = 50.0; // positive scroll offset moves content up (acc add, bridge.rs:161)
      }
      app.update();

      let after = *app.world().get::<AncestorClip>(child).expect("still clipped");
      assert_eq!(
          after.min,
          Vec2::new(0.0, -50.0),
          "clip tracks the clipper's POST-bridge absolute position (not stale)"
      );
      assert_eq!(after.max, Vec2::new(100.0, 50.0), "clip max shifted by the scroll delta");
  }
  ```
  > Re-confirm at Phase 0: the sign of the scroll fold is `child_acc = acc + (off.x, off.y)` and `base = position − acc` (`bridge.rs:138,161`), so a positive `off.y` subtracts from the descendant translation (content moves up). If the rebased bridge changes the sign convention, flip the expected `(0,-50)`/`(100,50)` accordingly — re-derive with the Phase-0 probe.
- [ ] **Run it — expect it may already PASS or RACE.** Without the explicit edge, `write_clip_rects` and the bridge chain are unordered within `.after(Animate).before(Picking)`; the result is nondeterministic. Run it a few times:
  ```sh
  for i in 1 2 3 4 5; do cargo test -p buiy_core --test render_clip_rects clip_reflects_post_bridge_transform_same_frame 2>&1 | grep "test result"; done
  ```
  Expected: at least one FAIL (or flake) demonstrating the missing edge. If it passes deterministically by scheduler luck, the test still becomes the *regression guard* once the edge is added — note that and proceed (the edge is correct and required regardless; D3).
- [ ] **Add the scheduling edge.** In `crates/buiy_core/src/render/mod.rs`, replace the `write_clip_rects` registration (`render/mod.rs:119-124`):
  ```rust
          // Main-world render-prep: clip computation reads GlobalTransform (C1),
          // so it MUST run AFTER the bridge's propagation chain (sync_simple_transforms
          // is its last system, lib.rs) — and before Picking, so picking + extract
          // see settled ClipRects (architecture.md § 5.2). Runs on CI/headless —
          // no RenderApp required.
          app.add_systems(
              Update,
              clip::write_clip_rects
                  .after(bevy::transform::systems::sync_simple_transforms)
                  .after(crate::BuiySet::Animate)
                  .before(crate::BuiySet::Picking),
          );
  ```
  (`sync_simple_transforms` is reachable as `bevy::transform::systems::sync_simple_transforms`, the same path `lib.rs:7-9` imports. No re-export needed.)
- [ ] **Run the ordering test — expect deterministic GREEN:**
  ```sh
  for i in 1 2 3 4 5; do cargo test -p buiy_core --test render_clip_rects clip_reflects_post_bridge_transform_same_frame 2>&1 | grep "test result"; done
  ```
  Expected: all five `ok. 1 passed`.
- [ ] **Run the whole clip suite again — still GREEN:**
  ```sh
  cargo test -p buiy_core --test render_clip_rects 2>&1 | tail -8
  ```
  Expected: all pass.
- [ ] **Commit:** `fix(render): write_clip_rects runs after the transform propagation chain (C1)`

---

## Task 7: No-fallback assertion (pin D2 against silent reintroduction)

Pin the decision that a node with `ResolvedLayout` but **no** `GlobalTransform` (never bridged) is **dropped** from picking/clip — not silently placed at `ResolvedLayout.position` (spec §6 "No-fallback assertion", D2). This guards against a future change quietly re-adding the `unwrap_or(layout.position)` foot-gun.

**Files**
- Test: `crates/buiy_core/tests/picking.rs` (add the no-fallback case) and `crates/buiy_core/tests/render_clip_rects.rs` (add the clip no-fallback case)

**Steps**
- [ ] **Add the picking no-fallback test.** Append to `crates/buiy_core/tests/picking.rs`. It reuses the C7-owned `PointerHarness` (already imported at the top of the re-homed file) only to obtain a world whose picking query components are registered, then spawns a *detached, hand-written* node so the bridge never produces a `GlobalTransform` for it:
  ```rust
  #[test]
  fn node_without_global_transform_is_not_picked() {
      // A node carrying ResolvedLayout but NO GlobalTransform (hand-spawned,
      // detached, never bridged) must be ABSENT from hit_test — not silently
      // placed at ResolvedLayout.position (D2: no unwrap_or fallback).
      let mut h = PointerHarness::new();
      let bare = h
          .world_mut()
          .spawn((
              Node,
              ResolvedLayout { position: Vec2::new(10.0, 10.0), size: Vec2::new(100.0, 50.0) },
          ))
          .id();
      // Detached + layout never runs for it; strip any Transform/GlobalTransform
      // the require-graph or a prior frame may have inserted so it is genuinely
      // un-bridged. Confirm absence immediately before the hit_test call.
      h.world_mut().entity_mut(bare).remove::<GlobalTransform>();
      assert!(
          h.world_mut().get::<GlobalTransform>(bare).is_none(),
          "the bare node has no GlobalTransform before hit_test"
      );
      // A point inside the hand-set ResolvedLayout box must NOT hit — the node
      // has no GlobalTransform, so it is dropped from the query.
      assert_eq!(hit_test(h.world_mut(), Vec2::new(50.0, 30.0)), None);
  }
  ```
  > Re-confirm at Phase 0: `Node`'s `#[require(...)]` graph (`components.rs:42-57`) does **not** list `Transform`/`GlobalTransform`, so a hand-spawned `Node` has neither until the bridge inserts `Transform`. The explicit `.remove::<GlobalTransform>()` is belt-and-suspenders against a require-graph change; the assertion immediately before the `hit_test` call pins that the node is genuinely un-bridged (the harness's prior settle does not re-bridge a detached node, since layout never runs for it). Both `PointerHarness`, `ResolvedLayout`, and `hit_test` are already imported by the re-homed `picking.rs` (Task 4); `ResolvedLayout` is re-exported from the crate root.
- [ ] **Run it — expect GREEN** (and prove its teeth by temporarily adding an `unwrap_or` fallback to `hit_test`, confirming it then FAILS, then removing the fallback):
  ```sh
  cargo test -p buiy_core --test picking node_without_global_transform_is_not_picked 2>&1 | tail -6
  ```
  Expected: `ok. 1 passed`.
- [ ] **Add the clip no-fallback test.** Append to `crates/buiy_core/tests/render_clip_rects.rs`:
  ```rust
  #[test]
  fn clipper_without_global_transform_contributes_no_clip() {
      // A clipper Node with ResolvedLayout but NO GlobalTransform contributes no
      // clip (the `_ =>` arm clears, does not fall back to rl.position — D2). Its
      // child therefore has NO AncestorClip.
      let mut app = app();
      let clipper = app
          .world_mut()
          .spawn((
              Node,
              ResolvedLayout { position: Vec2::new(10.0, 10.0), size: Vec2::new(100.0, 100.0) },
              Style::default()
                  .width_px(100.0)
                  .height_px(100.0)
                  .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
          ))
          .id();
      let child = app
          .world_mut()
          .spawn((Node, ResolvedLayout { position: Vec2::ZERO, size: Vec2::new(300.0, 300.0) }))
          .id();
      app.world_mut().entity_mut(clipper).add_child(child);
      // Strip any GlobalTransform so the clipper is genuinely un-bridged.
      app.world_mut().entity_mut(clipper).remove::<GlobalTransform>();
      app.update();
      assert!(
          app.world().get::<AncestorClip>(child).is_none(),
          "an un-bridged clipper contributes no clip (no fallback to ResolvedLayout.position)"
      );
  }
  ```
  > Note: this test spawns `ResolvedLayout` by hand (it is testing the *absence* of `GlobalTransform`, so it must bypass layout). It does not contradict the "no hand-written ResolvedLayout" guidance — that guidance is about *positive* hit tests being blind to Bug 1; this is a *negative* test of the drop behavior. Re-confirm at Phase 0 that `app.update()` does not re-bridge a detached clipper (layout does not run for nodes outside a laid-out root; if `BuiyRenderPlugin`'s bridge inserts a `Transform` here, assert `GlobalTransform` absence immediately before `update()` and accept the one-frame check).
- [ ] **Run it — expect GREEN:**
  ```sh
  cargo test -p buiy_core --test render_clip_rects clipper_without_global_transform_contributes_no_clip 2>&1 | tail -6
  ```
  Expected: `ok. 1 passed`.
- [ ] **Commit:** `test(picking,render): pin no-fallback drop for un-bridged nodes (C1 D2)`

---

## Task 8: Snapshot-stability + supersede-note + full-gate verification

Confirm the §5 "no snapshots touched" claim (C1 changes no `ResolvedLayout` value, no paint, no `PackedInstance` byte layout — umbrella §6.7 untouched), annotate the superseded render-pipeline spec prose, and run the full project gate.

**Files**
- Modify: `docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md` (§A.2 supersede note)
- Verify only: layout + clip + golden suites (no edits)

**Steps**
- [ ] **Run the existing layout + clip + display-list suites unchanged — expect ZERO golden re-bless** (spec §6 "Snapshot stability check"; any diff means a value moved and the layout-local invariant was violated):
  ```sh
  cargo test -p buiy_core 2>&1 | tail -15
  cargo test -p buiy_verify 2>&1 | tail -15
  ```
  Expected: all PASS with **no** `.snap` / golden diffs. If `cargo insta` or a golden harness reports a pending snapshot, STOP — C1 must not change any snapshot; investigate the regression rather than blessing.
- [ ] **Add the supersede note to the render-pipeline spec.** In `docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md`, find the §A.2 prose that describes the producer reading `Rect::from(resolved.position, resolved.size)` (grep: `grep -n "resolved.position" docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md`). **Do not delete it** — add an italic note immediately after it (spec §4.3):
  ```markdown
  > **Superseded by C1 (2026-06-22, widget-catalog campaign):** the producer now
  > computes the own box from `GlobalTransform.translation().truncate()`, not
  > `ResolvedLayout.position`, consistent with pillar 5 § B.5 (render/picking read
  > `GlobalTransform`). `ResolvedLayout.position` is parent-local; the absolute
  > basis is `GlobalTransform`. See
  > docs/specs/2026-06-22-buiy-widget-catalog-design/coordinate-space-correctness.md.
  ```
- [ ] **Grep-confirm no new `ResolvedLayout.position`-as-absolute read crept in** (spec §5 step 5; outline ink-bounds + overlays must stay `GlobalTransform`-sourced):
  ```sh
  grep -rn "\.position" crates/buiy_core/src/picking/ crates/buiy_core/src/render/clip.rs
  ```
  Expected: the only `ResolvedLayout.position` reads remaining are in `bridge.rs` (the accumulator) and layout internals — none in `picking/` or `clip.rs`'s own-box computation.
- [ ] **Run the full project gate** (CLAUDE.md Build & Test; on this Linux host use the `xvfb-run -a` prefix; if the test step link-OOMs add `-j 2`):
  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace -j 2
  ```
  Expected: all green. The GPU `--ignored` lane is **not** required for C1 (it changes no GPU path; spec §5 "Snapshots/goldens: none"), but if a GPU host is available, `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` must still pass unchanged.
- [ ] **Commit:** `docs(render): annotate clip-and-transform §A.2 superseded by C1 absolute-basis (C1)`

---

## Done criteria (C1 acceptance)

- [ ] `point_in_aabb` is `(point, abs_pos, size)`; `hit_test` + `emit_picks` query `&GlobalTransform` non-optionally; `write_clip_rects`'s `walk` reads the absolute own box; `components.rs:65` doc is parent-local.
- [ ] `write_clip_rects` is scheduled `.after(sync_simple_transforms).before(BuiySet::Picking)`.
- [ ] `bridge.rs:138` `base = position − acc` is **unchanged** (verified by grep in Task 0 + Task 8).
- [ ] C7's offset RED proof (`synthetic_pointer_hits_offset_widget_at_its_global_position` in `crates/buiy_verify/tests/pointer_offset_regression.rs`) is **un-ignored** (its `#[ignore]` deleted) and GREEN — the committed RED→GREEN transition is C1's coordinate-fix proof. C1 did **not** create the harness, the test, or any inline `laid_out_app`; the two unit tests `picking.rs` / `picking_backend.rs` are re-homed onto the **C7-owned `PointerHarness`** (`spawn_offset_tree(offset, scene)` / `top_hit` / `global_center` / `world_mut`).
- [ ] C1's own buiy_core RED-first tests (harness does NOT cover): the **offset overflow-clip** test, the **not-stale ordering** test, and the two **no-fallback** tests are present and GREEN — each written RED pre-fix then GREEN post-fix within its own task.
- [ ] **RED-first proven** for: offset picking (C7's un-ignored offset test was committed RED, goes GREEN on C1's fix), offset clip (`offset_clipper_clips_in_absolute_space` RED pre-fix → GREEN), no-fallback (add an `unwrap_or` fallback → RED → remove). No manual hand-revert demonstration is used for anything the harness covers.
- [ ] **Zero** snapshot/golden re-bless; full workspace gate green.
