export const meta = {
  name: 'layout-phase-9',
  description: 'Run Buiy layout Phase 9 (stacking + top layer) end-to-end via the reusable layout-phase-impl driver',
  whenToUse: 'Execute the Phase 9 stacking + top-layer plan (docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md) task-by-task under subagent-driven-development. No args needed; it delegates to layout-phase-impl with the Phase 9 plan path, the T1..T13 task list, and a headless (no-xvfb) project gate. Override args.tasks to run a subset.',
  phases: [
    { title: 'Implement', detail: 'one implementer subagent per task (TDD + commit)' },
    { title: 'Review', detail: 'spec-compliance + code-quality review per task; one fix iteration; stop on unresolved blocker' },
    { title: 'Gate', detail: 'full workspace gate (headless, no xvfb) after the last task' },
  ],
}

// ---------------------------------------------------------------------------
// Phase 9 = stacking + top layer (sub-pass 6f). This is a thin, DRY wrapper
// over the reusable `layout-phase-impl` driver: it bakes in the Phase 9 plan
// path, the task list, and a gate command with `xvfb-run -a` dropped (this
// host has no xvfb; CLAUDE.md says drop the prefix — tests run headless via
// MinimalPlugins). Edit `args.tasks` to run a subset (e.g. resume at T8).
//
// T1 (plan doc + spec § 7 + docs/README entry) is completed at planning
// time — committed before this workflow runs — so the default task list
// starts at T2 (the first code task). Pass args.tasks to override (e.g.
// `[1,...,13]` to re-run T1, or `[8,9]` to resume mid-implementation).
//
// Parameters (via the Workflow `args` input, all optional):
//   args.tasks : subset of task numbers to run, in order (default = T2..T13)
//   args.gate  : run the final workspace gate (default = true)
// ---------------------------------------------------------------------------
const WT = '/mnt/storage/projects/buiy/.claude/worktrees/layout-finish'
const PLAN = `${WT}/docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md`
const TASKS = (args && args.tasks) || [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]
const RUN_GATE = args && typeof args.gate === 'boolean' ? args.gate : true

// Headless gate — identical to the CLAUDE.md "run all checks" minus the
// `xvfb-run -a` prefix (absent on this host).
const GATE_CMD = `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace`

log(`Phase 9 (stacking + top layer): delegating ${TASKS.length} task(s) to layout-phase-impl.`)

return await workflow(
  { scriptPath: `${WT}/.claude/workflows/layout-phase-impl.js` },
  { plan: PLAN, tasks: TASKS, gate: RUN_GATE, gateCmd: GATE_CMD },
)
