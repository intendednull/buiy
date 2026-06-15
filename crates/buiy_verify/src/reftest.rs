//! Tier 4 — reftests + the CPU-vs-GPU SDF cross-check (reftests.md).
//!
//! A reftest renders a `test` and a `reference` scene with the SAME engine in
//! ONE process and asserts their bitmaps match (`==`) or differ (`!=`), never
//! against a stored baseline — so every platform-variance term (driver SDF
//! rounding, glyph-atlas AA, sRGB encode, clock) cancels in the diff. The
//! harness stores ZERO bytes. GPU-coupled cases are `#[ignore]`; the pairing /
//! aggregation logic and the independence lint are pure-CPU and gate headless.

/// Whether a [`RefCase`] passes on equality or on difference.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefKind {
    /// Pass iff `test` and `reference` render to the same bitmap within `fuzz`.
    Match,
    /// Pass iff they render DIFFERENTLY (a `!=` anti-test guards silent no-ops).
    Mismatch,
}

impl RefKind {
    /// Parse the `reftest!` macro's kind token (`stringify!($kind)`).
    /// Panics on any other token — the macro only ever passes these two.
    pub fn reftest_kind(token: &str) -> Self {
        match token {
            "match" => RefKind::Match,
            "mismatch" => RefKind::Mismatch,
            other => panic!("reftest! kind must be `match` or `mismatch`, got `{other}`"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reftest_kind_parses_both_tokens() {
        assert_eq!(RefKind::reftest_kind("match"), RefKind::Match);
        assert_eq!(RefKind::reftest_kind("mismatch"), RefKind::Mismatch);
    }

    #[test]
    #[should_panic(expected = "must be `match` or `mismatch`")]
    fn reftest_kind_rejects_garbage() {
        let _ = RefKind::reftest_kind("nope");
    }
}
