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
    pub const EXACT: FuzzBudget = FuzzBudget {
        max_channel_delta: 0,
        max_diff_pixels: 0,
    };
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
        Self {
            threshold: 0.1,
            include_aa: false,
            mssim: true,
            emit_diff_image: false,
        }
    }
}

/// Compare two RGBA images. **Infallible** — returns a `Diff`, never a
/// `Result`. (AA exclusion is layered in 1a.3; here every over-threshold pixel
/// counts.)
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
        mssim: None,      // wired in 1a.5
        diff_image: None, // wired in 1a.6
    }
}

// ---- Vendored from pixelmatch (MIT). Verbatim constants; ported to image 0.25.
// "Measuring perceived color difference using YIQ NTSC transmission color space"
// (Kotsarenko & Ramos). `y_only` returns the signed luminance delta (used by the
// AA sibling test); otherwise the luminance-weighted YIQ squared delta, signed
// by which pixel is brighter.
fn color_delta(p1: &image::Rgba<u8>, p2: &image::Rgba<u8>, y_only: bool) -> f64 {
    let (mut r1, mut g1, mut b1, mut a1) = (p1[0] as f64, p1[1] as f64, p1[2] as f64, p1[3] as f64);
    let (mut r2, mut g2, mut b2, mut a2) = (p2[0] as f64, p2[1] as f64, p2[2] as f64, p2[3] as f64);

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

#[cfg(test)]
mod tests {
    use super::*;

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
            let d = compare(
                &a,
                &b,
                &CompareOpts {
                    include_aa: true,
                    mssim: false,
                    ..Default::default()
                },
            );
            assert_eq!(d.differing_pixels, 1, "N={n}: exactly one differing pixel");
            assert!(d.max_channel_delta >= 200, "N={n}: L∞ caught the 200 delta");
            assert_eq!(d.total_pixels, n * n);
        }
    }

    #[test]
    fn yiq_luminance_outweighs_chroma() {
        // Equal raw L∞ (delta 30 on a channel) but a luma-shifted pixel must
        // score a larger YIQ delta than a chroma-leaning shift — pins the
        // weighting. luma=+30 all channels (pure luminance, dY=-30); chroma=
        // +30 R / -30 B with G fixed (same L∞=30 but near-constant luminance,
        // dY=-5.5). At threshold 0.1 (max_delta=352) the luma delta (455) trips
        // while the lower-weighted chroma delta (244) does not — the YIQ
        // weighting, not L∞, is what separates them.
        let base = solid(4, 4, [120, 120, 120, 255]);
        let mut luma = base.clone();
        luma.put_pixel(0, 0, image::Rgba([150, 150, 150, 255])); // +30 all: pure luma
        let mut chroma = base.clone();
        chroma.put_pixel(0, 0, image::Rgba([150, 120, 90, 255])); // +30 R / -30 B: chroma-leaning, same L∞=30
        let opts = CompareOpts {
            include_aa: true,
            mssim: false,
            threshold: 0.1,
            ..Default::default()
        };
        let dl = compare(&base, &luma, &opts);
        let dc = compare(&base, &chroma, &opts);
        // At a threshold where luma trips but the lower-weighted chroma delta does
        // not, the luma case differs and the chroma case does not.
        assert_eq!(dl.differing_pixels, 1, "luma shift exceeds threshold");
        assert_eq!(
            dc.differing_pixels, 0,
            "chroma-leaning shift is under-weighted below threshold"
        );
    }

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
