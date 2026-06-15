//! The e2e golden-image harness (gate #2). The only proof of pixels, so its
//! reliability is load-bearing (verification.md § 4). This module owns the
//! device-free pieces — the flake-mitigation triad config (§ 4.3), the
//! perceptual-diff metric + tolerance-budget seam (§ 4.2), and the human-
//! curated `--accept` workflow flag (§ 4.4). The capture itself runs only on
//! the canonical CI GPU class (§ 4.1) behind an `#[ignore]` test.
//!
//! Per-fixture tolerance/perf/leak *numbers* are owned by
//! `buiy-verification-design`; this module commits to *having* a budget, not
//! its value.

use crate::render::atlas::{AtlasKey, AtlasWarmupQueue, BuiyAtlas};

/// Deterministic-capture configuration. The three flake sources of § 4.3 are
/// *necessary together*: a golden captured without all three is not
/// reproducible. `accept` is the § 4.4 human-curated golden-update gate —
/// never an automatic overwrite.
#[derive(Clone, Copy, Debug)]
pub struct GoldenConfig {
    /// Drive time from a fixed/virtual clock, not wall time, so any time-
    /// dependent visual is captured at a deterministic instant (§ 4.3.1).
    pub fixed_clock: bool,
    /// Block capture until every referenced font is loaded and its glyphs are
    /// resident (§ 4.3.2) — a half-loaded font flips the diff.
    pub wait_for_fonts: bool,
    /// Warm the texture atlas (glyphs/icons/gradients) before capture (§ 4.3.3)
    /// so first-frame upload latency does not perturb the image. Also
    /// establishes the gate-#15 steady-state baseline.
    pub warm_atlas: bool,
    /// `--accept`: update the stored golden instead of failing on mismatch.
    /// Off by default; gated behind human PR review (§ 4.4).
    pub accept: bool,
}

impl GoldenConfig {
    /// The capture config with the full flake-mitigation triad pinned and
    /// `accept` off — the configuration every golden is captured under.
    pub fn deterministic() -> Self {
        Self {
            fixed_clock: true,
            wait_for_fonts: true,
            warm_atlas: true,
            accept: false,
        }
    }
}

/// **Canonical device-pixel-ratio type.** Integer *milliscale* (1000 = 1.0×,
/// 2000 = 2.0×) so it is `Eq + Hash + Ord` without float pitfalls — it is a
/// *fixture axis* that keys a golden / coverage cell, **never** a tolerance.
///
/// Defined ONCE here; `buiy_verify::golden::GoldenKey.dpr` and
/// `buiy_verify::coverage::{Matrix.dprs, CoverageKey.dpr}` import this type,
/// they do **not** redefine it (verification-design `determinism.md`). The
/// capture boundary converts the window's `f32` `scale_factor` via
/// [`Dpr::from_f32`] and back via [`Dpr::as_f32`] when sizing the offscreen
/// target. Derives `serde` so the golden bless ledger can persist it directly.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
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

/// Perceptual difference between two RGBA8 frames, as a normalized mean
/// per-channel difference in `[0.0, 1.0]` (0 == identical). Comparison is
/// *perceptual*, not exact byte equality (§ 4.2): sub-LSB float jitter in the
/// SDF and linear→sRGB encode is invisible but not bit-stable, so the caller
/// compares this against an explicit per-fixture tolerance budget (owned by
/// `buiy-verification-design`) — the budget is the line between jitter and
/// regression. Frames must be the same length (same dimensions); mismatched
/// lengths return `1.0` (maximal difference).
pub fn perceptual_diff(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let sum: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| (x as f64 - y as f64).abs())
        .sum();
    (sum / (a.len() as f64 * 255.0)) as f32
}

/// verification § 3.2 — [`GoldenConfig::wait_for_fonts`], flipped from
/// declared flag to implemented predicate. With embedded deterministic
/// fonts, registration is synchronous at `FontSystem` construction (nothing
/// asynchronous exists to wait on), so "fonts ready" reduces to: the warmup
/// queue is drained AND every glyph key the fixture's producer emitted is
/// resident — probed via the **no-LRU-touch** [`BuiyAtlas::get`], so the
/// check never perturbs eviction order.
///
/// § 3.3 (`warm_atlas`) is satisfied STRUCTURALLY for text fixtures: the
/// producer inserts at extract, before Prepare's upload and the node's draw
/// (glyph-pipeline § 6.4), so by the time this predicate holds the atlas is
/// warm. `AtlasWarmupQueue` remains the seam for the optional production
/// ASCII pre-warm (rejected — text campaign T9; architecture § 2.3) and T6's
/// solid stamp.
pub fn fonts_ready(
    atlas: &BuiyAtlas,
    warmup: &AtlasWarmupQueue,
    visible_keys: &[AtlasKey],
) -> bool {
    warmup.is_empty() && visible_keys.iter().all(|key| atlas.get(key).is_some())
}

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
