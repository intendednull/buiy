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

### Added (Phase 6 — layout anchor positioning)
- **CSS anchor positioning (Phase 6 of the layout migration).** Spec:
  `docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md`
  § 3. Plan: `docs/plans/2026-05-21-buiy-layout-anchor-positioning.md`.
- **`Anchor` component** (decomposed-only per spec § 2.4 — NOT in the
  `Style` Bundle, spawned alongside): `anchor_name: Option<AnchorName>`
  declares the entity as an anchor target; `position_anchor:
  Option<AnchorRef>` declares the entity is anchored TO another;
  `position_try: Vec<PositionTry>` is the ordered fallback chain.
- **`LayoutAnchorBroken` devtools marker.** Unit-struct, present iff
  the anchor failed to resolve this frame (target missing, every
  fallback failed, or in a cycle). Idempotent insert/remove — no
  `Changed<LayoutAnchorBroken>` churn (preserves Phase 2 O(0)
  steady-state invariant).
- **Five value types** in `layout::types`: `AnchorName`
  (`Implicit` | `Named(String)` — spec uses `SmolStr` but Phase 6
  follows Phase 3 `String` precedent to avoid a new dep),
  `AnchorRef` (`Entity(Entity)` | `Name(String)`), `PositionTry`
  (`inset` + `conditions`), `TryCondition`
  (`FitsInViewport` | `FitsInContainer(AnchorRef)` |
  `AnchorVisible`), and `AnchorErrorKind` (5 per-frame warn-dedup
  categories).
- **Four `Inset` convenience constructors** mirroring the spec's
  authoring example (§ 3.3): `Inset::above(Length)`, `below(Length)`,
  `left_of(Length)`, `right_of(Length)`.
- **`AnchorNameRegistry` resource** (Phase 6 introduces the first
  `Resource` maintained by Bevy 0.18 observers, not by a regular
  system). Storage: `HashMap<String, Vec<(Entity, u64)>>` (last-wins
  semantics for duplicate names) + `HashMap<Entity, u64>` (per-entity
  insertion epoch — used by the Kahn cycle-edge-drop tiebreaker) +
  monotonic `next_epoch` counter. Public methods: `find_entity_by_name`,
  `entity_epoch`. Crate-internal accessors: `insert`, `track_epoch`
  (epoch-only path for unnamed anchors — no empty-string sentinel
  bucket), `remove`, `iter_buckets` (for the in-resolver
  `DuplicateName` scan).
- **Three closure-based observers** registered in `LayoutPlugin::build`:
  `On<Insert, Anchor>` (adds to registry via `handle_anchor_insert`
  private helper), `On<Replace, Anchor>` (pre-replace cleanup),
  `On<Remove, Anchor>` (post-remove cleanup). Closures were chosen
  over named `fn` items because `On<'w, 't, E, B>` has two lifetime
  parameters without defaults, and Rust's lifetime elision in named-fn
  signatures for multi-lifetime structs is subtle.
- **`AnchorOverrides` resource** (`pub by_entity: HashMap<Entity, Vec2>`).
  Cleared at the top of `anchor_resolution`, populated per resolved
  anchored entity, consulted by `write_resolved_layout` (step 7) to
  override the position (size still always from Taffy).
- **`LayoutAnchorWarnedThisFrame` resource** (`pub set: HashSet<(Entity,
  AnchorErrorKind)>`) for per-frame warn dedup. Cleared at the top of
  `anchor_resolution`; populated solely by `anchor_resolution` (not by
  observers — observer-side warns would be silently lost on the clear).
- **`anchor_resolution` system** attached in `BuiyLayoutStep::PostTaffyOverrides`
  (sub-pass 6d of the 9-step pipeline). Algorithm:
  1. Clear frame-local state.
  2. Build the `(anchored → anchor_target)` edge map. Targets are
     resolved by `AnchorRef::Entity(e) → e` or
     `AnchorRef::Name(n) → registry.find_entity_by_name(n)`, with
     `Display::None` targets treated as missing (D9: explicit
     `Query<&Display>` lookup, because `sync_styles` does NOT remove
     `Display::None` entities from `tree.by_entity` — it sets
     `taffy::Display::None` and `tree.tree.layout()` returns a
     zero-size box, not `Err`).
  3. Hand-rolled Kahn topological sort over the edge DAG with O(V+E)
     re-runs after each cycle-edge drop. The dropped edge belongs to
     the cycle node with the highest insertion epoch; both endpoints
     of the dropped edge are added to the `broken_set` (spec § 3.4
     line 229: "Both endpoints get `LayoutAnchorBroken` markers"). The
     Kahn helper does its own pre-pass to ensure external target
     nodes (entities pointed at via `AnchorRef::Entity(e)` that
     themselves have no `Anchor`) are well-defined keys in the edge
     map, preventing an infinite drop-loop.
  4. Detect duplicate names by scanning `reg.iter_buckets()` for
     `bucket.len() > 1` (D11 — the late inserter is the warn target;
     persists across frames as long as the duplicate is live).
  5. For each anchored entity in topological order, read the target's
     box from `tree.tree.layout()` (spec § 3.2 + architecture.md § 3.2
     prior-art for the same reason `cq_flip_check` reads from Taffy:
     `ResolvedLayout` is stale during sub-pass 6 because step 7 hasn't
     written it yet). Iterate `position_try`; first try whose
     `TryCondition`s all pass wins. Write the resolved position to
     `AnchorOverrides.by_entity`. On no-try-passes: write `Vec2::ZERO`,
     add to broken set, emit `AllFallbacksFailed` warn.
  6. Idempotent `LayoutAnchorBroken` marker management — covers both
     anchored entities AND plain-Node cycle-target entities via a
     second `broken_query`.
  7. Emit warns via per-(entity, kind) dedup gate.
- **Sub-pass 6d cost contract**: `O(anchored entities × tries + V + E)`
  per spec § 3.4 line 230. The pass does NOT trigger Taffy re-layout
  (spec § 3.4 line 231).
- **`anchor_resolution` takes `NonSend<LayoutTree>`** (read-only, no
  `compute_layout` calls — Phase 5 same-frame re-layout cap of 2×
  Taffy stays intact).
- **`write_resolved_layout` extended** to consult `Res<AnchorOverrides>`:
  position from override when present, else from Taffy. Size always
  from Taffy. Existing idempotent-insert compare is unchanged.
- **`sync_styles` `Or<>` filter widened** to include `Changed<Anchor>`
  in the inner nested `Or` (now at 5 entries: Container,
  ContainerQuery, ContainerQueryActive, ContainerQueryInactive,
  Anchor). Outer Or stays at Bevy 0.18's 15-tuple cap.
  `LayoutAnchorBroken` is intentionally OMITTED (devtools-only marker
  that doesn't affect Taffy translation).
- **11 integration tests** at `crates/buiy_core/tests/layout_anchor_positioning.rs`:
  basic anchor positioning, `AnchorOverrides`-vs-Taffy precedence,
  `sync_styles` re-runs on `Changed<Anchor>`, two-try fallback chain
  (first overflows viewport, second wins), 2-node cycle with
  both-endpoints-broken assertion, duplicate-name warn persistence
  across frames, missing target → broken + warn, broken marker clears
  when resolution succeeds, steady-state O(0) `sync_styles` invariant,
  observer registry cleanup on despawn, `Display::None` target →
  broken.
- **Pipeline-order test augmentation** at
  `crates/buiy_core/tests/layout_pipeline_order.rs`: anchor target +
  anchored entity pair so the 9-step chain assertion exercises the
  PostTaffyOverrides slot end-to-end.

### Changed (Phase 6)
- `LayoutPlugin::build` gains 3 `init_resource` calls (`AnchorNameRegistry`,
  `AnchorOverrides`, `LayoutAnchorWarnedThisFrame`), 3 `add_observer`
  registrations (Insert/Replace/Remove of `Anchor`), 7 `register_type`
  calls for reflection (`Anchor`, `LayoutAnchorBroken`, `AnchorName`,
  `AnchorRef`, `PositionTry`, `TryCondition`, `AnchorErrorKind`), and 1
  `add_systems` call attaching `anchor_resolution` to
  `BuiyLayoutStep::PostTaffyOverrides`.
- `buiy_core` and `buiy` facade crates re-export the 7 new public types.
- A forward-looking comment in `mod.rs` notes that future Phase 7
  systems attaching to `PostTaffyOverrides` (sticky 6a, table 6b,
  multicol 6c) must add `.before(anchor_resolution)` to preserve the
  spec's declared 6a → 6b → 6c → 6d sub-pass order.

### Deferred / divergences from spec (Phase 6)
- **`anchor-size()` in `PositionTry::inset`** — tier-C deferred to v1.x
  per spec § 3.4 line 231. Phase 6 ships the `AnchorErrorKind::AnchorSizeUsed`
  variant with a stub that resolves to `0.0`; a future
  `Length::AnchorSize` extension can land without churn.
- **`position_try_max_depth` resource cap** — README § 5 open question
  ("if profiling surfaces deeply-nested fallback hot paths"). Phase 6
  evaluates the full chain linearly. Tracked in
  `docs/plans/follow-ups.md`.
- **Cross-window anchor targets** — spec silent. Phase 6 implementation:
  cross-window targets emit `TargetMissing` and broken (their
  `tree.by_entity` lookup fails because they live in a different
  `LayoutTree` root). Tracked in `docs/plans/follow-ups.md`.
- **Anchor target IS sticky/table/multicol** (Phase 7 interaction) —
  `anchor_resolution` reads from `tree.tree.layout()`, which is Taffy's
  *pre-correction* position. When Phase 7 lands the 6a/6b/6c sub-passes,
  they need either (a) `.before(anchor_resolution)` ordering so the
  corrected ResolvedLayout is available *as a separate per-entity
  buffer* anchor reads from, OR (b) move `anchor_resolution` to run
  after those corrections. Tracked in `docs/plans/follow-ups.md`.
- **Steady-state cost** — `anchor_resolution` is `O(anchored)`, not
  `O(0)`. Spec architecture.md § 9 line 265 explicitly carves out
  "steps 0, 6, 7 are `O(roots + anchored)`" — this is within the
  declared cost contract. Phase 2's O(0) invariant applies to
  `sync_styles` and the absence of `Changed<ResolvedLayout>` churn
  from anchor pass — both preserved.
- **Per-frame warn dedup vs per-`BuiyExit` (architecture.md § 6)** —
  Phase 6 anchor errors use per-frame dedup (matching display-and-positioning.md
  § 3.2 step 4 "warn fires once per (entity, frame)"); other Phase 1-5
  warn paths (Taffy `Err`, `Length::Fr` outside grid, etc.) continue
  to use per-session `AtomicBool` gates. Documented spec divergence;
  a future cleanup may unify behind a single `LayoutWarnLog` resource
  with per-kind policies.
- **`cq_flip_rerun` filter NOT widened with `Changed<Anchor>`** — only
  `sync_styles` was. `cq_flip_rerun` only runs when a CQ flips, and
  `sync_styles` (step 1) already covers `Changed<Anchor>` in the same
  frame. No correctness gap; minor latency gap in a hypothetical
  scenario where a mid-pipeline system inserts an `Anchor` after step
  1 but before step 5 — no such system exists today.

### Removed (Phase 6)
- (none) — Phase 6 is purely additive.

### Added (Phase 7 — layout sticky/table/multicol)
- **Sticky positioning (sub-pass 6a, tier-F, full impl).** Spec:
  `display-and-positioning.md § 2.3`. `sticky_offset` system walks
  `ChildOf` for nearest scroll-container ancestor, computes per-axis
  displacement (CSS spec § 6.3 algorithm: `max(natural, threshold)`
  clamped by parent), writes resolved position to the shared
  `PostTaffyPositionOverrides` resource. Pure helpers:
  `nearest_scroll_container`, `world_position` (per-call memoized by
  `(Entity, ancestor)` pair), `resolve_sticky_inset` (closed match on
  `Length` variants), `compute_sticky_displacement` (pure function, per-
  axis).
- **Table layout (sub-pass 6b, tier-C, stub).** Spec:
  `display-and-positioning.md § 1.2`. `table_layout` no-op stub emits one
  warn per (entity, session) on first encounter of any `Display::Table*`
  variant. Fallback path (Table → Block) already ships from Phase 1.
  Algorithm deferred to v1.x.
- **Multi-column layout (sub-pass 6c, tier-E, stub).** Spec:
  `flex-and-grid.md § 3`. `multicol_pack` no-op stub emits one warn per
  session on first `MultiColumn` encounter (session-wide; no per-entity
  dedup). Algorithm deferred to v1.x.
- **`MultiColumn` component** with 9 fields per spec § 3.1
  (`column_count`, `column_width`, `column_gap`, `column_rule`,
  `column_span`, `column_fill`, `break_inside`, `break_before`,
  `break_after`). `Style.multi_column` field + `.multi_column()` fluent
  setter per spec § 2.4 (container-side convention).
- **8 supporting enum types:** `ColumnCount`, `ColumnRule`,
  `ColumnRuleStyle`, `ColumnSpan`, `ColumnFill`, `BreakInside`,
  `BreakBefore`, `BreakAfter`. All registered with reflection.
- **`LayoutWarnedOnceSession` resource** + `LayoutWarnOnceKey` enum
  (variants: `TableUnsupported(Entity)`, `MulticolUnsupported`,
  `StickyFrUnsupported(Entity)`, `StickyCqDeferred(Entity)`).
  Session-scoped warn dedup per spec § 6 ("HashSet resource cleared on
  `BuiyExit`"). Phase 6's per-frame `LayoutAnchorWarnedThisFrame` stays
  unchanged (different scope, different consumer).
- **`clear_post_taffy_overrides` system** — dedicated per-frame clear
  for the shared override map. Runs first in
  `BuiyLayoutStep::PostTaffyOverrides` chain.
- **31 integration tests** across `tests/layout_sticky.rs` (15),
  `tests/layout_table_multicol_stubs.rs` (8),
  `tests/layout_post_taffy_overrides_clear.rs` (1, from Task 2), and
  augmented `tests/layout_pipeline_order.rs` (cross-phase ordering
  proof). Plus 10+ unit tests in `systems.rs::mod tests`.
- **`Changed<MultiColumn>`** added to `sync_styles`'s inner `Or<>` filter
  per spec `architecture.md § 1.2` (forward-compat — multicol doesn't
  feed Taffy in v1 but the trigger is wired for the v1.x algorithm).

### Changed (Phase 7)
- **Renamed `AnchorOverrides` → `PostTaffyPositionOverrides`** (Phase 6
  → Phase 7). Same shape (`HashMap<Entity, Vec2>`), same per-frame-
  cleared semantics; widened scope from "anchor-only writer" to "any
  sub-pass writer." Public type rename — downstream code referencing
  `AnchorOverrides` directly must update. Phase 6 follow-up "anchor
  target IS sticky/table/multicol" closed: `anchor_resolution` (6d)
  reads target position from the override map first, falls back to
  `tree.tree.layout()` when no override exists — so anchors that target
  sticky-displaced elements produce correct dependents (per D1 fix in
  Task 9).
- **`BuiyLayoutStep::PostTaffyOverrides` set** now contains 5 systems
  chained in declared order:
  `clear_post_taffy_overrides → sticky_offset → table_layout → multicol_pack → anchor_resolution`
  (Phase 6 attached only `anchor_resolution`).

### Deferred / divergences from spec (Phase 7)
- **`Position::Fixed` — still a warn-once stub.** Phase 7's spec scope
  is sub-passes 6a/6b/6c; Fixed is a separate code path (Absolute with
  viewport-as-CB). Tracked in `docs/plans/follow-ups.md` "Layout —
  `Position::Fixed` implementation".
- **Multi-column algorithm — deferred to v1.x.** Per spec § 3.2
  ("Multi-column is tier-E; v1 ships the API but the algorithm is a
  stub"). Tracked in follow-ups.
- **Table layout algorithm — deferred to v1.x.** Per spec § 1.2 ("v1
  ships only the API surface and the fallback path"). The fallback path
  (Table → Block via `translate.rs::map_display`) already ships from
  Phase 1.
- **Sticky `Length::Cq*` inset resolution — deferred.** Full container-
  units resolution requires plumbing the cq-context (sticky's reference
  frame is the sticky entity's own cq-ancestor — distinct from anchor's
  "anchor target box" frame). v1 resolves to 0.0 with
  `LayoutWarnOnceKey::StickyCqDeferred(Entity)` warn per (entity,
  session). Tracked in follow-ups.
- **Sticky `Length::Fr` inset — invalid.** `fr` is a grid-only unit;
  applying it to a sticky inset is semantically wrong. v1 resolves to
  0.0 with `LayoutWarnOnceKey::StickyFrUnsupported(Entity)` warn per
  (entity, session).
- **No em/rem/V* support in sticky insets.** `Length::Em(_)`,
  `Length::Rem(_)`, `Length::Vh/Vw/Vmin/Vmax` do not exist as `Length`
  variants in v1 (verified at `types.rs:29-50`). When Phase 10
  (viewport units) or a font-rendering phase adds them, the sticky
  inset resolver gains new arms (currently closed-match — compiler
  forces the addition).
- **Both-top-and-bottom-inset sticky — "top wins" (v1 deviation).** Per
  D4. CSS spec § 6.3 implies a dual-clamp ("sticks to whichever edge
  the scroll position is currently closer to"); Phase 7's simpler "top
  wins" matches WebKit/Blink in the common case. Documented test
  `sticky_both_top_and_bottom_inset_top_wins` is the regression test;
  flipping it documents the algorithm upgrade. Tracked in follow-ups.
- **Sticky inside sticky — inner uses outer's natural (un-displaced)
  position.** `world_position` walks Taffy positions; an inner sticky
  inside an outer sticky resolves its threshold against the outer's
  *natural* position, not the displaced one. Rare authoring case.
  Tracked in follow-ups for v1.x.
- **`LayoutWarnedOnceSession` `BuiyExit` clear — wiring deferred.** The
  `clear_warned_once_on_exit` function exists with
  `#[allow(dead_code)]`; the wire-up to `OnExit(BuiyState::Active)`
  depends on the foundation lifecycle which is still draft. Until
  wired, `App::new()` in tests starts with a clean resource (Bevy
  default per `init_resource`).

### Removed (Phase 7)
- (none) — Phase 7 is purely additive (plus rename `AnchorOverrides →
  PostTaffyPositionOverrides` — see Changed).

### Added (Phase 8 — layout transforms + containment)
- `UiTransform` component (self-styling visual transform; named
  `UiTransform` to avoid the glob-imported `bevy::prelude::Transform`
  collision) + value types `TransformMatrix` (None default),
  `TransformOrigin` (50%/50%/0 default), `TransformStyle` (Flat),
  `BackfaceVisibility` (Visible). Spec § 1.
- `Translate` / `Rotate` / `Scale` decomposed-only longhand components
  (`Scale::default()` is `(1,1,1)` per CSS). Spec § 1.1.
- `compose_transform(ui, t, r, s) -> Mat4` — pure helper implementing
  `M = T·R·S·M_transform` (None=IDENTITY, Translate/Rotate/Scale/Skew/Matrix
  direct, Compose=fold product A·B·…). Spec § 1.
- `transform_composition` system — `PostTaffyOverrides` sub-pass **6e**,
  runs after `anchor_resolution` (6d). Writes the composed matrix to
  `ResolvedTransform`; writes nothing to `PostTaffyPositionOverrides` (a
  transform does not move the layout box). Identity → no `ResolvedTransform`
  (+removes stale); skips `Display::None`. Spec § 1.1, § 7.
- `ResolvedTransform { matrix: Mat4 }` — private render handoff for
  transforms, mirroring `ResolvedLayout`. Spec § 1.
- `Style.ui_transform` field + `.ui_transform()` / `.translate_px()` /
  `.rotate_z()` / `.scale()` fluent setters.
- `Containment` component (`contain: ContainFlags`,
  `content_visibility: ContentVisibility`, `will_change: WillChange`) +
  value types `ContainFlags` bitflags (CONTENT/STRICT are bit-unions;
  `impl_reflect_opaque!`), `ContentVisibility` (Visible default),
  `WillChange` (Auto default), `WillChangeProperty`. Spec § 5.
- `Style.containment` field + `.containment()` / `.contain()` setters.
- SIZE / INLINE_SIZE containment enforcement: under `contain: size` /
  `inline-size`, an auto width/height on a contained axis is treated as
  `0px` with a warn-once `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)`.
  Spec § 5.1.
- `LayoutWarnOnceKey::SizeContainmentZeroed(Entity)` +
  `ContentVisibilityDeferred(Entity)` variants.
- Integration tests `tests/layout_transforms.rs` +
  `tests/layout_containment.rs`; pipeline-order test extended to assert
  6e runs.

### Changed (Phase 8)
- **`BuiyLayoutStep::PostTaffyOverrides` chain now has six elements:**
  `clear → sticky 6a → table 6b → multicol 6c → anchor 6d → transform 6e`
  (Phase 7 attached five).

### Deferred / divergences from spec (Phase 8)
- **Bevy `Transform`/`GlobalTransform` write — deferred (deliberate
  divergence from spec § 2 approach (a) at the implementation-timing
  level).** Phase 8 produces the spec's `ResolvedTransform` artifact but
  does NOT write Bevy `Transform`: render reads `ResolvedLayout` directly
  and `buiy_core` has no `TransformPlugin` wiring (the harness uses
  `MinimalPlugins`), so a `Transform` write would be dead code. The
  Bevy-`Transform` ownership bridge (spec approach (a)) is a
  render-pipeline / 3D-anchored-UI follow-up. Tracked in
  `docs/plans/follow-ups.md`.
- **`content-visibility` `Auto` + `Hidden` — stored, not enforced.**
  `Auto` needs last-frame `ResolvedLayout` + viewport +
  `contain-intrinsic-size`; `Hidden` needs a tree-skip path. Both
  deferred; value is stored and `content_visibility != Visible` warns once
  (`ContentVisibilityDeferred`). Spec § 5.2. Tracked in follow-ups.
- **`will-change` — stored-only (tier-E).** No layer promotion, no SC
  trigger in Phase 8 (SC triggers are Phase 9). Valid stored hint; no
  warn. Spec § 5.3. Tracked in follow-ups.
- **`LAYOUT` / `PAINT` / `STYLE` contain flags — stored, no Phase 8
  layout effect** beyond what Taffy already does (spec § 5.1:
  "render-side primarily; layout records").
- **Non-px translate units in transforms** — `compose_transform` resolves
  only `Length::Px` for translate; percent/cq translate (resolved against
  the entity's own box) contributes `0.0` and is deferred to the
  render/animation phase.
- **Stacking-context formation / top-layer — Phase 9 (sub-pass 6f reads
  `ResolvedTransform`).** A non-identity transform forms a stacking
  context (spec § 3), but detection is Phase 9. Not in Phase 8.

### Removed (Phase 8)
- (none) — Phase 8 is purely additive.

### Performance contract (Phase 8)
- Steady-state O(0) preserved: `transform_composition` is
  `O(UiTransform-bearing entities)`; identity transforms insert nothing.
  SIZE-containment substitution is `O(1)` per contained entity inside the
  existing `sync_styles` pass.
