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
    /// Set only by the dimension-mismatch sentinel. A saturated `Diff` is an
    /// *unconditional fail*: [`Diff::passes`] returns `false` for it against
    /// EVERY budget — including a hypothetical maximal `(255, u32::MAX)` — so a
    /// mis-sized capture reds the gate loudly (metric.md § compare). It is
    /// distinct from an in-bounds all-different frame, which a wide-enough
    /// budget may legitimately accept.
    pub saturated: bool,
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
            saturated: false,
        };
    }
    if a.dimensions() != b.dimensions() {
        // Loud-red sentinel (metric.md): a saturated Diff fails EVERY budget.
        // total = max(area) so the saturation count is well-defined.
        let total = a
            .width()
            .saturating_mul(a.height())
            .max(b.width().saturating_mul(b.height()));
        return Diff {
            differing_pixels: total,
            max_channel_delta: 255,
            total_pixels: total,
            mssim: Some(0.0),
            diff_image: None,
            saturated: true,
        };
    }
    let (w, h) = a.dimensions();
    let total_pixels = w * h;
    let max_delta = 35_215_f64 * opts.threshold * opts.threshold;

    let mut diff_image = opts.emit_diff_image.then(|| RgbaImage::new(w, h));
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
    }

    let mssim = if opts.mssim {
        // Advisory MSSIM via image-compare's rgba blended hybrid compare,
        // premultiplied against an opaque (white) background — captures are
        // opaque, so the background is never sampled in practice.
        use image_compare::{BlendInput, rgba_blended_hybrid_compare};
        let bg = image::Rgb([255u8, 255, 255]);
        rgba_blended_hybrid_compare(BlendInput::from(a), BlendInput::from(b), bg)
            .map(|sim| sim.score)
            .ok()
    } else {
        None
    };

    Diff {
        differing_pixels,
        max_channel_delta,
        total_pixels,
        mssim,
        diff_image,
        saturated: false,
    }
}

impl Diff {
    /// PASS iff `max_channel_delta <= budget.max_channel_delta`
    /// AND `differing_pixels <= budget.max_diff_pixels`. MSSIM is advisory and
    /// never gates here. A [`saturated`](Self::saturated) (dimension-mismatch)
    /// Diff is an unconditional fail — `false` for every budget, including a
    /// maximal `(255, u32::MAX)` — so a mis-sized capture cannot squeak through.
    pub fn passes(&self, budget: &FuzzBudget) -> bool {
        !self.saturated
            && self.max_channel_delta <= budget.max_channel_delta
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

    /// An antialiased vertical edge — black | one gray AA column | white —
    /// whose gray column value JITTERS between `a` and `b`, modeling the
    /// sub-LSB SDF/sRGB re-rasterization the metric must tolerate. Every
    /// differing (gray) pixel has a strictly brighter (white) and strictly
    /// darker (black) horizontal sibling, and those extremes have 3+ identical
    /// siblings in both images, so pixelmatch's slope detector reads them as AA.
    /// A hard 2-tone edge would NOT work: a pure black/white step has no pixel
    /// with both a brighter and a darker neighbor, so pixelmatch (correctly)
    /// never classifies it as AA.
    fn aa_edge_pair() -> (image::RgbaImage, image::RgbaImage) {
        let (w, h) = (16u32, 16u32);
        let build = |gray: u8| {
            let mut img = image::RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p = if x < 7 {
                        [0, 0, 0, 255]
                    } else if x == 7 {
                        [gray, gray, gray, 255]
                    } else {
                        [255, 255, 255, 255]
                    };
                    img.put_pixel(x, y, image::Rgba(p));
                }
            }
            img
        };
        // The gray AA column is sampled at 128 in `a`, 180 in `b` — sub-edge
        // jitter, above the YIQ threshold so the pixels are over-threshold but
        // AA-excluded.
        (build(128), build(180))
    }

    #[test]
    fn aa_pixels_excluded_by_default_but_counted_with_include_aa() {
        let (a, b) = aa_edge_pair();
        let excluded = compare(
            &a,
            &b,
            &CompareOpts {
                mssim: false,
                ..Default::default()
            },
        );
        let counted = compare(
            &a,
            &b,
            &CompareOpts {
                include_aa: true,
                mssim: false,
                ..Default::default()
            },
        );
        assert_eq!(
            excluded.differing_pixels, 0,
            "edge pixels read as AA, excluded"
        );
        assert!(
            counted.differing_pixels > 0,
            "include_aa counts the same pixels"
        );
    }

    #[test]
    fn real_defect_is_not_excluded_as_aa() {
        // An isolated wrong pixel on a flat field has no brighter+darker sibling
        // pair, so it is NOT AA — it must still count with default opts.
        let a = solid(16, 16, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(8, 8, image::Rgba([200, 200, 200, 255]));
        let d = compare(
            &a,
            &b,
            &CompareOpts {
                mssim: false,
                ..Default::default()
            },
        );
        assert_eq!(d.differing_pixels, 1, "isolated defect is not AA-excluded");
    }

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
        let d = compare(
            &img,
            &img,
            &CompareOpts {
                mssim: false,
                ..Default::default()
            },
        );
        assert_eq!(d.mssim, None);
    }

    #[test]
    fn mssim_never_gates() {
        // A global 1-LSB wash: 0 differing pixels (the YIQ delta 0.5 is far
        // under max_delta=352) but a measurably-below-1 MSSIM. Against a budget
        // that admits the 1-LSB L∞ channel delta the wash introduces, the diff
        // PASSES — proving MSSIM does not participate in the gate. (EXACT would
        // reject this on the *channel* axis, not because of MSSIM, so it cannot
        // isolate the property; the budget here tolerates the L∞ delta and 0
        // diff pixels, leaving only MSSIM as a possible gate — which must not
        // bind.)
        let a = solid(32, 32, [128, 128, 128, 255]);
        let b = solid(32, 32, [129, 129, 129, 255]);
        let d = compare(&a, &b, &CompareOpts::default());
        assert_eq!(
            d.differing_pixels, 0,
            "1-LSB shift is under the YIQ threshold"
        );
        assert_eq!(d.max_channel_delta, 1, "the wash is a 1-LSB L∞ delta");
        let s = d.mssim.expect("mssim computed by default");
        assert!(s < 1.0, "a uniform wash measurably lowers MSSIM below 1.0");
        let budget = FuzzBudget {
            max_channel_delta: 1,
            max_diff_pixels: 0,
        };
        assert!(
            d.passes(&budget),
            "MSSIM is advisory — a sub-1 MSSIM does not gate passes() when both \
             pixel axes are satisfied"
        );
    }

    #[test]
    fn diff_image_paints_differing_pixels() {
        let a = solid(8, 8, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(3, 3, image::Rgba([255, 255, 255, 255]));
        let d = compare(
            &a,
            &b,
            &CompareOpts {
                emit_diff_image: true,
                mssim: false,
                ..Default::default()
            },
        );
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

    #[test]
    fn passes_requires_both_axes() {
        // One pixel off by 255: trips max_channel_delta, one differing pixel.
        let a = solid(8, 8, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        let d = compare(
            &a,
            &b,
            &CompareOpts {
                mssim: false,
                ..Default::default()
            },
        );
        assert!(!d.passes(&FuzzBudget::EXACT), "EXACT rejects any diff");
        assert!(
            !d.passes(&FuzzBudget {
                max_channel_delta: 255,
                max_diff_pixels: 0
            }),
            "diff-pixel axis still binds when channel axis is satisfied"
        );
        assert!(
            !d.passes(&FuzzBudget {
                max_channel_delta: 0,
                max_diff_pixels: 1
            }),
            "channel axis still binds when diff-pixel axis is satisfied"
        );
        assert!(
            d.passes(&FuzzBudget {
                max_channel_delta: 255,
                max_diff_pixels: 1
            }),
            "both axes satisfied -> pass"
        );
    }

    #[test]
    fn within_floor_catches_unexpectedly_clean() {
        // A clean render (0,0) must FAIL a widened budget whose min floor is > 0.
        let a = solid(8, 8, [5, 5, 5, 255]);
        let clean = compare(
            &a,
            &a,
            &CompareOpts {
                mssim: false,
                ..Default::default()
            },
        );
        let min = FuzzBudget {
            max_channel_delta: 1,
            max_diff_pixels: 1,
        };
        let max = FuzzBudget {
            max_channel_delta: 10,
            max_diff_pixels: 50,
        };
        assert!(
            !clean.within(&min, &max),
            "a clean render is below the expected floor"
        );
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
        let maximal = FuzzBudget {
            max_channel_delta: 255,
            max_diff_pixels: u32::MAX,
        };
        assert!(
            !d.passes(&maximal),
            "saturated diff fails the loudest budget too"
        );
    }

    #[test]
    fn empty_capture_forbidden_by_explicit_assertion() {
        // The metric returns total_pixels == 0 for empty; harnesses forbid it.
        let e = image::RgbaImage::new(0, 0);
        let d = compare(&e, &e, &CompareOpts::default());
        assert_eq!(d.total_pixels, 0);
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
