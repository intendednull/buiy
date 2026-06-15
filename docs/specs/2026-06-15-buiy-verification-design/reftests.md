# Tier 4 — reftests + CPU/GPU cross-check

**Date:** 2026-06-15
**Status:** draft
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

The reftest harness — Buiy's highest-leverage pixel investment and the one mechanism wholly absent from the tree. A reftest renders a **test** scene and a **reference** scene with the *same engine in one process* and asserts their bitmaps match (`==`) or differ (`!=`), never against a stored baseline — so every platform-variance term (driver SDF rounding, glyph-atlas AA, sRGB encode, clock) cancels in the diff. This file specifies `RefCase`, the `reftest!` macro, `run_reftest`, the reference-independence discipline + its lint/review enforcement, the CSS-subset authoring patterns, and the Vello-style CPU-vs-GPU SDF rasterization cross-check (Tier 4.5). It is GPU-coupled (`#[ignore]`, runs under `cargo test -- --ignored` on a real adapter here and pinned lavapipe in CI).

## Contract deviations

None. This file consumes `buiy_verify::metric` (`Diff`, `FuzzBudget`, `compare`, `Diff::passes`) and `CompareOpts` exactly as the contract defines them, and the promoted `buiy_core::render::golden::capture_to_image(&mut App, &GoldenConfig) -> image::RgbaImage` exactly as the crate-boundary clause defines it. One additive note flagged for the synthesizer, not a deviation: this tier needs `capture_to_image` to support **two captures in one `App`** (re-target the camera, re-readback) without rebuilding the device — see `run_reftest` below. If `golden.md` specs `capture_to_image` as one-shot-per-App, reconcile toward a `capture_scene(&mut App, scene: impl FnOnce(&mut App), &GoldenConfig) -> RgbaImage` shape that both tiers share.

## Module & public API

Lives in `buiy_verify::reftest` (pure pairing/aggregation logic + the macro) and calls into `buiy_core::render::golden` for capture. The harness itself stores **zero bytes**.

```rust
// buiy_verify::reftest

use buiy_verify::metric::{compare, CompareOpts, Diff, FuzzBudget};
use bevy::app::App;

/// One reftest pairing. `test` and `reference` each build a scene into a fresh,
/// deterministic `App` (spawn entities; do NOT drive frames — `run_reftest` owns
/// the capture loop). Co-locate the expectation with the `#[test]`.
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefKind {
    /// Pass iff `test` and `reference` render to the same bitmap within `fuzz`.
    Match,
    /// Pass iff they render DIFFERENTLY (a `!=` anti-test guards silent no-ops).
    Mismatch,
}

#[derive(Debug)]
pub struct RefOutcome {
    pub passed: bool,
    pub diff: Diff,
    /// On failure, a self-contained local HTML triage report (test | ref | diff),
    /// reusing golden.md's report emitter. Path printed to stderr; never committed.
    pub report_path: Option<std::path::PathBuf>,
}

/// Render BOTH scenes via buiy_core capture in ONE app run and diff with
/// `metric::compare`. Platform variance cancels because both halves share one
/// `wgpu::Device`, driver, atlas, and virtual clock.
pub fn run_reftest(case: &RefCase) -> RefOutcome;
```

`run_reftest` is the whole engine:

1. Build a `DeterministicApp` (determinism.md) — one device, fixed virtual clock, fonts/atlas warmed, DPR + MSAA-off pinned. Both captures share it.
2. Capture `test` → `RgbaImage` via `golden::capture_to_image` (Ahem/obscure-text layout-font mode on by default per determinism.md, so text-bearing reftests assert boxes, not glyph fidelity — glyph fidelity is Tier 5).
3. Capture `reference` → `RgbaImage` in the **same** `App` (re-target the offscreen camera, re-readback — see Contract deviations).
4. `let diff = compare(&test_img, &ref_img, &CompareOpts::reftest_default());` — AA-aware (pixelmatch YIQ `colorDelta` + the antialias sibling test), since two CSS-subset code paths can legitimately differ by one AA pixel on a shared corner.
5. `Match`: `diff.passes(&case.fuzz)`. `Mismatch`: `!diff.passes(&case.fuzz)` **and** the fuzz floor is `(0,0)` (a `!=` whose budget tolerates difference is meaningless — assert this at macro-expansion time).

`CompareOpts::reftest_default()` enables AA exclusion and the YIQ per-pixel decision; the secondary MSSIM (`image-compare`) channel is advisory and never gates a reftest.

### The `reftest!` macro

Generates one `#[test] #[ignore]` per pairing — keeps each case at the unit/integration tier under the existing `cargo test -- --ignored` GPU lane, no new CI infra, no manifest file (the type system *is* the manifest).

```rust
reftest!(match,    "container_query_collapse", cq_test, cq_reference);
reftest!(mismatch, "cv_hidden_actually_hides", cv_visible, cv_hidden);
reftest!(match,    "flex_justify_end", flex_test, literal_offsets_ref, fuzz = (1, 8));
```

Expansion (sketch):

```rust
macro_rules! reftest {
    ($kind:ident, $name:literal, $test:path, $reference:path $(, fuzz = ($d:literal, $p:literal))?) => {
        #[test]
        #[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
        fn $name() {
            let case = $crate::reftest::RefCase {
                name: $name,
                kind: $crate::reftest::RefKind::reftest_kind(stringify!($kind)),
                test: $test,
                reference: $reference,
                fuzz: reftest!(@fuzz $kind $(($d, $p))?),
            };
            let outcome = $crate::reftest::run_reftest(&case);
            assert!(outcome.passed, "reftest {} failed: {:?} (report: {:?})",
                    $name, outcome.diff, outcome.report_path);
        }
    };
    // mismatch with no explicit fuzz → (0,0); match with none → (0,0); macro
    // rejects a non-zero floor on `mismatch` at compile time.
}
```

## Reference independence — the load-bearing discipline

The whole bet (wpt-reftests/lessons.md, "Top of file"): **the reference must not use the feature under test.** A flex reference built with flex, or an `@container` reference built with `@container`, shares any bug and the comparison passes vacuously — the symmetric twin of the golden weakness named in the report's runner-up rejection. This is the report's Open Question #1; this spec closes it with three mechanisms, in priority order:

1. **Route references through the primitive layer.** Buiy has a layer below Taffy/CSS-subset: `DrawData::new(position, size, color, radius)` (`render/mod.rs:78`/`:85`) and literal-positioned `Node` boxes that bypass the flex/grid/container-query solver entirely. A reference authored with literal offsets *cannot* share a layout-solver bug. This is the default and covers the bulk of pairings.

2. **Lint-enforce disjointness (CI gate).** A `buiy_verify::reftest::lint` check, run as a `#[test]` (not `#[ignore]` — pure CPU), introspects each `RefCase`'s `reference` scene after building it into a headless no-GPU `App` and asserts the reference subtree carries **none** of the components the `test` exercises. Concretely, a declarative map keyed by feature:

   ```rust
   /// What a reference scene is FORBIDDEN to contain, per feature under test.
   /// Checked by component presence in the built ECS world — structural, not textual.
   pub struct IndependenceRule {
       pub feature: &'static str,
       pub forbidden_in_reference: &'static [ComponentMarker], // e.g. ContainerQuery, ContentVisibility
   }
   pub fn assert_reference_independent(case: &RefCase, rules: &[IndependenceRule]);
   ```

   E.g. a `@container` pairing's reference must contain **zero** `ContainerQuery` components; a `content-visibility` pairing's reference must contain zero `ContentVisibility::Hidden`. The check is structural (query the built world for the forbidden component), so it cannot be fooled by a textual rename. Pairings whose feature has no registered rule fail the lint until a rule is added — independence is opt-out-impossible by construction.

   **Limit — value-encoded features fall to human review (and may be the majority).** The lint queries for forbidden *components*, but many CSS-subset features are not their own component: `justify-content`, `align-items`, `direction`, `writing-mode`, `gap`, and the like are *field values* on a shared `Style`/`Node` component that every flex/grid scene carries — including a legitimately-disjoint reference. Component-presence cannot distinguish "reference uses flex *via* `justify-content`" from "reference is a plain literal-offset box that happens to carry a default `Style`," so these features have **no usable structural rule** and fall to the PR-time review checklist below. This is not the residue — for a CSS-subset engine where layout is value-encoded on one `Style` component, value-encoded features may be the *majority* of pairings, and mechanism 1 (route references through the primitive `DrawData`/literal-`Node` layer, which carries no `Style` at all) is what keeps them independent; the lint backstops only the features that *do* have a distinct marker component (`ContainerQuery`, `ContentVisibility`, `TopLayer`, transforms). Where a value-encoded feature cannot route through the primitive layer, human review (checklist item a) is the only enforcement, and the reviewer must treat it as load-bearing, not a formality.

3. **Multiple references where one disjoint path is impossible** (logical↔physical, transform↔literal where the literal still routes through one shared packer). Support `reference: &[fn(&mut App)]` semantics via a `RefCase::multi` constructor: for `Match`, **≥1** reference must match (OR); for `Mismatch`, **all** must mismatch (AND) — the WPT/Gecko aggregation (wpt-reftests/lessons.md Borrow #1). Build this into the harness deliberately; Blink supports neither multiple nor chained references, so it is not free (wpt-reftests/lessons.md Avoid row).

**Review checklist** (PR-time, complements the lint): (a) does the reference invoke the feature under test? (lint catches the structural cases; reviewer catches semantic ones the marker map misses); (b) is the fuzz floor `(0,0)` for a `Mismatch`?; (c) does a `Match` with non-zero fuzz cite a measured run-to-run jitter reason, ranges not including 0 (Mozilla discipline, wpt-reftests/lessons.md Avoid)?

## Authoring patterns — mapped to Buiy's CSS-subset

Each row is a `reftest!` pairing; the reference column is the disjoint oracle (wpt-reftests/lessons.md Borrow #6):

| Feature under test | `test` scene | `reference` scene (disjoint) | kind |
|---|---|---|---|
| flex `justify-content: SpaceBetween` | three 40px boxes in a 200px flex row | three boxes at literal x = 0, 80, 160 via primitive layer | `match` |
| `@container` query resolution | widget whose style resolves via a container query | same tree with the resolved branch inlined as a plain `Style`, no `ContainerQuery` | `match` |
| `content-visibility: hidden` | subtree with `ContentVisibility::Hidden` | identical subtree, visible | `mismatch` |
| logical → physical mirror | logical-property layout (writing-mode/`direction`) | hand-authored physical-property mirror | `match` |
| `translate(50,50)` | element with `Translate(50,50,0)` | element authored at the translated literal coordinates | `match` |
| forced-colors visual residual | a scene under forced-colors mode | hand-authored reconstruction using only system tokens (coverage.md catalog) | `match` |

**The forced-colors visual reftest is BLOCKED until the `BoxShadow` extract/draw path lands.** That row exercises the forced-colors `BoxShadow` draw-skip (shadows suppressed under forced colors), but `extract_buiy_nodes` has no `BoxShadow` branch today — it gains one only when `BoxShadow` gets a real extract/draw path (follow-ups.md:474–478, "extract_buiy_nodes has no such branch"). So this specific pairing is **specified now but not runnable** until that unlanded path exists; it must not be authored as a green test before then. The *structured* forced-colors checks cover the rest in the meantime: `analyze_forced_colors` / `analyze_shadow_only` over the live catalog (coverage.md § "Wiring `forced_colors_analyzer`", gate #11) gate the non-shadow forced-colors paint today, pure-CPU, with no dependency on the BoxShadow draw path.

`!=` anti-tests (the `mismatch` rows) prove a feature *does something* — guarding silent no-ops a `==` would pass vacuously on blank-vs-blank.

**Do not reftest the unreftestable** (wpt-reftests/lessons.md Avoid): underline position/thickness, dotted/dashed/ridge/groove/double borders, focus-ring geometry, font-metric-dependent rendering — no feature-free reference reproduces them. Route those to Tiers 1–3 (snapshot.md / invariant.md) or the Tier-5 golden residue (golden.md). The pyramid is the answer; do not force a reftest.

## CPU-vs-GPU SDF cross-check (Tier 4.5)

The golden-free rasterization oracle for the one property no markup reference can reach: **SDF corner AA**. Vello's pattern (vello/lessons.md "Top of file"), but *stronger* — Buiy's CPU oracle and GPU shader evaluate the **same closed-form `sdf_rounded_rect`** (`render/shader.wgsl:60`; CPU port at `tests/render_instance.rs:12`), so their agreement-to-tolerance is a *durable* invariant whose divergence localizes a real shader bug (wrong half-extent, radius clamp, premultiply, AA step). Keep it **permanently** — do not inherit Vello's "phase out the cross-check" posture (vello/lessons.md Avoid), which applies only to their two-independent-implementation case.

Promote the CPU port from three scalar point-probes to a **full-tile rasterizer** (vello/lessons.md Borrow #1):

```rust
// buiy_verify::reftest::sdf_oracle

/// Pure-CPU per-pixel evaluation of the WGSL SDF + AA coverage step, mirroring
/// shader.wgsl:60/:76-:79 (fwidth → smoothstep(-aa, aa, d)) at the same logical-px
/// scale. The single source of the SDF formula is shared with the shader via a
/// doc-pinned port (the port and shader.wgsl must stay 1:1 — checked by a unit
/// test that re-derives the few sample points the existing render_instance.rs uses).
pub fn rasterize_sdf_rect(draw: &buiy_core::render::DrawData, w: u32, h: u32) -> image::RgbaImage;

/// Render the same single primitive on the GPU (one-instance capture) and on the
/// CPU oracle, diff with metric. Tolerates sub-pixel AA noise via `fuzz`; zero
/// stored bytes. Catches AA/implementation drift no reftest can.
pub fn run_sdf_cross_check(draw: &buiy_core::render::DrawData, fuzz: &FuzzBudget) -> RefOutcome;
```

**Boundary, stated once (vello/lessons.md):** the shared SDF catches *implementation* drift, not a *spec* error in the SDF itself — if `sdf_rounded_rect` is wrong, both paths are wrong identically and the buffer matches. That residual ("is the shape the *intended* shape") is exactly Tier 5's job; the oracle does not subsume goldens. Use the **same** AA-aware metric as reftests (the report's pixelmatch-YIQ+AA primary); FLIP-for-the-oracle-tier (vello/lessons.md Borrow #2) is deferred to metric.md's Open Question, not adopted here.

## Determinism & the capture gate

Reftests need the determinism stack *less* than goldens (both halves share clock/atlas/DPR in one run, so drift cancels) but reuse it (wpt-reftests/lessons.md Borrow #4 — the `reftest-wait` settle handshake). Before each readback, `run_reftest` asserts the settle condition determinism.md owns: **0 pending assets, glyph atlas warmed, virtual clock at an explicit timestamp, DPR pinned, MSAA off**. This is `wait_for_text_ready` (`tests/support/mod.rs:266`) + `fonts_ready` (`golden.rs:82`) generalized into `DeterministicApp`. Capturing a half-settled frame diffs a half-rendered scene — the WPT capture-before-settle pitfall.

Both captures stay on the **same wgpu backend in the same process** — never a Vulkan-test-vs-Metal-reference pairing (wpt-reftests/lessons.md Avoid). Cross-platform confidence comes from running the whole suite on each *pinned* backend independently (lavapipe in CI, RADV here), not from cross-backend `==`.

## Dependencies

- **No new external crate for the harness itself.** `buiy_verify` already depends on `buiy_core`, `bevy`, `image` 0.25, `proptest`, `serde` (`crates/buiy_verify/Cargo.toml`). `RgbaImage` is `image::RgbaImage`.
- The AA-aware metric (`metric.md`) owns the only new deps (pixelmatch-YIQ port + advisory `image-compare`); reftest consumes them transitively. The SDF oracle is pure `glam`/`image` arithmetic — no new dep.
- **`cargo deny check` note:** no dep is added *by this file*. Any new transitive crate is introduced and license-cleared in metric.md; reftest adds nothing to clear.

## Verification — how the harness tests itself

The harness is test infrastructure, so its own correctness needs meta-tests (pure CPU, **not** `#[ignore]`, run in the headless gate):

1. **Aggregation truth table.** Unit-test `RefKind` + multi-reference OR/AND aggregation against a stub `compare` returning canned `Diff`s — `Match` passes iff within fuzz; `Mismatch` passes iff outside; multi `Match` is OR, multi `Mismatch` is AND. No GPU.
2. **Mismatch-floor guard.** Assert the macro/`run_reftest` rejects a `Mismatch` with a non-`(0,0)` fuzz floor (a `!=` that tolerates difference is vacuous).
3. **Known-good / known-bad pairs (GPU, `#[ignore]`).** A `match` pairing of a scene with *itself* must pass with `(0,0)` (proves capture determinism — the existing `render_golden_harness.rs` re-capture discipline). A `match` pairing of two deliberately-different scenes must **fail** (proves the harness can fail — guards a vacuous green). A `mismatch` of a scene with itself must fail.
4. **Independence lint self-test.** A reference scene that *illegally* contains the forbidden component must trip `assert_reference_independent` (RED), and the canonical disjoint reference must pass (GREEN) — the lint is itself tested, not trusted.
5. **SDF oracle vs. point-probes.** `rasterize_sdf_rect` must reproduce the scalar `d` values the existing `tests/render_instance.rs` point-probes assert (center inside, 2× half-extent outside) — pins the full-tile port to the unit-tested formula, pure CPU.

## Sources

- Code: `crates/buiy_core/tests/support/mod.rs` (capture/readback: `gpu_render_app`/`render_to_image`/`readback_rgba`/`wait_for_text_ready` :134/:204/:353/:266), `crates/buiy_core/src/render/golden.rs` (`GoldenConfig::deterministic`/`fonts_ready` :38/:82), `crates/buiy_core/src/render/shader.wgsl:60` + `tests/render_instance.rs:12` (the shared SDF + its CPU port), `crates/buiy_core/src/render/mod.rs:78` (`DrawData`), `crates/buiy_core/tests/render_golden_harness.rs` (re-capture discipline), `crates/buiy_core/src/layout/systems.rs:3775` (`compose_transform`, the transform-reftest oracle), `crates/buiy_verify/src/visual.rs` (superseded RMSE).
- Prior art: `docs/prior-art/wpt-reftests/lessons.md` (Top-of-file oracle finding, Validates, Avoid, Borrow #1/#4/#6, Open Questions #1/#2), `docs/prior-art/vello/lessons.md` (CPU-vs-GPU cross-check, Top-of-file "stronger oracle", Borrow #1/#2, the spec-error boundary).
- Report: `docs/reports/2026-06-14-visual-bug-detection-strategy.md` § "Tier 4 — reftests (`==` / `!=`)" + "The Vello-style CPU-vs-GPU cross-check" + Open Questions #1/#2.
