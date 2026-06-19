# Code-side observations from the docs audit

**Date:** 2026-06-18
**Status:** active
**Companion to:** [docs/reports/2026-06-18-docs-audit.md](2026-06-18-docs-audit.md)

## Why this is separate

The docs audit was scoped as a documentation-only change. Anything the audit surfaced that would require editing files under `crates/` (not `docs/`) is collected here so it can be addressed in a dedicated code session. The remediation plan ([docs/plans/2026-06-18-docs-audit-remediation.md](../plans/2026-06-18-docs-audit-remediation.md)) stays purely under `docs/`.

## Headline: no functional code defects

Every spec↔code misalignment the audit flagged was independently re-checked against the source and classified. **All 10 verified misalignments were confirmed doc-staleness, not code defects** — the implementation matches (and in places exceeds) what the specs describe. The items below are *in-code comments*, *dead-but-harmless legacy code*, and *spec targets not yet implemented (expected future work)*. None changes runtime behavior; none is urgent.

## C1 — Stale in-code comments in `crates/buiy/src/lib.rs` (doc-comment accuracy)

The audit's foundation-spec unit found the as-built `BuiyPlugin` composition diverges (intentionally and justifiably) from the spec's §2.8 "indicative" sub-plugin order, but two code comments over-claim alignment:

- `crates/buiy/src/lib.rs:110-111` — "Sub-plugin order **matches** architecture.md § 2.8." It does not literally match (no Buiy-owned `InputPlugin` — `input` is the `BuiySet::Input` system set; `layout`/`picking` are interleaved; `animation`/`forms`/`devtools` don't exist yet). Soften to "follows the spirit of §2.8 (see the as-built note there)."
- `crates/buiy/src/lib.rs:50-52` — the doc-comment lists the order "core → theme → a11y → focus → input → widgets," which likewise predates the real composition. Update to the actual order, or point at the as-built note.
- The §2.8 spec sentence "Render registration happens in `Plugin::finish`" is also stale (render is added in `build()`; `lib.rs:141-153` documents why `finish` was wrong) — **that half is a doc fix** and lives in the remediation plan (Batch 3); listed here only because the lib.rs comment and the spec describe the same thing.

These are comment-accuracy edits, not behavioral changes.

## C2 — `crates/buiy_core/src/invariant/predicates.rs` doc-comment mirrors a stale symbol

The verification spec's `invariants.md` cites a `tier_rank` private closure at `systems.rs:4113`; that closure was removed and promoted to `pub fn top_layer_paint_rank` (`layout/systems.rs:3816`). The same stale `:4113 tier_rank` text is **copied verbatim into the live `invariant/predicates.rs` doc-comment**, so fixing only the spec would leave the code comment wrong. Update the predicates.rs comment to `top_layer_paint_rank` in the same code session. *(Note: `invariant/predicates.rs` is in `buiy_verify`, not `buiy_core` — confirm the exact path when fixing.)*

## C3 — Dead-legacy code path: `extract_buiy_draws` / `ExtractedDraws`

The Phase-0 extract system `extract_buiy_draws` and its `ExtractedDraws` resource still exist and are still registered in `ExtractSchedule` alongside the R5 `extract_buiy_nodes` / `ExtractedNodes` path (`render/mod.rs:267-272`; `render_smoke.rs:192` asserts both coexist). In-code comments already mark the Phase-0 path as "retired by R6/R8 … when node.rs reads the per-view `ExtractedNodes` instead." This is a **code-cleanup opportunity**, not a defect — the legacy path is inert with respect to the current node draw. Removing it (and the dead `ExtractedDraws`) would simplify the render module; verify nothing still reads `ExtractedDraws` before deleting.

## C4 — Spec targets not yet in code (expected future work, NOT defects)

The audit confirmed these are documented *targets*, correctly framed as future in the specs — listed only so a future code session knows they are known gaps, not oversights:

- **`A11yStates` / `A11yRelations` components** — the foundation `accessibility.md` decomposed-a11y target names a five-component set; `A11yRole`/`A11yLabel`/`A11yDescription` exist in `crates/buiy_core/src/a11y/`, but `A11yStates`/`A11yRelations` do not yet. This is the spec's stated target, scheduled for a future a11y spec/campaign.
- **`animation` / `forms` / `devtools` crates** — named in the foundation §2.8 long-term sub-plugin order; the crates do not exist yet (`crates/` has only `buiy`, `buiy_core`, `buiy_widgets`, `buiy_verify`). Future subsystems.

## Suggested handling

A single small code PR can cover C1–C2 (comment accuracy) and optionally C3 (legacy-path removal, with its own verification). C4 needs no action — it is roadmap, captured here so it is not mistaken for a regression. Pair any C1–C2 edit with the corresponding spec fixes in the remediation plan so the code comment and the spec land consistent.
