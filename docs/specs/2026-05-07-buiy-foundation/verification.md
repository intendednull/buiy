# Feature inventory — verification pipeline

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

## 3.15 Verification pipeline

The verification subsystem realizes goal #7 ([README.md § 1](README.md#buiys-goals-the-product)). The inventory below enumerates the floor; detailed strategy (tolerances, baselines, failure thresholds, flake-mitigation, runner choice) lives in `buiy-verification-design`.

The pipeline has two tiers: **CI gates** (every PR; failure blocks merge; no human approval) and **manual release gates** (every release; explicit owner; documented cadence). Goal #7 covers only the CI tier.

### CI gates

| # | Category | Tier | What it verifies | Notes / risk |
|---|---|---|---|---|
| 1 | Unit tests | F | Component logic, layout calc, event handlers, state machines. | Standard `cargo test`. |
| 2 | Visual regression | F | Rendered output matches golden per widget × state × theme × viewport. | **Per-platform goldens** on a single canonical CI GPU class; perceptual diff with explicit tolerance budget; flake-mitigation via fixed clock + font-load sync + atlas warmup. Golden updates require a human-curated `--accept` workflow (intentional changes); CI policy "no approval gate" applies to *test outcomes*, not to golden updates. |
| 3 | AccessKit tree snapshots | F | Tree shape, role, name, description, states, relationships per widget × state. JSON diff. | `accesskit_consumer`-driven assertions. |
| 4 | Announcement-output snapshots | C | The string a live region produces; the order of name + role + state utterance for focus changes. Asserted via `accesskit_consumer`'s consumer view. | Independent of tree snapshot — verifies what an AT *would* announce, not the AccessKit tree shape itself. Does not run real NVDA/VoiceOver. |
| 5 | Layout snapshots | F | Resolved Taffy output per layout fixture (positions, sizes). | |
| 6 | Synthesized input replay | F | Keyboard, pointer, touch, gamepad events injected as Bevy events; assert resulting state. | IME composition is verified at the Buiy↔winit boundary only; full OS-IME conformance (IBus, fcitx, TSF, macOS IM) is in the manual release gate. |
| 7 | APG keyboard-contract conformance | F | Every APG pattern, every documented key, every state transition. Forms-mode and browse-mode contracts both exercised via `accesskit_consumer`. | Verifies key-to-state mapping; the *AT utterance* on each transition is in #4. |
| 8 | WCAG 2.2 SC suite (machine-testable) | F | Each Level A / AA SC marked **CI** in the [accessibility.md § 3.11 table](accessibility.md). | SCs marked **DC** / **LR** are explicitly NOT CI gates. Content-quality SCs (1.1.1 alt-text quality, 1.4.5, 2.4.4, 2.4.6, 3.3.2) are linter-with-review or design-constraint, not CI. |
| 9 | Contrast linter | F | Every theme × every token combination. WCAG 2 (4.5:1 / 3:1 / 3:1) is the gate; APCA (Lc thresholds: ~Lc 60 minimum, Lc 75 body, Lc 90 preferred) is advisory. | Both algorithms ship; WCAG 2 is the legal-bar gate. APCA upgrade path documented in `buiy-verification-design`. |
| 10 | Hit-target linter | F | Every interactive widget rendered with hit area ≥24×24 at every viewport in the fixture set. | Geometric check on the picking hit-rect at layout time. |
| 11 | Forced-colors compatibility scan | F | Two checks: (a) no widget paints a color outside the system-color token set when `forced-colors: active`; (b) no shadow-only affordance — every focusable / state-bearing widget has a non-shadow visual cue (border, fill, outline). | Token-flow analyzer + golden visual diff under forced-colors. |
| 12 | Property tests / fuzzing | F | Generators for hierarchies (max-depth N, max-breadth M, shrink-to-minimal-failing-tree), input streams, theme-variant matrices. Invariants: "focus tree reachable from any starting node," "AccessKit tree has no orphans," "every focusable node has an accessible name," "BiDi caret round-trip equals identity." | `proptest` with named strategies. |
| 13 | Hot-reload validation | F | Reload `.bsn` file or theme asset; assert live entity diff over stable IDs equals expected diff; no entity / atlas leaks. | Equality predicate is "stable-ID-keyed entity-state diff"; spec'd in `buiy-verification-design`. |
| 14 | Performance regression | F | Per-frame layout time + render time + AccessKit-update time relative to main-branch baseline on a fixed self-hosted runner. | The CI gate's *mechanism* (relative-to-main, fixed runner, ±10% default slack) is committed. The *actual budget numbers* per fixture are an open question ([README.md § 5](README.md#5-open-questions)) owned by `buiy-verification-design`. The gate exists at v1; the numbers calibrate over time. |
| 15 | Memory leak tests | F | RSS slope and atlas-entry count return to baseline after a defined long-running fixture (~10 minutes of scripted activity, then idle). | Threshold: RSS slope < 1 MB / minute after warmup; atlas entries return within ε of baseline. |

### Gate realization status (verification-design)

The *mechanisms* for gates **#2, #5, #11, #12** are realized by the
[`buiy-verification-design`](../2026-06-15-buiy-verification-design/README.md)
harness (the reftests-first five-tier pyramid in `buiy_verify`). This is a
realization note only — the gate definitions above are unchanged.

- **#5 — Layout snapshots:** **landed.** Tier 1 `buiy_verify::snapshot::assert_layout_snapshot` dumps `ResolvedLayout` positions/sizes as stable `Name`-keyed `insta` snapshots (`snapshots.md`). Pure-CPU, headless.
- **#12 — Property tests / fuzzing:** **landed** (the visual half). Tier 3 `buiy_verify::invariant` ships the six proptest predicates incl. "BiDi caret round-trip equals identity" over the live cosmic-text shaper, with mutation-fixture teeth (`invariants.md`). The a11y-tree invariants (focus reachability, no orphans, accessible-name) remain owned by the a11y subsystem, not this harness.
- **#11 — Forced-colors compatibility scan:** **landed** (check (a) token-flow + check (b) no-shadow-only) via the live-catalog `forced_colors_analyzer` wiring (`coverage.md` § Wiring). The forced-colors *visual* golden/reftest half (BoxShadow draw-skip) is **renderer-blocked** and deferred (`follow-ups.md`).
- **#2 — Visual regression:** **mechanism landed, residue deferred.** The metric (`metric.md`), `DeterministicApp` capture + lavapipe CI pin (`determinism.md`), reftests + CPU/GPU SDF cross-check (`reftests.md`), and `assert_golden` persistence + `BUIY_BLESS` accept workflow + HTML triage (`goldens.md`) are all built and gated; the golden corpus is *started* (two blessed cells). The full residue golden matrix (shadow blur kernel, color-emoji) is **renderer-blocked** and tracked in `follow-ups.md`.

### CI platform matrix

- **Desktop (Windows UIA, macOS NSAccessibility, Linux AT-SPI)** — full CI matrix for v1, all categories above.
- **Android** — deferred until `accesskit_android` exposes a headless harness; until then, manual release-gate platform.
- **iOS** — deferred until `accesskit_ios` ships and a CI strategy (Mac runner + simulator, or device farm) is selected; until then, manual release-gate platform.
- **Web** — deferred until AccessKit web adapter ships; until then, no a11y verification on web target.

Open question on platform staging: [README.md § 5](README.md#5-open-questions).

### CI policy

- Runs on every PR. Failure blocks merge.
- "No human approval gate" applies to **test outcomes**: green = mergeable. Golden-image and AccessKit-snapshot updates use a human-reviewed `--accept` workflow as part of standard PR review.
- Cross-platform matrix runs in parallel.

### Manual release gates (NOT CI gates; required at every release)

These gate releases, not PRs. Each has an owner, a documented cadence, and a release-blocking sign-off mechanism: each gate produces a checked-in sign-off document at `docs/release-notes/<version>/manual-gate-<gate>-signoff.md`. Tagging a release is gated on all four sign-off documents being present and approved on the release branch.

1. **Real-SR output sanity sweep** — run a curated fixture suite under NVDA + Firefox-equivalent host (Windows), VoiceOver (macOS), Orca (Linux GNOME), TalkBack (Android emulator). Verify utterances against expected-output strings. Owner: a11y maintainer. Cadence: every minor release. (May graduate to a CI gate if a headless real-SR harness becomes practical — open question.)
2. **Real-device mobile sweep** — Android + iOS on physical or simulated devices; verify TalkBack / VoiceOver behavior, IME composition with real OS IMEs (IBus, fcitx, macOS Japanese IM, Windows TSF), gesture recognizers under real touch. Owner: platform maintainer. Cadence: every minor release.
3. **Subjective visual review** — design lead reviews default theme(s), widget gallery, animation polish. Cadence: every minor release. Note: WCAG 1.4.3 / 1.4.11 contrast is *not* in this gate (it's CI #9); this gate covers polish and brand alignment.
4. **Content-quality SC review** — alt-text quality, link-purpose, label clarity in shipping examples and docs. WCAG 1.1.1 / 1.4.5 / 2.4.4 / 2.4.6 / 3.3.2. Owner: docs maintainer. Cadence: every minor release.

**Coverage tradeoff acknowledgment (cross-reference [README.md § 1](README.md#buiys-goals-the-product)):** these four gates collectively cover real-AT speech, real-device behavior, subjective polish, and content quality. Several user-experience claims sit at the manual tier rather than the CI tier. Goal #7 is honest about this — "every machine-testable claim" — but readers should understand that "Buiy's verification pipeline is fully automated" only refers to the CI tier, not to all things end-users experience.

### Multi-window verification

All CI gates run per-window where applicable. AccessKit tree snapshots, focus tree state, IME consumer state, and picking results are keyed by `WindowId`. Multi-window fixtures verify per-window stack ownership ([cross-cutting.md § 3.18](cross-cutting.md)).

### Hot-reload trigger flow

Asset hot-reload tests (gate #13) drive the verification harness through Bevy's standard `AssetEvent::Modified` for `BsnAsset`, `ThemeAsset`, and `FontAsset`. Buiy's reload systems observe these events and apply the diff; the harness then asserts the post-reload entity-state diff equals the expected diff. Asset graph and `AssetServer` integration is owned by `buiy-asset-pipeline-design`.

### Tooling

- `accesskit_consumer` — simulated AT consumer for tree-snapshot and announcement-snapshot testing.
- Bevy's screenshot system + a perceptual-diff crate — visual regression with tolerance budget.
- `proptest` — property-based testing.
- `buiy_verify` — Buiy's own harness crate; `dev-dependency` for every Buiy crate; usable by downstream Buiy users to test their own widgets.

### What is *not* a CI gate (and why)

- **Real-SR utterance verification.** `accesskit_consumer` simulates the consumer side; it does not run NVDA / JAWS / VoiceOver. Manual release gate (#1 above). Goal #7 explicitly excludes this from "machine-testable claims."
- **Full OS-IME conformance.** OS IME backends sit upstream of winit; we verify Buiy↔winit at the boundary, real OS IME at release time.
- **WCAG content-quality SCs** (1.1.1, 1.4.5, 2.4.4, 2.4.6, 3.3.2). Linter advises; humans confirm.
- **Subjective visual quality.** Manual release gate.
- **Real-device mobile / web a11y.** Pending platform staging ([README.md § 5](README.md#5-open-questions)).
