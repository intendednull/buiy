# Perceptual metric — `buiy_verify::metric`

**Date:** 2026-06-15
**Status:** draft
**Spec:** specs/2026-06-15-buiy-verification-design/README.md

The one image-comparison metric for the whole pyramid: an AA-aware, two-axis
fuzzy diff that replaces the two naive metrics on `main` — the L1
`perceptual_diff` (`render/golden.rs:56`) and the global RMSE `compare_images`
(`buiy_verify/src/visual.rs:18`). It is the shared primitive consumed by
**tier-4 reftests** (fuzzy `==`/`!=` in one process) and **tier-5 goldens**
(stored-baseline regression), so both tiers express tolerance the same way. The
per-pixel decision is pixelmatch's luminance-weighted YIQ `colorDelta` with an
antialias-sibling exclusion; an advisory MSSIM channel catches global drift a
small pixel budget under-weights.

## Contract deviations

None. Signatures below match the SHARED API CONTRACT. Two clarifications (not
deviations): the gate uses the YIQ-weighted per-pixel delta while `max_channel_delta`
is the raw L∞ kept for diagnostics; `mssim` is `Option` so it is skipped (`None`)
on empty/disabled input, never silently `0.0`.

## Why the naive metrics fail (report §4)

Both average one global scalar: L1 = `Σ|Δ|/(len·255)`, RMSE = `√(Σ Δ²/(px·4·255²))`.
A defect touching 0.5% of pixels (a mispositioned glyph, a missing focus ring, an
8px wrong-color badge) divides across the whole frame and rounds below any sane
tolerance — **sensitivity degrades as the app grows** — while imperceptible sub-pixel
AA re-rasterization inflates the same number. One knob cannot separate the two.
Mozilla `reftest` (`fuzzy`), wgpu (abandoned `Outlier count N over M`, then FLIP),
and pixelmatch all converged on the same fix: a **two-axis budget with AA awareness**,
not an average (report §4; `prior-art/wgpu-testing/lessons.md` — wgpu PR #3830 /
issue #2760).

## Module layout

`crates/buiy_verify/src/metric.rs` (pure CPU, no GPU, no `bevy`). Operates on
`image::RgbaImage` (`image = "0.25"`, already a workspace dep). Re-exported as
`buiy_verify::metric`.

```rust
//! Perceptual image diff — the shared metric for reftests (tier 4) and goldens
//! (tier 5). pixelmatch-YIQ colorDelta + antialias-sibling exclusion, gated on a
//! two-axis FuzzBudget. Supersedes render::golden::perceptual_diff (L1) and
//! visual::compare_images (RMSE).
```

### Types

```rust
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
    pub diff_image: Option<image::RgbaImage>,
}

/// The two-axis gate. A Diff PASSES iff BOTH hold. Default after determinism is
/// (0, 0); widen per fixture with a documented reason. Per Mozilla's
/// `fuzzy-if` discipline a *widened* budget should pin BOTH ends (a separate
/// min-budget assertion, below) so a shrinking diff is itself a regression.
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

/// Per-pixel and AA-detection knobs. `threshold` feeds the pixelmatch
/// `maxDelta = 35215 · threshold²` luminance model; `include_aa = true` makes
/// AA pixels COUNT (for the few tests that assert AA exactly).
#[derive(Clone, Copy, Debug)]
pub struct CompareOpts {
    /// Matching sensitivity in [0,1]; pixelmatch default 0.1. Smaller = stricter.
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
```

### Functions

```rust
/// Compare two RGBA images. **Infallible** — returns a `Diff`, never a
/// `Result`, so callers (`assert_golden`, `run_reftest`) need no error arm and
/// the crate stays `thiserror`-free (`thiserror` is project-deferred).
///
/// A **dimension mismatch** folds into a *saturated* `Diff` that FAILS any
/// budget: `differing_pixels == total_pixels`, `max_channel_delta == 255`,
/// `mssim == Some(0.0)`. This mirrors the existing `compare_images` /
/// `perceptual_diff`, which return a maximal-difference sentinel on a size
/// mismatch (`visual.rs:19`, `golden.rs:58`) — but, crucially, it is the
/// *fail* direction, not the silent-pass bug §4 removes: the naive `1.0` was a
/// problem only because a separate code path let a `1.0` score satisfy a max
/// budget. Here the saturated `Diff` makes `passes(&_)` false for **every**
/// budget, so a mis-sized capture reds the gate loudly instead of squeaking
/// through. (`total_pixels` is set to `max(a.area, b.area)` so the saturation
/// count is well-defined.)
///
/// An **empty image** (zero pixels) yields `Diff { differing_pixels: 0,
/// max_channel_delta: 0, total_pixels: 0, mssim: None, .. }` — there is no
/// difference to observe in an empty set, matching `compare_images`'s `0.0`
/// for the empty case. A harness that wants to forbid empty captures asserts
/// `total_pixels > 0` itself (the determinism quiescence gate already does).
pub fn compare(a: &image::RgbaImage, b: &image::RgbaImage, opts: &CompareOpts) -> Diff;

impl Diff {
    /// PASS iff `max_channel_delta <= budget.max_channel_delta`
    /// AND `differing_pixels <= budget.max_diff_pixels`. (MSSIM is advisory and
    /// never gates here — see below.)
    pub fn passes(&self, budget: &FuzzBudget) -> bool;

    /// Mozilla `fuzzy-if` "ranges must not include 0" discipline: when a fixture
    /// widens its budget because a difference is EXPECTED, assert the diff also
    /// meets a floor, so a suddenly-clean render is flagged as a regression.
    pub fn within(&self, min: &FuzzBudget, max: &FuzzBudget) -> bool;
}
```

## The per-pixel decision — pixelmatch YIQ `colorDelta`

For each overlapping pixel, convert both samples to YIQ and weight the squared
delta `0.5053·ΔY² + 0.299·ΔI² + 0.1957·ΔQ²` (luminance dominates, matching the
eye). The acceptance bound is pixelmatch's `maxDelta = 35215 · threshold²`; a
pixel is *differing* iff its weighted delta exceeds it. Adopting the reference
algorithm (not re-deriving the `35215`/YIQ constants) is the point — brightness
errors then outweigh chroma, unlike L1/RMSE's equal channel weighting (report §4).

### Antialias exclusion — the brightest/darkest-neighbor sibling test

The single feature both naive metrics lack and the biggest GPU-pipeline flake
source (SDF `smoothstep` edge + linear→sRGB encode jitter sub-LSB, `golden.rs:52`).
pixelmatch's `antialiased(img, x, y, …)` predicate: a pixel is AA iff it has a
neighbor that is the **brightest** and one the **darkest** relative to it (by YIQ
luminance) and is not a hard edge in *both* images. A differing pixel that is AA in
*either* image is excluded from `differing_pixels` unless `include_aa`. This lets
`FuzzBudget::EXACT` (0,0) hold across the pinned rasterizer's residual AA jitter
while still catching a one-pixel real defect (a glyph shifted off the AA band).

## Crate choice — vendor pixelmatch, don't hand-roll

| Option | Verdict |
|---|---|
| **Hand-roll** the YIQ delta + sibling test | rejected — re-deriving battle-tested constants is the anti-pattern §4 warns against |
| **`dify = "0.8.0"`** | rejected — packaged as a CLI binary; its diff core is not a clean library surface and pulls extra deps |
| **`pixelmatch = "0.1.0"`** | **selected** — pure-Rust port of the canonical JS pixelmatch (YIQ `colorDelta` + AA sibling test) over `image` buffers, ~150 LOC, zero native/FFI cost |

Primary dep (new): **`pixelmatch = "0.1.0"`** in `buiy_verify` — pure Rust, no
build script, MIT-licensed (compatible). It exposes the `colorDelta`/`antialiased`
primitives `compare` wraps; the `FuzzBudget` two-axis gate, `Diff` shape, MSSIM
channel, and the saturated-`Diff` mismatch handling are Buiy's layer on top
(pixelmatch returns only a flat changed-pixel count).

> **`cargo deny check` note.** `pixelmatch = "0.1.0"` and `image-compare =
> "0.5.0"` are both new workspace deps; run `cargo deny check` before adding
> either (CLAUDE.md "supply-chain check"). pixelmatch is a thin, dependency-light
> port; `image-compare` pulls `nalgebra` — confirm the license set
> (MIT/Apache/BSD) and no `RUSTSEC` advisories in the same audit. Both ride the
> existing `image = "0.25"`; no second image-decode stack enters the tree. Pin
> exact patch versions (`=0.1.0`, `=0.5.0`) so a rasterizer-independent metric
> bump cannot silently shift baselines.

## Advisory MSSIM — `image-compare`

Secondary, **advisory-only** channel via **`image-compare = "0.5.0"`**
(`rgba_blended_hybrid_compare`, premultiplied against the opaque capture canvas),
surfaced as `Diff::mssim: Option<f64>`. It catches global gamma/blend drift a
small-N pixel budget under-weights (a uniform 1-LSB gamma shift is zero differing
pixels but a visible wash). It is **never the primary gate** — its failure mode is
averaging out localized defects, exactly the L1/RMSE weakness — so `Diff::passes`
ignores it; harnesses log it or assert it as a soft secondary in goldens.
(`dssim-core` is the structural fallback if MSSIM proves too coarse; not adopted.)

## FLIP — the deferred fork (report Open Question #3 / prior-art)

`prior-art/wgpu-testing/lessons.md` and `prior-art/vello/lessons.md` both
recommend NVIDIA ꟻLIP (`nv-flip`) as *primary*: wgpu migrated to it (PR #3830)
for AA tolerance, Vello gates `vello_tests` on `FlipPool::mean()`. This spec picks
**pixelmatch-primary** because it is pure Rust (no `nv-flip-sys` C++ FFI build cost
— a CI burden Vello's lesson flags), and it natively yields the **two-axis budget
reftests need** (FLIP yields one mean scalar, not a count + max-delta). If
pixel-budget tuning proves insufficient for the oracle/golden tiers, `metric` gains
a `flip` feature adding an `nv-flip` dev-dependency behind the same `Diff`/`FuzzBudget`
surface (its mean → a single-axis budget) — designed as an additive swap, not a
rewrite. Per Vello, the metric may legitimately differ per failure mode; the shared
`compare` + `CompareOpts.threshold` already expresses that spread.

## How the two consuming tiers share this metric

- **tier-4 reftests** (`buiy_verify::reftest`): `run_reftest` renders the test
  and reference scenes in **one process** and calls `metric::compare`; platform
  variance cancels because both halves share the GPU/driver/clock, so
  `FuzzBudget` near `EXACT` holds. `RefCase.kind = Match` asserts `passes`;
  `Mismatch` asserts `!passes` (the feature must *do* something). Same call backs
  the CPU-vs-GPU SDF cross-check (CPU `sdf_rounded_rect` oracle vs GPU readback).
- **tier-5 goldens** (`buiy_verify::golden`): `assert_golden(name, &img,
  &budget)` loads `tests/goldens/<key>.png` and calls the *same* `compare`;
  `emit_diff_image` is on so the triage HTML embeds the heatmap. One metric, one
  budget vocabulary across both tiers — the §4 unification.

## Migration of the two naive metrics

1. **`buiy_verify::visual::compare_images` (RMSE)** — deleted. Its 4 callers in
   `crates/buiy_verify/tests/visual.rs` migrate to `metric::compare` +
   `Diff::passes(&budget)`. **A 5th reference** — the symbol-existence smoke test
   `crates/buiy_verify/tests/smoke.rs:4` (`let _ = visual::compare_images;`) —
   must also be deleted (or re-pointed at `metric::compare`) when the symbol goes,
   or the smoke test stops compiling. `DiffResult{score}`/`passed(tol)` removed.
2. **`buiy_core::render::golden::perceptual_diff` (L1)** — `buiy_core` cannot
   depend on `buiy_verify` in its *normal* (`[dependencies]`) graph — the harness
   depends on core, not the reverse — so the production `perceptual_diff` is
   **deprecated in place** (`#[deprecated(note = "use
   buiy_verify::metric::compare")]`), its L1 body kept only for the existing
   `#[ignore]` GPU re-capture tests until they migrate. To make
   `buiy_verify::metric` reachable from those tests, the plan **adds `buiy_verify`
   as a dev-dependency of `buiy_core`**: `buiy_verify = { path =
   "../buiy_verify" }` under `[dev-dependencies]` in `crates/buiy_core/Cargo.toml`
   (which today lists only `naga` there). This is a **dev-only dependency cycle**
   (`buiy_core` → `buiy_verify` → `buiy_core`), which Cargo permits — a
   dev-dependency edge does not participate in the normal build graph, so it
   creates no real cycle and does not affect `cargo deny`. The cycle is
   intentional and confined to `#[cfg(test)]`. With that edge in place, the ~20
   call sites in `tests/text_*_gpu.rs` (e.g. `text_gpu.rs:114`,
   `text_golden_suite_gpu.rs:260`) move to `buiy_verify::metric::compare` when
   those re-capture checks become stored goldens (tier-5, a later plan step).
   Net: one metric, with a deprecation gravestone, not a duplicate.

### Re-capture determinism / anti-tests — `compare`, not `assert_golden`

Not every `perceptual_diff` site is a latent golden. The `text_*_gpu.rs` suite
has two *non-golden* shapes that compare **two in-process captures** against each
other (never a stored baseline), and both migrate onto `metric::compare` while
**staying as in-test assertions** — no PNG is stored, no `assert_golden` is
involved. They are determinism / behavior checks, the reftest pattern expressed
without a markup reference:

- **"must be stable within budget"** — the re-capture determinism sites that today
  assert `perceptual_diff(a, b) < tol` (e.g. `text_gpu.rs:114`, `:216`, `:359`,
  `:452`; `text_gpu.rs:544`). These become
  `compare(&a, &b, &CompareOpts::default()).within(&min, &max)` where the budget
  is `FuzzBudget::EXACT` (`(0,0)`) once the determinism stack lands — i.e. *plain*
  `passes(&EXACT)` for the bit-exact case. They assert two fresh captures of the
  same scene agree; this is the `RefKind::Match`-of-a-scene-with-itself property,
  inlined in the text suite.

- **"must differ"** — the mismatch/anti-tests that today assert
  `perceptual_diff(a, b) > tol` (`text_gpu.rs:152`, `:271`): proof that flipping
  an input (a different glyph, a moved caret) actually *changes the pixels*, the
  silent-no-op guard. These become `!compare(&a, &b, &CompareOpts::default())
  .passes(&FuzzBudget::EXACT)` — i.e. the captures must **not** match at the exact
  budget. This is exactly `RefKind::Mismatch`'s `!passes` with a forced `(0,0)`
  floor (reftests.md); a convenience `assert_differs(&a, &b)` wrapper in the test
  module reads cleaner than the negation and is the recommended spelling. A budget
  that tolerated difference would make the anti-test vacuous, so the floor is
  pinned at `EXACT`.

Both shapes are **in-test assertions on a live pair**, NOT stored goldens — they
diff two captures from the same run, so no baseline corpus, no bless ledger, no
`tests/goldens/` entry. Only the sites that compare a capture against a *stored*
reference (the `text_golden_suite_gpu.rs` baselines) become tier-5
`assert_golden`. The migration is therefore three-way: stored-baseline →
`assert_golden`; same-run stability → `compare(..).passes(&EXACT)`; same-run
mismatch → `!compare(..).passes(&EXACT)` / `assert_differs`.

## Verification — testing the metric itself

The harness's own correctness is asserted with pure-CPU unit tests in
`crates/buiy_verify/tests/metric.rs` (no GPU), each a known-answer case:

- **Identity:** `compare(img, img, default)` ⇒ `differing_pixels == 0`,
  `max_channel_delta == 0`, `mssim == Some(1.0)`, `passes(&EXACT)`.
- **Single-pixel defect survives scale (the §4 regression):** an N×N image with
  exactly one wrong-by-200 pixel yields `differing_pixels == 1` and
  `!passes(&EXACT)` for *every* N — proving sensitivity does NOT dilute with
  frame size (the exact failure of L1/RMSE; assert across N ∈ {16, 256, 2048}).
- **AA exclusion vs `include_aa`:** a synthetic edge AA'd one pixel-band wide
  reads `differing_pixels == 0` with default opts and `> 0` with
  `include_aa = true` — pins the sibling test on/off.
- **Two-axis independence:** a case that trips `max_channel_delta` but not
  `max_diff_pixels` (one pixel off by 255) and the converse (many pixels off
  by 1, below the YIQ threshold) — each must fail the gate, proving BOTH axes
  bind.
- **`within` floor (fuzzy-if):** a diff below a widened `min` budget fails
  `within(min,max)` — proving an unexpectedly-clean render is caught.
- **Dimension mismatch** ⇒ a saturated `Diff` (`differing_pixels ==
  total_pixels`, `max_channel_delta == 255`) that `!passes(&_)` for *every*
  budget, including a hypothetical maximal one — pins the fail direction (the
  loud-red replacement for the naive `1.0` silent-pass). **Empty** ⇒ a zero
  `Diff` with `total_pixels == 0`; a separate assertion (`total_pixels > 0`)
  forbids empty captures where that matters.
- **YIQ luminance weighting:** an equal-L∞ luma-channel change scores a larger
  YIQ delta than a chroma-only change — pins that brightness outweighs chroma.
- **Advisory isolation:** a failing-MSSIM, zero-pixel diff still `passes` — MSSIM
  never gates. A checked-in 8×8 PNG pair + its expected `Diff` (an `insta`
  snapshot, floats redacted) guards the constants against a pixelmatch bump.

All run under the headless `cargo test --workspace` gate (no `#[ignore]`, no
adapter) — the metric is pure CPU, so its self-test needs no GPU lane.

## Sources

Code: `crates/buiy_core/src/render/golden.rs:48-66` (L1 `perceptual_diff`),
`crates/buiy_verify/src/visual.rs:18-45` (RMSE `compare_images`),
`crates/buiy_core/src/render/instance.rs:40-58` (`PackedInstance`, the
byte-snapshot sibling primitive), `crates/buiy_core/tests/text_gpu.rs:114`/`:152`/`:271`
(re-capture `perceptual_diff` call sites to migrate — `:114` stable, `:152`/`:271`
mismatch anti-tests),
`crates/buiy_verify/tests/visual.rs` + `crates/buiy_verify/tests/smoke.rs:4`
(RMSE `compare_images` callers to migrate, incl. the symbol-existence smoke test).
Prior-art:
`docs/prior-art/wgpu-testing/lessons.md` (outlier-count brittleness → FLIP, the
pixelmatch-vs-FLIP runner-up), `docs/prior-art/vello/lessons.md` (per-tier metric
choice, `nv-flip` FFI cost, FLIP-mean oracle gate). Report:
`docs/reports/2026-06-14-visual-bug-detection-strategy.md` §4 "Perceptual metric
— replace the two naive metrics" and Open Question #3.
