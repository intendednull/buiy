# Phase 13: Full multi-column layout algorithm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Replace the Phase-7 `multicol_pack` stub (sub-pass 6c — currently warn-once + single-column fallback) with a real multi-column packing pass. For every `MultiColumn` container, resolve the used column count (from `column_count`, or computed from `column_width` + the container's content width + `column_gap`, or the CSS used-value algorithm when both are set), distribute the container's in-flow children into columns as **whole boxes** (respecting `break-before` / `break-after` / `break-inside: avoid-column` as column-break hints at the child boundary), lay each column out top-to-bottom at its column-x offset, and write each child's corrected parent-relative position into `PostTaffyPositionOverrides`. True fragmentation (splitting a single child's content across a column boundary) stays deferred (tier-E in the blink/LayoutNG sense — see prior-art) behind a residual warn-once.

**Architecture (3 sentences):**
1. **Multi-column packing is a post-Taffy positional overlay, exactly like the table sub-pass (6b).** Taffy lays the container + its children out in block flow first; sub-pass 6c then reads the container's Taffy content box + each child's Taffy size, runs a pure column-count resolver and a pure whole-child packer, and writes corrected **parent-relative** child positions into the shared `PostTaffyPositionOverrides.by_entity` map — the same map and the same coordinate space (`layout.location`) that `write_resolved_layout` (step 7) consumes for sticky / table / anchor. Sizes are never touched (only position), mirroring 6a/6b (plan D1).
2. **Whole-child packing, not content fragmentation.** Buiy produces one rect per entity (no multi-fragment output tree); a child that is taller than the column is placed whole and overflows its column rather than being split (blink prior-art: fragmentation is the single most expensive, deliberately-last LayoutNG feature). `break-before/after: column`/`Always` force the next child to start a new column; `break-inside: AvoidColumn`/`Avoid` keeps a child whole (already guaranteed by whole-child packing, so it is a no-op refinement in v1) (plan D4). A residual `MulticolFragmentationDeferred` warn-once-per-session fires when a child would need true fragmentation (it is taller than the resolved column block-size and the container requested `column_fill: Balance`), recording the limitation (plan D5).
3. **The CSS used-value column-count algorithm lives in a pure, unit-tested helper.** `resolve_column_count(column_count, column_width, gap, available_width)` implements the CSS Multicol L1 § 7.3 pseudo-algorithm (count-only, width-only, both, neither) and returns the used `(count, column_width)` pair; the system supplies `available_width` from the container's Taffy content box and the children from `Children` document order (plan D3/D6).

**Tech Stack:** Bevy 0.18 (`bevy::prelude::{Children, ChildOf, Node, Query, NonSend, ResMut, With, Vec2}`, `bevy::ecs::entity::Entity`). `std::collections::HashMap` (no `bevy::utils::*`, per Phase 6/7/8 precedent). Taffy 0.10 read-only via the shared `NonSend<LayoutTree>` (`tree.tree.layout(node)` → `taffy::Layout { location, size }`); **no synthetic Taffy tree** (unlike 6b — column packing is pure arithmetic over already-resolved child sizes). No new external dependency.

**Date:** 2026-05-29
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/flex-and-grid.md`](../specs/2026-05-08-buiy-layout-design/flex-and-grid.md) § 3 (multi-column) — § 3.1 (`MultiColumn` component + value types, already shipped Phase 7), § 3.2 (the two-stage algorithm: 1 determine column count, 2 lay out children into columns), § 5 ("Multi-column stub warns" test bullet — this phase **graduates** that bullet from stub to real packing). Reads [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (sub-pass 6c in the `PostTaffyOverrides` chain), § 6 (warn-once dedup model). Prior-art: [`prior-art/blink/layout.md`](../prior-art/blink/layout.md) § 3–§ 5 (fragmentation = multi-fragment output tree, the deliberately-deferred tier-E piece), [`prior-art/servo-stylo/layout.md`](../prior-art/servo-stylo/layout.md), [`prior-art/taffy/architecture.md`](../prior-art/taffy/architecture.md) (Taffy has no fragmentation / inline formatting context, so multicol is an above-Taffy pass).

---

## Prior-art citations (used throughout this plan)

- **Sibling sub-pass = the table algorithm (6b)** — `crates/buiy_core/src/layout/systems.rs:902-1004` (`table_layout`). 6c is structurally a clone of 6b's shape: read the container's Taffy origin/box, gather a structural model from `Children` document order, run **pure** placement helpers, write corrected positions into `PostTaffyPositionOverrides`, and emit warn-once-per-(entity,session) diagnostics for deferred sub-features. 6c is *simpler* than 6b — no synthetic Taffy tree is needed (column placement is arithmetic over child sizes), and there is no spanning/ragged concern. The pure-helper split (`gather_table` / `resolve_column_widths` / `place_table_cells`) is mirrored by `resolve_column_count` / `pack_columns` (plan D3).
- **The stub being replaced** — `crates/buiy_core/src/layout/systems.rs:1006-1026` (`multicol_pack`): today queries `Query<&MultiColumn>`, and if any exist, `if warned.set.insert(LayoutWarnOnceKey::MulticolUnsupported) { warn!(…) }`. Phase 13 replaces the body; the `MulticolUnsupported` key is **retired** (kept as a `Reflect`-stable variant emitting nothing, exactly as `TableUnsupported` was retired in Phase 12 — `types.rs:980-987`).
- **`PostTaffyPositionOverrides` shape + coordinate space** — `crates/buiy_core/src/layout/systems.rs:176-179` (`pub by_entity: HashMap<Entity, Vec2>`). Consumed by `write_resolved_layout` (`systems.rs:2385-2389`): `position = overrides.by_entity.get(&entity).copied().unwrap_or_else(|| Vec2::new(layout.location.x, layout.location.y))`. **The override value is parent-relative** (the same space as Taffy `layout.location`). Multicol children are direct children of the multicol container, so a child override = its position relative to the container's content-box origin — which is exactly what `layout.location` already is for a direct child. So the packer computes child offsets **within the container's content box** and writes them directly (no `+ container_origin`, unlike 6b's table which re-parents cells relative to the *table* origin but the cells are also direct/transitive descendants — 6b adds `table_origin` because its `place_table_cells` offsets are table-content-relative and the cells' Taffy parent is the table; for 6c the children's Taffy parent IS the multicol container, so offsets are already in the right frame — see D7).
- **Taffy read idiom + skip-on-miss** — `crates/buiy_core/src/layout/systems.rs:930-936` (table) and `:559-566` (sticky): `let Some(node) = tree.by_entity.get(&e) else { continue }; let Ok(layout) = tree.tree.layout(*node) else { continue };` then `layout.location` / `layout.size`. A Taffy miss is "skip this frame" with no warn (Taffy logs its own errors). `LayoutTree.by_entity` is `pub` (`tree.rs`); `LayoutTree` is `NonSend`.
- **Per-session warn-once dedup** — `crates/buiy_core/src/layout/systems.rs:230-244` (`LayoutWarnedOnceSession { set: HashSet<LayoutWarnOnceKey> }`); idiom `if warned.set.insert(LayoutWarnOnceKey::X) { warn!(…) }` (`table_layout`, `systems.rs:940-949`). `LayoutWarnOnceKey` enum at `crates/buiy_core/src/layout/types.rs:978-1011`; it is already `register_type`'d (`mod.rs:169`), so new variants pick up `Reflect` for free. Phase 13 adds `MulticolFragmentationDeferred` (no `Entity` — session-wide, like the retired `MulticolUnsupported`; D5).
- **`MultiColumn` component + value types (already shipped, Phase 7)** — `crates/buiy_core/src/layout/components.rs:341-353` (`MultiColumn { column_count, column_width: Option<Length>, column_gap: Option<Length>, column_rule, column_span, column_fill, break_inside, break_before, break_after }`); value types `ColumnCount { Auto, Count(u32) }` (`types.rs:851-856`), `ColumnFill { Balance, Auto }` (`types.rs:899-904`), `BreakInside`/`BreakBefore`/`BreakAfter` (`types.rs:909-941`). Phase 13 reads these; it adds **no** new component or value type.
- **`Length` → px** — `crates/buiy_core/src/layout/systems.rs:3027-3032` (`fn length_px(l: &Length) -> f32`: `Length::Px(p) => *p, _ => 0.0`). Phase 13 reuses the `Px`-only semantics for `column_width` / `column_gap` (percent / cq column metrics are an explicit non-goal in v1 — D8); a dedicated multicol resolver `multicol_length_px(Option<Length>, fallback) -> f32` wraps it with a default (D8).
- **`Display::None` skip** — `crates/buiy_core/src/layout/systems.rs:547` (sticky) and `:858` (table cell filter): entities / children with `Display::None` are filtered in Rust (`matches!(display, Display::None)`). 6c skips `Display::None` children when gathering the in-flow child list.
- **Pipeline wiring (no change needed)** — `crates/buiy_core/src/layout/mod.rs:225-235`: 6c (`systems::multicol_pack`) is already the 4th element of the chained `PostTaffyOverrides` tuple (`clear → sticky 6a → table 6b → multicol 6c → anchor 6d → transform 6e → stacking 6f`). Phase 13 only changes the **body** of `multicol_pack`; its signature gains `tree` + `children_q` + the per-child queries (T5). The `.chain()` order is unchanged.
- **Test harness** — `crates/buiy_core/tests/layout_sticky.rs:20-24`: `fn app() { let mut app = App::new(); app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin); app }` (no `CorePlugin`, no render — `MinimalPlugins + LayoutPlugin` runs the whole layout pipeline including Taffy + the `PostTaffyOverrides` chain). Spawn `(Node, Style)`, parent via `app.world_mut().entity_mut(parent).add_children(&[child])`, `app.update()` (one frame), assert via `app.world().resource::<PostTaffyPositionOverrides>()` or `app.world().get::<ResolvedLayout>(e)`. `ResolvedLayout` / `Node` re-exported from `buiy_core` crate root (`use buiy_core::{Node, ResolvedLayout}`). Existing files: `tests/layout_sticky.rs` (covers sticky/table/multicol-region pipeline tests), `tests/layout_pipeline_order.rs`.

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/types.rs` | T2 (`LayoutWarnOnceKey::MulticolFragmentationDeferred`; retire `MulticolUnsupported` doc) |
| `crates/buiy_core/src/layout/systems.rs` | T3 (`multicol_length_px` helper), T4 (`resolve_column_count` pure helper), T5 (`pack_columns` pure helper + `MulticolChild` struct), T6 (`multicol_pack` system rewrite), T7 (residual fragmentation warn in `multicol_pack`) |
| `crates/buiy_core/tests/layout_multicol.rs` | T8 (new file — spec § 3.2 integration tests), T9 (break-* + residual-warn integration tests) |

No changes to: `crates/buiy_core/src/layout/mod.rs` (6c is already wired into the chain + `MultiColumn` already `register_type`'d; `multicol_pack` is already `pub(super)` and referenced — only its body/signature change, which needs no `mod.rs` edit since the chained tuple references `systems::multicol_pack` by path), `crates/buiy_core/src/layout/components.rs` (`MultiColumn` already shipped), `crates/buiy_core/src/layout/style.rs` (`.multi_column(m)` setter + `multi_column` field already shipped, `style.rs:58`, `:467`), `crates/buiy_core/src/layout/translate.rs`, `crates/buiy_core/src/layout/tree.rs`, `crates/buiy/src/lib.rs` / `crates/buiy_core/src/lib.rs` (no new public types).

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. Multi-column is a position-only overlay in sub-pass 6c (mirror 6b)

**Decision:** 6c reads Taffy's already-computed container box + child sizes and writes corrected child **positions** into `PostTaffyPositionOverrides`; it never touches sizes and never feeds Taffy. Children keep their Taffy-computed width/height; the packer only relocates them into columns.

**Why:** This is exactly the table sub-pass (6b) contract (`systems.rs:907` "Sizes are never touched — they stay from Taffy's block layout"), and the architecture spec § 3 (line 180) declares each step-6 sub-pass "mutates `ResolvedLayout` (via the shared override map)". `MultiColumn` is deliberately wired as a Taffy no-op (`architecture.md` line 55: "`Changed<MultiColumn>` is wired now but is currently a no-op — multicol does not feed Taffy"). Keeping it position-only honors that wiring without a Taffy round-trip.

**How to apply:** `multicol_pack` (T6) inserts into `overrides.by_entity` only; `write_resolved_layout` already composes it (T6 needs no step-7 change).

**Runner-up rejected:** Feed a synthetic Taffy tree per column (like 6b's `resolve_column_widths`). Rejected: 6b needs Taffy to *size* columns from intrinsic cell widths; 6c's columns are equal-width (CSS multicol columns are equal width by definition — § 3.2), so column geometry is pure arithmetic and a synthetic tree buys nothing.

### D2. Whole-child packing; true fragmentation deferred (tier-E) with a residual warn

**Decision:** Children are packed into columns as **indivisible boxes** in document order. A child is never split across a column boundary. When the current column's used block-size would be exceeded by adding the next child, that child starts the next column (greedy fill). A child taller than the column block-size is placed whole and overflows. True content fragmentation (splitting one box's content across columns) is **not** implemented; a residual `MulticolFragmentationDeferred` warn-once-per-session fires when fragmentation would have been required (D5).

**Why:** Spec § 3 marks multi-column **tier-E** and § 3.2 describes "walk children and pack them into columns" — child-granular, not content-granular. The blink prior-art (`prior-art/blink/layout.md` § 3 line 27, § 5 line 64) is explicit: a single box producing *several* fragments is *the* feature that forces a multi-fragment output tree, was deliberately LayoutNG's **last** feature (Chrome 102–103, ~a decade in), and "is a feature Buiy deliberately does *not* have yet … Buiy gets one rect per entity." Taffy has no fragmentation (`prior-art/taffy/architecture.md`). Whole-child packing is the correct, shippable v1 that matches Buiy's one-rect-per-entity model.

**How to apply:** `pack_columns` (T5) assigns each child a `(column_index, y_within_column)` and never produces two entries for one child. The fragmentation residual is detected + warned in T7.

**Runner-up rejected:** Split children's content across columns. Rejected: requires a multi-fragment output (multiple rects per entity) Buiy's `ResolvedLayout` (one position + one size per entity) cannot represent; it is the explicit tier-E deferral the prior-art warns is the decade-long cost.

### D3. CSS used-value column-count resolution is a pure helper

**Decision:** `resolve_column_count(column_count: ColumnCount, column_width_px: Option<f32>, gap_px: f32, available_width: f32) -> (usize, f32)` implements the CSS Multicol L1 § 7.3 used-value algorithm and returns `(used_column_count, used_column_width)`. Cases:
- **Neither** `column_count` nor `column_width` set (`Auto` + `None`): used count = 1, used width = `available_width` (a single column spanning the box).
- **Count only** (`Count(n)`, no width): used count = `max(1, n)`; used width = `(available_width - (count-1)*gap) / count`.
- **Width only** (`Auto` + `Some(w)`): used count = `max(1, floor((available_width + gap) / (w + gap)))`; used width = `(available_width - (count-1)*gap) / count` (columns then expand to fill, per CSS — the *used* width is the filled width, not the requested `w`).
- **Both** (`Count(n)` + `Some(w)`): `column-count` is a *maximum*. used count = `max(1, min(n, floor((available_width + gap) / (w + gap))))`; used width = `(available_width - (count-1)*gap) / count`.

**Why:** This is the canonical CSS used-value algorithm (W3C CSS Multicol L1 § 7.3 "Pseudo-algorithm"). Factoring it pure makes the four branches + the integer-floor arithmetic unit-testable without an `App` or Taffy — the exact precedent of 6b's pure `resolve_column_widths` (`systems.rs:689`) and Phase 8's pure `compose_transform`. The "both → min, count is a max" and "width-only → columns expand to fill" quirks are the kind of rule that needs focused unit tests.

**How to apply:** T4 defines + unit-tests the helper; T6 calls it with `available_width` from the container's Taffy content box. `floor` via `f32::floor` then `as usize` after a `max(0.0)` guard.

**Runner-up rejected:** Inline the four cases in the system. Rejected: the arithmetic quirks (the `+ gap` in the floor numerator, the max-vs-target interplay) demand isolated tests an `App`-level test obscures; matches the 6b/Phase-8 pure-helper precedent.

### D4. `break-*` honored as column-break hints at the child boundary; `break-inside` is a v1 no-op

**Decision:** During packing (T5):
- `break_before == Column | Always` on child *i* → child *i* starts a new column (unless it is already the first child of a column).
- `break_after == Column | Always` on child *i* → child *i+1* starts a new column.
- `break_inside == Avoid | AvoidColumn` → **no-op in v1**: whole-child packing already never splits a child, so "avoid break inside" is trivially satisfied. Documented as a no-op (not a warn — it is honored, just freely).
- `break_before/after == Avoid | AvoidColumn` → **no-op in v1**: forcing a child to *not* start a new column when greedy fill would put it there requires backtracking / fragmentation balancing Buiy does not do; documented as a best-effort deferral folded into the D5 residual (the layout is still valid, just not break-avoidance-optimized).

**Why:** Spec § 3.1 lists the `break-*` fields and § 3.2 says packing "respect[s] `break-*` properties". The forcing breaks (`Column`/`Always`) are cheap and meaningful with whole-child packing; the *avoidance* breaks are a fragmentation-balancing concern (CSS Fragmentation L3 § 4) that the tier-E deferral (D2) covers. Honoring forcing breaks + documenting avoidance as best-effort is the honest v1.

**How to apply:** `pack_columns` (T5) reads a `force_break_before: bool` / `force_break_after: bool` per child (derived from the enums by the caller in T6) and starts a new column accordingly. `break_inside`/avoidance breaks are not consulted in v1 (documented in the helper doc-comment + T9 asserts a forcing break works).

**Runner-up rejected:** Implement break-avoidance via a balancing pass. Rejected: balancing across columns to honor `break-inside: avoid` is exactly the fragmentation machinery deferred in D2; a partial implementation would be a band-aid (project guideline: no hacky workarounds).

### D5. Residual `MulticolFragmentationDeferred` warn-once (session-wide); retire `MulticolUnsupported`

**Decision:** Add one `LayoutWarnOnceKey::MulticolFragmentationDeferred` variant (no `Entity` payload — session-wide, like the retired `MulticolUnsupported`). It fires once per session when 6c detects a child that **would require true fragmentation** to lay out correctly — concretely: a child whose Taffy block-size exceeds the resolved column block-size **while** `column_fill == Balance` (balanced fill assumes content can be divided to equalize column heights, which whole-child packing cannot do). The stub's `MulticolUnsupported` key is **retired**: kept as a `Reflect`-stable variant with a doc note that no code emits it (exact precedent: `TableUnsupported`, `types.rs:980-987`).

**Why:** The packing algorithm now ships, so the blanket "multicol unsupported" warn is wrong (it would fire for the common, fully-supported case). But fragmentation genuinely remains deferred (D2), and an author relying on `column-fill: balance` with oversized content gets a degraded (greedy, unbalanced) layout — worth one diagnostic. Session-wide (not per-entity) because the limitation is a global capability gap, not a property of one entity, matching the original `MulticolUnsupported` scope. Retiring (not deleting) `MulticolUnsupported` preserves `Reflect`/serialization stability per the Phase-12 `TableUnsupported` precedent.

**How to apply:** T2 retires `MulticolUnsupported` (doc only) + adds the new variant; T7 emits it from `multicol_pack` when the residual condition holds.

**Runner-up rejected:** Per-entity `MulticolFragmentationDeferred(Entity)`. Rejected: the limitation is a capability gap (the same for every container), so per-entity keys would emit N warns for one global state — the same reasoning that kept `MulticolUnsupported` payload-free.

### D6. In-flow child set = `Children` document order, minus `Display::None` and absolute/fixed

**Decision:** The children packed into columns are the multicol container's direct `Children`, in iteration (document) order, **excluding** those with `Display::None` (skipped entirely) and those with `Position::Absolute`/`Fixed` (they escape flow — they keep their Taffy position, no override written). Sticky children are packed in-flow (their sticky displacement is 6a's concern and composes via the shared map — but 6c runs before 6a in chain order, so 6c writes the in-flow column position and 6a may further displace it; documented, the common case has no sticky-in-multicol overlap).

**Why:** CSS multicol flows in-flow content into columns; out-of-flow boxes (absolute/fixed) are not column items (CSS Multicol L1 § 9). `Display::None` boxes generate no layout. Matches 6b's `Display::None` cell filter (`systems.rs:858`) and the absolute-escape contract (`flex-and-grid.md` § 4 line 198: absolute children "escape both algorithms"). Chain order (6c before 6a) means a sticky multicol child gets its column position from 6c then sticky displacement from 6a — `PostTaffyPositionOverrides` is last-writer-wins per entity, and sticky's `e_natural_rel` read is from Taffy not the map, so this is a known minor seam (documented, not a v1 blocker).

**How to apply:** T6 builds the child list with `display_q.get(c)` and `position_q.get(c)` filters before calling `pack_columns`.

**Runner-up rejected:** Pack every direct child unconditionally. Rejected: would relocate absolute/fixed boxes (which must stay where Taffy's absolute algorithm placed them) and would place zero-box `Display::None` children, both visibly wrong.

### D7. Override coordinate space = container-content-relative (no origin add)

**Decision:** `pack_columns` returns each child's offset **relative to the multicol container's content-box origin** (column-x for the x, cumulative-y-within-column for the y). 6c writes these offsets **directly** into `overrides.by_entity` — it does **not** add the container's Taffy origin (contrast 6b, which adds `table_origin`).

**Why:** `write_resolved_layout` writes `ResolvedLayout.position = override_value` and treats it as the entity's parent-relative position (the same space as Taffy `layout.location`, which for a direct child is relative to the parent's content box — `systems.rs:2385-2389`). A multicol child's Taffy parent **is** the multicol container, so its position must be expressed relative to the container's content box — which is exactly what `pack_columns` produces. 6b adds `table_origin` because `place_table_cells` returns offsets relative to the *table content*, but a table cell's Taffy parent is the table (cells are direct/transitive descendants laid out in the table's frame) — 6b's add reconciles the table's own block position; for 6c there is no analogous reconciliation because column packing already works in the container-content frame. **Implementer must verify** against an integration test (T8) that asserts the absolute `ResolvedLayout.position` of a packed child equals the container's resolved position plus the in-column offset.

**How to apply:** T6: `overrides.by_entity.insert(child, offset)` with `offset` straight from `pack_columns` (no `+ container_origin`). T8's assertions pin the coordinate space.

**Runner-up rejected:** Add the container origin (copy 6b verbatim). Rejected: it would double-count the container's position (Taffy's `write_resolved_layout` does not subtract a parent origin — `ResolvedLayout.position` is already parent-relative), placing children at `2 × container_origin + offset`. The T8 integration test is the guardrail that catches this if the implementer gets it wrong.

### D8. `column_width` / `column_gap` resolve `Px` only; default gap; percent/cq deferred

**Decision:** A helper `multicol_length_px(l: Option<Length>, fallback: f32) -> f32` resolves `column_width` and `column_gap`: `Some(Length::Px(v)) => v`, `Some(_non-px_) => fallback` (silent — percent/cq column metrics are a non-goal in v1), `None => fallback`. `column_gap`'s fallback is the CSS initial `normal`, which Buiy maps to **0.0 px** in v1 (the spec ships no font metrics, so the CSS `1em` for `normal` is unavailable). `column_width`'s "fallback" is only reached when `column_width == None`, in which case the *width-only / both* branches of `resolve_column_count` are not taken — so `multicol_length_px` for width is only called when `Some`, and a non-`Px` width resolves to `0.0` which `resolve_column_count` treats as "no usable width" (count-only / single-column path).

**Why:** `length_px` (`systems.rs:3027`) already establishes the "`Px` only, else 0.0" v1 contract for transform translate, with percent/cq deferred. Multicol column metrics are not on any hot authoring path in v1 and the spec § 3 marks the whole feature tier-E; matching the established `Px`-only resolution avoids inventing percent-resolution plumbing the spec does not require. `normal` gap → 0.0 is the only font-free choice (em is unavailable pre-text-rendering).

**How to apply:** T3 defines + unit-tests `multicol_length_px`. T6 calls it for gap (`fallback = 0.0`) and, when `column_width.is_some()`, for width.

**Runner-up rejected:** Resolve percent column-width against the container content width. Rejected: speculative scope — no spec § 3 requirement, no test surface bullet (§ 5) for it, and it diverges from the `length_px` `Px`-only precedent without cause. Add it as a follow-up if demand appears.

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

### Task 1: Retire `MulticolUnsupported` + add `MulticolFragmentationDeferred` warn key

**Spec:** § 3.2, architecture § 6, D5.

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (retire-doc `MulticolUnsupported`; add `MulticolFragmentationDeferred`)

- [ ] **Step 1: Failing test.** Add to `types.rs::mod tests` (the same module as the existing `LayoutWarnOnceKey`-adjacent tests):
  ```rust
  #[test]
  fn multicol_fragmentation_warn_key_is_hashable() {
      let mut set = std::collections::HashSet::new();
      assert!(set.insert(LayoutWarnOnceKey::MulticolFragmentationDeferred));
      assert!(!set.insert(LayoutWarnOnceKey::MulticolFragmentationDeferred));
  }
  ```
  Run: `cargo test -p buiy_core multicol_fragmentation_warn_key_is_hashable` — expected FAIL (variant doesn't exist).

- [ ] **Step 2: Edit the `MulticolUnsupported` doc + add the new variant.** In `crates/buiy_core/src/layout/types.rs`, the `MulticolUnsupported` variant currently reads (lines 1005-1011):
  ```rust
      /// `MultiColumn` entity encountered. Sub-pass 6c emits one warn
      /// per session (no Entity payload — first multicol entity triggers,
      /// all subsequent are silent) — the multicol algorithm is
      /// deferred to v1.x.
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
      MulticolUnsupported,
  ```
  Replace that doc comment (keep the variant for `Reflect` stability — D5) with the retired note, and append the new variant immediately after it:
  ```rust
      /// **Retired in Phase 13** — the blanket "multicol unsupported" warn
      /// from the Phase-7 stub. Sub-pass 6c now packs children into
      /// columns; the residual fragmentation gap is reported by
      /// `MulticolFragmentationDeferred`. Kept as a variant for
      /// `Reflect`/serialization stability; no code emits it. (Same
      /// retire pattern as `TableUnsupported`.)
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
      MulticolUnsupported,

      /// Sub-pass 6c packs `MultiColumn` children into columns as whole
      /// boxes (no content fragmentation — a box is never split across a
      /// column boundary). When a child taller than the resolved column
      /// block-size is encountered under `column_fill: Balance` (balanced
      /// fill needs divisible content), the layout falls back to greedy
      /// whole-child packing and this warns once per session. True
      /// fragmentation is tier-E, deferred to v1.x (plan D2/D5).
      ///
      /// No `Entity` payload: the limitation is a global capability gap,
      /// not a per-entity error (session-wide, like the retired
      /// `MulticolUnsupported`).
      ///
      /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
      MulticolFragmentationDeferred,
  ```

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core multicol_fragmentation_warn_key_is_hashable
  ```
  Expected PASS.

- [ ] **Step 4: Project gate.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Expected: all green. (`MulticolUnsupported` is still emitted by the Phase-7 `multicol_pack` stub at this point — the workspace tests including `multicol_pack_warns_once_per_session` still pass; that test is removed in T6 when the stub body is replaced.)

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/types.rs
  git commit -m "feat(layout): retire MulticolUnsupported, add MulticolFragmentationDeferred key (Phase 13 — spec § 3.2, D5)

Multicol packing ships in Phase 13; the blanket stub warn is retired (kept
Reflect-stable like TableUnsupported). New session-wide key reports the residual
true-fragmentation gap (tier-E, deferred). Emitter lands in T7.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 2: `multicol_length_px` length resolver helper

**Spec:** § 3.1, D8.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `multicol_length_px` pure helper + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn multicol_length_px_px_passes_through() {
      assert_eq!(multicol_length_px(Some(Length::Px(120.0)), 0.0), 120.0);
  }

  #[test]
  fn multicol_length_px_none_uses_fallback() {
      assert_eq!(multicol_length_px(None, 16.0), 16.0);
  }

  #[test]
  fn multicol_length_px_non_px_uses_fallback() {
      // percent / cq column metrics are a v1 non-goal (D8) — fall back.
      assert_eq!(multicol_length_px(Some(Length::Percent(50.0)), 0.0), 0.0);
      assert_eq!(multicol_length_px(Some(Length::Cqw(10.0)), 7.0), 7.0);
  }
  ```
  Run: `cargo test -p buiy_core multicol_length_px` — expected FAIL.

- [ ] **Step 2: Add the helper to `systems.rs`.** Place it next to `length_px` (`systems.rs:3027`):
  ```rust
  /// Resolve a `MultiColumn` length metric (`column_width` / `column_gap`)
  /// to px for the v1 packer. Only `Length::Px` is meaningful in v1
  /// (percent / cq column metrics are a non-goal — plan D8); any other
  /// variant, or `None`, yields `fallback`. The gap's fallback is `0.0`
  /// (CSS `normal` maps to 0 pre-font-metrics); a width is only resolved
  /// when `Some`, and a non-`Px` width resolving to its fallback (0.0)
  /// makes `resolve_column_count` treat it as "no usable width".
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
  pub(super) fn multicol_length_px(l: Option<Length>, fallback: f32) -> f32 {
      match l {
          Some(Length::Px(v)) => v,
          _ => fallback,
      }
  }
  ```
  **Implementer note:** `Length` is already in scope in `systems.rs` (used by `length_px`, `length_inset_to_px`, etc.). No new import.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core multicol_length_px
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): multicol_length_px resolver (Phase 13 — spec § 3.1, D8)

Px-only column metric resolution (percent/cq deferred), matching the length_px
precedent. Consumed by resolve_column_count (T3) + multicol_pack (T6).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 3: `resolve_column_count` pure helper (CSS used-value algorithm)

**Spec:** § 3.2 step 1, D3.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `resolve_column_count` pure helper + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn resolve_column_count_neither_is_single_column() {
      // No count, no width → 1 column spanning the box.
      let (n, w) = resolve_column_count(ColumnCount::Auto, None, 0.0, 400.0);
      assert_eq!(n, 1);
      assert_eq!(w, 400.0);
  }

  #[test]
  fn resolve_column_count_count_only_divides_with_gaps() {
      // count = 3, gap = 20, width 440 → 3 cols, (440 - 2*20)/3 = 133.33.
      let (n, w) = resolve_column_count(ColumnCount::Count(3), None, 20.0, 440.0);
      assert_eq!(n, 3);
      assert!((w - 400.0 / 3.0).abs() < 1e-3, "used width = {w}");
  }

  #[test]
  fn resolve_column_count_count_zero_clamps_to_one() {
      let (n, _w) = resolve_column_count(ColumnCount::Count(0), None, 0.0, 400.0);
      assert_eq!(n, 1, "count 0 clamps to 1 column");
  }

  #[test]
  fn resolve_column_count_width_only_floors_then_fills() {
      // width 100, gap 0, available 350 → floor((350+0)/(100+0)) = 3 cols;
      // used width = (350 - 0)/3 = 116.67 (columns expand to fill).
      let (n, w) = resolve_column_count(ColumnCount::Auto, Some(100.0), 0.0, 350.0);
      assert_eq!(n, 3);
      assert!((w - 350.0 / 3.0).abs() < 1e-3, "used width = {w}");
  }

  #[test]
  fn resolve_column_count_width_only_with_gap() {
      // width 100, gap 25, available 350 → floor((350+25)/(100+25)) =
      // floor(375/125) = 3 cols; used width = (350 - 2*25)/3 = 100.
      let (n, w) = resolve_column_count(ColumnCount::Auto, Some(100.0), 25.0, 350.0);
      assert_eq!(n, 3);
      assert!((w - 100.0).abs() < 1e-3, "used width = {w}");
  }

  #[test]
  fn resolve_column_count_both_count_is_max() {
      // count = 2 (a maximum), width 100, gap 0, available 350.
      // width-derived = floor(350/100) = 3, capped at count 2 → 2 cols.
      let (n, _w) = resolve_column_count(ColumnCount::Count(2), Some(100.0), 0.0, 350.0);
      assert_eq!(n, 2, "column-count caps the width-derived count");
  }

  #[test]
  fn resolve_column_count_both_width_wins_when_smaller() {
      // count = 5, width 100, gap 0, available 350 →
      // width-derived = 3, min(5, 3) = 3 cols.
      let (n, _w) = resolve_column_count(ColumnCount::Count(5), Some(100.0), 0.0, 350.0);
      assert_eq!(n, 3);
  }

  #[test]
  fn resolve_column_count_width_wider_than_box_is_one_column() {
      // width 500 > available 400 → floor((400+0)/(500+0)) = 0 → clamp 1.
      let (n, w) = resolve_column_count(ColumnCount::Auto, Some(500.0), 0.0, 400.0);
      assert_eq!(n, 1);
      assert!((w - 400.0).abs() < 1e-3);
  }
  ```
  Run: `cargo test -p buiy_core resolve_column_count` — expected FAIL.

- [ ] **Step 2: Add the helper to `systems.rs`.** Place it near `resolve_column_widths` (`systems.rs:689`) or just above `multicol_pack`:
  ```rust
  /// Resolve the CSS Multicol L1 § 7.3 *used* `(column_count, column_width)`
  /// pair from the declared `column-count` / `column-width` / `column-gap`
  /// and the container's available (content-box inline) width.
  ///
  /// Four cases (plan D3):
  /// - **neither** (`Auto` + `None`): 1 column, used width = `available_width`.
  /// - **count only**: used count = `max(1, n)`; used width =
  ///   `(available - (count-1)*gap) / count`.
  /// - **width only**: used count =
  ///   `max(1, floor((available + gap) / (width + gap)))`; used width =
  ///   `(available - (count-1)*gap) / count` (columns expand to fill).
  /// - **both**: `column-count` is a *maximum* — used count =
  ///   `max(1, min(n, width_derived_count))`; used width as above.
  ///
  /// Pure (no Bevy queries / no Taffy reads). Unit-tested in `mod tests`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
  pub(super) fn resolve_column_count(
      column_count: ColumnCount,
      column_width: Option<f32>,
      gap: f32,
      available_width: f32,
  ) -> (usize, f32) {
      let avail = available_width.max(0.0);
      let gap = gap.max(0.0);

      // Count derivable from a usable (> 0) width: how many `width + gap`
      // slabs fit, with one fewer gap than columns (the `+ gap` numerator
      // term cancels the trailing gap). 0 → fall through to clamp.
      let width_derived = |w: f32| -> usize {
          if w <= 0.0 {
              return 0;
          }
          (((avail + gap) / (w + gap)).floor() as i64).max(0) as usize
      };

      let count = match (column_count, column_width) {
          (ColumnCount::Auto, None) => 1,
          (ColumnCount::Count(n), None) => (n as usize).max(1),
          (ColumnCount::Auto, Some(w)) => width_derived(w).max(1),
          (ColumnCount::Count(n), Some(w)) => {
              // column-count is a maximum; clamp the width-derived count.
              (n as usize).min(width_derived(w).max(1)).max(1)
          }
      };

      let used_width = if count <= 1 {
          avail
      } else {
          ((avail - (count as f32 - 1.0) * gap) / count as f32).max(0.0)
      };
      (count, used_width)
  }
  ```
  **Implementer note (import):** `ColumnCount` is NOT yet referenced in `systems.rs` (the Phase-7 stub never matches on `MultiColumn`'s fields). Add `ColumnCount` to the `use super::types::{…}` block (`systems.rs:26-30`) — alongside `BreakAfter`, `BreakBefore`, `ColumnFill` which T5/T6 also need (add all four now to avoid churn). The `width_derived` floor uses `avail + gap` over `w + gap`: for N columns there are N-1 gaps, so `avail = N*w + (N-1)*gap` ⇒ `N = (avail + gap)/(w + gap)`; `floor` gives the most columns that fit. Verify the T1 fixtures arithmetic matches.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core resolve_column_count
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): resolve_column_count CSS used-value helper (Phase 13 — spec § 3.2, D3)

Pure helper for the count-only / width-only / both / neither cases of the CSS
Multicol used-value algorithm; column-count is a maximum when both are set.
Consumed by multicol_pack (T6).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 4: `MulticolChild` + `pack_columns` pure helper (whole-child greedy packing)

**Spec:** § 3.2 step 2, D2, D4, D7.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `MulticolChild` struct + `PackedChild` + `pack_columns` + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  // Build a MulticolChild test fixture (entity, height, no forced breaks).
  fn mc_child(world: &mut World, height: f32) -> MulticolChild {
      let e = world.spawn_empty().id();
      MulticolChild { entity: e, height, force_break_before: false, force_break_after: false }
  }

  #[test]
  fn pack_columns_fills_columns_top_to_bottom() {
      // 2 columns, width 100, gap 20, col block-size 100.
      // children heights [40, 40, 40]: col0 gets [40,40] (y 0,40),
      // col1 gets [40] (y 0). col x: col0 = 0, col1 = 120.
      let mut world = World::new();
      let a = mc_child(&mut world, 40.0);
      let b = mc_child(&mut world, 40.0);
      let c = mc_child(&mut world, 40.0);
      let (ea, eb, ec) = (a.entity, b.entity, c.entity);
      let packed = pack_columns(&[a, b, c], 2, 100.0, 20.0, 100.0);
      let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
      assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
      assert_eq!(pos(eb), Vec2::new(0.0, 40.0));
      assert_eq!(pos(ec), Vec2::new(120.0, 0.0));
  }

  #[test]
  fn pack_columns_overflow_starts_next_column() {
      // col block-size 50; heights [40, 40] → b doesn't fit after a
      // (40+40 > 50) → b starts col1.
      let mut world = World::new();
      let a = mc_child(&mut world, 40.0);
      let b = mc_child(&mut world, 40.0);
      let (ea, eb) = (a.entity, b.entity);
      let packed = pack_columns(&[a, b], 2, 100.0, 0.0, 50.0);
      let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
      assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
      assert_eq!(pos(eb), Vec2::new(100.0, 0.0), "overflow pushes b to col1");
  }

  #[test]
  fn pack_columns_force_break_before_starts_new_column() {
      // both fit in col0 by size, but b has force_break_before → col1.
      let mut world = World::new();
      let a = mc_child(&mut world, 10.0);
      let mut b = mc_child(&mut world, 10.0);
      b.force_break_before = true;
      let (ea, eb) = (a.entity, b.entity);
      let packed = pack_columns(&[a, b], 2, 100.0, 0.0, 500.0);
      let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
      assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
      assert_eq!(pos(eb), Vec2::new(100.0, 0.0), "force-break-before starts col1");
  }

  #[test]
  fn pack_columns_force_break_after_pushes_next_child() {
      // a has force_break_after → b starts col1 even though it would fit.
      let mut world = World::new();
      let mut a = mc_child(&mut world, 10.0);
      a.force_break_after = true;
      let b = mc_child(&mut world, 10.0);
      let (ea, eb) = (a.entity, b.entity);
      let packed = pack_columns(&[a, b], 2, 100.0, 0.0, 500.0);
      let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
      assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
      assert_eq!(pos(eb), Vec2::new(100.0, 0.0), "force-break-after pushes b to col1");
  }

  #[test]
  fn pack_columns_break_at_column_zero_is_no_op() {
      // force_break_before on the very first child must not create an empty
      // column 0 — it stays in col0.
      let mut world = World::new();
      let mut a = mc_child(&mut world, 10.0);
      a.force_break_before = true;
      let ea = a.entity;
      let packed = pack_columns(&[a], 1, 100.0, 0.0, 500.0);
      assert_eq!(packed[0].offset, Vec2::new(0.0, 0.0), "break on first child is a no-op");
  }

  #[test]
  fn pack_columns_single_column_stacks_all() {
      let mut world = World::new();
      let a = mc_child(&mut world, 30.0);
      let b = mc_child(&mut world, 30.0);
      let (ea, eb) = (a.entity, b.entity);
      let packed = pack_columns(&[a, b], 1, 400.0, 0.0, 1000.0);
      let pos = |e: Entity| packed.iter().find(|p| p.entity == e).unwrap().offset;
      assert_eq!(pos(ea), Vec2::new(0.0, 0.0));
      assert_eq!(pos(eb), Vec2::new(0.0, 30.0));
  }
  ```
  **Implementer note:** these tests build `Entity` values via a throwaway `World` (`world.spawn_empty().id()`) so the pure helper can be tested without an `App`. `Vec2` is in scope in `systems.rs`. Confirm `World` is importable in the test module (`use bevy::prelude::*` or `bevy::ecs::world::World` — check the existing `mod tests` imports; `App`-based tests already import the prelude).
  Run: `cargo test -p buiy_core pack_columns` — expected FAIL.

- [ ] **Step 2: Add the struct + helper to `systems.rs`.** Place above `multicol_pack`:
  ```rust
  /// One in-flow multi-column child as seen by the packer: its entity,
  /// its Taffy-computed block-size (height in horizontal writing mode),
  /// and whether a forced column break is requested immediately before /
  /// after it (derived from `break-before` / `break-after`). Width is not
  /// stored — every column is the resolved `column_width`; the packer
  /// places children at the column-x, it does not resize them (plan D1).
  #[derive(Clone, Copy, Debug)]
  pub(super) struct MulticolChild {
      pub entity: Entity,
      pub height: f32,
      pub force_break_before: bool,
      pub force_break_after: bool,
  }

  /// A packed child: its entity and its offset relative to the multicol
  /// container's content-box origin (plan D7 — written straight into the
  /// override map, no container-origin add).
  #[derive(Clone, Copy, Debug)]
  pub(super) struct PackedChild {
      pub entity: Entity,
      pub offset: Vec2,
  }

  /// Distribute `children` (document order) into `count` equal-width
  /// columns via greedy whole-child packing (plan D2): fill a column
  /// top-to-bottom until the next child would exceed `col_block_size`,
  /// then move to the next column. A child is never split. Forced column
  /// breaks (`force_break_before` / `force_break_after`, plan D4) start a
  /// new column at the child boundary; a break before the first child of
  /// column 0 is a no-op (no empty leading column). The last column
  /// absorbs any remaining children even past `count` columns is never
  /// produced — the column index saturates at `count - 1` so an
  /// overlong content stream stacks into the final column (whole-child
  /// packing, no overflow column).
  ///
  /// Column `c`'s x-offset is `c * (col_width + gap)`. A child's y is the
  /// running cumulative height within its column.
  ///
  /// Pure (no Bevy queries / no Taffy reads). Unit-tested in `mod tests`.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
  pub(super) fn pack_columns(
      children: &[MulticolChild],
      count: usize,
      col_width: f32,
      gap: f32,
      col_block_size: f32,
  ) -> Vec<PackedChild> {
      let count = count.max(1);
      let last_col = count - 1;
      let mut out: Vec<PackedChild> = Vec::with_capacity(children.len());
      let mut col = 0usize;
      let mut y = 0.0f32;

      for (i, child) in children.iter().enumerate() {
          let is_first_in_layout = i == 0;
          // A forced break-before, or an overflow of the current column,
          // advances to the next column — but never before placing the
          // very first child (no empty leading column).
          let force_break = child.force_break_before && !is_first_in_layout;
          let overflow = y > 0.0 && (y + child.height) > col_block_size;
          if (force_break || overflow) && col < last_col {
              col += 1;
              y = 0.0;
          }

          let x = col as f32 * (col_width + gap);
          out.push(PackedChild {
              entity: child.entity,
              offset: Vec2::new(x, y),
          });
          y += child.height;

          // A forced break-after moves the *next* child to a new column.
          if child.force_break_after && col < last_col {
              col += 1;
              y = 0.0;
          }
      }
      out
  }
  ```
  **Implementer note:** the `y > 0.0` guard on `overflow` ensures a single child taller than the column does not get bumped to a fresh column on its own (it is placed at y=0 and overflows — D2 whole-child). The `col < last_col` guards realize "saturate at the last column" (no overflow column — whole-child packing means the final column simply grows). The forced-break-after advancing inside the loop means a subsequent break-before on the next child is idempotent (both want a new column; the `col < last_col` guard prevents double-advance past the last column).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core pack_columns
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): pack_columns whole-child greedy packer (Phase 13 — spec § 3.2, D2/D4/D7)

Greedy top-to-bottom column fill over indivisible children, with forced
break-before/after starting a new column and column-index saturation at the
last column (no fragmentation). Offsets are container-content-relative.
Consumed by multicol_pack (T6).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 5: Rewrite `multicol_pack` system (6c) — gather, resolve, pack, write overrides

**Spec:** § 3.2, architecture § 3, D1, D6, D7.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (replace the `multicol_pack` body + signature; remove the obsolete `multicol_pack_warns_once_per_session` stub test)

- [ ] **Step 1: Failing test.** Add to `tests/layout_multicol.rs` (create the file; model the harness on `tests/layout_sticky.rs:20-24`):
  ```rust
  //! Phase 13 — multi-column packing integration tests (sub-pass 6c).
  //! Harness: MinimalPlugins + LayoutPlugin (runs Taffy + the
  //! PostTaffyOverrides chain headless). Spec: flex-and-grid.md § 3.
  use bevy::prelude::*;
  use buiy_core::layout::{
      ColumnCount, Display, LayoutPlugin, MultiColumn, PostTaffyPositionOverrides, Style,
  };
  use buiy_core::{Node, ResolvedLayout};

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
      app
  }

  /// Spawn a multicol container of fixed content-box width/height with the
  /// given `MultiColumn`, plus `n` fixed-size block children. Returns
  /// (container, child entities in document order).
  fn multicol_container(
      app: &mut App,
      width: f32,
      height: f32,
      mc: MultiColumn,
      child_sizes: &[(f32, f32)],
  ) -> (Entity, Vec<Entity>) {
      let container = app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(width).height_px(height).multi_column(mc),
          ))
          .id();
      let mut kids = Vec::new();
      for &(w, h) in child_sizes {
          let c = app
              .world_mut()
              .spawn((Node, Style::default().width_px(w).height_px(h)))
              .id();
          app.world_mut().entity_mut(container).add_children(&[c]);
          kids.push(c);
      }
      (container, kids)
  }

  #[test]
  fn two_column_count_packs_children_into_columns() {
      // 2 columns, gap 0, container 200x100. Three 100x40 children.
      // resolve_column_count(Count(2), None, 0, 200) → (2, 100).
      // Greedy with col_block_size = 100: col0 [c0@y0, c1@y40], col1 [c2@y0].
      // col0 x = 0, col1 x = 100.
      let mut app = app();
      let mc = MultiColumn { column_count: ColumnCount::Count(2), ..Default::default() };
      let (container, kids) = multicol_container(
          &mut app, 200.0, 100.0, mc, &[(100.0, 40.0), (100.0, 40.0), (100.0, 40.0)],
      );
      app.update();

      let overrides = app.world().resource::<PostTaffyPositionOverrides>();
      // Container-content-relative offsets (plan D7).
      assert_eq!(overrides.by_entity.get(&kids[0]).copied(), Some(Vec2::new(0.0, 0.0)));
      assert_eq!(overrides.by_entity.get(&kids[1]).copied(), Some(Vec2::new(0.0, 40.0)));
      assert_eq!(overrides.by_entity.get(&kids[2]).copied(), Some(Vec2::new(100.0, 0.0)));
  }

  #[test]
  fn packed_child_resolved_layout_is_container_relative() {
      // Guard for D7: the child's ResolvedLayout.position must be the
      // in-column offset (parent-relative), NOT double-counting the
      // container origin. Container at root → container origin (0,0), so
      // the child's ResolvedLayout.position equals its in-column offset.
      let mut app = app();
      let mc = MultiColumn { column_count: ColumnCount::Count(2), ..Default::default() };
      let (_container, kids) = multicol_container(
          &mut app, 200.0, 100.0, mc, &[(100.0, 40.0), (100.0, 40.0), (100.0, 40.0)],
      );
      app.update();
      let rl = app.world().get::<ResolvedLayout>(kids[2]).unwrap();
      assert_eq!(rl.position, Vec2::new(100.0, 0.0), "child 2 sits at col1 x=100, y=0");
  }

  #[test]
  fn no_multicol_writes_no_overrides() {
      // A plain block container with plain children writes nothing to the map.
      let mut app = app();
      let container = app
          .world_mut()
          .spawn((Node, Style::default().width_px(200.0).height_px(100.0)))
          .id();
      let c = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(20.0))).id();
      app.world_mut().entity_mut(container).add_children(&[c]);
      app.update();
      let overrides = app.world().resource::<PostTaffyPositionOverrides>();
      assert!(overrides.by_entity.get(&c).is_none(), "non-multicol child untouched");
  }
  ```
  **Implementer note:** `ColumnCount`, `MultiColumn`, `PostTaffyPositionOverrides`, `Style`, `Display` are all re-exported from `buiy_core::layout` (`mod.rs:13-37`). `ResolvedLayout`/`Node` from the crate root. Confirm `.multi_column(...)`, `.width_px`, `.height_px` setters exist (they do — `style.rs`). The container content-box width = `width_px` here (no padding/border in the fixture), so `available_width` = 200.
  Run: `cargo test -p buiy_core --test layout_multicol two_column_count packed_child_resolved no_multicol_writes` — expected FAIL (stub still warns + writes nothing).

- [ ] **Step 2: Replace the `multicol_pack` body + signature.** The current stub (`systems.rs:1014-1026`) is:
  ```rust
  pub(super) fn multicol_pack(
      multicol_q: Query<&MultiColumn>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      if multicol_q.iter().next().is_none() {
          return;
      }
      if warned.set.insert(LayoutWarnOnceKey::MulticolUnsupported) {
          bevy::log::warn!(...);
      }
  }
  ```
  Replace the **whole function** (keep the `T7` fragmentation warn out for now — it lands in T7; this task ships packing without the residual warn) with:
  ```rust
  /// Sub-pass 6c — multi-column packing (spec § 3.2). For each
  /// `MultiColumn` container: resolve the used column count + width from
  /// the container's Taffy content box (step 1), pack its in-flow
  /// children into columns as whole boxes top-to-bottom (step 2,
  /// respecting forced `break-before`/`after`), and write each child's
  /// corrected container-content-relative position into
  /// `PostTaffyPositionOverrides` (plan D7). Sizes are never touched —
  /// they stay from Taffy's block layout (plan D1), matching 6a/6b.
  ///
  /// Out-of-flow children (`Position::Absolute`/`Fixed`) and
  /// `Display::None` children are excluded (plan D6). True content
  /// fragmentation is deferred (plan D2); the residual warn lands in a
  /// later task.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
  pub(super) fn multicol_pack(
      tree: NonSend<LayoutTree>,
      multicol_q: Query<(Entity, &MultiColumn), With<Node>>,
      children_q: Query<&Children>,
      display_q: Query<&Display>,
      position_q: Query<&Position>,
      mut overrides: ResMut<PostTaffyPositionOverrides>,
  ) {
      for (container, mc) in multicol_q.iter() {
          // The container's Taffy box (content width drives column count).
          let Some(container_node) = tree.by_entity.get(&container) else {
              continue;
          };
          let Ok(container_layout) = tree.tree.layout(*container_node) else {
              continue;
          };
          let content_width = container_layout.size.width;
          let content_height = container_layout.size.height;

          // Gather in-flow children in document order (plan D6).
          let Ok(kids) = children_q.get(container) else {
              continue;
          };
          let mut packed_input: Vec<MulticolChild> = Vec::new();
          for child in kids.iter() {
              // Skip Display::None.
              if matches!(display_q.get(child), Ok(Display::None)) {
                  continue;
              }
              // Skip out-of-flow (absolute / fixed escape the columns).
              if let Ok(pos) = position_q.get(child)
                  && matches!(pos.kind, PositionKind::Absolute | PositionKind::Fixed)
              {
                  continue;
              }
              // Child block-size from Taffy; skip if not yet placed.
              let Some(child_node) = tree.by_entity.get(&child) else {
                  continue;
              };
              let Ok(child_layout) = tree.tree.layout(*child_node) else {
                  continue;
              };
              let bf = matches!(mc.break_before, BreakBefore::Column | BreakBefore::Always);
              let af = matches!(mc.break_after, BreakAfter::Column | BreakAfter::Always);
              packed_input.push(MulticolChild {
                  entity: child,
                  height: child_layout.size.height,
                  // NOTE: break-before/after are container-level fields on
                  // MultiColumn in v1 (the spec models them on the
                  // multicol box). They apply uniformly — see implementer
                  // note. Per-child break support is a follow-up.
                  force_break_before: bf,
                  force_break_after: af,
              });
          }
          if packed_input.is_empty() {
              continue;
          }

          let gap = multicol_length_px(mc.column_gap, 0.0);
          let width = mc.column_width.map(|_| multicol_length_px(mc.column_width, 0.0));
          let (count, col_width) =
              resolve_column_count(mc.column_count, width, gap, content_width);

          let packed = pack_columns(&packed_input, count, col_width, gap, content_height);
          for p in packed {
              overrides.by_entity.insert(p.entity, p.offset);
          }
      }
  }
  ```
  **Implementer note (break-* placement):** `break_before` / `break_after` / `break_inside` are fields on the **container's** `MultiColumn` component in the v1 API (`components.rs:350-352`), not on individual children — the spec § 3.1 models them on the multicol box. So in v1 they apply *uniformly* to every child (a `break_before: Column` on the container makes every child force a break, which with the first-child no-op effectively means one-child-per-column). This is the honest v1 reading of the shipped component shape; **per-child breaks** (the richer CSS model where each child carries its own `break-*`) require a per-child component that does not exist yet — record it as a follow-up (T9 asserts the container-level forcing-break behavior, not per-child). If the reviewer judges container-level uniform breaks too surprising, the alternative is to NOT wire breaks at all in v1 (set both `bf`/`af` to `false`) and defer all break handling — but wiring the container-level field is the literal reading of "respect break-* properties" given the shipped API. **Implementer: keep the container-level wiring; document the per-child follow-up.**
  **Implementer note (width):** `mc.column_width.map(|_| multicol_length_px(mc.column_width, 0.0))` yields `Some(px)` only when `column_width` is set, passing `None` through to `resolve_column_count` otherwise (so the count-only / neither branches are taken). A `Some` non-`Px` width resolves to `0.0`, which `resolve_column_count`'s `width_derived` treats as 0 → clamps to the count-only/neither path (D8).
  **Implementer note (imports):** add `BreakAfter`, `BreakBefore`, `PositionKind` to the `systems.rs` `use super::types::{…}` block if not already present (`PositionKind` is used by sticky/anchor so it is; `BreakBefore`/`BreakAfter` may need adding). `NonSend`, `Children`, `With`, `Node` are in the Bevy prelude already used by sibling systems. Confirm the `Position` component import (`use super::components::{…}` — used by sticky).

- [ ] **Step 3: Remove the obsolete stub test.** Delete the `multicol_pack_warns_once_per_session` test (`systems.rs:4353-4381`) — the stub it tested no longer exists (the system no longer warns on mere presence; the residual warn is tested via integration in T7/T9). Replace it with a short comment marking the move:
  ```rust
  // Phase 13 — the Phase-7 `multicol_pack_warns_once_per_session` stub
  // test was removed: 6c now packs columns (no blanket warn). Packing +
  // the residual fragmentation warn are covered by
  // `tests/layout_multicol.rs`.
  ```

- [ ] **Step 4: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_multicol two_column_count packed_child_resolved no_multicol_writes
  ```
  Expected PASS. Then the unit suite:
  ```bash
  cargo test -p buiy_core multicol
  ```
  Expected PASS (the deleted stub test no longer runs; `multicol_length_px` + `resolve_column_count` + `pack_columns` still pass).

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_multicol.rs
  git commit -m "feat(layout): real multicol_pack sub-pass 6c (Phase 13 — spec § 3.2, D1/D6/D7)

Replaces the warn-once stub: resolve used column count from the container
content box, pack in-flow children into equal-width columns as whole boxes,
write container-relative positions to PostTaffyPositionOverrides. Out-of-flow
+ Display::None children excluded. Residual fragmentation warn lands in T7.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 6: Residual fragmentation warn in `multicol_pack`

**Spec:** § 3.2, architecture § 6, D2, D5.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the `MulticolFragmentationDeferred` emit + `LayoutWarnedOnceSession` param to `multicol_pack`)
- Modify: `crates/buiy_core/tests/layout_multicol.rs` (residual-warn integration test)

- [ ] **Step 1: Failing test.** Add to `tests/layout_multicol.rs`:
  ```rust
  use buiy_core::layout::{ColumnFill, LayoutWarnOnceKey, LayoutWarnedOnceSession};

  #[test]
  fn balanced_fill_with_oversized_child_warns_once() {
      // column_fill: Balance + a child taller than the resolved column
      // block-size → fragmentation would be needed; v1 greedy-packs and
      // warns once per session (plan D5).
      let mut app = app();
      let mc = MultiColumn {
          column_count: ColumnCount::Count(2),
          column_fill: ColumnFill::Balance,
          ..Default::default()
      };
      // Container content-box 200x100; one 100x250 child (250 > 100).
      let (_container, _kids) =
          multicol_container(&mut app, 200.0, 100.0, mc, &[(100.0, 250.0)]);
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert_eq!(
          warned.set.iter()
              .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolFragmentationDeferred))
              .count(),
          1,
          "oversized child under Balance warns once",
      );

      // A second frame does not re-warn (session-wide dedup).
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert_eq!(
          warned.set.iter()
              .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolFragmentationDeferred))
              .count(),
          1,
      );
  }

  #[test]
  fn auto_fill_oversized_child_does_not_warn() {
      // column_fill: Auto does not promise balancing → no fragmentation
      // warn even for an oversized child.
      let mut app = app();
      let mc = MultiColumn {
          column_count: ColumnCount::Count(2),
          column_fill: ColumnFill::Auto,
          ..Default::default()
      };
      let (_container, _kids) =
          multicol_container(&mut app, 200.0, 100.0, mc, &[(100.0, 250.0)]);
      app.update();
      let warned = app.world().resource::<LayoutWarnedOnceSession>();
      assert_eq!(
          warned.set.iter()
              .filter(|k| matches!(k, LayoutWarnOnceKey::MulticolFragmentationDeferred))
              .count(),
          0,
          "Auto fill does not warn",
      );
  }
  ```
  **Implementer note:** `ColumnFill`, `LayoutWarnOnceKey`, `LayoutWarnedOnceSession` are re-exported from `buiy_core::layout` (`mod.rs:21-37`). The container content height in the fixture = `height_px(100.0)` (no padding), so `content_height` = 100 and the 250-tall child exceeds it.
  Run: `cargo test -p buiy_core --test layout_multicol balanced_fill_with_oversized auto_fill_oversized` — expected FAIL (no warn emitted yet; `LayoutWarnedOnceSession` not a param).

- [ ] **Step 2: Add the warn to `multicol_pack`.** Add the `warned` param to the signature:
  ```rust
  pub(super) fn multicol_pack(
      tree: NonSend<LayoutTree>,
      multicol_q: Query<(Entity, &MultiColumn), With<Node>>,
      children_q: Query<&Children>,
      display_q: Query<&Display>,
      position_q: Query<&Position>,
      mut overrides: ResMut<PostTaffyPositionOverrides>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
  ```
  After computing `(count, col_width)` and before/around the `pack_columns` call, detect the residual condition and emit once. Insert this block just before `let packed = pack_columns(...)`:
  ```rust
          // Residual: balanced fill cannot be honored without splitting an
          // oversized child across columns (true fragmentation — tier-E,
          // deferred, plan D2/D5). Detect a child taller than a column's
          // block-size under `column_fill: Balance` and warn once per
          // session; the layout still greedy-packs whole children.
          let col_block_size = content_height;
          if matches!(mc.column_fill, ColumnFill::Balance)
              && packed_input.iter().any(|c| c.height > col_block_size)
              && warned.set.insert(LayoutWarnOnceKey::MulticolFragmentationDeferred)
          {
              bevy::log::warn!(
                  "Layout: a multi-column child is taller than its column and \
                   `column-fill: balance` needs content fragmentation, which is \
                   deferred to v1.x (flex-and-grid.md § 3.2). Falling back to \
                   greedy whole-child packing. This warn fires once per session.",
              );
          }
  ```
  **Implementer note:** `col_block_size` reuses `content_height` (the container's Taffy content box height) — the same value passed to `pack_columns` as `col_block_size`, so "taller than a column" is consistent between the warn and the packer. `ColumnFill` is in scope via `MultiColumn`'s field type; add it to the `use super::types::{…}` block if the compiler complains. `LayoutWarnOnceKey` / `LayoutWarnedOnceSession` are already used by sibling sub-passes in `systems.rs`.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_multicol balanced_fill_with_oversized auto_fill_oversized
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_multicol.rs
  git commit -m "feat(layout): residual fragmentation warn in 6c (Phase 13 — spec § 3.2, D5)

Balanced fill + an oversized child needs true fragmentation (tier-E, deferred);
6c greedy-packs and emits MulticolFragmentationDeferred once per session. Auto
fill does not warn.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

### Task 7: Integration suite — width-derived count, gap, break, out-of-flow exclusion

**Spec:** § 3.2, § 5 ("Multi-column" test bullet — graduated), D4, D6.

**Files:**
- Modify: `crates/buiy_core/tests/layout_multicol.rs` (extend with the remaining § 3.2 / § 5 fixtures)

- [ ] **Step 1: Add the remaining fixtures.** Append to `tests/layout_multicol.rs`:
  ```rust
  use buiy_core::layout::{BreakBefore, Length, PositionKind};

  #[test]
  fn column_width_derives_count_with_gap() {
      // column_width 90px, gap 20px, container content width 340.
      // width_derived = floor((340+20)/(90+20)) = floor(360/110) = 3 cols;
      // used width = (340 - 2*20)/3 = 100. Four 100x40 children, col block
      // 100 → col0 [c0@0,c1@40], col1 [c2@0,c3@40], col2 [].
      // col x: col0=0, col1=100+20=120.
      let mut app = app();
      let mc = MultiColumn {
          column_width: Some(Length::Px(90.0)),
          column_gap: Some(Length::Px(20.0)),
          ..Default::default()
      };
      let (_c, kids) = multicol_container(
          &mut app, 340.0, 100.0, mc,
          &[(100.0, 40.0), (100.0, 40.0), (100.0, 40.0), (100.0, 40.0)],
      );
      app.update();
      let o = app.world().resource::<PostTaffyPositionOverrides>();
      assert_eq!(o.by_entity.get(&kids[0]).copied(), Some(Vec2::new(0.0, 0.0)));
      assert_eq!(o.by_entity.get(&kids[1]).copied(), Some(Vec2::new(0.0, 40.0)));
      // col1 x = used_width(100) + gap(20) = 120.
      assert_eq!(o.by_entity.get(&kids[2]).copied(), Some(Vec2::new(120.0, 0.0)));
      assert_eq!(o.by_entity.get(&kids[3]).copied(), Some(Vec2::new(120.0, 40.0)));
  }

  #[test]
  fn container_level_break_before_forces_one_child_per_column() {
      // break_before: Column on the container applies to every child
      // uniformly (v1 container-level model). First child no-op; each
      // subsequent child starts a new column → one child per column until
      // the last column saturates. 3 cols, 3 children → c0 col0, c1 col1,
      // c2 col2.
      let mut app = app();
      let mc = MultiColumn {
          column_count: ColumnCount::Count(3),
          break_before: BreakBefore::Column,
          ..Default::default()
      };
      let (_c, kids) = multicol_container(
          &mut app, 300.0, 500.0, mc,
          &[(100.0, 10.0), (100.0, 10.0), (100.0, 10.0)],
      );
      app.update();
      let o = app.world().resource::<PostTaffyPositionOverrides>();
      // used width = 300/3 = 100, gap 0 → col x = 0,100,200.
      assert_eq!(o.by_entity.get(&kids[0]).copied(), Some(Vec2::new(0.0, 0.0)));
      assert_eq!(o.by_entity.get(&kids[1]).copied(), Some(Vec2::new(100.0, 0.0)));
      assert_eq!(o.by_entity.get(&kids[2]).copied(), Some(Vec2::new(200.0, 0.0)));
  }

  #[test]
  fn absolute_child_is_excluded_from_columns() {
      // An absolutely-positioned child escapes the column flow (plan D6):
      // it gets no override. The in-flow child is packed normally.
      let mut app = app();
      let mc = MultiColumn { column_count: ColumnCount::Count(2), ..Default::default() };
      let container = app
          .world_mut()
          .spawn((Node, Style::default().width_px(200.0).height_px(100.0).multi_column(mc)))
          .id();
      let abs = app
          .world_mut()
          .spawn((Node, Style::default().width_px(50.0).height_px(20.0).position(PositionKind::Absolute)))
          .id();
      let flow = app
          .world_mut()
          .spawn((Node, Style::default().width_px(100.0).height_px(40.0)))
          .id();
      app.world_mut().entity_mut(container).add_children(&[abs, flow]);
      app.update();
      let o = app.world().resource::<PostTaffyPositionOverrides>();
      assert!(o.by_entity.get(&abs).is_none(), "absolute child escapes columns");
      assert_eq!(o.by_entity.get(&flow).copied(), Some(Vec2::new(0.0, 0.0)), "in-flow child packed");
  }

  #[test]
  fn display_none_child_is_skipped() {
      // A Display::None child is skipped; the following in-flow child takes
      // the first slot (no phantom gap from the hidden box).
      let mut app = app();
      let mc = MultiColumn { column_count: ColumnCount::Count(1), ..Default::default() };
      let container = app
          .world_mut()
          .spawn((Node, Style::default().width_px(200.0).height_px(500.0).multi_column(mc)))
          .id();
      let hidden = app
          .world_mut()
          .spawn((Node, Style::default().display(Display::None)))
          .id();
      let visible = app
          .world_mut()
          .spawn((Node, Style::default().width_px(100.0).height_px(40.0)))
          .id();
      app.world_mut().entity_mut(container).add_children(&[hidden, visible]);
      app.update();
      let o = app.world().resource::<PostTaffyPositionOverrides>();
      assert!(o.by_entity.get(&hidden).is_none());
      assert_eq!(o.by_entity.get(&visible).copied(), Some(Vec2::new(0.0, 0.0)));
  }
  ```
  **Implementer note:** `BreakBefore`, `Length`, `PositionKind` are re-exported from `buiy_core::layout`. `.position(PositionKind::Absolute)` + `.display(Display::None)` setters exist (`style.rs:205`, `:51`). The `container_level_break_before_forces_one_child_per_column` test encodes the v1 container-level break semantics from T5's implementer note — if the reviewer changes that decision to "no break wiring in v1", this test must change to assert greedy packing instead (flag it to the reviewer).

- [ ] **Step 2: Run.**
  ```bash
  cargo test -p buiy_core --test layout_multicol
  ```
  Expected PASS (all fixtures).

- [ ] **Step 3: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_multicol.rs
  git commit -m "test(layout): Phase 13 multicol integration suite (spec § 3.2, § 5)

Width-derived count + gap, container-level forced break, absolute/Display::None
exclusion. Graduates the spec § 5 multicol bullet from stub-warns to real packing.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Self-review (against the spec)

**Spec coverage** (`flex-and-grid.md` § 3):
- § 3.1 `MultiColumn` component + value types → already shipped (Phase 7); Phase 13 reads them (T5/T6), adds no new type. ✓
- § 3.2 step 1 (determine column count: count / width / both / neither) → T3 (`resolve_column_count`), T5 (system supplies `available_width`). ✓
- § 3.2 step 2 (lay out children into columns, respect `break-*`, overwrite position) → T4 (`pack_columns` whole-child greedy + forced breaks), T5 (system gathers + writes overrides), T7 (break integration). ✓
- § 3.2 "stub … emits the deferral warn" → graduated: the blanket `MulticolUnsupported` warn is retired (T1); a *residual* `MulticolFragmentationDeferred` warn covers the genuine tier-E gap (T1 key, T6 emit). ✓
- § 5 "Multi-column stub warns" test bullet → graduated to real packing tests (T5/T6/T7); the residual-warn behavior is the documented replacement. ✓
- Architecture § 3 (6c in the `PostTaffyOverrides` chain, position-only override) → T5 writes only `PostTaffyPositionOverrides` (D1); pipeline wiring unchanged (6c already in the chain). ✓
- Architecture § 6 (warn-once session dedup) → T1 key + T6 emit via `LayoutWarnedOnceSession` (`if set.insert {warn!}`). ✓

**Deferrals recorded (tier-E, prior-art-grounded):**
- True content fragmentation (one box → multiple column fragments) — D2/D5, blink prior-art (the decade-long, deliberately-last LayoutNG feature; Buiy is one-rect-per-entity). Residual warn `MulticolFragmentationDeferred`.
- `break-inside: avoid` / break-*-avoidance balancing — D4 (folded into the fragmentation deferral; whole-child packing trivially satisfies avoid-inside).
- Per-child `break-*` (v1 wires the container-level fields uniformly per the shipped `MultiColumn` shape) — D4 + T5 implementer note; per-child component is a follow-up.
- Percent / cq `column-width` / `column-gap` — D8 (`Px`-only, matching `length_px`).
- `column-rule` / `column-span` — render-side / not-positional; spec § 3.1 says "render side honors this; layout side passes it through" (`types.rs:858-860`); 6c does not consume them (correct — they are paint, not packing).

**Placeholder scan:** every task has full test code + full implementation code + exact commands. No "TBD" / "similar to" / "add error handling". The one judgment seam (container-level vs no break wiring, T5 implementer note) is explicitly flagged with both options + a default + the dependent test (T7) called out.

**Type consistency:** `multicol_length_px(Option<Length>, f32) -> f32` (T2, used T5/T6); `resolve_column_count(ColumnCount, Option<f32>, f32, f32) -> (usize, f32)` (T3, used T5); `MulticolChild { entity, height, force_break_before, force_break_after }` + `PackedChild { entity, offset }` + `pack_columns(&[MulticolChild], usize, f32, f32, f32) -> Vec<PackedChild>` (T4, used T5); `LayoutWarnOnceKey::MulticolFragmentationDeferred` (T1, used T6); `multicol_pack` final signature `(NonSend<LayoutTree>, Query<(Entity,&MultiColumn),With<Node>>, Query<&Children>, Query<&Display>, Query<&Position>, ResMut<PostTaffyPositionOverrides>, ResMut<LayoutWarnedOnceSession>)` (T5 base + T6 adds `warned`). Every symbol used in a later task is defined in an earlier one.
