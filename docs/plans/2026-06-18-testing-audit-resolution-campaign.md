# Testing-audit resolution campaign — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (fresh subagent per task + two-stage review) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Mark each box as it lands and commit per task.

**Date:** 2026-06-18
**Status:** active
**Spec:** [`docs/reports/2026-06-18-testing-infrastructure-coverage-audit.md`](../reports/2026-06-18-testing-infrastructure-coverage-audit.md) — the audit IS the spec for this campaign; it carries the full evidence (file:line), severity, and per-finding recommendation. **Every task below cites a finding number `#N`; read that finding in the report before implementing.**

**Goal:** Resolve all 43 findings from the 2026-06-18 testing-infrastructure & coverage audit — close the coverage gaps, fix the harness/CI/determinism defects, and stand up the five wholly-absent testing dimensions — leaving both gates green and the report marked resolved.

**Architecture:** Dependency-ordered phases. Phase 0 fixes reproducibility + CI so every later verification is trustworthy. Phase 1 lays the shared test-helper foundation so new tests don't re-drift. Phases 2–4 add the coverage/quality/harness work (largely parallelizable headless tests). Phase 5 does the big structural binary-consolidation last so it absorbs every new test at once. Phase 6 closes out. Four **decision gates** need a call before their phase runs.

**Tech stack:** Rust 2024, Bevy 0.18, Taffy 0.10, cosmic-text/harfrust, wgpu (lavapipe-pinned CI GPU lane), insta (snapshots), proptest, `buiy_verify` 5-tier harness. CI: GitHub Actions `ci.yml` (lint/doc/deny/test/gpu).

---

## Execution model (read first)

- **Granularity:** This is a **task-spec** plan, not pre-written code. The audit report holds exact file:line evidence + a recommendation for each finding. Pre-writing 43 unverified code blocks would encode guesses against code none of us has compiled. So each task gives **exact files, the test/change intent, the verification command, and acceptance criteria**; the executor **writes the failing test FIRST (TDD)** by reading the cited source, watches it fail, implements minimally, watches it pass, commits.
- **TDD per task:** RED (new test fails for the right reason) → GREEN (minimal impl) → commit. For pure coverage gaps the "implementation" is often nothing — the test passes once written because the behavior already exists; the value is the regression guard. For *dead-branch* findings (#4, #5, #20) the test may surface a real bug — if so, root-cause per CLAUDE.md, don't paper over.
- **Verification:** per-task headless: `cargo test -p <crate> --test <file> <name>`. GPU tasks: `cargo test -p <crate> -j 2 -- --ignored --test-threads=1`. **Phase gate:** full headless gate `cargo test --workspace -j 2` (no xvfb on this dev box; `DISPLAY=:0` present) **must stay green** before the next phase. Mechanical rigor before each commit: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`.
- **Note (env):** `/usr/bin/time` and `xvfb-run` are NOT installed on this Manjaro dev box; a display exists (`DISPLAY=:0`), so run `cargo test` directly. A cold full headless run is **~63 min** at `-j 2` (the #9 tax) — use per-test/per-crate runs during the loop, full gate only at phase boundaries.
- **Worktree/branch:** campaign runs on `worktree-testing-audit-report` (the report + this plan are its first commit). Findings touch many subsystems but rarely the same file; subagents can fan out within a phase. Use `isolation: worktree` only if two tasks mutate the same file concurrently.
- **Review gates:** after each phase, dispatch a fresh-context review agent (correctness + did-the-test-actually-bind, anti-vacuity) before advancing. Don't carry unreviewed work forward.

## Decision gates — RESOLVED 2026-06-18 (user)

All four resolved to the recommended option; recorded here so they are durable and not re-litigated after compaction.

- **DG-1 (#16, T0.4): MSRV — RESOLVED: pin a real MSRV + job.** Confirm Bevy 0.18's actual MSRV first; set `Cargo.toml rust-version` to that real floor; add a CI job `dtolnay/rust-toolchain@<floor>` → `cargo check --workspace --locked` enforcing it. (Not "keep 1.85" — pin to the *true* floor.)
- **DG-2 (#9, Phase 5): consolidation — RESOLVED: group binaries + nextest (both).** Group the ~162 per-file tests into <10 subsystem binaries (`layout`/`render`/`text`/`text_edit`/`crosscut` in buiy_core; `verify_headless`/`verify_gpu` in buiy_verify) via `mod`/`#[path]` includes, AND adopt `cargo-nextest` in CI (isolation + flaky-retry). Keep `cargo test --doc` separate (nextest skips doctests).
- **DG-3 (#40, T4.2): perf posture — RESOLVED: criterion bench, informational.** Add a `criterion` bench on shape→layout→extract for a large scene; wire it non-gating (signal, not a hard CI fail). Also fix the misleading `text_*_latency.rs` intent comments.
- **DG-4 (#41, T0.8): coverage tooling — RESOLVED: llvm-cov informational.** Wire `cargo llvm-cov` as a CI artifact/report, no pass/fail gate yet. (A floor on the pure-logic modules can come later once a baseline number exists.)

---

## Phase 0 — Reproducibility & CI foundation

Fix the things that make every other verification untrustworthy. All CI-file edits; verify by re-reading `ci.yml` + a local `cargo test --locked` smoke.

- [x] **T0.1 — Commit `Cargo.lock`; add `--locked`** (#1, P0). Remove `.gitignore:10` (`Cargo.lock`); `git add Cargo.lock`; add `--locked` to the test/deny/doc cargo invocations in `ci.yml`. Verify: `cargo test --workspace --locked -j 2` builds; `git ls-files Cargo.lock` non-empty. Commit.
- [x] **T0.2 — Facade GPU test into the GPU lane** (#8, P0). In `ci.yml` gpu job add `cargo test -p buiy -j 2 -- --ignored --test-threads=1` (after the buiy_verify line). Verify locally: that command runs `facade_render_finish_registers_device_resources` (`crates/buiy/tests/plugin.rs:50`) on the real adapter. Commit.
- [x] **T0.3 — CI `timeout-minutes` + `concurrency`** (#43, P1). Add `timeout-minutes:` to every job (generous on `gpu`/`test`, e.g. 60/90; tight on lint/doc/deny). Add a workflow-level `concurrency: { group: ${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: true }`. Verify: YAML parses; re-read. Commit.
- [x] **T0.4 — MSRV** (#16, P1) — **needs DG-1.** _Landed: real floor = **1.89.0** (bevy/bevy_ecs); 1.85 could not build._ Per the decision: add a `msrv` job (`dtolnay/rust-toolchain@<floor>` → `cargo check --workspace --locked`) or edit/remove `Cargo.toml:35 rust-version`. Verify accordingly. Commit.
- [x] **T0.5 — Weekly schedule + mirror Mesa tarball** (#15, #30, P1). _Landed: sha256-pinned the lavapipe tarball in install-mesa (no binary committed — release assets are mutable, so the digest is the guard)._ Add `schedule: - cron: '0 6 * * 1'` to `ci.yml on:`; gate the heavy lanes appropriately so PRs aren't slowed. Mirror the pinned Mesa tarball referenced by `.github/actions/install-mesa` into a repo-controlled asset (or document the upstream pin's durability). Verify: action still installs lavapipe; adapter-assert passes. Commit.
- [x] **T0.6 — Snapshot hygiene gate** (#17, P1→low). _Reshaped per review: mismatch is already gated by insta's CI default; this adds `*.snap.new` gitignore + a force-committed-pending guard. Unreferenced-stale gate → **T5.2b**._ Add `*.snap.new` to `.gitignore`; add a CI step `cargo insta test --workspace --unreferenced=reject` (install `cargo-insta` pinned), OR a post-test `git diff --exit-code` over `**/*.snap`. Verify: current 65 snaps pass (all referenced). Commit.
- [x] **T0.7 — Unify debuginfo + cache shared-key** (#10, medium). _Landed: `[profile.dev]`/`[profile.test] debug=0`; `shared-key: buiy` on lint/doc/test/gpu/msrv; gpu per-job RUSTFLAGS dropped; local-DX tradeoff noted in CLAUDE.md._ Move debuginfo-off to `[profile.test] debug = 0` in root `Cargo.toml`; drop the per-job `CARGO_PROFILE_*` (ci.yml:111-113) and the `-C debuginfo=0` from the gpu `RUSTFLAGS` (keep `-D warnings`). Add `shared-key: buiy` to all four `Swatinem/rust-cache@v2` blocks. Verify: both lanes still build; re-read ci.yml. Commit.
- [x] **T0.8 — Coverage tooling** (#41, low/med) — **needs DG-4.** Add `cargo llvm-cov` (informational job or artifact). Verify: produces a report headlessly. Commit.
- [ ] **Phase 0 gate:** `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --locked -j 2` green; fresh-agent review of the CI diff.

## Phase 1 — Test-code foundation

Land shared helpers so Phase 2–4 tests don't re-create the ~40 drifted stacks.

- [ ] **T1.1 — Shared headless App builders** (#35, medium). In `crates/buiy_core/tests/support/mod.rs` add `headless_layout_app()` (the 3-plugin transform-bridge stack), `bare_layout_app()` (the 2-plugin stack — self-documenting name for the weaker variant), `headless_text_app()` (incl. the `ThemePlugin` that `text_decoration.rs::text_app` silently adds), and one documented `settle(&mut App)` (resolve the 2-vs-3-frame split; prefer a condition-poll per `superpowers:condition-based-waiting` over a magic frame count). Do NOT yet migrate all call-sites (that churns mid-campaign) — just add + document, and use them in all NEW Phase 2–4 tests. Verify: builders compile; one existing test migrated as proof. Commit.
- [x] **T1.2 — Fix `follow-ups.md` doc drift** (#29, medium). _Landed: reframed `matrix_goldens` entry — skip-as-pending shipped (`coverage_golden.rs:104-114`, lane green not RED); fixed stale `check.rs:261`→`golden/check.rs:282`; remaining = bless residue cells._ Update the `matrix_goldens` entry (`follow-ups.md:832-850`): record skip-as-pending landed (`coverage_golden.rs:104-109`), re-frame remaining work as "bless real residue cells," cross-check the golden-prune-bin + button-RED claims. Verify: entry matches code. Commit.
- [ ] **Phase 1 gate:** builders used by ≥1 test; headless gate green; review.

## Phase 2 — Coverage: behavioral gaps (headless)

The bulk. Largely independent → fan out subagents. Each is TDD; for #4/#5/#20 a failing test may reveal a real bug — root-cause it. Use the Phase 1 builders.

**Layout**
- [ ] **T2.1 — Flex justify/align T1 distribution** (#2, P0). New headless layout-number test (`crates/buiy_core/tests/layout_*.rs` or a `buiy_verify` snapshot self-test): three 40px boxes in a 200px row; assert `ResolvedLayout` x = 0,80,160 for `SpaceBetween`; add `SpaceAround`/`SpaceEvenly` and `AlignItems::Center`/`FlexEnd` cases. Keep the GPU reftest (`reftest_cases_gpu.rs:137`) as the rasterized cross-check. Verify: new test green headless.
- [ ] **T2.2 — Grid track-size functions** (#22, medium). T1 grid tests for `minmax(100px,1fr)`, `fit-content`, `repeat(auto-fit,...)`; at minimum extend `translate.rs` units to assert inner `Min/MaxTrackSizingFunction`. `layout_grid.rs` / grid translate.
- [ ] **T2.3 — subgrid/masonry warns are testable** (#24, medium). Route subgrid/masonry fallback warns through `LayoutWarnOnceKey`/`LayoutWarnedOnceSession` (mirror `layout_table.rs:433`, `layout_multicol.rs:148`) instead of the process-global `AtomicBool`; replace the vacuous `layout_grid_stubs.rs:20-40` bodies with asserts on the key + the Auto/Row fallback geometry.
- [ ] **T2.4 — Degenerate sizes through the pipeline** (#32, medium). T1 test feeding `Length` of 0 / negative / NaN / infinity through `sync_styles→Taffy→write_resolved_layout`; assert finite, non-negative `ResolvedLayout` and no panic.
- [ ] **T2.5 — Anchor entity path + pure seam** (#37, low). Unit tests against `try_anchored_position`/`try_conditions_pass` (`systems.rs:1494`); one `AnchorRef::Entity(e)` integration fixture (all 11 existing use `AnchorRef::Name`).
- [ ] **T2.6 — Intrinsic-auto fallthrough** (#31, low). Optional: assert MinContent/MaxContent/FitContent box arms collapse to `auto()` (`translate.rs:634-639`) — documents the Phase-10 deferral. Low value; skip if time-boxed.

**Render**
- [ ] **T2.7 — WGSL parse for the other two shaders** (#3, P0). Extend `crates/buiy_core/tests/render_shader_wgsl.rs` with `include_str!` consts + naga-parse + entry-point assertions for `coverage.wgsl` and `composite.wgsl` (currently only `shader.wgsl`+`shadow.wgsl` at lines 17-18).
- [ ] **T2.8 — Shadow `erf`/Gaussian CPU oracle** (#27, medium, latent). Add a Rust `erf`+blurred-box oracle pinning `shadow.wgsl:66-81`'s closed form at canonical points (GPU-independent). Note: draw path is unwired (`extract.rs:348` TODO) so this is a forward guard.
- [ ] **T2.9 — Decoration geometry push-down** (#28, medium, tier). Move the band count/pos/thickness/gap asserts from `text_decoration_gpu.rs:208-276` to extract-harness/display-list snapshots off `ExtractedTextQuads`; keep one GPU golden per kind for AA residue.

**Text-render**
- [ ] **T2.10 — `script_fallback` net** (#25, medium). Headless test: script text (Arabic/Hebrew/Devanagari/Han) with a stack that does NOT name the covering font; assert glyphs land on the Noto face (`font_system.rs:76-87`). One fixture per arm; fonts already on disk.
- [ ] **T2.11 — `SwashContent::Color` decision** (#26, medium). Factor the `SwashContent` match (`extract.rs:854-885`) into a pure `content→ResolveAction` helper; unit-test Mask/Color/SubpixelMask + the zero-area guard headless.

**Text-edit**
- [ ] **T2.12 — Redo through the keyboard system** (#11, P1/high). System test mirroring `text_undo_system.rs:108`: type → undo → Ctrl-Shift-Z (and Ctrl-Y on non-macOS); assert value restored + exactly one `EditRedone`. Covers `input.rs:434/436/586`.
- [ ] **T2.13 — Grapheme-correct delete** (#18, medium). Headless `text_editing_ops.rs` test via `shaped_editor`: insert emoji-ZWJ family + base+combining-mark; Backspace once; assert the whole cluster removed in one step (spec `editing-and-ime.md:598`). Emoji font on disk.
- [ ] **T2.14 — Undo proptest selection round-trip** (#19, medium). Add a `Motion(_, true)`/`SelectAll` op to `text_undo_property.rs:17-25` `ScriptOp`; assert selection round-trip (exercises `restore_cursor`'s non-collapsed branch `input.rs:343-347`). Or a focused select-replace-undo test.
- [ ] **T2.15 — Remove `UndoStack` test-only seam** (#23, medium). Delete `undo.rs:201 pub fn push_redo_for_test`; rewrite its caller (`text_clipboard_undo.rs:59`) through the real `record(); pop_undo()`. Make `undo`/`redo` (`undo.rs:66-67`) `pub(crate)` + a narrow `open_unit()`/`last_recorded()` read accessor; fix the direct-index test reads.

**Cross-cutting**
- [ ] **T2.16 — Picking z-order / top-most** (#4, P0). `hit_test` test (`picking/mod.rs:42`) with two overlapping AABBs → smaller-area wins; `picking_backend.rs` test asserting `picks[0]` is the smaller node with ascending `HitData` depths. Pure/headless. If a comparator bug surfaces, root-cause.
- [ ] **T2.17 — Focus `tab_order` branches** (#5, P0). Make `compute_next_focus` (`focus.rs:85`) `pub(crate)`; unit-test `tab_order=-1` (skipped via the `>= 0` filter line 92) and mixed positives (1,2 before Auto, ascending sort line 99). These branches are currently dead (set 0× in tests).
- [ ] **T2.18 — a11y `build_tree` branches** (#20, medium). Extend `a11y.rs:38`: spawn `A11yDescription` and assert it surfaces (`a11y/mod.rs:110`); spawn a non-a11y-only entity and assert it's skipped (line 103).
- [ ] **T2.19 — `Hovered` consumer chain** (#21, medium). Extend `picking_backend.rs`: `app.update()` then assert `Hovered.0 == Some(entity)` (`update_hovered` `picking/mod.rs:59`); with two overlapping nodes this also covers top-most.
- [ ] **T2.20 — System-set order membership** (#34, low). Add a delta-count membership assertion per plugin's set contribution (the `render_forced_colors_swap.rs` pattern) rather than only CorePlugin's empty sets.
- [ ] **Phase 2 gate:** full headless gate green; fresh-agent anti-vacuity review (do the new tests actually bind? would each fail under its target mutation?). Spot fault-inject 2–3.

## Phase 3 — Harness integrity, goldens, determinism

- [ ] **T3.1 — Tier-3 invariant calls production paint assembly** (#6, P0). Extract paint sub-pass 6f to a pure `buiy_core::pub fn painters_z_for_context(...)` (re-export `paint_key`); have `invariant/scene.rs:374-388 realize` call it instead of its local re-implementation (`scene.rs:636-643`). Verify with fault injection: reversing production 6f now reddens the Tier-3 invariant (per the 2026-06-15 report this previously did NOT). `follow-ups.md` open.
- [ ] **T3.2 — PositionKind generator axis** (#13, medium). Add a `PositionKind` axis to `SceneNode`/`arb_leaf` so the tier-2 (positioned, auto-z) class generates; once T3.1 lands, the generator just supplies `PositionKind` to the production `paint_key`. Land with T3.1.
- [ ] **T3.3 — `golden_sdf_corner` adapter gating** (#7, P0). Gate the committed-corpus comparison (`goldens.rs:288-303`) on the selected adapter being the pinned lavapipe (probe `RenderAdapterInfo`/`WGPU_ADAPTER_NAME`; **skip-as-pending**, not fail, otherwise — mirror `matrix_goldens`). OR split into a rasterizer-internal self-check (bless-to-temp + EXACT re-capture, sound on any adapter) + a CI-lavapipe-only baseline leg. Verify: the local `-p buiy_verify --ignored` lane no longer hard-fails on the RX 6700 XT.
- [ ] **T3.4 — Tier-5 golden non-vacuity** (#14, high). Make an all-pending `matrix_goldens` run report `ignored`/skipped rather than green (`coverage_golden.rs:149`), so green ⟹ ≥1 cell compared. Document in `goldens.md` that 5/6 residue classes are currently aspirational. Bless a residue golden per class only as the renderer paths land (most are renderer-blocked per follow-ups).
- [ ] **T3.5 — Condition-4 quiescence headless** (#12, low). Extract condition 4 (PipelineCache `Queued/Creating`, `golden.rs:398-408`) behind `fn no_pipeline_compiling(&PipelineCache)`; unit-test against synthetic state. (Conditions 2–3 already headless-covered per verification.)
- [ ] **T3.6 — DPR axis collapse + magenta sentinel** (confirmed cleanups from "Investigated & dismissed"). Drive dpr/breakpoint only at the GPU golden tier (24 of 48 button CPU snapshots are exact dpr1==dpr2 duplicates); assert dpr-sibling identity once as a property. Stop baselining the missing-token sentinel `#ff00ffff` in the 12 light-theme button display-list snapshots — paint a both-theme-resolving token or skip light cells for system-color-only fixtures, or assert the baseline is NOT the sentinel.
- [ ] **Phase 3 gate:** headless gate green; GPU lane green on the real adapter (`-p buiy_core` + `-p buiy_verify` + `-p buiy` `--ignored --test-threads=1`); fault-inject the 6f reversal to confirm T3.1.

## Phase 4 — Quality, docs, examples, the absent dimensions

- [ ] **T4.1 — Runnable doctests** (#39, medium). Audit the 5 `ignore` doc fences (`buiy/src/lib.rs:62`, `buiy_core/layout/style.rs:33`, `layout/components.rs:556`, `buiy_verify/coverage/fixture.rs:72`, `reftest.rs:463`). Drop `ignore` (use `no_run` where a window/GPU is needed) so `cargo test --doc` compile-checks them — at minimum the facade `BuiyPlugin` onboarding snippet. Verify: `cargo test --doc --workspace` now runs >0 tests.
- [ ] **T4.2 — Performance posture** (#40, medium) — **needs DG-3.** Per the decision: a criterion bench on shape→layout→extract for a large scene and/or an allocation-count assertion on the per-frame hot path; wire informational. If deferred, record "no performance gate exists" as an explicit, severitied known-gap in the report + follow-ups. Rename `text_*_latency.rs` intent comments (they measure frame-count semantics, not wall-clock).
- [ ] **T4.3 — Example e2e** (#42, medium). Add `tests/hello_text_e2e.rs` mirroring `hello_button_e2e.rs` (drive the scene through `app.update()`, assert no panic + glyphs publish). Add a GPU-lane e2e for the `capture` example's render-to-texture+readback path (the only end-to-end exercise of the screenshot pipeline). Verify per-example headless vs GPU.
- [ ] **T4.4 — Transform-bridge extract** (#33, low). Extract `fn compose_buiy_transform(position, scroll, matrix) -> Transform` from the inline `bridge.rs:117` walk; unit-test directly (App tests re-implement it at `render_transform_bridge.rs:96`).
- [ ] **T4.5 — Forced-colors rename** (#36, low). Rename `forced_theme_canvas_and_canvastext_contrast` (`theme_forced_colors.rs:37`) to `..._differ` (it's an `assert_ne!`, not a ratio); defer the real WCAG ratio gate until palette values land.
- [ ] **T4.6 — Low/info cleanup batch** (#38). Work the list as encountered: shaping-corpus breadth (pure-Hebrew/VS16/Thai/Khmer fixtures), drag-selection + word-nav motion tests, AABB inclusive-edge boundary, `Instant`-grep-lint scope beyond `golden.rs`, weak Tier-3 enrollment predicates + axis-distinctness self-test, the missing golden-prune bin, the blanket `#![allow(dead_code)]` masking dead `pub fixture_font_bytes`, the empty `boxshadow_visual_reftest_is_blocked` marker. Each its own small commit; skip any that prove low-value on inspection (note the skip).
- [ ] **Phase 4 gate:** headless + doctest green; review.

## Phase 5 — Build consolidation (big; last) — **needs DG-2**

- [ ] **T5.1 — Consolidate test binaries** (#9, P0). Group the ~162 per-file integration tests into <10 binaries by subsystem (e.g. `layout`, `render`, `text`, `text_edit`, `crosscut` in `buiy_core/tests/`; `verify_headless`, `verify_gpu` in `buiy_verify/tests/`), each a thin binary that `mod`s the current files (rename current `*.rs` → submodules under a `tests/<group>/` dir or `#[path]`-include them). Eliminates the ~28× `support/mod.rs` recompile and the linear-in-binary-count link cost. Keep `#[ignore]`/`_gpu` separation intact.
- [ ] **T5.2 — Adopt `cargo-nextest`** (per DG-2). Switch CI test steps to `cargo nextest run` (with a `--partition`/retry config for the GPU lane); keep `cargo test --doc` separately (nextest doesn't run doctests). Verify: cold full-run wall-clock drops materially from the ~63 min baseline; both lanes green.
- [ ] **T5.2b — Unreferenced-snapshot gate** (#17 carryover from Phase 0). Fold `cargo insta test --workspace --unreferenced=reject` (pinned `cargo-insta`) into the consolidated runner so a single suite run also rejects orphaned/stale committed `.snap` files. Phase 0 (T0.6) deferred this here because it needs a full-suite run via the cargo-insta runner — free to add once the runner is reworked. (Phase 0 already: gitignores `*.snap.new`, guards against force-committed pending snapshots, and relies on insta's CI default to fail the test step on a MISMATCH.) Verify: deleting a `.snap`'s referencing test, or orphaning a `.snap`, makes the runner step fail.
- [ ] **Phase 5 gate:** full headless gate green AND meaningfully faster; GPU lane green; review the consolidation didn't drop/rename any test (compare test counts before/after: baseline 1154 passed + the new tests).

## Phase 6 — Campaign closeout

- [ ] **T6.1 — Full both-lane verification.** Headless `cargo test --workspace --locked` green; GPU `--ignored --test-threads=1` across `buiy_core`/`buiy_verify`/`buiy` green on the real adapter; `fmt`/`clippy`/`doc`/`deny` clean.
- [ ] **T6.2 — Docs flip.** Add a closeout note + `Status: resolved` pointer to the audit report (don't rewrite findings — append "Resolution: campaign 2026-06-18-testing-audit-resolution landed, see plan"). Update this plan's status to `landed` and check every box. Update `docs/plans/follow-ups.md` (remove items now closed; keep genuinely-deferred ones e.g. renderer-blocked residue goldens). Update `docs/README.md` entries.
- [ ] **T6.3 — Final review + integration.** Fresh-agent review of the whole campaign diff against this plan + the report; then use `superpowers:finishing-a-development-branch` to merge/PR.

---

## Self-review (coverage check vs the report's 43 findings)

P0 (1–10): #1→T0.1, #2→T2.1, #3→T2.7, #4→T2.16, #5→T2.17, #6→T3.1, #7→T3.3, #8→T0.2, #9→T5.1, #10→T0.7. ✔
P1 (11–30): #11→T2.12, #12→T3.5, #13→T3.2, #14→T3.4, #15→T0.5, #16→T0.4, #17→T0.6, #18→T2.13, #19→T2.14, #20→T2.18, #21→T2.19, #22→T2.2, #23→T2.15, #24→T2.3, #25→T2.10, #26→T2.11, #27→T2.8, #28→T2.9, #29→T1.2, #30→T0.5/T6. ✔
P2 (31–38): #31→T2.6, #32→T2.4, #33→T4.4, #34→T2.20, #35→T1.1, #36→T4.5, #37→T2.5, #38→T4.6. ✔
Completeness additions (39–43): #39→T4.1, #40→T4.2, #41→T0.8, #42→T4.3, #43→T0.3. ✔
Confirmed-dismissed cleanups (DPR axis, magenta sentinel)→T3.6. ✔
All 43 + 2 cleanups mapped.

## Current state / next action (post-compaction pickup)

- **Done:** audit report + this plan (baseline `3397414`); all 4 decision gates resolved. **Phase 0 LANDED** (`adfd2ee`) via the research→implement→review Workflow (5 adversarial review clusters, all pass after fixes): T0.1–T0.8 done; `Cargo.lock` tracked; MSRV pinned to the real floor **1.89.0**. Gated on `fmt` + `clippy --all-targets --locked` (clean); full headless `cargo test --workspace --locked` run as the final Phase-0 gate (in-flight at handoff — confirm green before checking the gate box).
- **Next action:** **Phase 1** (test-code foundation) via Workflow. T1.1 (shared headless App builders in `crates/buiy_core/tests/support/mod.rs`) + T1.2 (`follow-ups.md` matrix_goldens drift). Phase-1 research (App-builder surface map) was dispatched in parallel with the Phase-0 gate.
- **Decisions:** all four RESOLVED (no further user input required to execute; surface only if a phase reveals a resolved decision was wrong).
- **Pointers:** evidence = the audit report (the Spec above). Cross-session memory = `buiy-testing-audit-campaign`.
