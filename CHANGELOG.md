# Changelog

All notable changes to Buiy are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once it reaches `0.1.0`. Pre-`0.1.0` releases are pre-alpha; APIs may break in any commit.

## [Unreleased]

Pre-`0.1.0` development. Detailed change tracking begins with the first
tagged release.

### Added
- Render pipeline now produces real pixels for Buiy nodes (instance-buffer
  construction, clip-space conversion, draw call). Closes the Phase 0
  render deferral.
- Per-window AccessKit tree-update bridge. Buiy translates its widget tree
  to `accesskit::TreeUpdate` each frame and pushes it through bevy_winit's
  `ACCESS_KIT_ADAPTERS` so real screen readers attached to a Buiy window
  see the live tree. (Bevy 0.18 owns adapter creation, so Buiy bridges
  rather than owning `Adapter` objects directly.) Closes the Phase 0 a11y
  deferral.
- `bevy_picking` backend. `Hovered` becomes a thin layer over the standard
  `PointerHits` event flow. Closes the Phase 0 picking deferral.
- `buiy_core::components::Visual` component (`background_token`,
  `foreground_token`, `border_radius`) carrying the render-side surface
  formerly mixed into the Phase 0 mega-`Style`. Authors who want themed
  widgets insert `Visual` alongside the new layout `Style` builder.
  Eventual home is `buiy-render-pipeline-design`.
- `buiy_core::layout` module: 8-step layout pipeline (`BuiyLayoutStep`
  system sets), decomposed `BoxModel` / `Display` / `Position` /
  `FlexParams` / `FlexItem` components, hybrid `Style` builder that
  expands to a `Bundle` on spawn.
- Doc-hidden read-only accessors on `LayoutTree`: `by_entity()` and
  `tree_ref()` for integration-test introspection.
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
- `Overflow::is_scroll_container()` predicate (spec § 1.2).
- 9 supporting layout enum types: `OverflowMode`, `ScrollbarGutter`,
  `ScrollbarWidth`, `ScrollbarColor`, `ScrollBehavior`,
  `OverscrollBehavior`, `SnapType`, `SnapAlign`, `SnapStop`.
- `Style` builder: `Overflow` and `Scroll` fields; 12 fluent setters
  (`.overflow_x()`, `.overflow_y()`, `.overflow()`, `.overflow_hidden()`,
  `.overflow_y_scroll()`, `.overflow_x_scroll()`, `.scrollbar_gutter()`,
  `.scrollbar_width()`, `.scroll_behavior()`, `.snap_type()`,
  `.snap_padding()`, `.snap_margin()`).
- **Layout grid (Phase 3 of the layout migration).**
  - `GridParams` component (container, joins `Style`'s Bundle):
    `template_columns`, `template_rows`, `template_areas`,
    `auto_columns`, `auto_rows`, `auto_flow`, `justify_items`,
    `align_items`, `justify_content`, `align_content`, `gap`.
  - `GridItem` component (decomposed-only, like `FlexItem` /
    `ScrollSnapItem`): `column`, `row`, `justify_self`, `align_self`.
  - `TrackSize` enum: `Auto`, `Length`, `MinContent`, `MaxContent`,
    `FitContent`, `MinMax(Vec<TrackSize>)` (arity-2 invariant
    enforced at translate time — the spec's `Box<TrackSize>` shape
    can't be reflected by Bevy 0.18), `Repeat`, `Subgrid` (reserved).
  - `RepeatCount` enum: `AutoFill`, `AutoFit`, `Count(u16)`.
  - `GridLine` enum: `Auto`, `Start(i16)`, `Span(u16)`,
    `StartEnd(i16, i16)`, `Area(String)`.
  - `GridAreas` + `NamedArea`: explicit-rectangle named-area registry
    plus `GridAreas::from_lines(&[&str])` CSS-syntax convenience parser.
  - `GridAutoFlow` enum: `Row`, `Column`, `RowDense`, `ColumnDense`,
    `Masonry` (reserved).
  - `JustifyItems` enum: `Stretch` (default), `Start`, `End`, `Center`,
    `Baseline`.
  - 13 fluent setters on `Style`: `.grid()`, `.inline_grid()`,
    `.grid_template_columns(_)`, `.grid_template_rows(_)`,
    `.grid_template_areas(_)`, `.grid_auto_columns(_)`,
    `.grid_auto_rows(_)`, `.grid_auto_flow(_)`,
    `.grid_justify_items(_)`, `.grid_align_items(_)`,
    `.grid_justify_content(_)`, `.grid_align_content(_)`,
    `.grid_gap_px(_)`.
  - `GridParams` + `GridItem` + 7 value types registered for reflection
    in `LayoutPlugin`.

### Breaking
- `Length` gains an `Fr(f32)` variant. Downstream code that pattern-matches
  `Length` exhaustively must add an `Fr` arm. The `Fr` variant is only
  meaningful inside `TrackSize::Length(Length::Fr(_))` in a grid template;
  outside grid contexts it warns once and falls back to `0 px` (in
  `LengthPercentage` contexts — Taffy's type has no Auto) or `Auto`
  (in `Dimension` / `LengthPercentageAuto` contexts). `Length` is *not*
  marked `#[non_exhaustive]` — the next planned `Length` change is
  Phase 10's full unit set, which is similarly breaking; callers will
  adapt once and the type stabilizes.

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
- `sync_styles`' change-detection trigger set widens to include
  `Changed<Overflow>` and `Changed<Scroll>`; remains exclusive of
  `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>`.
- `Display::Grid` and `Display::InlineGrid` now translate to
  `taffy::Display::Grid` (Phase 1 routed both to Block).
- `sync_styles`'s `Or<(Changed<...>)>` trigger filter widens with
  `Changed<GridParams>` and `Changed<GridItem>`. `Changed<ChildOf>`
  remains in the filter from Phase 2 — re-parenting a grid item under
  a different grid container picks up the new `template_areas` via
  the existing clause.
- Reserved variants emit one `warn!` per session and degrade:
  `TrackSize::Subgrid → Auto`; `GridAutoFlow::Masonry → Row`.
- `RepeatCount::Count` carries `u16`; `GridLine::Start` / `Span` /
  `StartEnd` carry `i16` / `u16`. Spec used `i32` / `u32`; Phase 3
  matches Taffy 0.10 directly to avoid a lossy conversion at translate
  time.
- `GridLine::Area` uses `String` for area names. Spec used `SmolStr`;
  Phase 3 uses `String` to avoid a new direct supply-chain dep.
- **Layout writing modes (Phase 4 of the layout migration).**
  - `WritingMode` component (joins `Style`'s Bundle): `mode`,
    `direction`, `text_orientation`, `unicode_bidi`. CSS-faithful surface
    for writing-mode + direction + text-orientation + unicode-bidi.
  - `WritingModeResolved` private cache component, populated by the new
    `BuiyLayoutStep::WritingModeInherit` pipeline step. Memoized
    multi-level ancestor walk; idempotent insert preserves Phase 1's
    O(0) steady-state contract.
  - 4 supporting enums: `WritingModeKind` (HorizontalTb / VerticalRl /
    VerticalLr / SidewaysRl / SidewaysLr), `Direction` (Ltr / Rtl),
    `TextOrientation` (Mixed / Upright / Sideways), `UnicodeBidi`
    (Normal / Embed / Isolate / BidiOverride / IsolateOverride / Plaintext).
  - `LogicalEdges` value type with `to_edges(WritingModeKind, Direction)`.
  - `LogicalBoxModel` and `LogicalInset` author-ergonomic builder
    structs (non-component, non-Bundle) with `.to_box_model(&WritingMode)`
    and `.to_inset(&WritingMode)` methods. Vertical modes swap inline ↔
    block onto width ↔ height.
  - 7 fluent setters on `Style`: `.writing_mode(_)`,
    `.writing_mode_kind(_)`, `.direction(_)`, `.ltr()`, `.rtl()`,
    `.text_orientation(_)`, `.unicode_bidi(_)`.
  - `WritingMode` + `WritingModeResolved` + 5 value types registered
    for reflection in `LayoutPlugin`.
  - New `BuiyLayoutStep::WritingModeInherit` pipeline step inserted
    between `RemovedNodesGc` and `SyncStyles`. The 8-step layout chain
    becomes 9. `tests/layout_pipeline_order.rs` widens to assert all 9.
  - `Direction::Rtl` flows through `taffy::Style.direction`, mirroring
    flex children under RTL.
  - `WritingModeKind::Sideways{Rl,Lr}` emit one `warn!` per session and
    fall back to their non-sideways vertical equivalent for layout
    purposes. Glyph rotation is `buiy-text-rendering-design`'s concern.

### Changed (Phase 4)
- `inherit_writing_mode`'s ancestor walk treats `WritingMode::default()`
  as "unset" (CSS `initial`-like). Without this, every `Style`-spawned
  entity's bundled `WritingMode::default()` would short-circuit the
  walk so descendants of a non-default ancestor would resolve to
  default instead of inheriting. Trade-off: an author cannot explicitly
  pin `WritingMode::default()` as an override against a non-default
  ancestor; the result is observationally identical to inheriting the
  root default.
- `sync_styles`'s `Or<>` trigger filter widens with `Changed<WritingMode>`
  and `Changed<WritingModeResolved>`. **Phase 2 invariant intact:**
  `Changed<ScrollOffset>` / `Changed<ScrollSnapItem>` remain excluded.
- Pipeline gains a 9th step (`WritingModeInherit`); the layout chain
  is now 0 → wmi → 1 → ... → 7 in declared order.

### Deferred (Phase 4)
- `ContainingBlock` cache — deferred to Phase 6 (anchor positioning)
  where it has a real consumer.
- Dynamic writing-mode switches (CSS `Changed<WritingMode>` →
  re-translation pass) — Phase 4 v1 ships construct-time-only
  `LogicalBoxModel` / `LogicalInset` translation; runtime flips
  require re-spawn / re-insert. Re-translation pass is a v1.x feature
  contingent on resolving spec § 4.1 vs § 4.2 internal inconsistency.
- Vertical-mode Taffy axis-swap — Taffy 0.10 has no writing-mode
  awareness. Vertical modes are honored only by the logical builders;
  Taffy still flows the main axis horizontally. Authors who want
  top-to-bottom flow under vertical-rl use `Display::Flex(Column)`.
- Sideways glyph rotation — owned by `buiy-text-rendering-design`.
- BiDi resolution algorithm for `unicode_bidi` — owned by
  `buiy-i18n-design`. Phase 4 stores the value only.

### Removed
- `buiy_core::components::Style` (the Phase 0 mega-component) and
  `buiy_core::components::FlexDirection`. Their roles are taken by
  `buiy_core::layout::Style` (the hybrid builder) and
  `buiy_core::layout::FlexAxis` (the four-variant axis enum).

### Added (Phase 5 — layout container queries)
- **`Container` component** (joins `Style`'s Bundle): `container_type`
  (`Normal` / `Size` / `InlineSize`) and `container_name: Option<String>`.
  Descendants resolve `@container` rules and container units against
  this entity's resolved size. `String` over `SmolStr` matches Phase 3's
  `GridLine::Area` precedent.
- **`ContainerQuery` component** (decomposed-only, child-side):
  `container: Option<String>` (None = nearest queried ancestor;
  Some(name) = nearest by name) and `conditions: Vec<QueryCondition>`
  (AND-combined; empty list = always active).
- **`ContainerQueryActive` / `ContainerQueryInactive`** unit-struct
  marker components — the activation surface. `cq_activate` /
  `cq_flip_check` toggle them; authors observe `With<...>` to apply
  whatever behavior they want (style-bundle application is
  consumer-responsibility per spec § 1.2).
- **`Length::Cqw` / `Cqh` / `Cqi` / `Cqb` / `Cqmin` / `Cqmax`** container
  units. Resolve at translate time against the nearest queried
  ancestor's previous-frame `ResolvedLayout`. `Cqi` / `Cqb` honor the
  entity's `WritingModeResolved` (sideways modes normalized to
  vertical for axis selection, mirroring Phase 4 `LogicalEdges`).
  Fallback to viewport (`bevy::window::Window.resolution`) when no
  queried ancestor exists; one `warn!` per session.
- **`ContainerType`, `Orientation`, `QueryCondition`** value types.
  `Orientation::Portrait` (default) = `inline_axis <= block_axis`;
  `Landscape` = strict greater. `QueryCondition` covers `MinWidth` /
  `MaxWidth` / `MinHeight` / `MaxHeight` (taking `Length`),
  `MinAspectRatio` / `MaxAspectRatio` (taking `f32`), and
  `Orientation(Orientation)`.
- **`BuiyLayoutStep::CqActivate`** pipeline step (was a Phase 4 stub).
  `cq_activate` system: memoized nearest-queried-ancestor walk
  (mirrors Phase 4 `inherit_writing_mode` at `systems.rs:308-362`),
  reads previous-frame `ResolvedLayout`, idempotent marker insert
  (compare-before-write preserves Phase 1's O(0) steady-state).
- **`BuiyLayoutStep::CqFlipCheck`** pipeline step (was a Phase 4 stub).
  `cq_flip_check` system reads **fresh `tree.layout(node_id)`** from
  Taffy (per architecture.md § 3.2 explicit pinning — NOT the stale
  entity-side `ResolvedLayout`). Detects activation flips against
  this frame's just-computed sizes; sets `CqReRunRequested(true)`
  when any rule flipped. No-ancestor case marks inactive (allows a
  previously-active rule to flip back when its ancestor is despawned).
- **`BuiyLayoutStep::CqFlipReRun`** pipeline step (was a Phase 4 stub).
  `cq_flip_rerun` system (Approach B — normal Bevy system with the
  union of `sync_styles` + `taffy_compute` params, gated on
  `CqReRunRequested.0`) re-runs translation + Taffy compute once per
  flip. Same-frame re-layout cap is 2× Taffy; transitive flips wait
  for next frame. `translate_one_entity` factored out as a shared
  `pub(super) fn` so both `sync_styles` and `cq_flip_rerun` reuse the
  per-entity work without body duplication.
- **`Style` fluent setters** for container declaration:
  `.container_size()`, `.container_inline_size()`,
  `.container_name(_)`, `.container(_)`. Field is unconditional
  `Container` (NOT `Option<Container>`) because `Style` is
  `#[derive(Bundle)]` and Bevy 0.18 does not impl `Bundle` for
  `Option<T>`. Default sentinel `{ Normal, None }` is inert.
- **`LayoutTaffyComputeCount`** resource (`pub`): per-frame counter
  of Taffy `compute_layout` invocations. Reset at top of
  `taffy_compute`, incremented per-root in `taffy_compute` and once
  in `cq_flip_rerun`. Used by the same-frame re-layout cap tests.
- **`SyncStylesIterCount`** resource (`pub`): per-frame count of
  entities matched by `sync_styles`' Or-filter. Used by the
  idempotent-insert invariant test.
- **`Container` / `ContainerQuery` / `ContainerQueryActive` /
  `ContainerQueryInactive` / `ContainerType` / `Orientation` /
  `QueryCondition` registered for reflection** in `LayoutPlugin::build`.
  Re-exported from `buiy_core::layout` and bubbled up to `buiy_core`
  root, mirroring Phase 4's `WritingMode*` precedent.

### Changed (Phase 5)
- `sync_styles`'s `Or<>` trigger filter widens with `Changed<Container>`,
  `Changed<ContainerQuery>`, `Changed<ContainerQueryActive>`,
  `Changed<ContainerQueryInactive>`. **Bevy 0.18 caps `Or` tuples at
  15**, so the four new entries are nested inside an inner
  `Or<(...)>` (one outer slot, four inner). **Phase 2 invariant
  intact:** `Changed<ScrollOffset>` / `Changed<ScrollSnapItem>`
  remain excluded; asserted by `tests/layout_scroll_offset_no_invalidate.rs`.
- `sync_styles`'s filter also gains `Changed<ResolvedLayout>` (added
  to support initial-frame container-unit cascade — without it,
  descendants of newly-sized containers can't read the just-populated
  `ResolvedLayout`). Idempotency preserved by making
  `write_resolved_layout` compare-before-write (mirrors Phase 4's
  `inherit_writing_mode` idempotent insert).
- `taffy_compute` instrumented to bump `LayoutTaffyComputeCount`.
- `LayoutPlugin::build` registers `CqReRunRequested`,
  `LayoutTaffyComputeCount`, `SyncStylesIterCount` resources and
  attaches `cq_activate` / `cq_flip_check` / `cq_flip_rerun` to their
  respective `BuiyLayoutStep` sets.

### Deferred / divergences from spec (Phase 5)
- **`when_active` / `when_inactive: Option<Entity>` fields on
  `ContainerQuery`** (spec § 1.2): omitted in v1. The marker
  components are the activation surface; spec § 1.2 last paragraph
  says style-bundle application is consumer-responsibility. There is
  no in-tree consumer for the Entity fields in v1. Adding them later
  is non-breaking (additive Rust schema; Bevy reflection
  default-initializes new fields).
- **Multi-level geometric cascade** (spec § 1.3 transitive scenarios,
  spec § 1.5 test surface): when an ancestor's `ResolvedLayout`
  changes and a `Cqw`-sized intermediate (not in any `Changed<>`
  filter) sits between the ancestor and a rule-bearing descendant,
  the intermediate is **never re-translated** and the descendant's
  rule never re-evaluates. Direct-ancestor geometric cascade (rule
  on a direct child of a resized container) IS handled in-frame by
  `cq_flip_check`'s `tree.layout()` read + `cq_flip_rerun`. The
  Task 10 test `cq_transitive_cascade_is_one_frame_stale` is a
  **negative assertion** documenting this gap; it will be promoted
  to a positive assertion when a future phase adds descendant
  invalidation for ancestor-resolved-size changes. See
  `docs/plans/follow-ups.md`.
- **Style-bundle cascade** (spec § 1.2 `when_active`/`when_inactive`
  Entity fields): not shipped (see above).
- **Viewport-unit fallback as `Length::Vw/Vh` rewriting** (spec § 1.4):
  Phase 5 reads `bevy::window::Window.resolution` inline; observable
  behavior matches spec but the implementation path is direct-pixel
  read, not unit-rewrite. Phase 10 (`buiy-layout-units-calc`)
  replaces the inline read with `Length::Vw/Vh` infrastructure
  without behavior change.
- **Warn-once granularity** (spec § 1.4): spec asks per-entity, Phase 5
  uses session-global `AtomicBool`. Per-entity tracking via a
  `HashSet` resource grows unboundedly across despawns; the spec's
  intent (avoid log flood) is better served by global once-only.
- **Multiple ContainerQuery per entity**: v1 stores at most one (Bevy
  `Component` single-instance). Multi-query is a follow-up.

### Removed (Phase 5)
- The Task 1 `cq_unit_fallback_px` placeholder (deleted in Task 7
  when the real ancestor-driven resolver landed; transitional bridge,
  never shipped to users).
