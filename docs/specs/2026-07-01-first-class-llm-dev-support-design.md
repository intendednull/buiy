**Date:** 2026-07-01
**Status:** draft

# First-Class LLM Development Support — Design (North Star)

The unifying target state for making Buiy a library an LLM can **develop against
autonomously** — author, compile, run headless, observe via a machine-readable
semantic tree, and iterate to correctness with no human in the loop — *without
sacrificing human DX*. Frames all four tracks + the feedback loop as one target;
the per-track plans decompose from here.

**Evidence base (read these first):**
- [LLM-dev-support research & LLM-lens audit](../reports/2026-07-01-llm-development-support-research.md) — principles P1–P5, the friction re-ranking, the evidence ledger.
- [Prototype journal](../reports/2026-07-01-llm-dev-support-prototype-journal.md) + [retrospective](../reports/2026-07-01-llm-dev-support-prototype-retrospective.md) — what was built + RUN, and the keep/refine/redesign verdicts.
- Human-DX corpus (frictions **F1–F8**, directions **D1–D7**, the **N-frictions**): the 2026-06-25/30 reports (companion; being committed alongside).

**Relationship to existing work.** This *builds on* the agent-interface campaign
(the in-process `a11y::inprocess` snapshot/perform driver shipped in Phase 0 is
Track A's substrate — do not reinvent it), *coordinates with* the MVU state
interface (the N-frictions), and *consumes* the human-DX audit (F1–F8) — it
re-weights those findings through the LLM-as-author lens rather than replacing
them.

## 1. Positioning

> **Buiy supports LLM development as a first-class workflow: an agent can author a
> UI, compile it, run it headless, observe the result through a machine-readable
> semantic tree, and iterate to correctness with no human in the loop — using the
> same library and the same idioms a human uses.** Human DX is not traded away;
> LLM support is additive, and where the two genuinely diverge it is resolved by
> gating (dev/test vs release) or layering (one canonical path + a reachable core),
> never by degrading the human path.

**Non-goals / hard constraints (unchanged):** stay ECS-native + retained-mode (no
foreign reactive runtime as substrate); AccessKit-first stays as an *output*
projection; `bsn!` is Bevy's macro (build on/around, don't fork the grammar);
humans remain first-class.

## 2. Principles (validated by the prototype)

- **P1 — Two feedback channels; every defect must surface on one.** The compiler
  and the semantic tree are the agent's only senses. Prevent at compile/load-time
  what's cheap to prevent; make the rest observable via headless-run + semantic
  tree. *Nothing load-bearing may be observable only as pixels.* (Prototype: all
  prior-mismatches were compile errors; the semantic tree gave full
  structure/state/layout/text eyes GPU-free.)
- **P2 — First-class for both; resolve divergences by gate or layering.** Fail-loud
  in dev/test, graceful-degrade in release; one canonical steered path + a reachable
  typed core; cut ceremony, keep explicitness.
- **P3 — Borrow correctness from the corpus.** Buiy has near-zero training presence;
  match popular-neighbor priors and ship an accuracy-gated example corpus.
- **P4 — Machine-readable by default on the agent's channel.** Structured (JSON)
  diagnostics for the harness; stable diagnostic codes; per-operation (not
  process-deduped) warnings; semantic snapshots that return names, not opaque IDs.
- **P5 — The loop is the eval harness.** Every ergonomics change is *measured* by
  running a (prelude-only) agent through authoring tasks against the loop, not
  asserted. (Prototype proved this is a cheap, decisive instrument — see §6.)

## 3. Resolved design decisions (the two forks the prototype surfaced)

### 3.1 Constructor shape — **Builder → Bundle** (proven feasible)

One canonical construction form per widget: `Widget::new(label)` returns a concrete
**builder that IS a `Bundle`** (`#[derive(Bundle)]` with component-typed fields),
whose chained setters store the actual components — e.g.
`Checkbox::new("Dark mode").checked(true)`, spawned directly with
`commands.spawn(...)`. This matches the strongest observed LLM prior (the
prelude-only agent wrote exactly this), gives one obvious way (collapsing F5's four
spellings), and keeps accessors chainable.

- **Proven** by the prototype spike: a derived-Bundle builder with `.checked(bool)`
  setting the real `A11yToggled` field spawns and reads `toggled=Some(True)`.
- **Children attachment (the one open sub-detail):** visible sub-parts (a checkbox's
  mark glyph + label text) are attached by a **component lifecycle hook / observer**
  on the marker, not hand-wired into the author's bundle. This keeps the builder-
  bundle flat *and* is better DX (the author never wires children; the model's
  `.with_children` expectation becomes unnecessary). The exact hook mechanism is a
  plan-level detail.
- **Marker + `#[require]` become internal contract**, not a public spelling; the
  bare-marker / raw `bsn!{Widget Field{}}` patch (the §4.1c suppression trap) and the
  `_scene` dual-names are deprecated. `bsn!` authors the builder-bundle's components.

### 3.2 Theme tokens — **closed enum + theme contract** (research D4)

Tokens are a **closed enum** (`ColorToken::Surface`, …; matches the observed prior)
covering **color + spacing + radius + typography + motion** (not color-only — the F6
half-wiring). A **`Theme` contract trait the compiler forces every theme to implement
fully** — no missing tokens, no typos, by construction (vanilla-extract
`createThemeContract` lineage).

- Kills BOTH F6 failure modes the prototype caught: hallucinate-a-plausible-key →
  magenta (invisible to the GPU-free loop), and the compiler-loop *silently dropping*
  the styled feature to reach green (E3b deleted the card background).
- Turns the prelude-only agent's `ColorToken::Surface` guess into *correct* code.
- **Migration:** the stringly `ColorToken::Token(Cow<str>)` path is retired; a
  deliberate, mechanical migration of existing token strings to variants. An escape
  hatch for genuinely dynamic/custom colors stays (an explicit `Color` value), but it
  is not the token path and not the steered default.

## 4. The four tracks (one target state)

The **closed autonomous loop is the spine**: `author → cargo build → run headless →
inspect semantic tree → assert → iterate`. The tracks are its two halves — *prevent*
(compile/load-time) and *detect* (the loop) — plus the corpus that reduces how often
the loop is needed.

### Track A — The loop / "eyes" (the spine + the eval rig)

- A first-class, agent-facing way to run a Buiy app **headless (GPU-free)** and read
  its **semantic tree + layout + text** as a text/JSON observation the agent's build/
  test command returns. Built on the existing `a11y::inprocess::{snapshot, perform,
  get_by_role, click, …}` (agent-interface Phase 0).
- Ship a **`BuiyProbePlugin` / headless-no-render preset** (the prototype showed no
  GPU-free preset exists today — `BuiyHeadlessPlugin` pulls the render app; a probe
  run means hand-listing 8 sub-plugins, none in the prelude).
- Exposure surfaces: a `buiy` CLI/dev-dep snapshot, the `buiy_mcp` transport (the
  chartered-but-deferred Phase 2), and a test-authoring helper — one driver, N
  consumers (human inspector ≡ LLM agent ≡ test).
- **Done =** an agent can, in one command, run an authored scene headless and get a
  diffable text tree with role/name/state/layout/text; the pixel class is explicitly
  out of scope for this channel (see §5).

### Track B — Kill silent-wrong (prevent) — **build first**

- The §3.2 typed tokens + theme contract (flagship — cheapest, fights no existing
  model, and it's the fix the prototype most sharply motivated: across the N=4 probes
  the stringly `ColorToken` was the **one deterministic failure in every task**, and
  agents always guessed the exact semantic variant a closed enum would provide).
- A **named loud failure for the render-blank class** the loop can't see: missing
  `Camera2d` → a startup panic/error that names the fix; auto-derived + collision-
  checked `LogicalId` (kill the silent-`UNRESOLVED` MVU footgun).
- Route these to the compile/startup/stdout channel (P4), gated dev/test-hard vs
  release-degrade where a divergence exists (P2).

### Track C — One coherent surface

- The §3.1 builder→Bundle canonical constructor for every widget; **domain accessors**
  (`Checkbox::checked() -> bool`, `Switch::on()`) so reading state doesn't go through a
  foreign `accesskit` enum (F1); typed per-widget events (`ValueChange<T>{is_final}`,
  D2) over the one untyped `OnPress` sink (F2).
- A **real curated `prelude`** that brings the canonical surface — including MVU
  (`Model`/`Cmd`/`enqueue`/`LogicalId`) **and the Bevy ECS authoring essentials the
  prototype found missing** (`Component` derive, `Commands`, `Query`, `MessageReader`,
  `With`, `Camera2d`, …) — so a prelude-only agent can author *and wire systems* without
  recalling module paths (N1), and **resolve the `Text`/`Node` glob-collision** with
  `bevy::prelude`. (N=4 probes: `buiy::prelude::*` alone can't even express a Bevy
  system today, forcing the colliding `bevy::prelude::*` glob — this is load-bearing.)
- Style authorable where the steered path leads (the F4 fix — decomposed style
  components reachable in `bsn!`), so the nice API and the canonical path coincide.

### Track D — Buy corpus presence

- An accuracy-gated **`AGENTS.md`/`llms.txt` + a *compiling* example cookbook** (one
  canonical example per task), CI-gated so examples always compile/run (drift poisons
  generation).
- **Measured-priors naming:** where a concept has a well-known neighbor, adopt the
  neighbor's spelling (`text("…")`, modifier-style setters) as sugar over the typed
  core — *derived from what constrained agents actually generate*, not guessed.
- Diagnostic engineering: `#[diagnostic::on_unimplemented]` on the Widget/Bundle
  contracts, `do_not_recommend` on misleading impls, stable Buiy diagnostic codes,
  and the harness fed `--message-format=json`.

## 5. The prevent/detect split (why A and B are complementary)

The GPU-free loop (Track A) sees **structure / state / layout / text** but **not
pixels** (color, paint, blank-from-missing-camera). Therefore the pixel class is
handled by **prevention** (Track B: typed tokens make a bad color a compile error;
missing-camera is a loud panic) — not by the cheap loop. An optional GPU pixel-
readback lane (reusing the existing `#[ignore]` GPU test infra) is a *possible*
addition for color goldens, but it is not required for the autonomous loop and is
explicitly deferred.

## 6. Acceptance instrument (P5, validated)

The prototype proved a cheap, decisive metric: **prelude-only agent authoring rate** —
prompt a fresh agent with only the prelude surface (no source dive) to author a task,
compile + run it through Track A, and score first-try compile-and-correct + self-
correction. Every track's change is validated against this probe (e.g. Track C
"done" = the prelude-only agent authors the checked-checkbox card first-try, which it
could NOT do in the prototype). Task coverage must span: a stateful card (done),
events (F2), a dynamic list (F7), and MVU authoring (N1).

## 7. Build order & delivery

Per the retrospective, re-decided with the full picture:

1. **Track B typed tokens + theme contract** (flagship; no model conflict).
2. **Track A `BuiyProbePlugin` + agent-facing snapshot** (enabler + the rig that
   *measures* everything after it).
3. **Track C** builder→Bundle canonical constructor + accessors + typed events +
   curated prelude (the largest refactor; children-via-hook).
4. **Track D** example cookbook + measured-priors naming + diagnostics, CI-gated.

**Delivery:** each track is its own `staged-development` pass (plan → execute →
review gate), validated by the §6 probe. This is the FINAL (Phase B of
prototype-first): commit as you go, **merge-gated on human review** — no self-merge.
The prototype code stays unmerged; this spec + the reports are the carried learning.

## 8. Residual gaps / open for the plans

Prototype spikes (R1/R2/R3, see the retrospective) closed the feasibility unknowns; what remains is
plan-level detail, not open risk:

- ~~Children-attachment hook mechanism (§3.1)~~ — **RESOLVED:** an `On<Add, Marker>` observer attaches
  visible sub-parts (proven R3); flat builder-bundle, author wires none. Plans just apply it per widget.
- ~~Typed-token feasibility (§3.2)~~ — **RESOLVED:** closed enum + exhaustive-`match` contract (proven
  R2, incl. the E0004 completeness). Still plan-level: migration mechanics off the stringly path + the
  dynamic-color escape hatch shape.
- Typed-event vocabulary detail (F2/D2) and whether it routes into MVU (N4 — one
  message substrate) — coordinate with the MVU state interface.
- ~~Broader task coverage before finalizing Track C/D naming~~ — **DONE:** measured-priors now spans
  N=4 (card / MVU / list / events); F6 tokens are the universal deterministic failure.
- Pixel-class GPU-readback lane — deferred; decide if/when (prevention via typed tokens + camera-panic
  covers the class for v1).
