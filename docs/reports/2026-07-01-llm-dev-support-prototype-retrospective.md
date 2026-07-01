**Date:** 2026-07-01
**Status:** active

# First-Class LLM Development Support — Prototype Retrospective

> The gate output of the prototype-first pass (companion to the
> [journal](2026-07-01-llm-dev-support-prototype-journal.md) and the
> [research/audit](2026-07-01-llm-development-support-research.md)). Prototype CODE
> (`examples/llm_probe` + the experiment scenes) stays **unmerged**; this document +
> the journal are the deliverables. This seeds the FINAL design brainstorm.

## Verdict

**The target is achievable, and the highest-leverage move is cheaper than expected.**
The prototype built and RAN the Track-A feedback loop and used it to measure real LLM
agents authoring Buiy UIs. Three things are now settled that the research could only
hypothesize:

1. **The agent's "eyes" (headless run + semantic-tree snapshot) are real and GPU-free.**
   `a11y::inprocess::snapshot` already exists (agent-interface Phase 0); a no-render
   headless app projects role / name / state / layout / text as pure ECS data. The
   expensive-looking headless-render path is **unnecessary** for the loop.
2. **A capable agent + source access is a HIGH baseline for static correctness** (E1: it
   authored the F1-coupled checkbox-checked path correctly first try). So Track A's value
   is **not** generic authoring help — it is the **runtime / silent-wrong class** the
   compiler and source-reading cannot reveal, plus realistic constrained-context agents.
3. **A prelude-only agent hallucinates *exactly the prior-matching ergonomic API the
   research prescribes*** (E3, keystone): `.checked(true)`, a typed `ColorToken::Surface`,
   `.with_children(|c| …)`. **The model wrote the target design for us.** Buiy's actual
   forms (stringly `Token("…")`, foreign `A11yToggled(Toggled::True)`, `.id()`+`add_children`)
   are the mismatch.

## Validated — KEEP (port with a re-derived rationale)

- **Track A: the semantic-tree loop as a first-class agent tool.** VALIDATED end-to-end
  (built + run). KEEP, and build it on the *existing* `a11y::inprocess::{snapshot, perform,
  get_by_role, click, …}` — do not reinvent. The FINAL should expose it as (a) a
  `BuiyProbePlugin` / headless-no-render preset (composing the 8 sub-plugins minus render —
  see bug #1) and (b) a text/JSON dump the agent's build/test command returns.
- **P1c — the compiler is the agent's feedback channel.** VALIDATED (E3: all 6
  prior-mismatches were compile errors, not silent-wrong). KEEP the principle: push defects
  to compile/load time wherever possible.
- **Track A + Track B are complementary, not redundant.** The GPU-free loop sees
  structure/state/layout/text but **not pixels** (color/paint). KEEP both; do not try to
  make one cover the other.
- **The four-track decomposition and the P1–P5 principles** survive contact with the
  experiments intact. KEEP.

## REFINE / REDESIGN (the FINAL does these differently, with the full picture)

- **Track D reframed: "match popular-neighbor priors" is now a concrete spec, not advice.**
  The prelude-only agent literally emitted `.checked(true)` / `ColorToken::Surface` /
  `.with_children`. The FINAL's naming/shape decisions should be **derived from what
  constrained agents actually generate** (a cheap, repeatable probe: prompt a prelude-only
  agent, diff its guesses against reality). REDESIGN the "borrow correctness" track around
  this measured-priors method.
- **Track C accessor shape is a genuine FORK, not a given.** `.checked(true)` fights Buiy's
  `Checkbox::new(..) -> impl Bundle` (marker + `#[require]`) model. The FINAL must decide the
  ONE canonical constructor form (builder vs Bundle vs Scene) *before* adding accessors —
  this is F5/F4 territory and must be decided WITH the owner. Do not assume a builder.
- **Track B typed tokens are the cheapest, highest-value, lowest-risk fix — do it first.**
  Unlike the accessor, swapping the `ColorToken` `Cow<str>` key for a closed enum does **not**
  fight the constructor model. It (a) makes the model's `ColorToken::Surface` guess *correct*,
  (b) turns the magenta silent-wrong into a compile error, and (c) removes the F6 feature-
  omission failure (see bug #3). REFINE to: build B first, as the flagship demonstration.
- **The loop's framing must be precise.** Sell Track A on the **runtime/silent-wrong +
  constrained-agent** case (where it uniquely wins), not "authoring help in general" (E1
  shows a strong agent + source is already good statically).

## Framework/behavioral facts the prototype surfaced (by running it)

1. **No GPU-free headless preset exists.** `BuiyHeadlessPlugin` pulls `BuiyRenderPlugin`
   (wants a RenderApp), so a pure layout+a11y run means hand-listing 8 `buiy_core::*` +
   `buiy_widgets` sub-plugins, none reachable via `buiy::prelude`. → FINAL: ship a
   `BuiyProbePlugin` (or headless-no-render preset) as part of Track A.
2. **Plain `Text` is invisible to the semantic tree** (no a11y node) — an agent verifying
   title/label content via the tree alone can't see it. → FINAL: decide whether decorative
   text carries a `text` a11y node, or whether the probe's text-content channel is the
   contract.
3. **F6 stringly tokens cause TWO agent-failure modes, both confirmed live:** (a) hallucinate
   a plausible variant/string that ships MAGENTA (invisible to the GPU-free loop), and (b)
   under the compiler loop, the agent **silently DROPS the styled feature** (E3b deleted the
   card background) to reach green — silent visual degradation with no signal. Typed tokens
   kill both.
4. **`bevy::prelude` vs `buiy::prelude` glob-collide on `Text`** (and `Node`) — an F-class
   ambiguity that bit the harness author and both experiment agents.
5. **`Commands` / MVU / `A11yToggled` / `Toggled` are not in `buiy::prelude`** — confirmed
   the N1 friction live (also already annotated in `hello_button/Cargo.toml`'s "PROTOTYPE
   DX-1" comment). A prelude-only agent cannot name the checked-state symbol at all.
6. **`buiy::prelude::*` alone cannot express a Bevy system** (found by the MVU probe) — it
   re-exports none of Bevy's ECS authoring essentials (`Component` derive, `Commands`, `Query`,
   `MessageReader`, `With`, `Camera2d`), so a prelude-only agent MUST add `use bevy::prelude::*`,
   which then glob-collides on `Text`/`Node`. The curated prelude (Track C) must re-export the
   ECS essentials + resolve the collision; this is load-bearing, not a nicety.

## Broadened validation (N=4 tasks — card / MVU / list / events)

After the owner resumed the loop, three more prelude-only probes extended the single-task
keystone to four tasks. The pattern held and sharpened:

- **F6 stringly `ColorToken` is the ONE deterministic failure in EVERY task** (E3, plist, pevents;
  pmvu blocked earlier). Agents always guess a semantic variant (`Surface`/`Card`/`Text`) — the exact
  shape a closed enum gives. → **Track B (closed enum + contract) is the confirmed flagship**: it turns
  every one of these guesses into correct code, and it fights no existing model.
- **F1 domain accessor:** the events probe reached for `checkbox.checked` + `Changed<Checkbox>`
  reaction (both wrong — unit marker, state on `A11yToggled`). → Track C accessor + typed value-events.
- **N1:** MVU invisible from the prelude (raw-Bevy fallback) + fact #6 above. → curated prelude.
- **F7:** static `Vec`→UI via an imperative `.with_children` loop **compiled fine** — the real F7 pain
  is *reactive* lists (deferred D6/D7), not static data.
- **Meta:** prior-matching is **probabilistic** for fluent-builder spellings (plist guessed `Style`
  right, pmvu wrong) but the token / non-prelude-symbol / missing-accessor failures are
  **deterministic**. Close the deterministic ones first (Tracks B, C). The measured-priors probe (P5)
  is confirmed cheap + decisive across 4 tasks — keep it as the FINAL's acceptance rig.

## Feasibility spikes — ALL PROVEN first-hand (R1/R2/R3)

The owner's fork decisions + the riskiest implementation questions were de-risked by building + running:

- **R1 — full runtime feedback loop:** a REAL click through the in-process driver (`get_by_role`+`click`
  → `OnPress`, GPU-free) on two counters: correct wiring `0→1` (loop OBSERVES), buggy compile-green
  wiring stays `0` (loop CATCHES). Track A's core thesis is now demonstrated, not asserted.
- **R2 — typed tokens + theme contract:** closed `ColorToken` enum + exhaustive-`match` `ThemeContract`.
  `Surface` resolves to a real color; a typo → E0599, a missing token → **E0004 (demonstrated first-hand)**.
  Track B feasible; turns the agents' universal `ColorToken::Surface` guess into correct code.
- **R3 — Builder→Bundle children:** a flat `#[derive(Bundle)]` builder + an `On<Add, Marker>` observer
  attaches the visible child; author wires none. Resolves §3.1's open sub-detail.

## Residual gaps for the FINAL to close (post-spikes)

- ~~canonical-constructor fork~~ — **RESOLVED:** Builder→Bundle (proven; children via on-add observer).
- ~~typed-token design feasibility~~ — **RESOLVED:** closed enum + exhaustive-match contract (proven).
  Migration mechanics from the stringly path + the dynamic-color escape hatch remain plan-level.
- **Pixel-class detection** — the GPU-free loop can't see color/paint; remedy is prevent (typed tokens +
  camera-panic) with an optional GPU pixel-readback lane deferred. (Confirmed unchanged.)
- **Curated prelude + `AGENTS.md`/`llms.txt` + compiling example cookbook** — not built;
  scope + the accuracy-gating CI owner still open.
- **Broader task coverage** — the prototype measured one representative task (a stateful
  card). The FINAL's measured-priors probe should span events (F2), dynamic lists (F7), and
  MVU authoring (N1) before finalizing shapes.

## Build strategy for the FINAL (hybrid port)

Shared base `origin/main 31bbbc6`, so the prototype's docs cherry-pick cleanly (the code
does not — it is throwaway).

1. **Docs first:** carry the research report + journal + this retrospective onto a branch /
   docs PR so the learning survives worktree cleanup (mandatory per prototype-first).
2. **Brainstorm the FINAL with the owner** to resolve the two forks (constructor shape;
   token design) — the reason the loop STOPPED here.
3. **Build order (re-decided):** Track B typed tokens (cheapest, no model conflict, flagship)
   → Track A `BuiyProbePlugin` + agent-facing snapshot (the enabler + the eval rig that
   *measures* every subsequent change) → Track C one canonical spelling + accessor + curated
   prelude (after the constructor fork is decided) → Track D example pack + measured-priors
   naming, accuracy-gated in CI.
4. **Validate each with the measured-priors probe** (prelude-only agent authoring rate) —
   the prototype proved this is a cheap, decisive instrument (P5).
