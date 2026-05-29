# Layout follow-ups

**Date:** 2026-05-21
**Status:** active

Tracked-but-not-yet-scheduled work surfaced by completed layout phases. Each
entry names the originating phase, the divergence from spec / open behavior,
and a sketch of the implementation direction. When a follow-up gets
chartered into its own phase or plan, move the entry to that plan and link
back here.

## Descendant invalidation on ancestor-resolved-size changes

**Originated:** Phase 5 (container queries), Task 10 implementer finding +
reviewer mandate. Plan `docs/plans/2026-05-21-buiy-layout-container-queries.md`
v3 revision documents the gap.

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

## Layout / render — Bevy `Transform` ownership bridge (`GlobalTransform` write)

**Originated:** Phase 8 (D2 — deliberate divergence from spec § 2
approach (a) at the implementation-timing level).

**Symptom:** Phase 8 produces the `ResolvedTransform { matrix: Mat4 }`
artifact via sub-pass 6e but does NOT write Bevy `Transform` /
`GlobalTransform`. Render reads `ResolvedLayout` directly and `buiy_core`
has no `TransformPlugin` wiring (the layout harness uses `MinimalPlugins`),
so a `Transform` write today would be dead code that nothing consumes.

**Implementation sketch:** implement spec § 2 approach (a) —
`write_resolved_layout` (or a dedicated render-prep system) composes
`ResolvedLayout.position` + `ResolvedTransform.matrix` into the entity's
Bevy `Transform`, so `TransformSystems::Propagate` owns `GlobalTransform`.
Requires pulling `TransformPlugin` into the relevant app + render reading
`GlobalTransform` instead of (or alongside) `ResolvedLayout`.

**Spec touchpoint:** `transforms-and-containment.md § 2`.

## Layout — `content-visibility: auto` off-screen skip

**Originated:** Phase 8 (D6 — stored, not enforced).

**Symptom:** `ContentVisibility::Auto` is stored on `Containment` and
warns once via `LayoutWarnOnceKey::ContentVisibilityDeferred(Entity)`, but
no off-screen layout/paint skip is performed.

**Implementation sketch:** implement the spec § 5.2 step-1 skip — check
`ContentVisibility::Auto` + off-screen (last-frame `ResolvedLayout` vs
viewport) + a `contain-intrinsic-size` hint; feed Taffy a sentinel size
and no-op the descendants' style sync; snap back on-screen. Needs a
`contain-intrinsic-size` component.

**Spec touchpoint:** `transforms-and-containment.md § 5.2`.

## Layout — `content-visibility: hidden` descendant skip

**Originated:** Phase 8 (D6 — stored, not enforced).

**Symptom:** `ContentVisibility::Hidden` is stored + warns
(`ContentVisibilityDeferred`), but descendants are still laid out.

**Implementation sketch:** equivalent to `Display::None` for descendants
(tree-prune in `sync_styles`); snap back on toggle.

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

## Layout — Phase 9 stacking sub-pass 6f reads `ResolvedTransform` — LANDED

**Originated:** Phase 8 (D1 — stacking deferred to Phase 9).

**Status:** **Landed** in Phase 9
(`docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md`). Sub-pass 6f
(`stacking_context`) now runs after `transform_composition` (6e) and reads
the composed `ResolvedTransform` (trigger 3). It implements the spec § 2 SC
trigger union (positioned + z-index, isolation, transform, paint/strict
containment, root), the § 2.1 five-tier z-index paint-order sort, and the
§ 4 top-layer escape. The render-side trigger-5 formers and the will-change
SC trigger remain deferred (separate follow-ups below).

**Spec touchpoint:** `transforms-and-containment.md § 3`, § 6;
`stacking-and-top-layer.md`.

## Layout — Phase 9 render-side stacking-context formers (`opacity` / `filter` / `mix_blend_mode`)

**Originated:** Phase 9 (D1 — trigger-5 formers deferred).

**Symptom:** CSS spec § 2 trigger 5 (a non-default `opacity`, `filter`, or
`mix-blend-mode` forms a stacking context) is not detected by sub-pass 6f.
Authors who set these properties get no `StackingContext` unless another
trigger (positioned + z-index, isolation, transform, paint/strict containment,
root) also fires.

**Cause:** the `opacity` / `filter` / `mix_blend_mode` properties live on
render-side components that do not exist in `buiy_core` yet (the
render-pipeline spec is unbuilt). 6f cannot read a property that has no
component.

**Implementation sketch:** when the render-side components carrying these
properties land, extend `forms_stacking_context` with an additional
`|| render_side_former` clause (the predicate signature takes only the inputs
available today; adding one is a localized change). Add the corresponding
unit tests next to the existing trigger tests.

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
