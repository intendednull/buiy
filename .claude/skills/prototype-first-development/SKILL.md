---
name: prototype-first-development
description: Use for substantial, high-uncertainty work where you can't write a confident final spec until you've built and RUN something — new framework capabilities, unfamiliar internals/APIs, exact visual/behavioral parity, "we don't know what we don't know." Suggest it whenever a change will take substantial work.
---

# Prototype-first development

## Overview

For high-uncertainty work, the first "real" build IS a throwaway whether you
admit it or not — your only choice is discovering your wrong assumptions in a
deliberate prototype or in production. So build the prototype on purpose, **to
learn**, then build the final with the full picture.

**A prototype's product is LEARNING, not code.** Its real deliverables are a
**journal** and a **structured retrospective**. The code is an unmerged
reference.

## When to use

Explicit; **suggest it for any substantial change**. The trigger is *epistemic*,
not size — you genuinely can't write a confident final spec without building and
running first. Symptoms:

- The plan needs capabilities / internals / APIs you haven't built or used.
- The target is exact parity (visual, behavioral) you can only judge by running it.
- Estimates are guesses; you keep saying "we'll find out when we build it."

If the path is already clear, don't prototype — run `staged-development` once.

## The shape: two passes + a learning gate

```
PROTOTYPE (to learn) ──▶ RETROSPECTIVE (the deliverable) ──▶ FINAL (re-decided, production)
  throwaway worktree         keep / refine / redesign          merge-gated on human review
  DO NOT MERGE
```

Each pass IS the full `staged-development` pipeline (research → spec → plan →
execute, gated, fan-outs under `reliable-agent-fleet`). This skill only sequences
it twice, with a learning gate between.

### Phase A — Prototype (to learn)
- Run `staged-development` in an **isolated worktree cut from the same base as
  the eventual final** (see Mechanics). Optimize for *learning + speed-to-running*,
  not polish. **Commit freely as you build** (the commits are what make the
  audited port possible) — but **DO NOT PUSH OR MERGE** the prototype.
- **Keep a journal** (template below) — append every wave: what you built, what
  broke, what surprised you, what you'd do differently.
- **Run the artifact every wave** (see Non-negotiables) — where the real bugs surface.

### Gate — Retrospective (the prototype's real output)
Synthesize the journal into a structured retrospective (template below): for
every decision, **keep / refine / redesign** + rationale; plus **validated
decisions**, **residual gaps**, and the **framework/system bugs the prototype
surfaced**. This seeds the final's research.

### Phase B — Final (re-decided, production)
- Run `staged-development` AGAIN, its **research seeded by the retrospective**.
  **Re-decide every choice** — the prototype decided sequentially, blind to
  downstream; the final has the whole picture. (Re-decision routinely catches the
  prototype's — and even the spec's — wrong premises.)
- Execute as a **hybrid: port the validated, redesign the pressure points** (see
  Mechanics).
- **Commit as you go onto the branch; don't push, merge, or open a PR until
  explicitly asked.** Merge-gate on human review — don't self-merge.

## The concrete mechanics (the battle-tested playbook)

- **Two worktrees, shared base.** Cut the prototype and the final from the *same*
  base commit. That makes the prototype's validated commits cherry-pickable into
  the final — an **audited port** (neither "rebuild from scratch" nor "copy
  blindly").
- **The prototype never merges.** It's a reference; the final is the deliverable.
- **Hybrid port.** In the final, port the keep-decisions (cherry-pick / checkout
  the validated code) and re-implement the refine/redesign-decisions as deliberate
  commits. Re-evaluate each piece — keep what's right *with a re-derived rationale*.
- **Durable journal.** It's the cross-phase bridge AND survives context loss —
  write it as you go, not after.
- **Sequential-in-warm-worktree for interdependent work.** Cold-cache isolated
  worktrees per agent + merging interdependent core changes = slow + conflict hell.
  Drive interdependent waves sequentially in one warm worktree; reserve isolated
  parallel worktrees for genuinely independent work.

## Non-negotiables (what only shows up the hard way)

- **RUN THE ARTIFACT every wave — don't trust green gates.** Headless tests and an
  agent's self-reported "green" miss the bugs that only manifest when you run the
  real thing: crashes in the un-tested runtime, invisible output, layout overflow,
  wrong colors, motion that snaps. Running it is the prototype's whole point.
- **Verify-don't-trust at every gate.** Run the gate yourself; don't trust agent
  self-reports. Stale editor/LSP diagnostics lie — force a real recompile before
  believing a "broken" signal.
- **The final re-decides, it doesn't copy.** keep/refine/redesign every decision
  with the full picture.
- **Preserve the learning — COMMIT it to the durable docs system.** The journal,
  retrospective, research synthesis, and any `prior-art/` folders are the
  prototype's real deliverables — they belong in the project's *committed* `docs/`
  (carried by the final's PR, or a dedicated docs PR), **not** stranded in the
  throwaway worktree. Only the prototype CODE is the throwaway. A retrospective or
  prior-art folder that lives only in a worktree is one `git worktree remove` from
  gone — and worktrees DO get cleaned up. **Before removing the prototype worktree,
  confirm its docs are on the main branch; if not, open a docs PR first.**

## Anti-patterns

- **Merging the prototype** — it's the learning, not the product.
- **Copying the prototype into the final unaudited** — re-decide; it was blind to
  downstream.
- **Treating prototype CODE as the deliverable** — the journal + retrospective are.
- **Trusting green tests without running the artifact** — the bugs hide exactly there.
- **Polishing the prototype** — speed-to-running + learning beats polish; polish
  the final.
- **Letting the journal / retrospective / research / prior-art die in the throwaway
  worktree** — they're the deliverable, not the code. If they aren't committed to
  the durable repo, cleaning up the worktree destroys the learning. Commit the docs
  (the code stays unmerged).

## Templates

### Journal skeleton (append every wave)
```markdown
# <Project> — Prototype Dev Journal
> PROTOTYPE — exploratory, DO NOT MERGE. The deliverable is this journal + the retrospective.
Goal: <what / why>.  Worktree: <branch, off base <sha>>.  Target/reference: <link>.

## Running log
### <date> — <wave>
- Built: ...
- Ran the artifact → found: ...        (← bugs only visible by running)
- Surprised by / friction: ...
- If we did this again: ...
```

### Retrospective (the gate — seeds the final's research)
```markdown
## Prototype retrospective — for the final

### Verdict
<Is the target achievable? how close did the prototype get? gate/verification status.>

### Validated — KEEP (port as-is, re-derive the rationale)
- <decision> — why it's right.

### REFINE / REDESIGN (the final does these differently)
- <decision> — prototype did X; final does Y; why (the full-picture reason).

### Framework/system BUGS the prototype surfaced (found by running it)
- <bug> — root cause + how the final fixes it.

### Residual gaps for the final to close
- <gap>.

### Build strategy
- Port the keep work (shared base → cherry-pick); implement refine/redesign cleanly.
```

## Pairs with

`staged-development` (each phase IS that pipeline) · `reliable-agent-fleet`
(fan-outs) · `verification-before-completion` (the run-the-artifact gate) ·
`subagent-driven-development` (execution).
