# Phase 14: Multi-level descendant invalidation on ancestor-resolved-size change Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Close the multi-level container-query cascade gap. Today, when a query container `A`'s `ResolvedLayout` changes, a `Cqw`-sized intermediate `B` between `A` and a rule-bearing descendant `C` is never re-translated, so `B`'s Taffy width stays at the previously-baked `Cqw` resolution and `C`'s `ContainerQuery` never re-evaluates. Phase 14 adds a step-8 descendant-invalidation pass (`cq_descendant_invalidate`) that, after `write_resolved_layout` (step 7), finds query containers whose `ResolvedLayout` changed this frame, walks their descendants, and marks them dirty into a `ContainerSizeDirty(HashSet<Entity>)` resource; a step-9 same-frame re-run (`cq_descendant_rerun`, analogous to `cq_flip_rerun`) re-translates the dirty descendants, recomputes Taffy, re-writes `ResolvedLayout`, and re-runs container-query activation so `C` flips **in the same frame** (`A`→`B`→`C` cascade settles in one frame instead of never).

**Architecture (3 sentences):**
1. **Geometric-cascade invalidation as a post-step-7 overlay + bounded same-frame re-run.** `cq_descendant_invalidate` (step 8) runs after `write_resolved_layout`, reads `Changed<ResolvedLayout>` on query containers (`Container { container_type != Normal }`), walks each changed container's `Children` subtree, and inserts the visited descendants into a `ContainerSizeDirty` `HashSet<Entity>` resource (setting `CqDescendantReRunRequested(true)` when non-empty); `cq_descendant_rerun` (step 9) is gated on that flag and re-runs the inner work of `sync_styles` + `taffy_compute` for exactly the dirty set, then re-writes `ResolvedLayout`, then re-runs `cq_activate` + `cq_flip_check` so a rule-bearing descendant flips its `ContainerQueryActive` / `ContainerQueryInactive` marker the same frame.
2. **The dirty-resource mechanism (follow-ups.md option b), not a `Changed`-filter marker (option a).** A private `ContainerSizeDirty(HashSet<Entity>)` resource is the cross-pass hand-off; the re-run iterates that set directly (like `cq_flip_rerun` iterates its `Changed`-filtered set), so the descendants re-resolve their `Cqw` against the new ancestor sizes **within the same frame** — a `sync_styles`-filter marker would only catch up next frame (it relies on the next-frame `Changed<ResolvedLayout>` cascade that already half-works), which does not satisfy spec § 1.5's same-frame reading.
3. **Bounded, loop-safe, single-re-run-per-frame.** The descendant walk is O(subtree) per changed container (the same worst case as Phase-4 writing-mode propagation, per follow-ups.md); the re-run is capped at **once** per frame (`CqDescendantReRunRequested` is cleared at the top of the re-run, exactly mirroring `cq_flip_rerun`'s `CqReRunRequested` discipline), so a deeper `A`→`B`→`C`→`D` chain settles one level per frame — matching the spec § 1.5 "frame N applies A's activation, frame N+1 applies B's" eventual-consistency contract while making the **direct** intermediate case (the `A`→`B`→`C` fixture) same-frame.

**Tech Stack:** Bevy 0.18 (`bevy::prelude::{Children, ChildOf, Node, Query, Commands, Res, ResMut, NonSend, NonSendMut, With, Changed, Or, Entity}`, `bevy::math::Vec2`). `std::collections::{HashSet, HashMap}` (no `bevy::utils::*`, per Phase 6/7/8/9 precedent). No new external dependency. Reuses the Phase-5 `translate_one_entity` per-entity translation helper, the `ContainerSnapshot` index, and the `cq_activate` / `cq_flip_check` systems.

**Date:** 2026-05-29
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md`](../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md) § 1.3 (same-frame re-layout / `CqFlipReRun` analogue), § 1.5 (the `A`→`B`→`C` transitive-cascade test surface) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (system pipeline). Chartered by [`plans/follow-ups.md`](follow-ups.md) "Descendant invalidation on ancestor-resolved-size changes".

---

## Prior-art citations (used throughout this plan)

- **Blink — bounded same-frame re-layout is the loop-breaker** — `docs/prior-art/blink/containment-and-queries.md` § 3 / § 4: "Buiy's container-query pass handles the loop directly via its **same-frame re-layout** strategy: step 2 (`CqActivate`) evaluates rules against the *previous* frame's `ResolvedLayout`, step 4 (`CqFlipCheck`) re-checks against the current frame and triggers at most one re-run … The 2×-Taffy cost ceiling is Buiy's loop-breaker." Phase 14 extends that bounded-re-run discipline to the geometric (size) cascade: the descendant re-run is likewise capped at **once per frame**, so the multi-level cascade cannot oscillate. Blink solves the same loop structurally (mandatory `size`/`layout` containment on a query container); Buiy's is explicit and bounded.
- **Servo/Stylo — ancestor-driven invalidation is a known cost, walked from the changed node down** — `docs/prior-art/servo-stylo/stylo.md` § 4 (bloom-filter ancestor matching) frames "does any ancestor's state affect this descendant?" as the central restyle question; Stylo answers it top-down from the changed subtree root. Phase 14's walk is the same shape: start at the container whose `ResolvedLayout` changed, descend, mark. Buiy has no descendant combinators so it needs no bloom filter — a plain `Children` walk per changed container is sufficient and is exactly what follow-ups.md prescribes.
- **`cq_flip_rerun` is the re-run template** — `crates/buiy_core/src/layout/systems.rs:3075-3225` (`cq_flip_rerun`): a normal Bevy system holding the union of `sync_styles` + `taffy_compute` params, gated on `CqReRunRequested.0`, clearing the flag at the top, rebuilding the `parent_areas_for` / `container_index` / `viewport_size` snapshots, calling `translate_one_entity` per changed entity, then `sync_children_pass`, then per-root `compute_layout` (bumping `LayoutTaffyComputeCount`). Phase 14's `cq_descendant_rerun` (T6) is structurally identical but iterates the `ContainerSizeDirty` set instead of a `Changed`-filtered query, and additionally re-writes `ResolvedLayout` + re-runs `cq_activate`/`cq_flip_check` (T7) so the marker flip is visible the same frame.
- **`translate_one_entity` — the per-entity translation sharing point** — `crates/buiy_core/src/layout/systems.rs:2246-2314`. Signature: `translate_one_entity(item: NodeQueryItem<'_>, parent_areas_for: &HashMap<Entity, GridAreas>, container_index: &HashMap<Entity, ContainerSnapshot>, cq_parent_chain: &Query<&ChildOf>, viewport_size: Vec2, content_visibility_intrinsic: Option<Vec2>, tree: &mut LayoutTree)`. `NodeQueryItem<'w>` is the 14-tuple at `systems.rs:2214-2229`. Phase 14's re-run calls this for each dirty descendant (passing `None` for the content-visibility sentinel, exactly as `cq_flip_rerun` does at `systems.rs:3174`).
- **`ContainerSnapshot` index + `nearest_container_with_size`** — `crates/buiy_core/src/layout/systems.rs:2057-2072` (the `container_index: HashMap<Entity, ContainerSnapshot>` built from `Query<(Entity, &Container, &ResolvedLayout)>`, skipping `ContainerType::Normal`). After a descendant's ancestor `Cqw` size changed, the snapshot built from the freshly-written `ResolvedLayout` is what `translate_one_entity` resolves `Length::Cq*` against. The re-run rebuilds this index from the current (just-written) `ResolvedLayout`, so the dirty descendant sees the new ancestor size.
- **Pipeline step set + `.chain()` wiring** — `crates/buiy_core/src/layout/pipeline.rs:16-64` (`BuiyLayoutStep` enum + `configure_pipeline`'s `.chain().in_set(BuiySet::Layout)`); `crates/buiy_core/src/layout/mod.rs:194-238` (`app.add_systems(Update, (… ).in_set(BuiyLayoutStep::X))`). Phase 14 adds **two** new variants `CqDescendantInvalidate` and `CqDescendantReRun` to the enum, **after** `WriteResolvedLayout`, and wires the two systems into them. (The current last step is `WriteResolvedLayout`; Phase 14 appends two steps after it.)
- **`CqReRunRequested` flag + `LayoutTaffyComputeCount` instrument** — `crates/buiy_core/src/layout/systems.rs:46` (`pub struct CqReRunRequested(pub bool)`), `:59` (`pub struct LayoutTaffyComputeCount(pub u32)`), wired in `mod.rs:53-54`. Phase 14 adds a parallel `CqDescendantReRunRequested(pub bool)` flag (same shape) and reuses `LayoutTaffyComputeCount` for the descendant re-run's Taffy bump so the "cap at 2× Taffy per frame" invariant stays observable.
- **`Changed<ResolvedLayout>` is the cascade signal** — `crates/buiy_core/src/layout/systems.rs:1958` (the comment block at `:1940-1958` in `sync_styles` documents that an ancestor's resolved-size change surfaces as `Changed<ResolvedLayout>` on the entity whose size shifted). Phase 14 reads `Changed<ResolvedLayout>` on the **container** entity (`A`) directly — the thing that actually changed — and walks **down** to find `B`/`C` (which see no `Changed` bit of their own; that is the gap).
- **`resolve_nearest_container` / `resolve_nearest_container_by_name` + `evaluate_conditions`** — `crates/buiy_core/src/layout/systems.rs:2889-2960` (ancestor walks) + `:2733` (`evaluate_conditions`). Phase 14's `cq_descendant_rerun` re-invokes `cq_activate` (`systems.rs:2800`) and `cq_flip_check` (`systems.rs:2986`) as **whole systems** via `World::run_system_once` is NOT used (systems are not registered one-shot); instead the re-run system holds the union of their params and calls the same pure pieces. (See D5 for why the re-run re-evaluates rules inline rather than depending on the next frame's `cq_activate`.)
- **The existing negative-assertion regression test** — `crates/buiy_core/tests/layout_container_queries.rs:219-314` (`cq_transitive_cascade_is_one_frame_stale`). It asserts `C` stays `ContainerQueryInactive` after `A` is widened 700→1000. Phase 14 **flips its polarity to positive** (renamed `cq_transitive_cascade_catches_up_in_frame`, asserting `C` becomes `ContainerQueryActive` the same frame `A` resizes) — T8 does the rename + flip, NOT a deletion.
- **Test harness** — `crates/buiy_core/tests/layout_container_queries.rs:14-18`: `fn app() { let mut app = App::new(); app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin); app }` (no render; `MinimalPlugins` runs headless). Spawn `(Node, Style::default()…)`, wire hierarchy with `app.world_mut().entity_mut(parent).add_children(&[child])`, `app.update()` once per frame, assert via `app.world().get::<ContainerQueryActive>(c)` / `app.world().get::<ResolvedLayout>(e)`.
- **Resource init block** — `crates/buiy_core/src/layout/mod.rs:53-82` (`app.init_resource::<systems::CqReRunRequested>()` … block). Phase 14 adds `app.init_resource::<systems::ContainerSizeDirty>()` and `app.init_resource::<systems::CqDescendantReRunRequested>()` here.

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/systems.rs` | T1 (`ContainerSizeDirty` + `CqDescendantReRunRequested` resources), T2 (`collect_dirty_descendants` pure helper), T3 (`cq_descendant_invalidate` system step 8), T4 (`cq_descendant_rerun` skeleton — gate + clear), T5 (re-run translation body), T6 (re-run Taffy + `write_resolved_layout` re-invoke), T7 (re-run `cq_activate` + `cq_flip_check` re-invoke) |
| `crates/buiy_core/src/layout/pipeline.rs` | T3 (`CqDescendantInvalidate` step variant + chain), T4 (`CqDescendantReRun` step variant + chain) |
| `crates/buiy_core/src/layout/mod.rs` | T3 (wire `cq_descendant_invalidate`; `init_resource::<ContainerSizeDirty>`), T4 (wire `cq_descendant_rerun`; `init_resource::<CqDescendantReRunRequested>`) |
| `crates/buiy_core/tests/layout_pipeline_order.rs` | T3 (assert step 8 runs after step 7) |
| `crates/buiy_core/tests/layout_container_queries.rs` | T8 (flip the `cq_transitive_cascade_is_one_frame_stale` negative assertion to positive `cq_transitive_cascade_catches_up_in_frame`) |

No changes to: `crates/buiy_core/src/layout/components.rs`, `crates/buiy_core/src/layout/types.rs` (no new component or value type — the dirty hand-off is a private resource, not author surface), `crates/buiy_core/src/layout/translate.rs`, `crates/buiy_core/src/layout/tree.rs`, `crates/buiy_core/src/components.rs`, `crates/buiy_core/src/lib.rs`, `crates/buiy/src/lib.rs` (the resources are crate-internal — `pub use systems::{…}` is NOT widened because tests reach them via the public systems' observable effects, mirroring how `CqReRunRequested` is not re-exported).

**Docs flips (spec § 1.5 confirmation, README entry, follow-ups closeout) happen in a separate stage — NO task in this plan edits `docs/`.**

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. Mechanism = `ContainerSizeDirty` `HashSet<Entity>` resource + dedicated same-frame re-run (follow-ups.md option b), NOT a `sync_styles`-filter marker (option a)

**Decision:** The cross-pass hand-off is a private `ContainerSizeDirty(pub HashSet<Entity>)` resource populated by step 8 and consumed by a dedicated step-9 re-run system that iterates the set directly. A `ContainerSizeDirty` **marker component** picked up by `sync_styles`'s `Or<(Changed<…>, …)>` filter is rejected.

**Why:** The spec § 1.5 `A`→`B`→`C` fixture's natural reading (and the follow-ups.md charter) is that the **direct** intermediate (`B`) catches up so `C` re-evaluates promptly. A `sync_styles`-filter marker fires on the **next** frame's `sync_styles` (the marker becomes a `Changed` bit observable next tick), which is the same multi-frame lag the existing `Changed<ResolvedLayout>` cascade already half-provides — it does not deliver the same-frame settle the spec's `CqFlipReRun` model (§ 1.3) establishes for the direct-ancestor case. A dedicated re-run (the `cq_flip_rerun` template) re-translates the dirty descendants **this frame**, so `B`'s `Cqw` re-resolves against `A`'s new size, Taffy recomputes, and `C`'s rule re-evaluates before the frame ends. The dirty resource is also cheaper to reason about: it is empty in steady state (no allocation churn), cleared each frame, and never leaves a stale `Changed` bit on a component.

**How to apply:** T1 defines `ContainerSizeDirty` + `CqDescendantReRunRequested`. T3 populates the set; T4-T7 build the re-run that drains it.

**Runner-up rejected:** Private `ContainerSizeDirty` **marker component** + widen the `sync_styles` `Or<>` filter. Rejected: (a) it is next-frame, not same-frame (fails the spec § 1.5 reading); (b) the `Or<>` tuple in `sync_styles` is already at its nested-15 cap (`systems.rs:1982-1994`), so adding `Changed<ContainerSizeDirty>` would force another nesting level for a worse outcome; (c) a marker that must be inserted-then-removed every frame adds `Commands` churn and a removal pass.

### D2. Step 8 reads `Changed<ResolvedLayout>` on the **container** and walks **down**; it does not try to read "ancestor changed" on the descendant

**Decision:** `cq_descendant_invalidate` queries `Query<(Entity, Option<&Children>), (With<Container>, Changed<ResolvedLayout>)>`-style — i.e. it iterates the entities that actually changed (the query containers) and walks their `Children` subtree to enumerate descendants. It does **not** attempt any per-descendant "did my ancestor change?" test (Bevy ships no such filter — the documented root cause in follow-ups.md).

**Why:** This is the only sound primitive: Bevy's `Changed<T>` is per-entity, and the cascade gap is precisely that `B`/`C` have no `Changed` bit when only `A`'s `ResolvedLayout` changed (follow-ups.md "Cause"). Reading `Changed<ResolvedLayout>` on `A` (which *does* fire) and walking down to `B`/`C` inverts the unanswerable "ancestor changed?" question into the answerable "I changed — who are my descendants?" question (the Stylo top-down-from-changed-subtree shape).

**How to apply:** T2's pure helper `collect_dirty_descendants` takes the changed-container entities + a `Query<&Children>` and returns the flattened descendant set. T3's system passes the `Changed<ResolvedLayout>`-filtered container entities into it.

**Runner-up rejected:** A `DescendantOf<Changed<T>>` query filter. Rejected: Bevy 0.18 does not ship it (follow-ups.md confirms); writing one is a framework-level change far outside this phase.

### D3. Only **query containers** (`Container { container_type != Normal }`) seed the dirty walk — not every changed `ResolvedLayout`

**Decision:** Step 8 filters the `Changed<ResolvedLayout>` set to entities that are query containers (`Container.container_type != ContainerType::Normal`). A non-container entity whose `ResolvedLayout` changed does NOT seed a descendant walk.

**Why:** The cascade only matters when a descendant resolves a `Cq*` unit or a `ContainerQuery` against the changed ancestor — and both resolve against a **query container** (`Size` / `InlineSize`), never a plain box (spec § 1.4 / `nearest_container_with_size` skips `Normal`). Seeding the walk from every changed box would re-translate descendants whose `Cqw` did not actually change reference (their nearest query container is elsewhere), wasting the re-run on no-op work and risking spurious re-run frames. Restricting to query containers makes the walk exactly the set whose descendants can observe a changed container size.

**How to apply:** T3's system query is `(With<Node>, Changed<ResolvedLayout>)` joined with a `Query<&Container>` lookup; only entities whose `Container.container_type != Normal` are passed to `collect_dirty_descendants`. (A non-`Container` entity is also skipped — no `Container` component → not a query target.)

**Runner-up rejected:** Seed from every `Changed<ResolvedLayout>` entity. Rejected: over-invalidates (most resized boxes are not query containers), bloats the re-run set, and can keep the re-run flag set on frames where nothing query-relevant changed.

### D4. The re-run re-resolves the dirty descendants only — bounded, single-re-run-per-frame, loop-safe

**Decision:** `cq_descendant_rerun` (step 9) re-translates **only** the entities in `ContainerSizeDirty` (plus the children-sync needed to attach them), recomputes Taffy once, re-writes `ResolvedLayout`, and re-evaluates container queries — then clears `CqDescendantReRunRequested`. It does **not** loop until fixpoint; a deeper `A`→`B`→`C`→`D` chain settles one further level per frame (the next frame's step 8 sees `C`'s now-changed `ResolvedLayout` and seeds `D`).

**Why:** Spec § 1.3 ("step 4 does not re-run; transitive flips wait until next frame") and § 1.5 ("frame N applies A's activation, frame N+1 applies B's") establish bounded per-frame re-layout as the loop-breaker (Blink prior-art: the 2×-Taffy ceiling). A single re-run per frame keeps the cost ceiling intact (at most 2× Taffy per frame even with the descendant pass, because the descendant re-run's Taffy compute is the second of the two — `cq_flip_rerun` and `cq_descendant_rerun` do not both fire on the same frame for the same cause; if both fire, the count is observable and capped by the per-frame instrument). Making the **direct** intermediate (`B`) same-frame while leaving deeper levels to subsequent frames is exactly the spec § 1.5 contract, now actually delivered (today `B` never updates at all).

**How to apply:** T4 clears the flag at the top (mirroring `cq_flip_rerun:3125`). T5-T7 re-run for the dirty set only. The dirty set itself is cleared at the start of step 8 each frame (T3) so it never accumulates across frames.

**Runner-up rejected:** Loop the re-run to fixpoint within one frame. Rejected: unbounded worst case (a pathological `Cqw`-chain could re-run O(depth) times), breaking the 2×-Taffy ceiling the spec and Blink prior-art both treat as load-bearing; the eventual-consistency-over-frames model is the established Buiy contract.

### D5. The re-run re-invokes `cq_activate` + `cq_flip_check` semantics inline (re-uses their pure pieces), so `C`'s marker flips the same frame

**Decision:** After re-translating + recomputing + re-writing `ResolvedLayout` for the dirty set, `cq_descendant_rerun` re-evaluates every `ContainerQuery` against the freshly-recomputed sizes and toggles `ContainerQueryActive` / `ContainerQueryInactive` — reusing `resolve_nearest_container` / `evaluate_conditions` (the same pieces `cq_activate` and `cq_flip_check` use). It does not wait for the next frame's `cq_activate`.

**Why:** The whole point of the phase is that `C` flips **in-frame**. `cq_activate` (step 2) and `cq_flip_check` (step 4) already ran earlier this frame, before `B`'s size changed — so without an in-re-run re-evaluation, `C`'s marker would not flip until next frame's `cq_activate` reads `B`'s now-updated `ResolvedLayout`. Re-evaluating inline (against the just-written sizes, the same source `cq_activate` reads) makes the marker correct before the frame ends, completing the same-frame settle the spec § 1.5 fixture asserts.

**How to apply:** T7 adds the rule re-evaluation: hold `Query<(Entity, &ContainerQuery, Option<&ContainerQueryActive>)>` + `Query<(&Container, &ResolvedLayout)>` + `Query<&ChildOf>` + `Commands` in `cq_descendant_rerun`'s param set, and after the size re-write, run the same active/inactive toggle as `cq_activate` (`systems.rs:2825-2847`) for the rules whose container is in the dirty set OR whose own ancestor chain touches a dirty entity. (Simplest correct scope: re-evaluate **all** rules — the rule count is small relative to leaf nodes; spec § 1.4 already treats query containers as sparse. T7 re-evaluates all rules for correctness and simplicity, matching `cq_activate`'s own full scan.)

**Runner-up rejected:** Let the next frame's `cq_activate` pick up `B`'s changed size. Rejected: that is the multi-frame lag the phase exists to remove; it would leave the renamed test failing (`C` would not be `Active` the same frame).

### D6. Two new pipeline steps (`CqDescendantInvalidate`, `CqDescendantReRun`) after `WriteResolvedLayout`, not folded into the `PostTaffyOverrides` chain

**Decision:** Add `BuiyLayoutStep::CqDescendantInvalidate` and `BuiyLayoutStep::CqDescendantReRun` to the `BuiyLayoutStep` enum, chained **after** `WriteResolvedLayout`. They are NOT sub-passes of `PostTaffyOverrides` (steps 6a-6f) and they run after step 7.

**Why:** The invalidation depends on `ResolvedLayout` being **written** (step 7) so it can read `Changed<ResolvedLayout>` on containers — `PostTaffyOverrides` runs before step 7 and reads Taffy directly, the wrong place. The re-run must then re-write `ResolvedLayout`, so it has to live after step 7 too. A dedicated pair of steps (mirroring how `CqActivate`/`CqFlipCheck`/`CqFlipReRun` are their own steps, not folded into `SyncStyles`) keeps the pipeline's "one concern per step" shape and lets `tests/layout_pipeline_order.rs` assert the ordering.

**How to apply:** T3 adds `CqDescendantInvalidate` after `WriteResolvedLayout` in both `pipeline.rs` (enum + `configure_sets` chain) and `mod.rs` (system wiring). T4 adds `CqDescendantReRun` after it.

**Runner-up rejected:** Fold both into a single new step. Rejected: invalidate (read-only scan + dirty-set write) and re-run (mutates the tree + re-evaluates rules) are distinct concerns with distinct param sets; splitting them mirrors the existing `CqFlipCheck` (detect) / `CqFlipReRun` (act) split and keeps each system's borrow set minimal.

---

## Tasks

> **Per-task workflow (subagent-driven):**
> 1. Implementer subagent reads the task block.
> 2. Implementer follows TDD: failing test first, then minimal impl to pass, then refactor if needed, then commit.
> 3. Spec-compliance reviewer subagent reads the spec sections + the diff and asserts coverage.
> 4. Code-quality reviewer subagent reads the diff and asserts the code-quality bar.
> 5. Both reviews must be ✅ before moving to the next task.

> **Project gate (run before every commit, exactly — drop `xvfb-run -a` on this host, which has no xvfb; `MinimalPlugins` runs headless):**
> ```sh
> cargo fmt --all -- --check && \
>   cargo clippy --workspace --all-targets -- -D warnings && \
>   RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
>   cargo test --workspace
> ```

### Task 1: `ContainerSizeDirty` + `CqDescendantReRunRequested` resources

**Spec:** § 1.3 (re-run flag analogue), D1.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add two resources + tests)

- [ ] **Step 1: Failing test.** Add to `systems.rs::mod tests` (the existing `#[cfg(test)] mod tests` block at the bottom of the file):
  ```rust
  #[test]
  fn container_size_dirty_default_is_empty() {
      assert!(ContainerSizeDirty::default().0.is_empty());
  }

  #[test]
  fn cq_descendant_rerun_requested_default_is_false() {
      assert!(!CqDescendantReRunRequested::default().0);
  }
  ```
  Run: `cargo test -p buiy_core container_size_dirty_default cq_descendant_rerun_requested_default` — expected FAIL (types don't exist).

- [ ] **Step 2: Add the resources to `systems.rs`.** Place them next to `CqReRunRequested` (`systems.rs:46`) so the re-run flags sit together:
  ```rust
  /// Dirty set for the multi-level container-query geometric cascade
  /// (Phase 14). Populated by step 8 (`cq_descendant_invalidate`) with the
  /// descendants of every query container whose `ResolvedLayout` changed
  /// this frame; drained by step 9 (`cq_descendant_rerun`), which
  /// re-translates exactly these entities so their `Length::Cq*` re-resolves
  /// against the new ancestor size in the same frame. Cleared at the top of
  /// step 8 each frame, so it never accumulates across frames (D1/D4).
  ///
  /// Private cross-pass hand-off (a resource, not an author-set component):
  /// follow-ups.md "Descendant invalidation on ancestor-resolved-size
  /// changes" option (b). Empty in steady state.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3, § 1.5.
  #[derive(Resource, Default, Debug)]
  pub struct ContainerSizeDirty(pub std::collections::HashSet<Entity>);

  /// Re-run request flag for the Phase-14 descendant invalidation, mirroring
  /// `CqReRunRequested` (Phase 5 step 5). Set `true` by step 8 when
  /// `ContainerSizeDirty` is non-empty; observed + cleared at the top of
  /// step 9 (`cq_descendant_rerun`). Capped at one re-run per frame (D4):
  /// deeper cascade levels settle on subsequent frames.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3.
  #[derive(Resource, Default, Debug)]
  pub struct CqDescendantReRunRequested(pub bool);
  ```
  **Implementer note:** `Entity` and `Resource` are in scope via the existing `use bevy::prelude::*;` at the top of `systems.rs` (confirm; `CqReRunRequested` already uses `#[derive(Resource, Default)]` there). Use the fully-qualified `std::collections::HashSet` in the field type (the file already imports `HashSet` for local use, but the public field type is spelled out for doc clarity, matching `TopLayerActivation`'s `std::collections::VecDeque` at `systems.rs`).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core container_size_dirty_default cq_descendant_rerun_requested_default
  ```
  Expected PASS.

- [ ] **Step 4: Project gate.** (Wiring into the plugin happens in T3/T4; here confirm compile/tests/doc green.)
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): ContainerSizeDirty + CqDescendantReRunRequested resources (Phase 14)

Private cross-pass hand-off resource (HashSet<Entity> dirty set) + re-run flag
for the multi-level container-query geometric cascade. init_resource wiring in
T3/T4.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 2: `collect_dirty_descendants` pure helper

**Spec:** § 1.5, D2, D3.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the pure descendant-walk helper + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`. These build a small `World` with a `Children`-based hierarchy and assert the walk flattens the subtree of the seed entities (excluding the seeds themselves):
  ```rust
  #[test]
  fn collect_dirty_descendants_flattens_subtree() {
      use bevy::prelude::*;
      let mut world = World::new();
      // a -> b -> c, plus a sibling leaf d under a.
      let c = world.spawn(Node).id();
      let d = world.spawn(Node).id();
      let b = world.spawn(Node).add_children(&[c]).id();
      let a = world.spawn(Node).add_children(&[b, d]).id();
      let mut q = world.query::<&Children>();
      let children_q = q.query(&world);
      let dirty = collect_dirty_descendants(&[a], &children_q);
      // a's descendants: b, c, d (a itself excluded).
      assert!(dirty.contains(&b));
      assert!(dirty.contains(&c));
      assert!(dirty.contains(&d));
      assert!(!dirty.contains(&a), "the seed container itself is not dirty");
      assert_eq!(dirty.len(), 3);
  }

  #[test]
  fn collect_dirty_descendants_empty_for_leaf_seed() {
      use bevy::prelude::*;
      let mut world = World::new();
      let leaf = world.spawn(Node).id();
      let mut q = world.query::<&Children>();
      let children_q = q.query(&world);
      let dirty = collect_dirty_descendants(&[leaf], &children_q);
      assert!(dirty.is_empty(), "a seed with no children produces no dirty descendants");
  }

  #[test]
  fn collect_dirty_descendants_dedups_overlapping_subtrees() {
      use bevy::prelude::*;
      let mut world = World::new();
      // a -> b -> c ; seed both a and b. c must appear once.
      let c = world.spawn(Node).id();
      let b = world.spawn(Node).add_children(&[c]).id();
      let a = world.spawn(Node).add_children(&[b]).id();
      let mut q = world.query::<&Children>();
      let children_q = q.query(&world);
      let dirty = collect_dirty_descendants(&[a, b], &children_q);
      assert!(dirty.contains(&b));
      assert!(dirty.contains(&c));
      assert_eq!(dirty.len(), 2, "c is reached from both a and b but appears once");
  }
  ```
  Run: `cargo test -p buiy_core collect_dirty_descendants` — expected FAIL (helper doesn't exist).

- [ ] **Step 2: Add the helper to `systems.rs`.** Place it near the other ancestor/descendant walk helpers (after `nearest_container_with_size`, `systems.rs:2873`):
  ```rust
  /// Flatten the descendant subtrees of `seeds` into a deduplicated set,
  /// EXCLUDING the seeds themselves. Phase 14 step 8 (`cq_descendant_invalidate`)
  /// calls this with the query containers whose `ResolvedLayout` changed this
  /// frame; the returned set is every entity that may resolve a `Length::Cq*`
  /// unit (or a `ContainerQuery`) against one of those containers and must be
  /// re-translated this frame (D2).
  ///
  /// Iterative breadth-first walk over `Children`. O(total subtree size); a
  /// `HashSet` membership guard makes overlapping seed subtrees (a container
  /// nested inside another changed container) cost each entity once (D2/D4).
  /// No cycle guard is needed — Bevy's `Children`/`ChildOf` hierarchy is a
  /// forest by construction.
  pub(super) fn collect_dirty_descendants(
      seeds: &[Entity],
      children_q: &Query<&Children>,
  ) -> std::collections::HashSet<Entity> {
      let mut dirty: std::collections::HashSet<Entity> = std::collections::HashSet::new();
      let mut stack: Vec<Entity> = seeds.to_vec();
      while let Some(entity) = stack.pop() {
          if let Ok(children) = children_q.get(entity) {
              for &child in children.iter() {
                  if dirty.insert(child) {
                      stack.push(child);
                  }
              }
          }
      }
      dirty
  }
  ```
  **Implementer note:** `Children` derefs to `&[Entity]`; `children.iter()` yields `&Entity` in Bevy 0.18 (the same idiom `sync_children_for_entity` uses at `systems.rs:1635`). The `dirty.insert(child)` return value (`true` if newly inserted) is the dedup + visited guard in one — only newly-seen children are pushed, so overlapping subtrees and the (impossible-by-construction) revisit both cost O(1). `Query<&Children>` is the read-only borrow; `children_q.get(entity)` returns `Err` for a leaf (no `Children` component), which the `if let Ok` handles.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core collect_dirty_descendants
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): collect_dirty_descendants subtree walk (Phase 14 — D2)

Pure breadth-first Children walk flattening the descendant subtrees of the
changed query containers into a deduped HashSet, excluding the seeds. The
step-8 invalidation seed-set builder.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 3: `cq_descendant_invalidate` system (step 8) + pipeline wiring

**Spec:** § 1.5, D2, D3, D6.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the `cq_descendant_invalidate` system)
- Modify: `crates/buiy_core/src/layout/pipeline.rs` (add `CqDescendantInvalidate` step + chain it after `WriteResolvedLayout`)
- Modify: `crates/buiy_core/src/layout/mod.rs` (wire the system into the step; `init_resource::<ContainerSizeDirty>`)
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` (assert step 8 runs after step 7 by observing the dirty set after a container resize)

- [ ] **Step 1: Failing test.** Add to `tests/layout_container_queries.rs` (it already has the `app()` helper + the imports this needs except `ContainerSizeDirty`, which is crate-internal — so this test asserts the **observable effect** rather than reading the resource directly). The observable effect of step 8 firing is that, on the frame `A` resizes, the dirty set is populated and the re-run flag is set, which (with T4-T7) makes `C` flip. But T4-T7 are not landed yet at T3 — so T3's test asserts the **narrower** fact that step 8 runs after step 7 and populates the resource. Because `ContainerSizeDirty` is `pub` on `systems` but not re-exported, the test reads it via the crate-internal path is NOT available to an integration test. Therefore T3's test lives in `tests/layout_pipeline_order.rs` and asserts ordering through a public proxy: spawn a query container with a child, resize the container, and assert the child's `ResolvedLayout` is unchanged THIS frame (step 8 only marks dirty; the re-run that would change it lands in T4-T7) AND no panic / system-order ambiguity. The robust ordering assertion is the run-order itself.

  Concretely, add to `crates/buiy_core/tests/layout_pipeline_order.rs` (mirror its existing `app()` + ordering-assertion idiom; the file already asserts the 9-step chain — extend it to assert the two new steps come last):
  ```rust
  #[test]
  fn cq_descendant_invalidate_runs_after_write_resolved_layout() {
      // Step 8 (CqDescendantInvalidate) must be ordered AFTER step 7
      // (WriteResolvedLayout) so it can read Changed<ResolvedLayout> on
      // containers. We assert the schedule ordering via the same
      // system-set ordering check the existing pipeline-order tests use.
      let app = app();
      // The existing helper in this file that resolves a step's run order;
      // if the file uses `assert_step_order(&app, &[...])`, extend its slice.
      // Otherwise assert via the schedule graph that CqDescendantInvalidate's
      // set runs after WriteResolvedLayout's set.
      assert_step_after(
          &app,
          BuiyLayoutStep::CqDescendantInvalidate,
          BuiyLayoutStep::WriteResolvedLayout,
      );
  }
  ```
  **Implementer note:** read `tests/layout_pipeline_order.rs` first and match its EXISTING assertion mechanism. If it asserts the full chain via a single ordered slice of `BuiyLayoutStep` variants, simply append `CqDescendantInvalidate` (and in T4, `CqDescendantReRun`) to that slice and that is the failing test — no new helper needed. The `assert_step_after` form above is a fallback only if the file has no full-chain slice. Import `BuiyLayoutStep` (`use buiy_core::layout::BuiyLayoutStep;`) if not already imported.
  Run: `cargo test -p buiy_core --test layout_pipeline_order cq_descendant_invalidate_runs_after` — expected FAIL (step variant doesn't exist / not in the chain).

- [ ] **Step 2a: Add the `CqDescendantInvalidate` step to `pipeline.rs`.** In the `BuiyLayoutStep` enum (`pipeline.rs:17-44`), after the `WriteResolvedLayout` variant, add:
  ```rust
      /// Step 8 — multi-level container-query geometric-cascade invalidation:
      /// mark the descendants of every query container whose `ResolvedLayout`
      /// changed this frame as dirty. **Phase 14.**
      CqDescendantInvalidate,
  ```
  And in `configure_pipeline`'s `.chain()` tuple (`pipeline.rs:51-59`), append after `BuiyLayoutStep::WriteResolvedLayout`:
  ```rust
              BuiyLayoutStep::WriteResolvedLayout,
              BuiyLayoutStep::CqDescendantInvalidate,
  ```
  Also update the module doc-comment at `pipeline.rs:1-10` count if it says "Nine ordered sub-sets" — it becomes "Eleven ordered sub-sets" after T4. (T3 makes it ten; T4 makes it eleven. Update to the final count "eleven" in T4 to avoid a mid-state lie; in T3 leave the prose and fix it in T4 — OR update to "ten" here and "eleven" in T4. Pick one and be consistent; the implementer-note: update the number in T4 to its final value.)

- [ ] **Step 2b: Add the `cq_descendant_invalidate` system to `systems.rs`.** Place it after `write_resolved_layout` (`systems.rs:2644`):
  ```rust
  /// Step 8 (`BuiyLayoutStep::CqDescendantInvalidate`) — seed the
  /// multi-level container-query geometric cascade. Runs AFTER
  /// `write_resolved_layout` (step 7) so it can read `Changed<ResolvedLayout>`
  /// on query containers (the entities that actually changed). For every
  /// query container (`Container { container_type != Normal }`) whose
  /// `ResolvedLayout` changed this frame, walk its descendants and mark them
  /// dirty in `ContainerSizeDirty`; if any were marked, set
  /// `CqDescendantReRunRequested(true)` so step 9 re-translates them this
  /// frame. Bevy ships no "ancestor changed" filter, so the cascade is found
  /// by reading the changed container and walking DOWN (D2/D3).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3, § 1.5.
  pub(super) fn cq_descendant_invalidate(
      changed_containers: Query<(Entity, &Container), (With<Node>, Changed<ResolvedLayout>)>,
      children_q: Query<&Children>,
      mut dirty: ResMut<ContainerSizeDirty>,
      mut rerun: ResMut<CqDescendantReRunRequested>,
  ) {
      // Fresh set each frame — never accumulate (D4).
      dirty.0.clear();

      // Seeds = query containers (Size / InlineSize) whose ResolvedLayout
      // changed this frame. Plain boxes and Normal containers are skipped:
      // descendants only resolve Cq* against a query container (D3).
      let seeds: Vec<Entity> = changed_containers
          .iter()
          .filter(|(_, c)| c.container_type != ContainerType::Normal)
          .map(|(e, _)| e)
          .collect();

      if seeds.is_empty() {
          rerun.0 = false;
          return;
      }

      dirty.0 = collect_dirty_descendants(&seeds, &children_q);
      rerun.0 = !dirty.0.is_empty();
  }
  ```
  **Implementer note:** `Container`, `ContainerType`, `ResolvedLayout`, `Changed`, `Children`, `With`, `Node`, `Query`, `ResMut` are all already in scope in `systems.rs`. The query joins `Changed<ResolvedLayout>` (the filter) with `&Container` (the data) — an entity with no `Container` component is simply not matched, so the `container_type != Normal` filter only sees actual containers. `collect_dirty_descendants` is the T2 helper.

- [ ] **Step 2c: Wire the system + resource in `mod.rs`.** In the `app.init_resource::<…>()` block (`mod.rs:53-82`), add after the Phase-11 line:
  ```rust
          // Phase 14 — multi-level descendant invalidation: the dirty set
          // (cleared + populated by step 8) and the same-frame re-run flag
          // (consumed by step 9). Both crate-internal (not re-exported).
          app.init_resource::<systems::ContainerSizeDirty>();
  ```
  In the `app.add_systems(Update, ( … ))` tuple (`mod.rs:194-238`), after the `write_resolved_layout` line, add:
  ```rust
                  systems::cq_descendant_invalidate
                      .in_set(BuiyLayoutStep::CqDescendantInvalidate),
  ```
  (The `CqDescendantReRunRequested` `init_resource` + the `cq_descendant_rerun` wiring land in T4.)

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_pipeline_order cq_descendant_invalidate_runs_after
  ```
  Expected PASS (the step exists, is chained after step 7, and the system is wired).

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/pipeline.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_pipeline_order.rs
  git commit -m "feat(layout): cq_descendant_invalidate step 8 (Phase 14 — D2/D3/D6)

New pipeline step after WriteResolvedLayout: seeds ContainerSizeDirty from the
descendants of query containers whose ResolvedLayout changed this frame, sets
the re-run flag. The same-frame re-run that drains it lands in T4-T7.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 4: `cq_descendant_rerun` skeleton (step 9) — gate, clear, wiring

**Spec:** § 1.3, D4, D6.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the `cq_descendant_rerun` system — gate + clear only; body lands in T5-T7)
- Modify: `crates/buiy_core/src/layout/pipeline.rs` (add `CqDescendantReRun` step + chain it after `CqDescendantInvalidate`)
- Modify: `crates/buiy_core/src/layout/mod.rs` (wire the system; `init_resource::<CqDescendantReRunRequested>`)
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` (assert step 9 runs after step 8)

- [ ] **Step 1: Failing test.** Add to `tests/layout_pipeline_order.rs`:
  ```rust
  #[test]
  fn cq_descendant_rerun_runs_after_invalidate() {
      let app = app();
      assert_step_after(
          &app,
          BuiyLayoutStep::CqDescendantReRun,
          BuiyLayoutStep::CqDescendantInvalidate,
      );
  }
  ```
  **Implementer note:** same as T3 — if the file uses a full-chain ordered slice, append `BuiyLayoutStep::CqDescendantReRun` after `CqDescendantInvalidate` in that slice instead of a per-pair assertion. Run: `cargo test -p buiy_core --test layout_pipeline_order cq_descendant_rerun_runs_after` — expected FAIL.

- [ ] **Step 2a: Add the `CqDescendantReRun` step to `pipeline.rs`.** In the enum, after `CqDescendantInvalidate`:
  ```rust
      /// Step 9 — conditional same-frame re-run of the inner work of
      /// `sync_styles` + `taffy_compute` (+ `ResolvedLayout` re-write + CQ
      /// re-evaluation) for the entities `cq_descendant_invalidate` marked
      /// dirty. Gated on `CqDescendantReRunRequested`; capped at one re-run
      /// per frame (D4). **Phase 14.**
      CqDescendantReRun,
  ```
  And in `configure_pipeline`'s chain, after `BuiyLayoutStep::CqDescendantInvalidate`:
  ```rust
              BuiyLayoutStep::CqDescendantInvalidate,
              BuiyLayoutStep::CqDescendantReRun,
  ```
  Update the module-doc count to its final value: change "Nine ordered sub-sets" to "Eleven ordered sub-sets" (steps 0-9 plus the pre-step-1 `WritingModeInherit` = eleven sets total).

- [ ] **Step 2b: Add the `cq_descendant_rerun` system skeleton to `systems.rs`.** Place it after `cq_descendant_invalidate`. This task lands ONLY the gate + flag-clear; T5-T7 fill the body (each TDD step adds one concern). The skeleton:
  ```rust
  /// Step 9 (`BuiyLayoutStep::CqDescendantReRun`) — when
  /// `cq_descendant_invalidate` (step 8) marked descendants dirty, re-run the
  /// inner work of `sync_styles` + `taffy_compute` for exactly that dirty set,
  /// re-write their `ResolvedLayout`, and re-evaluate container queries so a
  /// rule-bearing descendant flips its marker the SAME frame (D4/D5). Capped
  /// at one re-run per frame: deeper cascade levels settle on subsequent
  /// frames (spec § 1.3 / § 1.5). Mirrors `cq_flip_rerun` (step 5).
  ///
  /// Body is gated on `CqDescendantReRunRequested.0`; the flag is cleared at
  /// the top so the system is a no-op on non-cascade frames.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.3, § 1.5.
  pub(super) fn cq_descendant_rerun(mut rerun: ResMut<CqDescendantReRunRequested>) {
      if !rerun.0 {
          return;
      }
      rerun.0 = false;
      // T5: re-translate the dirty set; T6: recompute Taffy + re-write
      // ResolvedLayout; T7: re-evaluate container queries.
  }
  ```
  **Implementer note:** the param set grows in T5-T7 (it becomes the union of `sync_styles` + `taffy_compute` + `write_resolved_layout` + `cq_activate` params, like `cq_flip_rerun`'s 10-param set). T4 lands the minimal gate so the schedule wiring + ordering test pass without a half-written body. `#[allow(clippy::too_many_arguments)]` will be added in T5 when the param count climbs (mirroring `cq_flip_rerun:3074`).

- [ ] **Step 2c: Wire the system + resource in `mod.rs`.** In the `init_resource` block, after the `ContainerSizeDirty` line (T3):
  ```rust
          app.init_resource::<systems::CqDescendantReRunRequested>();
  ```
  In the `add_systems` tuple, after `cq_descendant_invalidate` (T3):
  ```rust
                  systems::cq_descendant_rerun.in_set(BuiyLayoutStep::CqDescendantReRun),
  ```

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_pipeline_order cq_descendant_rerun_runs_after
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/pipeline.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_pipeline_order.rs
  git commit -m "feat(layout): cq_descendant_rerun step 9 skeleton (Phase 14 — D4/D6)

Gated no-op re-run system after step 8, clears CqDescendantReRunRequested at
the top (one re-run per frame). Body (re-translate, recompute, re-evaluate)
lands in T5-T7.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 5: re-run body — re-translate the dirty descendants

**Spec:** § 1.4 (Cq* resolution), § 1.5, D4.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (fill `cq_descendant_rerun`'s body: re-translate the dirty set via `translate_one_entity`)

- [ ] **Step 1: Failing test.** Add to `tests/layout_container_queries.rs`. This is the first observable behavior change: after `A` resizes, `B`'s `Cqw`-derived `ResolvedLayout.size.x` updates **this frame** (today it stays stale — the very lag the existing negative test documents). Assert `B`'s width, NOT yet `C`'s marker (the marker re-eval lands in T7):
  ```rust
  #[test]
  fn cq_intermediate_b_reresolves_cqw_in_frame() {
      let mut app = app();
      let a = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(700.0).height_px(400.0).container_size(),
          ))
          .id();
      let b = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width(Sizing::Length(Length::Cqw(80.0)))
                  .height_px(400.0)
                  .container_size(),
          ))
          .id();
      let c = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              ContainerQuery {
                  container: None,
                  conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
              },
          ))
          .id();
      app.world_mut().entity_mut(a).add_children(&[b]);
      app.world_mut().entity_mut(b).add_children(&[c]);

      app.update();
      app.update();
      assert_eq!(
          app.world().get::<ResolvedLayout>(b).map(|l| l.size.x),
          Some(560.0),
          "B settles to Cqw(80) of A(700) = 560"
      );

      // Widen A to 1000. Cqw(80) of 1000 = 800. With the Phase-14 descendant
      // re-run, B re-resolves THIS frame.
      app.world_mut().entity_mut(a).insert(
          Style::default().width_px(1000.0).height_px(400.0).container_size(),
      );
      app.update();
      assert_eq!(
          app.world().get::<ResolvedLayout>(b).map(|l| l.size.x),
          Some(800.0),
          "B re-resolves Cqw(80) of A(1000) = 800 in the SAME frame A resized"
      );
      let _ = c; // C's marker flip is asserted in T7's test.
  }
  ```
  Run: `cargo test -p buiy_core --test layout_container_queries cq_intermediate_b_reresolves_cqw_in_frame` — expected FAIL (`B` still reports `560.0` — the re-run body is empty).

- [ ] **Step 2: Fill the re-translate body of `cq_descendant_rerun`.** Grow the param set to the `sync_styles`/`taffy_compute` union (the `cq_flip_rerun` shape) and re-translate exactly the dirty set. Replace the T4 skeleton signature + body with:
  ```rust
  #[allow(clippy::type_complexity, clippy::too_many_arguments)]
  pub(super) fn cq_descendant_rerun(
      mut rerun: ResMut<CqDescendantReRunRequested>,
      dirty: Res<ContainerSizeDirty>,
      mut compute_count: ResMut<LayoutTaffyComputeCount>,
      mut tree: NonSendMut<LayoutTree>,
      nodes: Query<NodeQueryItem<'_>, With<Node>>,
      parent_grid_lookup: Query<&GridParams>,
      container_snapshot_source: Query<(Entity, &Container, &ResolvedLayout)>,
      primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
      cq_parent_chain: Query<&ChildOf>,
      roots: Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>,
      windows: Query<&bevy::window::Window>,
  ) {
      if !rerun.0 {
          return;
      }
      rerun.0 = false;

      let tree = &mut *tree;

      // Snapshots rebuilt from the just-written ResolvedLayout (step 7),
      // exactly as cq_flip_rerun rebuilds them — the dirty descendants resolve
      // Cq* against the NEW ancestor size now present in container_index.
      let parent_areas_for: HashMap<Entity, GridAreas> = nodes
          .iter()
          .filter_map(|(entity, .., parent)| {
              let p = parent?;
              let grid = parent_grid_lookup.get(p.parent()).ok()?;
              grid.template_areas.clone().map(|a| (entity, a))
          })
          .collect();

      let container_index: HashMap<Entity, ContainerSnapshot> = container_snapshot_source
          .iter()
          .filter_map(|(entity, container, layout)| {
              if container.container_type == ContainerType::Normal {
                  None
              } else {
                  Some((
                      entity,
                      ContainerSnapshot {
                          container_type: container.container_type,
                          size: layout.size,
                      },
                  ))
              }
          })
          .collect();

      let viewport_size = primary_window
          .single()
          .ok()
          .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
          .unwrap_or(Vec2::ZERO);

      // Re-translate ONLY the dirty descendants (D4). Iterate the full node
      // set but act only on dirty members — keeps the borrow simple and the
      // work bounded by the dirty set.
      for item in nodes.iter() {
          let entity = item.0;
          if !dirty.0.contains(&entity) {
              continue;
          }
          translate_one_entity(
              item,
              &parent_areas_for,
              &container_index,
              &cq_parent_chain,
              viewport_size,
              None,
              tree,
          );
      }

      // Children-sync + Taffy recompute land in T6; CQ re-evaluation in T7.
      let _ = (&roots, &windows, &mut compute_count);
  }
  ```
  **Implementer note:** `NodeQueryItem<'_>` (`systems.rs:2214`), `ContainerSnapshot`, `GridAreas`, `Vec2`, `HashMap`, `LayoutTree`, `NonSendMut`, `LayoutTaffyComputeCount`, `translate_one_entity`, `ContainerType` are all in scope. The `let _ = (&roots, &windows, &mut compute_count);` line keeps the as-yet-unused params from tripping `unused_variables` / `dead_code` under `-D warnings` — T6 removes it when it consumes them. (If clippy flags the `let _ =` tuple, use `let _ = &roots; let _ = &windows; let _ = &mut compute_count;` — but a single tuple binding is accepted; verify with the gate.) The re-translate alone is NOT enough to change `B`'s `ResolvedLayout` — that needs the Taffy recompute + re-write in T6. **So the T5 test will still FAIL after T5's impl** unless T6 lands. To keep T5 self-contained and green, T5 MUST also include the children-sync + Taffy recompute + ResolvedLayout re-write. **Correction:** fold T6's recompute+rewrite into T5 so T5's test passes; T6 then only adds the Taffy-count assertion. See T6.

  **Resolved scoping (authoritative):** T5 lands re-translate **and** children-sync **and** Taffy recompute **and** `ResolvedLayout` re-write (everything needed for `B`'s width to change this frame). Append the following to the body, REPLACING the `let _ = (…)` placeholder line:
  ```rust
      // Children-sync over the FULL tree (`roots`): a dirty descendant that
      // re-translated may need its parent's Taffy child list rebuilt. Mirrors
      // cq_flip_rerun's full-tree children-sync (systems.rs:3182-3190).
      let rows: Vec<(Entity, bool, Option<&Children>, Option<&ChildOf>)> = roots
          .iter()
          .map(|(entity, children, parent, position)| {
              (entity, is_fixed_root(position), children, parent)
          })
          .collect();
      sync_children_pass(&rows, &HashSet::new(), tree);

      // Re-invoke Taffy compute per root (same shape as cq_flip_rerun:3196-3224;
      // NO compute_count reset — that lives only in taffy_compute, so a
      // cascade frame ends with count incremented, observable for the 2x cap).
      let window_size = windows
          .iter()
          .next()
          .map(|w| Vec2::new(w.width(), w.height()))
          .unwrap_or(Vec2::new(800.0, 600.0));
      for (entity, _children, parent, _position) in roots.iter() {
          let is_root = parent
              .map(|p| !tree.by_entity.contains_key(&p.parent()))
              .unwrap_or(true);
          if !is_root {
              continue;
          }
          if let Some(id) = tree.by_entity.get(&entity).copied() {
              match tree.tree.compute_layout(
                  id,
                  Size {
                      width: AvailableSpace::Definite(window_size.x),
                      height: AvailableSpace::Definite(window_size.y),
                  },
              ) {
                  Ok(_) => compute_count.0 += 1,
                  Err(err) => {
                      warn!(?entity, ?err, "buiy: layout compute_layout (descendant re-run) failed")
                  }
              }
          }
      }
  ```
  Then re-write `ResolvedLayout` for the dirty set so downstream (and the test) sees the new size. Add a final block (the re-run's analogue of `write_resolved_layout`, scoped to dirty entities — a fresh `Commands` param is needed; add `mut commands: Commands` to the signature and a `existing: Query<&ResolvedLayout>` + `overrides: Res<PostTaffyPositionOverrides>` to read the same inputs `write_resolved_layout` uses):
  ```rust
      // Re-write ResolvedLayout for the dirty set from the recomputed Taffy
      // tree (mirror write_resolved_layout:2616-2643, scoped to dirty).
      for &entity in dirty.0.iter() {
          let Some(&id) = tree.by_entity.get(&entity) else { continue };
          let Ok(layout) = tree.tree.layout(id) else { continue };
          let position = overrides
              .by_entity
              .get(&entity)
              .copied()
              .unwrap_or_else(|| Vec2::new(layout.location.x, layout.location.y));
          let new = ResolvedLayout {
              position,
              size: Vec2::new(layout.size.width, layout.size.height),
          };
          let unchanged = existing
              .get(entity)
              .map(|cur| cur.position == new.position && cur.size == new.size)
              .unwrap_or(false);
          if !unchanged {
              commands.entity(entity).insert(new);
          }
      }
  ```
  Add `mut commands: Commands`, `existing: Query<&ResolvedLayout>`, and `overrides: Res<PostTaffyPositionOverrides>` to the param list (between `dirty` and `compute_count`, order is irrelevant). `Size`, `AvailableSpace`, `is_fixed_root`, `sync_children_pass`, `PostTaffyPositionOverrides`, `ResolvedLayout`, `Commands`, `Res` are all in scope (`taffy::{Size, AvailableSpace}` are imported at the top — confirm via `taffy_compute`'s use at `systems.rs:2549`).

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_container_queries cq_intermediate_b_reresolves_cqw_in_frame
  ```
  Expected PASS (`B` now reports `800.0` the frame `A` resized).

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_container_queries.rs
  git commit -m "feat(layout): cq_descendant_rerun re-translates + recomputes dirty set (Phase 14 — D4)

Re-run body: rebuild snapshots from the just-written ResolvedLayout,
re-translate the dirty descendants via translate_one_entity, children-sync,
Taffy recompute (count-bumped for the 2x cap), and re-write ResolvedLayout for
the dirty set. Intermediate B now re-resolves Cqw in the same frame its query
container resizes.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 6: re-run instrumentation — assert the 2× Taffy per-frame cap holds

**Spec:** § 1.3 (cost ceiling), D4.

**Files:**
- Modify: `crates/buiy_core/tests/layout_container_queries.rs` (add a Taffy-count assertion on a cascade frame)

- [ ] **Step 1: Failing test.** The re-run from T5 bumps `LayoutTaffyComputeCount`. Assert that on a cascade frame the count is exactly 2 (one in `taffy_compute` step 3, one in the descendant re-run step 9) — and on a steady-state frame it is 1. `LayoutTaffyComputeCount` IS re-exported (`mod.rs:22` → `pub use systems::{… LayoutTaffyComputeCount …}`), so the integration test can read it. Add:
  ```rust
  #[test]
  fn cq_descendant_rerun_caps_at_2x_taffy() {
      let mut app = app();
      let a = app
          .world_mut()
          .spawn((Node, Style::default().width_px(700.0).height_px(400.0).container_size()))
          .id();
      let b = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width(Sizing::Length(Length::Cqw(80.0)))
                  .height_px(400.0)
                  .container_size(),
          ))
          .id();
      app.world_mut().entity_mut(a).add_children(&[b]);

      app.update();
      app.update();
      // Steady state: one Taffy compute, no descendant re-run.
      assert_eq!(
          app.world().resource::<LayoutTaffyComputeCount>().0,
          1,
          "steady-state frame runs Taffy once"
      );

      // Resize A → B is dirtied → descendant re-run fires → 2 Taffy computes.
      app.world_mut().entity_mut(a).insert(
          Style::default().width_px(1000.0).height_px(400.0).container_size(),
      );
      app.update();
      assert_eq!(
          app.world().resource::<LayoutTaffyComputeCount>().0,
          2,
          "cascade frame caps at 2x Taffy (step 3 + step 9 re-run)"
      );

      // Next steady frame settles back to one compute (cascade did not recur).
      app.update();
      assert_eq!(
          app.world().resource::<LayoutTaffyComputeCount>().0,
          1,
          "post-cascade frame returns to one Taffy compute"
      );
  }
  ```
  **Implementer note:** add `LayoutTaffyComputeCount` to the `use buiy_core::layout::{…}` import at the top of the test file if not already present (it is imported by the existing `cq_same_frame_relayout_caps_at_2x_taffy` test — confirm). `taffy_compute` resets the count to 0 at the start of each frame (`systems.rs:2549` region), so each frame's count is self-contained. The post-cascade frame returns to 1 because B's size is now stable (`Cqw(80)` of `1000` = `800` re-resolved last frame; no further `Changed<ResolvedLayout>` on A this frame → step 8 seeds nothing → re-run is a no-op).
  Run: `cargo test -p buiy_core --test layout_container_queries cq_descendant_rerun_caps_at_2x_taffy` — this may already PASS if T5's count-bump is correct; if it FAILS, the failure pinpoints a count bug in T5's recompute (e.g. resetting the count in the re-run, or skipping the bump). Fix the T5 body, not the test.

- [ ] **Step 2: Implementation.** If the test passes as-is, no code change — T5 already bumps the count correctly. If it FAILS, the bug is in T5's recompute loop: ensure the re-run does **not** reset `compute_count.0 = 0` (only `taffy_compute` does that) and **does** `compute_count.0 += 1` per successful root `compute_layout`. Make the minimal fix in `cq_descendant_rerun`.
  **Implementer note:** this task is a verification gate on the cost-ceiling invariant (spec § 1.3). It exists as a separate task so the 2×-Taffy contract is asserted explicitly rather than assumed; if T5 was implemented exactly as written, expect PASS with no code change and commit the test alone.

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_container_queries cq_descendant_rerun_caps_at_2x_taffy
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_container_queries.rs crates/buiy_core/src/layout/systems.rs
  git commit -m "test(layout): assert descendant re-run caps at 2x Taffy per frame (Phase 14 — D4)

Cascade frame = exactly 2 Taffy computes (step 3 + step 9 re-run); steady-state
and post-cascade frames = 1. Locks in the spec 1.3 cost ceiling.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 7: re-run body — re-evaluate container queries so `C` flips in-frame

**Spec:** § 1.3 (CqActivate/CqFlipCheck semantics), § 1.5, D5.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the rule re-evaluation block to `cq_descendant_rerun`)

- [ ] **Step 1: Failing test.** Now assert the full same-frame settle: `C`'s `ContainerQuery` flips to `Active` the frame `A` resizes (because `B`'s re-resolved width 800 ≥ 700). Add:
  ```rust
  #[test]
  fn cq_descendant_c_activates_in_frame_after_a_resize() {
      let mut app = app();
      let a = app
          .world_mut()
          .spawn((Node, Style::default().width_px(700.0).height_px(400.0).container_size()))
          .id();
      let b = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width(Sizing::Length(Length::Cqw(80.0)))
                  .height_px(400.0)
                  .container_size(),
          ))
          .id();
      let c = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              ContainerQuery {
                  container: None,
                  conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
              },
          ))
          .id();
      app.world_mut().entity_mut(a).add_children(&[b]);
      app.world_mut().entity_mut(b).add_children(&[c]);

      app.update();
      app.update();
      // Steady state: B = Cqw(80) of 700 = 560 < 700 → C inactive.
      assert!(
          app.world().get::<ContainerQueryInactive>(c).is_some(),
          "C inactive at steady-state (B=560 < 700)"
      );

      // Resize A → B re-resolves to 800 ≥ 700 → C activates THIS frame.
      app.world_mut().entity_mut(a).insert(
          Style::default().width_px(1000.0).height_px(400.0).container_size(),
      );
      app.update();
      assert!(
          app.world().get::<ContainerQueryActive>(c).is_some(),
          "C activates in the SAME frame A resized (B re-resolved to 800 >= 700)"
      );
      assert!(
          app.world().get::<ContainerQueryInactive>(c).is_none(),
          "the inactive marker is removed on activation"
      );
  }
  ```
  Run: `cargo test -p buiy_core --test layout_container_queries cq_descendant_c_activates_in_frame_after_a_resize` — expected FAIL (`B`'s width updates from T5, but `C`'s marker still reflects the pre-resize evaluation — no in-re-run rule re-eval yet).

- [ ] **Step 2: Add the rule re-evaluation block to `cq_descendant_rerun`.** Add three params to the signature — `rules: Query<(Entity, &ContainerQuery, Option<&ContainerQueryActive>, Option<&ContainerQueryInactive>), With<Node>>`, `containers: Query<(&Container, &ResolvedLayout)>`, and `rule_parent_chain: Query<&ChildOf>` (a second `Query<&ChildOf>` is a conflicting borrow with `cq_parent_chain` only if both are `&mut` — both are read-only `&ChildOf`, which Bevy 0.18 allows as multiple read borrows; if the borrow-checker rejects the duplicate, reuse `cq_parent_chain` for the rule walk instead of adding a second). After the `ResolvedLayout` re-write block (T5), add:
  ```rust
      // Re-evaluate container queries against the just-recomputed sizes so a
      // rule-bearing descendant flips its marker THIS frame (D5). Same toggle
      // logic as cq_activate (systems.rs:2825-2847), reading the same
      // (&Container, &ResolvedLayout) source. We re-write ResolvedLayout via
      // Commands above (deferred), so read sizes from the Taffy tree directly
      // (current this frame) — the same fresh source cq_flip_check uses
      // (architecture.md § 3.2), NOT the not-yet-applied Commands insert.
      let mut memo: HashMap<Entity, Option<Entity>> = HashMap::new();
      for (entity, rule, was_active, was_inactive) in rules.iter() {
          let container_entity = resolve_nearest_container(
              entity,
              &rule.container,
              &mut memo,
              &containers,
              &rule_parent_chain,
          );
          let active = match container_entity {
              Some(cont) => match tree.by_entity.get(&cont) {
                  Some(&node_id) => match tree.tree.layout(node_id) {
                      Ok(layout) => evaluate_conditions(
                          &rule.conditions,
                          Vec2::new(layout.size.width, layout.size.height),
                      ),
                      Err(_) => false,
                  },
                  None => false,
              },
              None => false,
          };
          if active && was_active.is_none() {
              commands
                  .entity(entity)
                  .insert(ContainerQueryActive)
                  .remove::<ContainerQueryInactive>();
          } else if !active && was_inactive.is_none() {
              commands
                  .entity(entity)
                  .insert(ContainerQueryInactive)
                  .remove::<ContainerQueryActive>();
          }
      }
  ```
  **Implementer note:** read sizes from `tree.tree.layout(node_id)` (the freshly-recomputed Taffy output this frame), NOT from the `ResolvedLayout` written via deferred `Commands` (which has not applied yet within this system) — this matches `cq_flip_check`'s explicit source pinning (`systems.rs:3008`, architecture.md § 3.2). `resolve_nearest_container` (`systems.rs:2889`) takes `Query<(&Container, &ResolvedLayout)>`, so the `containers` param is that shape. `evaluate_conditions` (`systems.rs:2733`), `ContainerQueryActive`, `ContainerQueryInactive` are in scope. If a second `Query<&ChildOf>` borrow is rejected by the scheduler at runtime (system param conflict), reuse `cq_parent_chain` for both walks — it is read-only and the re-translate loop has already finished consuming it. Confirm whether `resolve_nearest_container` needs the `containers` query borrowed simultaneously with `tree.tree.layout` — `containers` is the ancestor-walk source (reads `&Container`), `tree.tree.layout` reads Taffy; no conflict (one is a `Query`, one is the `NonSendMut<LayoutTree>` already borrowed as `tree`).

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_container_queries cq_descendant_c_activates_in_frame_after_a_resize
  ```
  Expected PASS (`C` is now `ContainerQueryActive` the frame `A` resized).

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_container_queries.rs
  git commit -m "feat(layout): descendant re-run re-evaluates container queries in-frame (Phase 14 — D5)

After recomputing the dirty set's Taffy sizes, re-run the cq_activate toggle
against the fresh Taffy output so a rule-bearing descendant (C) flips its
ContainerQueryActive/Inactive marker the same frame its grand-ancestor (A)
resizes. Completes the A->B->C same-frame cascade.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 8: flip the regression test polarity (negative → positive)

**Spec:** § 1.5 (the `A`→`B`→`C` transitive-cascade fixture), follow-ups.md (the documented polarity flip).

**Files:**
- Modify: `crates/buiy_core/tests/layout_container_queries.rs` (rename + flip `cq_transitive_cascade_is_one_frame_stale`)

- [ ] **Step 1: Flip the existing test.** The existing `cq_transitive_cascade_is_one_frame_stale` (`tests/layout_container_queries.rs:219-314`) asserts `C` stays `Inactive` after `A`'s resize. Phase 14 makes the cascade catch up in-frame, so this assertion is now FALSE — the test as written will FAIL the gate, which is the signal to flip it. **Rename** the function to `cq_transitive_cascade_catches_up_in_frame` and **rewrite** its post-resize assertions from "stays inactive" to "activates in-frame". Do NOT delete it. Replace the whole function (lines 199-314, including the doc comment block above it that describes the documented lag) with:
  ```rust
  /// Phase-14 regression: the multi-level container-query geometric cascade
  /// now catches up IN-FRAME. When query container `A` resizes, the
  /// `Cqw`-sized intermediate `B` is re-translated by the step-9 descendant
  /// re-run (seeded by step 8's `ContainerSizeDirty`), so `B`'s width
  /// re-resolves against the new `A` size and `C`'s `ContainerQuery`
  /// re-evaluates — all within the same frame.
  ///
  /// This is the polarity flip of the former
  /// `cq_transitive_cascade_is_one_frame_stale` negative assertion (Phase 5
  /// documented the gap; Phase 14 closes it — see
  /// docs/plans/follow-ups.md "Descendant invalidation on
  /// ancestor-resolved-size changes").
  ///
  /// Scenario:
  /// - A: outer container, width 700 → 1000, container_size.
  /// - B: child of A; width = `Cqw(80)` of A; container_size; no rule.
  /// - C: child of B; `ContainerQuery MinWidth(700)`.
  ///
  /// Steady-state: A=700, B=560 (Cqw(80) of 700), C inactive (560 < 700).
  /// After widening A to 1000: B=800 (Cqw(80) of 1000) same frame, C ACTIVE
  /// same frame (800 ≥ 700).
  #[test]
  fn cq_transitive_cascade_catches_up_in_frame() {
      let mut app = app();
      let a = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(700.0).height_px(400.0).container_size(),
          ))
          .id();
      let b = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .width(Sizing::Length(Length::Cqw(80.0)))
                  .height_px(400.0)
                  .container_size(),
          ))
          .id();
      let c = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              ContainerQuery {
                  container: None,
                  conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
              },
          ))
          .id();
      app.world_mut().entity_mut(a).add_children(&[b]);
      app.world_mut().entity_mut(b).add_children(&[c]);

      app.update();
      app.update();
      let b_settled = app.world().get::<ResolvedLayout>(b).map(|l| l.size.x);
      assert_eq!(
          b_settled,
          Some(560.0),
          "B should settle to Cqw(80) of A(700) = 560, got {b_settled:?}"
      );
      assert!(
          app.world().get::<ContainerQueryInactive>(c).is_some(),
          "C should be inactive at steady-state (B=560 < 700)"
      );

      // Widen A. Phase 14: the descendant re-run re-resolves B's Cqw and
      // re-evaluates C's rule THIS frame.
      app.world_mut().entity_mut(a).insert(
          Style::default().width_px(1000.0).height_px(400.0).container_size(),
      );
      app.update();
      assert_eq!(
          app.world().get::<ResolvedLayout>(a).map(|l| l.size.x),
          Some(1000.0),
          "A's new resolved width equals the styled width"
      );
      assert_eq!(
          app.world().get::<ResolvedLayout>(b).map(|l| l.size.x),
          Some(800.0),
          "B re-resolves to Cqw(80) of A(1000) = 800 in the same frame (geometric cascade caught up)"
      );
      assert!(
          app.world().get::<ContainerQueryActive>(c).is_some(),
          "C activates in the same frame A resized (B=800 >= 700)"
      );
      assert!(
          app.world().get::<ContainerQueryInactive>(c).is_none(),
          "the inactive marker is removed on activation"
      );
  }
  ```

- [ ] **Step 2: Run the renamed test.**
  ```bash
  cargo test -p buiy_core --test layout_container_queries cq_transitive_cascade_catches_up_in_frame
  ```
  Expected PASS. Confirm the old name is gone: `cargo test -p buiy_core --test layout_container_queries cq_transitive_cascade_is_one_frame_stale` reports `0 tests run` (the name no longer exists).

- [ ] **Step 3: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_container_queries.rs
  git commit -m "test(layout): flip transitive-cascade regression to positive (Phase 14 — spec § 1.5)

Renamed cq_transitive_cascade_is_one_frame_stale ->
cq_transitive_cascade_catches_up_in_frame and flipped the post-resize
assertions: C now activates IN-FRAME after A resizes (B re-resolves its Cqw the
same frame). Closes the Phase-5 documented gap.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Self-review (against the spec)

**Spec coverage** (`container-queries-and-writing-modes.md` § 1.3, § 1.5 + follow-ups.md charter):
- § 1.3 same-frame re-layout (the `CqFlipReRun` analogue extended to the geometric/size cascade) → T4 (re-run skeleton, one-per-frame gate), T5 (re-translate + recompute + re-write), T7 (CQ re-evaluation). Cost ceiling (2× Taffy) → T6 (explicit assertion). ✓
- § 1.5 "transitive cascade … frame N applies A's activation, frame N+1 applies B's" — Phase 14 delivers the **direct** intermediate same-frame (the natural reading of the `A`→`B`→`C` fixture, which today never updates `B` at all) while leaving deeper levels to subsequent frames (D4 keeps the eventual-consistency contract) → T5 (`B` re-resolves), T7 (`C` activates), T8 (the fixture's polarity flip). ✓
- follow-ups.md implementation sketch step 1 (identify query containers with changed `ResolvedLayout`) → T3 (`cq_descendant_invalidate`, D3). ✓
- follow-ups.md step 2 (walk descendants, mark dirty — option b `HashSet<Entity>` resource) → T1 (`ContainerSizeDirty`), T2 (`collect_dirty_descendants`), T3 (populate). D1 documents choosing option (b) over option (a) and why. ✓
- follow-ups.md step 3 (trigger a same-frame re-run analogous to `cq_flip_rerun`) → T4-T7 (`cq_descendant_rerun`, the `cq_flip_rerun` template). ✓
- follow-ups.md "Current test … flips to positive when this lands" → T8 (rename + flip, not delete). ✓
- architecture.md § 3 pipeline (steps are distinct concerns) → D6 + T3/T4 (two new steps after `WriteResolvedLayout`, pipeline-order tests). ✓

**Placeholder scan:** No "TBD" / "similar to Task N" / "add error handling" placeholders. Every code step shows actual code. The only deliberate cross-task forward reference is T5's authoritative resolution note (it explicitly states T5 lands the full re-translate+recompute+re-write so its own test is green, and T6 is a verification-only gate on the cost ceiling) — the full code is given in T5, not deferred. T3/T4's pipeline-order test has a documented fallback (`assert_step_after`) plus the primary instruction to extend the existing full-chain slice — the implementer reads the existing file and picks the matching mechanism; both forms are concrete.

**Type consistency:** `ContainerSizeDirty(pub HashSet<Entity>)` (T1) — field `.0` read in T3 (populate), T5 (drain). `CqDescendantReRunRequested(pub bool)` (T1) — `.0` gated/cleared in T3 (set), T4/T5 (clear). `collect_dirty_descendants(&[Entity], &Query<&Children>) -> HashSet<Entity>` (T2) — called in T3. `cq_descendant_invalidate` (T3) and `cq_descendant_rerun` (T4-T7) wired to `BuiyLayoutStep::CqDescendantInvalidate` / `CqDescendantReRun` (T3/T4 enum variants). Reused existing symbols (`translate_one_entity`, `NodeQueryItem`, `ContainerSnapshot`, `sync_children_pass`, `is_fixed_root`, `resolve_nearest_container`, `evaluate_conditions`, `ContainerQueryActive`/`Inactive`, `LayoutTaffyComputeCount`, `PostTaffyPositionOverrides`) are all cited with their `systems.rs` line numbers and used with their real signatures. No new component or value type is introduced (the hand-off is a private resource), so no `register_type` / facade re-export churn — consistent with `CqReRunRequested`'s crate-internal treatment.
