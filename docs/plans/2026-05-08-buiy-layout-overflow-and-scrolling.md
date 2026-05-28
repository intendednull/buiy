# Buiy layout overflow and scrolling — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Date:** 2026-05-08
**Status:** landed
**Spec:** [`specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md`](../specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md) (with cross-references to [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md))

**Goal:** Phase 2 of the layout migration — ship the overflow / scrolling component set: `Overflow` (per-axis overflow mode + scrollbar / scroll-behavior / overscroll settings), `Scroll` (snap container), `ScrollOffset` (runtime scroll position), `ScrollSnapItem` (per-snap-item child-side); wire `Overflow` and `ScrollbarWidth` into Taffy 0.10's `Style.overflow: Point<Overflow>` and `Style.scrollbar_width: f32`; widen `sync_styles`'s change-detection trigger set to include `Changed<Overflow>` / `Changed<Scroll>` while *excluding* `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` so scroll-position mutations never invalidate Taffy.

**Architecture:** Adds 4 components and 9 supporting enum types under the existing Phase 1 `crates/buiy_core/src/layout/` directory; no new module files. `Overflow` and `Scroll` are container-side self-styling and join `Style`'s `Bundle` (always-insert pattern from Phase 1). `ScrollOffset` (runtime state) and `ScrollSnapItem` (child-side, like `FlexItem`) are decomposed-only per [architecture.md § 2.4](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only). The translation layer extends `StyleView` with `&Overflow` and maps `OverflowMode::{Visible, Hidden, Clip, Scroll, Auto}` to `taffy::Overflow::{Visible, Hidden, Hidden, Scroll, Scroll}` (per [overflow-and-scrolling.md § 1.1](../specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md#11-mapping-to-taffy)) and `ScrollbarWidth::{Auto, Thin, None}` to `taffy::Style.scrollbar_width: f32` (12.0 / 8.0 / 0.0). `sync_styles`'s `Or<(Changed<...>)>` filter widens to include the two container-side components but neither runtime nor child-side ones; this is asserted by a test that mutates `ScrollOffset` and verifies the trigger query yields zero entities.

**Tech Stack:** Rust, Bevy 0.18 ECS + reflection + `Bundle`, Taffy 0.10. No new external dependencies.

---

## Phasing strategy reference

This is **Phase 2** of the migration kicked off by the [Phase 1 plan](2026-05-08-buiy-layout-foundation.md#phasing-strategy-this-plan-vs-follow-ups). The phasing-strategy table there lists Phase 2 as `*-buiy-layout-overflow-and-scrolling.md`. Subsequent phases consume Phase 2 surface but don't reorder it:

- Phase 7 (`*-buiy-layout-sticky-table-multicol.md`) — sub-pass 6a `StickyOffset` *reads* `ScrollOffset` and the spec's "scroll container" predicate (any axis is `Scroll` or `Auto`) to compute sticky displacement.
- Phase 4 (`*-buiy-layout-writing-modes.md`) — adds `Overflow` logical aliases (`overflow-block`, `overflow-inline`) and a translation pass that maps them to `x` / `y` based on `WritingModeResolved`. Phase 2 ships physical `x` / `y` only.
- `buiy-input-events-design` (sibling spec) — owns the actual scroll handler that mutates `ScrollOffset` in response to wheel / touch / keyboard input, and the snap-point math that consumes `Scroll` + `ScrollSnapItem`.
- `buiy-render-pipeline-design` (sibling spec) — consumes `Overflow.scrollbar_color` / `scrollbar_width` (visual style) and the per-axis clip rect derived from `OverflowMode::{Hidden, Clip, Scroll, Auto}`.

Phase 2 stores the data those consumers need; it does not implement them.

---

## File structure

### Modified files

```
crates/buiy_core/src/layout/
├── types.rs          — append 9 enums: OverflowMode, ScrollbarGutter,
│                       ScrollbarWidth, ScrollbarColor, ScrollBehavior,
│                       OverscrollBehavior, SnapType, SnapAlign, SnapStop  (Task 1)
├── components.rs     — add Overflow + Scroll (author-set styling)         (Task 2)
│                       add ScrollOffset + ScrollSnapItem (decomposed-only) (Task 3)
├── style.rs          — extend Style Bundle with `overflow` + `scroll`
│                       fields; add fluent setters                          (Task 4)
├── translate.rs      — extend StyleView with `&Overflow`; add
│                       map_overflow_mode + map_scrollbar_width helpers;
│                       wire taffy.overflow + taffy.scrollbar_width         (Task 5)
├── systems.rs        — widen sync_styles query: add `&Overflow`; widen
│                       trigger filter: Or now includes Changed<Overflow>
│                       and Changed<Scroll>                                  (Task 5)
└── mod.rs            — register Overflow, Scroll, ScrollOffset,
                        ScrollSnapItem with reflection; re-export new
                        component types and 9 enums                          (Task 6)
```

```
crates/buiy_core/src/lib.rs    — re-export new types from buiy_core         (Task 6)
crates/buiy/src/lib.rs         — re-export from buiy facade                  (Task 6)
```

### New tests

```
crates/buiy_core/tests/
├── layout_overflow.rs                     — Taffy mapping: every OverflowMode
│                                            variant produces the right
│                                            taffy::Overflow value;
│                                            ScrollbarWidth maps to f32;
│                                            scroll-container predicate     (Task 7)
└── layout_scroll_offset_no_invalidate.rs  — invariant: mutating
                                             ScrollOffset (or ScrollSnapItem)
                                             does NOT trigger sync_styles    (Task 8)
```

### Modified docs / non-code files

- `CHANGELOG.md` — `[Unreleased]` `### Added` and `### Changed` entries (Task 9).
- `docs/README.md` — entry already added under Layout > Plans with `[active]` tag during the plan-write commit; flipped to `[landed]` post-merge.

### No deletions

Phase 2 is purely additive — no Phase 1 file or item is removed.

---

## Coverage map

Every Phase 2 spec requirement maps to a task below. Items marked **deferred** are explicitly out of Phase 2 (deferred to a later phase or sibling spec); the table notes the deferral target.

| Spec section | Phase 2 coverage | Task |
|---|---|---|
| overflow-and-scrolling.md § 1 — `Overflow` component shape | `Overflow { x, y, scrollbar_gutter, scrollbar_width, scrollbar_color, scroll_behavior, overscroll_x, overscroll_y }` ships in full. | 2 |
| overflow-and-scrolling.md § 1 — `OverflowMode` enum (Visible / Hidden / Clip / Scroll / Auto) | Ships in full. | 1 |
| overflow-and-scrolling.md § 1 — `ScrollbarGutter` (Auto / Stable / StableBothEdges) | Stored. **`Stable` does not yet reserve gutter on non-scrolling containers** — Taffy 0.10 only reserves `scrollbar_width` when overflow is `Scroll` (or our `Auto`). The padding-override for "always reserve" lands in a follow-up phase that owns `Overflow` rendering. The non-scrolling gutter test in spec § 5 is deferred. | 1, 5 |
| overflow-and-scrolling.md § 1 — `ScrollbarWidth` (Auto / Thin / None) | Stored and mapped to `taffy::Style.scrollbar_width: f32` via `Auto → 12.0`, `Thin → 8.0`, `None → 0.0`. | 1, 5 |
| overflow-and-scrolling.md § 1 — `ScrollbarColor` (Auto / Custom { thumb, track }) | Stored. **Render-side concern; no Taffy mapping.** | 1 |
| overflow-and-scrolling.md § 1 — logical aliases (`overflow-block` / `overflow-inline`) | **Deferred to Phase 4 (writing-modes).** Phase 2 ships `x` / `y` only. | — |
| overflow-and-scrolling.md § 1.1 — Taffy mapping table | Implemented per spec: `Visible → Visible`, `Hidden → Hidden`, `Clip → Hidden` (Taffy 0.10 has a distinct `Clip` variant whose sizing-aware semantics differ from CSS; the spec's CSS-faithful mapping prevails — Phase 2 plan does not deviate), `Scroll → Scroll`, `Auto → Scroll`. | 5, 8 |
| overflow-and-scrolling.md § 1.2 — scroll container = either axis is `Scroll` or `Auto` | Predicate exposed as `Overflow::is_scroll_container(&self) -> bool`. **Sticky-positioning consumer is Phase 7.** | 2 |
| overflow-and-scrolling.md § 2 — `ScrollOffset` component | Ships as `ScrollOffset { x, y }`. **Author/input-system mutates; layout reads in Phase 7 (sticky) and step 7 picking.** | 3 |
| overflow-and-scrolling.md § 2.1 — `ScrollOffset` mutation does not invalidate `ResolvedLayout` | Invariant enforced by **excluding** `Changed<ScrollOffset>` from `sync_styles`'s trigger filter. Asserted by `tests/layout_scroll_offset_no_invalidate.rs`. | 6, 9 |
| overflow-and-scrolling.md § 2.2 — `ScrollBehavior` (Auto / Smooth) | Stored on `Overflow.scroll_behavior`. **Smooth-scroll interpolation lives in `BuiySet::Animate`; layout doesn't act on it.** | 1 |
| overflow-and-scrolling.md § 2.3 — `OverscrollBehavior` (Auto / Contain / None) | Stored on `Overflow.overscroll_x` / `overscroll_y`. **Honored by `buiy-input-events-design`'s scroll handler.** | 1 |
| overflow-and-scrolling.md § 3 — `Scroll` component (snap_type / snap_padding / snap_margin) | Ships in full as decomposed component; included in `Style` builder. | 2, 4 |
| overflow-and-scrolling.md § 3 — `ScrollSnapItem` component (align / stop) | Ships in full as decomposed-only child-side component (not in `Style` Bundle). | 3 |
| overflow-and-scrolling.md § 3 — `SnapType` / `SnapAlign` / `SnapStop` enums | Ship in full. | 1 |
| overflow-and-scrolling.md § 3 — snap point resolution | **Deferred to `buiy-input-events-design`.** Layout stores; input resolves. | — |
| overflow-and-scrolling.md § 4 — `ScrollbarColor` styling | Stored. **Render-side concern.** | 1 |
| overflow-and-scrolling.md § 5 test — `Visible` doesn't clip | Asserted by `tests/layout_overflow.rs` fixture: parent 100×100 with 200×100 child; child's `ResolvedLayout` extends beyond parent. | 8 |
| overflow-and-scrolling.md § 5 test — `Hidden` clips (Taffy stores) | Asserted by `tests/layout_overflow.rs`: same fixture with `OverflowMode::Hidden`; assert Taffy `Style.overflow.x == taffy::Overflow::Hidden`. (Render-side clip rect verification deferred per spec § 5 note.) | 8 |
| overflow-and-scrolling.md § 5 test — scroll container detection | `Overflow::is_scroll_container()` returns `true` when either axis is `Scroll` or `Auto`; asserted by `tests/layout_overflow.rs`. | 8 |
| overflow-and-scrolling.md § 5 test — `Stable` gutter on non-scrolling container reserves space | **Deferred** — see `ScrollbarGutter` row above. | — |
| overflow-and-scrolling.md § 5 test — scroll offset doesn't invalidate layout | Asserted by `tests/layout_scroll_offset_no_invalidate.rs`. | 9 |
| overflow-and-scrolling.md § 5 test — logical alias translation | **Deferred to Phase 4.** | — |
| overflow-and-scrolling.md § 6 — virtual scrolling | **Out of scope (widget catalog).** | — |
| overflow-and-scrolling.md § 7 — scroll-driven animations | **Deferred to `buiy-animation-design`.** | — |
| architecture.md § 1.2 — sync_styles trigger set widening | Add `Changed<Overflow>`, `Changed<Scroll>`. **Do NOT add `Changed<ScrollOffset>` or `Changed<ScrollSnapItem>`.** Verified by Task 8 test. | 5, 8 |
| architecture.md § 2.1 — decomposed component derive convention (`Component, Reflect, Default, Clone` + `#[reflect(Component, Default)]`) | Applied to all four new components. | 2, 3 |
| architecture.md § 2.4 — child-side components are decomposed-only | `ScrollSnapItem` follows `FlexItem`'s pattern: not in `Style` builder. `ScrollOffset` is *runtime state*, similarly excluded from the builder (and from the change-detection filter). | 3, 4 |

---

## Task list

10 tasks, each ending with a commit. Phase 1's branch model continues: feature branch `claude/v01-layout-overflow-and-scrolling`, one PR.

The TDD discipline carries forward: write failing test → run to confirm failure → minimal implementation → run to confirm pass → commit.

---

### Task 1: Add 9 enum types to `layout/types.rs`

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (append after line 204, the existing `Inset` struct, before the `#[cfg(test)] mod tests` block)

This task introduces every supporting enum the four Phase 2 components will reference. Adding them first means Tasks 2–3 can build components on top without forward references. The new enums are referenced from Task 1's own tests until Task 2 lands the components — no `#[allow(dead_code)]` is needed because every enum is `pub` and reachable from the existing `pub mod types;` in `layout/mod.rs` line 11.

- [ ] **Step 1.1: Write failing tests for the 9 enum defaults**

Append the following inside the existing `mod tests` block in `crates/buiy_core/src/layout/types.rs` (i.e., between lines 247 and 249, before the closing `}` at line 249):

```rust
    #[test]
    fn overflow_mode_default_is_visible() {
        assert_eq!(OverflowMode::default(), OverflowMode::Visible);
    }

    #[test]
    fn scrollbar_gutter_default_is_auto() {
        assert_eq!(ScrollbarGutter::default(), ScrollbarGutter::Auto);
    }

    #[test]
    fn scrollbar_width_default_is_auto() {
        assert_eq!(ScrollbarWidth::default(), ScrollbarWidth::Auto);
    }

    #[test]
    fn scrollbar_color_default_is_auto() {
        assert_eq!(ScrollbarColor::default(), ScrollbarColor::Auto);
    }

    #[test]
    fn scroll_behavior_default_is_auto() {
        assert_eq!(ScrollBehavior::default(), ScrollBehavior::Auto);
    }

    #[test]
    fn overscroll_behavior_default_is_auto() {
        assert_eq!(OverscrollBehavior::default(), OverscrollBehavior::Auto);
    }

    #[test]
    fn snap_type_default_is_none() {
        assert_eq!(SnapType::default(), SnapType::None);
    }

    #[test]
    fn snap_align_default_is_none() {
        assert_eq!(SnapAlign::default(), SnapAlign::None);
    }

    #[test]
    fn snap_stop_default_is_normal() {
        assert_eq!(SnapStop::default(), SnapStop::Normal);
    }
```

- [ ] **Step 1.2: Run the tests to verify they fail**

```sh
cargo test -p buiy_core --lib layout::types
```

Expected: compilation error — none of the 9 enums exist yet (`error[E0433]: failed to resolve: use of undeclared type 'OverflowMode'` etc.).

- [ ] **Step 1.3: Implement the 9 enum types**

In `crates/buiy_core/src/layout/types.rs`, append the following block immediately before the existing `#[cfg(test)] mod tests {` line (currently line 206). Each enum derives `Reflect, Default, Clone, Copy, Debug, PartialEq` plus `Eq` where the variant payloads support it (every enum here is payload-free except `ScrollbarColor::Custom`, which carries `bevy::color::Color` — `Color` is not `Eq`, so `ScrollbarColor` derives `PartialEq` only).

```rust
/// Per-axis overflow handling. CSS `overflow`.
///
/// `Visible` (default) lets children render outside the box. `Hidden` and
/// `Clip` clip without scrolling; the difference is render-side (per spec
/// § 1.1, both map to `taffy::Overflow::Hidden`). `Scroll` always shows a
/// scrollbar; `Auto` shows one only when content overflows. Layout
/// treats `Scroll` and `Auto` identically (both produce a scroll
/// container per § 1.2); the distinction is rendering's.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowMode {
    #[default]
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

/// CSS `scrollbar-gutter`. `Stable` reserves space even when not
/// scrolling; `StableBothEdges` reserves on both inline edges (useful
/// for centering). Phase 2 stores the value but does not yet enforce
/// `Stable` on non-scrolling containers — see plan coverage map.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollbarGutter {
    #[default]
    Auto,
    Stable,
    StableBothEdges,
}

/// CSS `scrollbar-width`. Drives `taffy::Style.scrollbar_width`:
/// `Auto → 12.0`, `Thin → 8.0`, `None → 0.0`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollbarWidth {
    #[default]
    Auto,
    Thin,
    None,
}

/// CSS `scrollbar-color`. Render-side concern; layout stores only.
///
/// `Color` derives `Reflect` but not `Eq` (it contains `f32`), so this
/// enum derives `PartialEq` only — matching the rest of the file's
/// convention for types that contain floats.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarColor {
    #[default]
    Auto,
    Custom {
        thumb: Color,
        track: Color,
    },
}

/// CSS `scroll-behavior`. Honored by `BuiySet::Animate` for programmatic
/// scrolls; layout doesn't act on it.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Smooth,
}

/// CSS `overscroll-behavior`, per axis. Honored by
/// `buiy-input-events-design`'s scroll handler; layout stores only.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverscrollBehavior {
    #[default]
    Auto,
    Contain,
    None,
}

/// CSS `scroll-snap-type`. `*Mandatory` means snap is required after
/// scroll ends; `*Proximity` only snaps when close enough.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapType {
    #[default]
    None,
    XMandatory,
    XProximity,
    YMandatory,
    YProximity,
    BothMandatory,
    BothProximity,
}

/// CSS `scroll-snap-align`. Per-item alignment to the snap viewport.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapAlign {
    #[default]
    None,
    Start,
    End,
    Center,
}

/// CSS `scroll-snap-stop`. `Always` forces snap to land on this item
/// even if a fast fling would skip past.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapStop {
    #[default]
    Normal,
    Always,
}
```

- [ ] **Step 1.4: Run the tests to verify they pass**

```sh
cargo test -p buiy_core --lib layout::types
```

Expected: all 13 tests pass (4 pre-existing + 9 new).

- [ ] **Step 1.5: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy -p buiy_core --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 1.6: Commit**

```sh
git add crates/buiy_core/src/layout/types.rs
git commit -m "$(cat <<'EOF'
feat(buiy_core): add overflow / scrolling enum types

Adds OverflowMode, ScrollbarGutter, ScrollbarWidth, ScrollbarColor,
ScrollBehavior, OverscrollBehavior, SnapType, SnapAlign, SnapStop —
the supporting type surface for Phase 2 components in the next tasks.

Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 1
EOF
)"
```

---

### Task 2: Add `Overflow` and `Scroll` author-set styling components

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (append after the existing `FlexItem` block, before the `#[cfg(test)] mod tests` block at line 137; also extend the `use super::types::{...}` import list at line 13)

`Overflow` and `Scroll` are container-side self-styling. Per the Phase 1 always-insert pattern, both join `Style`'s Bundle (Task 4) and ship default values that no-op (no clipping, no snap).

`Overflow::is_scroll_container(&self)` is added in this task because the predicate is part of the component's documented API per spec § 1.2; later phases (sticky, picking) consume it.

**Reflection-attribute convention:** every component in this task uses `#[reflect(Component, Default)]` (matching `BoxModel` / `Position` / `FlexParams` from Phase 1). Phase 1's `Display` enum uses `#[reflect(Component)]` without `Default` despite deriving `Default` (a Phase 1 oversight); do *not* propagate that omission to Phase 2 components. The `Default` reflect tag enables BSN / inspector defaulting; omitting it silently degrades inspector UX.

- [ ] **Step 2.1: Write failing tests for `Overflow` and `Scroll` defaults and `is_scroll_container`**

Append to the existing `mod tests` block in `crates/buiy_core/src/layout/components.rs`, right before its closing `}` (currently line 188). Note the existing imports at the top of the test module already cover `super::*`; add what's missing.

```rust
    #[test]
    fn overflow_default_is_visible_both_axes() {
        let o = Overflow::default();
        assert_eq!(o.x, OverflowMode::Visible);
        assert_eq!(o.y, OverflowMode::Visible);
        assert_eq!(o.scrollbar_gutter, ScrollbarGutter::Auto);
        assert_eq!(o.scrollbar_width, ScrollbarWidth::Auto);
        assert_eq!(o.scrollbar_color, ScrollbarColor::Auto);
        assert_eq!(o.scroll_behavior, ScrollBehavior::Auto);
        assert_eq!(o.overscroll_x, OverscrollBehavior::Auto);
        assert_eq!(o.overscroll_y, OverscrollBehavior::Auto);
    }

    #[test]
    fn overflow_is_scroll_container_only_when_either_axis_scrolls() {
        assert!(!Overflow::default().is_scroll_container());
        assert!(
            !Overflow {
                x: OverflowMode::Hidden,
                y: OverflowMode::Hidden,
                ..Default::default()
            }
            .is_scroll_container()
        );
        assert!(
            Overflow {
                x: OverflowMode::Scroll,
                ..Default::default()
            }
            .is_scroll_container()
        );
        assert!(
            Overflow {
                y: OverflowMode::Auto,
                ..Default::default()
            }
            .is_scroll_container()
        );
        assert!(
            Overflow {
                x: OverflowMode::Auto,
                y: OverflowMode::Scroll,
                ..Default::default()
            }
            .is_scroll_container()
        );
    }

    #[test]
    fn scroll_default_is_no_snap_zero_padding() {
        let s = Scroll::default();
        assert_eq!(s.snap_type, SnapType::None);
        assert_eq!(s.snap_padding, Edges::ZERO);
        assert_eq!(s.snap_margin, Edges::ZERO);
    }
```

- [ ] **Step 2.2: Run tests to verify failure**

```sh
cargo test -p buiy_core --lib layout::components
```

Expected: compilation error — `Overflow`, `Scroll`, `OverflowMode`, etc. unresolved in `super::*` (the components module hasn't imported them yet).

- [ ] **Step 2.3: Extend the import list and add the components**

In `crates/buiy_core/src/layout/components.rs`, replace the existing `use super::types::{...}` import (currently line 13) with the widened list:

```rust
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, OverflowMode, OverscrollBehavior, PositionKind, ScrollBehavior, ScrollbarColor,
    ScrollbarGutter, ScrollbarWidth, Sizing, SnapType,
};
```

Then append the two components after the existing `FlexItem` impl (line 135, immediately before `#[cfg(test)]` at line 137). Match the existing component conventions — `Component, Reflect, Default, Clone, Debug, PartialEq` + `#[reflect(Component, Default)]`:

```rust
/// Per-axis overflow handling and scroll/scrollbar configuration. CSS
/// `overflow`, `scrollbar-*`, `scroll-behavior`, `overscroll-behavior`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1.
///
/// Phase 2 wires `x` / `y` and `scrollbar_width` to Taffy. The other
/// fields are stored for downstream consumers (render: `scrollbar_color`,
/// animate: `scroll_behavior`, input: `overscroll_*`). `scrollbar_gutter`
/// is stored but `Stable` does not yet reserve space on non-scrolling
/// containers — see plan coverage map.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Overflow {
    pub x: OverflowMode,
    pub y: OverflowMode,
    pub scrollbar_gutter: ScrollbarGutter,
    pub scrollbar_width: ScrollbarWidth,
    pub scrollbar_color: ScrollbarColor,
    pub scroll_behavior: ScrollBehavior,
    pub overscroll_x: OverscrollBehavior,
    pub overscroll_y: OverscrollBehavior,
}

impl Overflow {
    /// True iff either axis is `Scroll` or `Auto`. Scroll containers
    /// establish a scroll viewport and a containing block for descendants
    /// with `Position::Sticky` (consumer: Phase 7 sub-pass 6a).
    ///
    /// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1.2.
    pub fn is_scroll_container(&self) -> bool {
        matches!(self.x, OverflowMode::Scroll | OverflowMode::Auto)
            || matches!(self.y, OverflowMode::Scroll | OverflowMode::Auto)
    }
}

/// Scroll-snap container settings. CSS `scroll-snap-type`,
/// `scroll-padding`, `scroll-margin`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 3.
///
/// Phase 2 stores; the snap-point math runs in
/// `buiy-input-events-design`'s scroll handler.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Scroll {
    pub snap_type: SnapType,
    pub snap_padding: Edges,
    pub snap_margin: Edges,
}
```

- [ ] **Step 2.4: Run tests to verify pass**

```sh
cargo test -p buiy_core --lib layout::components
```

Expected: all tests pass (4 pre-existing + 3 new).

- [ ] **Step 2.5: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy -p buiy_core --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 2.6: Commit**

```sh
git add crates/buiy_core/src/layout/components.rs
git commit -m "$(cat <<'EOF'
feat(buiy_core): add Overflow + Scroll author-set components

Both ship with default = no-op (Visible/Visible, no snap). Overflow
exposes is_scroll_container() — the spec § 1.2 predicate for
sticky-positioning containing-block resolution (Phase 7).

Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 2
EOF
)"
```

---

### Task 3: Add `ScrollOffset` and `ScrollSnapItem` decomposed-only components

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (append after the `Scroll` component added in Task 2, before the `#[cfg(test)]` block; also extend the `use super::types::{...}` import to include `SnapAlign` and `SnapStop`)

`ScrollOffset` is *runtime state* — the input system writes to it in response to scroll events; layout reads it (Phase 7 sub-pass 6a) and rendering / picking apply it during draw / hit-test. **It is not in `Style`'s Bundle, and `Changed<ScrollOffset>` is intentionally excluded from `sync_styles`'s trigger filter** (Task 5). Mutating `ScrollOffset` must not invalidate Taffy.

`ScrollSnapItem` is *child-side* — analogous to `FlexItem`, it's spawned alongside `Style` rather than nested. Per [architecture.md § 2.4](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only), child-side components stay decomposed-only.

- [ ] **Step 3.1: Write failing tests for `ScrollOffset` and `ScrollSnapItem`**

Append to the existing `mod tests` block in `crates/buiy_core/src/layout/components.rs`, right before its closing `}`:

```rust
    #[test]
    fn scroll_offset_default_is_origin() {
        let s = ScrollOffset::default();
        assert_eq!(s.x, 0.0);
        assert_eq!(s.y, 0.0);
    }

    #[test]
    fn scroll_snap_item_default_is_none_normal() {
        let s = ScrollSnapItem::default();
        assert_eq!(s.align, SnapAlign::None);
        assert_eq!(s.stop, SnapStop::Normal);
    }
```

- [ ] **Step 3.2: Run tests to verify failure**

```sh
cargo test -p buiy_core --lib layout::components
```

Expected: compilation error — `ScrollOffset` / `ScrollSnapItem` / `SnapAlign` / `SnapStop` unresolved.

- [ ] **Step 3.3: Extend the import list and add the components**

In `crates/buiy_core/src/layout/components.rs`, replace the import line (the one that ends with `..., Sizing, SnapType,`) with the further-widened list:

```rust
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, OverflowMode, OverscrollBehavior, PositionKind, ScrollBehavior, ScrollbarColor,
    ScrollbarGutter, ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType,
};
```

Then append the two components after the `Scroll` block from Task 2:

```rust
/// Runtime scroll position of a scroll container. Mutated by the
/// scroll-input handler in `buiy-input-events-design`. Read by render
/// (drawing) and picking (hit-testing) at consume time, and by Phase 7
/// sub-pass 6a (`StickyOffset`).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 2.
///
/// **Mutating `ScrollOffset` must NOT invalidate `ResolvedLayout`.** The
/// invariant is enforced by excluding `Changed<ScrollOffset>` from
/// `sync_styles`'s trigger filter (see `systems.rs` line ~80) and
/// asserted by `tests/layout_scroll_offset_no_invalidate.rs`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollOffset {
    pub x: f32,
    pub y: f32,
}

/// Per-snap-item child-side configuration. Lives on each child of a
/// scroll container that participates in snap.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 3.
///
/// Decomposed-only — not in `Style`'s Bundle. Following the `FlexItem`
/// pattern: spawn alongside `Style` rather than nested. The snap-point
/// math runs in `buiy-input-events-design`'s scroll handler at consume
/// time; this component stores the per-item declaration.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollSnapItem {
    pub align: SnapAlign,
    pub stop: SnapStop,
}
```

- [ ] **Step 3.4: Run tests to verify pass**

```sh
cargo test -p buiy_core --lib layout::components
```

Expected: all tests pass (7 pre-existing + 2 new).

- [ ] **Step 3.5: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy -p buiy_core --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 3.6: Commit**

```sh
git add crates/buiy_core/src/layout/components.rs
git commit -m "$(cat <<'EOF'
feat(buiy_core): add ScrollOffset + ScrollSnapItem decomposed components

ScrollOffset is runtime state — mutating it must not invalidate
ResolvedLayout. ScrollSnapItem is child-side, like FlexItem; both stay
decomposed-only (not in Style Bundle). Their exclusion from sync_styles'
change-detection filter is enforced in Task 5.

Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 3
EOF
)"
```

---

### Task 4: Extend `Style` Bundle with `overflow` and `scroll` fields + fluent setters

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` (extend the `Style` struct definition, the `use` block at the top, and add new fluent methods inside the existing `impl Style`; also extend the test module)

Adds two fields and ten fluent methods. The struct-literal-≡-fluent equivalence test from Phase 1 carries forward — extended to cover the new fields.

`ScrollOffset` and `ScrollSnapItem` are intentionally NOT added — `ScrollOffset` is runtime state (initialized via direct insert) and `ScrollSnapItem` is child-side per spec § 2.4.

- [ ] **Step 4.1: Write failing test extending the equivalence test**

Open `crates/buiy_core/src/layout/style.rs`. The existing `struct_literal_and_fluent_produce_identical_components` test at line 260 currently spawns `(Display, BoxModel, Position, FlexParams)` and compares. Replace its function body and the `spawn_and_extract` helper to also extract `Overflow` and `Scroll`. Replace the helper at line 239 with:

```rust
    fn spawn_and_extract(
        style: Style,
    ) -> (Display, BoxModel, Position, FlexParams, Overflow, Scroll) {
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
        (display, box_model, position, flex_params, overflow, scroll)
    }
```

Replace the existing `struct_literal_and_fluent_produce_identical_components` test body (currently lines 260-293) with:

```rust
    #[test]
    fn struct_literal_and_fluent_produce_identical_components() {
        let literal = Style {
            display: Display::Flex(FlexAxis::Column),
            box_model: BoxModel {
                padding: Edges::all(16.0),
                box_sizing: BoxSizing::BorderBox,
                width: Sizing::Length(Length::Px(200.0)),
                height: Sizing::Length(Length::Px(100.0)),
                ..Default::default()
            },
            flex_params: FlexParams {
                direction: FlexAxis::Column,
                gap: FlexGap {
                    row: Length::Px(8.0),
                    column: Length::Px(8.0),
                },
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            overflow: Overflow {
                x: OverflowMode::Hidden,
                y: OverflowMode::Auto,
                scrollbar_width: ScrollbarWidth::Thin,
                ..Default::default()
            },
            scroll: Scroll {
                snap_type: SnapType::YMandatory,
                snap_padding: Edges::all(4.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let fluent = Style::default()
            .flex_column()
            .padding(16.0)
            .border_box()
            .width_px(200.0)
            .height_px(100.0)
            .gap_px(8.0)
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center)
            .overflow_x(OverflowMode::Hidden)
            .overflow_y(OverflowMode::Auto)
            .scrollbar_width(ScrollbarWidth::Thin)
            .snap_type(SnapType::YMandatory)
            .snap_padding(Edges::all(4.0));

        assert_eq!(spawn_and_extract(literal), spawn_and_extract(fluent));
    }
```

Extend `default_style_inserts_every_decomposed_component` (currently lines 295-305) to assert `Overflow` and `Scroll` insertion:

```rust
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
        assert!(world.get::<Overflow>(entity).is_some());
        assert!(world.get::<Scroll>(entity).is_some());
    }
```

Then update the test imports — the existing `use crate::layout::components::{BoxModel, Display, FlexParams, Position};` (line 232) and `use crate::layout::types::{ AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, JustifyContent, Length, Sizing, };` (lines 233-235) need widening:

```rust
    use super::*;
    use crate::layout::components::{BoxModel, Display, FlexParams, Overflow, Position, Scroll};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, JustifyContent, Length, OverflowMode,
        ScrollbarWidth, Sizing, SnapType,
    };
```

- [ ] **Step 4.2: Run tests to verify failure**

```sh
cargo test -p buiy_core --lib layout::style
```

Expected: compilation error — `overflow` / `scroll` fields don't exist on `Style`; fluent methods (`overflow_x`, `overflow_y`, `scrollbar_width`, `snap_type`, `snap_padding`) don't exist.

- [ ] **Step 4.3: Extend the `Style` struct and add fluent methods**

In `crates/buiy_core/src/layout/style.rs`, update the top `use` block (currently lines 15-19) to add `Overflow` and `Scroll` from components and the new types:

```rust
use super::components::{BoxModel, Display, FlexParams, Overflow, Position, Scroll};
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, OverflowMode, PositionKind, ScrollBehavior, ScrollbarGutter,
    ScrollbarWidth, Sizing, SnapType,
};
```

Update the doc comment on `Style` at lines 22-36 to mention the now-six components — replace the existing comment with:

```rust
/// Hybrid builder over an entity's self-styling layout components.
///
/// Two authoring forms over the same fields:
///
/// ```ignore
/// // Struct-literal form.
/// let s = Style { display: Display::flex_row(), ..Default::default() };
///
/// // Fluent form.
/// let s = Style::default().flex_row();
/// ```
///
/// On `commands.spawn(s)` (or `entity.insert(s)`), expands into a Bundle
/// of `Display`, `BoxModel`, `Position`, `FlexParams`, `Overflow`,
/// `Scroll`. Decomposed components are canonical; the builder is sugar.
/// `ScrollOffset` (runtime state) and `ScrollSnapItem` (child-side) are
/// NOT in this Bundle — spawn them alongside `Style` per
/// `architecture.md § 2.4`.
```

Extend the struct definition (currently lines 37-43) to add `overflow` and `scroll` fields:

```rust
#[derive(Bundle, Clone, Debug, Default)]
pub struct Style {
    pub display: Display,
    pub box_model: BoxModel,
    pub position: Position,
    pub flex_params: FlexParams,
    pub overflow: Overflow,
    pub scroll: Scroll,
}
```

Add the following fluent methods at the end of `impl Style { ... }` (i.e., immediately before its closing `}` at line 227):

```rust
    // ---- Overflow ----

    pub fn overflow_x(mut self, mode: OverflowMode) -> Self {
        self.overflow.x = mode;
        self
    }

    pub fn overflow_y(mut self, mode: OverflowMode) -> Self {
        self.overflow.y = mode;
        self
    }

    pub fn overflow(mut self, x: OverflowMode, y: OverflowMode) -> Self {
        self.overflow.x = x;
        self.overflow.y = y;
        self
    }

    pub fn overflow_hidden(self) -> Self {
        self.overflow(OverflowMode::Hidden, OverflowMode::Hidden)
    }

    pub fn overflow_y_scroll(self) -> Self {
        self.overflow_y(OverflowMode::Scroll)
    }

    pub fn overflow_x_scroll(self) -> Self {
        self.overflow_x(OverflowMode::Scroll)
    }

    pub fn scrollbar_gutter(mut self, g: ScrollbarGutter) -> Self {
        self.overflow.scrollbar_gutter = g;
        self
    }

    pub fn scrollbar_width(mut self, w: ScrollbarWidth) -> Self {
        self.overflow.scrollbar_width = w;
        self
    }

    pub fn scroll_behavior(mut self, b: ScrollBehavior) -> Self {
        self.overflow.scroll_behavior = b;
        self
    }

    // ---- Scroll snap ----

    pub fn snap_type(mut self, t: SnapType) -> Self {
        self.scroll.snap_type = t;
        self
    }

    pub fn snap_padding(mut self, e: Edges) -> Self {
        self.scroll.snap_padding = e;
        self
    }

    pub fn snap_margin(mut self, e: Edges) -> Self {
        self.scroll.snap_margin = e;
        self
    }
```

- [ ] **Step 4.4: Run tests to verify pass**

```sh
cargo test -p buiy_core --lib layout::style
```

Expected: all tests pass.

- [ ] **Step 4.5: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy -p buiy_core --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 4.6: Commit**

```sh
git add crates/buiy_core/src/layout/style.rs
git commit -m "$(cat <<'EOF'
feat(buiy_core): extend Style Bundle with Overflow + Scroll

Adds container-side overflow and scroll-snap fields plus 12 fluent
setters. ScrollOffset (runtime) and ScrollSnapItem (child-side) stay
out of the Bundle per architecture.md § 2.4.

Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.2-2.4
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 4
EOF
)"
```

---

### Task 5: Wire `Overflow` to Taffy and widen `sync_styles` (atomic)

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` (extend `StyleView` struct, extend `style_to_taffy`, add `map_overflow_mode` and `map_scrollbar_width` helpers, extend tests)
- Modify: `crates/buiy_core/src/layout/systems.rs` (extend imports, doc comment, query, filter, destructuring)

**Atomic.** `StyleView` is the bridge between `sync_styles`'s query and `style_to_taffy`. Widening one without the other breaks the lib build (the call site in `systems.rs` would still construct the old shape). This task combines both edits in one commit so the workspace compiles continuously.

Per spec § 1.1, `OverflowMode → taffy::Overflow`: `Visible→Visible, Hidden→Hidden, Clip→Hidden, Scroll→Scroll, Auto→Scroll`. The plan picks `ScrollbarWidth::{Auto, Thin, None}` → `taffy::Style.scrollbar_width: f32` of `12.0 / 8.0 / 0.0` — values chosen to approximate common platform scrollbar widths (macOS overlay ~15 px, GTK ~12 px, Windows ~17 px; Thin matches typical "reduce-scrollbar-width" overlay widths). The spec is silent on exact pixel values; revisit when `buiy-render-pipeline-design` settles on canonical widths.

`Scroll`, `ScrollOffset`, `ScrollSnapItem` have no Taffy mapping — they're consumed by render / input / Phase 7 systems. `Scroll` is added to `StyleView` (so `Changed<Scroll>` flows through the same pipeline) but `style_to_taffy` does not consume it. `ScrollOffset` and `ScrollSnapItem` are deliberately not in `StyleView` *or* the trigger filter.

This is the **central correctness step** of Phase 2. Adding `Changed<Overflow>` is required because `Overflow` affects `taffy::Style`. Adding `Changed<Scroll>` matches [architecture.md § 1.2](../specs/2026-05-08-buiy-layout-design/architecture.md#change-detection-trigger-set)'s documented trigger set — Phase 7 (sticky) reads `Scroll` and the spec scopes it as layout-relevant. **`Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` are deliberately excluded**; the exclusion is asserted by Task 8's test.

- [ ] **Step 5.1a: Migrate the four pre-existing `translate.rs` tests to the new `StyleView` shape**

Open `crates/buiy_core/src/layout/translate.rs`. Locate the four pre-existing tests (`translate_default_components_to_taffy_default`, `translate_flex_row_with_dimensions`, `translate_position_absolute_emits_absolute_with_inset`, `translate_flex_item_basis_grow_shrink`). Each currently calls:

```rust
let taffy = style_to_taffy(StyleView {
    display: &display,
    box_model: &bm,
    position: &position,
    flex_params: &flex,
    flex_item: ...,
});
```

For each of the four tests, **before** the `style_to_taffy` call, add the two component constructions:

```rust
let overflow = Overflow::default();
let scroll = Scroll::default();
```

Then replace the `StyleView { ... }` literal so it includes both new fields:

```rust
let taffy = style_to_taffy(StyleView {
    display: &display,
    box_model: &bm,
    position: &position,
    flex_params: &flex,
    flex_item: ...,
    overflow: &overflow,
    scroll: &scroll,
});
```

Then update the test module's imports. The existing test-module imports (lines 251-255) read:

```rust
    use crate::layout::components::{BoxModel, Display, FlexItem, FlexParams, Position};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, JustifyContent, Length,
        PositionKind, Sizing,
    };
```

Replace them with:

```rust
    use crate::layout::components::{BoxModel, Display, FlexItem, FlexParams, Overflow, Position, Scroll};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, JustifyContent, Length,
        OverflowMode, PositionKind, ScrollbarWidth, Sizing,
    };
```

- [ ] **Step 5.1b: Append the two new failing tests to the test module**

Append before the test module's closing `}` at line 368:

```rust
    #[test]
    fn translate_overflow_modes_to_taffy() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let cases: &[(OverflowMode, OverflowMode, taffy::Overflow, taffy::Overflow)] = &[
            (
                OverflowMode::Visible,
                OverflowMode::Visible,
                taffy::Overflow::Visible,
                taffy::Overflow::Visible,
            ),
            (
                OverflowMode::Hidden,
                OverflowMode::Hidden,
                taffy::Overflow::Hidden,
                taffy::Overflow::Hidden,
            ),
            (
                OverflowMode::Clip,
                OverflowMode::Clip,
                taffy::Overflow::Hidden,
                taffy::Overflow::Hidden,
            ),
            (
                OverflowMode::Scroll,
                OverflowMode::Auto,
                taffy::Overflow::Scroll,
                taffy::Overflow::Scroll,
            ),
            (
                OverflowMode::Auto,
                OverflowMode::Visible,
                taffy::Overflow::Scroll,
                taffy::Overflow::Visible,
            ),
        ];
        for (x_in, y_in, x_expected, y_expected) in cases.iter().copied() {
            let overflow = Overflow {
                x: x_in,
                y: y_in,
                ..Default::default()
            };
            let scroll = Scroll::default();
            let taffy = style_to_taffy(StyleView {
                display: &display,
                box_model: &bm,
                position: &position,
                flex_params: &flex,
                flex_item: None,
                overflow: &overflow,
                scroll: &scroll,
            });
            assert_eq!(
                taffy.overflow.x, x_expected,
                "x {x_in:?} → expected {x_expected:?}"
            );
            assert_eq!(
                taffy.overflow.y, y_expected,
                "y {y_in:?} → expected {y_expected:?}"
            );
        }
    }

    #[test]
    fn translate_scrollbar_width_to_taffy_f32() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let scroll = Scroll::default();
        for (input, expected) in [
            (ScrollbarWidth::Auto, 12.0_f32),
            (ScrollbarWidth::Thin, 8.0),
            (ScrollbarWidth::None, 0.0),
        ] {
            let overflow = Overflow {
                scrollbar_width: input,
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
            });
            assert_eq!(taffy.scrollbar_width, expected, "{input:?}");
        }
    }
```

- [ ] **Step 5.2: Run tests to verify failure**

```sh
cargo test -p buiy_core --lib layout
```

Expected: lib compilation error — `StyleView` has no `overflow` / `scroll` fields; the migrated test calls fail. The lib won't build, so all `--lib` tests fail. This is the failing-test red state for the combined translate.rs+systems.rs implementation in Step 5.3.

- [ ] **Step 5.3a: Extend `translate.rs` — imports, `StyleView`, `style_to_taffy`, helpers**

In `crates/buiy_core/src/layout/translate.rs`, replace the existing `StyleView` struct (currently lines 17-23) with:

```rust
/// View into the decomposed-component set for one entity. Built by
/// `sync_styles`'s query and passed to `style_to_taffy`.
pub struct StyleView<'a> {
    pub display: &'a Display,
    pub box_model: &'a BoxModel,
    pub position: &'a Position,
    pub flex_params: &'a FlexParams,
    pub flex_item: Option<&'a FlexItem>,
    pub overflow: &'a Overflow,
    pub scroll: &'a Scroll,
}
```

The existing `use super::components::...` import (line 9) needs widening:

```rust
use super::components::{BoxModel, Display, FlexItem, FlexParams, Overflow, Position, Scroll};
```

And the existing `use super::types::...` import (lines 10-13) needs widening:

```rust
use super::types::{
    AlignContent, AlignItems, BoxSizing, Edges, FlexAxis, FlexWrap, Inset, JustifyContent, Length,
    OverflowMode, PositionKind, ScrollbarWidth, Sizing,
};
```

Then extend `style_to_taffy`'s body. The existing function body (lines 25-76) constructs a `taffy::Style` literal then conditionally writes flex-item / aspect-ratio. Inside the existing `let mut s = taffy::Style { ... }` literal (lines 26-56), add two new fields right before the existing `..Default::default()` closing line (line 55):

```rust
        overflow: taffy::Point {
            x: map_overflow_mode(view.overflow.x),
            y: map_overflow_mode(view.overflow.y),
        },
        scrollbar_width: map_scrollbar_width(view.overflow.scrollbar_width),
```

(The `view.scroll` field is intentionally not consumed in this task — `Scroll`'s data flows to other systems, not Taffy. Reading `&view.scroll` is enough to keep the field reachable.)

Then add two helper functions immediately after `map_align_content` (currently ends at line 172, before `sizing_to_dim` at line 174):

```rust
fn map_overflow_mode(o: OverflowMode) -> taffy::Overflow {
    use OverflowMode::*;
    match o {
        Visible => taffy::Overflow::Visible,
        // Spec § 1.1 maps both Hidden and Clip to taffy::Hidden. Taffy 0.10
        // distinguishes Hidden (clips and reserves scrollbar gutter via
        // scrollbar_width) from Clip (clips with no gutter); the spec
        // chose CSS-faithful: both CSS Hidden and CSS Clip route through
        // taffy::Hidden so ScrollbarGutter::Stable can later reserve a
        // gutter consistently when the author opts in.
        Hidden | Clip => taffy::Overflow::Hidden,
        // Auto (conditional scrollbar) is a render-time distinction;
        // layout treats it as Scroll so children may exceed the box.
        Scroll | Auto => taffy::Overflow::Scroll,
    }
}

fn map_scrollbar_width(w: ScrollbarWidth) -> f32 {
    // Approximate common platform scrollbar widths. Auto = ~12 px (GTK /
    // overlay style), Thin = ~8 px (CSS `scrollbar-width: thin` typical
    // rendering), None = 0 px (no gutter reserved). Revisit when
    // buiy-render-pipeline-design picks canonical widths.
    match w {
        ScrollbarWidth::Auto => 12.0,
        ScrollbarWidth::Thin => 8.0,
        ScrollbarWidth::None => 0.0,
    }
}
```

- [ ] **Step 5.3b: Extend `systems.rs` — doc comment, imports, query, filter, destructuring**

In `crates/buiy_core/src/layout/systems.rs`, replace the existing `use super::components::{BoxModel, Display, FlexItem, FlexParams, Position};` (line 14) with:

```rust
use super::components::{BoxModel, Display, FlexItem, FlexParams, Overflow, Position, Scroll};
```

The existing doc comment on `sync_styles` (lines 48-61) lists the Phase 1 trigger set. Replace lines 56-61 (from `Phase 1 trigger set:` to `components land.`) with:

```rust
/// Phase 2 trigger set: `Changed<BoxModel>`, `Changed<Display>`,
/// `Changed<Position>`, `Changed<FlexParams>`, `Changed<FlexItem>`,
/// `Changed<Overflow>`, `Changed<Scroll>`, `Changed<Children>`,
/// `Changed<ChildOf>`. Phases 4–9 widen it as new components land.
///
/// **`Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` are
/// intentionally excluded.** `ScrollOffset` is runtime state (mutated
/// every scroll-input frame) and `ScrollSnapItem` is consumed by the
/// snap-point math in `buiy-input-events-design`, not by layout. Their
/// exclusion is asserted by `tests/layout_scroll_offset_no_invalidate.rs`.
```

Replace the existing `nodes: Query<...>` parameter (currently lines 65-87) with the widened query that adds `&Overflow` / `&Scroll` and the two new `Changed<...>` filter clauses:

```rust
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
                Changed<Overflow>,
                Changed<Scroll>,
                Changed<Children>,
                Changed<ChildOf>,
            )>,
        ),
    >,
```

The existing two `for` loops in `sync_styles` destructure 7 fields. Replace each loop's destructuring tuple to include the two new components.

The first loop (currently lines 95-123) — replace its `for ... in nodes.iter() { ... }` header at line 95 with:

```rust
    for (entity, display, bm, position, flex, flex_item, overflow, scroll, _children) in
        nodes.iter()
    {
```

Inside that loop, the existing `let view = StyleView { ... };` block (lines 96-102) needs the two new fields:

```rust
        let view = StyleView {
            display,
            box_model: bm,
            position,
            flex_params: flex,
            flex_item,
            overflow,
            scroll,
        };
```

The second loop (currently lines 126-139) — replace its `for ... in nodes.iter() { ... }` header at line 126 with:

```rust
    for (entity, _display, _bm, _position, _flex, _flex_item, _overflow, _scroll, children) in
        nodes.iter()
    {
```

(All the unused destructured fields are prefixed with `_`. The leading `_overflow` / `_scroll` are required because the query tuple ordering must match the parameter ordering exactly.)

- [ ] **Step 5.4: Run the full lib test suite to verify pass**

```sh
xvfb-run -a cargo test -p buiy_core
```

Expected: every test passes. The 6 translate tests (4 pre-existing migrated + 2 new) plus every other Phase 1 test (`layout_pipeline_order.rs`, `layout_topology.rs`, `layout_style_equivalence.rs`, `layout_box_sizing.rs`, `layout.rs`, `components.rs`) all green. The Phase 1 tests carry forward unchanged because Task 4 ensures every `Style::default()` spawn produces an entity with the full Phase-2-extended component set.

- [ ] **Step 5.5: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy -p buiy_core --all-targets -- -D warnings
```

Expected: clean. The `clippy::type_complexity` allow at `systems.rs` line 62 still covers the now-9-element query tuple.

- [ ] **Step 5.6: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/systems.rs
git commit -m "$(cat <<'EOF'
feat(buiy_core): wire Overflow to Taffy + widen sync_styles trigger set

Atomic. Extends StyleView with `overflow` + `scroll`; maps OverflowMode
{Visible, Hidden, Clip, Scroll, Auto} → taffy::Overflow {Visible,
Hidden, Hidden, Scroll, Scroll} per spec § 1.1; ScrollbarWidth
{Auto, Thin, None} → 12.0 / 8.0 / 0.0 (f32).

Widens sync_styles' Or<(Changed<...>)> filter to include
Changed<Overflow> + Changed<Scroll>, matching architecture.md § 1.2.
Changed<ScrollOffset> and Changed<ScrollSnapItem> are intentionally
excluded — runtime state and child-side respectively (Task 8 asserts).

Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1.1
Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 5
EOF
)"
```

---

### Task 6: Register types in `LayoutPlugin`; update workspace re-exports

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` (extend `pub use ...` lines and `register_type::<...>()` calls)
- Modify: `crates/buiy_core/src/lib.rs` (extend re-export block)
- Modify: `crates/buiy/src/lib.rs` (extend the facade re-export)

This task makes the new types reachable from the public API and visible to reflection / BSN / inspectors. No behavior change — type registration only.

- [ ] **Step 6.1: Extend re-exports and registration in `layout/mod.rs`**

Replace the existing `pub use components::{...}` line (line 13) with:

```rust
pub use components::{
    BoxModel, Display, FlexItem, FlexParams, Overflow, Position, Scroll, ScrollOffset,
    ScrollSnapItem,
};
```

Replace the existing `pub use types::{...}` block (lines 17-20) with:

```rust
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, Length, OverflowMode, OverscrollBehavior, PositionKind, ScrollBehavior,
    ScrollbarColor, ScrollbarGutter, ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType,
};
```

Inside `LayoutPlugin::build` (currently lines 27-53), the existing `register_type::<...>()` chain (lines 31-40) registers 10 types. Replace it with the widened chain that includes the four new components:

```rust
        // Register decomposed components for reflection / BSN / inspectors.
        app.register_type::<BoxModel>()
            .register_type::<Display>()
            .register_type::<Position>()
            .register_type::<FlexParams>()
            .register_type::<FlexItem>()
            .register_type::<Overflow>()
            .register_type::<Scroll>()
            .register_type::<ScrollOffset>()
            .register_type::<ScrollSnapItem>()
            .register_type::<Edges>()
            .register_type::<Sizing>()
            .register_type::<Length>()
            .register_type::<AspectRatio>()
            .register_type::<Inset>();
```

- [ ] **Step 6.2: Extend `crates/buiy_core/src/lib.rs` re-export block**

Open `crates/buiy_core/src/lib.rs`. Find the layout re-export block (search for `pub use layout::` — there should be a single `pub use layout::{...};` line that re-exports the layout surface for buiy_core consumers). Add the new types to it.

The exact line to modify is whatever `pub use layout::{...};` exists today after Phase 1. The Phase 1 export was:

```rust
pub use layout::{
    AlignContent, AlignItems, AspectRatio, BoxModel, BoxSizing, BuiyLayoutStep, Display, Edges,
    FlexAxis, FlexGap, FlexItem, FlexParams, FlexWrap, Inset, JustifyContent, LayoutPlugin,
    LayoutTree, Length, Position, PositionKind, Sizing, Style,
};
```

Replace it with the widened export:

```rust
pub use layout::{
    AlignContent, AlignItems, AspectRatio, BoxModel, BoxSizing, BuiyLayoutStep, Display, Edges,
    FlexAxis, FlexGap, FlexItem, FlexParams, FlexWrap, Inset, JustifyContent, LayoutPlugin,
    LayoutTree, Length, Overflow, OverflowMode, OverscrollBehavior, Position, PositionKind, Scroll,
    ScrollBehavior, ScrollOffset, ScrollSnapItem, ScrollbarColor, ScrollbarGutter, ScrollbarWidth,
    Sizing, SnapAlign, SnapStop, SnapType, Style,
};
```

If the actual export shape on disk differs (e.g., split into multiple `pub use` lines), match the existing convention — append the new symbols in the same style.

- [ ] **Step 6.3: Extend `crates/buiy/src/lib.rs` facade re-export**

Open `crates/buiy/src/lib.rs`. Find the layout re-exports surfaced through the `buiy` facade. Phase 1 added the types (BoxModel, Display, Position, FlexParams, FlexItem, Style, etc.) into the facade.

Search for `BoxModel` or `Style` in `crates/buiy/src/lib.rs` — that block needs the same widening. Add the same type list as Step 7.2 to the facade re-export, preserving the existing re-export style (whether `pub use buiy_core::{...}` or per-module).

Specifically, append to the `pub use buiy_core::{...}` block (or equivalent) the symbols: `Overflow, OverflowMode, OverscrollBehavior, Scroll, ScrollBehavior, ScrollOffset, ScrollSnapItem, ScrollbarColor, ScrollbarGutter, ScrollbarWidth, SnapAlign, SnapStop, SnapType` (plus any of the 9 enum types not yet surfaced). Keep alphabetical order.

- [ ] **Step 6.4: Run the workspace test suite**

```sh
xvfb-run -a cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 6.5: Run the full check command**

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Expected: clean.

- [ ] **Step 6.6: Commit**

```sh
git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(buiy_core, buiy): register and re-export Phase 2 layout types

LayoutPlugin now reflects Overflow / Scroll / ScrollOffset /
ScrollSnapItem; the four components and 9 supporting enums are
re-exported from buiy_core and the buiy facade.

Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 6
EOF
)"
```

---

### Task 7: Integration test — Taffy overflow mapping + scroll-container detection

**Files:**
- Create: `crates/buiy_core/tests/layout_overflow.rs`

End-to-end test that spawns Buiy entities with various `Overflow` configurations, runs `LayoutPlugin`, and verifies (a) the resulting `taffy::Style` reads back as expected via Taffy's introspection, and (b) `Overflow::is_scroll_container()` matches the spec § 1.2 predicate. The Taffy-style assertion goes through `LayoutTree`, exercising the full Phase 2 chain (Bundle expansion → sync_styles → translate → Taffy storage).

The test uses `taffy::Overflow` directly. Cargo's integration-test compilation in this workspace does expose the lib's regular `[dependencies]` to the `tests/` binary (verified empirically against `taffy.workspace = true` in `crates/buiy_core/Cargo.toml`'s `[dependencies]`), so no dev-dep addition is needed.

- [ ] **Step 7.1: Write the failing tests**

Create `crates/buiy_core/tests/layout_overflow.rs` with the following content:

```rust
//! Phase 2 integration: every `OverflowMode` variant produces the
//! expected `taffy::Style.overflow` value through the full Buiy
//! pipeline, and `Overflow::is_scroll_container` matches spec § 1.2.

use bevy::prelude::*;
use buiy_core::{
    BoxModel, CorePlugin, LayoutTree, Length, Node, Overflow, OverflowMode, Sizing, Style,
    layout::LayoutPlugin,
};

fn run_one_frame_with_overflow(overflow: Overflow) -> taffy::Style {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                overflow,
                ..Default::default()
            },
        ))
        .id();
    app.update();
    let tree = app.world().non_send_resource::<LayoutTree>();
    let id = *tree.by_entity().get(&entity).expect("Taffy node assigned");
    tree.tree_ref().style(id).expect("style retrievable").clone()
}

#[test]
fn overflow_visible_maps_to_taffy_visible() {
    let s = run_one_frame_with_overflow(Overflow::default());
    assert_eq!(s.overflow.x, taffy::Overflow::Visible);
    assert_eq!(s.overflow.y, taffy::Overflow::Visible);
}

#[test]
fn overflow_hidden_and_clip_both_map_to_taffy_hidden() {
    let hidden = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Hidden,
        y: OverflowMode::Hidden,
        ..Default::default()
    });
    assert_eq!(hidden.overflow.x, taffy::Overflow::Hidden);
    assert_eq!(hidden.overflow.y, taffy::Overflow::Hidden);

    let clip = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Clip,
        y: OverflowMode::Clip,
        ..Default::default()
    });
    assert_eq!(clip.overflow.x, taffy::Overflow::Hidden);
    assert_eq!(clip.overflow.y, taffy::Overflow::Hidden);
}

#[test]
fn overflow_scroll_and_auto_both_map_to_taffy_scroll() {
    let scroll = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Scroll,
        y: OverflowMode::Scroll,
        ..Default::default()
    });
    assert_eq!(scroll.overflow.x, taffy::Overflow::Scroll);
    assert_eq!(scroll.overflow.y, taffy::Overflow::Scroll);

    let auto = run_one_frame_with_overflow(Overflow {
        x: OverflowMode::Auto,
        y: OverflowMode::Auto,
        ..Default::default()
    });
    assert_eq!(auto.overflow.x, taffy::Overflow::Scroll);
    assert_eq!(auto.overflow.y, taffy::Overflow::Scroll);
}

#[test]
fn is_scroll_container_matches_spec() {
    assert!(!Overflow::default().is_scroll_container());
    assert!(
        !Overflow {
            x: OverflowMode::Hidden,
            y: OverflowMode::Hidden,
            ..Default::default()
        }
        .is_scroll_container()
    );
    assert!(
        Overflow {
            x: OverflowMode::Scroll,
            ..Default::default()
        }
        .is_scroll_container()
    );
    assert!(
        Overflow {
            y: OverflowMode::Auto,
            ..Default::default()
        }
        .is_scroll_container()
    );
}
```

- [ ] **Step 7.2: Run the test to verify failure**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_overflow
```

Expected: compilation error — `LayoutTree::by_entity()` and `LayoutTree::tree_ref()` don't yet exist. The `LayoutTree` struct's fields are `pub(crate)` per Phase 1's `tree.rs` line 4: `pub(crate) tree: TaffyTree<()>` and `pub(crate) by_entity: HashMap<Entity, TaffyNodeId>`. Tests in `crates/buiy_core/tests/` are *integration tests* (separate crate), so they cannot reach `pub(crate)` items. We need read-only accessors for tests.

- [ ] **Step 7.3: Add public read-only accessors to `LayoutTree`**

Open `crates/buiy_core/src/layout/tree.rs`. Add the following two methods after the existing `len` / `is_empty` impl block:

```rust
impl LayoutTree {
    /// Test-only access to the entity-to-Taffy mapping. Read-only.
    #[doc(hidden)]
    pub fn by_entity(&self) -> &std::collections::HashMap<bevy::prelude::Entity, taffy::NodeId> {
        &self.by_entity
    }

    /// Test-only access to the inner Taffy tree. Read-only.
    #[doc(hidden)]
    pub fn tree_ref(&self) -> &taffy::TaffyTree<()> {
        &self.tree
    }
}
```

The `#[doc(hidden)]` plus the `_ref` suffix marks these as internal — used by integration tests, not part of the public Buiy API.

If the existing `tree.rs` `impl LayoutTree { ... }` block already exists, append both methods there instead of creating a second `impl` block.

- [ ] **Step 7.4: Run the tests to verify pass**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_overflow
```

Expected: all 4 tests pass.

- [ ] **Step 7.5: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 7.6: Commit**

```sh
git add crates/buiy_core/tests/layout_overflow.rs crates/buiy_core/src/layout/tree.rs
git commit -m "$(cat <<'EOF'
test(buiy_core): assert Taffy overflow mapping + scroll-container detection

End-to-end fixture: spawn Buiy entities with each OverflowMode variant,
verify the resulting taffy::Style.overflow.x/y matches spec § 1.1 mapping.
Adds doc-hidden by_entity / tree_ref accessors on LayoutTree to enable
integration-test inspection of the Taffy bridge state.

Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1, § 5
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 7
EOF
)"
```

---

### Task 8: Integration test — `ScrollOffset` mutation does not invalidate layout

**Files:**
- Create: `crates/buiy_core/tests/layout_scroll_offset_no_invalidate.rs`

This test pins the Phase 2 correctness invariant. The test runs `LayoutPlugin`, mutates `ScrollOffset` (and `ScrollSnapItem`) on a tracked entity, and verifies the next frame's `sync_styles` trigger query (the `Or<(Changed<...>)>` filter) yields zero entities — proving that scroll-position / snap-item mutations bypass the Taffy translate path entirely.

A second assertion verifies `ResolvedLayout` is byte-equal across the mutation frame (the user-visible consequence of the trigger-set design).

- [ ] **Step 8.1: Write the failing test**

Create `crates/buiy_core/tests/layout_scroll_offset_no_invalidate.rs`:

```rust
//! Phase 2 invariant: mutating ScrollOffset (or ScrollSnapItem) must
//! NOT cause sync_styles to re-translate the entity in the following
//! frame. Asserted by mirroring sync_styles' trigger query and counting
//! the entities it would yield.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 2.1
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2

use bevy::ecs::query::Or;
use bevy::prelude::*;
use buiy_core::{
    BoxModel, CorePlugin, Display, FlexItem, FlexParams, Length, Node, Overflow, OverflowMode,
    Position, ResolvedLayout, Scroll, ScrollOffset, ScrollSnapItem, Sizing, SnapAlign, Style,
    layout::LayoutPlugin,
};

/// Mirror of `sync_styles`' trigger filter. If `ScrollOffset` or
/// `ScrollSnapItem` mutation triggered this filter, the test would fail.
type SyncStylesFilter = (
    With<Node>,
    Or<(
        Changed<Display>,
        Changed<BoxModel>,
        Changed<Position>,
        Changed<FlexParams>,
        Changed<FlexItem>,
        Changed<Overflow>,
        Changed<Scroll>,
        Changed<Children>,
        Changed<ChildOf>,
    )>,
);

fn count_changed(world: &mut World) -> usize {
    let mut q = world.query_filtered::<Entity, SyncStylesFilter>();
    q.iter(world).count()
}

#[test]
fn mutating_scroll_offset_does_not_trigger_sync_styles() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                overflow: Overflow {
                    y: OverflowMode::Scroll,
                    ..Default::default()
                },
                ..Default::default()
            },
            ScrollOffset::default(),
        ))
        .id();

    // Frame 1: spawn frame; everything is `Changed`. sync_styles fired.
    // Snapshot ResolvedLayout's fields (Vec2 is Copy + PartialEq;
    // ResolvedLayout itself derives only Clone, not Copy or PartialEq —
    // see crates/buiy_core/src/components.rs).
    app.update();
    let pos_after_first_frame = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout written on frame 1")
        .position;
    let size_after_first_frame = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout written on frame 1")
        .size;

    // Frame 2: nothing has changed since frame 1. sync_styles trigger
    // query must yield zero entities.
    app.update();
    let count = count_changed(app.world_mut());
    assert_eq!(
        count, 0,
        "sync_styles trigger should be empty in steady-state frame 2"
    );

    // Mutate ScrollOffset. This is the operation that must NOT invalidate.
    {
        let mut offset = app
            .world_mut()
            .get_mut::<ScrollOffset>(entity)
            .expect("entity has ScrollOffset");
        offset.y = 50.0;
    }

    // Frame 3: ScrollOffset changed; sync_styles trigger query should
    // STILL yield zero entities (ScrollOffset is excluded from the filter).
    let count_after_offset_mutation = count_changed(app.world_mut());
    assert_eq!(
        count_after_offset_mutation, 0,
        "ScrollOffset mutation must not enter sync_styles' trigger set"
    );

    // Run frame 3 and verify ResolvedLayout fields are unchanged from frame 1.
    app.update();
    let pos_after_offset_mutation = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout still present")
        .position;
    let size_after_offset_mutation = app
        .world()
        .get::<ResolvedLayout>(entity)
        .expect("ResolvedLayout still present")
        .size;
    assert_eq!(
        pos_after_first_frame, pos_after_offset_mutation,
        "ResolvedLayout.position must be unchanged across a scroll-only frame"
    );
    assert_eq!(
        size_after_first_frame, size_after_offset_mutation,
        "ResolvedLayout.size must be unchanged across a scroll-only frame"
    );
}

#[test]
fn mutating_scroll_snap_item_does_not_trigger_sync_styles() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(50.0)),
                    height: Sizing::Length(Length::Px(50.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
            ScrollSnapItem::default(),
            ChildOf(parent),
        ))
        .id();

    app.update();
    app.update();
    assert_eq!(count_changed(app.world_mut()), 0, "steady-state");

    {
        let mut item = app
            .world_mut()
            .get_mut::<ScrollSnapItem>(child)
            .expect("child has ScrollSnapItem");
        item.align = SnapAlign::Center;
    }

    assert_eq!(
        count_changed(app.world_mut()),
        0,
        "ScrollSnapItem mutation must not enter sync_styles' trigger set"
    );
}
```

- [ ] **Step 8.2: Run the test to verify it fails (and *why* it fails)**

```sh
xvfb-run -a cargo test -p buiy_core --test layout_scroll_offset_no_invalidate
```

Expected behavior depends on Task 5's correctness:

- If Task 5's trigger filter does NOT include `Changed<ScrollOffset>`: the test compiles and passes immediately. Confirm pass output:
  ```
  test result: ok. 2 passed
  ```
- If a future contributor accidentally adds `Changed<ScrollOffset>` to the filter: this test fails the third assertion (`count_after_offset_mutation` would be 1).

If the test fails to compile, expect a missing-symbol error — likely `ResolvedLayout` not re-exported. If so, verify Task 6's re-export of `ResolvedLayout` (which is from `buiy_core::components`, not `layout`). The Phase 1 re-export of `ResolvedLayout` from `buiy_core::lib` should already cover it.

- [ ] **Step 8.3: If the test passes, no implementation step is needed — the invariant is held by Task 5**

This task is a *codification* of an invariant established earlier. The "test → fail → implement → pass" rhythm collapses to "test → pass" because Task 5 already excluded the problematic triggers. That's correct for an invariant test; the value is *future regression detection* if someone widens the filter wrongly.

If the test does fail unexpectedly, debug as follows:

1. Read `crates/buiy_core/src/layout/systems.rs`'s `sync_styles` filter; confirm it lists `Or<(Changed<Display>, Changed<BoxModel>, Changed<Position>, Changed<FlexParams>, Changed<FlexItem>, Changed<Overflow>, Changed<Scroll>, Changed<Children>, Changed<ChildOf>)>`.
2. If `Changed<ScrollOffset>` or `Changed<ScrollSnapItem>` is in the filter, remove it.
3. Re-run the test.

- [ ] **Step 8.4: Run clippy and fmt**

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 8.5: Commit**

```sh
git add crates/buiy_core/tests/layout_scroll_offset_no_invalidate.rs
git commit -m "$(cat <<'EOF'
test(buiy_core): assert ScrollOffset/ScrollSnapItem don't invalidate layout

Pins Phase 2's central correctness invariant: mutating runtime/child-side
scroll components must not enter sync_styles' trigger filter. Test
mirrors the filter directly so future filter widenings against this
contract surface in CI.

Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 2.1
Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 8
EOF
)"
```

---

### Task 9: Update CHANGELOG and run final verification

**Files:**
- Modify: `CHANGELOG.md`

`docs/README.md`'s Layout > Plans section was updated to include this plan when the plan itself was written (see git log for the plan-write commit). The `[active]` tag stays through Phase 2 execution and PR review; flip to `[landed]` happens in a small follow-up commit on `main` after merge.

- [ ] **Step 9.1: Update `CHANGELOG.md`**

Open `CHANGELOG.md`. Find the `## [Unreleased]` section. Add the following under it (creating `### Added` and `### Changed` subsections if not present, or appending to existing ones):

```markdown
### Added

- Layout `Overflow` component (per-axis `OverflowMode` + `scrollbar_*`,
  `scroll_behavior`, `overscroll_*`). Wired into `taffy::Style.overflow`
  and `taffy::Style.scrollbar_width`. Spec:
  `docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md`.
- Layout `Scroll` component (snap-type, snap padding, snap margin) for
  scroll-snap container declaration.
- Layout `ScrollOffset` runtime-state component (per-axis scroll
  position). Mutation does not invalidate `ResolvedLayout` (asserted by
  `tests/layout_scroll_offset_no_invalidate.rs`).
- Layout `ScrollSnapItem` decomposed-only child-side component.
- 9 supporting layout enum types: `OverflowMode`, `ScrollbarGutter`,
  `ScrollbarWidth`, `ScrollbarColor`, `ScrollBehavior`,
  `OverscrollBehavior`, `SnapType`, `SnapAlign`, `SnapStop`.
- `Style` builder: `Overflow` and `Scroll` fields; 12 fluent setters
  (`.overflow_x()`, `.overflow_y()`, `.overflow()`, `.overflow_hidden()`,
  `.overflow_y_scroll()`, `.overflow_x_scroll()`, `.scrollbar_gutter()`,
  `.scrollbar_width()`, `.scroll_behavior()`, `.snap_type()`,
  `.snap_padding()`, `.snap_margin()`).
- `Overflow::is_scroll_container()` predicate (spec § 1.2).
- Doc-hidden read-only accessors on `LayoutTree`: `by_entity()` and
  `tree_ref()` for integration-test introspection.

### Changed

- `sync_styles`' change-detection trigger set widens to include
  `Changed<Overflow>` and `Changed<Scroll>`; remains exclusive of
  `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>`.
```

- [ ] **Step 9.2: Run the full check command (the project's CI mirror)**

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

Expected: clean across the board. All Phase 1 tests still pass; all Phase 2 tests pass.

If anything fails, fix in place before committing — do NOT skip / suppress.

- [ ] **Step 9.3: Verify `cargo deny` passes (no dep changes, but routine)**

```sh
cargo deny check
```

Expected: clean.

- [ ] **Step 9.4: Commit**

```sh
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
docs: changelog entries for Phase 2 (overflow + scrolling)

Adds Unreleased entries for the four new components, 9 enum types,
Style Bundle extension, and the sync_styles trigger-set widening.

Plan: docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md task 9
EOF
)"
```

---

## Final note for executors

Phase 2 is purely additive — no Phase 1 contract changes. After Task 9:

- Branch should have 10 commits (one per task plus the plan-write commit).
- `git diff main...HEAD --stat` should show: `crates/buiy_core/src/layout/{components,mod,style,systems,translate,tree,types}.rs`, `crates/buiy_core/src/lib.rs`, `crates/buiy/src/lib.rs`, two new test files, `CHANGELOG.md`, `docs/README.md` (catalog entry from plan-write), and the plan file itself.
- All Phase 1 tests pass unchanged.
- The two new integration tests (`layout_overflow.rs`, `layout_scroll_offset_no_invalidate.rs`) pass.

Push the branch, open a PR titled "Phase 2: layout overflow and scrolling", let CI run all six gates (Lint, Test on ubuntu/macos/windows, Doc, Deny), and merge when green. Mark the plan `[landed]` in `docs/README.md` in a small follow-up commit on `main`.
