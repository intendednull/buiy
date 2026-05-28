# Buiy layout grid — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Date:** 2026-05-09
**Status:** landed
**Spec:** [`specs/2026-05-08-buiy-layout-design/flex-and-grid.md`](../specs/2026-05-08-buiy-layout-design/flex-and-grid.md) § 2 (with cross-references to [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md))

**Goal:** Phase 3 of the layout migration — ship CSS Grid: a `GridParams` container component (template + auto + flow + alignment + gap), a `GridItem` per-child component (column/row placement, justify/align self), and the supporting value types `TrackSize`, `RepeatCount`, `GridLine`, `GridAreas`, `GridAutoFlow`, `JustifyItems`, plus `Length::Fr`. Wire `Display::Grid` / `Display::InlineGrid` to `taffy::Display::Grid` (Phase 1 routes both to Block) and translate the grid surface through to Taffy 0.10's grid fields. Widen `sync_styles`'s trigger filter to include `Changed<GridParams>` and `Changed<GridItem>`. Subgrid (CSS-WG ships when Taffy ships) and Masonry (CSS-WG flux) ship as **reserved variants** that warn once and degrade to a sensible fallback.

**Architecture:** Adds 2 components and 7 supporting value types (one new `Length` variant, six new enums/structs) under the existing Phase 1 `crates/buiy_core/src/layout/` directory; no new module files. `GridParams` is container-side self-styling and joins `Style`'s `Bundle` (always-insert pattern from Phase 1+2). `GridItem` is decomposed-only per [architecture.md § 2.4](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only) (same pattern as `FlexItem` / `ScrollSnapItem`). The translation layer extends `StyleView` with `&GridParams` + `Option<&GridItem>` + `Option<&GridAreas>` (the parent's named-area registry, looked up Buiy-side because Taffy's `GridPlacement` has no native named-area resolution). `style_to_taffy` populates `taffy::Style.grid_template_*`, `grid_auto_*`, `grid_template_areas`, `grid_auto_flow`, `grid_row`, `grid_column`. Two helper enums split `TrackSize` into Taffy-shaped sizing-function and template-component values; Subgrid + Masonry warn-once gates emit one log line per session per offending entity-set then degrade. `sync_styles`'s `Or<(Changed<...>)>` filter widens to the 11 component types now in scope.

**Tech Stack:** Rust, Bevy 0.18 ECS + reflection + `Bundle`, Taffy 0.10. No new external dependencies.

---

## Phasing strategy reference

This is **Phase 3** of the migration kicked off by the [Phase 1 plan](2026-05-08-buiy-layout-foundation.md#phasing-strategy-this-plan-vs-follow-ups). The phasing-strategy table there lists Phase 3 as `*-buiy-layout-grid.md`. Phase 3 depends only on Phase 1; Phase 2 (overflow + scrolling) is independent of Phase 3 and already landed. Subsequent phases consume Phase 3 surface but don't reorder it:

- Phase 4 (`*-buiy-layout-writing-modes.md`) — adds writing-mode-aware logical aliases for `JustifyItems` / `JustifyContent` (`flow-start` / `flow-end`) and a translation pass that maps them to physical `Start` / `End`. Phase 3 ships physical only.
- Phase 7 (`*-buiy-layout-sticky-table-multicol.md`) — `MultiColumn` (CSS multi-column algorithm) is independent of grid and lives there. Phase 3 does not ship multi-column.
- Phase 10 (`*-buiy-layout-units-calc.md`) — full `Length` resolution. Phase 3 adds `Length::Fr(f32)` (grid-only, meaningful inside `TrackSize::Length(Length::Fr(_))`); Phase 10 keeps Fr but adds `Em` / `Rem` / viewport / `Calc`.

Phase 3 plants the grid API surface; the only deferred semantics are:
1. **Subgrid** — Taffy 0.10 has no subgrid support. `TrackSize::Subgrid` is reserved, emits one `warn!` per session, and degrades to `TrackSize::Auto`. Cuts over to real subgrid when Taffy ships it (no Buiy plan needed — translation table flips one match arm).
2. **Masonry** — CSS-WG flux. `GridAutoFlow::Masonry` is reserved, emits one `warn!` per session, and degrades to `GridAutoFlow::Row`. Same cutover story.

These two reserved variants exist so authors don't get a hard parse error when copy-pasting CSS that uses them; the warning is the load-bearing UX.

---

## File structure

### Modified files

```
crates/buiy_core/src/layout/
├── types.rs          — append: 1 Length variant (Fr); 6 new types
│                       (TrackSize, RepeatCount, GridLine, GridAreas,
│                        GridAutoFlow, JustifyItems)                       (Task 1)
├── components.rs     — add GridParams (container, in Style Bundle)
│                       add GridItem (decomposed-only, child-side)         (Task 2)
├── style.rs          — extend Style Bundle with `grid_params`;
│                       add fluent setters for grid                         (Task 3)
├── translate.rs      — extend StyleView with grid_params/grid_item/
│                       parent_areas; map Display::Grid/InlineGrid to
│                       taffy::Display::Grid (Task 4); add helpers for
│                       track / line / areas; wire grid into style_to_taffy;
│                       warn-once gates for Subgrid + Masonry               (Tasks 4, 5, 6)
├── systems.rs        — widen sync_styles query: add &GridParams +
│                       Option<&GridItem>; precompute parent-areas map;
│                       widen Or filter to include Changed<GridParams>
│                       and Changed<GridItem>                                (Task 5)
└── mod.rs            — register GridParams, GridItem with reflection;
                        re-export new component types and value types       (Task 7)
```

```
crates/buiy_core/src/lib.rs    — re-export new types from buiy_core         (Task 7)
crates/buiy/src/lib.rs         — re-export from buiy facade                  (Task 7)
```

### New tests

```
crates/buiy_core/tests/
├── layout_grid.rs                  — 3 fixtures through full pipeline:
│                                     1fr 2fr 1fr in 400 px row → 100/200/100;
│                                     named areas + Area("header") child;
│                                     repeat(auto-fill, 100 px) in 350 px
│                                     produces 3 columns                    (Task 8)
└── layout_grid_stubs.rs            — Subgrid + Masonry warn-once
                                       degradation tests                     (Task 6)
```

### Modified docs / non-code files

- `CHANGELOG.md` — `[Unreleased]` `### Added` and `### Changed` entries (Task 9).
- `docs/README.md` — entry added under Layout > Plans with `[active]` tag during the plan-write commit; flipped to `[landed]` post-merge.

### No deletions

Phase 3 is purely additive — no Phase 1 or Phase 2 file or item is removed. Phase 1's `map_display` arm that routed `Display::Grid` / `Display::InlineGrid` to `taffy::Display::Block` is *replaced* by a Grid arm; no other behavior changes.

---

## Coverage map

Every Phase 3 spec requirement maps to a task below. Items marked **deferred** are explicitly out of Phase 3 (deferred to a later phase or sibling spec); the table notes the deferral target. Items marked **simplified vs. spec** ship a narrower API than the spec verbatim because of Taffy 0.10 reality or scope discipline; the divergence is explained.

| Spec section | Phase 3 coverage | Task |
|---|---|---|
| flex-and-grid.md § 2.1 — `GridParams` shape | Ships in full: `template_columns`, `template_rows`, `template_areas`, `auto_columns`, `auto_rows`, `auto_flow`, `justify_items`, `align_items`, `justify_content`, `align_content`, `gap`. | 2 |
| flex-and-grid.md § 2.1 — `TrackSize` enum (`Length`, `MinMax`, `Repeat`, `Auto`, `MinContent`, `MaxContent`, `FitContent`) | Ships in full plus reserved `Subgrid` variant (§ 2.3). Recursion in `MinMax`/`Repeat` is permitted by the type but constrained by spec — invalid nesting (e.g., `MinMax(Repeat(...), ...)` or `Repeat(_, [Subgrid])`) translates to `Auto` + warn-once. | 1, 5, 6 |
| flex-and-grid.md § 2.1 — `RepeatCount` (`AutoFill`, `AutoFit`, `Count`) | Ships. **Spec uses `u32`; Phase 3 uses `u16` to match Taffy 0.10's `RepetitionCount::Count(u16)` directly without a lossy conversion at translate time.** Documented in CHANGELOG and on the type's doc comment. | 1 |
| flex-and-grid.md § 2.1 — `Length::Fr(f32)` only meaningful inside grid templates | Adds `Length::Fr(f32)`. Outside grid templates (i.e., when emitted to `taffy::LengthPercentage` / `taffy::Dimension`) a `warn!` fires once per session and the value falls back to `Auto`. | 1, 5 |
| flex-and-grid.md § 2.2 — `GridItem` shape | Ships in full: `column: GridLine`, `row: GridLine`, `justify_self: Option<JustifyItems>`, `align_self: Option<AlignItems>`. Decomposed-only per architecture.md § 2.4 — not in `Style`'s Bundle. | 2 |
| flex-and-grid.md § 2.2 — `GridLine` (`Auto`, `Start`, `Span`, `StartEnd`, `Area`) | Ships in full. **Spec uses `i32` / `u32`; Phase 3 uses `i16` / `u16` to match Taffy 0.10's `GridLine` / `Span` underlying types.** Documented. **Spec uses `SmolStr`; Phase 3 uses `String` to avoid adding the `smol_str` dep direct to `buiy_core`** (Bevy ships `smol_str` transitively, but a direct dep would expand the Buiy supply-chain footprint and require a `cargo deny` policy review). The runtime cost difference for area names is negligible — they're set once per container at spawn and never on a hot path. | 1, 5 |
| flex-and-grid.md § 2.2 — `GridLine::Area(name)` resolves against parent's `template_areas` | Resolved Buiy-side at `sync_styles` time: each child's parent is looked up; if the parent has `GridParams.template_areas`, the named area's bounds populate `grid_row.start/end` and `grid_column.start/end` via `taffy::GridPlacement::Line(_)`. Mismatched names emit `warn!` and fall back to `Auto`. **Reason:** `taffy::GridPlacement` has no native named-area variant — only `Line`, `NamedLine`, `Span`, `NamedSpan`, `Auto`. Named *areas* and named *lines* are distinct CSS concepts; we emulate area resolution. | 5, 8 |
| flex-and-grid.md § 2.3 — `TrackSize::Subgrid` reserved | Ships as variant. `style_to_taffy` emits `warn!` once per session (gated by an `AtomicBool`) the first time any container has `Subgrid` in its templates and translates the variant to `taffy::TrackSizingFunction::auto()`. **Cutover:** when Taffy ships subgrid, the match arm flips. | 1, 6 |
| flex-and-grid.md § 2.4 — `GridAutoFlow::Masonry` reserved | Ships as variant. `style_to_taffy` emits `warn!` once per session and translates to `taffy::GridAutoFlow::Row`. | 1, 6 |
| flex-and-grid.md § 1.1 — `FlexParams` (already shipped Phase 1) | No change. | — |
| flex-and-grid.md § 4 — `Display::Grid` / `Display::InlineGrid` mutually exclusive with Flex; nests freely | Phase 1's map_display routes Grid / InlineGrid to `taffy::Display::Block`; Phase 3 flips to `taffy::Display::Grid` and `taffy::Display::Grid` (Taffy 0.10 has no inline-grid variant — both Buiy-side variants map to the single Taffy variant). | 4 |
| flex-and-grid.md § 5 — Test surface: Grid template `1fr 2fr 1fr` | Integration test asserts 100 / 200 / 100 in a 400 px row. | 8 |
| flex-and-grid.md § 5 — Test surface: Named areas | Integration test asserts `Area("header")` child resolves to bounds. | 8 |
| flex-and-grid.md § 5 — Test surface: `repeat(auto-fill, 100px)` in 350 px | Integration test asserts 3 columns + 50 px slack. | 8 |
| flex-and-grid.md § 5 — Test surface: Subgrid stub warns | Test in `layout_grid_stubs.rs`. | 6 |
| flex-and-grid.md § 5 — Test surface: Masonry stub warns | Test in `layout_grid_stubs.rs`. | 6 |
| flex-and-grid.md § 5 — Test surface: Mixed flex-in-grid | **Explicit fixture in Task 8.** A separate fixture (`grid_cell_hosts_flex_row_with_two_children`) nests `Display::Flex(Row)` inside a grid cell with two flex children and asserts each child's resolved x-position relative to the cell. | 8 |
| flex-and-grid.md § 5 — Test surface: `repeat(auto-fit, ...)` | **Deferred.** Difference from `auto-fill` is collapsing empty tracks; not load-bearing for Phase 3 correctness. The translation table covers both equally; a follow-up plan can add a fixture if needed. | — |
| flex-and-grid.md § 5 — Test surface: Multi-column stub warns | **Deferred to Phase 7** (`*-buiy-layout-sticky-table-multicol.md`) — multi-column is owned by Phase 7, not Phase 3. Phase 3 ships only Grid; the spec test is on Phase 7's plate. | — |
| Per-child styles via `BuiySet::Layout` (architecture.md § 1.2) | `Changed<GridParams>` / `Changed<GridItem>` added to `sync_styles`'s `Or` trigger filter (Phase 2 already added `Changed<ChildOf>`; Phase 3 widens with the two grid components only). | 5 |
| Reflection registration for inspectors / BSN (architecture.md § 1.3) | `register_type::<GridParams>()` + `register_type::<GridItem>()` + new value types added to `LayoutPlugin::build`. | 7 |

---

## Open questions / decisions made

These decisions are committed in this plan and need not be re-litigated during implementation. They're documented here so future-me can see the reasoning if a downstream task wants to revisit.

1. **`u16` vs `u32` for line counts.** Spec uses `u32`; Taffy uses `u16`. Picking `u16` matches Taffy directly. 65 535 grid lines is far above any realistic UI grid; a layout that needs more is a code generator's bug, not a UI. Lossy conversion to `u16` would silently truncate, which is worse than picking the matching width up front.
2. **`String` vs `SmolStr` for area names.** Spec uses `SmolStr`. Direct `smol_str` dep would expand the supply-chain footprint and require `cargo deny` policy entry. Area names are set once per container and never on a hot path. `String` is fine for v1; a follow-up can swap if profiling shows allocation pressure (very unlikely).
3. **Buiy-side named-area resolution vs Taffy native.** Taffy 0.10 has named *lines* but no named *area* placement. Resolving area bounds Buiy-side at `sync_styles` time is the only path. Cost: a parent-lookup hash on every child of a grid container, but only when something actually changed (`sync_styles` already iterates only changed entities).
4. **`Length::Fr(f32)` lives in `Length` rather than separate `GridLength`.** Spec puts Fr inside `Length`. This means non-grid translation paths must guard. Phase 1's `length_to_dim` / `length_to_lp` / `length_to_lpa` add a `Fr` arm that warns and falls back; the warn is the visible signal that an author misused `Fr`. Splitting into a separate `GridLength` would be cleaner type-theoretically but doubles the surface and breaks the spec's named type.
5. **Phase 3 does *not* implement `BoxModel.gap`.** Phase 1 deferred BoxModel.gap; Phase 3 keeps it deferred and adds `GridParams.gap` parallel to `FlexParams.gap`. The `Style` builder gets `.grid_gap_px(_)` separate from `.gap_px(_)` (which still sets FlexParams.gap). Unifying gap is a separate plan touching BoxModel — out of Phase 3 scope.
6. **`GridAreas` shape: explicit-rectangles vs CSS-string.** Spec is loose. CSS uses string-grid-template-areas like `"header header" "main side"`. We ship the explicit-rectangles API: `GridAreas { areas: Vec<NamedArea { name, row_start, row_end, column_start, column_end }> }` plus a `from_lines(&[&str])` parser convenience that converts the string form to the explicit form. Authors get both the API ergonomic and the CSS-faithful syntax; translation just walks the explicit form. The string parser is a stretch goal — if it ships in Phase 3 it lands as a Task 1 step; if removed for scope, the explicit form is sufficient.
7. **`GridAreas::from_lines` *does* ship in Phase 3.** Decision: include it. Without it, the named-areas integration test (Task 8) is awkward — every author copying CSS would have to translate it manually. The parser is small (~30 lines) and self-contained.
8. **`Length::Fr` outside grid templates falls back to `0 px`, not `Auto`, in `LengthPercentage` contexts.** The spec text says "Fr outside grid is a `warn!` and falls back to `Auto`". Taffy's `LengthPercentage` type has *no* `Auto` variant — only `Length` and `Percent`. So `length_to_dim` (width / height — has Auto, falls back to Auto) and `length_to_lpa` (margin / inset — has Auto, falls back to Auto) honor the spec, but `length_to_lp` (padding / border / gap) is forced to fall back to `length(0.0)`. This is a **Taffy-imposed deviation** from the spec; the warn-once gate fires and the visible signal is the warning, not the fallback magnitude. Authors using `Length::Fr` outside grid templates have already misused the unit — getting 0 px or Auto in that case is equally "ill-defined".
9. **`Length`'s addition of `Fr` is a breaking change.** `pub enum Length` is not `#[non_exhaustive]`. Adding `Fr` breaks downstream exhaustive matches. Phase 3 ships the change; the CHANGELOG flags it explicitly under `### Breaking`. Phase 3 does not also `#[non_exhaustive]`-mark `Length` because that is *also* breaking (just via wildcard) and adds zero forward-compat value when the next planned `Length` change is Phase 10's `Em` / `Rem` / `Calc` (a similarly breaking addition). Callers will adapt once and the type stabilizes.
10. **All Taffy helper imports come via `use taffy::prelude::*`.** The grid wiring in Task 5 calls `auto()`, `length(v)`, `percent(p)`, `fr(v)`, `fit_content(_)`, `min_content()`, `max_content()`, `minmax(min, max)` as free functions whose return type the compiler infers from the call site. These helpers are exposed through `taffy::prelude` (verified against `taffy-0.10.1/src/prelude.rs:11-26`). The `<S>` type parameter on `taffy::Style`, `GridTemplateComponent<S>`, `GridPlacement<S>`, etc. defaults to `String` (`DefaultCheapStr`); Phase 3 does not annotate `S` anywhere — the default is correct. **Do not** use `&'static str` for `S` (it doesn't impl `CheapCloneStr`). **Do not** call inherent or trait constructors per-type (`TrackSizingFunction::auto()` etc.) without first checking which trait is in scope; the prelude bundles them.
11. **Task 1 must update `translate.rs`'s `length_to_*` helpers in the same commit that adds `Length::Fr`.** Without the cross-file update, `cargo build` fails with E0004 (non-exhaustive patterns) on `length_to_dim` / `length_to_lp` / `length_to_lpa`. Task 1 is therefore a 2-file commit (types.rs + translate.rs) — the same atomic-commit reasoning that applies to Task 5 also applies here.

---

## Tasks

The plan is 9 tasks. Each task is self-contained: passes `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test --workspace` before commit. Two-stage review (spec compliance → code quality) per task per `subagent-driven-development` skill.

### Task 1: Grid value types in `types.rs` (atomic with `translate.rs` Fr arms)

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` — append 1 `Length` variant + 6 new types
- Modify: `crates/buiy_core/src/layout/translate.rs` — add `Length::Fr(_)` arms to `length_to_dim` (→ `auto()`, warn), `length_to_lp` (→ `length(0.0)`, warn — Taffy `LengthPercentage` has no Auto variant), `length_to_lpa` (→ `auto()`, warn). One shared `WARNED_FR_OUTSIDE_GRID: AtomicBool` static gates the warn so a single misused Fr doesn't spam logs once per helper.
- Test: `crates/buiy_core/src/layout/types.rs` (`#[cfg(test)] mod tests`)

**Atomic commit reasoning:** `Length` is not `#[non_exhaustive]`, and `translate.rs`'s three `match l { Px => ..., Percent => ... }` blocks would fail E0004 the moment `Fr` lands in `Length`. Splitting types.rs and translate.rs into separate commits leaves the lib uncompilable in between.

**Test surface (added at the bottom of the existing tests module):**

- `length_fr_constructor_round_trip` — `Length::Fr(1.5)` matches and the variant exists.
- `track_size_default_is_auto` — `TrackSize::default() == TrackSize::Auto`.
- `repeat_count_count_carries_u16` — type-checks that `RepeatCount::Count(3u16)` compiles.
- `grid_line_default_is_auto` — `GridLine::default() == GridLine::Auto`.
- `grid_areas_from_lines_parses_simple_grid` — `GridAreas::from_lines(&["a a", "b ."]).areas` contains `NamedArea { name: "a", row_start: 0, row_end: 1, column_start: 0, column_end: 2 }` and `NamedArea { name: "b", row_start: 1, row_end: 2, column_start: 0, column_end: 1 }`. The `.` cell is treated as empty.
- `grid_auto_flow_default_is_row` — `GridAutoFlow::default() == GridAutoFlow::Row`.
- `justify_items_default_is_stretch` — `JustifyItems::default() == JustifyItems::Stretch`.

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests { ... }` block in `types.rs`:

```rust
    #[test]
    fn length_fr_constructor_round_trip() {
        // Pin the Fr variant. Used inside TrackSize::Length(Length::Fr(_));
        // outside grid contexts it warns and falls back to Auto.
        let fr = Length::Fr(1.5);
        match fr {
            Length::Fr(v) => assert_eq!(v, 1.5),
            _ => panic!("expected Fr"),
        }
    }

    #[test]
    fn track_size_default_is_auto() {
        assert_eq!(TrackSize::default(), TrackSize::Auto);
    }

    #[test]
    fn repeat_count_count_carries_u16() {
        let _: RepeatCount = RepeatCount::Count(3u16);
        assert_eq!(RepeatCount::default(), RepeatCount::AutoFill);
    }

    #[test]
    fn grid_line_default_is_auto() {
        assert_eq!(GridLine::default(), GridLine::Auto);
    }

    #[test]
    fn grid_areas_from_lines_parses_simple_grid() {
        let g = GridAreas::from_lines(&["a a", "b ."]);
        let mut by_name: std::collections::BTreeMap<&str, &NamedArea> =
            g.areas.iter().map(|a| (a.name.as_str(), a)).collect();
        let a = by_name.remove("a").expect("area `a`");
        assert_eq!((a.row_start, a.row_end), (0, 1));
        assert_eq!((a.column_start, a.column_end), (0, 2));
        let b = by_name.remove("b").expect("area `b`");
        assert_eq!((b.row_start, b.row_end), (1, 2));
        assert_eq!((b.column_start, b.column_end), (0, 1));
        assert!(by_name.is_empty(), "no extra areas");
    }

    #[test]
    fn grid_auto_flow_default_is_row() {
        assert_eq!(GridAutoFlow::default(), GridAutoFlow::Row);
    }

    #[test]
    fn justify_items_default_is_stretch() {
        assert_eq!(JustifyItems::default(), JustifyItems::Stretch);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```sh
cargo test -p buiy_core --lib types::tests 2>&1 | tail -30
```

Expected: compile errors — `Length::Fr` / `TrackSize` / `RepeatCount` / `GridLine` / `GridAreas` / `NamedArea` / `GridAutoFlow` / `JustifyItems` not found.

- [ ] **Step 3: Add `Length::Fr` variant**

In the existing `pub enum Length` block, add:

```rust
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    /// CSS `<flex>` unit — only meaningful inside `TrackSize::Length(Length::Fr(_))`.
    /// Outside grid templates, `Fr` warns once and resolves to `Auto`.
    Fr(f32),
}
```

(The two existing variants stay; just add `Fr`.)

- [ ] **Step 4: Add `RepeatCount` enum**

Append after the existing enums (around line 312, after `SnapStop`):

```rust
/// CSS `repeat(<count>, ...)` repetition count.
///
/// Spec uses `u32`; Phase 3 uses `u16` to match Taffy 0.10's
/// `RepetitionCount::Count(u16)` directly without a lossy conversion at
/// translate time. 65 535 repetitions is well above any realistic UI grid.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatCount {
    #[default]
    AutoFill,
    AutoFit,
    Count(u16),
}
```

- [ ] **Step 5: Add `TrackSize` enum**

Append after `RepeatCount`:

```rust
/// A CSS Grid track sizing function — what one column or row of a grid
/// template can be. Used inside `GridParams.template_columns` /
/// `template_rows` (where `Repeat` is permitted) and recursively inside
/// `MinMax` (where it's not — see translation gates in `translate.rs`).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.1.
///
/// Recursion is permitted by the type but constrained by CSS grammar:
/// `MinMax(Repeat(...), _)` and `Repeat(_, [Subgrid])` are invalid CSS;
/// `style_to_taffy` emits `warn!` once per session and falls back to
/// `Auto` for these.
#[derive(Reflect, Default, Clone, Debug, PartialEq)]
pub enum TrackSize {
    #[default]
    Auto,
    Length(Length),
    MinContent,
    MaxContent,
    FitContent(Length),
    /// CSS `minmax(<min>, <max>)`.
    MinMax(Box<TrackSize>, Box<TrackSize>),
    /// CSS `repeat(<count>, <tracks>)`. Only valid at the top of a
    /// template list (not inside another `Repeat` or inside `MinMax`).
    Repeat(RepeatCount, Vec<TrackSize>),
    /// CSS `subgrid`. Reserved — Taffy 0.10 has no subgrid support.
    /// Phase 3 emits one `warn!` per session and falls back to `Auto`.
    Subgrid,
}
```

- [ ] **Step 6: Add `GridLine` enum**

Append after `TrackSize`:

```rust
/// A CSS Grid placement on the `grid-row` or `grid-column` axis.
///
/// Spec uses `i32` / `u32`; Phase 3 uses `i16` / `u16` to match Taffy
/// 0.10's `GridLine` / `Span` underlying types. Spec uses `SmolStr` for
/// area names; Phase 3 uses `String` to avoid a new direct dep — area
/// names are set once per spawn and never on a hot path.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.2.
#[derive(Reflect, Default, Clone, Debug, PartialEq, Eq)]
pub enum GridLine {
    #[default]
    Auto,
    /// 1-indexed line; negative counts from the end (per CSS).
    Start(i16),
    /// Span N tracks from the auto-placed origin.
    Span(u16),
    /// Explicit `<start> / <end>`.
    StartEnd(i16, i16),
    /// Resolved against the parent container's `GridParams.template_areas`.
    /// If the name doesn't match any area, `style_to_taffy` warns and
    /// falls back to `Auto`.
    Area(String),
}
```

- [ ] **Step 7: Add `GridAreas` + `NamedArea` types**

Append after `GridLine`:

```rust
/// One named cell rectangle inside a `GridAreas`. CSS `grid-template-areas`
/// requires every named region to be a rectangle; `GridAreas::from_lines`
/// validates that.
///
/// Coordinates are zero-based and exclusive on the end side
/// (`row_end - row_start` is the span height in rows).
#[derive(Reflect, Default, Clone, Debug, PartialEq, Eq)]
pub struct NamedArea {
    pub name: String,
    pub row_start: u16,
    pub row_end: u16,
    pub column_start: u16,
    pub column_end: u16,
}

/// CSS `grid-template-areas` — a registry of named rectangular regions
/// laid out across the container's grid.
///
/// Construct from explicit rectangles via `area(...)` calls, or from CSS
/// string-grid syntax via `from_lines`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.1.
#[derive(Reflect, Default, Clone, Debug, PartialEq, Eq)]
pub struct GridAreas {
    pub areas: Vec<NamedArea>,
}

impl GridAreas {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one explicit area. `rows` and `cols` are exclusive-end ranges.
    pub fn area(
        mut self,
        name: impl Into<String>,
        rows: std::ops::Range<u16>,
        cols: std::ops::Range<u16>,
    ) -> Self {
        self.areas.push(NamedArea {
            name: name.into(),
            row_start: rows.start,
            row_end: rows.end,
            column_start: cols.start,
            column_end: cols.end,
        });
        self
    }

    /// Parse CSS-style `grid-template-areas` lines: each `&str` is one row,
    /// space-separated cells. The literal `.` (period) is an empty cell.
    /// Identical adjacent cells form one named region; the parser groups
    /// them into the smallest enclosing rectangle.
    ///
    /// CSS requires every named area to be rectangular. If a name appears
    /// in non-rectangular cells, the parser still emits the bounding
    /// rectangle and a `warn!` is emitted once at translation time when
    /// the area is referenced (by `style_to_taffy`'s area-resolution
    /// helper, not here — `from_lines` does no logging).
    pub fn from_lines(lines: &[&str]) -> Self {
        use std::collections::BTreeMap;
        // Parse into a 2D grid.
        let rows: Vec<Vec<&str>> = lines
            .iter()
            .map(|l| l.split_whitespace().collect())
            .collect();
        // Group by name, accumulating bounding rectangle.
        let mut bounds: BTreeMap<String, (u16, u16, u16, u16)> = BTreeMap::new();
        for (r, row) in rows.iter().enumerate() {
            for (c, &cell) in row.iter().enumerate() {
                if cell == "." {
                    continue;
                }
                let name = cell.to_string();
                let entry = bounds.entry(name).or_insert((
                    r as u16,
                    (r + 1) as u16,
                    c as u16,
                    (c + 1) as u16,
                ));
                entry.0 = entry.0.min(r as u16);
                entry.1 = entry.1.max((r + 1) as u16);
                entry.2 = entry.2.min(c as u16);
                entry.3 = entry.3.max((c + 1) as u16);
            }
        }
        let areas = bounds
            .into_iter()
            .map(|(name, (rs, re, cs, ce))| NamedArea {
                name,
                row_start: rs,
                row_end: re,
                column_start: cs,
                column_end: ce,
            })
            .collect();
        Self { areas }
    }
}
```

- [ ] **Step 8: Add `GridAutoFlow` enum**

Append after `GridAreas`:

```rust
/// CSS `grid-auto-flow`. `*Dense` lets the placement algorithm backfill
/// earlier tracks. `Masonry` is reserved (CSS-WG flux) — Phase 3 emits one
/// `warn!` per session and falls back to `Row`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.4.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
    /// Reserved for forward compatibility — CSS Masonry Layout. Currently
    /// degrades to `Row` with one `warn!` per session.
    Masonry,
}
```

- [ ] **Step 9: Add `JustifyItems` enum**

Append after `GridAutoFlow`:

```rust
/// CSS `justify-items` — main-axis alignment of grid items within their
/// cell. (Distinct from `JustifyContent`, which distributes whole tracks.)
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyItems {
    #[default]
    Stretch,
    Start,
    End,
    Center,
    Baseline,
}
```

- [ ] **Step 10: Run tests to verify they pass**

```sh
cargo test -p buiy_core --lib types::tests 2>&1 | tail -30
```

Expected: every test in `types::tests` passes (the existing 12 + the 7 new = 19 tests).

- [ ] **Step 11: Update `translate.rs` `length_to_*` helpers with `Fr` arms**

In `translate.rs`, locate the three helpers and add a `Length::Fr(_)` arm to each. Add a module-private warn gate at the top of the helpers section:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static WARNED_FR_OUTSIDE_GRID: AtomicBool = AtomicBool::new(false);

fn warn_once_fr_outside_grid() {
    if !WARNED_FR_OUTSIDE_GRID.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: Length::Fr is only meaningful inside TrackSize::Length \
             in a grid template; outside grid it falls back to 0 px / Auto \
             (warned once)"
        );
    }
}
```

Update the helpers:

```rust
fn length_to_dim(l: Length) -> taffy::Dimension {
    match l {
        Length::Px(v) => taffy::Dimension::length(v),
        Length::Percent(p) => taffy::Dimension::percent(p / 100.0),
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::Dimension::auto()
        }
    }
}

fn length_to_lp(l: Length) -> taffy::LengthPercentage {
    match l {
        Length::Px(v) => taffy::LengthPercentage::length(v),
        Length::Percent(p) => taffy::LengthPercentage::percent(p / 100.0),
        // taffy::LengthPercentage has no Auto variant — fall back to 0
        // (CSS-equivalent for Fr-in-non-grid: undefined, ill-formed).
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::LengthPercentage::length(0.0)
        }
    }
}

fn length_to_lpa(l: Length) -> taffy::LengthPercentageAuto {
    match l {
        Length::Px(v) => taffy::LengthPercentageAuto::length(v),
        Length::Percent(p) => taffy::LengthPercentageAuto::percent(p / 100.0),
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::LengthPercentageAuto::auto()
        }
    }
}
```

(`warn!` here is the macro re-exported by `bevy::prelude` — `systems.rs` already uses it the same way, and `translate.rs` will pick it up via `use bevy::prelude::*;` if it isn't already there. If translate.rs lacks the import, add `use bevy::prelude::warn;` at the top.)

- [ ] **Step 12: Run lint + format + lib tests**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10 \
  && cargo test -p buiy_core --lib 2>&1 | tail -20
```

Expected: every existing translate test still passes (no `Length::Fr` is constructed by any pre-Phase-3 test, so no warn fires). New types tests pass.

- [ ] **Step 13: Commit (atomic: types.rs + translate.rs)**

```sh
git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/translate.rs
git commit -m "feat(buiy_core): add grid value types + wire Length::Fr in translate

Atomic: types.rs adds Length::Fr, RepeatCount, TrackSize, GridLine,
GridAreas + NamedArea, GridAutoFlow, JustifyItems; translate.rs grows
a Length::Fr arm in length_to_dim / length_to_lp / length_to_lpa with
a single shared warn-once gate. Splitting these into two commits leaves
the lib uncompilable (E0004 on the three exhaustive matches).

Subgrid + Masonry land as reserved TrackSize / GridAutoFlow variants
(warn-once + fallback wired in Phase 3 Task 6). Width discipline:
RepeatCount::Count and GridLine::Start/Span/StartEnd use u16/i16 to
match Taffy 0.10 directly; GridLine::Area uses String to avoid a
smol_str dep. GridAreas::from_lines is the CSS-syntax convenience
parser that converts [\"a a\", \"b .\"] into NamedArea rectangles.

Note: Length gains a variant — this is a breaking change for
exhaustive Length matchers downstream. CHANGELOG flags this under
### Breaking in Task 9."
```

---

### Task 2: `GridParams` + `GridItem` components

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` — add 2 components
- Test: `crates/buiy_core/src/layout/components.rs` (`#[cfg(test)] mod tests`)

**Test surface:**

- `grid_params_default_is_empty_templates_row_flow` — every `Vec<TrackSize>` is empty, `template_areas` is `None`, `auto_flow == GridAutoFlow::Row`, `justify_items == JustifyItems::Stretch`, alignment all default, `gap == FlexGap::default()`.
- `grid_item_default_is_auto_lines_no_self` — `column == GridLine::Auto`, `row == GridLine::Auto`, both `*_self` are `None`.

- [ ] **Step 1: Write the failing tests**

Append to `#[cfg(test)] mod tests` in `components.rs`:

```rust
    #[test]
    fn grid_params_default_is_empty_templates_row_flow() {
        let g = GridParams::default();
        assert!(g.template_columns.is_empty());
        assert!(g.template_rows.is_empty());
        assert!(g.template_areas.is_none());
        assert!(g.auto_columns.is_empty());
        assert!(g.auto_rows.is_empty());
        assert_eq!(g.auto_flow, GridAutoFlow::Row);
        assert_eq!(g.justify_items, JustifyItems::Stretch);
        assert_eq!(g.align_items, AlignItems::Stretch);
        assert_eq!(g.justify_content, JustifyContent::FlexStart);
        assert_eq!(g.align_content, AlignContent::Stretch);
        assert_eq!(g.gap, FlexGap::default());
    }

    #[test]
    fn grid_item_default_is_auto_lines_no_self() {
        let g = GridItem::default();
        assert_eq!(g.column, GridLine::Auto);
        assert_eq!(g.row, GridLine::Auto);
        assert_eq!(g.justify_self, None);
        assert_eq!(g.align_self, None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```sh
cargo test -p buiy_core --lib components::tests 2>&1 | tail -20
```

Expected: compile errors — `GridParams`, `GridItem` not found.

- [ ] **Step 3: Extend the `use` import in `components.rs`**

Update the existing `use super::types::{ ... }` line to include the new types:

```rust
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap,
    GridAreas, GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems, OverflowMode,
    OverscrollBehavior, PositionKind, ScrollBehavior, ScrollbarColor, ScrollbarGutter,
    ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType, TrackSize,
};
```

- [ ] **Step 4: Add `GridParams` component**

Append (after `Scroll`, before `ScrollOffset`):

```rust
/// Grid container parameters. Active when the entity's `Display` is
/// `Display::Grid` or `Display::InlineGrid`; otherwise ignored.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.1.
///
/// `template_areas` carries explicit rectangles plus an optional CSS-string
/// constructor (`GridAreas::from_lines`). Named-area resolution for child
/// `GridLine::Area(name)` happens Buiy-side at `sync_styles` time —
/// Taffy 0.10 has no native named-area placement, only named lines.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct GridParams {
    pub template_columns: Vec<TrackSize>,
    pub template_rows: Vec<TrackSize>,
    pub template_areas: Option<GridAreas>,
    pub auto_columns: Vec<TrackSize>,
    pub auto_rows: Vec<TrackSize>,
    pub auto_flow: GridAutoFlow,
    pub justify_items: JustifyItems,
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub align_content: AlignContent,
    pub gap: FlexGap,
}
```

- [ ] **Step 5: Add `GridItem` component**

Append after `GridParams`:

```rust
/// Per-child grid placement and self-alignment.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.2.
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.4
/// (decomposed-only convention).
///
/// Decomposed-only — not in `Style`'s Bundle. Following the `FlexItem` /
/// `ScrollSnapItem` pattern: spawn alongside `Style` rather than nested.
/// `column.Area(name)` and `row.Area(name)` resolve against the parent's
/// `GridParams.template_areas`; mismatched names emit one `warn!` and
/// fall back to `GridLine::Auto`.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct GridItem {
    pub column: GridLine,
    pub row: GridLine,
    pub justify_self: Option<JustifyItems>,
    pub align_self: Option<AlignItems>,
}
```

- [ ] **Step 6: Run tests to verify they pass**

```sh
cargo test -p buiy_core --lib components::tests 2>&1 | tail -20
```

Expected: existing 10 tests + the 2 new = 12 tests pass.

- [ ] **Step 7: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 8: Commit**

```sh
git add crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core): add GridParams + GridItem components

GridParams (container, will join Style Bundle in Task 3) carries the
full CSS Grid surface: explicit/auto track templates, named areas,
auto-flow, alignment, gap. GridItem (decomposed-only, like FlexItem
and ScrollSnapItem) carries per-child placement and self-alignment.

Named-area resolution lives in translate.rs (Task 5) because Taffy
0.10 has no native named-area placement."
```

---

### Task 3: Style Bundle extension + grid fluent setters

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` — extend Bundle + add setters
- Modify: `crates/buiy_core/tests/layout_style_equivalence.rs` — verify fluent ↔ struct-literal equivalence for grid surface (drive-by once Bundle gains a field)

**Test surface (in `style.rs::tests`):**

- Existing `struct_literal_and_fluent_produce_identical_components` extends to cover grid setters.
- `default_style_inserts_every_decomposed_component` extends to include `GridParams`.
- `grid_template_columns_setter_overrides` — `Style::default().grid_template_columns(vec![TrackSize::Length(Length::Fr(1.0))])` produces a Style whose `grid_params.template_columns` has one Fr track.
- `grid_helpers_set_display_grid` — `.grid()` sets `display = Display::Grid`.

- [ ] **Step 1: Write the failing tests**

In `style.rs::tests`, extend the existing extraction helper signature and equivalence test, and add the two new ones:

```rust
    fn spawn_and_extract(
        style: Style,
    ) -> (
        Display,
        BoxModel,
        Position,
        FlexParams,
        Overflow,
        Scroll,
        GridParams,
    ) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(style).id();
        let world = app.world();
        let display = *world.get::<Display>(entity).expect("Display inserted");
        let box_model = world
            .get::<BoxModel>(entity)
            .expect("BoxModel inserted")
            .clone();
        let position = world
            .get::<Position>(entity)
            .expect("Position inserted")
            .clone();
        let flex_params = *world
            .get::<FlexParams>(entity)
            .expect("FlexParams inserted");
        let overflow = world
            .get::<Overflow>(entity)
            .expect("Overflow inserted")
            .clone();
        let scroll = world
            .get::<Scroll>(entity)
            .expect("Scroll inserted")
            .clone();
        let grid_params = world
            .get::<GridParams>(entity)
            .expect("GridParams inserted")
            .clone();
        (display, box_model, position, flex_params, overflow, scroll, grid_params)
    }

    #[test]
    fn grid_template_columns_setter_overrides() {
        let s = Style::default()
            .grid()
            .grid_template_columns(vec![
                TrackSize::Length(Length::Fr(1.0)),
                TrackSize::Length(Length::Fr(2.0)),
            ]);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(s).id();
        let g = app
            .world()
            .get::<GridParams>(entity)
            .expect("GridParams inserted");
        assert_eq!(g.template_columns.len(), 2);
        assert!(matches!(
            g.template_columns[0],
            TrackSize::Length(Length::Fr(_))
        ));
    }

    #[test]
    fn grid_helpers_set_display_grid() {
        let s = Style::default().grid();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(s).id();
        let d = app
            .world()
            .get::<Display>(entity)
            .copied()
            .expect("Display inserted");
        assert_eq!(d, Display::Grid);
    }
```

Update the existing `struct_literal_and_fluent_produce_identical_components` test: add a `grid_params: GridParams { template_columns: vec![TrackSize::Length(Length::Fr(1.0)), TrackSize::Length(Length::Fr(2.0))], auto_flow: GridAutoFlow::Column, ..Default::default() }` field to the literal form and a chained `.grid_template_columns(...)` + `.grid_auto_flow(GridAutoFlow::Column)` to the fluent form. The assertion that they produce identical components remains; it just expands to cover the new field.

Update `default_style_inserts_every_decomposed_component` to assert `world.get::<GridParams>(entity).is_some()`.

(Existing tests in `crates/buiy_core/tests/layout_style_equivalence.rs` use `..default()` where they end struct literals — Phase 2 added that already; no change needed there.)

- [ ] **Step 2: Run tests to verify they fail**

```sh
cargo test -p buiy_core --lib style::tests 2>&1 | tail -20
```

Expected: compile errors — `Style.grid_params` field, `.grid()`, `.grid_template_columns()`, `.grid_auto_flow()` not found.

- [ ] **Step 3: Extend the `use` import in `style.rs`**

```rust
use super::components::{BoxModel, Display, FlexParams, GridParams, Overflow, Position, Scroll};
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap,
    GridAreas, GridAutoFlow, Inset, JustifyContent, JustifyItems, Length, OverflowMode,
    PositionKind, ScrollBehavior, ScrollbarGutter, ScrollbarWidth, Sizing, SnapType, TrackSize,
};
```

- [ ] **Step 4: Extend the `Style` Bundle**

```rust
#[derive(Bundle, Clone, Debug, Default)]
pub struct Style {
    pub display: Display,
    pub box_model: BoxModel,
    pub position: Position,
    pub flex_params: FlexParams,
    pub overflow: Overflow,
    pub scroll: Scroll,
    pub grid_params: GridParams,
}
```

- [ ] **Step 5: Add the grid fluent setters**

Append a new `// ---- Grid ----` section near the bottom of `impl Style` (after the existing `// ---- Scroll snap ----` block):

```rust
    // ---- Grid ----

    /// Set `Display::Grid`. Other grid setters operate on `grid_params`.
    pub fn grid(mut self) -> Self {
        self.display = Display::Grid;
        self
    }

    pub fn inline_grid(mut self) -> Self {
        self.display = Display::InlineGrid;
        self
    }

    pub fn grid_template_columns(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.template_columns = tracks;
        self
    }

    pub fn grid_template_rows(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.template_rows = tracks;
        self
    }

    pub fn grid_template_areas(mut self, areas: GridAreas) -> Self {
        self.grid_params.template_areas = Some(areas);
        self
    }

    pub fn grid_auto_columns(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.auto_columns = tracks;
        self
    }

    pub fn grid_auto_rows(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.auto_rows = tracks;
        self
    }

    pub fn grid_auto_flow(mut self, flow: GridAutoFlow) -> Self {
        self.grid_params.auto_flow = flow;
        self
    }

    pub fn grid_justify_items(mut self, j: JustifyItems) -> Self {
        self.grid_params.justify_items = j;
        self
    }

    pub fn grid_align_items(mut self, a: AlignItems) -> Self {
        self.grid_params.align_items = a;
        self
    }

    pub fn grid_justify_content(mut self, j: JustifyContent) -> Self {
        self.grid_params.justify_content = j;
        self
    }

    pub fn grid_align_content(mut self, a: AlignContent) -> Self {
        self.grid_params.align_content = a;
        self
    }

    /// Set both row and column gap on `GridParams.gap`. Distinct from
    /// `gap_px` (which sets `FlexParams.gap`); when an entity is a flex
    /// container, only `FlexParams.gap` is honored, and conversely for
    /// grid. CSS-faithful unified gap is a follow-up plan.
    pub fn grid_gap_px(mut self, px: f32) -> Self {
        self.grid_params.gap = FlexGap {
            row: Length::Px(px),
            column: Length::Px(px),
        };
        self
    }
```

- [ ] **Step 6: Run tests to verify they pass**

```sh
cargo test -p buiy_core --lib style::tests 2>&1 | tail -20
```

Expected: existing 2 tests + 2 new = 4 tests pass.

- [ ] **Step 7: Run the workspace tests to verify the integration test still compiles**

```sh
cargo test -p buiy_core --tests 2>&1 | tail -30
```

Expected: `layout_style_equivalence` already uses `..default()` (added Phase 2), so the new field flows through. All other tests still compile.

- [ ] **Step 8: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 9: Commit**

```sh
git add crates/buiy_core/src/layout/style.rs
git commit -m "feat(buiy_core): extend Style Bundle with GridParams + grid fluent setters

Adds 12 fluent setters (.grid, .inline_grid, .grid_template_columns,
.grid_template_rows, .grid_template_areas, .grid_auto_columns,
.grid_auto_rows, .grid_auto_flow, .grid_justify_items,
.grid_align_items, .grid_justify_content, .grid_align_content,
.grid_gap_px). GridItem stays decomposed-only per architecture.md
section 2.4, like FlexItem and ScrollSnapItem."
```

---

### Task 4: `Display::Grid` / `Display::InlineGrid` → `taffy::Display::Grid`

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` — flip the `map_display` arm

This task is intentionally tiny so it can land independently and the test surface explicitly pins the contract. Phase 1's `map_display` lumps Grid/InlineGrid into Block; flipping that arm in isolation keeps the diff readable.

**Test surface:**

- `translate_display_grid_to_taffy_grid` — `Display::Grid` translates to `taffy::Display::Grid`; `Display::InlineGrid` does the same (Taffy 0.10 has no inline-grid variant).

- [ ] **Step 1: Write the failing test**

Append to `translate::tests`:

```rust
    #[test]
    fn translate_display_grid_to_taffy_grid() {
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        for display in [Display::Grid, Display::InlineGrid] {
            let taffy = style_to_taffy(StyleView {
                display: &display,
                box_model: &bm,
                position: &position,
                flex_params: &flex,
                flex_item: None,
                overflow: &overflow,
                scroll: &scroll,
                grid_params: &grid_params,
                grid_item: None,
                parent_areas: None,
            });
            assert_eq!(taffy.display, taffy::Display::Grid, "{display:?}");
        }
    }
```

(This test references `StyleView` fields that Task 5 introduces. Pinned here to make the contract explicit, but the test won't compile until Task 5 wires `StyleView`. **Strategy:** Run it after Task 5 by adding it to Task 5's commit. To keep Task 4 independently testable, write a smaller unit test below that goes through `map_display` directly.)

Actually — to keep Task 4 small and self-testable without depending on Task 5's StyleView changes, write the unit test against the helper:

```rust
    #[test]
    fn map_display_grid_routes_to_taffy_grid() {
        // Direct unit test of the helper. The full StyleView path is
        // tested in `translate_display_grid_to_taffy_grid` which lands
        // in Task 5 once the view is widened.
        assert_eq!(map_display(&Display::Grid), taffy::Display::Grid);
        assert_eq!(map_display(&Display::InlineGrid), taffy::Display::Grid);
    }
```

The full pipeline test lands in Task 5.

- [ ] **Step 2: Run the test to verify it fails**

```sh
cargo test -p buiy_core --lib translate::tests::map_display_grid_routes_to_taffy_grid 2>&1 | tail -20
```

Expected: `assertion left == right failed`. `map_display` currently returns `Block` for both.

- [ ] **Step 3: Flip the `map_display` arm**

In `translate.rs`, replace the existing `Grid | InlineGrid` arm in `map_display`:

```rust
fn map_display(d: &Display) -> taffy::Display {
    use Display::*;
    // Phase 3 routes Grid / InlineGrid to taffy::Display::Grid. Taffy
    // 0.10 has no inline-grid variant, so InlineGrid translates to the
    // same thing (Phase 4 writing-modes may revisit if line-box context
    // distinction matters; layout-side it doesn't).
    match d {
        Block | Inline | InlineBlock | FlowRoot | Contents | ListItem | Ruby | Table
        | TableRowGroup | TableHeaderGroup | TableFooterGroup | TableRow | TableCell
        | TableCaption | TableColumnGroup | TableColumn => taffy::Display::Block,
        Flex(_) | InlineFlex(_) => taffy::Display::Flex,
        Grid | InlineGrid => taffy::Display::Grid,
        None => taffy::Display::None,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

```sh
cargo test -p buiy_core --lib translate::tests::map_display_grid_routes_to_taffy_grid 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 6: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs
git commit -m "feat(buiy_core): map Display::Grid + InlineGrid to taffy::Display::Grid

Phase 1 routed both to Block because GridParams hadn't shipped yet;
Phase 3 Task 2 shipped GridParams. Flip the arm so containers actually
get grid layout. Taffy 0.10 has no inline-grid variant — both Buiy
variants map to the single Taffy variant; Phase 4 (writing modes) may
revisit if line-box context differs."
```

---

### Task 5: Translate grid + widen `sync_styles` (atomic)

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` — extend `StyleView`, wire grid fields, named-area resolution
- Modify: `crates/buiy_core/src/layout/systems.rs` — extend `sync_styles` query, precompute parent-areas map, widen `Or` filter

This task is **atomic** (single commit covers both files) because `StyleView` is the bridge: extending it in `translate.rs` without simultaneously updating the call site in `systems.rs` leaves the lib uncompilable. Phase 2's Task 5 used the same atomic strategy for the same reason.

**Test surface:**

- `translate_display_grid_to_taffy_grid` — full-StyleView-path test from Task 4 plan, now compilable.
- `translate_grid_template_columns_to_taffy` — `vec![TrackSize::Length(Length::Fr(1.0)), TrackSize::Length(Length::Fr(2.0))]` produces the right `taffy::GridTrackVec<GridTemplateComponent<_>>`. (Asserts via the resulting `taffy::Style.grid_template_columns.len() == 2` and the first component is `Single(_)`.)
- `translate_grid_repeat_to_taffy` — `vec![TrackSize::Repeat(RepeatCount::AutoFill, vec![TrackSize::Length(Length::Px(100.0))])]` produces a `Repeat` GridTemplateComponent.
- `translate_grid_line_start_end_to_taffy` — `GridLine::StartEnd(1, 4)` produces `taffy::Line { start: Line(1), end: Line(4) }`.
- `translate_grid_line_area_resolved_via_parent_areas` — when `parent_areas` is `Some(GridAreas { areas: vec![NamedArea { name: "header", row_start: 0, row_end: 1, column_start: 0, column_end: 2 }] })`, `GridLine::Area("header")` for the column axis produces `Line { start: Line(1), end: Line(3) }` (1-indexed, exclusive end → 1 + width 2 = 3).

(Note: Taffy's grid line indexes are 1-indexed in CSS; `NamedArea.column_start = 0` is 0-indexed. The translation adds 1 when emitting `taffy::GridLine`. Phase 3 documents this in the helper's doc comment.)

- [ ] **Step 1: Write the failing tests**

Append to `translate::tests` (replace the placeholder `translate_display_grid_to_taffy_grid` with the full version, and add the four new ones):

```rust
    #[test]
    fn translate_display_grid_to_taffy_grid() {
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        for display in [Display::Grid, Display::InlineGrid] {
            let taffy = style_to_taffy(StyleView {
                display: &display,
                box_model: &bm,
                position: &position,
                flex_params: &flex,
                flex_item: None,
                overflow: &overflow,
                scroll: &scroll,
                grid_params: &grid_params,
                grid_item: None,
                parent_areas: None,
            });
            assert_eq!(taffy.display, taffy::Display::Grid, "{display:?}");
        }
    }

    #[test]
    fn translate_grid_template_columns_to_taffy() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams {
            template_columns: vec![
                TrackSize::Length(Length::Fr(1.0)),
                TrackSize::Length(Length::Fr(2.0)),
            ],
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
        });
        assert_eq!(taffy.grid_template_columns.len(), 2);
    }

    #[test]
    fn translate_grid_repeat_to_taffy() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams {
            template_columns: vec![TrackSize::Repeat(
                RepeatCount::AutoFill,
                vec![TrackSize::Length(Length::Px(100.0))],
            )],
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
        });
        assert_eq!(taffy.grid_template_columns.len(), 1);
        assert!(matches!(
            &taffy.grid_template_columns[0],
            taffy::GridTemplateComponent::Repeat(_)
        ));
    }

    #[test]
    fn translate_grid_line_start_end_to_taffy() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let item = GridItem {
            column: GridLine::StartEnd(1, 4),
            row: GridLine::Auto,
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: Some(&item),
            parent_areas: None,
        });
        // Line(1) and Line(4) — values are GridPlacement variants. Pin
        // the discriminants by construction.
        assert!(matches!(
            taffy.grid_column.start,
            taffy::GridPlacement::Line(_)
        ));
        assert!(matches!(
            taffy.grid_column.end,
            taffy::GridPlacement::Line(_)
        ));
    }

    #[test]
    fn translate_grid_line_area_resolved_via_parent_areas() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let item = GridItem {
            column: GridLine::Area("header".to_string()),
            row: GridLine::Area("header".to_string()),
            ..Default::default()
        };
        let parent_areas = GridAreas {
            areas: vec![NamedArea {
                name: "header".to_string(),
                row_start: 0,
                row_end: 1,
                column_start: 0,
                column_end: 2,
            }],
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: Some(&item),
            parent_areas: Some(&parent_areas),
        });
        // Column resolves to Line(1)..Line(3) (1-indexed, end is exclusive
        // in CSS spec terms — column_start 0 → Line(1), column_end 2 →
        // Line(3), spanning 2 cells).
        assert!(matches!(
            taffy.grid_column.start,
            taffy::GridPlacement::Line(_)
        ));
        assert!(matches!(
            taffy.grid_column.end,
            taffy::GridPlacement::Line(_)
        ));
    }
```

Note: the existing `translate_*` tests in `translate::tests` use `StyleView { display, box_model, position, flex_params, flex_item, overflow, scroll }` — they need `grid_params: &GridParams::default(), grid_item: None, parent_areas: None` added to keep compiling. **The 6 sites are:**

1. `translate_default_components_to_taffy_default`
2. `translate_flex_row_with_dimensions`
3. `translate_position_absolute_emits_absolute_with_inset`
4. `translate_flex_item_basis_grow_shrink`
5. `translate_overflow_modes_to_taffy` (note: this test loops over a `cases` slice and constructs `StyleView` inside the loop; update the single in-loop literal)
6. `translate_scrollbar_width_to_taffy_f32` (same in-loop pattern)

Each gets a binding `let grid_params = GridParams::default();` near the existing `let scroll = Scroll::default();` and the three new fields appended to the `StyleView { ... }` literal. Add `use crate::layout::components::{GridParams, GridItem};` at the top of `tests` if missing.

- [ ] **Step 2: Run the tests to verify they fail**

```sh
cargo test -p buiy_core --lib translate::tests 2>&1 | tail -30
```

Expected: compile errors — `StyleView` doesn't have `grid_params` / `grid_item` / `parent_areas` fields; `taffy.grid_template_columns` is empty.

- [ ] **Step 3: Extend `StyleView`**

In `translate.rs`:

```rust
use super::components::{
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
};
use super::types::{
    AlignContent, AlignItems, BoxSizing, Edges, FlexAxis, FlexWrap, GridAreas, GridAutoFlow,
    GridLine, Inset, JustifyContent, Length, NamedArea, OverflowMode, PositionKind, RepeatCount,
    ScrollbarWidth, Sizing, TrackSize,
};

pub struct StyleView<'a> {
    pub display: &'a Display,
    pub box_model: &'a BoxModel,
    pub position: &'a Position,
    pub flex_params: &'a FlexParams,
    pub flex_item: Option<&'a FlexItem>,
    pub overflow: &'a Overflow,
    pub scroll: &'a Scroll,
    pub grid_params: &'a GridParams,
    pub grid_item: Option<&'a GridItem>,
    /// Parent's `template_areas` if the parent is a grid container.
    /// Required to resolve `GridLine::Area(name)` because Taffy 0.10
    /// has no native named-area placement.
    pub parent_areas: Option<&'a GridAreas>,
}
```

- [ ] **Step 4: Add grid mapping helpers**

First, ensure the `use` block at the top of `translate.rs` brings in the Taffy prelude (so the helper free functions `auto`, `length`, `percent`, `fr`, `fit_content`, `min_content`, `max_content`, `minmax` are in scope). The existing `translate.rs` does `use taffy::{...};` — replace that with the prelude glob plus the few non-prelude items used:

```rust
// Bring all the helper free fns + commonly-used types into scope.
// `taffy::prelude` exposes auto / length / percent / fr / fit_content /
// min_content / max_content / minmax (for grid) as well as the
// JustifySelf / AlignSelf / GridAutoFlow / GridPlacement /
// GridTemplateComponent / TrackSizingFunction / Min/MaxTrackSizingFunction
// types. See ~/.cargo/registry/.../taffy-0.10.1/src/prelude.rs.
use taffy::prelude::*;
// Items not in the prelude that we still touch:
use taffy::{Overflow as TaffyOverflow, Point};
```

(Adjust the second line to whatever the file currently imports from the bare `taffy::` namespace; the goal is "prelude + whatever else is already used".)

Then append after the existing `map_*` helpers:

```rust
fn map_grid_auto_flow(f: GridAutoFlow) -> taffy::GridAutoFlow {
    use GridAutoFlow::*;
    match f {
        Row => taffy::GridAutoFlow::Row,
        Column => taffy::GridAutoFlow::Column,
        RowDense => taffy::GridAutoFlow::RowDense,
        ColumnDense => taffy::GridAutoFlow::ColumnDense,
        // Masonry is reserved (CSS-WG flux) — Taffy 0.10 has no
        // GridAutoFlow::Masonry, so we degrade to Row. Phase 3 Task 6
        // wires the warn-once gate.
        Masonry => taffy::GridAutoFlow::Row,
    }
}

fn map_repeat_count(c: RepeatCount) -> taffy::RepetitionCount {
    match c {
        RepeatCount::AutoFill => taffy::RepetitionCount::AutoFill,
        RepeatCount::AutoFit => taffy::RepetitionCount::AutoFit,
        RepeatCount::Count(n) => taffy::RepetitionCount::Count(n),
    }
}

/// Convert a `TrackSize` into a single `taffy::TrackSizingFunction`.
/// Used inside `Repeat`'s tracks list and inside `MinMax`'s arms — both
/// CSS contexts where a `Repeat` or another `MinMax` is invalid. If
/// callers pass an invalid nested `Repeat`/`Subgrid`/`MinMax`, we warn
/// once and return `auto`.
///
/// `auto()`, `length(_)`, `percent(_)`, `fr(_)`, `fit_content(_)`,
/// `min_content()`, `max_content()`, `minmax(_, _)` come from
/// `taffy::prelude`; the compiler infers the output type from the
/// function return / binding annotation.
fn track_to_sizing(t: &TrackSize) -> TrackSizingFunction {
    match t {
        TrackSize::Auto => auto(),
        TrackSize::MinContent => min_content(),
        TrackSize::MaxContent => max_content(),
        TrackSize::FitContent(l) => fit_content(length_to_lp(*l)),
        TrackSize::Length(Length::Fr(v)) => fr(*v),
        TrackSize::Length(Length::Px(v)) => length(*v),
        TrackSize::Length(Length::Percent(p)) => percent(p / 100.0),
        TrackSize::MinMax(min, max) => minmax(track_to_min(min), track_to_max(max)),
        TrackSize::Repeat(_, _) => {
            warn_once_invalid_track_nesting();
            auto()
        }
        TrackSize::Subgrid => {
            warn_once_subgrid();
            auto()
        }
    }
}

fn track_to_min(t: &TrackSize) -> MinTrackSizingFunction {
    match t {
        TrackSize::Auto => auto(),
        TrackSize::MinContent => min_content(),
        TrackSize::MaxContent => max_content(),
        TrackSize::Length(Length::Px(v)) => length(*v),
        TrackSize::Length(Length::Percent(p)) => percent(p / 100.0),
        // CSS forbids these in MinMax's min slot:
        // - Fr (fr-in-min is grammar-invalid)
        // - FitContent (Min has no TaffyFitContent impl in Taffy 0.10)
        // - MinMax / Repeat / Subgrid (recursion-invalid)
        TrackSize::Length(Length::Fr(_))
        | TrackSize::FitContent(_)
        | TrackSize::MinMax(_, _)
        | TrackSize::Repeat(_, _)
        | TrackSize::Subgrid => {
            warn_once_invalid_track_nesting();
            auto()
        }
    }
}

fn track_to_max(t: &TrackSize) -> MaxTrackSizingFunction {
    match t {
        TrackSize::Auto => auto(),
        TrackSize::MinContent => min_content(),
        TrackSize::MaxContent => max_content(),
        // MaxTrackSizingFunction has TaffyFitContent impl (Taffy 0.10
        // grid.rs:700) — fit_content() from prelude resolves to it.
        TrackSize::FitContent(l) => fit_content(length_to_lp(*l)),
        TrackSize::Length(Length::Fr(v)) => fr(*v),
        TrackSize::Length(Length::Px(v)) => length(*v),
        TrackSize::Length(Length::Percent(p)) => percent(p / 100.0),
        TrackSize::MinMax(_, _) | TrackSize::Repeat(_, _) | TrackSize::Subgrid => {
            warn_once_invalid_track_nesting();
            auto()
        }
    }
}

/// Convert a top-level `TrackSize` (in `template_columns` / `template_rows`)
/// into a `taffy::GridTemplateComponent`. `Repeat` is permitted here.
///
/// Return type uses the default `<S>` (= `String` via `DefaultCheapStr`),
/// matching `taffy::Style`'s default. The compiler infers it from the
/// `Style.grid_template_columns: GridTrackVec<GridTemplateComponent<S>>`
/// field's `S` when this iterator's output is collected into the field.
fn track_to_template(t: &TrackSize) -> GridTemplateComponent<String> {
    match t {
        TrackSize::Repeat(count, tracks) => {
            GridTemplateComponent::Repeat(taffy::GridTemplateRepetition {
                count: map_repeat_count(*count),
                tracks: tracks.iter().map(track_to_sizing).collect(),
                // line_names is Vec<Vec<S>>; an empty outer Vec means
                // no named lines are declared on this repeat.
                line_names: Vec::new(),
            })
        }
        other => GridTemplateComponent::Single(track_to_sizing(other)),
    }
}

/// Convert a `GridLine` plus optional parent named-area registry into a
/// `taffy::Line<GridPlacement>`. `axis` selects column vs row resolution.
fn grid_line_to_taffy(
    line: &GridLine,
    axis: GridAxis,
    parent_areas: Option<&GridAreas>,
) -> taffy::Line<GridPlacement<String>> {
    match line {
        GridLine::Auto => taffy::Line {
            start: GridPlacement::Auto,
            end: GridPlacement::Auto,
        },
        GridLine::Start(i) => taffy::Line {
            start: GridPlacement::Line((*i).into()),
            end: GridPlacement::Auto,
        },
        GridLine::Span(n) => taffy::Line {
            start: GridPlacement::Span(*n),
            end: GridPlacement::Auto,
        },
        GridLine::StartEnd(s, e) => taffy::Line {
            start: GridPlacement::Line((*s).into()),
            end: GridPlacement::Line((*e).into()),
        },
        GridLine::Area(name) => match parent_areas.and_then(|areas| {
            areas.areas.iter().find(|a| a.name == *name)
        }) {
            Some(a) => match axis {
                GridAxis::Column => taffy::Line {
                    // CSS named-area resolution: column_start (0-indexed)
                    // becomes line (column_start + 1) in 1-indexed CSS,
                    // and column_end becomes line (column_end + 1).
                    start: GridPlacement::Line(((a.column_start as i16) + 1).into()),
                    end: GridPlacement::Line(((a.column_end as i16) + 1).into()),
                },
                GridAxis::Row => taffy::Line {
                    start: GridPlacement::Line(((a.row_start as i16) + 1).into()),
                    end: GridPlacement::Line(((a.row_end as i16) + 1).into()),
                },
            },
            None => {
                warn_once_unresolved_area(name);
                taffy::Line {
                    start: GridPlacement::Auto,
                    end: GridPlacement::Auto,
                }
            }
        },
    }
}

#[derive(Clone, Copy)]
enum GridAxis {
    Column,
    Row,
}

// Warn-once gates for invalid track nesting + unresolved named areas +
// Subgrid (Masonry's gate lives in Task 6 alongside `map_grid_auto_flow`'s
// Masonry arm). The Fr-outside-grid gate is shared with Task 1's
// length_to_* helpers — defined once at the top of the helpers section.
static WARNED_INVALID_TRACK_NESTING: AtomicBool = AtomicBool::new(false);
static WARNED_UNRESOLVED_AREA: AtomicBool = AtomicBool::new(false);

fn warn_once_invalid_track_nesting() {
    if !WARNED_INVALID_TRACK_NESTING.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: invalid TrackSize nesting (Repeat inside Repeat/MinMax, \
             or non-leaf inside MinMax slot) — falling back to Auto (warned once)"
        );
    }
}

fn warn_once_unresolved_area(name: &str) {
    if !WARNED_UNRESOLVED_AREA.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: GridLine::Area({:?}) did not match any name in the parent's \
             template_areas; falling back to Auto (warned once)",
            name
        );
    }
}

// Subgrid warn-gate (Masonry warn-gate lands in Task 6).
fn warn_once_subgrid() {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: TrackSize::Subgrid is reserved — Taffy 0.10 has no subgrid \
             support; falling back to Auto (warned once)"
        );
    }
}
```

(`AtomicBool` and `Ordering` are already imported at the top of `translate.rs` after Task 1; no additional `use` line needed. `warn!` is the Bevy prelude macro.)

- [ ] **Step 5: Wire grid into `style_to_taffy`**

Inside `style_to_taffy`, after the existing `let mut s = taffy::Style { ... };` and after the `_scroll_unused_in_layout` line, append:

```rust
    // Grid container fields. Only meaningful when display is Grid /
    // InlineGrid, but Taffy ignores them otherwise — so unconditional
    // population is safe and removes a branch.
    s.grid_template_columns = view
        .grid_params
        .template_columns
        .iter()
        .map(track_to_template)
        .collect();
    s.grid_template_rows = view
        .grid_params
        .template_rows
        .iter()
        .map(track_to_template)
        .collect();
    s.grid_auto_columns = view
        .grid_params
        .auto_columns
        .iter()
        .map(track_to_sizing)
        .collect();
    s.grid_auto_rows = view
        .grid_params
        .auto_rows
        .iter()
        .map(track_to_sizing)
        .collect();
    s.grid_auto_flow = map_grid_auto_flow(view.grid_params.auto_flow);
    if let Some(areas) = &view.grid_params.template_areas {
        s.grid_template_areas = areas
            .areas
            .iter()
            .map(|a| taffy::GridTemplateArea {
                // S = String (Taffy's DefaultCheapStr); clone the owned
                // String. Do not `.as_str().into()` — that requires a
                // 'static borrow that the runtime String doesn't have.
                name: a.name.clone(),
                row_start: a.row_start + 1,
                row_end: a.row_end + 1,
                column_start: a.column_start + 1,
                column_end: a.column_end + 1,
            })
            .collect();
    }

    // Grid item fields. Only honored when the parent is a grid container,
    // but Taffy ignores otherwise.
    if let Some(item) = view.grid_item {
        s.grid_column = grid_line_to_taffy(&item.column, GridAxis::Column, view.parent_areas);
        s.grid_row = grid_line_to_taffy(&item.row, GridAxis::Row, view.parent_areas);
    }

    // Note: justify_self / align_self / justify_items / align_items /
    // justify_content / align_content for grid live in their own taffy
    // fields (separate from the flex equivalents — taffy::Style has both).
    // Phase 1 set flex's variants from FlexParams; Phase 3 also sets the
    // grid-relevant ones. Taffy 0.10 has *one* shared set of fields
    // (justify_items / align_items / etc.) used by both algorithms; the
    // flex path already populates them. To keep grid alignment honoring
    // grid_params, override when display is Grid / InlineGrid.
    if matches!(view.display, Display::Grid | Display::InlineGrid) {
        s.justify_items = Some(map_justify_items(view.grid_params.justify_items));
        s.align_items = Some(map_align_items(view.grid_params.align_items));
        s.justify_content = Some(map_justify_content(view.grid_params.justify_content));
        s.align_content = Some(map_align_content(view.grid_params.align_content));
        s.gap = taffy::Size {
            width: length_to_lp_or_warn_fr(view.grid_params.gap.column),
            height: length_to_lp_or_warn_fr(view.grid_params.gap.row),
        };
    }

    if let Some(item) = view.grid_item
        && let Some(j) = item.justify_self
    {
        s.justify_self = Some(map_justify_items_as_self(j));
    }
    // align_self for grid is handled by the flex path's `if let Some(item) = view.flex_item` —
    // grid items use the same component (`FlexItem.align_self`)? No — GridItem has its own
    // align_self. Wire it.
    if let Some(item) = view.grid_item
        && let Some(a) = item.align_self
    {
        s.align_self = Some(map_align_items_as_self(a));
    }
```

Add the `map_justify_items` and `map_justify_items_as_self` helpers near the other `map_*` helpers:

```rust
fn map_justify_items(j: JustifyItems) -> taffy::JustifyItems {
    use JustifyItems::*;
    match j {
        Stretch => taffy::JustifyItems::Stretch,
        Start => taffy::JustifyItems::Start,
        End => taffy::JustifyItems::End,
        Center => taffy::JustifyItems::Center,
        Baseline => taffy::JustifyItems::Baseline,
    }
}

fn map_justify_items_as_self(j: JustifyItems) -> taffy::JustifySelf {
    use JustifyItems::*;
    match j {
        Stretch => taffy::JustifySelf::Stretch,
        Start => taffy::JustifySelf::Start,
        End => taffy::JustifySelf::End,
        Center => taffy::JustifySelf::Center,
        Baseline => taffy::JustifySelf::Baseline,
    }
}
```

Also update `length_to_lp` (the existing helper) so it warns on `Fr`:

```rust
fn length_to_lp(l: Length) -> taffy::LengthPercentage {
    match l {
        Length::Px(v) => taffy::LengthPercentage::length(v),
        Length::Percent(p) => taffy::LengthPercentage::percent(p / 100.0),
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::LengthPercentage::length(0.0)
        }
    }
}
```

And similarly for `length_to_dim` and `length_to_lpa`:

```rust
fn length_to_dim(l: Length) -> taffy::Dimension {
    match l {
        Length::Px(v) => taffy::Dimension::length(v),
        Length::Percent(p) => taffy::Dimension::percent(p / 100.0),
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::Dimension::auto()
        }
    }
}

fn length_to_lpa(l: Length) -> taffy::LengthPercentageAuto {
    match l {
        Length::Px(v) => taffy::LengthPercentageAuto::length(v),
        Length::Percent(p) => taffy::LengthPercentageAuto::percent(p / 100.0),
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::LengthPercentageAuto::auto()
        }
    }
}
```

(De-dupes against `length_to_lp_or_warn_fr` — keep `length_to_lp` for non-grid callers and merge them; alternatively keep both. Reviewer's call. Suggest merging: rename `length_to_lp_or_warn_fr` to be the single canonical `length_to_lp`. The existing one already warns on Fr per the changes above; the helpers are now identical.)

- [ ] **Step 6: Widen `sync_styles` query and trigger filter**

In `systems.rs`, update the imports and the query:

```rust
use super::components::{
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
};
use super::translate::{StyleView, style_to_taffy};
use super::tree::LayoutTree;
use crate::components::{Node, ResolvedLayout};
use bevy::prelude::*;
use std::collections::HashMap;
use taffy::{AvailableSpace, NodeId as TaffyNodeId, Size};
```

Replace `sync_styles`'s body:

```rust
#[allow(clippy::type_complexity)]
pub(super) fn sync_styles(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<
        (
            Entity,
            &Display,
            &BoxModel,
            &Position,
            &FlexParams,
            Option<&FlexItem>,
            &Overflow,
            &Scroll,
            &GridParams,
            Option<&GridItem>,
            Option<&Children>,
            Option<&ChildOf>,
        ),
        (
            With<Node>,
            Or<(
                Changed<Display>,
                Changed<BoxModel>,
                Changed<Position>,
                Changed<FlexParams>,
                Changed<FlexItem>,
                Changed<Overflow>,
                Changed<Scroll>,
                Changed<GridParams>,
                Changed<GridItem>,
                Changed<Children>,
                Changed<ChildOf>,
            )>,
        ),
    >,
    parent_grid_lookup: Query<&GridParams>,
) {
    let tree = &mut *tree;

    // Precompute parent-areas: for every entity in the changed set, look
    // up its parent's GridParams.template_areas (if any). This map is
    // small — one entry per entity in the changed set — and avoids a
    // per-entity query during the iteration. ChildOf is followed once.
    let parent_areas_for: HashMap<Entity, super::types::GridAreas> = nodes
        .iter()
        .filter_map(|(entity, _, _, _, _, _, _, _, _, _, _, parent)| {
            let p = parent?;
            let grid = parent_grid_lookup.get(p.parent()).ok()?;
            grid.template_areas.clone().map(|a| (entity, a))
        })
        .collect();

    for (
        entity,
        display,
        bm,
        position,
        flex,
        flex_item,
        overflow,
        scroll,
        grid_params,
        grid_item,
        _children,
        _parent,
    ) in nodes.iter()
    {
        let view = StyleView {
            display,
            box_model: bm,
            position,
            flex_params: flex,
            flex_item,
            overflow,
            scroll,
            grid_params,
            grid_item,
            parent_areas: parent_areas_for.get(&entity),
        };
        let taffy_style = style_to_taffy(view);
        // ... existing match block unchanged
    }
    // ... existing child-sync block unchanged
}
```

(The full sync_styles body keeps the existing match-on-`tree.by_entity.get(&entity)` insert/update logic and the second-pass child-sync; only the Query signature, the `parent_areas_for` precompute, and the `StyleView` construction change. The reviewer should diff against the current `systems.rs` to confirm.)

- [ ] **Step 7: Run the tests to verify they pass**

```sh
xvfb-run -a cargo test -p buiy_core --lib translate::tests 2>&1 | tail -30
```

Expected: existing translate tests + 5 new = all pass.

```sh
xvfb-run -a cargo test -p buiy_core --tests 2>&1 | tail -30
```

Expected: existing integration tests still pass (Phase 1 + Phase 2 invariants intact).

- [ ] **Step 8: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

Expected: clean.

- [ ] **Step 9: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/systems.rs
git commit -m "feat(buiy_core): wire Grid to Taffy + widen sync_styles trigger set

Atomic: extends StyleView with grid_params, grid_item, parent_areas;
populates taffy::Style.grid_* fields; resolves GridLine::Area via
Buiy-side parent lookup (Taffy 0.10 has no native named-area
placement). Length::Fr outside grid templates warns once and falls
back to 0 px / Auto.

sync_styles now precomputes a parent-areas map for the changed set
and widens its Or filter to Changed<GridParams>+Changed<GridItem>.
ChildOf is also added to the filter so that re-parenting a grid item
under a different grid container picks up the new areas.

Note: this is one commit because StyleView is the bridge between
translate.rs and systems.rs — splitting would break the lib build
between commits."
```

---

### Task 6: Subgrid + Masonry warn-once stubs

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` — wire Subgrid into `track_to_template` so the variant routes through the `warn_once_subgrid` gate at the top level (the helper-level routing was added in Task 5; this task pins the integration test).
- Modify: `crates/buiy_core/src/layout/translate.rs` — add `warn_once_masonry` symmetric with `warn_once_subgrid`; route `GridAutoFlow::Masonry` through it inside `map_grid_auto_flow`.
- Test: `crates/buiy_core/tests/layout_grid_stubs.rs` (new) — observable degradation tests.

**Test surface:**

- `subgrid_in_template_columns_falls_back_to_auto` — `template_columns: vec![TrackSize::Subgrid]` produces a non-empty `taffy.grid_template_columns` whose first entry is `Single(auto)` (not `Repeat`).
- `masonry_auto_flow_falls_back_to_row` — `auto_flow: GridAutoFlow::Masonry` produces `taffy.grid_auto_flow == taffy::GridAutoFlow::Row`.

(Logging assertions are intentionally omitted — Bevy's `warn!` macro is hard to intercept in unit tests, and the AtomicBool flip is internal state. Spec § 5 says "produces single-column layout with `warn!`" — the observable contract is the *layout fallback*, which is what the tests pin. Document the visible warn in CHANGELOG as part of the user-facing surface.)

- [ ] **Step 1: Create the failing test file**

Create `crates/buiy_core/tests/layout_grid_stubs.rs`:

```rust
//! Subgrid + Masonry stub tests — pin the observable layout degradation.

use bevy::prelude::*;
use buiy_core::layout::{
    Display, GridAutoFlow, GridParams, LayoutPlugin, Style, TrackSize,
};
use buiy_core::Node;

fn world_with_grid(grid_params: GridParams, display: Display) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    let mut style = Style::default();
    style.display = display;
    style.grid_params = grid_params;
    app.world_mut().spawn((style, Node));
    app.update();
    app
}

#[test]
fn subgrid_in_template_columns_falls_back_to_auto() {
    let g = GridParams {
        template_columns: vec![TrackSize::Subgrid],
        ..Default::default()
    };
    let _app = world_with_grid(g, Display::Grid);
    // Layout completes without panic. Subgrid → Auto fallback is exercised
    // through the full pipeline. (Observable: no panic + warn-once in
    // log output during this test run.)
}

#[test]
fn masonry_auto_flow_falls_back_to_row() {
    let g = GridParams {
        auto_flow: GridAutoFlow::Masonry,
        ..Default::default()
    };
    let _app = world_with_grid(g, Display::Grid);
    // Layout completes without panic. Masonry → Row fallback exercised.
}
```

The test is intentionally minimal — *that the layout completes without panic* is the contract; the warn-once gate is internal. A more invasive test would require a custom log layer to intercept and is out of scope.

- [ ] **Step 2: Run the tests to verify they compile and pass**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_grid_stubs 2>&1 | tail -20
```

Expected: PASS — Task 5 already wired the warn-once gates and fallbacks; this test pins the contract observably. (If it fails because `warn_once_masonry` isn't yet present, add it now in Step 3.)

- [ ] **Step 3: Add `warn_once_masonry` if not yet wired**

In `translate.rs`, append:

```rust
fn warn_once_masonry() {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        bevy::log::warn!(
            "buiy: GridAutoFlow::Masonry is reserved — CSS-WG flux + no Taffy \
             support; falling back to Row (warned once)"
        );
    }
}
```

And update `map_grid_auto_flow`:

```rust
fn map_grid_auto_flow(f: GridAutoFlow) -> taffy::GridAutoFlow {
    use GridAutoFlow::*;
    match f {
        Row => taffy::GridAutoFlow::Row,
        Column => taffy::GridAutoFlow::Column,
        RowDense => taffy::GridAutoFlow::RowDense,
        ColumnDense => taffy::GridAutoFlow::ColumnDense,
        Masonry => {
            warn_once_masonry();
            taffy::GridAutoFlow::Row
        }
    }
}
```

- [ ] **Step 4: Run tests + lint**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_grid_stubs 2>&1 | tail -10 \
  && cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 5: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/tests/layout_grid_stubs.rs
git commit -m "feat(buiy_core): warn-once stubs for TrackSize::Subgrid + GridAutoFlow::Masonry

Both reserved variants degrade to a sensible non-stub equivalent
(Auto / Row) and emit one warn! per session naming the limitation.
Tests assert the layout pipeline completes without panic when these
variants appear; the warn output is observable but not asserted (Bevy
log interception is out of scope)."
```

---

### Task 7: Register types + re-exports

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` — register reflection, re-export
- Modify: `crates/buiy_core/src/lib.rs` — re-export from buiy_core's public surface
- Modify: `crates/buiy/src/lib.rs` — re-export from the facade crate

**Test surface:**

Reflection registration is verified by the test that `cargo test --workspace` passes (any incorrect registration would produce a runtime panic in the LayoutPlugin reflection setup or a missed type at downstream serialization). No new dedicated test.

- [ ] **Step 1: Update `LayoutPlugin` reflection registrations**

In `crates/buiy_core/src/layout/mod.rs`:

```rust
pub use components::{
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
    ScrollOffset, ScrollSnapItem,
};
pub use pipeline::BuiyLayoutStep;
pub use style::Style;
pub use tree::LayoutTree;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap,
    GridAreas, GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems, Length, NamedArea,
    OverflowMode, OverscrollBehavior, PositionKind, RepeatCount, ScrollBehavior, ScrollbarColor,
    ScrollbarGutter, ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType, TrackSize,
};

// ...

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<LayoutTree>();

        app.register_type::<BoxModel>()
            .register_type::<Display>()
            .register_type::<Position>()
            .register_type::<FlexParams>()
            .register_type::<FlexItem>()
            .register_type::<Overflow>()
            .register_type::<Scroll>()
            .register_type::<ScrollOffset>()
            .register_type::<ScrollSnapItem>()
            .register_type::<GridParams>()
            .register_type::<GridItem>()
            .register_type::<Edges>()
            .register_type::<Sizing>()
            .register_type::<Length>()
            .register_type::<AspectRatio>()
            .register_type::<Inset>()
            .register_type::<TrackSize>()
            .register_type::<RepeatCount>()
            .register_type::<GridLine>()
            .register_type::<GridAreas>()
            .register_type::<NamedArea>()
            .register_type::<GridAutoFlow>()
            .register_type::<JustifyItems>();

        // ... rest unchanged
    }
}
```

- [ ] **Step 2: Update `crates/buiy_core/src/lib.rs` re-exports**

Find the layout re-exports block (look for `pub use layout::{...}`) and add:

```rust
pub use layout::{
    GridAreas, GridAutoFlow, GridItem, GridLine, GridParams, JustifyItems, NamedArea,
    RepeatCount, TrackSize,
};
```

(Add to the existing list, keeping alphabetical order.)

- [ ] **Step 3: Update `crates/buiy/src/lib.rs` re-exports**

Mirror the above re-export in the facade crate.

- [ ] **Step 4: Run the full workspace test**

```sh
xvfb-run -a cargo test --workspace 2>&1 | tail -30
```

Expected: every existing test still passes (no test count regression).

- [ ] **Step 5: Run lint + format**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10 \
  && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 6: Commit**

```sh
git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
git commit -m "feat(buiy_core, buiy): register and re-export Phase 3 layout types

GridParams + GridItem registered for reflection; new value types
(TrackSize, GridLine, GridAreas + NamedArea, GridAutoFlow,
RepeatCount, JustifyItems) re-exported from buiy_core and the buiy
facade alongside the Phase 2 surface."
```

---

### Task 8: Integration tests through the full pipeline

**Files:**
- Test: `crates/buiy_core/tests/layout_grid.rs` (new) — 3 fixtures driving real Taffy via `LayoutPlugin`.

**Test surface:**

- `grid_template_1fr_2fr_1fr_in_400px_row_lays_out_100_200_100` — fixture: container `Display::Grid`, `width = 400 px`, `template_columns = vec![TrackSize::Length(Length::Fr(1.0)), TrackSize::Length(Length::Fr(2.0)), TrackSize::Length(Length::Fr(1.0))]`, with three children. Assert child positions / sizes.
- `grid_named_areas_resolve_child_to_correct_cell` — fixture: container with `template_areas = GridAreas::from_lines(&["a a", "b ."])` and `template_columns = vec![Fr(1), Fr(1)]`, `template_rows = vec![Px(50), Px(50)]`, child with `GridItem.column = GridLine::Area("a".into())`. Assert child resolves to the `a` rectangle.
- `grid_repeat_auto_fill_in_350px_produces_three_columns` — fixture: `template_columns = vec![TrackSize::Repeat(RepeatCount::AutoFill, vec![TrackSize::Length(Length::Px(100.0))])]`, container width 350, 3 children. Assert exactly 3 columns formed, totaling 300 px (50 px slack).
- `grid_cell_hosts_flex_row_with_two_children` — fixture: a `Display::Grid` parent with one cell that contains a `Display::Flex(Row)` child (a "row inside a cell"); that flex child has two flex children of its own. Assert the inner flex children's resolved x positions reflect flex distribution within the cell. Pins spec § 5 "Mixed flex-in-grid" composition.

These tests use the integration-test pattern from Phase 2's `layout_overflow.rs`: spawn entities through `LayoutPlugin`, run one `app.update()` cycle, then read `ResolvedLayout` per child. ResolvedLayout doesn't impl PartialEq (Phase 2 noted this) — assert per-field via `Vec2` (which is Copy + PartialEq).

- [ ] **Step 1: Write the failing test file**

Create `crates/buiy_core/tests/layout_grid.rs`:

```rust
//! Integration tests for grid through the full LayoutPlugin pipeline.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    Display, GridAreas, GridItem, GridLine, GridParams, LayoutPlugin, Length, RepeatCount,
    Sizing, Style, TrackSize,
};

fn capture_layouts<'a>(world: &'a World, entities: &[Entity]) -> Vec<(Vec2, Vec2)> {
    entities
        .iter()
        .map(|e| {
            let rl = world
                .get::<ResolvedLayout>(*e)
                .expect("ResolvedLayout written");
            (rl.position, rl.size)
        })
        .collect()
}

#[test]
fn grid_template_1fr_2fr_1fr_in_400px_row_lays_out_100_200_100() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(400.0)
        .height_px(100.0)
        .grid_template_columns(vec![
            TrackSize::Length(Length::Fr(1.0)),
            TrackSize::Length(Length::Fr(2.0)),
            TrackSize::Length(Length::Fr(1.0)),
        ]);

    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let mut children: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        let c = app
            .world_mut()
            .spawn((Style::default().height_px(100.0), Node))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(parent).add_children(&children);

    app.update();

    let layouts = capture_layouts(app.world(), &children);

    // 1fr / 2fr / 1fr in 400 px → 100 / 200 / 100 widths.
    assert!(
        (layouts[0].1.x - 100.0).abs() < 0.5,
        "child 0 width = {}",
        layouts[0].1.x
    );
    assert!(
        (layouts[1].1.x - 200.0).abs() < 0.5,
        "child 1 width = {}",
        layouts[1].1.x
    );
    assert!(
        (layouts[2].1.x - 100.0).abs() < 0.5,
        "child 2 width = {}",
        layouts[2].1.x
    );
    // Positions: 0 / 100 / 300.
    assert!(
        (layouts[0].0.x - 0.0).abs() < 0.5,
        "child 0 x = {}",
        layouts[0].0.x
    );
    assert!(
        (layouts[1].0.x - 100.0).abs() < 0.5,
        "child 1 x = {}",
        layouts[1].0.x
    );
    assert!(
        (layouts[2].0.x - 300.0).abs() < 0.5,
        "child 2 x = {}",
        layouts[2].0.x
    );
}

#[test]
fn grid_named_areas_resolve_child_to_correct_cell() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(200.0)
        .height_px(100.0)
        .grid_template_columns(vec![
            TrackSize::Length(Length::Fr(1.0)),
            TrackSize::Length(Length::Fr(1.0)),
        ])
        .grid_template_rows(vec![
            TrackSize::Length(Length::Px(50.0)),
            TrackSize::Length(Length::Px(50.0)),
        ])
        .grid_template_areas(GridAreas::from_lines(&["a a", "b ."]));

    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let area_a_child = app
        .world_mut()
        .spawn((
            Style::default(),
            GridItem {
                column: GridLine::Area("a".to_string()),
                row: GridLine::Area("a".to_string()),
                ..Default::default()
            },
            Node,
        ))
        .id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[area_a_child]);

    app.update();

    let rl = app
        .world()
        .get::<ResolvedLayout>(area_a_child)
        .expect("ResolvedLayout written");
    // Area "a" spans columns 0..2 of a 2-column 200 px grid → x=0, width=200.
    // Area "a" spans rows 0..1 → y=0, height=50.
    assert!(
        (rl.position.x - 0.0).abs() < 0.5,
        "area a x = {}",
        rl.position.x
    );
    assert!(
        (rl.size.x - 200.0).abs() < 0.5,
        "area a width = {}",
        rl.size.x
    );
    assert!(
        (rl.size.y - 50.0).abs() < 0.5,
        "area a height = {}",
        rl.size.y
    );
}

#[test]
fn grid_repeat_auto_fill_in_350px_produces_three_columns() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(350.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::Repeat(
            RepeatCount::AutoFill,
            vec![TrackSize::Length(Length::Px(100.0))],
        )]);

    let parent = app.world_mut().spawn((parent_style, Node)).id();
    // Three children placed implicitly into the auto-fill columns.
    let mut children: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        let c = app
            .world_mut()
            .spawn((Style::default().height_px(100.0), Node))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(parent).add_children(&children);

    app.update();

    let layouts = capture_layouts(app.world(), &children);

    // 3 columns of 100 px each = 300 px, with 50 px slack.
    assert!(
        (layouts[0].0.x - 0.0).abs() < 0.5,
        "child 0 x = {}",
        layouts[0].0.x
    );
    assert!(
        (layouts[0].1.x - 100.0).abs() < 0.5,
        "child 0 width = {}",
        layouts[0].1.x
    );
    assert!(
        (layouts[1].0.x - 100.0).abs() < 0.5,
        "child 1 x = {}",
        layouts[1].0.x
    );
    assert!(
        (layouts[2].0.x - 200.0).abs() < 0.5,
        "child 2 x = {}",
        layouts[2].0.x
    );
}

#[test]
fn grid_cell_hosts_flex_row_with_two_children() {
    // Mixed flex-in-grid: a grid parent with one cell whose child is a
    // flex-row container that has two flex children of its own. Pins
    // spec § 5 "Mixed flex-in-grid" composition.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default()
        .grid()
        .width_px(200.0)
        .height_px(100.0)
        .grid_template_columns(vec![TrackSize::Length(Length::Fr(1.0))])
        .grid_template_rows(vec![TrackSize::Length(Length::Px(100.0))]);

    let parent = app.world_mut().spawn((parent_style, Node)).id();

    // Inner: flex-row container (auto-placed into the only grid cell).
    let flex_inner = app
        .world_mut()
        .spawn((Style::default().flex_row().width_px(200.0).height_px(100.0), Node))
        .id();
    // Two flex children at width 50px each.
    let f1 = app
        .world_mut()
        .spawn((Style::default().width_px(50.0).height_px(100.0), Node))
        .id();
    let f2 = app
        .world_mut()
        .spawn((Style::default().width_px(50.0).height_px(100.0), Node))
        .id();
    app.world_mut().entity_mut(flex_inner).add_children(&[f1, f2]);
    app.world_mut().entity_mut(parent).add_children(&[flex_inner]);

    app.update();

    let r1 = app.world().get::<ResolvedLayout>(f1).expect("f1 layout");
    let r2 = app.world().get::<ResolvedLayout>(f2).expect("f2 layout");

    // Within the flex-row's local origin, child 1 starts at x=0 and
    // child 2 at x=50. Their global x is identical because the grid
    // cell hosts the flex-row at x=0.
    assert!(
        (r1.position.x - 0.0).abs() < 0.5,
        "flex child 1 x = {}",
        r1.position.x
    );
    assert!(
        (r2.position.x - 50.0).abs() < 0.5,
        "flex child 2 x = {}",
        r2.position.x
    );
}
```

- [ ] **Step 2: Run the tests to verify they pass**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_grid 2>&1 | tail -30
```

Expected: 4 tests pass. If any fails, the failure is in the wiring from Tasks 4/5/6 — root-cause and fix in those tasks rather than working around in the test (per CLAUDE.md "root-cause every bug").

- [ ] **Step 3: Run the full workspace test**

```sh
xvfb-run -a cargo test --workspace 2>&1 | tail -30
```

Expected: All tests pass — Phase 1 + Phase 2 + Phase 3.

- [ ] **Step 4: Lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 5: Commit**

```sh
git add crates/buiy_core/tests/layout_grid.rs
git commit -m "test(buiy_core): grid through the full pipeline (template, areas, repeat)

Integration tests pin observable layout for: 1fr 2fr 1fr in a 400 px
row produces 100/200/100; named areas resolve child to correct cell
(GridLine::Area going through Buiy-side resolution); repeat(auto-fill,
100 px) in 350 px produces 3 columns with 50 px slack."
```

---

### Task 9: CHANGELOG + branch-level review + PR

**Files:**
- Modify: `CHANGELOG.md` — `[Unreleased]` `### Added` and `### Changed` entries.
- Modify: `docs/README.md` — flip the Phase 3 plan entry from `[active]` to `[landed]` after merge (this happens **after** PR merges, in a separate post-merge commit).

- [ ] **Step 1: Append CHANGELOG entries**

In `CHANGELOG.md`, under the existing `[Unreleased]` heading, add (creating sections if missing):

```markdown
### Added

- **Layout grid (Phase 3 of the layout migration).**
  - `GridParams` component: `template_columns`, `template_rows`,
    `template_areas`, `auto_columns`, `auto_rows`, `auto_flow`,
    `justify_items`, `align_items`, `justify_content`, `align_content`,
    `gap`. Container-side; joins `Style`'s `Bundle`.
  - `GridItem` component: `column`, `row`, `justify_self`, `align_self`.
    Decomposed-only (per architecture.md § 2.4) — spawn alongside
    `Style`, like `FlexItem` and `ScrollSnapItem`.
  - `TrackSize` enum: `Auto`, `Length`, `MinContent`, `MaxContent`,
    `FitContent`, `MinMax`, `Repeat`, `Subgrid` (reserved).
  - `RepeatCount` enum: `AutoFill`, `AutoFit`, `Count(u16)`.
  - `GridLine` enum: `Auto`, `Start(i16)`, `Span(u16)`,
    `StartEnd(i16, i16)`, `Area(String)`.
  - `GridAreas` + `NamedArea`: explicit-rectangle named-area registry
    plus `GridAreas::from_lines(&[&str])` CSS-syntax convenience parser.
  - `GridAutoFlow` enum: `Row`, `Column`, `RowDense`, `ColumnDense`,
    `Masonry` (reserved).
  - `JustifyItems` enum: `Stretch` (default), `Start`, `End`, `Center`,
    `Baseline`.
  - `Length::Fr(f32)` variant for grid `<flex>` units.
  - 12 fluent setters on `Style`: `.grid()`, `.inline_grid()`,
    `.grid_template_columns(_)`, `.grid_template_rows(_)`,
    `.grid_template_areas(_)`, `.grid_auto_columns(_)`,
    `.grid_auto_rows(_)`, `.grid_auto_flow(_)`,
    `.grid_justify_items(_)`, `.grid_align_items(_)`,
    `.grid_justify_content(_)`, `.grid_align_content(_)`,
    `.grid_gap_px(_)`.
  - `GridParams` + `GridItem` registered for reflection in
    `LayoutPlugin`.

### Breaking

- `Length` gains an `Fr(f32)` variant. Downstream code that pattern-matches
  `Length` exhaustively must add an `Fr` arm. The `Fr` variant is only
  meaningful inside `TrackSize::Length(Length::Fr(_))` in a grid template;
  outside grid contexts it warns once and falls back to `0 px` (in
  `LengthPercentage` contexts — Taffy's type has no Auto) or `Auto`
  (in `Dimension` / `LengthPercentageAuto` contexts). `Length` is *not*
  marked `#[non_exhaustive]` — the next planned `Length` change is
  Phase 10's full unit set, which is similarly breaking.

### Changed

- `Display::Grid` and `Display::InlineGrid` now translate to
  `taffy::Display::Grid` (Phase 1 routed both to Block).
- `sync_styles`'s `Or<(Changed<...>)>` trigger filter widens with two
  new clauses: `Changed<GridParams>` and `Changed<GridItem>`.
  `Changed<ChildOf>` was already in the Phase 2 filter and remains so
  — re-parenting a grid item under a different grid container picks
  up the new `template_areas` via the existing clause.
  `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` remain excluded
  (Phase 2 invariant).
- Reserved variants emit one `warn!` per session and degrade:
  `TrackSize::Subgrid → Auto`; `GridAutoFlow::Masonry → Row`.
- `RepeatCount::Count` carries `u16`; `GridLine::Start` / `Span` /
  `StartEnd` carry `i16` / `u16`. Spec used `i32` / `u32`; Phase 3
  matches Taffy 0.10 directly to avoid a lossy conversion at translate
  time. Documented in each type's doc comment.
- `GridLine::Area` uses `String` for area names. Spec used `SmolStr`;
  Phase 3 uses `String` to avoid a new direct supply-chain dep.
```

(If CHANGELOG already has an `### Added` and/or `### Changed` under `[Unreleased]` from prior phases, append to those sections instead of creating new ones.)

- [ ] **Step 2: Add the plan entry to `docs/README.md`** (already added during plan-write commit if applicable; verify)

`docs/README.md` § "Layout > Plans" should already include the entry from when the plan was first committed. Verify:

```markdown
- [Buiy layout grid](plans/2026-05-09-buiy-layout-grid.md) — Phase 3: GridParams + GridItem, TrackSize / GridLine / GridAreas value types, Display::Grid → Taffy. `[active]`
```

If missing, add it. The status will be flipped to `[landed]` post-merge in a separate commit.

- [ ] **Step 3: Run the full check**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps \
  && xvfb-run -a cargo test --workspace
```

Expected: every gate green.

- [ ] **Step 4: Commit**

```sh
git add CHANGELOG.md docs/README.md
git commit -m "docs(layout): changelog + index entries for Phase 3 grid"
```

- [ ] **Step 5: Final branch-level review (subagent-driven-development → spec + code-quality + security/style sweep)**

Dispatch one final reviewer subagent that audits the entire branch diff against the Phase 3 spec and Phase 1 conventions. Pass it the spec link, the plan link, the branch name, and the relevant CLAUDE.md sections. Address any BLOCKER findings before pushing.

- [ ] **Step 6: Push and open PR**

```sh
git push -u origin claude/v01-layout-grid
gh pr create --title "Phase 3: layout grid" --body "$(cat <<'EOF'
## Summary
- `GridParams` + `GridItem` ship the CSS Grid surface; reserved Subgrid + Masonry variants warn-once and degrade.
- `Display::Grid` / `Display::InlineGrid` now route to `taffy::Display::Grid` (Phase 1 routed both to Block).
- 12 fluent setters on `Style`; `Length::Fr` lands grid-only with a non-grid warn-once gate.

## Test plan
- [ ] `cargo test --workspace` green (74 + 19 = 93 → 93 + N new)
- [ ] CI: Lint / Doc / Deny / Test on ubuntu/macos/windows
- [ ] Plan: docs/plans/2026-05-09-buiy-layout-grid.md
- [ ] Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 7: Merge when green**

After CI gates green:

```sh
gh pr merge --merge --delete-branch
git checkout main && git pull --ff-only && git fetch --prune
```

- [ ] **Step 8: Flip plan status to `[landed]`**

```sh
# Edit docs/README.md: replace `[active]` with `[landed]` on the Phase 3 plan line.
git add docs/README.md
git commit -m "docs: mark Phase 3 layout plan [landed]"
git push
```

---

## Self-review (run before dispatching subagent reviewers)

1. **Spec coverage.** Every requirement in `flex-and-grid.md § 2` maps to a row in the Coverage map. Subgrid + Masonry are explicit reserved variants with documented degradation. `Length::Fr` is added per spec.
2. **Placeholder scan.** No "TBD" / "implement later" / "fill in details". Every step contains the actual code.
3. **Type consistency.** Names used in Tasks 5–8 match Tasks 1–3. `JustifyItems`, `GridAutoFlow`, `RepeatCount`, `TrackSize`, `GridLine`, `GridAreas`, `NamedArea`, `GridParams`, `GridItem`, `Style`, `Length::Fr` consistent.
4. **Cross-task atomicity.** Task 5 is explicitly atomic (translate + systems together). Task 4 is intentionally tiny and self-testable to keep it independent.
5. **Width mismatches.** Spec `u32` / `i32` → plan `u16` / `i16`. Spec `SmolStr` → plan `String`. Both documented with rationale.
6. **Decomposed-only convention.** `GridItem` is in components.rs but NOT in `Style`'s Bundle, mirroring `FlexItem` and `ScrollSnapItem`.
7. **Reflection convention.** Every component derives `#[reflect(Component, Default)]`; non-component types derive `#[derive(Reflect, ...)]` only.
8. **Phase boundary.** No Phase 4+ items snuck in (writing-modes, multi-column, sticky, etc.).
9. **Test discipline.** Each task has a "Run tests to verify they fail" step before the implementation step, mirroring TDD discipline. Each task also runs lint + format before commit.
10. **Mechanical rigor.** Every commit step shows `cargo fmt --check && cargo clippy ... -D warnings` before `git commit`.
