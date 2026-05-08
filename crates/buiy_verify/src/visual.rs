//! Visual regression — perceptual diff with a tolerance budget.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #2).

use image::{DynamicImage, GenericImageView};

#[must_use]
pub struct DiffResult {
    /// 0.0 = identical, 1.0 = totally different.
    pub score: f64,
}

impl DiffResult {
    pub fn passed(&self, tolerance: f64) -> bool {
        self.score <= tolerance
    }
}

pub fn compare_images(a: &DynamicImage, b: &DynamicImage) -> DiffResult {
    if a.dimensions() != b.dimensions() {
        return DiffResult { score: 1.0 };
    }
    let a8 = a.to_rgba8();
    let b8 = b.to_rgba8();
    let mut accumulated = 0u64;
    // Widen u32 → u64 BEFORE multiplying. `width * height` in u32 overflows
    // for images > 4 gigapixels (theoretical, but cheap to harden).
    let pixels = a8.width() as u64 * a8.height() as u64;
    for (pa, pb) in a8.pixels().zip(b8.pixels()) {
        for ch in 0..4 {
            let d = pa[ch] as i32 - pb[ch] as i32;
            accumulated += (d * d) as u64;
        }
    }
    let max = (pixels * 4 * 255 * 255) as f64;
    DiffResult {
        score: (accumulated as f64 / max).sqrt(),
    }
}
