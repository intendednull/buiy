//! The white-space collapse pre-pass (measure-and-layout § 5.2).
//!
//! cosmic-text lays out the source string VERBATIM, so CSS-default
//! collapsing must happen before `set_text` or measured widths include
//! literal runs of spaces. A pure `&str → Cow<str>` transform, run inside
//! `TextSync` immediately before `set_text`, parameterized by the collapse
//! mode. Rules per CSS Text Level 3 § 4.1 phase I.
//!
//! T2 always uses [`CollapseMode::Collapse`] (the `white-space: normal`
//! initial); T3 lands the white-space carrier and the full
//! (collapse mode × `Wrap`) value-table mapping, and the mode joins the
//! intrinsic-cache content version (measure § 3.2). The § 5.4 direction
//! strong-mark prepend (T5) runs AFTER this transform, so the trim sees the
//! authored leading/trailing spaces, never the mark.

use std::borrow::Cow;

/// CSS Text Level 3 § 4.1 phase-I collapse modes — the left column of the
/// § 5.2 white-space value table (`normal`/`nowrap` → `Collapse`;
/// `pre`/`pre-wrap` → `Preserve`; `pre-line` → `PreserveBreaks`). The
/// carrier component selecting a mode is T3's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollapseMode {
    /// Segment breaks (LF, CR, CRLF — normalized first) and tabs each
    /// become a collapsible space; runs of collapsible spaces collapse to
    /// one; leading and trailing collapsible spaces are trimmed. The result
    /// reaches cosmic-text as ONE logical line — soft wrapping, if any, is
    /// `Wrap`'s job.
    Collapse,
    /// Nothing collapses; segment breaks become hard line breaks
    /// (cosmic-text buffer lines, normalized to LF); tabs pass through
    /// untouched to cosmic-text's tab stops (`set_tab_width(8)` at
    /// `TextSync` — the CSS `tab-size` initial).
    Preserve,
    /// Segment breaks become hard line breaks; spaces and tabs collapse as
    /// in [`CollapseMode::Collapse`] within each segment.
    PreserveBreaks,
}

/// Apply the phase-I transform. Borrows through (`Cow::Borrowed`) when the
/// input needs no rewrite, so steady-state sync of plain words allocates
/// nothing.
pub fn collapse_whitespace(text: &str, mode: CollapseMode) -> Cow<'_, str> {
    match mode {
        CollapseMode::Collapse => collapse_all(text),
        CollapseMode::Preserve => normalize_segment_breaks(text),
        CollapseMode::PreserveBreaks => preserve_breaks(text),
    }
}

/// The collapsible set: spaces, tabs, and segment-break characters. NOT
/// other whitespace (U+00A0 no-break space etc. pass through).
fn is_collapsible(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// Does `text` need any collapse-mode rewrite? (Leading/trailing space,
/// any tab or break, or a multi-space run.)
fn needs_collapse(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.first() == Some(&b' ') || bytes.last() == Some(&b' ') {
        return true;
    }
    let mut prev_space = false;
    for &byte in bytes {
        match byte {
            b'\t' | b'\n' | b'\r' => return true,
            b' ' => {
                if prev_space {
                    return true;
                }
                prev_space = true;
            }
            _ => prev_space = false,
        }
    }
    false
}

fn collapse_all(text: &str) -> Cow<'_, str> {
    if !needs_collapse(text) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for c in text.chars() {
        if is_collapsible(c) {
            // A leading run never sets pending: it trims away. CRLF folds
            // naturally — both chars land in the same pending run.
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    // A trailing pending run is dropped: the trim.
    Cow::Owned(out)
}

/// CR and CRLF → LF, nothing else touched (the `Preserve` whole-mode and
/// the first step of `PreserveBreaks`).
fn normalize_segment_breaks(text: &str) -> Cow<'_, str> {
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

fn preserve_breaks(text: &str) -> Cow<'_, str> {
    let normalized = normalize_segment_breaks(text);
    if !normalized.split('\n').any(needs_collapse) {
        return normalized;
    }
    let rebuilt = normalized
        .split('\n')
        .map(collapse_all)
        .collect::<Vec<_>>()
        .join("\n");
    Cow::Owned(rebuilt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    /// The § 5.2 collapse rules: segment breaks (LF, CR, CRLF) and tabs
    /// each become a collapsible space; runs collapse to one.
    #[test]
    fn collapse_folds_breaks_tabs_and_runs_to_single_spaces() {
        assert_eq!(
            collapse_whitespace("hello\nworld", CollapseMode::Collapse),
            "hello world"
        );
        assert_eq!(collapse_whitespace("a\r\nb", CollapseMode::Collapse), "a b");
        assert_eq!(
            collapse_whitespace("a \t \n b", CollapseMode::Collapse),
            "a b"
        );
    }

    #[test]
    fn collapse_trims_leading_and_trailing() {
        assert_eq!(
            collapse_whitespace("  padded  ", CollapseMode::Collapse),
            "padded"
        );
        assert_eq!(
            collapse_whitespace("\n\tlead", CollapseMode::Collapse),
            "lead"
        );
        assert_eq!(collapse_whitespace(" \t\n ", CollapseMode::Collapse), "");
    }

    /// Steady-state typing over plain words must allocate nothing.
    #[test]
    fn collapse_borrows_through_when_already_collapsed() {
        assert!(matches!(
            collapse_whitespace("one two three", CollapseMode::Collapse),
            Cow::Borrowed(_)
        ));
    }

    /// Non-collapsible whitespace (U+00A0 no-break space) passes through —
    /// CSS phase I collapses only spaces, tabs, and segment breaks.
    #[test]
    fn nbsp_is_not_collapsible() {
        assert_eq!(
            collapse_whitespace("a\u{00A0} \u{00A0}b", CollapseMode::Collapse),
            "a\u{00A0} \u{00A0}b"
        );
        // The tab forces the rewrite path, so this guards `is_collapsible`
        // itself: NBSPs must survive `collapse_all`, not just the
        // `needs_collapse` fast path above.
        assert_eq!(
            collapse_whitespace("a\u{00A0}\t\u{00A0}b", CollapseMode::Collapse),
            "a\u{00A0} \u{00A0}b"
        );
    }

    /// `pre` / `pre-wrap`: nothing collapses; tabs pass through to the tab
    /// stops; segment breaks normalize to LF (hard buffer lines).
    #[test]
    fn preserve_keeps_spaces_and_tabs_normalizes_crlf() {
        assert_eq!(
            collapse_whitespace("a  b\tc", CollapseMode::Preserve),
            "a  b\tc"
        );
        assert_eq!(
            collapse_whitespace("a\r\nb\rc", CollapseMode::Preserve),
            "a\nb\nc"
        );
        assert!(matches!(
            collapse_whitespace("plain\nbreak", CollapseMode::Preserve),
            Cow::Borrowed(_)
        ));
    }

    /// `pre-line`: hard breaks survive; spaces/tabs collapse per segment.
    #[test]
    fn preserve_breaks_collapses_within_segments_keeps_breaks() {
        assert_eq!(
            collapse_whitespace("a  b\n  c\td  ", CollapseMode::PreserveBreaks),
            "a b\nc d"
        );
        assert_eq!(
            collapse_whitespace("x\r\ny", CollapseMode::PreserveBreaks),
            "x\ny"
        );
    }
}
