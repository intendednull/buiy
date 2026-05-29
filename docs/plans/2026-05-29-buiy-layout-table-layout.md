# Phase 12: Full table layout algorithm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Replace the Phase-7 `table_layout` warn-once stub (sub-pass 6b) with the real CSS table layout algorithm from [`display-and-positioning.md` § 1.2](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md#12-table-layout-status): gather entities by `Display::Table*` family, resolve per-column widths via a synthetic Taffy flex container, lay cells into a grid, stack rows and row-groups vertically, and write corrected per-entity positions to the shared `PostTaffyPositionOverrides` correction buffer that `write_resolved_layout` (step 7) consumes. The warn-once is **kept only for genuinely unsupported sub-features** (`colspan`/`rowspan` — no API exists; `Display::TableCaption` / `TableColumn` / `TableColumnGroup` — column-sizing-via-`<col>` and captions are deferred), documented per-feature.

**Architecture (3 sentences):**
1. **Table layout is a post-Taffy correction overlay, exactly like sticky (6a).** Taffy lays every `Display::Table*` entity out as `Display::Block` (the `map_display` fallback in [translate.rs:505-507](../../crates/buiy_core/src/layout/translate.rs) is unchanged — Buiy never asks Taffy to do table layout), so `table_layout` (6b) reads Taffy's computed cell **sizes** (intrinsic block boxes) and **overwrites** the cell / row / row-group **positions** into `PostTaffyPositionOverrides.by_entity`, the same `HashMap<Entity, Vec2>` correction buffer the other 6a-6f sub-passes share; size is never touched (spec § 1.2 step 3 + prior-art `taffy/capabilities.md:51` "Children of `Display::Table*` are positioned by Buiy after Taffy").
2. **Column widths come from a throwaway synthetic Taffy flex tree per table (spec § 1.2 step 2).** For each table the system builds a fresh `TaffyTree<()>` whose rows are flex containers and whose cells are flex leaves sized to their real (Taffy-block-computed) max-content widths; one `compute_layout` resolves the columns to a common width per column index (the widest cell in that column wins — CSS fixed/auto table column resolution, restricted to the no-`colspan` case), then the synthetic tree is dropped (the real `LayoutTree` is never mutated, mirroring how 6a only *reads* `tree.tree.layout`).
3. **Geometry is three pure, unit-tested helpers + a thin ECS system (D3 — the sticky/stacking pure-helper precedent).** `gather_table` walks the `Children` hierarchy into a `TableModel` (row-groups → rows → cells, document order); `resolve_column_widths` turns the per-row cell widths into a `Vec<f32>` of column widths via the synthetic flex tree; `place_table_cells` turns column widths + per-row heights into per-entity `(x, y)` offsets relative to the table origin; the `table_layout` system reads queries, calls the helpers, and writes the overrides + the per-feature warns.

**Tech Stack:** Bevy 0.18 (`bevy::prelude::{Children, ChildOf, Node, Query, NonSend, ResMut, With}`, `bevy::ecs::entity::Entity`, `bevy::math::Vec2`). `taffy::{TaffyTree, Style, Size, AvailableSpace, Dimension, FlexDirection, Display as TaffyDisplay}` for the synthetic per-table tree (already a workspace dep — `crates/buiy_core/src/layout/tree.rs:12`). `std::collections::HashMap` (no `bevy::utils::*`, per Phase 6/7/8/9 precedent). No new external dependency.

**Date:** 2026-05-29
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/display-and-positioning.md`](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md) § 1.2 (table layout algorithm + the v1 fallback it supersedes) + § 1 (`Display::Table*` family) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (sub-pass 6b in the `PostTaffyOverrides` chain), § 6 (warn-once error model); reads the Phase-7 `PostTaffyPositionOverrides` plumbing ([`systems.rs:176-179`](../../crates/buiy_core/src/layout/systems.rs)).

---

## Prior-art citations (used throughout this plan)

- **Tables are an embedder post-Taffy pass, not a Taffy mode** — `docs/prior-art/taffy/capabilities.md:51` ("`display: table` family — Not implemented. `item_is_table: bool` is a sizing hint only. Buiy implements table layout as **post-Taffy sub-pass 6b** … Children of `Display::Table*` are positioned by Buiy after Taffy") and `docs/prior-art/taffy/layout-algorithms.md:89,126` (no `display:table*`; `item_is_table` is a sizing-mode hint, not real table layout). This pins the design: Taffy gives sizes, Buiy assigns positions in 6b.
- **Tables are a real formatting context with column-width resolution as the hard part** — `docs/prior-art/blink/layout.md:41` (Chromium "TablesNG — a multi-year effort to re-architect rendering of tables … resolving 72 tracked Chromium bugs") and `docs/prior-art/servo-stylo/layout.md:43,90` (Servo's own `table/` FC — "A real table FC, not the sizing hint Taffy exposes"; "tables as a post-Taffy sub-pass (6b)"). Lesson: scope v1 to the no-span, single-pass auto/fixed column resolution; defer spanning + captions (the bug-heavy corners) behind warns.
- **Document order = `Children` order; stable, deterministic geometry** — `docs/prior-art/servo-stylo/layout.md:63` ("push entities in document order, then `sort_by_key` (stable)"). Table rows/cells are laid out in `Children` iteration order (no reordering); the gather walk preserves it, so column index = cell position in its row and row index = row position in its group.
- **Post-Taffy correction-buffer pattern (the 6a sticky precedent)** — `crates/buiy_core/src/layout/systems.rs:530-636` (`sticky_offset`): reads `tree.tree.layout(*node_id)` for sizes/natural positions, skips entities Taffy hasn't placed (`continue`), and writes the corrected position into `overrides.by_entity.insert(e, ...)`; `write_resolved_layout` (`systems.rs:2060-2064`) consumes it (`overrides.by_entity.get(&entity)` overrides position, size stays from Taffy). 6b follows this shape exactly.
- **The stub being replaced** — `crates/buiy_core/src/layout/systems.rs:639-679` (`table_layout` + `is_table_display`). The stub's blanket `TableUnsupported(e)` warn is removed (the algorithm now runs); the `is_table_display` predicate is generalized into the family classifier (T1).
- **Per-session warn-once dedup** — `crates/buiy_core/src/layout/systems.rs:241-244` (`LayoutWarnedOnceSession { set: HashSet<LayoutWarnOnceKey> }`); `LayoutWarnOnceKey` enum at `crates/buiy_core/src/layout/types.rs:979` (`TableUnsupported(Entity)`, `MulticolUnsupported`, `MultipleFullscreenTopLayer`, …). Idiom: `if warned.set.insert(key) { warn!(…) }`. The enum is already `register_type`'d (`mod.rs:169`); `Reflect` picks up new variants for free.
- **Synthetic `TaffyTree` construction idiom** — `crates/buiy_core/src/layout/systems.rs:1738` (`tree.tree.new_leaf(taffy_style)`), `:1916,:1970` (`tree.tree.set_children(parent, &child_ids)`), `:2015` (`tree.tree.compute_layout(id, Size { width: AvailableSpace::Definite(w), height: … })`). The synthetic per-table tree (T2) uses the identical `TaffyTree<()>` API on a *local* tree, never the shared `LayoutTree`.
- **Pipeline sub-pass chain** — `crates/buiy_core/src/layout/mod.rs:225-235`: the `.chain().in_set(BuiyLayoutStep::PostTaffyOverrides)` tuple `(clear_post_taffy_overrides, sticky_offset, table_layout, multicol_pack, anchor_resolution, transform_composition, stacking_context)`. 6b is `table_layout`, already wired (position 3) — this phase changes only the *body* of `table_layout`, not its slot.
- **Test harness** — `crates/buiy_core/tests/layout_table_multicol_stubs.rs:20-24`: `fn app() { let mut app = App::new(); app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin); app }`; spawn `(Node, Display::TableCell, Style::default().width_px(…))`; build hierarchy with `commands.spawn(...).add_children(&[...])` / `.add_child(c)`; `app.update()` runs the whole pipeline; assert via `app.world().get::<ResolvedLayout>(e)`. Existing files: `tests/layout_table_multicol_stubs.rs` (the stub tests this phase supersedes), `tests/layout_sticky.rs`, `tests/layout_pipeline_order.rs`.
- **`ResolvedLayout` shape** — `crates/buiy_core/src/components.rs:22-29` (`ResolvedLayout { position: Vec2, size: Vec2 }`). Tables assert on `position` (corrected by 6b) and `size` (from Taffy).

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/systems.rs` | T1 (`TablePart` classifier + `table_part`), T2 (`resolve_column_widths` synthetic-flex helper), T3 (`place_table_cells` geometry helper), T4 (`gather_table` + `table_layout` system base — single row), T5 (multi-row column widths), T6 (row-group stacking), T7 (per-feature warns) |
| `crates/buiy_core/src/layout/types.rs` | T7 (`LayoutWarnOnceKey::TableSpanUnsupported(Entity)`, `TableSubfeatureUnsupported(Entity)`) |
| `crates/buiy_core/tests/layout_table.rs` | T4, T5, T6, T7 (new integration-test file) |
| `crates/buiy_core/tests/layout_table_multicol_stubs.rs` | T4 (delete the 6 superseded table-stub tests; keep the 4 multicol/cross-pass tests) |

No changes to: `crates/buiy_core/src/layout/translate.rs` (`map_display` keeps `Table* → taffy::Display::Block`), `crates/buiy_core/src/layout/mod.rs` (6b already wired; `LayoutWarnOnceKey` already registered), `crates/buiy_core/src/layout/style.rs`, `crates/buiy_core/src/layout/components.rs`, `crates/buiy_core/src/components.rs`, `crates/buiy/src/lib.rs`.

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. 6b corrects positions only; sizes stay from Taffy (the sticky/clip precedent)

**Decision:** `table_layout` (6b) writes **only** `PostTaffyPositionOverrides.by_entity[entity] = corrected_position` for table cells, rows, and row-groups. It never alters `ResolvedLayout.size` — cell sizes are whatever Taffy computed for the block box (`tree.tree.layout(cell_node).size`).

**Why:** Spec § 1.2 step 3: "Write corrected **positions** back to `PostTaffyPositionOverrides`." `write_resolved_layout` (`systems.rs:2060-2068`) already merges `overrides` position over Taffy position while always taking size from Taffy — so 6b plugs into the existing merge with zero changes to step 7, exactly as `sticky_offset` (6a) does. A cell's content-driven size is genuinely a Taffy block-layout result; the table algorithm's job is *placement* into a column grid, plus widening cells to their column width (achieved by the position grid + the synthetic-tree column widths, not by mutating size — see D5).

**Runner-up rejected:** Have 6b also write a `size` override (e.g. stretch each cell to its column width). Rejected: `PostTaffyPositionOverrides` is position-only by construction (`HashMap<Entity, Vec2>`, `systems.rs:178`); adding a size buffer is a cross-cutting change to step 7 that the spec does not ask for. v1 places cells at column origins with Taffy-computed sizes; per-cell stretch-to-column-width is recorded as a follow-up (D7).

### D2. Column widths via a synthetic per-table Taffy flex tree (spec § 1.2 step 2, verbatim)

**Decision:** `resolve_column_widths(rows: &[Vec<f32>]) -> Vec<f32>` builds a throwaway `taffy::TaffyTree<()>`: one flex-row container per table-row, each containing one fixed-size (`Dimension::length(cell_width)`) leaf per cell, all rows children of a synthetic flex-**column** root; `compute_layout` runs once with `AvailableSpace::MaxContent` on both axes; the resolved width of column `c` is the **max** over all rows of cell `c`'s resolved width. The synthetic tree is a local variable, dropped at function end.

**Why:** Spec § 1.2 step 2 literally says "Compute column widths via Taffy on a synthetic flex container per row group." Using Taffy (rather than a hand-rolled `max()`) keeps the door open to honoring `flex-basis`-style intrinsic sizing and matches the "Taffy is the sizing engine" posture (`prior-art/taffy/capabilities.md:51`). For v1's no-`colspan` case the resolved column width *equals* `rows.iter().map(|r| r[c]).fold(0.0, f32::max)` — but routing it through `compute_layout` makes the helper's contract "Taffy resolves the columns," so future `colspan` / `%`-width work extends the synthetic style instead of rewriting the helper.

**How to apply:** `resolve_column_widths` is pure (input `&[Vec<f32>]`, output `Vec<f32>`) and fully unit-tested (T2). The caller (T4) supplies each row's cell widths from `tree.tree.layout(cell).size.width`.

**Runner-up rejected:** Hand-rolled per-column `max` with no Taffy. Rejected: deviates from the spec's explicit "via Taffy on a synthetic flex container" wording and the extension path it buys; the synthetic tree is cheap (one table's cells, dropped immediately).

### D3. Geometry decomposed into three pure helpers + a thin system (the 6a/6f precedent)

**Decision:** Factor the algorithm into pure, unit-tested helpers in `systems.rs`:
- `table_part(display: &Display) -> Option<TablePart>` (T1) — classify a `Display` into the table family role.
- `resolve_column_widths(rows: &[Vec<f32>]) -> Vec<f32>` (T2) — synthetic-flex column resolution (D2).
- `place_table_cells(model: &TableModel, col_widths: &[f32], row_heights: &[f32]) -> HashMap<Entity, Vec2>` (T3) — assign each cell/row/row-group an offset relative to the table origin.

`gather_table` (T4) walks `Children` into a `TableModel`; the `table_layout` system reads queries, calls the helpers, adds the table's own Taffy origin to each offset, and writes `overrides`.

**Why:** The CSS placement arithmetic (cumulative column x-offsets, cumulative row y-offsets, row-group stacking) is exactly the kind of logic that needs focused unit tests without an `App` — the `compute_sticky_displacement` (6a, `systems.rs:450`) and `paint_key`/`forms_stacking_context` (6f) pure-helper precedent. Keeping the system thin keeps the tree walk + Taffy reads readable.

**Runner-up rejected:** One monolithic system. Rejected: the geometry quirks (a row-group offsets all its rows; an empty row contributes zero height) need isolated tests an `App`-level test obscures.

### D4. Table family scope: table / row-group(s) / row / cell are LAID OUT; caption / column(-group) are deferred-with-warn

**Decision:** v1 lays out the structural spine: `Display::Table` (the table box), `Display::TableRowGroup` / `TableHeaderGroup` / `TableFooterGroup` (row groups), `Display::TableRow` (rows), `Display::TableCell` (cells). `Display::TableCaption`, `Display::TableColumn`, `Display::TableColumnGroup` are **classified** by `table_part` but produce **no geometry** in v1 — each warns once per (entity, session) via `TableSubfeatureUnsupported(Entity)` (T7) and is left at its Taffy-block position (no override written).

**Why:** Captions (position above/below the table box) and `<col>`/`<colgroup>` (column-spanning style carriers that set column widths/backgrounds without generating cells) are the spec's tier-C corners that Blink's TablesNG spent multi-year effort on (`prior-art/blink/layout.md:41`); they are not needed for the core "rows of cells in a column grid" deliverable and pulling them in now is speculative scope (CLAUDE.md "scope creep OK when warranted, not speculative"). Classifying them (so the warn names the exact unsupported feature) keeps the API stable for the v1.x follow-up.

**How to apply:** `table_part` returns all seven roles (T1); `gather_table` collects only Table/RowGroup*/Row/Cell into the `TableModel` (T4) and records Caption/Column/ColumnGroup entities for the warn (T7).

**Runner-up rejected:** Lay captions out (above the table). Rejected: caption placement interacts with table-box sizing (the table grows to include the caption band) — a sizing change, which D1 excludes from v1; deferring keeps the size contract clean.

### D5. Header/footer row-groups stack in source order in v1 (no header-floats-to-top reorder)

**Decision:** Row-groups stack vertically in **`Children` document order**: a `TableHeaderGroup` declared after a `TableRowGroup` is laid out below it. v1 does **not** reorder header groups to the top or footer groups to the bottom.

**Why:** CSS visually floats `thead` to the top and `tfoot` to the bottom regardless of source position, but that reordering is a cross-cutting placement rule that interacts with fragmentation (repeating headers per page — `prior-art/blink/layout.md:41-42` flags table+fragmentation as the expensive corner). v1's deterministic "document order" stacking matches the stable-document-order discipline (`prior-art/servo-stylo/layout.md:63`) and the most common authoring case (groups already in `thead, tbody, tfoot` order). Authors who need a different visual order place the groups in that order.

**How to apply:** `gather_table` collects row-groups in `Children` order; `place_table_cells` (T3) stacks them in that order (T6). The reorder is a follow-up (D7).

**Runner-up rejected:** Reorder header/footer at gather time. Rejected: speculative (interacts with the deferred fragmentation work) and the reorder rule is observationally a no-op when authors declare groups in the conventional order.

### D6. Bare rows / cells without a row-group wrapper get an anonymous implicit group

**Decision:** A `TableRow` that is a direct child of the `Table` (no intervening row-group) is treated as belonging to a single anonymous row-group spanning all such bare rows, in document order. Likewise a `Table` whose direct children are `TableRow`s lays them out as one implicit group.

**Why:** CSS generates anonymous `table-row-group` boxes around bare rows (the table fixup rules). The common authoring case `Table > TableRow > TableCell` (no explicit `tbody`) must work. Modeling the implicit group as "rows with no row-group parent are grouped together, document order" is the minimal faithful subset.

**How to apply:** `gather_table` (T4): when walking the table's children, a `TableRow` child contributes to an implicit group; an explicit row-group child contributes its own rows. T4 covers the bare-rows case; T6 covers explicit groups.

**Runner-up rejected:** Require an explicit row-group. Rejected: breaks the most common `Table > TableRow > TableCell` authoring shape and diverges from CSS fixup.

### D7. Deferred sub-features are recorded as follow-ups, not stubbed ahead

**Decision:** `colspan`/`rowspan` (no API surface exists on any Buiy component), per-cell stretch-to-column-width (size override — D1), header/footer reorder (D5), caption + `<col>`/`<colgroup>` geometry (D4), and `border-collapse` / table border-spacing are **out of v1 scope**. `colspan`/`rowspan` *requests* cannot be expressed (no field), so the only warn that can fire for spanning is a defensive one (D8 below covers why none is added). Caption/column(-group) warn via `TableSubfeatureUnsupported` (D4/T7).

**Why:** CLAUDE.md "don't add features … the task didn't ask for"; these are independent v1.x deliverables. Recording them in the follow-ups doc (a separate docs stage, not a task here) keeps the target state honest without speculative code.

### D8. No `colspan`/`rowspan` API ⇒ a uniform-grid assumption, with a ragged-row warn

**Decision:** v1 assumes each row has the same cell count (a uniform grid). When rows have **different** cell counts (a ragged table — the shape an author would build to *fake* spanning), the algorithm lays out each cell at its own column index up to that row's cell count (short rows simply have no cells in the trailing columns), and warns once per (table entity, session) via `TableSpanUnsupported(Entity)` (T7) that spanning is unsupported and ragged rows are laid out as-is.

**Why:** Without a `colspan` field the engine cannot know an author *intended* a span; the safe, deterministic behavior is positional (column index = cell index), which is correct for uniform grids and predictable for ragged ones. The warn names the limitation so a ragged table doesn't silently mislead.

**How to apply:** `resolve_column_widths` (T2) handles rows of differing lengths (column count = max row length; missing cells contribute 0 width). `table_layout` (T7) detects "rows differ in length" and warns once per table.

**Runner-up rejected:** Reject (skip) ragged tables. Rejected: skipping leaves cells at raw Taffy-block positions with no diagnostic — worse than positional placement + a warn.

---

## Tasks

> **Per-task workflow (subagent-driven):**
> 1. Implementer subagent reads the task block.
> 2. Implementer follows TDD: failing test first, then minimal impl to pass, then refactor if needed, then commit.
> 3. Spec-compliance reviewer subagent reads the spec sections + the diff and asserts coverage.
> 4. Code-quality reviewer subagent reads the diff and asserts the code-quality bar.
> 5. Both reviews must be ✅ before moving to the next task.

> **Project gate (run before every commit, exactly — this host has no xvfb; `MinimalPlugins` runs headless):**
> ```sh
> cargo fmt --all -- --check && \
>   cargo clippy --workspace --all-targets -- -D warnings && \
>   RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
>   cargo test --workspace
> ```

### Task 1: `TablePart` classifier + `table_part` pure helper

**Spec:** § 1 (`Display::Table*` family), D4.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `TablePart` enum + `table_part` helper + unit tests; keep `is_table_display` for now — T7 removes its last caller)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  use crate::layout::components::Display;

  #[test]
  fn table_part_classifies_every_family_member() {
      assert_eq!(table_part(&Display::Table), Some(TablePart::Table));
      assert_eq!(
          table_part(&Display::TableRowGroup),
          Some(TablePart::RowGroup)
      );
      assert_eq!(
          table_part(&Display::TableHeaderGroup),
          Some(TablePart::RowGroup)
      );
      assert_eq!(
          table_part(&Display::TableFooterGroup),
          Some(TablePart::RowGroup)
      );
      assert_eq!(table_part(&Display::TableRow), Some(TablePart::Row));
      assert_eq!(table_part(&Display::TableCell), Some(TablePart::Cell));
      assert_eq!(
          table_part(&Display::TableCaption),
          Some(TablePart::Caption)
      );
      assert_eq!(
          table_part(&Display::TableColumn),
          Some(TablePart::Column)
      );
      assert_eq!(
          table_part(&Display::TableColumnGroup),
          Some(TablePart::ColumnGroup)
      );
  }

  #[test]
  fn table_part_is_none_for_non_table_display() {
      assert_eq!(table_part(&Display::Block), None);
      assert_eq!(table_part(&Display::None), None);
      assert_eq!(
          table_part(&Display::Flex(crate::layout::types::FlexAxis::Row)),
          None
      );
  }
  ```
  Run: `cargo test -p buiy_core table_part_classifies table_part_is_none` — expected FAIL (type + fn don't exist).

- [ ] **Step 2: Add the enum + helper to `systems.rs`.** Place immediately above the existing `table_layout` system (`systems.rs:649`):
  ```rust
  /// The role an entity plays in a CSS table, derived from its
  /// `Display` (spec § 1, display-and-positioning.md). The four
  /// structural roles (`Table` / `RowGroup` / `Row` / `Cell`) are laid
  /// out by sub-pass 6b; `Caption` / `Column` / `ColumnGroup` are
  /// classified but deferred-with-warn in v1 (plan D4).
  #[derive(Clone, Copy, PartialEq, Eq, Debug)]
  pub(super) enum TablePart {
      Table,
      /// `table-row-group` / `table-header-group` / `table-footer-group`
      /// — all three collapse to `RowGroup`; header/footer reorder is a
      /// v1.x follow-up (D5).
      RowGroup,
      Row,
      Cell,
      Caption,
      Column,
      ColumnGroup,
  }

  /// Classify a `Display` into its `TablePart` role, or `None` if the
  /// entity is not a table-family member.
  pub(super) fn table_part(display: &Display) -> Option<TablePart> {
      match display {
          Display::Table => Some(TablePart::Table),
          Display::TableRowGroup
          | Display::TableHeaderGroup
          | Display::TableFooterGroup => Some(TablePart::RowGroup),
          Display::TableRow => Some(TablePart::Row),
          Display::TableCell => Some(TablePart::Cell),
          Display::TableCaption => Some(TablePart::Caption),
          Display::TableColumn => Some(TablePart::Column),
          Display::TableColumnGroup => Some(TablePart::ColumnGroup),
          _ => None,
      }
  }
  ```

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core table_part_classifies table_part_is_none
  ```
  Expected PASS.

- [ ] **Step 4: Project gate.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): TablePart classifier for table layout (Phase 12 — spec § 1)

table_part maps Display::Table* to its structural role (Table/RowGroup/Row/Cell)
plus the deferred Caption/Column/ColumnGroup roles (D4). Pure helper; the real 6b
algorithm consumes it in later tasks.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 2: `resolve_column_widths` — synthetic Taffy flex column resolution

**Spec:** § 1.2 step 2, D2, D8.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `resolve_column_widths` + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn resolve_column_widths_single_row_passes_cell_widths_through() {
      // One row, three cells 30/50/20 → columns resolve to 30/50/20.
      let cols = resolve_column_widths(&[vec![30.0, 50.0, 20.0]]);
      assert_eq!(cols.len(), 3);
      assert!((cols[0] - 30.0).abs() < 0.5);
      assert!((cols[1] - 50.0).abs() < 0.5);
      assert!((cols[2] - 20.0).abs() < 0.5);
  }

  #[test]
  fn resolve_column_widths_takes_per_column_max_across_rows() {
      // Row A: 30/50  Row B: 40/20 → columns = max(30,40)=40, max(50,20)=50.
      let cols = resolve_column_widths(&[vec![30.0, 50.0], vec![40.0, 20.0]]);
      assert_eq!(cols.len(), 2);
      assert!((cols[0] - 40.0).abs() < 0.5, "col0 = max(30,40) = 40");
      assert!((cols[1] - 50.0).abs() < 0.5, "col1 = max(50,20) = 50");
  }

  #[test]
  fn resolve_column_widths_ragged_rows_use_max_row_length() {
      // Row A has 3 cells, Row B has 1 → 3 columns; the missing cells
      // contribute 0 width (D8 ragged-row handling).
      let cols = resolve_column_widths(&[vec![10.0, 20.0, 30.0], vec![15.0]]);
      assert_eq!(cols.len(), 3, "column count = widest row");
      assert!((cols[0] - 15.0).abs() < 0.5, "col0 = max(10,15) = 15");
      assert!((cols[1] - 20.0).abs() < 0.5);
      assert!((cols[2] - 30.0).abs() < 0.5);
  }

  #[test]
  fn resolve_column_widths_empty_table_is_empty() {
      assert!(resolve_column_widths(&[]).is_empty());
      // A table with rows but no cells → zero columns.
      assert!(resolve_column_widths(&[vec![], vec![]]).is_empty());
  }
  ```
  Run: `cargo test -p buiy_core resolve_column_widths` — expected FAIL.

- [ ] **Step 2: Add the helper to `systems.rs`.** Place below `table_part` (T1):
  ```rust
  /// Resolve per-column widths for a table from each row's cell widths,
  /// via a throwaway synthetic Taffy flex tree (spec § 1.2 step 2). The
  /// synthetic tree has one flex-row container per table-row, each
  /// holding one fixed-width leaf per cell, all rows under a synthetic
  /// flex-column root; one `compute_layout` resolves the cells, and
  /// column `c`'s width is the max resolved width of cell `c` across
  /// rows. Column count = the widest row's cell count; rows shorter than
  /// that contribute nothing to the trailing columns (ragged-row case,
  /// plan D8). The synthetic `TaffyTree` is local and dropped on return
  /// — the shared `LayoutTree` is never touched.
  ///
  /// Pure (no Bevy queries / no shared state). Unit-tested in `mod tests`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  pub(super) fn resolve_column_widths(rows: &[Vec<f32>]) -> Vec<f32> {
      let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
      if col_count == 0 {
          return Vec::new();
      }

      let mut tree: taffy::TaffyTree<()> = taffy::TaffyTree::new();
      let mut row_nodes: Vec<taffy::NodeId> = Vec::with_capacity(rows.len());
      for row in rows {
          let mut cell_nodes: Vec<taffy::NodeId> = Vec::with_capacity(row.len());
          for &w in row {
              // Fixed-size leaf: the cell's Taffy-block-computed width.
              let leaf = tree
                  .new_leaf(taffy::Style {
                      size: taffy::Size {
                          width: taffy::Dimension::length(w),
                          height: taffy::Dimension::length(0.0),
                      },
                      flex_grow: 0.0,
                      flex_shrink: 0.0,
                      ..Default::default()
                  })
                  .expect("synthetic table column leaf");
              cell_nodes.push(leaf);
          }
          let row_node = tree
              .new_with_children(
                  taffy::Style {
                      display: taffy::Display::Flex,
                      flex_direction: taffy::FlexDirection::Row,
                      ..Default::default()
                  },
                  &cell_nodes,
              )
              .expect("synthetic table row");
          row_nodes.push(row_node);
      }
      let root = tree
          .new_with_children(
              taffy::Style {
                  display: taffy::Display::Flex,
                  flex_direction: taffy::FlexDirection::Column,
                  ..Default::default()
              },
              &row_nodes,
          )
          .expect("synthetic table root");
      // MaxContent so each column sizes to its widest cell, no shrink.
      tree.compute_layout(
          root,
          taffy::Size {
              width: taffy::AvailableSpace::MaxContent,
              height: taffy::AvailableSpace::MaxContent,
          },
      )
      .expect("synthetic table layout");

      let mut widths = vec![0.0f32; col_count];
      for (ri, &row_node) in row_nodes.iter().enumerate() {
          for ci in 0..rows[ri].len() {
              if let Ok(child) = tree.child_at_index(row_node, ci)
                  && let Ok(layout) = tree.layout(child)
              {
                  widths[ci] = widths[ci].max(layout.size.width);
              }
          }
      }
      widths
  }
  ```
  **Implementer note:** confirm the Taffy 0.10 API names against `crates/buiy_core/src/layout/tree.rs` + existing calls in `systems.rs`: `TaffyTree::new()`, `new_leaf(Style) -> Result<NodeId,_>`, `new_with_children(Style, &[NodeId]) -> Result<NodeId,_>`, `compute_layout(NodeId, Size<AvailableSpace>) -> Result<_,_>`, `layout(NodeId) -> Result<&Layout,_>`, `child_at_index(NodeId, usize) -> Result<NodeId,_>`, `AvailableSpace::MaxContent`, `Dimension::length`, `FlexDirection::Row`/`Column`, `Display::Flex`. If `new_with_children` is absent in 0.10, build with `new_leaf` for containers then `set_children(parent, &kids)` (the `sync_children_for_entity` idiom at `systems.rs:1970`) and `compute_layout` — same result. If `child_at_index` is absent, store the per-row `cell_nodes` vectors and index those directly instead of re-querying the tree.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core resolve_column_widths
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): resolve_column_widths via synthetic Taffy flex tree (Phase 12 — spec § 1.2 step 2)

Per-column width = max cell width across rows, resolved through a throwaway
TaffyTree flex container (D2). Handles ragged rows (column count = widest row,
missing cells contribute 0 — D8). Pure helper.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 3: `TableModel` + `place_table_cells` geometry helper

**Spec:** § 1.2 step 3, D3, D5, D6.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `TableModel`/`TableRowGroupModel`/`TableRowModel` structs + `place_table_cells` + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  // Build entities with stable ids for assertions.
  fn ent(n: u32) -> Entity {
      Entity::from_raw_u32(n).unwrap()
  }

  #[test]
  fn place_single_row_two_cells_in_column_grid() {
      // Two columns 40/60; one group with one row (height 20) holding
      // two cells. Cell 0 at x=0, cell 1 at x=40; both at y=0.
      let model = TableModel {
          groups: vec![TableRowGroupModel {
              entity: ent(1),
              rows: vec![TableRowModel {
                  entity: ent(2),
                  cells: vec![ent(3), ent(4)],
              }],
          }],
      };
      let placed = place_table_cells(&model, &[40.0, 60.0], &[20.0]);
      assert_eq!(placed[&ent(3)], bevy::math::Vec2::new(0.0, 0.0));
      assert_eq!(placed[&ent(4)], bevy::math::Vec2::new(40.0, 0.0));
      // Row + group sit at the table origin.
      assert_eq!(placed[&ent(2)], bevy::math::Vec2::new(0.0, 0.0));
      assert_eq!(placed[&ent(1)], bevy::math::Vec2::new(0.0, 0.0));
  }

  #[test]
  fn place_two_rows_stack_vertically_by_row_height() {
      // Row 0 height 20, row 1 height 30. Row 1 starts at y=20.
      let model = TableModel {
          groups: vec![TableRowGroupModel {
              entity: ent(1),
              rows: vec![
                  TableRowModel {
                      entity: ent(2),
                      cells: vec![ent(3)],
                  },
                  TableRowModel {
                      entity: ent(4),
                      cells: vec![ent(5)],
                  },
              ],
          }],
      };
      let placed = place_table_cells(&model, &[40.0], &[20.0, 30.0]);
      assert_eq!(placed[&ent(3)], bevy::math::Vec2::new(0.0, 0.0));
      assert_eq!(placed[&ent(2)], bevy::math::Vec2::new(0.0, 0.0));
      assert_eq!(placed[&ent(5)], bevy::math::Vec2::new(0.0, 20.0), "row 1 cell below row 0");
      assert_eq!(placed[&ent(4)], bevy::math::Vec2::new(0.0, 20.0), "row 1 at y=20");
  }

  #[test]
  fn place_two_groups_stack_in_document_order() {
      // Group A (1 row, height 20) then group B (1 row, height 30).
      // Group B's row starts at y=20 (D5 — document-order stacking).
      let model = TableModel {
          groups: vec![
              TableRowGroupModel {
                  entity: ent(1),
                  rows: vec![TableRowModel {
                      entity: ent(2),
                      cells: vec![ent(3)],
                  }],
              },
              TableRowGroupModel {
                  entity: ent(4),
                  rows: vec![TableRowModel {
                      entity: ent(5),
                      cells: vec![ent(6)],
                  }],
              },
          ],
      };
      let placed = place_table_cells(&model, &[40.0], &[20.0, 30.0]);
      assert_eq!(placed[&ent(1)], bevy::math::Vec2::new(0.0, 0.0), "group A at top");
      assert_eq!(placed[&ent(3)], bevy::math::Vec2::new(0.0, 0.0));
      assert_eq!(placed[&ent(4)], bevy::math::Vec2::new(0.0, 20.0), "group B below A");
      assert_eq!(placed[&ent(5)], bevy::math::Vec2::new(0.0, 20.0));
      assert_eq!(placed[&ent(6)], bevy::math::Vec2::new(0.0, 20.0));
  }
  ```
  **Implementer note:** `Entity::from_raw_u32(n).unwrap()` is the test-entity idiom already used at `systems.rs:3535`. `row_heights` is indexed by the **global** row index across all groups (group A's rows come first, then group B's), matching the flat row order `place_table_cells` walks.
  Run: `cargo test -p buiy_core place_single_row place_two_rows place_two_groups` — expected FAIL.

- [ ] **Step 2: Add the structs + helper to `systems.rs`.** Place below `resolve_column_widths` (T2):
  ```rust
  /// One table row: its entity and the cell entities it owns, in
  /// `Children` document order (column index = position in this vec).
  #[derive(Clone, Debug)]
  pub(super) struct TableRowModel {
      pub entity: Entity,
      pub cells: Vec<Entity>,
  }

  /// One row-group (explicit `table-row-group`/`header`/`footer`, or the
  /// implicit group around bare rows — plan D6): its entity and rows in
  /// document order.
  #[derive(Clone, Debug)]
  pub(super) struct TableRowGroupModel {
      pub entity: Entity,
      pub rows: Vec<TableRowModel>,
  }

  /// A table's structural spine gathered from the `Children` hierarchy,
  /// in document order (plan D5). Caption / column(-group) parts are not
  /// stored here — they are deferred-with-warn (plan D4).
  #[derive(Clone, Debug, Default)]
  pub(super) struct TableModel {
      pub groups: Vec<TableRowGroupModel>,
  }

  /// Assign each cell / row / row-group a position **relative to the
  /// table origin** (spec § 1.2 step 3). Cells sit at the cumulative
  /// column-x / cumulative-row-y grid; a row and its group sit at the
  /// row's y (a group at its first row's y); groups stack in document
  /// order (plan D5). `row_heights` is indexed by the flat row index
  /// across all groups (group order, then row order). Returns offsets
  /// keyed by entity; the caller adds the table's own Taffy origin.
  ///
  /// Pure. Unit-tested in `mod tests`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  pub(super) fn place_table_cells(
      model: &TableModel,
      col_widths: &[f32],
      row_heights: &[f32],
  ) -> std::collections::HashMap<Entity, Vec2> {
      // Cumulative column x-offsets: col_x[c] = sum of widths before c.
      let mut col_x: Vec<f32> = Vec::with_capacity(col_widths.len());
      let mut acc = 0.0;
      for &w in col_widths {
          col_x.push(acc);
          acc += w;
      }

      let mut placed: std::collections::HashMap<Entity, Vec2> = std::collections::HashMap::new();
      let mut y = 0.0f32;
      let mut row_index = 0usize;
      for group in &model.groups {
          let group_y = y;
          placed.insert(group.entity, Vec2::new(0.0, group_y));
          for row in &group.rows {
              placed.insert(row.entity, Vec2::new(0.0, y));
              for (ci, &cell) in row.cells.iter().enumerate() {
                  let x = col_x.get(ci).copied().unwrap_or(0.0);
                  placed.insert(cell, Vec2::new(x, y));
              }
              y += row_heights.get(row_index).copied().unwrap_or(0.0);
              row_index += 1;
          }
      }
      placed
  }
  ```

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core place_single_row place_two_rows place_two_groups
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): TableModel + place_table_cells grid geometry (Phase 12 — spec § 1.2 step 3)

Pure helper: cumulative column-x + cumulative-row-y grid placement; rows and
row-groups stack in document order (D5). Returns per-entity offsets relative to
the table origin.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 4: `gather_table` + `table_layout` system rewrite — single row, single cell end-to-end

**Spec:** § 1.2 (steps 1-3), D1, D6.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `gather_table`; replace the `table_layout` stub body)
- Create: `crates/buiy_core/tests/layout_table.rs` (new integration-test file)

- [ ] **Step 1: Failing test.** Create `crates/buiy_core/tests/layout_table.rs`:
  ```rust
  //! Phase 12 — full table layout algorithm (sub-pass 6b). Spawns
  //! Display::Table* hierarchies and asserts the corrected
  //! ResolvedLayout positions (cells in a column grid, rows + groups
  //! stacked vertically).
  //!
  //! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.

  use bevy::prelude::*;
  use buiy_core::layout::{Display, LayoutPlugin, Style};
  use buiy_core::{Node, ResolvedLayout};

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
      app
  }

  fn pos(app: &App, e: Entity) -> Vec2 {
      app.world()
          .get::<ResolvedLayout>(e)
          .expect("ResolvedLayout present")
          .position
  }

  #[test]
  fn single_row_two_cells_sit_in_a_column_grid() {
      // Table > Row > [Cell(w=40), Cell(w=60)]. (Bare row → implicit
      // group, D6.) Cell 0 at x=0; cell 1 at x=40 (after column 0).
      let mut app = app();
      let c0 = app
          .world_mut()
          .spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(20.0)))
          .id();
      let c1 = app
          .world_mut()
          .spawn((Node, Display::TableCell, Style::default().width_px(60.0).height_px(20.0)))
          .id();
      let row = app
          .world_mut()
          .spawn((Node, Display::TableRow, Style::default()))
          .add_children(&[c0, c1])
          .id();
      let _table = app
          .world_mut()
          .spawn((Node, Display::Table, Style::default()))
          .add_child(row)
          .id();

      app.update();

      assert_eq!(pos(&app, c0).x, 0.0, "cell 0 at column 0 origin");
      assert!((pos(&app, c1).x - 40.0).abs() < 0.5, "cell 1 starts after column 0 (40px)");
      assert_eq!(pos(&app, c0).y, pos(&app, c1).y, "both cells share the row's y");
  }

  #[test]
  fn cell_size_comes_from_taffy_not_overridden() {
      // 6b corrects position only; size stays from Taffy (D1).
      let mut app = app();
      let c0 = app
          .world_mut()
          .spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(25.0)))
          .id();
      let row = app
          .world_mut()
          .spawn((Node, Display::TableRow, Style::default()))
          .add_child(c0)
          .id();
      let _table = app
          .world_mut()
          .spawn((Node, Display::Table, Style::default()))
          .add_child(row)
          .id();

      app.update();

      let rl = app.world().get::<ResolvedLayout>(c0).unwrap();
      assert!((rl.size.x - 40.0).abs() < 0.5, "cell width from Taffy");
      assert!((rl.size.y - 25.0).abs() < 0.5, "cell height from Taffy");
  }
  ```
  Run: `cargo test -p buiy_core --test layout_table single_row_two_cells cell_size_comes_from_taffy` — expected FAIL (stub does no placement; cells sit at Taffy-block positions — both at x=0 stacked, so the x=40 assert fails).

- [ ] **Step 2: Add `gather_table` + rewrite `table_layout`.** Add `gather_table` below `place_table_cells` (T3), then replace the whole stub `table_layout` body (`systems.rs:649-664`) — keep `is_table_display` (`systems.rs:666-679`) for now (T7 removes its last use):
  ```rust
  /// Walk a table entity's `Children` hierarchy into a `TableModel`
  /// (spec § 1.2 step 1). Explicit row-groups contribute their own
  /// rows; bare `TableRow` children of the table form a single implicit
  /// anonymous row-group in document order (plan D6). Caption / column
  /// parts are skipped here (deferred-with-warn, plan D4). Returns the
  /// model plus the deferred-part entities for the caller's warn pass.
  ///
  /// `children_q` is the `Query<&Children>`; `display_q` reads each
  /// child's `Display`.
  fn gather_table(
      table: Entity,
      children_q: &Query<&Children>,
      display_q: &Query<&Display>,
  ) -> (TableModel, Vec<Entity>) {
      let mut model = TableModel::default();
      let mut deferred: Vec<Entity> = Vec::new();
      // The implicit group accumulates bare rows; flushed when a real
      // group is seen or at the end, preserving document order.
      let mut implicit = TableRowGroupModel {
          entity: table, // implicit group is the table box itself
          rows: Vec::new(),
      };

      let gather_row = |row: Entity| -> TableRowModel {
          let mut cells: Vec<Entity> = Vec::new();
          if let Ok(row_kids) = children_q.get(row) {
              for &cell in row_kids.iter() {
                  if matches!(display_q.get(cell), Ok(d) if table_part(d) == Some(TablePart::Cell)) {
                      cells.push(cell);
                  }
              }
          }
          TableRowModel { entity: row, cells }
      };

      let Ok(table_kids) = children_q.get(table) else {
          return (model, deferred);
      };
      for &child in table_kids.iter() {
          match display_q.get(child).ok().and_then(table_part) {
              Some(TablePart::Row) => implicit.rows.push(gather_row(child)),
              Some(TablePart::RowGroup) => {
                  let mut group = TableRowGroupModel {
                      entity: child,
                      rows: Vec::new(),
                  };
                  if let Ok(group_kids) = children_q.get(child) {
                      for &gk in group_kids.iter() {
                          if matches!(display_q.get(gk), Ok(d) if table_part(d) == Some(TablePart::Row)) {
                              group.rows.push(gather_row(gk));
                          }
                      }
                  }
                  model.groups.push(group);
              }
              Some(TablePart::Caption | TablePart::Column | TablePart::ColumnGroup) => {
                  deferred.push(child);
              }
              _ => {}
          }
      }
      if !implicit.rows.is_empty() {
          // Bare rows precede explicit groups in document order only if
          // they appeared first; for v1 the common case is *either* bare
          // rows *or* explicit groups, so prepend the implicit group.
          model.groups.insert(0, implicit);
      }
      (model, deferred)
  }

  /// Sub-pass 6b — table layout (spec § 1.2). For each `Display::Table`
  /// entity: gather its row-group / row / cell spine (step 1), resolve
  /// per-column widths via a synthetic Taffy flex tree (step 2), place
  /// every cell / row / row-group into the column grid relative to the
  /// table origin, and write the corrected absolute positions into
  /// `PostTaffyPositionOverrides` (step 3). Sizes are never touched —
  /// they stay from Taffy's block layout (plan D1), matching how 6a
  /// (sticky) corrects position only.
  ///
  /// Caption / column(-group) parts and ragged (span-faking) rows warn
  /// once per (entity, session) (plan D4 / D8); the warns land in a
  /// later task.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  pub(super) fn table_layout(
      tree: NonSend<LayoutTree>,
      table_q: Query<(Entity, &Display), With<Node>>,
      children_q: Query<&Children>,
      display_q: Query<&Display>,
      mut overrides: ResMut<PostTaffyPositionOverrides>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      let _ = &mut warned; // per-feature warns land in T7.
      for (table, display) in table_q.iter() {
          if table_part(display) != Some(TablePart::Table) {
              continue;
          }
          // The table's own natural position (Taffy-block). Skip if Taffy
          // hasn't placed it yet (mirrors sticky_offset's continue-on-miss).
          let Some(table_node) = tree.by_entity.get(&table) else {
              continue;
          };
          let Ok(table_layout) = tree.tree.layout(*table_node) else {
              continue;
          };
          let table_origin = Vec2::new(table_layout.location.x, table_layout.location.y);

          let (model, _deferred) = gather_table(table, &children_q, &display_q);
          if model.groups.is_empty() {
              continue;
          }

          // Per-row cell widths (from Taffy) + per-row heights (max cell
          // height in the row). Flat across groups, matching place order.
          let mut rows_widths: Vec<Vec<f32>> = Vec::new();
          let mut row_heights: Vec<f32> = Vec::new();
          for group in &model.groups {
              for row in &group.rows {
                  let mut widths: Vec<f32> = Vec::with_capacity(row.cells.len());
                  let mut max_h = 0.0f32;
                  for &cell in &row.cells {
                      let (w, h) = tree
                          .by_entity
                          .get(&cell)
                          .and_then(|n| tree.tree.layout(*n).ok())
                          .map(|l| (l.size.width, l.size.height))
                          .unwrap_or((0.0, 0.0));
                      widths.push(w);
                      max_h = max_h.max(h);
                  }
                  rows_widths.push(widths);
                  row_heights.push(max_h);
              }
          }

          let col_widths = resolve_column_widths(&rows_widths);
          let placed = place_table_cells(&model, &col_widths, &row_heights);
          for (entity, offset) in placed {
              overrides.by_entity.insert(entity, table_origin + offset);
          }
      }
  }
  ```
  **Implementer note:** the `gather_table` closure `gather_row` borrows `children_q` + `display_q` immutably — fine, both are read-only `Query` refs. `Children` derefs to `&[Entity]`; `row_kids.iter()` yields `&Entity` (use `&cell` pattern as shown, matching `sync_children_for_entity` at `systems.rs:1635`). The implicit-group `entity: table` reuses the table entity as the implicit group's identity — when bare rows exist, the table also gets a `(0,0)`-relative override (its own origin), which is a harmless no-op (`table_origin + (0,0) = table_origin`). Confirm `NonSend<LayoutTree>` import is already in scope (it is — `sticky_offset` uses it).

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core --test layout_table single_row_two_cells cell_size_comes_from_taffy
  ```
  Expected PASS.

- [ ] **Step 4: Delete the superseded stub tests.** In `crates/buiy_core/tests/layout_table_multicol_stubs.rs`, delete the six table-stub tests (`table_warns_once_per_entity_per_session`, `table_no_warn_when_no_table_entities`, `table_and_multicol_warns_are_independent`, `table_does_not_rewarn_on_component_replace`, `table_all_nine_variants_each_warn`) and their `// table_layout (sub-pass 6b)` + `// Cross-pass independence` section headers, plus the now-unused `Display`/`LayoutWarnOnceKey` imports if the multicol tests don't use them (keep `multicol_warns_once_per_session_regardless_of_entity_count`, `multicol_no_warn_when_no_multicol_entities`, `warned_once_session_manual_clear`). Verify `cargo test -p buiy_core --test layout_table_multicol_stubs` still compiles + passes the retained multicol tests. (Table behavior is now covered by `layout_table.rs`.)

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_table.rs crates/buiy_core/tests/layout_table_multicol_stubs.rs
  git commit -m "feat(layout): real table layout 6b — single-row column grid (Phase 12 — spec § 1.2)

gather_table walks the Children spine into a TableModel (implicit group for bare
rows, D6); table_layout resolves columns (synthetic flex), places cells, and
writes corrected positions to PostTaffyPositionOverrides (D1). Supersedes the
warn-once stub + its 6 stub tests.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 5: Multi-row column-width resolution end-to-end

**Spec:** § 1.2 step 2 (columns sized to the widest cell across rows), D2.

**Files:**
- Modify: `crates/buiy_core/tests/layout_table.rs` (add the multi-row integration test)

- [ ] **Step 1: Failing test.** Append to `crates/buiy_core/tests/layout_table.rs`:
  ```rust
  #[test]
  fn columns_size_to_widest_cell_across_rows() {
      // Row 0: cells 30 / 50.  Row 1: cells 70 / 20.
      // Column 0 = max(30,70) = 70; column 1 starts at x=70 for BOTH rows.
      let mut app = app();
      let r0c0 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let r0c1 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(50.0).height_px(20.0))).id();
      let r1c0 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(70.0).height_px(20.0))).id();
      let r1c1 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(20.0).height_px(20.0))).id();
      let row0 = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_children(&[r0c0, r0c1]).id();
      let row1 = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_children(&[r1c0, r1c1]).id();
      let _table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_children(&[row0, row1]).id();

      app.update();

      // Column 1 starts after the widest column-0 cell (70px) in BOTH rows.
      assert!((pos(&app, r0c1).x - 70.0).abs() < 0.5, "row 0 col 1 at x=70 (widest col 0)");
      assert!((pos(&app, r1c1).x - 70.0).abs() < 0.5, "row 1 col 1 also at x=70");
      assert_eq!(pos(&app, r0c0).x, 0.0);
      assert_eq!(pos(&app, r1c0).x, 0.0);
  }

  #[test]
  fn rows_stack_by_their_own_height() {
      // Row 0 cell height 25; row 1 cell height 40. Row 1 starts at y=25.
      let mut app = app();
      let r0 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(25.0))).id();
      let r1 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(40.0))).id();
      let row0 = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_child(r0).id();
      let row1 = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_child(r1).id();
      let _table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_children(&[row0, row1]).id();

      app.update();

      assert_eq!(pos(&app, r0).y, 0.0, "row 0 at top");
      assert!((pos(&app, r1).y - 25.0).abs() < 0.5, "row 1 below row 0 (25px tall)");
  }
  ```
  Run: `cargo test -p buiy_core --test layout_table columns_size_to_widest_cell rows_stack_by_their_own_height` — expected PASS already if T4's algorithm is correct (these exercise the multi-row path through the same code). If they FAIL, the multi-row aggregation in `table_layout` (or `resolve_column_widths`) has a bug — fix it (this task's purpose is to prove the multi-row path; no new production code is expected, but if a defect surfaces, the minimal fix lands here).

- [ ] **Step 2: Implementation.** No new production code is expected — T4's `table_layout` + `resolve_column_widths` already aggregate per-column max across rows and stack rows by height. If Step 1 surfaces a defect (e.g. row heights indexed wrong, or columns not maxed), apply the minimal fix in `systems.rs` and note it in the commit body.

- [ ] **Step 3: Run.**
  ```bash
  cargo test -p buiy_core --test layout_table columns_size_to_widest_cell rows_stack_by_their_own_height
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_table.rs crates/buiy_core/src/layout/systems.rs
  git commit -m "test(layout): multi-row table column-width + row-height resolution (Phase 12 — spec § 1.2 step 2)

Columns size to the widest cell across rows; rows stack by their own height. Pins
the multi-row path through table_layout + resolve_column_widths.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 6: Row-group stacking end-to-end

**Spec:** § 1.2 step 1 (row-group family), D5 (document-order stacking), D6.

**Files:**
- Modify: `crates/buiy_core/tests/layout_table.rs` (add the row-group integration test)

- [ ] **Step 1: Failing test.** Append to `crates/buiy_core/tests/layout_table.rs`:
  ```rust
  #[test]
  fn explicit_row_groups_stack_in_document_order() {
      // Table > [HeaderGroup > Row > Cell(h=20)], [RowGroup > Row > Cell(h=30)].
      // Header group's row at y=0; body group's row at y=20 (D5 — source order,
      // no header-floats-to-top reorder).
      let mut app = app();
      let hc = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(20.0))).id();
      let hrow = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_child(hc).id();
      let header = app.world_mut().spawn((Node, Display::TableHeaderGroup, Style::default())).add_child(hrow).id();

      let bc = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(30.0))).id();
      let brow = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_child(bc).id();
      let body = app.world_mut().spawn((Node, Display::TableRowGroup, Style::default())).add_child(brow).id();

      let _table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_children(&[header, body]).id();

      app.update();

      assert_eq!(pos(&app, hc).y, 0.0, "header group row at top");
      assert!((pos(&app, bc).y - 20.0).abs() < 0.5, "body group row below header (20px)");
      // Group entities sit at their first row's y.
      assert_eq!(pos(&app, header).y, 0.0);
      assert!((pos(&app, body).y - 20.0).abs() < 0.5);
  }

  #[test]
  fn cell_columns_align_across_groups() {
      // Two groups, each one row of two cells. Column 0 = max widths across
      // BOTH groups' rows; column 1 aligns across groups.
      let mut app = app();
      // group A row: 30 / 50
      let a0 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let a1 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(50.0).height_px(20.0))).id();
      let arow = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_children(&[a0, a1]).id();
      let ga = app.world_mut().spawn((Node, Display::TableRowGroup, Style::default())).add_child(arow).id();
      // group B row: 60 / 20  → column 0 = max(30,60) = 60
      let b0 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(60.0).height_px(20.0))).id();
      let b1 = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(20.0).height_px(20.0))).id();
      let brow = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_children(&[b0, b1]).id();
      let gb = app.world_mut().spawn((Node, Display::TableRowGroup, Style::default())).add_child(brow).id();

      let _table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_children(&[ga, gb]).id();

      app.update();

      assert!((pos(&app, a1).x - 60.0).abs() < 0.5, "group A col 1 at x=60 (widest col 0 across groups)");
      assert!((pos(&app, b1).x - 60.0).abs() < 0.5, "group B col 1 also at x=60");
  }
  ```
  Run: `cargo test -p buiy_core --test layout_table explicit_row_groups_stack cell_columns_align_across_groups` — expected PASS if T4's gather + place already handle explicit groups. If FAIL, the explicit-group branch of `gather_table` or the cross-group column aggregation has a defect — fix minimally here.

- [ ] **Step 2: Implementation.** No new production code expected — T4's `gather_table` handles explicit row-groups and `table_layout` aggregates `rows_widths` flat across groups (so columns align across groups). Apply a minimal fix only if Step 1 surfaces a defect.

- [ ] **Step 3: Run.**
  ```bash
  cargo test -p buiy_core --test layout_table explicit_row_groups_stack cell_columns_align_across_groups
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_table.rs crates/buiy_core/src/layout/systems.rs
  git commit -m "test(layout): table row-group stacking + cross-group column alignment (Phase 12 — spec § 1.2)

Explicit row-groups stack in document order (D5); columns align across all groups.
Pins the explicit-group path through gather_table + table_layout.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 7: Per-feature deferral warns (caption / column / ragged-span) + retire blanket `TableUnsupported`

**Spec:** § 1.2 (the warn now names the *specific* unsupported sub-feature, not "tables are unsupported"), § 6 (warn-once error model), D4, D7, D8.

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add two `LayoutWarnOnceKey` variants; deprecate-in-doc the now-unused `TableUnsupported`)
- Modify: `crates/buiy_core/src/layout/systems.rs` (emit the per-feature warns in `table_layout`; remove the now-dead `is_table_display`)
- Modify: `crates/buiy_core/tests/layout_table.rs` (warn-coverage tests)

- [ ] **Step 1: Failing tests.** Append to `crates/buiy_core/tests/layout_table.rs`:
  ```rust
  use buiy_core::layout::{LayoutWarnOnceKey, LayoutWarnedOnceSession};

  fn count_warns(app: &App, mut pred: impl FnMut(&LayoutWarnOnceKey) -> bool) -> usize {
      app.world()
          .resource::<LayoutWarnedOnceSession>()
          .set
          .iter()
          .filter(|k| pred(k))
          .count()
  }

  #[test]
  fn caption_warns_once_and_is_not_placed() {
      // A caption child is classified but deferred (D4): one warn, no
      // override (its position stays Taffy-block).
      let mut app = app();
      let cap = app.world_mut().spawn((Node, Display::TableCaption, Style::default().width_px(40.0).height_px(10.0))).id();
      let cell = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(40.0).height_px(20.0))).id();
      let row = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_child(cell).id();
      let _table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_children(&[cap, row]).id();

      app.update();
      app.update(); // second frame must NOT add another warn

      assert_eq!(
          count_warns(&app, |k| matches!(k, LayoutWarnOnceKey::TableSubfeatureUnsupported(e) if *e == cap)),
          1,
          "caption warns exactly once per (entity, session)",
      );
  }

  #[test]
  fn ragged_rows_warn_span_unsupported_once_per_table() {
      // Row 0 has 2 cells, row 1 has 1 → ragged (span-faking). One
      // TableSpanUnsupported warn for the table entity.
      let mut app = app();
      let a = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let b = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let c = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let row0 = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_children(&[a, b]).id();
      let row1 = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_child(c).id();
      let table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_children(&[row0, row1]).id();

      app.update();
      app.update();

      assert_eq!(
          count_warns(&app, |k| matches!(k, LayoutWarnOnceKey::TableSpanUnsupported(e) if *e == table)),
          1,
          "ragged table warns once per (table, session)",
      );
  }

  #[test]
  fn well_formed_table_emits_no_warns() {
      let mut app = app();
      let a = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let b = app.world_mut().spawn((Node, Display::TableCell, Style::default().width_px(30.0).height_px(20.0))).id();
      let row = app.world_mut().spawn((Node, Display::TableRow, Style::default())).add_children(&[a, b]).id();
      let _table = app.world_mut().spawn((Node, Display::Table, Style::default())).add_child(row).id();

      app.update();

      assert_eq!(
          count_warns(&app, |k| matches!(
              k,
              LayoutWarnOnceKey::TableSpanUnsupported(_) | LayoutWarnOnceKey::TableSubfeatureUnsupported(_)
          )),
          0,
          "a uniform, caption-free table produces no deferral warns",
      );
  }
  ```
  Run: `cargo test -p buiy_core --test layout_table caption_warns_once ragged_rows_warn well_formed_table_emits_no_warns` — expected FAIL (variants + warn logic don't exist).

- [ ] **Step 2a: Add the warn-once variants to `types.rs`.** Append to `LayoutWarnOnceKey` (after `MultipleFullscreenTopLayer`, `types.rs`), and update the doc on the now-unused `TableUnsupported`:
  ```rust
  /// A table has rows of differing cell counts (a ragged table). v1 has
  /// no `colspan`/`rowspan` API, so spanning cannot be expressed; ragged
  /// rows are laid out positionally (column index = cell index) and this
  /// warns once per (table entity, session) (plan D8).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  TableSpanUnsupported(Entity),

  /// A `Display::TableCaption` / `TableColumn` / `TableColumnGroup`
  /// entity was encountered. These table sub-features are deferred to
  /// v1.x; the entity is left at its Taffy-block position and not placed
  /// in the table grid. Warns once per (entity, session) (plan D4).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  TableSubfeatureUnsupported(Entity),
  ```
  Edit the `TableUnsupported(Entity)` doc comment to note it is retired:
  ```rust
  /// **Retired in Phase 12** — the blanket "tables are unsupported" warn
  /// from the Phase-7 stub. Sub-pass 6b now lays tables out; per-feature
  /// deferrals use `TableSpanUnsupported` / `TableSubfeatureUnsupported`.
  /// Kept as a variant for `Reflect`/serialization stability; no code
  /// emits it.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  TableUnsupported(Entity),
  ```
- [ ] **Step 2b: Emit the warns in `table_layout` + remove dead `is_table_display`.** In `table_layout` (T4), replace the `let _ = &mut warned;` line and the deferred-handling: after `gather_table`, for each entity in `deferred` warn via `TableSubfeatureUnsupported`; after building `rows_widths`, detect ragged rows and warn via `TableSpanUnsupported`. Concretely:
  ```rust
  let (model, deferred) = gather_table(table, &children_q, &display_q);
  for d in deferred {
      if warned
          .set
          .insert(LayoutWarnOnceKey::TableSubfeatureUnsupported(d))
      {
          bevy::log::warn!(
              "Layout: table sub-feature on entity {:?} (caption / column / column-group) \
               is deferred to v1.x (spec § 1.2); it is left at its block position.",
              d,
          );
      }
  }
  if model.groups.is_empty() {
      continue;
  }
  ```
  Then, after the `rows_widths` loop, before `resolve_column_widths`:
  ```rust
  // Ragged rows (differing cell counts) imply spanning, which has no
  // v1 API — lay out positionally + warn once per table (plan D8).
  let ragged = rows_widths
      .iter()
      .map(|r| r.len())
      .collect::<std::collections::HashSet<_>>()
      .len()
      > 1;
  if ragged
      && warned
          .set
          .insert(LayoutWarnOnceKey::TableSpanUnsupported(table))
  {
      bevy::log::warn!(
          "Layout: table {:?} has rows of differing cell counts; colspan/rowspan \
           is unsupported in v1 (spec § 1.2) — cells are placed positionally.",
          table,
      );
  }
  ```
  Remove the now-unused `is_table_display` function (`systems.rs:666-679`) — its only caller was the old stub body (T4 replaced it; `table_part` is the replacement). Confirm no other reference remains (`grep -rn is_table_display crates/`).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_table caption_warns_once ragged_rows_warn well_formed_table_emits_no_warns
  ```
  Expected PASS.

- [ ] **Step 4: Full table-suite + gate.**
  ```bash
  cargo test -p buiy_core --test layout_table
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Expected PASS.

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_table.rs
  git commit -m "feat(layout): per-feature table deferral warns; retire blanket TableUnsupported (Phase 12 — spec § 1.2, § 6)

Caption/column(-group) warn TableSubfeatureUnsupported once per (entity, session)
and stay at block position (D4); ragged (span-faking) tables warn
TableSpanUnsupported once per table (D8). The Phase-7 blanket TableUnsupported is
retired (kept as a no-emit variant for Reflect stability); dead is_table_display
removed.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Self-review (against the spec)

**Spec coverage** (`display-and-positioning.md` § 1.2):
- Step 1 "Gather entities by `Display::Table*` family" → T1 (`table_part` classifier), T4 (`gather_table` walks the spine; implicit group for bare rows — D6; explicit groups — T6). ✓
- Step 2 "Compute column widths via Taffy on a synthetic flex container per row group" → T2 (`resolve_column_widths` synthetic `TaffyTree` flex tree — D2), T4/T5 (per-column max across rows), T6 (cross-group alignment). ✓
- Step 3 "Write corrected positions back to `PostTaffyPositionOverrides`" → T3 (`place_table_cells` offsets), T4 (`overrides.by_entity.insert(entity, table_origin + offset)`; size untouched — D1, consumed by `write_resolved_layout` step 7). ✓
- "Keep the warn-once only for genuinely unsupported sub-features (document which)" → T7: `TableSubfeatureUnsupported` (caption/column/column-group — D4), `TableSpanUnsupported` (ragged/colspan — D8); blanket `TableUnsupported` retired. ✓
- Sub-pass 6b slot in the `PostTaffyOverrides` chain (architecture.md § 3) → unchanged; only the `table_layout` body changes (T4). ✓
- Decompose into incremental TDD tasks (chartering sketch: "first single-row single-cell geometry, then column-width resolution across rows, then row-group stacking") → T4 (single-row single-cell), T5 (column widths across rows), T6 (row-group stacking). ✓
- Fallback supersession: spec § 1.2 says the v1 fallback "is what actually runs in v1"; this phase replaces it (the docs flip — spec § 1.2 status + follow-ups entry — happens in a separate docs stage, not a code task here, per the plan-author charter). ✓

**Placeholder scan:** every task ships full test code + full implementation code + exact `cargo`/`git` commands. T5 and T6 are deliberately "test-only unless a defect surfaces" tasks (the multi-row + multi-group paths are exercised by code written in T4); each names the exact production file to touch *if* a defect appears, with no "TBD." The `resolve_column_widths` Taffy-API note (T2) lists concrete fallbacks (`set_children` instead of `new_with_children`; index stored `cell_nodes` instead of `child_at_index`) rather than leaving the API unverified. No "similar to Task N," no "add error handling."

**Type consistency:** `TablePart` (T1) — variants `Table`/`RowGroup`/`Row`/`Cell`/`Caption`/`Column`/`ColumnGroup`; `table_part(&Display) -> Option<TablePart>` (T1) used by `gather_table` (T4); `resolve_column_widths(&[Vec<f32>]) -> Vec<f32>` (T2) called by `table_layout` (T4); `TableModel`/`TableRowGroupModel`/`TableRowModel` (T3) produced by `gather_table` (T4) and consumed by `place_table_cells(&TableModel, &[f32], &[f32]) -> HashMap<Entity, Vec2>` (T3); `LayoutWarnOnceKey::{TableSpanUnsupported(Entity), TableSubfeatureUnsupported(Entity)}` (T7) emitted in `table_layout` (T7) and asserted in `layout_table.rs` (T7). `PostTaffyPositionOverrides.by_entity: HashMap<Entity, Vec2>` and `ResolvedLayout { position, size }` are used exactly as defined in `systems.rs:178` / `components.rs:24`. Every type/fn used in a later task is defined in an earlier one (T1→T2→T3→T4→T5→T6→T7).
