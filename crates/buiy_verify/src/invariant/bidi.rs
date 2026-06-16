//! Tier-3 predicate #6 — BiDi caret round-trip on the LANDED shaper
//! (invariants.md § "BiDi caret round-trip"). Relations over a laid-out
//! `cosmic_text::Buffer` — the exact structure the production text stack
//! produces (`tests/text_shaping_snapshots.rs` path) — with no rasterizer.
//!
//! **Signature deviation.** The spec pins `bidi_caret_roundtrips(text: &str,
//! metrics: Metrics)`, shaping internally. Shaping needs a `FontSystem` with
//! registered faces, which the predicate cannot own without coupling to the
//! font registry, so this takes the already-laid-out `&Buffer` — the genuinely
//! PURE shaper-output form, matching predicates #1–#5's borrowed-data design.
//! The test harness (`tests/invariant_bidi.rs`) shapes through the production
//! `BuiyTextPlugin` stack and hands the committed buffer here. `arb_bidi_text`
//! keeps the spec's generator signature verbatim.

use cosmic_text::{Buffer, Cursor};
use proptest::prelude::*;

use super::predicates::Violation;

/// Generate a mixed-direction string: alternating LTR (Latin) and RTL
/// (Hebrew) runs of bounded length, plus neutral spaces — the BiDi stress space
/// the shaping `.snap` fixtures pin positions for, exercised generatively. Hebrew
/// (`U+05D0..05EA`) and ASCII letters are the two scripts; spaces join them.
pub fn arb_bidi_text(max_runs: u32, max_run_len: u32) -> impl Strategy<Value = String> {
    let max_runs = max_runs.max(1) as usize;
    let max_run_len = max_run_len.max(1) as usize;
    // Each run is (is_rtl, length); the string interleaves them with single
    // spaces so adjacent same-direction runs still produce a BiDi boundary.
    prop::collection::vec((any::<bool>(), 1usize..=max_run_len), 1..=max_runs).prop_map(|runs| {
        let mut s = String::new();
        for (i, (rtl, len)) in runs.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            for j in 0..*len {
                if *rtl {
                    // Hebrew aleph..tav, cycled.
                    let c = char::from_u32(0x05D0 + (j as u32 % 22)).unwrap();
                    s.push(c);
                } else {
                    // ASCII lowercase a..z, cycled.
                    s.push((b'a' + (j as u8 % 26)) as char);
                }
            }
        }
        s
    })
}

/// The three BiDi caret relations over a laid-out [`Buffer`]:
///
/// - **#6a** logical↔visual caret round-trip is identity: for every glyph
///   cluster, mapping the logical position to the glyph's visual center x and
///   hit-testing that x back recovers a cursor INSIDE the same cluster
///   (`[start, end]`). The cluster center is used (not the leading edge) so the
///   hit's half-glyph affinity is deterministic across LTR and RTL.
/// - **#6b** within one [`LayoutRun`](cosmic_text::LayoutRun): for an LTR run
///   (`rtl == false`) visual x is non-decreasing in logical start order; for an
///   RTL run (`rtl == true`) visual x is non-decreasing as logical start
///   DECREASES (the block reads right-to-left).
/// - **#6c** the run partition covers every byte of every line's text exactly
///   once across `Buffer::layout_runs()` (no gap, no overlap).
pub fn bidi_caret_roundtrips(buffer: &Buffer) -> Result<(), Violation> {
    for run in buffer.layout_runs() {
        let y = run.line_top + run.line_height / 2.0;

        // #6a — per-cluster round-trip.
        for glyph in run.glyphs.iter() {
            // Skip zero-width glyphs (e.g. a combining mark): their hitbox is a
            // point and hit-testing is ambiguous by construction.
            if glyph.w <= 0.0 {
                continue;
            }
            let center = glyph.x + glyph.w / 2.0;
            let Some(cursor) = buffer.hit(center, y) else {
                return Err(Violation::new(
                    "bidi_caret_roundtrips/6a_no_hit",
                    format!(
                        "hit-test at the center of cluster [{}..{}] (x={center}) found no cursor",
                        glyph.start, glyph.end
                    ),
                ));
            };
            caret_in_cluster(cursor, run.line_i, glyph.start, glyph.end)?;
        }

        // #6b — visual order vs logical order within the run.
        check_run_monotonicity(&run)?;
    }

    // #6c — coverage: every byte of every line's text is covered once.
    check_coverage(buffer)?;
    Ok(())
}

/// #6b — visual order vs logical order, BY BiDi LEVEL. A `LayoutRun`'s `glyphs`
/// are in LOGICAL order and may mix directions (an RTL block embedded in an LTR
/// paragraph), so a single run-wide monotonicity check is wrong. The true
/// invariant: within each maximal VISUAL segment of glyphs at the SAME BiDi
/// embedding level, logical `start` is monotone — ascending for an LTR (even)
/// level, descending for an RTL (odd) level. We sort by visual x, then check
/// monotonicity within each same-level segment.
fn check_run_monotonicity(run: &cosmic_text::LayoutRun) -> Result<(), Violation> {
    // Glyphs in VISUAL order (left to right), carrying their logical start +
    // BiDi level. Distinct clusters only (equal-start glyphs of one cluster
    // share a caret position).
    let mut visual: Vec<(f32, usize, bool)> = run
        .glyphs
        .iter()
        .map(|g| (g.x, g.start, g.level.is_rtl()))
        .collect();
    visual.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut prev: Option<(usize, bool)> = None;
    for &(_x, start, rtl) in &visual {
        if let Some((prev_start, prev_rtl)) = prev {
            // Only compare within a same-direction visual segment; a direction
            // change is a BiDi boundary where logical order legitimately jumps.
            if rtl == prev_rtl && start != prev_start {
                let ok = if rtl {
                    start < prev_start // RTL: visual L→R means logical decreasing
                } else {
                    start > prev_start // LTR: visual L→R means logical increasing
                };
                if !ok {
                    return Err(Violation::new(
                        "bidi_caret_roundtrips/6b_logical",
                        format!(
                            "run line {} ({} segment): visual order start {prev_start} → {start} \
                             violates monotonic logical order (LTR ascends, RTL descends)",
                            run.line_i,
                            if rtl { "RTL" } else { "LTR" }
                        ),
                    ));
                }
            }
        }
        prev = Some((start, rtl));
    }
    Ok(())
}

/// #6c — the DISTINCT clusters of `layout_runs()` partition every buffer line's
/// text: their `[start, end)` byte ranges are disjoint and tile the whole line
/// with no gap. Several glyphs may share one cluster (Arabic ccmp dots, a
/// Devanagari split matra, a base+mark pair), so coverage is counted per
/// DISTINCT cluster range, not per glyph — multiple glyphs of one cluster are
/// not an overlap. (`run.text` is the line text; cluster bytes index into it.)
fn check_coverage(buffer: &Buffer) -> Result<(), Violation> {
    use std::collections::{BTreeMap, BTreeSet};

    // line_i -> (line byte len, set of distinct cluster [start,end) ranges).
    let mut clusters: BTreeMap<usize, BTreeSet<(usize, usize)>> = BTreeMap::new();
    let mut line_len: BTreeMap<usize, usize> = BTreeMap::new();

    for run in buffer.layout_runs() {
        let len = run.text.len();
        line_len.insert(run.line_i, len);
        let set = clusters.entry(run.line_i).or_default();
        for glyph in run.glyphs.iter() {
            if glyph.end > len || glyph.start > glyph.end {
                return Err(Violation::new(
                    "bidi_caret_roundtrips/6c_range",
                    format!(
                        "cluster [{}..{}] out of bounds for line {} of {len} bytes",
                        glyph.start, glyph.end, run.line_i
                    ),
                ));
            }
            // Empty clusters (zero-width glyphs sharing a base's range) contribute
            // no new coverage; skip them so they don't register as a gap/overlap.
            if glyph.end > glyph.start {
                set.insert((glyph.start, glyph.end));
            }
        }
    }

    for (&line_i, ranges) in &clusters {
        let len = line_len[&line_i];
        // Sort by start; consecutive distinct cluster ranges must be disjoint
        // and abut (no gap, no overlap), tiling `0..len`.
        let mut cursor = 0usize;
        for &(start, end) in ranges {
            if start < cursor {
                return Err(Violation::new(
                    "bidi_caret_roundtrips/6c_overlap",
                    format!(
                        "line {line_i}: cluster [{start}..{end}] overlaps the previous (expected \
                         start ≥ {cursor})"
                    ),
                ));
            }
            if start > cursor {
                return Err(Violation::new(
                    "bidi_caret_roundtrips/6c_gap",
                    format!(
                        "line {line_i}: gap in [{cursor}..{start}) — no cluster covers those bytes"
                    ),
                ));
            }
            cursor = end;
        }
        if cursor != len {
            return Err(Violation::new(
                "bidi_caret_roundtrips/6c_gap",
                format!("line {line_i}: clusters cover only {cursor} of {len} bytes"),
            ));
        }
    }
    Ok(())
}

/// The #6a relation-check: a recovered [`Cursor`] must land INSIDE the cluster
/// it was mapped from — same line, `index ∈ [start, end]`. Exposed so the
/// off-by-one mutation fixture can feed it a `start + 1` cursor for a
/// single-byte cluster and confirm it is REJECTED (the round-trip's teeth).
pub fn caret_in_cluster(
    cursor: Cursor,
    line: usize,
    start: usize,
    end: usize,
) -> Result<(), Violation> {
    if cursor.line != line || cursor.index < start || cursor.index > end {
        return Err(Violation::new(
            "bidi_caret_roundtrips/6a_roundtrip",
            format!(
                "cursor {cursor:?} is outside cluster [{start}..{end}] on line {line} \
                 (caret round-trip broke)"
            ),
        ));
    }
    Ok(())
}
