# Layout follow-ups

**Date:** 2026-05-21
**Status:** active

Tracked-but-not-yet-scheduled work surfaced by completed layout phases. Each
entry names the originating phase, the divergence from spec / open behavior,
and a sketch of the implementation direction. When a follow-up gets
chartered into its own phase or plan, move the entry to that plan and link
back here.

## Descendant invalidation on ancestor-resolved-size changes — LANDED

**Originated:** Phase 5 (container queries), Task 10 implementer finding +
reviewer mandate. Plan `docs/plans/2026-05-21-buiy-layout-container-queries.md`
v3 revision documents the gap.

**Status:** **Landed** in Phase 14
(`docs/plans/2026-05-29-buiy-layout-descendant-invalidation.md`). The multi-level
geometric cascade is closed by two new pipeline steps after `write_resolved_layout`
(step 7): step 8 `cq_descendant_invalidate` reads `Changed<ResolvedLayout>` on query
containers (`Container { container_type != Normal }`), walks each changed container's
`Children` subtree, and collects the descendants into a private
`ContainerSizeDirty(HashSet<Entity>)` resource (sketch option **(b)** — the dirty-set
resource, not the `sync_styles`-filter marker of option (a)); step 9
`cq_descendant_rerun` (analogous to `cq_flip_rerun`) drains that set, re-translates
exactly those descendants so their `Length::Cq*` re-resolves against the new ancestor
size, recomputes Taffy, re-writes `ResolvedLayout`, and re-evaluates container queries
inline — so a `Cqw`-sized intermediate `B` and a rule-bearing descendant `C` both
catch up the **same frame** `A` resizes. Capped at one re-run per frame (the
`CqDescendantReRunRequested` flag is cleared at the top, mirroring `cq_flip_rerun`'s
`CqReRunRequested` discipline), so the 2×-Taffy ceiling holds and a deeper
`A`→`B`→`C`→`D` chain settles one further level per frame. The negative regression
test `cq_transitive_cascade_is_one_frame_stale` was flipped to the positive
`cq_transitive_cascade_catches_up_in_frame`.

**Symptom:** when an ancestor `A`'s `ResolvedLayout` changes (e.g., a
parent container is resized) and a `Cqw`-sized intermediate `B` sits
between `A` and a rule-bearing descendant `C`, the intermediate is not
re-translated and the descendant's `ContainerQuery` never re-evaluates.
`B`'s Taffy width stays at the previously-baked `Cqw` resolution.

**Cause:** `sync_styles`'s `Or<(Changed<T>, ...)>` trigger filter is
per-entity. Bevy provides no "ancestor's `T` changed" filter primitive.
When `A.ResolvedLayout` changes, `A`'s own `Changed<ResolvedLayout>` bit
fires (and Phase 5 added that to the filter so initial-frame cascades
work), but `B` and `C` see no `Changed<*>` bits because their own
components are unchanged. They are skipped by `sync_styles` and
therefore by Buiy's container-unit resolution.

**Why Phase 5 didn't fix it:** the fix requires either (a) a separate
descendant-invalidation pass that walks down the hierarchy from each
container with a freshly-changed `ResolvedLayout` and force-ticks
sync_styles' filter on descendants, or (b) a fundamentally different
filter primitive (e.g., a `DescendantOf<Changed<T>>` query filter Bevy
doesn't ship). Either path expands Phase 5's chartered scope. Phase 5
ships the direct-ancestor cascade (handled by `cq_flip_check` reading
`tree.layout()` + `cq_flip_rerun`) and documents the multi-level gap.

**Current test that documents the gap:**
`crates/buiy_core/tests/layout_container_queries.rs::cq_transitive_cascade_is_one_frame_stale`
— a *negative assertion* asserting C stays Inactive after A's resize.
When this follow-up lands, the assertion polarity flips to positive
(C becomes Active in-frame).

**Implementation sketch (for the future phase):**
1. After `write_resolved_layout` (step 7) computes which entities'
   `ResolvedLayout` changed this frame, identify the subset that are
   query containers (have `Container { container_type != Normal }`).
2. For each such container, walk its descendants and mark them for
   re-translation. Mechanism options:
   - Insert/touch a private `ContainerSizeDirty` marker component that
     `sync_styles`' Or-filter picks up.
   - Or: build a `HashSet<Entity>` of "dirty descendants" resource that
     `sync_styles` checks alongside `Changed<>`.
3. Trigger a same-frame re-run analogous to `cq_flip_rerun` so the
   descendants re-resolve their `Cqw` against the new ancestor sizes
   within the same frame.

**Cost:** the descendant walk is O(N) per resized container per frame.
In a worst-case theme switch this is O(tree size), but the same
worst-case already exists for Phase 4 writing-mode propagation.

**Spec touchpoint:** `docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md`
§ 1.3 and § 1.5. Spec § 1.3 step 4-5 implies geometric cascade should
catch up in-frame for the *direct* ancestor case (Phase 5 does this).
The multi-level case isn't explicitly addressed by the spec; this
follow-up makes Phase 5's behavior match the most natural reading of
§ 1.5's "fixture A→B→C ... assert frame N applies A's activation,
frame N+1 applies B's" — currently the assertion would never fire
because B never updates.


## Anchor positioning — `anchor-size()` term

**Originated:** Phase 6 (anchor positioning), spec § 3.4 line 231
("tier-C feature deferred to v1.x").

**Symptom:** authors using `anchor-size()` in a `PositionTry::inset` term
get a `0.0` resolution and a per-frame warn via
`AnchorErrorKind::AnchorSizeUsed`. The variant is shipped in Phase 6 as
a forward-compatibility hook; no `Length::AnchorSize` term exists yet to
trigger it.

**Cause:** Phase 6 ships only static (`Px`, `Percent`) and container
units (`Cqw`, `Cqh`, etc.) for inset resolution. The CSS spec's
`anchor-size()` function references the anchor target's resolved size as
a length, which requires plumbing the *anchor target's box* into
`length_inset_to_px`. The plumbing exists (the anchor box is already
known at try-evaluation time) but the term type is missing from
`Length`.

**Implementation sketch:**
1. Add `Length::AnchorSize(AnchorRef, AxisDimension)` to `layout::types`.
2. Extend `length_inset_to_px` (or a wrapper for anchor contexts) to
   take the anchor's box and resolve the new variant.
3. Remove the stub warn variant or repurpose it for the residual error
   case (anchor box still resolving to zero size).

## Anchor positioning — `position_try_max_depth` resource cap

**Originated:** Phase 6 + README § 5 open question.

**Symptom:** an anchor with a deeply-nested `position_try` chain (e.g.,
20+ fallbacks) is evaluated linearly. No cap is enforced. If profiling
surfaces this as a hot path, a resource-configurable cap should bound
the search.

**Implementation sketch:** add `PositionTryMaxDepth(pub usize)` resource
defaulting to `usize::MAX`. In `anchor_resolution`, iterate
`anchor.position_try.iter().take(cap.0)` and warn-once if the chain
exceeds the cap (a new `AnchorErrorKind::ChainDepthCapped` variant).

## Anchor positioning — cross-window targets

**Originated:** Phase 6 (spec is silent on cross-window anchors).

**Symptom:** an `AnchorRef::Entity(e)` pointing at an entity in a
different window's `LayoutTree` resolves as `TargetMissing` because
`tree.by_entity.get(&e)` fails (separate `LayoutTree` per window).

**Cause:** Phase 6 has a single `LayoutTree` resource keyed by Entity
across all windows. Cross-window resolution would require either
per-window `LayoutTree` resources with a router that knows which tree
to query, or a global `LayoutTree` with window-tagged entries.

**Implementation sketch:** depends on `buiy-window-and-surface-design`
(currently incomplete). Out of Phase 6 scope.

## Anchor positioning — anchor target IS sticky/table/multicol (Phase 7 interaction)

**Originated:** Phase 6 (architectural quirk surfaced during plan
review).

**Symptom:** when Phase 7 lands sticky-offset (6a), table-layout (6b),
and multicol-pack (6c) sub-passes that correct `ResolvedLayout`
positions for affected entities, `anchor_resolution` (6d) reads from
`tree.tree.layout()` — Taffy's *pre-correction* position. An anchor
target that is itself sticky-displaced or table-repositioned will have
its anchored elements computed against the un-corrected position.

**Cause:** `anchor_resolution` runs AFTER 6a-6c in pipeline order, but
6a-6c write to `ResolvedLayout` (which is itself written by step 7,
after 6d). The corrections are invisible to 6d.

**Implementation sketch (two options):**
1. **Move 6a-6c into a shared per-entity correction buffer** that 6d
   reads. E.g., a `PostTaffyPositionOverrides` resource that all four
   sub-passes contribute to and that step 7 consumes (replacing the
   Phase 6 `AnchorOverrides`-only model).
2. **Reorder**: run anchor_resolution AFTER 6a-6c — but the spec
   declares the 6a → 6b → 6c → 6d order explicitly.

Option 1 preserves spec order and is the more flexible long-term
shape.

## Anchor positioning — `AnchorRef::Entity` integration test gap

**Originated:** Phase 6 (final whole-branch review).

**Symptom:** all 11 Phase 6 integration tests use `AnchorRef::Name`. The
`AnchorRef::Entity` direct-reference path has no end-to-end coverage.
The unit tests in `kahn_anchor_sort` and `handle_anchor_insert` exercise
it indirectly, but a regression in the direct-reference path inside
`anchor_resolution`'s edge-building closure could go undetected.

**Implementation sketch:** add one fixture that spawns an entity, gets
its `Entity` handle, and uses `AnchorRef::Entity(e)` as the anchor
reference (no name registry involved). Assert the anchored entity's
position tracks the target.

## Anchor positioning — extract `anchor_resolution` into sub-helpers

**Originated:** Phase 6 (final whole-branch review).

**Symptom:** `anchor_resolution` spans ~252 lines (`systems.rs` lines
497-749) following 7 numbered algorithm steps. Pure refactor candidate
when Phase 7 extends the sub-pass set.

**Implementation sketch:** split into `build_anchor_edge_map`,
`apply_anchor_broken_markers`, `emit_anchor_warns`. No behavior change;
makes future extension cleaner.

## Layout — `Position::Fixed` implementation — LANDED

**Originated:** Phase 7 (D13 — explicit deferral).

**Status:** **Landed** in Phase 10
(`docs/plans/2026-05-29-buiy-layout-position-fixed.md`). `PositionKind::Fixed`
now resolves against the **layout root**: its Taffy node is re-parented onto the
root's child list in `sync_children_for_entity` (excluded from its in-flow
parent's list, appended to the root's), so Taffy's native absolute algorithm
resolves it — including percentage insets — against the root's content box. A
pure `is_fixed_root(&Position)` predicate (not a stored flag) decides
re-parenting; `map_position_kind` already emitted `taffy::Position::Absolute`
for `Fixed`, so no emission change was needed and there was no warn-once to
remove. A `.fixed()` convenience setter was added to `Style`. Transformed-
ancestor-as-containing-block and per-window / multi-root `Fixed` targeting
remain deferred (single global root; gated on `buiy-window-and-surface-design`).

**Spec touchpoint:** `display-and-positioning.md § 2.1` (Fixed row + Known gap),
`§ 2.2` (Taffy mapping for Fixed).

## Layout — full table layout algorithm — LANDED

**Originated:** Phase 7 (Task 6 stub).

**Status:** **Landed** in Phase 12
(`docs/plans/2026-05-29-buiy-layout-table-layout.md`). The `table_layout` stub
(sub-pass 6b) is replaced with the real CSS table algorithm: gather
`Display::Table*` entities by family into a `TableModel`, resolve per-column
widths via a throwaway synthetic Taffy flex tree per table (`resolve_column_widths`),
place every cell / row / row-group into the column grid in document order
(`place_table_cells`), and write container-origin-relative corrected positions
straight into `PostTaffyPositionOverrides` (sizes stay from Taffy — position-only
overlay, like sticky 6a). Bare rows form an implicit anonymous row-group (CSS
fixup). The blanket `TableUnsupported` warn is retired (kept `Reflect`-stable
like `MulticolUnsupported`). Still deferred (tier-C corners): `colspan`/`rowspan`
(no API surface — ragged rows laid out positionally + `TableSpanUnsupported(Entity)`
warn), `Display::TableCaption`/`TableColumn`/`TableColumnGroup` geometry
(classified but no placement + `TableSubfeatureUnsupported(Entity)` warn),
header/footer float reorder (document-order stacking only), and per-cell
stretch-to-column-width / `border-collapse` / border-spacing.

**Spec touchpoint:** `display-and-positioning.md § 1.2`.

## Layout — full multi-column layout algorithm — LANDED

**Originated:** Phase 7 (Task 7 stub).

**Status:** **Landed** in Phase 13
(`docs/plans/2026-05-29-buiy-layout-multicol-layout.md`). The `multicol_pack`
stub (sub-pass 6c) is replaced with a real packing pass: a pure CSS Multicol L1
§ 7.3 used-value `resolve_column_count` resolver (count-only / width-only / both
/ neither, with `column-count` treated as a maximum), a pure greedy whole-child
`pack_columns` packer that fills columns top-to-bottom and honors forced
`break-before` / `break-after` (`Column` / `Always`) at the child boundary, and
container-content-relative offsets written straight into
`PostTaffyPositionOverrides`. The blanket `MulticolUnsupported` warn is retired
(kept `Reflect`-stable like `TableUnsupported`). True content fragmentation
(splitting one box across a column boundary) and break-*avoidance* remain
deferred (tier-E) behind a session-wide `MulticolFragmentationDeferred`
warn-once; `column_width` / `column_gap` resolve `Px` only (percent / cq column
metrics deferred).

**Spec touchpoint:** `flex-and-grid.md § 3` (multi-column).

## Layout — sticky `Length::Cq*` inset resolution

**Originated:** Phase 7 (D3 deferral).

**Symptom:** Sticky entity with `Length::Cqw/Cqh/Cqi/Cqb/Cqmin/Cqmax`
inset emits `LayoutWarnOnceKey::StickyCqDeferred(Entity)` and resolves to
0.0.

**Implementation sketch:** port Phase 6's `length_inset_to_px` cq-context
resolver. Sticky's reference frame is the sticky entity's own nearest CQ
ancestor (distinct from anchor's "anchor target box" frame). Multi-axis
fixture needed (Cqi/Cqb resolve against writing-mode inline/block axes).

**Spec touchpoint:** `display-and-positioning.md § 2.3`,
`container-queries-and-writing-modes.md § 1`.

## Layout — sticky em/rem/Vh/Vw/Vmin/Vmax inset support

**Originated:** Phase 7 (D3 — these `Length` variants don't exist yet).

**Symptom:** Authors cannot use em/rem/V*-typed insets on sticky elements
because `Length::Em / Rem / Vh / Vw / Vmin / Vmax` are not (yet) variants
of `Length`.

**Implementation sketch:** when Phase 10 (or a font-rendering phase) adds
these `Length` variants, extend `resolve_sticky_inset` with new arms
(currently a closed match so the compiler will force the change).

**Spec touchpoint:** Phase 10 — viewport units; future font-rendering
spec — em/rem.

## Layout — sticky both-top-and-bottom dual clamp

**Originated:** Phase 7 (D4 — v1 "top wins" deviation).

**Symptom:** Sticky element with both `inset_top` and `inset_bottom` set
ignores the bottom inset (top wins). CSS spec § 6.3 implies dual-clamp
behavior where the element sticks to whichever edge the scroll position
is closer to.

**Implementation sketch:** implement dual-clamp in
`compute_sticky_displacement` — likely requires storing both upper and
lower sticky thresholds and computing midpoint logic. The v2 test
`sticky_both_top_and_bottom_inset_top_wins` (in `tests/layout_sticky.rs`)
is the regression test for the v1 "top wins" behavior — flipping it
documents the algorithm upgrade.

**Spec touchpoint:** CSS spec § 6.3 (positioned layout).

## Layout — sticky inside sticky

**Originated:** Phase 7 (documented v1 limitation).

**Symptom:** When entity A is sticky-displaced and entity B is a sticky
child of A, B's `world_position` walks Taffy positions (un-displaced);
B's threshold computation uses A's *natural* position, not displaced.
Rare authoring case.

**Implementation sketch:** consult `PostTaffyPositionOverrides`
(just-written by 6a) when walking the ancestor chain in
`world_position`, so inner sticky sees displaced outer. Requires careful
ordering (inner sticky must run after outer; topological pre-pass or
two-frame eventual-consistency are both options).

**Spec touchpoint:** `display-and-positioning.md § 2.3` (does not
explicitly address nested-sticky).

## Layout — `clear_warned_once_on_exit` lifecycle wire-up

**Originated:** Phase 7 (D7 — `BuiyState` / `BuiyExit` lifecycle states
don't exist in `buiy_core` yet).

**Symptom:** `clear_warned_once_on_exit` system exists but is
`#[allow(dead_code)]`. Repeat `App::new()` cycles within a process
accumulate state across instances (won't matter in production where the
binary exits; matters in tests / hot-reload).

**Implementation sketch:** once foundation lifecycle states are settled,
wire the clear via
`app.add_systems(OnExit(BuiyState::Active), clear_warned_once_on_exit)`.
Until then, the function is exposed but never called — tests can invoke
directly via `world.run_system_once(clear_warned_once_on_exit)`.

**Spec touchpoint:** `architecture.md § 6`.

## Layout / render — Bevy `Transform` ownership bridge (`GlobalTransform` write) — LANDED

**Originated:** Phase 8 (D2 — deliberate divergence from spec § 2
approach (a) at the implementation-timing level).

**Status:** **Landed** in render-pipeline Phase R3
(`docs/plans/2026-06-03-buiy-render-r3-transform-bridge.md`). The new
render-prep module `crates/buiy_core/src/render/bridge.rs` adds
`write_buiy_transform` — the SOLE writer of each laid-out entity's Bevy
`Transform`. It is a top-down `Children` walk seeded by a `ScrollDirty`
resource (the union of `Changed<ResolvedLayout>`,
`Changed<ResolvedTransform>`, and `Changed<ScrollOffset>` on a
scroll-container) that, per entity, composes
`base = from_translation(ResolvedLayout.position − accumulated_ancestor_scroll)`
then `base * ResolvedTransform.matrix` into one `Transform` (change-gated;
inserting it pulls in the `GlobalTransform` + `TransformTreeChanged`
companions). `CorePlugin` then chains a DISTINCT `Update` copy of Bevy's
`mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms`
after the writer and `.before(BuiySet::Picking)`, so `GlobalTransform` is
final before picking + extract (no new `BuiySet` variant — the bridge slots
into the existing chain). `extract_buiy_draws` now reads
`GlobalTransform.translation()` for position (pillar 5), keeping
`ResolvedLayout` only for size. The bridge stays in logical-px, y-down,
window-relative space — the y-flip + logical→physical scale live in the GPU
view uniform, never the bridge (§ B.4). Perspective / `transform-style:
Preserve3d` stays C-tier deferred (`Transform::from_matrix` decomposes to
TRS and drops the projective row — pinned by a CPU test); `backface_visibility`
is consumed only as a per-primitive render flag read off `UiTransform` (no
new component). Apps must supply `TransformPlugin` (`DefaultPlugins` does;
`MinimalPlugins` needs it added) for the canonical `PostUpdate` pass +
reflection registration.

**Spec touchpoint:** `transforms-and-containment.md § 2`;
`docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § B`.

## Layout — `content-visibility: auto` off-screen skip — LANDED

**Originated:** Phase 8 (D6 — stored, not enforced).

**Status:** **Landed** in Phase 11
(`docs/plans/2026-05-29-buiy-layout-content-visibility.md`). The spec § 5.2
step-1 skip is now real: `sync_styles` classifies every entity via a pure
`content_visibility_skip(...)` helper, and a `ContentVisibility::Auto` entity
that is off-screen **and** carries a `ContainIntrinsicSize` hint gets the Taffy
skip — its own Taffy size is overridden with the intrinsic-size sentinel
(`StyleView.content_visibility_intrinsic`) and its descendants are detached from
the Taffy child list (reusing the children-sync exclusion-set mechanism;
descendant nodes are kept alive for a cheap `set_children` snap-back). Off-screen
is computed from the *last-frame* `ResolvedLayout` border box vs the primary-window
viewport expanded by a `ContentVisibilityMargin` (default 200px) — a single
symmetric expanded rect, so the margin doubles as a stateless hysteresis dead-band
that stops edge thrash. The blanket "content-visibility deferred" warn is gone; the
`ContentVisibilityDeferred(Entity)` warn-once is repurposed to fire only for the
residual degenerate case (Auto + off-screen + **no** intrinsic-size hint, where the
requested skip cannot run). The skip is mirrored identically in `cq_flip_rerun` so a
container-query flip frame does not transiently re-lay-out the skipped subtree (D8).
Auto's off-screen *paint* skip without a hint remains a render concern Phase 11 does
not own, and the Blink `contain-intrinsic-size: auto` "remembered size" auto-sizing
of the placeholder remains deferred (v1 gates the skip on an explicit hint).

**Spec touchpoint:** `transforms-and-containment.md § 5.2`.

## Layout — `content-visibility: hidden` descendant skip — LANDED

**Originated:** Phase 8 (D6 — stored, not enforced).

**Status:** **Landed** in Phase 11
(`docs/plans/2026-05-29-buiy-layout-content-visibility.md`). A
`ContentVisibility::Hidden` entity now prunes its descendants from the Taffy tree
(`content_visibility_skip` → `HiddenPrune`, added to the same per-frame
`skip_children` set as the Auto sentinel), so Taffy never lays the subtree out —
geometry-independent (no off-screen check, no intrinsic-size hint needed). Per spec
§ 5 / § 5.2 and CSS, only the *descendants* are skipped: the Hidden entity itself
still lays out and resolves its own box (it is not `Display::None` on self, D7).
Snap-back on toggle is a cheap `set_children` re-attach since the descendant Taffy
nodes are kept alive. Hidden never warns (fully implemented). Mirrored in
`cq_flip_rerun` (D8).

**Spec touchpoint:** `transforms-and-containment.md § 5.2`.

## Layout / render — `will-change` layer promotion + SC trigger

**Originated:** Phase 8 (D7 — tier-E, stored-only).

**Symptom:** `WillChange` is stored on `Containment` but no layer
promotion or stacking-context trigger behavior is produced.

**Implementation sketch:** honor `WillChange::Properties` as a render
layer-promotion hint and a stacking-context trigger when the list mentions
an SC-forming property (`WillChangeProperty::Transform` etc.) — coordinates
with Phase 9 stacking.

**Spec touchpoint:** `transforms-and-containment.md § 5.3`.

## Render — `UiTransform` paint + `Containment` PAINT clip rect + perspective / backface

**Originated:** Phase 8 (spec § 4 — render-side concerns stored only).

**Symptom:** `perspective`, `TransformStyle::Preserve3d`, and
`BackfaceVisibility::Hidden` are stored on `UiTransform` and the
`LAYOUT` / `PAINT` / `STYLE` contain flags are stored on `Containment`,
but render does not yet consume them.

**Implementation sketch:** render consumes `ResolvedTransform` + the
containment flags — applies the composed matrix, the PAINT clip rect, and
honors perspective / backface / `transform-style`.

**Spec touchpoint:** `transforms-and-containment.md § 4`, § 5.1.

## Render — R11 forced-colors cross-phase seams (CatalogPaint + BoxShadow draw-skip)

**Originated:** R11 (color / forced-colors / verify). R11's gate-#11 analyzers run
over a `CatalogPaint` descriptor seam, and the forced-colors `BoxShadow` draw-skip
has no live producer yet.

**Symptom / what's deferred:**
1. `forced_colors_analyzer::{analyze_forced_colors, analyze_shadow_only}` analyze
   `CatalogPaint` descriptors that tests construct by hand — there is no live
   widget-catalog source. When the `Background`/`Border`/`Outline`/`BoxShadow`
   component-model phase lands real painted components, the analyzer seam must be
   re-pointed at them (the gate then runs over the actual catalog, not fixtures).
2. color-and-forced-colors.md § 3.3: in forced-colors mode, extract must read
   `UserPreferences.forced_colors` and SKIP the `BoxShadow` batch (shadows are
   decorative, suppressed under forced colors). `extract_buiy_nodes` has no such
   branch — it lands when `BoxShadow` gets a real extract/draw path (the BoxShadow
   primitive pipeline is itself a later-tier seam, R7 bucket-reserved only).

**Spec touchpoint:** `color-and-forced-colors.md § 3.3`; the gate-#11 section.

## Render — effect-compositor GPU orchestration (R9 prepare body + composite draws) — LANDED

**Status:** **Landed** in the GPU-verify campaign (commit `c0a5fe0`,
`docs/specs/2026-06-03-buiy-render-pipeline-design/2026-06-08-render-effect-compositor-gpu-design.md`):
extract carries per-group membership (`EffectGroupExtract` + `ExtractedNode.group`),
`pack_view_partitioned` partitions contiguous per-group instance ranges,
`prepare_effect_groups` is fully implemented (budget → pooled `Rgba16Float`
targets → `PreparedEffectGroups`/`PreparedEffectTargets` on the view entity), and
`BuiyNode::run` runs the step-1 group passes + step-2 composites; the flat draw
excludes group ranges. The contiguity invariant now holds **by construction**
since the trigger-5 opacity/filter/blend stacking-context formers landed (see the
LANDED entry below); GPU regressions: `render_compositor_gpu.rs` +
`render_group_contiguity_gpu.rs`. The original deferral text follows.

**Originated:** R9 (effect compositor). R9 landed the full compositor MATH as
headless-tested pure fns (`painted_bounds`, `bucket_extent`, `post_order_indices`,
`plan_allocation`/`rt_pool_budget` degradation, `group_target_descriptor`,
`composite_src_over`) + the structural seams (`compositor::register`,
`PreparedEffectGroups` per-view component, the `BuiyNode::run` step-1/step-2
composite loops) — but `prepare_effect_groups` has an EMPTY body and the node
composite loops are inert (`prepared.groups` always empty in v1).

**Symptom:** the working compositor needs an extract→prepare data flow that does
not exist: R5's `extract_buiy_nodes` emits a FLAT node list with no effect-group
membership/tree/bounds carried to the render world, so `prepare_effect_groups`
has nothing to read. The GPU body (acquire pooled `Rgba16Float` TextureCache
targets, rasterize each group subtree, composite bottom-up) also needs a wgpu
adapter to write+verify, which this host/CI lack.

**Implementation sketch:** (1) extend extract to carry per-view effect-group
structure (which extracted nodes belong to which `EffectGroup`, the group tree,
and per-group instance ranges) into the render world; (2) fill
`prepare_effect_groups`: compose `painted_bounds → bucket_extent →
group_target_descriptor`, `post_order_indices`, `plan_allocation`, acquire pooled
targets, write `PreparedEffectGroups`; (3) wire the `BuiyNode::run` step-1/step-2
loops to rasterize + composite, and make the flat draw EXCLUDE group-member
instance ranges (TODO already in `node.rs` — else double-paint). GPU `#[ignore]`
goldens (group-opacity correctness, RSS return-to-baseline) run under an adapter.

**Spec touchpoint:** `effect-compositor.md § 1.1 / § 2 / § 3`; architecture.md § 1.4 / § 4.

## Render — node-draw model: per-entity clip + composite passes (R8 Task 8 / R9 blocker)

**Originated:** R8 (paint/clip/toplayer). R8 landed the pure consumer helpers
(`scissor_rect`, `clip_for_primitive`, `partition_top_layer`) but **not** the GPU
consumer (per-entity scissored draw + top-layer composite in `BuiyNode::run`).

**Symptom:** R6's `BuiyNode::run` is a single `draw(0..4, 0..quad_count)` against one
persistent buffer; it cannot express per-entity rectangular clip, the top-layer
composite pass, or (R9) effect-group off-screen targets — all while preserving
`painters_z` paint order. R8's plan assumed a "scissor side-table" but did not
reconcile it with the single-buffer draw, so Task 8 stopped.

**Implementation sketch:** decide the node-draw model (recommended **hybrid**:
per-instance fragment-discard clip via `clip_for_primitive` threaded into
`PackedInstance` + the reserved multi-pass node — normal pass → top-layer composite
pass driven by `partition_top_layer` → R9 effect-group passes). Changes R6's
instance layout (+ WGSL) and the node pass structure, so it needs ratifying before
R8 Task 8 / R9 implement against it.

**Spec touchpoint:** design note
`docs/specs/2026-06-03-buiy-render-pipeline-design/2026-06-06-render-node-draw-model-design.md`;
`architecture.md § 2`, `paint-order-and-top-layer.md § 3/§ 4`, `effect-compositor.md`.

## Render — subtree visibility suppression (`CssVisibility::Hidden` / `OffscreenAuto` descendants) — LANDED

**Originated:** R5 (per-view extract) — `node_skip_reason` was a per-entity leaf
predicate; the spec requires a subtree-scoped paint skip.

**Status:** **Landed** 2026-06-09 (Option A of the design note, ratified). The
`write_paint_skip` render-prep pass (`crates/buiy_core/src/render/visibility.rs`,
`.after(Animate).before(Picking)` alongside the clip/effect passes) is a
seed-gated top-down `Children` walk that writes the computed
`ComputedPaintSkip { reason: SkipReason }` marker (render/components.rs, lean
derives, not registered) onto each `CssVisibility::Hidden` / `OffscreenAuto`
root AND every descendant, removing it when an entity leaves the suppressed
subtree (an entity's own reason wins over the inherited one). Extract now reads
the marker as its SINGLE skip source — `CssVisibility`/`OffscreenAuto` left the
fan and probe; `node_skip_reason` moved producer-side — and its damage gate
hears both transitions: `Changed<ComputedPaintSkip>` in the `Or<…>` probe (hide)
plus a `RemovedComponents<ComputedPaintSkip>` stream (show, beside the despawn
stream). Steady state is O(0): the walk early-returns without seeds, and the
shared `reconcile_one` change-gate keeps re-walks op-free. v1 semantics: blanket
subtree drop (no `visibility:visible` override until a visibility cascade
exists). Coverage: `tests/render_paint_skip.rs` (headless subtree/flip/
reparent/steady-state + one `#[ignore]` GPU smoke: hidden subtree packs 0 quads
→ show packs 2 → re-hide packs 0).

**Spec touchpoint:** `paint-order-and-top-layer.md § 5.3 / § 5.4` (status notes
updated); `architecture.md § 1.2/§ 3.1` (fan + trigger union updated);
`component-model.md` (computed-component set);
`docs/specs/2026-06-03-buiy-render-pipeline-design/2026-06-06-render-subtree-visibility-suppression-design.md`
(now implemented, "As landed" section).

## Layout — Phase 9 stacking sub-pass 6f reads `ResolvedTransform` — LANDED

**Originated:** Phase 8 (D1 — stacking deferred to Phase 9).

**Status:** **Landed** in Phase 9
(`docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md`). Sub-pass 6f
(`stacking_context`) now runs after `transform_composition` (6e) and reads
the composed `ResolvedTransform` (trigger 3). It implements the spec § 2 SC
trigger union (positioned + z-index, isolation, transform, paint/strict
containment, root), the § 2.1 five-tier z-index paint-order sort, and the
§ 4 top-layer escape. The render-side trigger-5 formers have since landed
(next entry); the will-change SC trigger remains deferred (separate
follow-up below).

**Spec touchpoint:** `transforms-and-containment.md § 3`, § 6;
`stacking-and-top-layer.md`.

## Layout — Phase 9 render-side stacking-context formers (`opacity` / `filter` / `mix_blend_mode`) — LANDED

**Originated:** Phase 9 (D1 — trigger-5 formers deferred).

**Status:** **Landed** in the render-followups campaign (2026-06-09). The
render components (`Opacity`, `Filter`, `MixBlendMode`) have existed since
render R1 in the SAME crate, so 6f reads them with a plain import — the
"cross-layer seam" the deferral worried about was conceptual only.
`forms_stacking_context` (layout/systems.rs) gained the spec § 2 trigger-5
clause: `Opacity < 1.0`, non-empty `Filter`, or `MixBlendMode != Normal`
forms a `StackingContext`. The clause delegates to
`render::effect::forms_render_stacking_context`, which derives from the
canonical effect-group former predicate (`effect_reason_for`) — ONE source
of truth for the shared terms, so the SC trigger and the group former can
never drift apart. `will-change` stays deferred (separate follow-up below);
`BackdropFilter` deliberately forms an `EffectGroup` but never an SC
(render component-model.md § 8). Unit tests sit beside the other trigger
tests (layout/systems.rs); end-to-end coverage in tests/layout_stacking.rs.

**Spec touchpoint:** `stacking-and-top-layer.md § 2` trigger 5, § 7.

## Layout — Phase 9 `will-change` stacking-context former

**Originated:** Phase 9 (D1) — coordinates with the Phase-8 "will-change
layer promotion + SC trigger" follow-up (above).

**Symptom:** a `WillChange` value naming an SC-forming property (e.g.
`WillChangeProperty::Transform`, `Opacity`) should form a stacking context,
but sub-pass 6f does not treat `will-change` as a trigger. `WillChange` is
Phase-8 tier-E, stored-only with no behavior.

**Cause:** Phase 8 stores `WillChange` on `Containment` but ships no behavior
(D7); Phase 9 deliberately did not wire it as an SC trigger to keep the
deferral consistent. The two concerns (render layer promotion + SC trigger)
are the same underlying feature and should land together.

**Implementation sketch:** when honoring `WillChange`, extend
`forms_stacking_context` to return `true` when the `Containment.will_change`
list names an SC-forming property, in the same change that adds the render
layer-promotion hint. Cross-links the existing Phase-8 "will-change layer
promotion + SC trigger" follow-up.

**Spec touchpoint:** `transforms-and-containment.md § 5.3`;
`stacking-and-top-layer.md § 2` trigger 5, § 7.

## Layout — Phase 9 per-window top layer

**Originated:** Phase 9 (D2 — deliberate divergence from spec § 4.4 at the
implementation-scope level).

**Symptom:** Phase 9 ships a **single** global top layer + one global
`TopLayerActivation`. Spec § 4.4 wants a per-window top layer; top-layer
entities in distinct windows would currently share one activation order and
all escape to the same single root context.

**Cause:** `buiy_core` has one global `NonSend<LayoutTree>` and reads the
primary window only (`taffy_compute` uses `windows.iter().next()`); there is
no per-window layout segregation anywhere. A per-window top layer would
require per-window `LayoutTree`s (or window-tagged entries) plus a router that
knows which window root an escaped entity attaches to.

**Implementation sketch:** depends on `buiy-window-and-surface-design`
(currently incomplete), mirroring the Phase-6 "cross-window anchor targets"
follow-up. Once per-window layout exists, key `TopLayerActivation` by window
(or hold one per window) and attach escaped top-layer entities to their own
window's root context instead of `roots.first()`.

**Spec touchpoint:** `stacking-and-top-layer.md § 4.4`, § 7.

## Layout — non-px translate units in `compose_transform`

**Originated:** Phase 8 (CHANGELOG deferral note).

**Symptom:** `compose_transform` resolves only `Length::Px` for translate;
percent / `Cq*` translate contributes `0.0`.

**Implementation sketch:** resolve percent / `Cq*` translate against the
entity's own resolved box (currently `0.0`); coordinate with the animation
phase.

**Spec touchpoint:** `transforms-and-containment.md § 1`, § 1.1.

## Render — effect-compositor depends on the opacity stacking-context trigger (contiguity) — LANDED

**Originated:** render-pipeline GPU campaign item 5 (effect-compositor GPU
orchestration), fresh-review finding.

**Status:** **Landed** with the Phase-9 render-side SC formers (entry above,
2026-06-09). An `Opacity < 1` / non-empty `Filter` / non-Normal
`MixBlendMode` former now forms a `StackingContext`, so a group's subtree is
ONE atomic entry in its parent's `painters_z` — a non-member sibling can no
longer interleave between group members, and `pack_view_partitioned`'s
single-range-per-group partition (`render/buckets.rs`) holds **by
construction**. The `debug_assert_eq!` stays as a tripwire for the two
residual ways it could break: predicate drift (prevented structurally — the
SC trigger derives from `effect_reason_for`) and a `backdrop-filter`-ONLY
group (deliberately EffectGroup-but-not-SC; reserved, no v1 shader). Group
**membership** derivation is unchanged (`EffectGroup` + `ChildOf`
nearest-former climb — SC-agnostic); the SC's contribution is paint-order
atomicity. GPU regression for the previously-impossible latent case (a
z-indexed group member + an interleaving-tier non-member sibling renders
without double-paint or assert trip): tests/render_group_contiguity_gpu.rs.

**Spec touchpoint:** `stacking-and-top-layer.md § 2` trigger 5;
`effect-compositor.md § 3`; `2026-06-08-render-effect-compositor-gpu-design.md`
(fork 5 deviation + follow-up note).

## Render — glyphs bypass effect-group compositing (text-seam follow-up) — LANDED

**Status:** **Landed** by text campaign T8
([2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md](2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md)):
the glyph buffer is partitioned into flat/group ranges exactly like the quad
path — `partition_glyph_ranges` over the producer's per-entity `entity_runs`
(`ExtractedGlyphs` attribution, zero new producer params), with group
membership derived from the FRESH node list at prepare (the
decoration-and-paint § 4.6 discipline; never recorded into the carrier). The
step-1 group pass draws each group's glyph range into its `Rgba16Float`
target via the `Glyph@Rgba16Float` pipeline specialization (after its quads,
atlas `@group(1)` bound), and the flat glyph draw covers the complement —
text inside an `Opacity(0.5)` card dims exactly once. The `TODO(text-seam)`
block at the glyph draw is deleted. GPU regressions:
`tests/text_effect_group_gpu.rs` (partition wiring + the composite golden) +
the flipped `text_decoration_gpu.rs` asymmetry test
(`opacity_group_dims_underline_line_through_and_ink`). The original deferral
text follows.

**Originated:** render-pipeline GPU campaign item 5, fresh-review finding.

**Symptom:** the glyph draw in `BuiyNode::run` paints into the flat window pass
with no group mechanism. Latent today (`glyph_count == 0` — no v1 text producer),
but once the text seam lands, a glyph inside an `EffectGroup` subtree would render
at full opacity straight to the window, bypassing the group's off-screen target +
the opacity composite (text in an `Opacity(0.5)` card would not dim).

**Implementation sketch:** when the text seam connects, partition the glyph buffer
into flat/group ranges exactly like the quad path (`pack_view_partitioned`), and
draw a group's glyph instances into its `Rgba16Float` target in the step-1 group
pass via a `Glyph@Rgba16Float` pipeline specialization (mirroring the
`Quad@Rgba16Float` group pipeline). Marked `TODO(text-seam)` at the glyph draw.

**Spec touchpoint:** `effect-compositor.md § 3 step 1`; `atlas-and-text-seam.md`.

## Render — degraded effect groups vanish instead of drawing flat

**Originated:** text campaign T8 implementation reading (the T8 plan's D9).

**Symptom:** a `plan_allocation == false` group gets no pooled target,
`BuiyNode::run` step 1 `continue`s, and its members are excluded from
`flat_ranges` / `glyph_flat_ranges` — so under RT-pool budget pressure a
degraded group's quads AND glyphs paint nowhere, despite the "drawn flat
instead" comments (node.rs step 1; compositor.rs `PreparedEffectTargets`).
Latent under the 64 MiB budget (no fixture degrades today). T8 mirrored the
quad semantics for glyphs (a degraded group's glyph range is likewise
skipped) rather than silently widening scope.

**Implementation sketch:** either re-route a degraded group's ranges into the
flat draw at prepare (forward compositing, accepting the double-dim
approximation v1 rejected for targets) or document skip-as-degradation;
decide with `buiy-verification-design`'s budget calibration.

**Spec touchpoint:** `effect-compositor.md § 2.3`.
