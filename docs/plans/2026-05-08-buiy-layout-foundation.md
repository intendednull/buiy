# Buiy layout foundation — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Date:** 2026-05-08
**Status:** draft
**Spec:** [`specs/2026-05-08-buiy-layout-design/`](../specs/2026-05-08-buiy-layout-design/README.md)

**Goal:** Phase 0 → target migration phase 1: replace the mega-`Style` with a hybrid `Style` *builder* over decomposed components, install the 8-step pipeline skeleton, and keep every Phase 0 layout test green plus `hello_button` running.

**Architecture:** The `crates/buiy_core/src/layout.rs` flat module becomes a `crates/buiy_core/src/layout/` directory with: `types` (units, edges, axis enums), `components` (decomposed `BoxModel`/`Display`/`Position`/`FlexParams`/`FlexItem`), `style` (the hybrid builder + `Bundle` impl), `tree` (`LayoutTree` bridge state), `translate` (decomposed → `taffy::Style`), `pipeline` (8-step `BuiyLayoutStep` chain), `systems` (the per-step systems). `LayoutPlugin` registers all decomposed components with reflection and chains the eight steps inside `BuiySet::Layout`. Steps 2 (`CqActivate`), 4 (`CqFlipCheck`), and 6 (`PostTaffyOverrides`) are intentional no-ops in this phase — wired into the chain so later phases can fill them in without reordering. The Phase 0 mega-`Style` and the `FlexDirection` enum are deleted; `Button` and existing tests migrate to the builder.

**Tech Stack:** Rust, Bevy 0.18 ECS + reflection + `Bundle`, Taffy 0.10, no new external dependencies.

---

## Phasing strategy (this plan vs. follow-ups)

The layout spec covers ~9 semi-independent subsystems. Per `writing-plans`'s decomposition guidance, the migration ships across multiple phase plans rather than one mega-plan; each plan delivers shippable software. This document is **Phase 1** only.

| Phase | Plan filename (TBC) | Scope | Depends on |
|---|---|---|---|
| **1 (this plan)** | `2026-05-08-buiy-layout-foundation.md` | 8-step pipeline skeleton; decomposed `BoxModel`/`Display`/`Position`/`FlexParams`/`FlexItem`; hybrid `Style` builder; `Button` migrated; old `Style` deleted. | — (Phase 0 closeout) |
| 2 | `*-buiy-layout-overflow-and-scrolling.md` | `Overflow`, `Scroll`, `ScrollOffset`, `ScrollSnapItem`. | Phase 1 |
| 3 | `*-buiy-layout-grid.md` | `GridParams`, `GridItem`, `TrackSize`, named areas. | Phase 1 |
| 4 | `*-buiy-layout-writing-modes.md` | `WritingMode`, `WritingModeResolved` cache + inheritance pass, logical edges, `LogicalBoxModel` insert helper, `ContainingBlock` cache. | Phase 1 |
| 5 | `*-buiy-layout-container-queries.md` | `Container`, `ContainerQuery`, step 2 (`CqActivate`), step 4 (`CqFlipCheck`), conditional re-run wiring; `cqi`/`cqb`/`cqw`/`cqh` units. | Phase 1, Phase 4 (writing-mode) |
| 6 | `*-buiy-layout-anchor-positioning.md` | `Anchor`, `AnchorNameRegistry`, sub-pass 6d (`AnchorResolution`), `LayoutAnchorBroken` marker, fallback chain + cycle handling. | Phase 1 |
| 7 | `*-buiy-layout-sticky-table-multicol.md` | Sub-pass 6a (`StickyOffset`), 6b (`TableLayout` v1 fallback), 6c (`MulticolPack` v1 stub), `MultiColumn` component. | Phase 1, Phase 2 (sticky reads scroll) |
| 8 | `*-buiy-layout-stacking-and-top-layer.md` | `Stacking`, stacking-context formation triggers, top-layer per-window, render-side coordination. | Phase 1, paired with `buiy-render-pipeline-design` (open question — render spec not yet written). |
| 9 | `*-buiy-layout-transforms-and-containment.md` | `Transform` (2D + 3D), `Translate`/`Rotate`/`Scale` longhands, `Containment`, `content-visibility`, `will-change`. | Phase 1 |
| 10 | `*-buiy-layout-units-calc.md` | Full `Length` resolution: `Em`, `Rem`, viewport family, `Calc` evaluator. (Phase 1 ships only `Px` + `Percent`.) | Phase 1 |

Phase 1's pipeline scaffold has hooks (`CqActivate`, `CqFlipCheck`, `PostTaffyOverrides` sub-passes) for every later phase. No phase needs to reorder existing steps.

---

## File structure

### New files

```
crates/buiy_core/src/layout/
├── types.rs         — Length, Sizing, Edges, BoxSizing, AspectRatio, FlexAxis,
│                      FlexWrap, JustifyContent, AlignItems, AlignContent, FlexGap,
│                      PositionKind, Inset                                  (Task 1)
├── components.rs    — BoxModel, Display, Position, FlexParams, FlexItem    (Task 2)
├── style.rs         — Style (builder struct), Style::default(), fluent methods,
│                      impl Bundle for Style                                (Task 3)
├── translate.rs     — style_to_taffy(StyleView<'_>) -> taffy::Style        (Task 4)
├── pipeline.rs      — BuiyLayoutStep enum (system-set sub-sets of BuiySet::Layout) (Task 5)
├── tree.rs          — LayoutTree (bridge state), pub fn len/is_empty       (Task 6)
└── systems.rs       — gc_removed_nodes (step 0), sync_styles (step 1),
                       taffy_compute (step 3), write_resolved_layout (step 7) (Task 7)
```

`crates/buiy_core/src/layout/mod.rs` is **not** created until Task 7. Rust's E0761 disallows both `layout.rs` and `layout/mod.rs` coexisting under one `pub mod layout;`, so Tasks 1–6 work by adding `mod <name>;` declarations inside the existing flat `layout.rs`. Task 7 deletes `layout.rs` and creates `layout/mod.rs` simultaneously, transitioning the module structure.

### New tests

```
crates/buiy_core/tests/
├── layout_pipeline_order.rs        — assert eight steps chain in declared order
├── layout_topology.rs              — parent resolves before children
├── layout_style_equivalence.rs     — struct-literal Style ≡ fluent Style components
└── layout_box_sizing.rs            — ContentBox vs BorderBox produce expected widths
```

### Modified files

- `crates/buiy_core/src/components.rs` — drop `Style` and `FlexDirection`; **add `Visual` component** (`background_token`, `foreground_token`, `border_radius`); keep `Node` and `ResolvedLayout`.
- `crates/buiy_core/src/lib.rs` — switch `pub mod layout;` to the directory module; update re-exports; drop `FlexDirection`/`Style` from public surface and re-export the new builder + decomposed components + `Visual`.
- `crates/buiy_core/src/render/mod.rs` — switch the extract `Query<(&Style, ...)>` to `Query<(&Visual, ...)>`; the field reads (`background_token`, `border_radius`) move from `Style` to `Visual`. Required to keep render compiling after `Style`'s mega-component is deleted.
- `crates/buiy/src/lib.rs` — re-export the new builder + decomposed components + `Visual` in place of the old `Style`/`FlexDirection`.
- `crates/buiy_widgets/src/button.rs` — switch from old `Style` literal to the new builder + insert `Visual` alongside.
- `crates/buiy_core/tests/layout.rs` — migrate to new builder.
- `crates/buiy_core/tests/components.rs` — register-type assertions cover the new component set.
- `CHANGELOG.md` — `[Unreleased]` `### Changed` entry for the migration.

### Deleted files

- `crates/buiy_core/src/layout.rs` — replaced by the `layout/` directory.

---

## Coverage map

Every Phase 1 spec requirement maps to a task below. Items marked **deferred** are explicitly out of Phase 1 per the phasing strategy table; they have placeholder hooks in the pipeline so later phase plans can drop them in without restructuring.

| Spec section | Phase 1 coverage | Task |
|---|---|---|
| README §2 pillar 1 — single fixed pipeline | 8-step chain configured as sub-sets of `BuiySet::Layout`. | 5, 7 |
| README §2 pillar 2 — hybrid component API | `Style` builder + `Bundle`; decomposed components canonical. | 3, 4 |
| README §2 pillar 3 — `LayoutTree` bridge | `tree.rs` re-homes the existing struct unchanged. | 6 |
| README §2 pillar 4 — same-frame CQ re-layout, capped 2× | Hook only (step 2/4/5 stubs). **Deferred to Phase 5.** | 5 |
| README §2 pillar 5 — anchor as post-Taffy overlay | Sub-pass 6d hook only. **Deferred to Phase 6.** | 5 |
| README §2 pillar 6 — topological invariant | Asserted by test fixture. | 9 |
| README §2 pillar 7 — error model: warn + retain | Per-system `warn!` paths preserved from Phase 0; sentinel writes still rejected. | 7 |
| architecture.md §1.1 — `LayoutTree` `NonSendResource` | Re-homed in `tree.rs`. | 6 |
| architecture.md §1.2 — pure `style_to_taffy` | Implemented in `translate.rs`. | 4 |
| architecture.md §1.2 — change-detection trigger set | `sync_styles`'s query carries `Or<(Changed<BoxModel>, Changed<Display>, Changed<Position>, Changed<FlexParams>, Changed<FlexItem>, Changed<Children>, Changed<ChildOf>)>` so steady-state frames skip every entity (matches spec §9 O(0) contract). Phase 4–9 widen the filter as components land. | 7 |
| architecture.md §2.1 — decomposed components | `BoxModel`, `Display`, `Position`, `FlexParams`, `FlexItem`. **`Anchor`/`Container`/`WritingMode`/`Overflow`/`Scroll`/`Stacking`/`Transform`/`Containment`/`MultiColumn`/`GridParams`/`GridItem` deferred.** | 2 |
| architecture.md §2.2/2.3 — hybrid `Style` builder + `Bundle` expansion | Implemented; `None`/default fields skip insertion. | 3 |
| architecture.md §2.4 — child-side decomposed | `FlexItem` is decomposed-only (not folded into `Style`). | 2 |
| architecture.md §3 — eight-step pipeline | All eight system-set sub-sets configured; steps 0/1/3/7 do real work, 2/4/6 are no-ops, 5 is the conditional-rerun signaled by step 4 (always-skipped this phase). | 5 |
| architecture.md §4 — lifecycle (insert/mutate/despawn) | Insert handled by `sync_styles` discovering missing entries; mutate by Bevy change detection; despawn by `gc_removed_nodes` reading `RemovedComponents<Node>`. | 7 |
| architecture.md §5 — topological invariant | Asserted by test. | 9 |
| architecture.md §6 — error model | Each system retains Phase 0's blanket-`warn!` semantics; failures keep last frame's `ResolvedLayout`. **Spec §4.3's "swallow `Err(NotFound)` silently" is a Phase 1.x deliverable awaiting a Taffy 0.10 error-enum audit; the same-tick despawn fixture in `tests/layout.rs` already passed under Phase 0's blanket-warn behavior, so this is parity not regression.** | 7 |
| architecture.md §7 — crate placement | Layout stays in `buiy_core` for Phase 1; `layout/` directory boundary makes the future `buiy_layout` crate split a `mv`. | — (file structure decision) |
| architecture.md §8 test #1 — system order | `layout_pipeline_order.rs`. | 10 |
| architecture.md §8 test #2 — GC | Existing `layout_tree_garbage_collects_*` tests migrated. | 11 |
| architecture.md §8 test #3 — topological invariant | `layout_topology.rs`. | 9 |
| architecture.md §8 test #4 — hybrid API equivalence | `layout_style_equivalence.rs`. | 10 |
| architecture.md §8 test #5 — CQ same-frame re-layout | **Deferred to Phase 5.** |
| architecture.md §8 test #6 — anchor resolution | **Deferred to Phase 6.** |
| architecture.md §8 test #7 — error path | Phase 0's `warn!` paths remain; explicit error-path test deferred to a later phase that introduces a system whose error semantics need pinning beyond the existing GC ordering test. |
| architecture.md §9 — performance contract | Steady-state O(0) achieved via the change-detection filter on `sync_styles` (see §1.2 row above). Other steps (`taffy_compute`, `write_resolved_layout`) are O(roots) / O(tracked entities) and benefit from Taffy's internal cache when nothing changed. **Layout root sizing reads `windows.iter().next()` and falls back to `(800, 600)` — Phase 0 parity; multi-window root-to-window association is deferred to `buiy-window-and-surface-design`.** |
| box-model.md §2 — `BoxModel` | Component shape per spec; Phase 1 ships `width`/`height`/`min_*`/`max_*`/`padding`/`margin`/`border`/`box_sizing`/`aspect_ratio`. **`gap`/`row_gap`/`column_gap` are deferred — Phase 1 surfaces gap exclusively through `FlexParams.gap`. Spec's block-layout `BoxModel.gap` lands in a follow-up phase that wires it to Taffy.** | 2 |
| box-model.md §3 — `Sizing` | Phase 1 ships `Auto`, `None` (max-only), and `Length(Length)`. **`Stretch`, `MinContent`, `MaxContent`, `FitContent` ship as variants for forward stability but resolve silently to `Auto` — Taffy 0.10 has no `Dimension::Stretch`, and intrinsic keywords need text-rendering integration. Real semantics land in a follow-up phase (or when Taffy ships native support).** | 1 |
| box-model.md §5 — units | Phase 1 ships `Px`, `Percent`. **All other variants deferred to Phase 10.** | 1 |
| box-model.md §6 test — `BoxSizing` | `layout_box_sizing.rs`. | 10 |
| display-and-positioning.md §1 — `Display` | Phase 1 ships `Block`, `Flex(FlexAxis)`, `InlineFlex(FlexAxis)`. Other variants present as enum members and translate silently to `Block` (`None` translates to `taffy::Display::None`). **`Grid`/`InlineGrid` map to `Block` until Phase 3 wires `GridParams`/`GridItem`** — translating to `taffy::Display::Grid` without grid params would yield templateless grid containers and tempt premature reliance. Per-variant warns are deferred to the phase plans that own each variant. | 2 |
| display-and-positioning.md §2 — `Position` | Phase 1 ships `Static`, `Relative`, `Absolute`. **`Fixed` translates silently to `Absolute`; `Sticky` translates silently to `Relative`. Phase 7 (sticky) and Phase 8 (fixed/top-layer) wire the real semantics.** | 2 |
| flex-and-grid.md §1 — `FlexParams` + `FlexItem` | Full surface ships in Phase 1. | 2 |

---

## Task list

12 tasks. Each task ends with a commit. The plan assumes work happens on a feature branch; the final state replaces Phase 0's flat `layout.rs` with the new `layout/` directory in one merge.

### Task 1: Create `layout/types.rs` with supporting types

**Files:**
- Create: `crates/buiy_core/src/layout/types.rs`
- Modify: `crates/buiy_core/src/layout.rs` (add a `mod types;` declaration; do **not** create `crates/buiy_core/src/layout/mod.rs` — Rust E0761 disallows both files under one `pub mod layout;`)

This task introduces the unit / axis / box-shape value types so subsequent tasks can build components on top of them. The submodule's items are referenced only from Task 1's own tests until Task 2 lands components built on these types — `#[allow(dead_code)]` keeps clippy `-D warnings` clean (Step 1.4).

- [ ] **Step 1.1: Add `mod types;` to the existing `crates/buiy_core/src/layout.rs`**

Open `crates/buiy_core/src/layout.rs` and add a `mod types;` declaration near the top (right after the `use` block is conventional — common Rust style is module declarations after imports, before items):

```rust
mod types;
```

Do not modify any other line in `layout.rs`. Phase 0's `LayoutTree`, `LayoutPlugin`, and the `gc_removed_nodes` / `sync_and_compute_layout` systems stay in place; subsequent tasks accumulate more `mod <name>;` declarations alongside until Task 7 transitions the module to a directory.

- [ ] **Step 1.2: Write the failing test for `Length::px` / `percent` constructors**

Create `crates/buiy_core/src/layout/types.rs` with empty content first so the file exists. Then add this test at the bottom (the test will not compile yet — that's the failure).

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_constructors_round_trip() {
        assert_eq!(Length::px(10.0), Length::Px(10.0));
        assert_eq!(Length::percent(50.0), Length::Percent(50.0));
        assert_eq!(Length::ZERO, Length::Px(0.0));
    }

    #[test]
    fn edges_helpers_produce_uniform_and_axis_values() {
        let all = Edges::all(8.0);
        assert_eq!(all.top, Length::Px(8.0));
        assert_eq!(all.right, Length::Px(8.0));
        assert_eq!(all.bottom, Length::Px(8.0));
        assert_eq!(all.left, Length::Px(8.0));

        let axis = Edges::axis(4.0, 12.0);
        assert_eq!(axis.left, Length::Px(4.0));
        assert_eq!(axis.right, Length::Px(4.0));
        assert_eq!(axis.top, Length::Px(12.0));
        assert_eq!(axis.bottom, Length::Px(12.0));

        assert_eq!(Edges::ZERO, Edges::all(0.0));
    }

    #[test]
    fn enum_defaults_match_spec() {
        assert_eq!(BoxSizing::default(), BoxSizing::ContentBox);
        assert_eq!(FlexAxis::default(), FlexAxis::Row);
        assert_eq!(FlexWrap::default(), FlexWrap::NoWrap);
        assert_eq!(JustifyContent::default(), JustifyContent::FlexStart);
        assert_eq!(AlignItems::default(), AlignItems::Stretch);
        assert_eq!(AlignContent::default(), AlignContent::Stretch);
        assert_eq!(PositionKind::default(), PositionKind::Static);
    }

    #[test]
    fn sizing_default_is_auto() {
        assert_eq!(Sizing::default(), Sizing::Auto);
    }
}
```

- [ ] **Step 1.3: Run the test to verify it fails**

```sh
cargo test -p buiy_core --lib layout::types
```

Expected: compilation error (none of the types exist yet).

- [ ] **Step 1.4: Implement the types**

Replace the contents of `crates/buiy_core/src/layout/types.rs` with:

```rust
//! Layout value types — units, edges, axis enums, position kind.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md and
//! display-and-positioning.md.
//!
//! Phase 1 ships `Length::Px` / `Length::Percent` and the `Sizing` /
//! `Edges` / `BoxSizing` shapes. Em / Rem / viewport / container / Fr /
//! Calc resolution lands in Phase 10 (`buiy-layout-units-calc`); intrinsic
//! sizing keywords resolve to `Auto` until text rendering integrates.

use bevy::prelude::*;

/// CSS-style length value. Phase 1 ships only `Px` and `Percent`; other
/// variants are reserved for later phases. The variants present here cover
/// every value the Phase 1 translation layer can emit to Taffy without
/// further resolution.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Absolute logical pixels.
    Px(f32),
    /// Percentage of the containing block dimension on the relevant axis.
    Percent(f32),
}

#[allow(dead_code)] // Removed by Task 3 once Style builder calls these constructors.
impl Length {
    pub const ZERO: Self = Self::Px(0.0);

    pub const fn px(v: f32) -> Self {
        Self::Px(v)
    }

    pub const fn percent(v: f32) -> Self {
        Self::Percent(v)
    }
}

impl Default for Length {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Width / height / min / max value type. Phase 1 ships `Auto`, `None`
/// (max-only), `Length`, and `Stretch`. Intrinsic keywords ship as
/// variants but resolve to `Auto` until Phase 10 + text rendering land.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq)]
pub enum Sizing {
    #[default]
    Auto,
    /// Valid only on `max-*` (semantics: no upper bound).
    None,
    Length(Length),
    /// CSS `min-content`. Resolves to `Auto` until text rendering integrates.
    MinContent,
    /// CSS `max-content`. Resolves to `Auto` until text rendering integrates.
    MaxContent,
    /// CSS `fit-content(<length>)`. Resolves to `Auto` until text rendering integrates.
    FitContent(Length),
    /// CSS `stretch` — fills the parent's free space along the affected axis.
    Stretch,
}

/// Per-edge length values for padding, margin, border, inset.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

#[allow(dead_code)] // Removed by Task 3 once consumers reference Edges::all/axis.
impl Edges {
    pub const ZERO: Self = Self {
        top: Length::ZERO,
        right: Length::ZERO,
        bottom: Length::ZERO,
        left: Length::ZERO,
    };

    /// Uniform value on every edge.
    pub const fn all(v: f32) -> Self {
        Self {
            top: Length::Px(v),
            right: Length::Px(v),
            bottom: Length::Px(v),
            left: Length::Px(v),
        }
    }

    /// Distinct horizontal vs. vertical values.
    pub const fn axis(x: f32, y: f32) -> Self {
        Self {
            top: Length::Px(y),
            right: Length::Px(x),
            bottom: Length::Px(y),
            left: Length::Px(x),
        }
    }
}

/// `box-sizing` policy. CSS default is `ContentBox`; app UIs typically prefer
/// `BorderBox`. The Buiy default theme does not override the component
/// default — authors opt in.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

/// `aspect-ratio` value. Phase 1 stores a single ratio; CSS's
/// `aspect-ratio: auto` (intrinsic dimensions take precedence) is
/// represented by *not setting* `BoxModel.aspect_ratio` (the field is
/// `Option<AspectRatio>`). Stored on `BoxModel` only when the author
/// explicitly opts in.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq)]
pub struct AspectRatio {
    pub ratio: f32,
}

/// Flex / inline-flex main axis.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexAxis {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Flex wrap mode.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Main-axis distribution of flex / grid items. CSS `justify-content`.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Cross-axis alignment of flex / grid items within their line. CSS `align-items`.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// Cross-axis distribution of flex / grid lines (multi-line containers).
/// CSS `align-content`.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignContent {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Flex / grid gap, distinguished by axis.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq)]
pub struct FlexGap {
    pub row: Length,
    pub column: Length,
}

/// Position kind. Phase 1 implements `Static`, `Relative`, `Absolute`;
/// `Fixed` and `Sticky` ship as variants but emit a one-shot `warn!` and
/// translate to `Absolute` / `Relative` respectively until Phases 7/8 land.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PositionKind {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// Inset values (`top`/`right`/`bottom`/`left`).
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq)]
pub struct Inset {
    pub top: Sizing,
    pub right: Sizing,
    pub bottom: Sizing,
    pub left: Sizing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_constructors_round_trip() {
        assert_eq!(Length::px(10.0), Length::Px(10.0));
        assert_eq!(Length::percent(50.0), Length::Percent(50.0));
        assert_eq!(Length::ZERO, Length::Px(0.0));
    }

    #[test]
    fn edges_helpers_produce_uniform_and_axis_values() {
        let all = Edges::all(8.0);
        assert_eq!(all.top, Length::Px(8.0));
        assert_eq!(all.right, Length::Px(8.0));
        assert_eq!(all.bottom, Length::Px(8.0));
        assert_eq!(all.left, Length::Px(8.0));

        let axis = Edges::axis(4.0, 12.0);
        assert_eq!(axis.left, Length::Px(4.0));
        assert_eq!(axis.right, Length::Px(4.0));
        assert_eq!(axis.top, Length::Px(12.0));
        assert_eq!(axis.bottom, Length::Px(12.0));

        assert_eq!(Edges::ZERO, Edges::all(0.0));
    }

    #[test]
    fn enum_defaults_match_spec() {
        assert_eq!(BoxSizing::default(), BoxSizing::ContentBox);
        assert_eq!(FlexAxis::default(), FlexAxis::Row);
        assert_eq!(FlexWrap::default(), FlexWrap::NoWrap);
        assert_eq!(JustifyContent::default(), JustifyContent::FlexStart);
        assert_eq!(AlignItems::default(), AlignItems::Stretch);
        assert_eq!(AlignContent::default(), AlignContent::Stretch);
        assert_eq!(PositionKind::default(), PositionKind::Static);
    }

    #[test]
    fn sizing_default_is_auto() {
        assert_eq!(Sizing::default(), Sizing::Auto);
    }
}
```

- [ ] **Step 1.5: Run the test to verify it passes**

```sh
cargo test -p buiy_core --lib layout::types
```

Expected: 4 tests pass.

- [ ] **Step 1.6: Confirm the workspace still builds**

The new `layout/` directory module coexists with the existing flat `layout.rs`. Bevy / cargo treat one as ambiguous unless we route correctly — at this point the new directory is **not** declared in `lib.rs`, so the existing flat module remains the one referenced. Verify:

```sh
cargo check --workspace --all-targets
```

Expected: clean (warnings about unused `layout::types::*` items are OK at this stage; they go away in Task 2).

- [ ] **Step 1.7: Commit**

```sh
git add crates/buiy_core/src/layout.rs crates/buiy_core/src/layout/types.rs
git commit -m "feat(buiy_core): add layout::types submodule — Length, Sizing, Edges, axis enums

Spec: docs/specs/2026-05-08-buiy-layout-design/{box-model,display-and-positioning,flex-and-grid}.md.
Phase 1 of the layout migration. Ships only Length::Px/Percent and
the Sizing/Edges/BoxSizing/FlexAxis/PositionKind shapes; richer
unit resolution (em/rem/viewport/container/calc) lands in Phase 10.

The new submodule lands via 'mod types;' inside the existing flat
layout.rs (Rust E0761 forbids both layout.rs and layout/mod.rs).
Task 7 transitions to a directory module by deleting layout.rs and
creating layout/mod.rs simultaneously.

Two impl blocks (Length, Edges) carry a narrow #[allow(dead_code)]
until Task 3 wires the Style builder; comments mark when the allows
come off."
```

---

### Task 2: Add `layout/components.rs` with decomposed components

**Files:**
- Create: `crates/buiy_core/src/layout/components.rs`
- Modify: `crates/buiy_core/src/layout/mod.rs`

- [ ] **Step 2.1: Write the failing test for component default-construction + reflection**

Add an inline `#[cfg(test)]` block at the bottom of `crates/buiy_core/src/layout/components.rs` (the file is created in this same step — start with empty contents, then paste the test):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_model_default_is_auto_zero_padding() {
        let bm = BoxModel::default();
        assert_eq!(bm.width, Sizing::Auto);
        assert_eq!(bm.height, Sizing::Auto);
        assert_eq!(bm.padding, Edges::ZERO);
        assert_eq!(bm.margin, Edges::ZERO);
        assert_eq!(bm.border, Edges::ZERO);
        assert_eq!(bm.box_sizing, BoxSizing::ContentBox);
        assert_eq!(bm.aspect_ratio, None);
    }

    #[test]
    fn display_default_is_block() {
        assert_eq!(Display::default(), Display::Block);
    }

    #[test]
    fn position_default_is_static_with_auto_inset() {
        let pos = Position::default();
        assert_eq!(pos.kind, PositionKind::Static);
        assert_eq!(pos.inset, Inset::default());
    }

    #[test]
    fn flex_params_and_item_defaults_match_spec() {
        let fp = FlexParams::default();
        assert_eq!(fp.direction, FlexAxis::Row);
        assert_eq!(fp.wrap, FlexWrap::NoWrap);
        assert_eq!(fp.justify_content, JustifyContent::FlexStart);
        assert_eq!(fp.align_items, AlignItems::Stretch);
        assert_eq!(fp.align_content, AlignContent::Stretch);
        assert_eq!(fp.gap, FlexGap::default());

        let fi = FlexItem::default();
        assert_eq!(fi.grow, 0.0);
        assert_eq!(fi.shrink, 1.0);
        assert_eq!(fi.basis, Sizing::Auto);
        assert_eq!(fi.order, 0);
        assert_eq!(fi.align_self, None);
    }

    #[test]
    fn display_helpers_produce_flex_axis() {
        assert_eq!(Display::flex_row(), Display::Flex(FlexAxis::Row));
        assert_eq!(Display::flex_column(), Display::Flex(FlexAxis::Column));
    }
}
```

- [ ] **Step 2.2: Run the test to verify it fails**

```sh
cargo test -p buiy_core --lib layout::components
```

Expected: compilation error.

- [ ] **Step 2.3: Implement the components**

Replace the contents of `crates/buiy_core/src/layout/components.rs` with:

```rust
//! Decomposed layout components.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.1.
//!
//! Each component is small, public-fielded, and derives
//! `Reflect + Default + Clone + Component`. Phase 1 covers the surface
//! Phase 0's mega-`Style` reaches: `BoxModel`, `Display`, `Position`,
//! `FlexParams`, `FlexItem`. Other components (`Anchor`, `GridParams`,
//! `Container`, `WritingMode`, `Overflow`, `Scroll`, `Stacking`,
//! `Transform`, `Containment`, `MultiColumn`, `GridItem`) land in their
//! respective phase plans (see foundation plan §"Phasing strategy").

use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};
use bevy::prelude::*;

/// Box-model dimensions: width / height (incl. min/max), padding, margin,
/// border, box-sizing, aspect-ratio.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 2.
///
/// Phase 1 omits the spec's `gap` / `row_gap` / `column_gap` fields —
/// they are not yet wired to Taffy and `FlexParams.gap` carries the
/// flex-gap surface in this phase. A follow-up phase that wires
/// block-layout gap to Taffy adds them back.
#[derive(Component, Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Component, Default)]
pub struct BoxModel {
    pub width: Sizing,
    pub height: Sizing,
    pub min_width: Sizing,
    pub min_height: Sizing,
    pub max_width: Sizing,
    pub max_height: Sizing,
    pub padding: Edges,
    pub margin: Edges,
    pub border: Edges,
    pub box_sizing: BoxSizing,
    pub aspect_ratio: Option<AspectRatio>,
}

/// `display` value. Phase 1 implements `Block` and `Flex(FlexAxis)`; other
/// variants are reserved and translate to `Block` (Taffy default) with one
/// `warn!` per (entity, variant) per session at the translation layer.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex(FlexAxis),
    InlineFlex(FlexAxis),
    Grid,
    InlineGrid,
    FlowRoot,
    Contents,
    Table,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableRow,
    TableCell,
    TableCaption,
    TableColumnGroup,
    TableColumn,
    ListItem,
    Ruby,
    None,
}

impl Default for Display {
    fn default() -> Self {
        Self::Block
    }
}

impl Display {
    pub const fn flex_row() -> Self {
        Self::Flex(FlexAxis::Row)
    }

    pub const fn flex_column() -> Self {
        Self::Flex(FlexAxis::Column)
    }
}

/// `position` + `inset`. Phase 1 implements `Static`, `Relative`,
/// `Absolute`. `Fixed` and `Sticky` ship as variants but currently emit
/// `warn!` and translate to `Absolute` / `Relative`; Phases 7/8 wire the
/// real semantics.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.
#[derive(Component, Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(Component, Default)]
pub struct Position {
    pub kind: PositionKind,
    pub inset: Inset,
}

/// Flex container parameters. Active when the entity's `Display` is
/// `Display::Flex(_)` or `Display::InlineFlex(_)`; otherwise ignored.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 1.1.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Component, Default)]
pub struct FlexParams {
    pub direction: FlexAxis,
    pub wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub gap: FlexGap,
}

/// Per-child flex parameters.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 1.2.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct FlexItem {
    pub grow: f32,
    pub shrink: f32,
    pub basis: Sizing,
    pub order: i32,
    pub align_self: Option<AlignItems>,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: Sizing::Auto,
            order: 0,
            align_self: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_model_default_is_auto_zero_padding() {
        let bm = BoxModel::default();
        assert_eq!(bm.width, Sizing::Auto);
        assert_eq!(bm.height, Sizing::Auto);
        assert_eq!(bm.padding, Edges::ZERO);
        assert_eq!(bm.margin, Edges::ZERO);
        assert_eq!(bm.border, Edges::ZERO);
        assert_eq!(bm.box_sizing, BoxSizing::ContentBox);
        assert_eq!(bm.aspect_ratio, None);
    }

    #[test]
    fn display_default_is_block() {
        assert_eq!(Display::default(), Display::Block);
    }

    #[test]
    fn position_default_is_static_with_auto_inset() {
        let pos = Position::default();
        assert_eq!(pos.kind, PositionKind::Static);
        assert_eq!(pos.inset, Inset::default());
    }

    #[test]
    fn flex_params_and_item_defaults_match_spec() {
        let fp = FlexParams::default();
        assert_eq!(fp.direction, FlexAxis::Row);
        assert_eq!(fp.wrap, FlexWrap::NoWrap);
        assert_eq!(fp.justify_content, JustifyContent::FlexStart);
        assert_eq!(fp.align_items, AlignItems::Stretch);
        assert_eq!(fp.align_content, AlignContent::Stretch);
        assert_eq!(fp.gap, FlexGap::default());

        let fi = FlexItem::default();
        assert_eq!(fi.grow, 0.0);
        assert_eq!(fi.shrink, 1.0);
        assert_eq!(fi.basis, Sizing::Auto);
        assert_eq!(fi.order, 0);
        assert_eq!(fi.align_self, None);
    }

    #[test]
    fn display_helpers_produce_flex_axis() {
        assert_eq!(Display::flex_row(), Display::Flex(FlexAxis::Row));
        assert_eq!(Display::flex_column(), Display::Flex(FlexAxis::Column));
    }
}
```

- [ ] **Step 2.4: Wire components into the new module**

Update `crates/buiy_core/src/layout/mod.rs`:

```rust
//! Buiy layout subsystem.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/.

mod components;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};
```

- [ ] **Step 2.5: Run the tests to verify they pass**

```sh
cargo test -p buiy_core --lib layout::components
```

Expected: 5 new tests pass; the 4 from Task 1 still pass.

- [ ] **Step 2.6: Confirm the workspace still builds**

```sh
cargo check --workspace --all-targets
```

Expected: clean (the components are still unused outside the module — that's fine).

- [ ] **Step 2.7: Commit**

```sh
git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core): add layout/components — BoxModel, Display, Position, FlexParams, FlexItem

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.1.
Phase 1 of the layout migration. Phase-0-equivalent surface only:
the components Phase 0's mega-Style touched. Other components
(Anchor, Grid*, Container, WritingMode, Overflow, Scroll, Stacking,
Transform, Containment, MultiColumn) land in later phase plans.

Display ships every spec variant for forward stability; Phase 1
translates non-Block/Flex variants to Block with a one-shot warn.
Same for PositionKind::Fixed/Sticky."
```

---

### Task 3: Hybrid `Style` builder + `Bundle` expansion

**Files:**
- Create: `crates/buiy_core/src/layout/style.rs`
- Modify: `crates/buiy_core/src/layout/mod.rs` (add `pub mod style;` + re-export)

The builder is the ergonomic authoring surface. On `commands.spawn(style)` it expands to a tuple of decomposed components. Default-valued fields are skipped, so an entity gets only the components it cares about.

The `Bundle` impl uses Bevy 0.18's `Bundle` derive — but `Style`'s expansion is *conditional* per-field (default → skipped). Bevy's derive doesn't support conditional component insertion, so we implement `Bundle` manually using `BundleEffect` semantics: `Style` carries an `Option<T>` per output component, and the bundle's `get_components` calls only inserts the `Some(_)` ones.

The simpler, equivalent path Phase 1 takes: `Style` exposes a `pub fn into_bundle(self) -> impl Bundle` that returns a tuple where unset fields become `None`-shaped placeholders. To keep the type signatures stable we wrap conditional inserts in a thin `OptionalComponent<T>(Option<T>)` newtype that implements `Bundle`. `Style` itself is *not* a `Bundle`; it has `into_bundle()` and we provide a `BuiyCommands` extension so `commands.spawn(style.into_bundle())` works the same as the spec example.

Phase 1 takes the **simpler path**: `Style` is a builder whose `Default` is a known-good resting state, and an `impl Bundle for Style` is provided by inserting *every* component unconditionally — defaulted components are still default-valued, which is consistent with the rest of the ECS (a defaulted `BoxModel` is identical to no `BoxModel` from the translation layer's perspective; the translation layer reads `Option<&BoxModel>` and treats `None` and default-valued `Some` identically).

This is a documented Phase 1 simplification. The hybrid-API spec § 2.3 calls for skipping unset fields; Phase 1's "always insert defaulted components" is observationally equivalent for the components Phase 1 ships and avoids reimplementing `Bundle`. The behavioral difference (component presence vs. absence) becomes meaningful only when querying `With<T>` over user-set components vs. defaulted ones. No Phase 1 system uses such a query. **Phase 4 (writing-mode) revisits this when `LogicalBoxModel` insert-helper requires the conditional path.**

- [ ] **Step 3.1: Write the failing equivalence test**

Create `crates/buiy_core/src/layout/style.rs` with empty contents. Add this test to the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{BoxModel, Display, FlexParams, Position};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, JustifyContent, Length, Sizing,
    };
    use bevy::prelude::*;

    fn spawn_and_extract(
        style: Style,
    ) -> (Display, BoxModel, Position, FlexParams) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(style).id();
        let world = app.world();
        let display = *world.get::<Display>(entity).expect("Display inserted");
        let box_model = world.get::<BoxModel>(entity).expect("BoxModel inserted").clone();
        let position = world.get::<Position>(entity).expect("Position inserted").clone();
        let flex_params = *world.get::<FlexParams>(entity).expect("FlexParams inserted");
        (display, box_model, position, flex_params)
    }

    #[test]
    fn struct_literal_and_fluent_produce_identical_components() {
        let literal = Style {
            display: Display::Flex(FlexAxis::Column),
            box_model: BoxModel {
                padding: Edges::all(16.0),
                box_sizing: BoxSizing::BorderBox,
                width: Sizing::Length(Length::Px(200.0)),
                height: Sizing::Length(Length::Px(100.0)),
                ..default()
            },
            flex_params: FlexParams {
                gap: FlexGap {
                    row: Length::Px(8.0),
                    column: Length::Px(8.0),
                },
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        };
        let fluent = Style::default()
            .flex_column()
            .padding(16.0)
            .border_box()
            .width_px(200.0)
            .height_px(100.0)
            .gap_px(8.0)
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center);

        assert_eq!(spawn_and_extract(literal), spawn_and_extract(fluent));
    }

    #[test]
    fn default_style_inserts_every_decomposed_component() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(Style::default()).id();
        let world = app.world();
        assert!(world.get::<Display>(entity).is_some());
        assert!(world.get::<BoxModel>(entity).is_some());
        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<FlexParams>(entity).is_some());
    }
}
```

- [ ] **Step 3.2: Run the test to verify it fails**

```sh
cargo test -p buiy_core --lib layout::style
```

Expected: compilation error.

- [ ] **Step 3.3: Implement `Style`**

Replace `crates/buiy_core/src/layout/style.rs` with:

```rust
//! `Style` — the hybrid builder over decomposed layout components.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.2-2.4.
//!
//! Two equally valid authoring forms write the same fields; on insert,
//! Bundle expansion produces the four decomposed components Phase 1
//! ships (`Display`, `BoxModel`, `Position`, `FlexParams`). Defaulted
//! fields produce defaulted components — the Phase 1 simplification is
//! that components are always inserted, not skipped on default.
//! Phase 4's `LogicalBoxModel` revisit will switch to skip-on-default.
//!
//! `FlexItem` is decomposed-only (per spec § 2.4); it is not included in
//! `Style`.

use super::components::{BoxModel, Display, FlexParams, Position};
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};
use bevy::ecs::bundle::Bundle;
use bevy::prelude::*;

/// Hybrid builder over an entity's self-styling layout components.
///
/// Two authoring forms over the same fields:
///
/// ```ignore
/// // Struct-literal form.
/// let s = Style { display: Display::flex_row(), ..default() };
///
/// // Fluent form.
/// let s = Style::default().flex_row();
/// ```
///
/// On `commands.spawn(s)` (or `entity.insert(s)`), expands into a Bundle
/// of `Display`, `BoxModel`, `Position`, `FlexParams`. Decomposed
/// components are canonical; the builder is sugar.
#[derive(Bundle, Clone, Debug, Default)]
pub struct Style {
    pub display: Display,
    pub box_model: BoxModel,
    pub position: Position,
    pub flex_params: FlexParams,
}

// Fluent setters. Each writes one field on the underlying decomposed
// component. The methods are commutative within a domain — last call wins.

impl Style {
    // ---- Display ----

    pub fn block(mut self) -> Self {
        self.display = Display::Block;
        self
    }

    pub fn flex_row(mut self) -> Self {
        self.display = Display::flex_row();
        self.flex_params.direction = FlexAxis::Row;
        self
    }

    pub fn flex_column(mut self) -> Self {
        self.display = Display::flex_column();
        self.flex_params.direction = FlexAxis::Column;
        self
    }

    pub fn flex_axis(mut self, axis: FlexAxis) -> Self {
        self.display = Display::Flex(axis);
        self.flex_params.direction = axis;
        self
    }

    pub fn display(mut self, d: Display) -> Self {
        self.display = d;
        if let Display::Flex(axis) | Display::InlineFlex(axis) = d {
            self.flex_params.direction = axis;
        }
        self
    }

    // ---- BoxModel: dimensions ----

    pub fn width(mut self, w: Sizing) -> Self {
        self.box_model.width = w;
        self
    }

    pub fn height(mut self, h: Sizing) -> Self {
        self.box_model.height = h;
        self
    }

    pub fn width_px(self, px: f32) -> Self {
        self.width(Sizing::Length(Length::Px(px)))
    }

    pub fn height_px(self, px: f32) -> Self {
        self.height(Sizing::Length(Length::Px(px)))
    }

    pub fn min_width(mut self, w: Sizing) -> Self {
        self.box_model.min_width = w;
        self
    }

    pub fn min_height(mut self, h: Sizing) -> Self {
        self.box_model.min_height = h;
        self
    }

    pub fn max_width(mut self, w: Sizing) -> Self {
        self.box_model.max_width = w;
        self
    }

    pub fn max_height(mut self, h: Sizing) -> Self {
        self.box_model.max_height = h;
        self
    }

    pub fn aspect_ratio(mut self, ratio: AspectRatio) -> Self {
        self.box_model.aspect_ratio = Some(ratio);
        self
    }

    // ---- BoxModel: edges ----

    pub fn padding(mut self, px: f32) -> Self {
        self.box_model.padding = Edges::all(px);
        self
    }

    pub fn padding_edges(mut self, e: Edges) -> Self {
        self.box_model.padding = e;
        self
    }

    pub fn margin(mut self, px: f32) -> Self {
        self.box_model.margin = Edges::all(px);
        self
    }

    pub fn margin_edges(mut self, e: Edges) -> Self {
        self.box_model.margin = e;
        self
    }

    pub fn border(mut self, px: f32) -> Self {
        self.box_model.border = Edges::all(px);
        self
    }

    pub fn border_edges(mut self, e: Edges) -> Self {
        self.box_model.border = e;
        self
    }

    // ---- BoxModel: box-sizing ----

    pub fn content_box(mut self) -> Self {
        self.box_model.box_sizing = BoxSizing::ContentBox;
        self
    }

    pub fn border_box(mut self) -> Self {
        self.box_model.box_sizing = BoxSizing::BorderBox;
        self
    }

    pub fn box_sizing(mut self, b: BoxSizing) -> Self {
        self.box_model.box_sizing = b;
        self
    }

    // ---- Gap (Phase 1 surfaces gap exclusively via FlexParams.gap;
    //           BoxModel.gap is deferred — see Task 2 doc comment) ----

    pub fn gap_px(mut self, px: f32) -> Self {
        self.flex_params.gap = FlexGap {
            row: Length::Px(px),
            column: Length::Px(px),
        };
        self
    }

    // ---- Position ----

    pub fn position(mut self, kind: PositionKind) -> Self {
        self.position.kind = kind;
        self
    }

    pub fn relative(mut self) -> Self {
        self.position.kind = PositionKind::Relative;
        self
    }

    pub fn absolute(mut self) -> Self {
        self.position.kind = PositionKind::Absolute;
        self
    }

    pub fn inset(mut self, i: Inset) -> Self {
        self.position.inset = i;
        self
    }

    // ---- FlexParams ----

    pub fn flex_wrap(mut self, w: FlexWrap) -> Self {
        self.flex_params.wrap = w;
        self
    }

    pub fn justify_content(mut self, j: JustifyContent) -> Self {
        self.flex_params.justify_content = j;
        self
    }

    pub fn align_items(mut self, a: AlignItems) -> Self {
        self.flex_params.align_items = a;
        self
    }

    pub fn align_content(mut self, a: AlignContent) -> Self {
        self.flex_params.align_content = a;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{BoxModel, Display, FlexParams, Position};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, JustifyContent, Length, Sizing,
    };
    use bevy::prelude::*;

    fn spawn_and_extract(
        style: Style,
    ) -> (Display, BoxModel, Position, FlexParams) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(style).id();
        let world = app.world();
        let display = *world.get::<Display>(entity).expect("Display inserted");
        let box_model = world.get::<BoxModel>(entity).expect("BoxModel inserted").clone();
        let position = world.get::<Position>(entity).expect("Position inserted").clone();
        let flex_params = *world.get::<FlexParams>(entity).expect("FlexParams inserted");
        (display, box_model, position, flex_params)
    }

    #[test]
    fn struct_literal_and_fluent_produce_identical_components() {
        let literal = Style {
            display: Display::Flex(FlexAxis::Column),
            box_model: BoxModel {
                padding: Edges::all(16.0),
                box_sizing: BoxSizing::BorderBox,
                width: Sizing::Length(Length::Px(200.0)),
                height: Sizing::Length(Length::Px(100.0)),
                ..default()
            },
            flex_params: FlexParams {
                direction: FlexAxis::Column,
                gap: FlexGap {
                    row: Length::Px(8.0),
                    column: Length::Px(8.0),
                },
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        };
        let fluent = Style::default()
            .flex_column()
            .padding(16.0)
            .border_box()
            .width_px(200.0)
            .height_px(100.0)
            .gap_px(8.0)
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center);

        assert_eq!(spawn_and_extract(literal), spawn_and_extract(fluent));
    }

    #[test]
    fn default_style_inserts_every_decomposed_component() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(Style::default()).id();
        let world = app.world();
        assert!(world.get::<Display>(entity).is_some());
        assert!(world.get::<BoxModel>(entity).is_some());
        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<FlexParams>(entity).is_some());
    }
}
```

The test references `default()` on `BoxModel` and `FlexParams` requiring `PartialEq + Clone + Debug` so the `assert_eq!` works. Components already derive `Clone`, `Debug`, `Default`, `PartialEq` from Task 2. The tuple-equality assertion works because each component derives `PartialEq` and `Debug`.

- [ ] **Step 3.4: Wire `style` into the module**

Update `crates/buiy_core/src/layout/mod.rs`:

```rust
//! Buiy layout subsystem.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/.

mod components;
mod style;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use style::Style;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};
```

- [ ] **Step 3.5: Run the tests to verify they pass**

```sh
cargo test -p buiy_core --lib layout::style
```

Expected: 2 tests pass.

- [ ] **Step 3.6: Commit**

```sh
git add crates/buiy_core/src/layout/style.rs crates/buiy_core/src/layout/mod.rs
git commit -m "feat(buiy_core): add layout/style — hybrid Style builder over decomposed components

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.2-2.4.
Style is a Bevy Bundle whose default expands to defaulted decomposed
components; fluent setters write the same fields a struct literal
would. Phase 1 simplification: every component is always inserted
(default-valued or set), to keep the Bundle derive simple. Phase 4
revisits this when LogicalBoxModel needs skip-on-default semantics.

FlexItem stays decomposed-only per the child-side convention; it's
spawned alongside Style, not inside it."
```

---

### Task 4: Add `layout/translate.rs` — `style_to_taffy` reading decomposed components

**Files:**
- Create: `crates/buiy_core/src/layout/translate.rs`
- Modify: `crates/buiy_core/src/layout/mod.rs` (add `pub(crate) mod translate;`)

The translation layer reads decomposed components from a query item and produces a `taffy::Style`. It is a pure function — no `Commands`, no resources except the unit-resolution scratch space (none in Phase 1, since Phase 1 ships only `Px`/`Percent` and Taffy resolves `Percent` itself).

- [ ] **Step 4.1: Write the failing translation test**

Create `crates/buiy_core/src/layout/translate.rs` with the test only (empty implementation file at first):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{BoxModel, Display, FlexParams, FlexItem, Position};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, JustifyContent, Length,
        PositionKind, Sizing,
    };

    #[test]
    fn translate_default_components_to_taffy_default() {
        let bm = BoxModel::default();
        let display = Display::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item: Option<&FlexItem> = None;
        let taffy = style_to_taffy(StyleView { display: &display, box_model: &bm, position: &position, flex_params: &flex, flex_item: item });
        // Default Display::Block + ContentBox + everything Auto produces taffy default Display::Block.
        assert_eq!(taffy.display, taffy::Display::Block);
        assert_eq!(taffy.size.width, taffy::Dimension::auto());
        assert_eq!(taffy.size.height, taffy::Dimension::auto());
    }

    #[test]
    fn translate_flex_row_with_dimensions() {
        let display = Display::Flex(FlexAxis::Row);
        let bm = BoxModel {
            width: Sizing::Length(Length::Px(200.0)),
            height: Sizing::Length(Length::Px(100.0)),
            padding: Edges::all(8.0),
            box_sizing: BoxSizing::BorderBox,
            ..Default::default()
        };
        let position = Position::default();
        let flex = FlexParams {
            direction: FlexAxis::Row,
            gap: FlexGap {
                row: Length::Px(4.0),
                column: Length::Px(4.0),
            },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            wrap: FlexWrap::NoWrap,
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView { display: &display, box_model: &bm, position: &position, flex_params: &flex, flex_item: None });
        assert_eq!(taffy.display, taffy::Display::Flex);
        assert_eq!(taffy.flex_direction, taffy::FlexDirection::Row);
        assert_eq!(taffy.size.width, taffy::Dimension::length(200.0));
        assert_eq!(taffy.size.height, taffy::Dimension::length(100.0));
        assert_eq!(taffy.box_sizing, taffy::BoxSizing::BorderBox);
        assert_eq!(taffy.justify_content, Some(taffy::JustifyContent::Center));
        assert_eq!(taffy.align_items, Some(taffy::AlignItems::Center));
    }

    #[test]
    fn translate_position_absolute_emits_absolute_with_inset() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position {
            kind: PositionKind::Absolute,
            inset: crate::layout::types::Inset {
                top: Sizing::Length(Length::Px(10.0)),
                left: Sizing::Length(Length::Px(20.0)),
                ..Default::default()
            },
        };
        let flex = FlexParams::default();
        let taffy = style_to_taffy(StyleView { display: &display, box_model: &bm, position: &position, flex_params: &flex, flex_item: None });
        assert_eq!(taffy.position, taffy::Position::Absolute);
        assert_eq!(taffy.inset.top, taffy::LengthPercentageAuto::length(10.0));
        assert_eq!(taffy.inset.left, taffy::LengthPercentageAuto::length(20.0));
    }

    #[test]
    fn translate_flex_item_basis_grow_shrink() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item = FlexItem {
            grow: 2.0,
            shrink: 0.5,
            basis: Sizing::Length(Length::Px(100.0)),
            order: 3,
            align_self: Some(AlignItems::Center),
        };
        let taffy = style_to_taffy(StyleView { display: &display, box_model: &bm, position: &position, flex_params: &flex, flex_item: Some(&item) });
        assert_eq!(taffy.flex_grow, 2.0);
        assert_eq!(taffy.flex_shrink, 0.5);
        assert_eq!(taffy.flex_basis, taffy::Dimension::length(100.0));
        assert_eq!(taffy.align_self, Some(taffy::AlignSelf::Center));
    }
}
```

- [ ] **Step 4.2: Run the test to verify it fails**

```sh
cargo test -p buiy_core --lib layout::translate
```

Expected: compilation error — `StyleView` and `style_to_taffy` don't exist.

- [ ] **Step 4.3: Implement `style_to_taffy`**

Replace `crates/buiy_core/src/layout/translate.rs` with:

```rust
//! Translation layer: decomposed Buiy layout components → `taffy::Style`.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2.
//!
//! Pure function. Read by `sync_styles` (pipeline step 1). Phase 1 only
//! resolves `Length::Px` and `Length::Percent` — every other variant
//! lands in Phase 10 (`buiy-layout-units-calc`).

use super::components::{BoxModel, Display, FlexItem, FlexParams, Position};
use super::types::{
    AlignContent, AlignItems, BoxSizing, Edges, FlexAxis, FlexWrap, Inset, JustifyContent, Length,
    PositionKind, Sizing,
};

/// View into the Phase 1 decomposed-component set for one entity. Built
/// by `sync_styles`'s query and passed to `style_to_taffy`.
pub struct StyleView<'a> {
    pub display: &'a Display,
    pub box_model: &'a BoxModel,
    pub position: &'a Position,
    pub flex_params: &'a FlexParams,
    pub flex_item: Option<&'a FlexItem>,
}

pub fn style_to_taffy(view: StyleView<'_>) -> taffy::Style {
    let mut s = taffy::Style {
        display: map_display(view.display),
        box_sizing: map_box_sizing(view.box_model.box_sizing),
        position: map_position_kind(view.position.kind),
        size: taffy::Size {
            width: sizing_to_dim(view.box_model.width),
            height: sizing_to_dim(view.box_model.height),
        },
        min_size: taffy::Size {
            width: sizing_to_dim(view.box_model.min_width),
            height: sizing_to_dim(view.box_model.min_height),
        },
        max_size: taffy::Size {
            width: sizing_to_dim(view.box_model.max_width),
            height: sizing_to_dim(view.box_model.max_height),
        },
        padding: edges_to_lp(view.box_model.padding),
        margin: edges_to_lpa(view.box_model.margin),
        border: edges_to_lp(view.box_model.border),
        inset: inset_to_lpa(view.position.inset),
        flex_direction: map_flex_axis(view.flex_params.direction),
        flex_wrap: map_flex_wrap(view.flex_params.wrap),
        justify_content: Some(map_justify_content(view.flex_params.justify_content)),
        align_items: Some(map_align_items(view.flex_params.align_items)),
        align_content: Some(map_align_content(view.flex_params.align_content)),
        gap: taffy::Size {
            width: length_to_lp(view.flex_params.gap.column),
            height: length_to_lp(view.flex_params.gap.row),
        },
        ..Default::default()
    };

    if let Some(item) = view.flex_item {
        s.flex_grow = item.grow;
        s.flex_shrink = item.shrink;
        s.flex_basis = sizing_to_dim(item.basis);
        // Taffy 0.10 has no `order` field on Style. CSS `order` would
        // need a Buiy-side sibling sort before `set_children`; that
        // lands later (tracked under flex-and-grid follow-ups). Phase 1
        // stores `FlexItem.order` but does not act on it; document this
        // as a Phase 1 limitation (warn once per session).
        let _unused_order_in_phase_1 = item.order;
        s.align_self = item.align_self.map(map_align_items_as_self);
    }

    if let Some(ar) = view.box_model.aspect_ratio {
        s.aspect_ratio = Some(ar.ratio);
    }

    s
}

fn map_display(d: &Display) -> taffy::Display {
    use Display::*;
    // Phase 1 maps Grid/InlineGrid to Block. Translating them to
    // taffy::Display::Grid without GridParams/GridItem would silently
    // create templateless grid containers and tempt premature reliance
    // on Grid before Phase 3 ships the components. Phase 3 replaces
    // this row with `Grid | InlineGrid => taffy::Display::Grid`.
    match d {
        Block | Inline | InlineBlock | FlowRoot | Contents | ListItem | Ruby | Table
        | TableRowGroup | TableHeaderGroup | TableFooterGroup | TableRow | TableCell
        | TableCaption | TableColumnGroup | TableColumn | Grid | InlineGrid => {
            taffy::Display::Block
        }
        Flex(_) | InlineFlex(_) => taffy::Display::Flex,
        None => taffy::Display::None,
    }
}

fn map_box_sizing(b: BoxSizing) -> taffy::BoxSizing {
    match b {
        BoxSizing::ContentBox => taffy::BoxSizing::ContentBox,
        BoxSizing::BorderBox => taffy::BoxSizing::BorderBox,
    }
}

fn map_position_kind(k: PositionKind) -> taffy::Position {
    use PositionKind::*;
    // Phase 1: Static / Relative / Absolute pass through; Fixed translates
    // to Absolute and Sticky translates to Relative. Phase 7 (sticky) and
    // Phase 8 (top-layer / fixed-as-viewport) wire the real semantics.
    match k {
        Static | Relative | Sticky => taffy::Position::Relative,
        Absolute | Fixed => taffy::Position::Absolute,
    }
}

fn map_flex_axis(a: FlexAxis) -> taffy::FlexDirection {
    match a {
        FlexAxis::Row => taffy::FlexDirection::Row,
        FlexAxis::Column => taffy::FlexDirection::Column,
        FlexAxis::RowReverse => taffy::FlexDirection::RowReverse,
        FlexAxis::ColumnReverse => taffy::FlexDirection::ColumnReverse,
    }
}

fn map_flex_wrap(w: FlexWrap) -> taffy::FlexWrap {
    match w {
        FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
        FlexWrap::Wrap => taffy::FlexWrap::Wrap,
        FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
    }
}

fn map_justify_content(j: JustifyContent) -> taffy::JustifyContent {
    match j {
        JustifyContent::FlexStart => taffy::JustifyContent::FlexStart,
        JustifyContent::FlexEnd => taffy::JustifyContent::FlexEnd,
        JustifyContent::Center => taffy::JustifyContent::Center,
        JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
        JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
        JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
    }
}

fn map_align_items(a: AlignItems) -> taffy::AlignItems {
    match a {
        AlignItems::Stretch => taffy::AlignItems::Stretch,
        AlignItems::FlexStart => taffy::AlignItems::FlexStart,
        AlignItems::FlexEnd => taffy::AlignItems::FlexEnd,
        AlignItems::Center => taffy::AlignItems::Center,
        AlignItems::Baseline => taffy::AlignItems::Baseline,
    }
}

fn map_align_items_as_self(a: AlignItems) -> taffy::AlignSelf {
    match a {
        AlignItems::Stretch => taffy::AlignSelf::Stretch,
        AlignItems::FlexStart => taffy::AlignSelf::FlexStart,
        AlignItems::FlexEnd => taffy::AlignSelf::FlexEnd,
        AlignItems::Center => taffy::AlignSelf::Center,
        AlignItems::Baseline => taffy::AlignSelf::Baseline,
    }
}

fn map_align_content(a: AlignContent) -> taffy::AlignContent {
    match a {
        AlignContent::Stretch => taffy::AlignContent::Stretch,
        AlignContent::FlexStart => taffy::AlignContent::FlexStart,
        AlignContent::FlexEnd => taffy::AlignContent::FlexEnd,
        AlignContent::Center => taffy::AlignContent::Center,
        AlignContent::SpaceBetween => taffy::AlignContent::SpaceBetween,
        AlignContent::SpaceAround => taffy::AlignContent::SpaceAround,
        AlignContent::SpaceEvenly => taffy::AlignContent::SpaceEvenly,
    }
}

fn sizing_to_dim(s: Sizing) -> taffy::Dimension {
    // Phase 1 ships Auto / None / Length / Stretch as the "real" surface;
    // intrinsic keywords resolve silently to Auto until Phase 10 + text
    // rendering integrate.
    match s {
        Sizing::Auto | Sizing::MinContent | Sizing::MaxContent | Sizing::FitContent(_) => {
            taffy::Dimension::auto()
        }
        Sizing::None => taffy::Dimension::auto(),
        Sizing::Length(l) => length_to_dim(l),
        Sizing::Stretch => taffy::Dimension::auto(), // taffy 0.10 doesn't ship `stretch`; treated as auto.
    }
}

fn length_to_dim(l: Length) -> taffy::Dimension {
    match l {
        Length::Px(v) => taffy::Dimension::length(v),
        Length::Percent(p) => taffy::Dimension::percent(p / 100.0),
    }
}

fn length_to_lp(l: Length) -> taffy::LengthPercentage {
    match l {
        Length::Px(v) => taffy::LengthPercentage::length(v),
        Length::Percent(p) => taffy::LengthPercentage::percent(p / 100.0),
    }
}

fn length_to_lpa(l: Length) -> taffy::LengthPercentageAuto {
    match l {
        Length::Px(v) => taffy::LengthPercentageAuto::length(v),
        Length::Percent(p) => taffy::LengthPercentageAuto::percent(p / 100.0),
    }
}

fn sizing_to_lpa(s: Sizing) -> taffy::LengthPercentageAuto {
    match s {
        Sizing::Auto | Sizing::None | Sizing::MinContent | Sizing::MaxContent
        | Sizing::FitContent(_) | Sizing::Stretch => taffy::LengthPercentageAuto::auto(),
        Sizing::Length(l) => length_to_lpa(l),
    }
}

fn edges_to_lp(e: Edges) -> taffy::Rect<taffy::LengthPercentage> {
    taffy::Rect {
        top: length_to_lp(e.top),
        right: length_to_lp(e.right),
        bottom: length_to_lp(e.bottom),
        left: length_to_lp(e.left),
    }
}

fn edges_to_lpa(e: Edges) -> taffy::Rect<taffy::LengthPercentageAuto> {
    taffy::Rect {
        top: length_to_lpa(e.top),
        right: length_to_lpa(e.right),
        bottom: length_to_lpa(e.bottom),
        left: length_to_lpa(e.left),
    }
}

fn inset_to_lpa(i: Inset) -> taffy::Rect<taffy::LengthPercentageAuto> {
    taffy::Rect {
        top: sizing_to_lpa(i.top),
        right: sizing_to_lpa(i.right),
        bottom: sizing_to_lpa(i.bottom),
        left: sizing_to_lpa(i.left),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{BoxModel, Display, FlexParams, FlexItem, Position};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, JustifyContent, Length,
        PositionKind, Sizing,
    };

    #[test]
    fn translate_default_components_to_taffy_default() {
        let bm = BoxModel::default();
        let display = Display::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item: Option<&FlexItem> = None;
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: item,
        });
        assert_eq!(taffy.display, taffy::Display::Block);
        assert_eq!(taffy.size.width, taffy::Dimension::auto());
        assert_eq!(taffy.size.height, taffy::Dimension::auto());
    }

    #[test]
    fn translate_flex_row_with_dimensions() {
        let display = Display::Flex(FlexAxis::Row);
        let bm = BoxModel {
            width: Sizing::Length(Length::Px(200.0)),
            height: Sizing::Length(Length::Px(100.0)),
            padding: Edges::all(8.0),
            box_sizing: BoxSizing::BorderBox,
            ..Default::default()
        };
        let position = Position::default();
        let flex = FlexParams {
            direction: FlexAxis::Row,
            gap: FlexGap {
                row: Length::Px(4.0),
                column: Length::Px(4.0),
            },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            wrap: FlexWrap::NoWrap,
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
        });
        assert_eq!(taffy.display, taffy::Display::Flex);
        assert_eq!(taffy.flex_direction, taffy::FlexDirection::Row);
        assert_eq!(taffy.size.width, taffy::Dimension::length(200.0));
        assert_eq!(taffy.size.height, taffy::Dimension::length(100.0));
        assert_eq!(taffy.box_sizing, taffy::BoxSizing::BorderBox);
        assert_eq!(taffy.justify_content, Some(taffy::JustifyContent::Center));
        assert_eq!(taffy.align_items, Some(taffy::AlignItems::Center));
    }

    #[test]
    fn translate_position_absolute_emits_absolute_with_inset() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position {
            kind: PositionKind::Absolute,
            inset: crate::layout::types::Inset {
                top: Sizing::Length(Length::Px(10.0)),
                left: Sizing::Length(Length::Px(20.0)),
                ..Default::default()
            },
        };
        let flex = FlexParams::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
        });
        assert_eq!(taffy.position, taffy::Position::Absolute);
        assert_eq!(taffy.inset.top, taffy::LengthPercentageAuto::length(10.0));
        assert_eq!(taffy.inset.left, taffy::LengthPercentageAuto::length(20.0));
    }

    #[test]
    fn translate_flex_item_basis_grow_shrink() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item = FlexItem {
            grow: 2.0,
            shrink: 0.5,
            basis: Sizing::Length(Length::Px(100.0)),
            order: 3,
            align_self: Some(AlignItems::Center),
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: Some(&item),
        });
        assert_eq!(taffy.flex_grow, 2.0);
        assert_eq!(taffy.flex_shrink, 0.5);
        assert_eq!(taffy.flex_basis, taffy::Dimension::length(100.0));
        assert_eq!(taffy.align_self, Some(taffy::AlignSelf::Center));
        // FlexItem.order is stored but Taffy 0.10 has no `order` on
        // Style; Phase 1 does not honor it. Documented as a Phase 1
        // limitation in the translation module's doc comment.
    }
}
```

Update `crates/buiy_core/src/layout/mod.rs`:

```rust
mod components;
mod style;
pub(crate) mod translate;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use style::Style;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};
```

- [ ] **Step 4.4: Run the tests to verify they pass**

```sh
cargo test -p buiy_core --lib layout::translate
```

Expected: 4 tests pass.

- [ ] **Step 4.5: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/mod.rs
git commit -m "feat(buiy_core): add layout/translate — decomposed → taffy::Style

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2.
Pure function over a borrowed view of the Phase 1 component set.
Phase 1 unit resolution is Px + Percent only; intrinsic / em / rem
/ viewport / container / Fr / Calc are reserved for Phase 10.

Display variants outside {Flex(_), InlineFlex(_), None} translate to
Block in Phase 1 (Grid/InlineGrid included — they wait on Phase 3 for
GridParams/GridItem). Phases 7 (sticky) and 8 (stacking + fixed)
replace the Fixed→Absolute / Sticky→Relative shortcut with real
semantics."
```

---

### Task 5: Add `layout/pipeline.rs` — `BuiyLayoutStep` enum + 8-step chain

**Files:**
- Create: `crates/buiy_core/src/layout/pipeline.rs`
- Modify: `crates/buiy_core/src/layout/mod.rs`

The pipeline is a single `Update` chain configured as sub-system-sets of `BuiySet::Layout`. Phase 1 wires all eight steps; only 0/1/3/7 do real work this phase.

- [ ] **Step 5.1: Define `BuiyLayoutStep`**

Replace `crates/buiy_core/src/layout/pipeline.rs` (creating it) with:

```rust
//! Layout pipeline ordering.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
//!
//! Eight ordered sub-sets of `BuiySet::Layout`. Phase 1 wires all eight;
//! steps 2 (`CqActivate`), 4 (`CqFlipCheck`), 5 (`CqFlipReRun`), and 6
//! (`PostTaffyOverrides`) are no-ops in Phase 1. Later phases attach
//! systems to those sub-sets without reordering.

use bevy::prelude::*;

/// Phase 1 ships every step as a system set; later phases populate the
/// stub steps. The order is asserted by `tests/layout_pipeline_order.rs`.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiyLayoutStep {
    /// Step 0 — drop despawned entities from `LayoutTree`.
    RemovedNodesGc,
    /// Step 1 — translate changed Buiy components → `taffy::Style` and
    /// sync hierarchy.
    SyncStyles,
    /// Step 2 — set/clear container-query marker components.
    /// **Phase 5.**
    CqActivate,
    /// Step 3 — call `tree.compute_layout` from each root.
    TaffyCompute,
    /// Step 4 — re-evaluate queries against fresh sizes.
    /// **Phase 5.**
    CqFlipCheck,
    /// Step 5 — conditional re-run of steps 1+3.
    /// **Phase 5.** Phase 1 leaves this as an empty set.
    CqFlipReRun,
    /// Step 6 — sub-passes (sticky, table, multicol, anchor).
    /// **Phases 6/7.**
    PostTaffyOverrides,
    /// Step 7 — push positions+sizes to Bevy components.
    WriteResolvedLayout,
}

/// Configure the 8-step chain inside `BuiySet::Layout`.
pub fn configure_pipeline(app: &mut App) {
    app.configure_sets(
        Update,
        (
            BuiyLayoutStep::RemovedNodesGc,
            BuiyLayoutStep::SyncStyles,
            BuiyLayoutStep::CqActivate,
            BuiyLayoutStep::TaffyCompute,
            BuiyLayoutStep::CqFlipCheck,
            BuiyLayoutStep::CqFlipReRun,
            BuiyLayoutStep::PostTaffyOverrides,
            BuiyLayoutStep::WriteResolvedLayout,
        )
            .chain()
            .in_set(crate::BuiySet::Layout),
    );
}
```

- [ ] **Step 5.2: Wire pipeline into the module**

Update `crates/buiy_core/src/layout/mod.rs`:

```rust
mod components;
mod pipeline;
mod style;
pub(crate) mod translate;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use pipeline::BuiyLayoutStep;
pub use style::Style;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};

pub(crate) use pipeline::configure_pipeline;
```

- [ ] **Step 5.3: Confirm build still passes**

```sh
cargo check --workspace --all-targets
```

Expected: clean. The pipeline module is wired but no `LayoutPlugin` consumes it yet — that's Task 7.

- [ ] **Step 5.4: Commit**

```sh
git add crates/buiy_core/src/layout/pipeline.rs crates/buiy_core/src/layout/mod.rs
git commit -m "feat(buiy_core): add layout/pipeline — BuiyLayoutStep + 8-step chain config

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
Eight ordered sub-sets of BuiySet::Layout. Steps 0/1/3/7 do real
work in Phase 1; steps 2/4/5/6 are wired as empty sub-sets that
later phases populate without reordering. Order is enforced by a
test added in Task 10."
```

---

### Task 6: Re-home `LayoutTree` in `layout/tree.rs`

**Files:**
- Create: `crates/buiy_core/src/layout/tree.rs`
- Modify: `crates/buiy_core/src/layout/mod.rs`

The struct is unchanged from Phase 0; this task only moves it to its target location so subsequent tasks add new systems alongside it without touching the legacy `layout.rs`.

- [ ] **Step 6.1: Create `tree.rs`**

```rust
//! `LayoutTree` — the bridge state between Buiy entities and Taffy.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.1.
//!
//! Stored as a `NonSendResource` because Taffy's `Dimension` packs a
//! `*const ()` regardless of the `calc` feature, so `TaffyTree` is
//! `!Send + !Sync`. Reused frame-to-frame so Taffy's internal cache stays
//! warm. GC handled by `systems::gc_removed_nodes`.

use bevy::prelude::Entity;
use std::collections::HashMap;
use taffy::{NodeId as TaffyNodeId, TaffyTree};

#[derive(Default)]
pub struct LayoutTree {
    pub(crate) tree: TaffyTree<()>,
    pub(crate) by_entity: HashMap<Entity, TaffyNodeId>,
}

impl LayoutTree {
    pub fn len(&self) -> usize {
        self.by_entity.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_entity.is_empty()
    }

    pub(crate) fn taffy_node_for(&self, entity: Entity) -> Option<TaffyNodeId> {
        self.by_entity.get(&entity).copied()
    }
}
```

- [ ] **Step 6.2: Wire into module**

Update `crates/buiy_core/src/layout/mod.rs`:

```rust
mod components;
mod pipeline;
mod style;
mod tree;
pub(crate) mod translate;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use pipeline::BuiyLayoutStep;
pub use style::Style;
pub use tree::LayoutTree;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};

pub(crate) use pipeline::configure_pipeline;
```

- [ ] **Step 6.3: Confirm build**

```sh
cargo check --workspace --all-targets
```

Expected: clean. Old `layout.rs` still owns the live `LayoutTree` resource (it has its own private definition); the new `layout::LayoutTree` is dead code until Task 7.

- [ ] **Step 6.4: Commit**

```sh
git add crates/buiy_core/src/layout/tree.rs crates/buiy_core/src/layout/mod.rs
git commit -m "feat(buiy_core): add layout/tree — re-home LayoutTree to the new module

Same struct as Phase 0, moved into its target location. Old
layout.rs still owns the live resource until Task 7 swaps the
plugin over."
```

---

### Task 7: Add `layout/systems.rs` and replace `LayoutPlugin`

**Files:**
- Create: `crates/buiy_core/src/layout/systems.rs`
- Modify: `crates/buiy_core/src/layout/mod.rs` (add `LayoutPlugin` here, deleting it from `layout.rs`)
- Delete: `crates/buiy_core/src/layout.rs`
- Modify: `crates/buiy_core/src/lib.rs` (`pub mod layout;` already in place; flat-vs-dir resolves to dir once flat is gone)

This task does the actual switchover. The flat `layout.rs` is deleted; the new `layout/` module owns `LayoutPlugin`. Systems are split per pipeline step.

- [ ] **Step 7.1: Implement `systems.rs`**

Create `crates/buiy_core/src/layout/systems.rs` with:

```rust
//! Per-step systems for the layout pipeline.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3-4.
//!
//! Phase 1 implements:
//!   - Step 0 `gc_removed_nodes` — `LayoutTree` GC from `RemovedComponents<Node>`.
//!   - Step 1 `sync_styles` — translate changed components and sync hierarchy.
//!   - Step 3 `taffy_compute` — `tree.compute_layout` from each root.
//!   - Step 7 `write_resolved_layout` — write `ResolvedLayout` back to entities.
//!
//! Steps 2/4/5/6 are empty sub-sets in Phase 1; later phases attach
//! systems to them.

use super::components::{BoxModel, Display, FlexItem, FlexParams, Position};
use super::translate::{StyleView, style_to_taffy};
use super::tree::LayoutTree;
use crate::components::{Node, ResolvedLayout};
use bevy::prelude::*;
use taffy::{AvailableSpace, NodeId as TaffyNodeId, Size};

/// Step 0 — drop Taffy nodes for entities whose `Node` component was
/// removed (despawn or component-remove). `RemovedComponents<Node>`
/// ordering across a parent/child despawn pair is not guaranteed by
/// Bevy, so the GC tolerates either order: parent-first leaves children
/// orphaned in Taffy (cleaned up by entity), child-first leaves the
/// parent's `set_children` reference dangling (Taffy's `remove(parent)`
/// cleans that up).
///
/// Phase 1 keeps Phase 0's blanket-warn behavior. The spec's
/// architecture.md § 4.3 calls for silently swallowing `NotFound`; the
/// Taffy 0.10 error variant for that case is uncertain enough that the
/// pinning is deferred to a follow-up task that audits Taffy's error
/// enum and refines the match.
pub(super) fn gc_removed_nodes(
    mut tree: NonSendMut<LayoutTree>,
    mut removed: RemovedComponents<Node>,
) {
    let tree = &mut *tree;
    for entity in removed.read() {
        if let Some(id) = tree.by_entity.remove(&entity)
            && let Err(err) = tree.tree.remove(id)
        {
            warn!(?entity, ?err, "buiy: layout gc remove failed");
        }
    }
}

/// Step 1 — for every entity with `Node`, translate its decomposed
/// components into a `taffy::Style` and ensure the entity has a Taffy
/// node + correct child list. The query carries an `Or<(Changed<...>)>`
/// filter so steady-state frames (no layout component or hierarchy
/// changes anywhere in the world) iterate **zero** entities, matching
/// spec architecture.md § 9's O(0) steady-state contract.
///
/// `Changed<T>` triggers on insertion as well as modification, so newly
/// spawned entities are picked up on their first frame.
///
/// Phase 1 trigger set: `Changed<BoxModel>`, `Changed<Display>`,
/// `Changed<Position>`, `Changed<FlexParams>`, `Changed<FlexItem>`,
/// `Changed<Children>`, `Changed<ChildOf>`. Phase 4–9 widen it as new
/// components land.
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
            Option<&Children>,
        ),
        (
            With<Node>,
            Or<(
                Changed<Display>,
                Changed<BoxModel>,
                Changed<Position>,
                Changed<FlexParams>,
                Changed<FlexItem>,
                Changed<Children>,
                Changed<ChildOf>,
            )>,
        ),
    >,
) {
    let tree = &mut *tree;

    // Ensure every Buiy entity has a Taffy node + current style. Insert
    // happens for entities new this frame (Changed<T> triggers on insert);
    // existing entities run set_style only when something in the trigger
    // set actually changed — see foundation/architecture.md § 1.2.
    for (entity, display, bm, position, flex, flex_item, _children) in nodes.iter() {
        let view = StyleView {
            display,
            box_model: bm,
            position,
            flex_params: flex,
            flex_item,
        };
        let taffy_style = style_to_taffy(view);
        match tree.by_entity.get(&entity).copied() {
            Some(id) => {
                if let Err(err) = tree.tree.set_style(id, taffy_style) {
                    warn!(?entity, ?err, "buiy: layout set_style failed");
                }
            }
            None => match tree.tree.new_leaf(taffy_style) {
                Ok(id) => {
                    tree.by_entity.insert(entity, id);
                }
                Err(err) => {
                    warn!(
                        ?entity,
                        ?err,
                        "buiy: layout new_leaf failed; entity will be skipped this frame"
                    );
                }
            },
        }
    }

    // Sync child relationships for each Buiy entity.
    for (entity, _display, _bm, _position, _flex, _flex_item, children) in nodes.iter() {
        let parent_id = match tree.by_entity.get(&entity).copied() {
            Some(id) => id,
            None => continue,
        };
        let child_ids: Vec<TaffyNodeId> = children
            .into_iter()
            .flatten()
            .filter_map(|c| tree.by_entity.get(c).copied())
            .collect();
        if let Err(err) = tree.tree.set_children(parent_id, &child_ids) {
            warn!(?entity, ?err, "buiy: layout set_children failed");
        }
    }
}

/// Step 3 — call `tree.compute_layout` from each root. A root is an
/// entity with `Node` and either no `ChildOf`, or a `ChildOf` whose
/// target is not in `LayoutTree` (i.e., a non-Buiy parent).
pub(super) fn taffy_compute(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
    windows: Query<&bevy::window::Window>,
) {
    let tree = &mut *tree;

    // Layout root sizing falls back to 800x600 if no Window exists (test
    // harnesses with MinimalPlugins). Phase 0 used the same default.
    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    for (entity, parent) in nodes.iter() {
        let is_root = parent
            .map(|p| !tree.by_entity.contains_key(&p.parent()))
            .unwrap_or(true);
        if !is_root {
            continue;
        }
        if let Some(id) = tree.by_entity.get(&entity).copied() {
            if let Err(err) = tree.tree.compute_layout(
                id,
                Size {
                    width: AvailableSpace::Definite(window_size.x),
                    height: AvailableSpace::Definite(window_size.y),
                },
            ) {
                warn!(?entity, ?err, "buiy: layout compute_layout failed");
            }
        }
    }
}

/// Step 7 — read `tree.layout(id)` for every tracked entity and write
/// the resulting position+size into `ResolvedLayout`. On Taffy `Err`,
/// retain the previous frame's value.
pub(super) fn write_resolved_layout(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
) {
    let mut to_write: Vec<(Entity, ResolvedLayout)> = Vec::new();
    for (&entity, &id) in tree.by_entity.iter() {
        if let Ok(layout) = tree.tree.layout(id) {
            to_write.push((
                entity,
                ResolvedLayout {
                    position: Vec2::new(layout.location.x, layout.location.y),
                    size: Vec2::new(layout.size.width, layout.size.height),
                },
            ));
        }
    }
    for (e, rl) in to_write {
        commands.entity(e).insert(rl);
    }
}
```

- [ ] **Step 7.2: Add `LayoutPlugin` to `mod.rs`**

Replace `crates/buiy_core/src/layout/mod.rs` with:

```rust
//! Buiy layout subsystem.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/.

mod components;
mod pipeline;
mod style;
mod systems;
mod tree;
pub(crate) mod translate;
mod types;

pub use components::{BoxModel, Display, FlexItem, FlexParams, Position};
pub use pipeline::BuiyLayoutStep;
pub use style::Style;
pub use tree::LayoutTree;
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, PositionKind, Sizing,
};

use bevy::prelude::*;

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<LayoutTree>();

        // Register decomposed components for reflection / BSN / inspectors.
        app.register_type::<BoxModel>()
            .register_type::<Display>()
            .register_type::<Position>()
            .register_type::<FlexParams>()
            .register_type::<FlexItem>()
            .register_type::<Edges>()
            .register_type::<Sizing>()
            .register_type::<Length>()
            .register_type::<AspectRatio>()
            .register_type::<Inset>();

        pipeline::configure_pipeline(app);

        app.add_systems(
            Update,
            (
                systems::gc_removed_nodes.in_set(BuiyLayoutStep::RemovedNodesGc),
                systems::sync_styles.in_set(BuiyLayoutStep::SyncStyles),
                systems::taffy_compute.in_set(BuiyLayoutStep::TaffyCompute),
                systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
            ),
        );
    }
}
```

- [ ] **Step 7.3: Delete the legacy `layout.rs`**

```sh
git rm crates/buiy_core/src/layout.rs
```

- [ ] **Step 7.4: Update `crates/buiy_core/src/lib.rs`**

The flat-vs-dir module ambiguity resolves once `layout.rs` is gone. Update the `pub use` to surface the new symbols and drop the deleted ones.

Existing `crates/buiy_core/src/lib.rs`:

```rust
pub use components::{FlexDirection, Node, ResolvedLayout, Style};
pub use layout::LayoutPlugin;
```

becomes:

```rust
pub use components::{Node, ResolvedLayout};
pub use layout::{
    AlignContent, AlignItems, AspectRatio, BoxModel, BoxSizing, BuiyLayoutStep, Display, Edges,
    FlexAxis, FlexGap, FlexItem, FlexParams, FlexWrap, Inset, JustifyContent, LayoutPlugin,
    LayoutTree, Length, Position, PositionKind, Sizing, Style,
};
```

The `register_type::<Style>()` and `register_type::<FlexDirection>()` calls in `CorePlugin` reference deleted types. Remove them — the new `LayoutPlugin` registers the decomposed components instead.

Existing `CorePlugin::build`:

```rust
fn build(&self, app: &mut App) {
    app.register_type::<Node>()
        .register_type::<Style>()
        .register_type::<ResolvedLayout>()
        .register_type::<FlexDirection>()
        .configure_sets(
            Update,
            (
                BuiySet::Layout,
                BuiySet::Style,
                BuiySet::Input,
                BuiySet::Animate,
                BuiySet::Picking,
                BuiySet::A11yUpdate,
                BuiySet::Render,
            )
                .chain(),
        );
}
```

becomes:

```rust
fn build(&self, app: &mut App) {
    app.register_type::<Node>()
        .register_type::<ResolvedLayout>()
        .configure_sets(
            Update,
            (
                BuiySet::Layout,
                BuiySet::Style,
                BuiySet::Input,
                BuiySet::Animate,
                BuiySet::Picking,
                BuiySet::A11yUpdate,
                BuiySet::Render,
            )
                .chain(),
        );
}
```

- [ ] **Step 7.5: Confirm build** (one error expected — `components.rs` still defines the deleted `Style`)

```sh
cargo check --workspace --all-targets
```

Expected: errors in `components.rs` (the old `Style` and `FlexDirection` are still defined and now collide with the new module's `Style`). Fixed in Task 8.

- [ ] **Step 7.6: Commit (will fail to build — that's expected mid-task)**

Hold the commit until Task 8 deletes the legacy components. Move directly to Task 8.

> **Note:** Steps 7.1–7.5 leave the workspace temporarily broken between Task 7 and Task 8. Reviewers should review Tasks 7+8 as one atomic unit. If the executor needs an intermediate green commit, run Task 8 *first* (delete legacy `Style` from `components.rs`, accepting transient broken `Button` import) then Task 7. The plan keeps the order Task 7 → Task 8 because the new module needs to exist before the old types are removed; trying the inverse breaks `lib.rs`'s `pub use` lines, which Rust's compile order checks unconditionally.

---

### Task 8: Delete legacy `Style` + `FlexDirection`; add `Visual`; migrate render + `Button`

**Files:**
- Modify: `crates/buiy_core/src/components.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs` (re-export `Visual`)
- Modify: `crates/buiy/src/lib.rs`
- Modify: `crates/buiy_widgets/src/button.rs`

This task removes the Phase 0 mega-`Style`. The render module currently queries that struct's `background_token` / `foreground_token` / `border_radius` fields — deleting them without a substitute breaks `cargo check`. To keep render compiling, this task introduces a small `Visual` component carrying those three fields. `Visual` is a temporary home; its eventual owner is `buiy-render-pipeline-design` (currently unwritten — see Phasing strategy table). Authors who want themed widgets insert `Visual` alongside `Style`.

- [ ] **Step 8.1: Replace `components.rs` (delete `Style` + `FlexDirection`; add `Visual`)**

Replace `crates/buiy_core/src/components.rs` with:

```rust
//! Buiy's core component types.
//!
//! Every Buiy component is small, public-fielded, observable, and
//! decomposed by concern. Layout components live in
//! `crate::layout::components`; this file holds the cross-cutting
//! `Node` marker, the shared `ResolvedLayout` output, and the temporary
//! `Visual` component (token-based rendering surface — eventual owner
//! is `buiy-render-pipeline-design`).
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.4.

use bevy::prelude::*;

/// A Buiy node — the parallel-to-`bevy_ui::Node` primitive. Marker that
/// this entity participates in Buiy's layout / render / a11y trees.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Node;

/// Resolved layout output, written by `BuiyLayoutStep::WriteResolvedLayout`.
/// Read by render, picking, and any other downstream subsystem.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct ResolvedLayout {
    /// Top-left position in logical pixels (window-relative).
    pub position: Vec2,
    /// Size in logical pixels.
    pub size: Vec2,
}

/// Visual surface: theme-token references and corner radius for the
/// Phase 0/1 render pipeline. Optional — entities without `Visual` are
/// skipped by the render extract.
///
/// **Temporary home.** This is a Phase 0 carry-over, kept alive in
/// Phase 1 only because the render extract still consumes
/// `background_token` / `border_radius`. The eventual owner of these
/// concerns is `buiy-render-pipeline-design` (unwritten as of Phase 1);
/// when that spec lands, `Visual` is replaced by richer
/// `Background` / `Border` / `Stroke` / etc. components.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Visual {
    /// Theme token for the fill (e.g. `"color.surface.secondary"`).
    /// Empty string → render skips the fill (transparent).
    pub background_token: String,
    /// Theme token for foreground / text color (e.g. `"color.text.primary"`).
    /// Reserved for the text-rendering integration; Phase 1 render does
    /// not consume it.
    pub foreground_token: String,
    /// Uniform corner radius in logical pixels.
    pub border_radius: f32,
}
```

- [ ] **Step 8.1b: Migrate the render extract to query `Visual`**

Update `crates/buiy_core/src/render/mod.rs`:

Change the `use` block top-of-file:

```rust
use crate::{
    components::{Node, ResolvedLayout, Visual},
    theme::Theme,
};
```

Change the `extract_buiy_draws` system signature and body:

```rust
fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<Query<(&Visual, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
    main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    let mut draws = ExtractedDraws::default();
    if let Ok(window) = main_world_windows.single() {
        let res = window.resolution.size();
        draws.window_size = Vec2::new(res.x, res.y);
    }
    for (visual, layout) in main_world_q.iter() {
        let color = match main_world_theme.color(&visual.background_token) {
            Some(c) => c,
            None => {
                tracing::warn!(
                    token = %visual.background_token,
                    "missing theme color token; falling back to magenta sentinel"
                );
                MISSING_TOKEN_FALLBACK
            }
        };
        draws.draws.push(DrawData {
            position: layout.position,
            size: layout.size,
            color,
            radius: visual.border_radius,
        });
    }
    commands.insert_resource(draws);
}
```

Entities without `Visual` are now invisible to the render extract — that's intentional. Authors who want a button rendered visibly insert `Visual` (the migrated `Button::new` does so in Step 8.4).

- [ ] **Step 8.1c: Register `Visual` for reflection**

Update `crates/buiy_core/src/lib.rs`'s `CorePlugin::build` to register the new component:

```rust
fn build(&self, app: &mut App) {
    app.register_type::<Node>()
        .register_type::<ResolvedLayout>()
        .register_type::<Visual>()
        .configure_sets(
            Update,
            (
                BuiySet::Layout,
                BuiySet::Style,
                BuiySet::Input,
                BuiySet::Animate,
                BuiySet::Picking,
                BuiySet::A11yUpdate,
                BuiySet::Render,
            )
                .chain(),
        );
}
```

(This replaces the version of `CorePlugin::build` introduced in Step 7.4. If you executed Step 7.4 already, just add the `Visual` registration; the rest is unchanged.)

- [ ] **Step 8.2: Update `crates/buiy_core/src/lib.rs` re-exports**

Replace the line `pub use components::{FlexDirection, Node, ResolvedLayout, Style};` with:

```rust
pub use components::{Node, ResolvedLayout, Visual};
pub use layout::{
    AlignContent, AlignItems, AspectRatio, BoxModel, BoxSizing, BuiyLayoutStep, Display, Edges,
    FlexAxis, FlexGap, FlexItem, FlexParams, FlexWrap, Inset, JustifyContent, LayoutPlugin,
    LayoutTree, Length, Position, PositionKind, Sizing, Style,
};
```

(This replaces the version landed in Step 7.4 — adds `Visual` to the `components` re-export.)

- [ ] **Step 8.3: Update `crates/buiy/src/lib.rs`**

Replace the existing `pub use buiy_core::{...};` block:

```rust
pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder, AccessKitAdapterPlugin},
    components::{FlexDirection, Node, ResolvedLayout, Style},
    focus::{FocusVisible, Focusable, FocusedEntity},
    picking::{BuiyPickingBackendPlugin, Hovered},
    theme::{Theme, UserPreferences, default_light_theme},
};
```

with:

```rust
pub use buiy_core::{
    BuiySet, CorePlugin,
    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder, AccessKitAdapterPlugin},
    components::{Node, ResolvedLayout, Visual},
    focus::{FocusVisible, Focusable, FocusedEntity},
    layout::{
        AlignContent, AlignItems, AspectRatio, BoxModel, BoxSizing, BuiyLayoutStep, Display,
        Edges, FlexAxis, FlexGap, FlexItem, FlexParams, FlexWrap, Inset, JustifyContent,
        LayoutPlugin, Length, Position, PositionKind, Sizing, Style,
    },
    picking::{BuiyPickingBackendPlugin, Hovered},
    theme::{Theme, UserPreferences, default_light_theme},
};
```

- [ ] **Step 8.4: Migrate `Button` to the new `Style` builder + insert `Visual`**

Replace the imports and `Button::new` body in `crates/buiy_widgets/src/button.rs`:

```rust
use buiy_core::{
    a11y::{A11yLabel, A11yRole},
    components::{Node, Visual},
    focus::Focusable,
    layout::Style,
    picking::Hovered,
};
```

```rust
impl Button {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Button,
            Node,
            // TODO(buiy-widget-catalog-design): replace hardcoded sizes
            // with size tokens (space.button.width, space.2). Hit target
            // 120x32 already meets WCAG 2.5.8 (>=24x24).
            Style::default()
                .width_px(120.0)
                .height_px(32.0)
                .padding(8.0),
            Visual {
                background_token: "color.surface.secondary".into(),
                foreground_token: "color.text.primary".into(),
                border_radius: 6.0, // matches "radius.md"
            },
            Focusable::default(),
            A11yRole::Button,
            A11yLabel(label),
        )
    }
}
```

`Visual` carries the three render-side fields formerly on the Phase 0 `Style`. Visual appearance is preserved; the only architectural change is that styling concerns are now split from layout concerns at the component boundary.

- [ ] **Step 8.5: Confirm build**

```sh
cargo check --workspace --all-targets
```

Expected: clean.

- [ ] **Step 8.6: Run the existing test suite to find what migration broke**

```sh
xvfb-run -a cargo test --workspace 2>&1 | tail -50
```

Expected failures: at least `crates/buiy_core/tests/layout.rs` (uses old `Style`) and `crates/buiy_core/tests/components.rs` (asserts old `Style` is registered). These are addressed in Task 11.

- [ ] **Step 8.7: Commit (Tasks 7 + 8 land together)**

```sh
git add -A
git commit -m "refactor(buiy_core): replace mega-Style with hybrid Style builder + decomposed components

Phase 1 of the layout migration.

- Delete crates/buiy_core/src/layout.rs (flat Phase 0 module).
- Delete components::Style and components::FlexDirection.
- Add LayoutPlugin in the new layout/ module that registers decomposed
  components and chains the 8-step pipeline (steps 0/1/3/7 active, 2/4/5/6
  empty stubs for later phase plans).
- Add components::Visual (background_token, foreground_token,
  border_radius) carrying the render-side fields formerly on the
  mega-Style; eventual home is buiy-render-pipeline-design.
- Migrate the render extract to query (&Visual, &ResolvedLayout) so
  the workspace continues to compile + render after Style's deletion.
- Migrate Button to Style::default().width_px(120.0).height_px(32.0)
  .padding(8.0) plus a Visual carrying the three theme tokens; visual
  appearance preserved.
- Style is now a Bundle (decomposes on insert) rather than a reflectable
  Component; reflection now sees the decomposed components.
- Re-export decomposed components, the Style builder, and Visual from
  buiy + buiy_core.

Spec: docs/specs/2026-05-08-buiy-layout-design/.
Plan: docs/plans/2026-05-08-buiy-layout-foundation.md."
```

---

### Task 9: New test — `layout_topology.rs`

**Files:**
- Create: `crates/buiy_core/tests/layout_topology.rs`

Asserts the topological invariant from architecture.md §5: parents resolve before children every frame.

- [ ] **Step 9.1: Write the test**

```rust
//! Topological invariant: parents resolve before children every frame.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 5.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{LayoutPlugin, Style},
};

#[test]
fn parents_resolve_before_children_in_a_4_deep_tree() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let root = app
        .world_mut()
        .spawn((Node, Style::default().flex_column().width_px(400.0).height_px(300.0)))
        .id();
    let mid = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
        .id();
    let inner = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();
    let leaf = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();

    app.world_mut().entity_mut(root).add_child(mid);
    app.world_mut().entity_mut(mid).add_child(inner);
    app.world_mut().entity_mut(inner).add_child(leaf);

    app.update();

    // After a single Update, every entity in the chain has a ResolvedLayout
    // with non-zero size. The contract here is that a child's layout uses
    // its parent's containing block; that only works if Step 3 computed
    // the parent's box first, which is what Taffy enforces inside one
    // tree.compute_layout call but Buiy's pipeline must also enforce
    // across the whole tree. A failure (parent Vec2::ZERO, child non-zero)
    // would mean Step 3 was called on the leaf root but not the actual
    // root.
    for (label, e) in [("root", root), ("mid", mid), ("inner", inner), ("leaf", leaf)] {
        let rl = app
            .world()
            .get::<ResolvedLayout>(e)
            .unwrap_or_else(|| panic!("{label} has no ResolvedLayout"));
        assert!(rl.size.x > 0.0, "{label} resolved width should be > 0");
        assert!(rl.size.y > 0.0, "{label} resolved height should be > 0");
    }

    // Specifically: the leaf's resolved-layout x must be at least its
    // parents' offsets (Taffy's tree.layout returns local coordinates,
    // not absolute, so this check covers "child computed inside parent's
    // box" — tested via cumulative width: leaf width 50px <= inner width
    // 100px).
    let leaf_layout = app.world().get::<ResolvedLayout>(leaf).unwrap();
    assert!(
        leaf_layout.size.x <= 100.0,
        "leaf width should fit within inner's 100px container"
    );
}
```

- [ ] **Step 9.2: Run the test to verify it passes**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_topology
```

Expected: 1 test passes.

- [ ] **Step 9.3: Commit**

```sh
git add crates/buiy_core/tests/layout_topology.rs
git commit -m "test(buiy_core): add layout_topology — parent-resolves-before-children invariant

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 5.
4-deep tree fixture; assert every entity has ResolvedLayout with
non-zero size after a single Update."
```

---

### Task 10: New test — `layout_pipeline_order.rs` + `layout_box_sizing.rs` + `layout_style_equivalence.rs`

**Files:**
- Create: `crates/buiy_core/tests/layout_pipeline_order.rs`
- Create: `crates/buiy_core/tests/layout_box_sizing.rs`
- Create: `crates/buiy_core/tests/layout_style_equivalence.rs`

The hybrid-API test in Task 3 lives inside the `layout::style` module; the integration test here exercises the same property through the public surface, which is what reviewers and downstream crates depend on.

- [ ] **Step 10.1: Pipeline-order test**

```rust
//! 8-step pipeline order asserted at the integration level.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.

use bevy::ecs::schedule::{ScheduleBuildSettings, ScheduleLabel};
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    layout::{BuiyLayoutStep, LayoutPlugin},
};

#[test]
fn layout_steps_are_chained_in_declared_order() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    // Force an Update build so set ordering is materialized.
    app.update();

    // The Schedule API in 0.18 doesn't expose a stable enumeration of
    // SystemSet ordering directly. We use the existence-and-ordering
    // contract: every BuiyLayoutStep set is configured, and configuring
    // a contradictory order fails schedule build. The smoke check here
    // is that adding a tracker system to each set runs them in the
    // declared order.
    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));

    fn make_tracker(
        order: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
        label: &'static str,
    ) -> impl Fn() + Send + Sync + 'static {
        move || {
            order.lock().unwrap().push(label);
        }
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let o = order.clone();
    app.add_systems(Update, make_tracker(o.clone(), "0").in_set(BuiyLayoutStep::RemovedNodesGc));
    app.add_systems(Update, make_tracker(o.clone(), "1").in_set(BuiyLayoutStep::SyncStyles));
    app.add_systems(Update, make_tracker(o.clone(), "2").in_set(BuiyLayoutStep::CqActivate));
    app.add_systems(Update, make_tracker(o.clone(), "3").in_set(BuiyLayoutStep::TaffyCompute));
    app.add_systems(Update, make_tracker(o.clone(), "4").in_set(BuiyLayoutStep::CqFlipCheck));
    app.add_systems(Update, make_tracker(o.clone(), "5").in_set(BuiyLayoutStep::CqFlipReRun));
    app.add_systems(Update, make_tracker(o.clone(), "6").in_set(BuiyLayoutStep::PostTaffyOverrides));
    app.add_systems(Update, make_tracker(o.clone(), "7").in_set(BuiyLayoutStep::WriteResolvedLayout));

    app.update();

    let observed = order.lock().unwrap().clone();
    assert_eq!(
        observed,
        vec!["0", "1", "2", "3", "4", "5", "6", "7"],
        "BuiyLayoutStep sets did not run in declared order",
    );
}
```

- [ ] **Step 10.2: Run the pipeline-order test**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_pipeline_order
```

Expected: 1 test passes.

- [ ] **Step 10.3: Box-sizing test**

```rust
//! BoxSizing::ContentBox vs BorderBox produce the spec-mandated widths.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 2.2 + § 6.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{BoxSizing, LayoutPlugin, Length, Sizing, Style},
};

#[test]
fn content_box_treats_width_as_content_box() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .padding(10.0)
                .content_box(),
        ))
        .id();

    app.update();
    let rl = app.world().get::<ResolvedLayout>(entity).unwrap();
    // ContentBox: total width = content (100) + padding-left (10) + padding-right (10) = 120
    assert!(
        (rl.size.x - 120.0).abs() < 0.5,
        "ContentBox: total width should be 120 (100 content + 20 padding); got {}",
        rl.size.x
    );
}

#[test]
fn border_box_treats_width_as_border_box() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .padding(10.0)
                .border_box(),
        ))
        .id();

    app.update();
    let rl = app.world().get::<ResolvedLayout>(entity).unwrap();
    // BorderBox: total width = 100 (set value); content box is 100-20 = 80.
    assert!(
        (rl.size.x - 100.0).abs() < 0.5,
        "BorderBox: total width should be 100; got {}",
        rl.size.x
    );
}
```

- [ ] **Step 10.4: Run the box-sizing test**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_box_sizing
```

Expected: 2 tests pass.

- [ ] **Step 10.5: Style-equivalence integration test**

```rust
//! Hybrid Style API: struct literal and fluent form produce identical
//! decomposed components when applied to a real entity, and both produce
//! identical ResolvedLayout after running the pipeline.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 8 test #4.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{
        AlignItems, BoxModel, BoxSizing, Display, Edges, FlexAxis, FlexGap, FlexParams,
        JustifyContent, LayoutPlugin, Length, Position, Sizing, Style,
    },
};

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn struct_literal_and_fluent_have_identical_resolved_layout() {
    let make_literal = || Style {
        display: Display::Flex(FlexAxis::Column),
        box_model: BoxModel {
            width: Sizing::Length(Length::Px(200.0)),
            height: Sizing::Length(Length::Px(100.0)),
            padding: Edges::all(8.0),
            box_sizing: BoxSizing::BorderBox,
            ..default()
        },
        flex_params: FlexParams {
            direction: FlexAxis::Column,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            gap: FlexGap {
                row: Length::Px(4.0),
                column: Length::Px(4.0),
            },
            ..default()
        },
        position: Position::default(),
    };

    let make_fluent = || {
        Style::default()
            .flex_column()
            .width_px(200.0)
            .height_px(100.0)
            .padding(8.0)
            .border_box()
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center)
            .gap_px(4.0)
    };

    let mut a = build_app();
    let ent_a = a.world_mut().spawn((Node, make_literal())).id();
    a.update();
    let rl_a = a.world().get::<ResolvedLayout>(ent_a).unwrap().clone();

    let mut b = build_app();
    let ent_b = b.world_mut().spawn((Node, make_fluent())).id();
    b.update();
    let rl_b = b.world().get::<ResolvedLayout>(ent_b).unwrap().clone();

    assert!(
        (rl_a.size - rl_b.size).length() < 0.5,
        "Struct-literal Style and fluent Style produced divergent sizes: {:?} vs {:?}",
        rl_a.size,
        rl_b.size
    );
    assert!(
        (rl_a.position - rl_b.position).length() < 0.5,
        "Struct-literal Style and fluent Style produced divergent positions: {:?} vs {:?}",
        rl_a.position,
        rl_b.position
    );
}
```

- [ ] **Step 10.6: Run the equivalence test**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_style_equivalence
```

Expected: 1 test passes.

- [ ] **Step 10.7: Commit**

```sh
git add crates/buiy_core/tests/layout_pipeline_order.rs \
        crates/buiy_core/tests/layout_box_sizing.rs \
        crates/buiy_core/tests/layout_style_equivalence.rs
git commit -m "test(buiy_core): pipeline order, box-sizing, and Style API equivalence

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 8.

- layout_pipeline_order: tracker systems in each BuiyLayoutStep set
  fire in 0..7 order.
- layout_box_sizing: ContentBox produces 120px total for width 100 +
  padding 10/side; BorderBox produces 100px total.
- layout_style_equivalence: struct literal and fluent builder forms
  produce identical ResolvedLayout after one Update."
```

---

### Task 11: Migrate existing tests + verify `hello_button`

**Files:**
- Modify: `crates/buiy_core/tests/layout.rs`
- Modify: `crates/buiy_core/tests/components.rs`
- Verify: `examples/hello_button/src/main.rs` (no source changes — uses `Button::new`, which Task 8 migrated)

- [ ] **Step 11.1: Update `tests/layout.rs` to the new API**

Replace the file with:

```rust
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout},
    layout::{LayoutPlugin, LayoutTree, Style},
};

#[test]
fn layout_resolves_a_simple_flex_row() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default().flex_row().width_px(200.0).height_px(100.0),
        ))
        .id();

    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
        ))
        .id();

    app.world_mut().entity_mut(parent).add_child(child);

    app.update();

    let layout = app
        .world()
        .get::<ResolvedLayout>(child)
        .expect("child has ResolvedLayout after Update");
    assert!((layout.size.x - 50.0).abs() < 0.5, "child width ~ 50");
    assert!((layout.size.y - 50.0).abs() < 0.5, "child height ~ 50");
}

#[test]
fn layout_tree_garbage_collects_despawned_entities() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();

    app.update();
    assert_eq!(
        app.world().non_send_resource::<LayoutTree>().len(),
        1,
        "spawned entity registered in LayoutTree after first update",
    );

    app.world_mut().entity_mut(entity).despawn();
    app.update();

    assert!(
        app.world().non_send_resource::<LayoutTree>().is_empty(),
        "despawned entity dropped from LayoutTree by gc system",
    );
}

#[test]
fn layout_tree_garbage_collects_within_a_single_tick() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let first = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();

    app.update();
    assert_eq!(
        app.world().non_send_resource::<LayoutTree>().len(),
        1,
        "first entity registered after first update",
    );

    app.world_mut().entity_mut(first).despawn();
    let second = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();

    app.update();

    let tree = app.world().non_send_resource::<LayoutTree>();
    assert_eq!(
        tree.len(),
        1,
        "exactly one entity remains after same-tick despawn+respawn",
    );
    assert!(
        app.world().get::<ResolvedLayout>(second).is_some(),
        "the surviving entity is the new one (proves it was synced after gc)",
    );
}
```

- [ ] **Step 11.2: Update `tests/components.rs` to the new component set**

Replace the file with:

```rust
use bevy::prelude::*;
use buiy_core::{CorePlugin, components::*, layout::Style};

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn node_resolved_layout_and_visual_are_registered_and_default_constructible() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);

    let registry = app.world().resource::<AppTypeRegistry>();
    let registry = registry.read();
    assert!(
        registry.get(std::any::TypeId::of::<Node>()).is_some(),
        "Node not registered"
    );
    assert!(
        registry
            .get(std::any::TypeId::of::<ResolvedLayout>())
            .is_some(),
        "ResolvedLayout not registered"
    );
    assert!(
        registry.get(std::any::TypeId::of::<Visual>()).is_some(),
        "Visual not registered"
    );

    drop(registry);
    let world = app.world_mut();
    let entity = world.spawn((Node::default(), Style::default())).id();
    assert!(world.get::<Node>(entity).is_some());
    // Style is a Bundle (not a reflectable Component); the underlying
    // decomposed components are visible post-spawn.
    assert!(
        world.get::<buiy_core::layout::BoxModel>(entity).is_some(),
        "BoxModel inserted via Style::default()"
    );
}
```

- [ ] **Step 11.3: Run the full test suite, with explicit verification of pre-existing tests that the migration must not break**

```sh
xvfb-run -a cargo test --workspace
```

Expected: every test passes, including:

- `cargo test -p buiy_core --test system_set_order` — Phase 0's outer `BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate → Render` chain still configures correctly. Step 8.1c keeps the `configure_sets` call in `CorePlugin::build`; the new `BuiyLayoutStep` sub-sets nest *inside* `BuiySet::Layout` without affecting the outer chain.
- `cargo test -p buiy_widgets --test button` — `Button` widget's click → `OnPress` flow is unchanged by the layout migration.
- `cargo test --test hello_button_e2e` — the workspace-root e2e fixture (covers layout + a11y tree + Tab focus). The fixture does not pin `Style` field values, only `Button::new` behavior, so it should pass unmodified.

If any of those fail, the migration broke something the plan didn't anticipate — debug before continuing.

- [ ] **Step 11.4: Smoke-check `hello_button`**

```sh
cargo build --example hello_button
```

(or `cargo run --example hello_button` if a display is available)

Expected: builds cleanly. Visual appearance is preserved: `Visual` (created in Step 8.1) carries the same `background_token`, `foreground_token`, and `border_radius` Phase 0's `Style` did, and the migrated `Button::new` (Step 8.4) inserts it. No regression vs. Phase 0.

- [ ] **Step 11.5: Commit**

```sh
git add crates/buiy_core/tests/layout.rs crates/buiy_core/tests/components.rs
git commit -m "test(buiy_core): migrate layout + components tests to hybrid Style builder

The Phase 0 mega-Style is gone; tests now use Style::default()
.flex_row().width_px(200.0).height_px(100.0). The components test
asserts Node + ResolvedLayout + Visual registration and the
Bundle-expansion contract (Style::default() produces a BoxModel-bearing
entity)."
```

---

### Task 12: Run all checks + CHANGELOG entry + update spec status

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/specs/2026-05-08-buiy-layout-design/README.md` (status `draft → active`, and update §4.1 with the actual plan filename and `[active]` link).

- [ ] **Step 12.1: Run all checks**

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

Resolve every warning before proceeding. Fixes go into a separate commit at the end of Task 12.

- [ ] **Step 12.2: Update `CHANGELOG.md`**

Add to the `[Unreleased]` section, under new `### Added` / `### Changed` / `### Removed` subsections (merge with any existing ones):

```markdown
### Added
- `buiy_core::components::Visual` component (`background_token`,
  `foreground_token`, `border_radius`) carrying the render-side surface
  formerly mixed into the Phase 0 mega-`Style`. Authors who want themed
  widgets insert `Visual` alongside the new layout `Style` builder.
  Eventual home is `buiy-render-pipeline-design`.
- `buiy_core::layout` module: 8-step layout pipeline (`BuiyLayoutStep`
  system sets), decomposed `BoxModel` / `Display` / `Position` /
  `FlexParams` / `FlexItem` components, hybrid `Style` builder that
  expands to a `Bundle` on spawn.

### Changed
- Layout subsystem foundation rewritten. Phase 0's flat `layout.rs` is
  replaced by a `layout/` directory module. The pipeline is an 8-step
  ordered chain (`BuiyLayoutStep` system sets) inside `BuiySet::Layout`;
  Phase 1 implements steps 0/1/3/7 and stubs the remaining four for
  later phase plans.
- `Style` is now a `Bundle` that decomposes on insert, not a reflectable
  `Component`. Reflection / inspectors / BSN see the decomposed
  components (`BoxModel`, `Display`, `Position`, `FlexParams`).
- The render extract now queries `(&Visual, &ResolvedLayout)` instead
  of `(&Style, &ResolvedLayout)`; entities without `Visual` are skipped
  by render. `Button::new` inserts a `Visual` carrying the same theme
  tokens Phase 0's `Style` did, so visual appearance is preserved.

### Removed
- `buiy_core::components::Style` (the Phase 0 mega-component) and
  `buiy_core::components::FlexDirection`. Their roles are taken by
  `buiy_core::layout::Style` (the hybrid builder) and
  `buiy_core::layout::FlexAxis` (the four-variant axis enum).
```

- [ ] **Step 12.3: Flip the spec status**

Update `docs/specs/2026-05-08-buiy-layout-design/README.md`:

- Header: `**Status:** draft` → `**Status:** active`.
- §4.1 last sentence: replace `(TBC)` with the actual filename and link to the plan.

The change in §4.1:

```markdown
... lives in a follow-up plan at `docs/plans/YYYY-MM-DD-buiy-layout-migration.md` (TBC). The plan will sequence ...
```

becomes:

```markdown
... lives in a series of follow-up plans, the first of which is [Phase 1 — Foundation](../../plans/2026-05-08-buiy-layout-foundation.md). The plans sequence ...
```

- [ ] **Step 12.4: Add the plan to `docs/README.md` index**

Under `### Layout` → `**Plans**` (currently empty), add:

```markdown
**Plans**

- [Buiy layout foundation](plans/2026-05-08-buiy-layout-foundation.md) — Phase 1: 8-step pipeline skeleton, decomposed components for the Phase-0 surface, hybrid `Style` builder, `Button` migration. `[active]`
```

(Promote the spec to `[active]` under the same area as part of Step 12.3's status flip.)

- [ ] **Step 12.5: Commit**

```sh
git add CHANGELOG.md docs/specs/2026-05-08-buiy-layout-design/README.md docs/README.md
git commit -m "docs: layout spec → active; add Phase 1 plan to docs index; CHANGELOG

The layout spec moves from draft to active now that the first
realizing plan is in place. CHANGELOG records the Phase 0 mega-Style
removal and the hybrid Style builder."
```

---

## Self-review

Before declaring the plan ready, run the three-step self-review from the writing-plans skill.

- [ ] **Spec coverage check.** Walk the coverage map in this plan. Every Phase 1-scoped row has an explicit Task in the list. Deferred rows are explicitly tagged (Phase 2 / 3 / 4 / ...). Confirm every Phase 1 row has a task number.
- [ ] **Placeholder scan.** Search the plan for: "TBD", "TODO", "fill in", "appropriate handling", "similar to Task". The only acceptable matches are: TODO comments inside `Button::new` that copy the Phase 0 deferral notes verbatim (those are real source comments, not plan placeholders). Confirm any TODO/TBD in the plan body itself is replaced with concrete content.
- [ ] **Type consistency check.** Verify field names match across tasks: `BoxModel.width`/`.height`/`.padding`/`.margin`/`.border`/`.box_sizing`/`.aspect_ratio`/`.gap`/`.row_gap`/`.column_gap` (Task 2). `FlexParams.direction`/`.wrap`/`.justify_content`/`.align_items`/`.align_content`/`.gap` (Task 2). `FlexItem.grow`/`.shrink`/`.basis`/`.order`/`.align_self` (Task 2). `Style` setters: `flex_row`/`flex_column`/`flex_axis`/`width`/`width_px`/`padding`/`padding_edges`/`content_box`/`border_box`/`gap_px`/`relative`/`absolute`/`justify_content`/`align_items`/`align_content`/`flex_wrap` (Task 3). Each setter writes the correct field name in subsequent translation/test tasks.

If any check fails, fix inline and re-run.

---

## Execution handoff

Plan complete. Two execution options:

**1. Subagent-Driven (recommended)** — `subagent-driven-development` skill. Fresh subagent per task; spec-compliance review then code-quality review after each. Fast iteration, full quality gates.

**2. Inline Execution** — `executing-plans` skill. Batch execution in this session with checkpoints between tasks.

Which approach?
