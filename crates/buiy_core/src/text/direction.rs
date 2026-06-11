//! The § 5.4 direction pre-pass: strong-mark prepend (measure-and-layout
//! § 5.4). Runs AFTER `collapse_whitespace` (the trim must see authored
//! edges, never the mark) and BEFORE the resolver/`set_text`.
//!
//! **Editing consequence (successor campaign, § 5.4):** the mark shifts
//! every marked line's byte offsets by its UTF-8 length (3 bytes) —
//! hit-testing and cursor↔source mapping must map through the same
//! pre-pass offset table as the collapse transform.

use std::borrow::Cow;

use super::components::TextDirection;

/// U+200E LEFT-TO-RIGHT MARK / U+200F RIGHT-TO-LEFT MARK.
const LRM: char = '\u{200E}';
const RLM: char = '\u{200F}';

/// Prepend the strong mark per NON-EMPTY line (cosmic treats each buffer
/// line as a UAX #9 paragraph, so P2 runs per line). Empty lines stay
/// unmarked — a shaped mark could grow a phantom glyph and flip the
/// glyphs-keyed `ResolvedBaseline` semantics for empty text (T5 plan
/// decision 10). `Auto` borrows through: the steady path allocates nothing.
pub fn prepend_strong_marks(text: &str, dir: TextDirection) -> Cow<'_, str> {
    let mark = match dir {
        TextDirection::Auto => return Cow::Borrowed(text),
        TextDirection::Ltr => LRM,
        TextDirection::Rtl => RLM,
    };
    let lines = text.split('\n');
    let mut out = String::with_capacity(text.len() + 4 * (text.matches('\n').count() + 1));
    for (i, line) in lines.enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if !line.is_empty() {
            out.push(mark);
        }
        out.push_str(line);
    }
    Cow::Owned(out)
}
