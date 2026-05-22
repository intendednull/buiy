# Buiy layout — Phase 7: sticky positioning + table stub + multicol stub

**Date:** 2026-05-22
**Revision:** v2 (post 3-agent parallel review — see "Plan v2 revisions" section below)
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/display-and-positioning.md`](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md) § 1.2 (table layout status), § 2.3 (sticky positioning) + [`flex-and-grid.md`](../specs/2026-05-08-buiy-layout-design/flex-and-grid.md) § 3 (multi-column) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (sub-passes 6a/6b/6c), § 6 (error model).
**Supersedes:** none (graduates from empty sub-passes 6a/6b/6c stubs declared in Phase 1 `BuiyLayoutStep::PostTaffyOverrides` and partially populated in Phase 6 by sub-pass 6d).

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking.

**Goal:** Land the remaining three sub-passes of `BuiyLayoutStep::PostTaffyOverrides`: 6a (sticky-offset, full implementation), 6b (table-layout, no-op stub with warn-once-per-session per spec § 1.2), and 6c (multicol-pack, no-op stub with warn-once-per-session per `flex-and-grid.md` § 3.2). Add the `MultiColumn` component (tier-E API surface) and its supporting types so authors can write multicol code that compiles even though the algorithm is deferred to v1.x. Refactor the Phase-6 `AnchorOverrides` resource into a *shared* `PostTaffyPositionOverrides` map written to by all four sub-passes; `anchor_resolution` (6d) reads from this map when looking up an anchor target's position so anchors that target a sticky-displaced element produce correct dependents — closing the architectural quirk tracked in [`follow-ups.md` "Anchor positioning — anchor target IS sticky/table/multicol"](follow-ups.md). All warn-once messages introduced in this phase use the canonical *per-session* dedup pattern from spec § 6 (`LayoutWarnedOnceSession` HashSet resource cleared on `BuiyExit`); Phase 6's per-frame variant for anchor errors is preserved unchanged.

**Architecture (3 sentences):**
1. **Shared override map under one clear-system.** `AnchorOverrides` is renamed to `PostTaffyPositionOverrides` (same `HashMap<Entity, Vec2>` shape, same per-frame-cleared semantics) and lifted from "anchor-only writer" to "any sub-pass writer." A new tiny system `clear_post_taffy_overrides` runs first in the `PostTaffyOverrides` set and is the sole site that clears `by_entity`. Sub-passes 6a → 6b → 6c → 6d chain after it via `.chain()` in `mod.rs`; the per-frame clear of `LayoutAnchorWarnedThisFrame.set` remains inside `anchor_resolution` (anchor-specific, unchanged by Phase 7). `write_resolved_layout` (step 7) is unchanged in shape — it already reads the override map; only the resource name in its signature changes.
2. **Sticky-offset as a pure post-Taffy transform.** Sub-pass 6a `sticky_offset` queries entities with `Position { kind: PositionKind::Sticky, inset, .. }`; for each, walks `ChildOf` to find the nearest scroll-container ancestor (an entity whose `Overflow.is_scroll_container()` is true — same helper Phase 2 added). If no scroll container is in scope the entity behaves as `Relative` (no displacement, no warn — spec § 2.1 "falls back to parent's content box outside the sticky range" treats absence-of-context as a valid no-op). Otherwise the pass reads the sticky entity's natural box from `tree.tree.layout()` (same Taffy-direct read pattern as Phase 5 `cq_flip_check` and Phase 6 `anchor_resolution`), computes per-axis displacement by intersecting the natural box with the scroll container's visible viewport (offset by `ScrollOffset`), clamps the displacement so the box does not escape its parent's box (CSS sticky invariant), and writes the resolved position to `PostTaffyPositionOverrides`. Sticky writes do NOT invalidate Taffy (spec § 2.3) — sub-pass 6a runs after `taffy_compute` (step 3) and feeds `write_resolved_layout` (step 7) directly through the override map.
3. **Table + multicol as warn-once-per-session no-ops.** Sub-pass 6b `table_layout` queries entities with `Display::{Table, TableRowGroup, TableHeaderGroup, TableFooterGroup, TableRow, TableCell, TableCaption, TableColumnGroup, TableColumn}` and emits one `warn!` per (entity, session) on first encounter — the fallback path (Table → Block) is already handled by `translate.rs::map_display`. Sub-pass 6c `multicol_pack` queries entities with the new `MultiColumn` component and emits one `warn!` per session (single warn, not per-entity, per spec § 3.2 "once per session"). Both passes write nothing to `PostTaffyPositionOverrides`. The dedup mechanism is a new `LayoutWarnedOnceSession` resource holding `HashSet<LayoutWarnOnceKey>` cleared only on `BuiyExit` (canonical spec § 6 pattern); the existing Phase 6 `LayoutAnchorWarnedThisFrame` per-frame resource is preserved (different scope, different consumer).

**Tech Stack:** Bevy 0.18 (no new APIs vs Phase 6 — `Query<&Position>`, `Query<&Overflow>`, `Query<&ScrollOffset>`, `Query<&ChildOf>` ancestor walk with `.parent()` accessor, `NonSend<LayoutTree>` for `tree.tree.layout()` reads). Taffy 0.10 (read-only — no new Taffy emit path; sticky is a pure post-Taffy overlay, table maps to Block via existing `map_display`, multicol is a stub). `std::collections::{HashMap, HashSet}` (no `bevy::utils::HashMap` per Phase 6 precedent). **No new external dependency.**

---

## Plan v2 revisions (post 3-agent parallel review)

The first three reviewers (spec-coverage, feasibility, test-strategy) found a total of one spec-coverage BLOCKER, four feasibility BLOCKERs, and five test-strategy BLOCKERs. v2 addresses every BLOCKER and most CONCERNs. The key changes are:

1. **D3 simplified** — `Length` lacks `Vh/Vw/Vmin/Vmax/Em/Rem` (verified by direct inspection of `crates/buiy_core/src/layout/types.rs:29-50`). Only `Px`, `Percent`, `Fr`, and `Cqw/Cqh/Cqi/Cqb/Cqmin/Cqmax` exist. `resolve_sticky_inset` simplified accordingly. `StickyEmRemDeferred` variant dropped from `LayoutWarnOnceKey`; `StickyCqDeferred` added (sticky `Cq*` inset path is the new deferral, per follow-up vs. plumbing from Phase 6 `length_inset_to_px`).
2. **Task 5 helpers use `.parent()`** — verified `crates/buiy_core/src/layout/systems.rs:911,1120,1258` all access `ChildOf` via `.parent()`. `ChildOf::parent()` is the accessor; `ChildOf` is not a tuple struct in Bevy 0.18.
3. **`Entity::from_raw_u32(n).unwrap()`** — verified existing test pattern at `crates/buiy_core/src/layout/systems.rs:1806-1816` and `tests/a11y_translate.rs:29-91`. Task 2 + Task 4 test snippets use this form.
4. **`MultiColumn` always inserted via `#[derive(Bundle)]`** — verified `Style` at `crates/buiy_core/src/layout/style.rs:44` derives `Bundle`; every field is always inserted. The Phase-5 Container precedent confirms: Container is always inserted regardless of value. D8 corrected: drop the "only if non-default" claim. Task 3's `multi_column_default_not_inserted` test is INVERTED to `multi_column_always_inserted`.
5. **`Changed<MultiColumn>` added to nested Or<> in `sync_styles`** — spec `architecture.md § 1.2 line 42` lists `Changed<MultiColumn>` as a required trigger. Inner Or<> is currently at 5 entries (verified at `systems.rs:875-881`); cap is 15; plenty of room. Added to Task 3 Step 6.
6. **D4 step 8 formula text** — corrected to match the working code in `compute_sticky_displacement` (`.min(e_natural_in_s.y)` is the bottom-pin ceiling clamp, not `.min(e_natural_in_s.y + (parent_height - e_height))`).
7. **mod.rs:21 `pub use`** — Task 2 explicitly names this line as a rename site.
8. **5 new sticky unit tests** (Task 5) — bottom-pin-when-near-bottom, bottom-no-push-down-before-scroll, bottom-clamped-by-parent-top, both-top-and-bottom-active (documents "top wins" v1 deviation), clear-ordering-regression (2-frame, sticky + anchor target).
9. **5 new sticky integration tests** (Task 10) — explicit `overrides.by_entity.is_empty()` for no-scroll-container, table-no-rewarn-on-replace, `warned_once_session_manual_clear`, multicol with 3 entities (was 2 in v1), drop em/rem test (variant no longer exists).
10. **`Sizing::Length(Length::Cqw(_))` etc.** in `resolve_sticky_inset` either defer to a `StickyCqDeferred(Entity)` warn-once *or* port the Phase-6 `length_inset_to_px` helper. v2 chooses the **defer** path for Phase 7 — simpler, smaller scope; full Cq* sticky support tracked in `follow-ups.md` Phase 7 closeout.
11. **Sticky-inside-sticky behavior documented as a known limitation** — `world_position` walks Taffy positions (un-displaced), so an inner sticky inside an outer sticky resolves its threshold against the outer's *natural* position. Phase 7 ships this as known behavior; tracked in `follow-ups.md` Phase 7 closeout.

---

## Prior-art citations (used throughout this plan)

Each task below references these. Quoting the file + line here once so individual tasks stay tight.

- **Idempotent insert pattern** — `crates/buiy_core/src/layout/systems.rs:1158-1191` (`write_resolved_layout` compares `cur.position == new.position && cur.size == new.size` before `commands.entity(e).insert(new)`). Phase 7 reuses this when sub-pass 6a writes to `PostTaffyPositionOverrides` — the override map itself is overwritten freely each frame, but `ResolvedLayout` writes via `write_resolved_layout` continue to honor the prior-frame-comparison gate.
- **Memoized ancestor walk** — `crates/buiy_core/src/layout/systems.rs:1210-1290` (`inherit_writing_mode` / `resolve_writing_mode` allocate `HashMap<Entity, WritingMode>` once per call and use it as a per-call memoization cache). Phase 7's sticky `world_position` helper uses an analogous per-call `HashMap<Entity, Vec2>` so a tree with N sticky elements sharing an ancestor chain is O(N + depth), not O(N × depth).
- **Per-frame anchor warn-dedup** — `crates/buiy_core/src/layout/systems.rs:179-188` (`LayoutAnchorWarnedThisFrame` resource cleared at the top of `anchor_resolution`). Phase 7 preserves this Phase 6 divergence; sticky / table / multicol warns use the *new* per-session resource instead — see D6 below.
- **Per-session warn-once via AtomicBool gates** — `crates/buiy_core/src/layout/translate.rs:9-80` (`WARNED_CQ_NO_ANCESTOR`, `WARNED_CQB_AGAINST_INLINE`). Phase 7 does NOT extend this pattern (it's a per-process gate, not per-app, and would never re-warn in unit tests where multiple `App` instances are spun up). Phase 7 instead uses a per-app `Resource<HashSet>` — see Task 4.
- **Overflow scroll-container helper** — `crates/buiy_core/src/layout/components.rs:163-174` (`impl Overflow { pub fn is_scroll_container(&self) -> bool }`). Phase 7's `nearest_scroll_container` ancestor walk uses this method as the predicate.
- **ScrollOffset shape** — `crates/buiy_core/src/layout/components.rs:361-380` (`ScrollOffset { x: f32, y: f32 }` plus the contract "Mutating `ScrollOffset` must NOT invalidate `ResolvedLayout`"). Sticky pass reads `ScrollOffset` but does not depend on `Changed<ScrollOffset>`; the pass runs every frame unconditionally since sticky displacement is a function of *current* scroll offset.
- **Reading anchor / sticky / cq box from `tree.tree.layout()` not `ResolvedLayout`** — Phase 5 `cq_flip_check` precedent. `crates/buiy_core/src/layout/systems.rs:857-880` reads `let layout = tree.tree.layout(*node_id);` because at sub-pass 6 time, `ResolvedLayout` is *stale* (it's written in step 7, after step 6). Phase 6 `anchor_resolution` follows this. Phase 7 sticky / table / multicol all follow the same convention.
- **Pipeline step + attach point** — `crates/buiy_core/src/layout/pipeline.rs:17-44` (`BuiyLayoutStep::PostTaffyOverrides` enum slot). Phase 6 attached `anchor_resolution.in_set(PostTaffyOverrides)`. Phase 7 widens this to a chained tuple of five systems (clear + 6a + 6b + 6c + 6d) using `.chain().in_set(BuiyLayoutStep::PostTaffyOverrides)`.
- **Style components-only convention** — spec [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only). `Position` already lives in `Style` (Phase 1); `MultiColumn` is added to `Style` (it is a *container*-side property, not child-side, so the Phase 6 anchor decomposed-only rationale does not apply). `Style.multi_column: MultiColumn`, fluent setter `.multi_column(MultiColumn { .. })` mirrors the Container precedent at `crates/buiy_core/src/layout/style.rs:414-447`.
- **Phase 5 nested `Or<>` filter constraint** — `crates/buiy_core/src/layout/systems.rs:1042-1075` (`sync_styles` inner Or<> at 5 entries, outer at 14). Bevy 0.18 caps `Or<>` at 15 entries. Phase 7 does NOT widen the filter — sticky-inset changes do not require Taffy re-translation (sticky is a post-Taffy overlay; sub-pass 6a runs every frame and reads `Position` fresh). `MultiColumn` is a tier-E stub and does not feed Taffy in v1, so no `Changed<MultiColumn>` slot is needed in the filter. **Phase 8 (stacking) will likely need a filter slot** — see Task 8 deferred note.
- **Hierarchy walk via `ChildOf`** — `crates/buiy_core/src/layout/systems.rs:1245-1290` (`resolve_writing_mode` walks `parent_chain.get(entity)` until `Err`). Phase 7's `nearest_scroll_container` reuses the exact same walk shape, terminating on the first ancestor whose `Overflow.is_scroll_container()` is true.
- **`String` not `SmolStr`** — codebase uses `String` per Phase 3 / Phase 6 precedent (`crates/buiy_core/src/layout/types.rs:394` + Phase 6 plan D1). `MultiColumn`'s `column_rule` color and `BreakBefore::*` etc. are typed enums; no string payloads added.

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/types.rs` | T3 (add multicol enums: `ColumnCount`, `ColumnRule`, `ColumnRuleStyle`, `ColumnSpan`, `ColumnFill`, `BreakInside`, `BreakBefore`, `BreakAfter`; add `LayoutWarnOnceKey` enum) |
| `crates/buiy_core/src/layout/components.rs` | T3 (add `MultiColumn` component) |
| `crates/buiy_core/src/layout/systems.rs` | T2 (rename `AnchorOverrides` → `PostTaffyPositionOverrides`, add `clear_post_taffy_overrides`), T4 (add `LayoutWarnedOnceSession` resource), T5 (`sticky_offset` system + helpers), T6 (`table_layout` system), T7 (`multicol_pack` system), T9 (anchor_resolution target-position fallback to override map) |
| `crates/buiy_core/src/layout/style.rs` | T3 (add `multi_column: MultiColumn` field + fluent setter to Style builder + Bundle expansion) |
| `crates/buiy_core/src/layout/mod.rs` | T8 (`register_type` for 8 new public types, `init_resource` for `LayoutWarnedOnceSession`, replace single-system attach with chained tuple, re-exports) |
| `crates/buiy_core/src/lib.rs` | T8 (re-export `MultiColumn`, `ColumnCount`, `ColumnRule`, `ColumnRuleStyle`, `ColumnSpan`, `ColumnFill`, `BreakInside`, `BreakBefore`, `BreakAfter`, `LayoutWarnOnceKey`) |
| `crates/buiy/src/lib.rs` | T8 (re-export same set from top-level facade) |
| `crates/buiy_core/tests/layout_pipeline_order.rs` | T11 (augment fixture with sticky entity, table entity, multicol entity to assert all 4 sub-passes run in order) |
| `crates/buiy_core/tests/layout_sticky.rs` | T10 (new file — sticky behavior integration tests: pin-to-top during scroll, escape parent, no scroll container is no-op, percent inset against scroll container, anchor-target-is-sticky cross-phase) |
| `crates/buiy_core/tests/layout_table_multicol_stubs.rs` | T10 (new file — table & multicol stub warn-once tests) |
| `CHANGELOG.md` | T12 (post-merge) |
| `docs/plans/follow-ups.md` | T12 (Phase 7 follow-ups: full table algorithm, full multicol algorithm, Position::Fixed implementation, sticky-in-fixed-ancestor edge case) |
| `docs/README.md` | T12 (plan entry under "### Layout") |

No changes to: `translate.rs` (sticky maps to `taffy::Position::Relative` already; Table* → Block already; MultiColumn doesn't feed Taffy in v1), `pipeline.rs` (`PostTaffyOverrides` set is already declared in Phase 1), `tree.rs` (no LayoutTree shape change).

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. `AnchorOverrides` → `PostTaffyPositionOverrides` (rename + scope-widen)

**Decision:** Rename the Phase 6 `AnchorOverrides` resource to `PostTaffyPositionOverrides`. Same shape (`pub by_entity: HashMap<Entity, Vec2>`), same per-frame-cleared semantics, but conceptually scoped to "any post-Taffy position correction" rather than "anchor-only."

**Why:** The follow-up [Anchor positioning — anchor target IS sticky/table/multicol](../plans/follow-ups.md#anchor-positioning--anchor-target-is-stickytablemulticol-phase-7-interaction) Phase 7 interaction explicitly recommends "Option 1: move 6a-6c into a shared per-entity correction buffer that 6d reads." Phase 6 left the resource anchor-named because no other sub-pass existed; Phase 7 makes it shared. Without the rename, sticky-displaced positions would be invisible to `anchor_resolution` (sub-pass 6d) — an anchor target that is itself sticky would have its anchored elements compute against the un-displaced Taffy position (the exact symptom the follow-up describes).

**How to apply:** Single search-replace `AnchorOverrides → PostTaffyPositionOverrides` across `systems.rs`, `mod.rs`, and any tests. The `LayoutAnchorBroken` marker, `LayoutAnchorWarnedThisFrame` resource, `AnchorErrorKind` enum, and observer-maintained `AnchorNameRegistry` are NOT renamed (anchor-specific, correctly named).

**Runner-up rejected:** Keep `AnchorOverrides` anchor-only and add separate `StickyOffsets` / `TableOverrides` / `MulticolOverrides` resources. Rejected because (a) `write_resolved_layout` (step 7) would need to consult 4 resources in priority order, multiplying the read cost per entity; (b) anchor target → sticky target lookup in `anchor_resolution` would still need cross-resource read; (c) the "one per-frame correction buffer" model is the natural shape spec § 3 implies ("each sub-pass mutates `ResolvedLayout` for entities matching its concern").

### D2. Per-frame clear lives in a dedicated `clear_post_taffy_overrides` system

**Decision:** Move the `overrides.by_entity.clear()` line out of `anchor_resolution`'s top and into a new no-op-shaped system `clear_post_taffy_overrides` that runs first in `BuiyLayoutStep::PostTaffyOverrides`.

**Why:** With four sub-passes writing to the same map, the "first sub-pass clears it" pattern couples the clear to whichever pass happens to run first. If a future phase inserts a new sub-pass *before* sticky (e.g. a future 6Z that runs before 6a), the clear responsibility migrates with it — fragile. A dedicated clear system makes the lifecycle explicit and self-documenting.

**How to apply:** `pub(super) fn clear_post_taffy_overrides(mut overrides: ResMut<PostTaffyPositionOverrides>) { overrides.by_entity.clear(); }`. Attach as the first link in the chained tuple in `mod.rs`. The `LayoutAnchorWarnedThisFrame.set.clear()` line stays at the top of `anchor_resolution` (anchor-specific, unrelated to the shared override map).

**Runner-up rejected:** Have `sticky_offset` clear at its top (since it's the new "first sub-pass"). Rejected because (a) it duplicates the Phase 6 anchor pattern instead of fixing it, (b) it tightly couples ordering to sub-pass identity, (c) the test fixture for sticky would need to clear the override map explicitly to test stale-data scenarios — an awkward test boundary.

### D3. Sticky inset semantics — `Sizing::Auto` = "edge not set"

**Decision:** Treat `Sizing::Auto` (the `Inset` field default) as "this edge has no sticky inset." Only `Sizing::Length(Length::Px(_))` and `Sizing::Length(Length::Percent(_))` resolve to active sticky edges in v1. `Sizing::Length(Length::Fr(_))` resolves to `0.0` with one warn per (entity, session) `LayoutWarnOnceKey::StickyFrUnsupported` — `fr` is grid-only per spec. `Sizing::Length(Length::Cqw(_) | Length::Cqh(_) | Length::Cqi(_) | Length::Cqb(_) | Length::Cqmin(_) | Length::Cqmax(_))` resolves to `0.0` with one warn per (entity, session) `LayoutWarnOnceKey::StickyCqDeferred` — full container-units resolution requires plumbing the cq-context from Phase 6's `length_inset_to_px` (helper at `crates/buiy_core/src/layout/systems.rs::cq_compute_for_anchor` or similar; deferred to a Phase 7.x follow-up). `Sizing::None | FitContent(_) | MaxContent | MinContent | Stretch` resolve to "edge not set" (no warn — these are intrinsic-size keywords, never meaningful as positional insets in any CSS).

**What about `Vh/Vw/Vmin/Vmax/Em/Rem`?** They do not exist as `Length` variants in the current codebase (verified at `crates/buiy_core/src/layout/types.rs:29-50`). The doc comment on `Length` says viewport units arrive with Phase 10; em/rem are not on the spec roadmap for v1. Therefore the `resolve_sticky_inset` arm set is *closed* at Px / Percent (active) + Fr / Cq* (warn-defer-zero) + others (edge not set). No forward-compat arms for not-yet-existing variants — when Phase 10 adds `Length::Vh(_)` and friends, that phase extends this helper.

**Why:** Matches the CSS sticky spec: a sticky element only "sticks" to edges that have a non-auto inset. The `Inset::default()` shape (all `Sizing::Auto`) corresponds to a sticky element that participates as `Relative` with no displacement — a degenerate but valid case (e.g. mid-layout debugging). Cq*-as-zero plus warn is conservative — full support requires the Phase-6 cq-context helper and a re-entrant resolution path (the sticky element itself may be CQ-styled).

**How to apply:** In `sticky_offset`, a per-edge helper `fn resolve_sticky_inset(s: &Sizing, scroll_container_axis_size: f32, entity: Entity, warned: &mut LayoutWarnedOnceSession) -> Option<f32>` returns `Some(px)` for active edges and `None` for unset edges. Closed match on `Length` variants — no wildcard fallthrough so the compiler errors when Phase 10 adds new variants (forcing a deliberate decision per future variant).

**Runner-up rejected:** Define a new `StickyInset { top: Option<Length>, .. }` shape. Rejected because (a) `Inset` is reused across `Position` (absolute/relative/sticky) and `Anchor.position_try` — branching the type just for sticky's "unset" notion would fragment the API surface; (b) `Sizing::Auto` already exists and already means "unset" in most CSS contexts (margin auto, width auto). Reusing it here is the cheapest fit.

**Second runner-up rejected:** Plumb Phase-6 `length_inset_to_px` cq-context into sticky in Phase 7. Rejected because (a) the Phase-6 helper takes the anchor target's box as its second resolution reference; sticky has no anchor target, so the call shape differs; (b) the cq-context for sticky resolves against the *sticky entity's own* nearest CQ ancestor, not the scroll container — a re-entrant lookup that needs more design work; (c) full sticky-Cq tests need a multi-axis fixture (Cqi/Cqb resolve against writing-mode inline/block axes). Tracked in follow-ups.

### D4. Sticky algorithm — per-axis independent computation

**Decision:** Sticky displacement is computed per axis (X / Y) independently. The Y-axis algorithm:

1. Find nearest scroll container `S` via `ChildOf` walk. If none, write no override (sticky behaves as `Relative`) and continue.
2. Read `ScrollOffset` from `S` (default `Vec2::ZERO` if absent — not all scroll containers have a `ScrollOffset` component; `ScrollOffset` is opt-in per Phase 2).
3. Read `S`'s natural box from `tree.tree.layout(s_node_id)` — `position` (relative to S's parent) and `size` (S's content box).
4. Read the sticky entity `E`'s natural box from `tree.tree.layout(e_node_id)` — position (relative to E's parent) and size.
5. Read `E`'s parent box from `tree.tree.layout(parent_node_id)`.
6. Compute world-coords positions of `E` and `E`'s parent in `S`'s content-box coordinate system: walk up `ChildOf` from `E`, summing Taffy `.location` values, terminating at `S` (exclusive). Use a per-call `HashMap<Entity, Vec2>` memoization cache (mirrors `resolve_writing_mode`'s memo pattern).
7. Resolve `inset.top` per D3. If active:
   - Let `visible_top_in_S = scroll_offset.y` (the top of S's visible viewport in S's content-box coords).
   - Let `top_threshold = visible_top_in_S + top_inset_px`.
   - Let `desired_y = e_natural_y_in_S.max(top_threshold)`.
   - Clamp by parent: `desired_y = desired_y.min(parent_top_in_S + parent_height - e_height)`.
   - Also clamp at the floor: `desired_y = desired_y.max(e_natural_y_in_S)` (don't pull the element *up* past its natural position when scrolled to top).
   - Displacement: `displacement_y = desired_y - e_natural_y_in_S`.
8. Else if `inset.bottom` is active (mutually-exclusive precedence with top: when both are active, top wins — v1 deviation documented in CHANGELOG; bottom branch is unreachable when top is set):
   - Let `visible_bottom_in_S = scroll_offset.y + S_content_height`.
   - Let `bottom_threshold = visible_bottom_in_S - bottom_inset_px`.
   - Let `desired_y = (bottom_threshold - e_height).min(e_natural_y_in_S)` — pin to threshold or stay at natural, whichever is *smaller* (i.e., further up the page). This implements "don't push the element down past its natural position when the bottom threshold is below natural" — the bottom-sticky mirror of the top-sticky no-pull-up guard.
   - Clamp by parent floor: `desired_y = desired_y.max(parent_top_in_S)` — bottom-sticky element cannot go above parent top.
   - Clamp by parent bottom: `desired_y = desired_y.min(parent_in_S.y + parent_height - e_height)` — safety guard, redundant when Taffy correctly placed `e_natural_y_in_S` at or above `parent_bottom - e_height`.
   - `displacement_y = desired_y - e_natural_y_in_S`.
9. Final position in E's parent-relative coords: `position = e_natural_relative.x + displacement_x, e_natural_relative.y + displacement_y`. Write to `PostTaffyPositionOverrides.by_entity`.

X-axis mirror.

**Why this exact algorithm:**
- Matches CSS spec § 6.3 of CSS Positioned Layout Module Level 3 ("sticky positioning"): sticky element is positioned per its `position: relative` rules first, then "shifted by the sticky-offset along each axis."
- The "max(natural, threshold) then clamp by parent" sequence enforces the CSS invariant "the element does not appear to scroll past its parent." Without the parent clamp, a sticky element with a too-tall scroll container can scroll outside its parent's bounds — wrong per spec.
- The "max(desired, natural)" guard on the top-edge case is what makes sticky non-symmetric with `position: fixed`: when the user scrolls *to the top* (scroll offset < `inset.top`), a fixed element would still be pinned to `inset.top`; a sticky element falls back to its natural position. The guard implements this fall-back.

**Why both insets is "top wins":** Per CSS spec, when both `top` and `bottom` are set on a sticky element, the element behaves as if it has both upper and lower sticky-thresholds — it sticks to whichever edge the scroll position is currently closer to. Implementation: compute upper-stuck position from `top`, lower-stuck from `bottom`, take the one that is the *smaller perturbation* from natural. For Phase 7 we adopt the simpler "top wins on conflict" rule — this matches the WebKit/Blink implementation when both insets are set and produces visually identical results in the 99% case (top + bottom both set, scroll at top → top wins; scroll at bottom → bottom wins; this is what users expect). Document as a v1 deviation if profiling surfaces edge cases.

**How to apply:** Helper function `compute_sticky_displacement(e_box, parent_box, scroll_container_size, scroll_offset, inset, viewport, cq_ctx) -> Vec2`. Pure function for testability.

### D5. Sticky — no scroll container is a silent no-op

**Decision:** When a sticky element has no scroll-container ancestor, sub-pass 6a writes nothing to the override map (the element behaves as `Relative`). **No warn fires.**

**Why:** Spec § 2.1 says "Sticky — Nearest scroll container; falls back to parent's content box outside the sticky range." The "no scroll container at all" case is a special case of "outside the sticky range" — there is no range to be inside of. Sticky elements at the top of the layout tree (no scrollable ancestor) commonly arise during construction (e.g. a sticky header in a static demo). Warning would be noisy.

**How to apply:** Bail out of the sticky pass early when `nearest_scroll_container` returns `None`. No state update.

**Runner-up rejected:** Warn once per (entity, session). Rejected because the user might intentionally use sticky-in-static-context as a "no-op until you put me in a scroll container" placeholder pattern.

### D6. Per-session warn dedup — new `LayoutWarnedOnceSession` resource

**Decision:** Introduce `pub(super) struct LayoutWarnedOnceSession { pub set: HashSet<LayoutWarnOnceKey> }` (a Bevy `Resource`, `Default + Debug`), where `LayoutWarnOnceKey` enum variants name the specific (entity, error-kind) tuples a system has already warned about. Cleared **only on `BuiyExit`**, matching spec § 6: "deduplicated via a `HashSet` resource cleared on `BuiyExit`." Phase 7 sticky, table, multicol all use this resource for their warns. **Phase 6's `LayoutAnchorWarnedThisFrame` per-frame resource stays as-is** — anchor errors remain per-frame.

**Why:** Spec § 6 defines per-session dedup as the canonical pattern. Phase 6 introduced a per-frame variant *as a divergence* (documented in Phase 6 CHANGELOG "Deferred / divergences" item). Phase 7 does not extend that divergence to new error kinds; it re-aligns with spec for all new warns. Keeping both resources side-by-side makes the divergence explicit: per-frame for anchor errors (where a user might reposition an anchor and want to know each frame the error re-fires until they fix it), per-session for tier-E stub warnings (where the warning's purpose is to inform the developer once, not log spam every frame).

**How to apply:** `LayoutWarnOnceKey` lives in `types.rs` alongside `AnchorErrorKind`. Variants (v2 — Em/Rem dropped, Cq added):
- `TableUnsupported(Entity)` — one per table entity, one warn per session
- `MulticolUnsupported` — single session-wide warn (no Entity payload — first multicol entity triggers, all subsequent are silent)
- `StickyFrUnsupported(Entity)` — sticky entity uses `Length::Fr` inset (grid-only unit applied to inset). One warn per (entity, session); inset resolves to 0.0.
- `StickyCqDeferred(Entity)` — sticky entity uses a `Length::Cq*` inset (container query unit). Full cq-context resolution for sticky is deferred to a Phase 7.x follow-up; v1 resolves to 0.0. One warn per (entity, session).

Variant set is closed in Phase 7. Future phases extending warn-once can add variants here.

**v1 → v2 change:** v1's `StickyEmRemDeferred(Entity)` variant is dropped because `Length::Em(_)` and `Length::Rem(_)` do not exist in the codebase (verified at `crates/buiy_core/src/layout/types.rs:29-50`). If Phase 10 adds them, the variant returns then.

**Runner-up rejected:** Reuse Phase 6's per-frame `LayoutAnchorWarnedThisFrame`. Rejected because (a) clearing each frame means table/multicol warns fire every frame for the lifetime of the entity — log spam; (b) the resource is anchor-specific by name (`AnchorErrorKind`); cross-using it would force renaming and re-scoping that resource just to dodge a new resource.

### D7. `BuiyExit` clear of `LayoutWarnedOnceSession`

**Decision:** Wire `LayoutWarnedOnceSession.set.clear()` into a system that runs on `BuiyExit`. Spec § 6: "deduplicated via a `HashSet` resource cleared on `BuiyExit`."

**Why:** Without the clear, tests that drop and recreate `App` repeatedly (e.g. integration tests using `App::new()` per test) would accumulate state across runs. The clear ensures session-scoped warnings reset cleanly.

**How to apply:** New system `pub(super) fn clear_warned_once_on_exit(mut warned: ResMut<LayoutWarnedOnceSession>) { warned.set.clear(); }` registered in `LayoutPlugin::build` with `app.add_systems(OnExit(...), ...)`. **However, `BuiyExit` may not exist as a State value yet** — check `crates/buiy_core/src/lib.rs` for the lifecycle state names. If absent, the clear lives in `app.add_systems(OnExit(BuiyState::Active), clear_warned_once_on_exit)` (assuming a `BuiyState`), or — if no state machine exists — in a `pub(super) fn`-level documented "manual reset" via `world.resource_mut::<LayoutWarnedOnceSession>().set.clear()` exposed for tests, with a TODO comment marking it as a pending tie-in to the foundation lifecycle (cross-reference foundation spec). **The implementer must check what's available at task-time and pick the lowest-friction option that satisfies the spec contract** — see Task 4 for guidance.

**Why this open-endedness:** I (the planner) cannot verify the exact lifecycle state name without reading the foundation crate; the implementer reads it at task-time and picks. The contract is "warn-once persists for the lifetime of one App instance; recreating App resets the warns." Any mechanism that satisfies this is acceptable.

### D8. `MultiColumn` is a `Style` field, not decomposed-only

**Decision:** `MultiColumn` is a *container-side* property (declares the container as a multi-column formatter), not a child-side property. Per spec [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only), container-side properties live in `Style` as fields with fluent setters. `MultiColumn` follows the `Container` (Phase 5) and `GridParams` (Phase 3) precedent: it's a `Style` field that expands into the component on insert via the Bundle.

**Why:** Spec § 2.4 explicitly says "Style covers an entity's *self-styling* — properties that describe the entity's own box ... and the *container side* of layout algorithms it participates in (`FlexParams` when it's a flex container, `GridParams` when it's a grid container, `Container` when it's a query container)." Multicol is exactly this shape. Anchor is decomposed-only per § 2.4's "child side and relational properties" rationale; that does not apply to multicol.

**How to apply:** `Style { multi_column: MultiColumn::default(), .. }` field. Fluent setter `pub fn multi_column(mut self, m: MultiColumn) -> Self { self.multi_column = m; self }`. Defaults: `column_count: ColumnCount::Auto, column_width: None, column_gap: None, column_rule: ColumnRule::default(), column_span: ColumnSpan::None, column_fill: ColumnFill::Balance, break_inside: BreakInside::Auto, break_before: BreakBefore::Auto, break_after: BreakAfter::Auto`.

**v1 → v2 correction:** Style is `#[derive(Bundle, Clone, Debug, Default)]` at `crates/buiy_core/src/layout/style.rs:44`. The derive macro inserts every field unconditionally — there is no "include only if non-default" guard available with the derived Bundle. The Phase-5 Container precedent confirms: Container is **always** inserted regardless of value. MultiColumn follows suit: every entity spawned via `Style { .. }` gets a `MultiColumn` component, even when the field is default. This is consistent with how Container/Display/Position/etc. work. The Phase-1 "don't pollute entities with empty components" invariant *applies to optional Bundle fields* (e.g. `Option<&FlexItem>` is set only when an entity needs it); it does NOT apply to required Bundle fields (Container, MultiColumn). Task 3 Step 7's `multi_column_default_not_inserted` test is therefore **inverted** to `multi_column_always_inserted` — every Style-spawned entity has the component.

**Runner-up rejected:** Decomposed-only (mirror Phase 6 `Anchor`). Rejected because the spec is explicit and the precedent (Container, GridParams) is clear. Authors writing multicol layouts will write them in `Style { multi_column: .., flex_column().padding(..).multi_column(..) }` not as a separate `commands.spawn((Style::default(), MultiColumn { .. }))` pair.

### D9. Sticky in nested scroll containers — innermost wins

**Decision:** When a sticky element has multiple scroll-container ancestors (e.g., a sticky header inside a vertical-scroll list inside a horizontal-scroll viewport), the **nearest (innermost) scroll container** is the one that drives sticky behavior. The outer scroll container has no effect on the sticky displacement; the sticky element is positioned in the innermost scroll container's frame of reference.

**Why:** CSS spec § 6.3 ("sticky positioning"): "the sticky position rectangle is the intersection of the containing block of the sticky position element with the scroll-container's optimal viewing region." The nearest scroll container is the one that contains the element's containing block. Outer scroll containers don't affect sticky directly — they affect via the chain of natural positions (the element's parent is positioned by the outer container, and the sticky element is positioned relative to the parent).

**How to apply:** `nearest_scroll_container` returns the first ancestor whose `Overflow.is_scroll_container()` is true — terminate the `ChildOf` walk on first match. No special handling for nested containers.

**Test:** Spawn a sticky element inside two nested scroll containers; assert the sticky displacement responds only to the inner container's `ScrollOffset`.

### D10. Sticky bypassed for `Display::None` entities

**Decision:** If a sticky element has `Display::None`, it does not participate in layout (Taffy emits zero size). Sub-pass 6a skips it.

**Why:** `Display::None` entities are removed from layout per spec § 1.3. Writing a sticky override for a zero-sized box would set position-from-nowhere.

**How to apply:** In `sticky_offset`, filter the query to exclude `Display::None`:
```rust
Query<(Entity, &Position, &Display), (With<Node>, /* nothing else needed */)>
```
Then `.iter().filter(|(_, _, d)| !matches!(d, Display::None))`. Same pattern Phase 6 `anchor_resolution` uses for anchor target validation.

### D11. Sticky inset percent — against scroll container content box

**Decision:** When `inset.top` is `Sizing::Length(Length::Percent(p))`, the percentage resolves against the scroll container's content-box **height** (for top/bottom axes) or **width** (for left/right axes). Not the sticky element's parent. Not the sticky element itself.

**Why:** CSS spec: percentage insets on `position: sticky` resolve against the containing block, which for sticky is the nearest scroll container. (This differs from `position: absolute/fixed` where the containing block is the nearest positioned ancestor or viewport, respectively.)

**How to apply:** `resolve_sticky_inset` takes the scroll container's content-box size as the percentage base. The helper signature:
```rust
fn resolve_sticky_inset(
    s: &Sizing,
    scroll_container_axis_size: f32,
    viewport: Vec2,
    /* cq context if needed */
) -> Option<f32>
```

### D12. `LayoutAnchorBroken` marker remains anchor-specific (no extension to sticky/table/multicol)

**Decision:** Sticky / table / multicol entities that are degenerate (sticky with no scroll container, table with no algorithm, multicol with no algorithm) **do not** get a `LayoutAnchorBroken` marker. The marker is anchor-pass-specific and named to reflect that.

**Why:** Devtools markers signal "this entity is in an unexpected state visible to devtools tools." For sticky-no-scroll-container, the entity is in a fully-defined state (it behaves as relative). For table/multicol stubs, the entity falls back to Block / single-column — also fully-defined. No marker is justified.

**How to apply:** None — sticky/table/multicol systems write no marker. If a future phase needs a generic "layout error" marker, it would be a separate type (e.g. `LayoutBroken { reason: LayoutErrorKind }`), not an extension of `LayoutAnchorBroken`.

### D13. `Position::Fixed` remains a warn-once stub in Phase 7

**Decision:** Phase 7 does NOT implement `Position::Fixed`. The current Phase-1 warn-once behavior is preserved. Fixed-position elements continue to behave as Absolute with their natural containing block (parent or nearest positioned ancestor).

**Why:** Spec § 2.2 maps Fixed to "absolute with viewport-as-containing-block." That requires a separate `ContainingBlock` override pass (Fixed's CB is always the layout root, not the nearest positioned ancestor). That's a self-contained piece of work but is tangential to sticky/table/multicol — does not share code paths. Bundling Fixed into Phase 7 would balloon scope.

**How to apply:** No code change. Add a follow-up entry in `docs/plans/follow-ups.md` "Layout — Position::Fixed implementation" pointing at this decision.

**Runner-up rejected:** Implement Fixed in Phase 7. Rejected because the scope creep is unjustified by the spec's grouping of features into pipeline sub-passes.

### D14. `sticky_offset` query shape — `Or<>` for filter widening prevention

**Decision:** `sticky_offset` queries `Query<(Entity, &Position, &Display), With<Node>>` and filters in Rust. It does NOT use a `Or<>` query filter (e.g. `Without<Display::None>` is not a Bevy filter primitive; `With<Position>` is implicit by component access). The filter step inside the system:
```rust
for (e, pos, display) in query.iter() {
    if !matches!(pos.kind, PositionKind::Sticky) || matches!(display, Display::None) {
        continue;
    }
    // ... sticky algorithm
}
```

**Why:** Bevy's `Or<>` is limited to 15 entries (Phase 5 hit this cap). `sync_styles` is at 14. Phase 7 should NOT consume any of the remaining slots. Filtering in Rust is equivalent in performance for the typically-small set of `Position::Sticky` entities.

**How to apply:** Plain `Query<(Entity, &Position, &Display), With<Node>>` plus a Rust-side `continue` predicate.

### D15. Pipeline test fixture extension — minimal but representative

**Decision:** `tests/layout_pipeline_order.rs` is augmented with **one each** of: a sticky entity, a table entity, a multicol entity. The fixture validates that all four sub-passes (6a/6b/6c/6d — sticky/table/multicol/anchor) run in their declared order, and that all of them write/read `PostTaffyPositionOverrides` consistently. The existing Phase-6 anchor target + anchored pair stays.

**Why:** A single representative entity per sub-pass exercises the "system attached + observable side-effect" contract without inflating the fixture into a behavior-test surface (behavior tests live in their own files per T10). Pipeline-order tests should verify *ordering*, not *behavior*.

**How to apply:** Extend the existing fixture's spawn list. Assert that after `Update`, each of the four sub-passes has produced its expected side-effect (sticky → override map entry; table → warn count increment; multicol → warn count increment; anchor → override map entry for the anchored entity).

---

## Tasks

> **Per-task workflow (subagent-driven):**
> 1. Implementer subagent reads the task block.
> 2. Implementer follows TDD: failing test first, then minimal impl to pass, then refactor if needed, then commit.
> 3. Spec-compliance reviewer subagent reads the spec sections + the diff and asserts coverage.
> 4. Code-quality reviewer subagent reads the diff and asserts the code-quality bar.
> 5. Both reviews must be ✅ before moving to the next task.

### Task 1: Plan doc lands

**Files:**
- Create: `docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md` (this file)
- Modify: `docs/README.md` (Phase 7 entry under "### Layout")

- [ ] **Step 1: This plan doc is already drafted.** Confirm it covers (a) all four sub-passes, (b) decision blocks D1-D15, (c) tasks T1-T12 with TDD steps, (d) prior-art citations.
- [ ] **Step 2: Add docs/README.md entry.** Under "### Layout" → "**Plans**", append:
  ```markdown
  - [Buiy layout sticky/table/multicol](plans/2026-05-22-buiy-layout-sticky-table-multicol.md) — Phase 7: `sticky_offset` full impl (sub-pass 6a), `table_layout` and `multicol_pack` warn-once stubs (sub-passes 6b/6c), `MultiColumn` component, refactor of `AnchorOverrides` → `PostTaffyPositionOverrides` shared across all four sub-passes. `[active]`
  ```
- [ ] **Step 3: Commit.**
  ```bash
  git add docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md docs/README.md
  git commit -m "docs(layout): Phase 7 plan — sticky + table stub + multicol stub"
  ```

### Task 2: Refactor `AnchorOverrides` → `PostTaffyPositionOverrides` + add `clear_post_taffy_overrides`

**Spec:** D1, D2. [`architecture.md § 3`](../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline) (sub-pass ordering).

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs:174-188` (rename `AnchorOverrides` → `PostTaffyPositionOverrides`)
- Modify: `crates/buiy_core/src/layout/systems.rs:495-510` (remove `overrides.by_entity.clear()` from `anchor_resolution`'s top)
- Modify: `crates/buiy_core/src/layout/systems.rs:1158-1191` (update `write_resolved_layout` to read `PostTaffyPositionOverrides`)
- Add: `crates/buiy_core/src/layout/systems.rs` (after `LayoutAnchorWarnedThisFrame`, add `clear_post_taffy_overrides` system)
- Modify: `crates/buiy_core/src/layout/mod.rs:21` (the `pub use systems::{... AnchorOverrides ...}` re-export line — rename the identifier in that line)
- Modify: `crates/buiy_core/src/layout/mod.rs:57` (`init_resource::<AnchorOverrides>` → `init_resource::<PostTaffyPositionOverrides>`)
- Modify: `crates/buiy_core/src/layout/mod.rs:151` (the `anchor_resolution.in_set(...)` line will be rewired in Task 8 — leave it alone for now)
- Tests: existing tests at `systems.rs:1841-1851` (`anchor_overrides_default_empty`) renamed to `post_taffy_position_overrides_default_empty`
- Tests: existing tests in `tests/layout_anchor_positioning.rs` reference `AnchorOverrides` — update imports

- [ ] **Step 1: Read the current state.**
  ```bash
  grep -rn "AnchorOverrides" crates/ tests/ 2>/dev/null
  ```
  Expected hits: `systems.rs` (decl + 3 use sites + 1 test), `mod.rs` (init_resource), `tests/layout_anchor_positioning.rs` (1-3 import + assertion sites).

- [ ] **Step 2: Write failing test for clear_post_taffy_overrides.**
  Add to `systems.rs` `mod tests`:
  ```rust
  #[test]
  fn clear_post_taffy_overrides_clears_by_entity() {
      let mut app = App::new();
      app.init_resource::<PostTaffyPositionOverrides>();
      app.add_systems(Update, clear_post_taffy_overrides);
      // Pre-seed with a fake override.
      app.world_mut().resource_mut::<PostTaffyPositionOverrides>()
          .by_entity.insert(Entity::from_raw_u32(42).unwrap(), Vec2::new(10.0, 20.0));
      app.update();
      let overrides = app.world().resource::<PostTaffyPositionOverrides>();
      assert!(overrides.by_entity.is_empty(), "clear system did not empty the map");
  }
  ```
  Run: `cargo test -p buiy_core clear_post_taffy_overrides_clears_by_entity` — expected FAIL (`clear_post_taffy_overrides` doesn't exist, `PostTaffyPositionOverrides` doesn't exist).

- [ ] **Step 3: Mechanical rename.** Use `sed` or your editor to do an exact-match replace `AnchorOverrides` → `PostTaffyPositionOverrides` across:
  - `crates/buiy_core/src/layout/systems.rs`
  - `crates/buiy_core/src/layout/mod.rs`
  - `crates/buiy_core/tests/layout_anchor_positioning.rs`
  Then update the resource doc comment at `systems.rs:170-177` from:
  ```rust
  /// Phase 6 — transient override map populated by `anchor_resolution` and
  /// consumed by `write_resolved_layout`. ...
  pub struct AnchorOverrides {
      pub by_entity: std::collections::HashMap<Entity, Vec2>,
  }
  ```
  to:
  ```rust
  /// Phase 6/7 — transient override map populated by every sub-pass of
  /// `BuiyLayoutStep::PostTaffyOverrides` (`sticky_offset` 6a,
  /// `table_layout` 6b no-op, `multicol_pack` 6c no-op, and
  /// `anchor_resolution` 6d) and consumed by `write_resolved_layout`
  /// (step 7). Cleared by `clear_post_taffy_overrides` which runs first
  /// in the sub-pass chain.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
  #[derive(Resource, Default, Debug)]
  pub struct PostTaffyPositionOverrides {
      pub by_entity: std::collections::HashMap<Entity, Vec2>,
  }
  ```

- [ ] **Step 4: Remove the clear from `anchor_resolution`.** In `systems.rs` find the `anchor_resolution` body. Remove the line:
  ```rust
  overrides.by_entity.clear();
  ```
  Keep the `warned.set.clear();` line (anchor-specific, unchanged by Phase 7).

- [ ] **Step 5: Add `clear_post_taffy_overrides` system.** Insert immediately after the `LayoutAnchorWarnedThisFrame` resource declaration:
  ```rust
  /// Phase 7 — the sole site that clears `PostTaffyPositionOverrides`
  /// each frame. Runs first in `BuiyLayoutStep::PostTaffyOverrides`.
  /// Decouples per-frame clear from any one sub-pass so future
  /// sub-passes can be inserted without ordering surprises.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
  pub(super) fn clear_post_taffy_overrides(
      mut overrides: ResMut<PostTaffyPositionOverrides>,
  ) {
      overrides.by_entity.clear();
  }
  ```

- [ ] **Step 6: Run the test.**
  ```bash
  cargo test -p buiy_core clear_post_taffy_overrides_clears_by_entity
  ```
  Expected PASS.

- [ ] **Step 7: Run the full Phase 6 test suite to confirm the rename did not break anything.**
  ```bash
  cargo test -p buiy_core --test layout_anchor_positioning
  cargo test -p buiy_core layout::systems::tests
  ```
  Expected all PASS.

- [ ] **Step 8: Run the full project gate to catch any missed sites.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  ```
  Expected all green.

- [ ] **Step 9: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_anchor_positioning.rs
  git commit -m "refactor(layout): rename AnchorOverrides → PostTaffyPositionOverrides, extract clear_post_taffy_overrides system

Prep for Phase 7 — multiple sub-passes (sticky/table/multicol/anchor) all write to the
shared override map. The clear is lifted out of anchor_resolution into a dedicated
no-op system so the per-frame lifecycle is explicit and decoupled from sub-pass identity."
  ```

### Task 3: `MultiColumn` component + types + Style integration

**Spec:** D8, `flex-and-grid.md § 3` (multi-column).

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add 7 enums)
- Modify: `crates/buiy_core/src/layout/components.rs` (add `MultiColumn` component + reflect registration noted for Task 8)
- Modify: `crates/buiy_core/src/layout/style.rs` (add `multi_column: MultiColumn` field, fluent setter, Bundle expansion)

- [ ] **Step 1: Write failing component test.**
  Add to `components.rs::mod tests`:
  ```rust
  #[test]
  fn multi_column_default_is_auto() {
      let m = MultiColumn::default();
      assert_eq!(m.column_count, ColumnCount::Auto);
      assert!(m.column_width.is_none());
      assert!(m.column_gap.is_none());
      assert_eq!(m.column_span, ColumnSpan::None);
      assert_eq!(m.column_fill, ColumnFill::Balance);
      assert_eq!(m.break_inside, BreakInside::Auto);
      assert_eq!(m.break_before, BreakBefore::Auto);
      assert_eq!(m.break_after, BreakAfter::Auto);
  }
  ```
  Run: `cargo test -p buiy_core multi_column_default` — expected FAIL (types don't exist).

- [ ] **Step 2: Add enums to `types.rs`.** After the existing layout types (around the area where `AnchorErrorKind` lives), insert:
  ```rust
  // ============================================================
  // Phase 7 — multi-column types (flex-and-grid.md § 3)
  // ============================================================

  /// CSS `column-count`. Tier-E. Currently a stub field on
  /// `MultiColumn`; the algorithm warns-once and falls back to
  /// single-column layout in v1.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum ColumnCount {
      #[default]
      Auto,
      Count(u32),
  }

  /// CSS `column-rule` shorthand (width / style / color triple).
  /// Render side honors this; layout side passes it through.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
  #[derive(Reflect, Clone, Copy, PartialEq, Debug, Default)]
  pub struct ColumnRule {
      pub width: Length,
      pub style: ColumnRuleStyle,
      pub color: bevy::color::Color,
  }

  /// CSS `column-rule-style`. Subset of CSS line-style values.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum ColumnRuleStyle {
      #[default]
      None,
      Solid,
      Dashed,
      Dotted,
      Double,
  }

  /// CSS `column-span`.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum ColumnSpan {
      #[default]
      None,
      All,
  }

  /// CSS `column-fill`.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum ColumnFill {
      #[default]
      Balance,
      Auto,
  }

  /// CSS `break-inside`.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum BreakInside {
      #[default]
      Auto,
      Avoid,
      AvoidColumn,
  }

  /// CSS `break-before`.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum BreakBefore {
      #[default]
      Auto,
      Always,
      Avoid,
      Column,
      AvoidColumn,
  }

  /// CSS `break-after`.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub enum BreakAfter {
      #[default]
      Auto,
      Always,
      Avoid,
      Column,
      AvoidColumn,
  }
  ```

- [ ] **Step 3: Add `MultiColumn` component to `components.rs`.** After the `Container` component, add:
  ```rust
  /// CSS multi-column container (tier-E).
  ///
  /// **Status:** API stub. v1 ships every field for forward
  /// compatibility, but the multi-column packing algorithm is a no-op
  /// — sub-pass 6c emits one `warn!` per session on the first
  /// `MultiColumn` entity it encounters and produces single-column
  /// layout. Authors can write multi-column-aware code that compiles
  /// against v1; the algorithm lands in a v1.x point release.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.
  ///
  /// Sub-pass: 6c (`multicol_pack`) in `BuiyLayoutStep::PostTaffyOverrides`.
  #[derive(Component, Reflect, Clone, Copy, PartialEq, Debug, Default)]
  #[reflect(Component, Default)]
  pub struct MultiColumn {
      pub column_count: ColumnCount,
      pub column_width: Option<Length>,
      pub column_gap:   Option<Length>,
      pub column_rule:  ColumnRule,
      pub column_span:  ColumnSpan,
      pub column_fill:  ColumnFill,
      pub break_inside: BreakInside,
      pub break_before: BreakBefore,
      pub break_after:  BreakAfter,
  }
  ```
  And add the imports at the top of `components.rs`:
  ```rust
  use crate::layout::types::{
      // ... existing imports ...
      BreakAfter, BreakBefore, BreakInside, ColumnCount, ColumnFill, ColumnRule, ColumnSpan,
  };
  ```

- [ ] **Step 4: Add `multi_column` to Style builder.** In `style.rs:46-55` (the Style struct), add the field:
  ```rust
  pub struct Style {
      pub display: Display,
      pub box_model: BoxModel,
      pub position: Position,
      pub flex_params: FlexParams,
      pub grid_params: GridParams,
      pub container: Container,
      pub multi_column: MultiColumn,  // NEW
      pub overflow: Overflow,
      pub scroll: Scroll,
      pub writing_mode: WritingMode,
  }
  ```
  Add the fluent setter (near other container-side setters around line 320+):
  ```rust
  // ---- MultiColumn ----

  /// Set `MultiColumn` for this entity (declares it as a multi-column
  /// container). Tier-E API surface — v1 falls back to single-column
  /// with a warn-once-per-session.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.
  pub fn multi_column(mut self, m: MultiColumn) -> Self {
      self.multi_column = m;
      self
  }
  ```
  Update `style.rs` imports:
  ```rust
  use crate::layout::components::{
      // ... existing ...
      MultiColumn,
  };
  ```

- [ ] **Step 5: Bundle expansion is automatic.** No manual code needed — `Style` is `#[derive(Bundle, ...)]` at `crates/buiy_core/src/layout/style.rs:44`, so adding `multi_column: MultiColumn` as a field is sufficient for it to be inserted on every spawn. No "only if non-default" guard exists; every Style-spawned entity receives a `MultiColumn`. This mirrors the Container precedent (every Style-spawned entity receives a `Container`). **v1 → v2 correction.** Skip this step — the field add in Step 4 is sufficient.

- [ ] **Step 6: Widen `sync_styles` `Or<>` filter to include `Changed<MultiColumn>`.** Per spec `architecture.md § 1.2 line 42`, `Changed<MultiColumn>` is required in the trigger set. The outer Or<> is currently at 15 entries (cap); the *inner nested Or<>* (at `systems.rs:875-881`) is at 5 entries — plenty of room.
  Find this block in `systems.rs`:
  ```rust
  Or<(
      Changed<Container>,
      Changed<ContainerQuery>,
      Changed<ContainerQueryActive>,
      Changed<ContainerQueryInactive>,
      Changed<Anchor>,
  )>,
  ```
  Add `Changed<MultiColumn>` as the 6th entry. **Also import `MultiColumn` in the `use` statement at the top of `systems.rs`.** Note in a comment that the trigger is currently a no-op (multicol doesn't feed Taffy in v1) but is wired for forward-compat per spec.

  Run: `cargo test -p buiy_core` — no test specifically asserts this, but the project gate (`cargo clippy --workspace --all-targets -- -D warnings`) must remain green.

- [ ] **Step 7: Run tests.**
  ```bash
  cargo test -p buiy_core multi_column
  ```
  Expected PASS for the type tests added in Step 1.

- [ ] **Step 8: Add a Style-expansion test.** In `style.rs::mod tests`:
  ```rust
  #[test]
  fn multi_column_field_round_trips() {
      let mut world = World::new();
      let s = Style::default().multi_column(MultiColumn {
          column_count: ColumnCount::Count(3),
          ..Default::default()
      });
      let entity = world.spawn(s).id();
      let mc = world.get::<MultiColumn>(entity).expect("MultiColumn inserted");
      assert_eq!(mc.column_count, ColumnCount::Count(3));
  }

  // v1 → v2: this test asserts MultiColumn is ALWAYS inserted (mirrors Container
  // behavior). Style is #[derive(Bundle)]; every field always inserts.
  #[test]
  fn multi_column_always_inserted() {
      let mut world = World::new();
      let s = Style::default(); // multi_column is at default value
      let entity = world.spawn(s).id();
      assert!(world.get::<MultiColumn>(entity).is_some(),
          "Style is derived-Bundle: every field inserts unconditionally (matches Container)");
  }
  ```
  Run: `cargo test -p buiy_core multi_column_field`. Expected PASS.

- [ ] **Step 9: Project gate.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  ```

- [ ] **Step 10: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/components.rs crates/buiy_core/src/layout/style.rs crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): MultiColumn component + sync_styles trigger (Phase 7 tier-E API)

Adds MultiColumn + supporting enums (ColumnCount, ColumnRule, ColumnRuleStyle,
ColumnSpan, ColumnFill, BreakInside, BreakBefore, BreakAfter). Component is a
Style field per spec § 2.4 container-side convention; Style derives Bundle so the
component is always inserted (matches Container).

Also widens the sync_styles nested Or<> filter to include Changed<MultiColumn>
per spec architecture.md § 1.2. Filter is forward-compat — multicol doesn't feed
Taffy in v1, but the trigger will be live when the v1.x algorithm ships.

Algorithm is deferred to v1.x; sub-pass 6c (Task 7) emits the warn-once."
  ```

### Task 4: `LayoutWarnedOnceSession` resource + `LayoutWarnOnceKey` type

**Spec:** D6, D7. [`architecture.md § 6`](../specs/2026-05-08-buiy-layout-design/architecture.md#6-error-model) (error model).

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add `LayoutWarnOnceKey` enum)
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `LayoutWarnedOnceSession` resource + `clear_warned_once_on_exit` system if applicable)
- Modify: `crates/buiy_core/src/layout/mod.rs` (init_resource)

- [ ] **Step 1: Failing test.**
  ```rust
  // crates/buiy_core/src/layout/systems.rs mod tests:
  #[test]
  fn warned_once_session_default_empty() {
      let r = LayoutWarnedOnceSession::default();
      assert!(r.set.is_empty());
  }

  #[test]
  fn warned_once_session_dedup() {
      let mut r = LayoutWarnedOnceSession::default();
      let key = LayoutWarnOnceKey::TableUnsupported(Entity::from_raw_u32(1).unwrap());
      let first = r.set.insert(key);
      let second = r.set.insert(key);
      assert!(first, "first insert should report true (newly added)");
      assert!(!second, "second insert should report false (already present)");
  }
  ```
  Run: `cargo test -p buiy_core warned_once`. Expected FAIL.

- [ ] **Step 2: Add `LayoutWarnOnceKey` to `types.rs`.** After `AnchorErrorKind`:
  ```rust
  /// Phase 7 — session-scoped warn-once dedup key. Variants cover the
  /// non-anchor layout error/stub conditions introduced in Phase 7.
  ///
  /// Anchor errors continue to use the per-frame
  /// `LayoutAnchorWarnedThisFrame` resource — that divergence from
  /// spec § 6 is preserved by Phase 7 (see Phase 6 CHANGELOG).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 6.
  #[derive(Reflect, Clone, Copy, PartialEq, Eq, Hash, Debug)]
  pub enum LayoutWarnOnceKey {
      /// `Display::Table*` entity encountered. Sub-pass 6b emits one
      /// warn per (entity, session) — the table algorithm is deferred
      /// to v1.x.
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
      TableUnsupported(Entity),

      /// `MultiColumn` entity encountered. Sub-pass 6c emits one warn
      /// per session (no Entity payload — first multicol entity triggers,
      /// all subsequent are silent) — the multicol algorithm is
      /// deferred to v1.x.
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
      MulticolUnsupported,

      /// Sticky entity uses `Length::Fr` inset. `fr` is grid-only;
      /// applying it as a sticky inset is semantically invalid. Warn
      /// once per (entity, session); inset resolves to 0.0.
      ///
      /// Spec: D3, this plan.
      StickyFrUnsupported(Entity),

      /// Sticky entity uses a `Length::Cq*` inset (container query
      /// unit). Full cq-context resolution for sticky is deferred to
      /// a Phase 7.x follow-up (port from Phase 6 `length_inset_to_px`).
      /// v1 resolves to 0.0. One warn per (entity, session).
      ///
      /// Spec: D3, this plan.
      StickyCqDeferred(Entity),
  }
  ```

- [ ] **Step 3: Add `LayoutWarnedOnceSession` resource to `systems.rs`.** After `LayoutAnchorWarnedThisFrame`:
  ```rust
  /// Phase 7 — session-scoped warn-dedup set. Cleared only on
  /// `BuiyExit` (see `clear_warned_once_on_exit` below). Used by
  /// `sticky_offset`, `table_layout`, `multicol_pack`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 6
  /// ("deduplicated via a `HashSet` resource cleared on `BuiyExit`").
  #[derive(Resource, Default, Debug)]
  pub struct LayoutWarnedOnceSession {
      pub set: std::collections::HashSet<LayoutWarnOnceKey>,
  }
  ```

- [ ] **Step 4: Add the on-exit clear system.** Check the existing `BuiyState` / `BuiyExit` in `crates/buiy_core/src/lib.rs` (search for `enum BuiyState`, `OnExit`, `BuiyExit`). If found, attach a clear system. If not found, add the function but DO NOT wire it (the wiring will be done in a follow-up phase when the foundation lifecycle states are settled — track in `follow-ups.md`). The function definition is identical either way:
  ```rust
  /// Phase 7 — clears the session-scoped warn-dedup set on app
  /// shutdown. Wired to `OnExit(BuiyState::Active)` (or the
  /// equivalent foundation-lifecycle hook) in `LayoutPlugin::build`.
  ///
  /// Without this, repeated `App::new()` cycles in test harnesses
  /// would accumulate warns across test instances.
  pub(super) fn clear_warned_once_on_exit(
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      warned.set.clear();
  }
  ```

- [ ] **Step 5: Register the resource in `mod.rs`.** In `LayoutPlugin::build`:
  ```rust
  app.init_resource::<systems::LayoutWarnedOnceSession>();
  ```
  Group it alongside the existing `init_resource::<AnchorNameRegistry>()` and `init_resource::<PostTaffyPositionOverrides>()` calls.

- [ ] **Step 6: Run tests.**
  ```bash
  cargo test -p buiy_core warned_once
  ```
  Expected PASS.

- [ ] **Step 7: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs
  git commit -m "feat(layout): LayoutWarnedOnceSession resource (Phase 7 warn dedup)

New resource holds a HashSet<LayoutWarnOnceKey> for session-scoped warn dedup,
matching spec § 6 'cleared on BuiyExit'. Adds LayoutWarnOnceKey enum with
variants for table (per-entity), multicol (session-wide), sticky Fr (per-entity),
sticky Cq* (per-entity). Phase 6's per-frame LayoutAnchorWarnedThisFrame stays
unchanged. Em/Rem variants not added — those Length variants do not exist in
the codebase (verified at types.rs:29-50)."
  ```

### Task 5: `sticky_offset` system (sub-pass 6a, full implementation)

**Spec:** D3, D4, D5, D9, D10, D11, D14. [`display-and-positioning.md § 2.3`](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md#23-sticky-positioning).

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `sticky_offset` system + private helpers `nearest_scroll_container`, `world_position`, `resolve_sticky_inset`, `compute_sticky_displacement`)

- [ ] **Step 1: Failing test for `nearest_scroll_container`.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn nearest_scroll_container_finds_overflow_ancestor() {
      let mut world = World::new();
      let scroll_root = world.spawn((
          Node,
          Overflow { x: OverflowMode::Visible, y: OverflowMode::Scroll, ..Default::default() },
      )).id();
      let middle = world.spawn(Node).id();
      let sticky = world.spawn((
          Node,
          Position { kind: PositionKind::Sticky, ..Default::default() },
      )).id();
      world.entity_mut(middle).insert(ChildOf(scroll_root));
      world.entity_mut(sticky).insert(ChildOf(middle));

      let parent_chain = world.query::<&ChildOf>();
      let overflow_q = world.query::<&Overflow>();
      // We'll wrap the helper for the test if needed.
      // Note: this test is a sketch; the actual helper takes Query
      // params, so the integration test is a better location for this.
      // Keep this here as a smoke test by reading queries via SystemState.

      // ... (implementer fills in via SystemState pattern; see
      // resolve_writing_mode tests in systems.rs:1395+ for the pattern)
  }
  ```
  **Implementer note:** the test as written needs `SystemState` to read Bevy `Query`s outside of a system. See `systems.rs:1395+` for the existing `resolve_writing_mode` test pattern. Adapt that.

  Run: expected FAIL.

- [ ] **Step 2: Implement `nearest_scroll_container` helper.**
  ```rust
  /// Walk up `ChildOf` from `entity`, returning the first ancestor whose
  /// `Overflow.is_scroll_container()` is true. Returns `None` if no
  /// scroll-container ancestor exists.
  ///
  /// Phase 7 — sub-pass 6a (`sticky_offset`) uses this to find the
  /// reference frame for sticky displacement.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.1.
  fn nearest_scroll_container(
      entity: Entity,
      parent_chain: &Query<&ChildOf>,
      overflow_q: &Query<&Overflow>,
  ) -> Option<Entity> {
      let mut current = entity;
      loop {
          // v2 fix: ChildOf is not a tuple struct in Bevy 0.18; use .parent().
          let parent = parent_chain.get(current).ok()?.parent();
          if let Ok(overflow) = overflow_q.get(parent)
              && overflow.is_scroll_container()
          {
              return Some(parent);
          }
          current = parent;
      }
  }
  ```
  Run nearest_scroll_container test: PASS.

- [ ] **Step 3: Implement `world_position` helper.** Walks up the tree summing Taffy locations, terminating at the given ancestor (exclusive).
  ```rust
  /// Compute `entity`'s position in `ancestor`'s content-box coordinate
  /// system by walking `ChildOf` from `entity` up to (but not including)
  /// `ancestor`, summing the Taffy `.location` of each step.
  ///
  /// Uses the provided `memo` cache to avoid re-walking shared subpaths.
  ///
  /// Returns `None` if (a) `entity` has no LayoutTree mapping, (b) the
  /// walk leaves `ancestor`'s subtree without finding `ancestor`, or
  /// (c) a `tree.tree.layout()` read fails.
  ///
  /// Phase 7 — sub-pass 6a (`sticky_offset`).
  fn world_position(
      entity: Entity,
      ancestor: Entity,
      tree: &LayoutTree,
      parent_chain: &Query<&ChildOf>,
      memo: &mut std::collections::HashMap<Entity, Vec2>,
  ) -> Option<Vec2> {
      if entity == ancestor {
          return Some(Vec2::ZERO);
      }
      if let Some(cached) = memo.get(&entity) {
          return Some(*cached);
      }
      // v2 fix: ChildOf accessor is .parent() in Bevy 0.18.
      let parent = parent_chain.get(entity).ok()?.parent();
      let parent_position = world_position(parent, ancestor, tree, parent_chain, memo)?;
      let node_id = tree.by_entity.get(&entity)?;
      let layout = tree.tree.layout(*node_id).ok()?;
      let position = parent_position + Vec2::new(layout.location.x, layout.location.y);
      memo.insert(entity, position);
      Some(position)
  }
  ```

- [ ] **Step 4: Implement `resolve_sticky_inset` helper.**
  ```rust
  /// Resolve a `Sizing` inset to pixels in the scroll container's
  /// reference frame, per D3 / D11.
  ///
  /// Returns `Some(px)` for "this edge is sticky-active" or `None` for
  /// "this edge is not set." Inputs that are deferred (Cq*) or
  /// invalid (Fr) return `Some(0.0)` and record a warn-once via the
  /// caller.
  ///
  /// v2 — em/rem are not `Length` variants and never will be without a
  /// Phase 10 extension; the match is closed (no wildcard arm) so the
  /// compiler errors when Phase 10 adds new variants.
  ///
  /// Phase 7 — sub-pass 6a (`sticky_offset`).
  fn resolve_sticky_inset(
      s: &Sizing,
      scroll_container_axis_size: f32,
      entity: Entity,
      warned: &mut LayoutWarnedOnceSession,
  ) -> Option<f32> {
      use crate::layout::types::{Length, Sizing};
      let length = match s {
          Sizing::Length(l) => l,
          // Auto, None, FitContent, MinContent, MaxContent, Stretch —
          // edge not set; intrinsic-size keywords are not meaningful as
          // positional insets in any CSS.
          _ => return None,
      };
      Some(match length {
          Length::Px(p) => *p,
          Length::Percent(p) => scroll_container_axis_size * (p / 100.0),
          Length::Fr(_) => {
              if warned.set.insert(LayoutWarnOnceKey::StickyFrUnsupported(entity)) {
                  bevy::log::warn!(
                      "Sticky entity {:?} uses fr inset; fr is grid-only and resolves to 0.0 on sticky inset.",
                      entity,
                  );
              }
              0.0
          }
          // All Cq* variants — full resolution is deferred to a Phase
          // 7.x follow-up (would port Phase 6 length_inset_to_px which
          // takes an anchor-box second argument; sticky's reference
          // frame is the sticky entity's own cq-ancestor, a different
          // shape). v1: warn once per entity, resolve to 0.0.
          Length::Cqw(_) | Length::Cqh(_) | Length::Cqi(_)
          | Length::Cqb(_) | Length::Cqmin(_) | Length::Cqmax(_) => {
              if warned.set.insert(LayoutWarnOnceKey::StickyCqDeferred(entity)) {
                  bevy::log::warn!(
                      "Sticky entity {:?} uses Cq* inset; sticky-cq resolution is deferred to a Phase 7.x follow-up. Inset resolves to 0.0.",
                      entity,
                  );
              }
              0.0
          }
      })
  }
  ```
  **Implementer note:** the `axis: StickyAxis` parameter from v1 is dropped — sticky CSS percent semantics depend on which *axis* the inset is on, and the caller already passes the correct `scroll_container_axis_size` (height for top/bottom, width for left/right). No StickyAxis enum is needed. `viewport: Vec2` is also dropped — no viewport units exist in `Length` currently, so the function never reads it.

- [ ] **Step 5: Implement `compute_sticky_displacement` (pure function).**
  ```rust
  /// Compute the per-axis sticky displacement, given the natural-Taffy
  /// position and size of the sticky element, its parent, the scroll
  /// container's size, the current scroll offset, and the resolved
  /// inset values.
  ///
  /// All positions are in the scroll container's content-box
  /// coordinate frame. Output is a displacement to add to the sticky
  /// element's natural-relative-to-parent position to get the final
  /// position-in-parent-frame.
  ///
  /// Pure function — no Bevy queries, no Taffy reads. Easy to unit
  /// test.
  ///
  /// Phase 7 — sub-pass 6a.
  fn compute_sticky_displacement(
      e_natural_in_s: Vec2,           // sticky element position in S
      e_size: Vec2,                   // sticky element size
      parent_in_s: Vec2,              // parent position in S
      parent_size: Vec2,              // parent size
      _scroll_container_size: Vec2,    // S's content box size
      scroll_offset: Vec2,            // current ScrollOffset
      inset_top: Option<f32>,
      inset_bottom: Option<f32>,
      inset_left: Option<f32>,
      inset_right: Option<f32>,
  ) -> Vec2 {
      let visible_top = scroll_offset.y;
      let visible_bottom = scroll_offset.y + _scroll_container_size.y;
      let visible_left = scroll_offset.x;
      let visible_right = scroll_offset.x + _scroll_container_size.x;

      let parent_bottom = parent_in_s.y + parent_size.y;
      let parent_right  = parent_in_s.x + parent_size.x;

      let desired_y = if let Some(top_px) = inset_top {
          let threshold = visible_top + top_px;
          e_natural_in_s.y
              .max(threshold)
              .min(parent_bottom - e_size.y)
              .max(parent_in_s.y)
      } else if let Some(bottom_px) = inset_bottom {
          let threshold = visible_bottom - bottom_px;
          (threshold - e_size.y)
              .min(e_natural_in_s.y)
              .max(parent_in_s.y)
              .min(parent_bottom - e_size.y)
      } else {
          e_natural_in_s.y
      };
      let desired_x = if let Some(left_px) = inset_left {
          let threshold = visible_left + left_px;
          e_natural_in_s.x
              .max(threshold)
              .min(parent_right - e_size.x)
              .max(parent_in_s.x)
      } else if let Some(right_px) = inset_right {
          let threshold = visible_right - right_px;
          (threshold - e_size.x)
              .min(e_natural_in_s.x)
              .max(parent_in_s.x)
              .min(parent_right - e_size.x)
      } else {
          e_natural_in_s.x
      };

      Vec2::new(desired_x - e_natural_in_s.x, desired_y - e_natural_in_s.y)
  }
  ```

- [ ] **Step 6: Failing unit tests for `compute_sticky_displacement`.**
  ```rust
  #[test]
  fn sticky_no_inset_no_displacement() {
      let d = compute_sticky_displacement(
          Vec2::new(10.0, 20.0),  // natural in S
          Vec2::new(100.0, 50.0), // size
          Vec2::new(0.0, 0.0),    // parent in S
          Vec2::new(300.0, 1000.0), // parent size
          Vec2::new(300.0, 500.0), // S size
          Vec2::ZERO,             // scroll offset
          None, None, None, None, // no insets
      );
      assert_eq!(d, Vec2::ZERO);
  }

  #[test]
  fn sticky_top_pins_when_scrolled_past() {
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 50.0),   // natural at y=50
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 0.0),
          Vec2::new(300.0, 1000.0),
          Vec2::new(300.0, 500.0),
          Vec2::new(0.0, 100.0),  // scrolled down by 100
          Some(10.0), None, None, None, // top: 10px
      );
      // visible_top = 100, threshold = 110. natural_y = 50.
      // desired_y = max(50, 110) = 110, clamped by parent_bottom - size = 1000 - 30 = 970, by parent_in_s.y = 0
      // displacement.y = 110 - 50 = 60
      assert_eq!(d, Vec2::new(0.0, 60.0));
  }

  #[test]
  fn sticky_top_does_not_pull_up() {
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 50.0),   // natural at y=50
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 0.0),
          Vec2::new(300.0, 1000.0),
          Vec2::new(300.0, 500.0),
          Vec2::ZERO,             // not scrolled
          Some(10.0), None, None, None,
      );
      // visible_top = 0, threshold = 10. natural_y = 50.
      // desired_y = max(50, 10) = 50, clamped by parent. = 50
      // displacement = 50 - 50 = 0
      assert_eq!(d, Vec2::ZERO);
  }

  #[test]
  fn sticky_top_clamped_by_parent_bottom() {
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 10.0),
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 0.0),
          Vec2::new(300.0, 50.0), // small parent — height 50
          Vec2::new(300.0, 1000.0),
          Vec2::new(0.0, 100.0),
          Some(5.0), None, None, None,
      );
      // visible_top = 100, threshold = 105. natural_y = 10.
      // desired_y = max(10, 105) = 105, clamped by parent_bottom - size = 50 - 30 = 20, by parent_in_s.y = 0
      // = 20
      // displacement = 20 - 10 = 10
      assert_eq!(d, Vec2::new(0.0, 10.0));
  }

  // ---- v2 — bottom-pin branch coverage (BLOCKER B1) ----

  #[test]
  fn sticky_bottom_pins_when_scroll_near_bottom() {
      // visible_bottom = scroll_offset.y + S.y = 300 + 500 = 800.
      // threshold = 800 - 10 = 790. natural_y = 700, e_h = 30.
      // (threshold - e_h) = 760. min(760, 700) = 700.
      // .max(parent_top=0) = 700. .min(parent_bottom - e_h = 970) = 700.
      // displacement = 700 - 700 = 0 — wait, the threshold (760) is below natural (700)?
      // Re-examining: when bottom_threshold - e_height >= natural, sticky stays at natural.
      // Need scroll_offset such that threshold - e_h < natural. Try scroll_offset.y=150:
      // visible_bottom = 650, threshold = 640, threshold - e_h = 610. min(610, 700) = 610.
      // displacement = 610 - 700 = -90.
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 700.0),    // natural y=700
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 0.0),      // parent in S
          Vec2::new(300.0, 1000.0), // parent height
          Vec2::new(300.0, 500.0),  // S size
          Vec2::new(0.0, 150.0),    // scroll
          None, Some(10.0), None, None, // bottom: 10px
      );
      assert_eq!(d, Vec2::new(0.0, -90.0));
  }

  #[test]
  fn sticky_bottom_does_not_push_down_before_scroll() {
      // visible_bottom = 0 + 500 = 500, threshold = 490, threshold - e_h = 460.
      // min(460, natural=300) = 300. displacement = 0.
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 300.0),
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 0.0),
          Vec2::new(300.0, 1000.0),
          Vec2::new(300.0, 500.0),
          Vec2::ZERO,
          None, Some(10.0), None, None,
      );
      assert_eq!(d, Vec2::ZERO);
  }

  #[test]
  fn sticky_bottom_clamped_by_parent_top() {
      // parent_in_s.y = 100, parent_height = 200. natural_y = 280, e_h = 30.
      // visible_bottom = 0 + 100 = 100, threshold = 90, threshold - e_h = 60.
      // .min(natural=280) = 60. .max(parent_top=100) = 100. .min(parent_bottom - e_h = 270) = 100.
      // displacement = 100 - 280 = -180.
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 280.0),
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 100.0),    // parent has nonzero top
          Vec2::new(300.0, 200.0),
          Vec2::new(300.0, 100.0),  // tiny scroll container
          Vec2::ZERO,
          None, Some(10.0), None, None,
      );
      assert_eq!(d, Vec2::new(0.0, -180.0));
  }

  // ---- v2 — both-top-and-bottom-active behavior (BLOCKER B2) ----

  #[test]
  fn sticky_both_top_and_bottom_active_top_wins() {
      // v1 deviation: when both insets are set, top wins. This test documents
      // the behavior — a future correct dual-clamp impl will fail this test
      // and that's the signal to flip it.
      let d = compute_sticky_displacement(
          Vec2::new(0.0, 50.0),
          Vec2::new(100.0, 30.0),
          Vec2::new(0.0, 0.0),
          Vec2::new(300.0, 1000.0),
          Vec2::new(300.0, 500.0),
          Vec2::new(0.0, 100.0),
          Some(10.0), Some(10.0), None, None, // both insets set
      );
      // Top-pin branch fires: visible_top=100, threshold=110, max(50, 110)=110.
      // Clamped by parent_bottom - e_h = 970 → 110. Displacement = 60.
      // Bottom inset is ignored.
      assert_eq!(d, Vec2::new(0.0, 60.0));
  }
  ```
  Run: expected PASS (logic should compile and match these assertions).

- [ ] **Step 7: Implement `sticky_offset` system.** Top-level Bevy system:
  ```rust
  /// Sub-pass 6a — sticky offset.
  ///
  /// For each entity with `Position::Sticky`:
  /// 1. Find nearest scroll-container ancestor via `nearest_scroll_container`.
  /// 2. If none, skip (no warn — silent no-op per D5).
  /// 3. Compute world positions in scroll-container frame.
  /// 4. Resolve insets per `resolve_sticky_inset`.
  /// 5. Compute displacement per `compute_sticky_displacement`.
  /// 6. Write `entity natural-relative-to-parent + displacement` to
  ///    `PostTaffyPositionOverrides.by_entity`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.3.
  #[allow(clippy::too_many_arguments)]
  pub(super) fn sticky_offset(
      tree: NonSend<LayoutTree>,
      sticky_query: Query<(Entity, &Position, &Display), With<Node>>,
      overflow_q: Query<&Overflow>,
      scroll_offset_q: Query<&ScrollOffset>,
      parent_chain: Query<&ChildOf>,
      mut overrides: ResMut<PostTaffyPositionOverrides>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      let mut memo: std::collections::HashMap<Entity, Vec2> = std::collections::HashMap::new();

      for (e, pos, display) in sticky_query.iter() {
          if !matches!(pos.kind, PositionKind::Sticky) || matches!(display, Display::None) {
              continue;
          }
          let Some(scroll_container) = nearest_scroll_container(e, &parent_chain, &overflow_q) else {
              continue; // D5 — no scroll container, silent no-op
          };

          // Read sizes from Taffy.
          let Some(e_node) = tree.by_entity.get(&e) else { continue };
          let Ok(e_layout) = tree.tree.layout(*e_node) else { continue };
          let e_size = Vec2::new(e_layout.size.width, e_layout.size.height);
          let e_natural_rel = Vec2::new(e_layout.location.x, e_layout.location.y);

          let Ok(parent_co) = parent_chain.get(e) else { continue };
          // v2 fix: ChildOf accessor is .parent() in Bevy 0.18.
          let parent = parent_co.parent();
          let Some(parent_node) = tree.by_entity.get(&parent) else { continue };
          let Ok(parent_layout) = tree.tree.layout(*parent_node) else { continue };
          let parent_size = Vec2::new(parent_layout.size.width, parent_layout.size.height);

          let Some(s_node) = tree.by_entity.get(&scroll_container) else { continue };
          let Ok(s_layout) = tree.tree.layout(*s_node) else { continue };
          let s_size = Vec2::new(s_layout.size.width, s_layout.size.height);

          let Some(e_in_s) = world_position(e, scroll_container, &tree, &parent_chain, &mut memo)
          else { continue };
          let Some(parent_in_s) = world_position(parent, scroll_container, &tree, &parent_chain, &mut memo)
          else { continue };

          let scroll_offset = scroll_offset_q.get(scroll_container).copied().unwrap_or_default();

          // Resolve insets. v2: signature is (sizing, scroll_container_axis_size, entity, warned).
          let top = resolve_sticky_inset(&pos.inset.top,    s_size.y, e, &mut warned);
          let bottom = resolve_sticky_inset(&pos.inset.bottom, s_size.y, e, &mut warned);
          let left = resolve_sticky_inset(&pos.inset.left,   s_size.x, e, &mut warned);
          let right = resolve_sticky_inset(&pos.inset.right,  s_size.x, e, &mut warned);

          let displacement = compute_sticky_displacement(
              e_in_s, e_size, parent_in_s, parent_size, s_size,
              Vec2::new(scroll_offset.x, scroll_offset.y),
              top, bottom, left, right,
          );

          if displacement == Vec2::ZERO {
              continue; // No displacement — leave the override map untouched.
          }

          overrides.by_entity.insert(e, e_natural_rel + displacement);
      }
  }
  ```
  **Implementer notes:**
  - The `ScrollOffset` is the Phase-2 component `crates/buiy_core/src/layout/components.rs:361-380`. Default is zero per `ScrollOffset { x: 0.0, y: 0.0 }`.
  - The `Position` component is `crates/buiy_core/src/layout/components.rs:89-110`.
  - All Bevy query types are the same shapes used in `anchor_resolution`.
  - Add necessary imports at the top of `systems.rs`: `OverflowMode` (likely already present), `ScrollOffset`.
  - **v2:** `StickyAxis` enum is dropped from v1 — `resolve_sticky_inset` does not need an axis parameter (the caller passes the correct `scroll_container_axis_size`). `viewport: Vec2` is dropped — no viewport-unit variants exist in `Length` currently. `primary_window` query is removed from `sticky_offset` signature.

- [ ] **Step 8: Project gate + integration tests will follow in Task 10.** For now run the unit tests:
  ```bash
  cargo test -p buiy_core sticky_
  ```
  Expected: all sticky_* tests in `systems.rs::mod tests` PASS. Component-level tests will be added in Task 10.

- [ ] **Step 9: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): sticky_offset system — sub-pass 6a (Phase 7)

Full sticky-positioning implementation:
- nearest_scroll_container ancestor walk
- world_position helper (per-call memoized)
- resolve_sticky_inset (px/percent active; Fr + Cq* warn-once-defer; closed match)
- compute_sticky_displacement (pure function, per-axis algorithm per D4)
- sticky_offset system writes to PostTaffyPositionOverrides

Sticky behaves as Relative when no scroll-container ancestor is in scope (D5,
silent no-op). Em/rem and fr insets resolve to 0.0 with one warn per
(entity, session)."
  ```

### Task 6: `table_layout` system (sub-pass 6b stub)

**Spec:** [`display-and-positioning.md § 1.2`](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md#12-table-layout-status).

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `table_layout` system)

- [ ] **Step 1: Failing test.**
  ```rust
  #[test]
  fn table_layout_warns_once_per_entity() {
      let mut app = App::new();
      app.init_resource::<LayoutWarnedOnceSession>();
      app.add_systems(Update, table_layout);
      // Spawn a table entity.
      let e = app.world_mut().spawn((Node, Display::Table)).id();
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert!(warned.set.contains(&LayoutWarnOnceKey::TableUnsupported(e)));

      // Run again — should not duplicate.
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert_eq!(warned.set.iter().filter(|k| matches!(k, LayoutWarnOnceKey::TableUnsupported(_))).count(), 1);
  }
  ```
  Run: expected FAIL.

- [ ] **Step 2: Implement.**
  ```rust
  /// Sub-pass 6b — table layout stub.
  ///
  /// Spec § 1.2: "v1 ships only the API surface and the fallback path;
  /// the full algorithm is deferred to a v1.x point release." The
  /// fallback path (Table → Block) is handled by `translate.rs`. This
  /// sub-pass exists solely to emit a `warn!` once per (entity,
  /// session) the first time each `Display::Table*` value is
  /// encountered.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
  pub(super) fn table_layout(
      table_q: Query<(Entity, &Display), With<Node>>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      for (e, d) in table_q.iter() {
          if !is_table_display(d) {
              continue;
          }
          if warned.set.insert(LayoutWarnOnceKey::TableUnsupported(e)) {
              bevy::log::warn!(
                  "Layout: Display::Table* on entity {:?} — table layout algorithm is deferred to v1.x (spec § 1.2). Falling back to Display::Block. Use Display::Grid for v1 table-like layouts.",
                  e,
              );
          }
      }
  }

  fn is_table_display(d: &Display) -> bool {
      matches!(d,
          Display::Table | Display::TableRowGroup | Display::TableHeaderGroup
          | Display::TableFooterGroup | Display::TableRow | Display::TableCell
          | Display::TableCaption | Display::TableColumnGroup | Display::TableColumn
      )
  }
  ```

- [ ] **Step 3: Tests PASS.**
  ```bash
  cargo test -p buiy_core table_layout
  ```

- [ ] **Step 4: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): table_layout system — sub-pass 6b (Phase 7 stub)

Per spec § 1.2, table-layout algorithm is deferred to v1.x. The fallback path
(Display::Table* → Display::Block via translate.rs::map_display) already ships.
This sub-pass emits one warn per (entity, session) on first encounter of any
Display::Table* variant. Dedup via LayoutWarnedOnceSession.set."
  ```

### Task 7: `multicol_pack` system (sub-pass 6c stub)

**Spec:** [`flex-and-grid.md § 3.2`](../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#32-algorithm).

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `multicol_pack` system)

- [ ] **Step 1: Failing test.**
  ```rust
  #[test]
  fn multicol_pack_warns_once_per_session() {
      let mut app = App::new();
      app.init_resource::<LayoutWarnedOnceSession>();
      app.add_systems(Update, multicol_pack);
      // First multicol entity triggers.
      let _e1 = app.world_mut().spawn((Node, MultiColumn::default())).id();
      let _e2 = app.world_mut().spawn((Node, MultiColumn::default())).id();
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      // Single warn — no Entity payload.
      assert_eq!(
          warned.set.iter().filter(|k| matches!(k, LayoutWarnOnceKey::MulticolUnsupported)).count(),
          1,
      );

      // Run again — should not duplicate.
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert_eq!(
          warned.set.iter().filter(|k| matches!(k, LayoutWarnOnceKey::MulticolUnsupported)).count(),
          1,
      );
  }
  ```
  Run: expected FAIL.

- [ ] **Step 2: Implement.**
  ```rust
  /// Sub-pass 6c — multi-column packing stub.
  ///
  /// Spec § 3.2 (`flex-and-grid.md`): "Multi-column is tier-E; v1 ships
  /// the API but the algorithm is a stub that produces single-column
  /// layout with `warn!` once per session." This sub-pass emits the
  /// single warn — no per-entity tracking.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
  pub(super) fn multicol_pack(
      multicol_q: Query<&MultiColumn>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      if multicol_q.iter().next().is_none() {
          return; // No multicol entities; no warn.
      }
      if warned.set.insert(LayoutWarnOnceKey::MulticolUnsupported) {
          bevy::log::warn!(
              "Layout: MultiColumn detected — multi-column packing algorithm is deferred to v1.x (flex-and-grid.md § 3.2). Falling back to single-column layout. This warn fires once per session."
          );
      }
  }
  ```

- [ ] **Step 3: Tests PASS.**
  ```bash
  cargo test -p buiy_core multicol_pack
  ```

- [ ] **Step 4: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): multicol_pack system — sub-pass 6c (Phase 7 stub)

Per spec § 3.2 (flex-and-grid.md), the multi-column algorithm is deferred to
v1.x. Sub-pass 6c emits a single warn per session — no per-entity dedup, no
Entity payload in the warn-once-key. Component is the MultiColumn introduced
in Task 3."
  ```

### Task 8: Wire all systems with explicit chain ordering in `mod.rs`

**Spec:** D1, D2. [`architecture.md § 3`](../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline).

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs:140-153` (replace the single `anchor_resolution.in_set(...)` with chained tuple)
- Modify: `crates/buiy_core/src/layout/mod.rs:90+` (register new types via `register_type`)
- Modify: `crates/buiy_core/src/lib.rs` (re-exports)
- Modify: `crates/buiy/src/lib.rs` (top-level facade re-exports)

- [ ] **Step 1: Update `add_systems` block in `mod.rs`.** Find the existing block that ends with:
  ```rust
  systems::anchor_resolution.in_set(BuiyLayoutStep::PostTaffyOverrides),
  systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
  ```
  Replace with:
  ```rust
  (
      systems::clear_post_taffy_overrides,
      systems::sticky_offset,
      systems::table_layout,
      systems::multicol_pack,
      systems::anchor_resolution,
  )
      .chain()
      .in_set(BuiyLayoutStep::PostTaffyOverrides),
  systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
  ```

- [ ] **Step 2: Register new types in `mod.rs`.** Find the `register_type::<...>()` chain (around line 90+). Add:
  ```rust
  .register_type::<MultiColumn>()
  .register_type::<ColumnCount>()
  .register_type::<ColumnRule>()
  .register_type::<ColumnRuleStyle>()
  .register_type::<ColumnSpan>()
  .register_type::<ColumnFill>()
  .register_type::<BreakInside>()
  .register_type::<BreakBefore>()
  .register_type::<BreakAfter>()
  ```
  Also update the import at the top of `mod.rs` (the `use components::{...}` line) to include `MultiColumn` and the new types.

- [ ] **Step 3: Re-exports in `crates/buiy_core/src/lib.rs`.** Find the existing layout re-export line (Phase 6 added `Anchor, AnchorErrorKind, ...`). Extend:
  ```rust
  pub use layout::{
      // ... existing ...
      BreakAfter, BreakBefore, BreakInside, ColumnCount, ColumnFill, ColumnRule, ColumnRuleStyle,
      ColumnSpan, LayoutWarnOnceKey, MultiColumn,
  };
  ```

- [ ] **Step 4: Re-exports in `crates/buiy/src/lib.rs`** (top-level facade). Mirror Step 3.

- [ ] **Step 5: Update pipeline-order test (next task — Task 11), but for now run the project gate.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  ```
  Expected: all green. If any test fails, the most likely cause is the `clear_post_taffy_overrides` now removes overrides written in a frame that the prior pass-order test fixture expected — that's adjusted in Task 11.

- [ ] **Step 6: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
  git commit -m "feat(layout): wire Phase 7 sub-passes into PostTaffyOverrides chain

Replaces single anchor_resolution attach with chained tuple of 5 systems:
clear_post_taffy_overrides → sticky_offset → table_layout → multicol_pack →
anchor_resolution. Registers MultiColumn + 8 supporting types for reflection.
Re-exports new public types from both buiy_core and buiy facade crates."
  ```

### Task 9: Anchor target position lookup falls back to `PostTaffyPositionOverrides`

**Spec:** D1 (architectural quirk fix from follow-up).

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs:485+` (`anchor_resolution` body — anchor target position lookup)

- [ ] **Step 1: Failing test (integration test will land in Task 10).**
  Sketch of the integration test:
  - Spawn an entity A that is sticky and inside a scroll container.
  - Spawn entity B with `Anchor { position_anchor: Some(AnchorRef::Entity(A)), .. }`.
  - Scroll the container (move `ScrollOffset`).
  - Assert B's `ResolvedLayout.position` reflects A's *displaced* position, not A's natural Taffy position.

  Write this test in `tests/layout_sticky.rs` in Task 10 alongside other integration tests — Task 9 just makes the test possible.

- [ ] **Step 2: Find the anchor target position read in `anchor_resolution`.** It's roughly at `systems.rs:600+` where the code does something like:
  ```rust
  let anchor_box_position = Vec2::new(anchor_layout.location.x, anchor_layout.location.y);
  ```
  Replace with a helper that consults `PostTaffyPositionOverrides` first:
  ```rust
  let anchor_box_position = overrides
      .by_entity
      .get(&target_entity)
      .copied()
      .unwrap_or_else(|| Vec2::new(anchor_layout.location.x, anchor_layout.location.y));
  ```
  **Implementer note:** the exact line number depends on the Phase-6 code shape. Search for the comment "Read the anchor's box from `tree.tree.layout`" or similar in `anchor_resolution`. Apply the override-fallback at that read site.

- [ ] **Step 3: Run anchor + sticky tests in isolation.** No new tests yet (added in Task 10), but the existing Phase-6 anchor tests should still pass.
  ```bash
  cargo test -p buiy_core --test layout_anchor_positioning
  ```

- [ ] **Step 4: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "fix(layout): anchor target position honors PostTaffyPositionOverrides

Closes the architectural quirk from Phase 6 follow-up: an anchor target that is
itself sticky-displaced (or table/multicol corrected in a future phase) would
have its anchored elements computed against the un-displaced Taffy position.
After this change, anchor_resolution (6d) consults the override map written by
6a/6b/6c before falling back to tree.tree.layout()."
  ```

### Task 10: Integration tests — sticky behavior + table/multicol stubs

**Spec:** D3, D4, D5, D9, D10, D11.

**Files:**
- Create: `crates/buiy_core/tests/layout_sticky.rs`
- Create: `crates/buiy_core/tests/layout_table_multicol_stubs.rs`

- [ ] **Step 1: Create `layout_sticky.rs` with test scaffolding.** Mirror the Phase 6 integration test shape (`tests/layout_anchor_positioning.rs`). Use `App::new()` + `BuiyCorePlugin` + spawn fixtures + `app.update()` + assertions on `ResolvedLayout`.

  Tests to include (each as a separate `#[test]` function). v2 added tests T5-T11 to cover bottom-pin branches (test reviewer BLOCKER B1), both-active conflict (B2), clear-ordering regression (B4), explicit empty assertion (B5), and dropped the em/rem test (variant no longer exists).

  **Top-pin tests (carried from v1):**
  1. `sticky_pins_to_top_during_scroll` — sticky element with `top: 0px` inside a scrolling parent; scroll the parent, assert sticky position increases by scroll amount.
  2. `sticky_does_not_pull_up_before_scroll` — sticky element starts at y=50 with `top: 0px`; no scroll; assert sticky position == natural position.
  3. `sticky_clamped_by_parent_bottom` — sticky element inside a small parent; scroll enough that the threshold would push it past the parent bottom; assert it's clamped.

  **Bottom-pin tests (v2 — new per BLOCKER B1):**
  4. `sticky_bottom_pins_when_scroll_near_bottom` — element at natural y=700, parent height 1000, scroll container height 500, scroll_offset.y=300, bottom inset 10px. visible_bottom=800, threshold=790. Assert displacement is negative (element held at bottom edge of viewport).
  5. `sticky_bottom_does_not_push_down_before_scroll` — same setup as #4 but scroll_offset.y=0; threshold puts element above its natural position; the no-push-down clamp should fire; displacement should be zero.
  6. `sticky_bottom_clamped_by_parent_top` — scroll near the bottom enough that the bottom threshold would push the element above `parent_in_s.y`; assert clamp holds it at parent_top.

  **Both-active conflict test (v2 — new per BLOCKER B2):**
  7. `sticky_both_top_and_bottom_inset_top_wins` — element with both `inset_top = Some(10.0)` and `inset_bottom = Some(10.0)`, scrolled to a position where bottom threshold would displace but top threshold wouldn't (or vice versa); assert top-wins behavior; comment documents the v1 deviation so future correct-dual-clamp implementation knows what to change.

  **No-scroll-container test (v2 — strengthened per BLOCKER B5):**
  8. `sticky_no_scroll_container_is_no_op` — sticky element with no scroll container ancestor; after `app.update()`, **explicitly assert `app.world().resource::<PostTaffyPositionOverrides>().by_entity.is_empty()`** (not just that ResolvedLayout.position is unchanged — that would always pass for a no-displacement case).

  **Other coverage:**
  9. `sticky_percent_inset_against_scroll_container` — sticky element with `top: 10%`; scroll container is 200px tall; assert threshold is `scroll_offset.y + 20.0`.
  10. `sticky_cq_inset_deferred_resolves_to_zero_with_warn` (v2 — new) — sticky element with `Sizing::Length(Length::Cqw(20.0))` as top inset; assert position == natural (cq → 0 → no displacement); assert `LayoutWarnedOnceSession.set` contains `StickyCqDeferred(e)`.
  11. `sticky_fr_inset_resolves_to_zero_with_warn` (v2 — new) — sticky element with `Sizing::Length(Length::Fr(2.0))` as top inset; assert position == natural; assert `LayoutWarnedOnceSession.set` contains `StickyFrUnsupported(e)`.
  12. `sticky_in_nested_scroll_containers_uses_innermost` (D9) — two nested scroll containers; sticky inside the inner; scroll the outer but not the inner; assert sticky position unchanged.
  13. `sticky_display_none_is_skipped` (D10) — sticky element with `Display::None`; assert no override.
  14. `anchor_target_is_sticky_anchored_tracks_displaced_position` (closes the follow-up) — sticky target + anchored element; scroll; assert anchored element follows the displaced target.
  15. `clear_ordering_regression_two_frames` (v2 — new per BLOCKER B4) — spawn a sticky entity inside a scroll container, run frame 1 with scroll_offset=100, run frame 2 with scroll_offset=200; assert the override map's entry for the sticky entity on frame 2 reflects frame 2's displacement (not frame 1's leftover). Catches a regression where `clear_post_taffy_overrides` is moved to AFTER `sticky_offset` in the chain.

- [ ] **Step 2: Create `layout_table_multicol_stubs.rs`.** v2 added tests T6-T8 per CONCERNs.
  1. `table_warns_once_per_entity_per_session` — spawn two Table entities; run three frames; assert exactly two warns recorded (one per entity, each fires once, never duplicates across frames).
  2. `table_no_warn_when_no_table_entities` — empty world (no Table); assert no warn.
  3. `multicol_warns_once_per_session_regardless_of_entity_count` — spawn **3** MultiColumn entities (v2 — bumped from 2); assert exactly 1 `MulticolUnsupported` warn.
  4. `multicol_no_warn_when_no_multicol_entities` — empty world; no warn.
  5. `table_and_multicol_warns_are_independent` — spawn one Table + one MultiColumn; assert one `TableUnsupported(e)` + one `MulticolUnsupported`.
  6. `table_does_not_rewarn_on_component_replace` (v2 — new per CONCERN C3) — spawn a table entity (frame 1, 1 warn); re-insert `Display::Table` on the same entity via `world.entity_mut(e).insert(Display::Table)` (frame 2); assert warn count is still 1 (entity identity unchanged).
  7. `warned_once_session_manual_clear` (v2 — new per CONCERN C4) — pre-seed `LayoutWarnedOnceSession.set` with a `TableUnsupported` key; call `clear_warned_once_on_exit` as a direct system invocation; assert set is empty. Documents the expected `OnExit` behavior even while the `BuiyExit` wire-up is deferred.
  8. `table_all_nine_variants_each_warn` (v2 — new edge case) — spawn one entity per `Display::Table*` variant (9 total); assert 9 distinct `TableUnsupported(Entity)` keys in the session set. Validates `is_table_display` covers all variants.

- [ ] **Step 3: Run the integration suite.**
  ```bash
  cargo test -p buiy_core --test layout_sticky --test layout_table_multicol_stubs
  ```
  Expected all PASS.

- [ ] **Step 4: Commit.**
  ```bash
  git add crates/buiy_core/tests/layout_sticky.rs crates/buiy_core/tests/layout_table_multicol_stubs.rs
  git commit -m "test(buiy_core): Phase 7 integration tests — sticky behavior + stub warns

15 sticky tests covering scroll-pin (top + bottom + both), no-pull-up, no-push-down,
parent-clamp, no-scroll-container (empty assertion), percent inset, Cq deferred,
Fr unsupported, nested-innermost, Display::None, anchor-target-is-sticky,
clear-ordering regression. 8 stub-warn tests covering per-(entity,session) for
table including 9-variant coverage and component-replace dedup, session-wide
for multicol with 3 entities, independence, manual session-set clear."
  ```

### Task 11: Augment `tests/layout_pipeline_order.rs` fixture

**Spec:** D15.

**Files:**
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs`

- [ ] **Step 1: Read the existing fixture.** It already includes a Phase-6 anchor target + anchored entity pair (added in Phase 6).

- [ ] **Step 2: Extend with Phase 7 fixtures.**
  - Add: a scrolling container + a sticky child entity.
  - Add: a `Display::Table` entity.
  - Add: a `MultiColumn` entity.

- [ ] **Step 3: Update assertions.**
  - Assert `PostTaffyPositionOverrides.by_entity` contains an entry for the sticky entity (when scroll offset is nonzero).
  - Assert `LayoutWarnedOnceSession.set` contains `TableUnsupported(_)`.
  - Assert `LayoutWarnedOnceSession.set` contains `MulticolUnsupported`.
  - Assert anchor entry still works (Phase 6 invariant unchanged).
  - **v2 — explicit intra-sub-pass ordering proof (BLOCKER B3).** Include in the fixture: a sticky entity that is also an anchor target (sticky's displaced position is what `anchor_resolution` should read). After `app.update()`, assert that the anchored entity's `ResolvedLayout.position` reflects the displaced sticky position, not the natural Taffy position. This is the only test that distinguishes "sub-passes ran in declared order" from "sub-passes ran at all" — if `anchor_resolution` ran before `sticky_offset`, the anchored entity would track the un-displaced position. Cross-references `anchor_target_is_sticky_anchored_tracks_displaced_position` in `tests/layout_sticky.rs` (Task 10) for the standalone behavior test; this fixture-level assertion exercises the same invariant in the cross-phase pipeline test so a reader knows the ordering invariant is covered without needing to find it in another file.

- [ ] **Step 4: Run.**
  ```bash
  cargo test -p buiy_core --test layout_pipeline_order
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/tests/layout_pipeline_order.rs
  git commit -m "test(buiy_core): pipeline_order fixture covers Phase 7 sub-passes

Adds sticky / table / multicol entities to the cross-phase fixture so the
4-sub-pass chain (6a→6b→6c→6d) is asserted end-to-end alongside the Phase 6
anchor pair."
  ```

### Task 12: Closeout — final whole-branch review + CHANGELOG + follow-ups + PR

**Spec:** All of the above. Mirrors Phase 6 closeout cadence.

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/follow-ups.md`

- [ ] **Step 1: Final whole-branch review.** Dispatch one fresh `code-reviewer` agent over the full diff (from main). Address any BLOCKERs. Re-run the project gate to confirm green.

- [ ] **Step 2: CHANGELOG.md additions.**
  ```markdown
  ## Phase 7 — Layout: sticky positioning + table stub + multicol stub

  ### Added
  - `sticky_offset` system (sub-pass 6a) — full implementation of CSS sticky positioning per spec § 2.3. Walks `ChildOf` for nearest scroll container, computes per-axis displacement, writes to the shared override map.
  - `table_layout` system (sub-pass 6b) — no-op stub per spec § 1.2; emits one `warn!` per (entity, session) on first encounter of any `Display::Table*` variant.
  - `multicol_pack` system (sub-pass 6c) — no-op stub per `flex-and-grid.md` § 3.2; emits one `warn!` per session on first encounter of any `MultiColumn`.
  - `MultiColumn` component + 8 supporting enums (`ColumnCount`, `ColumnRule`, `ColumnRuleStyle`, `ColumnSpan`, `ColumnFill`, `BreakInside`, `BreakBefore`, `BreakAfter`). Tier-E API surface; algorithm deferred to v1.x.
  - `LayoutWarnedOnceSession` resource + `LayoutWarnOnceKey` enum (variants: `TableUnsupported(Entity)`, `MulticolUnsupported`, `StickyFrUnsupported(Entity)`, `StickyCqDeferred(Entity)`). Session-scoped warn dedup per spec § 6. (v1 plan's `StickyEmRemDeferred` variant dropped — `Length::Em(_)` and `Length::Rem(_)` do not exist in the codebase.)
  - `clear_post_taffy_overrides` system — dedicated per-frame clear for the shared override map. Runs first in `PostTaffyOverrides`.
  - 23 integration tests across `tests/layout_sticky.rs` + `tests/layout_table_multicol_stubs.rs` + augmented `tests/layout_pipeline_order.rs` (v2 — expanded from v1's 14 per test-reviewer findings).
  - 4 new sticky unit tests covering bottom-pin (3 cases) + both-active conflict (1 case) on `compute_sticky_displacement` (v2 — covers BLOCKER B1 + B2).
  - `Style.multi_column` field + `Style::multi_column()` fluent setter per spec § 2.4 (container-side property convention). Field always inserted (Style derives Bundle).
  - `Changed<MultiColumn>` added to `sync_styles` nested Or<> filter per spec architecture.md § 1.2 (forward-compat for v1.x multicol algorithm).

  ### Changed
  - Renamed `AnchorOverrides` → `PostTaffyPositionOverrides` (Phase 6 → Phase 7). Same shape (`HashMap<Entity, Vec2>`), same per-frame-cleared semantics; widened scope to "any sub-pass writes." This is a public type rename — code that referenced `AnchorOverrides` directly must update.
  - `anchor_resolution` (sub-pass 6d) now reads `PostTaffyPositionOverrides` when looking up anchor target positions, so anchors that target a sticky-displaced element produce correct dependents. Closes the architectural quirk tracked in `docs/plans/follow-ups.md` "Anchor positioning — anchor target IS sticky/table/multicol".
  - `BuiyLayoutStep::PostTaffyOverrides` set now contains five systems chained in declared order: `clear_post_taffy_overrides → sticky_offset → table_layout → multicol_pack → anchor_resolution` (Phase 6 attached only `anchor_resolution`).

  ### Deferred / divergences
  - **`Position::Fixed` — still a warn-once stub.** Phase 7's spec scope (per architecture.md § 3) is sub-passes 6a/6b/6c — Fixed is mapped to "Absolute with viewport-as-CB" which is a separate code path. Tracked in `follow-ups.md`.
  - **Multi-column algorithm — deferred to v1.x.** Per spec § 3.2 (`flex-and-grid.md`): "Multi-column is tier-E; v1 ships the API but the algorithm is a stub."
  - **Table layout algorithm — deferred to v1.x.** Per spec § 1.2: "v1 ships only the API surface and the fallback path; the full algorithm is deferred to a v1.x point release." The fallback path (Table → Block) already ships from Phase 1.
  - **Sticky `Length::Cq*` inset — deferred.** Full container-query-unit resolution for sticky requires plumbing a cq-context lookup analogous to (but distinct from) Phase 6's `length_inset_to_px` (sticky's reference frame is the sticky entity's own cq-ancestor, not the anchor target). v1 resolves to 0.0 with `LayoutWarnOnceKey::StickyCqDeferred(Entity)` warn per (entity, session). Tracked in `follow-ups.md`.
  - **Sticky `Length::Fr` inset — invalid.** `fr` is a grid-only unit; applying it to a sticky inset is semantically wrong. v1 resolves to 0.0 with `LayoutWarnOnceKey::StickyFrUnsupported(Entity)` warn per (entity, session).
  - **No em/rem support in sticky insets.** `Length::Em(_)` and `Length::Rem(_)` do not exist as `Length` variants in v1 (per `types.rs:29-50`). When Phase 10 (or a font-rendering phase) adds them, the sticky inset resolver gains new arms.
  - **Both-top-and-bottom-inset sticky — "top wins".** Per D4, when both insets are set, top wins. Documented v1 deviation; CSS spec § 6.3 implies a dual-clamp ("element sticks to whichever edge the scroll position is closer to") but Phase 7's simpler "top wins" matches WebKit/Blink in the common case. Tracked in `follow-ups.md`.
  - **Sticky inside sticky — inner uses outer's natural position.** `world_position` walks Taffy positions (un-displaced); inner sticky elements resolve their thresholds against the outer's *natural* position, not the displaced one. Rare authoring case; tracked in `follow-ups.md` for v1.x.
  - **`LayoutWarnedOnceSession` `BuiyExit` clear — wiring deferred.** The clear function exists; the wire-up to `OnExit(BuiyState::Active)` depends on the foundation lifecycle which is still draft. Tracked in `follow-ups.md`. Until wired, `App::new()` in tests starts with a clean resource (Bevy default).

  ### Removed
  - None.

  ### Performance contract
  - Steady-state O(0) preserved: `clear_post_taffy_overrides` and the four sub-passes are all `O(matching entities)`. Sticky-zero-entities → no work. Table-zero-entities → no work. Multicol-zero-entities → no work.
  - Sticky cost: `O(sticky entities × ancestor depth × per-axis)`. Typical: <10 sticky elements × <20 depth = <400 ops/frame.
  ```

- [ ] **Step 3: Add Phase 7 follow-ups to `docs/plans/follow-ups.md`.**
  - **Layout — `Position::Fixed` implementation.** Sketch: change `translate.rs::map_position` to emit `taffy::Position::Absolute` for `Fixed`, override the `ContainingBlock` resolution to point at the layout root. Single `sync_styles` change + a private `is_fixed_root` flag.
  - **Layout — full table algorithm.** Sketch: replace `table_layout` stub with the algorithm described in `display-and-positioning.md § 1.2` ("Gather entities by Display::Table* family. Compute column widths via Taffy on a synthetic flex container per row group. Write corrected positions back to `PostTaffyPositionOverrides`").
  - **Layout — full multicol algorithm.** Sketch: replace `multicol_pack` stub with a packing pass that respects `column_count` / `column_width` + `break-*` properties. Write each child's `PostTaffyPositionOverrides` entry.
  - **Layout — sticky `Length::Cq*` inset resolution.** Plumb container-query context from Phase 6's `length_inset_to_px` helper. v1 ships a `StickyCqDeferred` warn-once; the follow-up wires through the actual cq-context (sticky's reference frame is the sticky entity's own nearest CQ ancestor — distinct from anchor's "anchor target box" frame). Multi-axis fixture needed (Cqi/Cqb resolve against writing-mode inline/block axes).
  - **Layout — sticky em/rem inset support.** When `Length::Em(_)` and `Length::Rem(_)` are added to the `Length` enum (Phase 10 or font-rendering phase), extend `resolve_sticky_inset` with new arms (currently a closed match so the compiler will force the change).
  - **Layout — sticky `Length::Vh/Vw/Vmin/Vmax` inset support.** Same as em/rem follow-up; Phase 10 viewport-units extension.
  - **Layout — sticky both-top-and-bottom inset dual clamp.** v1 implements "top wins" per D4. The CSS spec § 6.3 implies a dual-clamp where the element sticks to whichever edge the scroll position is currently closer to. Follow-up: implement the dual-clamp (likely requires storing both upper and lower sticky thresholds and computing midpoint logic in `compute_sticky_displacement`). The v2 test `sticky_both_top_and_bottom_active_top_wins` becomes the regression test for the v1 behavior; flipping it documents the algorithm upgrade.
  - **Layout — sticky inside sticky.** v1's `world_position` walks Taffy positions (un-displaced); inner sticky elements resolve their thresholds against the outer's *natural* position. Follow-up: consult `PostTaffyPositionOverrides` (just-written by 6a) when walking the ancestor chain, so inner sticky sees the displaced outer. Requires careful ordering (inner sticky must run after outer; topological pre-pass or two-frame eventual-consistency are both options).
  - **Layout — `clear_warned_once_on_exit` lifecycle wire-up.** Once foundation lifecycle states are settled, wire the clear via `app.add_systems(OnExit(BuiyState::Active), ...)`. Until then, the function is exposed but never called — tests can invoke directly via `world.run_system_once(clear_warned_once_on_exit)`.

- [ ] **Step 4: Open PR.**
  ```bash
  git push -u origin worktree-v01-layout-sticky-table-multicol
  gh pr create --title "feat(layout): Phase 7 — sticky positioning + table stub + multicol stub" --body "$(cat <<'EOF'
  ## Summary

  - **Sticky positioning (sub-pass 6a, full implementation):** `sticky_offset` walks `ChildOf` for the nearest scroll-container ancestor, computes per-axis displacement (CSS spec § 6.3 algorithm: max(natural, threshold) clamped by parent), writes to the shared `PostTaffyPositionOverrides`. Tier-F.
  - **Table layout stub (sub-pass 6b):** `table_layout` emits one warn per (entity, session) for any `Display::Table*` variant. Fallback path (Table → Block) already ships from Phase 1. Tier-C, algorithm deferred to v1.x per spec § 1.2.
  - **Multi-column stub (sub-pass 6c):** `multicol_pack` emits one warn per session on first `MultiColumn` encounter. `MultiColumn` component + 8 supporting enums added as tier-E API surface. Algorithm deferred to v1.x per spec § 3.2.
  - **Architectural refactor:** `AnchorOverrides` → `PostTaffyPositionOverrides` (shared by all four sub-passes). `anchor_resolution` (6d) reads from the map for target lookups so anchors that target sticky-displaced elements produce correct dependents — closes the Phase-6 follow-up "anchor target IS sticky/table/multicol".
  - **Lifecycle:** dedicated `clear_post_taffy_overrides` system runs first in `PostTaffyOverrides`; sub-passes chain after it.

  Spec: `docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md` § 1.2 + § 2.3, `flex-and-grid.md` § 3, `architecture.md` § 3.

  Plan: `docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md`.

  ## Test plan

  - [x] Unit tests for `compute_sticky_displacement` (no-inset, top-pins-when-scrolled, no-pull-up-before-scroll, parent-clamp).
  - [x] Unit tests for `clear_post_taffy_overrides`, `nearest_scroll_container`, `world_position` memoization, `LayoutWarnedOnceSession` dedup.
  - [x] Integration tests in `tests/layout_sticky.rs` (15 scenarios — v2 expanded from v1's 9: top-pin trio, bottom-pin trio, both-active conflict, no-scroll-container, percent-inset, Cq-deferred, Fr-unsupported, nested-innermost, Display::None, anchor-target-is-sticky, clear-ordering-regression).
  - [x] Integration tests in `tests/layout_table_multicol_stubs.rs` (8 scenarios — v2 expanded from v1's 5: per-entity table dedup, no-table-no-warn, multicol-3-entity session dedup, no-multicol-no-warn, independence, table-no-rewarn-on-replace, manual-session-clear, all-9-table-variants).
  - [x] Unit tests in `systems.rs::mod tests` (4 new sticky-displacement tests added in v2: bottom-pins-when-near-bottom, bottom-no-push-down, bottom-clamped-by-parent-top, both-active-top-wins).
  - [x] Pipeline-order fixture (`tests/layout_pipeline_order.rs`) extended with sticky + table + multicol entities.
  - [x] All Phase 1-6 tests still pass (Phase 6 anchor suite, container queries, writing modes, grid, scrolling, foundations).
  - [x] All 6 CI gates green (Deny, Doc, Lint, Test ubuntu/macos/windows).

  🤖 Generated with [Claude Code](https://claude.com/claude-code)
  EOF
  )"
  ```

- [ ] **Step 5: Wait for CI, merge if green.** Watch the 6 gates (Deny, Doc, Lint, Test ubuntu/macos/windows). Fix any failures inline (per Phase 6 cadence). Squash-merge once all green.

- [ ] **Step 6: Flip plan to `[landed]` on main.**
  ```bash
  git checkout main && git pull --ff-only origin main
  # Edit docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md:
  #   **Status:** active → **Status:** landed
  # Edit docs/README.md:
  #   [active] → [landed]
  git add docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md docs/README.md
  git commit -m "docs: mark Phase 7 layout plan [landed]"
  git push origin main
  ```

- [ ] **Step 7: Cleanup.**
  ```bash
  git worktree remove --force .claude/worktrees/v01-layout-sticky-table-multicol
  git branch -D worktree-v01-layout-sticky-table-multicol
  git fetch --prune
  ```

---

## Self-review

1. **Spec coverage:**
   - `display-and-positioning.md § 1.2` (table) → Task 6.
   - `display-and-positioning.md § 2.3` (sticky) → Task 5.
   - `flex-and-grid.md § 3` (multicol) → Tasks 3 + 7.
   - `architecture.md § 3` (sub-pass ordering) → Tasks 2 + 8.
   - `architecture.md § 6` (error model) → Tasks 4 + 6 + 7.
   - Phase 6 follow-up architectural quirk → Tasks 2 + 9.

2. **Placeholder scan (v2):** No "TBD" or "implement later" terms. Implementer-choice points reduced to one — (a) sticky `Cq*` resolution is **decided in v2** as the deferred path with `StickyCqDeferred` warn-once; (b) `BuiyExit` clear wire-up is the remaining true implementer-choice (lifecycle states still in foundation draft).

3. **Type consistency (v2):**
   - `PostTaffyPositionOverrides` referenced consistently across T2/T5/T6/T7/T8/T9/T10/T11.
   - `LayoutWarnOnceKey` variants `TableUnsupported(Entity) / MulticolUnsupported / StickyFrUnsupported(Entity) / StickyCqDeferred(Entity)` consistent across T4/T5/T6/T7/T10 — em/rem variant dropped in v2.
   - `MultiColumn` field set in T3 matches the Default impl assertions in T3 Step 1 and the test in T10.
   - All `ChildOf` accesses use `.parent()` (v2 fix — Bevy 0.18 accessor, not `.0`).
   - All `Entity::from_raw` in test code use `Entity::from_raw_u32(n).unwrap()` (v2 fix — Bevy 0.18 API).

4. **Decision-block coverage:** D1-D15 all referenced from at least one task; D7 explicitly flagged as deferred-wire-up.

5. **v2 BLOCKER resolution:** all BLOCKERs from the 3-agent review are addressed:
   - Spec-coverage BLOCKER 1 (`Changed<MultiColumn>` missing) → Task 3 Step 6.
   - Feasibility BLOCKER 1 (`Length::Vh/Vw/Vmin/Vmax/Em/Rem` don't exist) → D3 rewritten + `resolve_sticky_inset` simplified.
   - Feasibility BLOCKER 2 (`ChildOf::.0` → `.parent()`) → Task 5 helpers updated.
   - Feasibility BLOCKER 3 (`Entity::from_raw` → `Entity::from_raw_u32(n).unwrap()`) → Task 2 + Task 4 test snippets.
   - Feasibility BLOCKER 4 (`Style` derives Bundle, no conditional insert) → D8 corrected, Task 3 Step 5 removed, Task 3 Step 8 test inverted.
   - Test BLOCKER B1 (bottom-pin branches untested) → 3 unit tests + 3 integration tests added.
   - Test BLOCKER B2 (both-active untested) → unit test + integration test added.
   - Test BLOCKER B3 (pipeline-order ordering vs execution) → Task 11 includes the sticky-anchor cross-test in the fixture.
   - Test BLOCKER B4 (clear-ordering regression) → `clear_ordering_regression_two_frames` integration test added.
   - Test BLOCKER B5 (no-scroll-container explicit assertion) → fixture spec strengthened.

---

## Open questions (defer to implementation, not plan revision)

- **Where exactly does `Position` import live in `sticky_offset`?** Likely `crate::layout::components::{Position, PositionKind}` and `crate::layout::types::Sizing`. Implementer confirms via existing imports at top of `systems.rs`.
- **`ScrollOffset` query missing entity → default zero?** Yes, per Phase 2 semantics: `ScrollOffset` is opt-in, absence means "not scrolled." Code path: `scroll_offset_q.get(scroll_container).copied().unwrap_or_default()`.

**v2 — pre-answered open questions:**
- **`Reflect` and `Copy` bounds on `ColumnRule.color: bevy::color::Color`?** `bevy::color::Color` derives `Reflect, Clone, Copy, PartialEq, Default` in Bevy 0.18 — verified via existing `ScrollbarColor` enum at `crates/buiy_core/src/layout/types.rs:326-334` which derives `Copy` alongside a `Color` field. `ColumnRule` can safely derive `Copy`. `Color::default()` returns `Color::WHITE`.
- **`Or<>` 15-cap status?** Outer Or<> at `sync_styles` is at 15/15 (cap); inner nested Or<> is at 5/15 with room for `Changed<MultiColumn>` (filling to 6/15). Phase 8+ filter widening will need to either grow the nested Or<> or add a second nested layer (`Or<(A, Or<(B, Or<(C, D, E)>)>)>`).
