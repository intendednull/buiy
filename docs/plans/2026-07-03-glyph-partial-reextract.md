# Glyph-tier keyed partial re-extract — implementation plan

Date: 2026-07-03
Realizes: `docs/specs/2026-07-03-glyph-partial-reextract-design.md`
Staging template: PR #84's #2 quad-path stages (safest-first; each stage compiles,
is independently verifiable, and is behavior-preserving until the last possible
moment — `docs/plans/2026-06-26-buiy-perf-2-implementation-map.md` § Stage A–E).
Branch: `worktree-mt-ceiling-experiment` (off origin/main `a969cbf`; carries the
`mt_ceiling` bench used for before/after).

## Stage 0 — H6 fix (prerequisite hardening)

`prepare_buiy_instances` publishes the per-tier dirty bits it actually used into a
small render resource (`PreparedDamage { quad_dirty, glyph_dirty, icon_dirty }`);
`prepare_effect_groups` reads it instead of re-deriving `is_changed()`
(compositor.rs:637-638). Behavior-identical today; becomes load-bearing at Stage C.
**Verify:** full headless suite green; GPU compositor tests green.

## Stage A — factor `emit_one_entity` + per-entity key ranges (pure refactor)

Extract the per-entity emission body (extract.rs:437-868) into one function
returning the entity's `(glyph slice, quad slice, key slice)`; the Full walk calls
it per entity — output byte-identical by construction. Restructure
`ResidentTextKeys` bookkeeping so per-entity key ranges are recorded at emission
(D2's not-1:1 constraint). No behavior change anywhere.
**Verify:** full headless suite (all § 12 pins + value-compare pins untouched);
GPU goldens byte-identical; `mt_ceiling` floors unchanged.

## Stage B — observation-only classifier + structural probe + RED tests

Add the un-scoped structural probe (D3), the `GlyphDamage` resource + classifier
publishing `Full|Patch(entities)` while **still always executing Full**, and
`RenderWorkCounters` glyph fields (`glyph_full_rebuilds`, `glyph_patches`,
`glyph_patch_candidates`). Author RED: the reorder-escalation test (probe the
suspected pre-existing ancestor-reorder under-trigger — if it reproduces on main
behavior, that is a real bug fixed by the probe, reported in the PR), the
changed-set-fraction bail threshold measurement (gallery + `mt_ceiling` scenes).
**Verify:** classifier gate test (1 text change ⇒ candidate==1); existing suite
green; counters registered in all harness homes.

## Stage C — the Patch branch (THE win)

On eligible frames: skip the order walk; touch-before-insert (D5); per changed
entity `emit_one_entity` → byte-compare → splice glyphs/quads/keys + renumber
subsequent runs; publication tick discipline (D4); despawn splice-delete from the
removal streams' entity ids.
**Verify:** counter FLIP (1 change ⇒ `glyph_patches==1, glyph_full_rebuilds==0`);
glyph R5 sibling-retention (N−1 entities' records byte-identical at same index);
blink/typing latency pins updated consciously; full headless + BOTH GPU legs;
**`mt_ceiling` before/after — the 3.7/14/56 ms dirty floors must collapse toward
the 0.3/0.5/1.25 ms update-only floors.**

## Stage D — prepare suffix ranged upload

`GlyphDamage::Patch` → `write_buffer_range(first_changed..len)`; growth/error →
full write + `warn_once`; `BufferUploadStats` glyph extension.
**Verify:** GPU reftest — upload delta == changed AND pixel-identical to cold Full
render; the fallback reftest (degradation-live / growth ⇒ full repack, identical).

## Stage E — acceptance + docs flip

Both GPU legs; gallery live-run + interaction tier; full `mt_ceiling` sweep re-run
appended to the ceiling report; docs flips (D8); `docs/README.md` catalog entries.

## Risk register (from the scoping fleet)

- Atlas pressure-eviction of retained keys (D5) — GPU stale-uv test pins it.
- Fold double-apply under partial repack — D3 forces Full while degradation live.
- Value-compare tick regression — per-entity compare before splice (D4).
- Scroll degeneracy — fraction bail (D3).
- 16-param cap — tuple/bundle new params.
- Publication of `(glyphs, entity_runs)` under one tick (T8 D4) — splice must
  update both before the single DerefMut.
