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
