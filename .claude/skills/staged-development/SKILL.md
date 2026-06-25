---
name: staged-development
description: Use when starting any non-trivial change — features, refactors, new subsystems, migrations — before writing implementation code. Especially when tempted to jump straight to coding because the task "looks clear."
---

# Staged development: research → spec → plan → execute, gated at every stage

## Overview

Don't jump straight to code. Run a staged pipeline where each stage produces an
**artifact** that feeds the next, and **fresh-context review agents gate every
transition**. The only off-ramp is genuinely trivial work (a one-line fix needs
no spec). When in doubt, prefer the structure.

**Why staged:** unexamined assumptions are cheapest to fix in a spec and most
expensive to fix in shipped code. Each gate catches drift before it compounds.

## The pipeline

| Stage | Produces | Driven by |
|---|---|---|
| **1. Research** | context — codebase, existing specs/plans/prior-art, **external prior art** | `researching-prior-art` / `using-prior-art` |
| **2. Spec** | target-state design `docs/specs/YYYY-MM-DD-<name>-design.md` — what + why, tradeoffs + **rejected alternatives named** | `brainstorming` |
| **3. Plan** | step-by-step implementation plan `docs/plans/YYYY-MM-DD-<name>.md` — how we get there | `writing-plans` |
| **4. Execute** | the change, in small verifiable units | `subagent-driven-development` / `executing-plans` |

Research feeds the spec; the approved spec feeds the plan; the plan drives
execution. Don't skip stages or run them out of order.

## Fan out under reliable-agent-fleet

Whenever a stage fans out parallel agents (research sweeps, multi-unit execution,
review panels), run them under **`reliable-agent-fleet`**: structured-output
contracts, count returns against spawns, retry the holes, never synthesize from
partial coverage. Partial coverage masquerading as complete is the failure it
prevents.

## Gate every stage AND every wave

After research, after the spec, after the plan, and after each implementation
wave, dispatch **fresh-context** agents to review — logic, correctness, quality,
design soundness, spec/plan alignment. How many and which kind is your call.
**Never carry unreviewed work into the next stage.**
→ `requesting-code-review` / `code-review`.

## Verify, don't just read

A review must confirm the code actually DOES what it's meant to — not only that
it reads well. Use the project's own pipeline: test suite, run/build/check
commands, visual/golden harnesses, manual UI checks. **Finding what verification
looks like for THIS project is part of the workflow** — read its testing docs
first. When you discover a verification strategy that isn't written down, document
it for the next change.
→ `verification-before-completion`.

## Quality over token cost

Quality and developer experience matter far more than token usage. Spend the
research, the extra reviewers, the deeper passes freely. A more correct, cleaner,
better-reviewed result is the right trade.

## Common mistakes

- **Jumping to code** because "it's clear enough" — clear-looking tasks are where
  unexamined assumptions cost the most. Write the short spec.
- **Skipping the gates** to move faster — unreviewed drift compounds; the gate is
  cheaper than the rework.
- **Reviewing by reading, not running** — a clean-looking diff can hide a broken
  artifact. Verify against the running thing.
- **One giant execution step** — fan out small, verifiable units; gate each wave.

## Pairs with

For substantial, **high-uncertainty** work where you can't write a confident spec
until you've built and run something, run this pipeline TWICE under
`prototype-first-development` — a throwaway prototype to learn, then a re-decided
final.
