//! Easing curves for tweens.
//!
//! The Widget Catalog design uses exactly two timing functions (values.md
//! § 5.1): the CSS default `ease` and `cubic-bezier(.2,.8,.2,1)` (the switch
//! thumb + progress-fill curve). Both are expressible as [`Easing`]:
//!
//! - `Easing::Linear` — identity, `f(t) = t`.
//! - `Easing::CubicBezier(x1, y1, x2, y2)` — the CSS `cubic-bezier()` form with
//!   control points `P1 = (x1, y1)`, `P2 = (x2, y2)`; `P0 = (0, 0)` and
//!   `P3 = (1, 1)` are implicit. The CSS `ease` keyword is
//!   `CubicBezier(0.25, 0.1, 0.25, 1.0)`; the design curve is
//!   `CubicBezier(0.2, 0.8, 0.2, 1.0)`.
//!
//! A CSS cubic-bezier is a *parametric* curve `(x(s), y(s))` over `s ∈ [0, 1]`;
//! the eased output for a time fraction `t` is `y(s)` where `s` solves
//! `x(s) = t`. Because `x(s)` is monotonic for the well-formed UI curves we use
//! (`x1, x2 ∈ [0, 1]`), the inversion has a unique root. We find it with a few
//! Newton-Raphson iterations seeded from a coarse LUT — accurate to well under a
//! pixel for UI, with no per-sample allocation. This mirrors the standard
//! browser implementation (WebKit's `UnitBezier`).

/// A timing function mapping an input fraction `t ∈ [0, 1]` to an output
/// fraction in `[0, 1]` (outputs may briefly exceed the range for overshoot
/// curves, but the design's curves stay in-range).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Easing {
    /// Identity: `f(t) = t`. The default.
    #[default]
    Linear,
    /// CSS `cubic-bezier(x1, y1, x2, y2)` with implicit endpoints
    /// `(0,0)`/`(1,1)`.
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// The CSS `ease` keyword — `cubic-bezier(0.25, 0.1, 0.25, 1.0)`. This is
    /// the default timing function for every design transition that does not
    /// name `cubic-bezier(.2,.8,.2,1)` (values.md § 5.1).
    pub const EASE: Easing = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);

    /// The design's named curve `cubic-bezier(.2,.8,.2,1)` — the switch-thumb
    /// `left` transition and the progress/meter-fill `width` transition
    /// (values.md § 5.1).
    pub const DESIGN: Easing = Easing::CubicBezier(0.2, 0.8, 0.2, 1.0);

    /// Sample the curve at input fraction `t`. `t` is clamped to `[0, 1]`; the
    /// endpoints are exact (`sample(0) == 0`, `sample(1) == 1`).
    pub fn sample(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            Easing::Linear => t,
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier_sample(x1, y1, x2, y2, t),
        }
    }
}

/// One Bezier coordinate `B(s)` (and its derivative `B'(s)`) for control
/// values `c1`, `c2` (endpoints fixed at 0 and 1). Coefficient form:
/// `B(s) = ((a·s + b)·s + c)·s` where
/// `c = 3·c1`, `b = 3·(c2 - c1) - c`, `a = 1 - c - b`.
#[inline]
fn bezier_coord(c1: f32, c2: f32, s: f32) -> f32 {
    let c = 3.0 * c1;
    let b = 3.0 * (c2 - c1) - c;
    let a = 1.0 - c - b;
    ((a * s + b) * s + c) * s
}

#[inline]
fn bezier_coord_deriv(c1: f32, c2: f32, s: f32) -> f32 {
    let c = 3.0 * c1;
    let b = 3.0 * (c2 - c1) - c;
    let a = 1.0 - c - b;
    (3.0 * a * s + 2.0 * b) * s + c
}

/// Number of LUT buckets used to seed the Newton-Raphson root-find. 64 buckets
/// give a seed within `1/64` of the true `s`; 4 Newton steps then converge to
/// machine-epsilon for the smooth UI curves we use.
const LUT_BUCKETS: usize = 64;
/// Newton-Raphson refinement iterations after the LUT seed.
const NEWTON_ITERS: usize = 4;
/// Below this `x'(s)` slope Newton is unstable; fall back to bisection.
const MIN_SLOPE: f32 = 1e-4;

/// Solve `x(s) = t` for `s ∈ [0, 1]`, then return `y(s)`.
fn cubic_bezier_sample(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    // Exact endpoints — no solve needed, and avoids any rounding drift at the
    // boundaries the unit tests pin.
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // A linear-in-x curve (x1 == x2 == ... degenerate) still resolves through
    // the generic path; the seed + Newton handle it.
    let s = solve_x(x1, x2, t);
    bezier_coord(y1, y2, s)
}

/// Invert `x(s) = t`. Seed from a coarse uniform LUT bucket, refine with
/// Newton-Raphson, and fall back to bisection where the slope is too flat for
/// Newton to be reliable.
fn solve_x(x1: f32, x2: f32, t: f32) -> f32 {
    // LUT seed: find the bucket whose x-range brackets t, then linearly
    // interpolate s within it. x(s) is monotonic for x1,x2 ∈ [0,1].
    let mut s = {
        let step = 1.0 / LUT_BUCKETS as f32;
        let mut seed = 0.0;
        let mut prev_x = 0.0;
        for i in 1..=LUT_BUCKETS {
            let s_i = i as f32 * step;
            let x_i = bezier_coord(x1, x2, s_i);
            if x_i >= t {
                let prev_s = (i - 1) as f32 * step;
                let span = x_i - prev_x;
                seed = if span > f32::EPSILON {
                    prev_s + (t - prev_x) / span * step
                } else {
                    prev_s
                };
                break;
            }
            prev_x = x_i;
            seed = s_i;
        }
        seed
    };

    // Newton-Raphson refinement.
    for _ in 0..NEWTON_ITERS {
        let x = bezier_coord(x1, x2, s) - t;
        if x.abs() < 1e-6 {
            return s.clamp(0.0, 1.0);
        }
        let dx = bezier_coord_deriv(x1, x2, s);
        if dx.abs() < MIN_SLOPE {
            break; // flat region — switch to bisection below.
        }
        s -= x / dx;
    }

    // Bisection fallback (always converges; bounded iterations).
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    let mut s = s.clamp(lo, hi);
    for _ in 0..32 {
        let x = bezier_coord(x1, x2, s);
        if (x - t).abs() < 1e-6 {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = 0.5 * (lo + hi);
    }
    s.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_identity() {
        let e = Easing::Linear;
        for &t in &[0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            assert!((e.sample(t) - t).abs() < 1e-6, "linear at {t}");
        }
    }

    #[test]
    fn clamps_out_of_range_input() {
        assert_eq!(Easing::DESIGN.sample(-1.0), 0.0);
        assert_eq!(Easing::DESIGN.sample(2.0), 1.0);
        assert_eq!(Easing::Linear.sample(-0.5), 0.0);
        assert_eq!(Easing::Linear.sample(1.5), 1.0);
    }

    /// The design curve `cubic-bezier(.2,.8,.2,1)` at the pinned samples.
    /// Endpoints are exact; the midpoint is solved.
    #[test]
    fn design_curve_pinned_samples() {
        let e = Easing::DESIGN;
        assert!((e.sample(0.0) - 0.0).abs() < 1e-6, "t=0");
        assert!((e.sample(1.0) - 1.0).abs() < 1e-6, "t=1");

        // Reference: solve x(s)=0.5 for cubic-bezier(.2,.8,.2,1) then eval y(s).
        // This is a strongly front-loaded ease-out curve. Independently
        // verified by a reference bisection solve (x controls .2/.2, y controls
        // .8/1.0): s≈0.7245, y(s)≈0.94608. Allow a small tolerance for the
        // iterative solve. Reference samples at t=.25/.5/.75 are
        // 0.7673 / 0.9461 / 0.9911.
        let mid = e.sample(0.5);
        assert!(
            (mid - 0.94608).abs() < 0.005,
            "design curve y at t=0.5 was {mid}, expected ~0.94608"
        );
        assert!(
            (e.sample(0.25) - 0.76728).abs() < 0.005,
            "design curve y at t=0.25 was {}, expected ~0.76728",
            e.sample(0.25)
        );
        assert!(
            (e.sample(0.75) - 0.99111).abs() < 0.005,
            "design curve y at t=0.75 was {}, expected ~0.99111",
            e.sample(0.75)
        );
        // Sanity: the curve is decidedly front-loaded.
        assert!(mid > 0.7, "design curve should be front-loaded, got {mid}");
    }

    /// `cubic-bezier(.2,.8,.2,1)` must be monotonic non-decreasing over `[0,1]`
    /// (no overshoot for this curve), and bracket its endpoints.
    #[test]
    fn design_curve_is_monotonic() {
        let e = Easing::DESIGN;
        let mut prev = e.sample(0.0);
        for i in 1..=200 {
            let t = i as f32 / 200.0;
            let y = e.sample(t);
            assert!(
                y >= prev - 1e-4,
                "non-monotonic at t={t}: {y} < prev {prev}"
            );
            assert!((0.0..=1.0001).contains(&y), "out of range at t={t}: {y}");
            prev = y;
        }
    }

    /// The CSS `ease` keyword should round-trip its endpoints and stay
    /// in-range / monotonic.
    #[test]
    fn ease_keyword_well_formed() {
        let e = Easing::EASE;
        assert!((e.sample(0.0)).abs() < 1e-6);
        assert!((e.sample(1.0) - 1.0).abs() < 1e-6);
        let mut prev = 0.0;
        for i in 0..=100 {
            let y = e.sample(i as f32 / 100.0);
            assert!(y >= prev - 1e-4, "ease non-monotonic");
            prev = y;
        }
    }

    /// `x(solve_x(...)) ≈ t` — the inversion is accurate.
    #[test]
    fn solve_x_inverts_accurately() {
        let (x1, x2) = (0.2_f32, 0.2_f32);
        for i in 1..100 {
            let t = i as f32 / 100.0;
            let s = solve_x(x1, x2, t);
            let x = bezier_coord(x1, x2, s);
            assert!((x - t).abs() < 1e-3, "solve_x off at t={t}: x(s)={x}");
        }
    }
}
