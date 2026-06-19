//! CPU oracle for the box-shadow closed form (#27, audit 2026-06-18 T2.8).
//!
//! `shadow.wgsl:66-81` carries an Abramowitz & Stegun 7.1.26 `erf`
//! approximation and a `blurred_box_1d` closed form (the integral of a unit
//! box convolved with a Gaussian). Today that math is only naga-parsed +
//! string-checked (`render_shader_wgsl.rs`), and the draw path is unwired
//! (`extract.rs:348` TODO — no Shadow primitive is emitted), so the
//! `#[ignore]` GPU lane never rasterizes it either: the formula is dead code
//! with zero behavioral coverage.
//!
//! This is a GPU-INDEPENDENT, lowest-tier guard: it re-implements the EXACT
//! same `erf` + `blurred_box_1d` closed form in Rust (the SAME literal
//! constants as the shader, so the oracle is a faithful mirror) and pins it at
//! canonical points. When the draw path lands and the GPU lane begins
//! rasterizing the shadow, this oracle remains the cheap algebraic regression
//! guard underneath the pixel residue — a constant typo or a sign flip in the
//! shader's `erf`/`blurred_box_1d` fails here, headless, long before any
//! adapter is involved.
//!
//! FORWARD-GUARD NOTE: because the shadow draw path is currently unwired, this
//! test guards the FORMULA, not a live render. It is the algebraic contract
//! the future GPU shadow test will sit on top of. The constants below are
//! copied verbatim from `shadow.wgsl`; if that shader's `erf`/`blurred_box_1d`
//! is edited, this oracle must be edited in lockstep (that lockstep IS the
//! regression signal).

/// Abramowitz & Stegun 7.1.26 erf approximation — the EXACT port of
/// `shadow.wgsl:67-74` (max abs error ~1.5e-7). f32 throughout to match the
/// shader's precision.
///
/// `excessive_precision` is allowed DELIBERATELY: the constants are copied
/// verbatim from the WGSL source so this oracle is a byte-faithful mirror of
/// the shader's polynomial — truncating them to f32-representable literals
/// would let the oracle and the shader silently disagree and defeat the
/// lockstep guard. Keep them identical to `shadow.wgsl`.
#[allow(clippy::excessive_precision)]
fn erf(x: f32) -> f32 {
    let s = x.signum();
    let a = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * a);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-a * a).exp();
    s * y
}

/// 1D Gaussian-blurred box coverage — the EXACT port of `shadow.wgsl:78-81`:
/// the integral of a unit box `[-half, half]` convolved with a Gaussian of
/// std-dev `sigma`, evaluated at `p`.
fn blurred_box_1d(p: f32, half: f32, sigma: f32) -> f32 {
    let inv = 1.0 / (std::f32::consts::SQRT_2 * sigma.max(1e-4));
    0.5 * (erf((half - p) * inv) + erf((half + p) * inv))
}

// --- erf canonical points (the closed form's algebraic skeleton) -----------

#[test]
fn erf_is_zero_at_origin() {
    // erf(0) = 0: at the origin the A&S polynomial sums to 1 and exp(0) = 1, so
    // y = 1 - 1 = 0 — the result is 0 regardless of the sign carry. (Rust's
    // f32::signum(0.0) is +1.0, unlike WGSL sign(0.0) = 0.0, so it is y → 0 that
    // zeroes this here, not s.)
    assert_eq!(erf(0.0), 0.0, "erf(0) = 0");
}

#[test]
fn erf_saturates_to_one_for_large_argument() {
    // erf(+inf) = 1; the A&S approximation reaches it to ~1.5e-7 by x = 4.
    let y = erf(4.0);
    assert!((y - 1.0).abs() < 1.5e-6, "erf(4) approaches 1 (got {y})");
    assert!(y <= 1.0 + 1e-6, "erf never overshoots 1 (got {y})");
    // …and the negative tail saturates to -1 (the sign carry).
    let yn = erf(-4.0);
    assert!(
        (yn + 1.0).abs() < 1.5e-6,
        "erf(-4) approaches -1 (got {yn})"
    );
}

#[test]
fn erf_is_odd() {
    // erf(-x) = -erf(x): the explicit `s = sign(x)` carry on |x|.
    for &x in &[0.25_f32, 0.5, 1.0, 2.0, 3.5] {
        let pos = erf(x);
        let neg = erf(-x);
        assert!(
            (pos + neg).abs() < 1e-6,
            "erf is odd at ±{x}: erf({x})={pos}, erf(-{x})={neg}"
        );
    }
}

#[test]
fn erf_matches_reference_at_one_half() {
    // A literal pin against the textbook value erf(0.5) = 0.5204998778…,
    // so a constant typo (not just a sign flip) is caught.
    let y = erf(0.5);
    assert!(
        (y - 0.520_499_9).abs() < 1e-5,
        "erf(0.5) ≈ 0.5205 (got {y})"
    );
}

// --- blurred_box_1d canonical points (the coverage closed form) -------------

#[test]
fn blurred_box_center_is_brightest() {
    // At the box center (p = 0) coverage is highest; at the edge (p = half)
    // it is ~half of center for a symmetric box; outside it decays toward 0.
    let half = 10.0;
    let sigma = 2.0;
    let center = blurred_box_1d(0.0, half, sigma);
    let edge = blurred_box_1d(half, half, sigma);
    let outside = blurred_box_1d(half + 4.0 * sigma, half, sigma);

    assert!(
        center > edge && edge > outside,
        "coverage decays center > edge > outside: {center} > {edge} > {outside}"
    );
    // The edge of a wide box (half ≫ sigma) sits at half-coverage: one erf
    // saturates to ~1, the other (erf(0)) is 0 → 0.5·(1+0) = 0.5.
    assert!(
        (edge - 0.5).abs() < 1e-3,
        "edge of a wide box is half-coverage (got {edge})"
    );
    // Deep interior of a wide box is near full coverage (both erfs saturate).
    assert!(
        center > 0.999,
        "center of a wide box is near-full coverage (got {center})"
    );
}

#[test]
fn blurred_box_normalization_is_pinned_to_a_reference_value() {
    // A literal pin on the 1/(√2·σ) normalization scale: blurred_box_1d(0, 1, 1)
    // reduces to erf(1/√2) = 0.682689… (the standard-normal P(|Z|<1)). The
    // shape/symmetry/limit tests below all still pass under a wrong scale (e.g.
    // a √2 → 2 typo, which would give erf(0.5) = 0.5205); this value pin is what
    // catches a normalization error in the closed form.
    let c = blurred_box_1d(0.0, 1.0, 1.0);
    assert!(
        (c - 0.682_689).abs() < 1e-3,
        "blurred_box_1d(0,1,1) = erf(1/√2) ≈ 0.6827 (got {c})"
    );
}

#[test]
fn blurred_box_is_symmetric_in_p() {
    // blurred_box_1d(p) == blurred_box_1d(-p): the two erf terms swap, and
    // erf is odd, so the sum is even in p.
    let half = 7.0;
    let sigma = 3.0;
    for &p in &[0.0_f32, 1.0, 5.0, 7.0, 12.0] {
        let a = blurred_box_1d(p, half, sigma);
        let b = blurred_box_1d(-p, half, sigma);
        assert!(
            (a - b).abs() < 1e-6,
            "even in p at ±{p}: f({p})={a}, f(-{p})={b}"
        );
    }
}

#[test]
fn tiny_sigma_approaches_a_hard_box() {
    // As sigma → 0 the Gaussian collapses to a delta and the closed form
    // approaches the hard box indicator: ~1 inside, ~0 outside. (The shader
    // clamps sigma at 1e-4, so this is the floor regime, not a true limit.)
    let half = 5.0;
    let sigma = 1e-4; // the shader's max(sigma, 1e-4) floor

    // Well inside → ~1.
    let inside = blurred_box_1d(0.0, half, sigma);
    assert!(
        inside > 0.999,
        "tiny sigma: interior is a hard 1 (got {inside})"
    );
    // Well outside → ~0.
    let outside = blurred_box_1d(half + 1.0, half, sigma);
    assert!(
        outside < 1e-3,
        "tiny sigma: exterior is a hard 0 (got {outside})"
    );
    // Exactly at the edge → ~half (the erf((half-p)) term hits erf(0)=0,
    // the other saturates): the box-indicator midpoint value.
    let edge = blurred_box_1d(half, half, sigma);
    assert!(
        (edge - 0.5).abs() < 1e-3,
        "tiny sigma: the edge is the box midpoint 0.5 (got {edge})"
    );
}

#[test]
fn coverage_stays_in_unit_range() {
    // The product of two axis coverages is an alpha in [0, 1]; each 1D term
    // must itself stay in [0, 1] across a sweep (no overshoot from the erf
    // approximation feeding a negative or >1 alpha).
    let half = 8.0;
    for sigma in [0.5_f32, 1.0, 2.0, 5.0] {
        for i in -40..=40 {
            let p = i as f32 * 0.5;
            let c = blurred_box_1d(p, half, sigma);
            assert!(
                (-1e-6..=1.0 + 1e-6).contains(&c),
                "coverage in [0,1] at p={p}, sigma={sigma} (got {c})"
            );
        }
    }
}
