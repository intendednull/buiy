export const meta = {
  name: 'layout-followups-overnight',
  description: 'Overnight: charter + implement the next several Buiy layout follow-up phases end-to-end (design plan -> TDD implement+review+gate via layout-phase-impl -> flip spec/README/follow-ups docs), one phase at a time',
  whenToUse: 'Autonomous multi-phase run that drains the next chunk of docs/plans/follow-ups.md. For each target it writes a TDD plan, drives it to green through the layout-phase-impl driver, then updates the spec (removes the deferral), docs/README catalog, and follow-ups.md. Pass args.worktree (absolute) and optionally args.targets / args.only.',
  phases: [
    { title: 'Plan', detail: 'design agent writes a TDD plan per phase (grounded in spec + integration surface + prior-art), commits it' },
    { title: 'Implement', detail: 'delegate to layout-phase-impl: per-task TDD implement -> adversarial review -> fix-once -> halt-on-blocker -> headless gate' },
    { title: 'Docs', detail: 'flip the spec deferral, add the README catalog entry [landed], move the follow-ups.md entry to landed' },
  ],
}

// ---------------------------------------------------------------------------
// Parameters (via the Workflow `args` input):
//   args.worktree : absolute path to the git worktree to operate in (REQUIRED in
//                   practice; falls back to the layout-followups worktree).
//   args.only     : optional array of phase numbers to run (e.g. [10, 11]);
//                   default = all TARGETS below, in order.
//   args.gateCmd  : override the headless gate command.
// Each phase runs strictly sequentially (they share layout/systems.rs +
// layout/mod.rs). layout-phase-impl commits per task and halts BEFORE committing
// broken work, so a halted phase leaves the tree at its last good commit and the
// remaining (independent) phases still run on a clean base.
// ---------------------------------------------------------------------------
const WT = (args && args.worktree) || '/mnt/storage/projects/buiy/.claude/worktrees/layout-followups'
const TODAY = '2026-05-29'

const GATE_CMD =
  (args && args.gateCmd) ||
  `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace`

const SPEC_DIR = `${WT}/docs/specs/2026-05-08-buiy-layout-design`

// The next chunk of follow-ups, ordered easy -> hard to build confidence before
// the two hard algorithmic phases (table, multicol). Each is NOT gated on the
// unbuilt render-pipeline / window-and-surface specs.
const TARGETS = [
  {
    num: 10,
    slug: 'position-fixed',
    title: 'Position::Fixed',
    specChild: `${SPEC_DIR}/display-and-positioning.md`,
    specSection: '§ 2.2 (Position type, Taffy mapping for Fixed)',
    followUpHeading: '## Layout — `Position::Fixed` implementation',
    priorArt: ['blink (positioned layout / containing block)', 'servo-stylo', 'taffy'],
    sketch:
      'PositionKind::Fixed currently emits a Phase-1 warn-once and translate.rs::map_position does NOT emit taffy::Position::Absolute for Fixed. ' +
      'Implement: map_position emits taffy::Position::Absolute for Fixed; the ContainingBlock resolution is overridden to the layout ROOT regardless of the nearest-positioned ancestor (that is the only behavioral difference from Absolute). ' +
      'Sketch from follow-ups.md: single sync_styles change + a private is_fixed_root flag on the entity translation state. Remove the Fixed warn-once. ' +
      'Read display-and-positioning.md § 2.2 for the exact Taffy mapping contract and how Absolute already resolves its containing block, then mirror it with a root override.',
  },
  {
    num: 11,
    slug: 'content-visibility',
    title: 'content-visibility auto + hidden enforcement',
    specChild: `${SPEC_DIR}/transforms-and-containment.md`,
    specSection: '§ 5.2 (content-visibility auto/hidden)',
    followUpHeading: '## Layout — `content-visibility: auto` off-screen skip',
    extraFollowUpHeadings: ['## Layout — `content-visibility: hidden` descendant skip'],
    priorArt: ['blink (CSS Containment, content-visibility shipped Chrome 85)', 'servo-stylo'],
    sketch:
      'ContentVisibility::Auto and ::Hidden are currently STORED on Containment and warn once via LayoutWarnOnceKey::ContentVisibilityDeferred(Entity), but no skip is performed. ' +
      'Implement TWO behaviors: (a) Auto = off-screen skip — check ContentVisibility::Auto + off-screen (last-frame ResolvedLayout vs viewport) + a contain-intrinsic-size hint; feed Taffy a sentinel size and no-op the descendants style sync; snap back when on-screen. This needs a NEW contain-intrinsic-size component (author-set width/height hint). ' +
      '(b) Hidden = equivalent to Display::None for descendants (tree-prune in sync_styles); snap back on toggle. ' +
      'Remove (or repurpose) the ContentVisibilityDeferred warn for the now-implemented arms. Read transforms-and-containment.md § 5.2 step-by-step (the spec describes the auto skip in step 1). Be careful: the off-screen check is per-frame and must not thrash (define the snap-back hysteresis the spec implies).',
  },
  {
    num: 12,
    slug: 'table-layout',
    title: 'Full table layout algorithm',
    specChild: `${SPEC_DIR}/display-and-positioning.md`,
    specSection: '§ 1.2 (table layout)',
    followUpHeading: '## Layout — full table layout algorithm',
    priorArt: ['blink (LayoutNG table)', 'servo-stylo (table in Layout 2020)', 'taffy'],
    sketch:
      'Entities with Display::Table* currently warn-once (table_layout stub, sub-pass 6b) and fall back to Display::Block semantics — no row/column geometry. ' +
      'Implement the algorithm from display-and-positioning.md § 1.2: gather entities by Display::Table* family (table / table-row-group / table-row / table-cell / ...); compute column widths via Taffy on a synthetic flex container per row group; write corrected positions back to PostTaffyPositionOverrides (the shared correction buffer all 6a-6f sub-passes use). Keep the warn-once only for genuinely unsupported sub-features (document which). ' +
      'This is the hard one — decompose into incremental TDD tasks: first single-row single-cell geometry, then column-width resolution across rows, then row-group stacking. Read flex-and-grid.md and the existing 6b stub + PostTaffyPositionOverrides plumbing before planning.',
  },
  {
    num: 13,
    slug: 'multicol-layout',
    title: 'Full multi-column layout algorithm',
    specChild: `${SPEC_DIR}/flex-and-grid.md`,
    specSection: '§ 3 (multi-column)',
    followUpHeading: '## Layout — full multi-column layout algorithm',
    priorArt: ['blink (multicol fragmentation)', 'servo-stylo'],
    sketch:
      'Entities with MultiColumn currently warn-once-per-session (multicol_pack stub, sub-pass 6c) and fall back to single-column. ' +
      'Implement a packing pass respecting column_count / column_width (+ break-* properties where present); write each child column position to its PostTaffyPositionOverrides entry. ' +
      'Read flex-and-grid.md § 3 and the existing 6c stub. Decompose: column count resolution (count vs width vs both, CSS used-value algorithm) first, then content distribution across columns, then gap handling. Defer true fragmentation across columns if the spec marks it tier-E — document the residual warn.',
  },
  {
    num: 14,
    slug: 'descendant-invalidation',
    title: 'Multi-level descendant invalidation on ancestor-resolved-size change',
    specChild: `${SPEC_DIR}/container-queries-and-writing-modes.md`,
    specSection: '§ 1.3 and § 1.5',
    followUpHeading: '## Descendant invalidation on ancestor-resolved-size changes',
    priorArt: ['blink (container queries, style/layout invalidation)', 'servo-stylo'],
    sketch:
      'sync_styles uses a per-entity Or<(Changed<T>, ...)> trigger; Bevy has no "ancestor changed" filter, so a Cqw-sized intermediate B between resized ancestor A and rule-bearing descendant C is not re-translated and C never re-evaluates. ' +
      'Implement the follow-ups.md sketch: after write_resolved_layout (step 7), identify query containers (Container { container_type != Normal }) whose ResolvedLayout changed this frame; walk their descendants and mark them for re-translation — either a private ContainerSizeDirty marker the sync_styles Or-filter picks up, or a HashSet<Entity> dirty resource sync_styles checks alongside Changed<>. Trigger a same-frame re-run analogous to cq_flip_rerun. ' +
      'The regression test ALREADY EXISTS as a negative assertion: tests/layout_container_queries.rs::cq_transitive_cascade_is_one_frame_stale asserts C stays Inactive after A resize. When this lands, FLIP its polarity to positive (C becomes Active in-frame) — do not delete it; rename + flip. Read container-queries-and-writing-modes.md § 1.3 and § 1.5 (the A->B->C fixture) before planning.',
  },
]

const only = args && Array.isArray(args.only) ? new Set(args.only) : null
const phases = TARGETS.filter((t) => !only || only.has(t.num))

// ---------------------------------------------------------------------------
const PLAN_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['wrotePlan', 'planPath', 'taskCount', 'committed', 'summary'],
  properties: {
    wrotePlan: { type: 'boolean', description: 'true iff a complete TDD plan file was written' },
    planPath: { type: 'string', description: 'absolute path to the plan .md that was written' },
    taskCount: { type: 'integer', description: 'number of numbered ### Task N code tasks in the plan (T1..TN, all TDD)' },
    committed: { type: 'boolean', description: 'true iff the plan doc was committed' },
    summary: { type: 'string', description: 'one-paragraph description of the plan + the key decision blocks' },
    risks: { type: 'array', items: { type: 'string' }, description: 'risks / parts most likely to halt during implementation' },
  },
}

const DOCS_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['updated', 'filesTouched', 'summary'],
  properties: {
    updated: { type: 'boolean' },
    filesTouched: { type: 'array', items: { type: 'string' } },
    committed: { type: 'boolean' },
    summary: { type: 'string' },
  },
}

const ENV = `Work entirely inside the Buiy worktree: ${WT}. ALL cargo/git commands must run with that directory as the working dir (the repo lives there; never touch the main checkout). Use absolute paths for file edits. This host has NO xvfb — tests run headless via MinimalPlugins, so the gate drops the \`xvfb-run -a\` prefix.`

const planPrompt = (t) => `${ENV}

You are the PLAN AUTHOR for Buiy layout Phase ${t.num}: ${t.title}.

Your job: write a complete, TDD, bite-sized implementation plan that a zero-context engineer can execute task-by-task, then commit it. You are NOT implementing — only planning.

## Grounding (read ALL of this before writing a line of the plan)
1. The spec child this realizes: ${t.specChild} — section ${t.specSection}. This is the canonical target; the plan must match it exactly.
2. The follow-up entry that chartered this work (in ${WT}/docs/plans/follow-ups.md, heading "${t.followUpHeading}"${t.extraFollowUpHeadings ? ' and "' + t.extraFollowUpHeadings.join('", "') + '"' : ''}). Implementation sketch:
   ${t.sketch}
3. Relevant prior-art for the real-engine behavior (folders live under ${WT}/docs/prior-art/): consult ${t.priorArt.join('; ')}. Read what is timely and cite the prior-art folders you used in the plan.
4. The integration surface — READ these files so every task cites real paths/line ranges and matches conventions:
   - ${WT}/crates/buiy_core/src/layout/mod.rs (the PostTaffyOverrides sub-pass CHAIN 6a sticky -> 6b table -> 6c multicol -> 6d anchor -> 6e transform -> 6f stacking; plugin wiring; type registration)
   - ${WT}/crates/buiy_core/src/layout/systems.rs (the sub-pass systems; the shared PostTaffyPositionOverrides resource; LayoutWarnedOnceSession + LayoutWarnOnceKey warn-once idiom)
   - ${WT}/crates/buiy_core/src/layout/translate.rs (Taffy mapping: map_position etc.)
   - ${WT}/crates/buiy_core/src/layout/{style.rs,components.rs,types.rs} (Style builder + setters; decomposed components; value types)
   - the matching test file(s) under ${WT}/crates/buiy_core/tests/layout_*.rs (harness = MinimalPlugins + CorePlugin + LayoutPlugin)
5. Read an existing LANDED plan as the structural template: ${WT}/docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md (Phase 9) — mirror its header, Decision blocks (Dn), Prior-art citations, and per-task TDD step shape.

## Plan requirements (writing-plans discipline)
- Save to: ${WT}/docs/plans/${TODAY}-buiy-layout-${t.slug}.md
- Header: a "# Phase ${t.num}: ${t.title} Implementation Plan" title, the "**For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development ... checkbox (- [ ]) syntax" line, then **Goal:** / **Architecture:** / **Tech Stack:**, then the doc header block (**Date:** ${TODAY} / **Status:** active / **Spec:** specs/2026-05-08-buiy-layout-design/<child>.md), then ---.
- Decision blocks D1..Dn for every non-obvious choice, each naming the runner-up and why rejected.
- Tasks numbered EXACTLY "### Task 1", "### Task 2", ... "### Task N" with NO gaps. EVERY task is pure TDD CODE (there is NO "write the plan" meta-task — you are writing it now; docs flips happen in a separate stage, so NO task should edit the spec/README/follow-ups). Each task has a **Files:** block (Create/Modify with line refs/Test) and checkbox steps: (1) write the failing test [FULL test code], (2) run it, exact cmd + expected FAIL, (3) minimal implementation [FULL code], (4) run it, exact cmd + expected PASS, (5) commit [exact git add + git commit].
- NO placeholders: no "TBD", no "add error handling", no "similar to Task N" — repeat the code. Every code step shows the actual code. Every type/fn used in a later task must be defined in an earlier task.
- The gate command (headless, this host): \`${GATE_CMD}\`
- End with the writing-plans Self-Review (spec coverage / placeholder scan / type consistency); fix anything you find before committing.

## Finish
- git add the plan file and commit it (message: "docs: Phase ${t.num} layout plan — ${t.title} [active]"). Do NOT touch any other file.
- Return the structured result. planPath = absolute path. taskCount = the number of ### Task N entries (all TDD code tasks). risks = the tasks most likely to halt and why.`

const docsPrompt = (t, planPath) => `${ENV}

Buiy layout Phase ${t.num} (${t.title}) has just been IMPLEMENTED and the workspace gate is green. Your job is the DOCS closeout — update the canonical docs to reflect that this is now landed, and commit. Do NOT change any Rust code.

1. Spec child ${t.specChild} (${t.specSection}): the behavior is now realized. Find the deferral / "stored, not enforced" / "stub" / "v1 limitation" wording that described this gap and update it to describe the SHIPPED behavior (state it is implemented; keep any genuinely-still-deferred sub-features explicit). If the child has a "v1 implementation status" section, add/update the relevant bullet. Do not contradict other sections — supersede cleanly.
2. ${WT}/docs/README.md — under "### Layout" > "**Plans**", add a one-line catalog entry for ${planPath} (pattern: "- [Title](plans/${TODAY}-buiy-layout-${t.slug}.md) — 5-15 word summary. \`[landed]\`").
3. ${WT}/docs/plans/follow-ups.md — the entry under "${t.followUpHeading}"${t.extraFollowUpHeadings ? ' (and "' + t.extraFollowUpHeadings.join('", "') + '")' : ''}: mark it LANDED — set a **Status:** **Landed** in Phase ${t.num} line and link the plan ${planPath} (mirror how the "Phase 9 stacking sub-pass 6f ... LANDED" entry already in that file is written). Do not delete the entry; convert it.
4. Also flip the plan file ${planPath} header **Status:** active -> landed.
5. git add the touched docs and commit (message: "docs: mark Phase ${t.num} layout (${t.title}) [landed]").

Return the structured result.`

// ---------------------------------------------------------------------------
log(`Overnight layout follow-ups: ${phases.length} phase(s) — ${phases.map((p) => 'P' + p.num).join(', ')}. Worktree: ${WT}`)

const report = []

for (const t of phases) {
  // ---- Stage 1: design the plan -----------------------------------------
  phase('Plan')
  log(`Phase ${t.num} (${t.title}): authoring TDD plan.`)
  const plan = await agent(planPrompt(t), {
    label: `plan:P${t.num}`,
    phase: 'Plan',
    agentType: 'general-purpose',
    schema: PLAN_SCHEMA,
  })

  if (!plan || !plan.wrotePlan || !plan.committed || !plan.planPath || !plan.taskCount) {
    log(`Phase ${t.num}: plan author did not produce a committed plan (wrotePlan=${plan?.wrotePlan}, committed=${plan?.committed}). Skipping phase.`)
    report.push({ num: t.num, title: t.title, stage: 'plan', ok: false, detail: plan?.summary || 'no plan produced' })
    continue
  }
  log(`Phase ${t.num}: plan at ${plan.planPath} with ${plan.taskCount} task(s). Risks: ${(plan.risks || []).join(' | ') || 'none flagged'}`)

  // ---- Stage 2: implement via the proven layout-phase-impl driver --------
  phase('Implement')
  const tasks = Array.from({ length: plan.taskCount }, (_, i) => i + 1)
  let impl = null
  try {
    impl = await workflow(
      { scriptPath: `${WT}/.claude/workflows/layout-phase-impl.js` },
      { worktree: WT, plan: plan.planPath, tasks, gate: true, gateCmd: GATE_CMD },
    )
  } catch (e) {
    log(`Phase ${t.num}: layout-phase-impl threw (${String(e).slice(0, 200)}). Recording and continuing.`)
    report.push({ num: t.num, title: t.title, stage: 'implement', ok: false, planPath: plan.planPath, detail: 'driver threw' })
    continue
  }

  const halted = impl && impl.haltedAtTask != null
  const completed = (impl && impl.completed) || []
  const gateText = (impl && impl.gate) || ''
  // The driver runs the workspace gate only when nothing halted; treat a clean
  // run (no halt, all requested tasks approved) as built.
  const built = !halted && completed.length === tasks.length

  if (!built) {
    log(`Phase ${t.num}: NOT fully built (haltedAtTask=${impl?.haltedAtTask}, completed=${completed.length}/${tasks.length}). Leaving spec deferral in place; continuing to next phase on the clean base.`)
    report.push({
      num: t.num, title: t.title, stage: 'implement', ok: false, planPath: plan.planPath,
      haltedAtTask: impl?.haltedAtTask, completed, taskCount: tasks.length, perTask: impl?.perTask,
    })
    continue
  }

  // ---- Stage 3: docs closeout (only when fully built + gate ran) ---------
  phase('Docs')
  log(`Phase ${t.num}: built + gate ran (${gateText.slice(0, 120)}). Flipping spec/README/follow-ups.`)
  const docs = await agent(docsPrompt(t, plan.planPath), {
    label: `docs:P${t.num}`,
    phase: 'Docs',
    agentType: 'general-purpose',
    schema: DOCS_SCHEMA,
  })

  report.push({
    num: t.num, title: t.title, stage: 'done', ok: true, planPath: plan.planPath,
    taskCount: tasks.length, completed, gate: gateText, docs,
  })
  log(`Phase ${t.num}: DONE (${tasks.length} tasks, docs updated=${docs?.updated}).`)
}

return {
  worktree: WT,
  branch: 'worktree-layout-followups',
  phasesAttempted: phases.map((p) => p.num),
  phasesDone: report.filter((r) => r.ok).map((r) => r.num),
  phasesIncomplete: report.filter((r) => !r.ok).map((r) => ({ num: r.num, stage: r.stage, haltedAtTask: r.haltedAtTask })),
  report,
}
