# Buiy layout writing modes — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Date:** 2026-05-10
**Status:** landed
**Spec:** [`specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md`](../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md) § 2 + cross-references to [`box-model.md`](../specs/2026-05-08-buiy-layout-design/box-model.md) § 4 (logical edges) and [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) §§ 1.2, 3.

**Goal:** Phase 4 of the layout migration — ship the writing-mode subsystem: a `WritingMode` author-set component (mode + direction + text-orientation + unicode-bidi), a `WritingModeResolved` private cache populated by a new pre-`SyncStyles` inheritance pass, and the `LogicalBoxModel` / `LogicalInset` ergonomic builder helpers that translate logical edges to physical at construct time. Wire `Direction::Rtl` to `taffy::Style.direction` so flex children mirror under RTL, and route `WritingModeKind::Sideways{Rl,Lr}` through a warn-once gate that falls back to the corresponding non-sideways vertical mode (the glyph-rotation pass is `buiy-text-rendering-design`'s concern, not layout). Widen `sync_styles`'s trigger filter to include `Changed<WritingMode>` and `Changed<WritingModeResolved>`.

**Architecture:** Adds 1 component, 1 private cache component, 4 supporting enum types, 3 builder helpers (non-component, non-Bundle — author-side ergonomic structs), and 1 new `BuiyLayoutStep` variant under the existing Phase 1 layout module tree. `WritingMode` is container-side self-styling and joins `Style`'s Bundle. `WritingModeResolved` is private (synced, not author-set) and not in the Bundle. The inheritance pass runs as a new system set `BuiyLayoutStep::WritingModeInherit` placed between `RemovedNodesGc` and `SyncStyles`; the existing 8-step chain becomes 9 steps. The translation layer extends `StyleView` with `&WritingModeResolved`; `style_to_taffy` maps `Direction` directly onto `taffy::Style.direction` (Taffy 0.10 ships the field as `Ltr` / `Rtl`). `WritingModeKind::Vertical{Rl,Lr}` and `Sideways{Rl,Lr}` are stored but **layout-side they have no Taffy correspondent** — Taffy 0.10 doesn't model writing-mode for axis-swap; vertical-mode authors get the `WritingModeResolved` cache wired through `LogicalBoxModel`/`LogicalInset` translation but no Taffy-side axis flip. The `LogicalBoxModel`-to-`BoxModel` translation honors vertical modes; the cascade ends there.

**Tech Stack:** Rust, Bevy 0.18 ECS + reflection + `Bundle`, Taffy 0.10. No new external dependencies.

---

## Phasing strategy reference

This is **Phase 4** of the migration kicked off by the [Phase 1 plan](2026-05-08-buiy-layout-foundation.md#phasing-strategy-this-plan-vs-follow-ups). The phasing-strategy table there lists Phase 4 as `*-buiy-layout-writing-modes.md`. Phase 4 depends only on Phase 1 (Phase 2 + Phase 3 already landed; no dependency on either, but their Bundle expansions cohabit cleanly).

What consumes Phase 4 surface in later phases:

- Phase 5 (`*-buiy-layout-container-queries.md`) — uses `cqi`/`cqb`/`cqw`/`cqh` units that resolve against `WritingModeResolved`'s inline/block axes. Phase 5 depends on Phase 4.
- Phase 6 (`*-buiy-layout-anchor-positioning.md`) — adds the `ContainingBlock` cache that Phase 4's phasing-table entry mentioned. **Phase 4 explicitly defers `ContainingBlock`** to Phase 6 where it has a real consumer; the spec notes the cache exists but no Phase 4 system reads it.
- `buiy-text-rendering-design` (sibling spec) — owns glyph rotation for `Sideways{Rl,Lr}` and the bidi resolution algorithm consuming `WritingMode.unicode_bidi`. Phase 4 stores those fields; the text spec consumes them.

What Phase 4 does NOT ship (deferred to follow-ups):

1. **`ContainingBlock` cache** — deferred to Phase 6 (anchor positioning) where it has a consumer. The spec's architecture.md § 1.2 mentions it; Phase 4 leaves it un-implemented.
2. **`Sideways{Rl,Lr}` glyph rotation** — owned by `buiy-text-rendering-design`. Phase 4 routes them to a warn-once gate and treats them as `Vertical{Rl,Lr}` for layout purposes.
3. **`UnicodeBidi` semantic resolution** — owned by `buiy-i18n-design`. Phase 4 stores the value on `WritingMode.unicode_bidi`; no Phase 4 system reads it.
4. **Vertical-mode Taffy axis-swap** — Taffy 0.10 has no writing-mode awareness, so vertical modes don't reorient the flex/grid main axis at the Taffy level. The `LogicalBoxModel`/`LogicalInset` builders translate logical-to-physical correctly so authors get the right pixels, but the translation runs at *construct time*, not as a per-frame pass. Dynamic writing-mode switches require re-spawn of logical helpers; this is a v1.x extension.

---

## File structure

### Modified files

```
crates/buiy_core/src/layout/
├── types.rs          — append: 4 new enums (WritingModeKind, Direction,
│                       TextOrientation, UnicodeBidi); 1 new value type
│                       (LogicalEdges)                                    (Task 1, 4)
├── components.rs     — add WritingMode (author-set, joins Style Bundle)
│                       add WritingModeResolved (private cache)            (Task 2)
├── pipeline.rs       — add BuiyLayoutStep::WritingModeInherit between
│                       RemovedNodesGc and SyncStyles                       (Task 3)
├── systems.rs        — add inherit_writing_mode system; widen sync_styles
│                       trigger filter (Changed<WritingMode>,
│                       Changed<WritingModeResolved>); StyleView add
│                       writing_mode_resolved                                (Task 3, 5)
├── translate.rs      — extend StyleView; map Direction → taffy::Direction;
│                       sideways-* warn-once fallback to vertical-rl/lr     (Task 5)
├── style.rs          — extend Style Bundle with `writing_mode`; add
│                       fluent setters; add LogicalBoxModel + LogicalInset
│                       builder structs with .to_box_model(&WritingMode)
│                       and .to_inset(&WritingMode) helpers                  (Task 4, 6)
└── mod.rs            — register WritingMode + WritingModeResolved;
                        re-export new types and helpers                       (Task 7)
```

```
crates/buiy_core/src/lib.rs    — re-export new types from buiy_core         (Task 7)
crates/buiy/src/lib.rs         — re-export from buiy facade                  (Task 7)
crates/buiy_core/tests/layout_pipeline_order.rs
                               — update expected step ordering: 9 steps    (Task 3)
```

### New tests

```
crates/buiy_core/tests/
└── layout_writing_modes.rs   — 5 fixtures: rtl flips flex children,
                                 vertical-rl swaps inline/block dimensions
                                 via LogicalBoxModel, inheritance pass
                                 propagates WritingMode down the hierarchy,
                                 LogicalEdges translate to physical Edges
                                 under vertical-rl, sideways-rl falls back
                                 to vertical-rl + warn                      (Task 8)
```

### Modified docs / non-code files

- `CHANGELOG.md` — `[Unreleased]` `### Added` and `### Changed` entries (Task 9).
- `docs/README.md` — Phase 4 plan entry tagged `[active]` during the plan-write commit; flipped to `[landed]` post-merge.

### No deletions

Phase 4 is purely additive — no Phase 1, 2, or 3 file or item is removed.

---

## Coverage map

Every Phase 4 spec requirement maps to a row below. **Deferred** items are explicitly out of Phase 4 (spec section names where they pick up). **Simplified vs spec** items name where Phase 4 narrows the spec text in favor of a tractable v1 surface.

| Spec section | Phase 4 coverage | Task |
|---|---|---|
| container-queries-and-writing-modes.md § 2.1 — `WritingMode` shape | Ships in full: `mode: WritingModeKind`, `direction: Direction`, `text_orientation: TextOrientation`, `unicode_bidi: UnicodeBidi`. | 2 |
| § 2.1 — `WritingModeKind` (HorizontalTb / VerticalRl / VerticalLr / SidewaysRl / SidewaysLr) | Ships in full. `Sideways{Rl,Lr}` reach the warn-once gate in `style_to_taffy` and get mapped to their non-sideways vertical equivalent for layout (text-rendering owns glyph rotation). | 1, 5 |
| § 2.1 — `Direction` (Ltr / Rtl) | Ships in full. Maps directly to `taffy::Direction::{Ltr, Rtl}` via `taffy::Style.direction`. | 1, 5 |
| § 2.1 — `TextOrientation` (Mixed / Upright / Sideways) | Stored on `WritingMode.text_orientation`. **Layout side does not act on it** (text-rendering's concern; foundation tier-E). | 1 |
| § 2.1 — `UnicodeBidi` (Normal / Embed / Isolate / BidiOverride / IsolateOverride / Plaintext) | Stored. **Resolution algorithm is `buiy-i18n-design`'s concern.** | 1 |
| § 2.2 — `WritingModeResolved` private cache + inheritance | Ships. New private `WritingModeResolved` component synced by a new system that runs in `BuiyLayoutStep::WritingModeInherit` (placed before `SyncStyles`). The walking is `O(subtree size)` per `Changed<WritingMode>` cluster; spec § 2.2 calls this acceptable because writing-mode changes are rare. | 2, 3 |
| § 2.3 — Logical → physical translation table | Implemented in `LogicalBoxModel::to_box_model(&WritingMode)` and `LogicalInset::to_inset(&WritingMode)`. The 6-row mapping table from the spec (horizontal-tb + ltr/rtl, vertical-rl + ltr/rtl, vertical-lr + ltr/rtl) lives in one helper that's exercised by every translation. | 4 |
| § 2.4 — Taffy integration: route `direction` through Taffy | `taffy::Style.direction = match WritingModeResolved.direction { Ltr => Direction::Ltr, Rtl => Direction::Rtl }`. Phase 4 wiring lives in `style_to_taffy`. | 5 |
| § 2.4 — Taffy integration: route logical insets through Taffy | **Simplified vs spec.** Spec hints at routing through `taffy::Style.inset.start/end`; Taffy 0.10 has no `inline-start`/`inline-end` field surface for inset (the geometry is `taffy::Rect<LengthPercentageAuto>` with `top/right/bottom/left`). Phase 4 instead translates logical insets to physical at `LogicalInset::to_inset(&WritingMode)` construct time. The author-side surface is still logical (CSS-faithful); Taffy gets physical. | 4 |
| box-model.md § 4.2 — `Changed<WritingMode>` propagating to a re-translation pass | **Deferred to v1.x.** Spec says writing-mode switches should re-derive physical `BoxModel` for entities whose source was a `LogicalBoxModel`. Phase 4 v1 ships construct-time-only translation: dynamic switches require re-spawn / re-insert of `Style`. The deferral is justified by spec § 4.1 ("not stored — insert-time transform") which is internally inconsistent with § 4.2; Phase 4 picks § 4.1's reading. See plan § Decisions made #13. | — |
| § 2.5 — `Sideways{Rl,Lr}` open question | **Deferred to `buiy-text-rendering-design`.** Layout side (this plan) treats them as `Vertical{Rl,Lr}` and emits one `warn!` per session naming the limitation. | 1, 5 |
| § 2.6 — `unicode-bidi` resolution | **Deferred to `buiy-i18n-design`.** Phase 4 stores the value only. | 1 |
| § 2.7 — Test surface: `direction: rtl` flips flex | Integration test pins flex-row child x-positions inverted under `Direction::Rtl`. | 8 |
| § 2.7 — Test surface: `writing-mode: vertical-rl` | Integration test asserts a `LogicalBoxModel { inline_size: 100, .. }` under `vertical-rl` produces `BoxModel { height: 100, .. }`. | 8 |
| § 2.7 — Test surface: inheritance | Integration test sets `WritingMode::VerticalRl` on a parent; asserts descendant's `WritingModeResolved == VerticalRl`. | 8 |
| § 2.7 — Test surface: logical → physical edge | `LogicalEdges { inline_start: 8.0, .. }.to_edges(&vertical_rl_ltr)` produces `Edges { top: 8.0, .. }`. Unit test in types.rs::tests. | 4 |
| § 2.7 — Test surface: `sideways-rl` falls back to `vertical-rl` + warn | Integration test asserts a `WritingMode::SidewaysRl` entity lays out exactly like a `WritingMode::VerticalRl` one and produces no panic. | 8 |
| architecture.md § 1.2 — `Changed<WritingMode>` and `Changed<WritingModeResolved>` in the trigger filter | Both added to the `Or<>` clause in `sync_styles`. The Resolved variant covers the "ancestor change → descendant invalidation" path. | 5 |
| architecture.md § 3 — pipeline ordering | New `BuiyLayoutStep::WritingModeInherit` step inserted between `RemovedNodesGc` (step 0) and `SyncStyles` (step 1). The existing chain becomes 9 ordered steps. The numerical step labels in `pipeline.rs` doc-comments stay (step 0 / 1 / 2 / etc. retain their names); the new `WritingModeInherit` is unnumbered ("step 0.5" if you must label) — its purpose is to populate `WritingModeResolved` before any system that reads it. | 3 |
| architecture.md § 1.2 — `ContainingBlock` cache | **Deferred to Phase 6** (`*-buiy-layout-anchor-positioning.md`) where the cache has a real consumer. Phase 4's phasing-strategy table mentioned this; the deferral keeps Phase 4 scope focused on writing-modes proper. | — |

---

## Decisions made

These commitments are documented up front so future-me / reviewer can see the reasoning without re-deriving it.

1. **`LogicalBoxModel` / `LogicalInset` are non-component builder structs, not stored components.** The spec text in § 4.1 says "not stored — insert-time transform" and § 4.2 mentions a "re-translation pass" — the two are inconsistent. Phase 4 picks § 4.1's reading: the helpers are pure data structures with `.to_box_model(&WritingMode)` and `.to_inset(&WritingMode)` methods returning `BoxModel` / `Inset`. Authors construct + spawn:
   ```rust
   let bm = LogicalBoxModel { inline_size: Sizing::Length(Length::Px(100.0)), .. }
       .to_box_model(&WritingMode::default());
   commands.spawn((Style { box_model: bm, ..default() }, Node));
   ```
   No system-level translation pass; no stored `LogicalBoxModel` component; no implicit inheritance lookup. Authors who want their logical authoring to honor an inherited writing-mode pass the resolved value themselves. **Dynamic writing-mode switches require re-spawn / re-insert** — this is a v1.x extension.
2. **`WritingMode` joins `Style`'s Bundle; `WritingModeResolved` does NOT.** Author-set is canonical; the resolved cache is private synced state.
3. **`WritingModeInherit` is a new pipeline step, not a sub-system of step 0 or step 1.** Reason: the inheritance walk needs its own change-detection scope (`Changed<WritingMode>` triggers it), and chaining it as a system inside `RemovedNodesGc` or `SyncStyles` would conflate concerns. A dedicated step gives a clean test target (`tests/layout_pipeline_order.rs` updates from 8 to 9) and matches the spec § 1.2 wording "an inheritance pass before step 1".
4. **`Direction::Rtl` is the only writing-mode field Taffy can act on natively.** Taffy 0.10 has `Style.direction: Direction { Ltr, Rtl }` and applies it to flex's main-axis mirroring + grid column flow. It has no `writing-mode: vertical-rl` axis-swap. Phase 4 wires `direction` through; vertical modes are stored on `WritingMode.mode` and consumed only by the `LogicalBoxModel`/`LogicalInset` translation helpers.
5. **Vertical writing-modes do not flip the Taffy main axis.** A `Display::Flex(Row)` container under `WritingMode::VerticalRl` still lays children left-to-right at the Taffy level. To get top-to-bottom under vertical-rl, authors use `Display::Flex(Column)` explicitly. The spec § 2.4 hints at "block-level mirroring" honored by Taffy's `rtl` flag; that's flex `flex-start ↔ flex-end` swapping, not axis swap. **Documented limitation** — pinning vertical-rl axis-swap is a v1.x feature contingent on Taffy upstream support or a Buiy-side post-Taffy axis-swap pass.
6. **`Sideways{Rl,Lr}` warn-once gates name `buiy-text-rendering-design` as the future owner.** Phase 4's warn message: `"buiy: WritingModeKind::SidewaysRl glyph rotation lives in buiy-text-rendering-design; layout treats it as VerticalRl (warned once)"`. Same shape for `SidewaysLr → VerticalLr`.
7. **Pipeline test (`layout_pipeline_order.rs`) widens to 9 steps.** Phase 1's test asserts the exact 8-step ordering; Task 3 updates it to expect 9 (insertion of `WritingModeInherit` between steps 0 and 1).
8. **`WritingMode` derives include `Copy` because every field is a small enum.** Matches `FlexParams` and `Display` precedent.
9. **`WritingModeResolved` is `Default = HorizontalTb + Ltr + Mixed + Normal`.** Same defaults as `WritingMode`. Entities without a `WritingMode` ancestor get the all-defaults resolved value.
10. **No `#[non_exhaustive]` on the new enums.** Phase 4 doesn't anticipate adding variants; if a future phase needs to (e.g., new CSS writing-mode keyword), it's a breaking change documented in the CHANGELOG. Same precedent as Phases 1-3.
11. **Inheritance is a true ancestor walk with memoization, not single-level.** Spec § 2.2 says "the nearest ancestor's" — Phase 4 honors this with a memoized recursive lookup that walks up the `ChildOf` chain until a `WritingMode` is found or the root is reached. Memoization makes the per-frame cost O(N) regardless of tree depth (each entity's effective value is resolved at most once per frame).
12. **Inheritance system writes `WritingModeResolved` only when the value actually changes.** Plan Task 3 reads the entity's current `WritingModeResolved` and skips the `insert` when the new value matches. This matters because Task 5 widens `sync_styles`'s trigger filter to include `Changed<WritingModeResolved>`; an unconditional re-insert every frame would void the O(0) steady-state contract that Phase 1 + Phase 2 carefully preserve.
13. **Dynamic writing-mode switches require re-spawn / re-insert in v1.** Spec box-model.md § 4.2 mentions a "re-translation pass" that recomputes physical `BoxModel` when an entity's source `LogicalBoxModel` is re-evaluated against a new `WritingMode`. Phase 4 v1 does **not** ship this pass — `LogicalBoxModel` is a non-component builder, so the physical `BoxModel` is computed once at construct time. **Documented limitation:** apps that flip writing-mode at runtime (e.g., LTR/RTL locale toggle) must re-spawn or re-insert `Style` to pick up the new physical box. The re-translation pass is deferred to v1.x; making `LogicalBoxModel` a stored component conflicts with spec § 4.1's "not stored" wording, and the spec text itself is internally fuzzy on this point. Phase 4's choice is to honor § 4.1 and document the trade-off; § 4.2's pass lands when the spec is tightened.

---

## Tasks

The plan is 9 tasks. Each is self-contained: passes `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` before commit. Two-stage review (spec compliance → code quality) per task.

### Task 1: WritingMode value types in `types.rs`

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` — append 4 new enums.
- Test: `crates/buiy_core/src/layout/types.rs` (`#[cfg(test)] mod tests`).

**Test surface (added at the bottom of the existing tests module):**

- `writing_mode_kind_default_is_horizontal_tb` — `WritingModeKind::default() == WritingModeKind::HorizontalTb`.
- `direction_default_is_ltr` — `Direction::default() == Direction::Ltr`.
- `text_orientation_default_is_mixed` — `TextOrientation::default() == TextOrientation::Mixed`.
- `unicode_bidi_default_is_normal` — `UnicodeBidi::default() == UnicodeBidi::Normal`.

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests { ... }` block in `types.rs`:

```rust
    #[test]
    fn writing_mode_kind_default_is_horizontal_tb() {
        assert_eq!(WritingModeKind::default(), WritingModeKind::HorizontalTb);
    }

    #[test]
    fn direction_default_is_ltr() {
        assert_eq!(Direction::default(), Direction::Ltr);
    }

    #[test]
    fn text_orientation_default_is_mixed() {
        assert_eq!(TextOrientation::default(), TextOrientation::Mixed);
    }

    #[test]
    fn unicode_bidi_default_is_normal() {
        assert_eq!(UnicodeBidi::default(), UnicodeBidi::Normal);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```sh
cargo test -p buiy_core --lib types::tests 2>&1 | tail -20
```

Expected: compile errors — `WritingModeKind`, `Direction`, `TextOrientation`, `UnicodeBidi` not found.

- [ ] **Step 3: Add the four enums**

Append to `types.rs` after the existing enum definitions (at the bottom of the public types section, before the `#[cfg(test)]` line):

```rust
/// CSS `writing-mode`.
///
/// `Sideways{Rl,Lr}` are tier-C polish modes that rotate text glyphs but
/// otherwise behave like `Vertical{Rl,Lr}` for layout. Glyph rotation is
/// `buiy-text-rendering-design`'s concern; layout treats them as their
/// non-sideways equivalents and emits one `warn!` per session.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.1.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritingModeKind {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    SidewaysRl,
    SidewaysLr,
}

/// CSS `direction`. Maps directly to `taffy::Direction`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// CSS `text-orientation`. Stored on `WritingMode`; consumed by
/// `buiy-text-rendering-design`, not layout.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOrientation {
    #[default]
    Mixed,
    Upright,
    Sideways,
}

/// CSS `unicode-bidi`. Stored on `WritingMode`; resolution lives in
/// `buiy-i18n-design`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnicodeBidi {
    #[default]
    Normal,
    Embed,
    Isolate,
    BidiOverride,
    IsolateOverride,
    Plaintext,
}
```

- [ ] **Step 4: Run tests to verify they pass**

```sh
cargo test -p buiy_core --lib types::tests 2>&1 | tail -10
```

Expected: existing tests + 4 new = all pass.

- [ ] **Step 5: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 6: Commit**

```sh
git add crates/buiy_core/src/layout/types.rs
git commit -m "feat(buiy_core): add WritingMode value types

Adds WritingModeKind, Direction, TextOrientation, UnicodeBidi. Sideways
variants reach a warn-once gate in Phase 4 Task 5; TextOrientation and
UnicodeBidi are stored only (consumed by buiy-text-rendering-design and
buiy-i18n-design respectively, not layout)."
```

---

### Task 2: `WritingMode` + `WritingModeResolved` components

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` — add 2 components.

**Test surface:**

- `writing_mode_default_is_horizontal_tb_ltr_mixed_normal` — every field defaults match spec.
- `writing_mode_resolved_default_is_horizontal_tb_ltr_mixed_normal` — same defaults.

- [ ] **Step 1: Write the failing tests**

Append to `#[cfg(test)] mod tests` in `components.rs`:

```rust
    #[test]
    fn writing_mode_default_is_horizontal_tb_ltr_mixed_normal() {
        let wm = WritingMode::default();
        assert_eq!(wm.mode, WritingModeKind::HorizontalTb);
        assert_eq!(wm.direction, Direction::Ltr);
        assert_eq!(wm.text_orientation, TextOrientation::Mixed);
        assert_eq!(wm.unicode_bidi, UnicodeBidi::Normal);
    }

    #[test]
    fn writing_mode_resolved_default_is_horizontal_tb_ltr_mixed_normal() {
        let wm = WritingModeResolved::default();
        assert_eq!(wm.mode, WritingModeKind::HorizontalTb);
        assert_eq!(wm.direction, Direction::Ltr);
        assert_eq!(wm.text_orientation, TextOrientation::Mixed);
        assert_eq!(wm.unicode_bidi, UnicodeBidi::Normal);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Expected: `WritingMode`, `WritingModeResolved` not found.

- [ ] **Step 3: Extend the `use` import in `components.rs`**

Add the new types to the existing `use super::types::{...}` line:

```rust
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Direction, Edges, FlexAxis, FlexGap,
    FlexWrap, GridAreas, GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems,
    OverflowMode, OverscrollBehavior, PositionKind, ScrollBehavior, ScrollbarColor,
    ScrollbarGutter, ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType, TextOrientation,
    TrackSize, UnicodeBidi, WritingModeKind,
};
```

- [ ] **Step 4: Add `WritingMode` component**

Append (after `GridItem`, alphabetically rough but the file groups by author-set then private):

```rust
/// CSS writing-mode + direction + text-orientation + unicode-bidi, all on
/// one component because they're authored together. Joins `Style`'s
/// Bundle. The inherited effective value is computed by the
/// `inherit_writing_mode` system and stored in `WritingModeResolved`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.1.
///
/// Note: vertical writing-modes (`VerticalRl` / `VerticalLr`) do *not*
/// reorient the Taffy main axis — Taffy 0.10 has no writing-mode
/// awareness at the layout-engine level. Vertical modes are honored only
/// by the `LogicalBoxModel` / `LogicalInset` ergonomic builders. Authors
/// who want top-to-bottom flow under vertical-rl use
/// `Display::Flex(Column)` explicitly. See plan § Decisions made #5.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct WritingMode {
    pub mode: WritingModeKind,
    pub direction: Direction,
    pub text_orientation: TextOrientation,
    pub unicode_bidi: UnicodeBidi,
}
```

- [ ] **Step 5: Add `WritingModeResolved` component**

Append after `WritingMode`:

```rust
/// Inherited effective writing-mode for an entity. Synced by the
/// `inherit_writing_mode` system in `BuiyLayoutStep::WritingModeInherit`,
/// run before `SyncStyles`. **Private cache — not author-set, not in
/// `Style`'s Bundle.**
///
/// The translation layer (`style_to_taffy`) reads this value to wire
/// `Direction::Rtl` to `taffy::Style.direction` and to gate `Sideways{Rl,Lr}`
/// through the warn-once fallback. The `LogicalBoxModel` and `LogicalInset`
/// builders take a `&WritingMode` directly (not the Resolved cache),
/// because they translate at construct time on the author's side.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.2.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct WritingModeResolved {
    pub mode: WritingModeKind,
    pub direction: Direction,
    pub text_orientation: TextOrientation,
    pub unicode_bidi: UnicodeBidi,
}

impl WritingModeResolved {
    /// Construct from a parent `WritingMode`. Used by the inheritance
    /// system to copy fields one-to-one.
    pub(crate) fn from_writing_mode(wm: &WritingMode) -> Self {
        Self {
            mode: wm.mode,
            direction: wm.direction,
            text_orientation: wm.text_orientation,
            unicode_bidi: wm.unicode_bidi,
        }
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

```sh
cargo test -p buiy_core --lib components::tests 2>&1 | tail -10
```

Expected: existing 12 + 2 new = 14 tests pass.

- [ ] **Step 7: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean **after** the `#[allow(dead_code)]` annotations described below. With `-D warnings`, `pub(crate) fn from_writing_mode` triggers `dead_code` because no module reads it until Task 3 lands. `WritingMode` itself is `pub`, re-exported in Task 7, so it's exempt.

Before running clippy, annotate the helper:

```rust
impl WritingModeResolved {
    /// Construct from a parent `WritingMode`. Used by the inheritance
    /// system to copy fields one-to-one.
    // Consumed by `inherit_writing_mode` system in Phase 4 Task 3.
    #[allow(dead_code)]
    pub(crate) fn from_writing_mode(wm: &WritingMode) -> Self {
        Self {
            mode: wm.mode,
            direction: wm.direction,
            text_orientation: wm.text_orientation,
            unicode_bidi: wm.unicode_bidi,
        }
    }
}
```

The allow can be removed in Task 3 once the inheritance system consumes the helper.

- [ ] **Step 8: Commit**

```sh
git add crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core): add WritingMode + WritingModeResolved components

WritingMode (author-set, will join Style Bundle in Task 6) carries the
full CSS writing-mode + direction + text-orientation + unicode-bidi
surface. WritingModeResolved (private cache, synced by Task 3's
inheritance pass) carries the inherited effective value used by
translate.rs in Task 5.

#[allow(dead_code)] on WritingModeResolved::from_writing_mode pending
Task 3's consumer; clippy -D warnings would otherwise reject the
unused pub(crate) helper."
```

---

### Task 3: `WritingModeInherit` pipeline step + inheritance system

**Files:**
- Modify: `crates/buiy_core/src/layout/pipeline.rs` — add `WritingModeInherit` variant + chain order.
- Modify: `crates/buiy_core/src/layout/systems.rs` — add `inherit_writing_mode` system.
- Modify: `crates/buiy_core/src/layout/mod.rs` — wire the new system into the new step set.
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` — assert 9 steps.

**Test surface (integration test in `layout_pipeline_order.rs`):**

The existing test checks that the 8 pipeline steps run in order. Phase 4 widens it to expect 9 — `WritingModeInherit` between `RemovedNodesGc` and `SyncStyles`.

- [ ] **Step 1: Read current `tests/layout_pipeline_order.rs`** to see the exact tracker pattern.

The test wires a tracker closure into each `BuiyLayoutStep` set, runs `app.update()`, and asserts the recorded order matches a string vector. The current shape (8 trackers labeled "0".."7"):

```rust
app.add_systems(Update, make_tracker(o.clone(), "0").in_set(BuiyLayoutStep::RemovedNodesGc));
app.add_systems(Update, make_tracker(o.clone(), "1").in_set(BuiyLayoutStep::SyncStyles));
// ... up to "7"
assert_eq!(observed, vec!["0", "1", "2", "3", "4", "5", "6", "7"]);
```

Phase 4 inserts a 9th tracker for `WritingModeInherit` between RemovedNodesGc and SyncStyles. **Re-label by enum order, not spec-step number, so the labels are visually unambiguous:** "gc", "wmi" (writing-mode inherit), "sync", "cq_activate", "taffy", "cq_flip", "cq_rerun", "post_taffy", "write".

- [ ] **Step 2: Update the test to expect the new ordering**

Replace the 8-tracker block with 9, re-labeled:

```rust
let o = order.clone();
app.add_systems(Update, make_tracker(o.clone(), "gc").in_set(BuiyLayoutStep::RemovedNodesGc));
app.add_systems(Update, make_tracker(o.clone(), "wmi").in_set(BuiyLayoutStep::WritingModeInherit));
app.add_systems(Update, make_tracker(o.clone(), "sync").in_set(BuiyLayoutStep::SyncStyles));
app.add_systems(Update, make_tracker(o.clone(), "cq_activate").in_set(BuiyLayoutStep::CqActivate));
app.add_systems(Update, make_tracker(o.clone(), "taffy").in_set(BuiyLayoutStep::TaffyCompute));
app.add_systems(Update, make_tracker(o.clone(), "cq_flip").in_set(BuiyLayoutStep::CqFlipCheck));
app.add_systems(Update, make_tracker(o.clone(), "cq_rerun").in_set(BuiyLayoutStep::CqFlipReRun));
app.add_systems(Update, make_tracker(o.clone(), "post_taffy").in_set(BuiyLayoutStep::PostTaffyOverrides));
app.add_systems(Update, make_tracker(o.clone(), "write").in_set(BuiyLayoutStep::WriteResolvedLayout));

app.update();

let observed = order.lock().unwrap().clone();
assert_eq!(
    observed,
    vec!["gc", "wmi", "sync", "cq_activate", "taffy", "cq_flip", "cq_rerun", "post_taffy", "write"],
    "BuiyLayoutStep sets did not run in declared order",
);
```

Update the file's top doc-comment from "8-step pipeline order" to "9-step pipeline order".

Run the test and confirm it FAILS (`WritingModeInherit` doesn't exist yet on `BuiyLayoutStep`).

- [ ] **Step 3: Add `BuiyLayoutStep::WritingModeInherit` variant**

In `pipeline.rs`:

```rust
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BuiyLayoutStep {
    /// Step 0 — drop despawned entities from `LayoutTree`.
    RemovedNodesGc,
    /// Pre-step-1 — populate `WritingModeResolved` by walking the
    /// hierarchy. Runs before `SyncStyles` so step 1 sees the effective
    /// inherited writing-mode for every entity.
    /// **Phase 4.**
    WritingModeInherit,
    /// Step 1 — translate changed Buiy components → `taffy::Style` and
    /// sync hierarchy.
    SyncStyles,
    // ... rest unchanged
}
```

Update the file-level doc comment from "Eight ordered sub-sets" to "Nine ordered sub-sets" and adjust the "Phase 1 wires all eight" line to mention the Phase 4 widening.

In `configure_pipeline`, insert `WritingModeInherit` between `RemovedNodesGc` and `SyncStyles`.

- [ ] **Step 4: Add the `inherit_writing_mode` system in `systems.rs`**

Append to `systems.rs`:

```rust
use std::collections::HashMap;

/// Pre-step-1 — populate `WritingModeResolved` for every `Node` entity
/// from the nearest ancestor with `WritingMode`, falling back to default
/// when no ancestor sets it.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.2.
///
/// Implementation:
/// 1. Resolve each entity's effective `WritingMode` by walking up the
///    `ChildOf` chain until a `WritingMode` is found (or the root is
///    reached, falling back to `default`).
/// 2. Memoize the resolution: each entity's effective value is computed
///    at most once per frame, even when many descendants share an
///    ancestor — total cost O(N), not O(N × depth).
/// 3. Compare against the entity's current `WritingModeResolved`. Only
///    `commands.insert(...)` when the value actually changes — avoids
///    cascading `Changed<WritingModeResolved>` to `sync_styles` every
///    frame, which would void the O(0) steady-state contract.
pub(super) fn inherit_writing_mode(
    mut commands: Commands,
    nodes: Query<(Entity, Option<&WritingModeResolved>), With<Node>>,
    wm_lookup: Query<&WritingMode>,
    parent_chain: Query<&ChildOf>,
) {
    let mut memo: HashMap<Entity, WritingMode> = HashMap::new();

    for (entity, current) in nodes.iter() {
        let effective = resolve_writing_mode(entity, &mut memo, &wm_lookup, &parent_chain);
        let new_resolved = WritingModeResolved::from_writing_mode(&effective);
        if current.copied() != Some(new_resolved) {
            commands.entity(entity).insert(new_resolved);
        }
    }
}

/// Walk up the `ChildOf` chain memoizing each ancestor's effective
/// `WritingMode`. Recursive on the parent path; depth bounded by the
/// hierarchy depth.
fn resolve_writing_mode(
    entity: Entity,
    memo: &mut HashMap<Entity, WritingMode>,
    wm_lookup: &Query<&WritingMode>,
    parent_chain: &Query<&ChildOf>,
) -> WritingMode {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let effective = if let Ok(wm) = wm_lookup.get(entity) {
        *wm
    } else if let Ok(p) = parent_chain.get(entity) {
        resolve_writing_mode(p.parent(), memo, wm_lookup, parent_chain)
    } else {
        WritingMode::default()
    };
    memo.insert(entity, effective);
    effective
}
```

**Why memoization matters:** without it, a chain of N descendants under one writing-mode-bearing root each walks up to that root, costing O(N × depth) total. Memoization caps this at O(N). The cost of an extra `HashMap` allocation per `inherit_writing_mode` call is dominated by the avoided redundant walks once trees get deeper than 3-4 levels. **Steady-state idempotence:** the `if current.copied() != Some(new_resolved)` guard means repeat frames with no `WritingMode` mutation produce zero `commands.insert(...)` calls, so `Changed<WritingModeResolved>` does not cascade — a hard requirement to preserve Phase 1's O(0) steady-state contract.

Drop the `#[allow(dead_code)]` on `WritingModeResolved::from_writing_mode` in `components.rs` now that it has a consumer.

- [ ] **Step 5: Wire the system into `LayoutPlugin::build`**

In `mod.rs`:

```rust
app.add_systems(
    Update,
    (
        systems::gc_removed_nodes.in_set(BuiyLayoutStep::RemovedNodesGc),
        systems::inherit_writing_mode.in_set(BuiyLayoutStep::WritingModeInherit),
        systems::sync_styles.in_set(BuiyLayoutStep::SyncStyles),
        systems::taffy_compute.in_set(BuiyLayoutStep::TaffyCompute),
        systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
    ),
);
```

- [ ] **Step 6: Run the pipeline-order test to verify it passes**

```sh
cargo test -p buiy_core --test layout_pipeline_order 2>&1 | tail -10
```

Expected: 9-step chain ordering pinned.

- [ ] **Step 7: Run lint + format + the full workspace test**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10 \
  && cargo test --workspace 2>&1 | tail -10
```

Expected: every existing test + the widened pipeline-order test pass.

- [ ] **Step 8: Commit**

```sh
git add crates/buiy_core/src/layout/pipeline.rs \
        crates/buiy_core/src/layout/systems.rs \
        crates/buiy_core/src/layout/mod.rs \
        crates/buiy_core/tests/layout_pipeline_order.rs
git commit -m "feat(buiy_core): add WritingModeInherit pipeline step + system

New BuiyLayoutStep::WritingModeInherit set runs between RemovedNodesGc
and SyncStyles, populating WritingModeResolved by single-level ancestor
lookup. Phase 4 v1 simplification: every-frame recompute over every
Node, single-level parent lookup. Multi-level ancestor walks and
Changed-driven incremental invalidation are documented limitations
revisited in v1.x if profiling flags them.

Pipeline test widens from 8 to 9 expected steps."
```

---

### Task 4: `LogicalEdges` + `LogicalBoxModel` + `LogicalInset` builder helpers

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` — add `LogicalEdges` value type with a `to_edges(&WritingMode)` method.
- Modify: `crates/buiy_core/src/layout/style.rs` — add `LogicalBoxModel` + `LogicalInset` builder structs.

**Note:** These three are **non-component, non-Bundle** ergonomic helpers. They live alongside `Style` because that's where author-side ergonomics live; they are NOT registered for reflection (they're transient builders, not stored data).

**Test surface:**

- `logical_edges_to_edges_horizontal_tb_ltr` — `inline-start → left, block-start → top`.
- `logical_edges_to_edges_vertical_rl_ltr` — `inline-start → top, block-start → right`.
- `logical_edges_to_edges_vertical_lr_ltr` — `inline-start → top, block-start → left`.
- `logical_box_model_inline_size_under_horizontal_tb_is_width` — `LogicalBoxModel { inline_size: Px(100), .. }.to_box_model(&horizontal_tb)` produces `BoxModel { width: Px(100), .. }`.
- `logical_box_model_inline_size_under_vertical_rl_is_height` — same input, vertical-rl produces `height: Px(100)`.
- `logical_inset_inline_start_under_vertical_rl_is_top` — `LogicalInset { inline_start: Px(8), .. }.to_inset(&vertical_rl)` produces `Inset { top: Px(8), .. }`.

**Resolved API shape:** `LogicalEdges::to_edges(&self, mode: WritingModeKind, direction: Direction) -> Edges` — takes the two enum values directly, lives in `types.rs`. Avoids a backward `types → components` dependency. Callers that have a `&WritingMode` pass `wm.mode, wm.direction`.

- [ ] **Step 1: Write the failing tests**

Append to `types.rs::tests`:

```rust
    #[test]
    fn logical_edges_to_edges_horizontal_tb_ltr() {
        let logical = LogicalEdges {
            inline_start: Length::Px(1.0),
            inline_end: Length::Px(2.0),
            block_start: Length::Px(3.0),
            block_end: Length::Px(4.0),
        };
        let physical = logical.to_edges(WritingModeKind::HorizontalTb, Direction::Ltr);
        assert_eq!(physical.left, Length::Px(1.0));
        assert_eq!(physical.right, Length::Px(2.0));
        assert_eq!(physical.top, Length::Px(3.0));
        assert_eq!(physical.bottom, Length::Px(4.0));
    }

    #[test]
    fn logical_edges_to_edges_vertical_rl_ltr() {
        let logical = LogicalEdges {
            inline_start: Length::Px(1.0),
            inline_end: Length::Px(2.0),
            block_start: Length::Px(3.0),
            block_end: Length::Px(4.0),
        };
        let physical = logical.to_edges(WritingModeKind::VerticalRl, Direction::Ltr);
        // vertical-rl + ltr: inline-start = top, block-start = right
        assert_eq!(physical.top, Length::Px(1.0));
        assert_eq!(physical.bottom, Length::Px(2.0));
        assert_eq!(physical.right, Length::Px(3.0));
        assert_eq!(physical.left, Length::Px(4.0));
    }

    #[test]
    fn logical_edges_to_edges_vertical_lr_ltr() {
        let logical = LogicalEdges {
            inline_start: Length::Px(1.0),
            inline_end: Length::Px(2.0),
            block_start: Length::Px(3.0),
            block_end: Length::Px(4.0),
        };
        let physical = logical.to_edges(WritingModeKind::VerticalLr, Direction::Ltr);
        // vertical-lr + ltr: inline-start = top, block-start = left
        assert_eq!(physical.top, Length::Px(1.0));
        assert_eq!(physical.bottom, Length::Px(2.0));
        assert_eq!(physical.left, Length::Px(3.0));
        assert_eq!(physical.right, Length::Px(4.0));
    }
```

Append to `style.rs::tests`:

```rust
    #[test]
    fn logical_box_model_inline_size_under_horizontal_tb_is_width() {
        let logical = LogicalBoxModel {
            inline_size: Sizing::Length(Length::Px(100.0)),
            block_size: Sizing::Length(Length::Px(50.0)),
            ..Default::default()
        };
        let wm = WritingMode::default(); // horizontal-tb + ltr
        let bm = logical.to_box_model(&wm);
        assert_eq!(bm.width, Sizing::Length(Length::Px(100.0)));
        assert_eq!(bm.height, Sizing::Length(Length::Px(50.0)));
    }

    #[test]
    fn logical_box_model_inline_size_under_vertical_rl_is_height() {
        let logical = LogicalBoxModel {
            inline_size: Sizing::Length(Length::Px(100.0)),
            block_size: Sizing::Length(Length::Px(50.0)),
            ..Default::default()
        };
        let wm = WritingMode {
            mode: WritingModeKind::VerticalRl,
            ..Default::default()
        };
        let bm = logical.to_box_model(&wm);
        assert_eq!(bm.height, Sizing::Length(Length::Px(100.0)));
        assert_eq!(bm.width, Sizing::Length(Length::Px(50.0)));
    }

    #[test]
    fn logical_inset_inline_start_under_vertical_rl_is_top() {
        let logical = LogicalInset {
            inline_start: Sizing::Length(Length::Px(8.0)),
            ..Default::default()
        };
        let wm = WritingMode {
            mode: WritingModeKind::VerticalRl,
            ..Default::default()
        };
        let inset = logical.to_inset(&wm);
        assert_eq!(inset.top, Sizing::Length(Length::Px(8.0)));
    }
```

- [ ] **Step 2: Run the failing tests**

Expected: `LogicalEdges`, `LogicalBoxModel`, `LogicalInset`, `to_edges`, `to_box_model`, `to_inset` not found.

- [ ] **Step 3: Add `LogicalEdges` value type + `to_edges` to `types.rs`**

Append to `types.rs`:

```rust
/// Logical-edge values (writing-mode-aware). Construct + call `to_edges`
/// to get a physical `Edges` for layout consumption.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 4 +
/// docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.3.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct LogicalEdges {
    pub inline_start: Length,
    pub inline_end: Length,
    pub block_start: Length,
    pub block_end: Length,
}

impl LogicalEdges {
    /// Translate to physical `Edges` honoring the given writing-mode + direction.
    /// 6-row mapping table:
    /// - horizontal-tb + ltr: inline-start = left, block-start = top
    /// - horizontal-tb + rtl: inline-start = right, block-start = top
    /// - vertical-rl + ltr: inline-start = top, block-start = right
    /// - vertical-rl + rtl: inline-start = bottom, block-start = right
    /// - vertical-lr + ltr: inline-start = top, block-start = left
    /// - vertical-lr + rtl: inline-start = bottom, block-start = left
    ///
    /// Sideways modes (`SidewaysRl` / `SidewaysLr`) are normalized to
    /// their non-sideways vertical equivalents — glyph rotation lives
    /// in `buiy-text-rendering-design`, layout treats them identically.
    pub fn to_edges(&self, mode: WritingModeKind, direction: Direction) -> Edges {
        use WritingModeKind::*;
        let mode = match mode {
            SidewaysRl => VerticalRl,
            SidewaysLr => VerticalLr,
            other => other,
        };
        match (mode, direction) {
            (HorizontalTb, Direction::Ltr) => Edges {
                left: self.inline_start,
                right: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (HorizontalTb, Direction::Rtl) => Edges {
                right: self.inline_start,
                left: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (VerticalRl, Direction::Ltr) => Edges {
                top: self.inline_start,
                bottom: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalRl, Direction::Rtl) => Edges {
                bottom: self.inline_start,
                top: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalLr, Direction::Ltr) => Edges {
                top: self.inline_start,
                bottom: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            (VerticalLr, Direction::Rtl) => Edges {
                bottom: self.inline_start,
                top: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            // Sideways modes were normalized above; this is unreachable.
            (SidewaysRl, _) | (SidewaysLr, _) => unreachable!("sideways normalized to vertical"),
        }
    }
}
```

This signature takes `WritingModeKind` and `Direction` directly (not `&WritingMode`), which keeps the `types.rs → components.rs` dependency direction one-way. Callers in `style.rs` (`LogicalBoxModel`, `LogicalInset`) hold a `&WritingMode` and pass `wm.mode, wm.direction` through.

- [ ] **Step 4: Add `LogicalBoxModel` + `LogicalInset` builders in `style.rs`**

Append to `style.rs`:

```rust
/// Builder for the box-model surface using logical (writing-mode-aware)
/// dimensions. **Not stored** — call `.to_box_model(&WritingMode)` to
/// produce a `BoxModel` and pass that into your `Style`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 4.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct LogicalBoxModel {
    pub inline_size: Sizing,
    pub block_size: Sizing,
    pub min_inline_size: Sizing,
    pub min_block_size: Sizing,
    pub max_inline_size: Sizing,
    pub max_block_size: Sizing,
    pub padding: LogicalEdges,
    pub margin: LogicalEdges,
    pub border: LogicalEdges,
    pub box_sizing: BoxSizing,
    pub aspect_ratio: Option<AspectRatio>,
}

impl LogicalBoxModel {
    /// Translate to a physical `BoxModel` honoring the given writing-mode.
    /// Vertical modes swap inline ↔ block onto width ↔ height; physical
    /// edges follow the LogicalEdges 6-row table.
    pub fn to_box_model(&self, wm: &WritingMode) -> BoxModel {
        let is_vertical = matches!(
            wm.mode,
            WritingModeKind::VerticalRl
                | WritingModeKind::VerticalLr
                | WritingModeKind::SidewaysRl
                | WritingModeKind::SidewaysLr
        );
        let (width, height) = if is_vertical {
            (self.block_size, self.inline_size)
        } else {
            (self.inline_size, self.block_size)
        };
        let (min_width, min_height) = if is_vertical {
            (self.min_block_size, self.min_inline_size)
        } else {
            (self.min_inline_size, self.min_block_size)
        };
        let (max_width, max_height) = if is_vertical {
            (self.max_block_size, self.max_inline_size)
        } else {
            (self.max_inline_size, self.max_block_size)
        };
        BoxModel {
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            padding: self.padding.to_edges(wm.mode, wm.direction),
            margin: self.margin.to_edges(wm.mode, wm.direction),
            border: self.border.to_edges(wm.mode, wm.direction),
            box_sizing: self.box_sizing,
            aspect_ratio: self.aspect_ratio,
        }
    }
}

/// Builder for the inset surface using logical (writing-mode-aware)
/// edges. **Not stored** — call `.to_inset(&WritingMode)` to produce an
/// `Inset`.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct LogicalInset {
    pub inline_start: Sizing,
    pub inline_end: Sizing,
    pub block_start: Sizing,
    pub block_end: Sizing,
}

impl LogicalInset {
    pub fn to_inset(&self, wm: &WritingMode) -> Inset {
        // Inset uses Sizing (not Length), so we duplicate the 6-row
        // mapping rather than reusing LogicalEdges::to_edges.
        use WritingModeKind::*;
        let mode = match wm.mode {
            SidewaysRl => VerticalRl,
            SidewaysLr => VerticalLr,
            other => other,
        };
        match (mode, wm.direction) {
            (HorizontalTb, Direction::Ltr) => Inset {
                left: self.inline_start,
                right: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (HorizontalTb, Direction::Rtl) => Inset {
                right: self.inline_start,
                left: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (VerticalRl, Direction::Ltr) => Inset {
                top: self.inline_start,
                bottom: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalRl, Direction::Rtl) => Inset {
                bottom: self.inline_start,
                top: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalLr, Direction::Ltr) => Inset {
                top: self.inline_start,
                bottom: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            (VerticalLr, Direction::Rtl) => Inset {
                bottom: self.inline_start,
                top: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            (SidewaysRl, _) | (SidewaysLr, _) => unreachable!("sideways normalized"),
        }
    }
}
```

Update the `use` at the top of `style.rs` to include `WritingMode`, `WritingModeKind`, `Direction`, `LogicalEdges`, `Inset`.

- [ ] **Step 5: Run all tests**

```sh
cargo test -p buiy_core --lib 2>&1 | tail -10
```

Expected: types::tests + style::tests grow with the new tests; all pass.

- [ ] **Step 6: Run lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 7: Commit**

```sh
git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/style.rs
git commit -m "feat(buiy_core): add LogicalEdges + LogicalBoxModel + LogicalInset builders

Non-component, non-Bundle author-ergonomic structs. Each carries logical
(writing-mode-aware) edges or dimensions and emits the corresponding
physical type via .to_edges / .to_box_model / .to_inset taking a
&WritingMode. The 6-row mapping table from the spec is exercised by the
new unit tests; vertical-rl / vertical-lr swap inline <-> block onto
height <-> width."
```

---

### Task 5: Translate writing-mode + widen `sync_styles` (atomic)

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` — extend `StyleView` with `writing_mode_resolved`; map `Direction` to `taffy::Direction`; wire sideways-* warn-once gate.
- Modify: `crates/buiy_core/src/layout/systems.rs` — widen `sync_styles` query and trigger filter.

**Atomic** because `StyleView` is the bridge — the same atomicity reasoning as Phase 2 / Phase 3 Task 5.

**Test surface:**

- `translate_direction_rtl_to_taffy_rtl` — `WritingModeResolved.direction = Rtl` produces `taffy.direction == taffy::Direction::Rtl`.
- `translate_direction_ltr_to_taffy_ltr` — default produces `taffy.direction == taffy::Direction::Ltr`.

(The sideways-* warn-once gate has no observable Taffy-side effect; it's exercised by the integration test in Task 8.)

- [ ] **Step 1: Write the failing tests**

Append to `translate::tests`:

```rust
    #[test]
    fn translate_direction_rtl_to_taffy_rtl() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let wmr = WritingModeResolved {
            direction: Direction::Rtl,
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
            writing_mode_resolved: &wmr,
        });
        assert!(matches!(taffy.direction, taffy::Direction::Rtl));
    }

    #[test]
    fn translate_direction_ltr_to_taffy_ltr() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let wmr = WritingModeResolved::default();
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
            writing_mode_resolved: &wmr,
        });
        assert!(matches!(taffy.direction, taffy::Direction::Ltr));
    }
```

The existing translate tests (5+5+5+5+5+5+1+5 = 31 tests as of HEAD) need `writing_mode_resolved: &WritingModeResolved::default()` added to their `StyleView { ... }` literals. **Enumerate the sites** during implementation: the 6 Phase 1+2+3 sites that already received drive-bys + the 5 Phase 3 grid-test sites = 11 tests need the new field. Use `grep -n "StyleView {" crates/buiy_core/src/layout/translate.rs` to count.

- [ ] **Step 2: Run tests to verify they fail**

Compile errors — `StyleView` doesn't have `writing_mode_resolved`.

- [ ] **Step 3: Extend `StyleView`**

In `translate.rs`:

```rust
use super::components::{
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
    WritingMode, WritingModeResolved,
};
use super::types::{
    AlignContent, AlignItems, BoxSizing, Direction, Edges, FlexAxis, FlexWrap, GridAreas,
    GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems, Length, NamedArea, OverflowMode,
    PositionKind, RepeatCount, ScrollbarWidth, Sizing, TextOrientation, TrackSize, UnicodeBidi,
    WritingModeKind,
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
    pub parent_areas: Option<&'a GridAreas>,
    pub writing_mode_resolved: &'a WritingModeResolved,
}
```

- [ ] **Step 4: Wire `Direction` into `taffy::Style.direction`**

In `style_to_taffy`, after the existing `let mut s = taffy::Style { ... };` block, set the `direction` field:

```rust
    s.direction = match view.writing_mode_resolved.direction {
        Direction::Ltr => taffy::Direction::Ltr,
        Direction::Rtl => taffy::Direction::Rtl,
    };

    // Sideways-* warn-once: layout treats them as their non-sideways
    // vertical equivalents; the glyph-rotation pass owns rotation.
    if matches!(
        view.writing_mode_resolved.mode,
        WritingModeKind::SidewaysRl | WritingModeKind::SidewaysLr
    ) {
        warn_once_sideways_layout_fallback();
    }
```

- [ ] **Step 5: Add `warn_once_sideways_layout_fallback`**

Append to the warn-once helpers section in `translate.rs`:

```rust
static WARNED_SIDEWAYS_FALLBACK: AtomicBool = AtomicBool::new(false);

fn warn_once_sideways_layout_fallback() {
    if !WARNED_SIDEWAYS_FALLBACK.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: WritingModeKind::Sideways{{Rl,Lr}} glyph rotation lives in \
             buiy-text-rendering-design; layout treats them as VerticalRl / \
             VerticalLr (warned once)"
        );
    }
}
```

- [ ] **Step 6: Update the 11 existing test sites**

Each existing test that constructs `StyleView { ... }` adds:
```rust
let writing_mode_resolved = WritingModeResolved::default();
// inside StyleView { ... }:
writing_mode_resolved: &writing_mode_resolved,
```

The 11 sites (verify with `grep -n "StyleView {" crates/buiy_core/src/layout/translate.rs`):
1. `translate_default_components_to_taffy_default`
2. `translate_flex_row_with_dimensions`
3. `translate_position_absolute_emits_absolute_with_inset`
4. `translate_flex_item_basis_grow_shrink`
5. `translate_overflow_modes_to_taffy` (loop body)
6. `translate_scrollbar_width_to_taffy_f32` (loop body)
7. `translate_display_grid_to_taffy_grid` (loop body)
8. `translate_grid_template_columns_to_taffy`
9. `translate_grid_repeat_to_taffy`
10. `translate_grid_line_start_end_to_taffy`
11. `translate_grid_line_area_resolved_via_parent_areas`

- [ ] **Step 7: Widen `sync_styles` query + filter in `systems.rs`**

The query gains `&WritingModeResolved` (read), the trigger filter gains `Changed<WritingMode>` and `Changed<WritingModeResolved>`.

```rust
use super::components::{
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
    WritingMode, WritingModeResolved,
};

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
            &WritingModeResolved,
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
                Changed<WritingMode>,
                Changed<WritingModeResolved>,
                Changed<Children>,
                Changed<ChildOf>,
            )>,
        ),
    >,
    parent_grid_lookup: Query<&GridParams>,
) {
    // ... body widens to destructure writing_mode_resolved from the query
    // tuple and pass &writing_mode_resolved through to StyleView.
}
```

- [ ] **Step 8: Run the full check**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test --workspace 2>&1 | tail -15
```

Expected: every test passes. The `Changed<ScrollOffset>` / `Changed<ScrollSnapItem>` exclusion (Phase 2 invariant) is preserved.

- [ ] **Step 9: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/systems.rs
git commit -m "feat(buiy_core): wire WritingMode to Taffy direction + widen sync_styles

Atomic: extends StyleView with writing_mode_resolved; populates
taffy::Style.direction from WritingModeResolved.direction; sideways-*
modes hit a warn-once gate naming buiy-text-rendering-design as the
future owner.

sync_styles' Or filter widens with Changed<WritingMode> and
Changed<WritingModeResolved>. Phase 2 invariant intact:
Changed<ScrollOffset> / Changed<ScrollSnapItem> remain excluded.

Note: this is one commit because StyleView is the bridge between
translate.rs and systems.rs - splitting would break the lib build
between commits."
```

---

### Task 6: Style Bundle extension + writing-mode fluent setters

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` — add `WritingMode` to Bundle; add fluent setters.

**Test surface:**

- Existing `struct_literal_and_fluent_produce_identical_components` extends to include `WritingMode`.
- `default_style_inserts_every_decomposed_component` extends to assert `WritingMode` insertion.
- `writing_mode_setter_overrides` — `.writing_mode(WritingMode { mode: VerticalRl, ..default() })` puts the right component on the entity.
- `rtl_setter_flips_direction` — `.rtl()` produces `WritingMode { direction: Rtl, ..default() }`.

- [ ] **Step 1: Write the failing tests**

In `style.rs::tests`, extend `spawn_and_extract` to include `WritingMode`, update `default_style_inserts_every_decomposed_component`, and add the two new tests.

- [ ] **Step 2: Run tests to verify they fail**

- [ ] **Step 3: Add `WritingMode` to the Style Bundle**

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
    pub writing_mode: WritingMode,
}
```

- [ ] **Step 4: Add the writing-mode fluent setters**

Append `// ---- WritingMode ----` section to `impl Style`:

```rust
    pub fn writing_mode(mut self, wm: WritingMode) -> Self {
        self.writing_mode = wm;
        self
    }

    pub fn writing_mode_kind(mut self, kind: WritingModeKind) -> Self {
        self.writing_mode.mode = kind;
        self
    }

    pub fn direction(mut self, d: Direction) -> Self {
        self.writing_mode.direction = d;
        self
    }

    pub fn ltr(mut self) -> Self {
        self.writing_mode.direction = Direction::Ltr;
        self
    }

    pub fn rtl(mut self) -> Self {
        self.writing_mode.direction = Direction::Rtl;
        self
    }

    pub fn text_orientation(mut self, t: TextOrientation) -> Self {
        self.writing_mode.text_orientation = t;
        self
    }

    pub fn unicode_bidi(mut self, u: UnicodeBidi) -> Self {
        self.writing_mode.unicode_bidi = u;
        self
    }
```

- [ ] **Step 5: Run tests + lint + commit**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && cargo test -p buiy_core 2>&1 | tail -15
```

Expected: clean.

```sh
git add crates/buiy_core/src/layout/style.rs
git commit -m "feat(buiy_core): extend Style Bundle with WritingMode + fluent setters

7 new setters: .writing_mode, .writing_mode_kind, .direction, .ltr,
.rtl, .text_orientation, .unicode_bidi. WritingModeResolved stays out
of the Bundle (private cache, populated by inherit_writing_mode)."
```

---

### Task 7: Register types + re-exports

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` — register WritingMode + WritingModeResolved + 4 enums.
- Modify: `crates/buiy_core/src/lib.rs` — re-export from buiy_core's public surface.
- Modify: `crates/buiy/src/lib.rs` — re-export from the facade.

- [ ] **Step 1: Update `LayoutPlugin::build` reflection registrations**

Add `register_type::<WritingMode>()`, `register_type::<WritingModeResolved>()`, plus the 4 value-type registrations (`WritingModeKind`, `Direction`, `TextOrientation`, `UnicodeBidi`, `LogicalEdges`).

- [ ] **Step 2: Update `crates/buiy_core/src/layout/mod.rs` re-exports**

`pub use components::{..., WritingMode, WritingModeResolved};`
`pub use types::{..., Direction, LogicalEdges, TextOrientation, UnicodeBidi, WritingModeKind};`

(The `LogicalBoxModel` / `LogicalInset` builder helpers are also re-exported from `style`. Decide whether they're public via the `style` re-export already.)

- [ ] **Step 3: Update `crates/buiy_core/src/lib.rs` and `crates/buiy/src/lib.rs`**

Mirror the new re-exports. Alphabetical ordering.

- [ ] **Step 4: Run the full workspace test**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps \
  && cargo test --workspace 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```sh
git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
git commit -m "feat(buiy_core, buiy): register and re-export Phase 4 layout types

WritingMode + WritingModeResolved registered for reflection; new value
types (WritingModeKind, Direction, TextOrientation, UnicodeBidi,
LogicalEdges) and builders (LogicalBoxModel, LogicalInset) re-exported
from buiy_core and the buiy facade alongside Phases 1-3 surface."
```

---

### Task 8: Integration tests through the full pipeline

**Files:**
- Test: `crates/buiy_core/tests/layout_writing_modes.rs` (new) — 5 fixtures.

**Test surface:**

1. `direction_rtl_flips_flex_row_children` — `Display::Flex(Row)` parent with `Direction::Rtl`; children's x positions are inverted compared to `Direction::Ltr`.
2. `vertical_rl_swaps_inline_block_via_logical_box_model` — `LogicalBoxModel { inline_size: 100px, .. }` under `WritingMode::VerticalRl`; spawn through Style; assert ResolvedLayout.size matches the swap.
3. `inheritance_propagates_writing_mode_to_descendant` — set `WritingMode::VerticalRl` on parent; child has no explicit `WritingMode`; after `app.update()`, child's `WritingModeResolved.mode == VerticalRl`.
4. `logical_edges_translate_under_vertical_rl` — covered by Task 4 unit tests (no integration test needed beyond unit-level).
5. `sideways_rl_falls_back_to_vertical_rl_layout` — `WritingMode::SidewaysRl` produces the same layout as `WritingMode::VerticalRl`; no panic; warn-once fires (observable contract is "no panic + warn", not asserted).

- [ ] **Step 1: Write the failing test file**

Create `crates/buiy_core/tests/layout_writing_modes.rs` using the integration-test pattern from Phase 2 / Phase 3 (`MinimalPlugins + LayoutPlugin`, spawn entities, `app.update()`, read `ResolvedLayout` per child).

```rust
//! Integration tests for writing-mode through the full LayoutPlugin pipeline.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    Direction, Display, LayoutPlugin, Length, LogicalBoxModel, Sizing, Style, WritingMode,
    WritingModeKind, WritingModeResolved,
};

#[test]
fn direction_rtl_flips_flex_row_children() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent_style = Style::default().flex_row().width_px(300.0).height_px(50.0).rtl();
    let parent = app.world_mut().spawn((parent_style, Node)).id();
    let mut children: Vec<Entity> = Vec::new();
    for _ in 0..3 {
        let c = app
            .world_mut()
            .spawn((Style::default().width_px(100.0).height_px(50.0), Node))
            .id();
        children.push(c);
    }
    app.world_mut().entity_mut(parent).add_children(&children);
    app.update();

    let r0 = app.world().get::<ResolvedLayout>(children[0]).expect("c0").position;
    let r2 = app.world().get::<ResolvedLayout>(children[2]).expect("c2").position;
    // Under RTL, the first child sits at the right edge, the last at the left.
    assert!(r0.x > r2.x, "rtl should put child 0 right of child 2 (got {} vs {})", r0.x, r2.x);
}

#[test]
fn vertical_rl_swaps_inline_block_via_logical_box_model() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let wm = WritingMode {
        mode: WritingModeKind::VerticalRl,
        ..Default::default()
    };
    let bm = LogicalBoxModel {
        inline_size: Sizing::Length(Length::Px(100.0)),
        block_size: Sizing::Length(Length::Px(50.0)),
        ..Default::default()
    }
    .to_box_model(&wm);

    let entity = app
        .world_mut()
        .spawn((
            Style {
                box_model: bm,
                writing_mode: wm,
                ..Default::default()
            },
            Node,
        ))
        .id();

    app.update();

    let rl = app.world().get::<ResolvedLayout>(entity).expect("layout");
    // inline-size 100, block-size 50 under vertical-rl → height = 100, width = 50.
    assert!((rl.size.x - 50.0).abs() < 0.5, "width = {}", rl.size.x);
    assert!((rl.size.y - 100.0).abs() < 0.5, "height = {}", rl.size.y);
}

#[test]
fn inheritance_propagates_writing_mode_to_descendant() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let parent = app
        .world_mut()
        .spawn((
            Style::default().writing_mode(WritingMode {
                mode: WritingModeKind::VerticalRl,
                ..Default::default()
            }),
            Node,
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Style::default(), Node))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
    app.update();

    let resolved = app
        .world()
        .get::<WritingModeResolved>(child)
        .expect("child should have WritingModeResolved after inherit pass");
    assert_eq!(resolved.mode, WritingModeKind::VerticalRl);
}

#[test]
fn sideways_rl_falls_back_to_vertical_rl_layout() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    let wm = WritingMode {
        mode: WritingModeKind::SidewaysRl,
        ..Default::default()
    };
    let bm = LogicalBoxModel {
        inline_size: Sizing::Length(Length::Px(100.0)),
        block_size: Sizing::Length(Length::Px(50.0)),
        ..Default::default()
    }
    .to_box_model(&wm);

    let entity = app
        .world_mut()
        .spawn((
            Style {
                box_model: bm,
                writing_mode: wm,
                ..Default::default()
            },
            Node,
        ))
        .id();
    app.update();

    let rl = app.world().get::<ResolvedLayout>(entity).expect("layout");
    // sideways-rl falls back to vertical-rl layout: inline 100 → height, block 50 → width.
    assert!((rl.size.x - 50.0).abs() < 0.5, "sideways-rl width = {}", rl.size.x);
    assert!((rl.size.y - 100.0).abs() < 0.5, "sideways-rl height = {}", rl.size.y);
}
```

- [ ] **Step 2: Run the tests**

```sh
cargo test -p buiy_core --test layout_writing_modes 2>&1 | tail -15
```

Expected: 4 tests pass.

- [ ] **Step 3: Run the full workspace test**

```sh
cargo test --workspace 2>&1 | tail -10
```

Expected: every test passes.

- [ ] **Step 4: Lint + format**

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 5: Commit**

```sh
git add crates/buiy_core/tests/layout_writing_modes.rs
git commit -m "test(buiy_core): writing-mode through the full pipeline

Integration tests pin: rtl flips flex-row child order; vertical-rl
swaps inline/block dimensions via LogicalBoxModel; inheritance pass
propagates parent's WritingMode to descendants' WritingModeResolved;
sideways-rl falls back to vertical-rl layout (warn-once fires
observably but is not asserted)."
```

---

### Task 9: CHANGELOG + branch-level review + PR

**Files:**
- Modify: `CHANGELOG.md` — `### Added` and `### Changed` entries.
- Modify: `docs/README.md` — flip Phase 4 plan entry from `[active]` to `[landed]` post-merge.

- [ ] **Step 1: Append CHANGELOG entries**

Add under `[Unreleased]`:

```markdown
- **Layout writing modes (Phase 4 of the layout migration).**
  - `WritingMode` component (joins `Style`'s Bundle): `mode`,
    `direction`, `text_orientation`, `unicode_bidi`.
  - `WritingModeResolved` private cache component, populated by the new
    `BuiyLayoutStep::WritingModeInherit` pipeline step (single-level
    parent lookup; deeper hierarchies revisited if profiling flags it).
  - 4 supporting enums: `WritingModeKind` (HorizontalTb / VerticalRl /
    VerticalLr / SidewaysRl / SidewaysLr), `Direction` (Ltr / Rtl),
    `TextOrientation` (Mixed / Upright / Sideways), `UnicodeBidi`
    (Normal / Embed / Isolate / BidiOverride / IsolateOverride / Plaintext).
  - `LogicalEdges` value type with `to_edges(WritingModeKind, Direction)`.
  - `LogicalBoxModel` and `LogicalInset` author-ergonomic builder
    structs (non-component, non-Bundle) with
    `.to_box_model(&WritingMode)` and `.to_inset(&WritingMode)` methods.
    Vertical-mode swap inline ↔ block onto width ↔ height.
  - 7 fluent setters on `Style`: `.writing_mode(_)`,
    `.writing_mode_kind(_)`, `.direction(_)`, `.ltr()`, `.rtl()`,
    `.text_orientation(_)`, `.unicode_bidi(_)`.
  - `WritingMode` + `WritingModeResolved` + 5 value types registered for
    reflection in `LayoutPlugin`.

### Changed

- Layout pipeline gains a 9th step. `BuiyLayoutStep::WritingModeInherit`
  is inserted between `RemovedNodesGc` and `SyncStyles`, populating
  `WritingModeResolved` before the translation pass reads it.
- `sync_styles`'s `Or<>` trigger filter widens with `Changed<WritingMode>`
  and `Changed<WritingModeResolved>`. Phase 2 invariant intact:
  `Changed<ScrollOffset>` / `Changed<ScrollSnapItem>` remain excluded.
- `WritingModeKind::Sideways{Rl,Lr}` emit one `warn!` per session and
  fall back to their non-sideways vertical equivalent for layout
  purposes. Glyph rotation is `buiy-text-rendering-design`'s concern.
- `Direction::Rtl` now flows through `taffy::Style.direction`, so
  `Display::Flex(Row)` mirrors children under RTL.
```

- [ ] **Step 2: Add the plan entry to `docs/README.md`** (if not already added during plan-write commit)

Verify the `Layout > Plans` section includes:
```markdown
- [Buiy layout writing modes](plans/2026-05-10-buiy-layout-writing-modes.md) — Phase 4: `WritingMode` + `WritingModeResolved`, inheritance pass, `LogicalBoxModel` / `LogicalInset` builders, sideways-* warn-once stubs. `[active]`
```

- [ ] **Step 3: Run the full check**

```sh
cargo fmt --all -- --check \
  && cargo clippy --workspace --all-targets -- -D warnings \
  && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps \
  && cargo test --workspace
```

- [ ] **Step 4: Commit**

```sh
git add CHANGELOG.md docs/README.md
git commit -m "docs(layout): changelog + index entries for Phase 4 writing-modes"
```

- [ ] **Step 5: Final branch-level review**

Dispatch one final reviewer subagent that audits the entire branch diff against Phase 4 spec § 2 and architecture.md §§ 1.2 + 3, plus Phase 1+2+3 conventions. Address any BLOCKER findings before pushing.

- [ ] **Step 6: Push and open PR**

```sh
git push -u origin claude/v01-layout-writing-modes
gh pr create --title "Phase 4: layout writing modes" --body "$(cat <<'EOF'
## Summary
- `WritingMode` + `WritingModeResolved` ship the writing-mode surface; sideways-* warn once and fall back to vertical-rl/lr.
- New `BuiyLayoutStep::WritingModeInherit` pipeline step populates the resolved cache before `SyncStyles`.
- `LogicalBoxModel` + `LogicalInset` ergonomic builders translate logical → physical at construct time.
- `Direction::Rtl` wires through `taffy::Style.direction`, mirroring flex children under RTL.

## Test plan
- [ ] `cargo test --workspace` green
- [ ] Pipeline order test asserts 9 steps
- [ ] CI: Lint / Doc / Deny / Test on ubuntu/macos/windows

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 7: Merge when green; flip plan status to `[landed]`**

---

## Self-review (run before dispatching subagent reviewers)

1. **Spec coverage.** Every § 2 requirement maps to a row in the Coverage map.
2. **Placeholder scan.** No "TBD" / "implement later". Every step contains the actual code.
3. **Type consistency.** `WritingMode`, `WritingModeResolved`, `WritingModeKind`, `Direction`, `TextOrientation`, `UnicodeBidi`, `LogicalEdges`, `LogicalBoxModel`, `LogicalInset` consistent across tasks.
4. **Cross-task atomicity.** Task 5 atomic (translate + systems). Task 1 alone is OK (no cross-file lint break — the new enums are not referenced by translate.rs until Task 5).
5. **Decomposed-only convention.** `WritingMode` joins Style Bundle. `WritingModeResolved` is private and does NOT.
6. **Reflection convention.** Components derive `#[reflect(Component, Default)]`; non-component types derive `#[derive(Reflect, ...)]` only.
7. **Phase boundary.** No Phase 5+ items snuck in. `ContainingBlock` cache is explicitly deferred.
8. **Test discipline.** Each task: failing test → confirm fail → implement → confirm pass → fmt/clippy → commit.
9. **Mechanical rigor.** Every commit step shows `cargo fmt --check && cargo clippy ... -D warnings` before `git commit`.
10. **Scope creep.** Phase 4 does NOT add: vertical-mode Taffy axis-swap (deferred), glyph rotation (text-rendering), bidi resolution (i18n), ContainingBlock cache (Phase 6).
