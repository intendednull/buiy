# Buiy documentation coverage & quality audit

**Date:** 2026-06-30
**Status:** `[active]` — findings surfaced; the cheap accuracy fixes are applied (see § Fixed in this pass), the structural gaps are an open remediation backlog.
**Baseline:** `origin/main` @ `d285e6b` (drift-sensitive findings re-verified against it; the audit fan-out ran on the `abb76fb` worktree that carries the in-flight docs-sync edits).
**Method:** 6-agent parallel audit (rustdoc/API · verification how-to · examples · authoring patterns · onboarding/navigation · accuracy/staleness) + a completeness critic; every "missing" claim grounded in a grep/`find`/`git show`.

## Headline

To the owner's question — *"do we have documentation covering all our verification pathways, code usage, examples, patterns?"* — the answer splits cleanly:

- **Design documentation is deep and, after the 2026-06-30 docs-sync, accurate.** The `docs/` tree (≈75 specs + 65 plans + 25 reports + a large prior-art corpus) and the type/module-level rustdoc are genuine strengths.
- **USAGE / how-to documentation is the systemic gap.** There is **no getting-started guide, no how-to home, and no worked example for the flagship MVU state interface.** Every "start here" pointer dead-ends in target-state *design* specs written for implementers. A user can spawn a `Button` from the README, but anything past the first screen (own state, theming, a custom widget, layout patterns) is only reconstructable by reading design docs — and for five subsystems the design doc the rustdoc points at was never written.

Net: coverage is **strong for "what/why" (design) and weak for "how" (usage)**. The single highest-impact fix is a user-facing *Building a UI with Buiy* guide with a compiled `hello_mvu` at its spine.

## What's strong (keep)

- **Module/type rustdoc.** Every major `buiy_core` module carries a `//!` header; key user types (`BuiyPlugin`, `BuiyHeadlessPlugin`, the prelude, widget markers, MVU `Model`/`Reducer`/`MvuAppExt`/`mvu_model`, `Theme`, `A11yRole`) have solid `///` docs. Item-doc coverage ≈ 88 % buiy_core, 86 % widgets, 94 % verify, 100 % buiy/buiy_bsn.
- **The verification design + skill.** The 5-tier visual pyramid is documented across four consistent surfaces (`using-buiy-verification` skill, the 2026-06-15 design spec, `buiy_verify/src/lib.rs`, CLAUDE.md § Build & Test), with fixture authoring, determinism knobs, and gotchas.
- **README first-minute.** Full description, subsystem tables, a compiling Quick-start, Demos, Requirements, Documentation section.
- **Accuracy of superlatives.** Sampled "fully/complete/100 %/covers all" claims in rustdoc are exact CSS terms or immediately caveated — no systemic overselling (contrast the 2026-06-18 prose audits).
- **Governance.** `SECURITY.md` + `CODEOWNERS` exist (CI-hardening PR #78).

## Gaps by dimension (severity in brackets)

### 1. Usage / how-to / getting-started — **the systemic gap** [high]
- **No usage-doc home.** `docs/` has only four doc types (spec/plan/report/prior-art); a `find` for guide/tutorial/getting-started/how-to returns nothing but external prior-art. The one how-to (`using-buiy-verification`) is contributor-only and skill-gated.
- **No "build your first Buiy UI" narrative.** The entire user-facing story is the ~40-line README Quick start + three ~30–80-line `hello_*` examples + `buiy_gallery` (a 5,900-line *showcase*, not a walkthrough).
- **MVU has no worked user example.** `crates/buiy_core/src/mvu/mod.rs` has an excellent module `//!` but **zero code fences**; no `examples/hello_mvu`; no example anywhere calls `add_model`/`mvu_model`/`add_reducer`. MVU is marketed as user-reachable but has no copy-pasteable end-to-end path. *(This gap is reported independently by 4 auditors — it is one real, high-value defect.)*
- **Patterns live only in agent-facing/implementer docs.** The three authoring fronts + the §4.1c "single-field patch drops `#[require]` defaults" gotcha, tier selection (leaf vs machine vs raw-ECS), and theming recipes are in CLAUDE.md Code Conventions (agent-facing) or buried per-widget rustdoc — not where a user looks.

### 2. Verification-pathway docs — strong, with concrete defects [high/medium]
- **[high] `--test goldens` is a dead command in 5 places.** The Audit-#9 test-binary consolidation folded `goldens` into `verify_gpu` (via `#[path]`), so `cargo test -p buiy_verify --test goldens …` now errors `no test target named 'goldens'`. It is wrong in `using-buiy-verification/SKILL.md:145,147`, `goldens.md:222`, the **runtime panic** `golden/check.rs:301,309`, and `verify_gpu/goldens.rs:7,10`. This blocks the primary golden-bless workflow *and* is the exact command the harness prints on a golden failure. Fix: `--test verify_gpu` + a `goldens::` filter (or the whole-crate form CLAUDE.md:77 already uses).
- **[medium] Two accessibility verification pathways have no how-to.** `buiy_verify::a11y` (AccessKit-tree snapshot, gate #3) and `::contrast` (WCAG linter, gate #9) are real CI-gated modules but appear in no how-to surface — notable for an accessibility-first library.
- **[medium] The live-interaction tier is undocumented as a tier.** `examples/buiy_gallery/tests/interaction.rs` (real shell + real picking + synthetic click) caught a class of bug all five documented tiers missed, yet exists only in a report + its own module doc; the skill still advertises a fixed "five-tier pyramid".
- **[medium] Skill drift from CLAUDE.md/CI:** the skill's GPU-lane command drops the `buiy_core` leg; the documented `cargo test` gate omits `--locked` and does not reproduce CI's `nextest --unreferenced=reject` (an orphan `.snap` fails CI but not the documented gate); CLAUDE.md's "mirrors what CI runs" is therefore imprecise.
- **[low] Bless env vars `BUIY_BLESS_REASON` / `BUIY_BLESS_REPLACE` are under-documented** (read by `check.rs:235,113`, absent from the skill's bless workflow).

### 3. Rustdoc / API usage [medium]
- **No `#![warn(missing_docs)]` anywhere**, so undocumented public items are silent — the root cause that lets the gaps below persist. (CI's `RUSTDOCFLAGS=-D warnings` catches broken links, not missing item docs.)
- **Almost no compiled doctests:** 3 runnable `` ``` `` blocks workspace-wide; **all 13 buiy_widgets examples are `` ```ignore ``** (never compiled → free to rot as the widget/Style/BSN APIs churn).
- **Style builder: 64 of 84 fluent methods + all 14 fields undocumented** (`layout/style.rs`) — the README sells it as the layout front-end, but its rustdoc is a wall of undocumented one-liners.
- **The `buiy` crate-root `//!` is a 3-line stub** whose only pointer is a repo-relative path that doesn't resolve on docs.rs — the front door for every Rust user shows one sentence, no quick-start, no prelude, no feature list.
- **Feature flags (`default_font`, `clipboard-image`, `multi_threaded`, `webgpu`) are invisible in rustdoc**, and no `[package.metadata.docs.rs]` exists — so feature-gated public items (`ClipboardImage`, `get_image`/`set_image`) won't appear on the published docs page at all.

### 4. Examples [medium]
- All 7 examples have good `//!` headers and appear in README Demos with a concept line, but **no per-example or `examples/` README** (so browsing `examples/` on GitHub shows bare dirs).
- **No `hello_mvu`** (the highest-value missing example — see §1).
- *(Caveat: the examples auditor returned a degraded/stub report; this dimension was spot-filled by the critic, not fully audited. Re-run recommended if a deeper example inventory is wanted.)*

### 5. Onboarding & navigation [medium]
- **`docs/README.md` "Where to start" serves only implementers/agents** — steps 1–6 are all design specs. There is no user track.
- **No `CONTRIBUTING.md`** (nor `CODE_OF_CONDUCT`, issue/PR templates); the only dev guide is the agent-facing CLAUDE.md, whose real check-commands are discoverable only if you know to open an agent-instruction file.
- Every "start here" / "good entry points" pointer terminates in a spec or bounces between the README ↔ `docs/README.md` ↔ CLAUDE.md index trio; none ends at a how-to.

### 6. Accuracy / staleness [medium/low — mostly fixed this pass]
- **[fixed] README said the pipeline "runs inside Bevy's render graph"** — Bevy 0.19 removed that API; the pass is a `Core2d`-schedule system (`render/node.rs:1-12`). Corrected.
- **[fixed] Merged specs/plans still headed "merge-gated / DO NOT MERGE / target-state"** (MVU #87, widget-catalog parity #83) and the `docs/README.md` widgets prose said "awaiting human verification + PR". Corrected.
- **[open, low] 8 public-rustdoc comments still label current Bevy 0.19 behavior "0.18"** (anchor: `crates/buiy/src/lib.rs:180` on the flagship `BuiyPlugin`) — behavior claims are still true, only the version label misleads. `render/node.rs`'s 0.18 mentions are correct historical framing (leave).
- **[open, low] Dangling rustdoc pointers to 5 never-written specs** (`buiy-accessibility-design`, `buiy-clipboard-and-os-integration-design`, `buiy-focus-model-design`, `buiy-input-events-design`, `buiy-theme-tokens-design`) — the "read the spec for usage" fallback dead-ends for exactly the interaction/theming areas a user needs.
- **[open, low] Render spec + `docs/README.md:104` still describe the deleted `extract_buiy_draws` path** (removed by perf #9 / PR #84).

## Fixed in this pass
Cheap, unambiguous accuracy defects, corrected as a continuation of the 2026-06-30 docs-sync:
- `README.md` — "render graph" → `Core2d` render-schedule phrasing (2 sites).
- MVU spec + plan headers → `[landed]`, merged PR #87.
- Widget-catalog parity design + plan headers → `[landed]`, merged PR #83 (removed "DO NOT MERGE" / "Merge-gated on HUMAN REVIEW").
- `docs/README.md` widgets prose → "COMPLETE + MERGED (PR #80 / #83)" (was "awaiting human verification + PR", stale 1653 count).

## Prioritized remediation backlog

| # | Item | Effort | Audience | Notes |
|---|---|---|---|---|
| 1 | **`docs/guide/getting-started.md` — "Build a UI with Buiy"** (plugin → widgets → Style → OnPress → theme → `bsn!`), registered as a doc type in `docs/README.md`, linked from README + the `buiy` crate `//!` | large | user | *the* answer to the owner's question; also the terminal destination that fixes the "every pointer ends in a spec" problem |
| 2 | **Fix the dead `--test goldens` command** in all 5 sites (skill, spec, `check.rs` panic, `goldens.rs`) | quick | contributor | touches code (a panic string) + skill + spec — one commit |
| 3 | **`examples/hello_mvu`** (Counter `Model`+`Msg`+reducer+`mvu_model`+`enqueue`) + a compiled MVU doctest in `mvu/mod.rs` | medium | user | pairs with #1 as the guide's spine |
| 4 | **Expand `crates/buiy/src/lib.rs` `//!`** into a real docs.rs landing (quick-start `no_run` doctest, prelude note, feature list) + add `[package.metadata.docs.rs] all-features` | quick | user | front door for every Rust user |
| 5 | **`#![warn(missing_docs)]`** on buiy/buiy_core/buiy_widgets/buiy_bsn + **backfill the 64 Style methods + 14 fields** | medium | both | lint prevents regression of #4/§1; Style backfill is the highest-value item-doc fill |
| 6 | **`CONTRIBUTING.md`** (branch-from-`origin/main` + PR-and-wait-for-green + GPU-lane + where docs go), pointing at CLAUDE.md § Build & Test | quick | contributor | human front door; GitHub surfaces it |
| 7 | **Verification how-to top-ups:** a11y + contrast gates, the live-interaction tier, both GPU legs, `--locked`, the nextest/orphan-snapshot reality, bless env vars | quick–medium | contributor | edits to the skill + spec tier table |
| 8 | **Fill CLAUDE.md `## Project Overview` + source-tree TODOs** (copy-forward from README) | quick | contributor | dead placeholders since well before 87 PRs |
| 9 | **Sweep "0.18" → "0.19" in the 8 public rustdoc comments**; fix/repoint the 5 dangling `buiy-*-design` pointers; drop the deleted `extract_buiy_draws` mentions | quick | both | accuracy tail |

## Remediation applied (2026-07-01)

Same-session follow-up, all verified green (`cargo fmt --check`, `cargo doc -D warnings`, the
`buiy` + `buiy_core` doctests, clippy):

- **#1 Getting-started guide** — `docs/guide/getting-started.md`, registered as a new **Guide**
  doc type + a user track in `docs/README.md`, linked from the README and the `buiy` crate `//!`.
- **#2 Dead `--test goldens`** — fixed in every live site (skill, spec `goldens.md`, the
  `check.rs` panic ×2, `verify_gpu/goldens.rs`, and the active `follow-ups.md`). The lone
  remaining hit is in the `[landed]` `verification-impl.md` plan, left intact as a historical
  snapshot (its surrounding file paths are also pre-consolidation).
- **#3 MVU worked example** — a compiled MVU doctest added to `mvu/mod.rs` (passes). The
  standalone `examples/hello_mvu` was **dropped as redundant** on rebase: PR #91 landed the same
  worked MVU counter (`hello_button` migrated to MVU) plus a whole-list `examples/todomvc`.
- **#4 `buiy` crate landing** — expanded `//!` (quick-start `no_run` doctest, prelude, MVU note,
  feature flags) + `[package.metadata.docs.rs] all-features` on `buiy` and `buiy_core`. The
  audit's MVU-not-in-prelude finding was independently closed by PR #93 (which preluded the MVU
  surface, acting on PR #91's dogfooding report); the guide + `//!` reflect that.
- **#6 `CONTRIBUTING.md`** — added.
- **#7 Verification how-to** — the skill now covers the a11y + contrast gates, the
  live-interaction tier, both GPU legs, `--locked`, the nextest/orphan-snapshot reality, and the
  bless env vars.
- **#8 CLAUDE.md TODOs** — Project Overview + source-tree filled (the "conventions" TODO left as
  genuinely forward-looking).
- **#9 (partial)** — the 8 stale `0.18` rustdoc labels swept to `0.19` (or dropped where
  version-independent). The 5 dangling `buiy-*-design` rustdoc pointers and the deleted
  `extract_buiy_draws` doc mentions remain **deferred**.

**Deferred by owner choice:** #5 (`#![warn(missing_docs)]` + the 64 Style-method / 14-field
backfill + converting the widget `` ```ignore `` examples to compiled doctests).

## Baseline & method note
The audit ran at `abb76fb`; `origin/main` then advanced rapidly during the session, and
drift-sensitive findings were re-verified as it moved. Notably **PR #91** (demos→MVU: the
`hello_button` counter + `examples/todomvc` + a DX report), **PR #93** (preluding the MVU surface —
its recommendation #1), and **PR #94** (WebGL2 render reach) independently overtook parts of the
remediation: the MVU-prelude finding was closed by #93, the worked-MVU-example need by #91 (so this
PR's `hello_mvu` was dropped), and the "WebGPU-only for v1" framing softened by #94. This PR is
rebased onto the current tip (`0a1ca73`, #94) and reconciled against all three.
