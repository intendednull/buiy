# Testing infrastructure & coverage audit

**Date:** 2026-06-18
**Scope:** the *whole* test estate on `main` — all four crates + examples + workspace tests, the `buiy_verify` 5-tier harness, CI (`ci.yml`), determinism/flakiness posture, golden/snapshot lifecycle, build ergonomics, and test-code maintainability. Not a re-review of `buiy_verify` internals (that was [2026-06-15](2026-06-15-verification-harness-adversarial-review.md)); this audit builds on it.
**Verdict:** Infrastructure is **strong and unusually disciplined for the project's age**; the suite is green. Exposures are concentrated and actionable: **9 high-severity findings** (reproducibility, two CPU-observable behaviors gated behind the GPU-only lane, two ordering subsystems with their core logic unexercised, a parallel-reimplementation trap in the metamorphic tier, a cross-rasterizer golden-integrity contradiction, an orphaned GPU regression test in no CI lane, and the per-file test-binary explosion behind the CI disk/link instability) plus a deeper P1/P2 backlog and **five whole dimensions the suite does not test at all** (runnable doctests, performance, code-coverage measurement, two of three example e2e paths, CI robustness guards).

**Resolution:** ✅ **RESOLVED 2026-06-19** by the [testing-audit resolution campaign](../plans/2026-06-18-testing-audit-resolution-campaign.md) — all 43 findings + 2 confirmed-dismissed cleanups landed on `worktree-testing-audit-report` (Phases 0–6, each task adversarially reviewed + mutation-verified). Final both-lane gate green via `cargo-nextest`: headless **1257 passed / 77 ignored** + **4 doctests** = **1338** (exact parity preserved through the **162→7** test-binary consolidation); GPU `--ignored` lane **76/0** on the RX 6700 XT; `fmt`/`clippy`/`doc`/`deny` clean; cold full-run **~63 min → ~1.7 min**. Two real production bugs were fixed en route (non-finite `ResolvedLayout` sizes; grapheme-correct Backspace). The findings below are retained as the historical evidence; see the plan for per-finding resolution + commit IDs.

## Method & confidence

Multi-agent audit, fresh cold-context throughout, with adversarial verification:

1. **15 parallel dimension auditors** — 5 subsystem-coverage (layout / render / text-render / text-edit+IME / cross-cut a11y·picking·focus·theme), 4 infrastructure (CI, harness integrity, determinism/flakiness, golden+snapshot), 4 redundant-lens quality (tier-misplacement ×2 lenses, anti-patterns ×2 lenses), 2 build (ergonomics, test-code maintainability).
2. **Adversarial per-finding verification** — every high/medium finding was handed to an independent skeptic prompted to *refute* it against the actual code (a coverage "gap" survived only if the skeptic could not find a covering test). **80 findings raised, 79 survived, 1 refuted** (the "SDF is only GPU-checked" claim — see *Investigated & dismissed*). Several were severity-adjusted down by their verifier; the adjusted view is what is recorded.
3. **Completeness critic** — a fresh agent re-examined the synthesized report for dimensions the *audit itself* missed; it surfaced 5, each independently re-verified by grep before inclusion (see *Completeness-pass additions*).
4. **Live ground truth** — the headless gate (`cargo test --workspace`, no `--ignored`) was run on real hardware during the audit; result folded into *Live verification* below. The GPU `#[ignore]` lane was not re-run here (it is additive and exercised locally + in CI).

64 agents total. Every P0 claim in this report was additionally hand-checked by the author against the source before publication. Findings carry the severity their verifier settled on; where two dimensions found the same thing they are merged with both citations.

## Live verification

- **Headless gate** (`cargo test --workspace -j 2`, `DISPLAY=:0`, no `--ignored`) was run on real hardware during the audit: **1154 passed, 0 failed, 83 ignored** across 175 test-result groups (the ~162 per-file integration binaries + per-crate lib-unit + doctest sections). Suite is **green** with zero panics.
- **Cold wall-clock: ~63 minutes** (`04:49 → 05:52`, `-j 2`). This is finding #9 made concrete — the per-file binary explosion plus the link-OOM-mitigation `-j 2` cap make a from-cold full run very slow; this is the inner-loop tax the consolidation in #9 targets.
- **Doctests ran 0 tests** (every per-crate `--doc` section reports `0 passed`), directly corroborating finding #39 — all doc code fences are `ignore`, so the doc estate is compile-checked by nothing.
- The GPU `#[ignore]` lane (124 core + 32 verify ignored tests) is the additive Tier-4/5 lane; not re-run in this audit.

## Coverage map

| Subsystem | Coverage | Dominant tier | Biggest gap |
|---|---|---|---|
| **Layout** | Strong (most mature in the repo) | T1 layout-number + T3 invariants | Flex justify/align distribution pinned **only** in the GPU lane; intrinsic-sizing variants + grid track-size functions (MinMax/AutoFit/FitContent) untested; degenerate sizes never driven through the pipeline |
| **Render (pipeline/draw/SDF)** | Good headless spine + GPU residue | T1–T3 headless (prepare/extract/instance/display-list), T4–T5 GPU | `coverage.wgsl`/`composite.wgsl` never naga-parsed headless; shadow `erf`/Gaussian has no oracle (path is dead code today); schedule-graph introspection GPU-gated by RenderApp adapter init |
| **Text render (shape/glyph/decoration/effects)** | Strong, tier-disciplined | T1 math + curated `.snap` shaping corpus | `script_fallback` net + color-emoji skip decision untested; decoration band geometry re-asserted in GPU goldens the extract harness already covers |
| **Text edit / IME / clipboard / undo** | Well-covered core | Pure ops units + system-level | Redo never driven through the keyboard system (undo is); grapheme-correct delete fixtures (spec-required) missing; undo proptest never creates a selection |
| **Cross-cutting (a11y / picking / focus / theme)** | Uneven — forced-colors strong, picking/focus weak | T1 system snapshots | **Picking z/overlap resolution untested (1 node/test); focus `tab_order` priority+skip branches dead;** `Hovered` consumer chain never asserted end-to-end; a11y description/skip branches untested |
| **Cross-cutting infra** (doctests, perf, coverage tooling, example e2e, CI robustness) | **Absent** | — | No runnable doctests; no performance/benchmark gate; no coverage measurement; `hello_text`/`capture` examples never run-asserted; no CI `timeout-minutes`/`concurrency` |

## Prioritized findings

### P0 — fix before more features

**1. `Cargo.lock` is gitignored — deny/build/cache run on a floating dependency graph** (high, hand-verified)
`.gitignore:10` lists `Cargo.lock`; `git ls-files Cargo.lock` is empty. Workspace deps are caret/float (`bevy="0.18"`, `taffy="0.10"`, `serde="1"`, `arboard="3.6"`). `ci.yml` uses `--locked` only for the wgpu-info install (line 174), never for test/deny/doc. This is a `publish=false` workspace, so committing the lockfile is the correct choice — without it `cargo deny`, the build, and the rust-cache key all float and `main` can go red with no code change.
→ Commit `Cargo.lock`; add `--locked` to the test/deny/doc cargo steps; bump deps in dedicated PRs.

**2. Flex `justify-content`/`align-items` child distribution is verified ONLY in the GPU `#[ignore]` lane** (high, hand-verified)
The only distribution assertion is `crates/buiy_verify/tests/reftest_cases_gpu.rs:137` (`flex_justify_eq_literal`); `reftest!` emits `#[ignore]` (`reftest.rs:494-496`). `layout_style_equivalence.rs:57` sets `SpaceBetween` only on a single childless container. The headless `test` job runs without `--ignored` (`ci.yml:114`). A child-distribution regression — a plain layout number — ships green on every PR.
→ Add a headless T1 layout-number test: three 40px boxes in a 200px row, assert `ResolvedLayout` x=0,80,160 for SpaceBetween (plus SpaceAround/Evenly, AlignItems::Center/FlexEnd). Keep the GPU reftest as the rasterized cross-check.

**3. `coverage.wgsl` + `composite.wgsl` have no headless naga-parse** (high, hand-verified)
`render_shader_wgsl.rs:17-18` `include_str!`s only `shader.wgsl` + `shadow.wgsl`. The other two shaders exist (`crates/buiy_core/src/render/{coverage,composite}.wgsl`), are loaded via `Shader::from_wgsl` (`render/mod.rs:215,225`), but their compilation is deferred to the device-backed PipelineCache — every test that compiles them is `#[ignore]`. A WGSL syntax/binding regression passes the headless merge gate.
→ Extend `render_shader_wgsl.rs` with two `include_str!` consts + naga-parse/entry-point assertions.

**4. Picking top-most / z-resolution completely untested — every test has one node** (high, hand-verified)
`hit_test` (`picking/mod.rs:42-44`, smallest-area-wins) and `emit_picks` (`backend.rs:53`, depth=area-rank) *are* the subsystem's value, yet `picking.rs` spawns 1 node and `picking_backend.rs:74` asserts only `.any(|(e,_)| *e==entity)` — never depth or order. A flipped comparator or dropped tiebreak passes every test.
→ `hit_test` test with two overlapping AABBs (smaller-area wins) + backend test asserting `picks[0]` is the smaller node with ascending `HitData` depths. Pure/headless.

**5. Focus explicit-priority and Skip (negative `tab_order`) branches are dead** (high, hand-verified)
`compute_next_focus` (`focus.rs:85`, private) branches on `tab_order`: positive=priority, 0=document order, negative=skip (`filter(|f| f.tab_order >= 0)` line 92; sort key line 99). `grep tab_order` across all test crates returns **zero** matches — every focus test uses `Focusable::default()` (0). The skip filter and the priority sort are entirely unexercised.
→ Make `compute_next_focus` `pub(crate)`; unit-test `tab_order=-1` (skipped) and mixed positives (1,2 before Auto, ascending).

**6. Tier-3 metamorphic invariant re-implements paint sub-pass 6f instead of calling production** (high)
`invariant/scene.rs:374-388` manually rebuilds `painters_z` with a local `paint_key` (`scene.rs:636-643`) keyed only on `z_index`, vs production `paint_key` (`systems.rs:3902`) keyed on `PositionKind`. Nothing in `buiy_core` is exported to call (`paint_key` is `pub(super)`; 6f is inline). The 2026-06-15 fault injection confirmed reversing production 6f was caught only by `buiy_core`'s `z_index_*` unit tests, **not** the Tier-3 invariant. Tracked open in `follow-ups.md`.
→ Extract the 6f per-context assembly to a pure `buiy_core::pub fn painters_z_for_context(...)` (re-export `paint_key`); have `realize` call it. This also auto-closes the PositionKind generator gap (#13).

**7. `golden_sdf_corner` asserts a lavapipe-EXACT baseline with no adapter gating — the documented local real-GPU lane hard-fails** (high)
`goldens.rs:288-303` calls `assert_golden(..., &FuzzBudget::EXACT)` with no `#[cfg]`/adapter guard; `check.rs:179-184` gates against the recorded `(0,0)` budget. Commit `b869eba` records `max_channel_delta=35` for RX 6700 XT vs lavapipe, and `determinism.md:170` cements "the local lane does not compare against the stored lavapipe baseline." No adapter probing exists anywhere in `buiy_verify`, so running the documented `-p buiy_verify --ignored` lane on real hardware hard-fails — directly contradicting the design doctrine.
→ Gate the committed-corpus comparison on the selected adapter being the pinned lavapipe (probe `RenderAdapterInfo`/`WGPU_ADAPTER_NAME`, **skip-as-pending** otherwise — not fail — mirroring `matrix_goldens`); OR split into a rasterizer-internal self-check (bless-to-temp + EXACT re-capture) plus a CI-lavapipe-only committed-baseline leg.

**8. Facade render-finish regression test runs in NO CI lane** (high, hand-verified)
`crates/buiy/tests/plugin.rs:50` (`facade_render_finish_registers_device_resources`) is `#[ignore]` — the only ignored test in the `buiy` crate. The GPU lane runs only `-p buiy_core` + `-p buiy_verify --ignored` (`ci.yml:184-185`); the headless job skips `#[ignore]`. So this guard for a real, documented production bug (Bevy `App::finish` snapshots `plugin_registry.len()` before the loop, so `BuiyRenderPlugin` added in `finish` never gets `finish()` → `AtlasGpu` panics on frame 1 of `hello_button`) runs nowhere.
→ Add `cargo test -p buiy -j 2 -- --ignored --test-threads=1` to the GPU lane (or broaden to `--workspace --ignored`).

**9. ~162 per-file integration-test binaries each fully link bevy — the SIGBUS/OOM root cause** (high, hand-verified)
124 `buiy_core` + 32 `buiy_verify` (+ 6 across `buiy`/`widgets`/workspace) test files, each its own integration binary statically linking bevy + buiy_core; no `harness=false`/`[[test]]` anywhere. CI's free-disk + `debuginfo=0` + `-j 2` mitigations only cap a cost that is *linear in binary count* — `ci.yml` itself attributes the `Bus error`/`No space left` to "the growing set of test binaries." The 17 KB `tests/support/mod.rs` recompiles ~28×.
→ Consolidate per-theme/per-feature files into a handful of binaries that `mod` the current ones (≈124 → <10), eliminating the harness re-compile fan-out. (`cargo-nextest` would also help isolation + flaky-retry; see #10/backlog.)

**10. GPU lane cache fragmented by divergent RUSTFLAGS + no shared key** (adjusted: medium)
The test job sets `debuginfo=0` via `CARGO_PROFILE_*` (`ci.yml:111-113`); the gpu job via `RUSTFLAGS -C debuginfo=0` (line 142). rust-cache hashes RUSTFLAGS into its key, and all four `rust-cache@v2` invocations are bare (no `shared-key`), so the GPU lane never warm-starts from the headless deps. Cost-only.
→ Unify the debuginfo mechanism (`[profile.test] debug=0`) **and** add a rust-cache `shared-key`.

### P1 — soon

**11. Redo never driven through the keyboard system** (high)
`EditCommand::Redo` appears in tests only via `apply_tracked` (`text_undo_property.rs:133`, `text_undo_ops.rs:59`). Undo *is* system-tested (`text_undo_system.rs:108` drives a real Ctrl/Cmd-Z asserting `EditUndone`); no test sends a redo chord. Untested production: `input.rs:434` (`'z' if shift => Redo`), `input.rs:436` (`'y' if !macos`), the `EditRedone` emit at `input.rs:586`.
→ System test mirroring undo: type → undo → Ctrl-Shift-Z (and Ctrl-Y non-macOS), assert value restored + exactly one `EditRedone`.

**12. Quiescence conditions remain GPU-lane-only** (reconciled to low after verification)
`quiescence_unmet` (`golden.rs:360`) early-returns at `golden.rs:377` (`get_sub_app(RenderApp)?`), gating conditions 2–4 headlessly. Verification found condition 3's `fonts_ready` *is* already headless-tested (`render_golden_harness.rs:232`, non-ignored), covering condition 2's predicate too. The genuine residual is **condition 4 only** (PipelineCache `Queued/Creating`, inline `golden.rs:398-408`, no pure helper, no headless test).
→ Extract condition 4 behind `fn no_pipeline_compiling(&PipelineCache)`; unit-test against synthetic state.

**13. PositionKind generator gap — local `paint_key` collapses production's tier-2 class** (medium)
Production `paint_key` (`systems.rs:3902`) emits `(2,0)` for positioned+auto-z (line 3911); `scene.rs:636-643` has no `(2,_)` arm, and `SceneNode` carries no `PositionKind` axis. They agree only because the generated domain enforces `positioned ⟺ z_index.is_some()`, so the Tier-3 proptests pass over a strictly smaller domain than production. (Production tier-2 *is* unit-tested at `systems.rs:4700` — purely a generator-coverage gap.)
→ Add a `PositionKind` axis to `SceneNode`/`arb_leaf`; land with #6.

**14. Tier-5 stored goldens cover 1 of 6 design-claimed residue classes; the matrix driver verifies 0 images** (high)
Committed corpus is exactly 2 PNGs — `rect-rounded` (SDF corner AA) + `text-ahem` (box-glyph layout, not a residue class). Shadow-kernel, color-emoji, blend/gamma, effect-compositor, forced-colors-visual (`goldens.md:40-43`) have no golden. `coverage_golden::matrix_goldens` (`coverage_golden.rs:104-109`) skips all 24 button cells as pending (`committed_positives()==0`) → its only assert is `asserted+pending>0` (line 149), which passes on pending alone — **green while comparing zero pixels** on the serialized GPU lane.
→ Make an all-pending run report `ignored`/skipped (green ⟹ ≥1 cell compared); bless one golden per residue class as renderer paths land; document in `goldens.md` that 5/6 classes are currently aspirational.

**15. EXACT(0,0) goldens are Mesa-brittle and never re-exercised on a schedule** (medium)
Both goldens are `FuzzBudget::EXACT`; `b869eba` already forced one re-bless. `ci.yml:3-6` triggers only on `push:main` + `pull_request` — no `schedule:`/`cron:`. Golden brittleness and the non-deterministic proptest invariants are never independently re-run on a cadence.
→ Add a weekly `schedule: cron` run of the GPU lane + proptest suites; document the Mesa-bump ⟹ re-bless contract once. (The related "guess a non-zero budget" suggestion was **adjusted to info** — the EXACT-on-pinned-rasterizer + re-bless posture is a deliberate, documented design decision and `rect-rounded` is byte-identical run-to-run on the canonical rasterizer.)

**16. MSRV 1.85 declared but never CI-verified** (medium)
`Cargo.toml:35` `rust-version="1.85"` + edition 2024; all five CI jobs use `dtolnay/rust-toolchain@stable`; `grep 1.85|msrv .github/` finds nothing. `architecture.md:99` says "MSRV tracks Bevy's MSRV" — the declared floor is unenforced and likely already below Bevy 0.18's real MSRV (the exact Bevy floor is worth confirming).
→ Add a `@1.85 cargo check --workspace` job, or reconcile/remove `rust-version` to stop advertising an unverified floor.

**17. No CI guard for orphaned `.snap` or committed `.snap.new`** (adjusted: low)
No `cargo insta --unreferenced=reject`, no `git diff --exit-code` gate, no `*.snap.new` in `.gitignore`. All 65 snaps are currently referenced and no `.snap.new` is committed (latent, not active). insta's CI default already fails on a snapshot *mismatch*, so the uncovered cases are narrow: an inert committed `.snap.new` and orphaned `.snap`.
→ Add `cargo insta test --workspace --unreferenced=reject` (or a post-test `git diff --exit-code`); add `*.snap.new` to `.gitignore`.

**18. Grapheme-correct delete fixtures (emoji ZWJ, combining marks) missing** (medium)
`editing-and-ime.md:598` (§12) explicitly requires these as a headless unit test; §13 lists "grapheme-correct delete" in the v1 slice. Every Backspace/Delete test uses ASCII; the undo proptest restricts typing to `'a'..'z'`. The EMOJI fixture font is already on disk.
→ Headless test in `text_editing_ops.rs` (via the `shaped_editor` idiom): insert emoji-ZWJ family + base+combining-mark, Backspace once, assert one-step cluster removal.

**19. Undo proptest never creates a selection** (medium)
`text_undo_property.rs:17-25` `ScriptOp` has no Select/Extend variant; only `Motion(_, false)`. `restore_cursor`'s non-collapsed branch (`input.rs:343-347`) is never round-trip-verified.
→ Add a `Motion(_, true)`/`SelectAll` op to the generator and assert selection round-trip, or a focused select-replace-undo test.

**20. a11y `build_tree` description + skip-empty branches untested** (medium)
`build_tree` (`a11y/mod.rs:90`) has a skip branch (line 103) and description extraction (line 110). The only system test (`a11y.rs:38`) spawns one fully-populated entity; `A11yDescription` is never spawned in any test (grep → zero).
→ Extend to (a) spawn `A11yDescription` and assert it surfaces, (b) spawn a non-a11y-only entity and assert it's absent.

**21. `Hovered` consumer chain never asserted end-to-end** (medium)
`update_hovered` (`picking/mod.rs:59`) is the only writer of `Hovered`; `picking_backend.rs` reads only the raw `PointerHits` message; `button.rs:52`/`text_input.rs:82` insert `Hovered` manually. No test reads the `Hovered` value after a backend emit.
→ Extend `picking_backend.rs`: `app.update()` then assert `Hovered.0 == Some(entity)`; with two overlapping nodes this also covers top-most.

**22. Grid track-size functions (MinMax/AutoFit/FitContent/Min/MaxContent/percent) untested** (medium)
`layout_grid.rs` exercises only `Length(Fr/Px)` + `Repeat(AutoFill,[Px])`; `translate.rs` units assert only the outer `Single`/`Repeat` shape. `AutoFit` (collapses empty tracks, unlike AutoFill) has zero coverage despite live arms.
→ T1 grid tests for `minmax(100px,1fr)`, fit-content, `repeat(auto-fit,...)`; at minimum assert inner `Min/MaxTrackSizingFunction` in the `translate.rs` units.

**23. `UndoStack` test-only seam + over-exposed internal Vecs** (medium / low)
`undo.rs:201` `pub fn push_redo_for_test` ships in the public API (no `#[cfg(test)]`); its sole caller (`text_clipboard_undo.rs:59`) can use the real `record(); pop_undo()` path. Separately, `undo`/`redo` are `pub Vec` (`undo.rs:66-67`) indexed directly by tests; production *does* read them (`input.rs:353,358`) so they need `pub(crate)`, not `pub` — the over-exposure is the defect.
→ Delete `push_redo_for_test`, rewrite its test through the real API; make `undo`/`redo` `pub(crate)` with a narrow read accessor.

**24. `subgrid`/`masonry` stub tests are vacuous** (medium)
`layout_grid_stubs.rs:20-40` — both bodies are comment-only, no asserts. The fallbacks warn via a process-global `AtomicBool` (`translate.rs`), unlike table/multicol which route through the testable `LayoutWarnedOnceSession` resource.
→ Route subgrid/masonry warns through `LayoutWarnOnceKey` (mirroring the multicol precedent) and assert the key + the observable Auto/Row fallback geometry.

**25. `BuiyFallback::script_fallback` net never exercised end-to-end** (adjusted: medium)
`font_system.rs:76-87` maps Arabic/Hebrew/Devanagari/Han to Noto families as pure data; every non-Latin test names the covering font explicitly so `FontFallbackIter` never fires. A broken match-arm wiring or script typo regresses silently.
→ Headless test: script text with a stack that does NOT name the covering font, asserting glyphs land on the Noto face. One fixture per arm.

**26. `SwashContent::Color` skip+warn decision untested** (medium)
`resolve_glyph` (`extract.rs:854-885`) maps Color → `warn_once_color_emoji_skipped()` + None; the only emoji fixture is monochrome (Mask). A Mask/Color branch swap silently drops or mis-bakes glyphs; the zero-area guard (`extract.rs:859`) is also unasserted. The *decision* is cheaper-tier-testable than the deferred Tier-5 golden.
→ Factor the `SwashContent` match into a pure `content→ResolveAction` helper; unit-test Mask/Color/SubpixelMask + the zero-area guard headless.

**27. Shadow `erf`/Gaussian math has no oracle** (medium, latent)
`shadow.wgsl:66-81` (erf + blurred box) is only naga-parsed + string-checked (`render_shader_wgsl.rs:31`). No CPU oracle; the draw path is unwired (`extract.rs:348` TODO, no Shadow primitive emitted), so the GPU lane never rasterizes it either — dead code today.
→ Add a Rust `erf`+blurred-box oracle pinning the closed form at canonical points (GPU-independent, lowest tier).

**28. Decoration GPU goldens re-assert geometry the extract harness already covers** (medium, tier-misplacement)
`text_decoration_gpu.rs:208-276` assert band count/pos/thickness/gap — already covered at the producer tier (`text_decoration.rs:212/253/295`) and extract tier (`text_extract.rs:362`). GPU adds only AA confidence.
→ Push geometry asserts to extract-harness/display-list snapshots off `ExtractedTextQuads`; keep one GPU golden per kind for the AA residue.

**29. `follow-ups.md` describes `matrix_goldens` as RED; code is skip-as-pending** (medium, doc-drift)
`follow-ups.md:832-850` says RED/fail-closed; `coverage_golden.rs:104-109` already adopted skip-as-pending (green). The doc entry (commit 9153c6e) predates the fix (d14e103) and was never updated.
→ Update the entry to record skip-as-pending landed; re-frame remaining work as "bless real residue cells"; cross-check the golden-prune-bin claim.

**30. Render-path GPU-lane exclusivity** (adjusted: low)
The "whole render tier unverified" framing is overstated — the CPU-observable spine *is* covered headless (`render_prepare.rs`, `snapshot_instance_hex.rs`, `snapshot_display_list.rs`, `invariant_*`). What is genuinely GPU-exclusive is the irreducible rasterization residue, which is correct by design. Residual risk: the un-mirrored Mesa tarball and unconfirmable required-check status.
→ Confirm the `gpu` job is a required status check in branch protection; mirror the pinned Mesa tarball into a repo-controlled asset.

### P1 — completeness-pass additions (found by the completeness critic, independently re-verified)

**39. The `doc` CI gate never compiles or runs example code — all 5 doc fences are ` ```ignore `** (medium, hand-verified)
Across all four crates' `src`, there are exactly 5 doc code fences and **every one is `ignore`** (`buiy/src/lib.rs:62`, `buiy_core/layout/style.rs:33`, `layout/components.rs:556`, `buiy_verify/coverage/fixture.rs:72`, `reftest.rs:463`). `cargo doc -D warnings` only checks that doc *prose* renders without broken intra-doc links; it never compiles example code, and `cargo test --doc` skips `ignore` fences. So the public-API onboarding examples (including the canonical `App::new()…BuiyPlugin` facade snippet) are completely unverified and can silently rot to non-compiling.
→ Audit which `ignore` fences encode real public-API contracts; drop `ignore` (use `no_run` where a window/GPU is needed) so the doc-gate compile-checks them. At minimum make the facade BuiyPlugin example runnable.

**40. Zero performance / benchmark / latency-regression testing** (medium, hand-verified)
No `benches/` dirs, no `criterion`, no `#[bench]` anywhere in the workspace. The files named `text_input_latency.rs`/`text_typing_latency.rs` measure *frame-count semantics* (edit lands frame N, glyph publishes N+1), **not** wall-clock/allocation/throughput — so "latency" in the suite name is misleading and there is no perf-regression gate at all. A change that makes layout/shaping/extract O(n²) or adds per-frame allocations ships green. For a 29k-LOC retained-mode renderer this is a material untested dimension.
→ Decide whether a perf-smoke tier is warranted (a criterion bench on shape→layout→extract for a large scene, or an allocation-count assertion via a counting allocator on the hot per-frame path). At minimum record "no performance testing exists" as a known, severitied gap.

**41. No code-coverage measurement wired** (low/medium, hand-verified)
No `llvm-cov`/`tarpaulin`/`grcov`/`codecov` in CI. Every dead/untested branch in this report (focus `tab_order`, picking z-order, a11y skip, redo keymap, `SwashContent::Color`) was found by hand-grep — exactly what a coverage tool surfaces mechanically and continuously.
→ Evaluate wiring `cargo llvm-cov` (works headless, no GPU) as an informational artifact, with a coverage floor on the pure-logic modules (layout/translate, focus, picking, text/edit ops). Note that GPU residue paths won't be covered headless.

**42. `hello_text` and `capture` examples are never run-asserted** (medium, hand-verified)
Only `hello_button` has a true e2e test (`tests/hello_button_e2e.rs`). `hello_text` (the text stack's named smoke target) and `capture` (the README-screenshot generator — the *only* end-to-end exercise of the offscreen render-to-texture + GPU readback path) have **zero** e2e tests; they are compiled via clippy `--all-targets` but never run. CLAUDE.md positions `cargo run -p hello_text`/`-p capture` as canonical smoke tests, yet nothing in CI or the suite runs them — a startup panic in either ships green.
→ Add an `hello_text` e2e mirroring `hello_button_e2e.rs` (drive the scene through `app.update()`, assert no panic + glyphs publish), and a GPU-lane e2e for the `capture` render-to-texture+readback path.

**43. CI has no `timeout-minutes` and no `concurrency`/`cancel-in-progress`** (medium, hand-verified)
Neither key appears anywhere in `ci.yml`. A hung GPU/lavapipe job (a real risk on adapter init) can run to GitHub's 6h ceiling, and stacked pushes burn duplicate runner-hours. Separately, `cargo deny check advisories` fetches the RustSec DB live, so the deny job is network-dependent and — combined with the floating `Cargo.lock` (#1) — runs its advisory check against a non-reproducible graph.
→ Add `timeout-minutes` (especially on `gpu` and `test`) and a workflow-level `concurrency` group with `cancel-in-progress`. Consider pinning/caching the advisory DB and note its interaction with #1.

### P2 — nice

**31. Intrinsic sizing on non-text boxes** (adjusted: low). MinContent/MaxContent/FitContent box arms collapse to `taffy::Dimension::auto()` (Phase-10 named deferrals, `translate.rs:634-639`); the gap is pre-documented (`box-model.md:260`) and a test would assert auto fall-through, not real intrinsic logic. Text-leaf MinContent/MaxContent *is* tested.

**32. Degenerate sizes never driven through the pipeline** (medium). No fixture feeds width/height of 0, negative, NaN, or infinity into `sync_styles→Taffy→write_resolved_layout`; the finiteness invariant validates output scenes generated in `0.0..512.0`, never pathological input. → T1 test feeding degenerate `Length` values, asserting finite non-negative `ResolvedLayout` + no panic.

**33. Transform-bridge compose formula lives inline** (low). `bridge.rs:117`; App tests re-implement it at `render_transform_bridge.rs:96`. → Optional: extract `fn compose_buiy_transform(...)` and unit-test directly.

**34. System-set order asserted only on CorePlugin's empty sets** (low). `system_set_order.rs` builds an app with only CorePlugin. → Consider a delta-count membership assertion per plugin's set contribution.

**35. No shared headless App builder — ~40 copy-pasted plugin stacks that have drifted** (medium, maintainability). `fn app()` ×21, `fn text_app()` ×10, `fn settle()` ×8 (a 2-vs-3-frame split), two unrelated `blink_app()`; `render_effect_groups.rs` inlines setup 16×. `text_decoration.rs`'s `text_app` silently adds `ThemePlugin`. → Add `headless_layout_app()`/`headless_text_app()`/`bare_layout_app()` to `tests/support/mod.rs`; migrate the drifted helpers; hoist one documented (or condition-polled) `settle`.

**36. Forced-colors theme never WCAG-contrast-linted** (adjusted: low). `forced_theme_canvas_and_canvastext_contrast` (`theme_forced_colors.rs:37`) is an `assert_ne!`, not a ratio; `forced_colors_theme()` is a documented v1 stub. → Defer the ratio gate until real palette values land; rename the test to `..._differ` now.

**37. `AnchorRef::Entity` path untested + no unit test on the pure anchor seam** (low). The pure resolver `try_anchored_position` (`systems.rs:1494`) *does* exist but has zero direct unit tests; all 11 anchor tests use `AnchorRef::Name` via `app.update()`. → Add unit tests against `try_anchored_position`/`try_conditions_pass` plus one `AnchorRef::Entity(e)` integration fixture.

**38. Lower-value detection holes (info/low — cleanup backlog, not blockers).** Shaping corpus breadth (pure-Hebrew, VS16, Thai/Khmer); cosmic-text caret-range vs glyph-id pin coupling; drag-selection + macOS letter-command + word-nav motion (host/cfg-limited); AABB inclusive-edge boundary; facade/widgets thin smoke; the `Instant`-grep-lint scoped only to `golden.rs`; production `*Count` instrumentation Resources compiled unconditionally; weak Tier-3 enrollment predicates over a constant box + no axis-distinctness self-test; SDF numeric pin GPU-only / textual copies; the documented-but-missing golden-prune bin; a blanket `#![allow(dead_code)]` masking one dead `pub fixture_font_bytes`; an empty `boxshadow_visual_reftest_is_blocked` marker. *(These passed through as low/info without adversarial verification.)*

### Investigated & dismissed

- **"DPR matrix axis is inert at CPU tiers"** (CONFIRMED, recorded as cleanup not bug): every dpr1 CPU snapshot is byte-identical to its dpr2 sibling because `ResolvedLayout` is logical-px (DPR lives only in the GPU view uniform) — 24 of 48 button CPU snapshots are exact duplicates. *Action: drive dpr/breakpoint only at the GPU golden tier; collapse the CPU enrollment matrix.* Kept out of the P-tiers because it inflates snapshot count rather than masking a regression.
- **"Magenta sentinel baked as light-theme baseline"** (CONFIRMED): the 12 light-theme button display-list snapshots assert `color=#ff00ffff` (the missing-token sentinel) as expected, so a real "renders magenta under light theme" regression is indistinguishable at that tier. *Action: paint a both-theme-resolving token (or skip light cells for system-color-only fixtures); folded into the Tier-5/snapshot-quality backlog with #14.*
- **"SDF is only GPU-checked; share fn + d-pin undone"** (**REFUTED**): false on both halves. A single canonical CPU oracle already lives at `reftest.rs:261` (`sdf_oracle::sdf_rounded_rect`, doc'd "kept PERMANENTLY… one shared analytic"), driving both the GPU cross-check and headless tests. The distance is pinned by 5+ non-`#[ignore]` tests (`render_instance.rs:18`, `sdf_oracle.rs:17/41`, `render_border_sdf.rs:65`). A WGSL fn cannot be cfg-shared into Rust, so the GPU/CPU twin is inherent to cross-checking and is held in sync by the cited point-probes. At most a 3-copy DRY nit.

## Infrastructure assessment

**CI gates.** Well-engineered for the project's age. Five jobs (lint, doc, deny, test, gpu); `fail-fast:false`; the deny job pins `cargo-deny` exactly. The GPU lane fails **closed** — `curl -f` + `set -euo pipefail` in install-mesa, an explicit `llvmpipe` adapter-assert that fails even if `wgpu-info` itself fails to install — and the headless `test` job is genuinely additive and adapter-free. The one-off bless-goldens job was correctly removed (`b869eba`); no auto-bless/write-back path exists in CI. The disk/`debuginfo=0`/`-j 2` hacks correctly diagnose disk-exhaustion-as-`ld SIGBUS` and do not reduce fidelity (debug-assertions stay on; no test asserts on backtrace content). The material CI gaps: the floating `Cargo.lock` (#1), the orphaned facade test in no lane (#8), unverified MSRV (#16), no scheduled cadence (#15), and no `timeout-minutes`/`concurrency` (#43).

**GPU/headless split.** The doctrine — low tiers (layout-number, display-list, instance-hex, invariant proptests) run headless on every PR; only irreducible rasterization residue is `#[ignore]` + GPU-gated — is sound and mostly honored (Vulkan render-to-texture needs no X server). The leaks across the split are two CPU-observable behaviors wrongly GPU-only (justify/align #2, WGSL parse #3), and three schedule-graph introspection tests GPU-gated only by RenderApp adapter init (adjusted-low, constrained by bevy 0.18.1's RenderApp/adapter coupling).

**Determinism / flakiness.** Well-engineered; the prior review's fixes held: per-timestamp clock pinned to `ManualDuration(ZERO)`, committed + bounded proptest seeds, same-`Name` sibling sort tiebreaking on content (not `Entity::index`), a fail-loud quiescence gate, float-rounded dumps ordered by Name+content (no HashMap-iteration leakage), and the pure-Rust `harfrust` shaper (no platform HarfBuzz skew). The one material defect is the cross-rasterizer golden-integrity violation (#7). Secondary: the `Instant::now` grep-lint scopes only `golden.rs`.

**Golden/snapshot lifecycle.** Architecturally sound and bless-safe. Every bless is env-gated (`BUIY_BLESS`/`BUIY_BLESS_REPLACE`/`BUIY_ACCEPT_SHAPING`); none set in CI. The two residue goldens carry a per-positive TOML ledger (commit+timestamp+reason+budget). All 65 `.snap` files are live-referenced; no `.snap.new` committed. Weaknesses are exposure/hygiene, not correctness: only 1 of 6 residue classes is golden-guarded and `matrix_goldens` verifies zero pixels (#14), no orphan/pending-snapshot CI guard (#17), no stale-positive expiry, and the documented `golden-prune` bin doesn't exist.

**Build ergonomics.** The dominant cost is the ~162-binary explosion (#9) — the root cause of the CI disk/link instability the workflow papers over. GPU-lane cache fragmentation (#10) wastes compile minutes. Test *code* is otherwise good (high comment density, `//!` docs, well-factored GPU support layer), but the headless end has no shared App builder, so ~40 copy-pasted plugin stacks have silently drifted (#35). `cargo-nextest` is used nowhere (faster, better isolation, built-in flaky-retry).

**Untested dimensions.** Beyond per-subsystem gaps, five whole testing dimensions are absent: runnable doctests (#39), performance/benchmarks (#40), code-coverage measurement (#41), example e2e for `hello_text`/`capture` (#42), and CI robustness guards (#43).

## Recommended test backlog

Ordered, implementer-ready. P0 first.

1. **Commit `Cargo.lock`** (remove `.gitignore:10`); add `--locked` to the test/deny/doc cargo steps.
2. **Add `cargo test -p buiy -j 2 -- --ignored --test-threads=1`** to the GPU lane so the facade render-finish test runs.
3. **Headless flex justify/align T1 test** — three 40px boxes in a 200px row; assert `ResolvedLayout` x=0,80,160 for SpaceBetween + SpaceAround/Evenly + AlignItems::Center/FlexEnd.
4. **Extend `render_shader_wgsl.rs`** with naga-parse + entry-point assertions for `coverage.wgsl` and `composite.wgsl`.
5. **`hit_test` overlap test** (smaller-area wins) + **backend depth test** (picks[0] smaller, ascending depths) + **`Hovered` consumer assertion** after `app.update()`.
6. **Focus `tab_order` tests** — `pub(crate)` `compute_next_focus`; `tab_order=-1` skipped, positives (1,2) before Auto ascending.
7. **Extract 6f to `buiy_core::pub fn painters_z_for_context(...)`** (re-export `paint_key`); have `invariant/scene.rs::realize` call it; add a `PositionKind` axis to `SceneNode`/`arb_leaf`.
8. **Gate `golden_sdf_corner`** on the pinned-lavapipe adapter (skip-as-pending otherwise), or split into a rasterizer-internal self-check + a lavapipe-only baseline leg.
9. **Consolidate per-file test files** into <10 binaries that `mod` the current ones; evaluate adopting `cargo-nextest`.
10. **Unify the GPU/test debuginfo mechanism** (`[profile.test] debug=0`) and add a rust-cache `shared-key`.
11. **System-level redo test** (type → undo → Ctrl-Shift-Z + Ctrl-Y) asserting value restored + one `EditRedone`.
12. **Extract condition-4 quiescence** behind `fn no_pipeline_compiling(&PipelineCache)` + unit-test.
13. **Tier-5 golden non-vacuity** — make `matrix_goldens` report `ignored` on an all-pending run; document 5/6 residue classes as aspirational.
14. **Weekly `schedule: cron`** running the GPU lane + proptests; mirror the pinned Mesa tarball into a repo-controlled asset.
15. **MSRV check job** (`@1.85 cargo check --workspace`) or reconcile `rust-version` with the documented policy.
16. **`cargo insta test --workspace --unreferenced=reject`** + `*.snap.new` in `.gitignore`.
17. **Grapheme-correct delete test** (emoji ZWJ + base+combining) via `shaped_editor`, asserting one-step cluster removal.
18. **Undo proptest selection round-trip** (add `Motion(_, true)`/`SelectAll`) or a focused select-replace-undo test.
19. **a11y `build_tree` branch tests** (`A11yDescription` surfaces; non-a11y-only entity skipped).
20. **Grid track-size tests** (`minmax(100px,1fr)`, fit-content, `repeat(auto-fit,...)`) at T1; minimum assert inner `Min/MaxTrackSizingFunction`.
21. **Route subgrid/masonry warns** through `LayoutWarnOnceKey`; assert key + Auto/Row fallback geometry.
22. **`script_fallback` test** (non-covering stack → glyphs on Noto), one fixture per arm.
23. **`SwashContent` classification helper** + unit test; **Rust `erf`+blurred-box oracle** for `shadow.wgsl`.
24. **Delete `UndoStack::push_redo_for_test`** (rewrite via the real API); make `undo`/`redo` `pub(crate)` + a read accessor.
25. **Add `headless_layout_app()`/`headless_text_app()`/`bare_layout_app()`** to `tests/support/mod.rs`; migrate the ~40 drifted helpers; hoist one documented `settle`.
26. **Degenerate-size T1 test** (0/negative/NaN/infinity → finite non-negative, no panic); **`AnchorRef::Entity` fixture** + unit tests against `try_anchored_position`.
27. **Collapse the dpr/breakpoint CPU enrollment axis** (drive dpr only at the GPU tier; assert dpr-sibling identity once) and **stop baselining the magenta sentinel** in light-theme button snapshots.
28. **Update `follow-ups.md`** `matrix_goldens` entry (skip-as-pending landed; button no longer RED); cross-check the golden-prune-bin claim.
29. **Make load-bearing doc examples runnable** (drop `ignore`/use `no_run`) so the `doc` gate compile-checks the facade BuiyPlugin snippet and the `fixture!`/`reftest!` examples.
30. **Decide a performance-testing posture** — a criterion bench on shape→layout→extract, or an allocation-count assertion on the per-frame hot path; record the decision.
31. **Evaluate `cargo llvm-cov`** as an informational CI artifact with a coverage floor on the pure-logic modules.
32. **Add `hello_text` e2e** (mirror `hello_button_e2e.rs`) + a GPU-lane e2e for the `capture` render-to-texture+readback path.
33. **Add CI `timeout-minutes` + `concurrency`/`cancel-in-progress`**; consider pinning the advisory DB.

## Appendix — audit completeness

The completeness critic's verdict was **gaps-found**; its 5 surfaced dimensions (doctests, performance, coverage tooling, example e2e, CI robustness) are folded in above as findings #39–#43 and backlog items 29–33, each re-verified by grep. No further audit-of-the-audit round was run — the additions are mechanical structural facts (presence/absence of files and CI keys), not judgment calls. One finding (the SDF cross-check claim) was refuted during verification and is recorded under *Investigated & dismissed* rather than dropped silently.
