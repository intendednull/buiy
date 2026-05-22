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
