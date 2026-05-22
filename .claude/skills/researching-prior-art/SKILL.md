---
name: researching-prior-art
description: Use when documenting an external project at docs/prior-art/<system>/. Drives a 7-stage workflow — landscape scan, structured overview, deep-dive fan-out, review, polish, second review, framing disclosure — that produces a ~10–20 file folder of reference material future readers can consult.
---

# Researching prior art

For when we need to capture an external project as durable reference material. The project might be a competitor we want to learn from, an integration target we're about to build on, a dependency we want a deep mental model of, a sibling design-space neighbor, or just something the team will keep referring to. Output is a `docs/prior-art/<system>/` folder of focused, cross-linked reference files. Process is parallel-agent-heavy because depth matters more than speed.

The skill is **not about competitive analysis**. It's about producing high-quality reference documentation of an existing project, written from our perspective, so future humans + agents can consult it when designing specs instead of redoing the research. **A load-bearing dependency we will hard-bake against deserves a folder *more*, not less, than a competitor** — the closer the project is to our build surface, the more our future spec authors will need a curated, version-pinned, in-tree reading. "Upstream docs already cover it" is not a valid skip reason for a dependency we are about to commit to.

## Required skills

- **REQUIRED:** `organizing-buiy-docs` — prior-art doc conventions (folder per system, header format, archival flow).
- **REQUIRED:** `superpowers:dispatching-parallel-agents` — every stage fans out 3–6 subagents.
- **RECOMMENDED:** `superpowers:requesting-code-review` — the review passes are the same shape.

## When to use

- Adding a new external project to `docs/prior-art/` (competitor, integration target, dependency, design-space neighbor — relationship doesn't matter).
- A load-bearing dependency we are about to commit to (e.g. picked layout engine, picked text shaper, picked a11y bridge). Build a folder *before* writing specs that depend on it, not after.
- Doing a periodic refresh of an existing prior-art doc whose subject project has shipped major releases since the last update.

## When NOT to use

- One-shot research questions ("how does X work?"). Use `docs/reports/` instead.
- Internal codebase audits. That's `general-audit`-shaped work.
- Project we've decided not to invest in tracking. The **landscape scan** stage exists to validate the investment — if a project doesn't merit a folder, capture findings as a `docs/reports/` entry instead.

**Common misjudgement to avoid:** dismissing a load-bearing dependency as "just a library we depend on, not prior art." That framing is wrong — the more we depend on something, the more its design choices, version churn, and gotchas will leak into our specs, and the more our future spec authors will need a curated reading. Treat hard dependencies as priority-1 prior-art targets, not as "out of scope."

## Workflow

Seven stages. Each stage is a checkpoint — verify completion before moving on.

### Stage 1: Landscape scan (validates investment)

Before committing to a full deep dive, place the project in its design-space neighborhood. The goal is twofold: confirm the project warrants a long-form folder, and surface adjacent systems we might want folders for next. Dispatch 3 parallel `general-purpose` agents:

- **Closest neighbors** — projects sharing the most architectural DNA. What each is *today*, not what their marketing says.
- **Adjacent stacks** — sister projects, integration targets, library-not-runtime layers, complementary tools.
- **Substrate layer** — common foundations (e.g. Taffy, cosmic-text, AccessKit, wgpu, WAI-ARIA, CSS specs) the project sits on or relates to.

Each agent: ~600 words, table format, cite source URLs inline. Synthesize into a one-paragraph verdict that names where the project sits and lists adjacent projects worth queueing for their own folders later.

When agents return "skip folder" verdicts, scrutinize the reason. Valid skip reasons: project is dead/abandoned, project pivoted out of relevant scope, lessons subsumed by an already-documented neighbor. **Invalid skip reasons:** "library we'll depend on, not prior art" (this is *exactly* a folder-worthy target — see "Common misjudgement to avoid" above), "upstream docs are good" (folders capture *our* synthesis + version-pinning, not a tutorial duplication), "narrow technical scope" (small folders are fine).

**Stop condition:** verdict places the project clearly, OR you decide the project isn't worth a folder (in which case land findings as a `docs/reports/` doc and stop). Skip this stage if the user has already named the target project explicitly and adjacency placement is obvious.

### Stage 2: Structured overview (one big file)

Dispatch one `general-purpose` agent for a 2,000–3,500-word structured overview covering:

1. What it is (1-paragraph + key facts table)
2. Architecture
3. Component / authoring model
4. Layout / styling
5. Rendering pipeline
6. Text & input
7. Accessibility posture
8. Theming / tokens
9. Distribution / versioning / platform support
10. Apps / users shipping
11. Open problems
12. Lessons for Buiy (validates / avoid / borrow)
13. Recommended reading order
14. Glossary
15. Sources

Output is one markdown file. Don't worry about file structure yet. (Section names above are tuned for UI-library subjects — adapt to the subject. For a non-UI substrate, swap the relevant sections.)

### Stage 3: Split into per-system folder

Following [`organizing-buiy-docs`](../organizing-buiy-docs/SKILL.md) conventions, break the overview into focused files inside `docs/prior-art/<system>/`:

- `README.md` — overview + key facts + ToC + glossary stub + canonical reading order.
- One file per major subsystem (e.g. `architecture.md`, `layout.md`, `rendering.md`, `text.md`, `accessibility.md`, `theming.md`, `distribution.md`, `apps.md`). Adapt to subject.
- `open-problems.md` — what the system structurally doesn't solve.
- `lessons.md` — **the consult-this-when-designing decision file.** validates / avoid / borrow.
- `glossary.md` — system-specific terms.

Each file ~50–150 lines, independently skimmable, ends with `## Sources`. Cross-link between siblings.

Header for every file uses the prior-art frontmatter from `organizing-buiy-docs`:

```
**Date:** YYYY-MM-DD
**Status:** active | archived
**Subject:** <System name + one-line scope>
```

### Stage 4: Deep-dive fan-out

The structured overview is shallow per subsystem. Dispatch ~6 parallel `general-purpose` agents to enrich each focus area. Suggested partition (adapt to subject):

- **A1:** runtime/render internals (architecture / pipeline / state-management additions).
- **A2:** layout + text + input (often new files for each).
- **A3:** accessibility + a11y-bridge integration + abandoned-features post-mortem (often new `abandoned.md`).
- **A4:** distribution + tooling + platform-support + ecosystem (often new `tooling.md` and `ecosystem.md`).
- **A5:** chronological history + governance / funding (new `history.md`, `governance.md`).
- **A6:** third-party critiques + comparisons (new `critiques.md`, `comparisons.md`).

Each agent reads the existing files first to avoid duplication. Returns labeled blocks (`=== ARCHITECTURE ADDITIONS ===`, etc.) for orchestrator to merge. Total enrichment: ~1500–2500 words per agent.

After merge: ~12–20 files, ~2000+ lines.

### Stage 5: First review pass

Dispatch a fresh `general-purpose` reviewer agent. Brief it as "you're seeing this for the first time; tell us if it's reference-grade." Have it evaluate:

1. Accuracy (named versions/dates/people/dollars — these get hallucinated).
2. Coverage gaps (what would a Buiy designer hit that's missing?).
3. Internal consistency (do files contradict each other?).
4. Redundancy (full-paragraph repeats are waste).
5. Tone (fluff, AI padding, soft-pedaling unflattering facts).
6. Practical usefulness (would `lessons.md` + a subsystem file actually help?).
7. Cross-link health.

Output is a verdict: **ship-as-is / fix-then-ship / major rework**, plus specific file:line citations.

**Expected verdict on first pass:** fix-then-ship. The next stage handles the fixes.

### Stage 6: Polish pass

Apply reviewer recommendations. Typically requires more parallel agents:

- **Verification agent for version/date facts** — query GitHub releases, official blog posts, dev pulses against suspect claims.
- **Verification agent for citations** — confirm academic paper IDs, verbatim quotes, ICO numbers, repo URLs all resolve as cited.
- **Worked-example agent** — pick a flagship shipping app, walk it end-to-end. This is the single biggest reviewer-flagged gap on every first pass.
- **Coverage-gap agent** — fill the technical gaps reviewer identified (binary formats, IPC protocols, error models, formal-verification status, hot-reload mechanics).

Apply local text fixes inline:
- Strip "Saves you N years" / "groundbreaking" / "comprehensive" rhetoric.
- Drop superlatives ("most important cautionary tale").
- Move Buiy-pitch paragraphs out of evidence files.
- Fix cross-link targets (after restructuring, anchors drift).
- Fold tiny single-point files into adjacent broader files.

**Mandatory exit step (per Agoric lesson):** `grep -rn` for each corrected fact across the whole folder *before* dispatching the second reviewer. If a fact lives in 3+ files, the grep-sweep catches stragglers that otherwise force a third reviewer round.

### Stage 7: Final review + framing disclosure

Dispatch a second fresh reviewer. Brief includes the polish-pass changelog. Should return **ship now**.

Add a one-paragraph **framing disclosure** to README.md's `## How to use` section. The corpus is written from whatever stance Buiy is currently committed to (parallel-to-bevy_ui, web-platform-parity, AccessKit-first, BSN-native, etc.). The "Implications for Buiy" sub-sections reflect that bias by design. State it explicitly so future readers auditing the design bet itself know the corpus is not a neutral catalog:

```markdown
**Framing disclosure.** These docs are written from a <Buiy-stance>
stance — most "Implications for Buiy" sub-sections frame <System>'s choices
through that lens. Future readers auditing whether <Buiy-stance> is itself the
right primitive should weigh the corpus accordingly: it's a learn-from-<System>-
into-<Buiy-stance> artifact, not a neutral catalog.
```

The disclosure pattern applies whether the project is a competitor, an integration target, or a neighbor — because the lessons file always reads the project through Buiy's current design commitments. Be explicit about that.

Commit. Move on.

## Hard rules

- **No on-disk category folders.** Each system gets its own folder under `docs/prior-art/<system>/`. Categories (`bevy-ecosystem`, `rust-gui`, `game-engines`, `substrate`, etc.) live as section headers in `docs/README.md` only.
- **Folder name = system name.** Lowercase, kebab-case. No date prefix (these are living docs).
- **Per-file `## Sources` section** with all URLs cited in that file. De-dupe across files is fine — sources can appear in multiple files.
- **Verify cited facts before stage 7.** Specific dates, version numbers, dollar amounts, named people, academic paper IDs, verbatim quotes are the high-risk claims. The polish pass MUST run a verification-agent over them.
- **Honest tone.** Surface unflattering facts. Don't soft-pedal. Quote third-party critiques verbatim. Marketing prose ("groundbreaking") is a smell.
- **Each file independently skimmable.** Some duplication across files is fine; full-paragraph repeats are waste. Use cross-links instead.
- **`lessons.md` is the decision file.** It's where the corpus's value lands. Other files are evidence; lessons.md is the synthesis. Format: validates / avoid / borrow.

## Scaling

The Holochain corpus shipped at 20 files / ~2,300 lines after the full 7-stage process. Smaller / less mature systems land at 10–15 files. Don't pad — if a subsystem only has 30 lines of substance, it's a 30-line file, not a 200-line file with filler.

If the system is research-grade (no shipping apps, small community), shrink stages 4 and 6:
- Skip the worked-example agent in stage 6 if there's no flagship app.
- Skip the testing-infra agent in stage 4 if the project's test harness is just `cargo test`.
- Abandoned-features file may not exist if the project is too young.

## Self-improvement

After each prior-art deep-dive, append a `## Lessons` section at the bottom of this file with what worked / what didn't. The user folds good lessons up over time.

## Lessons

<!-- Append `- YYYY-MM-DD — lesson` entries below. Keep each under ~3 lines. -->

**Lessons from prior usage in the Myrhiza project (original home of this skill). Buiy-specific lessons append below, dated 2026-05-22 onward.**

- 2026-05-08 — First execution on Holochain. The 6-agent stage-4 fan-out + worked-example stage-6 agent (Relay deep-dive) was the single biggest quality-mover. Framing disclosure was added in stage 7 after a reviewer flagged the corpus as "not neutral." Don't skip it.
- 2026-05-08 — Second execution on Spritely Goblins / OCapN. Smaller scope (research-grade, no Volla-equivalent flagship app) ⇒ folded the two-stage overview-then-fan-out approach into a single 4-agent fan-out that produced ~12 files directly. Worked-example stage was skipped because no shipping app at scale exists; that's an honest gap rather than a failure to fill it. Frame "no flagship" itself as the lesson in the lessons.md file.
- 2026-05-08 — Stage-1 scoping bias: agents have a tendency to dismiss load-bearing dependencies (transport libs, persistence libs, crypto libs) as "not prior art, just libraries we'll use." This is wrong — those are exactly the targets future spec authors most need a curated, version-pinned reading on. Skill intro + Stage 1 + "Common misjudgement" updated to call this out explicitly. Watch for the same bias on next execution.
- 2026-05-08 — Third execution on Iroh (load-bearing dependency, 21 files, ~2,250 lines). Pre-1.0 fast-moving project ⇒ version-and-date hallucinations clustered in 5-agent fan-out output: wrong workspace-crate listings, wrong fold-in dates, "iroh-ffi archived" claims that didn't match the GitHub `archived` flag, ~1-day blog-post-vs-crate-publication date drift, missed yanked-version flags. The first reviewer pass caught all of these via a `gh api` + `crates.io` API spot-check. **Direct verification (`curl crates.io`, `gh api repos/.../Cargo.toml`) is non-negotiable for projects with rapid release cadence.** Two reviewer passes were needed because the polish pass introduced one new contradiction (transports.md release-table row drifted from the corrected dates in distribution.md/history.md) — the second reviewer caught it before commit. Lesson: when a fact lives in 3+ files, sweep for it post-polish with grep before declaring done. Also: load-bearing-dep framing-disclosure (in addition to the standard Component-Model-stance one) is genuinely useful — flag the corpus's incentive to soft-pedal problems Myrhiza will inherit from its dependency.
- 2026-05-09 — Fourth execution on Agoric/Endo/SwingSet (learn-from target, ocap-lineage cousin to Spritely; 20 files, ~3,400 lines). Stage 1 skipped — adjacency obvious (closes the ocap-runtime pair with Spritely). Five-agent fan-out partitioned by Endo / SwingSet kernel / determinism+persistence / chain+contracts+apps / project-lens; the determinism+persistence pair was deliberately heavier (~560 lines combined) because those are the load-bearing-for-Myrhiza files. Iroh's verification lesson held: the first reviewer caught a 1-day `ses 2.0.0` date drift (2026-04-16 vs actual 2026-04-17), an `@endo/far` version hallucinated as 1.4.1 (actual 1.1.14, last published 2025-07-12 — agent guessed up-bumps), an `@endo/compartment-mapper` 1.4.1 → 2.1.0 jump missed, an `agoric-upgrade-22 → 22b` mainnet-tag drift, and a sha-256 vs SHA-512 bundle-hash error. Pre-1.0 npm packages = same hallucination shape as pre-1.0 crates. **Polish-pass discipline that worked this time:** ran `grep -rn` for each corrected fact across the whole folder *before* dispatching the second reviewer, so the second reviewer returned `ship-now` with no new contradictions (vs Iroh's three). The grep-sweep took ~1 minute and saved a third reviewer round. Make this Stage 6's mandatory exit step.
- 2026-05-09 — Fifth execution on the **WASM Component Model substrate** (load-bearing dependency, treated as one ecosystem-folder for spec + Wasmtime + tooling + WASI; 15 files, ~2,650 lines). Stage 1 skipped — substrate adjacency obvious. Five-agent fan-out partitioned by spec+abi+browser / wasmtime+preview-status / tooling+languages / governance+history+ecosystem / critiques+open-problems+lessons. **Cross-agent contradictions appeared even with each agent self-verifying** because pre-1.0 substrates have multiple "current versions" (WASI subsystem at 0.2.11 vs CM spec HEAD vs jco 1.19.0 vs Wasmtime 44.0.1) and agents picked different snapshots: A1 captured `WASI 0.2.8 + lone preview3 RC 2025-09-16` while A2/A4 captured `0.2.11 + three RCs 2026-01/02/03`. Lesson: **for substrate-level folders where one fact lives in 3+ agents' outputs, fix the canonical version pin BEFORE dispatching by sharing a verified-facts pre-amble in every agent's brief**, or accept the first-reviewer cleanup cost. Polish-pass grep-sweep (now mandatory per Agoric lesson) caught all leftovers; second reviewer returned `ship-now` first try. Three brief-correction patterns worth carrying forward: (a) Wasmer license is **MIT** not Apache-2.0 — agent corrected the brief in-place rather than fabricating; good behavior to encourage. (b) Wasmtime cadence is **monthly** (cut 5th, publish 20th) not the "6-week" my brief inherited — same agent self-correction. (c) When an asked-for verbatim quote isn't findable (Wasmer governance "dispute"), agent wrote "strategic divergence" and flagged the gap rather than fabricating. Encode "if you can't find a verbatim source, say so" in every agent brief.
- 2026-05-09 — Sixth execution on **wasmCloud** (production CM runtime, CNCF Incubating; 15 files, ~2,260 lines). The orchestrator-side verified-facts pre-amble pattern (lesson from Fifth) was applied — every agent got the same `wasmCloud v2.1.0 / wash 0.43.0 / wadm 0.21.1 / wRPC at bytecodealliance/wrpc / Cosmonic primary steward` pin set. **But the brief itself was wrong at a deeper level than version pins**: the brief encoded a v1 mental model (lattice + NATS-as-control-plane + capability providers + link definitions + wadm reconciler), and wasmCloud had reset to v2 (K8s `runtime-operator` + CRDs + in-process host plugins) on 2026-03-22, ~7 weeks before the fan-out. A1 caught the reset independently via `gh api repos/.../Cargo.toml` + `README.md` reads; A2 / A3 / A4 / A5 then converged on the v2-reset narrative as they wrote. Net: agents corrected the brief themselves, but the resulting files needed a heavy first-reviewer pass to surface internal inconsistencies (one file said "wadm reconciles the lattice" while another said "wadm subsumed by runtime-operator"; ~13 polish items total). **New lesson: when the target has known recent major churn (a v2 reset within the last quarter, a project pivot, a stewardship change), expand Stage 1 even when target is named — read upstream `README.md` and the most recent major release notes BEFORE composing agent briefs.** A simple `gh api repos/<owner>/<repo>/releases --jq '.[0:2]'` plus reading `body` of the latest major would have flipped the brief from v1- to v2-flavored before fan-out and saved the cross-file reconciliation cost. Other patterns this round: brief said wRPC license `NOASSERTION`, agent verified Apache-2.0 WITH LLVM-exception via decoded LICENSE; brief said "Spin uses wRPC under the hood", agent verified false (zero refs in `spinframework/spin`); brief said Microsoft Hyperlight is a wasmCloud production deployment, agent verified Hyperlight is a separate CNCF Sandbox project; brief said founders are "Bailey Hayes / Liam Randall / Stuart Harris", agent verified per CNCF announcement that founders are Liam Randall + Kevin Hoffman, Bailey Hayes is Cosmonic co-founder + current tech lead (not project co-founder). All four corrections landed in-doc.
- 2026-05-09 — Seventh execution on **Pears / Holepunch** (consumer-mobile P2P stack: Hypercore + Hyperswarm + Bare + Pear runtime + Keet; 17 files, ~3,550 lines). Closes the prior-art set for the immediate roadmap. Stage 1 skipped (target named, adjacency obvious — closest production-mobile-P2P data point). Verified-facts pre-amble pattern from #5/#6 applied. **Two new patterns worth carrying forward:** (a) **Run per-repo `gh api repos/<org>/<name>/license` for every repo cited.** My pre-amble said "all major Holepunch repos Apache-2.0" — wrong. Verified ground truth: `hypercore` MIT, `hyperbee` MIT, `hyperswarm` MIT, `hyperdht` MIT (Dat-era cores); `hyperdrive` Apache-2.0 (Holepunch-era relicense), `autobase` Apache-2.0, `pear` Apache-2.0, `bare` Apache-2.0 (Holepunch-era originals). Mixed-license families are common where a project has been acquired/forked from a foundation predecessor (Dat → Holepunch ~2020-2021). The first reviewer caught the discrepancy across files; second reviewer caught a residual inconsistency where the README/glossary still said "Hypercore stack: MIT" globally while governance.md/history.md correctly broke down per-repo. **Lesson: never assume a project family is single-license — list every repo + license in the pre-amble, verified per-repo.** (b) **Honest-scale recalibration.** Brief assumed Keet was "consumer-mobile P2P shipped at scale (hundreds of thousands of users)." Agent verified via `itunes.apple.com/lookup?bundleId=io.keet.app` (99 ratings) and AppBrain (Android ~690K lifetime downloads, ~1K ratings, ~110K last-30-day) that the realistic figure is **low-tens-of-thousands MAU class**, not hundreds of thousands and emphatically not millions. The README's framing was rewritten to "research-grade-but-shipping" + an honest-scale-disclosure paragraph. First reviewer caught 4 sites still echoing the old "hundreds-of-thousands" framing; polish-pass rewrote them. **Lesson: for any agent brief that includes a scale claim, require the agent to verify it via App Store / Play Store / npm-downloads / star-counts BEFORE writing the framing — and update the brief in the polish pass to match.** Also worth noting: agents corrected three brief errors in-place (`Noise-XX` → `Noise-IK` per `hyperdht/lib/noise-wrap.js`; Keet bundle id `io.keet.app` not `to.holepunch.keet`; Hyperdrive v13 layout = metadata-Hyperbee + Hyperblobs, not raw two-Hypercore). The agent self-correction discipline established in #5/#6 held cleanly here.
- 2026-05-09 — Eighth execution on **CRDTs (Automerge + Yjs + Loro multi-library survey)** (13 files, ~1,640 lines). First multi-subject survey-style folder — three libraries packed into one folder rather than one project per folder, similar in shape to wasm-component-model substrate. Six-agent fan-out partitioned by per-library × 3 + theory/history + ecosystem/governance + comparisons/open-problems/critiques. Verified-facts pre-amble pattern (lessons 5/6/7) applied across all six agents: shared pin set of versions/licenses/star-counts/algorithm names. **Two patterns worth carrying forward:** (a) **GitHub releases endpoint ≠ npm registry endpoint.** My pre-amble lifted `yjs` v14 release tags from `gh api repos/yjs/yjs/releases` (which lists git tag names like `v14.0.0-rc.13`) and labeled them as npm-published versions. Wrong — npm registry dist-tags showed `next: 14.0.0-8` and `beta: 14.0.0-16`; no `rc.13` was ever npm-published. First reviewer caught it via direct `registry.npmjs.org/yjs` fetch. **Lesson: for any version claim about an npm package, verify via `registry.npmjs.org/<pkg>` `dist-tags` rather than `gh api releases`. Same pattern probably applies for crates.io vs git tags. The release-tag-vs-published-version distinction is real.** (b) **Bundle-size figures decay fast.** First reviewer caught `loro-crdt`'s "1.05 MB raw / 399 KB gzipped" figure as 0.10.x-era (2024) — current 1.12.1 ships 3.16 MB raw / ~1 MB gzipped. The figure had propagated through Jahns's verbatim quote into Loro-current-state framing without version pinning. Lesson: if a quote is verbatim-historical, pin the date *and* the project version; if a current claim, verify via `npm tarball + ls -la *.wasm` directly. Other patterns: agents corrected three brief errors (Fugue authors are Weidner & Kleppmann, not Weidner/Gentle/Kleppmann; Peritext authors are Litt/Lim/Kleppmann/van Hardenberg, not Litt/Kleppmann/Gentle; the `1.23.2` Loro version that one WebFetch summary returned was a hallucination — actual is 1.12.1). All three were caught by the multi-agent triangulation: at least one agent in each parallel group nailed the right author list, exposing the inconsistency for the first reviewer to flag. Encouraging.
- 2026-05-09 — Ninth execution on **MLS / OpenMLS** (IETF group key agreement protocol RFC 9420 + Rust implementation; 13 files, ~1,560 lines). Smaller-scope four-agent fan-out partitioned by protocol+lifecycle+crypto / OpenMLS+other-implementations / production-users+governance / comparisons+open-problems+critiques. Verified-facts pre-amble pattern held: agents corrected three brief errors in-place (TreeKEM authorship is Bhargavan/Barnes/Rescorla — *NOT* Cohn-Gordon et al. 2018 which is the related ART paper at CCS 2018; Apple iMessage uses Apple's own PQ3 protocol — *NOT* MLS — Apple's only MLS exposure is RCS UP 3.0; libcrux is a HACL\*-extracted-Rust + hax-verified-Rust *hybrid*, not a single F\*-via-hax extraction). **Two new patterns worth carrying forward:** (a) **The F\*/hax/HACL\* relationship is the kind of fact that gets summarized inconsistently across agent outputs** because the lineage involves three separate verified-cryptography efforts (Inria HACL\*, Cryspen hax, Cryspen libcrux as the consumer of both). The first reviewer caught the wrong canonical description in governance.md; the second reviewer caught that the polish-pass fix in governance.md didn't propagate to two parallel descriptions in openmls.md and other-implementations.md. **Lesson: when a relationship-between-multiple-projects fact lives in 3+ files and the relationship is non-obvious, designate one file as canonical, write the canonical sentence there, and have the other files cross-reference rather than restating.** This pattern was present in earlier folders but mattered most here because the wrong wording is a real factual error, not just a style inconsistency. (b) **Production-version-distinction matters when reframing scale claims.** First reviewer caught "Webex, Discord DAVE shipped on draft" as wrong: Webex shipped on draft, but DAVE shipped on RFC 9420 / MLS 1.0 — different deployment posture. The brief had elided this distinction; the agents inherited the elision; the first reviewer caught it and the polish pass split the claim. Lesson: when the corpus tells a "they shipped on draft" story about multiple deployments, verify each deployment's exact MLS version individually; don't assume early-mover deployments all shipped on the same draft. Other patterns: ETK 2025 paper (Cremers et al.) on FCGKA/EUF-CMA failure is the most important load-bearing critique — properly framed as "(per authors' summary; primary fetch failed)" rather than fabricated. ART vs TreeKEM lineage continues to require explicit disambiguation in every place either appears (8th-execution skill lesson on multi-attribution facts held).
- 2026-05-09 — Tenth execution on **Spin (Akamai, formerly Fermyon)** (request-driven serverless WASM CM runtime; 11 files, ~1,470 lines). Smaller-scope four-agent fan-out partitioned by architecture+triggers / SDKs+SpinKube / governance+comparisons / open-problems+critiques. **Major brief-correction caught during Stage 1 verification:** `fermyon.com` redirected to `akamai.com/products/akamai-functions` — Akamai acquired Fermyon on 2025-12-01 (~5 months before this session). Verified-facts pre-amble was updated *before* dispatching agents; all 4 agents inherited the correct Akamai-stewardship framing. **Two new patterns worth carrying forward:** (a) **Always check `<vendor>.com` redirect status before composing a brief** — a domain-level redirect to a different company is the strongest possible signal of an acquisition that the brief might miss. Took 1 WebFetch to catch; saved the entire fan-out from being stewardship-narrative-wrong. (b) **Pre-acquisition repo migrations are easy to misattribute as post-acquisition.** The `fermyon/spin` → `spinframework/spin` rename happened on 2025-01-21 — *the same day* both Spin and SpinKube were accepted to CNCF Sandbox — driven by CNCF vendor-neutrality requirements, ~10 months *before* the Akamai deal. One agent in fan-out misattributed the WIT-namespace rename to post-acquisition; first reviewer caught it. **Lesson: when a project has both a recent acquisition AND a recent CNCF onboarding, distinguish carefully — CNCF onboarding often drives renames that look acquisition-related.** Other patterns this round: agents corrected three brief errors in-place (Hyperlight Wasm Sandbox accepted 2025-03-04 not 2025-03-02; Wasmtime version on v4.0.0 tag is 43.0.1 not 41.0.4/42.0.1 which were dev-cycle versions; Spin 1.0 published 2023-03-21 not 2023-03-29). The version-pin discipline established in lesson 5 (substrate folders) held: agents who fetched `Cargo.toml` directly were right; agents who summarized release notes prose were wrong. Encoding this: **for any "this released-tag pins X version" claim, fetch the tag's Cargo.toml directly via `gh api` rather than reading release notes prose.** First reviewer caught all errors; polish-pass grep-sweep cleared all leftovers; second reviewer returned `ship-now` first try.
- 2026-05-09 — Eleventh execution on **Croquet / Multisynq** (lockstep deterministic-VM lineage: academic 2003 → Croquet Corporation 2018 → Multisynq Network 2024 rebrand; 11 files, ~1,350 lines). Smaller-scope four-agent fan-out partitioned by architecture+determinism / programming-model+platform / governance+comparisons / open-problems+critiques. **Three brief errors caught and corrected by agents in-flight:** (a) Brief said `@croquet/croquet@2.0.4` was proprietary "SEE LICENSE.md"; programming-model agent verified npm registry shows `Apache-2.0` (relicensed in 2.0.4 republish 2025-06-09 as part of the Multisynq open-source rebrand). Architecture agent kept the wrong claim — first reviewer caught the inconsistency. **Lesson: when two agents disagree on a license claim, the npm/crates.io registry is the canonical authority — verify the exact version's metadata, not just the latest README/LICENSE file in the repo.** (b) Brief implied Croquet Corp $2.7M seed was 2018 (founding year); reviewer verified the seed actually closed Feb 2020 (~21 months after founding). **Lesson: distinguish founding year from seed-funding year carefully — these often diverge by 1-2 years.** (c) Brief omitted Vanessa Freudenberg as Croquet Corp co-founder + Chief Architect; she's a critical bus-factor signal (died 2025-10-22, HN obit 45672484); critiques agent had her in the corpus but governance agent didn't have her in the People list. **Lesson: when the critiques and governance files describe overlapping people/events, ensure the canonical biographical data lives in governance.md and is referenced from critiques.md, not the reverse.** **One more pattern worth carrying forward:** for any project that has had a major rebrand (Croquet → Multisynq), distinguish the rebrand date (2024) from auxiliary dates like network deprecation (2025-07-30). A pre-existing wart in critiques.md called 2025-07-30 the "rebrand date"; a polish-pass fix corrected it. **Lesson: rebrands often come with a long tail of "deprecated old name" deadlines; treat the announcement date and the deprecation dates as separate facts to verify.** First reviewer caught all major errors; polish-pass grep-sweep cleared all leftovers; second reviewer returned `ship-now` with no new issues.
- 2026-05-09 — Twelfth execution on **Willow** (internal architectural ancestor of Myrhiza, not external — single-author Rust+Leptos+iroh chat product + draft master-runtime spec PR #636 that is essentially the Myrhiza proto-spec; 16 files, ~3,880 lines). **Three internal-ancestor patterns worth carrying forward:** (a) **Skip Stages 1+2 entirely** when target is already named, adjacency is obvious (it's our own code), and materials are already organized (working tree + spec corpus + a master-spec PR). Combine Stages 3+4 into a single parallel-writer fan-out where each agent writes its cluster of files directly; saves ~2 turns of orchestration. (b) **Verify spec claims against shipped code before trusting them.** PR #636 (the user's own master spec) contained a factual error that propagated through every agent: it called "postcard with sorted collections the existing-codebase precedent." Shipped Willow actually uses bincode. Three must-fix items in Stage 5 traced to agents inheriting this wrong claim from the spec they were summarizing. **For internal ancestors, the master-spec is not ground truth — shipped code is.** Add a "verify spec claims against `git grep` / Cargo.toml before lifting" line to every agent brief. (c) **Internal-ancestor framing disclosure needs to be double:** bias toward our choices because we made them, AND bias against our gaps because we lived with them. Where this corpus and another `prior-art/` folder disagree, the other folder is more likely neutral. The user-mid-session also requested **GitHub URL inclusion** in case local files unavailable — captured in pre-amble + every file's `## Repo` block. Encode "if subject project has a remote, include the URL" as a default in the pre-amble template. Polish-pass grep-sweep (mandatory per Agoric lesson) caught remaining residues; second reviewer caught one straggler (`glossary.md` state-digest parenthetical) before commit; total 3 review cycles.

## Sibling skill

`using-prior-art` is the consume-side flow. When this skill spawns to fill a gap, the gap was likely surfaced by `using-prior-art` during a spec, plan, or review. Both skills exist; consult the other when working on the corpus.
