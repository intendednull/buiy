# Buiy verification harness — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development to implement task-by-task. Steps use checkbox (- [ ]) syntax.

**Date:** 2026-06-15
**Status:** active
**Spec:** specs/2026-06-15-buiy-verification-design/README.md
**Goal:** Build the five-tier, reftests-first visual-bug-detection pyramid (`buiy_verify`: metric, snapshots, invariants, reftests, goldens, determinism, coverage) on the landed `GoldenConfig` capture path, retiring the two naive metrics and closing foundation gates #2/#5/#11/#12.
**Architecture:** The harness is `buiy_verify` (pure-CPU metric, snapshot/invariant formatters, reftest pairing logic, golden persistence, `DeterministicApp`, coverage matrix) plus a device-coupled capture seam promoted into `buiy_core::render::golden` (`capture_to_image` / `capture_app`). `buiy_core` cannot depend on `buiy_verify` in its normal graph, so the L1 `perceptual_diff` is deprecated in place and a dev-only dependency cycle lets `#[ignore]` GPU tests reach the unified metric. Pure-CPU tiers gate headless; GPU tiers are `#[ignore]` and run on a real adapter (RX 6700 XT locally, pinned lavapipe in CI).
**Tech Stack:** Rust, Bevy 0.18, wgpu, `image` 0.25, `proptest`, `insta`, the vendored pixelmatch YIQ algorithm + `image-compare` MSSIM, `toml`/`base64` (golden ledger + triage), `inventory` (coverage catalog); GPU lane on a real adapter.

---

## File structure

Every file created (`C`) or modified (`M`) across the campaign, with its one-line responsibility. Paths are repo-relative to `/mnt/storage/projects/buiy/.claude/worktrees/visual-bug-detection-report/`.

### `buiy_core` (capture seam + deprecation)

| File | C/M | Responsibility |
|---|---|---|
| `crates/buiy_core/Cargo.toml` | M | Add `image.workspace = true` (direct dep for `capture_to_image`); add `buiy_verify` under `[dev-dependencies]` (dev-only cycle). |
| `crates/buiy_core/src/render/golden.rs` | M | Canonical `Dpr` milliscale type; promoted `capture_to_image` + `readback_rgba_into` + `capture_app`/`capture_app_scaled`; `CAPTURE_MSAA`/`CAPTURE_DITHER_OFF` consts; `GoldenConfig` extension (`FontMode`, `dpr`, `fidelity()`); quiescence flush; `#[deprecated]` `perceptual_diff`. |
| `crates/buiy_core/src/lib.rs` | M | `#[allow(deprecated)]` on the `perceptual_diff` re-export. |
| `crates/buiy_core/src/layout/systems.rs` | M | Extract the nested `tier_rank` fn (`:4113`) into `pub fn top_layer_paint_rank`; `compose_transform` is `pub(super)` at `:3775`. |
| `crates/buiy_core/src/layout/mod.rs` | M | Re-export `top_layer_paint_rank`. |
| `crates/buiy_core/tests/support/mod.rs` | M | Delegate `readback_rgba` / `gpu_render_app_with_resolution` to the promoted src builders (single-body anti-drift). |
| `crates/buiy_core/tests/render_golden_harness.rs` | M | `#[ignore]` dimension meta-test for `capture_to_image`; file-level `#![allow(deprecated)]`. |
| `crates/buiy_core/tests/render_capture_app_gpu.rs` | C | `#[ignore]` test: `capture_app` paints a non-blank frame. |
| `crates/buiy_core/tests/render_capture_quiescence.rs` | C | Quiescence-panic (`#[ignore]` GPU) + no-`Instant::now` grep-lint (headless). |
| `crates/buiy_core/tests/render_golden_config.rs` | C | `GoldenConfig::deterministic()`/`fidelity()` defaults. |
| `crates/buiy_core/tests/text_gpu.rs` | M | Migrate the 5 stable re-capture sites + 2 anti-tests onto `metric::compare`. |
| `crates/buiy_core/tests/{text_decoration_gpu,text_golden_suite_gpu,text_selection_caret_gpu}.rs` | M | File-level `#![allow(deprecated)]` (stay on `perceptual_diff` until Phase 3). |
| `crates/buiy_core/tests/{render_extract,render_buckets,render_paint_order,render_instance,top_layer}.rs` | M | Migrate per-field `assert_eq!` blocks to `assert_display_list_snapshot` / `assert_instance_hex_snapshot`. |
| `crates/buiy_core/tests/layout.rs` | M | Migrate the `< 0.5` flex-row asserts to `assert_layout_snapshot`. |
| `crates/buiy_core/tests/layout_stacking.rs` | M | `top_layer_paint_rank` mapping assert. |
| `crates/buiy_core/tests/fixtures/fonts/Ahem.ttf` (+ license) | C | Committed WPT Ahem box-font fixture. |
| `crates/buiy_core/tests/snapshots/` | C | Committed `.snap` files for the migrated core tests. |

### `buiy_verify` (the harness)

| File | C/M | Responsibility |
|---|---|---|
| `crates/buiy_verify/Cargo.toml` | M | Add `image-compare`, `insta`, `toml`, `base64`, `inventory`; remove nothing (pixelmatch is vendored, not depended on). |
| `crates/buiy_verify/src/lib.rs` | M | Register `metric`, `reftest`, `support`, `snapshot`, `invariant`, `golden`, `determinism`, `coverage`; drop `visual`. |
| `crates/buiy_verify/src/metric.rs` | C | AA-aware two-axis perceptual diff: `Diff`/`FuzzBudget`/`CompareOpts`, vendored YIQ `color_delta` + AA sibling exclusion, MSSIM, `passes`/`within`, `reftest_default()`, diff heatmap. |
| `crates/buiy_verify/src/visual.rs` | D (delete) | RMSE `compare_images` removed (superseded by `metric`). |
| `crates/buiy_verify/src/reftest.rs` | C | `RefKind`/`RefCase`/`RefOutcome`, `evaluate_outcome`, `mismatch_floor_ok`, `run_reftest`, `reftest!` macro, independence lint, `sdf_oracle`, `run_sdf_cross_check`. |
| `crates/buiy_verify/src/support.rs` | C | GPU-capture glue (`reftest_app`, `clear_reftest_scene`) — the one place Phase 3 swaps for `DeterministicApp`. |
| `crates/buiy_verify/src/snapshot/{mod,dump,layout,display_list}.rs` | C | Tier-1 layout dump + Tier-2 display-list/`PackedInstance`-hex dumps + shared `round`/version headers. |
| `crates/buiy_verify/src/invariant/{mod,scene,predicates,bidi}.rs` | C | Tier-3 proptest scene generators + predicate fns + BiDi caret round-trip. |
| `crates/buiy_verify/src/golden.rs` (+ `golden/report.rs`) | C | Tier-5 `GoldenKey`/`Backend`/`BlessLedger`, `check_golden`/`assert_golden`, multi-positive corpus, HTML triage. |
| `crates/buiy_verify/src/determinism/mod.rs` | C | `DeterministicApp` builder; re-exports `FontMode`/`Dpr` from `buiy_core::render::golden`. |
| `crates/buiy_verify/src/coverage/{mod,fixture,matrix,key,enroll,forced_colors}.rs` | C | `Fixture`/`fixture!`/`catalog`, `Matrix`/`Cell`/`CoverageKey`, `enroll_all`/`build_app`, live-catalog forced-colors producer. |
| `crates/buiy_verify/fixtures/<widget>/<state>.rs` | C | The single-source-of-truth BSN fixture corpus (`inventory`-registered, `glob!`-discoverable). |
| `crates/buiy_verify/tests/metric.rs` | C | Known-answer metric meta-suite + constants tripwire. |
| `crates/buiy_verify/tests/visual.rs` | M | Migrated off `compare_images` onto `metric::compare`. |
| `crates/buiy_verify/tests/smoke.rs` | M | Drop the `visual` re-export reference. |
| `crates/buiy_verify/tests/reftest_engine_gpu.rs` | C | `#[ignore]` known-good/known-bad engine pairs. |
| `crates/buiy_verify/tests/reftest_macro_gpu.rs` | C | `#[ignore]` macro-generated case. |
| `crates/buiy_verify/tests/reftest_independence.rs` | C | Headless RED/GREEN independence-lint self-test. |
| `crates/buiy_verify/tests/sdf_oracle.rs` | C | Headless full-tile CPU SDF oracle point-probes. |
| `crates/buiy_verify/tests/sdf_cross_check_gpu.rs` | C | `#[ignore]` GPU-vs-CPU SDF cross-check. |
| `crates/buiy_verify/tests/reftest_cases_gpu.rs` | C | Two real reftest cases (flex-justify `==`, content-visibility `!=`). |
| `crates/buiy_verify/tests/snapshot_*.rs` | C | Tier-1/2 dump self-tests (`_dump`, `_layout`, `_instance_hex`, `_display_list`, `_animation`). |
| `crates/buiy_verify/tests/invariant_mutations.rs` | C | Tier-3 mutation fixtures (the harness has teeth). |
| `crates/buiy_verify/tests/{golden_keys,golden_persistence,golden_report}.rs` | C | Tier-5 pure-CPU persistence/ledger/triage self-tests. |
| `crates/buiy_verify/tests/{determinism_ahem,determinism_capture}.rs` | C | Ahem-sole-family (headless) + `#[ignore]` idempotent/knob-sensitivity GPU. |
| `crates/buiy_verify/tests/goldens.rs` (+ `tests/goldens/` corpus) | C | `#[ignore]` end-to-end goldens per residue class + blessed PNGs. |
| `crates/buiy_verify/tests/coverage_{layout,display_list,invariants,golden,meta,forced_colors}.rs` | C | Per-tier enrollment drivers + coverage self-tests + live forced-colors scan. |
| `crates/buiy_verify/tests/snapshots/` + `proptest-regressions/` | C | Committed `.snap`s and minimized proptest counterexamples. |

### Repo-level

| File | C/M | Responsibility |
|---|---|---|
| `Cargo.toml` (workspace) | M | Add `toml`, `base64` to `[workspace.dependencies]` (and `insta` with `glob` if not already). |
| `deny.toml` | M | Add any new transitive SPDX id to the `[licenses] allow` list (never via an exceptions hack). |
| `.gitattributes` | M | Pin `crates/buiy_verify/tests/goldens/*.png -text`. |
| `.github/actions/install-mesa/action.yml` | C | CI lavapipe pin (consume `gfx-rs/ci-build` tarball; write own ICD JSON). |
| `.github/workflows/*` (CI) | M | Invoke the lavapipe action on the golden leg; export `VK_DRIVER_FILES`/`WGPU_ADAPTER_NAME`. |
| `docs/specs/2026-06-15-buiy-verification-design/*.md` | M | Flip `draft` → `active`/`implemented` with per-file "landed" notes (Phase 4.7). |
| `docs/README.md` | M | Flip the verification-design catalog tag `[draft]` → `[active]`; add this plan under Plans. |
| `docs/plans/follow-ups.md` | M | Resolve the live-catalog seam; keep `BoxShadow` visual reftest open; record deferred golden primitives. |
| `docs/specs/2026-05-07-buiy-foundation/verification.md` | M | Mark gates #2/#5/#11/#12 mechanisms landed. |

---

## Phasing & ordering

Five phases; **Phase 0** is the cross-cutting prerequisite, then the spec's reftests-first build order (metric+reftests → snapshots+invariants → goldens+determinism → coverage).

| Phase | Name | Depends on | Gate |
|---|---|---|---|
| **0** | Cross-cutting prerequisites (deps, dev-cycle edge, `Dpr`, `capture_to_image`) | — | Headless; one `#[ignore]` GPU meta-test (0.4) on the GPU lane |
| **1a** | Perceptual metric (`buiy_verify::metric`) + naive-metric retirement | 0 | Headless; the GPU-site migration (1a.10/1a.11) runs on the GPU lane |
| **1b** | Reftest harness + CPU/GPU SDF cross-check (`buiy_verify::reftest`) | 0, 1a | Pure-CPU meta-tests headless; reftest cases / cross-check / engine pairs `#[ignore]` GPU lane |
| **2** | Tier 1-2 snapshots + Tier 3 invariants | 0 (insta), 1a (metric) | **Wholly headless** (no `#[ignore]`) |
| **3** | Determinism stack + Tier 5 golden persistence | 0 (`Dpr`/`capture_to_image`), 1 (metric/reftest) | Pure-CPU half headless; capture/golden GPU half `#[ignore]` GPU lane; CI lavapipe leg |
| **4** | Coverage-by-construction + forced-colors live wiring + docs flip | 2, 3 | Coverage self-tests + enrollment drivers headless; `coverage_golden` `#[ignore]` GPU lane |

**Dependency order rationale.** Phase 0 lands every shared seam (the metric/snapshot deps, the dev-only `buiy_core → buiy_verify` edge, the canonical `Dpr`, the promoted `capture_to_image`) — nothing in Phase 1+ compiles without them. Phase 1a's `metric` is the shared primitive both pixel tiers (1b reftests, 3 goldens) consume, so it precedes them. Phase 1b's `run_reftest` and Phase 3's `DeterministicApp` both build on the same capture seam; 1b uses the landed `gpu_render_app`-derived `capture_app` directly and Phase 3 swaps that one line for `DeterministicApp::build` (identical `&mut App → RgbaImage` contract). Phase 2 is independent of the GPU tiers (it needs only `insta` + `metric`) and can land in parallel after Phase 1a. Phase 4 composes everything: it Cartesian-products the fixture corpus across all five tiers and ends with the docs flip.

**Gate legend.** *Headless gate* (run before each commit) = `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace` (no `--ignored`, no adapter). *GPU lane* (additive) = `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1` (and the `buiy_core` `#[ignore]` files) on a real adapter (RX 6700 XT locally; lavapipe in CI). New deps gate on `cargo deny check` first. Commit per task; Conventional Commits; body ends `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

---

## Phase 0 — Cross-cutting prerequisites

These four tasks land the shared seams every later tier imports: the metric/snapshot crate dependencies (0.1), the dev-only `buiy_core → buiy_verify` edge that lets the `#[ignore]` GPU tests reach `buiy_verify::metric` (0.2), the canonical `Dpr` type (0.3), and the promoted `capture_to_image` capture primitive (0.4). Nothing in Phase 1+ compiles without them. Each is independently committable and leaves the tree green (`cargo test --workspace` headless gate passes after every task).

**Spec anchors:** `metric.md` § "Crate choice" (pixelmatch over dify; `image-compare` for MSSIM), `metric.md` § "Migration" (dev-dep cycle), `determinism.md` § "Extending `GoldenConfig`" (`Dpr`), `determinism.md` § "Where the code lives" (`capture_to_image` promotion), README § "Crate-dependency note" (`image` is the only new `buiy_core` dep).

---

### Task 0.1 — Add the metric + snapshot deps to `buiy_verify`, gated by `cargo deny check`

`buiy_verify` gains the snapshot/MSSIM crates the metric and snapshot tiers consume: `image-compare` (advisory MSSIM channel; `metric.md` § "Advisory MSSIM") and `insta` (snapshot assertions; `snapshots.md`). **`pixelmatch` is NOT added** — Phase 1a vendors its algorithm (see Phase 1a's deviation note); this task does not depend on that decision because nothing here consumes `pixelmatch`. Exact patch pins (`=`) so a metric-crate bump cannot silently shift baselines (`metric.md` `cargo deny check` note). The supply-chain gate (`cargo deny check`) must pass — `deny.toml` is allow-list-only, so any new transitive license fails CI until added explicitly, never via an exception hack (CLAUDE.md).

**Files:**
- Modify: `crates/buiy_verify/Cargo.toml` (`[dependencies]`, after `proptest.workspace = true`)
- Test: the `cargo deny check` + `cargo build -p buiy_verify` runs below (no Rust test file — this task only proves the deps resolve + pass the license gate; the metric/snapshot code that *uses* them lands in Phase 1/2)

Steps:

- [ ] **Run `cargo deny check` on the unchanged tree to capture the green baseline.** From the repo root:
  ```sh
  cargo deny check
  ```
  Expected: `advisories ok`, `bans ok`, `licenses ok`, `sources ok` (the lone `paste` RUSTSEC-2024-0436 is already in `deny.toml`'s `ignore`). This is the "before" — proves the gate is green so a post-add failure is attributable to the new deps.

- [ ] **Add the two deps to `crates/buiy_verify/Cargo.toml`.** Append to the `[dependencies]` table (after the `proptest.workspace = true` line):
  ```toml
  # Advisory MSSIM channel (metric.md § "Advisory MSSIM"): catches global
  # gamma/blend drift a small pixel budget under-weights. NEVER the primary
  # gate — surfaced as `Diff::mssim: Option<f64>`. The `cargo deny check` below
  # confirms its license set + no RUSTSEC advisories.
  image-compare = "=0.5.0"
  # Tier-1/2 snapshot assertions (snapshots.md): insta drives the layout-number
  # and display-list `Display` dumps. Dev-time crate, but lives in `[dependencies]`
  # because the harness re-exports snapshot helpers from `src/`. The `glob` feature
  # drives the coverage fixture-dir fan-out (Phase 4).
  insta = { version = "=1.43.2", features = ["glob"] }
  ```
  (Pin `insta` to the exact latest 1.x patch resolved at implementation time — run `cargo search insta` and substitute; `=1.43.2` is the placeholder. `insta` carries no baseline-shifting risk like the metric crates, but exact-pinning keeps the dep set reproducible.)

- [ ] **Resolve the new deps and confirm they compile.** From the repo root:
  ```sh
  cargo build -p buiy_verify
  ```
  Expected: `image-compare v0.5.0`, `insta v1.43.2` (+ their transitives, notably `moxcms`/`pxfm`/`thiserror`/`itertools`/`byteorder-lite` under `image-compare`) appear in the `Compiling …` output, then `Finished`. If a version does not exist, Cargo errors here — pick the nearest existing patch and re-pin.

- [ ] **Run the supply-chain gate on the new dep graph.** From the repo root:
  ```sh
  cargo deny check
  ```
  Expected: PASS. If `licenses` now FAILS, read which SPDX id the new transitive introduced, confirm it is OSI-permissive (MIT / Apache-2.0 / BSD / Unicode / Zlib), and add that exact short SPDX id to `deny.toml`'s `[licenses] allow` list with a one-line comment naming the crate that pulled it — **never** via a `[licenses] exceptions` hack (CLAUDE.md). If `advisories` FAILS on a new RUSTSEC id, stop and surface it — do not bulk-suppress.

- [ ] **Run the headless gate to confirm the workspace still builds + tests green** (the new deps must not break the existing `buiy_verify` smoke/visual tests, which Phase 1 migrates):
  ```sh
  cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test -p buiy_verify
  ```
  Expected: clippy clean, existing `buiy_verify` tests pass (they still reference `visual::compare_images` — that migration is Phase 1, not here).

- [ ] **Commit.**
  ```sh
  git commit -am "build(verify): add image-compare + insta deps (deny-gated)

  Phase 0.1 of the verification pyramid: the advisory MSSIM channel
  (image-compare) and the tier-1/2 snapshot driver (insta, glob feature)
  land in buiy_verify with exact patch pins. cargo deny check passes; any
  new transitive license is added explicitly to deny.toml's allow list.
  pixelmatch is NOT added here — Phase 1a vendors its algorithm.

  No code consumes them yet — the metric/snapshot modules land in Phase 1/2.
  Spec: docs/specs/2026-06-15-buiy-verification-design/metric.md § Crate choice.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 0.2 — Add `buiy_verify` as a `[dev-dependencies]` of `buiy_core` (the dev-only cycle)

The `#[ignore]` GPU re-capture tests in `crates/buiy_core/tests/text_*_gpu.rs` (~20 sites, e.g. `text_gpu.rs:114`/`:152`/`:271`) migrate off the deprecated `buiy_core::render::golden::perceptual_diff` (L1) onto `buiy_verify::metric::compare` (`metric.md` § "Migration"). For those *tests* to name `buiy_verify`, `buiy_core` needs a dev-dependency edge to it. This is a **dev-only dependency cycle** (`buiy_core → buiy_verify → buiy_core`): Cargo permits it because a `[dev-dependencies]` edge does not participate in the normal build graph, so it creates no real cycle, does not affect the production `cargo build -p buiy_core`, and does not enter `cargo deny`'s normal-graph audit. The edge is confined to `#[cfg(test)]`. No test *consumes* it in this task — Phase 1a migrates the call sites; here we only prove the edge resolves. **This is the canonical site of the dev-dep edge; Phase 1a.10 assumes it already exists.**

**Files:**
- Modify: `crates/buiy_core/Cargo.toml` (`[dev-dependencies]` — today lists only `naga = "27"`)
- Test: the `cargo build -p buiy_core --tests` run below (proves the dev-dep edge resolves without a cycle error; no Rust test file — the consuming migration is Phase 1a)

Steps:

- [ ] **Confirm the edge does not yet exist (compile a probe that should fail).** Append a throwaway probe to the bottom of `crates/buiy_core/tests/render_golden_harness.rs`:
  ```rust
  #[test]
  fn buiy_verify_is_reachable_from_buiy_core_tests() {
      // Phase 0.2 tripwire: proves the dev-only buiy_core → buiy_verify edge
      // resolves (Cargo permits the dev-dep cycle). Re-targeted to
      // buiy_verify::metric::compare in Phase 1a when the call sites migrate.
      let _ = buiy_verify::visual::compare_images;
  }
  ```
  (The probe targets the still-present `visual` module because `metric::compare` does not exist until Phase 1a.) Then run:
  ```sh
  cargo build -p buiy_core --tests 2>&1 | head -5
  ```
  Expected FAILURE: `error[E0433]: failed to resolve: use of undeclared crate or module 'buiy_verify'`.

- [ ] **Add the dev-dependency edge.** Edit `crates/buiy_core/Cargo.toml`'s `[dev-dependencies]`:
  ```toml
  [dev-dependencies]
  naga = "27"
  # Dev-only dependency edge for the #[ignore] GPU re-capture tests, which
  # migrate off the deprecated `render::golden::perceptual_diff` (L1) onto
  # `buiy_verify::metric::compare` (metric.md § Migration). This forms a
  # DEV-ONLY cycle (buiy_core → buiy_verify → buiy_core): a [dev-dependencies]
  # edge is excluded from the normal build graph, so Cargo permits it, the
  # production `cargo build -p buiy_core` is unaffected, and it adds no
  # `cargo deny` surface. Confined to #[cfg(test)].
  buiy_verify = { path = "../buiy_verify" }
  ```

- [ ] **Verify the edge resolves (no cycle error).**
  ```sh
  cargo build -p buiy_core --tests 2>&1 | tail -5
  ```
  Expected: `Finished` — Cargo resolves the dev-dep edge with no `cyclic package dependency` error (the tripwire test compiles, proving `buiy_verify` is now reachable). If Cargo *does* error with a cycle, the edge was mistakenly added to `[dependencies]` instead of `[dev-dependencies]` — fix and re-run.

- [ ] **Remove the temporary tripwire test** (its job — proving the edge resolves — is done; leaving it would block the Phase 1a deletion of `visual::compare_images`). Delete the `buiy_verify_is_reachable_from_buiy_core_tests` fn from `render_golden_harness.rs`.

- [ ] **Run the headless gate** to confirm the edge introduced no breakage:
  ```sh
  cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test -p buiy_core
  ```
  Expected: clippy clean, all `buiy_core` headless tests pass.

- [ ] **Commit.**
  ```sh
  git commit -am "build(core): add buiy_verify as a dev-dependency (dev-only cycle)

  Phase 0.2 of the verification pyramid: the #[ignore] GPU re-capture tests
  in tests/text_*_gpu.rs migrate (Phase 1a) off the deprecated L1
  perceptual_diff onto buiy_verify::metric::compare, so buiy_core's tests need
  to name buiy_verify. Added under [dev-dependencies] only — this forms a
  DEV-ONLY cycle (core → verify → core) that Cargo permits because dev-dep
  edges are excluded from the normal build graph. Confined to #[cfg(test)].

  Spec: docs/specs/2026-06-15-buiy-verification-design/metric.md § Migration.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 0.3 — Define the canonical `Dpr` type in `buiy_core::render::golden`

`Dpr` is device-pixel-ratio as **integer milliscale** (1000 = 1.0×, 2000 = 2.0×) so it is `Eq + Hash + Ord` with no float pitfalls — it is a *fixture axis* that keys goldens and coverage cells, never a tolerance. Defined **once** here (`determinism.md` § "Extending `GoldenConfig`"); `goldens.md`'s `GoldenKey.dpr` and `coverage.md`'s `Matrix.dprs`/`CoverageKey.dpr` import this type, they do not redefine it. The capture boundary converts the window's `f32` `scale_factor` via `Dpr::from_f32` and back via `Dpr::as_f32`. TDD: the round-trip unit test is written first and must fail (the type does not exist), then the type makes it pass.

**Files:**
- Modify: `crates/buiy_core/src/render/golden.rs` (insert after the `GoldenConfig` impl, ~line 46)
- Test: `crates/buiy_core/src/render/golden.rs` (new `#[cfg(test)] mod tests` at the file tail — a pure-CPU unit test, runs under the headless gate, no `#[ignore]`)

Steps:

- [ ] **Write the failing round-trip unit test.** Append to the tail of `crates/buiy_core/src/render/golden.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn dpr_milliscale_round_trips_f32() {
          // The canonical fixture axis: integer milliscale so it is Eq+Hash+Ord,
          // but it must convert losslessly to/from the f32 scale_factor the
          // window/extract path carries (determinism.md § Extending GoldenConfig).
          assert_eq!(Dpr::from_f32(1.0), Dpr::X1);
          assert_eq!(Dpr::from_f32(2.0), Dpr::X2);
          assert_eq!(Dpr::X1.as_f32(), 1.0);
          assert_eq!(Dpr::X2.as_f32(), 2.0);
          // Round-trip through both directions for a fractional ratio (1.5×).
          assert_eq!(Dpr::from_f32(1.5), Dpr(1500));
          assert_eq!(Dpr(1500).as_f32(), 1.5);
          // from_f32 rounds to nearest milliscale (no truncation drift).
          assert_eq!(Dpr::from_f32(1.2345), Dpr(1235));
      }

      #[test]
      fn dpr_is_ord_and_hashable() {
          // It keys a golden/coverage cell, so Ord + Hash must hold (the reason
          // for milliscale over f32). A plain compile-and-run proof.
          use std::collections::HashSet;
          assert!(Dpr::X1 < Dpr::X2);
          let mut set = HashSet::new();
          assert!(set.insert(Dpr::X1));
          assert!(!set.insert(Dpr::X1)); // already present — Hash + Eq agree
          assert!(set.insert(Dpr::X2));
      }
  }
  ```

- [ ] **Run to verify it fails to compile** (the type does not exist):
  ```sh
  cargo test -p buiy_core --lib render::golden 2>&1 | head -15
  ```
  Expected FAILURE: `error[E0433]: failed to resolve: use of undeclared type 'Dpr'` (and `Dpr::X1`, `from_f32`, `as_f32` all unresolved).

- [ ] **Write the minimal `Dpr` definition.** Insert into `crates/buiy_core/src/render/golden.rs` immediately after the `GoldenConfig` impl block (after the closing `}` on line 46, before the `perceptual_diff` doc comment on line 48):
  ```rust
  /// **Canonical device-pixel-ratio type.** Integer *milliscale* (1000 = 1.0×,
  /// 2000 = 2.0×) so it is `Eq + Hash + Ord` without float pitfalls — it is a
  /// *fixture axis* that keys a golden / coverage cell, **never** a tolerance.
  ///
  /// Defined ONCE here; `buiy_verify::golden::GoldenKey.dpr` and
  /// `buiy_verify::coverage::{Matrix.dprs, CoverageKey.dpr}` import this type,
  /// they do **not** redefine it (verification-design `determinism.md`). The
  /// capture boundary converts the window's `f32` `scale_factor` via
  /// [`Dpr::from_f32`] and back via [`Dpr::as_f32`] when sizing the offscreen
  /// target. Derives `serde` so the golden bless ledger can persist it directly;
  /// `buiy_core` already carries `serde` as a workspace dep.
  #[derive(
      Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord,
      serde::Serialize, serde::Deserialize,
  )]
  pub struct Dpr(pub u32);

  impl Dpr {
      /// 1.0× device-pixel-ratio (the headless capture default).
      pub const X1: Self = Dpr(1000);
      /// 2.0× device-pixel-ratio (the HiDPI fixture axis).
      pub const X2: Self = Dpr(2000);

      /// Round an `f32` scale factor to integer milliscale (`1.0 → Dpr(1000)`).
      /// Rounds to nearest so a `1.5×` window maps to `Dpr(1500)` exactly.
      pub fn from_f32(scale: f32) -> Self {
          Dpr((scale * 1000.0).round() as u32)
      }

      /// Back to the `f32` scale factor the window / extract path consumes.
      pub fn as_f32(&self) -> f32 {
          self.0 as f32 / 1000.0
      }
  }
  ```
  (`serde` is already in `buiy_core`'s dep graph via the workspace `serde` dep used elsewhere in `render/`; if `cargo doc`/`clippy` flags `serde` as not a direct dep, add `serde.workspace = true` to `crates/buiy_core/Cargo.toml`'s `[dependencies]` in this same step and note it in the commit — but verify first, as bevy re-exports may already satisfy it.)

- [ ] **Run to verify the tests pass:**
  ```sh
  cargo test -p buiy_core --lib render::golden 2>&1 | tail -10
  ```
  Expected: `dpr_milliscale_round_trips_f32 ... ok`, `dpr_is_ord_and_hashable ... ok`.

- [ ] **Run the doc + clippy gate** (the new `pub` type carries doc comments that must pass `RUSTDOCFLAGS="-D warnings"`, and the `serde` derive must not trip clippy):
  ```sh
  cargo clippy -p buiy_core --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc -p buiy_core --no-deps
  ```
  Expected: both clean.

- [ ] **Commit.**
  ```sh
  git commit -am "feat(core): canonical Dpr milliscale type in render::golden

  Phase 0.3 of the verification pyramid: Dpr is device-pixel-ratio as integer
  milliscale (1000 = 1×, 2000 = 2×) so it is Eq+Hash+Ord — a fixture axis that
  keys goldens/coverage cells, never a tolerance. Defined ONCE here; goldens
  and coverage import it. from_f32/as_f32 round-trip the window's f32
  scale_factor at the capture boundary; serde-derived for the bless ledger.

  Spec: docs/specs/2026-06-15-buiy-verification-design/determinism.md
        § Extending GoldenConfig.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 0.4 — Promote `capture_to_image(&mut App, &GoldenConfig) -> image::RgbaImage` into `render/golden.rs` src

The shared capture seam moves out of `crates/buiy_core/tests/support/mod.rs` (where only `buiy_core`'s own tests can reach it) into `render/golden.rs` *src*, so `buiy_verify`'s reftest and golden tiers can call it (`determinism.md` § "Where the code lives"; README § Architecture). The promoted body extracts the existing `render_to_image` (offscreen target sized to **physical** pixels = `logical × dpr`) + `spawn_capture_camera` (`CAPTURE_MSAA = Msaa::Off`, opaque-black clear) + frame-drive + `readback_rgba` machinery and assembles the un-padded RGBA8 bytes into an `image::RgbaImage`. This requires `buiy_core` to gain `image = "0.25"` as a direct dep (README "Crate-dependency note": the *only* new GPU dep). GPU-coupled, so its meta-test is `#[ignore]`.

**Scope boundary (honest):** Phase 0.4 promotes the *capture mechanics only* — size-to-physical, paint, readback, assemble `RgbaImage` — reusing the landed `gpu_render_app_scaled`/`readback_rgba` path. The full four-condition quiescence flush (asset-server + pipeline-cache gates) and the `cfg.dpr` `scale_factor` assertion described in `determinism.md` § "Async-asset flush" are **Phase 3.3**'s additions to this same function, not Phase 0. Phase 0.4 drives a bounded fixed frame count + the existing `wait_for_text_ready`-style atlas settle, exactly as `render_golden_harness.rs` does today, so the seam exists and is callable; Phase 3 hardens it.

**Files:**
- Modify: `crates/buiy_core/Cargo.toml` (`[dependencies]` — add `image`)
- Modify: `crates/buiy_core/src/render/golden.rs` (new `capture_to_image` fn + `readback_rgba_into` + the `CAPTURE_MSAA`/`CAPTURE_DITHER_OFF` constants; src, production-callable infra)
- Test: `crates/buiy_core/tests/render_golden_harness.rs` (new `#[ignore]` GPU test asserting `capture_to_image` returns an `RgbaImage` of the expected physical dimensions)

Steps:

- [ ] **Add `image` as a direct dep of `buiy_core`.** Edit `crates/buiy_core/Cargo.toml`'s `[dependencies]` — append after the last existing dep:
  ```toml
  # The promoted `render::golden::capture_to_image` returns an
  # `image::RgbaImage` (verification-design README § Crate-dependency note: the
  # ONLY new GPU dep buiy_core gains). Rides the existing workspace `image`
  # pin — no second image-decode stack enters the tree.
  image.workspace = true
  ```

- [ ] **Write the failing `#[ignore]` GPU dimension meta-test.** Append to `crates/buiy_core/tests/render_golden_harness.rs`:
  ```rust
  // Needs a wgpu adapter (real GPU or lavapipe). Proves the promoted
  // `capture_to_image` seam paints a fixture and returns an `image::RgbaImage`
  // of the expected PHYSICAL dimensions (logical × dpr). Run with:
  //   cargo test -p buiy_core --test render_golden_harness -- --ignored --nocapture
  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
  fn capture_to_image_returns_physical_dimensions() {
      use bevy::prelude::*;
      use buiy_core::Node;
      use buiy_core::layout::{Inset, Length, Sizing, Style};
      use buiy_core::render::color::ColorToken;
      use buiy_core::render::components::Background;
      use buiy_core::render::golden::{GoldenConfig, capture_to_image};
      use std::borrow::Cow;

      const LOGICAL_W: u32 = 48;
      const LOGICAL_H: u32 = 32;

      // 1.0× capture: physical == logical. (Phase 0.4 sizes via the literal 1.0
      // path; GoldenConfig has no `dpr` field until Phase 3.1.)
      let cfg = GoldenConfig::deterministic();
      let mut app = support::gpu_render_app_scaled(LOGICAL_W, LOGICAL_H, 1.0);

      // A known opaque fill so the capture is non-trivial (a blank frame would
      // pass the dimension check vacuously; this proves real paint flows through).
      {
          let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
          theme
              .colors
              .insert("cap.fill".into(), Color::srgb(0.2, 0.6, 0.9));
      }
      let fill = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .absolute()
                  .inset(Inset {
                      top: Sizing::Length(Length::px(4.0)),
                      left: Sizing::Length(Length::px(4.0)),
                      ..default()
                  })
                  .width_px(16.0)
                  .height_px(16.0),
              Background {
                  color: ColorToken::Token(Cow::Borrowed("cap.fill")),
              },
          ))
          .id();
      app.world_mut()
          .spawn((Node, Style::default()))
          .add_children(&[fill]);

      let img = capture_to_image(&mut app, &cfg);

      assert_eq!(
          (img.width(), img.height()),
          (LOGICAL_W, LOGICAL_H),
          "1× capture is logical-sized in physical pixels"
      );
      // Non-vacuous: at least one pixel differs from the opaque-black clear.
      let any_painted = img.pixels().any(|p| p.0 != [0, 0, 0, 255]);
      assert!(any_painted, "capture produced non-clear pixels");
  }
  ```

- [ ] **Run to verify it fails to compile** (`capture_to_image` does not exist):
  ```sh
  cargo test -p buiy_core --test render_golden_harness 2>&1 | head -15
  ```
  Expected FAILURE: `error[E0432]: unresolved import 'buiy_core::render::golden::capture_to_image'`.

- [ ] **Write the `capture_to_image` implementation + capture constants.** Insert into `crates/buiy_core/src/render/golden.rs` after the `Dpr` impl (Phase 0.3) and before `perceptual_diff`. The body mirrors the proven `render_golden_harness.rs` capture flow but lives in src and returns an `RgbaImage`:
  ```rust
  /// Single-sampled capture: a 4× MSAA resolve antialiases edges
  /// nondeterministically across drivers, while Buiy's in-shader analytic AA is
  /// deterministic given identical FP — so MSAA buys nothing here and costs
  /// determinism. Mirrors the capture camera's landed `Msaa::Off`
  /// (verification-design `determinism.md`).
  pub const CAPTURE_MSAA: bevy::render::view::Msaa = bevy::render::view::Msaa::Off;

  /// Deband dither perturbs the low bits of the tonemapped output; the capture
  /// camera pins it off. A `true` sentinel the capture path documents (the
  /// camera spawns with no `DebandDither::Enabled`).
  pub const CAPTURE_DITHER_OFF: bool = true;

  /// **The shared capture seam** (verification-design README § Architecture):
  /// render the already-built, fixture-populated `app` into an offscreen target
  /// sized to the window's PHYSICAL pixel grid and read it back as an
  /// `image::RgbaImage`. Re-runnable against one `App` (a reftest calls it twice
  /// on one device; spec § "Resolved during synthesis" #4).
  ///
  /// Phase-0 scope: the capture mechanics (size-to-physical, paint, readback,
  /// assemble). The four-condition quiescence flush and the
  /// `scale_factor == cfg.dpr` assertion are Phase 3.3's hardening of this same
  /// function (`determinism.md` § Async-asset flush).
  ///
  /// Drives `MAX_CAPTURE_FRAMES` update frames after finishing the app (pipeline
  /// async-compile + extract + prepare + paint settle), then reads back the
  /// offscreen target's un-padded RGBA8 bytes.
  pub fn capture_to_image(app: &mut bevy::app::App, _cfg: &GoldenConfig) -> image::RgbaImage {
      use bevy::asset::RenderAssetUsages;
      use bevy::camera::RenderTarget;
      use bevy::image::Image;
      use bevy::prelude::*;
      use bevy::render::render_resource::{TextureFormat, TextureUsages};

      // Physical pixel grid the offscreen target must match: the primary
      // window's physical size (logical × scale_factor), which the view uniform
      // is built from (extract fills `logical_size` from the primary window).
      let (phys_w, phys_h) = {
          let window = app
              .world_mut()
              .query::<&bevy::window::Window>()
              .single(app.world())
              .expect("primary window for capture sizing");
          let r = window.resolution.physical_size();
          (r.x, r.y)
      };

      // Offscreen Rgba8UnormSrgb target with COPY_SRC for the readback copy and
      // RenderAssetUsages::all() so the GpuImage exists in the render world.
      let target = {
          let mut image =
              Image::new_target_texture(phys_w, phys_h, TextureFormat::Rgba8UnormSrgb, None);
          image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
          image.asset_usage = RenderAssetUsages::all();
          app.world_mut().resource_mut::<Assets<Image>>().add(image)
      };

      // Capture camera: opaque-black clear, CAPTURE_MSAA (single-sampled),
      // dither off (bare Camera2d at Msaa::Off carries no DebandDither::Enabled).
      app.world_mut().spawn((
          Camera2d,
          RenderTarget::from(target.clone()),
          CAPTURE_MSAA,
          Camera {
              clear_color: ClearColorConfig::Custom(Color::BLACK),
              ..default()
          },
      ));

      // Finish materializes the device + pipelines; drive frames so layout →
      // extract → prepare → paint settle before the readback poll.
      const MAX_CAPTURE_FRAMES: usize = 3;
      app.finish();
      app.cleanup();
      for _ in 0..MAX_CAPTURE_FRAMES {
          app.update();
      }

      let bytes = readback_rgba_into(app, &target, phys_w, phys_h);
      image::RgbaImage::from_raw(phys_w, phys_h, bytes)
          .expect("readback byte count matches phys_w * phys_h * 4")
  }
  ```
  Then add `readback_rgba_into` directly below `capture_to_image` — the src twin of the test-support `readback_rgba` (the readback poll cannot stay in `tests/support`, so promote its body too). Copy the proven poll + 256-byte row-padding strip from `tests/support/mod.rs:353`–end, with the signature `fn readback_rgba_into(app: &mut bevy::app::App, target: &bevy::asset::Handle<bevy::image::Image>, w: u32, h: u32) -> Vec<u8>`. Keep it `pub(crate)` if no external caller needs it; `pub` if a reftest reads back directly. Verify the exact `Readback`/`ReadbackComplete` import paths against `tests/support/mod.rs` when copying.

- [ ] **Migrate `tests/support` to delegate** (DRY — the support helper must not duplicate the now-promoted logic). Re-point `tests/support/mod.rs`'s `readback_rgba` to call `buiy_core::render::golden::readback_rgba_into` (or, if `readback_rgba_into` was made `pub(crate)` and is thus unreachable from tests, keep both and note the intentional duplication is temporary until Phase 3 consolidates). Prefer the delegation; confirm `cargo build -p buiy_core --tests` still compiles.

- [ ] **Run the `#[ignore]` GPU meta-test on the real adapter** (this host, AMD RX 6700 XT):
  ```sh
  cargo test -p buiy_core --test render_golden_harness -- --ignored --test-threads=1 capture_to_image_returns_physical_dimensions --nocapture
  ```
  Expected: `capture_to_image_returns_physical_dimensions ... ok` — the returned `RgbaImage` is `48×32` and has non-clear pixels.

- [ ] **Run the full headless gate + the doc gate** (the new `pub` fn + constants must pass clippy and `RUSTDOCFLAGS="-D warnings"`; the headless gate must stay green with `image` now a direct dep):
  ```sh
  cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc -p buiy_core --no-deps && xvfb-run -a cargo test -p buiy_core
  ```
  Expected: all clean/green. (The headless leg does NOT run the `#[ignore]` capture test — that is the GPU lane above — but it confirms the src compiles + the support migration did not break the existing headless tests.)

- [ ] **Run the supply-chain gate** (`image` is already a workspace dep, so no new license — but the gate is cheap insurance that adding it as a *direct* `buiy_core` dep changed nothing):
  ```sh
  cargo deny check
  ```
  Expected: PASS (no new transitive — `image = "0.25"` is already resolved for `buiy_verify`).

- [ ] **Commit.**
  ```sh
  git commit -am "feat(core): promote capture_to_image into render::golden src

  Phase 0.4 of the verification pyramid: the shared GPU capture seam moves out
  of tests/support into render::golden src as
  capture_to_image(&mut App, &GoldenConfig) -> image::RgbaImage, so
  buiy_verify's reftest + golden tiers can call it. Sizes the offscreen target
  to the window's physical pixel grid, paints under CAPTURE_MSAA (single-
  sampled, dither off), and reads back into an RgbaImage. buiy_core gains
  image as a direct dep (README § Crate-dependency note: the only new GPU
  dep). #[ignore] GPU meta-test asserts physical dimensions + non-vacuous paint.

  Phase-0 scope is the capture mechanics; the four-condition quiescence flush
  and the scale_factor==dpr assertion are Phase 3.3's hardening.
  Spec: docs/specs/2026-06-15-buiy-verification-design/determinism.md
        § Where the code lives.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

**Phase 0 exit criteria:** `cargo deny check` green with the two new metric/snapshot deps; `cargo build -p buiy_core --tests` resolves the dev-only `buiy_verify` edge with no cycle error; `Dpr` round-trips `f32` under the headless gate; `capture_to_image` returns a correctly-sized non-blank `RgbaImage` on the GPU lane. Phase 1 (metric + reftests) now has every seam it imports.

---

## Phase 1a — Perceptual metric

Realizes [`metric.md`](../specs/2026-06-15-buiy-verification-design/metric.md). Builds `buiy_verify::metric` — the AA-aware two-axis perceptual diff shared by tiers 4 (reftests) and 5 (goldens) — then retires the two naive metrics it supersedes. Pure CPU; every task here runs under the headless gate with **no** `--ignored`, except the final two GPU-site migration tasks (which only *compile* under the headless gate and *run* on the GPU lane).

> **Critical deviation from `metric.md` — `pixelmatch = "0.1.0"` is not usable as specified; vendor the algorithm instead.** Verified against the published crate source (`~/.cargo/registry/.../pixelmatch-0.1.0/src/lib.rs`): the crate's only public surface is `pixelmatch(img1: impl Read, img2: impl Read, out, w, h, Options) -> Result<usize>` — it (a) consumes **PNG-encoded byte streams**, not `image::RgbaImage`; (b) returns only a **flat changed-pixel `usize`**, exposing neither the per-pixel YIQ delta nor an L∞ channel delta, so it cannot feed `Diff`'s `max_channel_delta` axis; (c) keeps `color_delta` / `antialiased` **private**, contradicting metric.md's "It exposes the `colorDelta`/`antialiased` primitives `compare` wraps"; and (d) is pinned to the **`image` 0.24** API (`ImageOutputFormat`, `DynamicImage::from_decoder`), which **does not compile against the workspace `image = "0.25"`**. The spec's own directive is "adopt the reference algorithm — don't re-derive the `35215`/YIQ constants." We honor that directive precisely by **vendoring the ~150 LOC reference algorithm** (the exact `color_delta` luminance-weighted YIQ delta + the `antialiased` brightest/darkest-sibling test + `has_many_siblings`) into `metric.rs`, ported verbatim from pixelmatch's MIT source onto `image` 0.25 / `RgbaImage`, with a provenance comment. This is strictly *more* faithful to metric.md's intent than depending on an unusable, `image`-incompatible crate, and it is what gives `compare` the per-pixel hooks the two-axis `Diff` requires. **Net dependency delta for Phase 1a: `image-compare = "=0.5.0"` only** (MSSIM; landed in Phase 0.1); **no `pixelmatch` dependency is added.** The constants are guarded against drift by the known-answer unit tests below, which is exactly the protection a version pin would give. *(This deviation should be reflected back into `metric.md` § "Crate choice" / "Migration" — see Self-review § gaps.)*

---

### Task 1a.0 — Confirm `image-compare` resolves (Phase 0.1 already added it)

Phase 0.1 added `image-compare = "=0.5.0"` to `buiy_verify`. This task is a thin re-confirmation that the dep is present and the supply-chain gate is green before the metric code consumes it. (If Phase 0.1 was skipped or the dep is absent, add it here per the Phase 0.1 step.)

**Files:**
- Verify: `crates/buiy_verify/Cargo.toml` (the `image-compare = "=0.5.0"` line from Phase 0.1)

- [ ] Step — confirm the dep resolves and the gate is green:
  ```sh
  grep -n "image-compare" crates/buiy_verify/Cargo.toml && cargo build -p buiy_verify && cargo deny check 2>&1 | tail -8
  ```
  Expected: the dep line is present; `Finished`; `advisories ok`, `licenses ok`, `bans ok`, `sources ok`. If `image-compare` is missing, add it now per Phase 0.1 and re-run `cargo deny check`, recording any new SPDX id added to `deny.toml`'s allow list in the eventual commit. No commit for this confirmation-only task.

---

### Task 1a.1 — Module skeleton: `Diff` / `FuzzBudget` / `CompareOpts` types + identity-on-empty `compare` stub

Smallest red-green slice that pins the type shapes and wires the module into `lib.rs`. `compare` is a deliberately-incomplete stub (returns the empty/identity `Diff`) so the type-shape tests bind before the algorithm lands.

**Files:**
- Create: `crates/buiy_verify/src/metric.rs`
- Modify: `crates/buiy_verify/src/lib.rs` (add `pub mod metric;`)
- Test (inline `#[cfg(test)]` for type-shape unit checks): `crates/buiy_verify/src/metric.rs`

- [ ] Step — write the failing test. Append this `#[cfg(test)]` block at the end of the new `metric.rs` (it references types that do not yet exist, so it fails to compile — the RED state):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn exact_budget_is_zero_zero() {
          assert_eq!(FuzzBudget::EXACT.max_channel_delta, 0);
          assert_eq!(FuzzBudget::EXACT.max_diff_pixels, 0);
      }

      #[test]
      fn default_opts_are_lenient_aware() {
          let o = CompareOpts::default();
          assert_eq!(o.threshold, 0.1);
          assert!(!o.include_aa);
          assert!(o.mssim);
          assert!(!o.emit_diff_image);
      }

      #[test]
      fn empty_vs_empty_is_zero_diff() {
          let e = image::RgbaImage::new(0, 0);
          let d = compare(&e, &e, &CompareOpts::default());
          assert_eq!(d.differing_pixels, 0);
          assert_eq!(d.max_channel_delta, 0);
          assert_eq!(d.total_pixels, 0);
          assert_eq!(d.mssim, None);
          assert!(d.diff_image.is_none());
      }
  }
  ```

- [ ] Step — run to verify it fails (compile error — types/fn undefined):
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -20
  ```
  Expected: `error[E0433]`/`E0432` — cannot find `FuzzBudget`, `CompareOpts`, `Diff`, or `compare` in `super`.

- [ ] Step — write the minimal implementation. Prepend the module doc + types + the empty-only `compare` stub above the `#[cfg(test)]` block, so `metric.rs` begins:
  ```rust
  //! Perceptual image diff — the shared metric for reftests (tier 4) and goldens
  //! (tier 5). Luminance-weighted YIQ colorDelta + antialias-sibling exclusion,
  //! gated on a two-axis FuzzBudget. Supersedes render::golden::perceptual_diff
  //! (L1) and visual::compare_images (RMSE).
  //!
  //! The per-pixel YIQ `color_delta`, the `antialiased` brightest/darkest-sibling
  //! test, and `has_many_siblings` are ported verbatim from the canonical
  //! pixelmatch reference (MIT; mapbox/pixelmatch, the Rust `pixelmatch` 0.1.0
  //! crate). They are vendored, not depended on: the published crate consumes
  //! PNG byte streams, returns only a flat count, keeps these primitives private,
  //! and is image-0.24-bound — none of which fits `Diff`'s two-axis shape on
  //! image 0.25. Vendoring is metric.md's "adopt the reference algorithm, don't
  //! re-derive the 35215/YIQ constants" applied exactly.

  use image::RgbaImage;

  /// Outcome of one comparison. All counts are over the diffed (overlapping)
  /// pixel set. `diff_image` is emitted only when `CompareOpts::emit_diff_image`.
  #[derive(Clone, Debug)]
  pub struct Diff {
      /// Non-AA pixels whose YIQ colorDelta exceeded the per-pixel threshold.
      pub differing_pixels: u32,
      /// Largest single-channel L∞ delta over all pixels (diagnostic; 0..=255).
      pub max_channel_delta: u8,
      /// Total pixels compared (== w*h; 0 only for empty/degenerate input).
      pub total_pixels: u32,
      /// Advisory MSSIM in [0,1] (1 == identical). `None` when skipped.
      pub mssim: Option<f64>,
      /// Heatmap: AA pixels dimmed, differing pixels painted (pixelmatch palette).
      pub diff_image: Option<RgbaImage>,
  }

  /// The two-axis gate. A Diff PASSES iff BOTH hold. Default after determinism is
  /// (0, 0); widen per fixture with a documented reason.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct FuzzBudget {
      /// No single channel of any pixel may differ by more than this (L∞).
      pub max_channel_delta: u8,
      /// At most this many non-AA pixels may exceed the per-pixel YIQ threshold.
      pub max_diff_pixels: u32,
  }

  impl FuzzBudget {
      /// The post-determinism default: bit-exact within one pinned rasterizer.
      pub const EXACT: FuzzBudget = FuzzBudget { max_channel_delta: 0, max_diff_pixels: 0 };
  }

  /// Per-pixel and AA-detection knobs. `threshold` feeds the
  /// `max_delta = 35215 · threshold²` luminance model; `include_aa = true` makes
  /// AA pixels COUNT (for the few tests that assert AA exactly).
  #[derive(Clone, Copy, Debug)]
  pub struct CompareOpts {
      /// Matching sensitivity in [0,1]; default 0.1. Smaller = stricter.
      pub threshold: f64,
      /// Treat antialiased pixels as differences instead of excluding them.
      pub include_aa: bool,
      /// Also compute the advisory MSSIM channel (image-compare). Default true.
      pub mssim: bool,
      /// Allocate and fill `Diff::diff_image`. Off in the hot reftest path.
      pub emit_diff_image: bool,
  }

  impl Default for CompareOpts {
      fn default() -> Self {
          Self { threshold: 0.1, include_aa: false, mssim: true, emit_diff_image: false }
      }
  }

  /// Compare two RGBA images. **Infallible** — returns a `Diff`, never a
  /// `Result`. (Stub: only the empty case is correct until 1a.2/1a.3 land.)
  pub fn compare(a: &RgbaImage, b: &RgbaImage, _opts: &CompareOpts) -> Diff {
      let _ = (a, b);
      Diff {
          differing_pixels: 0,
          max_channel_delta: 0,
          total_pixels: 0,
          mssim: None,
          diff_image: None,
      }
  }
  ```
  Then add the module to `lib.rs` so the `pub mod` block reads (alphabetical):
  ```rust
  pub mod a11y;
  pub mod contrast;
  pub mod metric;
  pub mod visual;
  ```

- [ ] Step — run to verify it passes:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 3 passed` (`exact_budget_is_zero_zero`, `default_opts_are_lenient_aware`, `empty_vs_empty_is_zero_diff`).

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/src/metric.rs crates/buiy_verify/src/lib.rs
  git commit -m "feat(verify): metric module skeleton — Diff/FuzzBudget/CompareOpts

Type shapes + empty-case compare stub, wired into lib.rs. Algorithm
lands next. Realizes metric.md § Types.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.2 — Vendored per-pixel core: YIQ `color_delta` + the differing-pixel count + `max_channel_delta`

Ports the pixelmatch YIQ luminance model and fills the two non-AA-dependent axes of `Diff`. AA exclusion comes in 1a.3; here every over-threshold pixel counts (i.e. behaves as `include_aa = true`). This lets the YIQ-weighting and L∞ axis tests bind before the sibling test complicates them.

**Files:**
- Modify: `crates/buiy_verify/src/metric.rs` (replace the `compare` stub body; add private `color_delta`/`rgb2y`/`rgb2i`/`rgb2q`/`blend` helpers; extend `#[cfg(test)]`)
- Test: inline `#[cfg(test)] mod tests`

- [ ] Step — write the failing tests. Add inside `mod tests`:
  ```rust
  /// Solid w×h image of one color.
  fn solid(w: u32, h: u32, px: [u8; 4]) -> image::RgbaImage {
      image::RgbaImage::from_pixel(w, h, image::Rgba(px))
  }

  #[test]
  fn identity_is_zero_diff() {
      let img = solid(8, 8, [10, 200, 30, 255]);
      let d = compare(&img, &img, &CompareOpts::default());
      assert_eq!(d.differing_pixels, 0);
      assert_eq!(d.max_channel_delta, 0);
      assert_eq!(d.total_pixels, 64);
  }

  #[test]
  fn single_wrong_pixel_survives_every_scale() {
      // The §4 regression: one wrong-by-200 pixel must be caught at any N.
      for n in [16u32, 256, 2048] {
          let a = solid(n, n, [0, 0, 0, 255]);
          let mut b = a.clone();
          b.put_pixel(n / 2, n / 2, image::Rgba([200, 200, 200, 255]));
          let d = compare(&a, &b, &CompareOpts { include_aa: true, mssim: false, ..Default::default() });
          assert_eq!(d.differing_pixels, 1, "N={n}: exactly one differing pixel");
          assert!(d.max_channel_delta >= 200, "N={n}: L∞ caught the 200 delta");
          assert_eq!(d.total_pixels, n * n);
      }
  }

  #[test]
  fn yiq_luminance_outweighs_chroma() {
      // Equal raw L∞ (delta 60 on one channel) but a luma-shifted pixel must
      // score a larger YIQ delta than a chroma-only shift — pins the weighting.
      let base = solid(4, 4, [120, 120, 120, 255]);
      let mut luma = base.clone();
      luma.put_pixel(0, 0, image::Rgba([180, 180, 180, 255])); // +60 all channels: pure luma
      let mut chroma = base.clone();
      chroma.put_pixel(0, 0, image::Rgba([180, 120, 60, 255])); // +60 R / -60 B: chroma-leaning, same L∞=60
      let opts = CompareOpts { include_aa: true, mssim: false, threshold: 0.05, ..Default::default() };
      let dl = compare(&base, &luma, &opts);
      let dc = compare(&base, &chroma, &opts);
      // At a threshold where luma trips but the lower-weighted chroma delta does
      // not, the luma case differs and the chroma case does not.
      assert_eq!(dl.differing_pixels, 1, "luma shift exceeds threshold");
      assert_eq!(dc.differing_pixels, 0, "chroma-only shift is under-weighted below threshold");
  }
  ```

- [ ] Step — run to verify it fails:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -20
  ```
  Expected: `single_wrong_pixel_survives_every_scale`, `yiq_luminance_outweighs_chroma` FAIL (`differing_pixels == 0`, `max_channel_delta == 0` from the stub); `identity_is_zero_diff` also fails on `total_pixels == 64` (stub returns 0).

- [ ] Step — write the minimal implementation. Replace the `compare` stub body and add the vendored helpers (drop the `let _ = (a, b);`):
  ```rust
  pub fn compare(a: &RgbaImage, b: &RgbaImage, opts: &CompareOpts) -> Diff {
      // Empty: nothing to observe (matches compare_images's 0.0 empty case).
      if a.width() == 0 || a.height() == 0 {
          return Diff {
              differing_pixels: 0,
              max_channel_delta: 0,
              total_pixels: 0,
              mssim: None,
              diff_image: None,
          };
      }
      // Dimension mismatch handled in 1a.4 (saturated Diff). For now assume equal.
      let (w, h) = a.dimensions();
      let total_pixels = w * h;
      let max_delta = 35_215_f64 * opts.threshold * opts.threshold;

      let mut differing_pixels = 0u32;
      let mut max_channel_delta = 0u8;
      for (pa, pb) in a.pixels().zip(b.pixels()) {
          for ch in 0..4 {
              let d = (pa[ch] as i16 - pb[ch] as i16).unsigned_abs() as u8;
              max_channel_delta = max_channel_delta.max(d);
          }
          let delta = color_delta(pa, pb, false);
          if delta.abs() > max_delta {
              // AA exclusion is layered in 1a.3; here every over-threshold pixel counts.
              differing_pixels += 1;
          }
      }

      Diff {
          differing_pixels,
          max_channel_delta,
          total_pixels,
          mssim: None, // wired in 1a.5
          diff_image: None, // wired in 1a.6
      }
  }

  // ---- Vendored from pixelmatch (MIT). Verbatim constants; ported to image 0.25.
  // "Measuring perceived color difference using YIQ NTSC transmission color space"
  // (Kotsarenko & Ramos). `y_only` returns the signed luminance delta (used by the
  // AA sibling test); otherwise the luminance-weighted YIQ squared delta, signed
  // by which pixel is brighter.
  fn color_delta(p1: &image::Rgba<u8>, p2: &image::Rgba<u8>, y_only: bool) -> f64 {
      let (mut r1, mut g1, mut b1, mut a1) =
          (p1[0] as f64, p1[1] as f64, p1[2] as f64, p1[3] as f64);
      let (mut r2, mut g2, mut b2, mut a2) =
          (p2[0] as f64, p2[1] as f64, p2[2] as f64, p2[3] as f64);

      if (a1 - a2).abs() < f64::EPSILON
          && (r1 - r2).abs() < f64::EPSILON
          && (g1 - g2).abs() < f64::EPSILON
          && (b1 - b2).abs() < f64::EPSILON
      {
          return 0.0;
      }
      if a1 < 255.0 {
          a1 /= 255.0;
          r1 = blend(r1, a1);
          g1 = blend(g1, a1);
          b1 = blend(b1, a1);
      }
      if a2 < 255.0 {
          a2 /= 255.0;
          r2 = blend(r2, a2);
          g2 = blend(g2, a2);
          b2 = blend(b2, a2);
      }
      let y1 = rgb2y(r1, g1, b1);
      let y2 = rgb2y(r2, g2, b2);
      let y = y1 - y2;
      if y_only {
          return y;
      }
      let i = rgb2i(r1, g1, b1) - rgb2i(r2, g2, b2);
      let q = rgb2q(r1, g1, b1) - rgb2q(r2, g2, b2);
      let delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;
      if y1 > y2 { -delta } else { delta }
  }

  // blend semi-transparent color with white
  fn blend(c: f64, a: f64) -> f64 {
      255.0 + (c - 255.0) * a
  }
  fn rgb2y(r: f64, g: f64, b: f64) -> f64 {
      r * 0.298_895_31 + g * 0.586_622_47 + b * 0.114_482_23
  }
  fn rgb2i(r: f64, g: f64, b: f64) -> f64 {
      r * 0.595_977_99 - g * 0.274_176_10 - b * 0.321_801_89
  }
  fn rgb2q(r: f64, g: f64, b: f64) -> f64 {
      r * 0.211_470_17 - g * 0.522_617_11 + b * 0.311_146_94
  }
  ```

- [ ] Step — run to verify it passes:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 6 passed` (the three new + the three from 1a.1).

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/src/metric.rs
  git commit -m "feat(verify): vendored YIQ color_delta + two-axis pixel scan

Ports pixelmatch's luminance-weighted YIQ delta (verbatim constants)
and adds the raw L∞ max_channel_delta scan. Single-wrong-pixel is now
caught at N in {16,256,2048} — the §4 dilution regression. AA exclusion
and MSSIM follow.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.3 — Antialias sibling exclusion (the brightest/darkest-neighbor test)

Adds the one feature both naive metrics lack: a differing pixel that is AA in *either* image is excluded from `differing_pixels` unless `include_aa`.

**Files:**
- Modify: `crates/buiy_verify/src/metric.rs` (gate the `differing_pixels += 1` behind `opts.include_aa || (!aa(a,..) && !aa(b,..))`; add private `antialiased` + `has_many_siblings`)
- Test: inline `#[cfg(test)] mod tests`

- [ ] Step — write the failing test. Add inside `mod tests`:
  ```rust
  /// A 1px-wide diagonal AA band: a hard black/white edge whose boundary column
  /// is shifted by one in `b`, producing a sibling-detectable AA pixel.
  fn aa_edge_pair() -> (image::RgbaImage, image::RgbaImage) {
      let (w, h) = (16u32, 16u32);
      let mut a = image::RgbaImage::new(w, h);
      let mut b = image::RgbaImage::new(w, h);
      for y in 0..h {
          for x in 0..w {
              // a: edge at x == y ; b: edge at x == y+1 (shifted one column).
              let pa = if x < y { [0, 0, 0, 255] } else { [255, 255, 255, 255] };
              let pb = if x < y + 1 { [0, 0, 0, 255] } else { [255, 255, 255, 255] };
              a.put_pixel(x, y, image::Rgba(pa));
              b.put_pixel(x, y, image::Rgba(pb));
          }
      }
      (a, b)
  }

  #[test]
  fn aa_pixels_excluded_by_default_but_counted_with_include_aa() {
      let (a, b) = aa_edge_pair();
      let excluded = compare(&a, &b, &CompareOpts { mssim: false, ..Default::default() });
      let counted = compare(&a, &b, &CompareOpts { include_aa: true, mssim: false, ..Default::default() });
      assert_eq!(excluded.differing_pixels, 0, "edge pixels read as AA, excluded");
      assert!(counted.differing_pixels > 0, "include_aa counts the same pixels");
  }

  #[test]
  fn real_defect_is_not_excluded_as_aa() {
      // An isolated wrong pixel on a flat field has no brighter+darker sibling
      // pair, so it is NOT AA — it must still count with default opts.
      let a = solid(16, 16, [0, 0, 0, 255]);
      let mut b = a.clone();
      b.put_pixel(8, 8, image::Rgba([200, 200, 200, 255]));
      let d = compare(&a, &b, &CompareOpts { mssim: false, ..Default::default() });
      assert_eq!(d.differing_pixels, 1, "isolated defect is not AA-excluded");
  }
  ```

- [ ] Step — run to verify it fails:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -20
  ```
  Expected: `aa_pixels_excluded_by_default_but_counted_with_include_aa` FAILS (`excluded.differing_pixels` is `> 0`, not `0`); `real_defect_is_not_excluded_as_aa` passes already (no sibling pair).

- [ ] Step — write the minimal implementation. In `compare`, the AA decision needs each pixel's `(x, y)`, so switch the loop to `enumerate_pixels` and gate the count. Replace the per-pixel loop body:
  ```rust
      let mut differing_pixels = 0u32;
      let mut max_channel_delta = 0u8;
      for (x, y, pa) in a.enumerate_pixels() {
          let pb = b.get_pixel(x, y);
          for ch in 0..4 {
              let d = (pa[ch] as i16 - pb[ch] as i16).unsigned_abs() as u8;
              max_channel_delta = max_channel_delta.max(d);
          }
          let delta = color_delta(pa, pb, false);
          if delta.abs() > max_delta {
              let is_aa = !opts.include_aa
                  && (antialiased(a, x, y, w, h, b) || antialiased(b, x, y, w, h, a));
              if !is_aa {
                  differing_pixels += 1;
              }
          }
      }
  ```
  Then add the two vendored predicates (verbatim port; `image` 0.25 `get_pixel` is inherent on `RgbaImage`):
  ```rust
  // Vendored from pixelmatch (MIT): "Anti-aliased Pixel and Intensity Slope
  // Detector" (Vyšniauskas, 2009). A pixel is AA iff it has a strictly brighter
  // and a strictly darker sibling and that extreme has 3+ equal siblings in BOTH
  // images (so it is an intensity slope, not a real edge in both).
  fn antialiased(img1: &RgbaImage, x: u32, y: u32, w: u32, h: u32, img2: &RgbaImage) -> bool {
      let mut zeroes: u8 = u8::from(x == 0 || y == 0 || x == w - 1 || y == h - 1);
      let (mut min, mut max) = (0.0f64, 0.0f64);
      let (mut min_x, mut min_y, mut max_x, mut max_y) = (0u32, 0u32, 0u32, 0u32);
      let center = img1.get_pixel(x, y);

      let x0 = x.saturating_sub(1);
      let x1 = if x < w - 1 { x + 1 } else { x };
      let y0 = y.saturating_sub(1);
      let y1 = if y < h - 1 { y + 1 } else { y };
      for ax in x0..=x1 {
          for ay in y0..=y1 {
              if ax == x && ay == y {
                  continue;
              }
              let delta = color_delta(center, img1.get_pixel(ax, ay), true);
              if delta == 0.0 {
                  zeroes += 1;
                  if zeroes > 2 {
                      return false;
                  }
                  continue;
              }
              if delta < min {
                  min = delta;
                  min_x = ax;
                  min_y = ay;
                  continue;
              }
              if delta > max {
                  max = delta;
                  max_x = ax;
                  max_y = ay;
              }
          }
      }
      if min == 0.0 || max == 0.0 {
          return false;
      }
      (has_many_siblings(img1, min_x, min_y, w, h) && has_many_siblings(img2, min_x, min_y, w, h))
          || (has_many_siblings(img1, max_x, max_y, w, h)
              && has_many_siblings(img2, max_x, max_y, w, h))
  }

  // Vendored from pixelmatch (MIT): 3+ adjacent pixels of identical color.
  fn has_many_siblings(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> bool {
      let mut zeroes: u8 = u8::from(x == 0 || y == 0 || x == w - 1 || y == h - 1);
      let center = img.get_pixel(x, y);
      let x0 = x.saturating_sub(1);
      let x1 = if x < w - 1 { x + 1 } else { x };
      let y0 = y.saturating_sub(1);
      let y1 = if y < h - 1 { y + 1 } else { y };
      for ax in x0..=x1 {
          for ay in y0..=y1 {
              if ax == x && ay == y {
                  continue;
              }
              if center == img.get_pixel(ax, ay) {
                  zeroes += 1;
                  if zeroes > 2 {
                      return true;
                  }
              }
          }
      }
      false
  }
  ```

- [ ] Step — run to verify it passes:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 8 passed`.

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/src/metric.rs
  git commit -m "feat(verify): antialias sibling exclusion (pixelmatch port)

A differing pixel that is AA in either image is excluded unless
include_aa. EXACT (0,0) now holds across residual AA jitter while still
catching an isolated real defect. Vendored verbatim from pixelmatch.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.4 — `Diff::passes` / `Diff::within` + the saturated dimension-mismatch `Diff`

Adds the gate methods and the loud-red mismatch handling (the spec's explicit replacement for the naive silent `1.0`).

**Files:**
- Modify: `crates/buiy_verify/src/metric.rs` (add `impl Diff { passes, within }`; add the dim-mismatch early return in `compare`)
- Test: inline `#[cfg(test)] mod tests`

- [ ] Step — write the failing tests. Add inside `mod tests`:
  ```rust
  #[test]
  fn passes_requires_both_axes() {
      // One pixel off by 255: trips max_channel_delta, one differing pixel.
      let a = solid(8, 8, [0, 0, 0, 255]);
      let mut b = a.clone();
      b.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
      let d = compare(&a, &b, &CompareOpts { mssim: false, ..Default::default() });
      assert!(!d.passes(&FuzzBudget::EXACT), "EXACT rejects any diff");
      assert!(!d.passes(&FuzzBudget { max_channel_delta: 255, max_diff_pixels: 0 }),
          "diff-pixel axis still binds when channel axis is satisfied");
      assert!(!d.passes(&FuzzBudget { max_channel_delta: 0, max_diff_pixels: 1 }),
          "channel axis still binds when diff-pixel axis is satisfied");
      assert!(d.passes(&FuzzBudget { max_channel_delta: 255, max_diff_pixels: 1 }),
          "both axes satisfied -> pass");
  }

  #[test]
  fn within_floor_catches_unexpectedly_clean() {
      // A clean render (0,0) must FAIL a widened budget whose min floor is > 0.
      let a = solid(8, 8, [5, 5, 5, 255]);
      let clean = compare(&a, &a, &CompareOpts { mssim: false, ..Default::default() });
      let min = FuzzBudget { max_channel_delta: 1, max_diff_pixels: 1 };
      let max = FuzzBudget { max_channel_delta: 10, max_diff_pixels: 50 };
      assert!(!clean.within(&min, &max), "a clean render is below the expected floor");
  }

  #[test]
  fn dimension_mismatch_is_saturated_and_fails_every_budget() {
      let a = solid(4, 4, [0, 0, 0, 255]);
      let b = solid(5, 4, [0, 0, 0, 255]);
      let d = compare(&a, &b, &CompareOpts::default());
      assert_eq!(d.max_channel_delta, 255);
      assert_eq!(d.differing_pixels, d.total_pixels);
      assert_eq!(d.total_pixels, 20, "total = max(area) = 5*4");
      assert_eq!(d.mssim, Some(0.0));
      // Fails even a hypothetical maximal budget.
      let maximal = FuzzBudget { max_channel_delta: 255, max_diff_pixels: u32::MAX };
      assert!(!d.passes(&maximal), "saturated diff fails the loudest budget too");
  }

  #[test]
  fn empty_capture_forbidden_by_explicit_assertion() {
      // The metric returns total_pixels == 0 for empty; harnesses forbid it.
      let e = image::RgbaImage::new(0, 0);
      let d = compare(&e, &e, &CompareOpts::default());
      assert_eq!(d.total_pixels, 0);
  }
  ```

- [ ] Step — run to verify it fails:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -20
  ```
  Expected: `passes_requires_both_axes`, `within_floor_catches_unexpectedly_clean` FAIL to compile (no `passes`/`within`); `dimension_mismatch_is_saturated_and_fails_every_budget` FAILS at runtime once the others compile (current `compare` assumes equal dims).

- [ ] Step — write the minimal implementation. Add the dim-mismatch early return at the top of `compare`, right after the empty guard:
  ```rust
      if a.dimensions() != b.dimensions() {
          // Loud-red sentinel (metric.md): a saturated Diff fails EVERY budget.
          // total = max(area) so the saturation count is well-defined.
          let total = a.width().saturating_mul(a.height())
              .max(b.width().saturating_mul(b.height()));
          return Diff {
              differing_pixels: total,
              max_channel_delta: 255,
              total_pixels: total,
              mssim: Some(0.0),
              diff_image: None,
          };
      }
  ```
  And add the `impl Diff`:
  ```rust
  impl Diff {
      /// PASS iff `max_channel_delta <= budget.max_channel_delta`
      /// AND `differing_pixels <= budget.max_diff_pixels`. MSSIM is advisory and
      /// never gates here.
      pub fn passes(&self, budget: &FuzzBudget) -> bool {
          self.max_channel_delta <= budget.max_channel_delta
              && self.differing_pixels <= budget.max_diff_pixels
      }

      /// Mozilla `fuzzy-if` "ranges must not include 0": PASS iff the diff meets
      /// the `max` budget AND exceeds the `min` floor on at least one axis, so a
      /// suddenly-clean render (below an expected difference) is flagged.
      pub fn within(&self, min: &FuzzBudget, max: &FuzzBudget) -> bool {
          let over_floor = self.max_channel_delta > min.max_channel_delta
              || self.differing_pixels > min.max_diff_pixels;
          self.passes(max) && over_floor
      }
  }
  ```

- [ ] Step — run to verify it passes:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 12 passed`.

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/src/metric.rs
  git commit -m "feat(verify): Diff::passes/within + saturated dim-mismatch Diff

Two-axis gate (both bind); within() pins the fuzzy-if floor so an
unexpectedly-clean render reds. A dimension mismatch folds into a
saturated Diff that fails EVERY budget — the loud-red replacement for
the naive silent 1.0.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.5 — Advisory MSSIM channel (`image-compare`), never gating

Wires the secondary advisory channel and proves it never participates in `passes`.

**Files:**
- Modify: `crates/buiy_verify/src/metric.rs` (compute `mssim` when `opts.mssim` for the equal-dims path)
- Test: inline `#[cfg(test)] mod tests`

- [ ] Step — write the failing tests. Add inside `mod tests`:
  ```rust
  #[test]
  fn identity_reports_full_mssim() {
      let img = solid(16, 16, [40, 90, 160, 255]);
      let d = compare(&img, &img, &CompareOpts::default()); // mssim on by default
      assert_eq!(d.differing_pixels, 0);
      let s = d.mssim.expect("mssim computed when opts.mssim");
      assert!(s > 0.999, "identical images report MSSIM ~1.0, got {s}");
  }

  #[test]
  fn mssim_skipped_when_disabled() {
      let img = solid(8, 8, [1, 2, 3, 255]);
      let d = compare(&img, &img, &CompareOpts { mssim: false, ..Default::default() });
      assert_eq!(d.mssim, None);
  }

  #[test]
  fn mssim_never_gates() {
      // A global 1-LSB wash: 0 differing pixels (under YIQ threshold) but a
      // measurably-below-1 MSSIM. passes(&EXACT) must still hold.
      let a = solid(32, 32, [128, 128, 128, 255]);
      let b = solid(32, 32, [129, 129, 129, 255]);
      let d = compare(&a, &b, &CompareOpts::default());
      assert_eq!(d.differing_pixels, 0, "1-LSB shift is under the YIQ threshold");
      assert!(d.passes(&FuzzBudget::EXACT), "MSSIM is advisory — never gates passes()");
  }
  ```

- [ ] Step — run to verify it fails:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -20
  ```
  Expected: `identity_reports_full_mssim` FAILS (`mssim` is `None`); the other two pass (current behavior matches).

- [ ] Step — write the minimal implementation. After the per-pixel loop, before constructing the returned `Diff`, compute `mssim`:
  ```rust
      let mssim = if opts.mssim {
          // Advisory MSSIM via image-compare's rgba blended hybrid compare,
          // premultiplied against an opaque (white) background — captures are
          // opaque, so the background is never sampled in practice.
          use image_compare::{rgba_blended_hybrid_compare, BlendInput};
          let bg = image::Rgb([255u8, 255, 255]);
          rgba_blended_hybrid_compare(BlendInput::from(a), BlendInput::from(b), bg)
              .map(|sim| sim.score)
              .ok()
      } else {
          None
      };
  ```
  Then use `mssim` in the returned `Diff` (replace the `mssim: None, // wired in 1a.5` line with `mssim,`).
  *(Verify the exact `image-compare` API surface — `rgba_blended_hybrid_compare` / `BlendInput::from` / `Similarity::score` — against the resolved 0.5.0 crate docs at impl time; substitute the correct symbol if the 0.5.x API differs, keeping the `Option<f64>` contract.)*

- [ ] Step — run to verify it passes:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 15 passed`.

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/src/metric.rs
  git commit -m "feat(verify): advisory MSSIM channel via image-compare

Diff::mssim from rgba_blended_hybrid_compare, Option (None when
disabled/errored — never silently 0.0). Proven non-gating: a 1-LSB wash
(0 differing pixels) still passes EXACT despite sub-1 MSSIM.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.6 — `diff_image` heatmap on `emit_diff_image`

Fills the optional triage heatmap (consumed by tier-5 golden HTML in Phase 3). AA pixels yellow, differing pixels red — the pixelmatch palette.

**Files:**
- Modify: `crates/buiy_verify/src/metric.rs` (allocate + paint `diff_image` when `opts.emit_diff_image`)
- Test: inline `#[cfg(test)] mod tests`

- [ ] Step — write the failing test. Add inside `mod tests`:
  ```rust
  #[test]
  fn diff_image_paints_differing_pixels() {
      let a = solid(8, 8, [0, 0, 0, 255]);
      let mut b = a.clone();
      b.put_pixel(3, 3, image::Rgba([255, 255, 255, 255]));
      let d = compare(&a, &b, &CompareOpts { emit_diff_image: true, mssim: false, ..Default::default() });
      let img = d.diff_image.expect("emit_diff_image fills the heatmap");
      assert_eq!(img.dimensions(), (8, 8));
      // The differing pixel is painted red (pixelmatch diff_color).
      assert_eq!(*img.get_pixel(3, 3), image::Rgba([255, 0, 0, 255]));
  }

  #[test]
  fn diff_image_absent_by_default() {
      let a = solid(4, 4, [10, 10, 10, 255]);
      let d = compare(&a, &a, &CompareOpts::default());
      assert!(d.diff_image.is_none());
  }
  ```

- [ ] Step — run to verify it fails:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -20
  ```
  Expected: `diff_image_paints_differing_pixels` FAILS (`diff_image` is `None`).

- [ ] Step — write the minimal implementation. Allocate the heatmap before the loop and paint inside it; wire it into the returned `Diff`. Add before the per-pixel loop:
  ```rust
      let mut diff_image = opts.emit_diff_image.then(|| RgbaImage::new(w, h));
  ```
  Inside the loop, in the over-threshold branch, paint by AA/real classification (replace the `if !is_aa { differing_pixels += 1; }` block):
  ```rust
          if delta.abs() > max_delta {
              let is_aa = !opts.include_aa
                  && (antialiased(a, x, y, w, h, b) || antialiased(b, x, y, w, h, a));
              if is_aa {
                  if let Some(out) = &mut diff_image {
                      out.put_pixel(x, y, image::Rgba([255, 255, 0, 255])); // AA: yellow
                  }
              } else {
                  differing_pixels += 1;
                  if let Some(out) = &mut diff_image {
                      out.put_pixel(x, y, image::Rgba([255, 0, 0, 255])); // diff: red
                  }
              }
          }
  ```
  Then set `diff_image` in the returned `Diff` (replace `diff_image: None, // wired in 1a.6` with `diff_image,`).

- [ ] Step — run to verify it passes:
  ```sh
  cargo test -p buiy_verify --lib metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 17 passed`.

- [ ] Step — run the full crate clippy (catch any warnings the gate would reject):
  ```sh
  cargo clippy -p buiy_verify --all-targets -- -D warnings 2>&1 | tail -8
  ```
  Expected: `Finished`, no warnings.

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/src/metric.rs
  git commit -m "feat(verify): diff_image heatmap on emit_diff_image

pixelmatch palette: differing pixels red, AA pixels yellow. Off in the
hot reftest path; on for tier-5 golden triage HTML.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.7 — Promote the known-answer suite into `tests/metric.rs` + the constants-pin

metric.md § Verification specifies the meta-tests live in `crates/buiy_verify/tests/metric.rs` (integration tier), and a checked-in 8×8 pair + its expected `Diff` guards the vendored constants against drift. The inline `src` tests stay (they exercise private helpers); this task adds the public-surface integration suite and the constants tripwire.

> **Note — `insta` is available (Phase 0.1) but the snapshot upgrade is deferred.** metric.md's constants-pin uses a floats-redacted `insta` snapshot. To keep this task self-contained and the tripwire un-vacuous, the constants pin here is a **plain `assert_eq!` on the exact integer `Diff` fields** (`mssim` asserted with tolerance, not snapshotted). Phase 2 (which introduces the snapshot dump infra) converts this to the redacted `insta` snapshot metric.md calls for; the assertion form is behavior-identical and cannot pass vacuously.

**Files:**
- Create: `crates/buiy_verify/tests/metric.rs`
- Test: itself (integration test)

- [ ] Step — write the integration suite (RED: `tests/metric.rs` does not exist):
  ```rust
  //! Known-answer meta-tests for `buiy_verify::metric` (metric.md § Verification).
  //! Pure CPU, no GPU lane.

  use buiy_verify::metric::{compare, CompareOpts, Diff, FuzzBudget};
  use image::{Rgba, RgbaImage};

  fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
      RgbaImage::from_pixel(w, h, Rgba(px))
  }

  #[test]
  fn identity_zero_diff_full_mssim() {
      let img = solid(8, 8, [12, 34, 56, 255]);
      let d = compare(&img, &img, &CompareOpts::default());
      assert_eq!(d.differing_pixels, 0);
      assert_eq!(d.max_channel_delta, 0);
      assert!(d.mssim.unwrap() > 0.999);
      assert!(d.passes(&FuzzBudget::EXACT));
  }

  #[test]
  fn single_defect_survives_scale() {
      for n in [16u32, 256, 2048] {
          let a = solid(n, n, [0, 0, 0, 255]);
          let mut b = a.clone();
          b.put_pixel(n / 2, n / 2, Rgba([200, 200, 200, 255]));
          let d = compare(&a, &b, &CompareOpts { include_aa: true, mssim: false, ..Default::default() });
          assert_eq!(d.differing_pixels, 1, "N={n}");
          assert!(!d.passes(&FuzzBudget::EXACT), "N={n}");
      }
  }

  #[test]
  fn dimension_mismatch_fails_every_budget() {
      let a = solid(4, 4, [0, 0, 0, 255]);
      let b = solid(4, 5, [0, 0, 0, 255]);
      let d = compare(&a, &b, &CompareOpts::default());
      assert_eq!(d.differing_pixels, d.total_pixels);
      assert_eq!(d.max_channel_delta, 255);
      assert!(!d.passes(&FuzzBudget { max_channel_delta: 255, max_diff_pixels: u32::MAX }));
  }

  /// Constants tripwire: a fixed 8×8 pair yields an exact integer Diff. A
  /// pixelmatch-constant drift changes these numbers and reds this test. (Phase 2
  /// upgrades this to the floats-redacted insta snapshot metric.md specifies.)
  #[test]
  fn vendored_constants_are_pinned() {
      let mut a = solid(8, 8, [0, 0, 0, 255]);
      let mut b = solid(8, 8, [0, 0, 0, 255]);
      // Three deterministic, isolated, non-AA defects of known magnitude.
      a.put_pixel(1, 1, Rgba([0, 0, 0, 255]));
      b.put_pixel(1, 1, Rgba([255, 0, 0, 255])); // luma-heavy
      a.put_pixel(4, 4, Rgba([0, 0, 0, 255]));
      b.put_pixel(4, 4, Rgba([0, 255, 0, 255]));
      a.put_pixel(6, 2, Rgba([10, 10, 10, 255]));
      b.put_pixel(6, 2, Rgba([250, 250, 250, 255]));
      let d = compare(&a, &b, &CompareOpts { mssim: false, ..Default::default() });
      // EXPECTED: re-bless intentionally if the algorithm changes.
      let Diff { differing_pixels, max_channel_delta, total_pixels, .. } = d;
      assert_eq!(
          (differing_pixels, max_channel_delta, total_pixels),
          (3, 255, 64),
          "vendored YIQ/AA constants drifted — re-derive deliberately, do not patch the number",
      );
  }
  ```

- [ ] Step — run to verify it fails / confirm the tuple:
  ```sh
  cargo test -p buiy_verify --test metric 2>&1 | tail -20
  ```
  Expected: compiles and runs; **if** the `(3, 255, 64)` tuple does not match the real output, the test FAILS and prints the actual tuple. (This is the one task whose expected literal must be confirmed against the real run — see next step.)

- [ ] Step — bless the pinned tuple. If `vendored_constants_are_pinned` failed only on the tuple value, read the actual `(differing_pixels, max_channel_delta, total_pixels)` from the failure message and replace `(3, 255, 64)` with it (the three defects are isolated and non-AA by construction, so `differing_pixels` should be `3` and `max_channel_delta` `255`; only confirm). Re-run:
  ```sh
  cargo test -p buiy_verify --test metric 2>&1 | tail -12
  ```
  Expected: `test result: ok. 4 passed`.

- [ ] Step — run the headless gate slice for the crate:
  ```sh
  cargo fmt -p buiy_verify -- --check && cargo clippy -p buiy_verify --all-targets -- -D warnings && cargo test -p buiy_verify 2>&1 | tail -15
  ```
  Expected: all green; `metric` (lib) 17 passed, `metric` (test) 4 passed, plus the existing `visual`/`smoke`/`a11y`/`contrast` suites (still present — deleted next task).

- [ ] Step — commit:
  ```sh
  git add crates/buiy_verify/tests/metric.rs
  git commit -m "test(verify): known-answer meta-suite + constants tripwire for metric

metric.md § Verification: identity, scale-invariant single defect,
saturated dim-mismatch, and an exact-integer constants pin guarding the
vendored YIQ/AA numbers. (insta-snapshot upgrade deferred to Phase 2.)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.8 — Delete the RMSE metric (`visual.rs`) + migrate its 4 callers + the smoke symbol

metric.md § Migration step 1: `buiy_verify::visual::compare_images` (RMSE) is deleted; its 4 callers in `tests/visual.rs` move to `metric::compare` + `Diff::passes`; the 5th reference (`smoke.rs`) is deleted.

**Files:**
- Delete: `crates/buiy_verify/src/visual.rs`
- Modify: `crates/buiy_verify/src/lib.rs` (drop `pub mod visual;`)
- Modify: `crates/buiy_verify/tests/visual.rs` (rewrite the 4 tests onto `metric`)
- Modify: `crates/buiy_verify/tests/smoke.rs` (drop the `visual` import + `visual::compare_images` line)
- Delete: `crates/buiy_verify/tests/fixtures/visual/baseline.png`, `.../tinted.png` (the new tests are in-memory)

- [ ] Step — rewrite the migrated callers FIRST (RED via deleted symbol). Replace the entire contents of `crates/buiy_verify/tests/visual.rs`:
  ```rust
  //! Migrated from the deleted RMSE `visual::compare_images` to the unified
  //! `buiy_verify::metric` (metric.md § Migration). In-memory fixtures; the old
  //! baseline/tinted PNGs are gone.

  use buiy_verify::metric::{compare, CompareOpts, FuzzBudget};
  use image::{Rgba, RgbaImage};

  fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
      RgbaImage::from_pixel(w, h, Rgba(px))
  }

  #[test]
  fn identical_images_pass_exact() {
      let img = solid(16, 16, [30, 60, 90, 255]);
      let d = compare(&img, &img, &CompareOpts::default());
      assert_eq!(d.differing_pixels, 0);
      assert!(d.passes(&FuzzBudget::EXACT), "identical images pass the exact budget");
  }

  #[test]
  fn tinted_image_fails_exact() {
      let a = solid(16, 16, [40, 40, 40, 255]);
      let b = solid(16, 16, [40, 40, 200, 255]); // uniform blue tint
      let d = compare(&a, &b, &CompareOpts { include_aa: true, ..Default::default() });
      assert!(d.differing_pixels > 0, "a uniform tint differs");
      assert!(!d.passes(&FuzzBudget::EXACT), "tinted image fails the exact budget");
  }

  #[test]
  fn dimension_mismatch_fails_every_budget() {
      let a = solid(2, 2, [0, 0, 0, 255]);
      let b = solid(3, 2, [0, 0, 0, 255]);
      let d = compare(&a, &b, &CompareOpts::default());
      assert!(!d.passes(&FuzzBudget { max_channel_delta: 255, max_diff_pixels: u32::MAX }),
          "mismatched dims saturate and fail even a maximal budget");
  }

  #[test]
  fn empty_vs_empty_is_zero_diff() {
      let e = RgbaImage::new(0, 0);
      let d = compare(&e, &e, &CompareOpts::default());
      assert_eq!(d.total_pixels, 0);
      assert!(d.passes(&FuzzBudget::EXACT), "empty-vs-empty observes no difference");
  }
  ```

- [ ] Step — rewrite `crates/buiy_verify/tests/smoke.rs` (drop the deleted symbol):
  ```rust
  #[test]
  fn re_exports_compile() {
      use buiy_verify::{a11y, contrast, metric};
      let _ = metric::compare;
      let _ = a11y::snapshot_tree;
      let _ = contrast::wcag2_ratio;
  }
  ```
  *(Confirm the exact `a11y`/`contrast` symbol names against the live crate at impl time — `snapshot_tree`/`wcag2_ratio` are placeholders for whatever the smoke test references today.)*

- [ ] Step — delete the module and its re-export:
  ```sh
  git rm crates/buiy_verify/src/visual.rs crates/buiy_verify/tests/fixtures/visual/baseline.png crates/buiy_verify/tests/fixtures/visual/tinted.png
  ```
  Then edit `crates/buiy_verify/src/lib.rs` so the module block reads:
  ```rust
  pub mod a11y;
  pub mod contrast;
  pub mod metric;
  ```
  (the lib doc line mentioning "visual regression" also needs updating to "perceptual metric" so the doc gate's wording stays honest.)

- [ ] Step — run to verify the migration compiles and passes (RED→GREEN: the deleted symbol forced the rewrite):
  ```sh
  cargo test -p buiy_verify 2>&1 | tail -20
  grep -rn "compare_images\|DiffResult\|mod visual\|::visual" crates/buiy_verify/ 2>&1
  ```
  Expected: all green; `visual` test now 4 passed (migrated), `smoke` 1 passed; the grep returns no matches.

- [ ] Step — commit:
  ```sh
  git add -A crates/buiy_verify
  git commit -m "refactor(verify): delete RMSE visual::compare_images, migrate callers to metric

metric.md § Migration step 1: the RMSE metric and DiffResult are gone;
tests/visual.rs and smoke.rs move onto metric::compare + Diff::passes
(in-memory fixtures replace baseline/tinted PNGs). One metric now.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.9 — Deprecate `buiy_core::render::golden::perceptual_diff` (keep body)

metric.md § Migration step 2: `perceptual_diff` is **deprecated in place** (`buiy_core` cannot depend on `buiy_verify` in its normal graph). Its L1 body stays so the `#[ignore]` GPU re-capture tests still link until they migrate (1a.10 migrates the `text_gpu.rs` subset; the rest defer to Phase 3 goldens).

**Files:**
- Modify: `crates/buiy_core/src/render/golden.rs` (add `#[deprecated]` above `pub fn perceptual_diff`)
- Modify: `crates/buiy_core/src/lib.rs` (the `pub use` re-export — `#[allow(deprecated)]`)
- Modify: each remaining in-crate `perceptual_diff` caller test file to `#![allow(deprecated)]`

- [ ] Step — confirm the full caller set the deprecation will warn:
  ```sh
  grep -rln "perceptual_diff" crates/buiy_core/tests/ crates/buiy_core/src/
  ```
  Expected files: `tests/render_golden_harness.rs`, `tests/text_gpu.rs`, `tests/text_decoration_gpu.rs`, `tests/text_golden_suite_gpu.rs`, `tests/text_selection_caret_gpu.rs`, and `src/lib.rs` (the re-export). (1a.10 removes `text_gpu.rs` from this list.)

- [ ] Step — write the change. Add the attribute above `pub fn perceptual_diff`:
  ```rust
  #[deprecated(note = "use buiy_verify::metric::compare; kept only for unmigrated #[ignore] GPU re-capture tests")]
  pub fn perceptual_diff(a: &[u8], b: &[u8]) -> f32 {
  ```
  At the `src/lib.rs` re-export, suppress the deprecation locally so the prod build stays `-D warnings`-clean:
  ```rust
  #[allow(deprecated)]
  pub use render::golden::{GoldenConfig, perceptual_diff};
  ```
  In each remaining caller test file (`render_golden_harness.rs`, `text_decoration_gpu.rs`, `text_golden_suite_gpu.rs`, `text_selection_caret_gpu.rs` — NOT `text_gpu.rs`, migrated next), add at the top of the file:
  ```rust
  #![allow(deprecated)] // perceptual_diff is deprecated; these GPU sites migrate to buiy_verify::metric in Phase 3 (tier-5 goldens).
  ```

- [ ] Step — run the headless gate slice for `buiy_core`:
  ```sh
  cargo clippy -p buiy_core --all-targets -- -D warnings 2>&1 | tail -10
  ```
  Expected: `Finished`, no `use of deprecated function` warnings (each site is `allow`-gated or migrated next task).

- [ ] Step — commit:
  ```sh
  git add crates/buiy_core/src/render/golden.rs crates/buiy_core/src/lib.rs crates/buiy_core/tests/render_golden_harness.rs crates/buiy_core/tests/text_decoration_gpu.rs crates/buiy_core/tests/text_golden_suite_gpu.rs crates/buiy_core/tests/text_selection_caret_gpu.rs
  git commit -m "refactor(core): deprecate perceptual_diff in place

metric.md § Migration step 2: buiy_core cannot depend on buiy_verify in
its normal graph, so perceptual_diff carries a #[deprecated] gravestone
pointing at buiy_verify::metric::compare; its L1 body stays for the
unmigrated #[ignore] GPU re-capture tests (Phase 3). Callers gain a
file-level allow(deprecated) until they migrate.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.10 — Migrate the `text_gpu.rs` re-capture / anti-test sites onto `metric::compare`

metric.md § "Re-capture determinism / anti-tests": the `text_gpu.rs` sites are **not** goldens — they diff two in-process captures. They migrate now onto `metric::compare` via the dev-dependency cycle (**already landed in Phase 0.2** — `buiy_core → buiy_verify` under `[dev-dependencies]`). The stable (`<`) sites become `passes(&EXACT)`; the two named anti-tests (`> 5e-4` at `:152`, `> 1e-4` at `:271`) become `!compare(..).passes(&EXACT)` via an `assert_differs` helper. The `text_golden_suite_gpu.rs` / `text_decoration_gpu.rs` / `text_selection_caret_gpu.rs` stored-baseline-shaped sites stay on deprecated `perceptual_diff` until Phase 3's `assert_golden` (per the 1a.9 `allow`).

> **GPU lane.** These are `#[ignore]` tests. They compile under the headless gate but only RUN on the GPU lane (`cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`), which this host (RX 6700 XT) can do. The bridging wrinkle: `readback_rgba` returns `Vec<u8>` raw RGBA; `metric::compare` wants `&RgbaImage`. A test-local `img(&[u8]) -> RgbaImage` wraps the bytes at the known `W`/`H` (`= 128`/`64`).

**Files:**
- Modify: `crates/buiy_core/tests/text_gpu.rs` (imports; add `img` + `assert_differs` helpers; rewrite the 5 stable `<` sites + 2 `>` anti-tests; drop the file from the deprecated set so no `allow` is needed)

- [ ] Step — confirm the dev-dependency edge from Phase 0.2 is present (no re-add):
  ```sh
  grep -n "buiy_verify" crates/buiy_core/Cargo.toml && cargo build -p buiy_core --tests 2>&1 | tail -5
  ```
  Expected: the `buiy_verify = { path = "../buiy_verify" }` line is under `[dev-dependencies]`; `Finished` (no cyclic-dependency error). If the line is absent, add it per Phase 0.2 in this step.

- [ ] Step — write the migration. In `crates/buiy_core/tests/text_gpu.rs`, change the import line from:
  ```rust
  use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
  ```
  to:
  ```rust
  use buiy_core::render::golden::GoldenConfig;
  use buiy_verify::metric::{compare, CompareOpts, FuzzBudget};
  ```
  Add the two helpers near the `W`/`H` consts:
  ```rust
  /// Wrap a raw RGBA readback (W×H) as an `RgbaImage` for `metric::compare`.
  fn img(bytes: &[u8]) -> image::RgbaImage {
      image::RgbaImage::from_raw(W, H, bytes.to_vec())
          .expect("readback length == W*H*4")
  }

  /// The anti-test spelling: two captures must NOT match at the exact budget —
  /// proof the input change actually moved pixels (metric.md § anti-tests).
  fn assert_differs(a: &[u8], b: &[u8], msg: &str) {
      let d = compare(&img(a), &img(b), &CompareOpts::default());
      assert!(!d.passes(&FuzzBudget::EXACT), "{msg}");
  }
  ```
  Then rewrite each site (the line numbers are the pre-migration positions; locate by the surrounding assert):
  - `:114` stable — `perceptual_diff(&frame_a, &frame_b) < 1e-4` → `compare(&img(&frame_a), &img(&frame_b), &CompareOpts::default()).passes(&FuzzBudget::EXACT)` with message "two fresh captures diverged (must be bit-exact within the pinned rasterizer)".
  - `:152` anti-test — `assert!(perceptual_diff(..) > 5e-4, ..)` → `assert_differs(&frame_a, &frame_b, "the retint is visible in the framebuffer (byte-identity is not vacuous)")`.
  - `:216` stable — `perceptual_diff(..) < 1e-4` → the `passes(&EXACT)` form, message "retained frames render identically".
  - `:271` anti-test — `assert!(perceptual_diff(&frame_a, &frame_c) > 1e-4, ..)` → `assert_differs(&frame_a, &frame_c, "stale UVs sampled the filler — the silent corruption § 6.3's un-gated touch pass exists to prevent")`.
  - `:359` stable — the `passes(&EXACT)` form, message "two independent captures are byte-stable (deterministic fonts + resolver)".
  - `:452` stable — the `passes(&EXACT)` form, message "the storm is invisible: same bytes, same shaping, same pixels".
  - `:544` stable — the `passes(&EXACT)` form, dropping the `{diff}` interpolation.

- [ ] Step — verify the file compiles under the headless gate (the `#[ignore]` bodies must build even though they won't run here):
  ```sh
  cargo test -p buiy_core --test text_gpu --no-run 2>&1 | tail -10
  grep -n "perceptual_diff" crates/buiy_core/tests/text_gpu.rs 2>&1
  ```
  Expected: `Finished` / `Executable …`; the grep returns no matches (so the file needs no `allow(deprecated)`).

- [ ] Step — RUN the migrated tests on the GPU lane (this host has the adapter) to prove behavior is preserved:
  ```sh
  cargo test -p buiy_core --test text_gpu -j 2 -- --ignored --test-threads=1 2>&1 | tail -25
  ```
  Expected: all previously-passing `text_gpu` `#[ignore]` tests still pass; the two anti-test owners (`retint_real_text_leaves_atlas_byte_identical`, `touch_pass_prevents_stale_uv_corruption`) pass. If any stable site now FAILS at `EXACT` where it passed at `< 1e-4`, that is a real finding — the old L1 tolerance was masking sub-threshold drift — investigate per `systematic-debugging`; do **not** widen the budget without a documented reason.

- [ ] Step — commit:
  ```sh
  git add crates/buiy_core/tests/text_gpu.rs
  git commit -m "refactor(core): migrate text_gpu re-capture/anti-tests to metric::compare

The #[ignore] GPU re-capture tests reach the unified metric over the
dev-only buiy_core -> buiy_verify edge (landed Phase 0.2). Stable
re-capture sites -> passes(&EXACT); the must-differ anti-tests (:152,
:271) -> !passes(&EXACT) via assert_differs. Verified on the RX 6700 XT
GPU lane. The stored-baseline sites in the other text_*_gpu.rs files
stay on deprecated perceptual_diff until Phase 3.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1a.11 — Phase 1a gate: full headless gate + supply-chain + GPU lane

Final verification that Phase 1a is gate-clean before Phase 1b (reftests) builds on `metric`. Verification-only — no commit.

- [ ] Step — run the full project gate (the "all checks" command):
  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace 2>&1 | tail -30
  ```
  Expected: all green. The `buiy_verify` `metric` lib + integration suites pass; the migrated `visual`/`smoke` pass; `buiy_core` builds with the deprecation + dev-edge; no `-D warnings` violations.

- [ ] Step — re-run the supply-chain audit:
  ```sh
  cargo deny check 2>&1 | tail -12
  ```
  Expected: `advisories ok`, `licenses ok`, `bans ok`, `sources ok`.

- [ ] Step — run the GPU lane (additive) to confirm the migrated `text_gpu.rs` and the still-deprecated re-capture suites all pass together:
  ```sh
  cargo test -p buiy_core -j 2 -- --ignored --test-threads=1 2>&1 | tail -30
  ```
  Expected: the full `#[ignore]` GPU suite passes.

- [ ] Step — no commit. If the gate surfaced any warning or failure, root-cause it per `systematic-debugging` and fix in a follow-up task before declaring Phase 1a done.

---

**Phase 1a exit criteria:** the headless gate is green with the unified `metric` (17 lib + 4 integration meta-tests), the RMSE metric deleted and its callers migrated, `perceptual_diff` deprecated-in-place with every kept caller `allow`-gated; the `text_gpu.rs` re-capture/anti-test sites run bit-exact on the GPU lane; `cargo deny check` clean. Phase 1b (reftests) now has `metric::compare`/`Diff`/`FuzzBudget`/`CompareOpts` to build on.

---

## Phase 1b — Reftest harness + CPU/GPU cross-check

Realizes [`reftests.md`](../specs/2026-06-15-buiy-verification-design/reftests.md) (Tier 4 + Tier 4.5). Builds `crates/buiy_verify/src/reftest.rs` (new module): `RefCase`/`RefKind`/`RefOutcome`, the `reftest!` macro, `run_reftest` (two captures in one app via `buiy_core::render::golden::capture_to_image`, diffed by `buiy_verify::metric::compare`), the reference-independence structural lint, the CPU-vs-GPU SDF cross-check, and at least two real reftest cases.

**Depends on:** Phase 0 (the `Dpr` type, the promoted `capture_to_image`, the dev-dep edge) **and** Phase 1a (`buiy_verify::metric` — `Diff`, `FuzzBudget`, `FuzzBudget::EXACT`, `CompareOpts`, `compare`, `Diff::passes`). Phase 1b's pure-CPU meta-tests run in the headless gate; the GPU reftest cases, the SDF cross-check, and the known-good/known-bad pairs are `#[ignore]` and run on the GPU lane.

> **Scope note — single-reference only in v1.** `reftests.md` § "Reference independence" #3 specs a `RefCase::multi` / `reference: &[fn]` multiple-references aggregation (`Match` = OR over references, `Mismatch` = AND). Phase 1b builds the **single-reference** `RefCase` (`reference: fn(&mut App)`) only — it covers both real cases and the cross-check. Multiple-references is a **deferred follow-up** (see Self-review § gaps); the `evaluate_outcome` split keeps the aggregation logic addable without reworking the engine.

---

### Task 1b.1 — Add `CompareOpts::reftest_default()` to the metric

The reftest path needs AA-exclusion on, MSSIM kept advisory, and `emit_diff_image` off in the hot loop. A thin constructor on the already-landed `CompareOpts`.

**Files:**
- Modify: `crates/buiy_verify/src/metric.rs` (add an `impl CompareOpts` block after the `Default` impl)
- Test: `crates/buiy_verify/tests/metric.rs` (append)

Steps:

- [ ] **Write the failing test.** Append to `crates/buiy_verify/tests/metric.rs`:
  ```rust
  #[test]
  fn reftest_default_excludes_aa_and_skips_diff_image() {
      let opts = buiy_verify::metric::CompareOpts::reftest_default();
      assert!(!opts.include_aa, "reftest excludes AA-sibling pixels");
      assert!(opts.mssim, "MSSIM stays computed (advisory)");
      assert!(!opts.emit_diff_image, "hot reftest path allocates no diff image");
      assert_eq!(opts.threshold, 0.1, "pixelmatch default sensitivity");
  }
  ```

- [ ] **Run to verify it fails.**
  ```sh
  cargo test -p buiy_verify --test metric reftest_default_excludes_aa_and_skips_diff_image
  ```
  Expected: compile error `no function or associated item named 'reftest_default' found for struct 'CompareOpts'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/metric.rs`, after the `impl Default for CompareOpts` block, add:
  ```rust
  impl CompareOpts {
      /// The reftest-tier options: AA-sibling pixels excluded (two CSS-subset
      /// code paths can legitimately differ by one AA pixel on a shared corner),
      /// MSSIM advisory-on, and no diff-image allocation in the hot capture loop
      /// (the report is emitted with `emit_diff_image` only on failure).
      pub fn reftest_default() -> Self {
          Self {
              threshold: 0.1,
              include_aa: false,
              mssim: true,
              emit_diff_image: false,
          }
      }
  }
  ```

- [ ] **Run to verify it passes.**
  ```sh
  cargo test -p buiy_verify --test metric reftest_default_excludes_aa_and_skips_diff_image
  ```
  Expected: `test result: ok. 1 passed`.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/metric.rs crates/buiy_verify/tests/metric.rs
  git commit -m "feat(verify): add CompareOpts::reftest_default for tier-4

AA-exclusion on, MSSIM advisory, no diff-image alloc in the hot path —
the options run_reftest passes to metric::compare (reftests.md § API).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.2 — `RefKind` enum + `reftest_kind` parser + module skeleton

The macro stringifies its kind token (`match`/`mismatch`) into a `RefKind`. Land the enum and the `&str → RefKind` constructor, with unit tests. Start the new module file.

**Files:**
- Create: `crates/buiy_verify/src/reftest.rs`
- Modify: `crates/buiy_verify/src/lib.rs` (add `pub mod reftest;`)
- Test: inline `#[cfg(test)]` in `reftest.rs`

Steps:

- [ ] **Create the module file with the failing test.** Write `crates/buiy_verify/src/reftest.rs`:
  ```rust
  //! Tier 4 — reftests + the CPU-vs-GPU SDF cross-check (reftests.md).
  //!
  //! A reftest renders a `test` and a `reference` scene with the SAME engine in
  //! ONE process and asserts their bitmaps match (`==`) or differ (`!=`), never
  //! against a stored baseline — so every platform-variance term (driver SDF
  //! rounding, glyph-atlas AA, sRGB encode, clock) cancels in the diff. The
  //! harness stores ZERO bytes. GPU-coupled cases are `#[ignore]`; the pairing /
  //! aggregation logic and the independence lint are pure-CPU and gate headless.

  /// Whether a [`RefCase`] passes on equality or on difference.
  #[derive(Clone, Copy, PartialEq, Eq, Debug)]
  pub enum RefKind {
      /// Pass iff `test` and `reference` render to the same bitmap within `fuzz`.
      Match,
      /// Pass iff they render DIFFERENTLY (a `!=` anti-test guards silent no-ops).
      Mismatch,
  }

  impl RefKind {
      /// Parse the `reftest!` macro's kind token (`stringify!($kind)`).
      /// Panics on any other token — the macro only ever passes these two.
      pub fn reftest_kind(token: &str) -> Self {
          match token {
              "match" => RefKind::Match,
              "mismatch" => RefKind::Mismatch,
              other => panic!("reftest! kind must be `match` or `mismatch`, got `{other}`"),
          }
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn reftest_kind_parses_both_tokens() {
          assert_eq!(RefKind::reftest_kind("match"), RefKind::Match);
          assert_eq!(RefKind::reftest_kind("mismatch"), RefKind::Mismatch);
      }

      #[test]
      #[should_panic(expected = "must be `match` or `mismatch`")]
      fn reftest_kind_rejects_garbage() {
          let _ = RefKind::reftest_kind("nope");
      }
  }
  ```

- [ ] **Register the module.** In `crates/buiy_verify/src/lib.rs`, add `pub mod reftest;` alongside the existing `pub mod` lines.

- [ ] **Run to verify it passes (new code, both tests).**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests
  ```
  Expected: `test result: ok. 2 passed`.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs crates/buiy_verify/src/lib.rs
  git commit -m "feat(verify): reftest module skeleton + RefKind parser

RefKind{Match,Mismatch} and reftest_kind(&str) — the token parser the
reftest! macro calls. reftests.md § Module & public API.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.3 — `RefCase` + `RefOutcome` types

The data the harness operates on: one pairing (`name`, `kind`, `test`/`reference` scene builders, per-pairing `fuzz`) and the outcome (`passed`, the `Diff`, an optional report path).

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add types above `#[cfg(test)]`)
- Test: inline `#[cfg(test)]` in `reftest.rs`

Steps:

- [ ] **Write the failing test.** Append inside `mod tests`:
  ```rust
  #[test]
  fn refcase_is_constructible_with_zero_fuzz_default() {
      use buiy_verify::metric::FuzzBudget;
      use bevy::app::App;
      fn noop(_: &mut App) {}
      let case = RefCase {
          name: "constructs",
          kind: RefKind::Match,
          test: noop,
          reference: noop,
          fuzz: FuzzBudget::EXACT,
      };
      assert_eq!(case.name, "constructs");
      assert_eq!(case.fuzz, FuzzBudget::EXACT);
  }
  ```

- [ ] **Run to verify it fails.**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests::refcase_is_constructible_with_zero_fuzz_default
  ```
  Expected: compile error `cannot find struct, variant or union type 'RefCase'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`, after `impl RefKind` (above `#[cfg(test)]`), add:
  ```rust
  use buiy_verify::metric::{Diff, FuzzBudget};
  use bevy::app::App;

  /// One reftest pairing. `test` and `reference` each build a scene into a
  /// fresh, deterministic `App` (spawn entities; do NOT drive frames —
  /// `run_reftest` owns the capture loop). Co-locate the expectation with the
  /// `#[test]` the `reftest!` macro generates.
  pub struct RefCase {
      pub name: &'static str,
      pub kind: RefKind,
      /// Builds the scene exercising the feature under test.
      pub test: fn(&mut App),
      /// Builds the independent-oracle scene (see "Reference independence").
      pub reference: fn(&mut App),
      /// Per-pairing fuzz, à la Mozilla `fuzzy-if`. Default `(0,0)` once the
      /// determinism stack is in (determinism.md); widen with a documented reason.
      pub fuzz: FuzzBudget,
  }

  /// The result of running one [`RefCase`].
  #[derive(Debug)]
  pub struct RefOutcome {
      pub passed: bool,
      pub diff: Diff,
      /// On failure, a self-contained local HTML triage report (test | ref |
      /// diff). Path printed to stderr; never committed.
      pub report_path: Option<std::path::PathBuf>,
  }
  ```
  (If a self-import lint trips on `use buiy_verify::...` inside the crate, use `crate::metric::{...}` instead.)

- [ ] **Run to verify it passes.**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests::refcase_is_constructible_with_zero_fuzz_default
  ```
  Expected: `test result: ok. 1 passed`.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs
  git commit -m "feat(verify): RefCase + RefOutcome reftest types

The pairing (name/kind/test/reference/fuzz) and its outcome
(passed/diff/report_path). reftests.md § Module & public API.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.4 — Pure-CPU pass-decision logic: `evaluate_outcome`

Split the `Match`/`Mismatch` pass decision out of `run_reftest` into a pure, GPU-free `fn evaluate_outcome(kind, &Diff, &FuzzBudget) -> bool` so it gates headless via the aggregation truth-table meta-test (`reftests.md` § Verification #1).

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add `evaluate_outcome` above `#[cfg(test)]`)
- Test: inline `#[cfg(test)]` in `reftest.rs`

Steps:

- [ ] **Write the failing truth-table test.** Append inside `mod tests`:
  ```rust
  use buiy_verify::metric::Diff;

  /// A stub Diff with `n` differing pixels and `max_channel_delta = d`, no MSSIM.
  fn stub_diff(n: u32, d: u8) -> Diff {
      Diff {
          differing_pixels: n,
          max_channel_delta: d,
          total_pixels: 1024,
          mssim: None,
          diff_image: None,
      }
  }

  #[test]
  fn match_passes_within_fuzz_fails_outside() {
      assert!(evaluate_outcome(RefKind::Match, &stub_diff(0, 0), &FuzzBudget::EXACT));
      assert!(!evaluate_outcome(RefKind::Match, &stub_diff(1, 200), &FuzzBudget::EXACT));
      assert!(evaluate_outcome(
          RefKind::Match,
          &stub_diff(1, 8),
          &FuzzBudget { max_channel_delta: 8, max_diff_pixels: 1 }
      ));
  }

  #[test]
  fn mismatch_passes_outside_fuzz_fails_within() {
      assert!(evaluate_outcome(RefKind::Mismatch, &stub_diff(50, 200), &FuzzBudget::EXACT));
      // A scene that did NOT change (zero diff) FAILS a mismatch — the no-op guard.
      assert!(!evaluate_outcome(RefKind::Mismatch, &stub_diff(0, 0), &FuzzBudget::EXACT));
  }
  ```

- [ ] **Run to verify it fails.**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests::match_passes_within_fuzz_fails_outside
  ```
  Expected: compile error `cannot find function 'evaluate_outcome'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`, above `#[cfg(test)]`:
  ```rust
  /// The pure pass-decision: `Match` passes iff the diff fits the budget;
  /// `Mismatch` passes iff it does NOT (the feature must *do* something). Split
  /// out of `run_reftest` so it gates headless via the aggregation truth table —
  /// no GPU. The `(0,0)`-floor enforcement for `Mismatch` lives at macro
  /// expansion time, so `evaluate_outcome` takes the budget as given.
  pub fn evaluate_outcome(kind: RefKind, diff: &Diff, fuzz: &FuzzBudget) -> bool {
      match kind {
          RefKind::Match => diff.passes(fuzz),
          RefKind::Mismatch => !diff.passes(fuzz),
      }
  }
  ```

- [ ] **Run to verify both pass.**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests::match_passes_within_fuzz_fails_outside reftest::tests::mismatch_passes_outside_fuzz_fails_within
  ```
  Expected: `test result: ok. 2 passed`.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs
  git commit -m "feat(verify): pure evaluate_outcome pass-decision + truth table

Match passes within budget, Mismatch passes outside it (the silent-no-op
guard). Pure CPU so it gates headless. reftests.md § Verification #1.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.5 — `run_reftest`: two captures in one app, diffed by the metric (GPU)

The engine: build one painting app, capture `test` then `reference` via `capture_to_image` in the **same** `App` (re-target + re-readback), `compare` with `CompareOpts::reftest_default()`, decide with `evaluate_outcome`, emit a triage report on failure. `#[ignore]` — GPU.

> **Phase ordering note.** `DeterministicApp` (determinism.md) lands in Phase 3. Phase 1b builds `run_reftest` against the **already-landed** capture seam directly: `capture_to_image(&mut app, &GoldenConfig::deterministic())` on a `capture_app(w, h)`-built app (promoted in Task 1b.6). Phase 3 swaps the `reftest_app` body for `DeterministicApp::new(w, h).build()` in one place (the seam is identical: `&mut App` in, `RgbaImage` out). The `#[ignore]` reftest cases pin behavior across that swap.

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add `run_reftest` + `capture_to_image_with` + `emit_report`)
- Create: `crates/buiy_verify/src/support.rs` (the build-seam glue — `reftest_app`, `clear_reftest_scene`)
- Modify: `crates/buiy_verify/src/lib.rs` (add `pub mod support;`)
- Test: `crates/buiy_verify/tests/reftest_engine_gpu.rs` (the self-vs-self / two-different known-good/known-bad pairs)

Steps:

- [ ] **Write the failing GPU known-good/known-bad test.** Create `crates/buiy_verify/tests/reftest_engine_gpu.rs`:
  ```rust
  //! GPU lane (`--ignored`): proves the reftest engine can both PASS and FAIL.
  //! reftests.md § Verification #3 — a scene-vs-itself match passes at (0,0); a
  //! scene-vs-different match fails (guards a vacuous green); a scene-vs-itself
  //! mismatch fails. Real adapter (RX 6700 XT here) / pinned lavapipe in CI.

  use bevy::prelude::*;
  use buiy_core::layout::{Inset, Length, Sizing, Style};
  use buiy_core::render::components::Background;
  use buiy_core::render::ColorToken;
  use buiy_core::components::Node;
  use buiy_verify::metric::FuzzBudget;
  use buiy_verify::reftest::{run_reftest, RefCase, RefKind};
  use std::borrow::Cow;

  /// A single 40×40 fill at (left,8) in `token` color.
  fn box_at(app: &mut App, left: f32, token: &'static str) {
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .absolute()
                  .inset(Inset {
                      top: Sizing::Length(Length::px(8.0)),
                      left: Sizing::Length(Length::px(left)),
                      ..default()
                  })
                  .width_px(40.0)
                  .height_px(40.0),
              Background { color: ColorToken::Token(Cow::Borrowed(token)) },
          ))
          .id();
      app.world_mut().spawn((Node, Style::default())).add_children(&[e]);
  }

  fn red_at_8(app: &mut App) { box_at(app, 8.0, "test.fill.a"); }
  fn red_at_120(app: &mut App) { box_at(app, 120.0, "test.fill.a"); }

  #[test]
  #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
  fn match_of_scene_with_itself_passes() {
      let case = RefCase {
          name: "self_match", kind: RefKind::Match,
          test: red_at_8, reference: red_at_8, fuzz: FuzzBudget::EXACT,
      };
      let outcome = run_reftest(&case);
      assert!(outcome.passed, "self-match must pass at (0,0): {:?}", outcome.diff);
  }

  #[test]
  #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
  fn match_of_two_different_scenes_fails() {
      let case = RefCase {
          name: "different_match_fails", kind: RefKind::Match,
          test: red_at_8, reference: red_at_120, fuzz: FuzzBudget::EXACT,
      };
      let outcome = run_reftest(&case);
      assert!(!outcome.passed, "differing scenes must NOT match (vacuous-green guard)");
      assert!(outcome.report_path.is_some(), "failure emits a triage report");
  }

  #[test]
  #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
  fn mismatch_of_scene_with_itself_fails() {
      let case = RefCase {
          name: "self_mismatch_fails", kind: RefKind::Mismatch,
          test: red_at_8, reference: red_at_8, fuzz: FuzzBudget::EXACT,
      };
      let outcome = run_reftest(&case);
      assert!(!outcome.passed, "a scene cannot mismatch itself");
  }
  ```

- [ ] **Run to verify it fails (compile).**
  ```sh
  cargo test -p buiy_verify --test reftest_engine_gpu --no-run
  ```
  Expected: compile error `cannot find function 'run_reftest' in module 'buiy_verify::reftest'`.

- [ ] **Write `run_reftest` + helpers.** In `crates/buiy_verify/src/reftest.rs`, above `#[cfg(test)]`:
  ```rust
  use buiy_verify::metric::{compare, CompareOpts};
  use buiy_core::render::golden::{capture_to_image, GoldenConfig};

  /// The capture viewport for reftest pairings, in logical px. Both halves are
  /// captured at this size in one app run; large enough that a single 40px box
  /// and a 120px-shifted twin do not overlap (so a moved box is a real diff).
  const REFTEST_LOGICAL: (u32, u32) = (200, 120);

  /// Render BOTH scenes via the buiy_core capture seam in ONE app run and diff
  /// with `metric::compare`. Platform variance cancels because both halves share
  /// one `wgpu::Device`, driver, atlas, and virtual clock. GPU-coupled.
  ///
  /// Until the determinism stack lands this builds the app via `reftest_app`
  /// (the canonical `capture_app` seam); Phase 3 swaps that one line for
  /// `DeterministicApp::build` with an identical `&mut App`→capture contract.
  pub fn run_reftest(case: &RefCase) -> RefOutcome {
      assert!(
          mismatch_floor_ok(case.kind, &case.fuzz),
          "reftest `{}`: a Mismatch with a non-(0,0) fuzz floor is vacuous",
          case.name
      );
      let (w, h) = REFTEST_LOGICAL;
      let mut app = crate::support::reftest_app(w, h);
      let cfg = GoldenConfig::deterministic();

      let test_img = capture_to_image_with(&mut app, case.test, &cfg);
      let ref_img = capture_to_image_with(&mut app, case.reference, &cfg);

      let diff = compare(&test_img, &ref_img, &CompareOpts::reftest_default());
      let passed = evaluate_outcome(case.kind, &diff, &case.fuzz);
      let report_path = if passed {
          None
      } else {
          Some(emit_report(case.name, &test_img, &ref_img, &diff))
      };
      RefOutcome { passed, diff, report_path }
  }

  /// Clear the previous scene, spawn `scene`, capture via the buiy_core seam.
  fn capture_to_image_with(
      app: &mut bevy::app::App,
      scene: fn(&mut bevy::app::App),
      cfg: &GoldenConfig,
  ) -> image::RgbaImage {
      crate::support::clear_reftest_scene(app);
      scene(app);
      capture_to_image(app, cfg)
  }

  /// Write a self-contained HTML triage report (test | ref | diff) to a temp
  /// path and return it. Phase 3 swaps this for the golden-tier emitter; until
  /// then, a minimal three-PNG dump. Never committed.
  fn emit_report(
      name: &str,
      test: &image::RgbaImage,
      reference: &image::RgbaImage,
      diff: &Diff,
  ) -> std::path::PathBuf {
      let dir = std::env::temp_dir().join("buiy-reftest");
      let _ = std::fs::create_dir_all(&dir);
      let base = dir.join(name);
      let _ = test.save(base.with_extension("test.png"));
      let _ = reference.save(base.with_extension("ref.png"));
      if let Some(img) = &diff.diff_image {
          let _ = img.save(base.with_extension("diff.png"));
      }
      let report = base.with_extension("html");
      let _ = std::fs::write(
          &report,
          format!(
              "<h1>reftest {name} FAILED</h1><p>differing_pixels={} max_channel_delta={}</p>\
               <img src='{name}.test.png'><img src='{name}.ref.png'><img src='{name}.diff.png'>",
              diff.differing_pixels, diff.max_channel_delta
          ),
      );
      eprintln!("reftest {name} report: {}", report.display());
      report
  }
  ```
  (`mismatch_floor_ok` lands in Task 1b.7; until then drop the leading `assert!` or stub `mismatch_floor_ok` returning `true`. Cleaner: land 1b.7 before running this engine on the GPU lane — the macro/engine guard and the engine compile together. The plan orders 1b.7 after 1b.6's green checkpoint; if 1b.5/1b.6 must compile first, temporarily inline `true` and replace it in 1b.7.)

- [ ] **Add the build-seam glue.** Create `crates/buiy_verify/src/support.rs`:
  ```rust
  //! GPU-capture glue for the reftest/golden tiers — the ONE place that names
  //! the concrete app builder, so Phase 3 swaps it for `DeterministicApp` in a
  //! single edit. `pub` so `tests/` integration tests reach it.

  use bevy::prelude::*;

  /// Build the headless painting app both reftest captures share. Until the
  /// determinism builder lands this delegates to the promoted
  /// `buiy_core::render::golden::capture_app` (Task 1b.6).
  pub fn reftest_app(logical_w: u32, logical_h: u32) -> App {
      buiy_core::render::golden::capture_app(logical_w, logical_h)
  }

  /// Despawn the previous scene's spawned roots between the two captures so the
  /// second scene renders alone. Keeps the camera + render-target entities.
  pub fn clear_reftest_scene(app: &mut App) {
      let roots: Vec<Entity> = app
          .world_mut()
          .query_filtered::<Entity, (With<buiy_core::components::Node>, Without<ChildOf>)>()
          .iter(app.world())
          .collect();
      for e in roots {
          app.world_mut().entity_mut(e).despawn();
      }
  }
  ```
  Register it in `crates/buiy_verify/src/lib.rs`: `pub mod support;`. Confirm `image`'s PNG `save` is available (workspace `image = "0.25"` default features include `png`); if `cargo build` reports `save` missing, add `image = { workspace = true, features = ["png"] }` to `crates/buiy_verify/Cargo.toml` and re-run `cargo deny check`.

- [ ] **Defer the compile to 1b.6.** `run_reftest`/`reftest_app` reference `capture_app`, added next. Do NOT run yet; the green checkpoint is at the end of Task 1b.6. **No standalone commit** — commit 1b.5 + 1b.6 together at 1b.6's checkpoint to keep the tree green.

---

### Task 1b.6 — Promote the painting-app builder into `render/golden.rs` src (`capture_app`)

`run_reftest` needs a painting-capable `App` from `src` (not test-only `tests/support`). Promote the canonical single-body plugin stack into `buiy_core::render::golden` as `capture_app`, mirroring the already-promoted `capture_to_image`. Closes the compile from Task 1b.5.

**Files:**
- Modify: `crates/buiy_core/src/render/golden.rs` (add `capture_app` + `capture_app_scaled`; reuse the exact plugin list `gpu_render_app_with_resolution` uses, `tests/support/mod.rs:168`)
- Test: `crates/buiy_core/tests/render_capture_app_gpu.rs` (proves `capture_app` builds a painting app that captures a non-blank frame; GPU `#[ignore]`)

Steps:

- [ ] **Write the failing GPU test.** Create `crates/buiy_core/tests/render_capture_app_gpu.rs`:
  ```rust
  //! GPU lane: `render::golden::capture_app` builds a painting-capable headless
  //! App identical to the test-support `gpu_render_app` stack, so the reftest /
  //! golden tiers in buiy_verify build their app from `src` (reftests.md § build
  //! seam). #[ignore] — needs a real adapter.

  use bevy::prelude::*;
  use buiy_core::layout::{Inset, Length, Sizing, Style};
  use buiy_core::render::components::Background;
  use buiy_core::render::golden::{capture_app, capture_to_image, GoldenConfig};
  use buiy_core::render::ColorToken;
  use buiy_core::components::Node;
  use std::borrow::Cow;

  #[test]
  #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
  fn capture_app_paints_a_non_blank_frame() {
      let mut app = capture_app(64, 64);
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .absolute()
                  .inset(Inset {
                      top: Sizing::Length(Length::px(8.0)),
                      left: Sizing::Length(Length::px(8.0)),
                      ..default()
                  })
                  .width_px(40.0)
                  .height_px(40.0),
              Background { color: ColorToken::Token(Cow::Borrowed("test.fill.a")) },
          ))
          .id();
      app.world_mut().spawn((Node, Style::default())).add_children(&[e]);

      let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
      assert_eq!(img.dimensions(), (64, 64));
      let painted = img.pixels().any(|p| p.0 != [0, 0, 0, 255]);
      assert!(painted, "capture_app must paint the box, not a blank frame");
  }
  ```

- [ ] **Run to verify it fails (compile).**
  ```sh
  cargo test -p buiy_core --test render_capture_app_gpu --no-run
  ```
  Expected: compile error `cannot find function 'capture_app' in module 'buiy_core::render::golden'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_core/src/render/golden.rs`, add `capture_app` + `capture_app_scaled`, moving the canonical plugin-stack body from `tests/support/mod.rs:168`'s `gpu_render_app_with_resolution` into src (the test-support `gpu_render_app*` then delegate to these — single source so the scaled / test-support builders cannot drift):
  ```rust
  use bevy::app::App;

  /// Build the canonical headless painting App at a logical viewport size,
  /// promoted from `tests/support/mod.rs` into src so `buiy_verify`'s reftest /
  /// golden tiers build their app without the test crate. NOT finished:
  /// `capture_to_image` finishes + drives to quiescence + reads back.
  pub fn capture_app(logical_w: u32, logical_h: u32) -> App {
      capture_app_scaled(logical_w, logical_h, 1.0)
  }

  /// [`capture_app`] at an explicit window scale factor (the DPR-pin builder
  /// determinism.md sizes the offscreen target through). Bevy 0.18
  /// `WindowResolution::new` takes PHYSICAL units; pass `logical × scale` plus
  /// the override so `resolution.size()` reads back the logical size the view
  /// uniform is built from.
  pub fn capture_app_scaled(logical_w: u32, logical_h: u32, scale_factor: f32) -> App {
      use bevy::window::{Window, WindowPlugin, WindowResolution};
      let resolution = WindowResolution::new(
          (logical_w as f32 * scale_factor).round() as u32,
          (logical_h as f32 * scale_factor).round() as u32,
      )
      .with_scale_factor_override(scale_factor);

      let mut app = App::new();
      app.add_plugins(bevy::MinimalPlugins)
          .add_plugins(WindowPlugin {
              primary_window: Some(Window { resolution, ..bevy::prelude::default() }),
              ..bevy::prelude::default()
          })
          .add_plugins(bevy::asset::AssetPlugin::default())
          .add_plugins(bevy::render::RenderPlugin::default())
          .add_plugins(bevy::image::ImagePlugin::default())
          .add_plugins(bevy::camera::CameraPlugin)
          .add_plugins(bevy::core_pipeline::CorePipelinePlugin)
          .add_plugins(crate::theme::ThemePlugin)
          .add_plugins(crate::layout::LayoutPlugin)
          .add_plugins(crate::CorePlugin)
          .add_plugins(crate::text::BuiyTextPlugin::default())
          .add_plugins(crate::render::BuiyRenderPlugin);
      app.init_asset::<bevy::prelude::Mesh>();
      app
  }
  ```
  **Verify the exact plugin list against `tests/support/mod.rs:168` at impl time** — the list above mirrors the documented stack but must match the canonical builder byte-for-byte (plugin set + init order). Then make `gpu_render_app_with_resolution` delegate to `capture_app_scaled` so there is one body. The existing `render_golden_harness.rs` GPU test transitively re-verifies the stack.

- [ ] **Run the new GPU test (GPU lane).**
  ```sh
  cargo test -p buiy_core --test render_capture_app_gpu -j 2 -- --ignored --test-threads=1
  ```
  Expected: `test result: ok. 1 passed`.

- [ ] **Run the reftest engine GPU test (now compiles + runs).**
  ```sh
  cargo test -p buiy_verify --test reftest_engine_gpu -j 2 -- --ignored --test-threads=1
  ```
  Expected: `test result: ok. 3 passed`. (If `run_reftest` still references the not-yet-landed `mismatch_floor_ok`, inline `true` per 1b.5's note, run, then restore in 1b.7.)

- [ ] **Run the headless gate (no regressions; the pure-CPU reftest meta-tests stay green).**
  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test --workspace
  ```
  Expected: all green; the `#[ignore]` GPU tests are skipped here.

- [ ] **Commit (Tasks 1b.5 + 1b.6 together — first green checkpoint for the engine).**
  ```sh
  git add crates/buiy_core/src/render/golden.rs crates/buiy_core/tests/render_capture_app_gpu.rs crates/buiy_core/tests/support/mod.rs crates/buiy_verify/src/reftest.rs crates/buiy_verify/src/support.rs crates/buiy_verify/src/lib.rs crates/buiy_verify/tests/reftest_engine_gpu.rs crates/buiy_verify/Cargo.toml
  git commit -m "feat(verify): run_reftest engine + promote capture_app to src

run_reftest captures test+reference in ONE app via capture_to_image
(re-target + re-readback) and diffs with metric::compare; the painting-app
builder is promoted from tests/support into render::golden::capture_app so
buiy_verify builds its app from src. GPU known-good/known-bad pairs prove
the harness can both pass and fail (vacuous-green guard). reftests.md §§
API, Verification #3.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.7 — Mismatch-floor guard (compile-time + run-time)

A `Mismatch` whose fuzz budget tolerates difference is vacuous (`reftests.md` § Verification #2). The `reftest!` macro forces `(0,0)` for `mismatch` at expansion (Task 1b.8); `run_reftest` also rejects a non-`(0,0)` floor on a `Mismatch` at run time as a belt. This task adds the run-time guard + its meta-test.

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add `mismatch_floor_ok`; the `assert!` at the top of `run_reftest` from 1b.5 now calls the real fn)
- Test: inline `#[cfg(test)]` in `reftest.rs`

Steps:

- [ ] **Write the failing test.** Append inside `mod tests`:
  ```rust
  #[test]
  fn mismatch_requires_zero_fuzz_floor() {
      assert!(mismatch_floor_ok(RefKind::Mismatch, &FuzzBudget::EXACT));
      assert!(!mismatch_floor_ok(RefKind::Mismatch, &FuzzBudget { max_channel_delta: 1, max_diff_pixels: 0 }));
      assert!(!mismatch_floor_ok(RefKind::Mismatch, &FuzzBudget { max_channel_delta: 0, max_diff_pixels: 1 }));
      // Match may carry any budget.
      assert!(mismatch_floor_ok(RefKind::Match, &FuzzBudget { max_channel_delta: 8, max_diff_pixels: 4 }));
  }
  ```

- [ ] **Run to verify it fails.**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests::mismatch_requires_zero_fuzz_floor
  ```
  Expected: compile error `cannot find function 'mismatch_floor_ok'` (unless 1b.5 stubbed it `true` — in that case the test fails on the non-`(0,0)` assertions).

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`, above `#[cfg(test)]`:
  ```rust
  /// A `Mismatch` budget that tolerates difference is meaningless — its floor
  /// must be `(0,0)`. `Match` may carry any widening. Pure CPU so it gates
  /// headless (reftests.md § Verification #2); the `reftest!` macro enforces the
  /// same at expansion time, and `run_reftest` asserts it as a belt.
  pub fn mismatch_floor_ok(kind: RefKind, fuzz: &FuzzBudget) -> bool {
      match kind {
          RefKind::Mismatch => *fuzz == FuzzBudget::EXACT,
          RefKind::Match => true,
      }
  }
  ```
  Confirm the `assert!(mismatch_floor_ok(...))` at the top of `run_reftest` (from 1b.5) now references this fn (restore it if 1b.5 inlined `true`).

- [ ] **Run to verify it passes.**
  ```sh
  cargo test -p buiy_verify --lib reftest::tests::mismatch_requires_zero_fuzz_floor
  ```
  Expected: `test result: ok. 1 passed`.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs
  git commit -m "feat(verify): reject non-(0,0) fuzz floor on a Mismatch

A != that tolerates difference is vacuous — mismatch_floor_ok gates it
pure-CPU and run_reftest asserts it as a belt. reftests.md § Verification #2.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.8 — The `reftest!` macro

Generate one `#[test] #[ignore]` per pairing from `reftest!(kind, fn_ident, test_fn, ref_fn[, fuzz = (d, p)])`, parse the kind token via `RefKind::reftest_kind`, default fuzz to `(0,0)`, and reject a non-`(0,0)` floor on `mismatch` at **compile time** (a `const` assertion). `#[macro_export]`.

> **Function-name surface (load-bearing).** The generated `fn` cannot be named `match` (a keyword), and two `reftest!(match, …)` in one module would collide. So the macro surface takes the **generated test fn name as an `$fn:ident`** (`reftest!(match, flex_justify_eq_literal, test, ref)`), with `stringify!($fn)` as `RefCase.name`. This is the spelling the real cases in 1b.12 use.

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add `macro_rules! reftest` + `@gen` internal rule)
- Test: `crates/buiy_verify/tests/reftest_macro_gpu.rs` (a macro-generated case, `#[ignore]`)

Steps:

- [ ] **Write the failing test.** Create `crates/buiy_verify/tests/reftest_macro_gpu.rs`:
  ```rust
  //! GPU lane: the `reftest!` macro generates an `#[ignore]` test per pairing.
  //! Uses the same self-match scene as the engine test to prove the macro wires
  //! through to a passing run. reftests.md § "The reftest! macro".

  use bevy::prelude::*;
  use buiy_core::layout::{Inset, Length, Sizing, Style};
  use buiy_core::render::components::Background;
  use buiy_core::render::ColorToken;
  use buiy_core::components::Node;
  use std::borrow::Cow;

  fn solid_box(app: &mut App) {
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .absolute()
                  .inset(Inset {
                      top: Sizing::Length(Length::px(8.0)),
                      left: Sizing::Length(Length::px(8.0)),
                      ..default()
                  })
                  .width_px(40.0)
                  .height_px(40.0),
              Background { color: ColorToken::Token(Cow::Borrowed("test.fill.a")) },
          ))
          .id();
      app.world_mut().spawn((Node, Style::default())).add_children(&[e]);
  }

  buiy_verify::reftest!(match, macro_self_match, solid_box, solid_box);
  ```

- [ ] **Run to verify it fails (compile).**
  ```sh
  cargo test -p buiy_verify --test reftest_macro_gpu --no-run
  ```
  Expected: compile error `cannot find macro 'reftest' in crate 'buiy_verify'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`, at module scope:
  ```rust
  /// Generate one `#[test] #[ignore]` per reftest pairing — keeps each case at
  /// the unit/integration tier under the existing `cargo test -- --ignored` GPU
  /// lane, no new CI infra, no manifest file (the type system IS the manifest).
  ///
  /// ```ignore
  /// reftest!(match,    flex_justify_end, flex_test, literal_offsets_ref);
  /// reftest!(mismatch, cv_hidden_hides,  cv_visible, cv_hidden);
  /// reftest!(match,    transform_xy,     xfm_test,   literal_ref, fuzz = (1, 8));
  /// ```
  ///
  /// A non-`(0,0)` fuzz floor on a `mismatch` fails to COMPILE (a `const`
  /// assertion), not at runtime — reftests.md § Verification #2.
  #[macro_export]
  macro_rules! reftest {
      // mismatch with explicit fuzz → compile-time reject of a non-zero floor.
      (mismatch, $fn:ident, $test:path, $reference:path, fuzz = ($d:literal, $p:literal)) => {
          const _: () = assert!(
              $d == 0 && $p == 0,
              concat!("reftest mismatch `", stringify!($fn), "`: a non-(0,0) fuzz floor is vacuous"),
          );
          $crate::reftest!(@gen mismatch, $fn, $test, $reference, ($d, $p));
      };
      // match with explicit fuzz.
      (match, $fn:ident, $test:path, $reference:path, fuzz = ($d:literal, $p:literal)) => {
          $crate::reftest!(@gen match, $fn, $test, $reference, ($d, $p));
      };
      // no explicit fuzz → (0,0) for either kind.
      ($kind:ident, $fn:ident, $test:path, $reference:path) => {
          $crate::reftest!(@gen $kind, $fn, $test, $reference, (0, 0));
      };
      // internal: emit the #[ignore] test named $fn.
      (@gen $kind:ident, $fn:ident, $test:path, $reference:path, ($d:literal, $p:literal)) => {
          #[test]
          #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
          fn $fn() {
              let case = $crate::reftest::RefCase {
                  name: stringify!($fn),
                  kind: $crate::reftest::RefKind::reftest_kind(stringify!($kind)),
                  test: $test,
                  reference: $reference,
                  fuzz: $crate::metric::FuzzBudget {
                      max_channel_delta: $d,
                      max_diff_pixels: $p,
                  },
              };
              let outcome = $crate::reftest::run_reftest(&case);
              assert!(
                  outcome.passed,
                  "reftest {} failed: {:?} (report: {:?})",
                  stringify!($fn), outcome.diff, outcome.report_path
              );
          }
      };
  }
  ```

- [ ] **Run to verify it compiles + the generated test is `#[ignore]`.**
  ```sh
  cargo test -p buiy_verify --test reftest_macro_gpu --no-run
  cargo test -p buiy_verify --test reftest_macro_gpu 2>&1 | grep -E "macro_self_match|ignored"
  ```
  Expected: compiles; the generated `macro_self_match` is listed as `ignored` in the headless run (`1 ignored`).

- [ ] **Run the generated case on the GPU lane.**
  ```sh
  cargo test -p buiy_verify --test reftest_macro_gpu -j 2 -- --ignored --test-threads=1
  ```
  Expected: `test result: ok. 1 passed`.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs crates/buiy_verify/tests/reftest_macro_gpu.rs
  git commit -m "feat(verify): reftest! macro generating #[ignore] GPU cases

reftest!(kind, fn_ident, test, reference[, fuzz=(d,p)]) emits one
#[test] #[ignore] per pairing; a non-(0,0) floor on a mismatch fails to
COMPILE via a const assert. reftests.md § 'The reftest! macro'.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.9 — Reference-independence structural lint (`assert_reference_independent`)

A reference must not carry the marker component the feature-under-test exercises (`reftests.md` § "Reference independence", mechanism 2). Build a headless no-GPU `App`, run the case's `reference` scene, query for forbidden component markers, assert none present. The value-encoded-feature caveat is documented as human-review.

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add `ComponentMarker`, `IndependenceRule`, `assert_reference_independent`, `default_rules`)
- Test: `crates/buiy_verify/tests/reftest_independence.rs` (pure CPU, **not** `#[ignore]`: the RED/GREEN self-test)

Steps:

- [ ] **Write the failing RED/GREEN self-test.** Create `crates/buiy_verify/tests/reftest_independence.rs`:
  ```rust
  //! Pure-CPU lint self-test (NOT #[ignore]): a reference that ILLEGALLY carries
  //! the forbidden marker trips assert_reference_independent (RED); the canonical
  //! disjoint reference passes (GREEN). reftests.md § Verification #4. The lint
  //! is itself tested, not trusted.

  use bevy::prelude::*;
  use buiy_core::layout::{ContentVisibility, Style};
  use buiy_core::layout::components::Containment;
  use buiy_core::components::Node;
  use buiy_verify::metric::FuzzBudget;
  use buiy_verify::reftest::{
      assert_reference_independent, default_rules, ComponentMarker, IndependenceRule, RefCase, RefKind,
  };

  fn empty(_: &mut App) {}

  fn visible_box(app: &mut App) {
      app.world_mut().spawn((Node, Style::default()));
  }

  fn hidden_box(app: &mut App) {
      app.world_mut().spawn((
          Node,
          Style::default(),
          Containment { content_visibility: ContentVisibility::Hidden, ..default() },
      ));
  }

  #[test]
  fn legal_reference_passes_the_lint() {
      let case = RefCase {
          name: "cv_green", kind: RefKind::Mismatch,
          test: empty, reference: visible_box, fuzz: FuzzBudget::EXACT,
      };
      assert_reference_independent(&case, &default_rules());
  }

  #[test]
  #[should_panic(expected = "reference for `content-visibility` illegally contains")]
  fn illegal_reference_trips_the_lint() {
      let case = RefCase {
          name: "cv_red", kind: RefKind::Mismatch,
          test: empty, reference: hidden_box, fuzz: FuzzBudget::EXACT,
      };
      assert_reference_independent(&case, &[IndependenceRule {
          feature: "content-visibility",
          forbidden_in_reference: &[ComponentMarker::ContentVisibilityHidden],
      }]);
  }
  ```

- [ ] **Run to verify it fails (compile).**
  ```sh
  cargo test -p buiy_verify --test reftest_independence --no-run
  ```
  Expected: compile errors `cannot find type 'ComponentMarker'` / `cannot find function 'assert_reference_independent'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`, above `#[cfg(test)]`:
  ```rust
  use bevy::prelude::World;

  /// A structural marker the independence lint can query for in a built world.
  /// Each variant maps to a `buiy_core` component whose *presence* proves a
  /// reference re-used the feature under test. Value-encoded features
  /// (`justify-content`, `direction`, `gap` — fields on a shared `Style`) have NO
  /// marker here and fall to human review (see `assert_reference_independent`).
  #[derive(Clone, Copy, PartialEq, Eq, Debug)]
  pub enum ComponentMarker {
      ContentVisibilityHidden,
      ContainerQuery,
      TopLayer,
      Translate,
  }

  impl ComponentMarker {
      /// True iff ANY entity in `world` carries this marker.
      fn present_in(self, world: &mut World) -> bool {
          use buiy_core::layout::components::Containment;
          use buiy_core::layout::{ContainerQuery, ContentVisibility, Translate};
          use buiy_core::components::TopLayer;
          match self {
              ComponentMarker::ContentVisibilityHidden => world
                  .query::<&Containment>()
                  .iter(world)
                  .any(|c| c.content_visibility == ContentVisibility::Hidden),
              ComponentMarker::ContainerQuery =>
                  world.query::<&ContainerQuery>().iter(world).next().is_some(),
              ComponentMarker::TopLayer =>
                  world.query::<&TopLayer>().iter(world).next().is_some(),
              ComponentMarker::Translate =>
                  world.query::<&Translate>().iter(world).next().is_some(),
          }
      }
  }

  /// What a reference scene is FORBIDDEN to contain, per feature under test.
  pub struct IndependenceRule {
      pub feature: &'static str,
      pub forbidden_in_reference: &'static [ComponentMarker],
  }

  /// The registered marker rules for marker-bearing features. Value-encoded
  /// features (flex `justify-content`, `direction`, `gap`) are deliberately
  /// ABSENT — component-presence cannot distinguish them, so they fall to the
  /// PR-time review checklist. A pairing whose feature has no rule here fails the
  /// lint until a rule (or documented waiver) is added — independence is
  /// opt-out-impossible by construction for marker features.
  pub fn default_rules() -> Vec<IndependenceRule> {
      vec![
          IndependenceRule { feature: "content-visibility", forbidden_in_reference: &[ComponentMarker::ContentVisibilityHidden] },
          IndependenceRule { feature: "@container", forbidden_in_reference: &[ComponentMarker::ContainerQuery] },
          IndependenceRule { feature: "top-layer", forbidden_in_reference: &[ComponentMarker::TopLayer] },
          IndependenceRule { feature: "translate", forbidden_in_reference: &[ComponentMarker::Translate] },
      ]
  }

  /// Assert the case's `reference` scene carries NONE of the marker components a
  /// rule forbids. Builds the reference into a headless **no-GPU** `App` (layout
  /// types registered, no render plugins) and queries the built world. Panics
  /// naming the feature + marker on violation.
  ///
  /// **Limit — value-encoded features fall to human review.** Features that are
  /// field *values* on a shared `Style`/`Node` (`justify-content`, `direction`,
  /// `gap`) have no distinct marker, so this lint cannot see them; mechanism 1
  /// (route the reference through the primitive literal-`Node` layer) keeps THOSE
  /// independent, and the PR-time checklist enforces it. This backstops only
  /// marker-bearing features.
  pub fn assert_reference_independent(case: &RefCase, rules: &[IndependenceRule]) {
      let mut app = bevy::app::App::new();
      app.add_plugins(buiy_core::layout::LayoutPlugin);
      (case.reference)(&mut app);
      let world = app.world_mut();
      for rule in rules {
          for &marker in rule.forbidden_in_reference {
              assert!(
                  !marker.present_in(world),
                  "reference for `{}` illegally contains {:?} — it re-uses the \
                   feature under test, so the comparison would pass vacuously \
                   (reftests.md § Reference independence)",
                  rule.feature, marker
              );
          }
      }
  }
  ```
  (If `LayoutPlugin` requires render/asset plugins to build, substitute a minimal `App::new()` + `register_type` and direct `world.spawn` through the scene fn; the query only needs the components to exist as data, not the plugin systems. Confirm `ContainerQuery`/`Translate`/`TopLayer`/`Containment` import paths against the live crate.)

- [ ] **Run to verify both pass (GREEN passes, RED panics-as-expected).**
  ```sh
  cargo test -p buiy_verify --test reftest_independence
  ```
  Expected: `test result: ok. 2 passed`.

- [ ] **Run the headless gate.**
  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test --workspace
  ```
  Expected: all green (this lint runs in the headless gate — pure CPU, not `#[ignore]`).

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs crates/buiy_verify/tests/reftest_independence.rs
  git commit -m "feat(verify): reference-independence structural lint

assert_reference_independent builds the reference into a no-GPU App and
rejects any forbidden marker (ContentVisibility/ContainerQuery/TopLayer/
Translate). Value-encoded features fall to human review (documented). The
lint is itself RED/GREEN-tested. reftests.md §§ Reference independence,
Verification #4.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.10 — CPU SDF rasterizer oracle (`rasterize_sdf_rect`) + point-probe pin

Promote the CPU SDF port from three scalar probes (`tests/render_instance.rs:12`) to a full-tile rasterizer that mirrors `shader.wgsl:60` (`sdf_rounded_rect`) and `:76–:79` (`fwidth → smoothstep(-aa, aa, d)` AA). Pin it to the existing point-probes (`reftests.md` § Verification #5) — pure CPU, no GPU.

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add a `sdf_oracle` submodule with `rasterize_sdf_rect`)
- Test: `crates/buiy_verify/tests/sdf_oracle.rs` (pure CPU)

Steps:

- [ ] **Write the failing pure-CPU test.** Create `crates/buiy_verify/tests/sdf_oracle.rs`:
  ```rust
  //! Pure-CPU (NOT #[ignore]): the full-tile CPU SDF oracle must reproduce the
  //! scalar `d` the existing render_instance.rs point-probes assert — center
  //! inside (filled), 2× half-extent outside (empty). Pins the full-tile port to
  //! the unit-tested shader formula. reftests.md § Verification #5.

  use bevy::prelude::*;
  use buiy_core::render::DrawData;
  use buiy_verify::reftest::sdf_oracle::rasterize_sdf_rect;

  #[test]
  fn oracle_fills_center_and_clears_far_outside() {
      let inset = DrawData::new(Vec2::new(50.0, 25.0), Vec2::new(40.0, 20.0), Color::WHITE, 0.0);
      let img = rasterize_sdf_rect(&inset, 200, 100);
      assert_eq!(img.dimensions(), (200, 100));
      assert_eq!(img.get_pixel(5, 5).0[3], 0, "far outside the box is empty");
      assert_eq!(img.get_pixel(70, 35).0[3], 255, "inside the inset box is filled");
  }

  #[test]
  fn oracle_edge_band_is_partial_alpha() {
      // The AA band must be neither fully 0 nor fully 255 for at least one pixel
      // (proves the smoothstep coverage step is live) — the property the GPU
      // shader's fwidth→smoothstep produces.
      let draw = DrawData::new(Vec2::new(50.0, 25.0), Vec2::new(40.0, 20.0), Color::WHITE, 8.0);
      let img = rasterize_sdf_rect(&draw, 200, 100);
      let has_partial = img.pixels().any(|p| { let a = p.0[3]; a > 0 && a < 255 });
      assert!(has_partial, "a rounded-rect edge must produce AA partial-alpha pixels");
  }
  ```
  *(Confirm the `DrawData::new` constructor signature `(position, size, color, radius)` against `crates/buiy_core/src/render/instance.rs` at impl time; adjust field order/names if the real API differs.)*

- [ ] **Run to verify it fails (compile).**
  ```sh
  cargo test -p buiy_verify --test sdf_oracle --no-run
  ```
  Expected: compile error `could not find 'sdf_oracle' in 'reftest'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`, add the submodule:
  ```rust
  /// Pure-CPU per-pixel evaluation of the WGSL SDF + AA coverage step, the
  /// golden-free oracle for SDF corner AA (Tier 4.5). The SDF formula is shared
  /// 1:1 with `shader.wgsl:60` / `:76-:79` — the port and the shader must stay
  /// identical, pinned by the point-probe test that re-derives the values
  /// `tests/render_instance.rs:12` already asserts.
  pub mod sdf_oracle {
      use bevy::math::Vec2;
      use buiy_core::render::DrawData;

      /// 1:1 CPU port of `shader.wgsl::sdf_rounded_rect`.
      pub fn sdf_rounded_rect(p: Vec2, half_size: Vec2, r: f32) -> f32 {
          let q = p.abs() - half_size + Vec2::splat(r);
          q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
      }

      /// Rasterize one `DrawData` rounded-rect into a `w×h` RGBA tile, mirroring
      /// the fragment shader: SDF in logical px, AA via a `fwidth` estimate (the
      /// per-pixel SDF gradient via central difference) fed to
      /// `smoothstep(-aa, aa, d)`.
      pub fn rasterize_sdf_rect(draw: &DrawData, w: u32, h: u32) -> image::RgbaImage {
          let half = draw.size * 0.5;
          let center = draw.position + half;
          let r = draw.radius;
          let lin = bevy::color::LinearRgba::from(draw.color);
          let srgba = bevy::color::Srgba::from(lin);
          let (rr, gg, bb) = (
              (srgba.red * 255.0).round() as u8,
              (srgba.green * 255.0).round() as u8,
              (srgba.blue * 255.0).round() as u8,
          );
          let base_a = draw.color.alpha();

          let mut img = image::RgbaImage::new(w, h);
          for y in 0..h {
              for x in 0..w {
                  let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - center;
                  let d = sdf_rounded_rect(p, half, r);
                  let dx = (sdf_rounded_rect(p + Vec2::X, half, r)
                      - sdf_rounded_rect(p - Vec2::X, half, r)).abs() * 0.5;
                  let dy = (sdf_rounded_rect(p + Vec2::Y, half, r)
                      - sdf_rounded_rect(p - Vec2::Y, half, r)).abs() * 0.5;
                  let aa = (dx + dy).max(1e-4);
                  let coverage = 1.0 - smoothstep(-aa, aa, d);
                  let a = (base_a * coverage * 255.0).round().clamp(0.0, 255.0) as u8;
                  img.put_pixel(x, y, image::Rgba([rr, gg, bb, a]));
              }
          }
          img
      }

      /// `smoothstep` matching WGSL `smoothstep(edge0, edge1, x)`.
      fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
          let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
          t * t * (3.0 - 2.0 * t)
      }
  }
  ```
  > **AA-estimate fidelity note.** The GPU `fwidth` is a screen-space derivative; the CPU central-difference approximates it. This is *intended* — the cross-check (1b.11) tolerates sub-pixel AA noise via `fuzz`. The oracle catches *implementation* drift (wrong half-extent, radius clamp, premultiply), not a *spec* error in the shared `sdf_rounded_rect` (both paths share it — that is Tier 5's job).

- [ ] **Run to verify both pass.**
  ```sh
  cargo test -p buiy_verify --test sdf_oracle
  ```
  Expected: `test result: ok. 2 passed`.

- [ ] **Run the headless gate.**
  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test --workspace
  ```
  Expected: all green.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs crates/buiy_verify/tests/sdf_oracle.rs
  git commit -m "feat(verify): full-tile CPU SDF oracle (rasterize_sdf_rect)

Promotes the CPU SDF port from scalar probes to a full-tile rasterizer
mirroring shader.wgsl:60/:76-:79 (sdf_rounded_rect + fwidth→smoothstep).
Pinned to the render_instance.rs point-probes. reftests.md §§ CPU-vs-GPU
cross-check, Verification #5.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.11 — `run_sdf_cross_check`: GPU rounded-rect vs CPU oracle (GPU)

Render one rounded-rect on the GPU (single-instance capture via `capture_app` + `capture_to_image`), rasterize the same `DrawData` with `rasterize_sdf_rect`, diff via `metric::compare` within a documented `fuzz` budget (`reftests.md` § "CPU-vs-GPU SDF cross-check"). Zero stored bytes. `#[ignore]` — GPU.

> **`fn(&mut App)` vs a captured `DrawData` (load-bearing).** `RefCase` builders are `fn(&mut App)` (no captured environment), but `run_sdf_cross_check` must spawn a box matching the *runtime* `draw`. So it bypasses the `RefCase` path: it spawns the single primitive inline against `&mut app`, then calls `capture_to_image` directly.

**Files:**
- Modify: `crates/buiy_verify/src/reftest.rs` (add `run_sdf_cross_check` + `spawn_single_primitive`)
- Test: `crates/buiy_verify/tests/sdf_cross_check_gpu.rs` (GPU `#[ignore]`)

Steps:

- [ ] **Write the failing GPU test.** Create `crates/buiy_verify/tests/sdf_cross_check_gpu.rs`:
  ```rust
  //! GPU lane (`--ignored`): the GPU rounded-rect render and the CPU SDF oracle
  //! must agree within a documented AA fuzz budget — the golden-free oracle for
  //! SDF corner AA (Tier 4.5). A wrong half-extent / radius-clamp / premultiply
  //! in the shader would diverge here. reftests.md § CPU-vs-GPU SDF cross-check.

  use bevy::prelude::*;
  use buiy_core::render::DrawData;
  use buiy_verify::metric::FuzzBudget;
  use buiy_verify::reftest::run_sdf_cross_check;

  #[test]
  #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
  fn gpu_rounded_rect_matches_cpu_oracle() {
      let draw = DrawData::new(Vec2::new(40.0, 20.0), Vec2::new(120.0, 80.0), Color::WHITE, 16.0);
      // AA band tolerance: a sub-pixel rim may differ between the GPU `fwidth`
      // derivative and the CPU central-difference — the documented AA residue,
      // NOT a regression. Interior + exterior match bit-exactly; only the ~1px
      // rim is fuzzed. (Record the measured rim pixel count here after the run.)
      let fuzz = FuzzBudget { max_channel_delta: 12, max_diff_pixels: 600 };
      let outcome = run_sdf_cross_check(&draw, &fuzz);
      assert!(
          outcome.passed,
          "GPU vs CPU-SDF oracle diverged: {:?} (report: {:?})",
          outcome.diff, outcome.report_path
      );
  }
  ```

- [ ] **Run to verify it fails (compile).**
  ```sh
  cargo test -p buiy_verify --test sdf_cross_check_gpu --no-run
  ```
  Expected: compile error `cannot find function 'run_sdf_cross_check'`.

- [ ] **Write the minimal implementation.** In `crates/buiy_verify/src/reftest.rs`:
  ```rust
  /// Render the same single primitive on the GPU (one-instance capture) and on
  /// the CPU oracle, diff with the AA-aware metric. Tolerates sub-pixel AA noise
  /// via `fuzz`; zero stored bytes. Catches SDF AA / implementation drift no
  /// markup reftest can, and is kept PERMANENTLY (one shared analytic
  /// `sdf_rounded_rect`). A *spec* error in `sdf_rounded_rect` is invisible here
  /// (both paths share it) — that is Tier 5's job.
  pub fn run_sdf_cross_check(
      draw: &buiy_core::render::DrawData,
      fuzz: &FuzzBudget,
  ) -> RefOutcome {
      let (w, h) = REFTEST_LOGICAL;
      let cfg = GoldenConfig::deterministic();

      let mut app = crate::support::reftest_app(w, h);
      crate::support::clear_reftest_scene(&mut app);
      spawn_single_primitive(&mut app, draw);
      let gpu = capture_to_image(&mut app, &cfg);

      let cpu = sdf_oracle::rasterize_sdf_rect(draw, w, h);

      let diff = compare(&gpu, &cpu, &CompareOpts::reftest_default());
      let passed = diff.passes(fuzz);
      let report_path = if passed {
          None
      } else {
          Some(emit_report("sdf_cross_check", &gpu, &cpu, &diff))
      };
      RefOutcome { passed, diff, report_path }
  }

  /// Spawn one rounded-rect under a root, mapping `DrawData`'s position/size/
  /// radius to the layout components the extract path turns back into one
  /// `DrawData`. (Confirm the exact `Radius` component spelling against
  /// `render::components` at impl time.)
  fn spawn_single_primitive(app: &mut bevy::app::App, draw: &buiy_core::render::DrawData) {
      use bevy::prelude::*;
      use buiy_core::components::Node;
      use buiy_core::layout::{Inset, Length, Sizing, Style};
      use buiy_core::render::components::{Background, Radius};
      use buiy_core::render::ColorToken;
      use std::borrow::Cow;
      // The capture path resolves a token; install draw.color under a fixed key.
      let key = "sdf.cross.fill";
      {
          let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
          theme.colors.insert(key.into(), draw.color);
      }
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .absolute()
                  .inset(Inset {
                      left: Sizing::Length(Length::px(draw.position.x)),
                      top: Sizing::Length(Length::px(draw.position.y)),
                      ..default()
                  })
                  .width_px(draw.size.x)
                  .height_px(draw.size.y),
              Background { color: ColorToken::Token(Cow::Borrowed(key)) },
              Radius::circular(draw.radius),
          ))
          .id();
      app.world_mut().spawn((Node, Style::default())).add_children(&[e]);
  }
  ```
  *(`Radius::circular(px)` / `Radius::ZERO` is the real API at `render/components.rs:112-126`. The `Theme::colors.insert` + `Background`/`ColorToken` spellings are placeholders — confirm against the live `render::components` / `theme` API at impl time. The intent: one box whose extracted `DrawData` matches `draw`.)*

- [ ] **Run to verify it compiles.**
  ```sh
  cargo test -p buiy_verify --test sdf_cross_check_gpu --no-run
  ```
  Expected: compiles clean.

- [ ] **Run on the GPU lane.**
  ```sh
  cargo test -p buiy_verify --test sdf_cross_check_gpu -j 2 -- --ignored --test-threads=1
  ```
  Expected: `test result: ok. 1 passed`. If the AA rim exceeds the budget, adjust `fuzz` in the test with a *measured* comment — do NOT widen `max_channel_delta` past the interior's bit-exact agreement (the interior must match at delta 0; only the ~1px rim is fuzzed). Record the measured rim pixel count in the test comment.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/src/reftest.rs crates/buiy_verify/tests/sdf_cross_check_gpu.rs
  git commit -m "feat(verify): CPU-vs-GPU SDF cross-check (run_sdf_cross_check)

Renders one rounded-rect on the GPU and via the CPU oracle, diffs within a
documented AA fuzz budget. Zero stored bytes; kept permanently (one shared
analytic SDF). reftests.md § CPU-vs-GPU SDF cross-check.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

### Task 1b.12 — Two real reftest cases: flex-justify `==` literal offsets; content-visibility `!=` visible

The harness's payoff. Case 1 (`match`): a flex row with `justify-content: SpaceBetween` of three 40px boxes in a 200px row renders the same as three boxes at literal x = 0, 80, 160 via the primitive layer (reference routes through literal `Node` offsets, NOT flex — mechanism 1). Case 2 (`mismatch`): a subtree with `ContentVisibility::Hidden` renders **differently** from the identical visible subtree. Both `#[ignore]` — GPU; the cv reference's independence is asserted pure-CPU.

**Files:**
- Create: `crates/buiy_verify/tests/reftest_cases_gpu.rs` (the two `reftest!`-generated cases + scenes + a headless independence assertion for case 2)

Steps:

- [ ] **Write the two cases.** Create `crates/buiy_verify/tests/reftest_cases_gpu.rs`:
  ```rust
  //! GPU lane (`--ignored`): two real Tier-4 reftest pairings.
  //!   1. flex `justify-content: SpaceBetween` == three literal-offset boxes
  //!      (reference routes through the literal-Node layer — NOT flex). `match`.
  //!   2. `content-visibility: hidden` != the identical VISIBLE subtree — the
  //!      `!=` anti-test proving the feature suppresses paint. `mismatch`.
  //! reftests.md § Authoring patterns.

  use bevy::prelude::*;
  use buiy_core::components::Node;
  use buiy_core::layout::components::Containment;
  use buiy_core::layout::{
      ContentVisibility, FlexAxis, Inset, JustifyContent, Length, Sizing, Style,
  };
  use buiy_core::render::components::Background;
  use buiy_core::render::ColorToken;
  use std::borrow::Cow;

  fn fill_box(width: f32) -> impl Bundle {
      (
          Node,
          Style::default().width_px(width).height_px(40.0),
          Background { color: ColorToken::Token(Cow::Borrowed("test.fill.a")) },
      )
  }

  fn abs_box(app: &mut App, left: f32) -> Entity {
      app.world_mut()
          .spawn((
              Node,
              Style::default()
                  .absolute()
                  .inset(Inset {
                      left: Sizing::Length(Length::px(left)),
                      top: Sizing::Length(Length::px(0.0)),
                      ..default()
                  })
                  .width_px(40.0)
                  .height_px(40.0),
              Background { color: ColorToken::Token(Cow::Borrowed("test.fill.a")) },
          ))
          .id()
  }

  fn flex_justify(app: &mut App) {
      let a = app.world_mut().spawn(fill_box(40.0)).id();
      let b = app.world_mut().spawn(fill_box(40.0)).id();
      let c = app.world_mut().spawn(fill_box(40.0)).id();
      app.world_mut()
          .spawn((
              Node,
              Style::default()
                  .flex()
                  .flex_axis(FlexAxis::Row)
                  .justify_content(JustifyContent::SpaceBetween)
                  .width_px(200.0)
                  .height_px(40.0),
          ))
          .add_children(&[a, b, c]);
  }

  fn literal_offsets(app: &mut App) {
      let a = abs_box(app, 0.0);
      let b = abs_box(app, 80.0);
      let c = abs_box(app, 160.0);
      app.world_mut().spawn((Node, Style::default())).add_children(&[a, b, c]);
  }

  fn subtree(app: &mut App, hidden: bool) {
      let child = app.world_mut().spawn(fill_box(80.0)).id();
      let mut parent = app.world_mut().spawn((
          Node,
          Style::default()
              .absolute()
              .inset(Inset {
                  left: Sizing::Length(Length::px(20.0)),
                  top: Sizing::Length(Length::px(20.0)),
                  ..default()
              })
              .width_px(80.0)
              .height_px(40.0),
      ));
      if hidden {
          parent.insert(Containment { content_visibility: ContentVisibility::Hidden, ..default() });
      }
      let p = parent.id();
      app.world_mut().entity_mut(p).add_children(&[child]);
      app.world_mut().spawn((Node, Style::default())).add_children(&[p]);
  }

  fn cv_visible(app: &mut App) { subtree(app, false); }
  fn cv_hidden(app: &mut App) { subtree(app, true); }

  buiy_verify::reftest!(match, flex_justify_eq_literal, flex_justify, literal_offsets);
  buiy_verify::reftest!(mismatch, cv_hidden_actually_hides, cv_visible, cv_hidden);

  #[test]
  fn cv_hidden_reference_is_independent() {
      use buiy_verify::metric::FuzzBudget;
      use buiy_verify::reftest::{assert_reference_independent, default_rules, RefCase, RefKind};
      // The REFERENCE in case 2 is `cv_visible`; it must carry NO Hidden marker.
      let case = RefCase {
          name: "cv_hidden_actually_hides", kind: RefKind::Mismatch,
          test: cv_hidden, reference: cv_visible, fuzz: FuzzBudget::EXACT,
      };
      assert_reference_independent(&case, &default_rules());
  }
  ```
  *(Confirm `.flex()`/`.flex_axis`/`.justify_content`/`FlexAxis`/`JustifyContent` spellings against `crates/buiy_core/src/layout/style.rs`; the test must compile headless first.)*

- [ ] **Run the independence guard + confirm the GPU cases are ignored headless.**
  ```sh
  cargo test -p buiy_verify --test reftest_cases_gpu cv_hidden_reference_is_independent
  cargo test -p buiy_verify --test reftest_cases_gpu 2>&1 | grep -E "flex_justify_eq_literal|cv_hidden_actually_hides|ignored"
  ```
  Expected: `cv_hidden_reference_is_independent` passes; the two `reftest!` cases show as `ignored` headless.

- [ ] **Run the two real cases on the GPU lane.**
  ```sh
  cargo test -p buiy_verify --test reftest_cases_gpu -j 2 -- --ignored --test-threads=1
  ```
  Expected: `test result: ok. 2 passed`. If `flex_justify_eq_literal` fails by a 1px AA rim on a shared edge, widen its fuzz with `reftest!(match, flex_justify_eq_literal, flex_justify, literal_offsets, fuzz = (8, N))` citing the *measured* rim pixel count `N` (Mozilla discipline — a non-zero `Match` budget needs a measured reason; ranges must not include 0).

- [ ] **Run the full project gate (headless).**
  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && xvfb-run -a cargo test --workspace
  ```
  Expected: all green; the GPU test files contribute only `ignored` counts here.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_verify/tests/reftest_cases_gpu.rs
  git commit -m "feat(verify): two real Tier-4 reftest cases

flex justify-content: SpaceBetween == three literal-offset boxes (reference
routes through the primitive layer, NOT flex — independence by construction);
content-visibility: hidden != the visible subtree (the != anti-test). The
cv reference's independence is asserted pure-CPU. reftests.md § Authoring
patterns.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

**Phase 1b exit criteria:**
- **Headless gate green:** the pure-CPU reftest meta-tests (RefKind parse, `evaluate_outcome` truth table, `mismatch_floor_ok`, the independence lint RED/GREEN, the SDF-oracle point-probes, the two cases' independence guard) all pass; every GPU test reports `ignored`.
- **GPU lane green:** `reftest_engine_gpu` (3), `reftest_macro_gpu` (1), `sdf_cross_check_gpu` (1), `reftest_cases_gpu` (2), `render_capture_app_gpu` (1) all pass on the RX 6700 XT.
- **The harness can fail:** `match_of_two_different_scenes_fails` + `mismatch_of_scene_with_itself_fails` confirm a vacuous green is impossible.
- **Reference independence is enforced + tested:** `assert_reference_independent` rejects an illegal marker (RED) and passes the disjoint one (GREEN); the value-encoded caveat is documented.
- **Deliberately absent:** the forced-colors `BoxShadow` visual reftest (BLOCKED on the unlanded `BoxShadow` extract/draw path) is NOT authored; multiple-references aggregation is a deferred follow-up.

---

## Phase 2-4 — Task outlines (JIT-expanded)

These three phases are **outlines**, not bite-sized tasks: each lists files to create/modify (exact paths), the spec signatures to implement, the RED tests to write (intent + assertion), success criteria, and the gate to run. JIT-expand each task into the canonical RED→verify-fail→GREEN→verify-pass→commit five-step shape (per the writing-plans format used in Phase 0/1) when you reach it — the signatures and assertions below are pinned, so the expansion is mechanical. **Phase 2** (snapshots + invariants) is pure-CPU/headless and closes gates #5 + #12; **Phase 3** (determinism + golden persistence) carries the GPU `#[ignore]` parts; **Phase 4** (coverage + forced-colors live wiring + docs flip) composes everything and ends the campaign.

> **Prerequisite (all three phases):** Phase 0 (deps + `Dpr` + `capture_to_image`) and Phase 1 (`metric` + `reftest`) are landed. Phase 2 needs only Phase 0's `insta` dep + Phase 1's `metric`; Phase 3 needs Phase 0's `Dpr`/`capture_to_image` + Phase 1's `metric` + `reftest`; Phase 4 needs all of Phase 2/3. The headless/GPU gate definitions are in § "Phasing & ordering" above.

---

### Phase 2 — Tier 1-2 snapshots + Tier 3 invariants (pure-CPU, headless)

**Closes gate #5 (layout snapshots) and gate #12 (property invariants); adds the missing Tier-2 display-list gate.** Every task runs under the **headless gate** — no `#[ignore]`, no GPU adapter. `insta` (Phase 0.1) is the only dep; `proptest` is already a `buiy_verify` dep so Tier 3 adds zero crates. The migration discipline is **replace, don't duplicate**: keep each test's scene construction + intent comment, collapse only the trailing field-by-field assert block into one snapshot; asserts pinning a *single named invariant* (e.g. `render_buckets.rs:9` `Shadow.paint_order() < Quad…`, the GC cardinality checks) **stay** as `assert!`/`assert_eq!`.

#### Task 2.1 — Shared dump primitives: `round` + format-version headers

- **Files — create:** `crates/buiy_verify/src/snapshot/mod.rs`, `crates/buiy_verify/src/snapshot/dump.rs`. **Modify:** `crates/buiy_verify/src/lib.rs` (add `pub mod snapshot;`). **Test:** `crates/buiy_verify/tests/snapshot_dump.rs` (new).
- **Signatures:** `const ROUND_DP: usize = 2;` and `fn round(f32) -> String` (shared by Tier 1 + Tier 2); `const LAYOUT_DUMP_VERSION: &str = "# buiy-layout-dump v1"` and `const DISPLAY_LIST_DUMP_VERSION: &str = "# buiy-display-list-dump v1"`.
- **RED test:** `round_table` — `round(1.005) == "1.0"`, `round(50.0) == "50"`, `round(-0.001) == "0"` (sub-ULP + negative inputs, snapshots.md § Verification #2).
- **Success:** `round` deterministic across the table; constants exist. (Header tripwire tests land with the dumps in 2.2/2.4.)
- **Gate:** headless.

#### Task 2.2 — Tier 1: `layout_dump` + `assert_layout_snapshot`

- **Files — create:** `crates/buiy_verify/src/snapshot/layout.rs`. **Modify:** `crates/buiy_verify/src/snapshot/mod.rs` (re-export). **Test:** `crates/buiy_verify/tests/snapshot_layout.rs` (new self-tests) + the migration in `crates/buiy_core/tests/layout.rs:33`.
- **Signatures:** `pub fn layout_dump(world: &World) -> String` (`(name, position, size)` per entity, sorted by `Name` then `Entity` index, indentation via `ChildOf`, floats via `round`, `LAYOUT_DUMP_VERSION` header, `entity#<idx>` fallback for unnamed); `pub fn assert_layout_snapshot(app: &mut App, name: &str)` (one `update()`, then `insta::assert_snapshot!`).
- **RED tests (self-tests, plain asserts):** `dump_is_entity_order_invariant` (same fixture in two differently-spawned apps ⇒ `assert_eq!(layout_dump(a), layout_dump(b))`, snapshots.md § Verification #1); `layout_dump_has_version_header` (first line `== LAYOUT_DUMP_VERSION`, § Verification #4).
- **Migration:** `layout.rs:33` — replace the `(layout.size.x - 50.0).abs() < 0.5` pair in `layout_resolves_a_simple_flex_row` with `assert_layout_snapshot(&mut app, "flex_row_basic")` after tagging entities with `Name`. The two `layout_tree_garbage_collects_*` tests **stay** plain `assert_eq!` (cardinality, not geometry).
- **Success:** self-tests green; first run blesses `flex_row_basic.snap`; reviewer diffs the `.snap` against the old `< 0.5` asserts. `.snap` committed under `tests/snapshots/`. `INSTA_UPDATE=no` in CI ⇒ an unreviewed `.snap.new` fails.
- **Gate:** headless.

#### Task 2.3 — Tier 2: `NameLookup` + `instance_hex` / `assert_instance_hex_snapshot`

- **Files — create:** `crates/buiy_verify/src/snapshot/display_list.rs` (NameLookup + instance hex first; the dump in 2.4). **Test:** `crates/buiy_verify/tests/snapshot_instance_hex.rs` (new).
- **Signatures:** `pub struct NameLookup(HashMap<Entity,String>)` + `NameLookup::from_world(world)` (World-built once so the dump stays World-free, per README § Resolved #5); `pub fn instance_hex(p: &PackedInstance) -> String` (`bytemuck::bytes_of`, host-endian — document the little-endian-x86-64 assumption); `pub fn assert_instance_hex_snapshot(p: &PackedInstance, name: &str)`.
- **RED test:** `hex_round_trips_bytes` — `instance_hex(p)` → parse → `bytemuck::pod_read_unaligned::<PackedInstance>` → `assert_eq!` reconstructed `== p` (snapshots.md § Verification #3). `PackedInstance` shape per `render/instance.rs:41`.
- **Success:** round-trip green; hex byte-exact and format-version-free.
- **Gate:** headless.

#### Task 2.4 — Tier 2: `display_list_dump` + `assert_display_list_snapshot`

- **Files — modify:** `crates/buiy_verify/src/snapshot/display_list.rs`. **Test:** `crates/buiy_verify/tests/snapshot_display_list.rs` (new self-tests).
- **Signatures:** `pub fn display_list_dump(nodes: &ExtractedNodes, names: &NameLookup) -> String` (nodes in `painters_z` stored order — never re-sorted, `extract.rs:141`; color as `token:<Name>` when resolvable else `#rrggbbaa`; `clip=none`/`min..max`; `group=<idx>|none`; then `pack_view()` `InstanceBuckets` in `BTreeMap` draw order with per-batch `xN` counts; `DISPLAY_LIST_DUMP_VERSION` header); `pub fn assert_display_list_snapshot(nodes: &ExtractedNodes, name: &str, names: &NameLookup)`.
- **RED tests (self-tests):** `display_dump_is_entity_order_invariant` (Name-keyed, two differently-spawned worlds equal); `display_dump_has_version_header`; `missing_token_surfaces_as_magenta` (a `MISSING_TOKEN_FALLBACK`-color node dumps as `#ff00ffff`).
- **Success:** self-tests green; the dump pins per-node set + batched draw order in one artifact; `InstanceBuckets` counts in the readable dump, exact payload in the hex check (complementary).
- **Gate:** headless.

#### Task 2.5 — Migrate the five render `assert_eq!` tests to snapshots

- **Files — modify (keep scene + intent comment, collapse trailing asserts):**
  - `crates/buiy_core/tests/render_extract.rs` — per-field `node.position/size/color/clip` + the `assemble_context_tree` order `assert_eq!` (`:423`) → one `assert_display_list_snapshot`.
  - `crates/buiy_core/tests/render_buckets.rs` — `b.len(q0)`/`total_instances`/`batch[0]`/`PackedPartition` field asserts (`:239`) → display-list dump + `assert_instance_hex_snapshot`. **Keep** `render_buckets.rs:9` `Shadow.paint_order() < Quad…` as a plain assert.
  - `crates/buiy_core/tests/render_paint_order.rs` — `assert_eq!(tail, vec![...])` (`:64`) → display-list dump.
  - `crates/buiy_core/tests/render_instance.rs` — per-field `PackedInstance` asserts incl. the half-size sign regression → `assert_instance_hex_snapshot`.
  - `crates/buiy_core/tests/top_layer.rs` — `partition_top_layer` order asserts → display-list dump.
  - Each `use buiy_verify::snapshot::…` (Phase 0 already gave `buiy_core` the `buiy_verify` + `insta` dev-deps).
- **RED step (per file):** first `cargo test` produces `.snap.new` (RED: no committed `.snap`); `cargo insta review` the diff against the old per-field asserts, accept, commit the `.snap`.
- **Migration behavior-preserving (mandatory):** for `render_instance.rs`, after migrating, *re-introduce* the half-size sign bug in a scratch edit and confirm `assert_instance_hex_snapshot` now fails (hex flips) — then revert. Same mutation check for the `assemble_context_tree` order in `render_extract.rs` (snapshots.md § Verification #5).
- **Success:** all migrated tests green with committed `.snap`s; the mutation checks confirm teeth; headless gate green (§ Verification #6). `.snap` under `crates/buiy_core/tests/snapshots/`.
- **Gate:** headless.

#### Task 2.6 — Tier 2 opt-in: per-timestamp animation snapshots

- **Files — modify:** `crates/buiy_verify/src/snapshot/display_list.rs`. **Test:** `crates/buiy_verify/tests/snapshot_animation.rs` (new).
- **Signature:** `pub fn assert_display_list_snapshot_at(app: &mut App, name: &str, steps: &[std::time::Duration])` — advances `Time<Virtual>` to each absolute logical time (the landed manual clock, `tests/text_caret_selection.rs:178`), emits `display_list_dump` per step, keyed `<name>@<t_ms>`. Default three timestamps.
- **RED test:** `per_timestamp_is_deterministic` — drive a caret-blink fixture through `&[ZERO, mid, end]` twice on fresh apps; `assert_eq!` the per-step dumps. Opt-in: this fixture enrolls *because* its timing curve is the behavior under test.
- **Success:** one `.snap` per step; a timing regression shows as a diff in exactly the drifted frame. Pure-CPU.
- **Gate:** headless.

#### Task 2.7 — Tier 3 scaffolding: `invariant` module + `Scene` model + generators

- **Files — create:** `crates/buiy_verify/src/invariant/mod.rs`, `…/invariant/scene.rs`. **Modify:** `crates/buiy_verify/src/lib.rs` (`pub mod invariant;`). **Test:** `crates/buiy_verify/tests/scene_generator_smoke.rs` (bounds) + the predicate proptests in 2.9–2.10.
- **Signatures:** `pub struct SceneNode { name, children, z_index: Option<i32>, isolation: bool, top_layer: TopLayer, transform: GenTransform, size: (f32,f32), background: Option<TokenRef> }`; `pub struct Scene { roots: Vec<SceneNode> }`; `pub struct SceneParams { max_depth=4, max_breadth=4, max_nodes=24, p_stacking=0.3, p_top_layer=0.1 }`; `pub fn arb_scene(p: SceneParams) -> impl Strategy<Value=Scene>` (`prop_recursive`); `pub fn realize(scene: &Scene) -> ExtractedNodes` (through the production `assemble_context_tree`/`partition_top_layer`, no GPU). `GenTransform` draws `Translate -512..512`, `Rotate 0..2π`, `Scale 0.1..8.0` (identity reachable); `z_index` from `{-1,0,1,2}`; `top_layer` all five variants skewed to `None`; name uniqueness via pre-order rename `n0..nK`. **No new dep.**
- **RED test:** `arb_scene_respects_bounds` — `proptest!`: every realized scene has `node_count <= max_nodes` and `depth <= max_depth`.
- **Success:** generator terminates, shrinks legibly; `realize` round-trips a `Scene` through the CPU paint path into a flat paint-ordered list.
- **Gate:** headless.

#### Task 2.8 — `buiy_core` surface add: promote `tier_rank` → `top_layer_paint_rank`

- **Files — modify:** `crates/buiy_core/src/layout/systems.rs:4113` (extract the private `tier_rank` closure body into `pub fn top_layer_paint_rank(t: TopLayer) -> u8`; have the existing layout sort call it). **Modify:** `crates/buiy_core/src/layout/mod.rs` (re-export). **Test:** `crates/buiy_core/tests/layout_stacking.rs` (rank-mapping assert).
- **Signature:** `pub fn buiy_core::layout::top_layer_paint_rank(TopLayer) -> u8` mapping `Fullscreen→0, Tooltip→1, Popover→2, Modal→3, None→u8::MAX` (README § Resolved #3 / invariants.md deviation #3 — the *declared* enum order is NOT the paint order, so `#[derive(Ord)]` is wrong; compare via this rank).
- **RED test:** `paint_rank_matches_documented_order` — `assert_eq!(top_layer_paint_rank(Fullscreen), 0)` … `(None) == u8::MAX`; and the layout sort still produces the same tail order (behavior-preserving).
- **Success:** the closure is gone, the `pub fn` is the only rank source; existing `layout_stacking.rs` tests stay green. A small, accepted `buiy_core` surface add.
- **Gate:** headless.

#### Task 2.9 — Tier 3 predicates #1–#5 (paint-order, transform, top-layer, finiteness, contexts)

- **Files — create:** `crates/buiy_verify/src/invariant/predicates.rs`. **Modify:** `…/invariant/mod.rs` (the `proptest!` harness + mutation-fixture `#[test]`s). **Test:** `…/mod.rs` (proptest blocks) + `crates/buiy_verify/tests/invariant_mutations.rs`.
- **Signatures (each `pub fn … -> Result<(), Violation>`; `Violation { rule: &'static str, detail: String }`, no `thiserror`):**
  - `paint_order_is_total(nodes)` — no entity twice; equal-paint-key pairs keep document order (`extract.rs:139`).
  - `transform_roundtrips(t: &GenTransform)` — on the **composed** `Mat4` from `compose_transform` (`systems.rs:3775`, compose `T·R·S·M`), within `EPS`: `translate(d)·translate(-d) ≈ I`; `rotate(2π) ≈ I`; `scale(k)` scales geometry by `k`, off-diagonals stay 0.
  - `top_layer_dominates(nodes)` — every `top_layer != None` paints after every normal node; escaped tail ordered by `top_layer_paint_rank` (Task 2.8), **never** the discriminant.
  - `all_finite(nodes)` — every `ExtractedNode.size.{x,y} ≥ 0` and finite (`extract.rs:73`).
  - `all_finite_packed(packed)` — every field finite and `rect_size[1] ≥ 0` *directly* (y-flip in the view uniform, `instance.rs:46`, deviation #2 — no un-flip).
  - `contexts_do_not_interleave(nodes, scene)` — no entity of context A between two of context B.
- **RED tests — proptest blocks** (`ProptestConfig { cases: 256, max_shrink_iters: 4096 }`): `prop_paint_order_total`, `prop_transform_roundtrips`, `prop_top_layer_dominates`, `prop_all_finite`, `prop_contexts_no_interleave`.
- **RED tests — mutation fixtures (teeth):** duplicate-entity ⇒ `Err`; mis-composed `S·R·T` ⇒ `Err`; `Modal` (rank 3) before `Fullscreen` (rank 0) ⇒ `Err` (**fails if anyone "fixes" the predicate to use the discriminant** — pins deviation #3); `NaN`/negative `size.y` ⇒ `Err`; positive packed `rect_size[1]` ⇒ `Ok`; hand-built interleaved list ⇒ `Err`. Each with a passing control.
- **Persistence:** `proptest-regressions/invariant/<file>.txt` is **committed**, not gitignored.
- **Success:** all proptest blocks green at 256 cases; every mutation fixture rejects its one broken relation; controls pass.
- **Gate:** headless.

#### Task 2.10 — Tier 3 predicate #6: BiDi caret round-trip (`bidi.rs`)

- **Files — create:** `crates/buiy_verify/src/invariant/bidi.rs`. **Modify:** `…/invariant/mod.rs`. **Test:** same `mod.rs`.
- **Signatures:** `pub fn arb_bidi_text(max_runs, max_run_len) -> impl Strategy<Value=String>` (alternating LTR/RTL runs + neutrals); `pub fn bidi_caret_roundtrips(text: &str, metrics: Metrics) -> Result<(), Violation>` — on the **landed shaper** (`cosmic_text::Buffer` through the production text stack, same path as `tests/text_shaping_snapshots.rs`): **#6a** logical↔visual caret round-trip is identity over every grapheme boundary; **#6b** within one `LayoutRun`, visual caret order is monotonic in logical order for `rtl==false`, strictly reversed for `rtl==true`; **#6c** the run partition covers every codepoint exactly once across `Buffer::layout_runs()`. Uses `cosmic_text::Cursor` (`text/components.rs:10`).
- **RED tests:** `prop_bidi_caret_roundtrips` (`proptest` over `arb_bidi_text` ⇒ `.is_ok()`); mutation fixtures — the six shaping-snapshot scripts as known-good controls ⇒ `Ok`; an off-by-one caret-map fixture ⇒ `Err`.
- **Success:** proptest green; controls pass; the off-by-one fixture rejected. **Closes gate #12.**
- **Gate:** headless.

**Phase 2 exit criteria:** headless gate fully green; gate #5 satisfied by `assert_layout_snapshot`; gate #12 satisfied by the six proptest predicates + mutation fixtures; the new Tier-2 display-list gate live; all migrated `render_*`/`layout.rs`/`top_layer.rs` tests carry committed `.snap`s; `proptest-regressions/` committed. No GPU touched.

---

### Phase 3 — Determinism stack + Tier 5 golden persistence (headless + GPU `#[ignore]`)

**Realizes the determinism substrate (`DeterministicApp` + `GoldenConfig` extensions + CI lavapipe pin) and the Tier-5 stored-golden corpus.** Two halves: the **pure-CPU half** (config types, golden persistence/ledger/triage — headless gate) and the **GPU half** (`capture_to_image` quiescence, knob-sensitivity, end-to-end goldens — `#[ignore]`, GPU lane). New deps: `toml = "0.8"` + `base64 = "0.22"` — gate on `cargo deny check` before adding. The Ahem `.ttf` is a committed fixture, not a dep.

> Phase 0 already promoted `capture_to_image` and defined the canonical `Dpr`. Phase 3 *extends* `GoldenConfig` and *hardens* that capture primitive.

#### Task 3.1 — Extend `GoldenConfig`: `FontMode`, `Dpr` field, MSAA/dither constants

- **Files — modify:** `crates/buiy_core/src/render/golden.rs` (add `font_mode: FontMode` + `dpr: Dpr` fields; `enum FontMode { Real, Ahem }`; `deterministic()` defaults `font_mode: Ahem`, `dpr: Dpr::X1`; `fidelity()` = `font_mode: Real`; the `CAPTURE_MSAA`/`CAPTURE_DITHER_OFF` consts already landed in Phase 0.4). Struct stays `Copy`. **Test:** `crates/buiy_core/tests/render_golden_config.rs` (new).
- **RED tests:** `deterministic_defaults_collapse_font_axis` (`deterministic().font_mode == FontMode::Ahem`, `.dpr == Dpr::X1`); `fidelity_uses_real_font` (`fidelity().font_mode == FontMode::Real`, other knobs pinned).
- **Note:** Phase 0.4's `capture_to_image` ignores `cfg.dpr` (it sizes via the window). Phase 3.3 makes `capture_to_image` assert `scale_factor == cfg.dpr.as_f32()`; the field exists from this task so 3.3 can read it. **MSAA/dither are constants, never per-fixture knobs.**
- **Success:** config compiles `Copy`; defaults are the deterministic values.
- **Gate:** headless.

#### Task 3.2 — Ahem font asset + registration through the production bytes path

- **Files — create:** `crates/buiy_core/tests/fixtures/fonts/Ahem.ttf` (committed WPT Ahem, license file beside it, mirroring the `OFL-*.txt` precedent). **Modify:** `crates/buiy_verify/src/determinism/mod.rs` (font wiring helper). **Test:** `crates/buiy_verify/tests/determinism_ahem.rs` (new).
- **Wiring:** register via `FontRegistry::register_bytes("Ahem", ahem_bytes, FontFaceDescriptors::default())` (`registry.rs:165`) under family `"Ahem"`; when `font_mode == Ahem`, make it the **sole resolvable family** for fixture text (disable system-font loading; fixtures run bundled-only, `tests/support/mod.rs:292`/`:306`).
- **RED test:** `ahem_is_sole_family_under_ahem_mode` — register Ahem, resolve a fixture string under `FontMode::Ahem`, assert the resolved face family is `"Ahem"` (no fallback face). Pure-CPU (shaping/resolve, no rasterizer).
- **Success:** Ahem loads through the real bytes path; under Ahem mode it is the only resolvable family. Real vs Ahem is a per-fixture declaration (default Ahem).
- **Gate:** headless.

#### Task 3.3 — Async-asset flush to quiescence in `capture_to_image`

- **Files — modify:** `crates/buiy_core/src/render/golden.rs` (the `capture_to_image` body — replace Phase 0.4's bounded fixed-frame loop with: drive `app.update()` until the four conditions hold, bounded by `MAX_SETTLE_FRAMES`, panic naming the unmet condition; add the `scale_factor == cfg.dpr.as_f32()` assertion). **Test:** `crates/buiy_core/tests/render_capture_quiescence.rs` (new).
- **Quiescence conditions (generalizes `wait_for_text_ready`, `support/mod.rs:266`):** (1) `asset_server` pending loads `== 0`; (2) `AtlasWarmupQueue::is_empty()` (`golden.rs:87`); (3) `fonts_ready(atlas, warmup, &keys)` (`golden.rs:82`); (4) `PipelineCache` has no `Queued`/`Compiling` Buiy pipeline. Time advances via `Time::<Virtual>::advance_by`, never `Instant::now()`.
- **RED tests:** **GPU `#[ignore]`** `quiescence_panics_on_never_loading_asset` — inject a never-loading asset (or undrained warmup queue), assert `capture_to_image` **panics naming the unmet condition** (determinism.md § Verification #3 — fail loudly). **Headless** `capture_path_has_no_instant_now` — a grep-lint `#[test]` asserting `Instant::now()` does not appear in the capture path source (§ Verification #4).
- **Success:** quiescence loop terminates deterministically under the fixed clock; the panic fires with the named condition; no wall-clock read.
- **Gate:** the grep-lint headless; the never-loading-asset panic test **GPU lane** (`#[ignore]`).

#### Task 3.4 — `DeterministicApp` builder (`buiy_verify::determinism`)

- **Files — create:** `crates/buiy_verify/src/determinism/mod.rs` (+ `lib.rs` `pub mod determinism;`). Re-export `FontMode`/`Dpr` from `buiy_core::render::golden`, do **not** redefine. **Test:** the idempotent/knob-sensitivity tests in 3.5.
- **Signatures:** `pub struct DeterministicApp { cfg: GoldenConfig, logical: (u32,u32) }`; `new(w,h)`, `with(cfg)`, `font_mode(m)`, `dpr(d)`; `pub fn build(self) -> App` (a **single-bodied** wrapper over the landed `capture_app_scaled(w, h, cfg.dpr.as_f32())` so it cannot drift; applies `TimeUpdateStrategy::ManualDuration(0)` + manual `Time<Virtual>`; registers Ahem + sole-family when `font_mode==Ahem`; capture camera at `CAPTURE_MSAA`, dither off); `pub fn capture(self, fixture: impl FnOnce(&mut App)) -> RgbaImage` (`build` + spawn + `capture_to_image`).
- **RED test:** `build_applies_dpr_and_msaa` — `DeterministicApp::new(64,64).dpr(Dpr::X2).build()`; assert the window `scale_factor == 2.0` and the capture camera carries `Msaa::Off`. (CPU-observable on the built app; no readback.)
- **Success:** `build` is a thin single-bodied wrapper; knobs applied + asserted; **`run_reftest`'s `support::reftest_app` (Phase 1b) is re-pointed to `DeterministicApp::new(w,h).build()` in this task** — the one-line swap the 1b seam was designed for; the 1b reftest `#[ignore]` cases re-run green to pin behavior across the swap.
- **Gate:** headless (building/inspecting the app); the *capture* is GPU (3.5). The reftest re-run is GPU lane.

#### Task 3.5 — Determinism self-tests: idempotent capture + knob sensitivity (GPU)

- **Files — create:** `crates/buiy_verify/tests/determinism_capture.rs` (`#[ignore]`, GPU lane).
- **RED tests (all `#[ignore]`):** `idempotent_capture` — capture the same fixture twice in two fresh `DeterministicApp`s ⇒ `compare(a, b, &CompareOpts::default()).passes(&FuzzBudget::EXACT)` (determinism.md § Verification #1); `knob_sensitivity_dpr` (`dpr(X1)` vs `dpr(X2)` **differ** — `!passes(&EXACT)`); `knob_sensitivity_font_mode` (`Real` vs `Ahem` of a text fixture differ); `knob_sensitivity_msaa` (a fixture with MSAA forced on differs from `CAPTURE_MSAA`). § Verification #2 — the knobs are load-bearing.
- **Success:** idempotent capture passes at `(0,0)`; every knob flip changes the bytes.
- **Gate:** **GPU lane**; headless stays green independently.

#### Task 3.6 — Tier-5 keys + ledger types (`buiy_verify::golden`, pure-CPU)

- **Files — create:** `crates/buiy_verify/src/golden.rs` (+ `lib.rs` `pub mod golden;`). **Deps:** add `toml = "0.8"` + `base64 = "0.22"` to `[workspace.dependencies]` and `buiy_verify` — **run `cargo deny check` first** (both MIT/Apache-2.0). **Test:** `crates/buiy_verify/tests/golden_keys.rs` (new).
- **Signatures:** `pub struct GoldenKey { widget, state, theme, viewport, backend: Backend, dpr: Dpr }` (imports canonical `Dpr` from `buiy_core::render::golden`); `pub enum Backend { Lavapipe, Vulkan, Gl, Metal, Dx12 }`; `GoldenKey::slug()` (deterministic lower-kebab `widget/state/theme__viewport__backend__dpr`), `GoldenKey::dir(root)`; `pub struct BlessLedger { key, positives: Vec<Positive> }`; `pub struct Positive { file, blessed_commit, blessed_at, budget: FuzzBudget, reason }` (serde, TOML on disk).
- **RED test:** `key_slug_round_trips` — `proptest`: a `GoldenKey` round-trips through `slug()`→parse; two distinct keys never collide (goldens.md § Verification #6).
- **Success:** key schema is **fixed before any golden is generated** (skia-gold lesson); ledger serializes to human-diffable TOML.
- **Gate:** headless.

#### Task 3.7 — `check_golden`/`assert_golden` + multi-positive + bless workflow

- **Files — modify:** `crates/buiy_verify/src/golden.rs`. **Test:** `crates/buiy_verify/tests/golden_persistence.rs` (new; all pure-CPU — synthesize `RgbaImage`s in memory).
- **Signatures:** `pub fn check_golden(key, actual: &RgbaImage, budget: &FuzzBudget) -> GoldenOutcome` (compares `actual` against each stored positive via `metric::compare`, passes if *any* `Diff::passes(budget)`; on fail carries the best/smallest-Diff candidate); `pub enum GoldenOutcome { Pass{matched_positive, diff}, Fail{best, report}, Blessed{positive, was_new} }`; `pub fn assert_golden(key, actual: &RgbaImage, budget: &FuzzBudget)` (panics on non-bless `Fail`; on `BUIY_BLESS=1` blesses). Default budget after the determinism pin is `(0,0)`.
- **RED tests (pure-CPU, goldens.md § Verification #1–#4):** `match_and_mismatch`; `multi_positive_any_matches` (bless two positives, image matching the second ⇒ `Pass { matched_positive: 1 }`); `bless_round_trip` (`BUIY_BLESS=1` blesses to a temp corpus, re-run without env passes, ledger records commit/timestamp/reason); `fail_closed_on_empty_corpus` (empty corpus + unset env ⇒ `assert_golden` **panics with the bless instruction**, à la `text_shaping_snapshots.rs:301`).
- **Success:** set-valued match + budget gate work without a renderer; bless env-gated (`BUIY_BLESS`, modeled on `BUIY_ACCEPT_SHAPING`), never a silent overwrite. The stale-positive guard (`golden-prune` bin) is **advisory, deferred**.
- **Gate:** headless.

#### Task 3.8 — Diff-PNG + self-contained HTML triage report

- **Files — modify:** `crates/buiy_verify/src/golden.rs` (or `…/golden/report.rs`). **Test:** `crates/buiy_verify/tests/golden_report.rs` (new).
- **Signatures:** `pub struct TriageReport { path, cards: Vec<TriageCard> }`; `pub struct TriageCard { key, actual_png, baseline_png, diff_png, diff: Diff, budget: FuzzBudget }`; `open_or_create(path)`, `push(card)`, `write()` (one self-contained HTML: side-by-side expected|actual, JS opacity-slider overlay, diff heatmap — all PNGs base64-inlined). On any `Fail`: write `target/buiy-goldens/<slug>.diff.png` (the `Diff::diff_image` heatmap) + append a card to `target/buiy-goldens/report.html`.
- **RED test:** `report_is_self_contained` — generate a `TriageReport` with one card, `write()`, assert the HTML **contains the base64 PNGs and references no external URL** (grep for `http`/`src="./"` ⇒ absent). Offline-first (goldens.md § Verification #5).
- **Success:** the report opens straight from CI artifacts, no network/SaaS. Time-boxed-ignore + flaky-auto-ignore are **deferred follow-ups**.
- **Gate:** headless.

#### Task 3.9 — End-to-end goldens per residue class (GPU) + storage hygiene

- **Files — create:** `crates/buiy_verify/tests/goldens.rs` (`#[ignore]`, GPU lane) + the blessed corpus under `crates/buiy_verify/tests/goldens/`. **Modify:** `.gitattributes` (add `crates/buiy_verify/tests/goldens/*.png -text`, mirroring the `*.snap -text` pin).
- **RED tests (`#[ignore]`, one per residue class — goldens.md § Verification #7):** SDF corner AA (beyond the CPU cross-check), shadow blur kernel, real-font glyph (one pinned bundled OFL font, `FontMode::Real`), color-emoji (the irreducible golden — pinned bundled emoji font, generous per-fixture budget). Plus the **Ahem layout-class** golden asserting *both* byte-identity across two fresh captures **and** equality to the stored positive (proving the box-font collapse holds). Each captured via `capture_to_image` under `DeterministicApp`, blessed once with `BUIY_BLESS=1 cargo test -p buiy_verify --test goldens -- --ignored --test-threads=1`.
- **Storage:** positives in-git under `tests/goldens/`, reviewed as the PR diff. **Migration trigger named now:** total in-git golden bytes > 50 MB OR positive count > 500 ⇒ move to commit-hash-keyed object storage (a *step, not a crisis*).
- **Success:** each residue class has a blessed positive passing on the pinned rasterizer; the Ahem golden double-asserts the collapse.
- **Gate:** **GPU lane**; headless stays green.

#### Task 3.10 — CI lavapipe pin (composite action + env contract)

- **Files — create:** `.github/actions/install-mesa/action.yml` (consume `gfx-rs/ci-build`'s prebuilt lavapipe tarball — no self-build; pin `MESA_VERSION` + `ci-binary-build` tag explicitly). **Modify:** the CI workflow to invoke it on the golden leg and export the env contract.
- **Env contract:** `VK_DRIVER_FILES=$PWD/icd.json` (the action writes its **own** ICD JSON so the loader sees *only* lavapipe); `WGPU_ADAPTER_NAME=llvmpipe`. **NOT set:** `LP_NUM_THREADS` (README § Resolved #6 / determinism.md deviation #1 — determinism comes from the pinned Mesa version). Use `VK_DRIVER_FILES`, not the deprecated `VK_ICD_FILENAMES` (deviation #2).
- **RED test (CI-only smoke, determinism.md § Verification #5):** `lavapipe_adapter_selected` — on the lavapipe leg, assert the selected adapter name contains `llvmpipe` **before any golden runs** (the pin is active, not silently falling back to hardware).
- **Success:** CI goldens run on pinned lavapipe; the local lane runs on the RX 6700 XT but does **not** compare against the lavapipe baseline (cross-rasterizer pixels are non-comparable — it runs the rasterizer-internal determinism/reftest checks). One canonical rasterizer ⇒ one golden per cell; `backend` is a constant today.
- **Gate:** CI (lavapipe leg). The smoke guard is CI-only; locally the GPU lane runs against real hardware for determinism/reftest checks.

**Phase 3 exit criteria:** headless gate green (config types, golden persistence/ledger/triage, all pure-CPU self-tests); GPU lane green on the RX 6700 XT (idempotent capture at `(0,0)`, knob-sensitivity negatives, end-to-end goldens per residue class, the 1b reftests re-run through `DeterministicApp`); CI lavapipe pin wired with the `llvmpipe` smoke guard; `toml`+`base64` cleared by `cargo deny check`; `.gitattributes` pins `goldens/*.png -text`.

---

### Phase 4 — Coverage-by-construction + forced-colors live wiring + docs flip (headless + GPU `#[ignore]`)

**Composes every prior tier: a `Fixture` corpus × a global `Matrix` Cartesian product auto-enrolls each fixture across all five tiers, and `forced_colors_analyzer` is re-pointed from hand-built `CatalogPaint` at the live widget catalog (closing gate #11's live-catalog half).** New dep: `inventory = "0.3"` — gate on `cargo deny check`. `insta`'s `glob` feature (Phase 0.1) drives the snapshot tiers' fixture-dir fan-out. The pure-CPU coverage self-tests run on the **headless gate**; only `coverage_golden` is `#[ignore]` GPU.

#### Task 4.1 — `Fixture` corpus + `fixture!` macro + `inventory` catalog

- **Files — create:** `crates/buiy_verify/src/coverage/mod.rs`, `…/coverage/fixture.rs` (+ `lib.rs` `pub mod coverage;`); the first fixtures under `crates/buiy_verify/fixtures/<widget>/<state>.rs` (start with `button/resting.rs` from the `hello_button` spawn). **Deps:** add `inventory = "0.3"` — **run `cargo deny check` first**. **Test:** the `verify_catalog_matches_glob` self-test in 4.5.
- **Signatures:** `pub struct Fixture { name: &'static str, state: &'static str, spawn: fn(&mut App) }`; `pub fn catalog() -> &'static [Fixture]` (`inventory`-collected); the `fixture!` macro emitting **both** an `inventory::submit!` **and** a glob-discoverable file. The `spawn` MUST spawn a `Camera2d` and tag the widget root with a `Name`. State (resting/hover/focus/pressed/disabled) is **per-fixture** (one file per state), encoded by spawning the widget already in that state.
- **RED test:** (deferred to 4.5's `verify_catalog_matches_glob`).
- **Success:** a fixture is the catalog row authored once, the same `fn(&mut App)` every tier consumes; `catalog()` enumerates via `inventory`.
- **Gate:** headless.

#### Task 4.2 — `Matrix` + `Cell` + `CoverageKey`

- **Files — create:** `crates/buiy_verify/src/coverage/matrix.rs`, `…/coverage/key.rs`. **Test:** the `verify_keys_unique` / `verify_cell_count_under_ceiling` self-tests in 4.5.
- **Signatures:** `pub struct Matrix { themes: Vec<ThemeAxis>, viewports: Vec<Viewport>, forced_colors: Vec<bool>, dprs: Vec<Dpr> }` (imports canonical `Dpr`, **not** a local `f32`); `enum ThemeAxis { Light, ForcedColors }` (`build() -> Theme` via `default_light_theme`/`forced_colors_theme`; `key()`); `struct Viewport { w, h, key }`; `Matrix::ci_default()` (≈ 2 themes × 3 viewports × 2 fc × 2 dpr = 24 cells/fixture); `Matrix::cells() -> impl Iterator<Item=Cell>` (stable axis-declaration order); `struct Cell { theme, viewport, forced_colors, dpr }`; `pub struct CoverageKey { widget, state, theme, viewport, forced_colors, dpr: Dpr, backend: Backend }` derives `Eq + Hash` (**because `dpr: Dpr` is `Eq + Hash`** — the old `f32` made this impossible, the bug this fix unblocks); `CoverageKey::for_cell(fx, cell, backend)`; `CoverageKey::stem()` (e.g. `button.resting.forced.desktop.fc1.dpr2.lavapipe`).
- **RED test:** (keying self-tests in 4.5).
- **Success:** `CoverageKey` derives `Eq + Hash` so keys collect into a `HashSet`; `backend` is `cpu` for Tiers 1–3, the rasterizer name for GPU — reserved now to avoid the painful retrofit.
- **Gate:** headless.

#### Task 4.3 — `enroll_all` + `build_app` (the one-body-per-tier driver)

- **Files — create:** `crates/buiy_verify/src/coverage/enroll.rs`. **Test:** the `enrollment_fan_out` self-test in 4.5.
- **Signatures:** `pub fn build_app(fx: &Fixture, cell: &Cell) -> App` (`DeterministicApp` with `cell.theme.build()` installed, viewport + `DeterministicApp::dpr(cell.dpr)` pinned — the `Dpr`→`f32` conversion happens **here** via `cell.dpr.as_f32()` — `forced_colors` set on `UserPreferences`, then the fixture spawned); `pub fn enroll_all(matrix: &Matrix, body: impl Fn(App, CoverageKey))` (drives `body` across `catalog() × matrix.cells()`).
- **RED test:** (fan-out totality in 4.5).
- **Success:** each tier is a thin caller of `enroll_all`; no per-widget test code exists; the `Dpr` milliscale stays the key, the window `scale_factor` is the derived `f32`.
- **Gate:** headless.

#### Task 4.4 — Per-tier enrollment tests (the five `coverage_*.rs` drivers)

- **Files — create:** `crates/buiy_verify/tests/coverage_layout.rs` (Tier 1, gate #5), `coverage_display_list.rs` (Tier 2), `coverage_invariants.rs` (Tier 3), `coverage_golden.rs` (Tier 5, `#[ignore]` GPU). Each is a `#[test]` calling `enroll_all(&Matrix::ci_default(), |app, key| { … })`.
- **Bodies:** layout → `assert_layout_snapshot(&key.stem(), &app)` (the `insta` tiers additionally use `glob!` over the fixture dir as the collection-time fan-out); display-list → `assert_display_list_snapshot(&key.stem(), …)`; invariants → for each Tier-3 predicate, assert on the realized scene; golden → `let img = capture_to_image(&mut app, &cfg); assert_golden(&key-derived GoldenKey, &img, &budget_for(&key))`.
- **RED step:** first run of the snapshot drivers produces `.snap.new` per cell ⇒ `cargo insta review` → accept → commit. The golden driver is GPU `#[ignore]`, blessed via `BUIY_BLESS=1`.
- **Success — the decisive property:** adding `fixtures/slider/resting.rs` enrolls a slider into **all five tiers at once** **with no edit to any test file**.
- **Gate:** `coverage_layout`/`_display_list`/`_invariants` headless; `coverage_golden` **GPU lane**.

#### Task 4.5 — Coverage harness self-tests

- **Files — create:** `crates/buiy_verify/tests/coverage_meta.rs` (all pure-CPU, headless).
- **RED tests (coverage.md § Verification #1–#5):** `verify_catalog_matches_glob` (`catalog()` and the `glob!` walk enumerate the identical `name×state` set); `verify_keys_unique` (over `catalog() × Matrix::ci_default().cells()`, every `stem()` unique and round-trips; keys collect into a `HashSet`); `verify_cell_count_under_ceiling` (product size below the named CI ceiling); `enrollment_fan_out` (a stub tier body pushing its `CoverageKey` into a `Vec` asserts `enroll_all` invokes the body exactly `fixtures × cells` times with **no duplicate key**).
- **Success:** enumeration/keying verified independent of any tier's pass/fail.
- **Gate:** headless.

#### Task 4.6 — `forced_colors_analyzer` live-catalog producer (gate #11)

- **Files — create:** `crates/buiy_verify/src/coverage/forced_colors.rs`. **Test:** `crates/buiy_verify/tests/coverage_forced_colors.rs` (new, pure-CPU, gate #11).
- **Signature:** `pub fn live_catalog_paint() -> Vec<CatalogPaint>` — walk the live catalog: for each fixture build its app, query the spawned `Background`/`Border`/`Outline` (+ shadow-only-delta) off the `Name`-tagged root, project into the **existing** `CatalogPaint`. The analyzer (`analyze_forced_colors`/`analyze_shadow_only`, `forced_colors_analyzer.rs:51`/`:89`) is called **unchanged** — only its *input source* moves from hand-built fixtures (`tests/render_forced_colors_analyzer.rs:11`) to the live tree (the live components exist: `buiy_widgets/src/button.rs:18,47` spawns `Background`/`Border`/`Corners`/`Radius`, closing follow-ups.md:469–473).
- **RED tests:** `live_catalog_has_no_forced_colors_violations` (`analyze_forced_colors(&live_catalog_paint(), &forced_colors_theme()).is_empty()` + `analyze_shadow_only(...).is_empty()`); `broken_fixture_produces_violation` (a `#[cfg(test)]`-only fixture painting a **brand** token under forced-colors **must** produce a `NonSystemColor` violation through `live_catalog_paint` — proves the producer observes *real paint*; excluded from the real `catalog()` so it never reds production).
- **Success:** gate #11's live-catalog half falls out of the same enrollment; every new widget auto-enrolls into the forced-colors scan by construction.
- **Gate:** headless.

> **BLOCKED — forced-colors `BoxShadow` *visual* reftest (do NOT plan as runnable).** The residual forced-colors *visual* half — the `BoxShadow` draw-skip under `forced-colors: active` — is a Tier-4 reftest **blocked on the unlanded `BoxShadow` extract/draw path** (`extract_buiy_nodes` has no `BoxShadow` branch yet; follow-ups.md:474–478). Coverage only enrolls the forced-colors **mode** (`forced_colors: true` cell) into every tier so the visual reftest is *matrixed* once it exists. The structured `analyze_forced_colors`/`analyze_shadow_only` gate (Task 4.6) covers the rest **now**. Track the visual reftest as a follow-up keyed to the `BoxShadow` pipeline landing; **do not author a runnable RED test for it**.

#### Task 4.7 — Docs flip (spec draft→active; README; follow-ups; verification gate progress)

- **Files — modify:**
  - `docs/specs/2026-06-15-buiy-verification-design/README.md` + each child file (`metric.md`, `snapshots.md`, `invariants.md`, `reftests.md`, `goldens.md`, `determinism.md`, `coverage.md`) — flip `**Status:** draft` → `**Status:** active` (or `implemented`, matching the T9 closure precedent), with a one-line "landed" note per file pointing at this plan/commits. **`metric.md` additionally records the pixelmatch-vendoring deviation** (it no longer depends on the crate).
  - `docs/README.md` — line 49 catalog entry: `[draft]` → `[active]`; add this plan under **Foundation → Plans** (`[landed]`).
  - `docs/plans/follow-ups.md` — mark the forced-colors live-catalog seam (lines 462–473) **resolved**; leave the `BoxShadow` draw-skip visual reftest (lines 474–478) **open**, cross-referenced to this campaign; record the deferred golden primitives (time-boxed ignore, flaky auto-ignore, `golden-prune` advisory bin, object-store migration trigger) **and the deferred reftest multiple-references aggregation** as named follow-ups.
  - `docs/specs/2026-05-07-buiy-foundation/verification.md` — mark gate progress in the CI-gates table: **#2** (visual — relational reftests + residue goldens + metric + determinism landed), **#5** (layout snapshots — `assert_layout_snapshot` landed), **#11** (forced-colors — live-catalog token-flow scan landed; the *visual* `BoxShadow` half still blocked — note it), **#12** (property tests — the six invariants + mutation fixtures landed).
- **Verify step:** docs-only — the "test" is a consistency check: `grep` confirms no child file still says `draft`; the `docs/README.md` tag matches; the verification.md rows reference the landed mechanisms. Run the **headless gate** once more (`RUSTDOCFLAGS="-D warnings" cargo doc` catches stale intra-doc links if doc-comments reference the new modules).
- **Success:** the spec is `active`/`implemented`, not contradicted by the code; the docs index current; gates #2/#5/#11/#12 show landed mechanisms; follow-ups records what was deferred + still blocked.
- **Gate:** headless (the full gate, as a final closeout run).

**Phase 4 exit criteria:** headless gate green (the four coverage self-tests, the three pure-CPU enrollment drivers, the forced-colors live-catalog scan + its broken-fixture teeth test); GPU lane green (`coverage_golden`); `inventory` cleared by `cargo deny check`; adding one fixture file demonstrably enrolls it across all five tiers with zero test-file edits; the docs flip complete and gates #2/#5/#11/#12 reflect the landed mechanisms. The `BoxShadow` forced-colors *visual* reftest remains an open, blocked follow-up.

---

## Self-review

Run against the writing-plans self-review checklist and the spec.

### (a) Spec coverage — every tier / gate → the task that implements it

| Spec element | Child file | Implementing task(s) |
|---|---|---|
| Capture seam promotion (`capture_to_image`) | README § Architecture; determinism.md § "Where the code lives" | **0.4** (mechanics) + **3.3** (quiescence + dpr assertion) |
| Canonical `Dpr` milliscale type | determinism.md § "Extending GoldenConfig" | **0.3** |
| Dev-only `buiy_core → buiy_verify` cycle | metric.md § Migration | **0.2** (edge) + **1a.10** (consumed) |
| **Tier 4/5 metric** (`Diff`/`FuzzBudget`/`CompareOpts`/`compare`/`passes`/`within`) | metric.md | **1a.1–1a.6** |
| Metric known-answer meta-suite (§4 dilution, AA, dim-mismatch) | metric.md § Verification | **1a.2** (scale-invariant), **1a.3** (AA), **1a.4** (dim-mismatch), **1a.7** (suite + constants pin) |
| Advisory MSSIM (never gates) | metric.md § "Advisory MSSIM" | **1a.5** |
| Migrate the two naive metrics | metric.md § Migration | **1a.8** (RMSE delete), **1a.9** (perceptual_diff deprecate), **1a.10** (text_gpu sites) |
| **Tier 4 reftests** (`RefCase`/`RefKind`/`RefOutcome`/`reftest!`/`run_reftest`) | reftests.md | **1b.2–1b.8** |
| Reftest aggregation truth table + mismatch-floor guard | reftests.md § Verification #1, #2 | **1b.4** (truth table), **1b.7** (floor) |
| Harness-can-fail (vacuous-green guard) | reftests.md § Verification #3 | **1b.5/1b.6** (known-good/known-bad GPU pairs) |
| Reference-independence lint (RED/GREEN-tested) | reftests.md § "Reference independence", Verification #4 | **1b.9** |
| **Tier 4.5** CPU-vs-GPU SDF cross-check | reftests.md § "CPU-vs-GPU cross-check", Verification #5 | **1b.10** (oracle) + **1b.11** (cross-check) |
| Two real reftest cases | reftests.md § "Authoring patterns" | **1b.12** |
| **Tier 1** layout-number snapshots (gate #5) | snapshots.md | **2.1**, **2.2** |
| **Tier 2** display-list / `PackedInstance`-hex snapshots | snapshots.md | **2.3**, **2.4**, **2.5**, **2.6** |
| Snapshot order-invariance + version tripwire + behavior-preserving migration | snapshots.md § Verification | **2.2/2.4** (order + header), **2.5** (mutation re-check) |
| `top_layer_paint_rank` promotion | README § Resolved #3; invariants.md deviation #3 | **2.8** |
| **Tier 3** proptest predicates (gate #12) | invariants.md | **2.7** (generators), **2.9** (predicates #1–5 + mutations), **2.10** (BiDi #6) |
| `GoldenConfig` extensions (FontMode, Dpr field, MSAA/dither) | determinism.md | **3.1** (config), **0.4** (MSAA/dither consts) |
| Ahem layout-determinism font | determinism.md § "Ahem"; flutter prior-art | **3.2** |
| Quiescence flush + no-`Instant::now` | determinism.md § "Async-asset flush", Verification #3/#4 | **3.3** |
| `DeterministicApp` builder | determinism.md | **3.4** (+ re-points the 1b reftest seam) |
| Idempotent capture + knob-sensitivity negatives | determinism.md § Verification #1/#2 | **3.5** |
| **Tier 5** goldens (`GoldenKey`/`Backend`/`BlessLedger`/`check_golden`/`assert_golden`) | goldens.md | **3.6**, **3.7** |
| Multi-positive + bless + fail-closed | goldens.md § Verification #1–#4 | **3.7** |
| Self-contained HTML triage report | goldens.md § Verification #5 | **3.8** |
| End-to-end goldens per residue class | goldens.md § Verification #7 | **3.9** |
| CI lavapipe pin (`VK_DRIVER_FILES`, no `LP_NUM_THREADS`) | determinism.md § "lavapipe pin"; README § Resolved #6 | **3.10** |
| **Coverage** `Fixture`/`Matrix`/`Cell`/`CoverageKey`/`enroll_all` | coverage.md | **4.1**, **4.2**, **4.3**, **4.4** |
| Coverage self-tests (catalog↔glob, key-uniqueness, fan-out) | coverage.md § Verification #1–#5 | **4.5** |
| `forced_colors_analyzer` live-catalog producer (gate #11) | coverage.md; README § gate #11 | **4.6** |
| Docs flip (spec→active, README, follow-ups, verification.md) | CLAUDE.md docs discipline | **4.7** |

Every spec tier, every named `§ Verification` meta-test, and every foundation gate (#2/#5/#11/#12) maps to at least one task. No spec element is unaddressed.

### (b) Placeholder scan of Phase 0/1 — must be clean (real code, no TBD)

Phases 0, 1a, 1b contain **full, real code in every implementation step** — every type, fn body, test, and command is concrete. There are **no `TBD`/`TODO`/`???`/`<placeholder>` tokens in any code block.** The non-code "confirm against the live API at impl time" notes are deliberate and bounded — each names the exact symbol to verify (e.g. `image-compare`'s `rgba_blended_hybrid_compare`, the `Radius::all` spelling, the `tests/support/mod.rs:168` plugin list, the `a11y`/`contrast` smoke symbols) and gives the contract to preserve if the spelling differs. These are *grounding instructions*, not unwritten code: the algorithm, control flow, and assertions are all present. The one literal requiring a live-run confirmation — the `(3, 255, 64)` constants tuple in **1a.7** — has an explicit bless step that reads the actual value from the failure message. **Scan result: clean.**

### (c) Type-consistency check across tasks

Verified the load-bearing type names are identical across every task and match the spec child files:

- **`Diff`** — fields `differing_pixels: u32`, `max_channel_delta: u8`, `total_pixels: u32`, `mssim: Option<f64>`, `diff_image: Option<RgbaImage>` — identical in 1a.1 (def), 1a.4 (`passes`/`within`), 1b.4 (`stub_diff`), 2.9 (predicates consume `ExtractedNodes`, not `Diff`), 3.7/3.8 (golden). ✓
- **`FuzzBudget`** — `{ max_channel_delta: u8, max_diff_pixels: u32 }` + `EXACT` const — identical in 1a.1, 1b (all reftest tasks), 3.x (goldens). Matches metric.md §73–82. ✓
- **`CompareOpts`** — `{ threshold, include_aa, mssim, emit_diff_image }` + `Default` + `reftest_default()` — 1a.1 (def), 1b.1 (`reftest_default`). ✓
- **`Dpr`** — `Dpr(u32)` milliscale, `X1`/`X2`, `from_f32`/`as_f32` — defined once in 0.3; imported (not redefined) by 3.1 (`GoldenConfig.dpr`), 3.6 (`GoldenKey.dpr`), 4.2 (`Matrix.dprs`/`CoverageKey.dpr`). Matches determinism.md §68–75. ✓
- **`RefCase`** — `{ name, kind, test: fn(&mut App), reference: fn(&mut App), fuzz: FuzzBudget }` — identical in 1b.3 (def), 1b.5/1b.8/1b.9/1b.12 (use). Single-reference (the spec's multi-reference is a deferred follow-up, flagged below). ✓
- **`RefKind`** / **`RefOutcome`** — `{Match, Mismatch}` / `{ passed, diff, report_path }` — consistent across 1b.2–1b.12. ✓
- **`Backend`** — `{ Lavapipe, Vulkan, Gl, Metal, Dx12 }` — `GoldenKey.backend` (3.6) and `CoverageKey.backend` (4.2) name the same enum. Matches goldens.md §58. ✓
- **`GoldenKey` / `CoverageKey`** — both carry `dpr: Dpr` + `backend: Backend`; `CoverageKey` derives `Eq + Hash` *because* `Dpr` is `Eq + Hash` (the milliscale payoff, stated identically in 0.3, 4.2, and coverage.md §122). ✓
- **`FontMode`** — `{ Real, Ahem }` defined on `GoldenConfig` (3.1), re-exported by `determinism` (3.4), never redefined. ✓

No type is defined twice; every cross-task reference uses the canonical name and shape.

### Gaps fixed / flagged during assembly

1. **`pixelmatch` is vendored, not depended on** (1a deviation note). The published crate is unusable (PNG-stream input, flat-count output, private primitives, `image` 0.24-bound). The plan vendors the ~150 LOC YIQ + AA algorithm into `metric.rs`. **`metric.md` § "Crate choice" / "Migration" should be corrected** to say "vendored from the pixelmatch reference," not "depends on `pixelmatch`." Net Phase-1a dep delta is `image-compare` only. *(Flagged for the doc-flip in Task 4.7.)*
2. **Dev-dep edge de-duplicated.** The Phase 0 and Phase 1a drafts both added the `buiy_core → buiy_verify` dev-dep. Resolved: **0.2 is the canonical site**; **1a.10** now only *verifies* it is present (re-adds defensively if absent). No double-add.
3. **`capture_app` promotion is an addition to Phase 0's scope.** Phase 1b needs a painting `App` builder from `src`; the spec only promoted `capture_to_image`. The plan promotes `capture_app`/`capture_app_scaled` in **1b.6** (single-body with the test-support builder, anti-drift). This is consistent with README § Architecture's "promote the shared seam into `render/golden.rs` src" but extends it — noted so the impl author honors it.
4. **`reftest!` macro surface uses an `$fn:ident`**, not the spec's `$name:literal`, because `match` is a keyword and two `reftest!(match, …)` would collide. The generated fn is named from the ident; `stringify!($fn)` is the `RefCase.name`. Documented in 1b.8.
5. **Multiple-references aggregation is deferred.** reftests.md § "Reference independence" #3 specs `RefCase::multi` / `reference: &[fn]` (Match = OR, Mismatch = AND). Phase 1b builds single-reference only — it covers both real cases + the cross-check. The `evaluate_outcome` split keeps the aggregation addable without reworking the engine. **Recorded as a follow-up in Task 4.7's `follow-ups.md` edit.** *(Open gap for the reviewer: confirm single-reference is acceptable for v1, or pull multi-reference forward into Phase 1b.)*
6. **`insta` constants-pin deferred within Phase 1a.** metric.md § Verification wants the constants tripwire as a floats-redacted `insta` snapshot; 1a.7 uses an exact-integer `assert_eq!` instead (Phase 2 introduces the snapshot dump infra). Behavior-identical, no vacuous pass. The upgrade is folded into Phase 2's snapshot work.
7. **MSAA/dither constants land in Phase 0.4, not Phase 3.** The drafts had `CAPTURE_MSAA`/`CAPTURE_DITHER_OFF` in both Phase 0.4 (capture camera) and Phase 3.1 (`GoldenConfig`). Resolved: the **consts land in 0.4** (the capture camera needs them); **3.1** only adds the `FontMode`/`dpr` config fields and references the existing consts.
