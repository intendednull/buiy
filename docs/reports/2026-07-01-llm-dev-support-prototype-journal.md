**Date:** 2026-07-01
**Status:** active

# First-Class LLM Development Support — Prototype Dev Journal

> **PROTOTYPE — exploratory, DO NOT MERGE THE CODE.** The deliverables are this
> journal + the retrospective (`2026-07-01-llm-dev-support-prototype-retrospective.md`,
> written at the gate). The prototype code is an unmerged reference.

Goal: learn what *actually* makes Buiy easy for an LLM to develop against — by
building minimal prototypes of the four tracks and **measuring** them with real
LLM agents authoring Buiy UIs (principle P5: the loop is the eval harness).
Worktree: `worktree-llm-dev-support`, off `origin/main` `31bbbc6`.
Grounding: `2026-07-01-llm-development-support-research.md` (the LLM-lens audit +
principles P1–P5) and the human-DX corpus (F1–F8, D1–D7, N-frictions).

## What we're testing (the high-uncertainty questions)

1. **Track A / the spine:** does a headless-run + **semantic-tree snapshot as a
   text observation** actually let a *sightless* agent DETECT and SELF-CORRECT a
   silent-wrong bug (magenta token, missing `Camera2d`, wrong widget state) it
   otherwise ships as "success"? (Validates P1 + the "give the LLM eyes" thesis.)
2. **Track B:** do typed tokens + load/compile-time failure measurably cut the
   hallucinate-a-plausible-key→magenta rate vs. the current stringly API?
3. **Track C:** does one canonical spelling + a real prelude (incl. MVU) + a
   domain accessor cut import/idiom-blend errors and "read the checkbox value"
   failures?
4. **Track D:** does an accurate in-context example pack move first-try
   correctness *more than* any single API change? (Research predicts presence
   dominates — test it.)
5. The **"plausible, needs A/B"** question: `bsn!` vs an imperative builder —
   which does an agent get right more often on first try?

## The experiment protocol (how we RUN the artifact)

- **Authoring tasks** (small, real Buiy UIs), each with machine-checkable success
  criteria expressed as assertions over the semantic tree / a compile result.
- **Variants:** `V0` = current Buiy API, no loop tool, current docs only.
  `V1` = prototype API (typed tokens / one spelling / prelude+MVU / accessor) +
  the example pack in context + the loop tool the agent may call.
- **Scoring (orchestrator runs it in the warm worktree, sequentially):** did it
  compile first try? did it render the intended tree (or silent-wrong)? for `V1`,
  did the agent self-correct using the snapshot within N iterations?
- **Non-negotiable:** RUN the artifact every wave. Agents PRODUCE code (text); the
  orchestrator compiles + runs + snapshots in the one warm worktree (no cold
  isolated per-agent worktrees — that's the conflict-hell anti-pattern).

## Wave plan (learning-optimized; minimal, not polished)

- **P0** — harness + baseline: the example crate agents' scenes slot into; the
  text `snapshot` dump; run V0 baseline and record the raw failure modes.
- **P1** — Track A: expose the existing `snapshot`/`perform` driver as an
  agent-facing text tool; test self-correction on a seeded silent-wrong bug.
- **P2** — Track B: typed color tokens (enum/newtype, compile/load-time fail) +
  missing-`Camera2d` loud panic; A/B the magenta-hallucination rate.
- **P3** — Track C: real `prelude` incl. MVU + one canonical widget spelling +
  a `Checkbox::checked()`-style accessor; A/B import/idiom + state-read failures.
- **P4** — Track D: `llms.txt`/`AGENTS.md` + a small *compiling* example cookbook;
  A/B with vs without the pack in context.
- **P5** — synthesis → retrospective (keep/refine/redesign per track, validated
  decisions, surfaced framework bugs, residual gaps, build strategy for the final).

## Running log

### 2026-07-01 — P0 (setup)
- Built: this journal + the committed research report (`71476bd`). Confirmed the
  prototype worktree is off `origin/main` `31bbbc6` (includes MVU-as-core).
- Grounding read: the in-process driver already exists (agent-interface Phase 0) —
  `a11y::snapshot(world) -> SemanticTree` + `perform(world, action, target, data)`,
  headless and **GPU-free** (reads the per-frame `A11yNodeView` list from ECS; no
  render world). `SemanticNode` carries role / accessible name / decomposed state /
  advertised actions / relations / children. So Track A's substrate exists; the
  prototype work is to make it an *agent-facing text tool* and prove self-correction.
- Surprised by: the "eyes" for an agent need **no GPU** — the semantic tree is a
  pure ECS projection. The expensive-looking part (headless render) is unnecessary
  for the feedback loop; layout + a11y + widget state are all observable without an
  adapter. This lowers the cost of the highest-leverage track dramatically.
- Next: warm the build cache (running), then P0 harness + V0 baseline.

### 2026-07-01 — P0/P1 (harness built + RUN; Track A mechanism validated)
- Built: `examples/llm_probe` — a headless, **no-render** Buiy app (MinimalPlugins +
  Input + Asset + Core/Theme/A11y/Focus/Layout/Text/Widgets, render OMITTED) that runs
  an agent-authored `scene::scene` and prints (1) the SEMANTIC TREE via the existing
  `a11y::inprocess::snapshot` (role / accessible name / toggled / expanded / value /
  disabled / focused / children) and (2) a LAYOUT+TEXT dump (size / pos / a11y_label /
  text content). Compiled in 57s (cold render-feature bevy), then cached.
- **Ran the artifact** on a known-good hand scene (card + `Button::new("Save")` +
  `Checkbox::new("Dark mode")` + a `Text` title). Output correct: the semantic tree
  shows `Checkbox name="Dark mode" toggled=Some(False)` and `Button name="Save"`; layout
  shows nonzero sizes and all text content ("Settings", "Save", "Dark mode").
- **Validated:** a sightless agent CAN get complete eyes on structure / state / naming /
  layout / text content **with zero GPU** — the "eyes" are a pure ECS projection. The
  expensive-looking headless-render path is unnecessary for the feedback loop.
- Ran-the-artifact findings (bugs/gaps only visible by running):
  1. **Plain `Text` is invisible to the semantic tree** (no a11y role) — an agent
     verifying title/label text via the tree alone would not see it. Fixed in the probe
     by also dumping `Text` content in the layout section; but it flags a real design
     question for the final: should decorative text carry a `text`/`label` a11y node so
     the loop can assert content? (Otherwise agents must read a second channel.)
  2. **The GPU-free loop cannot see the PIXEL class** (color/paint, magenta token,
     forgotten-`Camera2d` blank). So Track A (semantic tree) and Track B (prevent-at-
     compile) are **complementary, not redundant** — B is non-optional precisely because
     the cheap loop is blind to pixels. Sharpens the whole design.
- Surprised by / friction (as the harness AUTHOR, itself a data point): composing the
  headless stack meant hand-listing 8 sub-plugins from `buiy_core::{theme,a11y,focus,
  layout,text}` + `buiy_widgets` (not reachable via `buiy::prelude`); `BuiyHeadlessPlugin`
  exists but pulls `BuiyRenderPlugin` (wants a RenderApp), so it's not usable for a
  GPU-free run. A first-class `BuiyProbePlugin` / headless-no-render preset is a likely
  Track-A deliverable. Also hit the `bevy::prelude::*` vs `buiy::prelude::*` `Text`
  glob-collision (had to import `Text` explicitly) — an F-class ambiguity for LLMs.
- Next: E1 experiment — does the loop catch a silent-wrong the compiler can't (spawn a
  checkbox that must start CHECKED; F1 says there's no domain setter)? Attempt-1 = would-ship;
  looped-final = with eyes.

### 2026-07-01 — E1 (checkbox-must-start-checked; capable agent + source access)
- Setup: a fresh Opus agent, task = "card + title + Dark-mode checkbox that starts CHECKED
  + Save button", allowed to read the Buiy source, produce scene.rs, NOT run it (attempt-1 =
  what it would ship blind).
- Attempt-1: `(Checkbox::new("Dark mode"), A11yToggled(Toggled::True))` — reached into
  `buiy_core::a11y::{A11yToggled, Toggled}` (NOT in the prelude), used the foreign tri-state
  `Toggled` enum, and reasoned (correctly) that the explicit component overrides the widget's
  `#[require(A11yToggled)]` default. Confidence: HIGH.
- **Ran it → CORRECT.** Semantic tree: `Checkbox name="Dark mode" toggled=Some(True)`; the ✓
  glyph rendered (layout shows the 18×18 mark with text="✓"). No loop needed to fix it.
- Findings (these RE-SHAPE the design emphasis):
  1. **A capable agent + source access is a HIGH baseline for static correctness.** It got the
     F1-coupled path right first try. So "give the LLM eyes" (Track A) is NOT primarily justified
     by static authoring correctness — it is justified by the **runtime / silent-wrong class**
     (behavior + pixels) that source-reading cannot reveal, and by realistic constrained-context
     agents. The retrospective must frame Track A's value precisely, not as generic.
  2. **The friction that actually bit is F1/N1 discoverability, not correctness.** The "checked"
     concept is a foreign tri-state a11y enum reachable only past the prelude. An agent working
     from the prelude/priors (not diving into source) would look for `Checkbox::new(..).checked(true)`
     and not find it. This is the Track C (domain accessor + prelude) case — to be isolated in E3.
  3. Confirms the audit's F1 + N1 + the "reach into buiy_core::a11y" pattern, live and first-hand.
- Next: E3 — same task, but a PRELUDE-ONLY agent (no source dive) to expose the priors/
  discoverability gap; then a minimal Track-C prototype (`.checked(true)` + prelude) to show it closes.

### 2026-07-01 — E3 (prelude-only agent; the discoverability/priors gap) — KEYSTONE FINDING
- Setup: fresh Opus agent, SAME task, given ONLY the `buiy::prelude` surface (names incl.
  `A11yLabel`/`A11yRole` but NOT `A11yToggled`/`Toggled`), forbidden from reading crate source.
- What it wrote (all LOW-confidence, self-flagged): `Checkbox::new("Dark mode").checked(true)`,
  `Background(ColorToken::Surface)`, `Corners::all(Radius::px(8.0))`, `FontWeight::Bold`,
  `.with_children(|card| …)`, bare `Text` (assumed `#[require(Node)]`).
- **Ran it (compiled) → 6 errors**, every one a prior-mismatch: `Commands` not in prelude;
  `ColorToken::Surface` (no such variant — it's stringly `Token("…")`); `Background` is a struct
  not a tuple; `Radius::px` (it's `Radius::circular`); `FontWeight::Bold` (no such variant);
  `.checked(true)` (no such method on the `impl Bundle`).
- **KEYSTONE FINDING — the LLM's hallucinated API IS the target design.** With no source access,
  the model reached for exactly the ergonomic, prior-matching APIs the research prescribes:
  a builder accessor `.checked(true)` (Track C / F5 / F1 domain accessor), a TYPED semantic token
  `ColorToken::Surface` (Track B/D / F6), and closure children `.with_children(|c| …)` (F8). It did
  NOT reach for Buiy's actual forms (stringly `Token("…")`, foreign `A11yToggled(Toggled::True)`,
  `.id()`+`add_children`). **So "match popular-neighbor priors" is not abstract advice — the model
  wrote the spec for us.** If Buiy's surface matched these priors, the prelude-only agent would have
  ~succeeded.
- **P1c validated:** all 6 prior-mismatches surfaced as COMPILE errors, not silent-wrong — the type
  system is the agent's feedback channel exactly as the research claimed.
- **The sharp limit (sets up E3b):** two errors are semantically undiscoverable from the compiler —
  `ColorToken::Surface` (compiler can't suggest the magic string; a guessed string then COMPILES and
  ships MAGENTA — invisible to the headless probe: F6 converts a compile error into a silent-wrong on
  workaround) and `.checked(true)` (compiler can't reveal the `A11yToggled` path). Hypothesis: the
  compiler loop fixes the mechanical/typed errors, STALLS on the stringly + missing-accessor ones —
  which is precisely the boundary between Track A (loop) and Tracks B/C/D (prevent + examples).
- Next: E3b (sent) — feed the 6 errors back, prelude-only, and watch where it converges vs stalls.

### 2026-07-01 — E3b (compiler-loop convergence boundary) + stop decision
- Fed the 6 compiler errors back to the prelude-only agent (no source). Its strategy:
  - FIXED the compiler-grounded ones: `use bevy::prelude::Commands`; `Radius::circular(8.0)`
    (exact suggestion).
  - **DROPPED the ungroundable ones** rather than re-fail: deleted the card `Background`
    (compiler rejected `ColorToken::Surface` but couldn't name a real token) and the
    `FontWeight::Bold`. → **F6 causes SILENT FEATURE OMISSION under the loop:** the build
    goes green with the card background simply *gone*, and nothing flags it — a new
    silent-wrong flavor (omission, not magenta).
  - Still GUESSED `Checked(true)` for the hard requirement (real path is the non-prelude
    `A11yToggled(Toggled::True)`).
- **Ran it → 2 errors, both at the predicted stall boundary:**
  1. `(Node, Style, Corners)` is not a `Bundle` — the guessed `Corners::all(Radius::circular(8.0))`
     isn't a spawnable component (it's a value type); compiler gives no shape hint.
  2. `Checked` unresolved — the real symbol (`A11yToggled`) is not in the prelude, so a
     prelude-only agent **literally cannot name it** from compiler output. HARD STALL.
- **Finding (the Track-A-vs-B/C/D boundary, drawn empirically):** the compiler loop
  CONVERGES on mechanical / typed-with-a-hint errors, and STALLS on (a) symbols unreachable
  from the prelude (the checked state), (b) mis-assumed API shapes with no compiler hint, and
  (c) stringly tokens (→ omission or, on a string guess, magenta). The compiler is necessary
  but not sufficient; the residue needs Track B (typed tokens so the compiler CAN guide),
  Track C (prior-matching accessor in the prelude), or Track D (an example).
- **`.checked(true)` feasibility probe (a real FINAL fork):** `Checkbox::new` returns an
  opaque `impl Bundle`, so the prior-matching `.checked(true)` chaining fights the
  marker + `#[require]` + return-`impl Bundle` model. Providing it forces a FINAL decision:
  do widget constructors return a builder, a Bundle, or a Scene? **Typed tokens (Track B) do
  NOT fight this model** (just swap the `Cow<str>` key for an enum), so B is the cheapest,
  highest-value, lowest-risk fix to build.
- **STOP decision:** decisive, decision-quality learning reached across A/B/C/D. Per the
  `/loop` terminal condition ("ready for final version brainstorm"), stopping the prototype
  here. Building the track fixes is the FINAL's re-decided work (Phase B), and two of them
  (`.checked` accessor shape; typed-token migration) are design forks best decided WITH the
  owner in the brainstorm. Retrospective written next.

### 2026-07-01 — Builder→Bundle feasibility spike (post-brainstorm fork-1 decision)
- Owner chose the constructor fork = **Builder → Bundle** (`Checkbox::new("…").checked(true)`),
  the model's #1 prior but the option flagged as fighting the return-`impl Bundle` model.
  De-risked it immediately (prototype-first applied to the design decision).
- Built + RAN a minimal `#[derive(Bundle)] struct CheckboxB { node, role, label, toggled,
  focusable }` with `new(label) -> Self` and a chained `checked(bool) -> Self` that sets the
  real `A11yToggled` field. Spawned `commands.spawn(CheckboxB::new("Dark mode").checked(true))`.
- **Result → PROVEN.** Semantic tree: `role=Checkbox name="Dark mode" toggled=Some(True)`.
  The Builder→Bundle shape is feasible + clean + chainable + spawnable + correct initial state,
  because `.checked()` stores the actual component (not a bool) on a derived-Bundle field.
- **One open sub-detail (not a blocker):** the spike had height 0 — no children. Real widgets
  need their mark-glyph + label children, which a flat derived-Bundle can't easily carry
  (the `children!` type is hard to name as a struct field). Recommended FINAL approach: attach
  the visible children via a component **lifecycle hook / observer** on the marker (a standard
  Bevy pattern) so the author's builder-bundle stays flat and the children are framework-managed
  — which is *better* DX (author never hand-wires children) and matches the model's `.with_children`
  expectation being unnecessary. Flag for the final spec.

### 2026-07-01 — E3b addendum (late inline reply confirms non-convergence)
- e3author's inline E3b revision arrived after the on-disk synthesis above and CONFIRMS the
  boundary: given another compiler round on the ungrounded symbols it did NOT converge — it
  emitted fresh plausible-but-wrong guesses (`ColorToken::Card`, `FontWeight::BOLD`,
  `Checked(true)`) and explicitly flagged #2/#5/#6 as unresolvable from the compiler alone.
  So the compiler loop **oscillates among plausible names** for prelude-unreachable / stringly
  symbols rather than converging → hard dependence on Track B (typed, compiler-guidable),
  Track C (prior-matching accessor in the prelude), or Track D (an example). It also
  independently re-flagged the `bevy::prelude::*` vs `buiy::prelude::*` `Text`/`Node`
  glob-collision. No design change; strengthens the §3.2/§4 rationale in the spec.

### 2026-07-01 — Broadening probes (prelude-only, N=4 task coverage for the final)
Three more prelude-only authoring probes (events/F2, dynamic-list/F7, MVU-counter/N1) to
extend the keystone beyond the single stateful-card task, so the final brainstorm rests on
more than one datapoint.

**Pmvu (MVU counter, N1) — 10 compile errors. Confirms N1 + a NEW curated-prelude finding.**
- MVU is **undiscoverable from the prelude** → agent fell back to raw Bevy (a `Counter(i32)`
  component + an `increment` system reading `OnPress`). Reasonable fallback, but:
- **NEW, high-impact:** `buiy::prelude::*` alone **cannot express a Bevy system** — `Component`
  (derive), `Commands`, `Query`, `MessageReader`, `With`, `Camera2d` are ALL out of scope (the
  prelude re-exports none of Bevy's ECS essentials). So every prelude-only agent is forced to
  add `use bevy::prelude::*`, which then **glob-collides on `Text`/`Node`**. The curated prelude
  (Track C) must re-export the Bevy ECS essentials AND resolve the collision — this is a concrete,
  load-bearing requirement, not a nicety. Strengthens spec §4 Track C ("curated prelude incl. the
  bevy essentials the prototype found missing (`Commands`)") — widen it to the full system-authoring
  set (`Component`/`Query`/`MessageReader`/`With`/…).
- Also re-guessed the `Style` builder: `Style::new().column().gap(Length::px()).padding(Edges::all())`
  vs the real `Style::default().flex_column().gap_px(f32).padding(f32)` (the `padding` mismatch is a
  type error — real takes `f32`, not `Edges`). Same prior-mismatch class as E3.

**Plist (dynamic todo list, F7) — 2 errors, BOTH `ColorToken` variants. Cleanest result yet.**
- Approach: an imperative `.with_children(|p| for todo in &todos { p.spawn((Text..)) })` loop over
  the `Vec` — reasonable; the agent noted no `bsn_list!`/list-combinator was findable from the surface.
- **Everything compiled EXCEPT the two stringly tokens** (`ColorToken::Surface`, `ColorToken::Text`).
  `Node::default()`, the full `Style` builder (`.flex_column().padding(16.0).gap_px(8.0).width_px()` —
  guessed RIGHT this time), `Background { color }`, `TextColor(ColorToken)` shape, and the
  `.with_children` loop ALL worked.
- **Two big findings:** (1) **F6 stringly tokens are the single most consistent, isolated failure
  across EVERY task** (E3, E3b, plist) — always the *only* thing blocking otherwise-correct code, and
  agents always guess the exact semantic names (`Surface`/`Card`/`Text`) a closed enum would have.
  This is the strongest possible validation of Track B as the flagship: the closed enum turns each
  guess into correct code. (2) **Static data→UI (F7) is NOT actually blocking** — the imperative loop
  handled it cleanly; the real F7 pain is *reactive* lists (data mutates → respawn/reconcile), which
  this static case doesn't exercise and which the research already defers to the D6/D7 reactivity call.
- Cross-probe note: plist guessed the `Style` builder RIGHT while pmvu guessed it WRONG — prior-matching
  is **probabilistic** (a familiar-shaped fluent builder is often but not reliably guessed); the token
  stall is **deterministic** (nobody can guess a magic string). → prioritize closing the deterministic
  failures (tokens, non-prelude symbols) over the probabilistic ones.

**Pevents (checkbox→label reaction, F2/F1) — 2 errors: `ColorToken::Surface` + `checkbox.checked`.**
- The agent read state via `Query<&Checkbox, Changed<Checkbox>>` and assumed a public
  `checkbox.checked: bool` field — exactly the F1 domain-accessor expectation. Both are wrong:
  `Checkbox` is a unit marker (no `checked` field) and its state lives on `A11yToggled` (so even
  `Changed<Checkbox>` would never fire — a *semantic* silent-wrong that would compile if the field
  existed). Its documented fallback guessed a `Checked` component (again). Everything else compiled
  (nested `.with_children` rows, Style builder, StatusLabel marker, the `Changed<>` query shape).
- Blockers, again: (1) the stringly token (F6, deterministic-universal) and (2) the missing domain
  accessor (F1). The agent WANTS `checkbox.checked` + `ValueChange`-style reactivity → Track C.

**Synthesis across N=4 tasks (card / MVU / list / events):**
- **F6 stringly `ColorToken` is the ONE deterministic failure in every task.** Agents always guess a
  semantic variant (`Surface`/`Card`/`Text`). Track B (closed enum) turns every guess into correct
  code — highest-consistency payoff; flagship confirmed.
- **F1/F2:** the agent reaches for `checkbox.checked` + change-reaction; a domain accessor + typed
  value-events (Track C) is what it expects.
- **N1:** MVU is invisible from the prelude, AND the prelude can't even express a system (no
  `Component`/`Commands`/`Query`/`MessageReader`) → the curated prelude (Track C) must re-export MVU
  + the Bevy ECS authoring essentials and resolve the `Text`/`Node` glob-collision.
- **F5/constructor:** Builder→Bundle (proven) is what the agent wants (`.checked(true)`).
- **F7:** static data→UI is fine imperatively; reactive lists remain the deferred D6/D7 call.
- **Meta:** deterministic failures (tokens, non-prelude symbols, missing accessors) are the ones to
  close first; probabilistic ones (fluent-builder method spellings) matter less. The measured-priors
  probe (P5) is confirmed cheap + decisive across 4 tasks → keep it as the FINAL's acceptance rig.

### 2026-07-01 — R1: the FULL runtime feedback loop (author→run→click→observe) — PROVEN first-hand
- Built + RAN `examples/llm_probe/src/bin/runtime_loop.rs`: two counters driven by a REAL click
  through the in-process driver (`get_by_role(Button,"+1")` → `click`, which fires the same
  `OnPress` the pointer path does — GPU-free, no picking plugin), reading the counter text back
  before/after.
- **Result:** CORRECT wiring → count `0→1` (loop OBSERVES the interaction). BUGGY wiring (the "+1"
  button never tagged with the marker the increment system filters on — compiles green) → count
  stays `0` (loop CATCHES the runtime silent-wrong).
- **This converts Track A's central thesis from asserted to demonstrated first-hand:** a bug that
  compiles + runs + exits 0 is invisible to the compiler but VISIBLE to the author→run→drive→observe
  loop, entirely GPU-free. It is the definitive justification for Track A (the "eyes"), complementing
  E1's point that source-reading already covers *static* correctness. The `click`→`OnPress` route works
  headless with no picking plugin (the action router lives in Core/A11y), so the whole interactive loop
  costs zero GPU and no window.

### 2026-07-01 — R3: Builder→Bundle children via on-add observer — PROVEN
- Built + RAN `src/bin/children_hook.rs`: a flat `#[derive(Bundle)] MyCheckbox { node, role, mark,
  label, toggled, focusable }` + `.new(label).checked(bool)`, plus a framework-side
  `On<lifecycle::Add, CbMark>` observer (mirroring Buiy's existing `On<Insert, Anchor>` pattern) that
  does `commands.entity(e).with_child((Text("✓"), FontSize))`.
- Author wrote ONE line: `commands.spawn(MyCheckbox::new("Dark mode").checked(true))`.
- **Result → PROVEN:** the ✓ mark child was auto-attached by the observer (`mark_present=true`) and the
  checkbox is in the semantic tree `toggled=Some(True)`. (The mark is a decorative `Text` with no a11y
  role, so it is correctly NOT an a11y child — `children=0` in the tree is right.)
- **Resolves the §3.1 open sub-detail:** Builder→Bundle + on-add observer for visible children works
  cleanly — the author never hand-wires children (kills the `.with_children` ceremony the prelude-only
  agent expected), builder-bundle stays flat + chainable. The FINAL should attach widget sub-parts
  (checkbox mark, button label, …) via on-add observers, not explicit `children!` in a constructor.

### 2026-07-01 — R2: typed tokens + compiler-enforced theme contract — PROVEN (incl. first-hand E0004)
- Built + RAN `src/bin/typed_tokens.rs`: a closed `enum ColorToken { Surface, Text, Accent }` + a
  `trait ThemeContract { fn resolve(&self, ColorToken) -> Color }` whose impl `match`es the enum
  **exhaustively**. `LightTheme.resolve(ColorToken::Surface)` → a real `Srgba(0.98,…)` (never magenta).
- **Both F6 failure modes are compile errors, the completeness one demonstrated first-hand:** dropping
  the `Accent` arm produced `error[E0004]: non-exhaustive patterns: ColorToken::Accent not covered`
  (the vanilla-extract "theme contract" completeness, for free, via enum exhaustiveness); a typo can't
  name a variant (E0599). Contrast: today's `ColorToken::Token(Cow<str>)`→HashMap typechecks any string
  and ships magenta on a miss (invisible to the GPU-free loop).
- **Confirms Track B is feasible AND turns the prelude-only agent's universal `ColorToken::Surface`
  guess into correct code.** The exhaustive-match-as-contract is cleaner than N per-token methods and
  needs no new machinery.

### 2026-07-01 — Prototype-first exploration EXHAUSTED (stop point)
All high-uncertainty questions are now answered by building + RUNNING (not asserting):
- Track A eyes work GPU-free (P0/P1); the **full runtime loop catches compile-green silent-wrong** (R1).
- The measured-priors probe generalizes across **N=4** tasks; F6 is the universal deterministic failure.
- Both forks resolved AND their risky feasibility points proven: **Builder→Bundle** (spike) +
  **children via on-add observer** (R3); **typed tokens + compiler-enforced contract** (R2).
There is no remaining *prototype* work — the next forward motion is either the collaborative FINAL
brainstorm (the loop's stated terminal condition) or Phase-B implementation (human-review-gated per the
spec). Stopping the prototype loop here.
