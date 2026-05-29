# Phase 10: Position::Fixed Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Give `PositionKind::Fixed` its real CSS semantics: a `Fixed` entity is laid out as an absolutely-positioned box whose **containing block is the layout root**, regardless of its nearest positioned ancestor. This is the *only* behavioral difference from `Absolute` (which resolves against its real parent). The Taffy emission (`taffy::Position::Absolute`) is already correct; the missing piece is overriding the Taffy *parent edge* for `Fixed` entities so Taffy's native absolute-positioning algorithm resolves them against the root node's content box instead of their in-flow parent's box.

**Architecture (3 sentences):**
1. **Containing-block override via Taffy re-parenting, in the existing children-sync pass.** Buiy does not compute containing blocks itself (display-and-positioning.md § 2.1): a child's Taffy *parent edge* carries the containing-block relationship, and Taffy's native absolute algorithm resolves an `Absolute` child against its Taffy parent's content box. So to make a `Fixed` entity resolve against the layout root, its Taffy node is **detached from its real parent's `set_children` list and attached to the root node's `set_children` list** — the entity stays a Bevy child of its real parent (size/inheritance/`ResolvedLayout` write are unchanged), but its Taffy box is positioned relative to the root.
2. **A pure `is_fixed_root` predicate decides re-parenting; root identity comes from the existing root-detection rule.** `sync_children_for_entity` gains a `Position` lookup and (a) skips children whose `Position.kind == Fixed` when building a non-root parent's child list, and (b) when building the *root* entity's child list, appends every `Fixed` entity in the tree. The "is this entity the root" test reuses the exact rule already used by `taffy_compute` / `stacking_context` (`ChildOf` absent, or `ChildOf` whose parent is not in `LayoutTree`).
3. **Taffy emission is unchanged; no new component, no new sub-pass, no warn.** `map_position_kind` already maps `Fixed → taffy::Position::Absolute` (translate.rs); the spec § 2.2 contract is satisfied for emission. There is no Fixed warn-once in the codebase to remove (the follow-up sketch's "remove the warn-once" is stale — `PositionKind`'s doc already states "No `warn!` is emitted"). The change is confined to the children-sync pass + its callers and a doc-comment refresh.

**Tech Stack:** Bevy 0.18 (`bevy::prelude::{Children, ChildOf, Node, Query, With}`, `bevy::ecs::entity::Entity`). Taffy 0.10 (`set_children`, native `Position::Absolute` resolution). `std::collections::HashMap` (the existing `LayoutTree::by_entity`). No new external dependency, no new component, no new resource.

**Date:** 2026-05-29
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/display-and-positioning.md`](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md) § 2.1 (containing-block resolution — `Fixed` row), § 2.2 (Taffy mapping — `Fixed` row), § 2 (`Position`/`PositionKind`) + chartered by [`plans/follow-ups.md`](follow-ups.md) "## Layout — `Position::Fixed` implementation".

---

## Prior-art citations (used throughout this plan)

- **blink — positioned layout / containing block** (`docs/prior-art/blink/`): Blink's `LayoutBox::ContainingBlock()` returns the *initial containing block* (the viewport / `LayoutView`) for `position: fixed`, short-circuiting the normal "nearest positioned ancestor" walk used for `position: absolute`. Buiy mirrors this: `Fixed`'s containing block is the layout root, `Absolute`'s is the nearest positioned ancestor — and the *only* difference is which box the offsets resolve against. The known gap (transformed ancestor becomes the containing block for `Fixed` descendants) is NOT in Phase 10 scope (spec § 2.1 "Known gap"); Phase 10 ships the root-as-containing-block case that Blink applies when no transformed ancestor intervenes.
- **servo-stylo — positioning** (`docs/prior-art/servo-stylo/`): Servo computes `position: fixed` against the viewport rectangle in its fragment tree; the in-flow box tree placement is decoupled from the positioned placement. Buiy's decoupling is structural (Taffy parent edge ≠ Bevy `ChildOf`), achieving the same "laid out in flow for inheritance, positioned against the root for geometry" split.
- **taffy — absolute positioning** (`docs/prior-art/taffy/`): Taffy 0.10 has no `position: fixed`; it models only `Relative`/`Absolute`, resolving an `Absolute` node against its **parent node's** padding box. Buiy therefore cannot express "resolve against a different ancestor" through the node's `Style` — it must change the node's *parent in the Taffy tree*. This is why the override lives in `sync_children_for_entity` (the only place that calls `set_children`), not in `translate.rs`.
- **Taffy parent edge = containing block (Buiy)** — `crates/buiy_core/src/layout/systems.rs:1656-1669` (`sync_children_for_entity`): the single call site of `tree.tree.set_children`. The Bevy `Children` order becomes the Taffy child order; this is the only place the Taffy tree topology is written, so it is the only place the `Fixed` re-parent can happen.
- **Buiy doesn't resolve containing blocks itself** — `display-and-positioning.md § 2.1` (lines 127-129): "Buiy does **not** resolve containing blocks itself … the real parent edge carries the relationship." Phase 10's change is exactly to make the `Fixed` entity's Taffy parent edge point at the root rather than its real parent — staying inside this "edge carries the relationship" model rather than introducing a `ContainingBlock` component.
- **Root-detection rule (single source of truth)** — `crates/buiy_core/src/layout/systems.rs:1704-1706` (`taffy_compute`) and `:2595-2598` (`stacking_context`): `parent.map(|p| !tree.by_entity.contains_key(&p.parent())).unwrap_or(true)`. A root is a `Node` with no `ChildOf`, or a `ChildOf` whose parent is not in `LayoutTree`. Phase 10 reuses this exact predicate to find the root entity to re-parent `Fixed` nodes onto.
- **`map_position_kind` already emits Absolute for Fixed** — `crates/buiy_core/src/layout/translate.rs:502-511`: `Absolute | Fixed => taffy::Position::Absolute`. The spec § 2.2 emission contract is already met; Phase 10 changes only the *parent edge*, not the emission. (The translate-side test `translate_position_absolute_emits_absolute_with_inset`, translate.rs:1126, is the precedent for the Fixed emission unit test in T1.)
- **`LayoutTree` shape** — `crates/buiy_core/src/layout/tree.rs` + `systems.rs:178` (`pub by_entity: HashMap<Entity, TaffyNodeId>`), `:1666` (`tree.tree.set_children(parent_id, &child_ids)`). `LayoutTree` is `NonSend`. `sync_children_for_entity` takes `&mut LayoutTree`.
- **Test harness** — `crates/buiy_core/tests/layout_transforms.rs:11-17`: `fn app() { let mut app = App::new(); app.add_plugins(MinimalPlugins); app.add_plugins(CorePlugin); app.add_plugins(LayoutPlugin); app }` (no `TransformPlugin`, no render, headless). Spawn `(Node, Style::default()…)`; children via `commands.spawn(...).add_child(c)` / `add_children(&[...])` / `with_children`; `app.update()` runs the whole pipeline once; assert via `app.world().get::<ResolvedLayout>(e)`. `ResolvedLayout { position: Vec2, size: Vec2 }` is the geometry artifact (`crate::components`). Existing positioning-relevant test files: `tests/layout.rs`, `tests/layout_topology.rs`. The MinimalPlugins viewport falls back to **800×600** when no `Window` exists (`taffy_compute`, systems.rs:1700-1701).
- **`Style` position setters** — `crates/buiy_core/src/layout/style.rs:203-223`: `.position(PositionKind)`, `.relative()`, `.absolute()`, `.inset(Inset)`. Phase 10 adds a `.fixed()` convenience setter mirroring `.absolute()` (T5). `Inset` builder helpers live in `types.rs` (`Inset { top, right, bottom, left }`, each `Sizing`).
- **`PositionKind`/`Position` already registered + re-exported** — `crates/buiy_core/src/layout/mod.rs:110` (`register_type::<Position>()`), `:16` (`pub use components::{… Position …}`), `:32` (`pub use types::{… PositionKind …}`). No registration / re-export change is needed (this is a behavior change to an existing type, not a new type).

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/translate.rs` | T1 (Fixed-emits-Absolute unit test + doc-comment refresh) |
| `crates/buiy_core/src/layout/systems.rs` | T2 (`is_fixed_root` pure predicate + unit tests), T3 (`sync_children_for_entity` re-parent: exclude Fixed from non-root parents), T4 (root attaches Fixed children) |
| `crates/buiy_core/src/layout/components.rs` | T6 (refresh `Position`/doc comment to state Fixed semantics) |
| `crates/buiy_core/src/layout/types.rs` | T6 (refresh `PositionKind` doc comment) |
| `crates/buiy_core/src/layout/style.rs` | T5 (`.fixed()` convenience setter + test) |
| `crates/buiy_core/tests/layout_fixed.rs` | T3, T4, T7 (new integration test file) |

No changes to: `crates/buiy_core/src/layout/mod.rs` (no new types to register/re-export — `Position`/`PositionKind` already wired), `crates/buiy_core/src/layout/pipeline.rs` (no new sub-pass — the change is inside step-1 children-sync), `crates/buiy_core/src/components.rs` (no new render handoff — `Fixed` writes the existing `ResolvedLayout`), `crates/buiy/src/lib.rs`, `crates/buiy_core/src/lib.rs`.

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. The containing-block override is implemented by re-parenting the `Fixed` node onto the layout root in the Taffy tree — NOT by a `ContainingBlock` component or a post-Taffy override

**Decision:** A `Fixed` entity's Taffy node is moved to be a child of the **root** Taffy node (via `set_children`) instead of its real-parent's Taffy node. The entity remains a Bevy child of its real parent (so writing-mode inheritance, the changed-set filter, and `write_resolved_layout` are all unchanged). Taffy's native `Position::Absolute` algorithm then resolves the node against the root's content box = the layout root containing block (spec § 2.1 `Fixed` row).

**Why:** display-and-positioning.md § 2.1 states Buiy "does **not** resolve containing blocks itself … the real parent edge carries the relationship." The faithful way to change a containing block under that model is to change the Taffy parent edge. Taffy 0.10 has no `position: fixed` (prior-art/taffy) and resolves `Absolute` against the node's Taffy parent's padding box (prior-art/taffy) — so the only lever is the parent edge, written exclusively in `sync_children_for_entity` (the single `set_children` call site, systems.rs:1666). This mirrors Blink (`ContainingBlock()` returns the initial containing block for fixed — prior-art/blink) and Servo (positioned placement decoupled from box-tree placement — prior-art/servo-stylo).

**How to apply:** T2 (predicate) + T3 (exclude `Fixed` from non-root parents) + T4 (attach `Fixed` to root).

**Runner-up rejected:** A post-Taffy override sub-pass (like sticky 6a) that reads the `Fixed` node's Taffy position and rewrites it relative to the root. Rejected: it would double-resolve (Taffy already positions the node relative to its in-flow parent, so the override would have to *subtract* the parent's world position and *add* the root's — re-deriving Taffy's absolute algorithm by hand), and it would not give Taffy the correct available-space for percentage insets (`top: 50%` must resolve against the root's size, which only the parent-edge approach gets for free). The re-parent approach lets Taffy do all the math correctly.

### D2. The root is identified by the existing root-detection rule; `Fixed` attaches to the *single* root in `by_entity` iteration order

**Decision:** "Root" = a `Node` entity with `ChildOf` absent, or whose `ChildOf` parent is not in `LayoutTree` (the rule already used by `taffy_compute` systems.rs:1704-1706 and `stacking_context` :2595-2598). `Fixed` entities are appended to the root's Taffy child list. If multiple roots exist (not expected under `MinimalPlugins` — the harness spawns one tree), `Fixed` entities attach to the **first root encountered in the `Node` query iteration**; this matches the single-global-tree assumption already baked into `stacking_context`'s top-layer escape (Phase 9 D2) and `taffy_compute`'s primary-window read.

**Why:** Reusing the one root-detection rule keeps a single source of truth (no third definition of "root" to drift). The single-root attach matches every other place in `buiy_core` that assumes one layout tree / primary window (`taffy_compute` `windows.iter().next()`, `stacking_context` `roots.first()`). Per-window / multi-root fixed targeting is gated on `buiy-window-and-surface-design` (unbuilt), exactly like the Phase-6 cross-window-anchor and Phase-9 per-window-top-layer follow-ups.

**How to apply:** T4 computes the root entity once (first matching the rule), looks up its Taffy node, and appends the Fixed node ids.

**Runner-up rejected:** Attach `Fixed` to *every* root. Rejected: a `Fixed` node can have exactly one Taffy parent (`set_children` is exclusive); attaching to multiple roots is impossible and the multi-root case has no tested meaning yet.

### D3. Re-parenting is decided per-entity in the children-sync pass; no per-entity stored `is_fixed_root` flag is persisted

**Decision:** The "this entity re-parents to root" decision (`is_fixed_root`) is a **pure function of the entity's `Position.kind`** evaluated each frame inside the children-sync pass — `is_fixed_root(position) == matches!(position.kind, PositionKind::Fixed)`. No marker component or stored field is added (the follow-up sketch's "private `is_fixed_root` flag on the entity's translation state" is realized as this pure predicate, not a stored component — there is no persistent "translation state" struct to hang a field on; the decision is cheap and recomputed each frame).

**Why:** The decision depends only on `Position.kind`, which is already in the changed-set filter (`Changed<Position>` triggers re-sync) and already queried by `sync_styles`. A stored flag would need its own insert/remove lifecycle (when `Position.kind` flips Fixed↔Absolute) and a registration — pure recomputation is simpler, has no stale-state risk, and matches the `forms_stacking_context` pure-predicate precedent (Phase 9 D5). Cost is O(1) per child during a pass that already iterates children.

**How to apply:** T2 defines `is_fixed_root(&Position) -> bool`; T3/T4 call it.

**Runner-up rejected:** A persisted `#[derive(Component)] struct FixedRoot;` marker inserted/removed by an observer. Rejected: adds a component + registration + observer lifecycle for a value trivially derived from `Position.kind` every frame; stale-marker risk on rapid Fixed↔Absolute toggles.

### D4. `sync_children_for_entity` gains a `Position` lookup parameter; the children-sync pass passes a borrow of the Fixed set

**Decision:** `sync_children_for_entity` is extended to (a) take a way to test a child's `is_fixed_root` so it can **exclude** `Fixed` children when building a non-root parent's list, and (b) the root-attach of `Fixed` children is done by computing, once in `sync_styles`, the set of all `Fixed` entity Taffy node-ids, and appending them to the root entity's child list. Concretely: `sync_styles`'s second pass (the children-sync loop, systems.rs:1547-1549) is restructured to (1) build `fixed_node_ids: Vec<TaffyNodeId>` from all entities whose `Position.kind == Fixed`, (2) for each entity call `sync_children_for_entity` passing a `is_fixed: &dyn Fn(Entity) -> bool` closure (or a `&HashSet<Entity>` of Fixed entities) so non-root parents drop Fixed children, and (3) when the entity is the root, append `fixed_node_ids` to its child list.

**Why:** `sync_children_for_entity` is the only `set_children` caller; both the exclusion and the root-attach must happen there to keep the Taffy topology consistent in one place. Building the Fixed set once (rather than re-querying per parent) keeps the pass O(N). Passing a borrow (`&HashSet<Entity>`) rather than a new query keeps the helper testable in isolation (T2/T3 unit tests construct the set directly).

**How to apply:** T3 changes the signature + exclusion; T4 adds the root-attach. There are **two** callers of `sync_children_for_entity` — `sync_styles` (systems.rs:1548) and `cq_flip_rerun` (systems.rs:2305) — both with the identical `nodes` query shape. Both must build the Fixed set + run the root-attach. To prevent drift, extract a single `sync_children_pass` helper (builds the Fixed set + node-ids from a `nodes` iterator, runs the per-entity exclusion loop, then the root-attach) and call it from both systems. T3 wires `sync_styles`; T4's root-attach lands in the shared helper so `cq_flip_rerun` gets it for free.

**Runner-up rejected:** A separate post-`sync_styles` system that re-parents Fixed nodes. Rejected: it would run after `set_children` has already placed Fixed under its real parent, requiring a remove-then-re-add; doing it inline in the one children-sync pass is atomic and avoids an extra tree mutation.

### D5. A `.fixed()` convenience setter on `Style`, mirroring `.absolute()`

**Decision:** Add `pub fn fixed(mut self) -> Self { self.position.kind = PositionKind::Fixed; self }` to `Style`, next to `.absolute()` (style.rs:215). Authors can already write `.position(PositionKind::Fixed)`; `.fixed()` is the ergonomic parity with `.relative()`/`.absolute()`.

**Why:** `Position::{Relative, Absolute}` each have a no-arg convenience setter; `Fixed` becoming a real, first-class position warrants the same ergonomics. Trivial, additive, no behavior risk.

**How to apply:** T5.

**Runner-up rejected:** No convenience setter (force `.position(PositionKind::Fixed)`). Rejected: inconsistent with `.relative()`/`.absolute()` now that `Fixed` is fully implemented.

### D6. Doc comments on `Position`/`PositionKind` updated to state the shipped Fixed semantics; the stale "Phase 8 wires fixed" / "fall-back-to-Absolute stub" wording is removed

**Decision:** `components.rs` `Position` doc (lines 94-101) and `types.rs` `PositionKind` doc (lines 214-219) are updated to say `Fixed` is implemented (containing block = layout root via Taffy re-parenting), replacing "Phase 8 wires its real (viewport / transformed-ancestor) semantics" and "remains a fall-back-to-`Absolute` stub pending Phase 8". The transformed-ancestor-as-containing-block case stays noted as the remaining known gap (spec § 2.1).

**Why:** The current doc comments are stale (they reference Phase 8, which shipped transforms but explicitly deferred Fixed — Phase 7 D13). Code doc must match shipped behavior. This is a code-doc refresh (the `### Task` tasks are all code/TDD; the spec/README/follow-ups doc flips happen in a separate stage, NOT in these tasks).

**How to apply:** T6 (doc-comment edits only; verified by the existing reflect/registration tests still passing — no behavior change, so the gate is the verification).

**Runner-up rejected:** Leave the stale comments. Rejected: violates "code doc matches behavior"; a future reader would think Fixed is still a stub.

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

### Task 1: Confirm + lock the `Fixed → taffy::Position::Absolute` emission (translate.rs)

**Spec:** § 2.2 (Fixed row: `taffy::Position::Absolute`).

This emission is already implemented (translate.rs:509). T1 adds a regression test that locks it and refreshes the misleading translate-side doc comment (it says "Phase 7 / Phase 8 wire the real semantics" — Phase 10 *is* that wiring for Fixed).

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` (add a unit test next to `translate_position_absolute_emits_absolute_with_inset` at ~:1126; refresh the `map_position_kind` doc comment at :502-506)

- [ ] **Step 1: Failing test.** Add to `translate.rs::mod tests` (after `translate_position_absolute_emits_absolute_with_inset`, ~:1161). Mirror that test's `StyleView` construction exactly (every field spelled out):
  ```rust
  #[test]
  fn translate_position_fixed_emits_absolute_with_inset() {
      // Fixed resolves against the layout root (spec § 2.1); at the Taffy
      // emission layer it is Absolute (spec § 2.2). The root containing
      // block is achieved by re-parenting in sync_children (not here).
      let display = Display::default();
      let bm = BoxModel::default();
      let position = Position {
          kind: PositionKind::Fixed,
          inset: crate::layout::types::Inset {
              top: Sizing::Length(Length::Px(5.0)),
              left: Sizing::Length(Length::Px(7.0)),
              ..Default::default()
          },
      };
      let flex = FlexParams::default();
      let overflow = Overflow::default();
      let scroll = Scroll::default();
      let grid_params = GridParams::default();
      let writing_mode_resolved = WritingModeResolved::default();
      let taffy = style_to_taffy(StyleView {
          display: &display,
          box_model: &bm,
          containment: &Containment::default(),
          position: &position,
          flex_params: &flex,
          flex_item: None,
          overflow: &overflow,
          scroll: &scroll,
          grid_params: &grid_params,
          grid_item: None,
          parent_areas: None,
          writing_mode_resolved: &writing_mode_resolved,
          nearest_container: None,
          viewport_size: bevy::math::Vec2::ZERO,
      });
      assert_eq!(taffy.position, taffy::Position::Absolute);
      assert_eq!(taffy.inset.top, taffy::LengthPercentageAuto::length(5.0));
      assert_eq!(taffy.inset.left, taffy::LengthPercentageAuto::length(7.0));
  }
  ```

- [ ] **Step 2: Run it — expected PASS-already (emission exists), so refactor the test into a true RED first.** Because `map_position_kind` already emits Absolute for Fixed, this test will PASS immediately, which is not a valid RED. To get a real RED→GREEN cycle that proves the test exercises the right code path, temporarily break the emission, confirm RED, then restore:
  ```bash
  cargo test -p buiy_core translate_position_fixed_emits_absolute_with_inset
  ```
  If it PASSES immediately: edit `map_position_kind` (translate.rs:509) to split `Fixed` onto the `Relative` arm (`Static | Relative | Sticky | Fixed => taffy::Position::Relative,` and `Absolute => taffy::Position::Absolute,`), re-run the command, and confirm it now FAILS (`assert_eq!(taffy.position, ...)` — got `Relative`, expected `Absolute`). This proves the test is wired to the emission. Then proceed to Step 3 to restore.

- [ ] **Step 3: Restore the emission + refresh the doc comment.** Set `map_position_kind` back to its correct shipped form and update its doc comment (translate.rs:502-506) to reflect Phase 10:
  ```rust
  fn map_position_kind(k: PositionKind) -> taffy::Position {
      use PositionKind::*;
      // Static / Relative / Sticky map to Taffy Relative (Sticky offsets
      // are applied in sub-pass 6a). Absolute and Fixed both emit
      // taffy::Position::Absolute (spec § 2.2). The Absolute-vs-Fixed
      // difference is the *containing block*, which is the Taffy parent
      // edge, not the emitted Position: `sync_children_for_entity`
      // re-parents Fixed nodes onto the layout root so Taffy resolves
      // them against the root's content box (spec § 2.1 Fixed row).
      match k {
          Static | Relative | Sticky => taffy::Position::Relative,
          Absolute | Fixed => taffy::Position::Absolute,
      }
  }
  ```

- [ ] **Step 4: Run it — expected PASS.**
  ```bash
  cargo test -p buiy_core translate_position_fixed_emits_absolute_with_inset
  ```
  Expected PASS.

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/translate.rs
  git commit -m "test(layout): lock Fixed->taffy::Absolute emission + refresh map_position_kind doc (Phase 10 — spec § 2.2)

Fixed already emits taffy::Position::Absolute; add a regression test and
refresh the stale Phase-7/8 doc comment to describe the Phase-10
containing-block-via-reparent model. No behavior change in this task.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 2: `is_fixed_root` pure predicate (systems.rs)

**Spec:** § 2.1 (Fixed row), D1, D3.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the pure predicate + unit tests near `forms_stacking_context` / the other pure helpers)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn is_fixed_root_true_for_fixed() {
      let p = Position { kind: PositionKind::Fixed, ..Default::default() };
      assert!(is_fixed_root(&p));
  }

  #[test]
  fn is_fixed_root_false_for_absolute() {
      let p = Position { kind: PositionKind::Absolute, ..Default::default() };
      assert!(!is_fixed_root(&p));
  }

  #[test]
  fn is_fixed_root_false_for_static_relative_sticky() {
      for k in [PositionKind::Static, PositionKind::Relative, PositionKind::Sticky] {
          let p = Position { kind: k, ..Default::default() };
          assert!(!is_fixed_root(&p), "{k:?} must not re-parent to root");
      }
  }
  ```
  **Implementer note:** confirm `Position` + `PositionKind` are imported into the `systems.rs` test module (mirror how `forms_stacking_context`'s tests import `crate::layout::types::PositionKind` and `crate::layout::components::…`; add `Position` to the components import).
  Run: `cargo test -p buiy_core is_fixed_root` — expected FAIL (predicate doesn't exist).

- [ ] **Step 2: Add the predicate to `systems.rs`.** Near the other pure layout helpers (e.g. just above `sync_children_for_entity` at :1656):
  ```rust
  /// Whether this entity's box re-parents to the layout root in the Taffy
  /// tree so its containing block is the root (spec § 2.1 `Fixed` row).
  /// Pure function of `Position.kind` (D3): `Fixed` re-parents, everything
  /// else keeps its in-flow Taffy parent. `Absolute` does NOT re-parent —
  /// it resolves against its nearest positioned ancestor (= its real
  /// Taffy parent), which is the only behavioral difference from `Fixed`.
  pub(super) fn is_fixed_root(position: &Position) -> bool {
      matches!(position.kind, PositionKind::Fixed)
  }
  ```

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core is_fixed_root
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): is_fixed_root pure predicate (Phase 10 — spec § 2.1, D3)

Pure Position.kind == Fixed test deciding Taffy-root re-parenting. Wired
into sync_children_for_entity in T3/T4.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 3: Exclude `Fixed` children from their real parent's Taffy child list

**Spec:** § 2.1 (Fixed row), D1, D4.

After this task, a `Fixed` entity is removed from its in-flow parent's Taffy children (so Taffy no longer positions it relative to that parent) but is NOT YET attached to the root — that is T4. The in-between state means a lone `Fixed` child temporarily has no Taffy parent; T3's integration test asserts only the *exclusion* fact (the Fixed node is absent from the parent's child list / the parent's other children are unaffected), and T4 completes the attach. To keep the tree valid between T3 and T4, T3 attaches `Fixed` nodes to the root in the same change — but the **root-attach assertion** is deferred to T4. (Practically: implement the full re-parent in T3+T4 as two commits, T3 = exclusion path + Fixed-set build, T4 = root append + its assertion. If the implementer prefers, T3 and T4 can be a single commit — but keep the two test blocks.)

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (`sync_children_for_entity` signature + exclusion; build the Fixed set in `sync_styles`'s children-sync pass; update any other caller)
- Create: `crates/buiy_core/tests/layout_fixed.rs` (exclusion + sibling-unaffected integration tests)

- [ ] **Step 1: Failing test.** Create `crates/buiy_core/tests/layout_fixed.rs`:
  ```rust
  //! Phase 10 — Position::Fixed: containing block = layout root.
  //!
  //! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.1, § 2.2.

  use bevy::prelude::*;
  use buiy_core::{
      CorePlugin, Node, ResolvedLayout,
      layout::{Inset, LayoutPlugin, Length, PositionKind, Sizing, Style},
  };

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app
  }

  fn inset_top_left(top: f32, left: f32) -> Inset {
      Inset {
          top: Sizing::Length(Length::Px(top)),
          left: Sizing::Length(Length::Px(left)),
          ..Default::default()
      }
  }

  // A fixed child does not displace its in-flow sibling: the sibling lays
  // out as if the fixed child were not a flow participant of the parent
  // (the fixed child is removed from the parent's Taffy child list — D1/D4).
  #[test]
  fn fixed_child_does_not_affect_in_flow_sibling() {
      let mut app = app();
      // Parent is a column flex with two children: one in-flow (height 40),
      // one fixed (height 40). The in-flow sibling must sit at y=0 inside
      // the parent regardless of the fixed child.
      let in_flow = app
          .world_mut()
          .spawn((
              Node,
              Style::default().height(Length::Px(40.0)).width(Length::Px(100.0)),
          ))
          .id();
      let fixed = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Fixed)
                  .inset(inset_top_left(0.0, 0.0))
                  .height(Length::Px(40.0))
                  .width(Length::Px(100.0)),
          ))
          .id();
      let parent = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .display(buiy_core::layout::Display::flex_column())
                  .width(Length::Px(100.0))
                  .height(Length::Px(200.0)),
          ))
          .add_children(&[in_flow, fixed])
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(800.0)).height(Length::Px(600.0))))
          .add_child(parent)
          .id();
      app.update();

      // The in-flow sibling is the ONLY flow child of `parent`, so it sits
      // at the parent's content origin (relative position y == 0). If the
      // fixed child were still in the parent's flex flow, the column would
      // place the in-flow sibling after/around it and break this.
      let in_flow_layout = app
          .world()
          .get::<ResolvedLayout>(in_flow)
          .expect("in-flow sibling has ResolvedLayout");
      assert_eq!(
          in_flow_layout.position.y, in_flow_rel_y_for_parent_origin(&app, parent),
          "in-flow sibling sits at the parent's content origin; fixed child is out of flow",
      );
      assert_eq!(in_flow_layout.size, Vec2::new(100.0, 40.0));
  }

  // Helper: the parent's own resolved Y (Taffy positions are parent-relative
  // for in-flow children, so the in-flow sibling's resolved Y equals the
  // parent's resolved Y when it is the first/only flow child at content
  // origin with zero padding/border).
  fn in_flow_rel_y_for_parent_origin(app: &App, parent: Entity) -> f32 {
      app.world().get::<ResolvedLayout>(parent).unwrap().position.y
  }
  ```
  **Implementer note:** `ResolvedLayout.position` is the **resolved (post-Taffy, post-override) absolute-in-tree** position (Taffy `location` is parent-relative, written through as-is by `write_resolved_layout` — confirm by reading `tests/layout_topology.rs:48-75`, which asserts child-inside-parent geometry). If `ResolvedLayout.position` turns out to be world/absolute rather than parent-relative in this harness, adapt the assertion to compare the in-flow sibling's Y against the parent's content-box origin Y accordingly — the load-bearing assertion is "the in-flow sibling is NOT pushed down by a 40px fixed sibling," i.e. its Y equals what it would be with the fixed child absent. Confirm `Style` has `.display(Display)`, `.width(Length)`, `.height(Length)` setters (style.rs); if `.width`/`.height` take `Sizing` instead of `Length`, wrap with `Sizing::Length(Length::Px(..))`.
  Run: `cargo test -p buiy_core --test layout_fixed fixed_child_does_not_affect_in_flow_sibling` — expected FAIL (Fixed still in the parent's flex flow, so the column displaces or stretches the sibling — or, depending on Taffy's absolute handling, the sibling is fine but the test proves the exclusion path runs).

- [ ] **Step 2: Build the Fixed set + change `sync_children_for_entity` to exclude Fixed children.**
  - **2a.** Change the helper signature to accept the set of entities that re-parent to root, and skip them when building a parent's child list:
    ```rust
    fn sync_children_for_entity(
        entity: Entity,
        children: Option<&Children>,
        fixed_set: &std::collections::HashSet<Entity>,
        tree: &mut LayoutTree,
    ) {
        let parent_id = match tree.by_entity.get(&entity).copied() {
            Some(id) => id,
            None => return,
        };
        let child_ids: Vec<TaffyNodeId> = children
            .into_iter()
            .flatten()
            // Fixed children re-parent to the layout root (spec § 2.1) —
            // exclude them from their in-flow parent's Taffy child list.
            // The root-attach happens in `sync_styles` (T4).
            .filter(|c| !fixed_set.contains(c))
            .filter_map(|c| tree.by_entity.get(c).copied())
            .collect();
        if let Err(err) = tree.tree.set_children(parent_id, &child_ids) {
            warn!(?entity, ?err, "buiy: layout set_children failed");
        }
    }
    ```
  - **2b.** In `sync_styles`, build the Fixed set once before the children-sync loop and pass it in. Replace the loop at systems.rs:1546-1549:
    ```rust
    // Entities whose box re-parents to the layout root in the Taffy tree
    // (Position::Fixed — spec § 2.1). Built once; consumed by the
    // children-sync pass to (a) exclude Fixed from their in-flow parent's
    // child list and (b) attach them to the root's child list (T4).
    let fixed_set: HashSet<Entity> = nodes
        .iter()
        .filter(|item| is_fixed_root(item.4)) // item.4 == &Position (NodeQueryItem)
        .map(|item| item.0)
        .collect();

    // Sync child relationships for each Buiy entity.
    for (entity, .., children, _parent) in nodes.iter() {
        sync_children_for_entity(entity, children, &fixed_set, tree);
    }
    ```
    **Implementer note:** `item.4` is the `&Position` slot in `NodeQueryItem` (Entity=0, Display=1, BoxModel=2, Containment=3, Position=4 — confirm against the `NodeQueryItem` tuple at systems.rs:1556-1571 before relying on the index; prefer a destructure `let position = item.4;` with a comment, or destructure the tuple by name). `HashSet` is already imported in `systems.rs` (used by `LayoutWarnedOnceSession`); confirm and add `use std::collections::HashSet;` if not.
  - **2c.** Update the other caller of `sync_children_for_entity`. There are exactly two callers: `sync_styles` (systems.rs:1548) and `cq_flip_rerun` (systems.rs:2305 — same `nodes` query shape, so `item.4 == &Position` holds identically). `cq_flip_rerun` re-runs the children-sync after a same-frame container-query activation flip, and a `Fixed` entity *can* be in the re-run set, so build the real Fixed set there too (the same `nodes.iter().filter(is_fixed_root)…` build) and pass it — do NOT pass an empty set. The root-attach block (T4) must also be added to `cq_flip_rerun` after its children-sync loop, or extracted into a shared helper called by both systems. Prefer extracting a `sync_children_pass(nodes-iter, tree)` helper that builds the Fixed set + node-ids, runs the per-entity loop, and does the root-attach, so both `sync_styles` and `cq_flip_rerun` call one function and cannot drift. Confirm via `grep -n "sync_children_for_entity" crates/buiy_core/src/layout/systems.rs`.

- [ ] **Step 3: Run the test.** It may still not fully pass until T4 attaches Fixed to the root (a Fixed node with no Taffy parent is not laid out, so it gets no fresh `ResolvedLayout` — but the *in-flow sibling* assertion this test makes should now pass because the Fixed child is out of the parent's flow). Run:
  ```bash
  cargo test -p buiy_core --test layout_fixed fixed_child_does_not_affect_in_flow_sibling
  ```
  Expected PASS (the in-flow sibling is no longer displaced by the fixed child). If the assertion about the *fixed* entity's own position is needed, it lands in T4.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_fixed.rs
  git commit -m "feat(layout): exclude Fixed children from in-flow parent Taffy list (Phase 10 — spec § 2.1, D4)

sync_children_for_entity now drops Position::Fixed children from their real
parent's set_children list so Taffy stops positioning them in-flow. Root-attach
follows in T4. Fixed set built once per frame in sync_styles.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 4: Attach `Fixed` children to the layout root + assert root-relative positioning

**Spec:** § 2.1 (Fixed row), D1, D2, D4.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (root-attach in the children-sync pass)
- Modify: `crates/buiy_core/tests/layout_fixed.rs` (Fixed-resolves-against-root integration tests)

- [ ] **Step 1: Failing tests.** Append to `crates/buiy_core/tests/layout_fixed.rs`:
  ```rust
  // A fixed entity nested deep under a positioned ancestor resolves its
  // inset against the LAYOUT ROOT, not the nearest positioned ancestor.
  // This is the sole behavioral difference from Absolute (spec § 2.1).
  #[test]
  fn fixed_resolves_against_root_not_nearest_ancestor() {
      let mut app = app();
      // root (800x600) > offset_parent (relative, positioned at 100,100,
      // sized 200x200) > fixed (top:0,left:0, size 50x50).
      // Absolute would place `fixed` at the offset_parent origin (100,100);
      // Fixed must place it at the ROOT origin (0,0).
      let fixed = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Fixed)
                  .inset(inset_top_left(0.0, 0.0))
                  .width(Length::Px(50.0))
                  .height(Length::Px(50.0)),
          ))
          .id();
      let offset_parent = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Relative)
                  .inset(inset_top_left(100.0, 100.0))
                  .width(Length::Px(200.0))
                  .height(Length::Px(200.0)),
          ))
          .add_child(fixed)
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(800.0)).height(Length::Px(600.0))))
          .add_child(offset_parent)
          .id();
      app.update();

      let fixed_layout = app
          .world()
          .get::<ResolvedLayout>(fixed)
          .expect("fixed entity has ResolvedLayout (laid out under the root)");
      assert_eq!(
          fixed_layout.position,
          Vec2::new(0.0, 0.0),
          "fixed resolves top:0/left:0 against the ROOT origin (0,0), not the \
           offset parent at (100,100)",
      );
      assert_eq!(fixed_layout.size, Vec2::new(50.0, 50.0));
  }

  // A fixed entity with a percentage inset resolves the percentage against
  // the ROOT's size (800x600), proving Taffy got the root as the available
  // space (the parent-edge re-parent gives this for free — D1 runner-up note).
  #[test]
  fn fixed_percent_inset_resolves_against_root_size() {
      let mut app = app();
      let fixed = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Fixed)
                  .inset(Inset {
                      left: Sizing::Length(Length::Percent(50.0)),
                      top: Sizing::Length(Length::Percent(50.0)),
                      ..Default::default()
                  })
                  .width(Length::Px(10.0))
                  .height(Length::Px(10.0)),
          ))
          .id();
      let parent = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(200.0)).height(Length::Px(200.0))))
          .add_child(fixed)
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(800.0)).height(Length::Px(600.0))))
          .add_child(parent)
          .id();
      app.update();

      let fixed_layout = app.world().get::<ResolvedLayout>(fixed).unwrap();
      // 50% of the ROOT (800x600) = (400, 300), NOT 50% of the 200x200 parent.
      assert_eq!(
          fixed_layout.position,
          Vec2::new(400.0, 300.0),
          "percent inset resolves against the root size (800x600), not the parent (200x200)",
      );
  }
  ```
  **Implementer note (CRITICAL — verify the root-relative coordinate model):** Taffy's `compute_layout` is called from each root (`taffy_compute`, systems.rs:1710-1717), and `tree.layout(id).location` is **parent-relative**. For a node re-parented onto the root, its `location` is therefore relative to the root's content box — which for a root at tree-origin equals the absolute/world coordinate. `write_resolved_layout` writes `location` straight into `ResolvedLayout.position` (systems.rs:1760). So a Fixed node re-parented to the root with `top:0/left:0` gets `location == (0,0)`. **Confirm this against `tests/layout_topology.rs`** (which asserts the parent-relative model) before trusting the exact `(0,0)` / `(400,300)` numbers; if `ResolvedLayout.position` is composed into world coordinates somewhere (it is not, per the Phase-8 follow-up note "Render reads `ResolvedLayout` directly"), adjust the expected values to the root-relative equivalents. The load-bearing facts are: (1) Fixed's position is independent of the offset parent's (100,100); (2) percent insets use the root's (800,600), not the parent's (200,200).

- [ ] **Step 2: Add the root-attach to the children-sync pass.** Extend the `sync_styles` children-sync section (the block edited in T3 step 2b). After building `fixed_set` and the per-entity `sync_children_for_entity` loop, attach the Fixed nodes to the root:
  ```rust
  // Attach Fixed nodes to the layout ROOT's Taffy child list so Taffy
  // resolves them against the root's content box (spec § 2.1 Fixed row;
  // D1/D2). Root = the existing root-detection rule (no ChildOf, or a
  // ChildOf whose parent is not in LayoutTree). Single global tree: the
  // first matching root wins (D2). The root's own in-flow children were
  // already set above (Fixed excluded); we re-set with the union so we
  // do NOT clobber them.
  if !fixed_set.is_empty() {
      // Find the root entity + its current (Fixed-excluded) in-flow child
      // ids, then append the Fixed node ids and re-set.
      for (entity, .., children, parent) in nodes.iter() {
          let is_root = parent
              .map(|p| !tree.by_entity.contains_key(&p.parent()))
              .unwrap_or(true);
          if !is_root {
              continue;
          }
          let Some(root_id) = tree.by_entity.get(&entity).copied() else {
              continue;
          };
          // In-flow children of the root, Fixed excluded (mirror the
          // helper's filter so we reproduce exactly what it set).
          let mut child_ids: Vec<TaffyNodeId> = children
              .into_iter()
              .flatten()
              .filter(|c| !fixed_set.contains(c))
              .filter_map(|c| tree.by_entity.get(c).copied())
              .collect();
          // Append every Fixed node (in nodes-iteration order, D2).
          for (e, ..) in nodes.iter() {
              if fixed_set.contains(&e)
                  && let Some(fid) = tree.by_entity.get(&e).copied()
              {
                  child_ids.push(fid);
              }
          }
          if let Err(err) = tree.tree.set_children(root_id, &child_ids) {
              warn!(?entity, ?err, "buiy: layout set_children (fixed root attach) failed");
          }
          break; // single global tree — attach to the first root only (D2).
      }
  }
  ```
  **Implementer note:** this re-sets the root's children with the union of (in-flow non-Fixed) ∪ (all Fixed) so the prior `sync_children_for_entity` call for the root is superseded in the same pass — no double-add, Fixed appears exactly once. The `break` enforces the single-root attach (D2). A Fixed entity that *is itself the root* (`ChildOf` absent + `Position::Fixed`) is degenerate; it ends up in its own child list via the append — harmless (Taffy ignores a node parenting itself? confirm: if Taffy errors on self-parent, guard with `if e != entity`). Add `if e != entity` to the append loop to be safe. Prefer collecting the Fixed node ids once into a `Vec<TaffyNodeId>` before the root loop (mirrors D4) rather than re-iterating `nodes` inside the root loop — refactor to build `fixed_node_ids: Vec<TaffyNodeId>` alongside `fixed_set` in T3's set-build step and reuse it here.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_fixed fixed_resolves_against_root_not_nearest_ancestor fixed_percent_inset_resolves_against_root_size
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_fixed.rs
  git commit -m "feat(layout): attach Fixed nodes to layout root (Phase 10 — spec § 2.1, D1/D2)

Fixed entities are appended to the root Taffy node's child list so Taffy's
native absolute algorithm resolves them (incl. percent insets) against the
root content box — the sole behavioral difference from Absolute. Single
global root (D2).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 5: `.fixed()` convenience setter on `Style`

**Spec:** § 2 (`PositionKind::Fixed`), D5.

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` (add `.fixed()` next to `.absolute()` + test)

- [ ] **Step 1: Failing test.** Add to `style.rs::mod tests`:
  ```rust
  #[test]
  fn style_fixed_setter_sets_position_kind() {
      let s = Style::default().fixed();
      assert_eq!(s.position.kind, PositionKind::Fixed);
  }
  ```
  **Implementer note:** confirm `PositionKind` is imported in the `style.rs` test module (the existing position-setter tests use it; mirror them).
  Run: `cargo test -p buiy_core style_fixed_setter_sets_position_kind` — expected FAIL (`.fixed()` doesn't exist).

- [ ] **Step 2: Add the setter.** In `style.rs`, after `.absolute()` (:215-218):
  ```rust
  pub fn fixed(mut self) -> Self {
      self.position.kind = PositionKind::Fixed;
      self
  }
  ```

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core style_fixed_setter_sets_position_kind
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/style.rs
  git commit -m "feat(layout): Style::fixed() convenience setter (Phase 10 — D5)

Parity with .relative()/.absolute() now that Fixed is fully implemented.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 6: Refresh `Position` / `PositionKind` doc comments to the shipped Fixed semantics

**Spec:** § 2, § 2.1, D6.

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (`Position` doc, lines 94-101)
- Modify: `crates/buiy_core/src/layout/types.rs` (`PositionKind` doc, lines 214-219)

This task has no new runtime behavior — it is a code-doc refresh. The "test" is the gate (the doc tests / `cargo doc -D warnings` must stay green and the existing reflect/registration tests pass). To keep a TDD shape, add one assertion that pins the *behavior the doc now claims* (Fixed is registered + reflectable as before — i.e. nothing regressed), then edit the docs.

- [ ] **Step 1: Pinning test (guards against accidental behavior change while editing docs).** Add to `types.rs::mod tests` (or confirm an equivalent exists):
  ```rust
  #[test]
  fn position_kind_fixed_variant_is_distinct() {
      // Doc-refresh guard: Fixed is a real, distinct variant (not aliased
      // to Absolute) — the doc now claims Fixed has root-containing-block
      // semantics distinct from Absolute.
      assert_ne!(PositionKind::Fixed, PositionKind::Absolute);
      assert_eq!(PositionKind::Fixed, PositionKind::Fixed);
  }
  ```
  Run: `cargo test -p buiy_core position_kind_fixed_variant_is_distinct` — expected PASS immediately (variant already exists). This is a guard, not a RED; its role is to lock the invariant the doc now asserts.

- [ ] **Step 2: Edit `components.rs` `Position` doc** (replace lines 94-101's stale Phase-7/8 wording):
  ```rust
  /// `position` + `inset`. `Static`, `Relative`, `Absolute`, and `Fixed`
  /// are fully implemented; `Sticky` is a post-Taffy overlay (sub-pass 6a,
  /// `sticky_offset`). `Absolute` resolves against its nearest positioned
  /// ancestor; `Fixed` resolves against the layout root (its Taffy node is
  /// re-parented onto the root in `sync_children_for_entity` so Taffy's
  /// native absolute algorithm uses the root's content box — spec § 2.1).
  /// The remaining known gap is the transformed-ancestor-as-containing-block
  /// case for `Fixed` descendants (spec § 2.1 "Known gap"), which is not yet
  /// modeled.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.
  ```

- [ ] **Step 3: Edit `types.rs` `PositionKind` doc** (replace lines 214-219's stale wording):
  ```rust
  /// Position kind. `Static`, `Relative`, and `Absolute` pass through to
  /// Taffy directly (`Static`/`Relative` → `taffy::Position::Relative`,
  /// `Absolute` → `taffy::Position::Absolute` resolved against the nearest
  /// positioned ancestor). `Fixed` also emits `taffy::Position::Absolute`
  /// but its node is re-parented onto the layout root in the children-sync
  /// pass, so it resolves against the root content box (spec § 2.1 Fixed
  /// row). `Sticky` maps to `Relative` for the Taffy pass and gets its
  /// displacement from sub-pass 6a (`sticky_offset`). No `warn!` is emitted
  /// for any translation.
  ```

- [ ] **Step 4: Run the gate (doc warnings + tests).**
  ```bash
  cargo test -p buiy_core position_kind_fixed_variant_is_distinct
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Expected PASS / clean.

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/components.rs crates/buiy_core/src/layout/types.rs
  git commit -m "docs(layout): refresh Position/PositionKind comments for shipped Fixed (Phase 10 — D6)

Replace stale 'Phase 8 wires fixed' / 'fall-back-to-Absolute stub' wording
with the implemented root-containing-block semantics. No behavior change.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 7: Integration coverage — Fixed re-targets on Position flip + Display::None Fixed + sticky/anchor non-interaction

**Spec:** § 2.1 (Fixed row), § 2 (test surface), D2, D3.

Rounds out the spec § 4 test-surface intent for positioning with the edge cases the re-parent introduces.

**Files:**
- Modify: `crates/buiy_core/tests/layout_fixed.rs` (add the edge-case fixtures)

- [ ] **Step 1: Add the tests.** Append to `crates/buiy_core/tests/layout_fixed.rs`:
  ```rust
  // Flipping Position::Fixed -> Absolute on a deep descendant re-parents it
  // back to its in-flow parent in the same/next frame (D3 — decision is a
  // pure per-frame function of Position.kind, recomputed; no stale flag).
  #[test]
  fn flipping_fixed_to_absolute_re_homes_under_offset_parent() {
      let mut app = app();
      let child = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Fixed)
                  .inset(inset_top_left(0.0, 0.0))
                  .width(Length::Px(20.0))
                  .height(Length::Px(20.0)),
          ))
          .id();
      let offset_parent = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Relative)
                  .inset(inset_top_left(100.0, 100.0))
                  .width(Length::Px(200.0))
                  .height(Length::Px(200.0)),
          ))
          .add_child(child)
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(800.0)).height(Length::Px(600.0))))
          .add_child(offset_parent)
          .id();
      app.update();
      // While Fixed: resolves against root (0,0).
      assert_eq!(app.world().get::<ResolvedLayout>(child).unwrap().position, Vec2::new(0.0, 0.0));

      // Flip to Absolute: now resolves against the offset parent (100,100).
      app.world_mut()
          .entity_mut(child)
          .insert(
              Position {
                  kind: PositionKind::Absolute,
                  inset: inset_top_left(0.0, 0.0),
              },
          );
      app.update();
      assert_eq!(
          app.world().get::<ResolvedLayout>(child).unwrap().position,
          Vec2::new(100.0, 100.0),
          "after flip to Absolute, resolves against the nearest positioned ancestor (100,100)",
      );
  }

  // A Display::None Fixed entity is removed from the Taffy tree entirely
  // (map_display -> taffy::Display::None) and contributes nothing; it must
  // not error the root-attach and must produce a zero-size layout.
  #[test]
  fn display_none_fixed_is_inert() {
      let mut app = app();
      let fixed_none = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .display(buiy_core::layout::Display::None)
                  .position(PositionKind::Fixed)
                  .inset(inset_top_left(0.0, 0.0))
                  .width(Length::Px(30.0))
                  .height(Length::Px(30.0)),
          ))
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(800.0)).height(Length::Px(600.0))))
          .add_child(fixed_none)
          .id();
      // Must not panic / error.
      app.update();
      if let Some(rl) = app.world().get::<ResolvedLayout>(fixed_none) {
          assert_eq!(rl.size, Vec2::ZERO, "Display::None Fixed has zero size");
      }
  }

  // Two Fixed siblings both attach to the root and keep their own insets
  // (single global root, D2) — neither displaces the other.
  #[test]
  fn two_fixed_entities_both_resolve_against_root() {
      let mut app = app();
      let a = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Fixed)
                  .inset(inset_top_left(10.0, 10.0))
                  .width(Length::Px(20.0))
                  .height(Length::Px(20.0)),
          ))
          .id();
      let b = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Fixed)
                  .inset(inset_top_left(50.0, 60.0))
                  .width(Length::Px(20.0))
                  .height(Length::Px(20.0)),
          ))
          .id();
      let _root = app
          .world_mut()
          .spawn((Node, Style::default().width(Length::Px(800.0)).height(Length::Px(600.0))))
          .add_children(&[a, b])
          .id();
      app.update();
      assert_eq!(app.world().get::<ResolvedLayout>(a).unwrap().position, Vec2::new(10.0, 10.0));
      assert_eq!(app.world().get::<ResolvedLayout>(b).unwrap().position, Vec2::new(60.0, 50.0));
  }
  ```
  **Implementer note:** `Position` + `Display` must be imported in the test file (`use buiy_core::layout::{Display, Position, …};`). `inset_top_left(top, left)` produces `Inset { top, left, .. }`; assertion vectors are `(x, y) = (left, top)` so `inset_top_left(50.0, 60.0)` → position `(60.0, 50.0)` (x=left=60, y=top=50) — double-check the x/y vs top/left mapping against `ResolvedLayout`'s `position.x = location.x` (Taffy x = horizontal = left). Adjust expected tuples to match the verified coordinate model from T4. `entity_mut(child).insert(Position { … })` overwrites the component, firing `Changed<Position>` so `sync_styles` re-runs the re-parent decision (D3).

- [ ] **Step 2: Run.**
  ```bash
  cargo test -p buiy_core --test layout_fixed
  ```
  Expected PASS (all fixtures, including T3/T4's).

- [ ] **Step 3: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_fixed.rs
  git commit -m "test(layout): Fixed edge cases — flip re-home, Display::None inert, two-fixed (Phase 10 — D2/D3)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Self-review (against the spec)

**Spec coverage** (`display-and-positioning.md`):
- § 2.2 `Fixed → taffy::Position::Absolute` emission → T1 (regression test + doc refresh; emission already shipped). ✓
- § 2.1 `Fixed` containing block = layout root → T2 (`is_fixed_root` predicate), T3 (exclude from in-flow parent), T4 (attach to root + root-relative + percent-against-root assertions). ✓
- § 2 `Position`/`PositionKind` surface → T5 (`.fixed()` setter), T6 (doc refresh). ✓
- § 2.1 "Known gap" (transformed ancestor becomes containing block for Fixed) → explicitly OUT of scope (D1 "How to apply" + D6 doc note); remains a documented gap. ✓
- Test-surface intent (positioning fixtures) → T3 (sibling not displaced), T4 (root-relative, percent-against-root), T7 (flip re-home, Display::None inert, two-fixed). ✓

**Placeholder scan:** every step has concrete code + exact commands. The two coordinate-model "confirm against `tests/layout_topology.rs`" notes (T3 step 1, T4 step 1) are explicit *verification* instructions for the implementer, not placeholders — they name the file to read and the exact load-bearing invariant to preserve if the harness's coordinate convention differs from the assumed parent-relative model. The `item.4 == &Position` index note (T3 step 2b) instructs the implementer to confirm the `NodeQueryItem` tuple slot before relying on it. No "TBD" / "similar to Task N" / "add error handling" placeholders.

**Type consistency:** `is_fixed_root(&Position) -> bool` (T2) is used in T3/T4. `sync_children_for_entity(entity, children, fixed_set: &HashSet<Entity>, tree)` (T3) is the one signature used by both `sync_styles` callers (T3 step 2c updates the other caller). `fixed_set: HashSet<Entity>` / `fixed_node_ids: Vec<TaffyNodeId>` (T3/T4) are built once and reused. `.fixed()` (T5) sets `Position.kind = PositionKind::Fixed`. No new component, resource, or registration is introduced (verified against mod.rs:110/:16/:32 — `Position`/`PositionKind` already wired), so no T touches `mod.rs`/`lib.rs`. `ResolvedLayout { position: Vec2, size: Vec2 }` is the only geometry artifact asserted (T3/T4/T7).

**Divergences from the follow-up sketch (recorded so the implementer is not surprised):**
1. The sketch says "PositionKind::Fixed still emits a Phase-1 warn-once" and "remove the Fixed warn-once" — **there is no Fixed warn-once in the code** (verified: `grep -rn Fixed crates/buiy_core/src` shows only doc comments + the already-correct `map_position_kind` arm; `PositionKind`'s doc explicitly says "No `warn!` is emitted"). No warn to remove. (T1 only refreshes a doc comment.)
2. The sketch says "`translate.rs::map_position` does NOT emit `taffy::Position::Absolute` for Fixed" — **it already does** (translate.rs:509). T1 locks this with a test rather than adding the emission.
3. The sketch's "private `is_fixed_root` flag on the entity's translation state" is realized as a **pure per-frame predicate** (D3), not a stored field — there is no persistent translation-state struct to hang a flag on, and the value is trivially derived from `Position.kind`.

The real, remaining work the spec demands is the **containing-block override** (root re-parenting), which this plan implements in T2–T4.
