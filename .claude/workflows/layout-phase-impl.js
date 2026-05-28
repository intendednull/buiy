export const meta = {
  name: 'layout-phase-impl',
  description: 'Execute a Buiy layout phase plan task-by-task: TDD implement -> spec+quality review -> fix-once -> stop-on-blocker',
  whenToUse: 'Drive a written Buiy layout implementation plan (e.g. the Phase 8 transforms+containment plan) to completion under subagent-driven-development discipline. Pass args.plan (absolute path) and optionally args.tasks (subset of task numbers) and args.gate (run the full workspace gate at the end).',
  phases: [
    { title: 'Implement', detail: 'one implementer subagent per task (TDD + commit)' },
    { title: 'Review', detail: 'spec-compliance + code-quality review per task; one fix iteration; stop on unresolved blocker' },
    { title: 'Gate', detail: 'optional full workspace gate after the last task' },
  ],
}

// ---------------------------------------------------------------------------
// Parameters (via the Workflow `args` input)
//   args.plan  : absolute path to the plan .md (default = Phase 8 plan)
//   args.tasks : array of task numbers to run, in order (default = T1..T14)
//   args.gate  : if true, run the full CLAUDE.md project gate after the last task
// ---------------------------------------------------------------------------
const WT = '/mnt/storage/projects/buiy/.claude/worktrees/layout-finish'
const PLAN = (args && args.plan) || `${WT}/docs/plans/2026-05-28-buiy-layout-transforms-containment.md`
const TASKS = (args && args.tasks) || [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
const RUN_GATE = !!(args && args.gate)

const GATE = `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace`

const IMPL_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['taskDone', 'committed', 'testsPassed', 'summary', 'unresolved'],
  properties: {
    taskDone: { type: 'boolean', description: 'true iff every TDD step of the task was completed' },
    committed: { type: 'boolean', description: 'true iff the task commit step was run' },
    testsPassed: { type: 'boolean', description: 'true iff the task-level cargo tests passed' },
    commitSha: { type: 'string', description: 'short SHA of the commit, or empty' },
    summary: { type: 'string', description: 'what was implemented + any deviation from the task and why' },
    unresolved: { type: 'array', items: { type: 'string' }, description: 'anything left incomplete or that failed' },
  },
}

const REVIEW_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['approved', 'blockers'],
  properties: {
    approved: { type: 'boolean', description: 'true iff the task is correct, spec-compliant, and clean' },
    blockers: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['severity', 'detail', 'fix'],
        properties: {
          severity: { type: 'string', enum: ['blocker', 'concern'] },
          detail: { type: 'string' },
          fix: { type: 'string', description: 'concrete instruction to resolve it' },
        },
      },
    },
  },
}

const ENV = `Work entirely inside the Buiy worktree: ${WT}. ALL cargo/git commands must run with that directory as the working dir (the repo lives there; do not touch the main checkout). Use absolute paths for file edits. The plan is at ${PLAN}.`

const implPrompt = (n) => `${ENV}

You are the IMPLEMENTER for Task ${n} of this plan, working under TDD (superpowers:subagent-driven-development).

Steps:
1. Read the plan file ${PLAN} and locate "### Task ${n}". Read it in full, plus the Decision blocks and Prior-art citations it references.
2. Execute EVERY checkbox step of Task ${n} in order: write the failing test, run it to confirm it fails, write the minimal implementation, run the test to confirm it passes, then run any task-level gate the step specifies, then the commit step.
3. Match the existing code's conventions exactly (naming, derives, doc-comment style, no bevy::utils::HashMap, etc.). Honor every Decision block (Dn) the task cites — do not re-decide.
4. If a step gives an implementer-judgment fallback (e.g. an import path to grep, a writing-mode accessor name), resolve it by reading the actual code and pick the documented fallback if unsure.
5. Do NOT implement any task other than Task ${n}. Do NOT mark the plan checkboxes. Stop after the Task ${n} commit.

Return the structured result. testsPassed must reflect the ACTUAL cargo test outcome you observed (run it; do not assume). If you could not complete a step, set taskDone=false and list what failed in unresolved.`

const reviewPrompt = (n) => `${ENV}

You are the REVIEWER for Task ${n}. You did not write this code. Be adversarial: assume there is a bug or a spec deviation until you have verified otherwise.

1. Read "### Task ${n}" in ${PLAN} and the spec sections it cites (under docs/specs/2026-05-08-buiy-layout-design/).
2. Inspect the work: run \`git -C ${WT} log --oneline -3\` and \`git -C ${WT} show --stat HEAD\` and read the actual diff of the task's commit, plus the final state of the files it touched.
3. Verify, by reading the code (not assuming):
   - Spec compliance: the types/systems/behavior match the cited spec sections exactly (field names, derives, defaults, composition order, sub-pass placement, warn-once keys).
   - Tests genuinely assert behavior (not tautologies / not testing mocks) and actually ran green — re-run the task's cargo test command yourself and confirm.
   - No placeholders, no TODO-shaped gaps, no dead code, conventions match the surrounding crate.
   - The Decision blocks the task cites are honored.
4. Return approved=true only if all of the above hold. Otherwise list blockers (severity "blocker" = must fix before proceeding; "concern" = note but not fatal), each with a concrete fix instruction.`

const fixPrompt = (n, blockers) => `${ENV}

You are the IMPLEMENTER addressing review blockers on Task ${n}. The reviewer found these BLOCKERS:
${blockers.map((b, i) => `${i + 1}. [${b.severity}] ${b.detail}\n   FIX: ${b.fix}`).join('\n')}

Resolve every blocker by editing the code/tests in ${WT}, re-run the task's cargo tests to confirm green, and amend or add a follow-up commit (your choice; prefer \`git commit --amend\` if the task commit is the latest and unpushed, else a fixup commit). Do not expand scope beyond fixing these blockers. Return the structured implementer result.`

// ---------------------------------------------------------------------------
phase('Implement')
log(`Driving ${TASKS.length} task(s) of ${PLAN} sequentially (implement -> review -> fix-once -> stop-on-blocker).`)

const results = []
let halted = null

for (const n of TASKS) {
  // 1. Implement (TDD + commit).
  let impl = await agent(implPrompt(n), { label: `impl:T${n}`, phase: 'Implement', agentType: 'general-purpose', schema: IMPL_SCHEMA })

  if (!impl || !impl.taskDone || !impl.testsPassed) {
    log(`T${n}: implementer did not finish cleanly (taskDone=${impl?.taskDone}, testsPassed=${impl?.testsPassed}). Halting.`)
    results.push({ task: n, impl, review: null })
    halted = n
    break
  }

  // 2. Review (spec + quality), adversarial.
  let review = await agent(reviewPrompt(n), { label: `review:T${n}`, phase: 'Review', agentType: 'general-purpose', schema: REVIEW_SCHEMA })
  const hasBlocker = (r) => r && !r.approved && (r.blockers || []).some((b) => b.severity === 'blocker')

  // 3. One fix iteration if there are blockers.
  if (hasBlocker(review)) {
    const blk = review.blockers.filter((b) => b.severity === 'blocker')
    log(`T${n}: ${blk.length} blocker(s) — running one fix iteration.`)
    const fixed = await agent(fixPrompt(n, blk), { label: `fix:T${n}`, phase: 'Implement', agentType: 'general-purpose', schema: IMPL_SCHEMA })
    impl = fixed || impl
    review = await agent(reviewPrompt(n), { label: `review2:T${n}`, phase: 'Review', agentType: 'general-purpose', schema: REVIEW_SCHEMA })
  }

  results.push({ task: n, impl, review })

  // 4. Stop on an unresolved blocker — do not build later tasks on a broken base.
  if (hasBlocker(review)) {
    log(`T${n}: still blocked after fix iteration. Halting so a human can intervene.`)
    halted = n
    break
  }
  log(`T${n}: done (${impl.commitSha || 'committed'}), review approved=${review?.approved}.`)
}

// 5. Optional full project gate after the last completed task.
let gate = null
if (RUN_GATE && !halted) {
  log('Running the full CLAUDE.md project gate (fmt + clippy + doc + xvfb test)...')
  gate = await agent(
    `${ENV}\n\nRun the full project gate from the worktree root and report the result verbatim (do not fix anything — just run and report):\n\n${GATE}\n\nReturn a short summary: did it pass? If not, quote the first failure.`,
    { label: 'gate', phase: 'Gate', agentType: 'general-purpose' }
  )
}

return {
  plan: PLAN,
  tasksRequested: TASKS,
  haltedAtTask: halted,
  completed: results.filter((r) => r.review && r.review.approved).map((r) => r.task),
  perTask: results.map((r) => ({
    task: r.task,
    done: r.impl ? r.impl.taskDone : false,
    committed: r.impl ? r.impl.committed : false,
    testsPassed: r.impl ? r.impl.testsPassed : false,
    approved: r.review ? r.review.approved : null,
    blockers: r.review ? (r.review.blockers || []).filter((b) => b.severity === 'blocker').map((b) => b.detail) : [],
    unresolved: r.impl ? r.impl.unresolved : [],
  })),
  gate,
}
