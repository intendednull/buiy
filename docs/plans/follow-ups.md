# Cross-phase follow-ups

**Date:** 2026-05-21
**Status:** active

Tracked-but-not-yet-scheduled work surfaced by completed layout phases. Each
entry names the originating phase, the divergence from spec / open behavior,
and a sketch of the implementation direction. When a follow-up gets
chartered into its own phase or plan, move the entry to that plan and link
back here.

## App-author ergonomics campaign (2026-07-13) — deferrals & minor follow-ups

The campaign (`docs/specs/2026-07-13-app-author-ergonomics-campaign-design.md`,
resolving the open items of the 2026-07-13 learnings report per the reconciliation
ledger `docs/reports/2026-07-13-app-author-ergonomics-reconciliation.md`) landed
Tracks A–F. What it deliberately did NOT do, plus minor follow-ups surfaced in review:

### DEFERRED — 7a-lint: §4.1c required-component suppression at compile/lint time

XL / high-uncertainty. The suppression lives inside upstream Bevy's `bsn!` proc-macro
Buiy does not own; there is no in-repo dylint/clippy-driver infra; a runtime
debug-assert cannot substitute (it can't distinguish a legit partial-patch from an
accidental require-suppression — the signal is author-time-only). Interim mitigation is
landed: a scene-fn for every styleable widget (all 14) + a round-trip regression test
pinning the behavior. **Revisit** if Buiy adopts a lint toolchain or forks `bsn!`.

### ROUTED → web-firstclass #143 — 1d web capability inertness (loud-surface + CI guard)

Outbound web a11y is solved (`WebA11ySinkPlugin`). The residual (inbound web AT still
inert; best-effort clipboard/IME degrade silently; no web-a11y CI conformance guard) is
owned by the active web-firstclass campaign (draft PR #144). Not duplicated here; the
campaign's Track A fail-loud mechanism (`buiy_core::mvu::MvuDiagnostics`) is the natural
home if #143 later routes an inertness signal.

### Minor follow-ups (surfaced in review; none blocking)

- **Prelude `ControlledLeaf`** (Track C): the `using-mvu` guide recommends
  `ControlledLeaf` for the leaf tier, but it is not re-exported through `buiy::prelude`
  (only `buiy_core::mvu::ControlledLeaf`). The guide shows the import; consider adding it
  to the facade prelude for 5c self-sufficiency.
- **Occluder predicate strictness** (Track B): `is_transparent_top_layer_occluder` flags
  any top-layer transparent node whose `Pickable != IGNORE`, stricter than "blocks every
  click" (a non-blocking-but-hoverable `Pickable` is also flagged). No false-positive
  today; if an intentional hoverable-non-blocking transparent overlay ever arises, tighten
  the predicate to the actual `should_block_lower` semantics or document the escape.
- **`workflow_dispatch` runs the full matrix** (Track E): a manual GPU-lane dispatch also
  runs the informational `llvm-cov` job (gated `!= pull_request`). Harmless, opt-in;
  inherent to the single-workflow-file design.
- **`TooltipNode::is_open` not root-re-exported** (Track F): reachable via
  `buiy_widgets::tooltip::TooltipNode`, unlike the root-level Popover/Menu/Dialog
  accessors (TooltipNode is an internal render node, not an author-facing marker).
- **Informational** — Track A's env-reducer diagnostic is compile-only (no runtime caller
  exists); Track D's companion `apply_background` fix + `update_hover_style` backstop are
  twin guards of the "Background always present on a HoverStyle node" invariant (the
  companion fix additionally avoids remove+insert churn at rest).

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


## Anchor positioning — `anchor-size()` term — LANDED

**Originated:** Phase 6 (anchor positioning), spec § 3.4 line 231
("tier-C feature deferred to v1.x").

**Status:** **Landed.** `anchor-size()` ships as
`Length::AnchorSize(AxisDimension)` (no `AnchorRef` payload — the
over-specified sketch below is superseded; the per-try anchor box is
already known at the resolution site `try_anchored_position`, so only the
axis selector is needed). The `to_px` closure resolves
`AnchorSize(Width) → anchor_size.x` / `AnchorSize(Height) → anchor_size.y`
against the per-try box; every non-anchor `Length` match site degrades it
to `0`/`auto` alongside the existing `Cq*` defensive arms. Sticky has no
anchor box, so `resolve_sticky_inset` resolves it to `0.0` and warns once
via the new session-scoped `LayoutWarnOnceKey::StickyAnchorSizeUnsupported`.
The previously-unreachable `AnchorErrorKind::AnchorSizeUsed` variant +
warn arm were **deleted** (not repurposed): anchor-size now resolves to a
real value, so an error/warn kind is semantically wrong; the variant had
no Reflect/serialization consumer worth preserving (it never fired).
Spec § 3.4 + README § 5 deferral reversed. Covered by
`try_anchored_position_resolves_anchor_size_{height,width...}` (unit) and
`anchor_size_inset_resolves_to_anchor_height` (integration).

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

**Status:** **Deferred (not built)** — reviewed by the follow-ups-drain campaign
(`2026-06-18-followups-drain.md`) and deliberately left deferred: both spec § 3.5
and README § 5 gate this on "if profiling surfaces a hot path", and no profiling
evidence exists, so the cap would be a speculative unused knob. **Re-open
trigger:** a *measured* deeply-nested-fallback hot path against a perf budget.
The implementation sketch below stands when that trigger fires.

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

## Anchor positioning — anchor target IS sticky/table/multicol (Phase 7 interaction) — LANDED

**Status:** **Landed** in Phase 7 (Task 9, the D1 fix), confirmed by the
follow-ups-drain campaign. `anchor_resolution` (6d) reads the anchor target's
position from `PostTaffyPositionOverrides.by_entity` (written by sticky 6a /
table 6b / multicol 6c) first, falling back to Taffy only when absent — exactly
**Option 1** below (the shared per-entity correction buffer all four sub-passes
contribute to). Tested end-to-end in
`tests/layout_sticky.rs::anchor_target_is_sticky_anchored_tracks_displaced_position`.
The original analysis follows.

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

## Anchor positioning — `AnchorRef::Entity` integration test gap — LANDED

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

**Status:** **Landed.** Two end-to-end tests added to
`crates/buiy_core/tests/layout_anchor_positioning.rs` —
`anchor_entity_ref_positions_relative_to_target` (positive: a direct
`AnchorRef::Entity` to a plain-Node target with no registry entry
resolves to the target box edge + inset; override `y == 60`, no
`LayoutAnchorBroken`) and `anchor_entity_ref_display_none_target_is_missing`
(negative: `AnchorRef::Entity` at a `Display::None` target → the D9 guard
in `resolve_target` fires via the Entity arm → zero override +
`LayoutAnchorBroken` + `TargetMissing` warn). Test-only; no production
change — the `AnchorRef::Entity(t) => Some(*t)` arm and the `Display::None`
guard already existed inside `build_anchor_edge_map`'s `resolve_target`
closure. Fills declared coverage of spec §4 'Test surface'; no
target-state change.

## Anchor positioning — extract `anchor_resolution` into sub-helpers — LANDED

**Originated:** Phase 6 (final whole-branch review).

**Symptom:** `anchor_resolution` (`crates/buiy_core/src/layout/systems.rs`,
~265 lines) followed 7 numbered algorithm steps inline. Pure refactor
candidate to keep later anchor edits rebasing onto clean helpers.

**Status:** **Landed.** Behavior-preserving extraction — zero observable
change; the existing anchor + pipeline-order suites stayed green throughout.
`anchor_resolution` is now a thin driver delegating to three private free
helpers, co-located directly below it (matching the existing
`kahn_anchor_sort` / `try_anchored_position` / `try_conditions_pass`
neighbors):

- `build_anchor_edge_map` (steps 2 + 3) — owns the `resolve_target` and
  `entity_epochs_fn` closures (capturing `&AnchorNameRegistry` /
  `&Query<&Display>` by shared ref), the per-entity edge insert +
  `TargetMissing` warn, the `kahn_anchor_sort` call, and the
  cycle-endpoint `InCycle` warn / `dropped_targets` loop. Returns
  `(edges, order, dropped, dropped_targets, new_warns)`; `dropped` is a
  `HashSet<Entity>` mirroring `kahn_anchor_sort`'s return so the driver's
  set-membership consumption (`dropped.contains`, the seeding loops) is
  unchanged.
- `apply_anchor_broken_markers` (step 6) — idempotent `LayoutAnchorBroken`
  insert/remove across the anchored set, the `dropped_targets` plain-Node
  set, and the non-anchored cleanup set (loop order + the
  `anchored_query.get(t).is_err()` cleanup guard kept verbatim).
- `emit_anchor_warns` (step 7) — the `warned.set.insert` dedupe gate + the
  exhaustive per-`AnchorErrorKind` `warn!` match.

The step-5 topological walk was **intentionally left inline** in the driver
(out of the named sketch): it is the most entangled body — it closes over
`tree` + `overrides` + `reg` + `display_query` + `viewport` +
`anchored_query` and mutates `overrides` / `broken_set` / `new_warns` — so
extracting it was out of this slice's scope.

**Spec touchpoint:** none — pure internal refactor, no target-state change.

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

## Layout — sticky `Length::Cq*` inset resolution — LANDED

**Originated:** Phase 7 (D3 deferral).

**Status:** **Landed.** Sticky `Length::Cqw/Cqh/Cqi/Cqb/Cqmin/Cqmax`
insets now resolve via the shared `translate.rs::resolve_cq_unit_px`
against the sticky entity's own nearest container-query ancestor
(`Container { container_type != Normal }`, found by walking `ChildOf`
from the sticky entity — distinct from anchor's per-try "anchor target
box" frame). The CQ-ancestor size is read CURRENT-frame from Taffy
(`tree.by_entity` + `tree.tree.layout`), NOT from last-frame
`&ResolvedLayout`: `sticky_offset` runs in `PostTaffyOverrides` (after
`TaffyCompute`, before `WriteResolvedLayout`), so a container's
`ResolvedLayout` is stale there but its Taffy size is fresh — this keeps
sticky `Cq*` same-frame-consistent with the self/parent/scroll sizes
`sticky_offset` already reads from Taffy, and collapses the tests to a
single `app.update()`. Cqi/Cqb resolve on the writing-mode inline/block
axes (per-entity `WritingModeResolved`); the no-CQ-ancestor case rides
`resolve_cq_unit_px`'s existing viewport fallback, identical to every
other Cq* site. The `StickyCqDeferred` warn variant was **retired**
(delegating to `resolve_cq_unit_px` makes it dead; keeping it would
double-warn). Covered by `sticky_cqw_inset_resolves_against_nearest_cq_ancestor`,
`sticky_cqi_inset_resolves_on_inline_axis_under_vertical_writing_mode`,
`sticky_cqb_inset_resolves_on_block_axis_under_vertical_writing_mode`,
and `sticky_cqw_resolves_against_inner_cq_ancestor_not_scroll_container`
in `tests/layout_sticky.rs`.

**Symptom:** Sticky entity with `Length::Cqw/Cqh/Cqi/Cqb/Cqmin/Cqmax`
inset emitted `LayoutWarnOnceKey::StickyCqDeferred(Entity)` and resolved to
0.0.

**Implementation sketch (as landed):** reuse `resolve_cq_unit_px` (the
same resolver sizing/tracks/edges use) instead of porting Phase 6's
anchor-box-shaped `length_inset_to_px`. Sticky's reference frame is the
sticky entity's own nearest CQ ancestor. Multi-axis fixtures pin the
Cqi/Cqb writing-mode axis swap.

**Spec touchpoint:** `display-and-positioning.md § 2.3`,
`container-queries-and-writing-modes.md § 1.4`.

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

## Layout — sticky both-top-and-bottom dual clamp — LANDED

**Originated:** Phase 7 (D4 — v1 "top wins" deviation).

**Status:** **Landed.** `compute_sticky_displacement` now honors the
bottom inset: when both `inset_top` and `inset_bottom` (or both
`inset_left`/`inset_right`) are set, both clamps apply simultaneously per
CSS § 6.3. Each axis applies the top line `U = visible_top + top_px`
(`.max(U)`), then the bottom line `L = (visible_bottom − bottom_px) −
size` (`.min(L)`); when the band is shorter than the box (`U > L`) it
re-applies `.max(U)` so the top edge takes precedence. The band clamp
stays an explicit `.max(parent_lo).min(parent_hi)` (NOT `f32::clamp`,
which panics when `lo > hi`). Each axis reduces to the prior single-edge
formula when only one inset is set (the single-edge regression tests stay
green). The v1 "top wins" tests were flipped: the integration regression
test `sticky_both_top_and_bottom_inset_top_wins` →
`sticky_both_top_and_bottom_bottom_honored_near_scroll_end` and the pure
unit test `sticky_both_top_and_bottom_active_top_wins` →
`sticky_both_top_and_bottom_dual_clamp_bottom_honored` (both now assert
the bottom inset wins near the scroll end). New fixtures: the
`sticky_both_top_and_bottom_conflict_top_precedence` unit test locks the
`U > L` top-precedence re-max branch (verified anti-vacuous: deleting the
branch breaks it), and `sticky_both_insets_clamp_at_both_extremes`
(integration) proves both edges clamp at their respective scroll
extremes.

**Symptom:** Sticky element with both `inset_top` and `inset_bottom` set
ignored the bottom inset (top wins). CSS spec § 6.3 implies dual-clamp
behavior where the element sticks to whichever edge the scroll position
is closer to.

**Implementation sketch (as landed):** replace each axis's
`if-top else-if-bottom` chain (which left the bottom branch unreachable
when both were set) with a single dual-clamp expression applying both
thresholds, plus the `U > L` top-precedence re-max for the degenerate
band-shorter-than-box case. The 10-arg signature is unchanged.

**Spec touchpoint:** `display-and-positioning.md § 2.3`; CSS spec § 6.3
(positioned layout).

## Layout — sticky inside sticky — LANDED

**Originated:** Phase 7 (documented v1 limitation).

**Status:** **Landed.** A sticky element nested inside a
sticky-displaced ancestor now tracks the ancestor's DISPLACED position,
same-frame. Two coordinated changes inside sub-pass 6a (`sticky_offset`):
(1) the qualifying sticky entities are DEPTH-SORTED by `ChildOf`-chain
depth ascending (`child_of_depth`) so a shallower (outer) sticky resolves
and inserts its `PostTaffyPositionOverrides` entry BEFORE a deeper (inner)
sticky reads it — same-frame eventual consistency via depth ordering, NOT
two-frame; (2) `world_position` takes an `&HashMap<Entity, Vec2>` override
map and, per ancestor-walk segment, uses the just-written override
(`natural_rel + displacement` = that entity's displaced rel-to-parent
location) when present, else the Taffy `.location`. The per-call `memo`
is CLEARED at the top of each sticky-entity iteration (its only purpose is
reuse between the `e` and `parent` walks of the SAME entity; sharing it
across the depth-ordered loop would return values cached before an outer
override existed). The override map is passed as `&overrides.by_entity`
(shared borrow); the current entity's own `.insert` happens at the END of
its loop body after both reads, so no borrow conflict and no per-frame
clone. A's displacement reaches B's final render position through the
normal parent→child `ResolvedLayout`/`Transform` composition (applied via
A's own override), NOT by baking it into B's stored value — so it is not
double-counted. Siblings at equal depth in unrelated subtrees have no
ordering dependency, so an unstable sort by depth alone suffices (no full
toposort). Covered by `sticky_inside_sticky_inner_tracks_displaced_outer`
(inner tracks displaced outer → no over-displacement) and
`sticky_inside_sticky_override_value_is_not_double_counted` (pins the
exact inner override value to prove the outer displacement is not
re-added) in `tests/layout_sticky.rs`; the single-level
`sticky_pins_to_top_during_scroll` and nested-scroll
`sticky_in_nested_scroll_containers_uses_innermost` tests stay green
(no regression — single-level depth-sort is trivial, no ancestor override
means `world_position` falls back to Taffy `.location` exactly as before).

**Symptom:** When entity A is sticky-displaced and entity B is a sticky
child of A, B's `world_position` walks Taffy positions (un-displaced);
B's threshold computation uses A's *natural* position, not displaced.
Rare authoring case.

**Implementation sketch (as landed):** depth-sort the sticky set in
`sticky_offset` (outer resolves first), and consult the just-written
`PostTaffyPositionOverrides` per segment when walking the ancestor chain
in `world_position`, clearing the `world_position` memo between sticky
entities. L4 (the widened `resolve_sticky_inset` Cq/viewport/wmr params,
the `sticky_offset` container_q/wmr_q/primary_window plumbing, the
current-frame `container_index`) and L5 (the dual-clamp form of
`compute_sticky_displacement`) are untouched.

**Spec touchpoint:** `display-and-positioning.md § 2.3`.

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

## Layout / render — `will-change` layer promotion + SC trigger — SC TRIGGER LANDED; layer promotion DEFERRED

**Originated:** Phase 8 (D7 — tier-E, stored-only).

**Status — SC trigger (landed):** the stacking-context half is **landed**.
`forms_stacking_context` (layout/systems.rs) now reads `Containment.will_change`
and forms a `StackingContext` when the list names an SC-forming property. The
SC-forming subset is encoded once in `WillChangeProperty::forms_stacking_context`
(Transform / Opacity / Filter; `ZIndex` / `ScrollPosition` excluded — `will-change:
z-index` does not form an SC, matching CSS). Unit tests sit beside the other
trigger tests (layout/systems.rs); end-to-end coverage in tests/layout_stacking.rs.
See the "Phase 9 `will-change` stacking-context former" entry below.

**Status — layer promotion (deferred):** the **render layer-promotion hint**
remains deferred. There is no composition-layer / `RenderLayers` concept in
render/ to hang a promotion hint on, so this half is not yet actionable — it
stays open until such a mechanism exists.

**Spec touchpoint:** `transforms-and-containment.md § 5.3`;
`stacking-and-top-layer.md § 2` trigger 5, § 7.

## Render — `UiTransform` paint + `Containment` PAINT clip rect + perspective / backface — TRANSFORM-PAINT + PAINT-CLIP LANDED (R1); perspective / backface / transform-origin / skew DEFERRED

**Originated:** Phase 8 (spec § 4 — render-side concerns stored only).

**Status:** transform-paint (rotation + (non-)uniform scale) LANDED (R1); the
PAINT clip rect was ALREADY done; perspective / `Preserve3d` /
`BackfaceVisibility` remain C-tier deferred. Do NOT close — the residuals below
keep this entry open.

**LANDED (R1, as landed):** the 2D affine paint. Extract consumes the
`GlobalTransform` 2D linear part (`global_transform.affine().matrix3` xy columns
— NOT a re-read of `ResolvedTransform`; pillar-5 contract, the bridge already
folded `ResolvedTransform.matrix` into `Transform`) and carries it as
`ExtractedNode.affine`. `PackedInstance` grew by APPENDING the 2x2 basis
(`[m00,m10,m01,m11]`) AFTER the existing 13 floats so every prior offset stays
byte-stable; the named `COLOR_FLOAT_OFFSET = 4` / `ALPHA_FLOAT_OFFSET = 7`
consts were added for R2's degraded-group re-tint. The quad + shadow vertex
stages apply the affine to each box-local corner (`mat2x2 * local`) before the
logical→clip view map, interpolating `frag_logical` for the clip-AABB discard
(stride 52 B → 68 B, vertex attrs `@location(8)/(9)`). Identity basis
`[1,0,0,1]` is byte-identical to the pre-R1 axis-aligned path. GPU rotate/scale
reftest: `tests/render_transform_paint_gpu.rs` (`#[ignore]`).

**ALREADY done (pre-R1):** the `Containment` PAINT clip rect — the per-primitive
clip AABB via `clip::clip_for_primitive` + `write_clip_rects` (clip.rs ~196),
packed onto every instance (the R8b fragment discard).

**Residual (still C-tier deferred):** `perspective`, `TransformStyle::Preserve3d`,
and `BackfaceVisibility::Hidden` are stored on `UiTransform` but render does not
consume them (render/mod.rs ~388). The 2D affine path does not carry a
projective channel.

**Residual A (LANDED 2026-07-01 — `docs/specs/2026-07-01-glyph-affine-transform-design.md`):**
`transform-origin` IS now honored by 6e — `compose_transform` conjugates the
composed matrix by the resolved origin (`M = T(O)·t·r·s·m·T(-O)`, default
`50% 50%` = center), baking the pivot into `ResolvedTransform`/`GlobalTransform`
once so every consumer agrees. The identity fast-path is bit-exact; a pure
translate skips the conjugation. The meter fill opts back into a left-edge origin
to keep its left-anchored `Scale`. **Picking correction:** picking is a
translation-anchored AABB that never modeled rotation (it does NOT invert the
affine), so the pivot bake is safe only because every rotated/scaled element is
`Pickable::IGNORE` — see the new picking residual below.

**Residual A2 (newly surfaced — picking, DEFERRED):** picking
(`picking::point_in_aabb`) hit-tests the *unrotated* `layout.size` at
`gt.translation()` and never inverts the 2D affine, so a rotated/scaled element's
pick box is wrong (shifted). Harmless today (all rotated elements are
`Pickable::IGNORE`), but a rotated *pickable* element (e.g. a rotated drag handle)
would pick wrong. Fix = invert `ResolvedTransform` in the hit-test (spec
`buiy-input-events` says picking is done in transformed space — currently
aspirational for rotation).

**Residual A3 (newly surfaced — text-quad decorations, DEFERRED):**
underline / overline / selection / preedit paint through the `PackedInstance`
text-quad carrier (`pack_text_quad`), which packs `IDENTITY_AFFINE` — so a rotated
text run's *decorations* stay axis-aligned even though its glyphs (+ strike, a
coverage stamp) now rotate. Same root cause, different carrier; no current
consumer rotates a decorated text run, so deferred. Fix = thread the entity affine
into `pack_text_quad` (the carrier already has the slot).

**Residual B (newly surfaced — bridge fidelity):** skew (`TransformMatrix::Skew`)
and general `TransformMatrix::Matrix` paint are BOUNDED by the bridge's lossy
TRS-only `Transform::from_matrix` decompose (bridge.rs; proven lossy by
`from_matrix_drops_projective_perspective_row_keeps_affine`). A Bevy `Transform`
is TRS-only and cannot represent a general shear, so the extracted 2D linear part
is FAITHFUL for rotation + non-uniform scale but skew/general-matrix do NOT paint
faithfully yet. Faithful skew needs the bridge to stop round-tripping through TRS
(or render to read a non-TRS source).

**Spec touchpoint:** `transforms-and-containment.md § 4`, § 5.1;
`clip-and-transform.md § B.5`.

## Render — R11 forced-colors cross-phase seams (CatalogPaint + BoxShadow draw-skip)

**Originated:** R11 (color / forced-colors / verify). R11's gate-#11 analyzers run
over a `CatalogPaint` descriptor seam, and the forced-colors `BoxShadow` draw-skip
has no live producer yet.

**Symptom / what's deferred:**
1. ~~`forced_colors_analyzer::{analyze_forced_colors, analyze_shadow_only}` analyze
   `CatalogPaint` descriptors that tests construct by hand — there is no live
   widget-catalog source.~~ **DONE** (`buiy-verification-design` Phase 4.6,
   commit `a73de05`). `buiy_verify::coverage::forced_colors::live_catalog_paint`
   builds each fixture's app, queries the spawned `Background`/`Border`/`Outline`
   (+ `BoxShadow`-presence delta) off the `Name`-tagged root, and projects them
   into the existing `CatalogPaint`; the analyzers run **unchanged** — only the
   input source moved from hand-built descriptors to the live tree. Teeth: a
   `#[cfg(test)]` brand-token fixture (excluded from the real catalog) MUST flag
   `NonSystemColor`, proving the producer reads real paint. Gate #11 now
   auto-enrolls every new widget by construction. (`coverage.md` § Wiring /
   § Landed.)
2. **STILL DEFERRED (renderer-blocked).** color-and-forced-colors.md § 3.3: in
   forced-colors mode, extract must read `UserPreferences.forced_colors` and SKIP
   the `BoxShadow` batch (shadows are decorative, suppressed under forced colors).
   `extract_buiy_nodes` has no such branch — it lands when `BoxShadow` gets a real
   extract/draw path (the BoxShadow primitive pipeline is itself a later-tier
   seam, R7 bucket-reserved only). The forced-colors **visual** reftest that would
   exercise the draw-skip is therefore specified but BLOCKED; it is an
   `#[ignore]`'d, assertion-free placeholder
   (`coverage::forced_colors::boxshadow_visual_reftest_is_blocked`), not a green
   test. The structured token-flow / no-shadow-only analyzers cover the rest of
   gate #11 now, with no dependency on this path.

**Spec touchpoint:** `color-and-forced-colors.md § 3.3`; the gate-#11 section;
`buiy-verification-design` `coverage.md` / `reftests.md`.

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

## Render — node-draw model: per-entity clip + composite passes (R8 Task 8 / R9 blocker) — LANDED

**Status:** **Landed** as R8b
([2026-06-07-buiy-render-r8b-node-draw.md](2026-06-07-buiy-render-r8b-node-draw.md)),
ratified as Option C of the design note
(`docs/specs/2026-06-03-buiy-render-pipeline-design/2026-06-06-render-node-draw-model-design.md`).
The recommended **hybrid** shipped: per-instance clip-AABB fragment-discard
(`clip_for_primitive` → `PackedInstance` `clip_min`/`clip_max`, threaded into R6's
instance layout + WGSL) plus the multi-pass composite node (`BuiyNode::run` —
normal pass → `partition_top_layer`-driven top-layer composite pass), which the R9
effect-compositor passes then build on (see the **effect-compositor GPU
orchestration** LANDED entry above). So this is no longer a blocker before R8 Task
8 / R9: both consumed the ratified model. The original deferral text follows.

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
(next entry); the will-change SC trigger has since landed too (separate
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
never drift apart. The `will-change` SC trigger has since landed (separate
follow-up below); `BackdropFilter` deliberately forms an `EffectGroup` but never an SC
(render component-model.md § 8). Unit tests sit beside the other trigger
tests (layout/systems.rs); end-to-end coverage in tests/layout_stacking.rs.

**Spec touchpoint:** `stacking-and-top-layer.md § 2` trigger 5, § 7.

## Layout — Phase 9 `will-change` stacking-context former — LANDED

**Originated:** Phase 9 (D1) — coordinates with the Phase-8 "will-change
layer promotion + SC trigger" follow-up (above).

**Status:** **Landed.** `forms_stacking_context` (layout/systems.rs) gained a
trigger-5b clause: when `Containment.will_change` is `WillChange::Properties`
and names an SC-forming property, the entity forms a `StackingContext`. The
SC-forming subset is encoded once as `WillChangeProperty::forms_stacking_context`
(types.rs) = Transform / Opacity / Filter; `ZIndex` and `ScrollPosition` are
excluded (CSS: `will-change: z-index` does not create an SC — z-index needs
positioning). No signature change was needed: the 6f `forms` closure already
passed `containment_q.get(e).ok()`, so the unit-level predicate and the
end-to-end 6f path lit up together. Unit tests sit beside the other trigger
tests (layout/systems.rs) and the subset helper (types.rs); end-to-end
coverage (positive + layout-only negative) in tests/layout_stacking.rs.

Only the SC-trigger half landed. The `will-change` **render layer-promotion
hint** stays deferred (see the combined Phase-8 entry above) — there is no
composition-layer / `RenderLayers` concept in render/ to honor it.

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

## Layout — non-px translate units in `compose_transform` — PERCENT LANDED (Cq* residual)

**Originated:** Phase 8 (CHANGELOG deferral note).

**Symptom:** `compose_transform` resolves only `Length::Px` for translate;
percent / `Cq*` translate contributes `0.0`.

**Status — PERCENT landed (2026-06-18).** As landed: `compose_transform` and
`transform_matrix_to_mat4` now take the entity's own current-frame border box
and resolve a `Percent` translate term against it per CSS Transforms —
`translateX(p%)` = `p%` of border-box **width**, `translateY(p%)` = `p%` of
**height** (each axis against its own dimension), `translateZ` percent (invalid
in CSS) → `0`. Sub-pass 6e (`transform_composition`) reads the box straight from
the **current-frame** Taffy tree (`tree.tree.layout(node).size`, mirroring
`anchor_resolution` (6d)) — *not* `ResolvedLayout`, which is still last-frame at
6e time. The `Length::Px` translate path is byte-for-byte unchanged (regression
guarded by `translate_transform_composes_to_resolved_transform`). Tests:
`translate_percent_x_resolves_against_own_width`,
`translate_percent_y_resolves_against_own_height`,
`translate_mixed_percent_and_px`, `cq_translate_is_residual_zero`
(`crates/buiy_core/tests/layout_transforms.rs`); the `Style::translate(Length,
Length)` builder was added to express percent translate.

**RESIDUAL — `Cq*` translate still deferred.** `Cq*` translate
(`cqw/cqh/cqi/cqb/cqmin/cqmax`) needs the entity's nearest CQ-ancestor container
frame (the sticky-L4 / `resolve_cq_unit_px` machinery), which sub-pass 6e does
not gather. It resolves to `0.0` and fires a one-shot warn
(`warn_once_cq_translate_residual`). Resolving it requires threading the nearest
CQ-ancestor `ContainerSnapshot` into 6e — held out for scope discipline.

**Spec touchpoint:** `transforms-and-containment.md § 1` ("Translate length
units"), § 1.1.

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

**Status: Root-degraded LANDED; nested-degraded follow-up filed** (R2).

**Originated:** text campaign T8 implementation reading (the T8 plan's D9).

**As-was symptom:** a `plan_allocation == false` group got no pooled target,
`BuiyNode::run` step 1 `continue`d, and its members were excluded from
`flat_ranges` / `glyph_flat_ranges` — so under RT-pool budget pressure a
degraded group's quads AND glyphs painted nowhere, despite the "drawn flat
instead" comments. Latent under the 64 MiB budget (no fixture degraded). T8
had mirrored the quad semantics for glyphs (a degraded group's glyph range
was likewise skipped) rather than silently widening scope.

**Resolution (R2):** the route-flat-vs-skip fork is RESOLVED in favor of
**forward-compositing**, per `effect-compositor.md § 2.3` (skip contradicted
the spec). A ROOT degraded group (`parent == None`) now folds `group.opacity`
into each member instance's alpha IN PLACE (quad alpha at
`ALPHA_FLOAT_OFFSET` = 7 on the `[f32;17]` record; glyph alpha at the parallel
`GLYPH_ALPHA_FLOAT_OFFSET` = 11 = `GlyphAlphaInstance.color[3]`) and merges
its instance ranges into `flat_ranges`/`glyph_flat_ranges` so the flat WINDOW
draw paints it — it dims exactly once and paints flat, never vanishes
(`compositor::fold_root_degraded_into_flat`, called from
`prepare_effect_groups`). Per-tier idempotency: the fold runs iff the
corresponding BUFFER was repacked this frame (quad on `quad_dirty`, glyph on
`glyph_dirty` — the buffer-repack signals, which DIFFER from the wider
glyph-partition signal), so a retained buffer never re-compounds. Gated on
`allocate.iter().any(|a| !a)` to preserve the gate-#14 zero-upload steady
state. The budget is overridable via the new `RtPoolBudget` resource so a test
forces degradation deterministically. (R1's named `ALPHA_FLOAT_OFFSET = 7`
const + append-after-13 `PackedInstance` layout supplied the byte-stable quad
offset this fold depends on.)

**Out of scope (nested):** a NESTED degraded child (`parent == Some`) was NOT
handled by this slice — see the next section, now **LANDED (case A)**. (The
`fold_root_degraded_into_flat` fn has since been renamed `fold_degraded_groups`.)

**Spec touchpoint:** `effect-compositor.md § 2.3` (as-landed note added).

## Render — nested degraded effect group must forward-composite into the parent target (not the window) — LANDED (case A; chain + kept-child deferred)

**Originated:** R2 (degraded-group forward-composite), MAJOR-1 scope decision.

**Problem:** `plan_allocation` (`compositor.rs`) ranks purely by (extent,
reason) and CAN degrade a NESTED child (`extracted[i].parent == Some`) while
its parent keeps a target. R2's fix routes a degraded group's instance ranges
into `flat_ranges`, which the node draws in the WINDOW pass (`buiy_pass`,
`node.rs`). That equals "the parent target" the spec § 2.3 mandates ONLY when
the degraded group is a ROOT group. For a nested degraded child, window-level
flat-merge would paint it in the wrong space/clip, and the parent's step-2a
composite (which already skips when either end lacks a target) would then
sample a parent target the child never reached — double-wrong. So R2 scoped to
root-degraded.

**Status: LANDED (case A, 2026-07-01).**
[spec](../specs/2026-07-01-nested-degraded-forward-composite-design.md) /
[plan](2026-07-01-nested-degraded-forward-composite.md).

**Fix (as landed):** the fold (`fold_root_degraded_into_flat` → renamed
`fold_degraded_groups`) now folds EVERY degraded group's own opacity into its
members' alpha (root + nested) and merges into `flat_ranges` only for ROOTS. The
node's step-2a loop gains a `(child=None, parent=Some)` arm — case A — that draws
the nested child's already-folded `instance_range`/`glyph_range` directly into the
PARENT's `Rgba16Float` target (`LoadOp::Load`, the parent's `target_view_columns`)
before the parent composites, so the child rides along in the parent's composite.
No cumulative opacity is needed for case A (the parent's own composite supplies the
parent opacity). Because bounds grow by OWN DIRECT members only
(`extract.rs:1685-1689`), case A occurs when the parent carries the larger,
spatially-containing direct paint — the spec-gate corrected an initial premise that
assumed nesting depth drives degrade order.

**Bonus win:** removing the nested `debug_assert!(false, …)` means a nested
degraded group no longer PANICS prepare in a debug build (incl. the default test
gate) — it skips cleanly (folded-but-undrawn) where not injected.

**Verification:** a headless `plan_allocation` pin
(`plan_allocation_pins_case_a_budget_outer_kept_inner_degraded`) proves the GPU
fixture's exact bucketed extents (outer 64²=32768 B, inner 32²=8192 B) + budget
33000 yield [keep outer, degrade inner] with no GPU; the flipped fold unit test
(`degraded_fold_folds_nested_alpha_but_never_merges_it`) proves nested is
folded-not-merged; the GPU test (renamed
`nested_degraded_child_forward_composites_into_parent`) proves the injected inner
is present at the exact two-stage composed level (`composite_src_over` ×2). RX 6700
XT: allocate=[true,false], injection fires, all 4 degraded-group GPU tests green.

**Deferred (still vanish — folded + node-skipped, no worse than before), each its own follow-up:**
1. **`(None,None)` degraded chain** — a nested group whose immediate parent is ALSO
   degraded. Needs cumulative opacity (the child gets no free parent composite) AND
   an ancestor-first injection order (post-order would flatten a descendant UNDER
   its ancestor). Often the MORE common shape: a bare-`Opacity` wrapper parent (no
   direct paint) has ~empty bounds and degrades first, so wrapper-nesting tends to
   land here, not in case A.
2. **`(Some,None)` kept child under a degraded parent** — a group that kept its own
   target but whose parent degraded away its target; its target is orphaned (the
   parent has none to composite into). Fix = route the kept child's composite past
   the degraded parent to the nearest kept ancestor/window with the parent's
   opacity folded — a distinct forward-composite case.
3. **Parent-target undersizing (pre-existing, out of scope of the fix)** — because a
   group's bounds omit nested-child extent (`extract.rs:1685-1689`), a parent's
   pooled target can be too small to hold a nested child that exceeds the parent's
   own paint box; this already threatens the `(Some,Some)` composite path and would
   equally clip a case-A injection. The case-A test sidesteps it by keeping the
   inner ⊆ the outer's own paint.

**Spec touchpoint:** `effect-compositor.md § 2.3` (flipped to case-A landed);
`docs/specs/2026-07-01-nested-degraded-forward-composite-design.md`.

## Text — production ASCII pre-warm (rejected as unmeasured)

**Originated:** text campaign T9
(`docs/plans/2026-06-11-buiy-text-t9-verification-closure.md` D1), deciding
the architecture § 2.3 deferral carried since T4.

**Status:** **rejected, not deferred** — no production pre-warm ships. The
T4–T8 evidence: every golden passed with warmup satisfied structurally; the
T8 gate-#14 fixture proved one frame from edit to publish including
rasterize-on-miss, so the win is sub-frame CPU at most; grace eviction is
unconditional, so pre-warmed keys no visible text uses drain `eviction_grace`
(~1 s) after startup; and no theme-font/size enumeration exists to warm from.
The seam stays named (`AtlasWarmupQueue`, T6's solid-stamp push as the worked
example).

**Re-open trigger:** a *measured* first-keystroke-latency miss against a
`buiy-verification-design` latency budget — measurement, not speculation (the
`shape-run-cache` precedent). Any revival must explicitly solve the
grace-drain constraint: unused warm keys get no `ResidentTextKeys` touch and
evict within `eviction_grace`, and the pin/refcount mechanism that would keep
them resident was already rejected (glyph-pipeline § 6.3 runner-ups).

**Spec touchpoint:** text `architecture.md § 2.3` (as-landed note).

## Render / verification — GPU tests rewrote committed report-asset PNGs — LANDED

**Status:** **Landed (2026-07-01).**
[spec](../specs/2026-07-01-parity-report-asset-test-hygiene-design.md) /
[plan](2026-07-01-parity-report-asset-test-hygiene.md).

**Originated:** noticed during the verification-followups campaign (2026-07-01) —
a plain GPU `--ignored` lane run left `git status` dirty.

**What it was:** six `#[ignore]` GPU-lane tests in
`crates/buiy_core/tests/render/` each ended with an **unconditional**
`img.save(&out)` into a committed `docs/reports/parity-*-assets/*.png` — a
gratuitous side-effect no assertion consumed (the tests verify programmatically
via adapter-tolerant pixel sampling). On an adapter whose bytes differ from the
committed capture (CI's lavapipe vs the RX host the baselines were blessed on)
the write dirtied the tree with adapter-specific rasterizer noise; on the RX host
it was byte-identical but still `touch`ed the file (proven via mtime).

**Fix:** removed the six write blocks (the committed PNGs freeze as the one-time
parity/paint-order proof captures — they are report illustrations, **not**
goldens; the golden corpus under `crates/buiy_verify/tests/goldens/` is the only
place a test writes a committed PNG, and only under `BUIY_BLESS`). Reworded the
five module doc-comments that claimed the test "writes the PNG". Added a durable
CI guard after the GPU-lane step (`ci.yml`): `git diff --exit-code -- docs/reports/`
fails the lane if any future test rewrites a committed report asset. Verified
RED→GREEN on the RX 6700 XT: pre-fix the write moved the PNG mtime; post-fix the
mtime is stable across a full six-writer run and `git status docs/reports/` stays
clean, all six tests still green.

**Known out-of-scope sibling (NOT fixed):** `docs/reports/2026-06-30-demos-mvu-migration-assets/`
is written by the same pattern from `src/bin/` capture binaries
(`capture_todomvc`/`capture_counter`), which the test lane never runs. The
broadened `docs/reports/` CI guard keeps the whole report tree honest regardless.

**Spec touchpoint:** none in `buiy-verification-design` (governs the golden
corpus, not these report illustrations).

## Render / verification — stored-PNG golden machinery (`--accept`) — LANDED

**Status:** **Landed** by `buiy-verification-design`
([spec](../specs/2026-06-15-buiy-verification-design/README.md),
[plan](2026-06-15-buiy-verification-impl.md), Phases 0–4). The original deferral
text follows.

**Originated:** render GPU campaign Phase 3 deferral
([2026-06-07-render-gpu-verify-campaign.md](2026-06-07-render-gpu-verify-campaign.md)),
carried unchanged through the text campaign's golden suite — every pixel
golden T4–T9 uses the inline + double-capture discipline (capture in a fresh
app, assert inline expected pixels, re-capture in a second fresh app,
`perceptual_diff < 1e-4`; "the re-capture IS the golden").

**Now landed (the machinery the deferral asked for):**
- **Stored-PNG golden machinery + `--accept`** — `buiy_verify::golden`
  (`assert_golden`/`check_golden`, multi-positive `tests/goldens/` corpus, the
  `BUIY_BLESS`/`BUIY_BLESS_REPLACE` accept-FILE workflow modeled on
  `BUIY_ACCEPT_SHAPING`, the `BlessLedger` durable accept record, the
  self-contained offline HTML triage report + diff-PNG). Corpus **started**:
  `rect-rounded` + `text-ahem` cells blessed.
- **Unified perceptual metric** — `buiy_verify::metric` (AA-aware two-axis
  pixelmatch-YIQ diff, vendored; advisory MSSIM), replacing the L1
  `perceptual_diff` (now `#[deprecated]`) and the RMSE `compare_images` (deleted).
- **Determinism stack** — `DeterministicApp` + `GoldenConfig` extensions
  (`FontMode` Ahem/Real, `Dpr` pin, MSAA/dither pinned, the `PendingCaptureAssets`
  quiescence flush), the lavapipe CI pin (`VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME`).
- **Layout snapshots (gate #5)** — `assert_layout_snapshot` over `ResolvedLayout`.
- **Property invariants (gate #12, visual half)** — the six `buiy_verify::invariant`
  proptest predicates incl. BiDi caret round-trip, with mutation-fixture teeth.
- **Forced-colors live wiring (gate #11)** — `live_catalog_paint` re-points the
  analyzers at the live catalog (see the R11 entry above).

**Still DEFERRED (renderer-blocked or out-of-scope, tracked here):**
- **Shadow-blur-kernel golden — LANDED (2026-07-01, V4).** `golden_shadow_blur_kernel`
  (`goldens.rs`) commits the drop-shadow Gaussian blur-kernel residue (an offset
  `BoxShadow`'s AA falloff), blessed on pinned lavapipe — the renderer block was
  stale (`resolve_shadows` + `shadow.wgsl` landed). **Color-emoji golden — still
  deferred (V5):** blocked on the color-glyph render leg (`SwashContent::Color` is
  still `SkipColorEmoji`; no color `IconInstance` producer/shader) AND a pinned
  bundled COLR/CBDT emoji font — a real renderer feature, out of scope here.
- **`coverage_golden::matrix_goldens` — skip-as-pending LANDED; remaining work is
  blessing the real residue cells.** The Tier-5 enrollment driver
  (`coverage_golden.rs`) iterates `Matrix::ci_default()` over the catalog and
  `assert_golden`s each cell. Resolution option (i) — *skip un-blessed cells
  instead of fail-closing the lane* — has shipped: `coverage_golden.rs:104-114`
  skips any cell whose `committed_positives(&key) == 0` (counting it as
  `pending`) unless `BUIY_BLESS` is set, so the `--ignored` GPU lane is now
  **green**, not RED, while the corpus is still being built. The test's real
  non-vacuity guard (audit #14): on the pinned lavapipe, if the corpus blesses
  ANY matrix cell, a green run must have compared ≥1 (`assert!(asserted > 0)`,
  `coverage_golden.rs:178-185`); the separate `asserted + pending > 0`
  (`coverage_golden.rs:191`) is only a catalog-non-empty sanity check. It
  prints an HONEST status line distinguishing cells *compared* from cells
  *pending*; a *blessed* cell still fails closed on drift (the "no golden
  committed for `<slug>`" message is `golden/check.rs:282`, reached only on the
  bless/assert path — never for skipped cells). **Blessing the forced-colors-safe
  residue cells — LANDED (2026-07-01, V6):** `matrix_goldens` now honors the
  fixture paintability predicate (`snapshots_cell`) in BOTH the assert and
  `BUIY_BLESS` paths, so it can only ever bless the paintable cells; the 12
  `theme==ForcedColors` `button` cells are blessed on pinned lavapipe (real white
  `ButtonText` fill, zero magenta sentinel), flipping the lane from `asserted==0`
  (aspirational) to 12 compared. The 12 *light-theme* cells stay pending because
  the default `Button` under Buiy's *wholesale* forced-colors swap still paints the
  magenta sentinel (`color-and-forced-colors.md § 3.1`) — blessing them verbatim
  would cement a known-wrong pixel; they land once the default widget is
  forced-colors-safe (`buiy-widget-catalog-design`). The headless every-PR gate
  never runs `--ignored` and is unaffected throughout.
- **Forced-colors `BoxShadow` *visual* reftest — LANDED (2026-07-01, V1).** The
  `BoxShadow` extract/draw path landed (`resolve_shadows` returns empty under
  `forced_colors`; `shadow.wgsl` rasterizes), so the assertion-free `#[ignore]`
  placeholder is replaced by a real Tier-4 reftest `forced_colors_boxshadow_suppressed`
  (`coverage_forced_colors.rs`): a shadowed box vs the same box unshadowed, both
  under forced-colors, must rasterize identically (the draw-skip). Adapter-agnostic,
  verified on the RX; proven non-vacuous (disabling the suppression reds it with 566
  differing shadow pixels).
- **Multi-reference reftest aggregation** — the `RefCase::multi` OR/AND
  aggregation (`reftests.md` § Reference independence #3) is specified but not
  built; single-reference reftests cover the current pairings. (Follow-ups-drain
  reviewed and left deferred — re-open when a real logical-vs-physical, ≥2-reference
  pairing appears; building it now is tested-but-unused machinery.)
  *(Re-verified 2026-07-01 by the verification-followups campaign — still no
  ≥2-reference pairing exists to consume it, so building it now would ship
  tested-but-unused machinery in the highest-risk tier; DEFERRED.)*
- **`golden-prune` bin** — the advisory stale-positive pruner (`goldens.md`
  § Stale-positive guard) is a design hook, machinery deferred. (Follow-ups-drain
  reviewed and left deferred — re-open when the golden corpus grows enough that
  stale positives are plausible; nothing to prune at the current few cells.)
  *(Re-verified 2026-07-01 by the verification-followups campaign — still only 2
  single-positive cells, so the bin would always print "nothing to prune";
  DEFERRED.)*
- **Golden triage-report ignore primitives (time-boxed-ignore + flaky-auto-ignore)**
  — recorded here to close the Task-4.7 tracking gap (2026-07-01). time-boxed-ignore
  = a `BlessLedger` `[[ignore]]` table (slug/glob + RFC3339 `expires` + reason)
  consulted in `check_golden` to soft-skip an unexpired cell; flaky-auto-ignore = an
  Argos-style min-occurrences heuristic needing per-cell CI failure-history
  persistence. Both DEFERRED until the corpus grows (see `goldens.md`
  § deferred primitives).
- **Shaping-corpus breadth (pure-Hebrew / VS16 / Thai / Khmer fixtures)** —
  named in audit #38 (T4.6), deliberately DEFERRED rather than landed in the
  testing-audit campaign. The shaping corpus is a curated, byte-reproducible
  artifact: every fixture font is a `pyftsubset` subset of a SHA256-pinned
  upstream produced ONLY by `tools/fonts/subset_fixture_fonts.sh`, which hard-pins
  `fontTools==4.56.0`. Adding new script arms is a font-acquisition task (new
  pinned OFL upstreams + digests + provenance) run through that pipeline, not a
  low/info test cleanup — and the dev box has fontTools 4.63.0, so any subset
  produced here would diverge byte-wise and break the corpus's reproducibility
  guarantee. Existing fixtures already cover the shaper behaviors (Hebrew RTL via
  `mixed_bidi`; Arabic joining; Devanagari reordering; CJK; emoji-ZWJ ligation).
  Land the new arms when a fontTools-4.56.0 environment + the pinned upstreams are
  available; verify via `BUIY_ACCEPT_SHAPING=1` + a reviewed `.snap` diff.
- **Object-store golden migration** — in-git PNGs until the named trigger
  (>50 MB total or >500 positives); the `GoldenKey`/`BlessLedger` schema is fixed
  now so the migration is mechanical (`goldens.md` § Storage staging).
  *(Re-verified 2026-07-01 by the verification-followups campaign — the trigger is
  UNMET: the corpus is 590 B / 2 positives on origin/main; confirmed still
  deferred.)*
- **Invariant generator — `PositionKind` (tier-2 positioned/auto-z) coverage — RESOLVED** —
  `invariant/scene.rs`'s `SceneNode` carries no `PositionKind`, so the production
  paint `paint_key`'s tier-2 *(positioned, auto-z)* class is unrepresentable and
  never exercised by the metamorphic suite. On the generated domain
  `positioned ⟺ z_index.is_some()` so the realized order still matches production
  there; closing the gap means adding a `PositionKind` axis to the generator.
  Surfaced by the 2026-06-15 fresh-agent quality review (scene.rs module doc
  records the bound). **RESOLVED 2026-07-01 (verification-followups campaign):**
  landed 4c1acbb; `invariant/scene.rs` carries the `PositionKind` axis
  (`position_kind` + `arb_position_kind`, testing-audit #13) exercising all four
  paint tiers.
- **Invariant `realize` mirrors the painters_z assembly instead of calling it — RESOLVED** —
  `invariant/scene.rs`'s `realize` re-implements layout sub-pass 6f (the per-
  context `painters_z` z-tier sort) and passes its OWN `painters_z_of` into the
  production `context_tree_paint_order`. **Empirically confirmed (2026-06-15
  fault-injection):** reversing the production 6f sort (`layout/systems.rs` z-tier
  `sort_by_cached_key`) was NOT caught by the Tier-3 invariant suite — only by
  buiy_core's own `z_index_*` unit tests (`static_z_index_paints_in_document_order`,
  `z_index_ordering_neg_zero_pos`). So the metamorphic tier verifies a *parallel
  copy* of 6f, not the real assembly; a 6f-only regression relies on buiy_core's
  z-index tests + the GPU golden tier. Harden by having `realize` CALL the
  production `painters_z` assembly (extract it to a pure fn) so the invariant
  exercises the real code path. The CPU display-list helper
  (`snapshot::extract_nodes_from_world`) likewise re-sorts by `Name`, so it does
  not observe production paint order either — Tier-2 paint-order coverage is the
  GPU golden tier's job. (The `Name`-sort is correct for the *dump's* determinism;
  the point is only that Tier-2's CPU dump is not a paint-order oracle.)
  **RESOLVED 2026-07-01 (verification-followups campaign):** `invariant/scene.rs:446`'s
  `realize` now CALLS the shared production assembly
  `buiy_core::layout::painters_z_for_context` (extracted as a pure fn) instead of
  re-implementing sub-pass 6f, so the metamorphic tier exercises the real z-tier
  code path; landed 4c1acbb.
- **Quiescence gate — conditions 2-4 headless coverage — RESOLVED** — `capture_to_image`'s
  `quiescence_unmet` (`buiy_core::render::golden`) checks four conditions; only
  condition 1 (the `PendingCaptureAssets` asset gate) is reachable without a
  render sub-app, and it now has a headless unit test. Conditions 2-4 (atlas
  warmup drained, fonts-resident, no Queued/Creating pipeline) need a hand-built
  render world to unit-test headlessly, so a vacuous-check regression in those
  three is currently caught only by the GPU lane. Extract each probe behind a
  pure helper + unit-test against synthetic resources. Surfaced by the
  2026-06-15 review (its top-3 recommendation). **RESOLVED 2026-07-01
  (verification-followups campaign):** conditions 2-4 are now behind pure helpers
  (`pipelines_compiled`/`fonts_ready`) with headless non-`#[ignore]` unit tests
  against synthetic resources, so a vacuous-check regression is caught on every-PR
  without the GPU lane; commits 52d9194, ae42b96.
- **CPU SDF oracle ↔ shader numeric pin — DONE (DRY half)** — the reftest CPU oracle
  (`reftest.rs`) and `render/shader.wgsl`'s `sdf_rounded_rect` are textual
  twins; the point-probe pins only sign invariants, so a numeric `d`-value drift
  between them is caught only by the GPU cross-check lane. Add a numeric
  `d`-value agreement test (or share one Rust SDF fn across oracle + probe).
  Surfaced by the 2026-06-15 review (`reftests.md` § SDF cross-check).
  **DONE 2026-07-01 (verification-followups campaign, V10):** the Rust DRY half
  shipped — one canonical `buiy_core::render::sdf_rounded_rect` now replaces the 3
  duplicate Rust copies (reftest `sdf_oracle`, `render_instance.rs`,
  `render_border_sdf.rs`). The WGSL↔Rust numeric pin remains the CI lavapipe
  cross-check (unchanged — a DRY nit per the 2026-06-18 audit, not a coverage gap).

**Owner:** `buiy-verification-design` — worth building once the canonical CI
GPU class exists (render `verification.md § 4.1`); tolerance budgets are that
design's numbers, never this backlog's.

**Spec touchpoint:** `buiy-verification-design` (`goldens.md`, `determinism.md`,
`metric.md`, `reftests.md`, `coverage.md`); render `verification.md § 4.1`; text
`verification.md § 4` (as-landed note).

## Text editing — BiDi split caret (E3 deferral)

**Status: LANDED** (follow-up slice after E3–E6; `followups-drain` worktree).
Superseded the E3 deferral. Was: deferred from E3
([2026-06-13-buiy-text-editing-e3-caret-selection.md](2026-06-13-buiy-text-editing-e3-caret-selection.md)).

**What it is:** when the caret sits on a bidirectional direction boundary, the
spec (editing-and-ime.md §§ 4.1, 5) calls for **two** caret marks — a primary
full-height bar at one run's edge plus a secondary indicator at the other — so
the user can tell which direction the next typed character will flow.

**As landed.** The secondary indicator rides
`CaretVisual.secondary: Option<Rect>` (a FIELD, not a standalone component — the
extract producer's query/`Changed`/`RemovedComponents` params are at Bevy's
15-tuple cap, so the field reuses the primary's damage trigger and clear).
`secondary_caret_rect_for(buffer, caret)` (caret.rs) returns `Some` only at a
direction boundary — where a BEFORE glyph (`end == index`) and an AFTER glyph
(`start == index`) abut with OPPOSITE `level.is_rtl()`. The secondary x is the
**BEFORE glyph's logical-end visual edge**: LTR → `x + w`, RTL → `x` (cosmic's
own convention, `buffer.rs:120-142` / `cursor_from_glyph_right`). It is
top-anchored at `SECONDARY_CARET_H_FRAC` (0.5) of the line box — a shorter mark
than the full-height primary. The paint is a SECOND solid-stamp instance in
extract.rs (CPU geometry only, reusing the primary's atlas entry/color/clip/page
— no new GPU, no new atlas insert). The writer compares the `(rect, secondary)`
pair so a boundary crossing that changes only the secondary still re-emits.

**Why a field, not a new component.** The §6.3-style primary `CaretVisual` is
already the seat; the cap on the extract producer's tuples (extract.rs § 6.1)
makes a 16th seat impossible without a risky refactor of the hottest text
system. Recorded in the §5 spec update so a future "cleanup" to a standalone
component is not silently attempted.

**The cosmic gotcha (why E3 couldn't surface it).** `cursor_glyph`
(`buffer.rs:151-174`) is affinity-blind AND order-defined — it resolves
`index == glyph.start` BEFORE `index == glyph.end`, so its single
`cursor_position` only ever reports the AFTER (start-glyph) edge, which the
PRIMARY already paints. The SECONDARY is the OTHER abutting glyph's logical-end
edge — a glyph-level position cosmic's one cursor call cannot reach (the second
position is glyph-level, not run-level).

**Soft-wrap (multiple runs per logical line).** A logical line that wraps emits
SEVERAL `LayoutRun`s sharing one `line_i` (cosmic 0.19 `LayoutRunIter`: one run
per wrapped `layout_line`; glyph byte indices are line-relative across the
segments). So `secondary_caret_rect_for` mirrors `caret_rect_for`'s all-runs
scan: a `line_i`-matching run that holds neither abutting glyph at the index is
the WRONG wrap segment (or a run extremity) — it CONTINUES rather than concluding
`None`, and the rect uses the OWNING run's `line_top`. Only single-line editors
force `Wrap::None` (sync.rs:521); multi-line wrapping editors and display text
produce multi-run logical lines, so a boundary on a continuation segment must
still surface the secondary.

**Tests.** Headless `text_caret_geometry.rs`: `secondary_caret_rect_for` returns
`None` for a pure-LTR caret and `Some` at a data-derived mixed-BiDi boundary
(asserting the EXACT before-glyph edge x — equality with the primary allowed,
never `primary.x != secondary.x`); a narrow soft-wrapped mixed line surfaces the
secondary on a NON-FIRST run of the logical line (asserting the owning
continuation run's `line_top`); the end-to-end editor caret carries no
secondary on ASCII; `CaretVisual::default().secondary == None`. GPU `#[ignore]`
`text_caret_selection_e3_gpu.rs`: the existing single-band primary golden is
unchanged; an additive boundary golden drives the caret to the data-derived
boundary via logical-order `Motion::Next` and asserts TWO red bands with the
secondary shorter in row-extent.

**Owner:** the text-editing campaign (delivered).

**Spec touchpoint:** editing-and-ime.md §§ 4.1, 5, 13 + abstract (as-landed notes).

## Text editing — multi-range selection *behavior* (E-campaign deferral)

**Status:** deferred from the `buiy-text-editing` campaign (E1–E6;
[campaign plan](2026-06-13-buiy-text-editing-campaign.md)).

**What it is:** the `TextSelection` type is multi-range-**shaped** (`primary` +
`secondary: SmallVec<[…; 2]>`, editing-and-ime § 4.2) and the geometry pipeline,
`SelectionChanged` payload, and `::selection` APIs all carry the shape — but v1
ships single-range **behavior** (`secondary` always empty). Multi-cursor editing
(multiple simultaneous carets/ranges, e.g. Ctrl-click-to-add-caret) is the named
next slice.

**Why deferred:** cosmic-text's `Selection` is structurally single-range;
multi-range behavior is Buiy-layer aggregation over N mirrored ranges + N-caret
input routing — a focused slice, cheap because the type is already shaped (no
reshape needed).

**Owner:** a focused follow-up slice after E1–E6.

**Spec touchpoint:** editing-and-ime.md §§ 4.2, 13 (named deferral).

## Text editing — HTML + image clipboard flavors (E-campaign deferral) — LANDED

**Status:** **Landed.** The `ClipboardProvider` facade gained `get_html`/
`set_html` (always available) and `get_image`/`set_image` over a Buiy-owned
`ClipboardImage { width, height, bytes }` (behind the new `buiy_core`
`clipboard-image` cargo feature, which forwards to arboard's `image-data`).
Both impls carry the new flavors: `MemClipboard` stores an html slot and (gated)
an image slot — the headless-testable path; `ArboardClipboard` delegates to
arboard `Get::html()`/`Set::html()` and (gated) `get_image`/`set_image`,
converting at the borrowed-`ImageData<'a>` boundary. `Cut`/`Copy` now set BOTH
the plain-text flavor and an escaped-html flavor (`escape_html` escapes
`& < > " '`; a plain-text editor has no rich runs, so its html is just the
escaped text). **`Paste` is unchanged** — it takes the § 3.3 newline-strip text
path and never consults the html getter (the getter is for rich-content
callers); a regression test (`paste_prefers_text_and_ignores_html`) pins this.
OQ#3 **resolved**: arboard 3.6.1 `Get::html()`/`Set::html()` are on the
cross-platform builder and not feature-gated (verified against the locked
source); only image needs `image-data`. **Implementing files:**
`crates/buiy_core/src/text/edit/clipboard.rs` (trait + `ClipboardImage` + both
impls), `crates/buiy_core/src/text/edit/input.rs` (Copy/Cut dual-set +
`escape_html`), `crates/buiy_core/src/text/edit/mod.rs` (gated re-export),
`crates/buiy_core/Cargo.toml` (`clipboard-image = ["arboard/image-data"]`).
**Tests:** `crates/buiy_core/tests/text_clipboard_undo.rs` (MemClipboard html
round-trip + independent slots + trait-object; `#[cfg(feature="clipboard-image")]`
image round-trip + independent slots) and
`crates/buiy_core/tests/text_undo_ops.rs` (copy/cut dual-set the escaped-html
flavor; paste-prefers-text-and-ignores-html). **CI note:** the default
`cargo test --workspace` / `clippy --workspace --all-targets` gate runs with
`clipboard-image` **OFF** (the image module compiles out); the gated lane is
`cargo test -p buiy_core --features clipboard-image` (and a matching clippy run)
— add it to CI so the image path cannot rot silently. No new GPU, no new event
surface; arboard real-OS clipboard is not headless-testable, so only MemClipboard
is asserted (matching the E4 decision). The original deferral context is
preserved below.

**Originally deferred** from the `buiy-text-editing` campaign (E4 shipped plain
text).

**What it was:** the F row names text + HTML + image MIME for cut/copy/paste; E4
shipped **plain text only** (`Cut`/`Copy` via `copy_selection`, `Paste` through
the § 3.3 newline policy) behind the `ClipboardProvider` facade. HTML + image
flavors were the named next slice.

**Why it was deferred:** arboard's HTML *read-side* support was unverified
(editing-and-ime OQ#3) and had to be confirmed before the slice was promised; the
facade made adding flavors local (no API churn).

**Spec touchpoint:** editing-and-ime.md §§ 7, 13 (named deferral); OQ#3.

## Text editing — compose-over-selection (E5 deferral) — LANDED

**Status:** **Landed.** When a composition starts over a non-collapsed
selection, the selection is now deleted first (replace-selection convention) and
the preedit is spliced at the now-collapsed caret. The selection-delete is
captured as a reversible cosmic `Change`, **stashed** on the new
`TextEditState::compose_delete` field (not pushed onto the undo stack — invariant
(a) still holds for the splice), and at `Ime::Commit` it is **folded into the
same `GroupKind::Composition` undo unit** as the commit-insert
(`caret_before`/`selection_before` captured pre-delete): one undo restores BOTH
the deleted text and the committed text, one redo replays both. A **cancel**
(empty `Preedit` / `Ime::Disabled` / `Escape`) reverse-applies the stash,
re-inserting the deleted text and restoring the selection. `TextChanged` fires on
the delete (a genuine value change — the one documented exception to "never
preedit") and again on the cancel-restore. The plain-text unselected-caret path
is byte-identical to E5. *Approach rejected:* recording the delete as its own
Composition unit and coalescing the commit into it — `Composition` never
coalesces (§ 6.2c) and the intervening preedit splice breaks caret-adjacency, so
it would yield two units. **Implementing files:** `crates/buiy_core/src/text/edit/ime.rs`,
`crates/buiy_core/src/text/edit/state.rs` (`ComposeDelete` + `compose_delete`
field), with the keyboard Escape value-change routed through
`crates/buiy_core/src/text/edit/input.rs`. **Tests:**
`crates/buiy_core/tests/text_ime_ops.rs` (splice-deletes-and-stashes,
commit-is-one-unit + one-undo-restores-both + redo, cancel-restores-via-remove) and
`crates/buiy_core/tests/text_ime_system.rs` (delete-fires-TextChanged,
cancel/Disabled-restores + re-fires-TextChanged, commit-one-unit). Spec
editing-and-ime.md §§ 6.1 / 6.2 / 13 updated (deferral reversed). No new GPU, no
new event surface. The original deferral context is preserved below.

**Originally deferred** from the `buiy-text-editing` campaign (E5 IME;
[E5 plan](2026-06-13-buiy-text-editing-e5-ime.md)).

**What it is:** when text is selected and the user starts an IME composition,
the platform/web convention is to **replace the selection** with the preedit
(the selection is deleted, composition begins at the caret). E5's
`splice_preedit` (`text/edit/ime.rs`) splices the preedit at the editor cursor
and does **not** delete an active selection first, so composing over a selection
leaves the selected text in place and inserts the preedit beside it.

**Why deferred:** plain-text IME composition (the F-tier core path — preedit
splice + reflow + the four § 6.2 invariants) is complete and correct for the
unselected-caret case, which is the overwhelmingly common one. Compose-over-
selection needs a `delete_selection` (as one undo unit, paired with the
composition group per § 6.2c) before the first splice plus a re-anchor of the
preedit span — a focused behavioral slice, no new GPU and no new event surface.

**Owner:** a focused follow-up slice after E1–E6.

**Spec touchpoint:** editing-and-ime.md §§ 6.1, 6.2, 13.

## BSN / Bevy 0.19 — rc.3 → 0.19.0 stable bump (closes the rc-pin exception)

**Originated:** the Bevy 0.19-rc + BSN migration
([plan](2026-06-18-bevy-0.19-bsn-migration.md);
[spec](../specs/2026-06-18-buiy-bsn-integration-design.md) § 2).

**Status:** deferred — gated on the upstream 0.19.0 **stable** release.

**What it is:** Buiy pins `bevy 0.19.0-rc.3` (with `bevy_scene`) to reach
`bsn!` authoring — a deliberate, scoped exception to the foundation's
"rolling latest-stable Bevy" policy (architecture.md § 2.9). When Bevy
0.19.0 stable releases, Buiy bumps to it and the exception **closes**.

**Why deferred:** the BSN baseline (PR #23413) ships only in the 0.19
line, and 0.19 has no stable tag yet. The rc is API-frozen enough that
the rc.3→stable delta is expected to be small (a likely-mechanical
version bump + a re-resolve + `cargo deny check`), but it must be done
when stable lands. Watch for any BSN / render-graph API churn between
rc.3 and stable.

**Implementation sketch:** bump the `bevy` pin in the workspace
`Cargo.toml` to `0.19.0`, regenerate the on-disk lock, verify single
resolved versions (`cargo tree -i`), re-run `cargo deny check`, then run
both gates (headless + the GPU `--ignored` lane). Remove the rc-exception
note from foundation architecture.md § 2.9 and the
`2026-06-18-buiy-bsn-integration-design.md` § 2 callout once closed.

**Owner:** a focused version-bump slice when 0.19.0 stable releases.

**Spec touchpoint:** `2026-06-18-buiy-bsn-integration-design.md` § 2, § 7;
foundation `architecture.md` § 2.9.

## BSN — `.bsn` asset-file loader + component hot-reload (await upstream loader)

**Originated:** the BSN integration design
([spec](../specs/2026-06-18-buiy-bsn-integration-design.md) § 4.4, § 7).

**Status:** deferred — blocked on the upstream `.bsn` asset-file loader.

**What it is:** Buiy targets **inline `bsn!`** (and function / `SceneList`
scenes) only. The `.bsn` **asset-file** form
(`asset_server.load("x.bsn")`) and component hot-reload via it are not in
scope for the initial BSN work.

**Why deferred:** the `.bsn` asset-file loader was explicitly deferred out
of Bevy's BSN baseline (PR #23413) to a future upstream PR — it has no
runtime backing in `0.19.0-rc.3`. Component hot-reload depends on that
loader, so it defers with it. The reflection registry that the loader (and
the editor/inspector) will consume is already maintained by Buiy's
per-crate `register_type` plugins (spec § 4.3) — no reflect work is
blocked, only the consumer.

**Implementation sketch:** when the upstream `.bsn` loader lands, wire it
through Buiy's asset pipeline (`buiy-asset-pipeline-design`), add a `.bsn`
asset-load + hot-reload path, and verify the registered Buiy components
resolve from a `.bsn` file. Until then, inline `bsn!` is the authoring
surface.

**Owner:** `buiy-asset-pipeline-design` (the still-unwritten asset-pipeline
sub-spec), gated on the upstream loader.

**Spec touchpoint:** `2026-06-18-buiy-bsn-integration-design.md` § 4.4, § 7;
foundation README § 5 ("hot-reload of components"); bevy-ui prior-art
`open-problems.md` § "Hot-reload of components".

## BSN — widget scene-fn coverage as the widget catalog grows

**Originated:** the BSN integration design
([spec](../specs/2026-06-18-buiy-bsn-integration-design.md) § 4.1c) + Phase 4
([plan](2026-06-18-bevy-0.19-bsn-migration.md) T4.3/T4.4).

**Status:** incremental — extend per widget as the catalog grows.

**What it is:** Buiy ships parameterized **widget scene-fns** in
`buiy_widgets` (`button(label) -> impl Scene`, `text_input_single_line`,
`text_input_multi_line`) that spell a widget's styling as explicit `bsn!`
field-patches so `bsn! { button("Save") BoxModel { … } }` merges
field-wise and keeps the widget's other canonical defaults (the § 4.1c
require-suppression remedy). Today's catalog is `Button` + `TextInput`.

**Why incremental:** every new widget added to `buiy_widgets` needs its own
`#[require]` contract **and** a matching scene-fn (reusing the same private
require-initializer fns as the one source of truth, so `bsn!{ widget() }`
stays byte-equal to `spawn(Widget)`). This is mechanical per-widget work
that lands with each widget, not a one-shot task.

**Implementation sketch:** when a widget is added under
`buiy-widget-catalog-design`, add its `#[require(...)]` markers and a
`buiy_widgets` scene-fn re-exported through `buiy::prelude`, and extend the
round-trip test's styled-widget cases (§ 5) to cover it.

**Owner:** `buiy-widget-catalog-design`, per widget.

**Spec touchpoint:** `2026-06-18-buiy-bsn-integration-design.md` § 4.1a,
§ 4.1c, § 5.

## BSN — round-trip test full-`BoxModel`-equality robustness (optional nicety)

**Originated:** the BSN integration Phase 4 round-trip test
([plan](2026-06-18-bevy-0.19-bsn-migration.md) T4.5;
[spec](../specs/2026-06-18-buiy-bsn-integration-design.md) § 5).

**Status:** optional hardening — not blocking; current assertions are
correct.

**What it is:** the round-trip authorability test asserts patched field
values on the resulting entity components. A nicety would assert the
**full** `BoxModel` (every field) equals the expected materialized value
after a scene-fn merge — not only the patched fields — to pin the § 4.1c
field-wise-merge behavior end-to-end (patched field changed, all others
retain the widget's canonical defaults) against any future drift in the
require-initializers or scene-fn bodies.

**Why deferred:** the current per-field assertions already cover the
load-bearing cases (the patch applied; the suppression gotcha; the scene-fn
merge keeps padding). A full-struct equality assertion is stricter
regression insurance, not a correctness gap.

**Implementation sketch:** in the styled-widget round-trip case, construct
the expected `BoxModel` explicitly (widget defaults with the one patched
field overridden) and assert struct equality, rather than spot-checking
individual fields.

**Owner:** a focused test-hardening slice (low priority).

**Spec touchpoint:** `2026-06-18-buiy-bsn-integration-design.md` § 4.1c, § 5.

## Verify — `rect-rounded` lavapipe golden re-bless on CI Mesa after the wgpu29/naga29 bump — RESOLVED (no re-bless needed)

**RESOLVED 2026-07-01 (verification-followups campaign):** the golden passes EXACT
on the pinned CI lavapipe post-wgpu29 (blessed `b869eba`, byte-current) — no
re-bless is needed. The standing "watch at the next toolchain bump" concern is
owned by the sibling "0.19 lavapipe pixel-residue recalibration" note below.

**Originated:** the Bevy 0.19-rc migration final gate
([plan](2026-06-18-bevy-0.19-bsn-migration.md) Phase 3 / T3.2).

**Status:** CI action item — NOT a regression; do not bless on a dev host.

**What it is:** the zero-tolerance `buiy_verify` golden
`rect-rounded/default/dark__sm__fc0__lavapipe__dpr1` (`tests/goldens.rs ::
golden_sdf_corner`) diverges by `max_channel_delta=35` (0 differing pixels,
`mssim=0.9995`) when run on a dev host. The baseline was blessed on **CI
pinned lavapipe Mesa 24.3.4**; dev hosts run a different Mesa (e.g. 26.0.2),
and the render path also crossed **naga 27 → 29** — both produce sub-pixel
AA deltas at the curved SDF corner on the lavapipe *software* rasterizer.
This is the ONLY golden that diverges; the Button reftest, all `buiy_core`
self-validating GPU goldens (the project's actual dev GPU lane), and the
other `buiy_verify` goldens all pass. It is a structural no-op (mssim
0.9995), not a paint regression.

**Why not blessed here:** re-blessing on a dev host would commit non-CI-Mesa
output and break the CI gate. The CI-pinned baseline is the authority.

**Action:** on the next CI run on the pinned lavapipe (Mesa 24.3.4), if
`golden_sdf_corner` diverges from the wgpu29/naga29 toolchain (vs. the
pre-bump baseline), review the triage diff (`target/buiy-goldens/report.html`)
to confirm it is corner-AA-only, then re-bless with
`BUIY_BLESS=1 cargo test -p buiy_verify --test verify_gpu -- --ignored
--test-threads=1 goldens` and commit. The dev-host divergence above is expected and
not the signal — only the CI-Mesa result is.

**Owner:** the CI maintainer at the 0.19 toolchain bump.

**Spec touchpoint:** `2026-06-18-buiy-bsn-integration-design.md` § 7.

## Testing-audit × 0.19 merge — cosmetic test residue (post-reconciliation)

Two zero-impact cosmetic items the reconciliation review surfaced when the
testing-audit branch merged with main's Bevy 0.19 + affine-paint work. Neither
affects correctness or either gate (headless 1324/84 green, GPU 83/0); both are
follow-up cleanups.

- **`render_smoke.rs` stale test name** —
  `clip_aabb_pipeline_registers_with_stride_52` (and two "stride-52" comments)
  predate main's affine layout, which lifted the per-instance stride to **68**
  (`primitive.rs array_stride: 68`, `instance.rs PACKED_INSTANCE_STRIDE_BYTES=68`).
  Inherited verbatim from main's affine commit (cf554b9) — the test body asserts
  no stride (`let _ = pipeline.id;`), so nothing passes wrongly. Rename to
  `..._with_stride_68` + fix the comments.
- **insta `source:` headers stale after the Phase-5 consolidation** — the
  relocated snapshots under `crates/buiy_core/tests/render/snapshots/`
  (`pack_instance_logical_px` + siblings) still carry pre-move `source:` paths
  (e.g. `tests/render_instance.rs`). insta matches by snapshot name + module path
  (not the header), and the bodies are correct (they took main's affine-blessed
  form), so the suite is green. Regenerate to refresh the headers.

## 0.19 lavapipe pixel-residue recalibration (LANDED) — re-verify at the next toolchain bump

The wgpu27→29 (Bevy 0.18→0.19) bump shifted one lavapipe pixel-residue fact the
GPU lane pins: the floored 2-physical-px **underline** band
(`text_decoration_gpu::underline_quad_band_residue_on_pinned_lavapipe`, was
`..._has_the_antialiased_quad_signature`). Under wgpu27 the pinned lavapipe read
it at AA alpha 0.84375 (≈237, no full-coverage row); wgpu29 pixel-aligns the 2px
band so both rows read FULL coverage (255) — confirmed against the pinned Mesa
24.3.4 lavapipe (the same artifact CI uses). The assertion was recalibrated to the
solid 2-row residue. **NOT a regression** — the band rasterizes correctly (2px,
solid red, deterministic; the band-count + re-capture-determinism legs are
unchanged). The other lavapipe-pinned residues did NOT drift: `golden_sdf_corner`
and the blessed coverage cells (rect-rounded, text-ahem) still pass EXACT on
wgpu29 lavapipe (the full 83-test GPU lane is green on the pinned rasterizer).

**Forward-looking:** lavapipe pixel residues are rasterizer+toolchain-pinned
(determinism.md). At the NEXT wgpu/Mesa bump, re-run the full `--profile gpu` lane
on the pinned lavapipe and recalibrate/re-bless whatever shifts. A dev host can
reproduce the canonical rasterizer USER-SPACE (no sudo): download the
`install-mesa`-pinned Mesa tarball + a matching `libLLVM`/`libxml2`/`icu` (e.g.
from the Arch archive) onto `LD_LIBRARY_PATH`, write an ICD JSON pointing at
`libvulkan_lvp.so`, and set `VK_DRIVER_FILES` + `WGPU_ADAPTER_NAME=llvmpipe`.

## Verify (C7) — catalog-wide `content_is_present` enroll auto-check — LANDED (2026-07-01)

**Originated:** widget-catalog C7 (verification real-input tier + content-presence),
`docs/plans/2026-06-22-buiy-widget-catalog-c7-verification.md` Task 1. The
`content_is_present` invariant + golden bless-guard landed (RED-first proof via a
whitespace-only zero-glyph fixture); the predicate runs the production
`extract_buiy_glyphs` adapterless and asserts a text-bearing scene emits >0 glyph
instances.

**Deferred:** a catalog-wide `enroll_all`-driven auto-check that EVERY text-bearing
fixture cell satisfies `content_is_present`. Blocked on **(a)** a text fixture in
the catalog (Wave 1's only fixture is the non-text `button`) and **(b)** a
text-capable enroll stack — `coverage::enroll::build_app` is
`MinimalPlugins + CorePlugin + LayoutPlugin` only, with NO `BuiyTextPlugin`, so
`glyph_census` would panic on the missing `SharedFontSystem` on every enrolled cell.
Until both land (C8 adds a text fixture; the enroll stack gains a text-capable
variant), the predicate's teeth are gated by the two dedicated full-stack unit tests
in `crates/buiy_verify/tests/verify_headless/content_presence.rs` (the whitespace
zero-glyph RED + the "Hi!" GREEN) and the `bless_guard_refuses_zero_glyph_text_bearing`
unit test. Pick this up when C8's text-bearing gallery fixtures land.

**LANDED (verification-followups campaign, 2026-07-01):** `coverage::enroll::build_app`
is now text-capable (`BuiyTextPlugin { system_fonts: false }` + a staged Ahem face →
`SharedFontSystem` present), so `glyph_census` no longer panics on enrolled cells. The
catalog-wide `every_text_bearing_catalog_cell_emits_glyphs` test asserts >0 glyph
instances for every text-bearing cell, guarded by a `text_bearing > 0` vacuity check
(RED-first proven).

## Widget-catalog × agent-interface co-drive — post-landing follow-ups (2026-06-23)

The co-drive campaign (Waves 0–5: P1a/P1b a11y substrate, P1c inspection driver,
C1/C2 correctness, C3 Pointer<E>, C4 widget visuals, C5 containers, C6 F-tier
styling, C8 gallery) landed on the integration branch (`worktree-todomvc-reimpl-research2`,
not yet PR'd). Demand-pulled deferrals are tracked in the coordination plan's ledger
(`docs/plans/2026-06-22-widget-catalog-agent-interface-codrive.md` §3.2). Net-new
follow-ups surfaced during implementation:

- **Idle-CPU / per-frame scan at scale (corroborates the prototype audit's ~50%
  idle-CPU concern).** C8-b's 1000-row S2 screen burns ~22 ms/idle-frame (debug)
  with zero input/state change — an all-entities-per-frame scan in the
  `CorePlugin + LayoutPlugin + Text` path — plus ~6 ms from `A11yPlugin::build_tree`
  rebuilding the whole tree each frame. Candidates: layout/reshape change-detection
  gating, and the agent-interface follow-up "lazy `TreeUpdate` diffing gated on
  `AccessibilityRequested`" (its phasing.md follow-up #4). Not a correctness issue;
  pick up as a perf pass.
- **Gallery authoring guide + matrix enrollment.** `examples/buiy_gallery` has the 5
  screens + per-screen layout snapshots + the inspection-driver acceptance, but the
  spec's `Matrix::gallery_screen()` reduced-matrix enrollment + an `AUTHORING.md`
  (how to add a screen) are not yet written. The coverage `enroll::build_app` still
  lacks a text-capable stack (the deferred auto-check above) — C8 added text fixtures
  (S1) but via full `A11yPlugin` apps, not the coverage matrix.
  **Re-verified 2026-07-01 (verification-followups campaign) — DEFERRED to C8 as its
  rightful owner:** `Matrix::gallery_screen()` + gallery-as-fixtures + `AUTHORING.md`
  are *designed C8 deliverables* (`widget-gallery-exemplar.md` §3.3/§3.5/§4/step 7);
  buildable now, but the scope/ownership belongs to the widget-catalog campaign, not
  verification. The one verification-native piece — a text-capable `enroll::build_app`
  — WAS delivered by this campaign (V13/V14), so the "still lacks a text-capable stack"
  clause above is now superseded.
- **Text-caret/selection GPU ink predicates adapter-brittle — RESOLVED (2026-07-01, V19).**
  The five `>=180`-family ink predicates (`is_white_ink`×3, `is_blue_ink`,
  `is_strong_blue`) across `text_caret_selection_e3_gpu` / `text_selection_caret_gpu` /
  `text_ime_preedit_gpu` assumed pinned-lavapipe's coverage/gamma, so a dimmer
  rasterizer could match no pixels and panic `.expect("the glyph ink painted")`.
  Replaced the HIGH "channel is lit" checks with a shared `crate::support::channel_lit`
  (`>= 128`, background-relative to the opaque-black clear), keeping each predicate's
  LOW bounds for per-color discrimination (white = all channels lit; blue = blue lit,
  red not). Verified 6/6 on BOTH the RX 6700 XT (RADV) and pinned Mesa 24.3.4 lavapipe.
  (Note: the older RX panic no longer reproduced on this driver — portability-hardening
  against a latent cross-adapter flake, not a live-bug fix.)
- **Doc-status flip on merge.** The widget-catalog child specs (C1–C8) are still
  `[draft]`; flip to `[landed]` when the campaign PRs to `main` (they describe the
  now-built target state).
- **Deferred-by-ledger (not regressions), for completeness:** `EditCommand::SetSelection`
  + `SetTextSelection`/`ReplaceSelectedText`; the actionability gates
  (`act_when_actionable`/`HitTargetable`/`Stable`) — the stacking-aware `hit_test`
  they'd consume IS built (C1+C3), so un-deferring reads a real hit_test, no AABB
  shim; `MultilineTextInput`/`AlertDialog`/multi-thumb-slider/Accordion variants;
  `owns` re-parent + `TreeView::Merged` + the #12 proptest fuzz corpus; P2/`buiy_mcp`.

## Widget-catalog live-run fixes (2026-06-24) — remaining minor items

See `docs/reports/2026-06-24-widget-catalog-rendering-and-crash-bugs.md` for the
two bugs (the editor-coherence crash + the invisible-content rendering bug) and
their fixes. Remaining polish (none blocking; the gallery renders + runs):

- **Symbol glyphs render as tofu (latin-subset font).** The checkbox check `✓`
  (U+2713) and the disclosure caret `▸` (U+25B8) are absent from the embedded
  `FiraSans-latin` subset, so they render as `.notdef` boxes. Options: embed a
  fuller glyph (a Symbols/Dingbats subset), draw the check/caret as a primitive
  (a rotated border, like real TodoMVC), or enable opt-in system fonts
  (`BuiyTextPlugin { system_fonts: true }` — the async scan path is now
  coherence-safe under `reshape_edited_editors`).
- **S3 overlay closed-state positioning.** Closed/anchored overlays (the popover,
  the closed menu) overlap in a single captured frame — the pre-existing
  `painters_z` single-frame-ordering fragility (resolves over live frames). The
  `overlay_menu_screen.snap` is sensitive to unrelated `Update`-set additions for
  the same reason; pin the popover/menu positioning explicitly after
  layout+transform so the single-frame result is deterministic.
- **`Disclosure` restructured — LANDED (this change).** The disclosure trigger now
  lays its `[caret, label]` out as a centered flex-row header (`Display::flex_row`
  + `disclosure_row_flex`), with `Position::Relative` on the trigger and the
  controlled `DisclosurePanel` as `Position::Absolute` (inset top=24/left=0) so it
  drops BELOW the header out of flow (collapsed ⇒ a clean header row; the panel
  reveals below). Caret + panel stay direct children so `update_disclosure_visual`
  /`wire_disclosure_controls` are unchanged. Verified in `showcase_screen.snap`
  (caret `pos=0,2`, label `pos=17,2`, panel `pos=0,24`). Residual: the caret glyph
  `▸` (U+25B8) is tofu under the latin-subset font — same font-coverage item as the
  `✓` checkmark above.
- **Verify `button` coverage fixture is now content-width-empty — RESOLVED.** With the
  content-width button default, the `button.resting.*` CPU coverage fixture spawns
  a tiny empty box (no label); give the fixture a label so it remains a meaningful
  visual sample. **RESOLVED (verification-followups campaign, 2026-07-01):**
  `build_app` now stages the Ahem face, so the fixture's "Save" label measures 34×20
  (was 0×0) and the button is a meaningful visual sample; the 12 `button.resting.*`
  snaps were reblessed (geometry only).
- **Fully CI-enforce the WebGPU shader-conformance gate (`web-smoke`).** The
  `web-smoke` job gates the wasm BUILD always, but the shader-conformance / paint
  check (the only thing that catches the WGSL-uniformity class via real Tint —
  spec `2026-06-25-buiy-wasm-browser-support-design.md` § 4 / D2) **skips** on the
  GPU-less hosted runner: software WebGPU is unavailable there (Dawn exposes no
  adapter over lavapipe, and headless SwiftShader yields none — every flag
  combination gives `requestAdapter() -> null`; naga is too lenient to catch it
  without a real Tint compile). It IS enforced on a WebGPU-capable host (the dev
  GPU lane machine). Two ways to make it CI-gating: (a) run buiy's WGSL through the
  **`tint` CLI** (Dawn's standalone validator — catches the uniformity error with
  NO adapter; needs packaging a prebuilt/built `tint` into CI), or (b) a
  **self-hosted GPU runner** (the project already runs the GPU lane on real
  hardware locally). Until then the render dimension stays manual-release-gate per
  foundation § 2.9.
- **Touch taps (and same-frame synthetic clicks) miss the first press.** The
  picking pipeline has a documented one-frame hover lag
  (`crates/buiy_core/src/picking/backend.rs` § 3.3): `emit_picks` runs in `PreUpdate`
  and the hover map updates a frame behind, so a press arriving with **no prior
  settled hover** lands before the hovered target is known and produces no
  `Pointer<Click>`. A desktop mouse always hovers first (continuous `CursorMoved`
  before the click), so this is **invisible for v1 web (mouse)** — empirically
  verified: `gallery_web` is fully interactive when driven move→settle→click, but a
  cold same-frame click misses. It bites **touch** (a tap has no prior hover —
  deferred with the mobile/touch work, spec § D9) and any synthetic test that clicks
  without a settled move. Fix when touch lands: resolve the press's own-frame pointer
  location (seed the hit-test / a hover for the press's location in the same frame).
  **Not web-specific** — identical on native; surfaced by the wasm interactivity
  verification. **Spec touchpoint:**
  `2026-06-25-buiy-wasm-browser-support-design.md` § 6 (Pointer interactivity — touch caveat).

## Widgets — default `Switch` track never recolors on toggle — LANDED

**Status:** **Landed** (2026-07-02) — `update_switch_visual` now recolors the
`SwitchTrack` fill (`Accent` on / `SurfaceRaisedAlt` off) and the resting/thumb
initializers were aligned to the design (white thumb). Decided in
`docs/specs/2026-07-02-default-switch-track-and-menu-active-highlight-design.md`.

**Originated:** the 2026-07-01 gallery widget-catalog audit
(`docs/reports/2026-07-01-gallery-widget-catalog-audit.md`, finding N1).

**Symptom:** the framework `Switch` widget's visual driver
(`crates/buiy_widgets/src/switch.rs` `update_switch_visual`) slides only the
`SwitchThumb` `Translate`; the `SwitchTrack` fill is a static
`color.surface.secondary` (`switch.rs` ~124) that is never recolored. So a default
`Switch` distinguishes on/off by thumb position only — the modal register switch
(`examples/buiy_gallery` `#ModalRegisterSwitch`, a default `Switch`) does not turn
its track accent-on the way the widget-catalog design shows (`swOn ? accent : …`).
The **showcase** switches look right only because they use a custom track +
`drive_showcase_switches`, which does recolor.

**Direction:** give `update_switch_visual` an on/off track-fill token (accent-tinted
when on, `surface.secondary` when off), guarded with `set_if_neq`. Framework-level
(affects every default `Switch`), so decide deliberately rather than bundling into a
gallery fix — an opinionated default-widget change with a wide blast radius. Not a
regression (thumb still conveys state).

## Widgets — menu items have no active-descendant highlight (roving focus is invisible) — LANDED

**Status:** **Landed** (2026-07-02) — `bind_menu_model` now rings the
`MenuModel.active` item inline with a framework focus-ring `Outline` (the
`MenuActiveRing` marker keeps it from clobbering an author outline), meeting SC 1.4.11
(a background fill can't — no surface token reaches 3:1 on the near-black menu panel).
Painted inline in the bind rather than as a sibling system, because adding an `Update`
system perturbed a schedule-fragile hidden-node layout under the MT executor. Decided
in `docs/specs/2026-07-02-default-switch-track-and-menu-active-highlight-design.md`.

**Originated:** the 2026-07-01 gallery widget-catalog audit
(`docs/reports/2026-07-01-gallery-widget-catalog-audit.md`, finding N2).

**Symptom:** arrow-key roving moves `MenuModel.active` and the aria
`active_descendant`, but nothing paints a highlight on the active item
(`crates/buiy_widgets/src/menu.rs` ~229: "item highlight is a C6 paint concern, not
built here"; gallery `build_menu_item` gives every item a static
`color.surface.transparent` bg). Design-faithful today (the reference renders items
flat), but Buiy's real keyboard roving has no visible feedback — an
expected-but-unwired highlight (accessibility/UX gap, not a design regression).

**Direction:** a C6-style active-item paint — `bind_menu_model` (or a gallery
reflect) tints the `MenuModel.active` item (e.g. `surface.raised-alt`) and clears the
rest, on `Changed<MenuModel>`. Low severity; pairs with a keyboard-nav visual pass.
**Landed as** a focus-ring `Outline` rather than a fill (see Status above).

## Widgets — `menu_item_background()` doc/code contradiction

**Originated:** the 2026-07-02 audit of N2
(`docs/specs/2026-07-02-default-switch-track-and-menu-active-highlight-design.md`).

**Symptom:** `crates/buiy_widgets/src/menu.rs` `menu_item_background()` returns
`ColorToken::SurfacePrimary` while its doc comment says "transparent — the menu panel
shows through". Visually moot today (the default `menu_background()` is also
`SurfacePrimary`, so a default item is the same color as the panel), but the code
contradicts its own contract. N2 landed as an `Outline` (never touches item
`Background`), so this was left untouched.

**Direction:** change `menu_item_background()` → `ColorToken::Transparent` to match
its doc (pixel-identical on the default panel; the gallery already sets items
`Transparent`). One-line cleanup; confirm no default-`Menu` display-list snapshot
counts the item fill quad first.

## Widgets — disabled `Switch` would still recolor its track

**Originated:** the 2026-07-02 N1 fix.

**Symptom:** `update_switch_visual` is gated purely on `Changed<A11yToggled>` with no
disabled check, so if a disabled `Switch` state lands later, toggling it would still
recolor the track to `Accent`. No disabled `Switch` state exists today, so this is
latent, not a live bug.

**Direction:** when a disabled `Switch` visual lands, give the disabled state a muted
track/thumb and skip the accent recolor (or resolve via the theme's disabled tokens).

## Widgets — open-menu container ring vs active-item ring (possible double-ring)

**Originated:** the 2026-07-02 N2 fix.

**Symptom:** when a menu opens, `bind_menu_model` focuses the menu **container**, so
`lower_focus_ring` rings the whole panel; the bind's inline ring pass also rings the
active **item**. Both are correct in isolation but may read as a double-ring. The
2026-07-02 GPU eyeball judged the combination acceptable, so no change was made.

**Direction:** if a keyboard-nav visual pass finds the container ring competes with
the item ring, suppress the container ring for `active_descendant` containers (a menu
that delegates its visible focus to the active item does not also need a panel ring).

## Verify/layout — gallery layout snapshots pin schedule-order-fragile invisible nodes

**Originated:** the 2026-07-02 N2 fix.

**Symptom:** the N2 fix was FIRST implemented as a separate `paint_menu_active_item`
system in `Update`. That flipped the resolved `pos.y` of a **size-0, hidden** tooltip
child node (`InfoTip`'s `entity#…`) from `0,6` to `0,0` in two gallery layout
snapshots — and, decisively, the **single-threaded and multi-threaded executors
disagreed** on the value (single→0,0, MT→0,6), so no snapshot value passed both CI
lanes. The node is invisible (size 0,0 in a hidden subtree), so the shift has zero
visible/functional impact (all visible entities + every `#MenuItem` unchanged; all
behavior + GPU tests green) — the resolved position of a degenerate node is simply
sensitive to the `Update` system SET, which differs by executor, and the layout
snapshots pin those invisible nodes. **Worked around** by folding the ring paint into
the existing `bind_menu_model` (adds no system → schedule unchanged → node keeps its
base position under both executors), but the underlying fragility remains: any future
system addition can re-trigger it.

**Direction:** either (a) make the layout/transform resolve for size-0/hidden nodes
schedule-order-invariant in `buiy_core` (the robustness fix), or (b) have the gallery
layout-snapshot dump SKIP invisible (size-0 in a hidden subtree) nodes so the Tier-1
gate pins only observable geometry (the cheaper, snapshot-side fix). Low severity.

## Render — icon-tier keyed partial re-extract — MEASURED, CLOSED (not worth building; 2026-07-03)

**Originated:** the 2026-07-03 glyph partial re-extract
(`docs/specs/2026-07-03-glyph-partial-reextract-design.md`), which names the icon
tier a non-goal and mandated "measure first". Measured 2026-07-03; the numbers
say don't build it.

**What was measured** (release, adapterless `PipelineHarness` main-world pipeline
+ a bare render world running `(maintain_atlas, extract_buiy_icons).chain()`;
48-frame warmup, extract phase timed in isolation over 400 frames; N `Icon`
children cycling 5 real gallery paths, one icon's tint toggled per frame to force
the wholesale rebuild):

| scenario | N=30 | N=300 | N=1000 |
|---|---|---|---|
| icon-dirty (wholesale rebuild) p50 | 8.4 µs | 71 µs | 229 µs |
| text-dirty sibling (icons untouched) p50 | 2.6 µs | 18 µs | 56 µs |

Wholesale rebuild scales linearly at ~0.23 µs/icon; the pathological 1000-icon
case is 0.23 ms — 1.4 % of a 60 Hz frame and ~60–240× below the 14–57 ms
dirty-extract cost that justified the glyph tier's Full/Patch machinery.

**Why closed, not just deferred:**
- The dirty gate is already tight: `extract_buiy_icons` rebuilds only on
  `Changed<Icon|GlobalTransform|ResolvedLayout|ClipRect|AncestorClip|ComputedPaintSkip|Stacking>`
  (scoped `With<Icon>`), theme change, or scale change. A glyph-Patch frame
  (caret blink, text value change) takes the steady touch-only path — proven by
  the text-dirty row staying flat vs the rebuild column. There is no
  over-triggering to fix.
- Icon counts stay small in practice: checkbox checks, radio dots, and
  disclosure arrows are TEXT glyphs (e.g. `checkbox.rs` `CHECK_GLYPH`), not
  `Icon`s; the icon tier carries only real vector icons (nav rail, steppers,
  close/search/gear), ~30 on the busiest gallery screen, and off-screen icons
  are paint-skipped so visible counts are viewport-bounded.
- A Full/Patch mirror would not remove the per-frame O(N) atlas
  `touch_existing` pass (LRU-warmth maintenance, same as the glyph tier keeps),
  so it would only shave the already-cheap rebuild while importing the
  classifier/splice/fold-residue complexity.

Reopen only if a widget starts emitting `Icon`s per row/cell at data-grid scale
AND animates them (per-frame `Changed<GlobalTransform>` is the one realistic
wholesale-every-frame trigger — the icon analog of active text).

## Render — stale-dim residue when an effect group drops on a glyph-clean frame (degradation-only) — RESOLVED (2026-07-03)

**Originated:** Stage D of the 2026-07-03 glyph partial re-extract (commit
`0f0ebeb`'s follow-up note) — pre-existing, out of scope there.

**Was:** `fold_degraded_groups` mutates the glyph CPU mirror + GPU buffer in place
on a degraded frame; when the degradation set SHRINKS (a group un-degrades or
drops) on a frame where the glyph carrier is clean, nothing re-uploaded — the
folded (dimmed) glyph bytes stayed on the GPU until the next glyph-dirty frame, so
a group compositing back through its own target was DOUBLE-dimmed (folded-alpha ×
opacity). Reachable only under a forced tiny RT-pool budget (`RtPoolBudget`
override); production-dead at the default 64 MiB (nothing ever degrades).

**Fix (direction (i) — restore-from-source on the un-degrade edge).**
`prepare_effect_groups` now takes `Res<ExtractedGlyphs>` and, when
`glyph_mirror_folded && !glyph_dirty`, rebuilds `BuiyInstanceBuffers::glyph` from
that retained, never-folded source BEFORE the empty-groups early return (so a full
group drop restores too). It treats the tier as freshly repacked
(`fold_glyph = glyph_dirty || glyph_restore`) so a surviving degraded group
re-folds from the clean bytes, re-uploads once, and sets `glyph_mirror_folded` to
whether a fold actually applied this frame — clearing it on a full un-degrade /
drop (the Patch fast path may then resume), keeping it true on a partial drop. The
prepare.rs Patch-path guard stays as defense-in-depth.

*Runner-up rejected — draw-time fold* (fold the group opacity in the shader
instead of mutating the mirror): far larger blast radius (glyph pipeline / WGSL /
wasm) for a production-dead bug, and the un-fold belongs in the same system that
owns the fold (`prepare_effect_groups`), so restore-from-source is the local,
low-risk fix.

**Regression test:** `undegrade_on_glyph_clean_frame_restores_unfolded_glyphs`
(`crates/buiy_core/tests/render_degraded_group_gpu.rs`, GPU `#[ignore]` lane) —
budget-grow un-degrade fixture (Opacity held constant so the glyph tier is
provably clean on the edge frame), white-glyph ink R+G parity against a cold
never-degraded render. RED (pre-fix): App A double-dimmed `[146,146,193]` vs App B
`[183,183,199]`.

## Reconcile `FocusedEntity` when its target entity despawns — OPEN

**Originated:** 2026-07-05, code-review of the Dooduel a11y focus-clamp fix
(`7c1529d`). The Dooduel networked GUI test (`apps/dooduel/tests/gui_networked.rs`)
surfaced it: a synthetic pointer click focuses a button, the resulting screen
transition despawns it, and the a11y tree then emitted a focus id absent from the
node set — which panicked `accesskit_consumer` (and would panic the real platform
adapter under a screen reader).

**Status:** The CRASH is fixed centrally in `build_tree_update`
(`crates/buiy_core/src/a11y/translate.rs`) — it now clamps `focus` to the root when
the focused id isn't in the node set, enforcing AccessKit's TreeUpdate invariant at
the one chokepoint feeding both the real adapter and the in-process snapshot. This
follow-up is the RESIDUAL, lower-severity gap the clamp does **not** close.

**Symptom:** nothing reconciles `FocusedEntity` when its target entity despawns
(the only writers are `focus_on_click` / `handle_tab` / the a11y `Focus`/`Blur`
router, all writing `Some`). After any screen swap that despawns the focused
widget, `FocusedEntity.0` stays `Some(dead_entity)` indefinitely, so the next
`TreeUpdate` clamps AT focus to the bare window root — a **WCAG 2.4.3 (Focus
Order)** degradation until the user's next Tab (which recovers cleanly:
`compute_next_focus` can't find the dead entity → lands on the first tab stop;
entity generation is part of `node_id_for`, so a reused index never false-matches).
`lower_focus_ring` also paints no ring in the interim (no panic).

**Direction:** a focus-reconcile system (a `RemovedComponents<Focusable>` observer,
or an existence check each frame) that clears — or better, RE-TARGETS — `FocusedEntity`
to a sensible element on the incoming screen when its entity despawns, so AT users
don't lose focus context on every navigation. The `focus.rs` module header already
scopes focus *restoration* to the deferred `buiy-focus-model-design`
(`docs/prior-art/bevy-a11y/focus-model.md`); this belongs there. Keep the
`build_tree_update` clamp regardless (defense-in-depth for any pruned-node focus,
e.g. `A11yHidden`).

## Mouse text-selection on non-editable (static) text — CHARTERED (own /staged-development)

**Originated:** 2026-07-05, Dooduel M1 playtest — the lobby room code is a static
`text()` label and Buiy has no mouse text-selection on non-editable text, so the
code can't be drag-selected/copied. A `Msg::CopyCode` "Copy" button
(`apps/dooduel/src/view/lobby.rs`, commit `6e1a724`) is the shipped stop-gap.

**Status:** **Deferred to its own full `/staged-development` cycle** (user request,
2026-07-05: "useful and important"). Not started.

**Scope sketch (for the design cycle, not decided here):** web-parity behavior —
any static text selectable by mouse drag + `Ctrl/Cmd+C` to copy, a selection
highlight, and reflection in the a11y tree. Key design questions: default-selectable
(web-like) vs opt-in per node vs a container "selection region"; single-node MVP vs
cross-node/cross-paragraph selection (much harder — spans the layout tree); how much
of the editor's existing selection model / selection-highlight rendering / clipboard
facade (`crates/buiy_core/src/text/edit/`) can be reused in a read-only mode. Aligns
with Buiy's modern-web-parity north star. Once shipped, it SUPERSEDES the Dooduel
Copy-button stop-gap.

## Dooduel — color-code digits vs letters in the room code (O/0, I/1 confusion) — OPEN

**Originated:** 2026-07-06, Dooduel M1 acceptance run — the room code `SD1CI0` was read
back as `SD1CIO` (trailing **0** vs letter **O**), so the agent seats hit `RoomNotFound`
until cross-checked against the clipboard. The code charset is `[A-Z0-9]`
(`apps/dooduel_server/src/util.rs` `random_room_code`), which mixes visually-ambiguous
glyph pairs (0/O, 1/I/l, 8/B, 5/S, 2/Z).

**Direction (app-level display polish):** in the lobby code box
(`apps/dooduel/src/view/lobby.rs` `code_box`), render each character in a color keyed to
its class — e.g. digits in the accent/positive tint and letters in `ink` — so a human
reading the code aloud can't confuse `0`/`O`. Cheap: split the code string into
per-character `text()` spans in a `row!` with the class-based color, rather than one label.
Optionally also do this on the Join screen's code entry echo. A stronger, separate option
is to shrink the *generator* alphabet to an unambiguous set (Crockford-style: drop
`O/I/L/U`, treat `0/1` carefully) — but that's a server/protocol change affecting code
space and existing-code assumptions, so keep it distinct from this display-only fix.

## Dooduel — live scoreboard stuck at 0 + sticky `guessed ✓` badge — RESOLVED (2026-07-06)

**Originated:** 2026-07-06, Dooduel M1 acceptance run. Two symptoms, one root cause:
(a) the per-player `— N pts` in the roster/scoreboard stayed `0` all match (correct only at
the podium), and (b) the `[guessed ✓]` badge was sticky — once a seat guessed any turn it
read "guessed" forever (turn-4 `bicycle` was guessed only by seat 0 per the transcript, yet
seats 1/2 both showed guessed). Confirmed in BOTH clients (the GUI scoreboard screenshot and
the `dooduel_mcp` seat view), so it was shared, not client-specific.

**Root cause (`apps/dooduel_core/src/session.rs`):** the `Session` is authoritative and both
clients rebuild their roster from a `Roster` event applied wholesale. But `Session` only
emitted a `Roster` on **membership** changes (join / vacate) — never on a **score** change
or a **turn** boundary. `resync` diffed phase / chat / countdown but not scores, and a
correct guess's `GuessResult` only carries `points` (which the clients ignore) + flips the
guessed flag optimistically. So live scores sat at whatever the last membership Roster
carried (0) until an unrelated join/leave happened to send one — which is exactly why the
final scores popped in the instant the host vacated at match-end (`vacate` → `broadcast_roster`).
The `guessed` flag (server-derived per-turn from `turn_guesses`) was never re-synced at a new
turn because no Roster was sent at turn start.

**Fix:** make the `Roster` the authoritative roster-refresh it was designed to be —
(1) snapshot per-seat scores in `Pre` and broadcast a `Roster` from `resync` whenever any
score moves (covers the guesser's award AND the drawer's turn-end payout); (2) broadcast a
`Roster` at each turn's `Picking` transition (a fresh-turn sync that resets every seat's
`guessed` badge and re-carries scores). One mechanism, in the shared core, so it fixes solo
+ networked and GUI + agents at once. Tests: `guess_gate_phase_drawer_and_correct_path` now
asserts the Roster follow carries the awarded score; `a_new_turns_picking_broadcasts_a_fresh_roster_that_resets_guessed`
asserts the turn-start Roster clears the flag. dooduel_core 94/94 green.

## Dooduel — chat rows render as empty pills (glyph text vanishes) — ✅ RESOLVED (2026-07-09)

**Resolved** by [multi-page coverage atlas bind](../specs/2026-07-09-multipage-coverage-atlas-bind-design.md)
(spec) / [plan](2026-07-09-multipage-coverage-atlas-bind.md), on `feat/dooduel-multiplayer-m1`
(commits `90b1e44` tests, `9181f63` fix, `c592927` warn removal). All resident coverage
atlas pages are now bound as a `texture_2d_array` (**forked** from the raster/image layout so
the drawing canvas is untouched) and `coverage.wgsl` samples the per-instance page layer via
explicit-LOD — uniformity-clean + WebGL2-safe. Verified end-to-end: GPU ink census (RED→GREEN)
+ recreate/re-upload-all test + byte-identical goldens + raster GPU guards + SwiftShader WebGL2
(0 shader errors) + native Dooduel render smoke. The v1 first-page-1 warning was retired. Closes
glyph-pipeline § 11.1. **Original root-cause analysis kept below for the record.**

**Originated:** 2026-07-06, Dooduel M1 acceptance run. In the in-game chat pane, once the
chat accumulates many messages, the newest rows render as empty colored pills — the
background bubble paints (node/quad pipeline) but the message text is blank. App code
(`in_game.rs` `chat_pane`/`chat_line`) is blameless; the text is correctly shaped and every
glyph is emitted and uploaded.

**Root cause — a FRAMEWORK render-pipeline bug (`crates/buiy_core/src/render/`):** the
coverage glyph atlas is paged (`atlas/types.rs` `page_size=1024`, `page_budget=8`), but the
GPU draw binds **only page 0** (`node.rs:158-162` single `coverage_bind_group`; `atlas/gpu.rs`
rebuilds page 0's texture only) and the shader ignores the per-instance page
(`coverage.wgsl:78-80` `_ = i.page` — the documented v1 stub, glyph-pipeline § 11.1). When
page 0 fills and the format is still under the 8-page budget, the allocator APPENDS page 1+
without evicting (`atlas/atlas.rs:151-209`); `emit_glyph` emits those glyphs with `page>0`
and fires `warn_once_page_overflow` (`text/extract.rs:1828-1848`, `2291-2299`). Overflow
glyphs then sample page 0 at page-1 UVs → wrong/empty texels → blank ink over a painted
background. It strikes the newest-resident keys (fresh chat glyphs) once the whole screen's
distinct-glyph working set (scoreboard + word + chat across several sizes/families) exceeds
one 1024² page — the first Dooduel workload to cross it. Affects ANY text-heavy long-running
Buiy screen, not just Dooduel. (Ruled out shaping / `stick_to_bottom` / `keyed_column` /
prepare-upload via a headless repro — all rows shape fully; the drop is purely the atlas page
bind.)

**Fix (deferred — a focused render effort, not an inline patch):** build the multi-page bind
the code already TODOs (§ 11.1): expose all resident coverage pages as a texture-2d-array in
`atlas/gpu.rs`, consume `i.page` in `coverage.wgsl` (`textureSampleLevel(atlas_array, …, i.page)`),
and bind the array in `node.rs` + the flat glyph draw. The eviction/pooling/`page`-field
machinery already exists — it's a bind + shader change, not new allocation logic. **Verify**
on the GPU `#[ignore]` lane (this repo has a real adapter): a content-presence census that
fills past one page, renders to texture, and asserts the last-emitted text node has non-zero
ink — plus a cheap headless guard that no live entry lands on `page>0` within budget. Deferred
because a shader / GPU-bind change deserves its own effort with golden verification rather than
a rushed inline fix. **Stopgap available if needed:** cap the Dooduel chat to the last ~N
messages so the working set stays under one page (mitigation only — not the real fix).

## Render — stale `GlyphAlphaInstance` "stride 68" comment in primitive.rs — OPEN (trivial)

**Originated:** 2026-07-09, spotted during the multi-page coverage atlas bind cycle
(spec § 7). `crates/buiy_core/src/render/primitive.rs:768` has a comment saying
`GlyphAlphaInstance` is "stride 68", but the actual asserted stride is **84** (the
`page @64` + `affine @68` fields the comment predates). Comment-only; the code + the
stride assertion are correct. Fix: correct the comment to 84. Kept out of the atlas-bind
change to preserve its scope.

## Dooduel — smoothly animate the turn countdown timer — OPEN

**Originated:** 2026-07-06, Dooduel M1 acceptance run (user request). The turn countdown
advances in discrete ~1 s steps (the `~Ns left` seat-view line; the GUI's countdown
ring/label). Make it a smooth continuous animation — a per-frame-interpolated shrinking
ring/bar driven off the wall-clock remaining fraction — instead of a 1 Hz jump. The GUI
already holds the remaining `Duration`; drive the visual from `remaining / total` each frame
through the existing animation/tick plumbing. Display-only polish (no authority change).

## Dooduel QA — 3 harness-found bugs FIXED (regression watch) — RESOLVED (2026-07-10)

**Originated:** 2026-07-10, the QA seat-driver harness campaign (spec
`docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md`). Building + running the driver
surfaced three real bugs, each staged + fresh-reviewed + covered by a regression test; logged
here (and on known-issues §1) so a future audit sees them closed:

1. **Track 1 (app) — in-game theme toggle occludes the chat Send button** (pick≠paint at
   1280×800). Fix `e891000` (suppress the floating toggle on the InGame screen); spec
   `docs/specs/2026-07-10-dooduel-theme-toggle-occlusion-design.md`; test
   `apps/dooduel/tests/in_game_occlusion.rs`.
2. **Track 2 (framework) — `probe`/AT `set_value` didn't emit `TextChanged`** so the model
   never folded (`SubmitJoin`/`SubmitGuess` read `""`). Fix `23540a0` (emit `TextChanged` on a
   value-changing `SetValue`), driver workaround dropped `7931f22`, spec rev-2.2; tests
   `crates/buiy_core/tests/a11y_set_value_route.rs` + the emit-count/empty-clear cover `5f4032b`.
3. **Track 3 (framework) — a controlled `text_input` clobbered an un-folded AT `set_value` on a
   rebuilding screen** (the in-game chat under the countdown). Fix `e81b91f`
   (`PendingProgrammaticEdit` marker) + placeholder re-patch `d2f4863`; design note
   `docs/specs/2026-07-10-dooduel-controlled-input-setvalue-fold-design.md`; RED→GREEN
   `crates/buiy_view/tests/controlled_input_rebuild_clobber.rs` + `controlled_placeholder_patch.rs`.

## Dooduel/QA — benign Bevy warn on set_value-then-same-frame-navigate-away — OPEN

**Originated:** 2026-07-10, Track 3 review. When a `set_value` is followed by a same-frame
navigation that despawns the edited field, `route_text_input` clears the
`PendingProgrammaticEdit` marker / drains the queued `TextChanged` against an already-despawned
entity, emitting a benign Bevy "command on a despawned entity" warn. The router clear is
unconditional **by design** (it must clear even the no-`on_input` edge so a marker never sticks);
the warn is cosmetic — no lost edit, no wrong state. Fix (if ever): guard the clear on the entity
still existing. Low priority; cosmetic log noise only.

## Dooduel — guesser sees the full drawing toolbar in-game — OPEN (design check)

**Originated:** 2026-07-10, W1 QA-gate observation. The in-game screen renders the full drawing
toolbar (Brush / Fill / brush-size dots / the 16 swatches / Undo / Clear) for **every** seat,
including guessers who cannot draw during another seat's turn. Verify against the reference
Dooduel bundle whether the toolbar should be hidden/disabled for non-drawing seats; if so, gate
its visibility on `is_current_drawer`. Design question, not a confirmed defect — confirm intent
before changing.

## Dooduel — in-game theme-toggle pick/semantic rect 88x50 vs authored 72x34 pill — CLOSED (no-violation)

**Status:** **Closed as no-violation** 2026-07-13 (app-author-ergonomics campaign, Track B
4b-pickrect). Confirmed by reading the code: pick and paint BOTH read `ResolvedLayout.size` (the
**border box**) — pick at `crates/buiy_core/src/picking/backend.rs:99,153` (`point_in_node(cursor,
abs_pos, layout.size, clip)`), paint at `crates/buiy_core/src/render/extract.rs:109-110,249-250`
(`ExtractedNode.size = ResolvedLayout.size`). So the toggle's pick == paint == **88×50 border
box** — there is **no pick≠paint violation** (the invariant Track B protects is intact). The
`72×34` is the `ContentBox` *content* box (border box minus `button()`'s 8px padding), NOT a
"painted pill" that pick overshoots. Nothing to fix here; no picking/button change.

**The real residual is a *sizing* surprise, not a picking bug → Track C guidance.** `BoxSizing`
defaults to `ContentBox`, so `.width(72)` on a padded `button()` renders an 88px border box (the
authored width is the *content* width, not the outer size). That footgun is documented as a Track
C app-author guidance item (the `using-mvu` / view-layout `BoxSizing`/`ContentBox`+padding gotcha),
not a code change here.

**Originated:** 2026-07-10, Track 1 investigation. The floating theme-toggle button's resolved
pick + semantic rect measured `88×50` where the design authors a `72×34` pill; the extra size is
`button()`'s default padding, read as an oversized *hit box beyond the painted pill*. That reading
was corrected above (pick == paint == border box). It contributed to the Track 1 occlusion, since
fixed by suppressing the toggle in-game.

## Dooduel — in-game desktop chat pane sits ~30px low (top_bar 72 vs design 60) — OPEN

**Originated:** 2026-07-10, Track 1 investigation. On the desktop in-game screen the chat pane's
bottom edge sits ~30px lower than the reference design's, traced to the app's `top_bar` height
being 72px where the design uses 60px (the extra 12px in the top bar cascades down the 3-pane
column). Cosmetic layout drift vs the bundle; confirm the intended `top_bar` height and reconcile.
Low priority.

## Render/framework — single-tier glyph paint can't be occluded by a top-layer quad (pick≠paint) — LANDED

**Status:** **Landed** 2026-07-12 by the top-layer stacking composite refactor (spec
`docs/specs/2026-07-10-toplayer-stacking-composite-design.md` rev-4, plan
`docs/plans/2026-07-10-toplayer-stacking-composite.md`, waves W0–W6 on `feat/dooduel-multiplayer-m1`).
`buiy_pass` now draws each `.top_layer()` subtree's COMPLETE tier-stack (glyphs, icons, bands
included) over the base block on the same window surface, so a top-layer overlay occludes base
text/icons/borders — not just fills. The pick≠paint seam is closed (`toplayer_paint_pick`).
GPU-verified on the RX 6700 XT: `scrim_tier_bleed_gpu` flipped bands/glyphs/icons BLEED→DIM; 8
`toplayer_occludes_all_tiers_gpu` fixtures GREEN; both legs byte-stable (buiy_core 95/0,
buiy_verify 24/0). Proven in the real dooduel app: base top-bar text dims ~34% under the word-pick
scrim (`in_game_picking` vs `in_game_drawer`).

**Originated:** 2026-07-10, Track 1 framework observation. Glyphs paint in a single global tier
**after** all quads (the flat glyph draw runs once, on top), so a top-layer quad drawn *over* a
base-screen text run cannot visually occlude that run's glyphs — the text bleeds through the quad
even though picking (which is z/top-layer-ordered) treats the quad as on top. This pick≠paint seam
is exactly what let the Track 1 theme toggle (a top-layer quad) win the Send *pick* while the
underlying "Send" glyphs still painted through it. Structural: honoring per-tier/top-layer glyph
occlusion needs glyphs to participate in the paint-order tiers (the top-layer composite pass /
effect-group seam), not a single post-quad draw. Larger render-pipeline effort; noted, not
scheduled. *(This is the observation the LANDED refactor above resolved.)*

## Render/framework — top-layer per-context ordering (overlapping overlays) — DEFERRED (single-boundary-v1)

**Originated:** 2026-07-12, top-layer stacking composite refactor (W6 close-out). The landed v1
draws ALL top-layer content as ONE block over the base. When two `.top_layer()` overlays OVERLAP,
the lower does not occlude the higher's base-bleed within the top block (they share one boundary,
not a per-context ordering). Characterized by the GPU gate
`toplayer_occludes_all_tiers_gpu::single_boundary_v1_scrim_dims_base_band_not_a_fellow_top_layer_band`
(the scrim dims a BASE band but NOT a fellow top-layer tooltip band, Δ0 — the accepted v1 limit).
No current app hits it (Dooduel overlays don't overlap each other). Per-context v1 = carry a
per-top-layer-ROOT id on the extract record (mirror the `nearest_group_entity` climb) + an N-range
`RangePartitioner` + `cross_root_rank` ordering per block; spike-confirmed cheap. Marked
`FIXME(per-context-v1)` in the fixture. Deferred.

## Render/framework — top-layer same-block backdrop-blur vs composite spatial overlap — DEFERRED

**Originated:** 2026-07-12, top-layer stacking composite refactor (W6 close-out). Within a single
block the sub-pass order is LOCKED to today's tier-stack → backdrop-blur → backdrop-filter-fills →
composite (§3.3). A same-block backdrop-blur and an effect-group composite that spatially OVERLAP
could in principle order-depend; no fixture constructs this (the fixtures test cross-block ordering,
which is correct). Named for completeness; revisit if a real overlay stacks a backdrop-blur under a
composited sibling in the same context.

## Dooduel (app) — avatar editor can now be a top-layer overlay (top-layer occludes correctly) — OPEN

**Originated:** 2026-07-12, top-layer stacking composite refactor (W6 close-out). The avatar editor
was authored as a full in-flow SCREEN (not a `.top_layer()` overlay) specifically because the old
global-tier rendering meant a top-layer overlay couldn't occlude base text/icons (see the LANDED
pick≠paint entry above + the note at `apps/dooduel/src/view/mod.rs`). That constraint is now lifted
— a top-layer overlay occludes all tiers. The avatar editor (and any other full-screen takeover)
could be re-authored as a top-layer modal overlay if desired. App-side; not scheduled.

## Dooduel (app) — dark-mode scrim near-iso-luminant with the background — OPEN

**Originated:** 2026-07-10 (root-caused), reconfirmed 2026-07-12. `SCRIM = 0x14161b` (rgba
20,22,27,~0.61) is theme-invariant and, in DARK mode, near-iso-luminant with the dark backgrounds
(the in-game canvas `0x1b1e25` is actually *lighter* than the scrim), so even a correctly-composited
scrim reads as a very subtle dim in dark mode. The tier-bleed refactor fixed the OCCLUSION (base
text now dims — proven ~34% in LIGHT mode); this is the SEPARATE, lower-priority COLOR issue: give
the scrim a theme-aware (darker / higher-alpha) value in dark mode so the dim is perceptible.
Independent of the framework refactor.

## Dooduel — stale guess-draft survives a mid-Drawing RECONNECT (reseed asymmetry) — OPEN

**Originated:** 2026-07-10, cycle-1 Track B review (F2). The `PhaseChanged` arm now clears
`chat_input` on every turn transition (`5320ae3`), but the `RoomState` mid-match-reconnect reseed
arm (`apps/dooduel/src/lib.rs:873`) sets a fresh phase WITHOUT clearing `chat_input` — so a stale
guess draft could survive a reconnect that lands mid-Drawing. Pre-existing, out of F2's
turn-transition scope, at most cosmetic for one word (guessing is open there, draft still
editable). Add `chat_input.clear()` to the reseed arm for reconnect symmetry if desired. Low
priority.

## Dooduel — Join/Lobby room-code field renders narrower than its dashed wrapper — OPEN (S5)

**Originated:** 2026-07-10, cycle-1 finding F4. The `2.5px dashed` room-code frame is the intended
"sketchy" aesthetic (`apps/dooduel/src/view/join.rs:15,27`, `lobby.rs:31,45`), but the inner
`text_input` (`.fill_width()`, `join.rs:23`) renders narrower than its dashed wrapper, so the field
looks offset/inset inside the frame — a naive first-timer read it as a glitch. Cosmetic; tighten the
field-to-frame fit. Low priority.

## Render/framework — content-less green chat pills at high volume (KI-02 atlas family) — OPEN (framework render track)

**Originated:** 2026-07-10, Dooduel QA cycle-2 (C2-02), all 4 seats. Once the in-game chat
passes ~20 rows, up to TWO content-less light-green (`ChatKind::Correct` #DCEFE3 tint) pills
render at the chat TAIL — a stale/extra green QUAD with no glyphs and no backing node in the
app's realized tree. The `apps/dooduel` side is PROVEN clean (core never emits empty chat
text; a faithful 24-row networked repro shows `keyed_rows(rendered) == model chat` one-for-one,
no phantom/dropped rows — guard test committed `4ae7764`), so the root is BELOW the app in the
GPU render layer — a stale/extra quad instance at high volume / the atlas-page or chat-pane
scroll edge. Same lineage as KI-02 (the multi-page coverage-atlas-bind cycle). S3 (cosmetic —
no lost content). Needs a FRAMEWORK render-track investigation on the GPU lane (repro the extra
quad at volume → localize the stale instance in the render/atlas/scroll pipeline → fix +
GPU-verify). Deferred from cycle-2 (app-clean; framework fix is a separate focused effort).
Tracked as known-issue KI-34.

## Dooduel — confetti not observed on podium / correct-guess reveals — OPEN (S5, verify)

**Originated:** 2026-07-10, Dooduel QA cycle-3 (Hugo, host+visual, low confidence). No confetti
was seen on the podium (2 static shots) or on any correct-guess reveal. The `ConfettiPlugin` IS
in `install_runtime`, so this is most likely an animated burst missed between the ~1 Hz static
screenshots rather than a real miss — but it was never positively confirmed firing in 3 cycles
of playtests (agents read static frames). VERIFY: run the native app and watch a correct-guess
reveal + the podium for a confetti burst; if it never fires, it's a real (S4) missing-celebration
bug, possibly tied to the deferred emoji/character-layer path (KI-23). Low priority.

## Dooduel — add a turn-end-boundary characterization test (after-expiry guess → WrongPhase) — OPEN (cheap, docs-expected-behavior)

**Originated:** 2026-07-10, Dooduel QA cycle-3 (C3-S2-01 adjudication). NOT a bug — a guess that
reaches the room actor after the draw-timer expiry tick is correctly WrongPhase-rejected, never
silently dropped (traced: `session.rs:404-416` synchronous `on_guess` with a `phase != Drawing`
guard; `room.rs:306-309` messages-before-ticks; `game.rs:535-547` all-guessed credits the last
guesser before `end_turn`; a 2/3 end with drawer +67 can only be a timer-end). Existing coverage
`guess_before_drawing_is_wrong_phase` (session.rs:1423) covers the Picking case but NOT the
after-draw-expiry case. Add a cheap session-tier test: force the draw timer to expire (tick
past `total`), submit a correct guess, assert it returns WrongPhase and does NOT enter
`turn_guesses` / change score. Documents the boundary + guards a future refactor from silently
dropping instead of rejecting. Low priority.
