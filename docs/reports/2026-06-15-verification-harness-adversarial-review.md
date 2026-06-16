# Adversarial fresh-agent review of the `buiy_verify` visual-bug-detection harness

**Date:** 2026-06-15
**Verdict:** Architecture sound; 7 real bugs (2 high, 3 medium, 2 low) + 1 maintainability trap found by independent review and **fixed** (TDD); 3 coverage gaps recorded as follow-ups. Both gates green. **Fault-injection confirmed** the gate goes RED on injected production layout, color, and paint-order bugs (see § Fault-injection verification), surfacing one hardening follow-up (the Tier-3 invariant mirrors the paint-order assembly).

One-shot investigation: after the `buiy-verification-design` harness (`crate buiy_verify`, ~9.7 k LOC, the five-tier pyramid) landed, a fresh-agent adversarial review re-examined it with no prior context — 7 independent module reviewers (one per tier), each finding bug found verified by a separate skeptic, plus a ground-truth agent that re-ran the real test gates. This report records what it found and how each was dispositioned. Audience: Buiy maintainers and the next person to touch `buiy_verify`.

## Why review a "complete" campaign

The harness was built TDD with a fresh-agent review *between* each phase, and both lanes were green when it was declared done. Grading one's own just-written code misses a specific failure class: **a verification harness whose checks can't fail** — a tautological test, an invariant that doesn't bind, a determinism guarantee resting on an assumption the tests never exercise. That class is invisible from inside the build (every test is green *by construction*) and is exactly what a cold-context adversary catches. It earned its keep: two latent landmines were one `bless` away from silently passing real visual regressions.

## What it found (all dispositioned)

### High — latent correctness landmines (fixed)

1. **A blank/failed render silently passed any golden.** `metric::compare`'s empty-image fast-path returned a non-saturated zero-`Diff` *before* the dimension-mismatch check, and the golden gate feeds the live capture as the first argument — so a render emitting a `0×0` image passed every budget, defeating the saturated sentinel on the exact path the regression tier uses. Fix `924ce89`: dimension-mismatch check first; asymmetric both-orders regression test (the prior tests only covered equal-dim and empty-vs-empty).

2. **`GoldenKey` dropped the forced-colors axis.** `Matrix::ci_default` crosses forced-colors (24 cells) and `CoverageKey` encodes `fc0`/`fc1`, but `GoldenKey` had no such field, so the GPU-tier mapping collapsed `fc=false` and `fc=true` onto one baseline — once blessed, a forced-colors regression (the hole gate #11 exists to close) would pass against the other mode's baseline. Fix `880a38a`: `forced_colors` added to the key/slug schema, injectivity test over the matrix, the 2 committed residue goldens re-pathed (bytes unchanged, captured at default `fc=false`). No `button` golden is committed yet, so the schema change cost zero re-baseline.

### Medium (fixed)

3. **Per-timestamp snapshots ran on a wall-clock virtual clock.** `build_app` never pinned `TimeUpdateStrategy`, so each `app.update()` advanced `Time<Virtual>` by wall-clock — defeating the documented byte-for-byte determinism for any animated fixture. Fix `8992d1f`: `assert_display_list_snapshot_at` pins `ManualDuration(ZERO)`; a wall-clock-sleep regression test whose phase (a) proves the leak is real (not a tautology).

4. **Same-`Name` sibling sort tiebroke on `Entity::index()`** (spawn-order dependent) in both the Tier-1 layout and Tier-2 display-list dumps — list rows all `Name::new("row")` dumped in spawn order, a flaky snapshot. Fix `8992d1f`: content tiebreak (position then size via `f32::total_cmp`); genuinely-indistinguishable siblings fail loudly rather than emit a flaky dump.

5. **The per-positive ledger budget was inert.** `Positive.budget` is written, persisted, round-trip-tested, and documented as "the budget this positive is asserted against" — but `check_golden` gated with the caller's budget and never read it, so the per-fixture widened-budget workflow did nothing. Fix `a68b655`: gate positive *i* against `positives[i].budget`; the triage card reports which bar was missed.

### Low (fixed)

6. **A saturated diff made a reftest `Mismatch` pass vacuously** (`!passes(saturated)` ⇒ `true`): a structural capture error read as a legitimate difference. Fix `ebfbd24`: saturated fails both kinds.

### Maintainability (fixed)

7. **Hardcoded `assert_eq!(cells, 24)`** in the enrollment tests would redden the moment a second fixture is added — breaking the "zero test edits to add a fixture" guarantee. Fix `85007b6`: derive from `sorted_catalog().len() * cells_per_fixture()`; the literal `24` stays pinned in one place.

### Doc-overstatement (reconciled, `87cd098`)

The review's headline quality finding: several docstrings oversold what the tests guarantee — the recurring theme, and a doc-as-deliverable violation. Reconciled `transform_roundtrips` (blind to inter-factor order — pinned elsewhere), `scene.rs` ("can never diverge" → bounded to the generated domain), the `matrix_goldens` vacuity message (green ≠ covered), `invariants.md`'s paint-order stability clause (inexpressible at the predicate boundary), and `reftests.md`'s `RefCase::multi` (marked deferred). Also added headless teeth to the determinism gate's first probe (`quiescence_unmet` condition 1).

## Quality verdict (from the synthesis, not self-assessment)

Architecture **excellent** — the Tier-3 metamorphic layer threads generated scenes through the *real* production paint-order functions rather than a parallel re-implementation; the vendored pixelmatch port is faithful line-for-line; the headless/GPU-lane split is clean. Test-rigor **good** — the 13 mutation fixtures genuinely have teeth, though several proptests were weaker than their docstrings claimed (now reconciled). The bugs were genuine, not nitpicks: two were latent only because no golden corpus is committed yet and captures happen to be well-sized — both become live the instant the harness is used as intended.

## Deferred (recorded in `docs/plans/follow-ups.md`)

- **`PositionKind` generator coverage** — `SceneNode` can't represent the tier-2 *(positioned, auto-z)* paint class; agrees with production over the generated domain only.
- **Quiescence conditions 2–4 headless coverage** — condition 1 now has a headless test; the three GPU-world conditions need a hand-built render world to test without an adapter.
- **CPU SDF oracle ↔ shader numeric pin** — the oracle and shader are textual twins; numeric drift is caught only by the GPU cross-check lane today.

## Fault-injection verification — does it actually catch bugs?

The regression tests prove each *fix* works; they don't prove the harness catches a real bug end-to-end. So I injected real one-line bugs into `buiy_core` **production** code and confirmed the gate goes RED (then reverted each):

| Injected production bug | Result | Caught by |
|---|---|---|
| **Layout:** `+7px` on every `ResolvedLayout.position` | **RED ✓** | Tier-1 layout snapshot (`coverage_layout::layout_snapshots`) |
| **Color/visual:** kill the red channel in `extracted_node_for` | **RED ✓** | Tier-2 display-list snapshot (`#ff00ffff` → `#0000ffff`) |
| **Paint order:** reverse the production `painters_z` z-tier sort (sub-pass 6f) | **RED ✓** | buiy_core's own `z_index_*` unit tests — **NOT** the new Tier-3 invariant |

Two findings worth their weight:

1. **A color R↔B swap was initially NOT caught** — because the button fixture's only colors are white `#ffffffff` and the magenta sentinel `#ff00ffff`, both *symmetric* under R↔B. A red-channel *kill* (asymmetric) was caught immediately. Lesson: the harness catches color bugs, but the corpus needs fixtures with asymmetric colors to make any given mutation observable — a fixture-coverage note, not a harness defect.

2. **A production paint-order bug is NOT caught by the new Tier-3 invariant** — only by buiy_core's pre-existing `z_index_*` tests. Root cause: `invariant/scene.rs`'s `realize` *re-implements* the painters_z assembly (sub-pass 6f) and feeds its own copy into `context_tree_paint_order`, so the metamorphic suite verifies a parallel copy, not the real assembly. The gate still catches the bug (via buiy_core's tests), but the cheap structured tier we built doesn't. Recorded as a hardening follow-up: make `realize` CALL the production assembly. This confirms the adversarial review's "scene.rs over-claims it builds painters_z exactly as 6f does" with a live reproduction.

Net: **layout bugs, color/visual bugs, and paint-order bugs all produce a RED gate.** The structured tiers catch layout and color directly; paint-order is caught by buiy_core's own tests today, with a clear path to also cover it in the metamorphic tier.

## Process note

Run as a background `Workflow` (20 agents, find → adversarially-verify → synthesize). One reported a `SIGSEGV` in `coverage_forced_colors`; re-running the headless gate **alone** exits 0 — it was build-cache contention from 20 agents sharing one `target/`, not a code defect. Every claimed bug was confirmed against the actual code before fixing; every fix is TDD (red test first) with a named regression test. The fault-injection pass above was inline (one production mutation at a time, reverted after each).
