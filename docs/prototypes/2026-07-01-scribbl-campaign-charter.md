# Scribbl Campaign Charter — a fully-featured game app, whole-app prototype-first

> **Kickoff brief for the Scribbl campaign — the target app is now named DOODUEL (see the
> 2026-07-02 amendment at the end).** Status: **✅ Phase A COMPLETE; 🚀 Phase B (FINAL) in
> flight.** The begin condition fired 2026-07-02 (the LLM-dev-support campaign fully
> merged: Tracks B #113, A #117, C1–C4 #120/#121/#123/#124 — C4 descoped to
> setter-builders, C5 dropped — and Track D #125, AGENTS.md) and the resume protocol below
> was executed: the gap audit **re-baselined** against `origin/main @ a969cbf`
> (`../reports/2026-07-02-dooduel-rebaseline-audit.md` — G1/G2 delivered, G3 the one
> Tier-1 gap left, canvas still the sole net-new subsystem), and Phase A ran in the
> throwaway worktree `dooduel-proto1` off `a969cbf` — **all waves W0–W8-live
> run-verified, strong design parity reached, and the multi-agent-playtest acceptance
> bar MET** (4 independent agents, one per seat, completed a full match). The learning
> gate is written: the [retrospective](2026-07-02-dooduel-PROTO1-RETROSPECTIVE.md)
> (synthesizing the [journal](2026-07-02-dooduel-PROTO1-journal.md) + the
> [playtest evidence](2026-07-02-dooduel-PROTO1-playtest/)). Phase B is designed: the
> [FINAL spec](../specs/2026-07-03-dooduel-final-design.md) is **approved at revision 3**
> (3-reviewer gate + the 2026-07-03 human gate, skribbl.io-grounded) with the
> [implementation plan](../plans/2026-07-03-dooduel-final.md) committed alongside. This
> charter fixes the intent, method, and resume protocol so any session can pick the
> campaign up without re-deriving it.

## What we're building

**Scribbl** — a skribbl.io-style draw-and-guess game — as a **fully-featured standalone
Buiy app**, targeting **native desktop, web-desktop, and web-mobile** (all three are v1
targets; the mobile-web decision adds touch activation + web soft-keyboard/IME to the
framework feature list).

**Design source:** Claude Design project "Scribbl.io clone design" (DesignSync projectId
`19e829e9-b6ac-4c4a-af97-e70782a0da67`). **The user will share the latest design files at
kickoff — do not build from the cached 2026-07-01 extraction without re-syncing**; the
requirements digest in the gap audit §1 describes that snapshot and may be stale by then.

## The three products (all first-class)

1. **The app** — full feature parity with the design, verified by running it on all three
   targets.
2. **The framework features Buiy lacks** — current Buiy is likely not up to full parity;
   that is expected and is the point. The app is a forcing function and a verification
   opportunity for Buiy itself — another way to ground framework work.
3. **Dev-experience feedback** — what it is actually like to build a real app on Buiy, and
   how to improve it. Especially for the newly-landed UX work we build on top of: we are
   its first consumers, and improving it from dev experience is an explicit goal.

## Method — whole-app prototype-first

The **entire app build is Phase A of `prototype-first-development`** ("we don't know what
we don't know"):

1. **Prototype to full feature parity** in a throwaway worktree — including whatever
   framework changes that takes, at prototype quality, committed to the worktree but
   **never merged**. Speed-to-running and learning over polish.
2. **Journal everything** (the prototype's real deliverable): every issue hit and how it
   was resolved — classified as *framework bug / missing framework feature / DX friction /
   app bug* — plus what surprised us and what we'd do differently. DX friction entries are
   the raw material for product #3.
3. **Retrospective gate at parity:** review **all** the changes it took to get there;
   keep/refine/redesign every decision.
4. **Final, designed from the ground up** with full knowledge of what needs to be done —
   a fresh `staged-development` pass seeded by the retrospective. Framework features ship
   as their own reviewed, **general-purpose** PRs (dogfooded by Scribbl, not
   Scribbl-local); the app ships on top.

This **supersedes the gap-audit report §6 "proposed campaign shape"** (per-wave
prototype-first for the canvas only): the whole app is now the prototype. The §6 wave
decomposition survives as the likely *internal build order* of Phase A (canvas first —
highest uncertainty; then MVU ergonomics; then primitives; then screens).

Working assumption on merge policy during Phase A: everything stays in the prototype
worktree until the retrospective. Obviously-correct upstream bug fixes discovered along
the way *may* merge early as their own PRs at the user's discretion — ask, don't assume.

## Non-negotiables (inherited from the skill, made concrete)

- **Run the artifact every wave** — native desktop routinely; web-desktop and web-mobile
  at least at every milestone (their gaps — touch, soft-keyboard — are exactly the kind
  headless gates can't see).
- **Journal as you go, not after** — it must survive context loss and worktree cleanup;
  commit journal/retrospective/charter docs to the durable `docs/` system (the code is
  the only throwaway).
- **The final re-decides; it doesn't copy.**

## Resume protocol (in order, at the user's ping)

1. **Get the latest design files** from the user (DesignSync re-sync).
2. **Identify what actually landed** from the UX work (PRs, specs, new APIs) and read it
   as research input — we build on top of it and feed back on it.
3. **Re-baseline the gap audit** against then-current main. The Tier-1 MVU-ergonomics gaps
   (view/keyed-list-reconcile helper, press→Msg routing, timer/Subscription) may have been
   delivered by the widget-UX work, and LLM-UX may supply the headless app-driving
   surface — don't duplicate. Evidence file:line in the audit cites `6c4ff22`; re-verify.
4. **Cut a fresh worktree off current `origin/main`** (`git fetch` first — the stale-base
   lesson from the audit itself).
5. **Run Phase A as a full `staged-development` pass** (research → spec → plan → execute,
   gated, fan-outs under `reliable-agent-fleet`), seeded by this charter, the re-baselined
   audit, and the design files. Create the journal from the skill's template at kickoff.

## Known shape of the work (from the audit — subject to re-baseline)

Per `../reports/2026-07-01-scribbl-app-capability-gap-audit.md`: **buildable, one net-new
subsystem** — the freehand drawing canvas (texture-presenting `ImageNode` keystone + a
paint mechanism; strategy **decided by building both** candidates, report Appendix A).
Then MVU app-ergonomics (the missing "V", routing, timer/Subscription), smaller primitives
(timer-ring, MVU Dialog seam, Toast, light-theme completion + toggle + persistence), and
app-level work atop proven demo patterns. Mobile-web adds touch activation + web
soft-keyboard/IME. The audit's §4 lists refuted non-gaps — capabilities that already
exist; don't rebuild them.

## Open questions for kickoff

- Report §7's remaining open decisions (canvas strategy and platform are decided:
  by-prototype and mobile-web-in respectively): MVU-ergonomics scope, timer-ring
  primitive-vs-workaround — both may be mooted by the re-baseline.
- Phase A merge policy for framework fixes (see working assumption above).
- Whether the LLM-UX work changes how we *verify* the app (agent-driven screens?) — fold
  into the re-baselined research.

## Amendment — 2026-07-02 (user direction at go-ahead)

1. **Target renamed Dooduel.** The latest design files are fetched and archived under
   `../reference-designs/` (Dooduel Prototype + Game Spec + a NEW `DoodleAvatar`
   component + a full protokit **design-system token bundle** — colors/typography/
   spacing/motion/elevation CSS — + screenshots incl. a violet theme and a mobile check).
   A `REQUIREMENTS-DELTA.md` in the bundle records what changed vs the audited Scribbl
   snapshot. **Match the target exactly.**
2. **A game on a game engine.** Treat Dooduel as a *game built on Bevy*, not a widget
   demo: game logic through engine idioms (ECS systems/resources, timers/state machines,
   game-time handling), integrated with the MVU surface. The campaign doubles as an
   experiment in **how well Buiy's UI layer composes with game-engine mechanics** —
   journal that seam's DX as first-class material.
3. **Acceptance bar (the stated goal):** the final design fully developed and verified
   working, including a **multi-agent playtest — multiple agents, each playing a
   different player** (Track A's `BuiyProbePlugin` + `snapshot_report` + in-process
   driver is the enabler). Whether "different players" means hotseat seat-switching or
   multiple app instances is a spec-stage decision grounded in the new design files.
4. **Multiple prototypes are allowed** where uncertainty warrants (per the skill — e.g.
   competing canvas strategies, or a second whole-app iteration).
5. **Begin condition:** no user ping needed. Monitor the LLM-dev-support campaign
   (memory `buiy-llm-dev-support-campaign.md`; PRs land on `origin/main`); when it is
   fully merged (C4 + C5 + Track D), run the resume protocol — step 1 (design files) is
   **done** by this amendment; steps 2–5 unchanged — and begin Phase A autonomously.
