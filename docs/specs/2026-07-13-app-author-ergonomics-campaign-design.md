<!-- Spec: campaign charter — resolve the open items of the app-author-ergonomics learnings report. -->
<!-- Date: 2026-07-13. Type: campaign design / charter. -->

# Buiy — App-Author Ergonomics Campaign (charter)

- **Date:** 2026-07-13
- **Status:** COMPLETE (on the campaign branch; landing held for the M1 merge).
  Spec-gate passed (3-lens adversarial review, all APPROVE_WITH_CHANGES, zero BLOCK;
  every finding applied). All six tracks A–F executed, each implement→adversarial-
  review→(fix)→gate, committed, and **fully verified**: both GPU legs on the RX 6700 XT
  (buiy_core 55/0 incl the top-layer occlusion fixtures, buiy_verify GPU), workspace
  headless (155 suites / 0 failed), release (`-p buiy_core`, debug diagnostics compile
  out), `clippy -D warnings`, `fmt`. The GPU lane earned its keep — it caught a real
  Track B predicate false-positive (occluder predicate missed gradient/band paint) the
  headless review missed (fixed, commit `0cfafb9`; reinforces rec 2 / Track E). Two
  benign snapshot classes re-blessed (Bevy-0.19 resource-as-entity +1 anonymous-entity
  shift from Track A's debug `MvuDiagnostics`; each verified layout-identical + both
  executor lanes). **Landing:** rebase onto post-M1-merge `main`, re-run the gate, then
  PR — per the owner's "rebase when the merge is in".
- **Base:** `campaign/app-author-ergonomics`, cut from `feat/dooduel-multiplayer-m1`
  (which fully contains `origin/main` + the 142 Dooduel M1 commits). Landing is
  **gated on the M1 merge**; the campaign branch will be **rebased onto
  post-merge `main`** before its PR (per the owner's call: "get started and
  rebase when the merge is in").
- **Source report:** `docs/reports/2026-07-13-buiy-app-author-ergonomics-learnings.md`
  (the cross-campaign synthesis of six dogfooding campaigns, with 7 ranked
  recommendations).
- **Reconciliation:** a 7-agent audit of every recommendation sub-item vs this
  base, gated by a 3-lens adversarial re-verification. Result: **24 LANDED, 7
  PARTIAL, 6 OPEN** of 37 sub-items — the report is ~65% already delivered on this
  base. The full per-item table with code evidence and dispositions is persisted at
  **`docs/reports/2026-07-13-app-author-ergonomics-reconciliation.md`**.

## 1. What this campaign is

Resolve the **genuinely-open** items of the 2026-07-13 learnings report. The report
synthesizes campaigns, several of which already merged fixes (ColorToken/
ThemeContract, domain accessors) or landed them on this Dooduel M1 base
(top-layer composite, AT `set_value` fold, the coherent view layout surface, the
runtime interaction layer, `ClockPlugin`). Chartering against the raw 7-rec list
would re-do landed work; this campaign scopes to the 13 open/partial sub-items,
reconciled below.

## 2. Reconciliation ledger — what is already done

Fully **LANDED** on this base (no work here — cited for provenance):

- **Rec 3 (view layout surface)** — ENTIRELY landed: `scroll_column` + controlled
  stick-to-bottom, `.wrap()`, per-side padding, `.text_align()`, `inset/absolute`,
  shipped in one coherent pass (Dooduel F2, `2026-07-03-dooduel-final-design.md`
  §2.2). The report's "design it in ONE pass" ask was executed literally.
- **Rec 4 (pick≠paint)** — top-layer stacking composite LANDED 2026-07-12 (W0–W7,
  GPU-verified); auto-`Pickable::IGNORE` for transparent `.top_layer()` +
  `.ignore_picking()` escape landed in `buiy_view`; pick order == paint order via a
  shared `global_paint_order` derivation. Residuals only (Track B).
- **Rec 1b** ColorToken/ThemeContract (Track B, PR #113); **1c** `TextChanged` on AT
  `set_value` (Dooduel M1); **2a** `Last`-scheduled render-coherence assert; **2b**
  native-pointer live-interaction tier; **5b-1** documented headless plugin preset;
  **5c** self-sufficient prelude; **6a** `on_submit_with`; **6b** `ClockPlugin<M>`;
  **6d** `ControlledLeaf`; **6e** MVU-side field clear (the *mechanism*: controlled
  `text_input` value → `reconcile.rs:278 set_editor_value`; there is no literal
  `EditCommand::SetValue` verb — a reducer that sets the draft to `""` clears the
  field); **7a-convention** scene-fn per widget (all 14); **7b** domain accessors +
  un-preluded accesskit enum.

## 3. Scope — in / deferred

### 3.1 In scope (6 tracks, 11 items)

| Track | Items | Rec | Effort | Uncertainty |
|---|---|---|---|---|
| **A — Fail-loud diagnostics** | 1a-ii, 1a-iii (on the §7.5 precedent), 1e = extract the shared debug-validation helper | 1 | M + M + S | low–med |
| **B — Pick/paint hardening** | 4b-invariant, 4b-scope (fail-loud), 4b-pickrect (confirm-no-violation → doc) | 4 | S + M + S | low–med |
| **C — App-author guidance** | 5a using-mvu guide (+ ContentBox/padding sizing gotcha), 5b-2 MvuTestApp | 5 | M + M | low–med |
| **D — Declarative `:hover`/`:active`** | 6c | 6 | M | med |
| **E — Incremental GPU CI** | 2c | 2 | S | med |
| **F — Open-state accessors** | 7b-overlay | 7 | S | low |

### 3.2 Deferred — with rationale (surfaced for owner veto)

- **7a-lint — DEFER.** Surfacing the §4.1c required-component suppression at
  *compile/lint* time is **XL / high-uncertainty**: the suppression lives inside
  upstream Bevy's `bsn!` proc-macro that Buiy does not own; there is **no
  dylint/clippy-driver infra in-repo**; the spec (`2026-06-18-buiy-bsn-integration
  -design.md` §4.2) explicitly rejects a proxy-DSL wrapper. All three candidate
  mechanisms (custom `LateLintPass`, upstream `bsn!` change, proxy DSL) are
  problematic, and the fresh auditor independently recommended *not* hand-rolling a
  bespoke lint. The **interim mitigation is already landed** — a scene-fn for every
  styleable widget (all 14) + a round-trip regression test pinning the behavior.
  **Action here:** strengthen the documented convention (Track C's using-mvu / BSN
  guidance references it), and log 7a-lint as a deferred follow-up revisitable if
  Buiy adopts a lint toolchain or forks `bsn!`. Not worth XL speculative,
  brittle infra now.
- **1d-deep-web — ROUTE to the web-firstclass campaign (#143).** The report's
  literal "a correct tree reaches ZERO AT on web" worry is **obsolete for outbound
  a11y** (`WebA11ySinkPlugin` ships), and the genuine residual (inbound web AT
  actions still inert; best-effort clipboard/IME degrade silently; no web-a11y CI
  conformance guard) is **already owned by the active web-firstclass campaign**
  (draft PR #144, blocked on the M1 merge). Duplicating it here would collide.
  **Action here:** Track A's 1e mechanism becomes the designated *home* for a
  "capability inert" fail-loud signal (a hook), so #143 can route web-inertness
  through it; the substantive inbound-AT/clipboard/IME work stays in #143.

## 4. Tracks

Each track runs its own staged sub-pipeline: a brief per-track spec **only where
design is genuinely open** (A's 1e mechanism, C's 5b-2 API, D's style API),
otherwise plan → execute directly. Every unit is executed by a fresh implementer
subagent, gated by a reviewer, and verified against the running artifact — never
inline. Fan-outs run under `reliable-agent-fleet`.

### Track A — Fail-loud diagnostics (rec 1; the report's #1 leverage theme)

**The concrete deliverable is 1a-ii + 1a-iii** — a single debug-only validation
over `Model`-bearing entities. The codebase already has the exact pattern to reuse
(the review gate confirmed this): the MVU §7.5 auditor (`mvu/mod.rs:566-593` —
`#[cfg(debug_assertions)]` `FunnelWriteStamps`/`FunnelViolation`/`FunnelAuditLog`:
per-operation `warn!` *plus* a typed resource tests assert on) and the
`keyed_column` dup-key `debug_assert` (`reconcile.rs:395`). **1a-ii/1a-iii build
directly on that precedent — they are NOT gated behind a 1e design step.**

**1a-ii.** A live fold on a `Model` entity lacking a `LogicalId` silently stamps
`LogicalId::UNRESOLVED` (`mvu/mod.rs:922-925,1257`), corrupting the replay log
*before* the (already loud+typed) replay-time dead-letter fires. Add a debug-only
`Last`/startup validation flagging `Model`-bearing entities without a `LogicalId`,
mirroring the §7.5 `FunnelWriteStamps` auditor (per-operation `warn!` + typed log).

**1a-iii.** Duplicate `LogicalId` (two entities sharing an id) is undetected —
`resolve_lid` (`replay.rs:225-230`) silently picks the first. Extend the same
debug-only validation to detect collisions. Shares the impl with 1a-ii.

**1e (minimal — extract, don't invent).** Once 1a-ii/1a-iii are written on the
§7.5 pattern, factor out the *shared* piece they and the existing precedents have
in common: a small `#[cfg(debug_assertions)]` validation-system + typed
violation-log helper with **one** release-degrade path (loud per-operation in
dev/test, silent in release). Scope guard: **NOT** a `dev_assert!` macro-DSL or a
structured-diagnostic-code registry (over-engineering — no in-repo consumer needs
it), and **NO** "capability-inert hook for #143" (zero in-repo caller until #143
lands — #143 builds its own home). Migrate the ad-hoc precedents onto the helper
only where it's a clean no-behavior-change refactor; do not churn stable code.

**Verify:** headless tests that assert the loud path fires in dev/test (a model
with no id → per-operation `warn!` + a typed-log entry a test asserts on; a
duplicate id → same), and that release builds degrade silently; the existing
zero-dead-letter assertions (`mvu_at_seam.rs`, `todomvc.rs`) stay green.

### Track B — Pick/paint invariant hardening (rec 4 residuals)

**4b-invariant.** The occluder predicate exists and is app-agnostic
(`buiy_verify/src/invariant/occluder.rs`) but only runs over `buiy_view`'s
6-fixture catalog (`pointer_occluder.rs`). Wire it as a **standing gate over the
`buiy_widgets` gallery fixture catalog** (the higher-yield target — hand-authored
widget trees), and over the `apps/dooduel` screens *if* additive beyond the
existing targeted `apps/dooduel/tests/in_game_occlusion.rs` coverage. Not purely
mechanical: it means reusing the gallery/dooduel headless drivers to reconcile
each screen and sweep the invariant — budget a small per-surface fixture-boot
harness. Fail-on-revert (add a transparent occluder to a fixture → red) is the
acceptance.

**4b-scope.** The auto-`Pickable::IGNORE` construction guarantee lives only in the
`buiy_view` reconciler; hand-authored `buiy_core`/`buiy_widgets`/`bsn!` top-layer
nodes rely on the invariant, not construction. Implement the framework-wide guard
as a **debug-only `buiy_core` `Last`-scheduled coherence system that DEBUG-PANICS
(fail-loud) on a transparent top-layer `Node` lacking `Pickable::IGNORE`** — *not*
a silent auto-IGNORE. Rationale: silently auto-repairing a hand-authored bug is
the *exact* fail-silent anti-pattern this campaign exists to kill (rec 1); a
loud panic is consistent with `reconcile.rs:395` / the §7.5 precedent.
Auto-IGNORE-at-construction stays confined to the `buiy_view` reconciler, where
"unwritable by construction" is the accepted, escape-hatchable (`.ignore_picking()`)
design. Subordinate to 4b-invariant as the standing test-time net.

**4b-pickrect.** *Corrected from the report/follow-ups phrasing:* there is **no
pick≠paint violation** here. Pick (`picking/backend.rs:99,153`) and paint
(`render/extract.rs:109-110,249-250`) both read `ResolvedLayout.size` = the border
box (88×50); the 72×34 is the `ContentBox` *content* box, not a "painted pill." So
pick == paint == 88×50 — **working-as-intended, the invariant Track B protects is
intact.** Disposition: **document, don't fix** — record in this spec + close the
`follow-ups.md:2294` entry as no-violation. The *real* footgun it exposed —
`BoxSizing` default `ContentBox` + `button()`'s 8px padding makes `.width(72)`
render 88px — is a sizing surprise, not a picking bug, and is handled as a **Track
C guidance item** (§ Track C), not a Track B change.

### Track C — App-author guidance (rec 5 residuals)

**5a.** Author a task-oriented **`using-mvu` guide/skill** (mirror
`using-buiy-verification`): define Model+reducer, register via `mvu_model`,
**enqueue-not-fold**, read live widget state, clock-drive (`ClockPlugin`/
`advance_clock`), headless test (the preset + the Track C `MvuTestApp` builder),
the tiers (leaf/machine/raw-ECS), and the silent-wrong gotchas (LogicalId
dead-letter, `set_if_neq`, enqueue-not-fold). Consolidates the scattered
AGENTS.md §State + getting-started §8 coverage. **Content coupling:** the
LogicalId-gotcha section is authored *after* Track A lands, so it describes the
actual shipped fail-loud shape (per-operation `warn!` + typed log) rather than
guessing.

**5a-sizing (from Track B 4b-pickrect).** Document the `BoxSizing` gotcha the
theme-toggle exposed: `BoxSizing` defaults to `ContentBox`, so `.width(72)` on a
padded `button()` (8px) renders an 88px border box — the authored width is the
*content* width, not the outer size. Add it to the guide's gotchas (and/or the
view-layout doc): to size the outer box, account for padding or use
`BoxSizing::BorderBox`. This is the real, documented residual of 4b-pickrect; no
code change.

**5b-2.** Ship an ergonomic exported **`MvuTestApp`-style builder** (a
`buiy::test` module or test-support helper): stand up the headless preset +
register the reducer + spawn the model + step + read/assert, so unit-testing a
model isn't a per-suite hand-rolled harness (`counter_app`/`logic_app` prove the
pattern is re-derived per suite). Building blocks are shipped + preluded. Small
API-placement design call first.

### Track D — Declarative `:hover`/`:active` style API (rec 6c residual)

The runtime interaction layer (`buiy_view/src/interaction.rs`,
`InteractionState{None,Hover,Press}`) already owns transient state **outside the
pure model** and press-down visuals resolve. Add a **declarative hover/active
style API** + a resolver reading `InteractionState::Hover`. Design call: how
state-conditional styling is expressed in a pure-view world (e.g.
`.on_hover(Style)` / `.hover_bg(token)` builder that the runtime applies, never
the model). Brief spec first.

### Track E — Incremental GPU CI (rec 2c)

The `gpu` job (`ci.yml`) runs on push-to-main, on any open PR, and weekly — but
**not** on a campaign/feature branch with no open PR, which hid a real lavapipe
regression until PR-open (widget-catalog #80). The full lane is a ~90-min
pinned-lavapipe/disk-heavy job, so blanket per-push on all branches is costly.
**Recommended:** document the **draft-PR-early** practice (opens the PR-triggered
GPU lane on every push, zero extra cost beyond a PR) as the primary fix, and
optionally add a **scoped `campaign/**` push trigger** for a *lighter* GPU subset.
Deliver as a short process doc + a minimal, cost-aware `ci.yml` change if warranted.

### Track F — Open-state accessor consistency (rec 7b-overlay)

Namespace the shared overlay open-state reader: add `Popover::is_open` /
`Menu::is_open` / `Dialog::is_open` (today only a free `popover::is_open(vis)` at
`popover.rs:284`, reused by menu/dialog) + a `Tooltip` open-state reader, matching
the value-bearing widget domain-accessor pattern. Keep the shared implementation;
this is a namespacing/uniformity nicety. Cheap.

## 5. Sequencing

1. **Track A first** — build 1a-ii/1a-iii directly on the §7.5 precedent, then
   extract the minimal 1e helper from them (no internal design-gate; 1e is a
   refactor-out, not a prerequisite). A first because Track C's `using-mvu` guide
   documents the resulting fail-loud shape.
2. **Tracks B, E, F in parallel with A** — genuinely independent, no shared surface
   (B = render/verify + widgets, E = CI, F = widgets). Run as parallel waves under
   `reliable-agent-fleet`.
3. **Track C** — 5b-2 (`MvuTestApp`) and most of 5a can start alongside A, but 5a's
   **LogicalId-gotcha section is content-coupled to Track A** and is finalized only
   after A's fail-loud shape ships. Sequence C's write-up to land after A.
4. **Track D independent** — can run alongside the above; its design call warrants
   its own brief spec + gate.
5. **Docs close-out + rebase + land** — after all tracks verified: update the
   learnings-report status, `follow-ups.md`, `docs/README`; record the deferrals;
   rebase onto post-M1-merge `main`; full gate green; then PR.

## 6. Verification strategy

Per `CLAUDE.md`'s Build & Test: the headless workspace gate
(`cargo fmt --check` + `clippy -D warnings` + `cargo doc -D warnings` +
`nextest --workspace`) must stay green **without** an adapter, and **both GPU legs**
(`buiy_core` + `buiy_verify`, `--ignored --test-threads=1`) must pass on a GPU host
(RX 6700 XT / lavapipe). Track-specific:

- **A:** headless assertions that the loud path fires in dev/test and degrades in
  release; existing zero-dead-letter tests stay green.
- **B:** the invariant sweep must fail-on-revert (add a transparent occluder to a
  fixture → red); no golden churn (top-layer composite already byte-stable).
- **C:** the `using-mvu` guide's examples compile (doctest / an `agents_md_examples`
  -style test); the `MvuTestApp` builder has its own passing tests + is dogfooded
  by migrating one hand-rolled harness.
- **D:** a live-interaction test drives synthetic move→settle (hover) and asserts
  the hover style resolves; press still works.
- **E:** the process doc + any `ci.yml` change validated (`actionlint` / dry trigger).
- **F:** accessor doctests; existing overlay tests stay green.

Every track: **run the actual artifact** (the learnings report's dominant lesson —
headless-green ≠ works) before calling it done — the gallery / dooduel / the MVU
example, as applicable.

## 7. Rejected alternatives

- **Charter against the raw 7-rec list** (rejected): would re-do ~24 landed
  sub-items; the reconciliation audit is the whole point.
- **Build the 7a-lint compile guard now** (rejected): XL/high-uncertainty,
  upstream-owned, no in-repo lint infra, interim mitigation landed — see §3.2.
- **Re-implement web inertness surfacing here** (rejected): owned by #143; would
  collide. (Also rejected: shipping a "capability-inert" 1e hook now — it would
  have zero in-repo caller until #143 lands.)
- **1e as a `dev_assert!` macro-DSL + diagnostic-code registry** (rejected):
  over-engineering — no in-repo consumer needs it; 1e is the minimal shared helper
  extracted from 1a-ii/1a-iii, on the existing §7.5 pattern (charter review, MAJOR).
- **4b-scope as a silent auto-`Pickable::IGNORE`** (rejected): silently
  auto-repairing a hand-authored bug is the fail-silent anti-pattern the campaign
  exists to kill — use a debug-panic instead (charter review, MAJOR).
- **One big execution step** (rejected): staged, per-unit subagent + review gate,
  parallel waves where independent (`staged-development` / `reliable-agent-fleet`).
