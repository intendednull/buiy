**Date:** 2026-06-14
**Status:** active
**Subject:** Vello — Validates / Avoid / Borrow decisions for Buiy's visual-bug-detection strategy

# Lessons for Buiy

The consult-this-when-designing file. The other Vello files are evidence; this file is decisions. Buiy implications live here.

Vello is the **canonical precedent** for a wgpu 2D renderer paired with a CPU reference rasterizer (`vello_cpu`) born specifically to backstop GPU shortcomings — directly analogous to Buiy promoting its CPU SDF port to an oracle. It sits in the visual-bug-detection corpus as the closest *greenfield neighbor*: same substrate (wgpu, Rust, MIT-OR-Apache-2.0), same idea (CPU oracle cross-checks GPU output with a perceptual metric in tests). Buiy's strategy doc is `docs/reports/2026-06-14-visual-bug-detection-strategy.md`; this file feeds its reftests-first pyramid.

## Top of file: one finding reframes the rest

### Buiy's oracle is *stronger* than Vello's, because Buiy's two paths share one analytic function.

Vello's tier-3 CPU-vs-GPU "comparison tests" are slated to be "**largely phased out**" because `vello_cpu` and `vello` are two *independently-implemented* pipelines — their agreement is a weak invariant (each could be wrong in the same way, or the test could drift as either evolves). That is why Vello calls them transitional scaffolding ([`cpu-gpu-testing.md`](cpu-gpu-testing.md)).

Buiy's situation is different: the CPU oracle and the GPU shader evaluate **the same closed-form `sdf_rounded_rect`**. They are not two implementations of "draw a rounded rect"; they are two evaluations of one function. Their agreement-to-float-tolerance is therefore a *durable* invariant, and any divergence **localizes a real shader bug** — wrong half-extent, radius clamp, premultiply error. Buiy should NOT inherit Vello's "phase out the cross-check" posture. The cross-check is Buiy's tier-2 backbone, permanently.

**The asymmetry has a cost, and it is the same one Buiy levels at Vello.** Sharing one function means the cross-check catches *shader-implementation* divergence (premultiply, clamp, half-extent, AA step) but is blind to a *spec* error in the SDF itself: if `sdf_rounded_rect` is wrong, the CPU port and the GPU shader are wrong *identically*, the buffers still match, and the test stays green — the exact "each could be wrong in the same way" failure mode this section uses to discount Vello's two-implementation cross-check. The shared-SDF oracle is strictly stronger against implementation drift and strictly *no help* against a wrong SDF. That residual class — does the rendered shape match the *intended* shape — is precisely what the golden-screenshot and reftest tiers exist to cover; the oracle tier does not subsume them.

---

## Validates

Buiy decisions confirmed by Vello's experience:

- **CPU reference rasterizer as a deliberate backstop for GPU output.** `vello_cpu` was built *specifically* to backstop the flagship GPU renderer's shortcomings. Buiy's plan to promote its CPU SDF port to an oracle is the same move, validated by the canonical wgpu-2D precedent. See [`sparse-strips.md`](sparse-strips.md).
- **Non-exact, perceptual image comparison for GPU output.** Vello never asserts exact pixel equality anywhere — GPU fast-math and precision differences guarantee small divergence even when both renderers are correct (verbatim rationale in [`metric-and-kompari.md`](metric-and-kompari.md)). Buiy's instinct to tolerate sub-pixel AA noise rather than demand exact match is correct.
- **A high-precision (`f32`) CPU path as the snapshot/oracle generator.** `vello_cpu`'s `f32` `OptimizeQuality` pipeline exists "especially … for rendering test snapshots." Confirms that an oracle should be the *most accurate* available evaluation of the spec, even if slow — it only runs in tests. See [`sparse-strips.md`](sparse-strips.md).
- **Renderer-with-no-a11y boundary.** Vello keeps a11y out of the renderer entirely (it lives in the framework). Buiy's render layer likewise carries no a11y; the decomposed a11y components sit above it. See [`ecosystem-maturity.md`](ecosystem-maturity.md).
- **One scene format, multiple backends with comparable output.** Vello's `Encoding` is consumed by three backends. Buiy gets comparability for free because its oracle and shader share the SDF function — no separate scene-encoding layer needed.

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **The compute-centric architecture.** Vello's pipeline is four prefix-sum compute stages (flatten/binning/coarse/fine) plus a `Scene`→`Encoding`→stream machinery — solving GPU-compute-portability problems Buiy doesn't have. | [`architecture.md`](architecture.md), [`open-problems.md`](open-problems.md) | Borrow the *testing* idea, not the renderer. Buiy is instanced quads + per-fragment SDF; its oracle is a trivial per-pixel function eval, far simpler than porting a sparse-strip rasterizer. |
| **Runtime dependency on pre-1.0 `vello` / `vello_cpu` as the oracle.** Flagship is alpha; sparse-strips crates are `0.0.x` ("do not depend on stability"); glyph/blur/memory strategies churn — the output would be a moving target. MSRV Rust 1.88. | [`ecosystem-maturity.md`](ecosystem-maturity.md), [`sparse-strips.md`](sparse-strips.md) | Buiy's oracle is its *own* CPU SDF port (already half-built), not a Vello dependency. No external renderer in the correctness loop. |
| **Treating the CPU-vs-GPU cross-check as transitional.** Vello phases it out because its two paths are different implementations. | [`cpu-gpu-testing.md`](cpu-gpu-testing.md) | Buiy's two paths share one analytic SDF → the cross-check is a durable invariant, kept permanently (Top of file). |
| **Assuming one image-diff metric fits all tiers.** Linebender accidentally runs two (FLIP for `vello_tests`, tolerance-16 pixel diff for xilem) because FLIP has false negatives on sub-perceptual changes ("dark grey and white … very similar"). | [`metric-and-kompari.md`](metric-and-kompari.md) | Buiy picks per failure mode *deliberately*: FLIP for the oracle-agreement tier (tolerate GPU noise), tight pixel tolerance for the golden-screenshot tier (catch small intentional regressions). |
| **Adopting Vello's threshold number blindly.** Vello's mean-error bound is tuned to *its* AA model. | [`metric-and-kompari.md`](metric-and-kompari.md) | Calibrate Buiy's threshold on a known-good Buiy frame. |
| **`nv-flip`'s native-toolchain cost in any shipping path.** `nv-flip` is pre-1.0 (0.1.2, unchanged since 2023) and wraps a C++ lib via `nv-flip-sys`, adding a build-time native cost. | [`metric-and-kompari.md`](metric-and-kompari.md) | Acceptable as a **dev-dependency** in the test harness only; never in a shipping path. (Or consider a pure-Rust FLIP/ΔE if the native cost bites CI.) |
| **Git LFS PNG reference store for the cross-check tier.** Vello keeps `snapshots/*.png` in LFS. | [`cpu-gpu-testing.md`](cpu-gpu-testing.md) | The CPU-oracle approach lets Buiy *defer* LFS entirely for the rasterization cross-check; reserve LFS for the genuine golden-screenshot top tier only. |

## Borrow

Concrete primitives worth adapting:

1. **CPU SDF as oracle — promote the existing port.** `crates/buiy_core/tests/render_instance.rs` (the comment + `fn sdf_rounded_rect` at lines 10-15) already contains a "Pure-CPU port of `shader.wgsl::sdf_rounded_rect`" that mirrors the GPU SDF 1:1. Today it is used only for **scalar distance assertions at a few sample points** (`logical_sdf_inside_is_filled_outside_is_empty`, lines 17-34: probes center-inside and 2×-half-extent-outside). The Vello lesson is to **promote it to a full-tile rasterizer**: evaluate `sdf_rounded_rect` per-pixel with the same AA coverage step the WGSL uses, producing a coverage/color buffer, then diff that buffer against a real GPU readback of the same instance. Three point-probes become a dense per-pixel oracle with **zero checked-in PNGs**. Because the SDF is analytic, CPU and GPU should agree to within float/AA tolerance; divergence localizes a real shader-implementation bug — exactly the failure class the existing single-point `d_center` assertion only gestures at. (Reminder of the boundary from Top-of-file: this catches implementation drift, *not* a wrong SDF — both paths would share that error.) **Prerequisite to scope before committing to this Borrow:** the "real GPU readback" half is not free. It needs a headless GPU path in CI — adapter selection, an `xvfb`-style virtual display (Buiy's gate already wraps tests in `xvfb-run`), and a buffer-readback step (Vello's own pipeline exposes a `Download` command for exactly this, [`architecture.md`](architecture.md):27, but the folder does not document Vello's headless-readback mechanics, so treat that as Buiy-side design work, not a liftable recipe). Stand up the readback-in-CI path first; the per-pixel diff is the easy half. See [`sparse-strips.md`](sparse-strips.md), [`cpu-gpu-testing.md`](cpu-gpu-testing.md).

2. **`nv-flip` mean-error gate for the oracle tier — resolving pixelmatch-YIQ-vs-FLIP toward FLIP.** Copy Vello's shape: `FlipPool::mean()` with a small fixed threshold via an `assert_mean_less_than`-style helper. FLIP models the difference a human perceives when *flipping* between two images — the exact reftest viewing mode — and yields a continuous, perceptually-weighted scalar that tolerates sub-pixel AA/dithering noise inherent in GPU-vs-CPU SDF agreement, *without* hand-tuned color thresholds. This beats pixelmatch's binary YIQ-threshold count for the **oracle** tier. (For the **golden** tier, see the Avoid row on per-tier metrics — a tight pixel tolerance may be better there.) Rust binding `nv-flip` v0.1.2; API `FlipImageRgb8::with_data` → `flip(ref, test, DEFAULT_PIXELS_PER_DEGREE=67.0)` → `FlipPool::mean()`; license MIT/Apache-2.0/Zlib (FLIP core BSD-3-Clause), all Buiy-compatible. See [`metric-and-kompari.md`](metric-and-kompari.md).

3. **The snapshot-harness skeleton (render A, render B, perceptual-diff, assert).** Vello's loop is reusable independent of whether B is a CPU oracle (oracle mode) or a checked-in PNG (golden mode). Buiy reuses **one harness for two tiers of its pyramid**. Lift the `TestParams`-style config struct and the env-var blessing flow (`VELLO_TEST_CREATE` / `VELLO_TEST_UPDATE` → a `BUIY_TEST_*` equivalent: missing reference + create flag writes it; mismatch + update flag overwrites). See [`cpu-gpu-testing.md`](cpu-gpu-testing.md).

4. **Runtime SIMD `Level` pattern (if/when the oracle vectorizes).** `vello_cpu`'s `Level` enum detects the best SIMD level at runtime with a scalar fallback as the definition-of-correct. If Buiy's per-pixel oracle ever needs to vectorize, adopt the same shape: detect once, dispatch the inner loop, keep scalar as ground truth. See [`sparse-strips.md`](sparse-strips.md).

5. **Watch Kompari, don't depend on it yet.** Kompari (HTML diff reports + interactive blessing server) is the Linebender convergence plan but has **no published releases**. If Buiy wants a diff-report UX later, Kompari is the reference shape; for now, the env-var blessing flow (Borrow #3) is enough. See [`metric-and-kompari.md`](metric-and-kompari.md).

## How to use this file

When designing a Buiy visual-bug-detection tier:

1. **For the rasterization cross-check tier** (GPU readback vs CPU SDF), read Borrow #1 + #2 and the Top-of-file note: promote the existing CPU SDF port, gate with FLIP mean-error.
2. **For the golden-screenshot top tier**, read the Avoid "per-tier metric" row: prefer a tight pixel tolerance over FLIP there; reserve LFS for this tier only.
3. **For the harness shape itself**, read Borrow #3.
4. **Don't take a Vello runtime dependency** — Avoid rows 1-2. Borrow the pattern, build Buiy's own oracle.
5. **Promote decisions into the strategy report / Buiy specs**, not just this file.

## Sources

- This corpus's evidence files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`sparse-strips.md`](sparse-strips.md), [`cpu-gpu-testing.md`](cpu-gpu-testing.md), [`metric-and-kompari.md`](metric-and-kompari.md), [`ecosystem-maturity.md`](ecosystem-maturity.md), [`open-problems.md`](open-problems.md), [`glossary.md`](glossary.md)
- Buiy existing CPU SDF port: `crates/buiy_core/tests/render_instance.rs` (lines 10-34)
- Buiy visual-bug-detection strategy report: `docs/reports/2026-06-14-visual-bug-detection-strategy.md`
- Sibling Linebender prior-art (framework angle): [`../xilem-masonry/lessons.md`](../xilem-masonry/lessons.md)
- FLIP paper: https://dl.acm.org/doi/10.1145/3406183
- `vello_tests`: https://github.com/linebender/vello/tree/main/vello_tests
- DeepWiki testing & validation: https://deepwiki.com/linebender/vello/5.2-testing-and-validation
