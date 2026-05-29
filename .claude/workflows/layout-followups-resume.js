export const meta = {
  name: 'layout-followups-resume',
  description: 'Resume the 3 incomplete Buiy layout follow-up phases from the overnight run: P12 (docs flip only), P11 (fix doc blocker + finish T6-T10 + docs), P14 (complete re-run body + flip negative test as one gate-green unit + docs)',
  whenToUse: 'Second pass after layout-followups-overnight left P11/P12/P14 incomplete. P10 + P13 already landed. Pass args.worktree (absolute).',
  phases: [
    { title: 'P12-docs', detail: 'verify table T7 is complete + gate-green, then flip spec/README/follow-ups' },
    { title: 'P11-finish', detail: 'fix the T5 stale-doc blocker, drive T6-T10 via layout-phase-impl, then docs flip' },
    { title: 'P14-finish', detail: 'complete cq_descendant_rerun body (T5-T7) + flip the negative test (T8) as one gate-green unit, then docs flip' },
  ],
}

// ---------------------------------------------------------------------------
const WT = (args && args.worktree) || '/mnt/storage/projects/buiy/.claude/worktrees/layout-followups'
const SPEC_DIR = `${WT}/docs/specs/2026-05-08-buiy-layout-design`
const GATE_CMD =
  (args && args.gateCmd) ||
  `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace`

const ENV = `Work entirely inside the Buiy worktree: ${WT}. ALL cargo/git commands must run with that directory as the working dir (never touch the main checkout). Use absolute paths for edits. This host has NO xvfb — tests run headless via MinimalPlugins, so the gate drops the \`xvfb-run -a\` prefix. The headless gate is:\n${GATE_CMD}`

const DONE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['ok', 'gatePassed', 'committed', 'summary'],
  properties: {
    ok: { type: 'boolean', description: 'true iff the stage fully succeeded (work complete + gate green + committed)' },
    gatePassed: { type: 'boolean', description: 'true iff the FULL headless gate was run and passed (you ran it; do not assume)' },
    committed: { type: 'boolean' },
    blockers: { type: 'array', items: { type: 'string' }, description: 'anything that stopped you / needs a human' },
    summary: { type: 'string' },
  },
}

const docsFlip = (num, title, slug, specChild, specSection, headings) => `${ENV}

Buiy layout Phase ${num} (${title}) is implemented and the workspace gate is green. DOCS closeout — update canonical docs to reflect it is landed, commit. Do NOT change Rust.
1. Spec child ${specChild} (${specSection}): replace the deferral / stub / "v1 limitation" wording for this feature with the SHIPPED behavior; keep any genuinely-still-deferred sub-feature explicit. Supersede cleanly, do not contradict.
2. ${WT}/docs/README.md — under "### Layout" > "**Plans**", add a one-line entry for plans/2026-05-29-buiy-layout-${slug}.md tagged \`[landed]\` (mirror the existing Phase 9/10/13 entries).
3. ${WT}/docs/plans/follow-ups.md — the entry under ${headings.map((h) => '"' + h + '"').join(' and ')}: convert it to LANDED (heading "— LANDED" suffix + a **Status:** **Landed** in Phase ${num} line linking the plan), mirroring the existing "Phase 9 stacking ... LANDED" entry. Do not delete it.
4. Flip the plan file ${WT}/docs/plans/2026-05-29-buiy-layout-${slug}.md header **Status:** active -> landed.
5. git add the touched docs + commit: "docs: mark Phase ${num} layout (${title}) [landed]".
Return the structured result (gatePassed=true only if you re-ran the gate; for a docs-only change you may set gatePassed=true after confirming \`cargo fmt --all -- --check\` is clean on the docs — Rust is untouched).`

// ---------------------------------------------------------------------------
const report = {}

// ===== P12: table layout — verify T7 then docs flip ========================
phase('P12-docs')
const P12_PLAN = `${WT}/docs/plans/2026-05-29-buiy-layout-table-layout.md`
log('P12 (table): verifying T7 completeness + gate, then docs flip.')
const p12verify = await agent(
  `${ENV}

You are VERIFYING Buiy layout Phase 12 (full table layout) before its docs are flipped to landed. The overnight run committed all 7 tasks (tests passed) but the Task-7 reviewer returned approved=false WITHOUT listing a concrete blocker, so it was not auto-flipped. Your job: determine whether the table layout phase is actually complete + correct, and run the gate.

1. Read the plan ${P12_PLAN} (esp. Task 7) and the spec ${SPEC_DIR}/display-and-positioning.md § 1.2.
2. \`git -C ${WT} log --oneline\` and inspect the Phase-12 commits (the table-layout 6b work: TablePart classifier, resolve_column_widths, TableModel/place_table_cells, real 6b sub-pass, per-feature deferral warns). Read the final state of the table_layout sub-pass in crates/buiy_core/src/layout/systems.rs and tests/layout_table.rs.
3. Run the FULL headless gate. Report whether it passes (quote the first failure if not).
4. Judge: is the table layout genuinely implemented per § 1.2 (single-row column grid, multi-row column-width + row-height resolution, row-group stacking, cross-group column alignment), with only genuinely-out-of-scope sub-features left as documented warns? If you find a REAL defect (placeholder, dead code, spec deviation, tautological test), fix it minimally, re-run the gate, and commit a fixup. If it is sound, do NOT change code.

Return the structured result: ok=true iff the phase is sound + gate green. List any real blockers you could not fix.`,
  { label: 'verify:P12', phase: 'P12-docs', agentType: 'general-purpose', schema: DONE_SCHEMA },
)

if (p12verify && p12verify.ok && p12verify.gatePassed) {
  const p12docs = await agent(
    docsFlip(12, 'Full table layout algorithm', 'table-layout', `${SPEC_DIR}/display-and-positioning.md`, '§ 1.2 (table layout)', ['## Layout — full table layout algorithm']),
    { label: 'docs:P12', phase: 'P12-docs', agentType: 'general-purpose', schema: DONE_SCHEMA },
  )
  report.p12 = { verified: p12verify, docs: p12docs }
  log(`P12: docs flip ${p12docs?.committed ? 'committed' : 'NOT committed'}.`)
} else {
  report.p12 = { verified: p12verify, docs: null, note: 'P12 not flipped — verification did not pass.' }
  log(`P12: NOT flipped (ok=${p12verify?.ok}, gate=${p12verify?.gatePassed}). Blockers: ${(p12verify?.blockers || []).join(' | ')}`)
}

// ===== P11: content-visibility — fix doc blocker, finish T6-T10, docs ======
phase('P11-finish')
const P11_PLAN = `${WT}/docs/plans/2026-05-29-buiy-layout-content-visibility.md`
log('P11 (content-visibility): fixing T5 stale-doc blocker, then driving T6-T10.')
const p11fix = await agent(
  `${ENV}

You are fixing a REVIEW BLOCKER on Buiy layout Phase 11 (content-visibility) Task 5 before the remaining tasks run. The Task-5 code is committed and tests pass, but Decision D6 in the plan (${P11_PLAN}) required rewriting doc comments to describe the now-SHIPPED behavior, and that was not done. The stale comments now factually contradict the code (the "silently contradict outdated docs" anti-pattern the project forbids).

Fix these in crates/buiy_core/src/layout/types.rs (verify exact line numbers by reading — they may have shifted):
- The LayoutWarnOnceKey::ContentVisibilityDeferred variant doc (~lines 1019-1024) still says Phase 8 does NOT enforce Auto/Hidden, "both deferred". WRONG: Task 5 implements Hidden (no longer warns) and enforces Auto's off-screen skip; the variant now fires ONLY for the residual case (off-screen Auto WITHOUT a contain-intrinsic-size hint). Rewrite the doc to describe this new, narrower meaning (per D6).
- The ContentVisibility enum-level doc (~1140-1142): "Auto / Hidden enforcement is deferred (warn-once ...)" — update to shipped.
- ~line 1150: "Skip rendering off-screen content (deferred in Phase 8)." — update to shipped (Auto skip enforced).
- ~line 1152: "Skip rendering content like \`display: none\` for descendants (deferred)." — update to shipped (Hidden prunes descendants).

Read the actual Task-5 behavior in the plan + crates/buiy_core/src/layout/systems.rs to describe it accurately. Make ONLY doc-comment changes (no behavior change). Run the FULL gate (doc comments affect \`cargo doc -D warnings\`). Commit: "docs(layout): rewrite ContentVisibility/warn-key comments for shipped Phase 11 behavior (D6)".

Return the structured result.`,
  { label: 'fix:P11-doc', phase: 'P11-finish', agentType: 'general-purpose', schema: DONE_SCHEMA },
)
report.p11fix = p11fix

let p11impl = null
if (p11fix && p11fix.committed && p11fix.gatePassed) {
  log('P11: doc blocker fixed + gate green. Driving T6-T10 via layout-phase-impl.')
  try {
    p11impl = await workflow(
      { scriptPath: `${WT}/.claude/workflows/layout-phase-impl.js` },
      { worktree: WT, plan: P11_PLAN, tasks: [6, 7, 8, 9, 10], gate: true, gateCmd: GATE_CMD },
    )
  } catch (e) {
    log(`P11: layout-phase-impl threw (${String(e).slice(0, 160)}).`)
  }
  const p11built = p11impl && p11impl.haltedAtTask == null && (p11impl.completed || []).length === 5
  report.p11impl = p11impl
  if (p11built) {
    const p11docs = await agent(
      docsFlip(11, 'content-visibility auto + hidden enforcement', 'content-visibility', `${SPEC_DIR}/transforms-and-containment.md`, '§ 5.2 (content-visibility)', ['## Layout — `content-visibility: auto` off-screen skip', '## Layout — `content-visibility: hidden` descendant skip']),
      { label: 'docs:P11', phase: 'P11-finish', agentType: 'general-purpose', schema: DONE_SCHEMA },
    )
    report.p11docs = p11docs
    log(`P11: DONE (T6-T10 built, docs ${p11docs?.committed ? 'committed' : 'not committed'}).`)
  } else {
    log(`P11: T6-T10 not fully built (halted=${p11impl?.haltedAtTask}, completed=${(p11impl?.completed || []).length}/5). Spec deferral left in place.`)
  }
} else {
  log(`P11: doc-blocker fix did not land cleanly (committed=${p11fix?.committed}, gate=${p11fix?.gatePassed}). Skipping T6-T10.`)
}

// ===== P14: descendant invalidation — complete body + flip test as a unit ==
phase('P14-finish')
const P14_PLAN = `${WT}/docs/plans/2026-05-29-buiy-layout-descendant-invalidation.md`
log('P14 (descendant invalidation): completing re-run body (T5-T7) + flipping the negative test (T8) as one gate-green unit.')
const p14 = await agent(
  `${ENV}

You are COMPLETING Buiy layout Phase 14 (multi-level descendant invalidation). Tasks 1-4 are committed (resources, collect_dirty_descendants helper, cq_descendant_invalidate step 8, cq_descendant_rerun skeleton step 9). The HEAD commit is the T4 skeleton. The plan is ${P14_PLAN}.

CRITICAL ORDERING FIX: the plan splits the re-run BODY (T5 re-translate, T6 Taffy recompute + ResolvedLayout re-write, T7 CQ re-evaluation) from T8 (rename + polarity-flip of the existing negative test tests/layout_container_queries.rs::cq_transitive_cascade_is_one_frame_stale -> cq_transitive_cascade_catches_up_in_frame, asserting C becomes Active in-frame). But T5's behavior change makes the OLD negative test FAIL, so T5 cannot be committed gate-green before T8 runs. The overnight run halted exactly here. RESOLUTION: implement T5 + T6 + T7 + T8 TOGETHER as one coherent unit and run the FULL gate ONCE at the end before committing, so the gate is green.

Steps:
1. Read the plan Tasks 5, 6, 7, 8 IN FULL (they contain the exact body code for cq_descendant_rerun and the exact test flip). Also read the current cq_descendant_rerun skeleton + cq_flip_rerun (the template) in crates/buiy_core/src/layout/systems.rs, and the existing test at tests/layout_container_queries.rs.
2. Implement the full cq_descendant_rerun body per T5+T6+T7: re-translate the dirty set via translate_one_entity, children-sync, Taffy recompute (bump LayoutTaffyComputeCount), re-write ResolvedLayout for the dirty set, and re-evaluate container queries (cq_activate/cq_flip_check semantics inline) so C flips its ContainerQueryActive marker the SAME frame. Honor decision blocks D4 (one re-run per frame, loop-safe) and D5 (inline rule re-eval).
3. Add the T5 behavior test (cq_intermediate_b_reresolves_cqw_in_frame) AND flip the existing negative test per T8 (rename to cq_transitive_cascade_catches_up_in_frame, assert C becomes Active in-frame). Add any T6/T7 tests the plan specifies.
4. Run the FULL headless gate. It MUST be green (this is the whole point of doing T5-T8 as a unit). If a test fails, debug the re-run body against cq_flip_rerun until green. Watch the LayoutTaffyComputeCount 2x-per-frame cap (D4) — if the descendant re-run pushes Taffy compute above the documented ceiling, reconcile per the plan's D4 note (the guard in cq_descendant_invalidate).
5. Commit the body+tests in one commit (or a small logical sequence) with a clear message, e.g. "feat(layout): cq_descendant_rerun body + transitive-cascade test flip (Phase 14 — T5-T8, D4/D5)". Do NOT edit docs/ (the spec/README/follow-ups flip is a separate stage).

Return the structured result: ok + gatePassed reflect the ACTUAL gate outcome you observed. If you cannot get the gate green, set ok=false and list precisely what fails.`,
  { label: 'finish:P14', phase: 'P14-finish', agentType: 'general-purpose', schema: DONE_SCHEMA },
)
report.p14 = p14
if (p14 && p14.ok && p14.gatePassed) {
  const p14docs = await agent(
    docsFlip(14, 'Multi-level descendant invalidation', 'descendant-invalidation', `${SPEC_DIR}/container-queries-and-writing-modes.md`, '§ 1.3 and § 1.5', ['## Descendant invalidation on ancestor-resolved-size changes']),
    { label: 'docs:P14', phase: 'P14-finish', agentType: 'general-purpose', schema: DONE_SCHEMA },
  )
  report.p14docs = p14docs
  log(`P14: DONE (body+test unit built, docs ${p14docs?.committed ? 'committed' : 'not committed'}).`)
} else {
  log(`P14: NOT completed (ok=${p14?.ok}, gate=${p14?.gatePassed}). Blockers: ${(p14?.blockers || []).join(' | ')}`)
}

return {
  worktree: WT,
  landed: Object.entries({ 11: report.p11docs, 12: report.p12 && report.p12.docs, 14: report.p14docs })
    .filter(([, v]) => v && v.committed)
    .map(([n]) => Number(n)),
  report,
}
