# Documentation audit remediation

**Date:** 2026-06-18
**Status:** landed
**Spec:** [docs/specs/2026-05-07-docs-organization-design.md](../specs/2026-05-07-docs-organization-design.md)
**Realizes findings from:** [docs/reports/2026-06-18-docs-audit.md](../reports/2026-06-18-docs-audit.md)

> **Executed 2026-06-18.** All batches applied; verification clean (catalog↔filesystem 0 gaps, 0 broken internal links, no non-standard doc-header statuses, no code touched). Decision 2 resolved to option (c) — the dated render children were *not* renamed (it would have broken path references in 5 code files); they were blessed as a documented exception instead. Code-side items remain open in the [code-side companion](../reports/2026-06-18-spec-code-findings.md).

## Goal

Bring `docs/` back into conformance with the docs-organization spec and remove every misleading/stale claim the audit found, **without touching code** (code-side observations are split into [docs/reports/2026-06-18-spec-code-findings.md](../reports/2026-06-18-spec-code-findings.md) for a separate session). Each batch below cites the audit theme(s)/IDs it resolves.

## Decision points (resolve before executing the affected batches)

Five findings need a human convention call. Each has a recommended default so the plan is executable as-is; the recommendation is what the batches assume unless overridden.

1. **Report status convention (B8 / D5).** Reports use `**Verdict:**` + a date catalog tag, which violates the documented status set. **Recommend:** add `**Status:**` to each report and use real catalog statuses (`[superseded]` for the two whose premise is overtaken, `[landed]` for the adversarial review); do *not* bless date-tags. Alternative: amend the spec to define a report-specific `Date`+`Verdict` header (no lifecycle status) and legitimize date catalog tags.
2. **Render dated sub-design children (D1).** Five date-prefixed children violate the no-date-prefix child rule. **RESOLVED → option (c)** (changed from the original "rename" recommendation): bless dated design-note/ADR children inside a spec folder as an explicit exception in the docs-organization spec, and only normalize their status words. *Why the change:* the five filenames are referenced by path from **5 code files** (`render/components.rs`, `render/visibility.rs`, `render/extract.rs`, `tests/render_paint_skip.rs`, `tests/render_prepare.rs`) plus ~12 docs — renaming would break those code references or force code edits, violating the documentation-only scope. Option (c) keeps everything under `docs/` and breaks nothing.
3. **R8 plan status (B2 / D2).** R8 Task 8 was superseded by R8b; Tasks 1–7 landed. **Recommend:** `**Status:** superseded` with a one-line pointer to R8b, and keep R8's explanatory blockquote.
4. **`### Render` catalog area (A2).** **Recommend:** create it; move the render-pipeline spec out of `### Foundation` and all render plans out of `### Layout` into it.
5. **`follow-ups.md` disposition (A3).** **Recommend:** catalog it as-is (do *not* rename — it is a living tracker referenced by name everywhere), retitle the heading "Layout follow-ups" → "Cross-phase follow-ups," and add one sentence to the docs-organization spec blessing a rolling, intentionally-undated backlog file as an explicit exception to the dated-plan naming rule.

---

## Batch 1 — Catalog completeness & structure (`docs/README.md`) — resolves A1, A2, A4, A5, A6

1. **Add a `### Render` catalog area** (decision 4). Move the render-pipeline spec entry from `### Foundation` and all render plan entries from `### Layout` into it.
2. **Add the 11 missing catalog entries** (A1), each `[landed]` unless decision 3 says otherwise:
   - `render-r1-component-model` — render-side decomposed component model; sole definer of shared render types (`components.rs` + `color.rs`).
   - `render-r2-clip-rects` — `WriteClipRects` prep pass; `ClipRect`/`AncestorClip` clip-chain components.
   - `render-r6-prepare-buffers` — Prepare phase: single persistent per-frame instance buffer + view uniform; owns `BuiyPrimitiveKind`.
   - `2026-06-07-render-gpu-verify-campaign` — verified R6–R11 GPU path on real hardware; built the deferred atlas-glyph + effect-compositor + node-draw GPU orchestration; added the `--ignored` GPU CI lane; fixed 4 GPU bugs.
   - editing `e1`…`e6` — six entries mirroring the T1–T9 style under the Editing campaign subsection.
3. **Catalog `follow-ups.md`** (decision 5 / A3) under the Render or a "Backlog" line: rolling cross-phase deferral backlog; note it is intentionally undated.
4. **Expand "Where to start"** (A4) from 2 to 3–5 entries — add the layout, verification, and/or text-rendering specs.
5. **Remove the dead `reference-designs/` hyperlink** (A5); keep prose only ("…will live in `reference-designs/` if/when any are added").
6. **Delete the stray blank line** between the RmlUi and Coherent Gameface catalog bullets (A6).

## Batch 2 — Status-tag flips (catalog + in-file headers) — resolves B1–B7, parts of B8/B9

Flip in lockstep (catalog tag *and* in-file `**Status:**`) for each:

- **render-pipeline spec** `[draft]` → `[active]` (B1); add a one-line note that remaining C-tier seams — filter/backdrop-filter/mix-blend shaders, `ClipRadius` — are tracked in `follow-ups.md`.
- **docs-organization spec** `[draft]` → `[landed]` (B3).
- **Text T9 plan** `proposed` → `landed` (B4).
- **R8 / R8b / R9** drop `[partial]`; R8b `[active]` → `[landed]`, R9 → `[landed]`, R8 → `[superseded]` (decision 3 / B2). Rewrite the R9 catalog "Deferred (GPU orchestration)" clause and the R8 "blocked on the node-draw-model design" clause to past tense, pointing at the GPU-verify campaign.
- **Render dated sub-designs** `proposed`/`implemented`/`DECIDED` → documented set (B5): `extract-retain-damage` → `landed`, `effect-compositor-gpu` → `landed`, `node-draw-model` → `landed`, `subtree-visibility-suppression` → `landed`, `atlas-glyph-gpu` → `landed`. Rewrite the parent README "Children" bullet for node-draw from "open design decision" to "decided (Option C) / landed via R8b."
- **verification `open-questions.md`** `draft` → `landed` (B6).
- **Editing plans** (B7): `e1` `ready to execute` → `landed`; add `**Status:** landed` to `e2`–`e6`; rename their `**Spec realized:**` key to `**Spec:**`.
- **Foundation / layout spec status (B9):** keep `[draft]`/`[active]` respectively but add a one-line rationale note to each header (foundation: "draft = inventory still accreting, not architecture unsettled"; layout: list the genuinely-unbuilt target features).

## Batch 3 — Rewrite stale "deferred/landed" claims (the misleading class) — resolves C1–C13

Edit prose so landed work reads as landed. All are doc-only (code confirmed correct):

- **Layout `architecture.md`** (C1, C3, C4, C5, C6): rewrite the †footnote (Stacking/Transform/Containment landed Phase 8/9); add `Changed<Containment>` to the §1.2 trigger `Or` and fix the prose; reword multicol from "stub" to "Phase-13 real packer"; extend the §3/§8 pipeline to steps 0–9 (+ note text-owned `TextSync`/`TextCommit`, thirteen sub-sets total) cross-referencing `container-queries-and-writing-modes.md §1.3`; change `LayoutTree { tree: TaffyTree<()> }` → `TaffyTree<Entity>` with the node-context/manual-`Default` note.
- **Layout `transforms-and-containment.md §2`** (C2): keep approach (a) as the chosen, *implemented* design; correct the deferral note — the Bevy `Transform` write landed in render Phase R3 (`write_buiy_transform`, `render/bridge.rs`); render reads `GlobalTransform`. Cross-link `render/clip-and-transform.md §B`.
- **Render `architecture.md §7`** (C8): replace "R5 renames `extract_buiy_draws`" with "`extract_buiy_nodes` is registered alongside the retained Phase-0 `extract_buiy_draws`, retired by R6/R8."
- **Render `paint-order-and-top-layer.md §6`** (C9): `node_skip_reason` lives in `render::visibility.rs`; extract reads only `ComputedPaintSkip`. Repoint the test pointer to `tests/render_paint_skip.rs`.
- **Render dated sub-designs** (C7): re-tense `extract-retain-damage` and `effect-compositor-gpu` scope prose from "inert/never implemented" to "landed" (header flips are Batch 2).
- **`follow-ups.md`** (C10, C11): rewrite the `matrix_goldens` entry — option (i) landed (skip un-blessed cells; lane green; keep only the magenta-sentinel note); add a `— LANDED` block to the node-draw-model entry pointing at R8b + the R9 entry.
- **`blink` + `servo-stylo` prior-art READMEs + blink catalog line** (C12): re-tense Phase-9 framing from "NEXT/about to build" to "landed / designed against"; bump the README `Date` headers.
- **Foundation `README.md §4` roadmap** (C13): turn the graduated rows (layout, text-rendering, verification) into links with graduation/status notes; update the render-pipeline row off "drafted"; annotate the text-editing row as realized via `editing-and-ime.md` + the E1–E6 campaign. Leave the genuinely-future rows untouched.

## Batch 4 — Convention & header conformance — resolves D1, D2, D3, D4

1. **Render dated sub-design children (D1, decision 2 → option c):** do **not** rename (would break code references). Instead: (a) add a sentence to the docs-organization spec + the `organizing-buiy-docs` skill blessing dated design-note/ADR children inside a spec folder as an explicit exception to the no-date-prefix rule; (b) normalize the five children's `Status:` words to the documented set (Batch 2 B5). Filenames and all inbound references (docs + code comments) stay valid.
2. **Render plans r1–r11 headers (D2):** retrofit `**Date:** 2026-06-03` + `**Status:** landed` (R8 → `superseded`, decision 3). `**Spec:**` already present except the GPU-verify campaign — add `**Date:** 2026-06-07` / `**Status:** landed` / `**Spec:**` there.
3. **Author the two missing prior-art READMEs (D3):** `prior-art/bevy-feathers/README.md` and `prior-art/bevy-a11y/README.md`, each with the prior-art header (`Date`/`Status: active`/`Subject`), a one-paragraph overview, and a reading-order list of the existing children. Use `bevy-ui/README.md` as the template; anchor bevy-a11y on the megacomponent (#17644/#24308) narrative already in its children, keeping all Buiy-side statements sourced to the foundation spec (target, not decided-here).
4. **Editing plan `e3`/`e6` (D4):** remove the `**Repo root:**` header field and the hard-coded worktree path from `e6` prose.

## Batch 5 — Broken / misleading links — resolves E1–E4

1. **E1:** in `bevy-a11y/comparisons.md` (lines 9, 124) drop/repoint `../accesskit/comparisons.md` → `../accesskit/ecosystem.md`.
2. **E2:** in `egui/styling-and-theming.md` repoint `../bevy-egui/styling.md` → `../bevy-egui/api-surface.md`.
3. **E3:** replace `/home/user/buiy/docs/…` display labels with the matching relative path across the affected prior-art children (`bevy-a11y`, `bevy-ui`, `bevy-egui`, `godot-control`, `unity-ui`).
4. **E4:** replace stale "(pending)" cross-refs with live links in `unreal-slate-umg`, `godot-control`, `kayak-ui` READMEs.

## Batch 6 — Reports (status + stale framing) — resolves B8, C-reports, D5

1. Apply decision 1: add `**Status:**` to all three reports; set catalog tags (`text-editing-design-readiness` → `[superseded]`, `visual-bug-detection-strategy` → `[superseded]`/`[landed]`, `adversarial-review` → `[landed]`).
2. **Do not edit report bodies** (immutable). Surface staleness via extended catalog summaries: the readiness report's "types don't exist yet" premise is overtaken by landed E1–E6; the strategy report was realized by `buiy-verification-design` + the landed harness.

## Batch 7 — Accuracy nits & citation drift — resolves F1–F15

- **F1:** add the `buiy_verify` `--ignored` leg to the `CLAUDE.md` GPU-lane block so it mirrors CI.
- **F2:** reword the root-README MSRV line (enforced by `rust-version` in the manifest; `rust-toolchain.toml` pins the `stable` channel).
- **F3:** fix "two" → "three" example crates (or "two visual demos + the headless `capture` tool").
- **F4 / F11 / F15:** the citation-drift cluster — fix the specific stale numbers/symbols (`tier_rank` → `top_layer_paint_rank`; `compose_transform` `:3778`; the support/golden line ranges; the README "grep-confirmed" claim) **and** convert `:NNNN` citations to symbol-only across the verification + layout specs to stop recurrence.
- **F5:** correct the `goldens.md` example slug to the 7-field `…__fc0__lavapipe__dpr1`.
- **F6:** fix the `assert_golden(name, …)` mislabel → `assert_golden(&key, …)` in `goldens.md` + `metric.md`.
- **F7:** update the `atlas-glyph-gpu` "4 atlas_gpu.rs tests" framing (now 1; glyph tests in `text_gpu.rs`).
- **F8:** correct the text `README §Status` "E6 `TextInput` golden" → "E6 placeholder golden (`text_placeholder_gpu.rs`)"; note the `TextInput` bundle is verified headlessly only.
- **F9:** drop the BiDi split caret from the `§13` v1-shipped slice (it belongs only in Deferred).
- **F10:** decouple the units/`calc()` work from the "Phase 10" number (collides with landed position-fixed).
- **F12:** reconcile the cosmic-text PR #417 date contradiction.
- **F13:** make the APG pattern count agree (catalog "32" vs folder "30").
- **F14:** update foundation `accessibility.md`/`architecture.md` iOS wording (`accesskit_ios 0.1.0` shipped 2026-05-11; deferred posture unchanged).
- **D6 / D7:** remove the duplicate framing-disclosure block in `wpt-reftests`/`wgpu-testing`; add the missing H1 to `cosmic-text/README.md`.

## Execution & verification

- Each batch is independent and can be a separate PR (Batches 1–2 highest value: they remove the discovery gaps and the misleading statuses).
- After edits: re-run the audit's mechanical scripts (catalog↔filesystem diff, header scan, internal-link scan) — all three should come back clean (0 missing catalog entries, 0 broken in-prose links, every post-2026-05-07 spec/plan/report carrying the required header fields).
- Update `docs/README.md`'s own catalog entries for any doc whose status this plan flips, and update the `organizing-buiy-docs` skill if decision 1/2/5 amends the docs-organization spec (skill mirrors the spec; update in the same change).

## Out of scope

Code edits — including stale in-code comments and the dead-legacy `extract_buiy_draws` path — are deferred to [docs/reports/2026-06-18-spec-code-findings.md](../reports/2026-06-18-spec-code-findings.md). No functional code defects were found by the audit.
