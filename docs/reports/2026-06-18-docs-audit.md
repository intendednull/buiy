# Documentation audit — full accuracy / quality / spec-alignment pass

**Date:** 2026-06-18
**Status:** landed
**Scope:** all of `docs/` (564 files) + `CLAUDE.md` + repo-root `README.md`/`CHANGELOG.md`
**Remediation plan:** [docs/plans/2026-06-18-docs-audit-remediation.md](../plans/2026-06-18-docs-audit-remediation.md)
**Code-side companion:** [docs/reports/2026-06-18-spec-code-findings.md](2026-06-18-spec-code-findings.md)

## What this is

A full audit of the project documentation for anything that could **mislead a future agent** or **misalign with the specs**: stale claims, lying status tags, catalog gaps, broken cross-references, convention drift, and spec↔code mismatches. This report is the *list of problems*. The ordered fix is in the remediation plan; genuine code-side observations are split into the code-side companion so this remediation stays purely documentation.

## Method

1. **Mechanical pass** (scripted): catalog↔filesystem diff, header-field scan, and a full internal-link scan over all 8,161 relative links.
2. **Semantic pass** (26-agent workflow): one deep audit agent per load-bearing unit (master index, `CLAUDE.md`/root-README/CHANGELOG, docs-organization conformance, the foundation/layout/render/text/verification specs, the layout+render plans, the text+foundation plans + `follow-ups.md`, the reports), each cross-checking doc claims against the actual code under `crates/`; plus 6 prior-art cluster agents over the ~38 external-system folders.
3. **Adversarial code verification:** every claimed spec↔code misalignment was independently re-checked against the source by a separate agent and classified **doc-fix** vs **code-defect**.

## Headline result

**The docs lag the code; the code does not lag the specs.** All 10 spec↔code misalignments routed to adversarial code verification came back **confirmed doc-staleness — zero code defects.** Across the subsystems audited, the implementation matches (and in places exceeds) what the specs describe; the documentation simply was not flipped/updated when work landed. The dominant failure mode is **"deferred/future/draft" language and status tags left in place after the work shipped** — the single most dangerous class for a future agent, because it invites re-implementing landed, working subsystems.

The two genuinely structural defects are: **two prior-art folders missing their required `README.md`** (`bevy-a11y`, `bevy-feathers`), and **11 plan files absent from the master-index catalog**.

## Counts

| | High | Medium | Low | Total |
|---|---|---|---|---|
| Core docs (10 units) | 19 | 41 | 26 | 86 |
| Prior-art (6 clusters) | 3* | 10 | 11 | 24 |
| **Raw total** | | | | **110** |

\*Prior-art highs collapse to 2 distinct issues after cross-cluster dedup (the two missing READMEs; the broken `bevy-a11y → accesskit/comparisons.md` link was reported as HIGH in one cluster and MEDIUM in another).

Deduplicated, the 110 raw findings collapse to **~50 distinct issues** across seven themes below (raw findings often surface the same edit from several units — e.g. the 11 missing catalog entries appear in six different unit reports).

---

## Theme A — Catalog completeness & structure (`docs/README.md`)

The master index is the single highest-leverage doc; gaps here are discovery-blocking.

- **A1 — [HIGH] 11 plan files exist on disk but are absent from the catalog.** Render `r1-component-model`, `r2-clip-rects`, `r6-prepare-buffers`; `2026-06-07-render-gpu-verify-campaign`; the six editing plans `e1-substrate`…`e6-lifecycle-widget-closure`; and `follow-ups.md`. The render gaps are especially misleading: `r3,r4,r5,r7,r8,r8b,r9,r10,r11` are all listed, so the missing `r1/r2/r6` read as "never planned." A future agent could re-plan the render component model, clip rects, or prepare-buffers work that already exists. The GPU-verify campaign is the *single most load-bearing render plan* (it landed the deferred R8b/R9/atlas GPU orchestration and fixed 4 real GPU bugs) and is invisible. The editing gap is inconsistent with text-rendering, where T1–T9 are each listed individually.
- **A2 — [MEDIUM] No `### Render` catalog area.** The render-pipeline spec sits under `### Foundation` and all eleven render plans under `### Layout`. Render is its own subsystem (own spec, own `crates/buiy_core/src/render/` module). Burying render under Layout misrepresents the area structure and is *why the r1/r2/r6 omissions went unnoticed*. A `### Render` area is the natural home for the missing entries.
- **A3 — [MEDIUM] `follow-ups.md` type/discovery gap.** Referenced ~15× across specs/plans as the canonical cross-phase backlog (with `LANDED` flips), but uncataloged, has no date-prefixed filename, no `Spec:` field, and its title "Layout follow-ups" is now too narrow (it carries Render / Text / Verification items). It is a living tracker, not a one-shot migration plan — a type-fit gap the docs-organization spec does not anticipate.
- **A4 — [LOW] "Where to start" reading list has 2 items;** the docs-organization spec prescribes 3–5 foundational specs. Layout/render/text/verification specs are now landed and are the obvious additions.
- **A5 — [LOW] Dead link in the index:** `reference-designs/` is linked ("when they exist") but the directory does not exist.
- **A6 — [LOW] Stray blank line** splits the Coherent Gameface bullet from the rest of the "Game engine UI systems" list, rendering it as a separate one-item list.

## Theme B — Stale status tags (catalog + in-file headers)

Status tags are a discovery aid, but a `[draft]` on a fully-shipped subsystem actively misleads.

- **B1 — [HIGH] render-pipeline spec is `[draft]`** (catalog line 62 *and* the in-file `README.md` header) though R1–R11 + the GPU-verify campaign all landed and every `§3.2` component exists in `render/*.rs`. `[draft]` ("target not yet stable") is the most-misleading possible tag for a fully-realized target spec. → `[active]` (or `[landed]`; a few C-tier seams remain — see B9).
- **B2 — [HIGH/MED] Render plan tags R8 `[partial]`, R8b `[active]`, R9 `[partial]` describe GPU work that has since LANDED.** `[partial]` is not even a documented status. R8b's per-instance fragment-discard clip is live (`render/instance.rs` `PackedInstance` `clip_min/clip_max`); R9's `prepare_effect_groups` is a full ~250-line system (`compositor.rs`) and `BuiyNode::run`'s composite loops are real (`node.rs`), not inert; the GPU-verify campaign verified all of it on real hardware. These tags tell a future agent the compositor / node-draw path is unbuilt.
- **B3 — [MEDIUM] docs-organization spec is `[draft]`** (catalog + in-file) though the entire tree demonstrably follows it and `CLAUDE.md` + the `organizing-buiy-docs` skill both cite it as cemented. → `[landed]`.
- **B4 — [HIGH] Text T9 plan Status `proposed`** though T9 (and the whole campaign + spec) landed; the catalog already tags it `[landed]`. It was simply never flipped after closure.
- **B5 — [MED] Render dated sub-designs carry stale/non-standard statuses.** `extract-retain-damage-design` and `effect-compositor-gpu-design` say `Status: proposed` for work that is built and that `follow-ups.md` + sibling notes mark LANDED (each file even contains an internal "as-landed" note contradicting its own header). The README's "Children" bullet calls the node-draw model an "open design decision (blocks R8 Task 8 + R9)" though it was ratified (Option C) and shipped as R8b.
- **B6 — [MED] verification `open-questions.md` Status `draft`** though it records eight *resolved* decisions the whole landed harness was built against, and every sibling child is `landed`.
- **B7 — [LOW/MED] Editing plan headers:** `e1` Status `ready to execute` (non-standard) and stale; `e2/e3/e4/e5/e6` lack a `Status:` field entirely. All E1–E6 merged (PRs #62–#67).
- **B8 — [MED, DECISION] Reports diverge from the status convention.** All three reports open with `**Verdict:**` and no `**Status:**` field, and the catalog tags them with a bare date (`[2026-06-13]`…) instead of a documented status. Either bless date-tags for reports in the docs-organization spec, or add `**Status:**` and a real catalog status. Recommend the latter: the two reports whose premise is overtaken by landed work (`text-editing-design-readiness`, `visual-bug-detection-strategy`) → `[superseded]`; the adversarial review → `[landed]`. (See also Theme C / reports.)
- **B9 — [LOW, NEEDS-DECISION] Spec-level status calls.** Foundation spec `[draft]` (it is a long-lived inventory still accreting future areas — defensible, but add a clarifying note that "draft" means "still accreting," not "architecture unsettled"). Layout spec `[active]` (Phases 1–14 landed, but real target features — units/`calc()`, `Display::Contents`, colspan/rowspan, masonry, subgrid, per-window top layer, multicol fragmentation — remain unbuilt; `[active]` is defensible). These are conscious calls, not silent changes.

## Theme C — Stale "deferred / future" claims that have LANDED (most misleading class)

These read as live to-dos or unbuilt features but the code shipped. Each verified doc-fix below was confirmed against source by an independent agent.

- **C1 — [HIGH] layout `architecture.md` †footnote** declares Stacking/Transform/Containment "Future (Phases 8/9, unimplemented) … not yet defined, exported, or registered." All three are defined, exported, and registered (`layout/mod.rs`). Phases 8 and 9 landed.
- **C2 — [HIGH] layout `transforms-and-containment.md` §2** frames "Buiy owns the entity's Bevy `Transform`" as deferred to Phase 8. **It shipped** — relocated to the render bridge: `write_buiy_transform` (`render/bridge.rs`) composes `ResolvedLayout.position` + `ScrollOffset` + `ResolvedTransform.matrix` into the Bevy `Transform`, Bevy propagates `GlobalTransform`, and `extract.rs` reads `GlobalTransform` for position. Approach (a) is the realized design, not a soon-to-land one; the doc should keep it as chosen+implemented and cross-link `render/clip-and-transform.md §B`.
- **C3 — [MED] layout `architecture.md §1.2`** says "Containment is not in the shipped trigger set." It is (`Changed<Containment>` in the `sync_styles` `Or`, Phase 8). Stacking/`UiTransform` remain correctly excluded.
- **C4 — [MED] layout `architecture.md §1.2`** calls multicol "a warn-once-per-session stub." Phase 13 replaced it with a real packer (`multicol_pack`); the sibling `flex-and-grid.md §3.2` already says "Implemented (Phase 13)" — so the parent contradicts its own child.
- **C5 — [MED] layout `architecture.md §3/§8`** lists the pipeline as steps 0–7 / "nine system sets." The code has **thirteen** sub-sets including layout-owned step 8 `CqDescendantInvalidate` and step 9 `CqDescendantReRun` (Phase 14), plus text-owned `TextSync`/`TextCommit`. The sibling `container-queries-and-writing-modes.md §1.3` documents steps 8/9 — the canonical pipeline file is contradicted by its own child.
- **C6 — [MED] layout `architecture.md §1.1`** shows `LayoutTree { tree: TaffyTree<()> }`. Code is `TaffyTree<Entity>` (text leaves register their entity as the node context for the measure closure; forced a manual `Default` impl). Drift originated in the text campaign.
- **C7 — [HIGH] render dated sub-designs present GPU orchestration as inert/"proposed"** though built (overlaps B5: `prepare_effect_groups` full body; `node.rs` real composite passes; `composite.rs`/`.wgsl` present).
- **C8 — [LOW] render `architecture.md §7`** says "R5 renames `extract_buiy_draws`." R5 *added* `extract_buiy_nodes`; both coexist (Phase-0 path retired later by R6/R8 per in-code comments).
- **C9 — [LOW] render `paint-order-and-top-layer.md §6`** attributes `node_skip_reason` to `render::extract`. It moved producer-side to `render::visibility.rs`; extract reads only `ComputedPaintSkip`. The sibling subtree-visibility design already documents the move.
- **C10 — [HIGH] `follow-ups.md`** flags `coverage_golden::matrix_goldens` as "RED on the GPU lane … the lane is not green," with a resolution "option (i)" listed as an open campaign-owner decision. **Option (i) landed** — the test skips un-blessed cells (`coverage_golden.rs:106-109`), so the lane is green. The fix commit (`d14e103`) landed 16 minutes after the stale doc text was written and the entry was never updated.
- **C11 — [MED] `follow-ups.md`** presents the node-draw model as an unresolved "blocker … needs ratifying before R8 Task 8 / R9." It shipped as R8b (per-instance clip-AABB fragment-discard + multi-pass composite node). No `LANDED` marker.
- **C12 — [MED] `blink` + `servo-stylo` prior-art READMEs (and the blink catalog line)** frame Phase 9 (sub-pass 6f stacking + top layer) as future/"NEXT"/"about to build." Phase 9 landed (`[landed]` plan in the catalog). The technical content stays; only the tense/timeliness framing is stale.
- **C13 — [HIGH] foundation `README.md §4` sub-spec roadmap is frozen pre-graduation.** Its intro is future-tense ("graduates … when it's that subsystem's turn"), and only the render-pipeline row is a link (annotated "drafted — `[draft]`"). Layout, text-rendering, and verification have all graduated to real specs on disk but appear as bare backtick text with no link or graduation note — a future agent would conclude they are undesigned and might re-design them. The text-editing row also mispredicts the doc shape (it was realized as the child `editing-and-ime.md`, not a standalone spec).

## Theme D — Convention violations (structure / headers)

- **D1 — [MED, DECISION] The render-pipeline spec folder has 5 date-prefixed children** (`2026-06-06-…`, `2026-06-07-…`, `2026-06-08-…`) using non-standard status words (`proposed`/`implemented`/`DECIDED`). The convention is explicit: children are bare kebab-case with no date prefix and inherit the parent date ("if a child genuinely needs its own date, it is a separate spec, not a child"). These are dated design-notes/ADRs dropped into the spec folder, blurring the spec-vs-report boundary. Decide: (a) rename to bare kebab children + normalize statuses, (b) promote to top-level specs/ADRs, or (c) amend the docs-organization spec to bless dated "design-note" children as an explicit ADR-style exception.
- **D2 — [MED] All 11 render plans (r1–r11) lack `**Date:**`/`**Status:**` header fields** (they use a blockquote header style). They postdate the 2026-05-07 convention, so retrofitting is required, not optional. R8 specifically should be `**Status:** superseded` (Task 8 superseded by R8b; Tasks 1–7 landed). The `**Spec:**` ref is present on r1–r7/r9–r11 (so the "missing Spec refs on render plans" mechanical finding is a partial false positive — only the GPU-verify campaign truly lacks one).
- **D3 — [HIGH] Two prior-art folders are missing their required `README.md`:** `bevy-a11y` (12 children) and `bevy-feathers` (12 children). The catalog links `prior-art/bevy-a11y/` and `prior-art/bevy-feathers/` as if a README exists; both resolve to a folder with no entry point or reading-order guide.
- **D4 — [LOW] `e3` header has a `**Repo root:**` field** hard-coding a machine-/worktree-specific absolute path (`…/worktrees/render-pipeline`), repeated in `e6` prose. Not part of the documented header set; misleading in a landed plan.
- **D5 — [LOW] Reports lack the in-file `**Status:**` field** (use `**Verdict:**`) — overlaps B8.
- **D6 — [LOW] Duplicate framing-disclosure sections** in `wpt-reftests` and `wgpu-testing` prior-art READMEs (a generic boilerplate `## How to use` block whose body is a framing disclosure, immediately adjacent to the specific `## Framing disclosure`). The other three verification folders carry a single block.
- **D7 — [LOW] `cosmic-text` README lacks an H1 title** (jumps from the header block to `## What it is`); every sibling prior-art README opens with `# <System>`.

## Theme E — Broken / misleading links

- **E1 — [MED] `bevy-a11y/comparisons.md` → `../accesskit/comparisons.md` (×2)** — no such file; the comparison material lives in `accesskit/ecosystem.md`. Repoint or drop.
- **E2 — [MED] `egui/styling-and-theming.md` → `../bevy-egui/styling.md`** — no such file; bevy-egui's styling content is in `api-surface.md`. Repoint.
- **E3 — [LOW] Fictional `/home/user/buiy/docs/…` absolute paths used as link *display text*** in several prior-art children (`bevy-a11y`, `bevy-ui`, `bevy-egui`, `godot-control`, `unity-ui`). The link *targets* are correct relative paths and resolve; only the visible labels are wrong (repo lives at `/mnt/storage/projects/buiy`).
- **E4 — [LOW] Stale "(pending)" cross-references** in `unreal-slate-umg`, `godot-control`, and `kayak-ui` READMEs to sibling folders (`unity-ui`, `slint`, `woodpecker-ui`) that now exist and are `[active]`.

## Theme F — Accuracy nits & citation drift

- **F1 — [MED] `CLAUDE.md` GPU lane omits the `buiy_verify` `--ignored` leg that CI actually runs.** A contributor following `CLAUDE.md` verbatim before pushing skips the entire `buiy_verify` GPU suite (goldens, reftests, the perceptual metric — 34 `#[ignore]` tests) and could land a regression that fails CI's GPU lane. *(Verified: CI runs both legs; the doc is incomplete.)*
- **F2 — [LOW] Root `README.md` MSRV claim wrong:** says Rust 1.85 is "selected automatically via `rust-toolchain.toml`," but that file pins the floating `stable` channel; the MSRV is enforced by `rust-version` in the workspace manifest.
- **F3 — [LOW] Root `README.md` says "two runnable example crates"** but there are three (`hello_button`, `hello_text`, `capture`).
- **F4 — [MED] Pervasive stale line-number citations across the verification specs** (`tests/support/mod.rs`, `render/golden.rs`, `layout/systems.rs` off by 25–460 lines), plus a stale symbol: `tier_rank` (promoted to `pub fn top_layer_paint_rank`) and `compose_transform` (now `:3778`, `pub`, not `pub(super) :3775`). The README's "grep-confirmed: … at line 3775 / 4113" claim **no longer reproduces** — actively misleading. The same stale `:4113 tier_rank` text is mirrored in the live `invariant/predicates.rs` doc-comment (see code-side companion). Recommend dropping `:NNNN` suffixes for symbol-only citations to stop the recurring rot.
- **F5 — [HIGH] verification `goldens.md` as-landed example slug omits the forced-colors axis** (`…/dark__sm__lavapipe__dpr1`) — this is the *exact* pre-fix form of the bug the adversarial-review report flags as a HIGH bug it FIXED ("`GoldenKey` dropping the forced-colors axis"). The landed slug is the 7-field `…__fc0__lavapipe__dpr1`. A future agent reconstructing a corpus path from this example builds the wrong directory.
- **F6 — [LOW] verification `goldens.md`/`metric.md` mislabel `assert_golden`'s first arg as `name`;** the signature is `assert_golden(key: &GoldenKey, …)` (the adjacent code block already shows the right form).
- **F7 — [LOW] render `atlas-glyph-gpu-design`** anchors on "all 4 `atlas_gpu.rs` tests"; `atlas_gpu.rs` now has 1 test — the glyph/retint GPU tests moved into the text crate (`text_gpu.rs` etc.) when T8 took the glyph producer.
- **F8 — [MED] text `README.md §Status` claims an "E6 `TextInput` golden"** on the GPU lane; no GPU golden renders the `buiy_widgets::TextInput` bundle (that has only a headless composition test). The actual E6 GPU golden renders an editor `Placeholder` (`text_placeholder_gpu.rs`).
- **F9 — [LOW] text `editing-and-ime.md §13`** lists the BiDi split caret in *both* the v1-shipped slice and the deferred slice; the `§4.1` as-landed note correctly defers it (cosmic 0.19 has no dual-caret API).
- **F10 — [LOW] layout `box-model.md`** tags units/`calc()` as "Phase 10 (`buiy-layout-units-calc`)" — colliding with the landed Phase 10 (`position-fixed`). Same number, opposite status across two children. (The substance — `Length` lacks `Em/Rem/Vw/Calc` — is accurate.)
- **F11 — [LOW] layout source-line citations have drifted** (`write_resolved_layout` cited `:1662`, now `:2717`; several `style.rs`/`types.rs` numbers off). Symbols still exist by name. Recommend symbol-only citations.
- **F12 — [MED] `cosmic-text` README internal date contradiction:** PR #417 (rustybuzz→harfrust) dated `2025-10-30` on line 14 and `2025-09-09` on line 33. Reconcile against the upstream PR.
- **F13 — [MED] APG catalog summary says "32 widget design patterns";** the `wai-aria-apg` folder says 30 everywhere. Confirm the live W3C count and make catalog + folder agree.
- **F14 — [MED] foundation `accessibility.md`/`architecture.md` call AccessKit iOS "in-progress upstream"** — `accesskit_ios 0.1.0` shipped 2026-05-11. The catalog and the accesskit prior-art README already flag this; the stale wording lives in the *foundation spec*. The deferred-platform CI posture is unchanged; only the upstream-status phrasing is stale.
- **F15 — [LOW] text OQ#1 line citation drift** (`lib.rs:57-87` vs sibling's `:64-95`; actual enum `:65`, chain `:87-96`).

## Theme G — Code-side observations (no functional defects)

Recorded in the [code-side companion report](2026-06-18-spec-code-findings.md) for a separate session, keeping this remediation documentation-only. **No functional code defects were found.** The items there are: stale *in-code comments* (`lib.rs` "matches §2.8" / "Render registration in `Plugin::finish`"; `invariant/predicates.rs` mirrored `:4113 tier_rank`), a dead-legacy code path (`extract_buiy_draws`/`ExtractedDraws` coexisting with the R5 node path, retirement already noted in-code), and spec targets not yet in code that are *expected future work, not defects* (`A11yStates`/`A11yRelations` components; `animation`/`forms`/`devtools` crates).

## Cross-cutting recommendation

Two patterns caused most of the high-severity findings and are worth fixing structurally, not just instance-by-instance:

1. **Status/deferral flips lag landing.** When a plan/phase lands, its spec deferral notes, plan Status, catalog tag, and `follow-ups.md` entry are not flipped in the same change. The remediation plan flips the current backlog; going forward, "docs flipped" should be part of a phase's definition of done (the global guidance already says so).
2. **`:NNNN` line-number citations rot on every refactor.** They are the source of the entire F4/F11/F15 cluster and several low-severity prior-art nits. Recommend a blanket move to symbol-only citations (`write_resolved_layout in layout/systems.rs`), which the specs already use in most places.
