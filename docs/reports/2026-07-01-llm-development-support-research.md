**Date:** 2026-07-01
**Status:** active

# Buiy — First-Class LLM Development Support: Research & LLM-Lens Audit

Research pass on what "first-class support for LLM development" means for a code
library like Buiy, and a re-framing of Buiy's already-documented developer-experience
frictions through an **LLM-as-author** lens.

> **Companion** to the human-DX corpus produced 2026-06-25/30 (currently uncommitted
> in the `dx-composition-research` worktree — they should be committed alongside this):
> the [Developer-Experience audit](2026-06-25-developer-experience-audit.md) (frictions
> **F1–F8**), the [UI-DX composition prior-art](2026-06-25-ui-dx-composition-prior-art.md)
> (design directions **D1–D7**), and the [post-MVU current-state audit](2026-06-30-current-state-audit-post-mvu.md)
> (the **N-frictions** + re-scorecard). That corpus is framed for **human** developers.
> This report adds the **LLM** lens: it does not replace those findings, it **re-weights**
> them and adds facets the human corpus never considered.

**Method.** A 23-agent research workflow: one grounding agent (read the three DX reports
+ the relevant prior-art folders), seven parallel research facets (codegen success
factors, compiler-as-feedback-loop, machine-readable diagnostics, docs-for-LLMs, a
critical audit of the "verbosity is fine" premise, the agent feedback loop / "eyes", and
LLM-favorable API patterns), an **adversarial verification pass** that fact-checked every
load-bearing / quantitative / surprising claim against primary sources, and a synthesis.
The verification pass materially downgraded several attractive numbers (see the Evidence
Ledger, §6) — those corrections are baked into the claims below.

**Framing.** "First-class support for LLM development" is a **both/and**, not a
displacement of humans. Most LLM-ergonomics wins are strictly better for humans too
(§1a). Where the LLM-optimal choice genuinely diverges from the human-optimal one (§1b),
the resolution is a *dev/test-vs-release gate* or a *canonical-path-plus-escape-hatch* —
not sacrificing human DX.

---

## 1. What "LLM ergonomics" means for a code library

The single governing fact, from which everything else follows:

> **An LLM authoring a GUI has no eyes on the running window. Its tightest — and often
> only — pre-human feedback loop is the compiler.** Anything that compiles, passes
> headless tests, and exits 0 while rendering a blank or magenta screen is reported by
> the agent as *success*.

Principles are sorted by their relationship to good *human* DX. The divergences (§1b)
and the LLM-specific levers (§1c) are the intellectual core of the report.

### 1a. Agrees with good human DX (do both at once)

- **Make the ergonomic path and the correct path identical.** Both humans and LLMs use
  whatever route the docs/examples steer toward; if the nicest API is off the steered
  path (Buiy **F4**: fluent `Style` is a `Bundle`, unauthorable in `bsn!`), every
  generation inherits the footgun. Pure win-win.
- **Encode intent in types and signatures.** A precise signature is the model's *only*
  spec when it has no examples, and it is what a human reads first. Precise types
  collapse the space of hallucinated payloads for both (Buiy **F2**).
- **Make illegal states unrepresentable / poka-yoke.** A constructor taking all required
  fields, or a builder that can't drop defaults, forces both audiences onto the correct
  construction. The `#[require]`-suppression trap (**F5**) is the anti-pattern.
- **Typed, per-widget event/value vocabulary** over one polymorphic sink; **uniform,
  composable structure** so one in-context example generalizes (**F7**).
- **Name the fix in the diagnostic, not just the fault**, and use **stable, documented
  diagnostic codes** (rustc `E0277` + `--explain` style) — humans search them, agents/harnesses
  additionally use them as retrieval keys.

### 1b. Diverges — LLM-optimal ≠ human-optimal

- **Stringly-typed magic keys are the sharpest divergence in the set.** A human survives
  `ColorToken::Token("...")` (**F6**) via editor autocomplete + *seeing the magenta on
  first run*. The agent has **neither** guardrail, so an open string space is a direct
  invitation to confidently invent a plausible key that typechecks and ships wrong —
  already live in the gallery (`"color.shadow.card"` is registered in no theme). Mild
  human nuisance, reliable agent-failure generator. Close the space with enums/newtypes.
- **Fail-loud over graceful degradation.** Humans usually *prefer* "don't crash my whole
  app over one bad token" — they'll see and fix the glitch anyway. The sightless agent
  strictly *needs* the crash; a clean exit on broken output reads as success. **Resolution
  for a both/and library: hard-error in dev/test, allow graceful degradation in release.**
- **One obvious way — a more monomorphic API than humans want.** Multiple idioms (**F5**'s
  four spellings) are a human virtue (pick what fits) but split the model's probability
  mass — it blends idioms into incoherent hybrids or lands on the variant carrying the
  suppression footgun. Resolution: one canonical steered path + a reachable core for power
  users, not four co-equal spellings.
- **A single canonical import surface (`prelude::*`) matters far more for LLMs.** Humans
  lean on IDE auto-import, so Buiy's path fragmentation (**F5**) and the empty MVU prelude
  (**N1** — `buiy::prelude` re-exports zero `mvu` items) cost them little; the model must
  emit exact paths from memory and hallucinates modules. Among the cheapest, highest-yield
  fixes and near-invisible to humans.
- **Verbosity is not one axis — it splits, and the split favors the model differently than
  the human.** *Type-structure* verbosity (`Sizing::Length(Length::Px(340.0))`, long
  qualified enum paths, named args) is fine-to-good for the model — each layer is
  machine-checked and self-describing. *Ceremony* verbosity (manual `.id()` +
  `add_children`, dual-writes) is bad for both, and **worse** for the agent, because each
  uncoupled duplicated site is a silent desync it cannot see.
- **Structured (JSON) diagnostics over the human-pretty ANSI render**, and **less
  suggestion text, not more** — Rust shipped `#[diagnostic::do_not_recommend]` to *remove*
  misleading impls from an error. The agent acts on the most concrete instruction; a bad
  recommendation is a trap a human skims past for free. (Note: the strong "excess text
  *measurably degrades* repair" claim was flagged **overstated** in verification — it is a
  token-budget heuristic, not a controlled result.)
- **"Loud warn-once" must be re-tuned for machines.** Once-per-process dedup exists to
  spare humans log fatigue; an agent may only ever see one run's output, so a warning
  deduped away is simply gone. The `LogicalId` silent-`UNRESOLVED` collision must re-emit
  per failing operation on the captured channel, not dedup itself into invisibility.

### 1c. LLM-specific (absent from the human corpus entirely)

- **Training-data presence dominates every local design choice.** API frequency in
  training corpora is the single strongest predictor of LLM correctness. *(CONFIRMED —
  Jain et al., arXiv 2407.09726: GPT-4o valid-invocation rate drops **93.66% → 38.58%**
  from high- to low-frequency APIs despite ~90% HumanEval. UI-framework code is **<1%** of
  code pretraining corpora; **12–25%** of generated declarative-UI code fails to compile.)*
  Buiy's `bsn!` is brand-new Bevy 0.19 with near-zero corpus, so the model will hallucinate
  Bevy-0.15-era or web-CSS patterns **before any API decision is made**. The highest-leverage
  authoring moves are therefore to **borrow correctness from popular neighbors** (match
  names/shapes) and to **synthetically inject presence** via an accurate, curated in-context
  example corpus.
- **Compiler-as-feedback-loop primacy.** Every defect pushed from runtime to compile time
  becomes autonomously fixable; every one that stays at runtime is invisible to a sightless
  agent. *(CONFIRMED — arXiv 2504.09246, PLDI 2025: ~**94%** of compiler-catchable LLM
  errors are type errors, not syntax; type-constrained decoding roughly halves compile
  errors. TypeScript scope.)* A Bundle-not-Component (**F4**) or stringly (**F6**) surface
  that dodges the type-checker removes the agent's best safety net.
- **Silent-wrong is the single catastrophic failure mode and the governing constraint.**
  Buiy's **F3/F6/N3** pass every signal the sightless agent has (compiler green, headless
  tests green — they never open the render world, process exits 0). The human audit already
  crowned "silent-wrong is the default failure mode"; the LLM lens elevates it from headline
  annoyance to **top priority**. Making an error *exist* (compile/load-time) is worth more
  than making existing errors prettier.
- **Route the signal to the channel the loop actually reads.** An IDE-hover lint, a
  rust-analyzer inlay, or a GUI cue is invisible to a headless `cargo build`/`test` loop.
  Detection must live in the compile/startup/panic/stdout path.
- **Give the LLM eyes: a headless render + a queryable/assertable semantic tree
  (AccessKit) as a first-class feedback tool.** This is the direct remedy for the missing
  runtime channel — the reason silent-wrong is fatal. *(Token economics CONFIRMED: a
  screenshot is >1000 vision tokens; a serialized semantic tree is ~2–5 KB; Anthropic's
  tool-design guidance confirms returning structured names over opaque IDs measurably
  improves agent precision.)* **Higher lever than any single authoring-API tweak.**

## 2. The user's premise, examined

Premise: *"an LLM doesn't mind being verbose; it values clarity and logic-tracing; it can
hold lots of context."* **Half-right, and where right it points at the wrong axis.**

1. **"Doesn't mind verbosity" — SPLIT, not true as stated.** Type-structure verbosity is
   fine-to-good (machine-checked anchors); ceremony verbosity actively hurts (each
   uncoupled site is a silent desync). The **F8** fix is *cut ceremony, keep explicitness* —
   the opposite of a blanket "verbosity is fine."
2. **"Values clarity + logic-tracing" — true only when "clarity" = machine-checked type
   structure + self-describing names, not prose.** Past the token that names the fix, more
   explanation costs context and can mislead.
3. **"Holds lots of context" — true, but context is not free; signal density matters.**
   (Honesty flag: the strong "lost-in-the-middle drops >30%" / "code length correlates with
   errors" numbers were adversarially **downgraded to overstated** — the qualitative
   direction holds, the hard numbers do not.)
4. **The premise omits the two load-bearing levers:** training-data presence and
   silent-wrong. Clarity helps at the margin; **matching popular priors** and **converting
   silent-wrong to loud compile/load-time failure** are the moves that actually move the needle.

## 3. The audit, re-ranked by LLM impact

Building on the existing **F1–F8 + N** frictions (not redone). The right-hand column is
the change vs. the human ranking.

| Friction | Human sev | **LLM sev** | Why it moves under the LLM lens |
|---|---|---|---|
| **F3** silent-wrong footguns | high | **critical (#1)** | Zero detection signal for a sightless agent: green compile, green headless tests, exit 0 → reports success on a blank/magenta UI. The human's one-glance catch is exactly the channel the agent lacks. |
| **F6** stringly theme tokens | medium | **critical** | Sharpest divergence: open `Cow<str>` key space invites confident hallucination that typechecks + ships magenta; both human guardrails (autocomplete, seeing it) absent. |
| **F2** untyped `OnPress(Entity)` | high | **critical** | No type to follow to answer "read the checkbox value" → fabricates an accessor or reads the wrong shared bus; silent-wrong again. Types are the model's entire discovery mechanism with no examples. |
| **F1** state = a11y tree | high | high | No `Checkbox::checked()` → must read a foreign `accesskit::Toggled` enum never seen in training data; fabricates the accessor it expects. Leaf tier still blesses the coupling post-MVU. |
| **F5** four spellings | medium (a feature) | high | Splits probability mass → idiom-blending or the suppression-trap variant; import-path-dependent names add an import-hallucination class. A human virtue is an LLM liability. |
| **F4** `Style` is a `Bundle` | med-high | high | The *steered path is the footgun*: the model reproduces the demonstrated 5-component hand-spell and never explores to the nice API. |
| **N1** no MVU prelude / path frag. | low-med | high (**cheapest fix**) | One `prelude::*` line vs. recalling obscure module paths from memory. Near-invisible to humans, highest yield per unit effort. |
| **LogicalId** silent-`UNRESOLVED` | medium | high | New F3-class trap introduced by MVU: collision degrades silently on the very replay capstone it exists for. |
| **F7** retained-mode boilerplate | high | high (desync half) | Splits: ~17 marker structs = per-widget memorized ceremony a novel framework has no examples for; dual-writes/re-walks = silent-desync sites; discarded scene seeds = another silent-wrong. |
| **N4** two message substrates | medium | med-high | F5-style probability-mass splitting at the state layer; the direct source of the `EventReader`-vs-`MessageReader` silent-wrong. |
| **N2** no world-level `enqueue` | medium | medium | Ceremony/plumbing tax with no typed anchor; bad but recoverable at compile time (unlike the silent tiers above). |
| **F8** verbosity | **high (annoying)** | **LOW / split** | The biggest re-rank **down**. Cut the plumbing half; the descriptive verbosity humans hate is often *helping* the model. |

## 4. Design directions (options for the brainstorm, ranked by leverage)

Organized as four **tracks** of a program, not one spec.

**Track A — The loop / "eyes" (the spine).** Package headless-run + semantic-tree
snapshot + assert/perform as a first-class agent tool (CLI / `buiy_mcp` / test helper),
finishing the deferred pieces of the already-chartered agent-interface campaign. Converts
the invisible-runtime class into a detectable one, **and** serves as the measurement rig
for every other track. *Substrate mostly exists (`BuiyHeadlessPlugin`, the bidirectional
AccessKit tree, the in-process driver, the parity live-interaction tier). Highest lever.*

**Track B — Convert silent-wrong → loud compile/load-time errors.** Typed tokens with a
load-time hard-error + CI contrast lint (D4); a named panic for missing `Camera2d`;
auto-derived, collision-checked `LogicalId`; unify the two message substrates. *Every item
moves a defect from invisible-runtime to visible. Highest per-item leverage.*

**Track C — One coherent authoring surface.** One canonical spelling per widget (scene-fn,
D5) + a real unified `prelude` including MVU; typed two-event vocabulary
(`ValueChange<T>{is_final}`, D2) + domain accessors (`Checkbox::checked()`);
`Style`-as-`bsn!`-authorable components (D3). *Concentrates probability mass, kills import
hallucination.*

**Track D — Buy training-data presence.** A maintained, accuracy-gated context pack
(`llms.txt` / `AGENTS.md` + a *compiling* example cookbook — one canonical example per
task); match popular-framework priors in naming (`text("…")`, modifiers) as sugar over the
typed core; diagnostic engineering (`#[diagnostic::on_unimplemented]`, `do_not_recommend`,
`--message-format=json` for the harness). *The only lever against Buiy's structural
zero-corpus problem. `AGENTS.md` CONFIRMED as a real standard: 60k+ projects, Linux
Foundation. **Caveat:** doc drift actively poisons generation — needs an owner + a CI gate
that compiles/runs the examples.*

**Recommended order:** A first (enabler + measurement), then B (self-contained, verifiable
via the loop), with C and D in parallel behind them.

**One honest tension:** whether the declarative `bsn!` DSL or an imperative builder
generates more reliably from an LLM came back **plausible, not proven** (verification
verdict) — it hinges on how in-distribution `bsn!` is, and `bsn!` is brand-new. This
deserves an empirical A/B *through Track A's loop*, not an assumption.

## 5. Open questions (for the design)

- **Where is the boundary** between the pure authoring API and the surrounding harness
  (`buiy_mcp` / driver / semantic-tree snapshot)? The biggest lever sits partly outside the
  library. *(User steer: full development autonomy is the target, so the harness is in
  scope.)*
- **Which prior wins when they conflict — Bevy or SwiftUI/React?** The model holds a strong
  Bevy/ECS prior *and* a strong declarative-UI prior. How far to chase conventional-neighbor
  naming vs. staying Bevy-idiomatic (keeping sugar over the typed core so it never becomes a
  competing spelling)?
- **How aggressively to break the existing API** (collapse F5's four spellings)? Pre-1.0
  with only reserved crates.io placeholders makes now the cheap moment.
- **Fail-loud gating:** hard-error in dev/test + graceful degradation in release — does that
  satisfy both audiences?
- **Which is the canonical state path** — the MVU `Model`+`Message` surface or the widget
  scene-fns — and can we pick **one** message substrate to kill the two-substrate split?
- **Who owns the context pack** and does CI compile/run its examples? (Drift is poison.)

## 6. Evidence ledger (verification pass)

Adversarial fact-checks of the load-bearing / quantitative / surprising claims. Corrections
are already reflected in §§1–4.

**Confirmed**
- Training-data frequency ↔ LLM correctness; GPT-4o valid-invocation 93.66%→38.58%
  high→low frequency; UI-framework code <1% of corpora; 12–25% of generated declarative-UI
  code fails to compile. *(Jain, arXiv 2407.09726.)*
- ~94% of compiler-catchable LLM errors are type (not syntax) errors; type-constrained
  decoding roughly halves them. *(arXiv 2504.09246, PLDI 2025; TypeScript scope.)*
- `AGENTS.md` is a real standard: 60,000+ projects, stewarded by the Agentic AI Foundation
  (Linux Foundation, Dec 2025).
- Anthropic tool-design guidance: `ResponseFormat` concise 72 / detailed 206 tokens;
  returning names over opaque IDs improves agent precision. Screenshot >1000 vision tokens;
  Claude Code's 25,000-token tool-response cap. MCP code-execution 150,000→2,000 token
  (98.7%) reduction. *(All verbatim from Anthropic engineering posts.)*

**Overstated (direction holds, numbers do not)**
- "Lost-in-the-middle drops >30%; 200K models degrade by ~50K; 1M by ~300–400K" — qualitative
  phenomena supported, specific figures pushed beyond sources.
- "Generated-code length/complexity correlates with error rate; ~19.6% package-hallucination"
  — the 19.6% is a specific package-hallucination study, not a general length↔error law.
- "~10 in-context demos raised BLEU 19.3%→34.5% for code generation" — that study was code
  *comment/summary* generation, not code gen.
- "Excess diagnostic text *measurably* degrades repair" — a token-budget/readability
  heuristic, not a controlled measurement.
- "SWE-bench SOTA via tool-description refinement → dramatic improvements" — the SOTA claim
  is verbatim Anthropic, the "dramatic" magnitude is not established.

**Refuted (magnitude wrong)**
- "RAG docs reduce API hallucination 60–80% alone / 96% hybrid" — the actual primary source
  (USENIX 2025, arXiv 2406.10279) reports RAG *alone* ≈ 24–49% relative reduction.

**Plausible (needs an A/B, do not assume)**
- "A well-formed declarative UI DSL beats an imperative builder for first-try LLM
  compile-and-correct" — depends on how in-distribution the DSL is; `bsn!` is brand-new.
